// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkStringUtils.h, src/core/SkStringUtils.cpp, src/core/SkString.cpp
// (SkStrAppendScalar)

//! Scalar-to-text helpers (`SkStringUtils.h`, `SkStrAppendScalar`). `SkString` itself is replaced
//! by `String`.

use crate::float_bits::float_to_bits;
use crate::floating_point::{is_finite, is_nan};
use crate::scalar::scalar;

/// How [`append_scalar`] formats a value.
// Port of: src/core/SkStringUtils.h#L17-L20 (chrome/m156)
#[doc(alias = "SkScalarAsStringType")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ScalarAsStringType {
    Dec,
    Hex,
}

/// C's `printf("%.{precision}g", v)`.
///
/// Rust's float formatting is correctly rounded, as are the C libraries Skia is tested with, so
/// the digits match; this reproduces `%g`'s choice between fixed and exponential notation and its
/// trailing-zero removal.
///
/// # Panics
///
/// Never panics for finite or non-finite input: the exponent always follows the `e` that
/// `{:e}` formatting writes.
#[must_use]
pub fn format_g(v: f64, precision: usize) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan" } else { "nan" }.to_string();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    let p = precision.max(1);
    // The exponent after rounding to `p` significant digits.
    let e_str = format!("{:.*e}", p - 1, v);
    let exp_pos = e_str.rfind('e').expect("exponent");
    let x: i32 = e_str[exp_pos + 1..].parse().expect("exponent value");
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)] // small precision
    let p_i = p as i32;
    if p_i > x && x >= -4 {
        #[allow(clippy::cast_sign_loss)] // p - 1 - x >= 0
        let decimals = (p_i - 1 - x) as usize;
        let s = format!("{v:.decimals$}");
        strip_trailing_zeros(&s)
    } else {
        let mantissa = strip_trailing_zeros(&e_str[..exp_pos]);
        let sign = if x < 0 { '-' } else { '+' };
        format!("{mantissa}e{sign}{:02}", x.abs())
    }
}

fn strip_trailing_zeros(s: &str) -> String {
    if s.contains('.') {
        let t = s.trim_end_matches('0');
        t.trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

/// `SkStrAppendScalar`: `%.8g`, with `nan`, `inf` and `-inf` spelled out.
// Port of: src/core/SkString.cpp#L164-L189 (chrome/m156)
#[doc(alias = "SkStrAppendScalar")]
pub fn str_append_scalar(string: &mut String, value: scalar) {
    // Handle infinity and NaN ourselves to ensure consistent cross-platform results.
    // (e.g.: `inf` versus `1.#INF00`, `nan` versus `-nan` for high-bit-set NaNs)
    if is_nan(value) {
        string.push_str("nan");
        return;
    }
    if !is_finite(value) {
        if value > 0.0 {
            string.push_str("inf");
        } else {
            string.push_str("-inf");
        }
        return;
    }

    // since floats have at most 8 significant digits, we limit our %g to that.
    string.push_str(&format_g(f64::from(value), 8));
}

/// `SkAppendScalar`: either `SkBits2Float(0x...)` or `%.9g` with an `f` suffix when the text
/// contains a decimal point.
// Port of: src/core/SkStringUtils.cpp#L20-L35 (chrome/m156)
#[doc(alias = "SkAppendScalar")]
#[allow(clippy::format_push_string)] // simple formatting
pub fn append_scalar(str: &mut String, value: scalar, as_type: ScalarAsStringType) {
    match as_type {
        ScalarAsStringType::Hex => {
            str.push_str(&format!("SkBits2Float(0x{:08x})", float_to_bits(value)));
        }
        ScalarAsStringType::Dec => {
            let mut tmp = format_g(f64::from(value), 9);
            if tmp.contains('.') {
                tmp.push('f');
            }
            str.push_str(&tmp);
        }
    }
}

/// `SkAppendScalarDec`.
// Port of: src/core/SkStringUtils.h#L24-L26 (chrome/m156)
#[doc(alias = "SkAppendScalarDec")]
pub fn append_scalar_dec(str: &mut String, value: scalar) {
    append_scalar(str, value, ScalarAsStringType::Dec);
}
