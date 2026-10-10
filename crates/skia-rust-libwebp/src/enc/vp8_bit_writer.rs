// Copyright 2010 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the VP8 boolean (arithmetic) bit writer of libwebp 1.4.0
//! (`src/utils/bit_writer_utils.c` and `.h`, the `VP8BitWriter` part): `VP8PutBit`,
//! `VP8PutBitUniform`, `VP8PutBits`, `VP8PutSignedBits`, `VP8BitWriterAppend` and the
//! `Flush` / carry propagation with the pending `0xff` run.
//!
//! The C buffer grows by `BitWriterResize`; here `buf.len()` is the C `pos_`, which is the only
//! observable of the buffer (`VP8BitWriterSize`, `VP8BitWriterFinish`).

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
    clippy::cast_lossless,
    clippy::cast_precision_loss,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::unreadable_literal,
    clippy::if_not_else,
    clippy::manual_range_contains,
    clippy::too_many_lines,
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
    clippy::cast_possible_wrap
)]

/// Port of `kNorm`: `renorm_sizes[i] = 8 - log2(i)`.
const K_NORM: [u8; 128] = [
    7, 6, 6, 5, 5, 5, 5, 4, 4, 4, 4, 4, 4, 4, 4, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0,
];

/// Port of `kNewRange`.
const K_NEW_RANGE: [u8; 128] = [
    127, 127, 191, 127, 159, 191, 223, 127, 143, 159, 175, 191, 207, 223, 239, 127, 135, 143, 151,
    159, 167, 175, 183, 191, 199, 207, 215, 223, 231, 239, 247, 127, 131, 135, 139, 143, 147, 151,
    155, 159, 163, 167, 171, 175, 179, 183, 187, 191, 195, 199, 203, 207, 211, 215, 219, 223, 227,
    231, 235, 239, 243, 247, 251, 127, 129, 131, 133, 135, 137, 139, 141, 143, 145, 147, 149, 151,
    153, 155, 157, 159, 161, 163, 165, 167, 169, 171, 173, 175, 177, 179, 181, 183, 185, 187, 189,
    191, 193, 195, 197, 199, 201, 203, 205, 207, 209, 211, 213, 215, 217, 219, 221, 223, 225, 227,
    229, 231, 233, 235, 237, 239, 241, 243, 245, 247, 249, 251, 253, 127,
];

/// Port of `VP8BitWriter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VP8BitWriter {
    /// `range_`: range minus one.
    range: i32,
    /// `value_`.
    value: i32,
    /// `run_`: the number of outstanding `0xff` bytes.
    run: i32,
    /// `nb_bits_`: the number of pending bits.
    nb_bits: i32,
    /// The output bytes (`buf_[0..pos_]`).
    buf: Vec<u8>,
}

impl VP8BitWriter {
    /// Port of `VP8BitWriterInit`. The expected size only pre-allocates, so it is not kept.
    #[must_use]
    pub fn new() -> Self {
        Self {
            range: 255 - 1,
            value: 0,
            run: 0,
            nb_bits: -8,
            buf: Vec::new(),
        }
    }

    /// Port of `Flush`: emits the settled bytes, propagating a carry into the pending run.
    fn flush(&mut self) {
        let s = 8 + self.nb_bits;
        let bits = self.value >> s;
        self.value = self.value.wrapping_sub(bits << s);
        self.nb_bits -= 8;
        if (bits & 0xff) != 0xff {
            let pos = self.buf.len();
            if bits & 0x100 != 0 {
                // overflow -> propagate carry over pending 0xff's
                if pos > 0 {
                    self.buf[pos - 1] = self.buf[pos - 1].wrapping_add(1);
                }
            }
            if self.run > 0 {
                let value: u8 = if bits & 0x100 != 0 { 0x00 } else { 0xff };
                while self.run > 0 {
                    self.buf.push(value);
                    self.run -= 1;
                }
            }
            self.buf.push((bits & 0xff) as u8);
        } else {
            self.run += 1; // delay writing of bytes 0xff, pending eventual carry.
        }
    }

    /// Port of `VP8PutBit`: codes `bit` with probability `prob` (of a zero, out of 256).
    pub fn put_bit(&mut self, bit: bool, prob: i32) -> bool {
        let split = (self.range * prob) >> 8;
        if bit {
            self.value += split + 1;
            self.range -= split + 1;
        } else {
            self.range = split;
        }
        if self.range < 127 {
            // emit 'shift' bits out and renormalize
            let shift = i32::from(K_NORM[self.range as usize]);
            self.range = i32::from(K_NEW_RANGE[self.range as usize]);
            self.value <<= shift;
            self.nb_bits += shift;
            if self.nb_bits > 0 {
                self.flush();
            }
        }
        bit
    }

    /// Port of `VP8PutBitUniform`: codes `bit` with probability one half.
    pub fn put_bit_uniform(&mut self, bit: bool) -> bool {
        let split = self.range >> 1;
        if bit {
            self.value += split + 1;
            self.range -= split + 1;
        } else {
            self.range = split;
        }
        if self.range < 127 {
            self.range = i32::from(K_NEW_RANGE[self.range as usize]);
            self.value <<= 1;
            self.nb_bits += 1;
            if self.nb_bits > 0 {
                self.flush();
            }
        }
        bit
    }

    /// Port of `VP8PutBits`: the `nb_bits` low bits of `value`, most significant first.
    pub fn put_bits(&mut self, value: u32, nb_bits: i32) {
        let mut mask: u32 = 1u32 << (nb_bits - 1);
        while mask != 0 {
            self.put_bit_uniform(value & mask != 0);
            mask >>= 1;
        }
    }

    /// Port of `VP8PutSignedBits`.
    pub fn put_signed_bits(&mut self, value: i32, nb_bits: i32) {
        if !self.put_bit_uniform(value != 0) {
            return;
        }
        if value < 0 {
            self.put_bits((((-value) << 1) | 1) as u32, nb_bits + 1);
        } else {
            self.put_bits((value << 1) as u32, nb_bits + 1);
        }
    }

    /// Port of `VP8BitWriterAppend`: appends raw bytes. Only valid at a byte boundary (after a
    /// flush); returns `false` otherwise, as the C code does.
    pub fn append(&mut self, data: &[u8]) -> bool {
        if self.nb_bits != -8 {
            return false; // Flush() must have been called
        }
        self.buf.extend_from_slice(data);
        true
    }

    /// Port of `VP8BitWriterFinish`: pads with zero bits and flushes. The bytes stay in the
    /// writer, as the C buffer does (see [`VP8BitWriter::bytes`]).
    pub fn finish_in_place(&mut self) {
        self.put_bits(0, 9 - self.nb_bits);
        self.nb_bits = 0; // pad with zeroes
        self.flush();
    }

    /// Port of `VP8BitWriterFinish` followed by taking the buffer.
    #[must_use]
    pub fn finish(mut self) -> Vec<u8> {
        self.finish_in_place();
        self.buf
    }

    /// Port of `VP8BitWriterBuf`: the bytes written so far.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Port of `VP8BitWriterSize`: the number of bytes written so far.
    #[must_use]
    pub fn size(&self) -> usize {
        self.buf.len()
    }

    /// Port of `VP8BitWriterPos` (approximate write position, in bits).
    #[must_use]
    pub fn pos_bits(&self) -> u64 {
        let nb_bits = 8 + i64::from(self.nb_bits); // bw->nb_bits_ is <= 0, note
        ((self.buf.len() as u64) + self.run as u64) * 8 + nb_bits as u64
    }
}

impl Default for VP8BitWriter {
    fn default() -> Self {
        Self::new()
    }
}
