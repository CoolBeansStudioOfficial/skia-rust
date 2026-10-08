// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLString.{h,cpp} (`SkSL::stod`, `SkSL::stoi`, `SkSL::String`).

//! Number parsing and `printf`-style formatting for the `SkSL` compiler.
//!
//! `printf` takes typed [`Arg`]s instead of C varargs. Only the conversions and flags that the
//! ported `SkSL` code uses are implemented, and any other format panics, because format strings are
//! literals in the port.

use crate::skstd::format_g;

/// Whitespace as `isspace` sees it in the C locale.
fn is_c_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r')
}

/// `SkSL::stod`: reads a floating-point number the way `std::istream >> double` does, and returns
/// it only if it is finite. Leading whitespace is skipped and trailing text is ignored, as the
/// stream leaves it unread.
// Port of: src/sksl/SkSLString.cpp#L58-L63 (chrome/m156)
#[doc(alias = "SkSL::stod")]
#[must_use]
pub fn stod(s: &str) -> Option<f64> {
    let s = s.trim_start_matches(is_c_space);
    let bytes = s.as_bytes();
    // `num_get` accumulates the characters a floating-point literal can contain, then converts
    // them with `strtod`, which must consume all of them.
    let mut end = 0;
    let mut seen_dot = false;
    let mut seen_exp = false;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        end = 1;
    }
    while let Some(&b) = bytes.get(end) {
        match b {
            b'0'..=b'9' => {}
            b'.' if !seen_dot && !seen_exp => seen_dot = true,
            b'e' | b'E' if !seen_exp => {
                seen_exp = true;
                if matches!(bytes.get(end + 1), Some(b'+' | b'-')) {
                    end += 1;
                }
            }
            _ => break,
        }
        end += 1;
    }
    let value: f64 = s[..end].parse().ok()?;
    value.is_finite().then_some(value)
}

/// `SkSL::stoi`: parses an integer the way `strtoull(s, base 0)` does (`0x` hex, leading `0`
/// octal, otherwise decimal), with an optional `u`/`U` suffix, and accepts it only if it fits in
/// 32 bits. A negative value is negated in 64-bit unsigned arithmetic, as `strtoull` does.
// Port of: src/sksl/SkSLString.cpp#L65-L84 (chrome/m156)
#[doc(alias = "SkSL::stoi")]
#[must_use]
pub fn stoi(s: &str) -> Option<i32> {
    if s.is_empty() {
        return None;
    }
    let s = s.strip_suffix(['u', 'U']).unwrap_or(s);
    let trimmed = s.trim_start_matches(is_c_space);
    let (negative, unsigned) = match trimmed.as_bytes().first() {
        Some(b'-') => (true, &trimmed[1..]),
        Some(b'+') => (false, &trimmed[1..]),
        _ => (false, trimmed),
    };
    let hex_digit_follows = matches!(unsigned.as_bytes().get(2), Some(b) if b.is_ascii_hexdigit());
    let (radix, digits) = if unsigned.starts_with("0x") || unsigned.starts_with("0X") {
        if hex_digit_follows {
            (16, &unsigned[2..])
        } else {
            // `0x` without hex digits: `strtoull` reads the `0` and stops at the `x`.
            (8, unsigned)
        }
    } else if unsigned.starts_with('0') {
        (8, unsigned)
    } else {
        (10, unsigned)
    };
    // `digits` is a suffix of `s`, so its offset in `s` is where the number starts.
    let start = s.len() - digits.len();
    let mut value: u64 = 0;
    let mut consumed = 0;
    for (i, c) in digits.char_indices() {
        let Some(d) = c.to_digit(radix) else { break };
        value = value
            .checked_mul(u64::from(radix))?
            .checked_add(u64::from(d))?;
        consumed = i + c.len_utf8();
    }
    // With no digits `strtoull` reports no conversion and endptr at the start of the string, so
    // the whole string must have been consumed: only an empty string succeeds.
    if consumed == 0 {
        return s.is_empty().then_some(0);
    }
    if start + consumed != s.len() {
        return None;
    }
    if negative {
        value = value.wrapping_neg();
    }
    u32::try_from(value).ok().map(u32::cast_signed)
}

/// One argument of [`printf`], with the C type the format expects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arg<'a> {
    /// `%s`, and `%.*s` with the precision passed as an [`Arg::Int`] first.
    Str(&'a str),
    /// `%d`, `%i`, `%c`, and `*` widths or precisions. `%u`, `%x`, `%X` and `%o` accept it as the
    /// same bits, as C does for an `int` passed to an unsigned conversion.
    Int(i32),
    /// `%u`, `%x`, `%X`, `%o`.
    UInt(u32),
    /// `%f`, `%F`, `%e`, `%E`, `%g`, `%G`.
    Float(f64),
}

/// The flags, width and precision of one conversion.
// The printf flags are independent booleans, as in C.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Default)]
struct Spec {
    left: bool,
    plus: bool,
    space: bool,
    zero: bool,
    alt: bool,
    width: usize,
    precision: Option<usize>,
}

/// `SkSL::String::printf`: formats `fmt` with `args`.
#[doc(alias = "SkSL::String::printf")]
#[must_use]
pub fn printf(fmt: &str, args: &[Arg<'_>]) -> String {
    let mut out = String::new();
    appendf(&mut out, fmt, args);
    out
}

/// `SkSL::String::appendf`: formats `fmt` with `args` and appends the result to `out`.
///
/// # Panics
///
/// If `fmt` uses a conversion or flag this port does not implement, or if `args` do not match its
/// conversions.
#[doc(alias = "SkSL::String::appendf")]
pub fn appendf(out: &mut String, fmt: &str, args: &[Arg<'_>]) {
    let mut args = args.iter();
    let mut rest = fmt;
    while let Some(pos) = rest.find('%') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos + 1..];
        if let Some(after) = rest.strip_prefix('%') {
            out.push('%');
            rest = after;
            continue;
        }
        let (spec, conv, after) = parse_spec(rest, &mut args);
        rest = after;
        format_one(out, &spec, conv, &mut args);
    }
    out.push_str(rest);
    assert!(
        args.next().is_none(),
        "printf: more arguments than conversions in {fmt:?}"
    );
}

/// Parses `[flags][width][.precision]conversion` after a `%`, taking `*` values from `args`.
/// Returns the spec, the conversion character and the rest of the format.
fn parse_spec<'f>(fmt: &'f str, args: &mut std::slice::Iter<'_, Arg<'_>>) -> (Spec, char, &'f str) {
    let mut spec = Spec::default();
    let mut pos = 0;
    let bytes = fmt.as_bytes();
    while let Some(&b) = bytes.get(pos) {
        match b {
            b'-' => spec.left = true,
            b'+' => spec.plus = true,
            b' ' => spec.space = true,
            b'0' => spec.zero = true,
            b'#' => spec.alt = true,
            _ => break,
        }
        pos += 1;
    }
    if bytes.get(pos) == Some(&b'*') {
        pos += 1;
        let width = int_arg(args.next(), "width");
        if width < 0 {
            spec.left = true;
        }
        spec.width = width.unsigned_abs() as usize;
    } else {
        let (width, len) = read_digits(&fmt[pos..]);
        spec.width = width;
        pos += len;
    }
    if bytes.get(pos) == Some(&b'.') {
        pos += 1;
        if bytes.get(pos) == Some(&b'*') {
            pos += 1;
            // A negative `*` precision is as if the precision were omitted.
            spec.precision = usize::try_from(int_arg(args.next(), "precision")).ok();
        } else {
            let (precision, len) = read_digits(&fmt[pos..]);
            spec.precision = Some(precision);
            pos += len;
        }
    }
    let Some(conv) = fmt[pos..].chars().next() else {
        panic!("printf: the format ends inside a conversion: {fmt:?}");
    };
    assert!(
        !matches!(conv, 'h' | 'l' | 'L' | 'q' | 'j' | 'z' | 't'),
        "printf: length modifiers are not implemented: {fmt:?}"
    );
    (spec, conv, &fmt[pos + conv.len_utf8()..])
}

/// Reads a run of decimal digits: its value and its length in bytes.
fn read_digits(s: &str) -> (usize, usize) {
    let len = s.bytes().take_while(u8::is_ascii_digit).count();
    let value = s[..len].parse().unwrap_or(0);
    (value, len)
}

fn int_arg(arg: Option<&Arg<'_>>, what: &str) -> i32 {
    match arg {
        Some(Arg::Int(v)) => *v,
        other => panic!("printf: the {what} argument must be an int, got {other:?}"),
    }
}

fn sign_text(negative: bool, spec: &Spec) -> &'static str {
    if negative {
        "-"
    } else if spec.plus {
        "+"
    } else if spec.space {
        " "
    } else {
        ""
    }
}

fn format_one(out: &mut String, spec: &Spec, conv: char, args: &mut std::slice::Iter<'_, Arg<'_>>) {
    let arg = args
        .next()
        .unwrap_or_else(|| panic!("printf: too few arguments for %{conv}"));
    match (conv, arg) {
        ('d' | 'i', Arg::Int(v)) => {
            let digits = v.unsigned_abs().to_string();
            let digits = apply_precision_digits(digits, spec.precision, *v == 0);
            pad_number(out, spec, sign_text(*v < 0, spec), "", &digits);
        }
        ('u' | 'x' | 'X' | 'o', Arg::UInt(_) | Arg::Int(_)) => {
            let bits = match arg {
                Arg::UInt(u) => *u,
                Arg::Int(i) => i.cast_unsigned(),
                _ => unreachable!("matched above"),
            };
            let digits = match conv {
                'u' => bits.to_string(),
                'x' => format!("{bits:x}"),
                'X' => format!("{bits:X}"),
                _ => format!("{bits:o}"),
            };
            let mut digits = apply_precision_digits(digits, spec.precision, bits == 0);
            let prefix = match conv {
                'x' if spec.alt && bits != 0 => "0x",
                'X' if spec.alt && bits != 0 => "0X",
                _ => "",
            };
            if conv == 'o' && spec.alt && !digits.starts_with('0') {
                digits.insert(0, '0');
            }
            pad_number(out, spec, "", prefix, &digits);
        }
        ('c', Arg::Int(v)) => {
            // `%c` converts the int to an unsigned char.
            let text = char::from(v.to_le_bytes()[0]).to_string();
            pad_text(out, spec, &text);
        }
        ('s', Arg::Str(s)) => {
            let text: String = match spec.precision {
                Some(p) => s.chars().take(p).collect(),
                None => (*s).to_owned(),
            };
            pad_text(out, spec, &text);
        }
        ('f' | 'F' | 'e' | 'E' | 'g' | 'G', Arg::Float(v)) => {
            let upper = conv.is_ascii_uppercase();
            if !v.is_finite() {
                // Non-finite values are never zero padded.
                let text = if v.is_nan() { "nan" } else { "inf" };
                let text = if upper {
                    text.to_ascii_uppercase()
                } else {
                    text.to_owned()
                };
                let plain = Spec {
                    zero: false,
                    ..*spec
                };
                pad_number(
                    out,
                    &plain,
                    sign_text(v.is_sign_negative(), spec),
                    "",
                    &text,
                );
                return;
            }
            let magnitude = v.abs();
            let precision = spec.precision.unwrap_or(6);
            let body = match conv.to_ascii_lowercase() {
                'f' => format!("{magnitude:.precision$}"),
                'e' => format_e(magnitude, precision),
                _ => format_g(magnitude, precision.max(1)),
            };
            let body = if upper {
                body.to_ascii_uppercase()
            } else {
                body
            };
            pad_number(out, spec, sign_text(v.is_sign_negative(), spec), "", &body);
        }
        (conv, arg) => panic!("printf: %{conv} does not take {arg:?}"),
    }
}

/// C's `%.*e` for a non-negative value: `precision` digits after the point and an exponent of at
/// least two digits.
fn format_e(magnitude: f64, precision: usize) -> String {
    let sci = format!("{magnitude:.precision$e}");
    let (mantissa, exp_text) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exponent: i32 = exp_text.parse().unwrap_or(0);
    let sign = if exponent < 0 { '-' } else { '+' };
    format!("{mantissa}e{sign}{:02}", exponent.unsigned_abs())
}

/// The digits of an integer with its minimum digit count applied. Zero with precision 0 has no
/// digits.
fn apply_precision_digits(digits: String, precision: Option<usize>, is_zero: bool) -> String {
    match precision {
        Some(0) if is_zero => String::new(),
        Some(p) if digits.len() < p => format!("{}{digits}", "0".repeat(p - digits.len())),
        _ => digits,
    }
}

/// Pads a number. The `0` flag puts zeros between the sign and the digits, unless `-` or a
/// precision is given; otherwise spaces go before the number, or after it with `-`.
fn pad_number(out: &mut String, spec: &Spec, sign: &str, prefix: &str, digits: &str) {
    let len = sign.len() + prefix.len() + digits.len();
    let fill = spec.width.saturating_sub(len);
    let zero_pad = spec.zero && !spec.left && spec.precision.is_none();
    if spec.left {
        out.push_str(sign);
        out.push_str(prefix);
        out.push_str(digits);
        out.push_str(&" ".repeat(fill));
    } else if zero_pad {
        out.push_str(sign);
        out.push_str(prefix);
        out.push_str(&"0".repeat(fill));
        out.push_str(digits);
    } else {
        out.push_str(&" ".repeat(fill));
        out.push_str(sign);
        out.push_str(prefix);
        out.push_str(digits);
    }
}

/// Pads plain text (`%s`, `%c`) with spaces.
fn pad_text(out: &mut String, spec: &Spec, text: &str) {
    let fill = spec.width.saturating_sub(text.chars().count());
    if spec.left {
        out.push_str(text);
        out.push_str(&" ".repeat(fill));
    } else {
        out.push_str(&" ".repeat(fill));
        out.push_str(text);
    }
}

/// `SkSL::String::Separator`: yields "" the first time and ", " afterwards, for comma lists.
#[derive(Debug)]
pub struct Separator {
    first: bool,
}

impl Default for Separator {
    fn default() -> Self {
        Self { first: true }
    }
}

impl Separator {
    /// A separator that yields "" on its first call.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The separator to write before the next item.
    pub fn next_str(&mut self) -> &'static str {
        if std::mem::replace(&mut self.first, false) {
            ""
        } else {
            ", "
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Arg, Separator, printf, stod, stoi};

    #[test]
    fn stod_reads_like_istream() {
        assert_eq!(stod("1.5"), Some(1.5));
        assert_eq!(stod("  -2e3x"), Some(-2000.0));
        assert_eq!(stod("1.5f"), Some(1.5));
        assert_eq!(stod(".5"), Some(0.5));
        assert_eq!(stod("1."), Some(1.0));
        assert_eq!(stod("1e400"), None);
        assert_eq!(stod("e5"), None);
        assert_eq!(stod(""), None);
    }

    #[test]
    fn stoi_reads_like_strtoull_base_0() {
        assert_eq!(stoi("42"), Some(42));
        assert_eq!(stoi("010"), Some(8));
        assert_eq!(stoi("0x1F"), Some(31));
        assert_eq!(stoi("7u"), Some(7));
        assert_eq!(stoi("4294967295"), Some(-1));
        assert_eq!(stoi("4294967296"), None);
        assert_eq!(stoi("-1"), None);
        assert_eq!(stoi("-0"), Some(0));
        assert_eq!(stoi("08"), None);
        assert_eq!(stoi("0x"), None);
        assert_eq!(stoi("12z"), None);
        assert_eq!(stoi(""), None);
    }

    #[test]
    fn printf_formats_like_c() {
        assert_eq!(
            printf(
                "vec%d<%.*s>",
                &[Arg::Int(3), Arg::Int(2), Arg::Str("float")]
            ),
            "vec3<fl>"
        );
        assert_eq!(printf("0x%08X", &[Arg::UInt(0x3F80_0000)]), "0x3F800000");
        assert_eq!(
            printf(
                "%-30.*s %s\n",
                &[Arg::Int(3), Arg::Str("abcdef"), Arg::Str("x")]
            ),
            format!("{:<30} x\n", "abc")
        );
        assert_eq!(
            printf(
                "%+d (label %d at #%d)",
                &[Arg::Int(5), Arg::Int(1), Arg::Int(2)]
            ),
            "+5 (label 1 at #2)"
        );
        assert_eq!(printf("%.0f", &[Arg::Float(2.5)]), "2");
        assert_eq!(printf("%.0f", &[Arg::Float(3.5)]), "4");
        assert_eq!(printf("%g", &[Arg::Float(0.0001)]), "0.0001");
        assert_eq!(printf("%g", &[Arg::Float(1e-5)]), "1e-05");
        assert_eq!(printf("%g", &[Arg::Float(100.0)]), "100");
        assert_eq!(printf("%e", &[Arg::Float(12345.678)]), "1.234568e+04");
        assert_eq!(
            printf(
                "%c|%5.2f|%-4d|%%",
                &[Arg::Int(i32::from(b'z')), Arg::Float(3.25), Arg::Int(7)]
            ),
            "z| 3.25|7   |%"
        );
        assert_eq!(
            printf("%#x %#o %x", &[Arg::UInt(255), Arg::UInt(8), Arg::Int(-1)]),
            "0xff 010 ffffffff"
        );
        assert_eq!(
            printf("%03d %+.2d", &[Arg::Int(-7), Arg::Int(3)]),
            "-07 +03"
        );
    }

    #[test]
    fn separator_yields_comma_list() {
        let mut sep = Separator::new();
        let mut out = String::new();
        for item in ["a", "b", "c"] {
            out.push_str(sep.next_str());
            out.push_str(item);
        }
        assert_eq!(out, "a, b, c");
    }
}
