// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLString.cpp (`skstd::to_string`, the float and double overloads).

//! `skstd::to_string` for floats: iostream's `%g` with precision 7, widened to 9 (float) or 17
//! (double) digits when the text does not round-trip, with `.0` appended when the text has no `.`
//! or `e`.

// Port of: src/sksl/SkSLString.cpp#L23-L56 (chrome/m156)

/// Formats `value` the way iostream's `%.{precision}g` does, which is also C's `%.{precision}g`:
/// `precision` significant digits, fixed notation for exponents in `[-4, precision)`, scientific
/// otherwise, with trailing zeros removed. Rust's `{:e}` rounds the exact binary value correctly,
/// as glibc and MSVC do. `precision` must be between 1 and 17.
#[must_use]
pub(crate) fn format_g(value: f64, precision: usize) -> String {
    if value.is_nan() {
        return if value.is_sign_negative() {
            "-nan".to_owned()
        } else {
            "nan".to_owned()
        };
    }
    if value.is_infinite() {
        return if value < 0.0 {
            "-inf".to_owned()
        } else {
            "inf".to_owned()
        };
    }

    // The decimal exponent after rounding to `precision` significant digits.
    let sci = format!("{:.*e}", precision - 1, value);
    let (mantissa, exp_text) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exponent: i32 = exp_text.parse().unwrap_or(0);

    let precision_i32 = i32::try_from(precision).unwrap_or(i32::MAX);
    let mut text = if exponent < -4 || exponent >= precision_i32 {
        // Scientific: strip trailing zeros from the mantissa, then `e±XX`.
        let mut mantissa = mantissa.to_owned();
        if mantissa.contains('.') {
            while mantissa.ends_with('0') {
                mantissa.pop();
            }
            if mantissa.ends_with('.') {
                mantissa.pop();
            }
        }
        let sign = if exponent < 0 { '-' } else { '+' };
        let digits = exponent.unsigned_abs();
        format!("{mantissa}e{sign}{digits:02}")
    } else {
        // Fixed: `precision - 1 - exponent` digits after the point, then strip trailing zeros.
        let decimals = usize::try_from(precision_i32 - 1 - exponent).unwrap_or(0);
        let mut fixed = format!("{value:.decimals$}");
        if fixed.contains('.') {
            while fixed.ends_with('0') {
                fixed.pop();
            }
            if fixed.ends_with('.') {
                fixed.pop();
            }
        }
        fixed
    };
    // `-0.0` prints as `-0` in `%g`; the formatting above already produced that.
    if text.is_empty() {
        text.push('0');
    }
    text
}

/// `skstd::to_string(float)`: `to_string_impl<float, 9>`.
#[doc(alias = "skstd::to_string")]
#[must_use]
// `(float)roundtripped` and `value != (float)roundtripped` are Skia's own conversion and
// comparison, reproduced exactly: the round-trip test is the point.
#[allow(clippy::cast_possible_truncation, clippy::float_cmp)]
pub fn to_string_f32(value: f32) -> String {
    let value = f64::from(value);
    let mut text = format_g(value, 7);

    // If it does not round-trip, use the 9 digits that always do for a float.
    let roundtripped: f64 = text.parse().unwrap_or(f64::NAN);
    if (value as f32) != (roundtripped as f32) && value.is_finite() {
        text = format_g(value, 9);
    }
    append_point_zero(text)
}

/// `skstd::to_string(double)`: `to_string_impl<double, 17>`.
#[doc(alias = "skstd::to_string")]
#[must_use]
// The round-trip test compares doubles exactly, as Skia does.
#[allow(clippy::float_cmp)]
pub fn to_string_f64(value: f64) -> String {
    let mut text = format_g(value, 7);

    let roundtripped: f64 = text.parse().unwrap_or(f64::NAN);
    if value != roundtripped && value.is_finite() {
        text = format_g(value, 17);
    }
    append_point_zero(text)
}

/// "We need to emit a decimal point to distinguish floats from ints."
fn append_point_zero(mut text: String) -> String {
    if !text.contains('.') && !text.contains('e') {
        text.push_str(".0");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{to_string_f32, to_string_f64};

    #[test]
    fn floats_print_like_skia() {
        assert_eq!(to_string_f32(1.0), "1.0");
        assert_eq!(to_string_f32(0.0), "0.0");
        assert_eq!(to_string_f32(-0.0), "-0.0");
        assert_eq!(to_string_f32(0.5), "0.5");
        assert_eq!(to_string_f32(1e10), "1e+10");
        assert_eq!(to_string_f32(1.5e-5), "1.5e-05");
        // Seven digits do not round-trip this float, so nine are used.
        assert_eq!(to_string_f32(123_456_789.0), "123456792.0");
    }

    #[test]
    fn doubles_print_like_skia() {
        assert_eq!(to_string_f64(1.0), "1.0");
        assert_eq!(to_string_f64(0.1), "0.1");
        assert_eq!(to_string_f64(-0.0), "-0.0");
        assert_eq!(to_string_f64(1e10), "1e+10");
        // Seven digits do not round-trip 0.1 + 0.2, so seventeen are used.
        assert_eq!(to_string_f64(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(to_string_f64(1.0 / 3.0), "0.33333333333333331");
    }

    /// Values whose text was produced by Skia's `to_string_impl` (`src/sksl/SkSLString.cpp`), run
    /// through a C++ harness at the pin. Bit patterns are given so that no decimal parsing is
    /// involved.
    #[test]
    fn matches_skia_text_for_float_bit_patterns() {
        let cases_f32: [(u32, &str); 12] = [
            (0x0000_0000, "0.0"),
            (0x8000_0000, "-0.0"),
            (0x5015_02f9, "1e+10"),
            (0x377b_a882, "1.5e-05"),
            (0x4ceb_79a3, "123456792.0"),
            (0x3eaa_aaab, "0.333333343"),
            (0x0000_0001, "1.401298e-45"),
            (0x7f7f_ffff, "3.40282347e+38"),
            (0x4b80_0000, "16777216.0"),
            (0x4b18_967f, "9999999.0"),
            (0x38d1_b717, "0.0001"),
            (0x3983_126f, "0.00025"),
        ];
        for (bits, text) in cases_f32 {
            assert_eq!(to_string_f32(f32::from_bits(bits)), text, "bits {bits:08x}");
        }
        let cases_f64: [(u64, &str); 6] = [
            (0x3fb9_9999_9999_999a, "0.1"),
            (0x3fd5_5555_5555_5555, "0.33333333333333331"),
            (0x79c3_287c_f79f_1963, "3.3960942232704952e+278"),
            (0x3225_c5d8_9696_14b6, "4.0379968834669047e-67"),
            (0xb749_04c3_f9fd_6f4d, "-2.2437470265414783e-42"),
            (0x0000_0000_0000_0001, "4.940656e-324"),
        ];
        for (bits, text) in cases_f64 {
            assert_eq!(
                to_string_f64(f64::from_bits(bits)),
                text,
                "bits {bits:016x}"
            );
        }
    }
}
