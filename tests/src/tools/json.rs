// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/utils/SkJSONWriter.{h,cpp} (kFast mode only) and the subset of
// modules/jsonreader/SkJSONReader.{h,cpp} (`skjson::DOM`) that tools/sksltrace uses (chrome/m156)

//! Minimal JSON for the trace tools: a writer that produces the same text as `SkJSONWriter` in
//! `Mode::kFast`, and a parser for the DOM subset `SkSLTraceUtils::ReadTrace` reads.
//!
//! The writer has no pretty mode, as the trace format only uses kFast. The parser accepts any
//! JSON text, but does not reproduce `skjson`'s error reporting: it returns `None` on any error.

use std::fmt::Write as _;

/// `SkJSONWriter` in `Mode::kFast`: the minimal text, no whitespace.
#[derive(Debug, Default)]
pub struct JsonWriter {
    out: String,
    /// One entry per open object or array: how many values it holds so far.
    scopes: Vec<usize>,
}

impl JsonWriter {
    /// A writer with nothing written yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes `,` before every value after the first in its object or array, and the name
    /// (`"name":`) when the value has one. `appendName` writes the name without escaping it.
    // Port of: src/utils/SkJSONWriter.h#L92-L106 (appendName) and #L336-L354 (beginValue)
    fn begin_value(&mut self, name: Option<&str>) {
        if let Some(count) = self.scopes.last_mut() {
            if *count > 0 {
                self.out.push(',');
            }
            *count += 1;
        }
        if let Some(name) = name {
            self.out.push('"');
            self.out.push_str(name);
            self.out.push_str("\":");
        }
    }

    /// `beginObject(name)`.
    // Port of: src/utils/SkJSONWriter.h#L116-L123 (chrome/m156)
    pub fn begin_object(&mut self, name: Option<&str>) {
        self.begin_value(name);
        self.out.push('{');
        self.scopes.push(0);
    }

    /// `endObject()`.
    // Port of: src/utils/SkJSONWriter.h#L128-L138 (chrome/m156)
    pub fn end_object(&mut self) {
        self.scopes.pop();
        self.out.push('}');
    }

    /// `beginArray(name)`.
    // Port of: src/utils/SkJSONWriter.h#L148-L155 (chrome/m156)
    pub fn begin_array(&mut self, name: Option<&str>) {
        self.begin_value(name);
        self.out.push('[');
        self.scopes.push(0);
    }

    /// `endArray()`.
    // Port of: src/utils/SkJSONWriter.h#L160-L170 (chrome/m156)
    pub fn end_array(&mut self) {
        self.scopes.pop();
        self.out.push(']');
    }

    /// `appendString(name, value)`: escapes the value as `SkJSONWriter` does.
    // Port of: src/utils/SkJSONWriter.h#L178-L213 (chrome/m156)
    pub fn append_string(&mut self, name: Option<&str>, value: &str) {
        self.begin_value(name);
        self.out.push('"');
        for c in value.chars() {
            match c {
                '"' => self.out.push_str("\\\""),
                '\\' => self.out.push_str("\\\\"),
                '\u{8}' => self.out.push_str("\\b"),
                '\u{c}' => self.out.push_str("\\f"),
                '\n' => self.out.push_str("\\n"),
                '\r' => self.out.push_str("\\r"),
                '\t' => self.out.push_str("\\t"),
                // `appendHex` writes uppercase digits, with at least four of them.
                c if (c as u32) < 0x20 => {
                    // Writing to a String cannot fail.
                    let _ = write!(self.out, "\\u{:04X}", c as u32);
                }
                c => self.out.push(c),
            }
        }
        self.out.push('"');
    }

    /// `appendS32(name, value)`: `%d`.
    // Port of: src/utils/SkJSONWriter.h#L239 and #L288 (DEFINE_NAMED_APPEND) (chrome/m156)
    pub fn append_s32(&mut self, name: Option<&str>, value: i32) {
        self.begin_value(name);
        self.out.push_str(&value.to_string());
    }

    /// The text written so far.
    #[must_use]
    pub fn finish(self) -> String {
        self.out
    }
}

/// A parsed JSON value (the `skjson::Value` kinds).
#[derive(Clone, Debug, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    /// Members in document order. Lookups return the first member with the name.
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    /// `ObjectValue::operator[]`: the member called `name`, if this is an object that has one.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&JsonValue> {
        match self {
            Self::Object(members) => members.iter().find(|(k, _)| k == name).map(|(_, v)| v),
            _ => None,
        }
    }

    /// `ArrayValue::operator[]`: the element at `index`, if this is an array that has one.
    #[must_use]
    pub fn index(&self, index: usize) -> Option<&JsonValue> {
        match self {
            Self::Array(items) => items.get(index),
            _ => None,
        }
    }

    /// The string, if this is a string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// The number, if this is a number.
    #[must_use]
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// The elements, if this is an array.
    #[must_use]
    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }
}

/// Parses a complete JSON document. `None` if the text is not one value followed by whitespace.
// Port of: modules/jsonreader/SkJSONReader.cpp (skjson::DOM), reduced to what ReadTrace needs.
#[must_use]
pub fn parse(text: &[u8]) -> Option<JsonValue> {
    let mut p = Parser { text, pos: 0 };
    let value = p.value()?;
    p.skip_whitespace();
    (p.pos == text.len()).then_some(value)
}

struct Parser<'a> {
    text: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, literal: &[u8]) -> bool {
        if self.text[self.pos..].starts_with(literal) {
            self.pos += literal.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self) -> Option<JsonValue> {
        self.skip_whitespace();
        let value = match self.peek()? {
            b'{' => self.object()?,
            b'[' => self.array()?,
            b'"' => JsonValue::String(self.string()?),
            b't' if self.eat(b"true") => JsonValue::Bool(true),
            b'f' if self.eat(b"false") => JsonValue::Bool(false),
            b'n' if self.eat(b"null") => JsonValue::Null,
            b'-' | b'0'..=b'9' => self.number()?,
            _ => return None,
        };
        self.skip_whitespace();
        Some(value)
    }

    fn object(&mut self) -> Option<JsonValue> {
        self.pos += 1; // '{'
        let mut members = Vec::new();
        self.skip_whitespace();
        if self.eat(b"}") {
            return Some(JsonValue::Object(members));
        }
        loop {
            self.skip_whitespace();
            if self.peek()? != b'"' {
                return None;
            }
            let name = self.string()?;
            self.skip_whitespace();
            if !self.eat(b":") {
                return None;
            }
            let value = self.value()?;
            members.push((name, value));
            self.skip_whitespace();
            if self.eat(b",") {
                continue;
            }
            return self.eat(b"}").then_some(JsonValue::Object(members));
        }
    }

    fn array(&mut self) -> Option<JsonValue> {
        self.pos += 1; // '['
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.eat(b"]") {
            return Some(JsonValue::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_whitespace();
            if self.eat(b",") {
                continue;
            }
            return self.eat(b"]").then_some(JsonValue::Array(items));
        }
    }

    fn hex4(&mut self) -> Option<u32> {
        let digits = self.text.get(self.pos..self.pos + 4)?;
        let digits = std::str::from_utf8(digits).ok()?;
        let value = u32::from_str_radix(digits, 16).ok()?;
        self.pos += 4;
        Some(value)
    }

    fn string(&mut self) -> Option<String> {
        self.pos += 1; // opening quote
        let mut bytes = Vec::new();
        loop {
            let c = self.peek()?;
            self.pos += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let escaped = self.peek()?;
                    self.pos += 1;
                    let ch = match escaped {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let unit = self.hex4()?;
                            if (0xD800..0xDC00).contains(&unit) && self.eat(b"\\u") {
                                let low = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&low) {
                                    return None;
                                }
                                char::from_u32(0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00))?
                            } else {
                                char::from_u32(unit)?
                            }
                        }
                        _ => return None,
                    };
                    let mut buf = [0u8; 4];
                    bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                c => bytes.push(c),
            }
        }
        String::from_utf8(bytes).ok()
    }

    fn number(&mut self) -> Option<JsonValue> {
        let start = self.pos;
        while matches!(self.peek(), Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')) {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.text[start..self.pos]).ok()?;
        text.parse::<f64>().ok().map(JsonValue::Number)
    }
}
