// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkUTF.h, src/core/SkUTF.cpp

//! UTF-8/16/32 counting, decoding and encoding with Skia's exact handling of invalid input.
//!
//! Skia's pointer pairs `(*ptr, end)` become slices: the slice passed to the `next_*`
//! functions is the remaining input (`*ptr..end`), and is advanced in place. On failure it is
//! set to the empty slice (Skia sets `*ptr = end`). Lengths of UTF-16/UTF-32 inputs are in
//! code units rather than bytes, so Skia's alignment and odd-byte-length failures cannot occur.

/// `SkUnichar`.
#[doc(alias = "SkUnichar")]
pub type Unichar = i32;

/// `SkUTF::kMaxBytesInUTF8Sequence`.
#[doc(alias = "kMaxBytesInUTF8Sequence")]
pub const K_MAX_BYTES_IN_UTF8_SEQUENCE: usize = 4;

const K_INVALID_UNICHAR_MASK: u32 = 0xFF00_0000; // unichar fits in 24 bits

// Port of: src/core/SkUTF.cpp#L8-L10 (chrome/m156)
fn left_shift(value: i32, shift: i32) -> i32 {
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // mirrors the C++ casts
    {
        ((value as u32) << shift) as i32
    }
}

// Port of: src/core/SkUTF.cpp#L16-L18 (chrome/m156)
fn utf16_is_high_surrogate(c: u16) -> bool {
    (c & 0xFC00) == 0xD800
}

fn utf16_is_low_surrogate(c: u16) -> bool {
    (c & 0xFC00) == 0xDC00
}

/// Returns -1 iff invalid UTF8 byte, 0 iff UTF8 continuation byte, 1 iff ASCII byte, 2 iff
/// leading byte of 2-byte sequence, 3 iff leading byte of 3-byte sequence, and 4 iff leading
/// byte of 4-byte sequence. I.e.: if return value > 0, then gives length of sequence.
// Port of: src/core/SkUTF.cpp#L20-L40 (chrome/m156)
#[allow(clippy::cast_possible_wrap)] // (0xe5 << 24) wraps negative in C++
fn utf8_byte_type(c: u8) -> i32 {
    if c < 0x80 {
        1
    } else if c < 0xC0 {
        0
    } else if c >= 0xF5 || (c & 0xFE) == 0xC0 {
        // "octet values c0, c1, f5 to ff never appear"
        -1
    } else {
        // (0xe5 << 24) overflows int in C++ and wraps to a negative value.
        let shift = u32::from(c) >> 4 << 1;
        (((0xe500_0000_u32 as i32) >> shift) & 3) + 1
    }
}

// Port of: src/core/SkUTF.cpp#L41-L43 (chrome/m156)
fn utf8_type_is_valid_leading_byte(ty: i32) -> bool {
    ty > 0
}

fn utf8_byte_is_continuation(c: u8) -> bool {
    utf8_byte_type(c) == 0
}

/// Given a sequence of UTF-8 bytes, returns the number of unicode codepoints.
/// If the sequence is invalid UTF-8, returns -1.
// Port of: src/core/SkUTF.cpp#L47-L68 (chrome/m156)
#[doc(alias = "CountUTF8")]
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)] // count mirrors C++ int; ty > 0 here
pub fn count_utf8(utf8: &[u8]) -> i32 {
    let mut count: i32 = 0;
    let stop = utf8.len();
    let mut i = 0;
    while i < stop {
        let mut ty = utf8_byte_type(utf8[i]);
        if !utf8_type_is_valid_leading_byte(ty) || i + ty as usize > stop {
            return -1; // Sequence extends beyond end.
        }
        while ty > 1 {
            ty -= 1;
            i += 1;
            if !utf8_byte_is_continuation(utf8[i]) {
                return -1;
            }
        }
        i += 1;
        count += 1;
    }
    count
}

/// Given a sequence of UTF-16 code units in machine-endian form, returns the number of unicode
/// codepoints. If the sequence is invalid UTF-16, returns -1.
// Port of: src/core/SkUTF.cpp#L70-L94 (chrome/m156)
#[doc(alias = "CountUTF16")]
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // count mirrors C++ int
pub fn count_utf16(utf16: &[u16]) -> i32 {
    let stop = utf16.len();
    let mut src = 0;
    let mut count: i32 = 0;
    while src < stop {
        let mut c = utf16[src];
        src += 1;
        if utf16_is_low_surrogate(c) {
            return -1;
        }
        if utf16_is_high_surrogate(c) {
            if src >= stop {
                return -1;
            }
            c = utf16[src];
            src += 1;
            if !utf16_is_low_surrogate(c) {
                return -1;
            }
        }
        count += 1;
    }
    count
}

/// Given a sequence of UTF-32 code units in machine-endian form, returns the number of unicode
/// codepoints. If the sequence is invalid UTF-32, returns -1.
// Port of: src/core/SkUTF.cpp#L96-L110 (chrome/m156)
#[doc(alias = "CountUTF32")]
#[must_use]
pub fn count_utf32(utf32: &[i32]) -> i32 {
    let Ok(len) = i32::try_from(utf32.len()) else {
        return -1;
    };
    for &v in utf32 {
        #[allow(clippy::cast_sign_loss)] // mirrors reading the int32 as uint32
        if (v as u32) & K_INVALID_UNICHAR_MASK != 0 {
            return -1;
        }
    }
    len
}

// Port of: src/core/SkUTF.cpp#L112-L116 (chrome/m156)
fn next_fail<T>(ptr: &mut &[T]) -> Unichar {
    *ptr = &ptr[ptr.len()..];
    -1
}

/// Given a sequence of UTF-8 bytes, returns the first unicode codepoint. The slice is advanced
/// to point at the next codepoint's start. If invalid UTF-8 is encountered, the slice is set to
/// empty and -1 is returned.
// Port of: src/core/SkUTF.cpp#L118-L152 (chrome/m156)
#[doc(alias = "NextUTF8")]
pub fn next_utf8(ptr: &mut &[u8]) -> Unichar {
    let s: &[u8] = ptr;
    if s.is_empty() {
        return next_fail(ptr);
    }
    let mut p = 0;
    let mut c = i32::from(s[p]);
    let mut hic = left_shift(c, 24);

    if !utf8_type_is_valid_leading_byte(utf8_byte_type(s[p])) {
        return next_fail(ptr);
    }
    if hic < 0 {
        let mut mask: u32 = !0x3F;
        hic = left_shift(hic, 1);
        loop {
            p += 1;
            if p >= s.len() {
                return next_fail(ptr);
            }
            // check before reading off end of array.
            let next_byte = s[p];
            if !utf8_byte_is_continuation(next_byte) {
                return next_fail(ptr);
            }
            c = (c << 6) | i32::from(next_byte & 0x3F);
            mask <<= 5;
            hic = left_shift(hic, 1);
            if hic >= 0 {
                break;
            }
        }
        #[allow(clippy::cast_possible_wrap)] // mirrors `c &= ~mask` on int/uint32
        {
            c &= !mask as i32;
        }
    }
    *ptr = &s[p + 1..];
    c
}

/// Like [`next_utf8`], but returns the replacement character (0xFFFD) on invalid input.
// Port of: src/core/SkUTF.cpp#L154-L157 (chrome/m156)
#[doc(alias = "NextUTF8WithReplacement")]
pub fn next_utf8_with_replacement(ptr: &mut &[u8]) -> Unichar {
    let val = next_utf8(ptr);
    if val < 0 { 0xFFFD } else { val }
}

/// Given a sequence of UTF-16 code units, returns the first unicode codepoint. The slice is
/// advanced to point at the next codepoint's start. If invalid UTF-16 is encountered, the
/// slice is set to empty and -1 is returned.
// Port of: src/core/SkUTF.cpp#L159-L195 (chrome/m156)
#[doc(alias = "NextUTF16")]
pub fn next_utf16(ptr: &mut &[u16]) -> Unichar {
    let s: &[u16] = ptr;
    if s.is_empty() {
        return next_fail(ptr);
    }
    let mut src = 0;
    let c = s[src];
    src += 1;
    let mut result = Unichar::from(c);
    if utf16_is_low_surrogate(c) {
        return next_fail(ptr); // srcPtr should never point at low surrogate.
    }
    if utf16_is_high_surrogate(c) {
        if src >= s.len() {
            return next_fail(ptr); // Truncated string.
        }
        let low = s[src];
        src += 1;
        if !utf16_is_low_surrogate(low) {
            return next_fail(ptr);
        }
        // unicode = (high - 0xD800) * 0x400 + low - 0xDC00 + 0x10000
        //         = (high << 10) + low - ((0xD800 << 10) + 0xDC00 - 0x10000)
        result = (result << 10) + Unichar::from(low) - ((0xD800 << 10) + 0xDC00 - 0x10000);
    }
    *ptr = &s[src..];
    result
}

/// Given a sequence of UTF-32 code units, returns the first unicode codepoint. The slice is
/// advanced to point at the next codepoint's start. If invalid UTF-32 is encountered, the
/// slice is set to empty and -1 is returned.
// Port of: src/core/SkUTF.cpp#L197-L212 (chrome/m156)
#[doc(alias = "NextUTF32")]
pub fn next_utf32(ptr: &mut &[i32]) -> Unichar {
    let s: &[i32] = ptr;
    if s.is_empty() {
        return next_fail(ptr);
    }
    let value = s[0];
    #[allow(clippy::cast_sign_loss)] // mirrors `value & kInvalidUnicharMask`
    if (value as u32) & K_INVALID_UNICHAR_MASK != 0 {
        return next_fail(ptr);
    }
    *ptr = &s[1..];
    value
}

/// Converts the unicode codepoint into UTF-8. If `utf8` is `Some`, places the result at its
/// start (it must hold at least as many bytes as the result). Returns the number of bytes in
/// the result. If `utf8` is `None`, simply returns the number of bytes that would be used. For
/// invalid unicode codepoints, returns 0.
// Port of: src/core/SkUTF.cpp#L214-L241 (chrome/m156)
#[doc(alias = "ToUTF8")]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // mirrors the C++ casts
#[must_use]
pub fn to_utf8(mut uni: Unichar, utf8: Option<&mut [u8]>) -> usize {
    if (uni as u32) > 0x0010_FFFF {
        return 0;
    }
    if uni <= 127 {
        if let Some(utf8) = utf8 {
            utf8[0] = uni as u8;
        }
        return 1;
    }
    let mut tmp = [0u8; 4];
    let mut p = 0;
    let mut count: usize = 1;
    while uni > (0x7F >> count) {
        tmp[p] = (0x80 | (uni & 0x3F)) as u8;
        p += 1;
        uni >>= 6;
        count += 1;
    }
    if let Some(utf8) = utf8 {
        let mut out = count;
        for &b in &tmp[..count - 1] {
            out -= 1;
            utf8[out] = b;
        }
        out -= 1;
        utf8[out] = (!(0xFF >> count) | uni) as u8;
    }
    count
}

/// Converts the unicode codepoint into UTF-16. If `utf16` is `Some`, places the result at its
/// start (it must hold at least as many units as the result). Returns the number of UTF-16 code
/// units in the result (1 or 2). If `utf16` is `None`, simply returns the number of code units
/// that would be used. For invalid unicode codepoints, returns 0.
// Port of: src/core/SkUTF.cpp#L243-L257 (chrome/m156)
#[doc(alias = "ToUTF16")]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // mirrors the C++ casts
#[must_use]
pub fn to_utf16(uni: Unichar, utf16: Option<&mut [u16]>) -> usize {
    if (uni as u32) > 0x0010_FFFF {
        return 0;
    }
    let extra = usize::from(uni > 0xFFFF);
    if let Some(utf16) = utf16 {
        if extra != 0 {
            utf16[0] = ((0xD800 - 64) + (uni >> 10)) as u16;
            utf16[1] = (0xDC00 | (uni & 0x3FF)) as u16;
        } else {
            utf16[0] = uni as u16;
        }
    }
    1 + extra
}

/// Returns the number of resulting UTF-16 values needed to convert the `src` UTF-8 sequence.
/// If `dst` is `Some`, it is filled with the corresponding values up to its capacity (its
/// length). If there is an error, -1 is returned and the `dst` buffer is undefined.
// Port of: src/core/SkUTF.cpp#L259-L289 (chrome/m156)
#[doc(alias = "UTF8ToUTF16")]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // lengths mirror C++ int
#[must_use]
pub fn utf8_to_utf16(mut dst: Option<&mut [u16]>, src: &[u8]) -> i32 {
    let mut dst_length: i32 = 0;
    let mut dst_pos = 0;
    let mut src = src;
    while !src.is_empty() {
        let uni = next_utf8(&mut src);
        if uni < 0 {
            return -1;
        }

        let mut utf16 = [0u16; 2];
        let mut count = to_utf16(uni, Some(&mut utf16));
        if count == 0 {
            return -1;
        }
        dst_length += count as i32;

        if let Some(dst) = dst.as_deref_mut() {
            let mut elems = 0;
            while dst_pos < dst.len() && count > 0 {
                dst[dst_pos] = utf16[elems];
                dst_pos += 1;
                elems += 1;
                count -= 1;
            }
        }
    }
    dst_length
}

/// Returns the number of resulting UTF-8 values needed to convert the `src` UTF-16 sequence.
/// If `dst` is `Some`, it is filled with the corresponding values up to its capacity (its
/// length). If there is an error, -1 is returned and the `dst` buffer is undefined.
// Port of: src/core/SkUTF.cpp#L291-L321 (chrome/m156)
#[doc(alias = "UTF16ToUTF8")]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // lengths mirror C++ int
#[must_use]
pub fn utf16_to_utf8(mut dst: Option<&mut [u8]>, src: &[u16]) -> i32 {
    let mut dst_length: i32 = 0;
    let mut dst_pos = 0;
    let mut src = src;
    while !src.is_empty() {
        let uni = next_utf16(&mut src);
        if uni < 0 {
            return -1;
        }

        let mut utf8 = [0u8; K_MAX_BYTES_IN_UTF8_SEQUENCE];
        let mut count = to_utf8(uni, Some(&mut utf8));
        if count == 0 {
            return -1;
        }
        dst_length += count as i32;

        if let Some(dst) = dst.as_deref_mut() {
            let mut elems = 0;
            while dst_pos < dst.len() && count > 0 {
                dst[dst_pos] = utf8[elems];
                dst_pos += 1;
                elems += 1;
                count -= 1;
            }
        }
    }
    dst_length
}

/// Given a UTF-16 code unit, returns true iff it is a leading surrogate.
// Port of: src/core/SkUTF.h#L91 (chrome/m156)
#[doc(alias = "IsLeadingSurrogateUTF16")]
#[must_use]
pub fn is_leading_surrogate_utf16(c: u16) -> bool {
    (c & 0xFC00) == 0xD800
}

/// Given a UTF-16 code unit, returns true iff it is a trailing surrogate.
// Port of: src/core/SkUTF.h#L97 (chrome/m156)
#[doc(alias = "IsTrailingSurrogateUTF16")]
#[must_use]
pub fn is_trailing_surrogate_utf16(c: u16) -> bool {
    (c & 0xFC00) == 0xDC00
}
