// Copyright 2010 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the COPYING file. Port by The skia-rust Authors.

//! Port of libwebp `src/utils/bit_reader_utils.{c,h}` and `src/utils/bit_reader_inl_utils.h`.
//!
//! Two readers: [`VP8BitReader`] is the boolean (arithmetic) decoder of the lossy bitstream, and
//! [`VP8LBitReader`] is the plain LSB-first reader of the lossless bitstream. Both keep libwebp's
//! state layout and its end-of-stream behaviour. The VP8 reader uses `BITS = 56` (the value libwebp
//! selects on 64-bit hosts); the decoded bits do not depend on `BITS`.
//!
//! Neither reader owns its bytes. The VP8 reader keeps indices into the caller's slice and every
//! method takes that slice, so a decoder can keep its input in one growable buffer.

// Module-level clippy allows. Each one mirrors the C source of this module.
// clippy::cast_possible_truncation: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_possible_wrap: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_sign_loss: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::manual_ilog2: mirrors the C bit expression (BitsLog2Floor), not the std method.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::manual_ilog2
)]

/// Port of `BITS` in `bit_reader_utils.h` on 64-bit hosts.
const BITS: i32 = 56;
/// Port of `VP8L_LBITS`.
const VP8L_LBITS: i32 = 64;
/// Port of `VP8L_WBITS`.
const VP8L_WBITS: i32 = 32;
/// Port of `VP8L_MAX_NUM_BIT_READ`.
pub const VP8L_MAX_NUM_BIT_READ: i32 = 24;

/// Port of `BitsLog2Floor` (`utils/utils.h`), for a non-zero argument.
#[inline]
#[must_use]
pub fn bits_log2_floor(n: u32) -> u32 {
    31 - n.leading_zeros()
}

/// Port of `VP8BitReader` (`bit_reader_utils.h`). `buf` is the index of the next byte to read,
/// `buf_end` the end of the partition and `buf_max` the first index at which the fast (multi-byte)
/// load is no longer safe.
#[derive(Debug, Clone, Default)]
pub struct VP8BitReader {
    value: u64,
    range: u32,
    bits: i32,
    buf: usize,
    buf_end: usize,
    buf_max: usize,
    eof: bool,
}

impl VP8BitReader {
    /// Port of `VP8InitBitReader`: reads `data[start..start + size]`.
    #[doc(alias = "VP8InitBitReader")]
    #[must_use]
    pub fn new(data: &[u8], start: usize, size: usize) -> Self {
        let mut br = Self {
            value: 0,
            range: 255 - 1,
            bits: -8, // to load the very first 8bits
            buf: 0,
            buf_end: 0,
            buf_max: 0,
            eof: false,
        };
        br.set_buffer(start, size);
        br.load_new_bytes(data);
        br
    }

    /// `VP8BitReader::buf_`: the index of the next byte the reader loads. The incremental decoder
    /// records it as the memory buffer's start (`token_br->buf_ - mem->buf_`).
    #[must_use]
    pub fn pos(&self) -> usize {
        self.buf
    }

    /// `buf_end_ - buf_`: the bytes of the partition not yet loaded (`CopyParts0Data`).
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.buf_end - self.buf
    }

    /// Port of `VP8BitReaderSetBuffer(br, br->buf_, end - br->buf_)`: extends the partition to
    /// `end`, the data available so far (`DoRemap` for the last partition).
    #[doc(alias = "VP8BitReaderSetBuffer")]
    pub fn set_end(&mut self, end: usize) {
        self.set_buffer(self.buf, end - self.buf);
    }

    /// Port of `VP8BitReaderSetBuffer`.
    #[doc(alias = "VP8BitReaderSetBuffer")]
    fn set_buffer(&mut self, start: usize, size: usize) {
        self.buf = start;
        self.buf_end = start + size;
        self.buf_max = if size >= 8 {
            start + size - 8 + 1
        } else {
            start
        };
    }

    /// Port of `VP8LoadFinalBytes`.
    #[doc(alias = "VP8LoadFinalBytes")]
    fn load_final_bytes(&mut self, data: &[u8]) {
        // Only read 8bits at a time
        if self.buf < self.buf_end {
            self.bits += 8;
            self.value = u64::from(data[self.buf]) | (self.value << 8);
            self.buf += 1;
        } else if !self.eof {
            self.value <<= 8;
            self.bits += 8;
            self.eof = true;
        } else {
            self.bits = 0; // This is to avoid undefined behaviour with shifts.
        }
    }

    /// Port of `VP8LoadNewBytes` (`bit_reader_inl_utils.h`).
    #[doc(alias = "VP8LoadNewBytes")]
    fn load_new_bytes(&mut self, data: &[u8]) {
        if self.buf < self.buf_max {
            let mut in_bits = [0u8; 8];
            in_bits.copy_from_slice(&data[self.buf..self.buf + 8]);
            // lbit_t is uint64_t here: memcpy of 8 bytes, then BSwap64 and shift by 64 - BITS.
            let bits = u64::from_le_bytes(in_bits).swap_bytes() >> (64 - BITS);
            self.buf += (BITS >> 3) as usize;
            self.value = bits | (self.value << BITS);
            self.bits += BITS;
        } else {
            self.load_final_bytes(data);
        }
    }

    /// Port of `VP8GetBit`.
    #[doc(alias = "VP8GetBit")]
    #[inline]
    pub fn get_bit(&mut self, data: &[u8], prob: i32) -> i32 {
        let mut range = self.range;
        if self.bits < 0 {
            self.load_new_bytes(data);
        }
        let pos = self.bits;
        let split = (range * prob as u32) >> 8;
        let value = (self.value >> pos) as u32;
        let bit = i32::from(value > split);
        if bit != 0 {
            range -= split;
            self.value -= u64::from(split + 1) << pos;
        } else {
            range = split + 1;
        }
        let shift = 7 ^ bits_log2_floor(range);
        range <<= shift;
        self.bits -= shift as i32;
        self.range = range - 1;
        bit
    }

    /// Port of `VP8GetSigned`.
    #[doc(alias = "VP8GetSigned")]
    #[inline]
    pub fn get_signed(&mut self, data: &[u8], v: i32) -> i32 {
        if self.bits < 0 {
            self.load_new_bytes(data);
        }
        let pos = self.bits;
        let split = self.range >> 1;
        let value = (self.value >> pos) as u32;
        let mask = (split.wrapping_sub(value) as i32) >> 31; // -1 or 0
        self.bits -= 1;
        self.range = self.range.wrapping_add(mask as u32);
        self.range |= 1;
        self.value -= u64::from((split + 1) & (mask as u32)) << pos;
        (v ^ mask).wrapping_sub(mask)
    }

    /// Port of `VP8Get`: one bit at probability 1/2.
    #[doc(alias = "VP8Get")]
    #[inline]
    pub fn get(&mut self, data: &[u8]) -> i32 {
        self.get_bit(data, 0x80)
    }

    /// Port of `VP8GetValue`.
    #[doc(alias = "VP8GetValue")]
    pub fn get_value(&mut self, data: &[u8], mut bits: i32) -> u32 {
        let mut v = 0u32;
        while bits > 0 {
            bits -= 1;
            v |= (self.get_bit(data, 0x80) as u32) << bits;
        }
        v
    }

    /// Port of `VP8GetSignedValue`.
    #[doc(alias = "VP8GetSignedValue")]
    pub fn get_signed_value(&mut self, data: &[u8], bits: i32) -> i32 {
        let value = self.get_value(data, bits) as i32;
        if self.get(data) != 0 { -value } else { value }
    }

    /// Port of `VP8BitReader::eof_`.
    #[must_use]
    pub fn eof(&self) -> bool {
        self.eof
    }
}

/// Port of `VP8LBitReader` (`bit_reader_utils.h`): LSB-first reader over the lossless bitstream.
///
/// The reader does not own the bytes: every method takes the buffer, so the lossless decoder can
/// keep its input in one place (the incremental decoder grows that buffer).
#[derive(Debug, Clone, Default)]
pub struct VP8LBitReader {
    val: u64,
    pos: usize,
    bit_pos: i32,
    eos: bool,
}

impl VP8LBitReader {
    /// Port of `VP8LInitBitReader`.
    #[doc(alias = "VP8LInitBitReader")]
    #[must_use]
    pub fn new(buf: &[u8]) -> Self {
        let mut value = 0u64;
        let length = buf.len().min(8);
        for (i, &b) in buf[..length].iter().enumerate() {
            value |= u64::from(b) << (8 * i);
        }
        Self {
            val: value,
            pos: length,
            bit_pos: 0,
            eos: false,
        }
    }

    /// Port of `VP8LPrefetchBits`.
    #[doc(alias = "VP8LPrefetchBits")]
    #[inline]
    #[must_use]
    pub fn prefetch_bits(&self) -> u32 {
        (self.val >> (self.bit_pos & (VP8L_LBITS - 1))) as u32
    }

    /// Port of `VP8LIsEndOfStream`.
    #[doc(alias = "VP8LIsEndOfStream")]
    #[inline]
    #[must_use]
    pub fn is_end_of_stream(&self, buf: &[u8]) -> bool {
        self.eos || (self.pos == buf.len() && self.bit_pos > VP8L_LBITS)
    }

    /// Port of `VP8LSetBitPos`.
    #[doc(alias = "VP8LSetBitPos")]
    #[inline]
    pub fn set_bit_pos(&mut self, val: i32) {
        self.bit_pos = val;
    }

    /// Port of `VP8LBitReader::bit_pos_`.
    #[inline]
    #[must_use]
    pub fn bit_pos(&self) -> i32 {
        self.bit_pos
    }

    /// Port of `VP8LBitReader::eos_`.
    #[inline]
    #[must_use]
    pub fn eos(&self) -> bool {
        self.eos
    }

    /// Sets `eos_` directly, as the C code does after `VP8LIsEndOfStream`.
    #[inline]
    pub fn set_eos(&mut self, eos: bool) {
        self.eos = eos;
    }

    /// Port of `VP8LSetEndOfStream`.
    fn set_end_of_stream(&mut self) {
        self.eos = true;
        self.bit_pos = 0; // To avoid undefined behaviour with shifts.
    }

    /// Port of `ShiftBytes`.
    fn shift_bytes(&mut self, buf: &[u8]) {
        while self.bit_pos >= 8 && self.pos < buf.len() {
            self.val >>= 8;
            self.val |= u64::from(buf[self.pos]) << (VP8L_LBITS - 8);
            self.pos += 1;
            self.bit_pos -= 8;
        }
        if self.is_end_of_stream(buf) {
            self.set_end_of_stream();
        }
    }

    /// Port of `VP8LDoFillBitWindow`. The fast 4-byte load of libwebp selects the same state as
    /// this byte loop (same `val`, `pos` and `bit_pos` meaning), so only the slow path is kept.
    #[doc(alias = "VP8LDoFillBitWindow")]
    pub fn do_fill_bit_window(&mut self, buf: &[u8]) {
        debug_assert!(self.bit_pos >= VP8L_WBITS);
        self.shift_bytes(buf);
    }

    /// Port of `VP8LFillBitWindow`.
    #[doc(alias = "VP8LFillBitWindow")]
    #[inline]
    pub fn fill_bit_window(&mut self, buf: &[u8]) {
        if self.bit_pos >= VP8L_WBITS {
            self.do_fill_bit_window(buf);
        }
    }

    /// Port of `VP8LReadBits`.
    #[doc(alias = "VP8LReadBits")]
    pub fn read_bits(&mut self, buf: &[u8], n_bits: i32) -> u32 {
        if !self.eos && n_bits <= VP8L_MAX_NUM_BIT_READ {
            let val = self.prefetch_bits() & ((1u32 << n_bits) - 1);
            let new_bits = self.bit_pos + n_bits;
            self.bit_pos = new_bits;
            self.shift_bytes(buf);
            val
        } else {
            self.set_end_of_stream();
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vp8l_reads_lsb_first() {
        let data = [0b1010_1100u8, 0xff];
        let mut br = VP8LBitReader::new(&data);
        assert_eq!(br.read_bits(&data, 3), 0b100);
        assert_eq!(br.read_bits(&data, 5), 0b10101);
        assert!(!br.eos());
    }
}
