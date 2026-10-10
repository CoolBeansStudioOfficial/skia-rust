// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/xml/SkXMLParser.h, src/xml/SkXMLParser.cpp, src/xml/SkDOM.cpp

//! `SkXMLParser`: a callback-driven XML parser.

use std::borrow::Cow;

use super::dom::{Dom, Node};
use crate::stream::Stream;

/// Why a parse failed, as the callers of `SkXMLParser` report it.
// Port of: src/xml/SkXMLParser.h#L19-L28 (chrome/m156)
#[doc(alias = "SkXMLParserError::ErrorCode")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum XmlParserErrorCode {
    #[default]
    NoError,
    EmptyFile,
    UnknownElement,
    UnknownAttributeName,
    ErrorInAttributeValue,
    DuplicateIDs,
    UnknownError,
}

// Port of: src/xml/SkXMLParser.cpp#L24-L31 (chrome/m156)
const ERROR_STRINGS: [&str; 6] = [
    "empty or missing file ",
    "unknown element ",
    "unknown attribute name ",
    "error in attribute value ",
    "duplicate ID ",
    "unknown error ",
];

/// Error state of a parse.
// Port of: src/xml/SkXMLParser.h#L17-L50 (chrome/m156)
#[doc(alias = "SkXMLParserError")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlParserError {
    code: XmlParserErrorCode,
    line_number: i32,
    native_code: i32,
    noun: String,
}

impl Default for XmlParserError {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlParserError {
    // Port of: src/xml/SkXMLParser.cpp#L33-L37 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self {
            code: XmlParserErrorCode::NoError,
            line_number: -1,
            native_code: -1,
            noun: String::new(),
        }
    }

    #[doc(alias = "getErrorCode")]
    #[must_use]
    pub fn error_code(&self) -> XmlParserErrorCode {
        self.code
    }

    /// Appends the error description to `str`.
    // Port of: src/xml/SkXMLParser.cpp#L44-L57 (chrome/m156)
    #[doc(alias = "getErrorString")]
    pub fn get_error_string(&self, str: &mut String) {
        let mut temp = String::new();
        if self.code == XmlParserErrorCode::NoError {
            XmlParserError::get_native_error_string(self.native_code, &mut temp);
        } else {
            // `(unsigned)fCode < std::size(gErrorStrings)` then `gErrorStrings[fCode - 1]`
            let code = self.code as usize;
            if code < ERROR_STRINGS.len() {
                temp.push_str(ERROR_STRINGS[code - 1]);
            }
            temp.push_str(&self.noun);
        }
        str.push_str(&temp);
    }

    #[doc(alias = "getLineNumber")]
    #[must_use]
    pub fn line_number(&self) -> i32 {
        self.line_number
    }

    #[doc(alias = "getNativeCode")]
    #[must_use]
    pub fn native_code(&self) -> i32 {
        self.native_code
    }

    #[doc(alias = "hasError")]
    #[must_use]
    pub fn has_error(&self) -> bool {
        self.code != XmlParserErrorCode::NoError || self.native_code != -1
    }

    #[doc(alias = "hasNoun")]
    #[must_use]
    pub fn has_noun(&self) -> bool {
        !self.noun.is_empty()
    }

    // Port of: src/xml/SkXMLParser.cpp#L59-L63 (chrome/m156)
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    #[doc(alias = "setCode")]
    pub fn set_code(&mut self, code: XmlParserErrorCode) {
        self.code = code;
    }

    #[doc(alias = "setNoun")]
    pub fn set_noun(&mut self, noun: &str) {
        self.noun.clear();
        self.noun.push_str(noun);
    }

    /// Records the line of a failed parse (`SkXMLParser::reportError`'s job).
    pub(crate) fn set_line_number(&mut self, line: i32) {
        self.line_number = line;
    }

    /// `SkXMLParser::GetNativeErrorString` has an empty body in Skia.
    // Port of: src/xml/SkXMLParser.cpp#L216-L219 (chrome/m156)
    #[doc(alias = "GetNativeErrorString")]
    pub fn get_native_error_string(_native_error_code: i32, _str: &mut String) {}
}

/// Receiver of the parse events. Every `on_*` returns `true` to stop parsing, as in
/// `SkXMLParser` (the expat front end ignores the result, `SkDOM`'s walkers honor it).
// Port of: src/xml/SkXMLParser.h#L52-L86 (chrome/m156)
#[doc(alias = "SkXMLParser")]
pub trait XmlParser {
    #[doc(alias = "onStartElement")]
    fn on_start_element(&mut self, _elem: &str) -> bool {
        false
    }
    #[doc(alias = "onAddAttribute")]
    fn on_add_attribute(&mut self, _name: &str, _value: &str) -> bool {
        false
    }
    #[doc(alias = "onEndElement")]
    fn on_end_element(&mut self, _elem: &str) -> bool {
        false
    }
    #[doc(alias = "onText")]
    fn on_text(&mut self, _text: &str) -> bool {
        false
    }

    /// The error state this parser reports into, if it has one (`fError`).
    fn error_mut(&mut self) -> Option<&mut XmlParserError> {
        None
    }

    // Port of: src/xml/SkXMLParser.cpp#L228-L246 (chrome/m156)
    #[doc(alias = "startElement")]
    fn start_element(&mut self, elem: &str) -> bool {
        self.on_start_element(elem)
    }
    #[doc(alias = "addAttribute")]
    fn add_attribute(&mut self, name: &str, value: &str) -> bool {
        self.on_add_attribute(name, value)
    }
    #[doc(alias = "endElement")]
    fn end_element(&mut self, elem: &str) -> bool {
        self.on_end_element(elem)
    }
    fn text(&mut self, text: &str) -> bool {
        self.on_text(text)
    }

    /// Parses an XML document held in memory. Returns `true` for success.
    // Port of: src/xml/SkXMLParser.cpp#L209-L214 (chrome/m156)
    fn parse(&mut self, doc: &[u8]) -> bool
    where
        Self: Sized,
    {
        match tokenize(doc, self) {
            Ok(()) => true,
            Err(line) => {
                if let Some(e) = self.error_mut() {
                    e.set_line_number(line);
                }
                false
            }
        }
    }

    /// Parses an XML document read from `doc_stream`.
    // Port of: src/xml/SkXMLParser.cpp#L143-L207 (chrome/m156)
    fn parse_stream(&mut self, doc_stream: &mut dyn Stream) -> bool
    where
        Self: Sized,
    {
        let mut doc = Vec::new();
        let mut memory = None;
        if let (true, Some(base)) = (doc_stream.has_length(), doc_stream.get_memory_base()) {
            // `base + getPosition()`, `getLength() - getPosition()`
            memory = Some(base[doc_stream.get_position()..].to_vec());
        }
        if let Some(m) = memory {
            doc = m;
        } else {
            let mut buffer = [0u8; 4096];
            loop {
                let len = doc_stream.read(&mut buffer);
                doc.extend_from_slice(&buffer[..len]);
                if doc_stream.is_at_end() {
                    break;
                }
            }
        }
        self.parse(&doc)
    }

    /// Replays the subtree of `dom` at `node` as parse events.
    // Port of: src/xml/SkDOM.cpp#L16-L40 (chrome/m156)
    fn parse_dom(&mut self, dom: &Dom, node: Node) -> bool
    where
        Self: Sized,
    {
        let elem_name = dom.get_name(node);

        if self.start_element(elem_name) {
            return false;
        }

        for (name, value) in dom.attrs(node) {
            if self.add_attribute(name, value) {
                return false;
            }
        }

        let mut child = dom.get_first_child(node, None);
        while let Some(c) = child {
            if !self.parse_dom(dom, c) {
                return false;
            }
            child = dom.get_next_sibling(c, None);
        }
        !self.end_element(elem_name)
    }
}

// ---------------------------------------------------------------------------------------------
// The tokenizer (stands in for expat; see the module docs).

struct Cursor<'a> {
    s: &'a [u8],
    pos: usize,
}

/// Byte offset of a well-formedness error.
type Fail = usize;

fn is_xml_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n')
}

fn is_name_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c == b':' || c >= 0x80
}

fn is_name_char(c: u8) -> bool {
    is_name_start(c) || c.is_ascii_digit() || c == b'-' || c == b'.'
}

/// The XML `Char` production.
fn is_xml_char(c: u32) -> bool {
    matches!(c, 0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x1_0000..=0x10_FFFF)
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn starts_with(&self, lit: &str) -> bool {
        self.s[self.pos..].starts_with(lit.as_bytes())
    }

    fn skip_space(&mut self) -> bool {
        let start = self.pos;
        while self.peek().is_some_and(is_xml_space) {
            self.pos += 1;
        }
        self.pos > start
    }

    fn name(&mut self) -> Result<&'a str, Fail> {
        let start = self.pos;
        if !self.peek().is_some_and(is_name_start) {
            return Err(self.pos);
        }
        while self.peek().is_some_and(is_name_char) {
            self.pos += 1;
        }
        // The input is a `&str` and every byte >= 0x80 is a name character, so a name ends on
        // a character boundary.
        std::str::from_utf8(&self.s[start..self.pos]).map_err(|_| start)
    }

    /// Skips up to and including `end`; fails at the end of input.
    fn skip_past(&mut self, end: &str) -> Result<(), Fail> {
        let hay = &self.s[self.pos..];
        let e = end.as_bytes();
        match hay.windows(e.len()).position(|w| w == e) {
            Some(i) => {
                self.pos += i + e.len();
                Ok(())
            }
            None => Err(self.s.len()),
        }
    }

    /// Parses `&...;` at the `&`. Appends the replacement to `out`.
    fn reference(&mut self, out: &mut Vec<u8>) -> Result<(), Fail> {
        let at = self.pos;
        self.pos += 1; // '&'
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|c| c != b';' && c != b'<' && c != b'&' && !is_xml_space(c))
        {
            self.pos += 1;
        }
        if self.peek() != Some(b';') {
            return Err(at);
        }
        let body = &self.s[start..self.pos];
        self.pos += 1;
        match body {
            b"lt" => out.push(b'<'),
            b"gt" => out.push(b'>'),
            b"amp" => out.push(b'&'),
            b"quot" => out.push(b'"'),
            b"apos" => out.push(b'\''),
            _ => {
                let num = body.strip_prefix(b"#").ok_or(at)?;
                let (digits, radix) = match num.strip_prefix(b"x") {
                    Some(h) => (h, 16),
                    None => (num, 10),
                };
                if digits.is_empty() {
                    return Err(at);
                }
                let text = std::str::from_utf8(digits).map_err(|_| at)?;
                if !text.chars().all(|ch| ch.is_digit(radix)) {
                    return Err(at);
                }
                let code = u32::from_str_radix(text, radix).map_err(|_| at)?;
                if !is_xml_char(code) {
                    return Err(at);
                }
                let ch = char::from_u32(code).ok_or(at)?;
                let mut buf = [0u8; 4];
                out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            }
        }
        Ok(())
    }

    fn attr_value(&mut self) -> Result<String, Fail> {
        let Some(quote @ (b'"' | b'\'')) = self.peek() else {
            return Err(self.pos);
        };
        self.pos += 1;
        let mut out = Vec::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.s.len());
            };
            if c == quote {
                self.pos += 1;
                break;
            }
            match c {
                b'<' => return Err(self.pos),
                b'&' => self.reference(&mut out)?,
                b'\r' => {
                    self.pos += 1;
                    if self.peek() == Some(b'\n') {
                        self.pos += 1;
                    }
                    out.push(b' ');
                }
                b'\n' | b'\t' => {
                    self.pos += 1;
                    out.push(b' ');
                }
                c if c < 0x20 => return Err(self.pos),
                c => {
                    self.pos += 1;
                    out.push(c);
                }
            }
        }
        String::from_utf8(out).map_err(|_| self.pos)
    }
}

/// The value of the `encoding` pseudo-attribute of an XML declaration at the start of `body`.
fn declared_encoding(body: &[u8]) -> Option<String> {
    if !(body.starts_with(b"<?xml") && body.get(5).copied().is_some_and(is_xml_space)) {
        return None;
    }
    let end = body.windows(2).position(|w| w == b"?>")?;
    let decl = &body[..end];
    let i = decl.windows(8).position(|w| w == b"encoding")?;
    let mut c = Cursor {
        s: decl,
        pos: i + 8,
    };
    c.skip_space();
    if c.peek() != Some(b'=') {
        return None;
    }
    c.pos += 1;
    c.skip_space();
    let value = c.attr_value().ok()?;
    Some(value.to_ascii_lowercase())
}

/// Decodes the document to UTF-8 according to its XML declaration, for the encodings expat
/// ships with (UTF-16 is not supported).
fn decode(doc: &[u8]) -> Result<Cow<'_, str>, Fail> {
    let body = doc.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(doc);
    let encoding = declared_encoding(body).unwrap_or_else(|| "utf-8".to_owned());
    match encoding.as_str() {
        "utf-8" | "us-ascii" => std::str::from_utf8(body)
            .map(Cow::Borrowed)
            .map_err(|e| e.valid_up_to()),
        "iso-8859-1" => Ok(Cow::Owned(body.iter().map(|&b| char::from(b)).collect())),
        _ => Err(0),
    }
}

/// Skips a `<!DOCTYPE ...>` declaration. Fails on an entity declaration, which `SkXMLParser`
/// refuses (it stops expat's processing, CVE-2013-0340).
fn skip_doctype(c: &mut Cursor<'_>) -> Result<(), Fail> {
    c.pos += "<!DOCTYPE".len();
    let mut depth = 0i32; // inside `[` ... `]`
    loop {
        let Some(ch) = c.peek() else {
            return Err(c.s.len());
        };
        match ch {
            b'"' | b'\'' => {
                c.pos += 1;
                c.skip_past(if ch == b'"' { "\"" } else { "'" })?;
            }
            b'[' => {
                depth += 1;
                c.pos += 1;
            }
            b']' => {
                depth -= 1;
                c.pos += 1;
            }
            b'>' if depth <= 0 => {
                c.pos += 1;
                return Ok(());
            }
            b'<' if c.starts_with("<!--") => {
                c.pos += 4;
                c.skip_past("-->")?;
            }
            b'<' if c.starts_with("<!ENTITY") => return Err(c.pos),
            _ => c.pos += 1,
        }
    }
}

/// Handles comments and processing instructions; returns whether one was consumed.
fn misc(c: &mut Cursor<'_>, doc_start: usize) -> Result<bool, Fail> {
    if c.starts_with("<!--") {
        c.pos += 4;
        c.skip_past("-->")?;
        Ok(true)
    } else if c.starts_with("<?") {
        let at = c.pos;
        c.pos += 2;
        let target = c.name()?;
        if target.eq_ignore_ascii_case("xml") && at != doc_start {
            // The XML declaration is only allowed at the very start.
            return Err(at);
        }
        c.skip_past("?>")?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// The text that expat buffered between two tags (`ParsingContext::fBufferedText`).
struct TextBuffer(Vec<u8>);

impl TextBuffer {
    // Port of: src/xml/SkXMLParser.cpp#L82-L87 (chrome/m156)
    fn flush(&mut self, handler: &mut dyn XmlParser) {
        if !self.0.is_empty() {
            let text = String::from_utf8_lossy(&self.0).into_owned();
            handler.text(&text);
            self.0.clear();
        }
    }
}

fn line_of(doc: &str, offset: usize) -> i32 {
    let end = offset.min(doc.len());
    #[allow(clippy::naive_bytecount)] // no extra dependency for a line count
    let n = doc.as_bytes()[..end]
        .iter()
        .filter(|&&b| b == b'\n')
        .count();
    i32::try_from(n + 1).unwrap_or(i32::MAX)
}

/// Runs `handler` over the events of `doc`. On a well-formedness error, returns the 1-based
/// line number where it was detected; events before the error have already been delivered.
// Port of: src/xml/SkXMLParser.cpp#L72-L207 (chrome/m156) (expat's role)
pub(crate) fn tokenize(doc: &[u8], handler: &mut dyn XmlParser) -> Result<(), i32> {
    let text = match decode(doc) {
        Ok(t) => t,
        Err(at) => return Err(line_of(&String::from_utf8_lossy(doc), at)),
    };
    run(&text, handler).map_err(|at| line_of(&text, at))
}

#[allow(clippy::too_many_lines)] // one pass over the document, like expat's state machine
fn run(text: &str, handler: &mut dyn XmlParser) -> Result<(), Fail> {
    let mut c = Cursor {
        s: text.as_bytes(),
        pos: 0,
    };
    if c.starts_with("\u{feff}") {
        c.pos += 3;
    }
    let doc_start = c.pos;

    // Prolog.
    let mut seen_doctype = false;
    loop {
        c.skip_space();
        if c.pos >= c.s.len() {
            return Err(c.s.len()); // no element found
        }
        if misc(&mut c, doc_start)? {
            continue;
        }
        if c.starts_with("<!DOCTYPE") && !seen_doctype {
            seen_doctype = true;
            skip_doctype(&mut c)?;
            continue;
        }
        break;
    }

    // The root element and its content.
    let mut stack: Vec<&str> = Vec::new();
    let mut buffered = TextBuffer(Vec::new());
    let mut root_done = false;
    while !root_done {
        let Some(ch) = c.peek() else {
            return Err(c.s.len()); // unclosed token
        };
        if ch != b'<' && stack.is_empty() {
            return Err(c.pos);
        }
        match ch {
            b'<' => {
                if misc(&mut c, doc_start)? {
                    continue;
                }
                if c.starts_with("<![CDATA[") {
                    if stack.is_empty() {
                        return Err(c.pos);
                    }
                    c.pos += 9;
                    let start = c.pos;
                    c.skip_past("]]>")?;
                    normalize_newlines(&c.s[start..c.pos - 3], &mut buffered.0);
                    continue;
                }
                if c.starts_with("</") {
                    c.pos += 2;
                    let name = c.name()?;
                    c.skip_space();
                    if c.peek() != Some(b'>') {
                        return Err(c.pos);
                    }
                    c.pos += 1;
                    if stack.pop() != Some(name) {
                        return Err(c.pos);
                    }
                    buffered.flush(handler);
                    handler.end_element(name);
                    root_done = stack.is_empty();
                    continue;
                }
                // Start tag.
                c.pos += 1;
                let name = c.name()?;
                buffered.flush(handler);
                let mut attrs: Vec<(&str, String)> = Vec::new();
                let empty;
                loop {
                    let had_space = c.skip_space();
                    match c.peek() {
                        Some(b'>') => {
                            c.pos += 1;
                            empty = false;
                            break;
                        }
                        Some(b'/') => {
                            c.pos += 1;
                            if c.peek() != Some(b'>') {
                                return Err(c.pos);
                            }
                            c.pos += 1;
                            empty = true;
                            break;
                        }
                        Some(_) if had_space => {
                            let attr_name = c.name()?;
                            c.skip_space();
                            if c.peek() != Some(b'=') {
                                return Err(c.pos);
                            }
                            c.pos += 1;
                            c.skip_space();
                            let value = c.attr_value()?;
                            if attrs.iter().any(|(n, _)| *n == attr_name) {
                                return Err(c.pos);
                            }
                            attrs.push((attr_name, value));
                        }
                        _ => return Err(c.pos),
                    }
                }
                handler.start_element(name);
                for (n, v) in &attrs {
                    handler.add_attribute(n, v);
                }
                if empty {
                    handler.end_element(name);
                    root_done = stack.is_empty();
                } else {
                    stack.push(name);
                }
            }
            b'&' => c.reference(&mut buffered.0)?,
            b'\r' => {
                c.pos += 1;
                if c.peek() == Some(b'\n') {
                    c.pos += 1;
                }
                buffered.0.push(b'\n');
            }
            b']' if c.starts_with("]]>") => return Err(c.pos),
            ch if ch < 0x20 && !matches!(ch, b'\t' | b'\n') => return Err(c.pos),
            ch => {
                c.pos += 1;
                buffered.0.push(ch);
            }
        }
    }

    // Epilog.
    loop {
        c.skip_space();
        if c.pos >= c.s.len() {
            return Ok(());
        }
        if !misc(&mut c, doc_start)? {
            return Err(c.pos);
        }
    }
}

fn normalize_newlines(raw: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'\r' {
            out.push(b'\n');
            if raw.get(i + 1) == Some(&b'\n') {
                i += 1;
            }
        } else {
            out.push(raw[i]);
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Log(Vec<String>);

    impl XmlParser for Log {
        fn on_start_element(&mut self, e: &str) -> bool {
            self.0.push(format!("<{e}"));
            false
        }
        fn on_add_attribute(&mut self, n: &str, v: &str) -> bool {
            self.0.push(format!("{n}={v}"));
            false
        }
        fn on_end_element(&mut self, e: &str) -> bool {
            self.0.push(format!("</{e}"));
            false
        }
        fn on_text(&mut self, t: &str) -> bool {
            self.0.push(format!("'{t}'"));
            false
        }
    }

    fn events(doc: &str) -> Option<Vec<String>> {
        let mut l = Log::default();
        l.parse(doc.as_bytes()).then_some(l.0)
    }

    #[test]
    fn basic() {
        let e = events("<?xml version='1.0'?><!-- c --><a x='1\n2' y=\"&lt;&#65;\">t&amp;<b/>u<![CDATA[<>]]></a>\n")
            .unwrap();
        assert_eq!(
            e,
            ["<a", "x=1 2", "y=<A", "'t&'", "<b", "</b", "'u<>'", "</a"]
        );
    }

    #[test]
    fn malformed() {
        for d in [
            "",
            "<a>",
            "<a></b>",
            "<a/><b/>",
            "<a x=1/>",
            "<a x='1' x='2'/>",
            "<a>&foo;</a>",
            "<!DOCTYPE a [<!ENTITY e 'x'>]><a/>",
            "<a",
            "x<a/>",
            "<a><</a>",
        ] {
            assert!(events(d).is_none(), "{d}");
        }
    }

    #[test]
    fn doctype_without_entities() {
        assert!(
            events("<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"x.dtd\"><svg/>").is_some()
        );
    }
}
