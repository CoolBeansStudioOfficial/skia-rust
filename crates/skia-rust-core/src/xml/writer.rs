// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/xml/SkXMLWriter.h, src/xml/SkXMLWriter.cpp

//! `SkXMLWriter`: writes XML to a stream, or replays it into an [`XmlParser`].

use super::dom::{Dom, Node, NodeType};
use super::parser::XmlParser;
use crate::scalar::scalar;
use crate::stream::WStream;
use crate::string::{str_append_hex, str_append_s32, str_append_scalar};

/// One open element (`SkXMLWriter::Elem`).
// Port of: src/xml/SkXMLWriter.h#L44-L52 (chrome/m156)
#[derive(Debug, Clone)]
pub struct Elem {
    pub name: String,
    pub has_children: bool,
    pub has_text: bool,
}

/// The state `SkXMLWriter` keeps for its subclasses.
// Port of: src/xml/SkXMLWriter.h#L19-L60 (chrome/m156)
#[derive(Debug, Clone)]
pub struct XmlWriterBase {
    elems: Vec<Elem>,
    do_escape_markup: bool,
}

impl Default for XmlWriterBase {
    fn default() -> Self {
        Self::new(true)
    }
}

impl XmlWriterBase {
    // Port of: src/xml/SkXMLWriter.cpp#L13-L14 (chrome/m156)
    #[must_use]
    pub fn new(do_escape_markup: bool) -> Self {
        Self {
            elems: Vec::new(),
            do_escape_markup,
        }
    }

    /// The open elements, outermost first (`fElems`).
    #[must_use]
    pub fn elems(&self) -> &[Elem] {
        &self.elems
    }

    // Port of: src/xml/SkXMLWriter.cpp#L63-L72 (chrome/m156)
    #[doc(alias = "doStart")]
    pub fn do_start(&mut self, name: &str) -> bool {
        let level = self.elems.len();
        let first_child = level > 0 && !self.elems[level - 1].has_children;
        if first_child {
            self.elems[level - 1].has_children = true;
        }
        self.elems.push(Elem {
            name: name.to_owned(),
            has_children: false,
            has_text: false,
        });
        first_child
    }

    // Port of: src/xml/SkXMLWriter.cpp#L74-L78 (chrome/m156)
    #[doc(alias = "getEnd")]
    pub fn get_end(&mut self) -> Elem {
        self.elems.pop().expect("an open element")
    }

    // Port of: src/xml/SkXMLWriter.cpp#L59-L61 (chrome/m156)
    #[doc(alias = "doEnd")]
    pub fn do_end(&mut self, _elem: Elem) {}
}

// Port of: src/xml/SkXMLWriter.cpp#L86-L119 (chrome/m156)
fn escape_char(c: char, storage: &mut String) -> &str {
    storage.clear();
    match c {
        '<' => "&lt;",
        '>' => "&gt;",
        //"\"&quot;",
        //"'&apos;",
        '&' => "&amp;",
        _ => {
            storage.push(c);
            storage
        }
    }
}

fn escape_markup(src: &str) -> String {
    let mut dst = String::with_capacity(src.len());
    let mut storage = String::new();
    for c in src.chars() {
        dst.push_str(escape_char(c, &mut storage));
    }
    dst
}

// Port of: src/xml/SkXMLWriter.cpp#L152-L170 (chrome/m156)
fn write_dom_node<W: XmlWriter + ?Sized>(dom: &Dom, node: Node, w: &mut W, skip_root: bool) {
    let mut node = node;
    if !skip_root {
        let elem = dom.get_name(node);
        if dom.get_type(node) == NodeType::Text {
            debug_assert_eq!(dom.count_children(node, None), 0);
            w.add_text(elem);
            return;
        }

        w.start_element(elem);

        for (name, value) in dom.attrs(node) {
            w.add_attribute(name, value);
        }
    }

    let mut child = dom.get_first_child(node, None);
    while let Some(c) = child {
        write_dom_node(dom, c, w, false);
        node = c;
        child = dom.get_next_sibling(node, None);
    }

    if !skip_root {
        w.end_element();
    }
}

/// Writes XML through four callbacks.
// Port of: src/xml/SkXMLWriter.h#L19-L60 (chrome/m156)
#[doc(alias = "SkXMLWriter")]
pub trait XmlWriter {
    fn base(&self) -> &XmlWriterBase;
    fn base_mut(&mut self) -> &mut XmlWriterBase;

    #[doc(alias = "onStartElementLen")]
    fn on_start_element_len(&mut self, elem: &str);
    #[doc(alias = "onAddAttributeLen")]
    fn on_add_attribute_len(&mut self, name: &str, value: &str);
    #[doc(alias = "onAddText")]
    fn on_add_text(&mut self, text: &str);
    #[doc(alias = "onEndElement")]
    fn on_end_element(&mut self);

    // Port of: src/xml/SkXMLWriter.cpp#L150-L151 (chrome/m156)
    #[doc(alias = "writeHeader")]
    fn write_header(&mut self) {}

    // Port of: src/xml/SkXMLWriter.cpp#L20-L24 (chrome/m156)
    fn flush(&mut self) {
        while !self.base().elems.is_empty() {
            self.end_element();
        }
    }

    // Port of: src/xml/SkXMLWriter.cpp#L26-L28 (chrome/m156)
    #[doc(alias = "addAttribute")]
    #[doc(alias = "addAttributeLen")]
    fn add_attribute(&mut self, name: &str, value: &str) {
        let escaped;
        let mut value = value;
        if self.base().do_escape_markup {
            let e = escape_markup(value);
            if e.len() != value.len() {
                escaped = e;
                value = &escaped;
            }
        }
        self.on_add_attribute_len(name, value);
    }

    // Port of: src/xml/SkXMLWriter.cpp#L30-L34 (chrome/m156)
    #[doc(alias = "addS32Attribute")]
    fn add_s32_attribute(&mut self, name: &str, value: i32) {
        let mut tmp = String::new();
        str_append_s32(&mut tmp, value);
        self.add_attribute(name, &tmp);
    }

    // Port of: src/xml/SkXMLWriter.cpp#L36-L40 (chrome/m156)
    #[doc(alias = "addHexAttribute")]
    fn add_hex_attribute(&mut self, name: &str, value: u32, min_digits: i32) {
        let mut tmp = String::from("0x");
        str_append_hex(&mut tmp, value, min_digits);
        self.add_attribute(name, &tmp);
    }

    // Port of: src/xml/SkXMLWriter.cpp#L42-L46 (chrome/m156)
    #[doc(alias = "addScalarAttribute")]
    fn add_scalar_attribute(&mut self, name: &str, value: scalar) {
        let mut tmp = String::new();
        str_append_scalar(&mut tmp, value);
        self.add_attribute(name, &tmp);
    }

    // Port of: src/xml/SkXMLWriter.cpp#L48-L57 (chrome/m156)
    #[doc(alias = "addText")]
    fn add_text(&mut self, text: &str) {
        if self.base().elems.is_empty() {
            return;
        }

        self.on_add_text(text);

        if let Some(back) = self.base_mut().elems.last_mut() {
            back.has_text = true;
        }
    }

    #[doc(alias = "endElement")]
    fn end_element(&mut self) {
        self.on_end_element();
    }

    // Port of: src/xml/SkXMLWriter.cpp#L121-L123 (chrome/m156)
    #[doc(alias = "startElement")]
    #[doc(alias = "startElementLen")]
    fn start_element(&mut self, elem: &str) {
        self.on_start_element_len(elem);
    }

    // Port of: src/xml/SkXMLWriter.cpp#L172-L176 (chrome/m156)
    #[doc(alias = "writeDOM")]
    fn write_dom(&mut self, dom: &Dom, node: Option<Node>, skip_root: bool) {
        if let Some(node) = node {
            write_dom_node(dom, node, self, skip_root);
        }
    }
}

/// `SkXMLWriter::getHeader`.
// Port of: src/xml/SkXMLWriter.cpp#L80-L84 (chrome/m156)
fn get_header() -> &'static str {
    "<?xml version=\"1.0\" encoding=\"utf-8\" ?>"
}

/// Flags of [`XmlStreamWriter`].
pub const K_NO_PRETTY_FLAG: u32 = 0x01;

/// Writes XML text to a [`WStream`].
// Port of: src/xml/SkXMLWriter.h#L62-L85 (chrome/m156)
#[doc(alias = "SkXMLStreamWriter")]
pub struct XmlStreamWriter<'a> {
    base: XmlWriterBase,
    stream: &'a mut dyn WStream,
    flags: u32,
}

impl std::fmt::Debug for XmlStreamWriter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XmlStreamWriter")
            .field("base", &self.base)
            .field("flags", &self.flags)
            .finish_non_exhaustive()
    }
}

impl<'a> XmlStreamWriter<'a> {
    /// `flags` is `0` or [`K_NO_PRETTY_FLAG`] (`kNoPretty_Flag`).
    // Port of: src/xml/SkXMLWriter.cpp#L178-L181 (chrome/m156)
    #[must_use]
    pub fn new(stream: &'a mut dyn WStream, flags: u32) -> Self {
        Self {
            base: XmlWriterBase::new(true),
            stream,
            flags,
        }
    }

    // Port of: src/xml/SkXMLWriter.cpp#L248-L257 (chrome/m156)
    fn newline(&mut self) {
        if self.flags & K_NO_PRETTY_FLAG == 0 {
            self.stream.newline();
        }
    }

    fn tab(&mut self, level: usize) {
        if self.flags & K_NO_PRETTY_FLAG == 0 {
            for _ in 0..level {
                self.stream.write_text("\t");
            }
        }
    }
}

impl Drop for XmlStreamWriter<'_> {
    // Port of: src/xml/SkXMLWriter.cpp#L183-L185 (chrome/m156)
    fn drop(&mut self) {
        XmlWriter::flush(self);
    }
}

impl XmlWriter for XmlStreamWriter<'_> {
    fn base(&self) -> &XmlWriterBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut XmlWriterBase {
        &mut self.base
    }

    // Port of: src/xml/SkXMLWriter.cpp#L187-L193 (chrome/m156)
    fn on_add_attribute_len(&mut self, name: &str, value: &str) {
        debug_assert!(
            self.base
                .elems
                .last()
                .is_some_and(|e| !e.has_children && !e.has_text)
        );
        self.stream.write_text(" ");
        self.stream.write_text(name);
        self.stream.write_text("=\"");
        self.stream.write(value.as_bytes());
        self.stream.write_text("\"");
    }

    // Port of: src/xml/SkXMLWriter.cpp#L195-L205 (chrome/m156)
    fn on_add_text(&mut self, text: &str) {
        let (has_children, has_text) = {
            let elem = self.base.elems.last().expect("an open element");
            (elem.has_children, elem.has_text)
        };

        if !has_children && !has_text {
            self.stream.write_text(">");
            self.newline();
        }

        let level = self.base.elems.len() + 1;
        self.tab(level);
        self.stream.write(text.as_bytes());
        self.newline();
    }

    // Port of: src/xml/SkXMLWriter.cpp#L207-L219 (chrome/m156)
    fn on_end_element(&mut self) {
        let elem = self.base.get_end();
        if elem.has_children || elem.has_text {
            let level = self.base.elems.len();
            self.tab(level);
            self.stream.write_text("</");
            self.stream.write_text(&elem.name);
            self.stream.write_text(">");
        } else {
            self.stream.write_text("/>");
        }
        self.newline();
        self.base.do_end(elem);
    }

    // Port of: src/xml/SkXMLWriter.cpp#L221-L233 (chrome/m156)
    fn on_start_element_len(&mut self, name: &str) {
        let level = self.base.elems.len();
        if self.base.do_start(name) {
            // the first child, need to close with >
            self.stream.write_text(">");
            self.newline();
        }

        self.tab(level);
        self.stream.write_text("<");
        self.stream.write(name.as_bytes());
    }

    // Port of: src/xml/SkXMLWriter.cpp#L235-L239 (chrome/m156)
    fn write_header(&mut self) {
        let header = get_header();
        self.stream.write(header.as_bytes());
        self.newline();
    }
}

/// Replays what is written into an [`XmlParser`].
// Port of: src/xml/SkXMLWriter.h#L87-L100 (chrome/m156)
#[doc(alias = "SkXMLParserWriter")]
pub struct XmlParserWriter<'a> {
    base: XmlWriterBase,
    parser: &'a mut dyn XmlParser,
}

impl std::fmt::Debug for XmlParserWriter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XmlParserWriter")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

impl<'a> XmlParserWriter<'a> {
    // Port of: src/xml/SkXMLWriter.cpp#L262-L265 (chrome/m156)
    #[must_use]
    pub fn new(parser: &'a mut dyn XmlParser) -> Self {
        Self {
            base: XmlWriterBase::new(false),
            parser,
        }
    }
}

impl Drop for XmlParserWriter<'_> {
    // Port of: src/xml/SkXMLWriter.cpp#L267-L269 (chrome/m156)
    fn drop(&mut self) {
        XmlWriter::flush(self);
    }
}

impl XmlWriter for XmlParserWriter<'_> {
    fn base(&self) -> &XmlWriterBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut XmlWriterBase {
        &mut self.base
    }

    // Port of: src/xml/SkXMLWriter.cpp#L271-L275 (chrome/m156)
    fn on_add_attribute_len(&mut self, name: &str, value: &str) {
        debug_assert!(
            self.base
                .elems
                .last()
                .is_none_or(|e| !e.has_children && !e.has_text)
        );
        self.parser.add_attribute(name, value);
    }

    // Port of: src/xml/SkXMLWriter.cpp#L277-L279 (chrome/m156)
    fn on_add_text(&mut self, text: &str) {
        self.parser.text(text);
    }

    // Port of: src/xml/SkXMLWriter.cpp#L281-L285 (chrome/m156)
    fn on_end_element(&mut self) {
        let elem = self.base.get_end();
        self.parser.end_element(&elem.name);
        self.base.do_end(elem);
    }

    // Port of: src/xml/SkXMLWriter.cpp#L287-L291 (chrome/m156)
    fn on_start_element_len(&mut self, name: &str) {
        let _ = self.base.do_start(name);
        self.parser.start_element(name);
    }
}
