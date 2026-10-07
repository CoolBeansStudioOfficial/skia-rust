// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkString.h, src/core/SkString.cpp (the number-to-text helpers)

//! Skia's number-to-text helpers (`SkStrAppend*`, `SkString::appendHex`).
//!
//! `SkString` itself is not ported (Rust uses [`String`]); these functions are, because their
//! output format is part of Skia's observable behaviour (`SkWStream::writeDecAsText` and
//! friends, SVG/PDF output, ...). They append to a [`String`] instead of writing into a `char`
//! buffer, so the `kSkStrAppend*_MaxSize` buffer constants are not needed.

use crate::scalar::scalar;

/// `SkHexadecimalDigits::gUpper`.
// Port of: src/core/SkUtils.cpp (chrome/m156)
const HEX_DIGITS_UPPER: [u8; 16] = *b"0123456789ABCDEF";

/// Appends `dec` in decimal.
// Port of: src/core/SkString.cpp#L100-L118 (chrome/m156)
#[doc(alias = "SkStrAppendU32")]
pub fn str_append_u32(string: &mut String, dec: u32) {
    str_append_u64(string, u64::from(dec), 0);
}

/// Appends `dec` in decimal, with a leading `-` if negative.
// Port of: src/core/SkString.cpp#L120-L127 (chrome/m156)
#[doc(alias = "SkStrAppendS32")]
pub fn str_append_s32(string: &mut String, dec: i32) {
    let mut udec = dec.cast_unsigned(); // mirrors `uint32_t udec = dec;`
    if dec < 0 {
        string.push('-');
        udec = (!udec).wrapping_add(1); // udec = -udec
    }
    str_append_u32(string, udec);
}

/// Appends `dec` in decimal, left-padded with zeros to at least `min_digits` digits.
// Port of: src/core/SkString.cpp#L129-L153 (chrome/m156)
#[doc(alias = "SkStrAppendU64")]
pub fn str_append_u64(string: &mut String, mut dec: u64, mut min_digits: i32) {
    let mut buffer = [0u8; K_STR_APPEND_U64_MAX_SIZE];
    let mut p = buffer.len();

    loop {
        p -= 1;
        buffer[p] = b'0' + (dec % 10) as u8; // dec % 10 < 10
        dec /= 10;
        min_digits -= 1;
        if dec == 0 {
            break;
        }
    }

    while min_digits > 0 {
        p -= 1;
        buffer[p] = b'0';
        min_digits -= 1;
    }

    string.extend(buffer[p..].iter().map(|&b| char::from(b)));
}

/// Appends `dec` in decimal, with a leading `-` if negative, zero-padded to `min_digits`.
// Port of: src/core/SkString.cpp#L155-L162 (chrome/m156)
#[doc(alias = "SkStrAppendS64")]
pub fn str_append_s64(string: &mut String, dec: i64, min_digits: i32) {
    let mut udec = dec.cast_unsigned(); // mirrors `uint64_t udec = dec;`
    if dec < 0 {
        string.push('-');
        udec = (!udec).wrapping_add(1); // udec = -udec
    }
    str_append_u64(string, udec, min_digits);
}

/// Largest buffer `SkStrAppendU32` needs.
// Port of: include/core/SkString.h#L84 (chrome/m156)
pub const K_STR_APPEND_U32_MAX_SIZE: usize = 10;
/// Largest buffer `SkStrAppendU64` needs.
// Port of: include/core/SkString.h#L86 (chrome/m156)
pub const K_STR_APPEND_U64_MAX_SIZE: usize = 20;
/// Largest buffer `SkStrAppendS32` needs.
// Port of: include/core/SkString.h#L89 (chrome/m156)
pub const K_STR_APPEND_S32_MAX_SIZE: usize = K_STR_APPEND_U32_MAX_SIZE + 1;
/// Largest buffer `SkStrAppendS64` needs.
// Port of: include/core/SkString.h#L91 (chrome/m156)
pub const K_STR_APPEND_S64_MAX_SIZE: usize = K_STR_APPEND_U64_MAX_SIZE + 1;
/// Largest buffer `SkStrAppendScalar` needs: floats have at most 8 significant digits, but the
/// total string could be 15 characters (`-1.2345678e-005`).
// Port of: include/core/SkString.h#L101 (chrome/m156)
pub const K_STR_APPEND_SCALAR_MAX_SIZE: usize = 15;

/// Appends the scalar in decimal format, as C's `%.8g` would print it (floats have at most 8
/// significant digits), except that NaN and infinities print as `nan`, `inf` and `-inf` on every
/// platform.
// Port of: src/core/SkString.cpp#L164-L190 (chrome/m156)
#[doc(alias = "SkStrAppendScalar")]
pub fn str_append_scalar(string: &mut String, value: scalar) {
    // Handle infinity and NaN ourselves to ensure consistent cross-platform results.
    // (e.g.: `inf` versus `1.#INF00`, `nan` versus `-nan` for high-bit-set NaNs)
    if value.is_nan() {
        string.push_str("nan");
        return;
    }
    if !value.is_finite() {
        string.push_str(if value > 0.0 { "inf" } else { "-inf" });
        return;
    }

    // since floats have at most 8 significant digits, we limit our %g to that.
    let text = format_g(f64::from(value), 8);
    debug_assert!(text.len() <= K_STR_APPEND_SCALAR_MAX_SIZE);
    string.push_str(&text);
}

/// C's `printf("%.{precision}g", value)` for a finite `value`.
///
/// skia-rust: the correctly rounded decimal digits come from Rust's exact `{:e}` formatting
/// (C's `printf` rounds exactly too); the `%g` layout rules (fixed vs exponent notation,
/// trailing zero removal, two-digit exponent) are those of C99 7.21.6.1.
fn format_g(value: f64, precision: usize) -> String {
    let mut out = String::new();
    if value.is_sign_negative() {
        out.push('-');
    }
    let magnitude = value.abs();
    if magnitude == 0.0 {
        out.push('0');
        return out;
    }

    let scientific = format!("{:.*e}", precision - 1, magnitude);
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("`{:e}` output has an exponent");
    let exponent: i32 = exponent.parse().expect("`{:e}` exponent is an integer");
    let digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
    debug_assert_eq!(digits.len(), precision);

    let precision_i32 = i32::try_from(precision).expect("small precision");
    if exponent >= -4 && exponent < precision_i32 {
        // Fixed notation with `precision - 1 - exponent` decimals, trailing zeros removed.
        if exponent >= 0 {
            let int_len = usize::try_from(exponent).expect("non-negative") + 1;
            out.extend(digits[..int_len].iter().map(|&b| char::from(b)));
            push_fraction(&mut out, &digits[int_len..]);
        } else {
            out.push('0');
            let zeros = usize::try_from(-exponent - 1).expect("non-negative");
            let mut fraction = vec![b'0'; zeros];
            fraction.extend_from_slice(&digits);
            push_fraction(&mut out, &fraction);
        }
    } else {
        // Exponent notation.
        out.push(char::from(digits[0]));
        push_fraction(&mut out, &digits[1..]);
        out.push('e');
        out.push(if exponent < 0 { '-' } else { '+' });
        let abs_exponent = exponent.unsigned_abs();
        if abs_exponent < 10 {
            out.push('0');
        }
        out.push_str(&abs_exponent.to_string());
    }
    out
}

/// Pushes `.` and the digits of `fraction`, without trailing zeros (nothing if all are zeros).
fn push_fraction(out: &mut String, fraction: &[u8]) {
    let end = fraction
        .iter()
        .rposition(|&b| b != b'0')
        .map_or(0, |last| last + 1);
    if end > 0 {
        out.push('.');
        out.extend(fraction[..end].iter().map(|&b| char::from(b)));
    }
}

/// Appends `hex` as upper-case hexadecimal, zero-padded to at least `min_digits` digits
/// (pinned to `0..=8`). This is the text `SkString::appendHex` produces.
// Port of: src/core/SkString.cpp#L506-L525 (chrome/m156)
#[doc(alias = "appendHex")]
#[doc(alias = "insertHex")]
pub fn str_append_hex(string: &mut String, mut hex: u32, min_digits: i32) {
    let mut min_digits = min_digits.clamp(0, 8);

    let mut buffer = [0u8; 8];
    let mut p = buffer.len();

    loop {
        p -= 1;
        buffer[p] = HEX_DIGITS_UPPER[(hex & 0xF) as usize];
        hex >>= 4;
        min_digits -= 1;
        if hex == 0 {
            break;
        }
    }

    loop {
        min_digits -= 1;
        if min_digits < 0 {
            break;
        }
        p -= 1;
        buffer[p] = b'0';
    }

    string.extend(buffer[p..].iter().map(|&b| char::from(b)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_text(value: scalar) -> String {
        let mut s = String::new();
        str_append_scalar(&mut s, value);
        s
    }

    #[test]
    fn scalar_matches_printf_8g() {
        assert_eq!(scalar_text(0.0), "0");
        assert_eq!(scalar_text(-0.0), "-0");
        assert_eq!(scalar_text(1.0), "1");
        assert_eq!(scalar_text(-0.5), "-0.5");
        // 1234.5678f32 is 1234.5677490234375
        assert_eq!(scalar_text(1234.5678), "1234.5677");
        assert_eq!(scalar_text(0.0625), "0.0625");
        // 0.0001f32 is 9.99999974737875e-05, below the 1e-4 limit of fixed notation.
        assert_eq!(scalar_text(0.0001), "9.9999997e-05");
        assert_eq!(scalar_text(6.103_515_6e-5), "6.1035156e-05");
        assert_eq!(scalar_text(12_345_678.0), "12345678");
        assert_eq!(scalar_text(100_000_000.0), "1e+08");
        assert_eq!(scalar_text(123_456_789.0), "1.2345679e+08");
        assert_eq!(scalar_text(0.1), "0.1");
        assert_eq!(scalar_text(1.0 / 3.0), "0.33333334");
        assert_eq!(scalar_text(scalar::NAN), "nan");
        assert_eq!(scalar_text(scalar::INFINITY), "inf");
        assert_eq!(scalar_text(scalar::NEG_INFINITY), "-inf");
        assert_eq!(scalar_text(f32::MAX), "3.4028235e+38");
    }

    #[test]
    fn integers_and_hex() {
        let mut s = String::new();
        str_append_s32(&mut s, i32::MIN);
        s.push(' ');
        str_append_u64(&mut s, 42, 5);
        s.push(' ');
        str_append_s64(&mut s, -7, 3);
        s.push(' ');
        str_append_hex(&mut s, 0xBEEF, 6);
        s.push(' ');
        str_append_hex(&mut s, 0, 0);
        assert_eq!(s, "-2147483648 00042 -007 00BEEF 0");
    }
}
