// Copyright 2010 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the COPYING file. Port by The skia-rust Authors.

//! Port of libwebp's lossy (VP8) decoder: `src/dec/vp8_dec.c` (frame header, residuals),
//! `src/dec/tree_dec.c` (probabilities, intra modes), `src/dec/quant_dec.c` and
//! `src/dec/frame_dec.c` (reconstruction and loop filter).
//!
//! The decoder reconstructs each macroblock row into full-frame planes. Intra prediction reads only
//! unfiltered samples (`yuv_b` and `yuv_t`, as in C), so the loop filter can run over the planes
//! afterwards, MB by MB in the same order as libwebp's `FilterRow`. The output stage
//! ([`crate::output`]) emits rows in libwebp's batches. Dithering is not ported: Skia leaves
//! `dithering_strength` at 0.
//!
//! The input is the VP8 chunk payload, `buf`. Readers take `buf` on every call, so nothing here
//! owns or copies it.

// Module-level clippy allows. Each one mirrors the C source of this module.
// clippy::bool_to_int_with_if: mirrors the C if/else that selects the value.
// clippy::cast_possible_truncation: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_possible_wrap: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_sign_loss: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::if_not_else: keeps the branch order of the C source.
// clippy::many_single_char_names: pixel, offset and loop variables keep the single-letter names of the C source.
// clippy::misrefactored_assign_op: v += v + bit is the C GetLargeValue update (v = 2 * v + bit); the lint misreads it.
// clippy::needless_late_init: keeps the declaration order of the C source.
// clippy::needless_range_loop: the loop index mirrors the C loop, which indexes several arrays.
// clippy::range_plus_one: the range bounds mirror the C pointer arithmetic.
// clippy::similar_names: local names mirror the C identifiers, which differ by one letter or a suffix.
// clippy::struct_field_names: field names mirror the C struct members.
// clippy::too_many_arguments: the signature mirrors the parameters of the C function.
// clippy::too_many_lines: the function is one C function; splitting it would change the port structure.
// clippy::unreadable_literal: the constant is copied verbatim from the C source.
#![allow(
    clippy::bool_to_int_with_if,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::if_not_else,
    clippy::many_single_char_names,
    clippy::misrefactored_assign_op,
    clippy::needless_late_init,
    clippy::needless_range_loop,
    clippy::range_plus_one,
    clippy::similar_names,
    clippy::struct_field_names,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::bit_reader::VP8BitReader;
use crate::io::Status;
use crate::vp8_dsp as dsp;
use crate::vp8_tables::{BMODES_PROBA, COEFFS_PROBA0, COEFFS_UPDATE_PROBA};
use crate::vp8_tables_small::{
    B_DC_PRED, B_DC_PRED_NOLEFT, B_DC_PRED_NOTOP, B_DC_PRED_NOTOPLEFT, B_HD_PRED, B_HE_PRED,
    B_HU_PRED, B_LD_PRED, B_RD_PRED, B_TM_PRED, B_VE_PRED, B_VL_PRED, B_VR_PRED, BPS, DC_PRED,
    H_PRED, K_AC_TABLE, K_BANDS, K_CAT3, K_CAT4, K_CAT5, K_CAT6, K_DC_TABLE, K_FILTER_EXTRA_ROWS,
    K_SCAN, K_ZIGZAG, TM_PRED, U_OFF, V_OFF, V_PRED, Y_OFF, YUV_SIZE,
};

/// Port of `NUM_MB_SEGMENTS`.
pub const NUM_MB_SEGMENTS: usize = 4;
const NUM_TYPES: usize = 4;
const NUM_BANDS: usize = 8;
const NUM_CTX: usize = 3;
const NUM_PROBAS: usize = 11;
/// Port of `VP8_FRAME_HEADER_SIZE`.
pub const VP8_FRAME_HEADER_SIZE: usize = 10;

/// Port of `VP8GetInfo`: `(width, height)` of a key frame, or `None`.
#[doc(alias = "VP8GetInfo")]
#[must_use]
pub fn get_info(data: &[u8], chunk_size: usize) -> Option<(i32, i32)> {
    if data.len() < VP8_FRAME_HEADER_SIZE || !check_signature(&data[3..]) {
        return None;
    }
    let bits = u32::from(data[0]) | (u32::from(data[1]) << 8) | (u32::from(data[2]) << 16);
    let key_frame = (bits & 1) == 0;
    let w = i32::from(((u16::from(data[7]) << 8) | u16::from(data[6])) & 0x3fff);
    let h = i32::from(((u16::from(data[9]) << 8) | u16::from(data[8])) & 0x3fff);
    if !key_frame || ((bits >> 1) & 7) > 3 || ((bits >> 4) & 1) == 0 {
        return None;
    }
    if (bits >> 5) as usize >= chunk_size || w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

/// Port of `VP8CheckSignature`.
fn check_signature(data: &[u8]) -> bool {
    data.len() >= 3 && data[0] == 0x9d && data[1] == 0x01 && data[2] == 0x2a
}

/// Port of `VP8SegmentHeader`.
#[derive(Debug, Clone, Default)]
#[doc(alias = "VP8SegmentHeader")]
struct SegmentHeader {
    use_segment: bool,
    update_map: bool,
    absolute_delta: bool,
    quantizer: [i32; NUM_MB_SEGMENTS],
    filter_strength: [i32; NUM_MB_SEGMENTS],
}

/// Port of `VP8FilterHeader`.
#[derive(Debug, Clone, Default)]
#[doc(alias = "VP8FilterHeader")]
struct FilterHeader {
    simple: bool,
    level: i32,
    sharpness: i32,
    use_lf_delta: bool,
    ref_lf_delta: [i32; 4],
    mode_lf_delta: [i32; 4],
}

/// Port of `VP8FInfo`.
#[derive(Debug, Clone, Copy, Default)]
#[doc(alias = "VP8FInfo")]
struct FInfo {
    f_limit: i32,
    f_ilevel: i32,
    f_inner: bool,
    hev_thresh: i32,
}

/// Port of `VP8QuantMatrix` (without the dithering fields, which are unused here).
#[derive(Debug, Clone, Copy, Default)]
#[doc(alias = "VP8QuantMatrix")]
struct QuantMatrix {
    y1_mat: [i32; 2],
    y2_mat: [i32; 2],
    uv_mat: [i32; 2],
}

/// Port of `VP8MBData`.
#[derive(Debug, Clone)]
#[doc(alias = "VP8MBData")]
struct MbData {
    coeffs: [i16; 384],
    is_i4x4: bool,
    imodes: [u8; 16],
    uvmode: u8,
    non_zero_y: u32,
    non_zero_uv: u32,
    skip: bool,
    segment: u8,
}

impl Default for MbData {
    fn default() -> Self {
        Self {
            coeffs: [0; 384],
            is_i4x4: false,
            imodes: [0; 16],
            uvmode: 0,
            non_zero_y: 0,
            non_zero_uv: 0,
            skip: false,
            segment: 0,
        }
    }
}

/// Port of `VP8MB`: the top and left non-zero contexts.
#[derive(Debug, Clone, Copy, Default)]
#[doc(alias = "VP8MB")]
struct MbCtx {
    nz: u8,
    nz_dc: u8,
}

/// Port of `VP8TopSamples`.
#[derive(Debug, Clone, Copy, Default)]
struct TopSamples {
    y: [u8; 16],
    u: [u8; 8],
    v: [u8; 8],
}

/// Port of `VP8Proba`: segment probabilities and band probabilities `[type][band][ctx][proba]`.
#[derive(Debug, Clone)]
#[doc(alias = "VP8Proba")]
struct Proba {
    segments: [u8; 3],
    bands: [[[[u8; NUM_PROBAS]; NUM_CTX]; NUM_BANDS]; NUM_TYPES],
}

/// The decoded planes of a lossy frame, after reconstruction and the loop filter. Planes are
/// MB-aligned: `y` has `mb_w * 16` samples per row, `u` and `v` `mb_w * 8`.
#[derive(Debug, Clone, Default)]
pub struct Planes {
    pub width: i32,
    pub height: i32,
    pub mb_w: usize,
    pub mb_h: usize,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
    pub y_stride: usize,
    pub uv_stride: usize,
    /// `filter_type_`: 0 off, 1 simple, 2 complex.
    pub filter_type: u8,
}

/// Crop window of the caller (`io->crop_*`). The whole frame when not cropping.
#[derive(Debug, Clone, Copy)]
pub struct Crop {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Port of `ParseSegmentHeader`.
fn parse_segment_header(
    buf: &[u8],
    br: &mut VP8BitReader,
    hdr: &mut SegmentHeader,
    proba: &mut Proba,
) -> bool {
    hdr.use_segment = br.get(buf) != 0;
    if hdr.use_segment {
        hdr.update_map = br.get(buf) != 0;
        if br.get(buf) != 0 {
            // update data
            hdr.absolute_delta = br.get(buf) != 0;
            for s in 0..NUM_MB_SEGMENTS {
                hdr.quantizer[s] = if br.get(buf) != 0 {
                    br.get_signed_value(buf, 7)
                } else {
                    0
                };
            }
            for s in 0..NUM_MB_SEGMENTS {
                hdr.filter_strength[s] = if br.get(buf) != 0 {
                    br.get_signed_value(buf, 6)
                } else {
                    0
                };
            }
        }
        if hdr.update_map {
            for s in 0..3 {
                proba.segments[s] = if br.get(buf) != 0 {
                    br.get_value(buf, 8) as u8
                } else {
                    255
                };
            }
        }
    } else {
        hdr.update_map = false;
    }
    !br.eof()
}

/// Port of `ParseFilterHeader`. Returns `(ok, filter_type)`.
fn parse_filter_header(buf: &[u8], br: &mut VP8BitReader, hdr: &mut FilterHeader) -> (bool, u8) {
    hdr.simple = br.get(buf) != 0;
    hdr.level = br.get_value(buf, 6) as i32;
    hdr.sharpness = br.get_value(buf, 3) as i32;
    hdr.use_lf_delta = br.get(buf) != 0;
    if hdr.use_lf_delta && br.get(buf) != 0 {
        // update lf-delta?
        for i in 0..4 {
            if br.get(buf) != 0 {
                hdr.ref_lf_delta[i] = br.get_signed_value(buf, 6);
            }
        }
        for i in 0..4 {
            if br.get(buf) != 0 {
                hdr.mode_lf_delta[i] = br.get_signed_value(buf, 6);
            }
        }
    }
    let filter_type = if hdr.level == 0 {
        0
    } else if hdr.simple {
        1
    } else {
        2
    };
    (!br.eof(), filter_type)
}

/// Port of `GetLargeValue`.
fn get_large_value(br: &mut VP8BitReader, buf: &[u8], p: &[u8; NUM_PROBAS]) -> i32 {
    if br.get_bit(buf, i32::from(p[3])) == 0 {
        if br.get_bit(buf, i32::from(p[4])) == 0 {
            2
        } else {
            3 + br.get_bit(buf, i32::from(p[5]))
        }
    } else if br.get_bit(buf, i32::from(p[6])) == 0 {
        if br.get_bit(buf, i32::from(p[7])) == 0 {
            5 + br.get_bit(buf, 159)
        } else {
            let mut v = 7 + 2 * br.get_bit(buf, 165);
            v += br.get_bit(buf, 145);
            v
        }
    } else {
        let bit1 = br.get_bit(buf, i32::from(p[8]));
        let bit0 = br.get_bit(buf, i32::from(p[9 + bit1 as usize]));
        let cat = (2 * bit1 + bit0) as usize;
        let tab: &[u8] = match cat {
            0 => &K_CAT3,
            1 => &K_CAT4,
            2 => &K_CAT5,
            _ => &K_CAT6,
        };
        let mut v = 0i32;
        for &prob in tab {
            v += v + br.get_bit(buf, i32::from(prob));
        }
        v + 3 + (8 << cat)
    }
}

/// Port of `GetCoeffsFast`: decodes the coefficients of one block into `out` (the 16 values of the
/// block, in raster order through `K_ZIGZAG`). Returns the index after the last non-zero coefficient.
fn get_coeffs(
    br: &mut VP8BitReader,
    buf: &[u8],
    proba: &Proba,
    t: usize,
    ctx: usize,
    dq: [i32; 2],
    mut n: usize,
    out: &mut [i16],
) -> usize {
    let bands = &proba.bands[t];
    let mut p = bands[K_BANDS[n]][ctx];
    while n < 16 {
        if br.get_bit(buf, i32::from(p[0])) == 0 {
            return n; // previous coeff was last non-zero coeff
        }
        while br.get_bit(buf, i32::from(p[1])) == 0 {
            // sequence of zero coeffs
            n += 1;
            p = bands[K_BANDS[n]][0];
            if n == 16 {
                return 16;
            }
        }
        // non zero coeff
        let p_ctx = bands[K_BANDS[n + 1]]; // &prob[n + 1]->probas_[0]: the three contexts
        let v;
        if br.get_bit(buf, i32::from(p[2])) == 0 {
            v = 1;
            p = p_ctx[1];
        } else {
            v = get_large_value(br, buf, &p);
            p = p_ctx[2];
        }
        let s = br.get_signed(buf, v);
        out[K_ZIGZAG[n]] = (s * dq[usize::from(n > 0)]) as i16;
        n += 1;
    }
    16
}

/// Port of `NzCodeBits`.
#[inline]
fn nz_code_bits(nz_coeffs: u32, nz: usize, dc_nz: bool) -> u32 {
    let low = if nz > 3 {
        3
    } else if nz > 1 {
        2
    } else {
        u32::from(dc_nz)
    };
    (nz_coeffs << 2) | low
}

/// Port of `Clip(v, M)`.
#[inline]
fn clip(v: i32, m: i32) -> i32 {
    v.clamp(0, m)
}

/// Port of `CheckMode`: the DC predictor variant for edge macroblocks.
fn check_mode(mb_x: usize, mb_y: usize, mode: u8) -> u8 {
    if mode == B_DC_PRED {
        if mb_x == 0 {
            return if mb_y == 0 {
                B_DC_PRED_NOTOPLEFT
            } else {
                B_DC_PRED_NOLEFT
            };
        }
        return if mb_y == 0 {
            B_DC_PRED_NOTOP
        } else {
            B_DC_PRED
        };
    }
    mode
}

/// Port of `DoTransform`: picks the inverse transform from the two top bits of `bits`.
#[inline]
fn do_transform(bits: u32, src: &[i16], dst: &mut [u8], o: usize) {
    match bits >> 30 {
        3 => dsp::transform_one(src, dst, o),
        2 => dsp::transform_ac3(src, dst, o),
        1 => dsp::transform_dc(src, dst, o),
        _ => {}
    }
}

/// Port of `DoUVTransform`.
#[inline]
fn do_uv_transform(bits: u32, src: &[i16], dst: &mut [u8], o: usize) {
    if bits & 0xff != 0 {
        if bits & 0xaa != 0 {
            // any non-zero AC coefficient? (note: no AC3 variant for U/V)
            dsp::transform_uv(src, dst, o);
        } else {
            dsp::transform_dc_uv(src, dst, o);
        }
    }
}

/// Port of `VP8Decoder` (the fields the frame decode needs).
#[derive(Debug)]
#[doc(alias = "VP8Decoder")]
struct Decoder {
    br: VP8BitReader,
    parts: Vec<VP8BitReader>,
    num_parts_minus_one: usize,
    mb_w: usize,
    mb_h: usize,
    filter_hdr: FilterHeader,
    segment_hdr: SegmentHeader,
    filter_type: u8,
    dqm: [QuantMatrix; NUM_MB_SEGMENTS],
    proba: Proba,
    use_skip_proba: bool,
    skip_p: u8,
    intra_t: Vec<u8>,
    intra_l: [u8; 4],
    yuv_t: Vec<TopSamples>,
    mb_info: Vec<MbCtx>,
    mb_data: Vec<MbData>,
    f_info: Vec<FInfo>,
    fstrengths: [[FInfo; 2]; NUM_MB_SEGMENTS],
    yuv_b: Vec<u8>,
    planes: Planes,
}

impl Decoder {
    /// `VP8GetHeaders` (the key-frame part) and the setup of `VP8InitFrame`.
    fn new(buf: &[u8]) -> Result<Self, Status> {
        if buf.len() < 4 {
            return Err(Status::NotEnoughData);
        }
        let bits = u32::from(buf[0]) | (u32::from(buf[1]) << 8) | (u32::from(buf[2]) << 16);
        let key_frame = (bits & 1) == 0;
        let profile = (bits >> 1) & 7;
        let show = (bits >> 4) & 1;
        let partition_length = (bits >> 5) as usize;
        if profile > 3 {
            return Err(Status::BitstreamError);
        }
        if show == 0 {
            return Err(Status::UnsupportedFeature);
        }
        if !key_frame {
            return Err(Status::UnsupportedFeature);
        }
        let mut pos = 3usize;
        let mut buf_size = buf.len() - 3;
        if buf_size < 7 {
            return Err(Status::NotEnoughData);
        }
        if !check_signature(&buf[pos..]) {
            return Err(Status::BitstreamError);
        }
        let width = i32::from(((u16::from(buf[pos + 4]) << 8) | u16::from(buf[pos + 3])) & 0x3fff);
        let height = i32::from(((u16::from(buf[pos + 6]) << 8) | u16::from(buf[pos + 5])) & 0x3fff);
        pos += 7;
        buf_size -= 7;
        let mb_w = ((width + 15) >> 4) as usize;
        let mb_h = ((height + 15) >> 4) as usize;
        if partition_length > buf_size {
            return Err(Status::NotEnoughData);
        }
        let mut br = VP8BitReader::new(buf, pos, partition_length);
        pos += partition_length;
        buf_size -= partition_length;
        // colorspace and clamp type: read, not used by the decoder.
        let _ = br.get(buf);
        let _ = br.get(buf);

        let mut proba = Proba {
            segments: [255; 3],
            bands: [[[[0; NUM_PROBAS]; NUM_CTX]; NUM_BANDS]; NUM_TYPES],
        };
        let mut segment_hdr = SegmentHeader {
            absolute_delta: true,
            ..SegmentHeader::default()
        };
        if !parse_segment_header(buf, &mut br, &mut segment_hdr, &mut proba) {
            return Err(Status::BitstreamError);
        }
        let mut filter_hdr = FilterHeader::default();
        let (ok, filter_type) = parse_filter_header(buf, &mut br, &mut filter_hdr);
        if !ok {
            return Err(Status::BitstreamError);
        }

        // ParsePartitions
        let num_parts_minus_one = (1usize << br.get_value(buf, 2)) - 1;
        let last_part = num_parts_minus_one;
        if buf_size < 3 * last_part {
            return Err(Status::NotEnoughData);
        }
        let mut sz = pos;
        let mut part_start = pos + last_part * 3;
        let mut size_left = buf_size - last_part * 3;
        let buf_end = pos + buf_size;
        let mut parts = Vec::with_capacity(num_parts_minus_one + 1);
        for _ in 0..last_part {
            let mut psize = usize::from(buf[sz])
                | (usize::from(buf[sz + 1]) << 8)
                | (usize::from(buf[sz + 2]) << 16);
            if psize > size_left {
                psize = size_left;
            }
            parts.push(VP8BitReader::new(buf, part_start, psize));
            part_start += psize;
            size_left -= psize;
            sz += 3;
        }
        parts.push(VP8BitReader::new(buf, part_start, size_left));
        if part_start >= buf_end {
            return Err(Status::NotEnoughData);
        }

        // VP8ParseQuant
        let base_q0 = br.get_value(buf, 7) as i32;
        let read_delta = |br: &mut VP8BitReader| -> i32 {
            if br.get(buf) != 0 {
                br.get_signed_value(buf, 4)
            } else {
                0
            }
        };
        let dqy1_dc = read_delta(&mut br);
        let dqy2_dc = read_delta(&mut br);
        let dqy2_ac = read_delta(&mut br);
        let dquv_dc = read_delta(&mut br);
        let dquv_ac = read_delta(&mut br);
        let mut dqm = [QuantMatrix::default(); NUM_MB_SEGMENTS];
        for i in 0..NUM_MB_SEGMENTS {
            let q = if segment_hdr.use_segment {
                let q = segment_hdr.quantizer[i];
                if segment_hdr.absolute_delta {
                    q
                } else {
                    q + base_q0
                }
            } else if i > 0 {
                dqm[i] = dqm[0];
                continue;
            } else {
                base_q0
            };
            let m = &mut dqm[i];
            m.y1_mat[0] = i32::from(K_DC_TABLE[clip(q + dqy1_dc, 127) as usize]);
            m.y1_mat[1] = i32::from(K_AC_TABLE[clip(q, 127) as usize]);
            m.y2_mat[0] = i32::from(K_DC_TABLE[clip(q + dqy2_dc, 127) as usize]) * 2;
            m.y2_mat[1] = (i32::from(K_AC_TABLE[clip(q + dqy2_ac, 127) as usize]) * 101581) >> 16;
            if m.y2_mat[1] < 8 {
                m.y2_mat[1] = 8;
            }
            m.uv_mat[0] = i32::from(K_DC_TABLE[clip(q + dquv_dc, 117) as usize]);
            m.uv_mat[1] = i32::from(K_AC_TABLE[clip(q + dquv_ac, 127) as usize]);
        }

        // Frame buffer marking: the key-frame `refresh_entropy_probs` bit (libwebp ignores its value).
        let _ = br.get(buf);

        // VP8ParseProba
        for t in 0..NUM_TYPES {
            for b in 0..NUM_BANDS {
                for c in 0..NUM_CTX {
                    for p in 0..NUM_PROBAS {
                        let idx = ((t * NUM_BANDS + b) * NUM_CTX + c) * NUM_PROBAS + p;
                        proba.bands[t][b][c][p] =
                            if br.get_bit(buf, i32::from(COEFFS_UPDATE_PROBA[idx])) != 0 {
                                br.get_value(buf, 8) as u8
                            } else {
                                COEFFS_PROBA0[idx]
                            };
                    }
                }
            }
        }
        let use_skip_proba = br.get(buf) != 0;
        let skip_p = if use_skip_proba {
            br.get_value(buf, 8) as u8
        } else {
            0
        };

        let mut dec = Self {
            br,
            parts,
            num_parts_minus_one,
            mb_w,
            mb_h,
            filter_hdr,
            segment_hdr,
            filter_type,
            dqm,
            proba,
            use_skip_proba,
            skip_p,
            intra_t: vec![B_DC_PRED; 4 * mb_w],
            intra_l: [B_DC_PRED; 4],
            yuv_t: vec![TopSamples::default(); mb_w],
            mb_info: vec![MbCtx::default(); mb_w + 1],
            mb_data: vec![MbData::default(); mb_w],
            f_info: vec![FInfo::default(); mb_w * mb_h],
            fstrengths: [[FInfo::default(); 2]; NUM_MB_SEGMENTS],
            yuv_b: vec![0; YUV_SIZE],
            planes: Planes::default(),
        };
        dec.precompute_filter_strengths();
        dec.planes.width = width;
        dec.planes.height = height;
        dec.planes.mb_w = mb_w;
        dec.planes.mb_h = mb_h;
        dec.planes.y_stride = mb_w * 16;
        dec.planes.uv_stride = mb_w * 8;
        dec.planes.y = vec![0; mb_w * 16 * mb_h * 16];
        dec.planes.u = vec![0; mb_w * 8 * mb_h * 8];
        dec.planes.v = vec![0; mb_w * 8 * mb_h * 8];
        dec.planes.filter_type = filter_type;
        Ok(dec)
    }

    /// Port of `PrecomputeFilterStrengths`.
    fn precompute_filter_strengths(&mut self) {
        if self.filter_type == 0 {
            return;
        }
        let hdr = self.filter_hdr.clone();
        for s in 0..NUM_MB_SEGMENTS {
            let base_level = if self.segment_hdr.use_segment {
                let mut b = self.segment_hdr.filter_strength[s];
                if !self.segment_hdr.absolute_delta {
                    b += hdr.level;
                }
                b
            } else {
                hdr.level
            };
            for i4x4 in 0..2usize {
                let mut level = base_level;
                if hdr.use_lf_delta {
                    level += hdr.ref_lf_delta[0];
                    if i4x4 == 1 {
                        level += hdr.mode_lf_delta[0];
                    }
                }
                level = level.clamp(0, 63);
                let info = &mut self.fstrengths[s][i4x4];
                if level > 0 {
                    let mut ilevel = level;
                    if hdr.sharpness > 0 {
                        ilevel >>= if hdr.sharpness > 4 { 2 } else { 1 };
                        if ilevel > 9 - hdr.sharpness {
                            ilevel = 9 - hdr.sharpness;
                        }
                    }
                    if ilevel < 1 {
                        ilevel = 1;
                    }
                    info.f_ilevel = ilevel;
                    info.f_limit = 2 * level + ilevel;
                    info.hev_thresh = if level >= 40 {
                        2
                    } else if level >= 15 {
                        1
                    } else {
                        0
                    };
                } else {
                    info.f_limit = 0; // no filtering
                }
                info.f_inner = i4x4 == 1;
            }
        }
    }

    /// Port of `ParseIntraMode` for macroblock `mb_x` of the current row.
    fn parse_intra_mode(&mut self, buf: &[u8], mb_x: usize) {
        let top_base = 4 * mb_x;
        let br = &mut self.br;
        let block = &mut self.mb_data[mb_x];
        if self.segment_hdr.update_map {
            block.segment = if br.get_bit(buf, i32::from(self.proba.segments[0])) == 0 {
                br.get_bit(buf, i32::from(self.proba.segments[1])) as u8
            } else {
                br.get_bit(buf, i32::from(self.proba.segments[2])) as u8 + 2
            };
        } else {
            block.segment = 0; // default for intra
        }
        if self.use_skip_proba {
            block.skip = br.get_bit(buf, i32::from(self.skip_p)) != 0;
        }
        block.is_i4x4 = br.get_bit(buf, 145) == 0;
        if !block.is_i4x4 {
            let ymode = if br.get_bit(buf, 156) != 0 {
                if br.get_bit(buf, 128) != 0 {
                    TM_PRED
                } else {
                    H_PRED
                }
            } else if br.get_bit(buf, 163) != 0 {
                V_PRED
            } else {
                DC_PRED
            };
            block.imodes[0] = ymode;
            self.intra_t[top_base..top_base + 4].fill(ymode);
            self.intra_l.fill(ymode);
        } else {
            for y in 0..4 {
                let mut ymode = self.intra_l[y];
                for x in 0..4 {
                    let top = self.intra_t[top_base + x];
                    let (ctx_top, ctx_left) = (usize::from(top), usize::from(ymode));
                    let prob = |k: usize| BMODES_PROBA[(ctx_top * 10 + ctx_left) * 9 + k];
                    // The explicit tree of the C code (`USE_GENERIC_TREE == 0`).
                    ymode = if br.get_bit(buf, i32::from(prob(0))) == 0 {
                        B_DC_PRED
                    } else if br.get_bit(buf, i32::from(prob(1))) == 0 {
                        B_TM_PRED
                    } else if br.get_bit(buf, i32::from(prob(2))) == 0 {
                        B_VE_PRED
                    } else if br.get_bit(buf, i32::from(prob(3))) == 0 {
                        if br.get_bit(buf, i32::from(prob(4))) == 0 {
                            B_HE_PRED
                        } else if br.get_bit(buf, i32::from(prob(5))) == 0 {
                            B_RD_PRED
                        } else {
                            B_VR_PRED
                        }
                    } else if br.get_bit(buf, i32::from(prob(6))) == 0 {
                        B_LD_PRED
                    } else if br.get_bit(buf, i32::from(prob(7))) == 0 {
                        B_VL_PRED
                    } else if br.get_bit(buf, i32::from(prob(8))) == 0 {
                        B_HD_PRED
                    } else {
                        B_HU_PRED
                    };
                    self.intra_t[top_base + x] = ymode;
                }
                block.imodes[4 * y..4 * y + 4]
                    .copy_from_slice(&self.intra_t[top_base..top_base + 4]);
                self.intra_l[y] = ymode;
            }
        }
        block.uvmode = if br.get_bit(buf, 142) == 0 {
            DC_PRED
        } else if br.get_bit(buf, 114) == 0 {
            V_PRED
        } else if br.get_bit(buf, 183) != 0 {
            TM_PRED
        } else {
            H_PRED
        };
    }

    /// Port of `ParseResiduals` for macroblock `mb_x`. Returns true when the block has no
    /// non-zero coefficient (the `skip` result of `VP8DecodeMB`).
    fn parse_residuals(&mut self, buf: &[u8], token: usize, mb_x: usize) -> bool {
        // `mb` is the top context of this column (mb_info_ + mb_x); `left` is the single running
        // left context (mb_info_ - 1), which index 0 of the array holds.
        let mb = mb_x + 1;
        let left = 0;
        let segment = self.mb_data[mb_x].segment as usize;
        let q = self.dqm[segment];
        let is_i4x4 = self.mb_data[mb_x].is_i4x4;
        let proba = &self.proba;
        let mut br = std::mem::take(&mut self.parts[token]);
        let block = &mut self.mb_data[mb_x];
        block.coeffs.fill(0);
        let coeffs = &mut block.coeffs;
        let first;
        let ac_type;
        if !is_i4x4 {
            // parse DC
            let mut dc = [0i16; 16];
            let ctx = usize::from(self.mb_info[mb].nz_dc) + usize::from(self.mb_info[left].nz_dc);
            let nz = get_coeffs(&mut br, buf, proba, 1, ctx, q.y2_mat, 0, &mut dc);
            let nzd = u8::from(nz > 0);
            self.mb_info[mb].nz_dc = nzd;
            self.mb_info[left].nz_dc = nzd;
            if nz > 1 {
                // more than just the DC -> perform the full transform
                dsp::transform_wht(&dc, coeffs);
            } else {
                // only DC is non-zero -> inlined simplified transform
                let dc0 = ((i32::from(dc[0]) + 3) >> 3) as i16;
                for i in (0..16 * 16).step_by(16) {
                    coeffs[i] = dc0;
                }
            }
            first = 1usize;
            ac_type = 0usize;
        } else {
            first = 0;
            ac_type = 3usize;
        }

        let mut dst = 0usize; // index of the current block in `coeffs`
        let mut tnz = self.mb_info[mb].nz & 0x0f;
        let mut lnz = self.mb_info[left].nz & 0x0f;
        let mut non_zero_y: u32 = 0;
        for _y in 0..4 {
            let mut l = lnz & 1;
            let mut nz_coeffs: u32 = 0;
            for _x in 0..4 {
                let ctx = usize::from(l + (tnz & 1));
                let nz = get_coeffs(
                    &mut br,
                    buf,
                    proba,
                    ac_type,
                    ctx,
                    q.y1_mat,
                    first,
                    &mut coeffs[dst..dst + 16],
                );
                l = u8::from(nz > first);
                tnz = (tnz >> 1) | (l << 7);
                nz_coeffs = nz_code_bits(nz_coeffs, nz, coeffs[dst] != 0);
                dst += 16;
            }
            tnz >>= 4;
            lnz = (lnz >> 1) | (l << 7);
            non_zero_y = (non_zero_y << 8) | nz_coeffs;
        }
        let mut out_t_nz = tnz;
        let mut out_l_nz = lnz >> 4;

        let mut non_zero_uv: u32 = 0;
        for ch in [0u32, 2] {
            let mut nz_coeffs: u32 = 0;
            let mut tnz_c = self.mb_info[mb].nz >> (4 + ch);
            let mut lnz_c = self.mb_info[left].nz >> (4 + ch);
            for _y in 0..2 {
                let mut l = lnz_c & 1;
                for _x in 0..2 {
                    let ctx = usize::from(l + (tnz_c & 1));
                    let nz = get_coeffs(
                        &mut br,
                        buf,
                        proba,
                        2,
                        ctx,
                        q.uv_mat,
                        0,
                        &mut coeffs[dst..dst + 16],
                    );
                    l = u8::from(nz > 0);
                    tnz_c = (tnz_c >> 1) | (l << 3);
                    nz_coeffs = nz_code_bits(nz_coeffs, nz, coeffs[dst] != 0);
                    dst += 16;
                }
                tnz_c >>= 2;
                lnz_c = (lnz_c >> 1) | (l << 5);
            }
            non_zero_uv |= nz_coeffs << (4 * ch);
            out_t_nz |= (tnz_c << 4) << ch;
            out_l_nz |= (lnz_c & 0xf0) << ch;
        }
        self.mb_info[mb].nz = out_t_nz;
        self.mb_info[left].nz = out_l_nz;
        block.non_zero_y = non_zero_y;
        block.non_zero_uv = non_zero_uv;
        self.parts[token] = br;
        non_zero_y == 0 && non_zero_uv == 0
    }

    /// Port of `VP8DecodeMB`. Returns false when the token partition ran out of data.
    fn decode_mb(&mut self, buf: &[u8], token: usize, mb_x: usize, mb_y: usize) -> bool {
        let use_skip = self.use_skip_proba && self.mb_data[mb_x].skip;
        let mut skip = use_skip;
        if !skip {
            skip = self.parse_residuals(buf, token, mb_x);
        } else {
            let left = 0;
            let mb = mb_x + 1;
            self.mb_info[left].nz = 0;
            self.mb_info[mb].nz = 0;
            if !self.mb_data[mb_x].is_i4x4 {
                self.mb_info[left].nz_dc = 0;
                self.mb_info[mb].nz_dc = 0;
            }
            let block = &mut self.mb_data[mb_x];
            block.non_zero_y = 0;
            block.non_zero_uv = 0;
        }
        if self.filter_type > 0 {
            // store filter info
            let block = &self.mb_data[mb_x];
            let mut finfo = self.fstrengths[block.segment as usize][usize::from(block.is_i4x4)];
            finfo.f_inner |= !skip;
            self.f_info[mb_y * self.mb_w + mb_x] = finfo;
        }
        !self.parts[token].eof()
    }

    /// Port of `ReconstructRow`: predicts and adds the residuals of one macroblock row, writing the
    /// unfiltered samples into the planes.
    fn reconstruct_row(&mut self, buf: &[u8], mb_y: usize) {
        let _ = buf;
        let mb_w = self.mb_w;
        let mb_h = self.mb_h;
        let yb = &mut self.yuv_b;
        let y_dst = Y_OFF;
        let u_dst = U_OFF;
        let v_dst = V_OFF;
        for j in 0..16 {
            yb[y_dst + j * BPS - 1] = 129;
        }
        for j in 0..8 {
            yb[u_dst + j * BPS - 1] = 129;
            yb[v_dst + j * BPS - 1] = 129;
        }
        if mb_y > 0 {
            yb[y_dst - 1 - BPS] = 129;
            yb[u_dst - 1 - BPS] = 129;
            yb[v_dst - 1 - BPS] = 129;
        } else {
            yb[y_dst - BPS - 1..y_dst - BPS - 1 + 16 + 4 + 1].fill(127);
            yb[u_dst - BPS - 1..u_dst - BPS - 1 + 8 + 1].fill(127);
            yb[v_dst - BPS - 1..v_dst - BPS - 1 + 8 + 1].fill(127);
        }
        let y_stride = self.planes.y_stride;
        let uv_stride = self.planes.uv_stride;
        for mb_x in 0..mb_w {
            if mb_x > 0 {
                for j in -1isize..16 {
                    let base = (y_dst as isize + j * BPS as isize) as usize;
                    yb.copy_within(base + 12..base + 16, base - 4);
                }
                for j in -1isize..8 {
                    let bu = (u_dst as isize + j * BPS as isize) as usize;
                    yb.copy_within(bu + 4..bu + 8, bu - 4);
                    let bv = (v_dst as isize + j * BPS as isize) as usize;
                    yb.copy_within(bv + 4..bv + 8, bv - 4);
                }
            }
            let block = &self.mb_data[mb_x];
            let top = self.yuv_t[mb_x];
            let mut bits = block.non_zero_y;
            if mb_y > 0 {
                yb[y_dst - BPS..y_dst - BPS + 16].copy_from_slice(&top.y);
                yb[u_dst - BPS..u_dst - BPS + 8].copy_from_slice(&top.u);
                yb[v_dst - BPS..v_dst - BPS + 8].copy_from_slice(&top.v);
            }
            if block.is_i4x4 {
                // 4x4
                let tr = y_dst - BPS + 16; // top_right: four samples above-right of the MB
                if mb_y > 0 {
                    if mb_x >= mb_w - 1 {
                        // on rightmost border
                        let v = top.y[15];
                        yb[tr..tr + 4].fill(v);
                    } else {
                        let next = self.yuv_t[mb_x + 1].y;
                        yb[tr..tr + 4].copy_from_slice(&next[0..4]);
                    }
                }
                // `top_right[BPS]` etc. in C are uint32 indices: byte offsets 4*BPS, 8*BPS, 12*BPS.
                let tr_vals = [yb[tr], yb[tr + 1], yb[tr + 2], yb[tr + 3]];
                for k in [4usize, 8, 12] {
                    let at = tr + k * BPS;
                    yb[at..at + 4].copy_from_slice(&tr_vals);
                }
                for n in 0..16 {
                    let dst = y_dst + K_SCAN[n];
                    dsp::pred_luma4(block.imodes[n], yb, dst);
                    do_transform(bits, &block.coeffs[n * 16..n * 16 + 16], yb, dst);
                    bits <<= 2;
                }
            } else {
                // 16x16
                let pred = check_mode(mb_x, mb_y, block.imodes[0]);
                dsp::pred_luma16(pred, yb, y_dst);
                if bits != 0 {
                    for n in 0..16 {
                        do_transform(
                            bits,
                            &block.coeffs[n * 16..n * 16 + 16],
                            yb,
                            y_dst + K_SCAN[n],
                        );
                        bits <<= 2;
                    }
                }
            }
            {
                let bits_uv = block.non_zero_uv;
                let pred = check_mode(mb_x, mb_y, block.uvmode);
                dsp::pred_chroma8(pred, yb, u_dst);
                dsp::pred_chroma8(pred, yb, v_dst);
                do_uv_transform(bits_uv, &block.coeffs[16 * 16..20 * 16], yb, u_dst);
                do_uv_transform(bits_uv >> 8, &block.coeffs[20 * 16..24 * 16], yb, v_dst);
            }
            if mb_y < mb_h - 1 {
                let mut t = TopSamples::default();
                t.y.copy_from_slice(&yb[y_dst + 15 * BPS..y_dst + 15 * BPS + 16]);
                t.u.copy_from_slice(&yb[u_dst + 7 * BPS..u_dst + 7 * BPS + 8]);
                t.v.copy_from_slice(&yb[v_dst + 7 * BPS..v_dst + 7 * BPS + 8]);
                self.yuv_t[mb_x] = t;
            }
            // Store the reconstructed macroblock in the planes.
            for j in 0..16 {
                let row = (mb_y * 16 + j) * y_stride + mb_x * 16;
                self.planes.y[row..row + 16]
                    .copy_from_slice(&yb[y_dst + j * BPS..y_dst + j * BPS + 16]);
            }
            for j in 0..8 {
                let row = (mb_y * 8 + j) * uv_stride + mb_x * 8;
                self.planes.u[row..row + 8]
                    .copy_from_slice(&yb[u_dst + j * BPS..u_dst + j * BPS + 8]);
                self.planes.v[row..row + 8]
                    .copy_from_slice(&yb[v_dst + j * BPS..v_dst + j * BPS + 8]);
            }
        }
    }

    /// Port of `DoFilter`: the loop filter of one macroblock.
    fn do_filter(&mut self, mb_x: usize, mb_y: usize) {
        let f = self.f_info[mb_y * self.mb_w + mb_x];
        let limit = f.f_limit;
        if limit == 0 {
            return;
        }
        let ys = self.planes.y_stride;
        let uvs = self.planes.uv_stride;
        let yo = mb_y * 16 * ys + mb_x * 16;
        let uvo = mb_y * 8 * uvs + mb_x * 8;
        let ilevel = f.f_ilevel;
        let hev = f.hev_thresh;
        let planes = &mut self.planes;
        if self.filter_type == 1 {
            // simple
            if mb_x > 0 {
                dsp::simple_h_filter16(&mut planes.y, yo, ys, limit + 4);
            }
            if f.f_inner {
                dsp::simple_h_filter16i(&mut planes.y, yo, ys, limit);
            }
            if mb_y > 0 {
                dsp::simple_v_filter16(&mut planes.y, yo, ys, limit + 4);
            }
            if f.f_inner {
                dsp::simple_v_filter16i(&mut planes.y, yo, ys, limit);
            }
        } else {
            // complex
            if mb_x > 0 {
                dsp::h_filter16(&mut planes.y, yo, ys, limit + 4, ilevel, hev);
                dsp::h_filter8(
                    &mut planes.u,
                    &mut planes.v,
                    uvo,
                    uvs,
                    limit + 4,
                    ilevel,
                    hev,
                );
            }
            if f.f_inner {
                dsp::h_filter16i(&mut planes.y, yo, ys, limit, ilevel, hev);
                dsp::h_filter8i(&mut planes.u, &mut planes.v, uvo, uvs, limit, ilevel, hev);
            }
            if mb_y > 0 {
                dsp::v_filter16(&mut planes.y, yo, ys, limit + 4, ilevel, hev);
                dsp::v_filter8(
                    &mut planes.u,
                    &mut planes.v,
                    uvo,
                    uvs,
                    limit + 4,
                    ilevel,
                    hev,
                );
            }
            if f.f_inner {
                dsp::v_filter16i(&mut planes.y, yo, ys, limit, ilevel, hev);
                dsp::v_filter8i(&mut planes.u, &mut planes.v, uvo, uvs, limit, ilevel, hev);
            }
        }
    }
}

/// Decodes one lossy VP8 frame (`buf` is the VP8 chunk payload) into planes.
///
/// Macroblock rows are parsed up to the crop bottom (`EnterCritical`'s `br_mb_y`), and the loop
/// filter runs on the macroblocks in the crop window with its margin, as in libwebp.
///
/// # Errors
///
/// Returns the `Status` recorded when the frame header or a partition fails to parse.
#[doc(alias = "VP8Decode")]
pub fn decode(buf: &[u8], crop: Crop) -> Result<Planes, Status> {
    decode_with_bypass(buf, crop, false)
}

/// `VP8Decode` with `io->bypass_filtering` as `bypass_filtering`: when set, `VP8EnterCritical`
/// turns the loop filter off (`dec->filter_type_ = 0`) before any row is decoded.
///
/// # Errors
///
/// Returns the `Status` recorded when the frame header or a partition fails to parse.
#[doc(alias = "VP8EnterCritical")]
pub fn decode_with_bypass(
    buf: &[u8],
    crop: Crop,
    bypass_filtering: bool,
) -> Result<Planes, Status> {
    let mut dec = Decoder::new(buf)?;
    if bypass_filtering {
        dec.filter_type = 0;
        dec.planes.filter_type = 0;
    }
    let filter_type = dec.filter_type;
    let extra_pixels = K_FILTER_EXTRA_ROWS[usize::from(filter_type)];
    let (tl_mb_x, tl_mb_y) = if filter_type == 2 {
        (0i32, 0i32)
    } else {
        (
            ((crop.left - extra_pixels) >> 4).max(0),
            ((crop.top - extra_pixels) >> 4).max(0),
        )
    };
    let br_mb_y = ((crop.bottom + 15 + extra_pixels) >> 4).min(dec.mb_h as i32);
    let br_mb_x = ((crop.right + 15 + extra_pixels) >> 4).min(dec.mb_w as i32);

    let num_parts_minus_one = dec.num_parts_minus_one;
    for mb_y in 0..br_mb_y as usize {
        let token = mb_y & num_parts_minus_one;
        // VP8ParseIntraModeRow
        for mb_x in 0..dec.mb_w {
            dec.parse_intra_mode(buf, mb_x);
        }
        if dec.br.eof() {
            return Err(Status::NotEnoughData); // Premature end-of-partition0 encountered.
        }
        for mb_x in 0..dec.mb_w {
            if !dec.decode_mb(buf, token, mb_x, mb_y) {
                return Err(Status::NotEnoughData); // Premature end-of-file encountered.
            }
        }
        // VP8InitScanline
        dec.mb_info[0] = MbCtx::default();
        dec.intra_l = [B_DC_PRED; 4];
        dec.reconstruct_row(buf, mb_y);
    }
    // FilterRow for each row in [tl_mb_y, br_mb_y), MBs in [tl_mb_x, br_mb_x).
    if filter_type > 0 {
        for mb_y in tl_mb_y as usize..br_mb_y as usize {
            for mb_x in tl_mb_x as usize..br_mb_x as usize {
                dec.do_filter(mb_x, mb_y);
            }
        }
    }
    Ok(dec.planes)
}
