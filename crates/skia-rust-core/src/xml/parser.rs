// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/xml/SkXMLParser.h, src/xml/SkXMLParser.cpp, src/xml/SkDOM.cpp

//! `SkXMLParser`: a callback-driven XML parser.

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
        // `SkXMLParser::parse` never records the line of a failure (`fLineNumber` stays -1).
        super::tokenizer::parse(doc, self)
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
