// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/core/SkMD5.cpp#L1-L281 (chrome/m156), include/private/SkMD5.h
// Ported from: src/core/SkMD5.cpp, src/core/SkMD5.h

//! MD5 message digest (RFC 1321), as used by Skia's tests and DM to compare bitmaps.

use crate::stream::WStream;

/// A 16-byte MD5 digest. Port of `SkMD5::Digest`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[doc(alias = "SkMD5::Digest")]
pub struct Digest {
    /// The digest bytes, in order.
    pub data: [u8; 16],
}

impl Digest {
    /// Port of `SkMD5::Digest::toHexString`: upper-case hexadecimal.
    // Port of: src/core/SkMD5.cpp#L102-L113 (chrome/m156)
    #[doc(alias = "toHexString")]
    #[must_use]
    pub fn to_hex_string(&self) -> String {
        hex_string(&self.data, b"0123456789ABCDEF")
    }

    /// Port of `SkMD5::Digest::toLowercaseHexString`.
    // Port of: src/core/SkMD5.cpp#L116-L118 (chrome/m156)
    #[doc(alias = "toLowercaseHexString")]
    #[must_use]
    pub fn to_lowercase_hex_string(&self) -> String {
        hex_string(&self.data, b"0123456789abcdef")
    }
}

// Port of: src/core/SkMD5.cpp#L102-L110 (to_hex_string)
fn hex_string(data: &[u8; 16], digits: &[u8; 16]) -> String {
    let mut out = String::with_capacity(32);
    for &byte in data {
        out.push(char::from(digits[usize::from(byte >> 4)]));
        out.push(char::from(digits[usize::from(byte & 0xF)]));
    }
    out
}

/// An MD5 hasher that is also a write stream. Port of `SkMD5`.
#[derive(Clone, Debug)]
#[doc(alias = "SkMD5")]
pub struct Md5 {
    byte_count: u64,
    state: [u32; 4],
    buffer: [u8; 64],
}

impl Default for Md5 {
    fn default() -> Self {
        Self::new()
    }
}

impl Md5 {
    /// Port of `SkMD5::SkMD5`.
    // Port of: src/core/SkMD5.cpp#L34-L40 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self {
            byte_count: 0,
            state: [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476],
            buffer: [0; 64],
        }
    }

    /// Port of `SkMD5::write`: feeds `input` to the hash.
    // Port of: src/core/SkMD5.cpp#L42-L70 (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // `byteCount & 0x3F` fits in unsigned int
    pub fn write_bytes(&mut self, input: &[u8]) -> bool {
        let mut buffer_index = (self.byte_count & 0x3F) as usize;
        let buffer_available = 64 - buffer_index;
        let input_length = input.len();
        let mut input_index;
        if input_length >= buffer_available {
            if buffer_index != 0 {
                self.buffer[buffer_index..64].copy_from_slice(&input[..buffer_available]);
                let block = self.buffer;
                transform(&mut self.state, &block);
                input_index = buffer_available;
            } else {
                input_index = 0;
            }
            while input_index + 63 < input_length {
                let mut block = [0u8; 64];
                block.copy_from_slice(&input[input_index..input_index + 64]);
                transform(&mut self.state, &block);
                input_index += 64;
            }
            buffer_index = 0;
        } else {
            input_index = 0;
        }
        self.buffer[buffer_index..buffer_index + (input_length - input_index)]
            .copy_from_slice(&input[input_index..]);
        self.byte_count += input_length as u64;
        true
    }

    /// Port of `SkMD5::finish`: pads the message and returns the digest. The hasher is reset to
    /// its zero state afterwards (`memset(this, 0, sizeof(*this))` in Skia).
    // Port of: src/core/SkMD5.cpp#L72-L100 (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // `byteCount & 0x3F` fits in unsigned int
    pub fn finish(&mut self) -> Digest {
        let mut bits = [0u8; 8];
        encode64(&mut bits, self.byte_count << 3);
        let buffer_index = (self.byte_count & 0x3F) as usize;
        let padding_length = if buffer_index < 56 {
            56 - buffer_index
        } else {
            120 - buffer_index
        };
        static PADDING: [u8; 64] = {
            let mut p = [0u8; 64];
            p[0] = 0x80;
            p
        };
        self.write_bytes(&PADDING[..padding_length]);
        self.write_bytes(&bits);
        let mut digest = Digest { data: [0; 16] };
        encode128(&mut digest.data, &self.state);
        *self = Self {
            byte_count: 0,
            state: [0; 4],
            buffer: [0; 64],
        };
        digest
    }
}

impl WStream for Md5 {
    fn write(&mut self, buffer: &[u8]) -> bool {
        self.write_bytes(buffer)
    }

    fn bytes_written(&self) -> usize {
        self.byte_count as usize
    }
}

// Port of: src/core/SkMD5.cpp#L240-L247 (encode, 4 words to 16 bytes, little endian)
fn encode128(output: &mut [u8; 16], input: &[u32; 4]) {
    for (i, word) in input.iter().enumerate() {
        output[4 * i..4 * i + 4].copy_from_slice(&word.to_le_bytes());
    }
}

// Port of: src/core/SkMD5.cpp#L249-L258 (encode, 64-bit bit count to 8 bytes, little endian)
fn encode64(output: &mut [u8; 8], input: u64) {
    output.copy_from_slice(&input.to_le_bytes());
}

#[inline]
fn rotate_left(x: u32, n: u32) -> u32 {
    (x << n) | (x >> (32 - n))
}

// The round functions F, G, H and I of Skia's struct F/G/H/I (SkMD5.cpp#L120-L136).
#[inline]
fn f(x: u32, y: u32, z: u32) -> u32 {
    ((y ^ z) & x) ^ z
}
#[inline]
fn g(x: u32, y: u32, z: u32) -> u32 {
    (x & z) | (y & !z)
}
#[inline]
fn h(x: u32, y: u32, z: u32) -> u32 {
    x ^ y ^ z
}
#[inline]
fn i(x: u32, y: u32, z: u32) -> u32 {
    y ^ (x | !z)
}

// Port of: src/core/SkMD5.cpp#L143-L147 (operation)
#[inline]
fn op(func: fn(u32, u32, u32) -> u32, a: &mut u32, b: u32, c: u32, d: u32, x: u32, s: u32, t: u32) {
    *a = b.wrapping_add(rotate_left(
        a.wrapping_add(func(b, c, d))
            .wrapping_add(x)
            .wrapping_add(t),
        s,
    ));
}

// Port of: src/core/SkMD5.cpp#L149-L232 (transform: the 64 steps in Skia's order)
fn transform(state: &mut [u32; 4], block: &[u8; 64]) {
    let (mut a, mut b, mut c, mut d) = (state[0], state[1], state[2], state[3]);
    let mut x = [0u32; 16];
    for (word, chunk) in x.iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    op(f, &mut a, b, c, d, x[0], 7, 0xd76aa478);
    op(f, &mut d, a, b, c, x[1], 12, 0xe8c7b756);
    op(f, &mut c, d, a, b, x[2], 17, 0x242070db);
    op(f, &mut b, c, d, a, x[3], 22, 0xc1bdceee);
    op(f, &mut a, b, c, d, x[4], 7, 0xf57c0faf);
    op(f, &mut d, a, b, c, x[5], 12, 0x4787c62a);
    op(f, &mut c, d, a, b, x[6], 17, 0xa8304613);
    op(f, &mut b, c, d, a, x[7], 22, 0xfd469501);
    op(f, &mut a, b, c, d, x[8], 7, 0x698098d8);
    op(f, &mut d, a, b, c, x[9], 12, 0x8b44f7af);
    op(f, &mut c, d, a, b, x[10], 17, 0xffff5bb1);
    op(f, &mut b, c, d, a, x[11], 22, 0x895cd7be);
    op(f, &mut a, b, c, d, x[12], 7, 0x6b901122);
    op(f, &mut d, a, b, c, x[13], 12, 0xfd987193);
    op(f, &mut c, d, a, b, x[14], 17, 0xa679438e);
    op(f, &mut b, c, d, a, x[15], 22, 0x49b40821);
    op(g, &mut a, b, c, d, x[1], 5, 0xf61e2562);
    op(g, &mut d, a, b, c, x[6], 9, 0xc040b340);
    op(g, &mut c, d, a, b, x[11], 14, 0x265e5a51);
    op(g, &mut b, c, d, a, x[0], 20, 0xe9b6c7aa);
    op(g, &mut a, b, c, d, x[5], 5, 0xd62f105d);
    op(g, &mut d, a, b, c, x[10], 9, 0x2441453);
    op(g, &mut c, d, a, b, x[15], 14, 0xd8a1e681);
    op(g, &mut b, c, d, a, x[4], 20, 0xe7d3fbc8);
    op(g, &mut a, b, c, d, x[9], 5, 0x21e1cde6);
    op(g, &mut d, a, b, c, x[14], 9, 0xc33707d6);
    op(g, &mut c, d, a, b, x[3], 14, 0xf4d50d87);
    op(g, &mut b, c, d, a, x[8], 20, 0x455a14ed);
    op(g, &mut a, b, c, d, x[13], 5, 0xa9e3e905);
    op(g, &mut d, a, b, c, x[2], 9, 0xfcefa3f8);
    op(g, &mut c, d, a, b, x[7], 14, 0x676f02d9);
    op(g, &mut b, c, d, a, x[12], 20, 0x8d2a4c8a);
    op(h, &mut a, b, c, d, x[5], 4, 0xfffa3942);
    op(h, &mut d, a, b, c, x[8], 11, 0x8771f681);
    op(h, &mut c, d, a, b, x[11], 16, 0x6d9d6122);
    op(h, &mut b, c, d, a, x[14], 23, 0xfde5380c);
    op(h, &mut a, b, c, d, x[1], 4, 0xa4beea44);
    op(h, &mut d, a, b, c, x[4], 11, 0x4bdecfa9);
    op(h, &mut c, d, a, b, x[7], 16, 0xf6bb4b60);
    op(h, &mut b, c, d, a, x[10], 23, 0xbebfbc70);
    op(h, &mut a, b, c, d, x[13], 4, 0x289b7ec6);
    op(h, &mut d, a, b, c, x[0], 11, 0xeaa127fa);
    op(h, &mut c, d, a, b, x[3], 16, 0xd4ef3085);
    op(h, &mut b, c, d, a, x[6], 23, 0x4881d05);
    op(h, &mut a, b, c, d, x[9], 4, 0xd9d4d039);
    op(h, &mut d, a, b, c, x[12], 11, 0xe6db99e5);
    op(h, &mut c, d, a, b, x[15], 16, 0x1fa27cf8);
    op(h, &mut b, c, d, a, x[2], 23, 0xc4ac5665);
    op(i, &mut a, b, c, d, x[0], 6, 0xf4292244);
    op(i, &mut d, a, b, c, x[7], 10, 0x432aff97);
    op(i, &mut c, d, a, b, x[14], 15, 0xab9423a7);
    op(i, &mut b, c, d, a, x[5], 21, 0xfc93a039);
    op(i, &mut a, b, c, d, x[12], 6, 0x655b59c3);
    op(i, &mut d, a, b, c, x[3], 10, 0x8f0ccc92);
    op(i, &mut c, d, a, b, x[10], 15, 0xffeff47d);
    op(i, &mut b, c, d, a, x[1], 21, 0x85845dd1);
    op(i, &mut a, b, c, d, x[8], 6, 0x6fa87e4f);
    op(i, &mut d, a, b, c, x[15], 10, 0xfe2ce6e0);
    op(i, &mut c, d, a, b, x[6], 15, 0xa3014314);
    op(i, &mut b, c, d, a, x[13], 21, 0x4e0811a1);
    op(i, &mut a, b, c, d, x[4], 6, 0xf7537e82);
    op(i, &mut d, a, b, c, x[11], 10, 0xbd3af235);
    op(i, &mut c, d, a, b, x[2], 15, 0x2ad7d2bb);
    op(i, &mut b, c, d, a, x[9], 21, 0xeb86d391);
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
}
