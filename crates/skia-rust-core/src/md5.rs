// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkMD5.h, src/core/SkMD5.cpp
//
// The following code is based on the description in RFC 1321. http://www.ietf.org/rfc/rfc1321.txt

//! MD5 message digest (`SkMD5`): a streaming [`WStream`] that computes a 128-bit digest of the
//! bytes written to it.

use crate::stream::WStream;

/// Lowercase hex digits (`SkHexadecimalDigits::gLower`).
// Port of: include/private/base/SkHexDigits.h (chrome/m156)
const HEX_DIGITS_LOWER: &[u8; 16] = b"0123456789abcdef";
/// Uppercase hex digits (`SkHexadecimalDigits::gUpper`).
// Port of: include/private/base/SkHexDigits.h (chrome/m156)
const HEX_DIGITS_UPPER: &[u8; 16] = b"0123456789ABCDEF";

/// Padding appended by `SkMD5::finish` (the RFC 1321 padding block: `0x80` then zeros).
// Port of: src/core/SkMD5.cpp#L81-L86 (chrome/m156)
const PADDING: [u8; 64] = {
    let mut padding = [0u8; 64];
    padding[0] = 0x80;
    padding
};

/// Calculates a 128-bit MD5 message-digest of the bytes sent to this stream.
// Port of: src/core/SkMD5.h#L18-L28 (chrome/m156)
#[doc(alias = "SkMD5")]
#[derive(Clone, Debug)]
pub struct Md5 {
    /// Number of bytes, modulo 2^64.
    byte_count: u64,
    /// State (ABCD).
    state: [u32; 4],
    /// Input buffer.
    buffer: [u8; 64],
}

/// The 16 bytes of a finished [`Md5`] (`SkMD5::Digest`).
// Port of: src/core/SkMD5.h#L31-L49 (chrome/m156)
#[doc(alias = "SkMD5::Digest")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Digest {
    /// The digest bytes.
    pub data: [u8; 16],
}

impl Digest {
    /// Uppercase hex text of the digest (`SkMD5::Digest::toHexString`).
    // Port of: src/core/SkMD5.cpp#L112-L114 (chrome/m156)
    #[doc(alias = "toHexString")]
    #[must_use]
    pub fn to_hex_string(&self) -> String {
        to_hex_string(&self.data, HEX_DIGITS_UPPER)
    }

    /// Lowercase hex text of the digest (`SkMD5::Digest::toLowercaseHexString`).
    // Port of: src/core/SkMD5.cpp#L116-L118 (chrome/m156)
    #[doc(alias = "toLowercaseHexString")]
    #[must_use]
    pub fn to_lowercase_hex_string(&self) -> String {
        to_hex_string(&self.data, HEX_DIGITS_LOWER)
    }
}

// Port of: src/core/SkMD5.cpp#L102-L110 (chrome/m156)
fn to_hex_string(data: &[u8; 16], hex_digits: &[u8; 16]) -> String {
    let mut hex = String::with_capacity(2 * data.len());
    for &byte in data {
        hex.push(char::from(hex_digits[usize::from(byte >> 4)]));
        hex.push(char::from(hex_digits[usize::from(byte & 0xF)]));
    }
    hex
}

impl Default for Md5 {
    // Port of: src/core/SkMD5.cpp#L34-L40 (chrome/m156)
    fn default() -> Self {
        Self::new()
    }
}

impl Md5 {
    /// Creates a digest with no bytes written (`SkMD5::SkMD5`).
    // Port of: src/core/SkMD5.cpp#L34-L40 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self {
            byte_count: 0,
            // These are magic numbers from the specification.
            state: [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476],
            buffer: [0; 64],
        }
    }

    /// Processes `input`, adding it to the digest (`SkMD5::write`).
    // Port of: src/core/SkMD5.cpp#L42-L70 (chrome/m156)
    fn write_bytes(&mut self, input: &[u8]) {
        let input_length = input.len();
        let mut buffer_index = (self.byte_count & 0x3F) as usize;
        let buffer_available = 64 - buffer_index;

        let mut input_index;
        if input_length >= buffer_available {
            if buffer_index != 0 {
                self.buffer[buffer_index..].copy_from_slice(&input[..buffer_available]);
                transform(&mut self.state, &self.buffer);
                input_index = buffer_available;
            } else {
                input_index = 0;
            }

            while input_index + 63 < input_length {
                let block: &[u8; 64] = (&input[input_index..input_index + 64])
                    .try_into()
                    .expect("slice of exactly 64 bytes");
                transform(&mut self.state, block);
                input_index += 64;
            }

            buffer_index = 0;
        } else {
            input_index = 0;
        }

        let rest = &input[input_index..];
        self.buffer[buffer_index..buffer_index + rest.len()].copy_from_slice(rest);

        self.byte_count = self.byte_count.wrapping_add(input_length as u64);
    }

    /// Computes and returns the digest (`SkMD5::finish`). Writing after this is undefined, as in
    /// Skia.
    // Port of: src/core/SkMD5.cpp#L72-L100 (chrome/m156)
    pub fn finish(&mut self) -> Digest {
        // Get the number of bits before padding.
        let mut bits = [0u8; 8];
        encode_u64(&mut bits, self.byte_count << 3);

        // Pad out to 56 mod 64.
        let buffer_index = (self.byte_count & 0x3F) as usize;
        let padding_length = if buffer_index < 56 {
            56 - buffer_index
        } else {
            120 - buffer_index
        };
        self.write_bytes(&PADDING[..padding_length]);

        // Append length (length before padding, will cause final update).
        self.write_bytes(&bits);

        // Write out digest.
        let mut digest = Digest { data: [0; 16] };
        encode_state(&mut digest.data, &self.state);
        digest
    }
}

impl WStream for Md5 {
    // Port of: src/core/SkMD5.cpp#L42-L70 (chrome/m156)
    fn write(&mut self, buffer: &[u8]) -> bool {
        self.write_bytes(buffer);
        true
    }

    // Port of: src/core/SkMD5.h#L22 (chrome/m156)
    #[doc(alias = "bytesWritten")]
    #[allow(clippy::cast_possible_truncation)] // SkToSizeT: the count is modulo 2^64, as in Skia
    fn bytes_written(&self) -> usize {
        self.byte_count as usize
    }
}

// Port of: src/core/SkMD5.cpp#L120-L123 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors the RFC 1321 names
fn func_f(x: u32, y: u32, z: u32) -> u32 {
    // return (x & y) | ((~x) & z);
    ((y ^ z) & x) ^ z // equivalent but faster
}

// Port of: src/core/SkMD5.cpp#L125-L128 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors the RFC 1321 names
fn func_g(x: u32, y: u32, z: u32) -> u32 {
    (x & z) | (y & !z)
}

// Port of: src/core/SkMD5.cpp#L130-L132 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors the RFC 1321 names
fn func_h(x: u32, y: u32, z: u32) -> u32 {
    x ^ y ^ z
}

// Port of: src/core/SkMD5.cpp#L134-L136 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors the RFC 1321 names
fn func_i(x: u32, y: u32, z: u32) -> u32 {
    y ^ (x | !z)
}

// Port of: src/core/SkMD5.cpp#L143-L147 (chrome/m156)
#[allow(clippy::many_single_char_names, clippy::too_many_arguments)] // mirrors RFC 1321
fn op(f: fn(u32, u32, u32) -> u32, a: &mut u32, b: u32, c: u32, d: u32, x: u32, s: u32, t: u32) {
    // a = b + rotate_left(a + operation(b, c, d) + x + t, s)
    *a = b.wrapping_add(
        a.wrapping_add(f(b, c, d))
            .wrapping_add(x)
            .wrapping_add(t)
            .rotate_left(s),
    );
}

// MD5 basic transformation. Transforms state based on block.
// Port of: src/core/SkMD5.cpp#L149-L238 (chrome/m156)
// The round constants are written exactly as in SkMD5.cpp, so their digit grouping is not changed.
#[allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]
fn transform(state: &mut [u32; 4], block: &[u8; 64]) {
    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];

    // Decodes the block as 16 little-endian 32-bit words (`decode`).
    let x: [u32; 16] = std::array::from_fn(|i| {
        u32::from_le_bytes([
            block[4 * i],
            block[4 * i + 1],
            block[4 * i + 2],
            block[4 * i + 3],
        ])
    });

    // Round 1
    op(func_f, &mut a, b, c, d, x[0], 7, 0xd76aa478); // 1
    op(func_f, &mut d, a, b, c, x[1], 12, 0xe8c7b756); // 2
    op(func_f, &mut c, d, a, b, x[2], 17, 0x242070db); // 3
    op(func_f, &mut b, c, d, a, x[3], 22, 0xc1bdceee); // 4
    op(func_f, &mut a, b, c, d, x[4], 7, 0xf57c0faf); // 5
    op(func_f, &mut d, a, b, c, x[5], 12, 0x4787c62a); // 6
    op(func_f, &mut c, d, a, b, x[6], 17, 0xa8304613); // 7
    op(func_f, &mut b, c, d, a, x[7], 22, 0xfd469501); // 8
    op(func_f, &mut a, b, c, d, x[8], 7, 0x698098d8); // 9
    op(func_f, &mut d, a, b, c, x[9], 12, 0x8b44f7af); // 10
    op(func_f, &mut c, d, a, b, x[10], 17, 0xffff5bb1); // 11
    op(func_f, &mut b, c, d, a, x[11], 22, 0x895cd7be); // 12
    op(func_f, &mut a, b, c, d, x[12], 7, 0x6b901122); // 13
    op(func_f, &mut d, a, b, c, x[13], 12, 0xfd987193); // 14
    op(func_f, &mut c, d, a, b, x[14], 17, 0xa679438e); // 15
    op(func_f, &mut b, c, d, a, x[15], 22, 0x49b40821); // 16

    // Round 2
    op(func_g, &mut a, b, c, d, x[1], 5, 0xf61e2562); // 17
    op(func_g, &mut d, a, b, c, x[6], 9, 0xc040b340); // 18
    op(func_g, &mut c, d, a, b, x[11], 14, 0x265e5a51); // 19
    op(func_g, &mut b, c, d, a, x[0], 20, 0xe9b6c7aa); // 20
    op(func_g, &mut a, b, c, d, x[5], 5, 0xd62f105d); // 21
    op(func_g, &mut d, a, b, c, x[10], 9, 0x2441453); // 22
    op(func_g, &mut c, d, a, b, x[15], 14, 0xd8a1e681); // 23
    op(func_g, &mut b, c, d, a, x[4], 20, 0xe7d3fbc8); // 24
    op(func_g, &mut a, b, c, d, x[9], 5, 0x21e1cde6); // 25
    op(func_g, &mut d, a, b, c, x[14], 9, 0xc33707d6); // 26
    op(func_g, &mut c, d, a, b, x[3], 14, 0xf4d50d87); // 27
    op(func_g, &mut b, c, d, a, x[8], 20, 0x455a14ed); // 28
    op(func_g, &mut a, b, c, d, x[13], 5, 0xa9e3e905); // 29
    op(func_g, &mut d, a, b, c, x[2], 9, 0xfcefa3f8); // 30
    op(func_g, &mut c, d, a, b, x[7], 14, 0x676f02d9); // 31
    op(func_g, &mut b, c, d, a, x[12], 20, 0x8d2a4c8a); // 32

    // Round 3
    op(func_h, &mut a, b, c, d, x[5], 4, 0xfffa3942); // 33
    op(func_h, &mut d, a, b, c, x[8], 11, 0x8771f681); // 34
    op(func_h, &mut c, d, a, b, x[11], 16, 0x6d9d6122); // 35
    op(func_h, &mut b, c, d, a, x[14], 23, 0xfde5380c); // 36
    op(func_h, &mut a, b, c, d, x[1], 4, 0xa4beea44); // 37
    op(func_h, &mut d, a, b, c, x[4], 11, 0x4bdecfa9); // 38
    op(func_h, &mut c, d, a, b, x[7], 16, 0xf6bb4b60); // 39
    op(func_h, &mut b, c, d, a, x[10], 23, 0xbebfbc70); // 40
    op(func_h, &mut a, b, c, d, x[13], 4, 0x289b7ec6); // 41
    op(func_h, &mut d, a, b, c, x[0], 11, 0xeaa127fa); // 42
    op(func_h, &mut c, d, a, b, x[3], 16, 0xd4ef3085); // 43
    op(func_h, &mut b, c, d, a, x[6], 23, 0x4881d05); // 44
    op(func_h, &mut a, b, c, d, x[9], 4, 0xd9d4d039); // 45
    op(func_h, &mut d, a, b, c, x[12], 11, 0xe6db99e5); // 46
    op(func_h, &mut c, d, a, b, x[15], 16, 0x1fa27cf8); // 47
    op(func_h, &mut b, c, d, a, x[2], 23, 0xc4ac5665); // 48

    // Round 4
    op(func_i, &mut a, b, c, d, x[0], 6, 0xf4292244); // 49
    op(func_i, &mut d, a, b, c, x[7], 10, 0x432aff97); // 50
    op(func_i, &mut c, d, a, b, x[14], 15, 0xab9423a7); // 51
    op(func_i, &mut b, c, d, a, x[5], 21, 0xfc93a039); // 52
    op(func_i, &mut a, b, c, d, x[12], 6, 0x655b59c3); // 53
    op(func_i, &mut d, a, b, c, x[3], 10, 0x8f0ccc92); // 54
    op(func_i, &mut c, d, a, b, x[10], 15, 0xffeff47d); // 55
    op(func_i, &mut b, c, d, a, x[1], 21, 0x85845dd1); // 56
    op(func_i, &mut a, b, c, d, x[8], 6, 0x6fa87e4f); // 57
    op(func_i, &mut d, a, b, c, x[15], 10, 0xfe2ce6e0); // 58
    op(func_i, &mut c, d, a, b, x[6], 15, 0xa3014314); // 59
    op(func_i, &mut b, c, d, a, x[13], 21, 0x4e0811a1); // 60
    op(func_i, &mut a, b, c, d, x[4], 6, 0xf7537e82); // 61
    op(func_i, &mut d, a, b, c, x[11], 10, 0xbd3af235); // 62
    op(func_i, &mut c, d, a, b, x[2], 15, 0x2ad7d2bb); // 63
    op(func_i, &mut b, c, d, a, x[9], 21, 0xeb86d391); // 64

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
}

// Encodes the four state words as little-endian bytes.
// Port of: src/core/SkMD5.cpp#L240-L247 (chrome/m156)
fn encode_state(output: &mut [u8; 16], input: &[u32; 4]) {
    for (i, word) in input.iter().enumerate() {
        output[4 * i..4 * i + 4].copy_from_slice(&word.to_le_bytes());
    }
}

// Encodes a little-endian 64-bit value.
// Port of: src/core/SkMD5.cpp#L249-L258 (chrome/m156)
fn encode_u64(output: &mut [u8; 8], input: u64) {
    output.copy_from_slice(&input.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc1321_vectors() {
        let digest = |s: &str| {
            let mut md5 = Md5::new();
            md5.write(s.as_bytes());
            md5.finish().to_lowercase_hex_string()
        };
        assert_eq!(digest(""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(digest("abc"), "900150983cd24fb0d6963f7d28e17f72");
    }
}
