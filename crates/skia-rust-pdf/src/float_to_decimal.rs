// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/utils/SkFloatToDecimal.cpp, include/... src/utils/SkFloatToDecimal.h (chrome/m156)

//! Float-to-decimal conversion used by every PDF number (`SkFloatToDecimal`).

/// `kMaximumSkFloatToDecimalLength`: the longest result (including the terminating NUL).
// Port of: src/utils/SkFloatToDecimal.h#L10 (chrome/m156)
#[doc(alias = "kMaximumSkFloatToDecimalLength")]
pub const MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH: usize = 49;

/// `log10(2.0)`, as the C++ literal is spelled (`clippy::approx_constant` would prefer the
/// `std` constant, which is not the same literal).
// Port of: src/utils/SkFloatToDecimal.cpp#L104 (chrome/m156)
#[allow(clippy::approx_constant)] // the C++ literal, kept byte for byte
const K_LOG2: f64 = 0.301_029_995_663_981_2;

/// `returns value * pow(base, e)`, assuming `e` is positive.
// Port of: src/utils/SkFloatToDecimal.cpp#L14-L28 (chrome/m156)
fn pow_by_squaring(mut value: f64, mut base: f64, mut e: i32) -> f64 {
    // https://en.wikipedia.org/wiki/Exponentiation_by_squaring
    debug_assert!(e > 0);
    loop {
        if e & 1 != 0 {
            value *= base;
        }
        e >>= 1;
        if e == 0 {
            return value;
        }
        base *= base;
    }
}

/// Return pow(10.0, e), optimized for common cases.
// Port of: src/utils/SkFloatToDecimal.cpp#L30-L58 (chrome/m156)
#[allow(clippy::unreadable_literal)] // the literals are spelled as in the C++ source
fn pow10(e: i32) -> f64 {
    match e {
        0 => 1.0, // common cases
        1 => 10.0,
        2 => 100.0,
        3 => 1e+03,
        4 => 1e+04,
        5 => 1e+05,
        6 => 1e+06,
        7 => 1e+07,
        8 => 1e+08,
        9 => 1e+09,
        10 => 1e+10,
        11 => 1e+11,
        12 => 1e+12,
        13 => 1e+13,
        14 => 1e+14,
        15 => 1e+15,
        _ => {
            if e > 15 {
                pow_by_squaring(1e+15, 10.0, e - 15)
            } else {
                debug_assert!(e < 0);
                pow_by_squaring(1.0, 0.1, -e)
            }
        }
    }
}

/// `frexp(value, &e)` for a finite, positive, non-zero `f32`, returning `e`.
///
/// Every `f32` is a normal `f64`, so the exponent of its `f64` representation is the exponent
/// `frexp` returns: `value = m * 2^e` with `0.5 <= m < 1`.
fn frexp_exponent(value: f32) -> i32 {
    let bits = f64::from(value).to_bits();
    let biased = i32::try_from((bits >> 52) & 0x7ff).unwrap_or(0);
    biased - 1022
}

/// Writes a `value` as a decimal string into `output`, including a terminating NUL (for unit
/// testing). Returns `strlen(output)`. The result is in the form `[-]?([0-9]*\.)?[0-9]+` and
/// `sscanf(output, "%f", &x)` returns the original value iff the value is finite. Every input is
/// accepted: infinities become `±FLT_MAX`, and NaN and zero become `"0"`.
///
/// Motivation: "PDF does not support [numbers] in exponential format (such as 6.02e23)."
// Port of: src/utils/SkFloatToDecimal.cpp#L60-L186 (chrome/m156)
#[doc(alias = "SkFloatToDecimal")]
#[allow(
    clippy::cast_possible_truncation, // mirrors the C++ (int) casts, whose operands are in range
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::too_many_lines,         // one-to-one with the C++ function
    clippy::similar_names
)]
pub fn float_to_decimal(
    mut value: f32,
    output: &mut [u8; MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH],
) -> usize {
    // The longest result is -FLT_MIN, "-.0000000000000000000000000000000000000117549435",
    // which has 48 characters plus a terminating NUL.
    //
    // Section C.1 of the PDF1.4 spec says that most PDF rasterizers use fixed-point scalars
    // that lack the dynamic range of floats. Even so, very small and very large values are
    // serialized with enough precision for a floating-point rasterizer to read them back exactly.
    let mut out: Vec<u8> = Vec::with_capacity(MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH);

    // This function accepts non-finite values too. For those we ignore value-correctness and
    // output a syntactically valid number.
    if value == f32::INFINITY {
        value = f32::MAX; // nearest finite float.
    }
    if value == f32::NEG_INFINITY {
        value = -f32::MAX; // nearest finite float.
    }
    if !value.is_finite() || value == 0.0 {
        // NAN is unsupported in PDF. Always output a valid number. Zero is a special case.
        out.push(b'0');
        return finish(output, &out);
    }
    if value < 0.0 {
        out.push(b'-');
        value = -value;
    }
    debug_assert!(value >= 0.0);

    let binary_exponent = frexp_exponent(value);
    let decimal_exponent = (K_LOG2 * f64::from(binary_exponent)).floor() as i32;
    let mut decimal_shift = decimal_exponent - 8;
    let power = pow10(-decimal_shift);
    debug_assert!(f64::from(value) * power <= f64::from(i32::MAX));
    let mut d = (f64::from(value) * power + 0.5) as i32;
    debug_assert!(d <= 999_999_999);
    if d > 167_772_159 {
        // floor(pow(10, 1 + log10(1 << 24))): one fewer decimal digit for 24-bit precision.
        decimal_shift = decimal_exponent - 7;
        // Recalculate to get the rounding right.
        d = (f64::from(value) * (power * 0.1) + 0.5) as i32;
        debug_assert!(d <= 99_999_999);
    }
    while d % 10 == 0 {
        d /= 10;
        decimal_shift += 1;
    }
    debug_assert!(d > 0);

    let mut buffer = [0u8; 9]; // decimal value buffer.
    let mut buffer_index: usize = 0;
    loop {
        buffer[buffer_index] = (d % 10) as u8;
        buffer_index += 1;
        d /= 10;
        if d == 0 {
            break;
        }
    }
    debug_assert!(buffer_index <= buffer.len() && buffer_index > 0);

    if decimal_shift >= 0 {
        loop {
            buffer_index -= 1;
            out.push(b'0' + buffer[buffer_index]);
            if buffer_index == 0 {
                break;
            }
        }
        out.resize(out.len() + usize::try_from(decimal_shift).unwrap_or(0), b'0');
    } else {
        let places_before_decimal =
            i32::try_from(buffer_index).unwrap_or(i32::MAX) + decimal_shift;
        if places_before_decimal > 0 {
            for _ in 0..places_before_decimal {
                buffer_index -= 1;
                out.push(b'0' + buffer[buffer_index]);
            }
            out.push(b'.');
        } else {
            out.push(b'.');
            let places_after_decimal = usize::try_from(-places_before_decimal).unwrap_or(0);
            out.resize(out.len() + places_after_decimal, b'0');
        }
        while buffer_index > 0 {
            buffer_index -= 1;
            out.push(b'0' + buffer[buffer_index]);
            if out.len() == MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH - 1 {
                break; // denormalized: we don't need extra precision.
                // Denormalized numbers do not have the same number of significant digits, but
                // they do not need them to round-trip.
            }
        }
    }
    debug_assert!(out.len() < MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH);
    finish(output, &out)
}

/// Copies `text` into `output` and appends the terminating NUL. Returns the length of `text`.
fn finish(output: &mut [u8; MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH], text: &[u8]) -> usize {
    output[..text.len()].copy_from_slice(text);
    output[text.len()] = 0;
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_values() {
        let mut buf = [0u8; MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH];
        let len = float_to_decimal(0.5, &mut buf);
        assert_eq!(&buf[..len], b".5");
        let len = float_to_decimal(-2.0, &mut buf);
        assert_eq!(&buf[..len], b"-2");
        let len = float_to_decimal(0.0, &mut buf);
        assert_eq!(&buf[..len], b"0");
        let len = float_to_decimal(f32::NAN, &mut buf);
        assert_eq!(&buf[..len], b"0");
        assert_eq!(buf[len], 0);
    }
}
