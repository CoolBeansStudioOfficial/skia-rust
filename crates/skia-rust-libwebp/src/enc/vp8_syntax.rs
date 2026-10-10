// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the VP8 frame syntax of libwebp 1.4.0: `src/enc/syntax_enc.c` (the RIFF and VP8
//! headers, the segment, filter and quantizer headers, `GeneratePartition0`, `VP8EncWrite`) and
//! `VP8CodeIntraModes` of `src/enc/tree_enc.c`.
//!
//! The file is the simple lossy layout: `RIFF`, `WEBP`, the `VP8 ` chunk with the 10-byte frame
//! header, partition 0, the partition sizes, the token partitions, and the padding byte. The
//! `VP8X` and `ALPH` chunks are for alpha pictures, which are not ported (see the lossy module).

// Clippy allows for the C arithmetic and control flow: the C code mixes int, uint32_t
// and uint8_t, spells table offsets as `0 + 0 * BPS`, nests the mode trees as `if` chains,
// and indexes by position. The port keeps those shapes so that each line can be checked
// against the C source; the casts are the width and sign conversions of the C source.
#![allow(
    clippy::identity_op,
    clippy::erasing_op,
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::too_many_arguments,
    clippy::bool_to_int_with_if,
    clippy::needless_range_loop,
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::cast_precision_loss,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::unreadable_literal,
    clippy::if_not_else,
    clippy::manual_range_contains,
    clippy::struct_excessive_bools,
    clippy::fn_params_excessive_bools,
    clippy::needless_pass_by_value,
    clippy::items_after_statements,
    clippy::float_cmp,
    clippy::int_plus_one,
    clippy::precedence,
    clippy::unusual_byte_groupings
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]

use super::vp8_bit_writer::VP8BitWriter;
use super::vp8_encoder::{NUM_MB_SEGMENTS, VP8EncIterator, VP8Encoder};
use super::vp8_tree::{
    bmode_probas, put_i4_mode, put_i16_mode, put_segment, put_uv_mode, write_probas,
};

/// Port of `RIFF_HEADER_SIZE`.
const RIFF_HEADER_SIZE: usize = 12;
/// Port of `CHUNK_HEADER_SIZE`.
const CHUNK_HEADER_SIZE: usize = 8;
/// Port of `VP8_FRAME_HEADER_SIZE`.
const VP8_FRAME_HEADER_SIZE: usize = 10;
/// Port of `VP8_SIGNATURE`.
const VP8_SIGNATURE: u32 = 0x9d_012a;
/// Port of `VP8_MAX_PARTITION0_SIZE`.
const VP8_MAX_PARTITION0_SIZE: usize = 1 << 19;
/// Port of `VP8_MAX_PARTITION_SIZE`.
const VP8_MAX_PARTITION_SIZE: usize = 1 << 24;

/// Port of `PutSegmentHeader`.
fn put_segment_header(bw: &mut VP8BitWriter, enc: &VP8Encoder) {
    let hdr = enc.segment_hdr;
    let proba = &enc.proba;
    if bw.put_bit_uniform(hdr.num_segments > 1) {
        let update_data = true;
        bw.put_bit_uniform(hdr.update_map != 0);
        if bw.put_bit_uniform(update_data) {
            bw.put_bit_uniform(true); // (segment_feature_mode = 1. Paragraph 9.3.)
            for s in 0..NUM_MB_SEGMENTS {
                bw.put_signed_bits(enc.dqm[s].quant, 7);
            }
            for s in 0..NUM_MB_SEGMENTS {
                bw.put_signed_bits(enc.dqm[s].fstrength, 6);
            }
        }
        if hdr.update_map != 0 {
            for s in 0..3 {
                if bw.put_bit_uniform(proba.segments[s] != 255) {
                    bw.put_bits(u32::from(proba.segments[s]), 8);
                }
            }
        }
    }
}

/// Port of `PutFilterHeader`.
fn put_filter_header(bw: &mut VP8BitWriter, enc: &VP8Encoder) {
    let hdr = enc.filter_hdr;
    let use_lf_delta = hdr.i4x4_lf_delta != 0;
    bw.put_bit_uniform(hdr.simple != 0);
    bw.put_bits(hdr.level as u32, 6);
    bw.put_bits(hdr.sharpness as u32, 3);
    if bw.put_bit_uniform(use_lf_delta) {
        let need_update = hdr.i4x4_lf_delta != 0;
        if bw.put_bit_uniform(need_update) {
            bw.put_bits(0, 4);
            bw.put_signed_bits(hdr.i4x4_lf_delta, 6);
            bw.put_bits(0, 3); // all others unused
        }
    }
}

/// Port of `PutQuant`.
fn put_quant(bw: &mut VP8BitWriter, enc: &VP8Encoder) {
    bw.put_bits(enc.base_quant as u32, 7);
    bw.put_signed_bits(enc.dq_y1_dc, 4);
    bw.put_signed_bits(enc.dq_y2_dc, 4);
    bw.put_signed_bits(enc.dq_y2_ac, 4);
    bw.put_signed_bits(enc.dq_uv_dc, 4);
    bw.put_signed_bits(enc.dq_uv_ac, 4);
}

/// Port of `VP8CodeIntraModes`: writes the segment, skip, and intra modes of every macroblock
/// into partition 0.
pub fn code_intra_modes(enc: &mut VP8Encoder) {
    let mut bw = std::mem::take(&mut enc.bw);
    let mut it = VP8EncIterator::new(enc);
    loop {
        let idx = it.mb_index(enc);
        let mb = enc.mb_info[idx];
        let preds = it.preds_index(enc);
        if enc.segment_hdr.update_map != 0 {
            put_segment(&mut bw, i32::from(mb.segment), &enc.proba.segments);
        }
        if enc.proba.use_skip_proba {
            bw.put_bit(mb.skip != 0, i32::from(enc.proba.skip_proba));
        }
        if bw.put_bit(mb.type_ != 0, 145) {
            // i16x16
            put_i16_mode(&mut bw, i32::from(enc.preds[preds]));
        } else {
            let preds_w = enc.preds_w;
            let mut top_pred = preds - preds_w;
            let mut p = preds;
            for _y in 0..4 {
                let mut left = i32::from(enc.preds[p - 1]);
                for x in 0..4 {
                    let probas = bmode_probas(i32::from(enc.preds[top_pred + x]), left);
                    left = put_i4_mode(&mut bw, i32::from(enc.preds[p + x]), probas);
                }
                top_pred = p;
                p += preds_w;
            }
        }
        put_uv_mode(&mut bw, i32::from(mb.uv_mode));
        if !it.next(enc) {
            break;
        }
    }
    enc.bw = bw;
}

/// Port of `GeneratePartition0`: writes partition 0 (the frame header, the probabilities, and the
/// macroblock modes), and finishes it.
fn generate_partition0(enc: &mut VP8Encoder) {
    let mut bw = VP8BitWriter::new();
    bw.put_bit_uniform(false); // colorspace
    bw.put_bit_uniform(false); // clamp type
    put_segment_header(&mut bw, enc);
    put_filter_header(&mut bw, enc);
    let parts_code = match enc.num_parts {
        8 => 3,
        4 => 2,
        2 => 1,
        _ => 0,
    };
    bw.put_bits(parts_code, 2);
    put_quant(&mut bw, enc);
    bw.put_bit_uniform(false); // no proba update
    write_probas(&mut bw, &enc.proba);
    enc.bw = bw;
    code_intra_modes(enc);
    enc.bw.finish_in_place();
}

/// Port of `VP8EncWrite`, for an opaque picture: the complete WebP file of the encoded frame.
/// Returns `None` where the C code reports an error (the partitions or the frame are too big).
#[must_use]
pub fn vp8_enc_write(enc: &mut VP8Encoder, width: usize, height: usize) -> Option<Vec<u8>> {
    generate_partition0(enc);
    let part0 = enc.bw.bytes().to_vec();
    let size0 = part0.len();
    let mut vp8_size = VP8_FRAME_HEADER_SIZE + size0 + 3 * (enc.num_parts - 1);
    for p in 0..enc.num_parts {
        vp8_size += enc.parts[p].size();
    }
    let pad = vp8_size & 1;
    vp8_size += pad;
    let riff_size = 4 + CHUNK_HEADER_SIZE + vp8_size;
    if riff_size > 0xffff_fffe {
        return None;
    }
    if size0 >= VP8_MAX_PARTITION0_SIZE {
        // partition #0 is too big to fit
        return None;
    }
    let mut out = Vec::with_capacity(RIFF_HEADER_SIZE + CHUNK_HEADER_SIZE + vp8_size);
    // PutRIFFHeader
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(riff_size as u32).to_le_bytes());
    out.extend_from_slice(b"WEBP");
    // PutVP8Header
    out.extend_from_slice(b"VP8 ");
    out.extend_from_slice(&(vp8_size as u32).to_le_bytes());
    // PutVP8FrameHeader: the profile (filter_type 1 gives 0), visible, and the partition 0 size.
    let profile = enc.profile as u32;
    let bits: u32 = (profile << 1) | (1 << 4) | ((size0 as u32) << 5);
    out.push((bits & 0xff) as u8);
    out.push(((bits >> 8) & 0xff) as u8);
    out.push(((bits >> 16) & 0xff) as u8);
    out.push(((VP8_SIGNATURE >> 16) & 0xff) as u8);
    out.push(((VP8_SIGNATURE >> 8) & 0xff) as u8);
    out.push((VP8_SIGNATURE & 0xff) as u8);
    out.push((width & 0xff) as u8);
    out.push((width >> 8) as u8);
    out.push((height & 0xff) as u8);
    out.push((height >> 8) as u8);
    out.extend_from_slice(&part0);
    // EmitPartitionsSize
    for p in 0..enc.num_parts - 1 {
        let part_size = enc.parts[p].size();
        if part_size >= VP8_MAX_PARTITION_SIZE {
            return None;
        }
        out.push((part_size & 0xff) as u8);
        out.push(((part_size >> 8) & 0xff) as u8);
        out.push(((part_size >> 16) & 0xff) as u8);
    }
    for p in 0..enc.num_parts {
        out.extend_from_slice(enc.parts[p].bytes());
    }
    if pad != 0 {
        out.push(0);
    }
    Some(out)
}
