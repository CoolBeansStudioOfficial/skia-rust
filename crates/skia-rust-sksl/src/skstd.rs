// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLString.cpp (`skstd::to_string`, the float overload).

//! `skstd::to_string` for floats: iostream's `%g` with precision 7, widened to 9 digits when the
//! text does not round-trip, with `.0` appended when the text has no `.` or `e`.

// Port of: src/sksl/SkSLString.cpp#L23-L56 (chrome/m156)

/// Formats `value` the way iostream's `%.{precision}g` does: `precision` significant digits,
/// fixed notation for exponents in `[-4, precision)`, scientific otherwise, with trailing zeros
/// removed. Rust's `{:e}` rounds the exact binary value correctly, as glibc and MSVC do.
// `precision` is at most 9 here, so the `i32` conversions are exact.
#[allow(clippy::cast_possible_truncation)] // precision is 7 or 9
fn format_g(value: f32, precision: usize) -> String {
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

/// `skstd::to_string(float)`.
#[doc(alias = "skstd::to_string")]
#[must_use]
// `(float)roundtripped` and `value != (float)roundtripped` are Skia's own conversion and
// comparison, reproduced exactly: the round-trip test is the point.
#[allow(clippy::cast_possible_truncation, clippy::float_cmp)]
pub fn to_string_f32(value: f32) -> String {
    let mut text = format_g(value, 7);

    // If it does not round-trip, use the 9 digits that always do for a float.
    let roundtripped: f64 = text.parse().unwrap_or(f64::NAN);
    if value != roundtripped as f32 && value.is_finite() {
        text = format_g(value, 9);
    }

    // We need to emit a decimal point to distinguish floats from ints.
    if !text.contains('.') && !text.contains('e') {
        text.push_str(".0");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::to_string_f32;

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
}
