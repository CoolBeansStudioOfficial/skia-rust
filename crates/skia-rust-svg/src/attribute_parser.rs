// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGAttributeParser.h,
// modules/svg/src/SkSVGAttributeParser.cpp

//! Parsers for the string forms of SVG attribute values (`SkSVGAttributeParser`).
//!
//! A C string becomes a byte slice: the end of the slice plays the role of the terminating
//! `'\0'`, as in [`skia_rust_core::utils::parse`].

use skia_rust_core::color::Color;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{degrees_to_radians, scalar, scalar_round_to_int};
use skia_rust_core::t_pin::t_pin;
use skia_rust_core::utf::next_utf8;
use skia_rust_core::utils::parse;
use skia_rust_core::utils::parse_color::find_named_color;

use crate::types::{
    Align, ColorKind, ColorType, Colorspace, DashArray, DashArrayType, Display, Fill, FillRule, FillRuleType, FontFamily, FontSize, FontStyle, FontStyleType, FontWeight,
    FontWeightType, FuncIri, IntegerType, Iri, IriType, Length, LengthUnit, LineCap, LineJoin,
    LineJoinType, NumberType, ObjectBoundingBoxUnits, ObjectBoundingBoxUnitsType, Paint,
    PreserveAspectRatio, Property, PropertyState, Scale, StringType, TextAnchor, TextAnchorType,
    TransformType, ViewBoxType, Visibility, VisibilityType,
};

// TODO: these should be shared with SkParse.cpp

// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L24-L27 (chrome/m156)
fn is_between(c: u8, min: u8, max: u8) -> bool {
    debug_assert!(min <= max);
    c.wrapping_sub(min) <= max - min
}

// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L29-L31 (chrome/m156)
fn is_ws(c: u8) -> bool {
    // `is_between(c, 1, 32)` on a (signed) char: bytes >= 0x80 are negative, so wrap to large
    // unsigned values and are not whitespace.
    is_between(c, 1, 32)
}

// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L33-L35 (chrome/m156)
fn is_sep(c: u8) -> bool {
    is_ws(c) || c == b',' || c == b';'
}

// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L37-L39 (chrome/m156)
fn is_nl(c: u8) -> bool {
    c == b'\n' || c == b'\r' || c == 0x0c
}

// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L41-L45 (chrome/m156)
fn is_hex(c: u8) -> bool {
    is_between(c, b'a', b'f') || is_between(c, b'A', b'F') || is_between(c, b'0', b'9')
}

/// A value that [`AttributeParser`] can parse (the `parse(T*)` specializations).
pub trait Parse: Sized {
    fn parse(parser: &mut AttributeParser<'_>) -> Option<Self>;
}

/// The result of parsing an attribute (`SkSVGAttributeParser::ParseResult`).
pub type ParseResult<T> = Option<T>;

/// `SkSVGAttributeParser`.
// Port of: modules/svg/include/SkSVGAttributeParser.h#L25-L145 (chrome/m156)
#[doc(alias = "SkSVGAttributeParser")]
#[derive(Debug)]
pub struct AttributeParser<'a> {
    // The input string and the current position in it.
    s: &'a [u8],
    cur_pos: usize,
}

impl<'a> AttributeParser<'a> {
    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L47-L50 (chrome/m156)
    #[must_use]
    pub fn new(attribute_string: &'a str) -> Self {
        Self {
            s: attribute_string.as_bytes(),
            cur_pos: 0,
        }
    }

    /// The unread rest of the input (`fCurPos` as a string).
    pub(crate) fn remaining(&self) -> &'a str {
        // The input came from a `&str` and the position is always on a character boundary.
        std::str::from_utf8(&self.s[self.cur_pos..]).unwrap_or("")
    }

    fn end_pos(&self) -> usize {
        self.s.len()
    }

    /// `*fCurPos`: the byte at the current position, `0` at the end.
    fn cur(&self) -> u8 {
        self.s.get(self.cur_pos).copied().unwrap_or(0)
    }

    /// Parses a value of type `T` out of the whole of `value`.
    // Port of: modules/svg/include/SkSVGAttributeParser.h#L48-L54 (chrome/m156)
    #[must_use]
    pub fn parse_value<T: Parse>(value: &str) -> ParseResult<T> {
        let mut parser = AttributeParser::new(value);
        T::parse(&mut parser)
    }

    /// Parses `value` as a `T` if `name` is `expected_name`.
    // Port of: modules/svg/include/SkSVGAttributeParser.h#L56-L65 (chrome/m156)
    #[must_use]
    pub fn parse_named<T: Parse>(expected_name: &str, name: &str, value: &str) -> ParseResult<T> {
        if name == expected_name {
            return Self::parse_value::<T>(value);
        }
        None
    }

    /// Parses a property: `inherit`, or a `T` value.
    // Port of: modules/svg/include/SkSVGAttributeParser.h#L67-L84 (chrome/m156)
    #[must_use]
    pub fn parse_property<T: Parse, const INHERITABLE: bool>(
        expected_name: &str,
        name: &str,
        value: &str,
    ) -> ParseResult<Property<T, INHERITABLE>> {
        if name != expected_name {
            return None;
        }

        if value == "inherit" {
            return Some(Property::from_state(PropertyState::Inherit));
        }

        Self::parse_value::<T>(value).map(Property::from_value)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L52-L59 (chrome/m156)
    fn advance_while(&mut self, f: impl Fn(u8) -> bool) -> bool {
        let initial = self.cur_pos;
        while self.cur_pos < self.end_pos() && f(self.s[self.cur_pos]) {
            self.cur_pos += 1;
        }
        self.cur_pos != initial
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L61-L77 (chrome/m156)
    fn match_string_token(&self, token: &str) -> Option<usize> {
        let token = token.as_bytes();
        let mut c = self.cur_pos;
        let mut t = 0;

        while c < self.end_pos() && t < token.len() && self.s[c] == token[t] {
            c += 1;
            t += 1;
        }

        if t < token.len() {
            return None;
        }

        Some(c)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L79-L81 (chrome/m156)
    fn parse_eos_token(&self) -> bool {
        self.cur_pos == self.end_pos()
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L83-L85 (chrome/m156)
    fn parse_sep_token(&mut self) -> bool {
        self.advance_while(is_sep)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L87-L89 (chrome/m156)
    fn parse_ws_token(&mut self) -> bool {
        self.advance_while(is_ws)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L91-L95 (chrome/m156)
    fn parse_comma_wsp_token(&mut self) -> bool {
        // comma-wsp:
        //     (wsp+ comma? wsp*) | (comma wsp*)
        self.parse_ws_token() || self.parse_expected_string_token(",")
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L97-L104 (chrome/m156)
    fn parse_expected_string_token(&mut self, expected: &str) -> bool {
        let Some(new_pos) = self.match_string_token(expected) else {
            return false;
        };

        self.cur_pos = new_pos;
        true
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L106-L112 (chrome/m156)
    fn parse_scalar_token(&mut self) -> Option<scalar> {
        if let Some((next, res)) = parse::find_scalar(self.s, self.cur_pos) {
            self.cur_pos = next;
            return Some(res);
        }
        None
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L114-L120 (chrome/m156)
    fn parse_int32_token(&mut self) -> Option<i32> {
        if let Some((next, res)) = parse::find_s32(self.s, self.cur_pos) {
            self.cur_pos = next;
            return Some(res);
        }
        None
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L122-L126 (chrome/m156)
    fn match_hex_token(&self) -> Option<usize> {
        let mut new_pos = self.cur_pos;
        while new_pos < self.end_pos() && is_hex(self.s[new_pos]) {
            new_pos += 1;
        }
        (new_pos != self.cur_pos).then_some(new_pos)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L128-L164 (chrome/m156)
    fn parse_escape(&mut self) -> Option<i32> {
        // \(hexDigit{1,6}whitespace?|[^newline|hexDigit])
        let saved = self.cur_pos;
        let result = self.parse_escape_inner();
        if result.is_none() {
            self.cur_pos = saved;
        }
        result
    }

    fn parse_escape_inner(&mut self) -> Option<i32> {
        if !self.parse_expected_string_token("\\") {
            return None;
        }
        let c;
        if let Some(mut hex_end) = self.match_hex_token() {
            if hex_end - self.cur_pos > 6 {
                hex_end = self.cur_pos + 6;
            }
            let hex_string = &self.s[self.cur_pos..hex_end];
            let found = parse::find_hex(hex_string, 0);
            let mut cp = found.map_or(0, |(_, cp)| cp);
            if found.is_none() || cp < 1 || (0xD800..=0xDFFF).contains(&cp) || 0x10_FFFF < cp {
                cp = 0xFFFD;
            }
            #[allow(clippy::cast_possible_wrap)] // cp <= 0x10_FFFF
            {
                c = cp as i32;
            }
            self.cur_pos = hex_end;
            self.parse_ws_token();
        } else if self.parse_eos_token() || is_nl(self.cur()) {
            return None;
        } else {
            let mut rest = &self.s[self.cur_pos..];
            let u = next_utf8(&mut rest);
            self.cur_pos = self.end_pos() - rest.len();
            if u < 0 {
                return None;
            }
            c = u;
        }

        Some(c)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L166-L222 (chrome/m156)
    fn parse_ident_token(&mut self) -> Option<String> {
        // <ident-token>
        // (--|-?([a-z|A-Z|_|non-ASCII]|escape))([a-z|A-Z|0-9|_|-|non-ASCII]|escape)?
        let saved = self.cur_pos;
        let result = self.parse_ident_token_inner();
        if result.is_none() {
            self.cur_pos = saved;
        }
        result
    }

    fn parse_ident_token_inner(&mut self) -> Option<String> {
        let mut ident = String::new();

        if self.parse_expected_string_token("--") {
            ident.push_str("--");
        } else {
            if self.parse_expected_string_token("-") {
                ident.push('-');
            }
            if let Some(c) = self.parse_escape() {
                append_unichar(&mut ident, c);
            } else {
                let mut rest = &self.s[self.cur_pos..];
                let c = next_utf8(&mut rest);
                self.cur_pos = self.end_pos() - rest.len();
                if c < 0 {
                    return None;
                }
                if !(('a' as i32..='z' as i32).contains(&c)
                    || ('A' as i32..='Z' as i32).contains(&c)
                    || c == '_' as i32
                    || (0x80..=0x10_FFFF).contains(&c))
                {
                    return None;
                }
                append_unichar(&mut ident, c);
            }
        }
        while self.cur_pos < self.end_pos() {
            if let Some(c) = self.parse_escape() {
                append_unichar(&mut ident, c);
                continue;
            }
            let mut next = &self.s[self.cur_pos..];
            let c = next_utf8(&mut next);
            if c < 0 {
                break;
            }
            if !(('a' as i32..='z' as i32).contains(&c)
                || ('A' as i32..='Z' as i32).contains(&c)
                || ('0' as i32..='9' as i32).contains(&c)
                || c == '_' as i32
                || c == '-' as i32
                || (0x80..=0x10_FFFF).contains(&c))
            {
                break;
            }
            append_unichar(&mut ident, c);
            self.cur_pos = self.end_pos() - next.len();
        }

        Some(ident)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L224-L246 (chrome/m156)
    fn parse_length_unit_token(&mut self) -> Option<LengthUnit> {
        const G_UNIT_INFO: [(&str, LengthUnit); 9] = [
            ("%", LengthUnit::Percentage),
            ("em", LengthUnit::EMS),
            ("ex", LengthUnit::EXS),
            ("px", LengthUnit::PX),
            ("cm", LengthUnit::CM),
            ("mm", LengthUnit::MM),
            ("in", LengthUnit::IN),
            ("pt", LengthUnit::PT),
            ("pc", LengthUnit::PC),
        ];

        for (name, unit) in G_UNIT_INFO {
            if self.parse_expected_string_token(name) {
                return Some(unit);
            }
        }
        None
    }

    // https://www.w3.org/TR/SVG11/types.html#DataTypeColor
    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L248-L262 (chrome/m156)
    fn parse_named_color_token(&mut self) -> Option<Color> {
        let saved = self.cur_pos;

        let Some(ident) = self.parse_ident_token() else {
            self.cur_pos = saved;
            return None;
        };
        let Some((_, c)) = find_named_color(&ident) else {
            self.cur_pos = saved;
            return None;
        };

        Some(c)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L264-L296 (chrome/m156)
    fn parse_hex_color_token(&mut self) -> Option<Color> {
        let saved = self.cur_pos;
        let result = self.parse_hex_color_token_inner();
        if result.is_none() {
            self.cur_pos = saved;
        }
        result
    }

    fn parse_hex_color_token_inner(&mut self) -> Option<Color> {
        if !self.parse_expected_string_token("#") {
            return None;
        }
        let hex_end = self.match_hex_token()?;

        let hex_string = &self.s[self.cur_pos..hex_end];
        let mut v = parse::find_hex(hex_string, 0).map_or(0, |(_, v)| v);

        match hex_string.len() {
            6 => {
                // matched #xxxxxxx
            }
            3 => {
                // matched '#xxx;
                v = ((v << 12) & 0x00f0_0000)
                    | ((v << 8) & 0x000f_f000)
                    | ((v << 4) & 0x0000_0ff0)
                    | (v & 0x0000_000f);
            }
            _ => return None,
        }

        self.cur_pos = hex_end;
        Some(Color::new(v | 0xff00_0000))
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L298-L313 (chrome/m156)
    fn parse_color_component_integral_token(&mut self) -> Option<i32> {
        let (mut p, mut c) = parse::find_s32(self.s, self.cur_pos)?;
        if at(self.s, p) == b'.' {
            // Fractional value.
            return None;
        }

        if at(self.s, p) == b'%' {
            #[allow(clippy::cast_precision_loss)] // mirrors `*c * 255.0f / 100`
            {
                c = scalar_round_to_int(c as f32 * 255.0 / 100.0);
            }
            c = t_pin(c, 0, 255);
            p += 1;
        }

        self.cur_pos = p;
        Some(c)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L315-L328 (chrome/m156)
    fn parse_color_component_fractional_token(&mut self) -> Option<i32> {
        let (mut p, s) = parse::find_scalar(self.s, self.cur_pos)?;
        if at(self.s, p) != b'%' {
            // Floating point must be a percentage (CSS2 rgb-percent syntax).
            return None;
        }
        p += 1; // Skip '%'

        let c = t_pin(scalar_round_to_int(s * 255.0 / 100.0), 0, 255);
        self.cur_pos = p;
        Some(c)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L330-L339 (chrome/m156)
    fn parse_color_component_scalar_token(&mut self) -> Option<i32> {
        if let Some((p, s)) = parse::find_scalar(self.s, self.cur_pos) {
            let c = t_pin(scalar_round_to_int(s * 255.0), 0, 255);
            self.cur_pos = p;
            return Some(c);
        }
        None
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L341-L344 (chrome/m156)
    fn parse_color_component_token(&mut self) -> Option<i32> {
        self.parse_color_component_integral_token()
            .or_else(|| self.parse_color_component_fractional_token())
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L346-L360 (chrome/m156)
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // static_cast<uint8_t>
    fn parse_rgb_color_token(&mut self) -> Option<Color> {
        let mut c = Color::BLACK;
        let ok = self.parse_parenthesized(
            Some("rgb"),
            |this, c: &mut Color| {
                if let Some(r) = this.parse_color_component_token()
                    && this.parse_sep_token()
                    && let Some(g) = this.parse_color_component_token()
                    && this.parse_sep_token()
                    && let Some(b) = this.parse_color_component_token()
                {
                    // static_cast<uint8_t>
                    *c = Color::from_rgb(r as u8, g as u8, b as u8);
                    return true;
                }
                false
            },
            &mut c,
        );
        ok.then_some(c)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L362-L380 (chrome/m156)
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // static_cast<uint8_t>
    fn parse_rgba_color_token(&mut self) -> Option<Color> {
        let mut c = Color::BLACK;
        let ok = self.parse_parenthesized(
            Some("rgba"),
            |this, c: &mut Color| {
                if let Some(r) = this.parse_color_component_token()
                    && this.parse_sep_token()
                    && let Some(g) = this.parse_color_component_token()
                    && this.parse_sep_token()
                    && let Some(b) = this.parse_color_component_token()
                    && this.parse_sep_token()
                    && let Some(a) = this.parse_color_component_scalar_token()
                {
                    // static_cast<uint8_t>
                    *c = Color::from_argb(a as u8, r as u8, g as u8, b as u8);
                    return true;
                }
                false
            },
            &mut c,
        );
        ok.then_some(c)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L382-L387 (chrome/m156)
    fn parse_color_token(&mut self) -> Option<Color> {
        self.parse_hex_color_token()
            .or_else(|| self.parse_named_color_token())
            .or_else(|| self.parse_rgba_color_token())
            .or_else(|| self.parse_rgb_color_token())
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L389-L396 (chrome/m156)
    fn parse_svg_color_type(&mut self) -> Option<ColorType> {
        self.parse_color_token()
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L416-L453 (chrome/m156)
    fn parse_svg_color(&mut self, vars: &mut Vec<String>) -> Option<Fill> {
        const VARS_LIMIT: usize = 32;

        if let Some(c) = self.parse_svg_color_type() {
            return Some(Fill::with_color_and_vars(c, std::mem::take(vars)));
        }
        if self.parse_expected_string_token("currentColor") {
            return Some(Fill::with_type_and_vars(
                ColorKind::CurrentColor,
                std::mem::take(vars),
            ));
        }
        // https://drafts.csswg.org/css-variables/#using-variables
        let mut result = Fill::default();
        if self.parse_parenthesized(
            Some("var"),
            |this, color_result: &mut Fill| {
                let ident = match this.parse_ident_token() {
                    Some(ident) if ident.len() >= 2 && ident.starts_with("--") => ident,
                    _ => return false,
                };
                vars.push(ident[2..].to_owned());
                this.parse_ws_token();
                if !this.parse_expected_string_token(",") {
                    *color_result = Fill::with_color_and_vars(Color::BLACK, std::mem::take(vars));
                    return true;
                }
                this.parse_ws_token();
                if this.match_string_token(")").is_some() {
                    *color_result = Fill::with_color_and_vars(Color::BLACK, std::mem::take(vars));
                    return true;
                }
                if vars.len() < VARS_LIMIT
                    && let Some(c) = this.parse_svg_color(vars)
                {
                    *color_result = c;
                    return true;
                }
                false
            },
            &mut result,
        ) {
            return Some(result);
        }
        None
    }

    // https://www.w3.org/TR/SVG11/types.html#DataTypeFuncIRI
    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L479-L488 (chrome/m156)
    fn parse_func_iri(&mut self) -> Option<FuncIri> {
        let mut out = FuncIri::default();
        let ok = self.parse_parenthesized(
            Some("url"),
            |this, iri_result: &mut FuncIri| {
                if let Some(iri) = Iri::parse(this) {
                    *iri_result = FuncIri::from_iri(iri);
                    return true;
                }
                false
            },
            &mut out,
        );
        ok.then_some(out)
    }

    /// <https://www.w3.org/TR/SVG11/types.html#DataTypeInteger>
    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L525-L544 (chrome/m156)
    #[doc(alias = "parseInteger")]
    pub fn parse_integer(&mut self) -> Option<IntegerType> {
        // consume WS
        self.parse_ws_token();

        // consume optional '+'
        self.parse_expected_string_token("+");

        if let Some(i) = self.parse_int32_token() {
            // consume trailing separators
            self.parse_sep_token();
            return Some(i);
        }

        None
    }

    /// <https://www.w3.org/TR/SVG11/coords.html#ViewBoxAttribute>
    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L563-L584 (chrome/m156)
    #[doc(alias = "parseViewBox")]
    pub fn parse_view_box(&mut self) -> Option<ViewBoxType> {
        self.parse_ws_token();

        let mut parsed_value = None;
        if let Some(x) = self.parse_scalar_token()
            && self.parse_sep_token()
            && let Some(y) = self.parse_scalar_token()
            && self.parse_sep_token()
            && let Some(w) = self.parse_scalar_token()
            && self.parse_sep_token()
            && let Some(h) = self.parse_scalar_token()
        {
            parsed_value = Some(Rect::from_xywh(x, y, w, h));
            // consume trailing whitespace
            self.parse_ws_token();
        }
        if self.parse_eos_token() {
            parsed_value
        } else {
            None
        }
    }

    // Parses a sequence of 'WS* <prefix> WS* (<nested>)', where the nested sequence
    // is handled by the passed functor.
    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L586-L612 (chrome/m156)
    fn parse_parenthesized<T>(
        &mut self,
        prefix: Option<&str>,
        f: impl FnOnce(&mut Self, &mut T) -> bool,
        result: &mut T,
    ) -> bool {
        let saved = self.cur_pos;

        self.parse_ws_token();
        if let Some(prefix) = prefix
            && !self.parse_expected_string_token(prefix)
        {
            self.cur_pos = saved;
            return false;
        }
        self.parse_ws_token();
        if !self.parse_expected_string_token("(") {
            self.cur_pos = saved;
            return false;
        }
        self.parse_ws_token();

        if !f(self, result) {
            self.cur_pos = saved;
            return false;
        }

        self.parse_ws_token();
        if !self.parse_expected_string_token(")") {
            self.cur_pos = saved;
            return false;
        }

        true
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L614-L627 (chrome/m156)
    fn parse_matrix_token(&mut self) -> Option<Matrix> {
        let mut m = Matrix::new_identity();
        let ok = self.parse_parenthesized(
            Some("matrix"),
            |this, m: &mut Matrix| {
                let mut scalars: [scalar; 6] = [0.0; 6];
                for (i, scalar) in scalars.iter_mut().enumerate() {
                    match this.parse_scalar_token() {
                        Some(v) if i > 4 || this.parse_sep_token() => *scalar = v,
                        _ => return false,
                    }
                }

                m.set_all(
                    scalars[0], scalars[2], scalars[4], scalars[1], scalars[3], scalars[5], 0.0,
                    0.0, 1.0,
                );
                true
            },
            &mut m,
        );
        ok.then_some(m)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L629-L645 (chrome/m156)
    fn parse_translate_token(&mut self) -> Option<Matrix> {
        let mut m = Matrix::new_identity();
        let ok = self.parse_parenthesized(
            Some("translate"),
            |this, m: &mut Matrix| {
                this.parse_ws_token();
                let Some(tx) = this.parse_scalar_token() else {
                    return false;
                };

                let ty = if this.parse_sep_token() {
                    this.parse_scalar_token().unwrap_or(0.0)
                } else {
                    0.0
                };

                m.set_translate((tx, ty));
                true
            },
            &mut m,
        );
        ok.then_some(m)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L647-L662 (chrome/m156)
    fn parse_scale_token(&mut self) -> Option<Matrix> {
        let mut m = Matrix::new_identity();
        let ok = self.parse_parenthesized(
            Some("scale"),
            |this, m: &mut Matrix| {
                let Some(sx) = this.parse_scalar_token() else {
                    return false;
                };

                let sy = if this.parse_sep_token() {
                    this.parse_scalar_token().unwrap_or(sx)
                } else {
                    sx
                };

                m.set_scale((sx, sy), None);
                true
            },
            &mut m,
        );
        ok.then_some(m)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L664-L685 (chrome/m156)
    fn parse_rotate_token(&mut self) -> Option<Matrix> {
        let mut m = Matrix::new_identity();
        let ok = self.parse_parenthesized(
            Some("rotate"),
            |this, m: &mut Matrix| {
                let Some(angle) = this.parse_scalar_token() else {
                    return false;
                };

                let mut cx = 0.0;
                let mut cy = 0.0;
                // optional [<cx> <cy>]
                if this.parse_sep_token()
                    && let Some(x) = this.parse_scalar_token()
                {
                    cx = x;
                    if !this.parse_sep_token() {
                        return false;
                    }
                    let Some(y) = this.parse_scalar_token() else {
                        return false;
                    };
                    cy = y;
                }

                m.set_rotate(angle, Some(Point::new(cx, cy)));
                true
            },
            &mut m,
        );
        ok.then_some(m)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L687-L696 (chrome/m156)
    fn parse_skew_x_token(&mut self) -> Option<Matrix> {
        let mut m = Matrix::new_identity();
        let ok = self.parse_parenthesized(
            Some("skewX"),
            |this, m: &mut Matrix| {
                let Some(angle) = this.parse_scalar_token() else {
                    return false;
                };
                // skia-rust: libm (tanf)
                m.set_skew_x(degrees_to_radians(angle).tan());
                true
            },
            &mut m,
        );
        ok.then_some(m)
    }

    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L698-L707 (chrome/m156)
    fn parse_skew_y_token(&mut self) -> Option<Matrix> {
        let mut m = Matrix::new_identity();
        let ok = self.parse_parenthesized(
            Some("skewY"),
            |this, m: &mut Matrix| {
                let Some(angle) = this.parse_scalar_token() else {
                    return false;
                };
                // skia-rust: libm (tanf)
                m.set_skew_y(degrees_to_radians(angle).tan());
                true
            },
            &mut m,
        );
        ok.then_some(m)
    }

    fn parse_enum_map<T: Copy>(&mut self, arr: &[(&str, T)]) -> Option<T> {
        for &(name, value) in arr {
            if self.parse_expected_string_token(name) {
                return Some(value);
            }
        }
        None
    }

    // https://www.w3.org/TR/SVG11/coords.html#PreserveAspectRatioAttribute
    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L1059-L1099 (chrome/m156)
    #[doc(alias = "parsePreserveAspectRatio")]
    pub fn parse_preserve_aspect_ratio(&mut self) -> Option<PreserveAspectRatio> {
        const G_ALIGN_MAP: [(&str, Align); 10] = [
            ("none", Align::None),
            ("xMinYMin", Align::XMinYMin),
            ("xMidYMin", Align::XMidYMin),
            ("xMaxYMin", Align::XMaxYMin),
            ("xMinYMid", Align::XMinYMid),
            ("xMidYMid", Align::XMidYMid),
            ("xMaxYMid", Align::XMaxYMid),
            ("xMinYMax", Align::XMinYMax),
            ("xMidYMax", Align::XMidYMax),
            ("xMaxYMax", Align::XMaxYMax),
        ];

        const G_SCALE_MAP: [(&str, Scale); 2] = [("meet", Scale::Meet), ("slice", Scale::Slice)];

        let mut par = PreserveAspectRatio::default();
        let mut parsed_value = false;

        // ignoring optional 'defer'
        self.parse_expected_string_token("defer");
        self.parse_ws_token();

        if let Some(align) = self.parse_enum_map(&G_ALIGN_MAP) {
            par.align = align;
            parsed_value = true;

            // optional scaling selector
            self.parse_ws_token();
            if let Some(scale) = self.parse_enum_map(&G_SCALE_MAP) {
                par.scale = scale;
            }
        }

        (parsed_value && self.parse_eos_token()).then_some(par)
    }

    // https://www.w3.org/TR/SVG11/types.html#DataTypeCoordinates
    // Port of: modules/svg/src/SkSVGAttributeParser.cpp#L1109-L1125 (chrome/m156)
    fn parse_list<T: Parse>(&mut self) -> Option<Vec<T>> {
        let mut vals = Vec::new();

        while let Some(v) = T::parse(self) {
            vals.push(v);

            self.parse_comma_wsp_token();
        }

        (!vals.is_empty() && self.parse_eos_token()).then_some(vals)
    }
}

fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

/// `SkString::appendUnichar`.
fn append_unichar(string: &mut String, c: i32) {
    #[allow(clippy::cast_sign_loss)] // a valid code point is not negative
    string.push(char::from_u32(c as u32).unwrap_or('\u{FFFD}'));
}

// https://www.w3.org/TR/SVG11/types.html#DataTypeColor
// And https://www.w3.org/TR/CSS2/syndata.html#color-units for the alternative
// forms supported by SVG (e.g. RGB percentages).
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L398-L408 (chrome/m156)
impl Parse for ColorType {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        p.parse_ws_token();
        let color = p.parse_svg_color_type()?;
        p.parse_ws_token();
        p.parse_eos_token().then_some(color)
    }
}

// https://www.w3.org/TR/SVG11/types.html#InterfaceSVGColor
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L455-L464 (chrome/m156)
impl Parse for Fill {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        p.parse_ws_token();
        let color = p.parse_svg_color(&mut Vec::new())?;
        p.parse_ws_token();
        p.parse_eos_token().then_some(color)
    }
}

// https://www.w3.org/TR/SVG11/linking.html#IRIReference
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L466-L488 (chrome/m156)
impl Parse for Iri {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        // consume preceding whitespace
        p.parse_ws_token();

        let ty = if p.parse_expected_string_token("#") {
            IriType::Local
        } else if p.match_string_token("data:").is_some() {
            IriType::DataURI
        } else {
            IriType::Nonlocal
        };

        let start = p.cur_pos;
        if !p.advance_while(|c| c != b')') {
            return None;
        }
        // The input is a `str` and the scan stops at an ASCII byte, so this slice is valid UTF-8.
        let text = String::from_utf8_lossy(&p.s[start..p.cur_pos]).into_owned();
        Some(Iri::new(ty, text))
    }
}

// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L490-L497 (chrome/m156)
impl Parse for StringType {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        if p.parse_eos_token() {
            return None;
        }
        let result = String::from_utf8_lossy(&p.s[p.cur_pos..]).into_owned();
        p.cur_pos += result.len();
        p.parse_eos_token().then_some(result)
    }
}

// https://www.w3.org/TR/SVG11/types.html#DataTypeNumber
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L499-L513 (chrome/m156)
impl Parse for NumberType {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        // consume WS
        p.parse_ws_token();

        if let Some(s) = p.parse_scalar_token() {
            // consume trailing separators
            p.parse_sep_token();
            return Some(s);
        }

        None
    }
}

impl Parse for IntegerType {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        p.parse_integer()
    }
}

// https://www.w3.org/TR/SVG11/types.html#DataTypeLength
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L546-L561 (chrome/m156)
impl Parse for Length {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        let s = p.parse_scalar_token()?;
        let mut u = LengthUnit::Number;

        if let Some(unit) = p.parse_length_unit_token() {
            u = unit;
        } else if !(p.parse_sep_token() || p.parse_eos_token()) {
            return None;
        }

        // consume trailing separators
        p.parse_sep_token();
        Some(Length::with_unit(s, u))
    }
}

// https://www.w3.org/TR/SVG11/coords.html#TransformAttribute
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L709-L740 (chrome/m156)
impl Parse for TransformType {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        let mut matrix = Matrix::new_identity();

        let mut parsed = false;
        while let Some(m) = p
            .parse_matrix_token()
            .or_else(|| p.parse_translate_token())
            .or_else(|| p.parse_scale_token())
            .or_else(|| p.parse_rotate_token())
            .or_else(|| p.parse_skew_x_token())
            .or_else(|| p.parse_skew_y_token())
        {
            matrix.pre_concat(&m);
            parsed = true;

            p.parse_comma_wsp_token();
        }

        p.parse_ws_token();
        if !parsed || !p.parse_eos_token() {
            return None;
        }

        Some(matrix)
    }
}

// https://www.w3.org/TR/SVG11/painting.html#SpecifyingPaint
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L742-L765 (chrome/m156)
impl Parse for Paint {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        let mut paint = None;

        p.parse_ws_token();
        if let Some(c) = p.parse_svg_color(&mut Vec::new()) {
            paint = Some(Paint::from_color(c));
        } else if p.parse_expected_string_token("none") {
            paint = Some(Paint::with_type(crate::types::PaintType::None));
        } else if let Some(iri) = p.parse_func_iri() {
            // optional fallback color
            p.parse_ws_token();
            let c = p.parse_svg_color(&mut Vec::new()).unwrap_or_default();
            paint = Some(Paint::from_iri(iri.iri().clone(), c));
        }
        p.parse_ws_token();
        if p.parse_eos_token() { paint } else { None }
    }
}

// https://www.w3.org/TR/SVG11/masking.html#ClipPathProperty
// https://www.w3.org/TR/SVG11/masking.html#MaskProperty
// https://www.w3.org/TR/SVG11/filters.html#FilterProperty
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L767-L784 (chrome/m156)
impl Parse for FuncIri {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        let firi = if p.parse_expected_string_token("none") {
            FuncIri::default()
        } else {
            p.parse_func_iri()?
        };

        p.parse_eos_token().then_some(firi)
    }
}

// https://www.w3.org/TR/SVG11/painting.html#StrokeLinecapProperty
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L786-L808 (chrome/m156)
impl Parse for LineCap {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        const G_CAP_INFO: [(&str, LineCap); 3] = [
            ("butt", LineCap::Butt),
            ("round", LineCap::Round),
            ("square", LineCap::Square),
        ];

        let cap = p.parse_enum_map(&G_CAP_INFO)?;
        p.parse_eos_token().then_some(cap)
    }
}

// https://www.w3.org/TR/SVG11/painting.html#StrokeLinejoinProperty
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L810-L834 (chrome/m156)
impl Parse for LineJoin {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        const G_JOIN_INFO: [(&str, LineJoinType); 4] = [
            ("miter", LineJoinType::Miter),
            ("round", LineJoinType::Round),
            ("bevel", LineJoinType::Bevel),
            ("inherit", LineJoinType::Inherit),
        ];

        let ty = p.parse_enum_map(&G_JOIN_INFO)?;
        p.parse_eos_token().then_some(LineJoin::new(ty))
    }
}

// https://www.w3.org/TR/SVG11/coords.html#ObjectBoundingBoxUnits
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L836-L851 (chrome/m156)
impl Parse for ObjectBoundingBoxUnits {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        let units = if p.parse_expected_string_token("userSpaceOnUse") {
            ObjectBoundingBoxUnits::new(ObjectBoundingBoxUnitsType::UserSpaceOnUse)
        } else if p.parse_expected_string_token("objectBoundingBox") {
            ObjectBoundingBoxUnits::new(ObjectBoundingBoxUnitsType::ObjectBoundingBox)
        } else {
            return None;
        };
        p.parse_eos_token().then_some(units)
    }
}

// https://www.w3.org/TR/SVG11/shapes.html#PolygonElementPointsAttribute
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L853-L901 (chrome/m156)
impl Parse for Vec<Point> {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        let mut pts = Vec::new();

        // Skip initial wsp.
        // list-of-points:
        //     wsp* coordinate-pairs? wsp*
        p.advance_while(is_ws);

        let mut parsed_value = false;
        loop {
            // Adjacent coordinate-pairs separated by comma-wsp.
            // coordinate-pairs:
            //     coordinate-pair
            //     | coordinate-pair comma-wsp coordinate-pairs
            if parsed_value && !p.parse_comma_wsp_token() {
                break;
            }

            let Some(x) = p.parse_scalar_token() else {
                break;
            };

            // Coordinate values separated by comma-wsp or '-'.
            // coordinate-pair:
            //     coordinate comma-wsp coordinate
            //     | coordinate negative-coordinate
            if !p.parse_comma_wsp_token() && !p.parse_eos_token() && p.cur() != b'-' {
                break;
            }

            let Some(y) = p.parse_scalar_token() else {
                break;
            };

            pts.push(Point::new(x, y));
            parsed_value = true;
        }

        (parsed_value && p.parse_eos_token()).then_some(pts)
    }
}

// https://www.w3.org/TR/SVG11/painting.html#FillRuleProperty
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L903-L926 (chrome/m156)
impl Parse for FillRule {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        const G_FILL_RULE_INFO: [(&str, FillRuleType); 3] = [
            ("nonzero", FillRuleType::NonZero),
            ("evenodd", FillRuleType::EvenOdd),
            ("inherit", FillRuleType::Inherit),
        ];

        let ty = p.parse_enum_map(&G_FILL_RULE_INFO)?;
        p.parse_eos_token().then_some(FillRule::new(ty))
    }
}

// https://www.w3.org/TR/SVG11/painting.html#VisibilityProperty
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L928-L951 (chrome/m156)
impl Parse for Visibility {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        const G_VISIBILITY_INFO: [(&str, VisibilityType); 4] = [
            ("visible", VisibilityType::Visible),
            ("hidden", VisibilityType::Hidden),
            ("collapse", VisibilityType::Collapse),
            ("inherit", VisibilityType::Inherit),
        ];

        let ty = p.parse_enum_map(&G_VISIBILITY_INFO)?;
        p.parse_eos_token().then_some(Visibility::new(ty))
    }
}

// https://www.w3.org/TR/SVG11/painting.html#StrokeDasharrayProperty
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L953-L984 (chrome/m156)
impl Parse for DashArray {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        let dash_array = if p.parse_expected_string_token("none") {
            DashArray::with_type(DashArrayType::None)
        } else if p.parse_expected_string_token("inherit") {
            DashArray::with_type(DashArrayType::Inherit)
        } else {
            let mut dashes = Vec::new();
            // parseLength() also consumes trailing separators.
            while let Some(dash) = Length::parse(p) {
                dashes.push(dash);
            }

            if dashes.is_empty() {
                return None;
            }
            DashArray::from_dashes(dashes)
        };

        p.parse_eos_token().then_some(dash_array)
    }
}

// https://www.w3.org/TR/SVG11/text.html#FontFamilyProperty
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L986-L1007 (chrome/m156)
impl Parse for FontFamily {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        let family;
        if p.parse_expected_string_token("inherit") {
            family = FontFamily::default();
        } else {
            // The spec allows specifying a comma-separated list for explicit fallback order.
            // For now, we only use the first entry and rely on the font manager to handle
            // fallback.
            let rest = &p.s[p.cur_pos..];
            let name = match rest.iter().position(|&c| c == b',') {
                Some(comma) => &rest[..comma],
                None => rest,
            };
            family = FontFamily::new(&String::from_utf8_lossy(name));
            p.cur_pos += rest.len();
        }

        p.parse_eos_token().then_some(family)
    }
}

// https://www.w3.org/TR/SVG11/text.html#FontSizeProperty
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L1009-L1030 (chrome/m156)
impl Parse for FontSize {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        let size = if p.parse_expected_string_token("inherit") {
            FontSize::default()
        } else {
            FontSize::new(Length::parse(p)?)
        };

        p.parse_eos_token().then_some(size)
    }
}

// https://www.w3.org/TR/SVG11/text.html#FontStyleProperty
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L1032-L1050 (chrome/m156)
impl Parse for FontStyle {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        const G_STYLE_MAP: [(&str, FontStyleType); 4] = [
            ("normal", FontStyleType::Normal),
            ("italic", FontStyleType::Italic),
            ("oblique", FontStyleType::Oblique),
            ("inherit", FontStyleType::Inherit),
        ];

        let ty = p.parse_enum_map(&G_STYLE_MAP)?;
        p.parse_eos_token().then_some(FontStyle::new(ty))
    }
}

// https://www.w3.org/TR/SVG11/text.html#FontWeightProperty
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L1052-L1081 (chrome/m156)
impl Parse for FontWeight {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        const G_WEIGHT_MAP: [(&str, FontWeightType); 14] = [
            ("normal", FontWeightType::Normal),
            ("bold", FontWeightType::Bold),
            ("bolder", FontWeightType::Bolder),
            ("lighter", FontWeightType::Lighter),
            ("100", FontWeightType::W100),
            ("200", FontWeightType::W200),
            ("300", FontWeightType::W300),
            ("400", FontWeightType::W400),
            ("500", FontWeightType::W500),
            ("600", FontWeightType::W600),
            ("700", FontWeightType::W700),
            ("800", FontWeightType::W800),
            ("900", FontWeightType::W900),
            ("inherit", FontWeightType::Inherit),
        ];

        let ty = p.parse_enum_map(&G_WEIGHT_MAP)?;
        p.parse_eos_token().then_some(FontWeight::new(ty))
    }
}

// https://www.w3.org/TR/SVG11/text.html#TextAnchorProperty
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L1083-L1101 (chrome/m156)
impl Parse for TextAnchor {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        const G_ANCHOR_MAP: [(&str, TextAnchorType); 4] = [
            ("start", TextAnchorType::Start),
            ("middle", TextAnchorType::Middle),
            ("end", TextAnchorType::End),
            ("inherit", TextAnchorType::Inherit),
        ];

        let ty = p.parse_enum_map(&G_ANCHOR_MAP)?;
        p.parse_eos_token().then_some(TextAnchor::new(ty))
    }
}

// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L1101-L1104 (chrome/m156)
impl Parse for PreserveAspectRatio {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        p.parse_preserve_aspect_ratio()
    }
}

// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L1127-L1129 (chrome/m156)
impl Parse for Vec<Length> {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        p.parse_list::<Length>()
    }
}

// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L1131-L1133 (chrome/m156)
impl Parse for Vec<NumberType> {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        p.parse_list::<NumberType>()
    }
}

// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L1135-L1145 (chrome/m156)
impl Parse for Colorspace {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        const G_COLORSPACE_MAP: [(&str, Colorspace); 3] = [
            ("auto", Colorspace::Auto),
            ("sRGB", Colorspace::SRGB),
            ("linearRGB", Colorspace::LinearRGB),
        ];

        let cs = p.parse_enum_map(&G_COLORSPACE_MAP)?;
        p.parse_eos_token().then_some(cs)
    }
}

// https://www.w3.org/TR/SVG11/painting.html#DisplayProperty
// Port of: modules/svg/src/SkSVGAttributeParser.cpp#L1147-L1167 (chrome/m156)
impl Parse for Display {
    fn parse(p: &mut AttributeParser<'_>) -> Option<Self> {
        const G_DISPLAY_INFO: [(&str, Display); 2] =
            [("inline", Display::Inline), ("none", Display::None)];

        let display = p.parse_enum_map(&G_DISPLAY_INFO)?;
        p.parse_eos_token().then_some(display)
    }
}
