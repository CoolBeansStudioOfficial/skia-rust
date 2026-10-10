// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/core/SkBase64.h and src/core/SkBase64.cpp (chrome/m156)

//! Base64 encoding and decoding (`SkBase64`).
//!
//! The C++ decoder reads one byte past the end of its input when the input does not end with a
//! NUL byte. This port reads that byte as `0`, the terminator that Skia's callers supply.

/// The error of [`decode`] (`SkBase64::Error`, without `kNoError`, which is `Ok`).
#[doc(alias = "SkBase64::Error")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// A pad character came after fewer than two data characters (`kPadError`).
    PadError,
    /// A character is not in the base64 alphabet (`kBadCharError`).
    BadCharError,
}

/// The pad character's index in an encoding map (`EncodePad`).
// Port of: src/core/SkBase64.cpp#L11 (chrome/m156)
const ENCODE_PAD: usize = 64;

/// The value of a pad character in the decode table (`DecodePad`).
// Port of: src/core/SkBase64.cpp#L12 (chrome/m156)
const DECODE_PAD: i8 = -2;

// Port of: src/core/SkBase64.cpp#L14-L17 (chrome/m156)
const DEFAULT_ENCODE: &[u8; 65] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=";

// The value of each character from '+' (43) to 'z' (122), or -1 if it is not in the alphabet.
// Port of: src/core/SkBase64.cpp#L19-L28 (chrome/m156)
const DECODE_DATA: [i8; 80] = [
    62, -1, -1, -1, 63, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, -1, //
    -1, -1, -2, -1, -1, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, //
    10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, //
    -1, -1, -1, -1, -1, -1, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, //
    36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51,
];

/// Returns the length of the buffer that needs to be allocated to encode `src_data_length`
/// bytes (`SkBase64::EncodedSize`).
// Port of: src/core/SkBase64.h#L40-L46 (chrome/m156)
#[doc(alias = "EncodedSize")]
#[must_use]
pub fn encoded_size(src_data_length: usize) -> usize {
    // Take the floor of division by 3 to find the number of groups that need to be encoded.
    // Each group takes 4 bytes to be represented in base64.
    src_data_length.div_ceil(3) * 4
}

/// Base64 encodes `src` into `dst` (`SkBase64::Encode`), and returns the length it needs.
///
/// Normally this is called once with `dst` of `None` to get the required size, then again with
/// a buffer of that size. `encode_map` is `None` for the default encoding, or 65 characters whose
/// last one is the pad character. Encodings other than the default cannot be decoded.
///
/// # Panics
/// If `dst` is shorter than [`encoded_size`] of `src`.
// Port of: src/core/SkBase64.cpp#L122-L156 (chrome/m156)
#[doc(alias = "Encode")]
#[must_use]
pub fn encode(src: &[u8], dst: Option<&mut [u8]>, encode_map: Option<&[u8; 65]>) -> usize {
    let length = src.len();
    let encode = encode_map.unwrap_or(DEFAULT_ENCODE);
    if let Some(dst) = dst {
        let remainder = length % 3;
        let end = length - remainder;
        let mut out_index = 0usize;
        let (groups, _) = src[..end].as_chunks::<3>();
        for &[byte0, byte1, byte2] in groups {
            let sextet3 = byte2 & 0x3F;
            let sextet2 = ((byte2 >> 6) | (byte1 << 2)) & 0x3F;
            let sextet1 = ((byte1 >> 4) | (byte0 << 4)) & 0x3F;
            let sextet0 = byte0 >> 2;
            for index in [sextet0, sextet1, sextet2, sextet3] {
                dst[out_index] = encode[usize::from(index)];
                out_index += 1;
            }
        }
        if remainder > 0 {
            let mut k1 = 0u8;
            let mut k2 = ENCODE_PAD;
            let byte0 = src[end];
            if remainder == 2 {
                let byte1 = src[end + 1];
                k1 = byte1 >> 4;
                k2 = usize::from((byte1 << 2) & 0x3F);
            }
            for index in [
                encode[usize::from(byte0 >> 2)],
                encode[usize::from((k1 | (byte0 << 4)) & 0x3F)],
                encode[k2],
                encode[ENCODE_PAD],
            ] {
                dst[out_index] = index;
                out_index += 1;
            }
        }
    }
    encoded_size(length)
}

/// The three bytes of a group of four decoded characters (`one`, `two`, `three` in
/// `SkBase64::Decode`).
// Port of: src/core/SkBase64.cpp#L80-L93 (chrome/m156)
fn decode_group(bytes: [u8; 4]) -> (u8, u8, u8) {
    let mut one = bytes[0] << 2;
    let mut two = bytes[1];
    one |= two >> 4;
    two <<= 4;
    let mut three = bytes[2];
    two |= three >> 2;
    three <<= 6;
    three |= bytes[3];
    (one, two, three)
}

/// Base64 decodes `src` into `dst` (`SkBase64::Decode`), and returns the length of the result.
///
/// Normally this is called once with `dst` of `None` to get the required size, then again with
/// a buffer of that size. Whitespace is skipped, and a NUL byte ends the input.
///
/// # Errors
/// [`Error::PadError`] for a pad character after fewer than two data characters, and
/// [`Error::BadCharError`] for a character outside the alphabet.
///
/// # Panics
/// If `dst` is shorter than the decoded length.
// Port of: src/core/SkBase64.cpp#L30-L120 (chrome/m156)
#[doc(alias = "Decode")]
pub fn decode(src: &[u8], mut dst: Option<&mut [u8]>) -> Result<usize, Error> {
    // The byte at `s`, or the NUL terminator past the end.
    let at = |s: usize| src.get(s).copied().unwrap_or(0);
    let end = src.len();
    let mut s = 0usize;
    let mut i = 0usize;
    let mut pad_two = false;
    let mut pad_three = false;
    while s < end {
        let mut bytes = [0u8; 4];
        let mut byte = 0usize;
        let mut handle_pad = false;
        loop {
            let src_byte = at(s);
            s += 1;
            if src_byte == 0 {
                // goto goHome
                return Ok(i);
            }
            // Whitespace (`continue`) is skipped; anything else is decoded.
            if src_byte > b' ' {
                if !(b'+'..=b'z').contains(&src_byte) {
                    return Err(Error::BadCharError);
                }
                let decoded = DECODE_DATA[usize::from(src_byte - b'+')];
                // `bytes[byte] = decoded` converts the signed value to an unsigned char.
                bytes[byte] = decoded.cast_unsigned();
                if decoded < 0 {
                    if decoded == DECODE_PAD {
                        handle_pad = true;
                        break;
                    }
                    return Err(Error::BadCharError);
                }
                byte += 1;
                if at(s) == 0 {
                    if byte == 4 {
                        break;
                    }
                    handle_pad = true;
                    break;
                }
            }
            // The `while (byte < 4)` of the do-while.
            if byte >= 4 {
                break;
            }
        }
        if handle_pad {
            if byte < 2 {
                return Err(Error::PadError);
            }
            pad_three = true;
            if byte == 2 {
                pad_two = true;
            }
        }
        let (one, two, three) = decode_group(bytes);
        if let Some(out) = dst.as_deref_mut() {
            out[i] = one;
        }
        i += 1;
        if pad_two {
            break;
        }
        if let Some(out) = dst.as_deref_mut() {
            out[i] = two;
        }
        i += 1;
        if pad_three {
            break;
        }
        if let Some(out) = dst.as_deref_mut() {
            out[i] = three;
        }
        i += 1;
    }
    // goHome
    Ok(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The RFC 4648 section 10 test vectors, which the Skia test does not check against.
    #[test]
    fn known_vectors() {
        for (plain, encoded) in [
            (&b""[..], &b""[..]),
            (b"f", b"Zg=="),
            (b"fo", b"Zm8="),
            (b"foo", b"Zm9v"),
            (b"foob", b"Zm9vYg=="),
            (b"fooba", b"Zm9vYmE="),
            (b"foobar", b"Zm9vYmFy"),
        ] {
            let mut dst = vec![0u8; encoded_size(plain.len())];
            assert_eq!(encode(plain, Some(&mut dst), None), dst.len());
            assert_eq!(dst, encoded);
            let mut out = vec![0u8; plain.len()];
            assert_eq!(decode(encoded, Some(&mut out)), Ok(plain.len()));
            assert_eq!(out, plain);
        }
    }

    #[test]
    fn errors_and_whitespace() {
        assert_eq!(decode(b"Z", None), Err(Error::PadError));
        assert_eq!(decode(b"Z!==", None), Err(Error::BadCharError));
        assert_eq!(decode(b"Zm 9v\n", None), Ok(3));
    }
}
