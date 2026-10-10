// Copyright 2010 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the VP8L half of libwebp `src/utils/bit_writer_utils.{c,h}`.
//!
//! libwebp accumulates bits in 16-bit words (`VP8L_WRITER_BITS`) and writes them little-endian.
//! The byte stream is the same whatever the word size: bits are packed LSB first, and
//! `NumBytes` is `ceil(total_bits / 8)` in both models. This port keeps a byte-granular
//! accumulator, which produces the same bytes and the same `num_bytes()` at every point.

/// Port of `VP8LBitWriter`: a bit accumulator in front of a byte buffer.
#[derive(Clone, Debug, Default)]
pub struct BitWriter {
    /// Complete bytes written so far (`buf_` .. `cur_`).
    buf: Vec<u8>,
    /// Pending bits not yet in `buf` (`bits_`); always fewer than 8 after a write.
    bits: u64,
    /// Number of valid bits in `bits` (`used_`).
    used: u32,
}

impl BitWriter {
    /// Port of `VP8LBitWriterInit`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Port of `VP8LBitWriterNumBytes`: the number of bytes the stream occupies so far.
    #[must_use]
    pub fn num_bytes(&self) -> usize {
        self.buf.len() + ((self.used as usize + 7) >> 3)
    }

    /// Port of `VP8LPutBits`: appends the low `n_bits` bits of `bits`, LSB first.
    ///
    /// Like the C code this does not mask `bits`; callers pass values that fit in `n_bits`.
    pub fn put_bits(&mut self, bits: u32, n_bits: u32) {
        debug_assert!(n_bits <= 32);
        if n_bits == 0 {
            return;
        }
        self.bits |= u64::from(bits) << self.used;
        self.used += n_bits;
        while self.used >= 8 {
            self.buf.push(self.bits as u8);
            self.bits >>= 8;
            self.used -= 8;
        }
    }

    /// Port of `VP8LBitWriterFinish`: flushes the pending bits (zero padded) and returns the
    /// bytes.
    #[must_use]
    pub fn finish(mut self) -> Vec<u8> {
        while self.used > 0 {
            self.buf.push(self.bits as u8);
            self.bits >>= 8;
            self.used = self.used.saturating_sub(8);
        }
        self.buf
    }
}
