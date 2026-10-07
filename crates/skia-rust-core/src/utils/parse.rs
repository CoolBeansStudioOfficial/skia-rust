// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/utils/SkParse.h, src/utils/SkParse.cpp

//! Parsing numbers out of text (`SkParse`).
//!
//! C strings become byte slices: the end of the slice plays the role of the terminating `'\0'`,
//! and the returned "pointer" is the index of the first unread byte (`None` for `nullptr`).

use crate::scalar::scalar;

/// The byte at `i`, or 0 past the end (the C string terminator).
pub(crate) fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

// Port of: src/utils/SkParse.cpp#L19-L22 (chrome/m156)
fn is_between(c: i32, min: i32, max: i32) -> bool {
    #[allow(clippy::cast_sign_loss)] // the C++ unsigned-compare trick
    let r = (c - min) as u32 <= (max - min) as u32;
    r
}

// Port of: src/utils/SkParse.cpp#L24-L27 (chrome/m156)
fn is_ws(c: u8) -> bool {
    is_between(i32::from(c), 1, 32)
}

// Port of: src/utils/SkParse.cpp#L29-L32 (chrome/m156)
fn is_digit(c: u8) -> bool {
    is_between(i32::from(c), i32::from(b'0'), i32::from(b'9'))
}

// Port of: src/utils/SkParse.cpp#L34-L37 (chrome/m156)
fn is_sep(c: u8) -> bool {
    is_ws(c) || c == b',' || c == b';'
}

// Port of: src/utils/SkParse.cpp#L39-L50 (chrome/m156)
fn to_hex(c: u8) -> i32 {
    if is_digit(c) {
        return i32::from(c - b'0');
    }

    let c = c | 0x20; // make us lower-case
    if is_between(i32::from(c), i32::from(b'a'), i32::from(b'f')) {
        i32::from(c) + 10 - i32::from(b'a')
    } else {
        -1
    }
}

// Port of: src/utils/SkParse.cpp#L52-L55 (chrome/m156)
fn is_hex(c: u8) -> bool {
    to_hex(c) >= 0
}

// Port of: src/utils/SkParse.cpp#L57-L63 (chrome/m156)
fn skip_ws(s: &[u8], mut i: usize) -> usize {
    while is_ws(at(s, i)) {
        i += 1;
    }
    i
}

// Port of: src/utils/SkParse.cpp#L65-L71 (chrome/m156)
fn skip_sep(s: &[u8], mut i: usize) -> usize {
    while is_sep(at(s, i)) {
        i += 1;
    }
    i
}

/// The number of scalars or int values (separated by whitespace, `,` or `;`).
// Port of: src/utils/SkParse.cpp#L73-L91 (chrome/m156)
#[doc(alias = "Count")]
#[must_use]
pub fn count(s: &[u8]) -> i32 {
    count_with(s, is_sep)
}

/// The number of values separated by `separator`.
// Port of: src/utils/SkParse.cpp#L93-L111 (chrome/m156)
#[must_use]
pub fn count_with_separator(s: &[u8], separator: u8) -> i32 {
    count_with(s, |c| c == separator)
}

fn count_with(s: &[u8], is_separator: impl Fn(u8) -> bool) -> i32 {
    let mut i = 0;
    let mut count = 0;
    // skipLeading
    loop {
        let c = at(s, i);
        i += 1;
        if c == 0 {
            return count;
        }
        if !is_separator(c) {
            break;
        }
    }
    loop {
        count += 1;
        loop {
            let c = at(s, i);
            i += 1;
            if c == 0 {
                return count;
            }
            if is_separator(c) {
                break;
            }
        }
        // skipLeading
        loop {
            let c = at(s, i);
            i += 1;
            if c == 0 {
                return count;
            }
            if !is_separator(c) {
                break;
            }
        }
    }
}

/// Parses up to 8 hex digits followed by the end or whitespace.
// Port of: src/utils/SkParse.cpp#L113-L141 (chrome/m156)
#[doc(alias = "FindHex")]
#[must_use]
pub fn find_hex(s: &[u8], start: usize) -> Option<(usize, u32)> {
    let mut i = skip_ws(s, start);

    if !is_hex(at(s, i)) {
        return None;
    }

    let mut n: u32 = 0;
    let mut max_digits = 8;

    loop {
        let digit = to_hex(at(s, i));
        if digit < 0 {
            break;
        }
        max_digits -= 1;
        if max_digits < 0 {
            return None;
        }
        #[allow(clippy::cast_sign_loss)] // 0..=15
        {
            n = (n << 4) | digit as u32;
        }
        i += 1;
    }

    if at(s, i) == 0 || is_ws(at(s, i)) {
        return Some((i, n));
    }
    None
}

/// Parses an optionally negative decimal `int32_t`.
// Port of: src/utils/SkParse.cpp#L143-L173 (chrome/m156)
#[doc(alias = "FindS32")]
#[must_use]
pub fn find_s32(s: &[u8], start: usize) -> Option<(usize, i32)> {
    let mut i = skip_ws(s, start);

    let mut sign: i64 = 1;
    let mut max_abs_value = i64::from(i32::MAX);
    if at(s, i) == b'-' {
        sign = -1;
        max_abs_value = -i64::from(i32::MIN);
        i += 1;
    }

    if !is_digit(at(s, i)) {
        return None;
    }

    let mut n: i64 = 0;
    while is_digit(at(s, i)) {
        n = 10 * n + i64::from(at(s, i)) - i64::from(b'0');
        if n > max_abs_value {
            return None;
        }

        i += 1;
    }
    #[allow(clippy::cast_possible_truncation)] // in range, checked above
    Some((i, (sign * n) as i32))
}

/// C's `strtod`: the length of the longest prefix of `s` that is a floating-point number, and
/// its value (`None` if there is no such prefix).
///
/// Recognizes an optional sign followed by `inf`/`infinity`/`nan[(...)]` (any case), a hex float
/// (`0x...[p...]`) or a decimal number with an optional exponent.
#[must_use]
#[allow(clippy::too_many_lines)] // mirrors the C++ function
#[allow(clippy::many_single_char_names)] // names follow the C++
pub fn strtod(s: &[u8]) -> Option<(usize, f64)> {
    let mut i = 0;
    let mut negative = false;
    if matches!(at(s, i), b'+' | b'-') {
        negative = at(s, i) == b'-';
        i += 1;
    }
    let sign = |v: f64| if negative { -v } else { v };

    let lower = |j: usize| at(s, j).to_ascii_lowercase();
    let starts_with = |j: usize, word: &[u8]| (0..word.len()).all(|k| lower(j + k) == word[k]);

    if starts_with(i, b"inf") {
        let len = if starts_with(i, b"infinity") { 8 } else { 3 };
        return Some((i + len, sign(f64::INFINITY)));
    }
    if starts_with(i, b"nan") {
        let mut j = i + 3;
        if at(s, j) == b'(' {
            let mut k = j + 1;
            while at(s, k).is_ascii_alphanumeric() || at(s, k) == b'_' {
                k += 1;
            }
            if at(s, k) == b')' {
                j = k + 1;
            }
        }
        return Some((j, sign(f64::NAN)));
    }

    // Hex float.
    if at(s, i) == b'0' && lower(i + 1) == b'x' {
        let mut j = i + 2;
        let mut mantissa: u64 = 0;
        let mut exp: i64 = 0;
        let mut any = false;
        let mut overflowed_bits: i64 = 0;
        while is_hex(at(s, j)) {
            any = true;
            #[allow(clippy::cast_sign_loss)] // 0..=15
            let d = to_hex(at(s, j)) as u64;
            if mantissa >> 60 == 0 {
                mantissa = (mantissa << 4) | d;
            } else {
                overflowed_bits += 4;
            }
            j += 1;
        }
        if at(s, j) == b'.' {
            let mut k = j + 1;
            let mut frac_any = false;
            while is_hex(at(s, k)) {
                frac_any = true;
                #[allow(clippy::cast_sign_loss)] // 0..=15
                let d = to_hex(at(s, k)) as u64;
                if mantissa >> 60 == 0 {
                    mantissa = (mantissa << 4) | d;
                    exp -= 4;
                }
                k += 1;
            }
            if any || frac_any {
                any = true;
                j = k;
            }
        }
        if any {
            exp += overflowed_bits;
            if lower(j) == b'p' {
                let mut k = j + 1;
                let mut eneg = false;
                if matches!(at(s, k), b'+' | b'-') {
                    eneg = at(s, k) == b'-';
                    k += 1;
                }
                if is_digit(at(s, k)) {
                    let mut e: i64 = 0;
                    while is_digit(at(s, k)) {
                        e = (e * 10 + i64::from(at(s, k) - b'0')).min(100_000);
                        k += 1;
                    }
                    exp += if eneg { -e } else { e };
                    j = k;
                }
            }
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            let v = (mantissa as f64) * 2f64.powi(exp.clamp(-2000, 2000) as i32);
            return Some((j, sign(v)));
        }
        // "0x" without digits: just the "0".
        return Some((i + 1, sign(0.0)));
    }

    // Decimal.
    let start = i;
    let mut j = i;
    let mut digits = 0;
    while is_digit(at(s, j)) {
        j += 1;
        digits += 1;
    }
    if at(s, j) == b'.' {
        j += 1;
        while is_digit(at(s, j)) {
            j += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if lower(j) == b'e' {
        let mut k = j + 1;
        if matches!(at(s, k), b'+' | b'-') {
            k += 1;
        }
        if is_digit(at(s, k)) {
            while is_digit(at(s, k)) {
                k += 1;
            }
            j = k;
        }
    }
    let text = std::str::from_utf8(&s[start..j]).ok()?;
    let v: f64 = text.parse().ok()?;
    Some((j, sign(v)))
}

/// Parses a scalar (as C's `strtod`, rounded to float), after skipping whitespace.
// Port of: src/utils/SkParse.cpp#L175-L188 (chrome/m156)
#[doc(alias = "FindScalar")]
#[must_use]
#[allow(clippy::cast_possible_truncation)] // (float)strtod(...)
pub fn find_scalar(s: &[u8], start: usize) -> Option<(usize, scalar)> {
    let i = skip_ws(s, start);

    let (len, v) = strtod(&s[i.min(s.len())..])?;
    Some((i + len, v as f32))
}

/// Parses `value.len()` scalars separated by whitespace, `,` or `;`. Values are written as they
/// are parsed, even if a later one fails.
// Port of: src/utils/SkParse.cpp#L190-L210 (chrome/m156)
#[doc(alias = "FindScalars")]
#[must_use]
pub fn find_scalars(s: &[u8], start: usize, value: &mut [scalar]) -> Option<usize> {
    let mut i = start;
    let mut count = value.len();
    let mut idx = 0;
    if count > 0 {
        loop {
            let (next, v) = find_scalar(s, i)?;
            value[idx] = v;
            i = next;
            count -= 1;
            if count == 0 {
                break;
            }

            // keep going
            i = skip_sep(s, i);
            idx += 1;
        }
    }
    Some(i)
}

/// `yes`/`1`/`true` or `no`/`0`/`false`.
// Port of: src/utils/SkParse.cpp#L220-L237 (chrome/m156)
#[doc(alias = "FindBool")]
#[must_use]
pub fn find_bool(s: &str) -> Option<bool> {
    const YES: [&str; 3] = ["yes", "1", "true"];
    const NO: [&str; 3] = ["no", "0", "false"];

    if YES.contains(&s) {
        Some(true)
    } else if NO.contains(&s) {
        Some(false)
    } else {
        None
    }
}

/// The index of `target` in the comma-separated `list`, or -1.
// Port of: src/utils/SkParse.cpp#L239-L263 (chrome/m156)
#[doc(alias = "FindList")]
#[must_use]
pub fn find_list(target: &str, list: &str) -> i32 {
    for (index, entry) in (0..).zip(list.split(',')) {
        if entry == target {
            return index;
        }
    }
    -1
}
