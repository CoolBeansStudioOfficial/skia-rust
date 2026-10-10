// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/xml/SkDOM.h, src/xml/SkDOM.cpp

//! `SkDOM`: an immutable tree of elements and text built from an XML document.
//!
//! Skia allocates nodes from an arena and hands out `const Node*`; here the nodes live in the
//! [`Dom`] and a [`Node`] is a copyable index into it.

use super::parser::{XmlParser, XmlParserError};
use crate::scalar::scalar;
use crate::stream::Stream;
use crate::utils::parse;

/// Whether a [`Node`] is an element or a text run.
// Port of: src/xml/SkDOM.h#L45-L48 (chrome/m156)
#[doc(alias = "SkDOM::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    #[doc(alias = "kElement_Type")]
    Element,
    #[doc(alias = "kText_Type")]
    Text,
}

/// A node of a [`Dom`] (`SkDOM::Node`).
// Port of: src/xml/SkDOM.cpp#L48-L63 (chrome/m156)
#[doc(alias = "SkDOMNode")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Node(u32);

/// An attribute of a [`Node`] (`SkDOM::Attr`), by position.
// Port of: src/xml/SkDOM.cpp#L42-L45 (chrome/m156)
#[doc(alias = "SkDOMAttr")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Attr(usize);

#[derive(Debug, Clone)]
struct NodeData {
    name: String,
    first_child: Option<Node>,
    next_sibling: Option<Node>,
    attrs: Vec<(String, String)>,
    ty: NodeType,
}

impl NodeData {
    fn index(node: Node) -> usize {
        node.0 as usize
    }
}

/// The `SkXMLParser` that builds a [`Dom`] (`SkDOMParser`).
// Port of: src/xml/SkDOM.cpp#L72-L170 (chrome/m156)
#[doc(alias = "SkDOMParser")]
#[derive(Debug)]
pub struct DomParser {
    nodes: Vec<NodeData>,
    parent_stack: Vec<Node>,
    root: Option<Node>,
    need_to_flush: bool,
    // state needed for flush_attributes()
    attrs: Vec<(String, String)>,
    elem_name: String,
    elem_type: NodeType,
    level: i32,
    parser_error: XmlParserError,
}

impl Default for DomParser {
    fn default() -> Self {
        Self::new()
    }
}

impl DomParser {
    // Port of: src/xml/SkDOM.cpp#L74-L79 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            parent_stack: Vec::new(),
            root: None,
            need_to_flush: true,
            attrs: Vec::new(),
            elem_name: String::new(),
            elem_type: NodeType::Element,
            level: 0,
            parser_error: XmlParserError::new(),
        }
    }

    #[doc(alias = "getRoot")]
    #[must_use]
    pub fn root(&self) -> Option<Node> {
        self.root
    }

    /// The error state (`fParserError`).
    #[must_use]
    pub fn parser_error(&self) -> &XmlParserError {
        &self.parser_error
    }

    // Port of: src/xml/SkDOM.cpp#L84-L110 (chrome/m156)
    fn flush_attributes(&mut self) {
        debug_assert!(self.level > 0);

        let node = Node(u32::try_from(self.nodes.len()).expect("too many DOM nodes"));
        self.nodes.push(NodeData {
            name: self.elem_name.clone(),
            first_child: None,
            next_sibling: None,
            attrs: std::mem::take(&mut self.attrs),
            ty: self.elem_type,
        });

        if self.root.is_none() {
            self.root = Some(node);
        } else {
            // this adds siblings in reverse order. gets corrected in on_end_element()
            let parent = *self.parent_stack.last().expect("parent of a DOM node");
            let first = self.nodes[NodeData::index(parent)].first_child;
            self.nodes[NodeData::index(node)].next_sibling = first;
            self.nodes[NodeData::index(parent)].first_child = Some(node);
        }
        self.parent_stack.push(node);
    }

    // Port of: src/xml/SkDOM.cpp#L152-L164 (chrome/m156)
    fn start_common(&mut self, elem: &str, ty: NodeType) {
        if self.level > 0 && self.need_to_flush {
            self.flush_attributes();
        }
        self.need_to_flush = true;
        self.elem_name = elem.to_owned();
        self.elem_type = ty;
        self.level += 1;
    }
}

impl XmlParser for DomParser {
    // Port of: src/xml/SkDOM.cpp#L112-L115 (chrome/m156)
    fn on_start_element(&mut self, elem: &str) -> bool {
        self.start_common(elem, NodeType::Element);
        false
    }

    // Port of: src/xml/SkDOM.cpp#L117-L122 (chrome/m156)
    fn on_add_attribute(&mut self, name: &str, value: &str) -> bool {
        self.attrs.push((name.to_owned(), value.to_owned()));
        false
    }

    // Port of: src/xml/SkDOM.cpp#L124-L143 (chrome/m156)
    fn on_end_element(&mut self, _elem: &str) -> bool {
        if self.need_to_flush {
            self.flush_attributes();
        }
        self.need_to_flush = false;
        self.level -= 1;

        let Some(parent) = self.parent_stack.pop() else {
            return false;
        };

        let mut child = self.nodes[NodeData::index(parent)].first_child;
        let mut prev = None;
        while let Some(c) = child {
            let next = self.nodes[NodeData::index(c)].next_sibling;
            self.nodes[NodeData::index(c)].next_sibling = prev;
            prev = Some(c);
            child = next;
        }
        self.nodes[NodeData::index(parent)].first_child = prev;
        false
    }

    // Port of: src/xml/SkDOM.cpp#L145-L150 (chrome/m156)
    fn on_text(&mut self, text: &str) -> bool {
        self.start_common(text, NodeType::Text);
        let name = self.elem_name.clone();
        // this->SkDOMParser::onEndElement(fElemName)
        self.on_end_element(&name);
        false
    }

    fn error_mut(&mut self) -> Option<&mut XmlParserError> {
        Some(&mut self.parser_error)
    }
}

/// An XML document tree.
// Port of: src/xml/SkDOM.h#L26-L96 (chrome/m156)
#[doc(alias = "SkDOM")]
#[derive(Debug, Default)]
pub struct Dom {
    nodes: Vec<NodeData>,
    root: Option<Node>,
    parser: Option<DomParser>,
}

impl Dom {
    // Port of: src/xml/SkDOM.cpp#L174-L176 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn data(&self, node: Node) -> &NodeData {
        &self.nodes[NodeData::index(node)]
    }

    /// Parses the document in `doc_stream`. Returns `None` on failure, and if
    /// `error_on_line_number` is present, sets it to the line where the error occurred. On
    /// success returns the root node and sets `error_on_line_number` to -1 (no error).
    // Port of: src/xml/SkDOM.cpp#L276-L291 (chrome/m156)
    pub fn build(
        &mut self,
        doc_stream: &mut dyn Stream,
        error_on_line_number: Option<&mut i32>,
    ) -> Option<Node> {
        let mut parser = DomParser::new();
        if !parser.parse_stream(doc_stream) {
            if let Some(line) = error_on_line_number {
                *line = parser.parser_error.line_number();
            }
            self.root = None;
            self.nodes.clear();
            return None;
        }
        if let Some(line) = error_on_line_number {
            *line = -1; // success
        }
        self.root = parser.root;
        self.nodes = parser.nodes;
        self.root
    }

    /// Like [`Dom::build`], from a document in memory.
    #[must_use]
    pub fn build_from_bytes(&mut self, doc: &[u8]) -> Option<Node> {
        let mut stream = crate::stream::MemoryStream::make_copy(doc);
        self.build(&mut *stream, None)
    }

    // Port of: src/xml/SkDOM.cpp#L316-L323 (chrome/m156)
    pub fn copy(&mut self, dom: &Dom, node: Node) -> Option<Node> {
        let mut parser = DomParser::new();

        walk_dom(dom, node, &mut parser);

        self.root = parser.root;
        self.nodes = parser.nodes;
        self.root
    }

    // Port of: src/xml/SkDOM.cpp#L178-L180 (chrome/m156)
    #[doc(alias = "getRootNode")]
    #[must_use]
    pub fn root_node(&self) -> Option<Node> {
        self.root
    }

    // Port of: src/xml/SkDOM.cpp#L325-L330 (chrome/m156)
    #[doc(alias = "beginParsing")]
    pub fn begin_parsing(&mut self) -> &mut DomParser {
        debug_assert!(self.parser.is_none());
        self.parser.insert(DomParser::new())
    }

    // Port of: src/xml/SkDOM.cpp#L332-L338 (chrome/m156)
    #[doc(alias = "finishParsing")]
    pub fn finish_parsing(&mut self) -> Option<Node> {
        let parser = self.parser.take().expect("begin_parsing was called");
        self.root = parser.root;
        self.nodes = parser.nodes;
        self.root
    }

    // Port of: src/xml/SkDOM.cpp#L200-L204 (chrome/m156)
    #[doc(alias = "getType")]
    #[must_use]
    pub fn get_type(&self, node: Node) -> NodeType {
        self.data(node).ty
    }

    // Port of: src/xml/SkDOM.cpp#L206-L209 (chrome/m156)
    #[doc(alias = "getName")]
    #[must_use]
    pub fn get_name(&self, node: Node) -> &str {
        &self.data(node).name
    }

    // Port of: src/xml/SkDOM.cpp#L182-L193 (chrome/m156)
    #[doc(alias = "getFirstChild")]
    #[must_use]
    pub fn get_first_child(&self, node: Node, name: Option<&str>) -> Option<Node> {
        let mut child = self.data(node).first_child;

        if let Some(name) = name {
            while let Some(c) = child {
                if self.data(c).name == name {
                    break;
                }
                child = self.data(c).next_sibling;
            }
        }
        child
    }

    // Port of: src/xml/SkDOM.cpp#L195-L205 (chrome/m156)
    #[doc(alias = "getNextSibling")]
    #[must_use]
    pub fn get_next_sibling(&self, node: Node, name: Option<&str>) -> Option<Node> {
        let mut sibling = self.data(node).next_sibling;
        if let Some(name) = name {
            while let Some(s) = sibling {
                if self.data(s).name == name {
                    break;
                }
                sibling = self.data(s).next_sibling;
            }
        }
        sibling
    }

    // Port of: src/xml/SkDOM.cpp#L212-L224 (chrome/m156)
    #[doc(alias = "findAttr")]
    #[must_use]
    pub fn find_attr(&self, node: Node, attr_name: &str) -> Option<&str> {
        self.data(node)
            .attrs
            .iter()
            .find(|(n, _)| n == attr_name)
            .map(|(_, v)| v.as_str())
    }

    // Port of: src/xml/SkDOM.cpp#L228-L230 (chrome/m156)
    #[doc(alias = "getFirstAttr")]
    #[must_use]
    pub fn get_first_attr(&self, node: Node) -> Option<Attr> {
        (!self.data(node).attrs.is_empty()).then_some(Attr(0))
    }

    // Port of: src/xml/SkDOM.cpp#L232-L238 (chrome/m156)
    #[doc(alias = "getNextAttr")]
    #[must_use]
    pub fn get_next_attr(&self, node: Node, attr: Attr) -> Option<Attr> {
        (attr.0 + 1 < self.data(node).attrs.len()).then_some(Attr(attr.0 + 1))
    }

    // Port of: src/xml/SkDOM.cpp#L240-L244 (chrome/m156)
    #[doc(alias = "getAttrName")]
    #[must_use]
    pub fn get_attr_name(&self, node: Node, attr: Attr) -> &str {
        &self.data(node).attrs[attr.0].0
    }

    // Port of: src/xml/SkDOM.cpp#L246-L250 (chrome/m156)
    #[doc(alias = "getAttrValue")]
    #[must_use]
    pub fn get_attr_value(&self, node: Node, attr: Attr) -> &str {
        &self.data(node).attrs[attr.0].1
    }

    /// The attributes of `node` in document order (`SkDOM::AttrIter`).
    #[must_use]
    pub fn attrs(&self, node: Node) -> AttrIter<'_> {
        AttrIter::new(self, node)
    }

    // Port of: src/xml/SkDOM.cpp#L296-L307 (chrome/m156)
    #[doc(alias = "countChildren")]
    #[must_use]
    pub fn count_children(&self, node: Node, elem: Option<&str>) -> i32 {
        let mut count = 0;

        let mut node = self.get_first_child(node, elem);
        while let Some(n) = node {
            count += 1;
            node = self.get_next_sibling(n, elem);
        }
        count
    }

    // Port of: src/xml/SkDOM.cpp#L315-L318 (chrome/m156)
    #[doc(alias = "findS32")]
    #[must_use]
    pub fn find_s32(&self, node: Node, name: &str) -> Option<i32> {
        let vstr = self.find_attr(node, name)?;
        parse::find_s32(vstr.as_bytes(), 0).map(|(_, v)| v)
    }

    // Port of: src/xml/SkDOM.cpp#L320-L323 (chrome/m156)
    #[doc(alias = "findScalars")]
    #[must_use]
    pub fn find_scalars(&self, node: Node, name: &str, value: &mut [scalar]) -> bool {
        self.find_attr(node, name)
            .is_some_and(|vstr| parse::find_scalars(vstr.as_bytes(), 0, value).is_some())
    }

    #[doc(alias = "findScalar")]
    #[must_use]
    pub fn find_scalar(&self, node: Node, name: &str) -> Option<scalar> {
        let mut v = [0.0];
        self.find_scalars(node, name, &mut v).then_some(v[0])
    }

    // Port of: src/xml/SkDOM.cpp#L325-L328 (chrome/m156)
    #[doc(alias = "findHex")]
    #[must_use]
    pub fn find_hex(&self, node: Node, name: &str) -> Option<u32> {
        let vstr = self.find_attr(node, name)?;
        parse::find_hex(vstr.as_bytes(), 0).map(|(_, v)| v)
    }

    // Port of: src/xml/SkDOM.cpp#L330-L333 (chrome/m156)
    #[doc(alias = "findBool")]
    #[must_use]
    pub fn find_bool(&self, node: Node, name: &str) -> Option<bool> {
        parse::find_bool(self.find_attr(node, name)?)
    }

    // Port of: src/xml/SkDOM.cpp#L335-L338 (chrome/m156)
    #[doc(alias = "findList")]
    #[must_use]
    pub fn find_list(&self, node: Node, name: &str, list: &str) -> i32 {
        self.find_attr(node, name)
            .map_or(-1, |vstr| parse::find_list(vstr, list))
    }

    // Port of: src/xml/SkDOM.cpp#L340-L343 (chrome/m156)
    #[doc(alias = "hasAttr")]
    #[must_use]
    pub fn has_attr(&self, node: Node, name: &str, value: &str) -> bool {
        self.find_attr(node, name) == Some(value)
    }

    // Port of: src/xml/SkDOM.cpp#L345-L349 (chrome/m156)
    #[doc(alias = "hasS32")]
    #[must_use]
    pub fn has_s32(&self, node: Node, name: &str, target: i32) -> bool {
        self.find_s32(node, name) == Some(target)
    }

    // Port of: src/xml/SkDOM.cpp#L351-L355 (chrome/m156)
    #[doc(alias = "hasScalar")]
    #[must_use]
    #[allow(clippy::float_cmp)] // mirrors `value == target`
    pub fn has_scalar(&self, node: Node, name: &str, target: scalar) -> bool {
        self.find_attr(node, name)
            .and_then(|vstr| parse::find_scalar(vstr.as_bytes(), 0))
            .is_some_and(|(_, value)| value == target)
    }

    // Port of: src/xml/SkDOM.cpp#L357-L361 (chrome/m156)
    #[doc(alias = "hasHex")]
    #[must_use]
    pub fn has_hex(&self, node: Node, name: &str, target: u32) -> bool {
        self.find_hex(node, name) == Some(target)
    }

    // Port of: src/xml/SkDOM.cpp#L363-L367 (chrome/m156)
    #[doc(alias = "hasBool")]
    #[must_use]
    pub fn has_bool(&self, node: Node, name: &str, target: bool) -> bool {
        self.find_bool(node, name) == Some(target)
    }
}

/// Iterates the attributes of a node as `(name, value)`.
// Port of: src/xml/SkDOM.h#L84-L91 (chrome/m156)
#[doc(alias = "SkDOM::AttrIter")]
#[derive(Debug, Clone)]
pub struct AttrIter<'a> {
    iter: std::slice::Iter<'a, (String, String)>,
}

impl<'a> AttrIter<'a> {
    // Port of: src/xml/SkDOM.cpp#L262-L266 (chrome/m156)
    #[must_use]
    pub fn new(dom: &'a Dom, node: Node) -> Self {
        Self {
            iter: dom.data(node).attrs.iter(),
        }
    }
}

impl<'a> Iterator for AttrIter<'a> {
    type Item = (&'a str, &'a str);

    // Port of: src/xml/SkDOM.cpp#L268-L277 (chrome/m156)
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(n, v)| (n.as_str(), v.as_str()))
    }
}

// Port of: src/xml/SkDOM.cpp#L295-L314 (chrome/m156)
fn walk_dom(dom: &Dom, node: Node, parser: &mut dyn XmlParser) {
    let elem = dom.get_name(node);
    if dom.get_type(node) == NodeType::Text {
        debug_assert_eq!(dom.count_children(node, None), 0);
        parser.text(elem);
        return;
    }

    parser.start_element(elem);

    for (name, value) in dom.attrs(node) {
        parser.add_attribute(name, value);
    }

    let mut child = dom.get_first_child(node, None);
    while let Some(c) = child {
        walk_dom(dom, c, parser);
        child = dom.get_next_sibling(c, None);
    }

    parser.end_element(elem);
}
