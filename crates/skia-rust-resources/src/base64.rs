// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBase64.h, src/core/SkBase64.cpp (chrome/m156)
//
// Base64 decoding for the data URIs of the resource providers. Like Skia, a call with no output
// buffer only measures the decoded length.

/// The decode table, indexed by `byte - '+'`: the 6-bit value of each base64 character, `-1` for
/// bytes that are not in the alphabet, and [`DECODE_PAD`] for `=`.
// Port of: src/core/SkBase64.cpp#L11-L21 (chrome/m156) (`decodeData`)
#[rustfmt::skip]
const DECODE_DATA: [i8; 80] = [
    62, -1, -1, -1, 63,
    52, 53, 54, 55, 56, 57, 58, 59, 60, 61, -1, -1, -1, DECODE_PAD, -1, -1,
    -1,  0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14,
    15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, -1, -1, -1, -1, -1,
    -1, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
    41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51,
];

/// The decode value of `=` (`DecodePad`).
// Port of: src/core/SkBase64.cpp#L9-L9 (chrome/m156) (`DecodePad`)
const DECODE_PAD: i8 = -2;

/// The alphabet of the encoder (`default_encode`), with `=` as the pad character.
// Port of: src/core/SkBase64.cpp#L23-L27 (chrome/m156) (`default_encode`)
const DEFAULT_ENCODE: &[u8; 65] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=";

/// The pad index of the encoder (`EncodePad`).
// Port of: src/core/SkBase64.cpp#L11-L11 (chrome/m156) (`EncodePad`)
const ENCODE_PAD: usize = 64;

/// The errors of [`decode`] (`SkBase64::Error`).
// Port of: src/core/SkBase64.h (chrome/m156) (`SkBase64::Error`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// `kBadCharError`: a byte that is not base64, and not white space.
    BadChar,
    /// `kPadError`: a pad character after fewer than two significant characters.
    Pad,
}

/// Decodes `src` as base64. White space is skipped, and decoding stops at a NUL byte or at the
/// end of `src`; bytes past the end read as NUL. With `dst` set, the decoded bytes are written
/// there, and `dst` must hold at least the length that a call without `dst` returns.
///
/// Returns the number of decoded bytes (`SkBase64::Decode`).
///
/// # Errors
/// [`Error::BadChar`] for a byte outside the alphabet, and [`Error::Pad`] for a pad character after
/// fewer than two significant characters.
// Port of: src/core/SkBase64.cpp#L29-L95 (chrome/m156) (`SkBase64::Decode`)
#[doc(alias = "SkBase64::Decode")]
#[allow(clippy::too_many_lines)] // mirrors the C++ state machine, which keeps its goto targets
pub fn decode(src: &[u8], mut dst: Option<&mut [u8]>) -> Result<usize, Error> {
    let at = |pos: usize| src.get(pos).copied().unwrap_or(0);
    let end = src.len();
    let mut s = 0_usize;
    let mut i = 0_usize;
    let mut pad_two = false;
    let mut pad_three = false;
    'outer: while s < end {
        let mut bytes = [0_u8; 4];
        let mut byte = 0_usize;
        loop {
            let src_byte = at(s);
            s += 1;
            if src_byte == 0 {
                break 'outer; // goHome
            }
            if src_byte <= b' ' {
                continue; // treat as white space
            }
            if !(b'+'..=b'z').contains(&src_byte) {
                return Err(Error::BadChar);
            }
            let decoded = DECODE_DATA[usize::from(src_byte - b'+')];
            // A pad value is stored as its unsigned byte, as in the C++ array; the pad path never
            // writes it out.
            #[allow(clippy::cast_sign_loss)] // -2 and the table values are stored as bytes
            {
                bytes[byte] = decoded as u8;
            }
            let pad = if decoded < 0 {
                if decoded != DECODE_PAD {
                    return Err(Error::BadChar);
                }
                true // goto handlePad
            } else {
                byte += 1;
                if at(s) != 0 {
                    // `continue` of the do-while: the loop condition `byte < 4` decides.
                    if byte < 4 {
                        continue;
                    }
                    break;
                }
                if byte == 4 {
                    break;
                }
                true // falls through to handlePad
            };
            if pad {
                if byte < 2 {
                    return Err(Error::Pad);
                }
                pad_three = true;
                if byte == 2 {
                    pad_two = true;
                }
                break;
            }
        }

        let mut two: u8 = 0;
        let mut three: u8 = 0;
        // The `int` arithmetic of the C++ is in `u8` here: every value that is stored fits.
        let one = (bytes[0] << 2) | (bytes[1] >> 4);
        if let Some(dst) = dst.as_deref_mut() {
            two = bytes[1];
            two <<= 4;
            three = bytes[2];
            two |= three >> 2;
            three = (three << 6) | bytes[3];
            dst[i] = one;
        }
        i += 1;
        if pad_two {
            break;
        }
        if let Some(dst) = dst.as_deref_mut() {
            dst[i] = two;
        }
        i += 1;
        if pad_three {
            break;
        }
        if let Some(dst) = dst.as_deref_mut() {
            dst[i] = three;
        }
        i += 1;
    }
    Ok(i)
}

/// The length of the base64 text of `length` bytes, with padding (`SkBase64::EncodedSize`).
// Port of: src/core/SkBase64.h (chrome/m156) (`SkBase64::EncodedSize`)
#[must_use]
pub fn encoded_size(length: usize) -> usize {
    length.div_ceil(3) * 4
}

/// Encodes `src` as base64 into `dst`, which holds [`encoded_size`] bytes, and returns that size
/// (`SkBase64::Encode` with the default alphabet).
// Port of: src/core/SkBase64.cpp#L97-L131 (chrome/m156) (`SkBase64::Encode`)
#[doc(alias = "SkBase64::Encode")]
#[allow(clippy::many_single_char_names)] // a..d are the 6-bit groups of the C++
pub fn encode(src: &[u8], dst: &mut [u8]) -> usize {
    let encode_map = DEFAULT_ENCODE;
    let length = src.len();
    let remainder = length % 3;
    let full = length - remainder;
    let mut o = 0_usize;
    for chunk in src[..full].as_chunks::<3>().0 {
        let a = u32::from(chunk[0]);
        let b = u32::from(chunk[1]);
        let c = u32::from(chunk[2]);
        let d = c & 0x3F;
        let c = (c >> 6 | b << 2) & 0x3F;
        let b = (b >> 4 | a << 4) & 0x3F;
        let a = a >> 2;
        dst[o] = encode_map[a as usize];
        dst[o + 1] = encode_map[b as usize];
        dst[o + 2] = encode_map[c as usize];
        dst[o + 3] = encode_map[d as usize];
        o += 4;
    }
    if remainder > 0 {
        let mut k1 = 0_usize;
        let mut k2 = ENCODE_PAD;
        let a = usize::from(src[full]);
        if remainder == 2 {
            let b = usize::from(src[full + 1]);
            k1 = b >> 4;
            k2 = (b << 2) & 0x3F;
        }
        dst[o] = encode_map[a >> 2];
        dst[o + 1] = encode_map[(k1 | a << 4) & 0x3F];
        dst[o + 2] = encode_map[k2];
        dst[o + 3] = encode_map[ENCODE_PAD];
        o += 4;
    }
    debug_assert_eq!(o, encoded_size(length));
    o
}
