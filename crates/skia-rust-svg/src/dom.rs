// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGDOM.h, modules/svg/src/SkSVGDOM.cpp

//! The SVG document (`SkSVGDOM`).

use std::error::Error;
use std::fmt;
use std::io;
use std::sync::Arc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::size::Size;
use skia_rust_core::stream::{MemoryStream, Stream};
use skia_rust_core::xml;

use crate::attribute::Attribute;
use crate::attribute_parser::AttributeParser;
use crate::circle::Circle;
use crate::container::{Defs, G};
use crate::ellipse::Ellipse;
use crate::id_mapper::IdMapper;
use crate::line::Line;
use crate::node::{Node, SvgNode, Tag};
use crate::path::Path;
use crate::poly::Poly;
use crate::rect::Rect;
use crate::render_context::{LengthContext, ObbScope, PresentationContext, RenderContext};
use crate::svg::{Svg, SvgType};
use crate::types::{
    Iri, Length, ObjectBoundingBoxUnits, TransformType,
};
use crate::use_::Use;
use crate::value::Value;

/// A trimmed copy of `first..=last`, as `TrimmedString` does. Bytes up to a space, and (as
/// with the signed `char` of the oracle's platform) bytes with the high bit set, are trimmed.
// Port of: modules/svg/src/SkSVGDOM.cpp#L142-L153 (chrome/m156)
fn trimmed_string(s: &[u8]) -> String {
    #[allow(clippy::cast_possible_wrap)] // `*first <= ' '` on a signed char
    let is_trimmed = |c: u8| (c as i8) <= b' ' as i8;

    let mut first = 0;
    let mut last = s.len();
    while first < last && is_trimmed(s[first]) {
        first += 1;
    }
    while first < last && is_trimmed(s[last - 1]) {
        last -= 1;
    }

    String::from_utf8_lossy(&s[first..last]).into_owned()
}

/// Breaks a "foo: bar; baz: ..." string into key:value pairs.
// Port of: modules/svg/src/SkSVGDOM.cpp#L155-L189 (chrome/m156)
struct StyleIterator<'a> {
    s: &'a [u8],
    pos: Option<usize>,
}

impl<'a> StyleIterator<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            s: s.as_bytes(),
            pos: Some(0),
        }
    }

    fn next_separator(&self, pos: usize) -> usize {
        let mut sep = pos;
        while sep < self.s.len() && self.s[sep] != b';' {
            sep += 1;
        }
        sep
    }

    fn next(&mut self) -> (String, String) {
        let mut name = String::new();
        let mut value = String::new();

        if let Some(pos) = self.pos {
            let sep = self.next_separator(pos);
            debug_assert!(sep == self.s.len() || self.s[sep] == b';');

            let value_sep = self.s[pos..].iter().position(|&c| c == b':').map(|i| pos + i);
            if let Some(value_sep) = value_sep
                && value_sep < sep
            {
                name = trimmed_string(&self.s[pos..value_sep]);
                value = trimmed_string(&self.s[value_sep + 1..sep]);
            }

            self.pos = if sep < self.s.len() { Some(sep + 1) } else { None };
        }

        (name, value)
    }
}

// Port of: modules/svg/src/SkSVGDOM.cpp#L191-L205 (chrome/m156)
fn set_style_attributes(node: &mut dyn SvgNode, string_value: &str) -> bool {
    let mut iter = StyleIterator::new(string_value);
    loop {
        let (name, value) = iter.next();
        if name.is_empty() {
            break;
        }
        set_string_attribute_on(node, &name, &value);
    }

    true
}

/// How `SetXAttribute` parses a string and hands the result to a node.
type AttrSetter = fn(&mut dyn SvgNode, Attribute, &str) -> bool;

// Port of: modules/svg/src/SkSVGDOM.cpp#L50-L58 (chrome/m156)
fn set_iri_attribute(node: &mut dyn SvgNode, attr: Attribute, string_value: &str) -> bool {
    let Some(parse_result) = AttributeParser::parse_value::<Iri>(string_value) else {
        return false;
    };

    node.set_attribute(attr, &Value::String(&parse_result.iri().to_owned()));
    true
}

// Port of: modules/svg/src/SkSVGDOM.cpp#L60-L65 (chrome/m156)
fn set_string_attribute(node: &mut dyn SvgNode, attr: Attribute, string_value: &str) -> bool {
    let str_type = string_value.to_owned();
    node.set_attribute(attr, &Value::String(&str_type));
    true
}

// Port of: modules/svg/src/SkSVGDOM.cpp#L67-L75 (chrome/m156)
fn set_transform_attribute(node: &mut dyn SvgNode, attr: Attribute, string_value: &str) -> bool {
    let Some(parse_result) = AttributeParser::parse_value::<TransformType>(string_value) else {
        return false;
    };

    node.set_attribute(attr, &Value::Transform(&parse_result));
    true
}

// Port of: modules/svg/src/SkSVGDOM.cpp#L77-L85 (chrome/m156)
fn set_length_attribute(node: &mut dyn SvgNode, attr: Attribute, string_value: &str) -> bool {
    let Some(parse_result) = AttributeParser::parse_value::<Length>(string_value) else {
        return false;
    };

    node.set_attribute(attr, &Value::Length(&parse_result));
    true
}

// Port of: modules/svg/src/SkSVGDOM.cpp#L87-L96 (chrome/m156)
fn set_view_box_attribute(node: &mut dyn SvgNode, attr: Attribute, string_value: &str) -> bool {
    let mut parser = AttributeParser::new(string_value);
    let Some(view_box) = parser.parse_view_box() else {
        return false;
    };

    node.set_attribute(attr, &Value::ViewBox(&view_box));
    true
}

// Port of: modules/svg/src/SkSVGDOM.cpp#L98-L107 (chrome/m156)
fn set_object_bounding_box_units_attribute(
    node: &mut dyn SvgNode,
    attr: Attribute,
    string_value: &str,
) -> bool {
    let Some(parse_result) = AttributeParser::parse_value::<ObjectBoundingBoxUnits>(string_value)
    else {
        return false;
    };

    node.set_attribute(attr, &Value::ObjectBoundingBoxUnits(&parse_result));
    true
}

// Port of: modules/svg/src/SkSVGDOM.cpp#L109-L120 (chrome/m156)
fn set_preserve_aspect_ratio_attribute(
    node: &mut dyn SvgNode,
    attr: Attribute,
    string_value: &str,
) -> bool {
    let mut parser = AttributeParser::new(string_value);
    let Some(par) = parser.parse_preserve_aspect_ratio() else {
        return false;
    };

    node.set_attribute(attr, &Value::PreserveAspectRatio(&par));
    true
}

fn set_style_attribute(node: &mut dyn SvgNode, _attr: Attribute, string_value: &str) -> bool {
    set_style_attributes(node, string_value)
}

/// `SortedDictionaryEntry<AttrParseInfo>`, sorted by key (it is binary searched).
// Port of: modules/svg/src/SkSVGDOM.cpp#L213-L243 (chrome/m156)
const ATTRIBUTE_PARSE_INFO: [(&str, Attribute, AttrSetter); 22] = [
    ("cx", Attribute::Cx, set_length_attribute),
    ("cy", Attribute::Cy, set_length_attribute),
    (
        "filterUnits",
        Attribute::FilterUnits,
        set_object_bounding_box_units_attribute,
    ),
    // focal point x & y
    ("fx", Attribute::Fx, set_length_attribute),
    ("fy", Attribute::Fy, set_length_attribute),
    ("height", Attribute::Height, set_length_attribute),
    (
        "preserveAspectRatio",
        Attribute::PreserveAspectRatio,
        set_preserve_aspect_ratio_attribute,
    ),
    ("r", Attribute::R, set_length_attribute),
    ("rx", Attribute::Rx, set_length_attribute),
    ("ry", Attribute::Ry, set_length_attribute),
    ("style", Attribute::Unknown, set_style_attribute),
    ("text", Attribute::Text, set_string_attribute),
    ("transform", Attribute::Transform, set_transform_attribute),
    ("viewBox", Attribute::ViewBox, set_view_box_attribute),
    ("width", Attribute::Width, set_length_attribute),
    ("x", Attribute::X, set_length_attribute),
    ("x1", Attribute::X1, set_length_attribute),
    ("x2", Attribute::X2, set_length_attribute),
    ("xlink:href", Attribute::Href, set_iri_attribute),
    ("y", Attribute::Y, set_length_attribute),
    ("y1", Attribute::Y1, set_length_attribute),
    ("y2", Attribute::Y2, set_length_attribute),
];

/// The node a tag name makes, or `None` for the elements that are not handled.
///
/// skia-rust: the elements of the paint server, clip, mask, filter, image and text modules
/// (`clipPath`, `fe*`, `filter`, `image`, `linearGradient`, `mask`, `pattern`, `radialGradient`,
/// `stop`, `text`, `textPath`, `tspan`) are ported with those modules (M14, M15) and are
/// not handled until then.
// Port of: modules/svg/src/SkSVGDOM.cpp#L245-L292 (chrome/m156)
fn make_tag_node(elem: &str) -> Option<Box<dyn SvgNode>> {
    match elem {
        "a" | "g" => Some(Box::new(G::new())),
        "circle" => Some(Box::new(Circle::new())),
        "defs" => Some(Box::new(Defs::new())),
        "ellipse" => Some(Box::new(Ellipse::new())),
        "line" => Some(Box::new(Line::new())),
        "path" => Some(Box::new(Path::new())),
        "polygon" => Some(Box::new(Poly::new_polygon())),
        "polyline" => Some(Box::new(Poly::new_polyline())),
        "rect" => Some(Box::new(Rect::new())),
        "use" => Some(Box::new(Use::new())),
        _ => None,
    }
}

// Port of: modules/svg/src/SkSVGDOM.cpp#L303-L334 (chrome/m156)
fn set_string_attribute_on(node: &mut dyn SvgNode, name: &str, value: &str) -> bool {
    if node.parse_and_set_attribute(name, value) {
        // Handled by new code path
        return true;
    }

    let Ok(attr_index) = ATTRIBUTE_PARSE_INFO.binary_search_by(|(key, _, _)| (*key).cmp(name))
    else {
        // unhandled attribute
        return false;
    };

    let (_, attr, setter) = &ATTRIBUTE_PARSE_INFO[attr_index];
    if !setter(node, *attr, value) {
        // could not parse attribute
        return false;
    }

    true
}

impl dyn SvgNode {
    /// Parses and sets an attribute given as strings.
    // Port of: modules/svg/src/SkSVGDOM.cpp#L532-L535 (chrome/m156)
    #[doc(alias = "setAttribute")]
    pub fn set_attribute_str(&mut self, attribute_name: &str, attribute_value: &str) -> bool {
        set_string_attribute_on(self, attribute_name, attribute_value)
    }
}

/// The nodes that carry an id, in document order, until they are complete.
struct ConstructionContext {
    ids: Vec<(String, Option<Node>)>,
}

// Port of: modules/svg/src/SkSVGDOM.cpp#L336-L349 (chrome/m156)
fn parse_node_attributes(
    xml_dom: &xml::Dom,
    xml_node: xml::Node,
    svg_node: &mut dyn SvgNode,
    ctx: &mut ConstructionContext,
) -> Option<usize> {
    let mut id_slot = None;
    for (name, value) in xml_dom.attrs(xml_node) {
        // We're handling id attributes out of band for now.
        if name == "id" {
            // The map is filled in document order once the nodes are complete.
            ctx.ids.push((value.to_owned(), None));
            id_slot = Some(ctx.ids.len() - 1);
            continue;
        }
        set_string_attribute_on(svg_node, name, value);
    }
    id_slot
}

// Port of: modules/svg/src/SkSVGDOM.cpp#L351-L411 (chrome/m156)
fn construct_svg_node(
    dom: &xml::Dom,
    ctx: &mut ConstructionContext,
    has_parent: bool,
    xml_node: xml::Node,
) -> Option<Node> {
    let elem = dom.get_name(xml_node);
    let elem_type = dom.get_type(xml_node);

    if elem_type == xml::NodeType::Text {
        // Text literals require special handling.
        debug_assert_eq!(dom.count_children(xml_node, None), 0);
        // skia-rust: `SkSVGTextLiteral` arrives with the text nodes (M15); until then text
        // content is dropped.
        return None;
    }

    debug_assert_eq!(elem_type, xml::NodeType::Element);

    let mut node: Box<dyn SvgNode> = if elem == "svg" {
        // Outermost SVG element must be tagged as such.
        Box::new(Svg::new(if has_parent {
            SvgType::Inner
        } else {
            SvgType::Root
        }))
    } else {
        // unhandled element
        make_tag_node(elem)?
    };

    let id_slot = parse_node_attributes(dom, xml_node, &mut *node, ctx);

    let mut child = dom.get_first_child(xml_node, None);
    while let Some(c) = child {
        if let Some(child_node) = construct_svg_node(dom, ctx, true, c) {
            node.append_child(child_node);
        }
        child = dom.get_next_sibling(c, None);
    }

    let node: Node = Arc::from(node);
    if let Some(slot) = id_slot {
        ctx.ids[slot].1 = Some(Arc::clone(&node));
    }
    Some(node)
}

/// Error when something goes wrong when loading an SVG file. Skia gives no further details.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoadError;

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to load svg (reason unknown)")
    }
}

impl Error for LoadError {}

impl From<LoadError> for io::Error {
    fn from(other: LoadError) -> Self {
        io::Error::other(other)
    }
}

/// An SVG document as a tree of nodes (`SkSVGDOM`).
///
/// skia-rust: the font manager, resource provider and text shaping factory of Skia's
/// `SkSVGDOM::Builder` arrive with the nodes that use them (images M14, text M15).
// Port of: modules/svg/include/SkSVGDOM.h#L26-L102 (chrome/m156)
#[doc(alias = "SkSVGDOM")]
#[derive(Debug)]
pub struct Dom {
    root: Arc<Svg>,
    id_mapper: IdMapper,
    container_size: Size,
}

impl Dom {
    // Port of: modules/svg/src/SkSVGDOM.cpp#L413-L443 (chrome/m156)
    /// Makes the DOM of the SVG document in `stream`.
    ///
    /// # Errors
    ///
    /// [`LoadError`] if the stream is not a well-formed SVG document.
    #[doc(alias = "MakeFromStream")]
    pub fn make_from_stream(stream: &mut dyn Stream) -> Result<Dom, LoadError> {
        let mut xml_dom = xml::Dom::new();
        let Some(xml_root) = xml_dom.build(stream, None) else {
            return Err(LoadError);
        };

        let mut ctx = ConstructionContext { ids: Vec::new() };

        let root = construct_svg_node(&xml_dom, &mut ctx, false, xml_root).ok_or(LoadError)?;
        if root.tag() != Tag::Svg {
            return Err(LoadError);
        }
        let root = root
            .into_any_arc()
            .downcast::<Svg>()
            .map_err(|_| LoadError)?;

        let mut mapper = IdMapper::new();
        for (id, node) in ctx.ids {
            if let Some(node) = node {
                mapper.set(id, node);
            }
        }

        Ok(Dom::new(root, mapper))
    }

    // Port of: modules/svg/src/SkSVGDOM.cpp#L445-L462 (chrome/m156)
    fn new(root: Arc<Svg>, mapper: IdMapper) -> Self {
        let container_size = root.intrinsic_size(&LengthContext::new(Size::new(0.0, 0.0)));
        Self {
            root,
            id_mapper: mapper,
            container_size,
        }
    }

    /// Reads an SVG document from `reader`.
    ///
    /// # Errors
    ///
    /// [`LoadError`] if reading fails or the data is not a well-formed SVG document.
    pub fn read<R: io::Read>(mut reader: R) -> Result<Self, LoadError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).map_err(|_| LoadError)?;
        Self::from_bytes(&bytes)
    }

    /// Makes the DOM of an SVG document held in a string.
    ///
    /// # Errors
    ///
    /// [`LoadError`] if the string is not a well-formed SVG document.
    #[allow(clippy::should_implement_trait)] // `skia_safe::svg::Dom::from_str` has this shape
    pub fn from_str(svg: impl AsRef<str>) -> Result<Self, LoadError> {
        Self::from_bytes(svg.as_ref().as_bytes())
    }

    /// Makes the DOM of an SVG document held in memory.
    ///
    /// # Errors
    ///
    /// [`LoadError`] if the data is not a well-formed SVG document.
    pub fn from_bytes(svg: &[u8]) -> Result<Self, LoadError> {
        let mut stream = MemoryStream::make_copy(svg);
        Self::make_from_stream(&mut *stream)
    }

    /// Returns the root (outermost) SVG element.
    #[doc(alias = "getRoot")]
    #[must_use]
    pub fn root(&self) -> &Svg {
        &self.root
    }

    /// Specifies a "container size" for the SVG dom.
    ///
    /// This is used to resolve the initial viewport when the root SVG width/height are specified
    /// in relative units.
    ///
    /// If the root dimensions are in absolute units, then the container size has no effect since
    /// the initial viewport is fixed.
    // Port of: modules/svg/src/SkSVGDOM.cpp#L506-L509 (chrome/m156)
    #[doc(alias = "setContainerSize")]
    pub fn set_container_size(&mut self, container_size: impl Into<Size>) {
        // TODO: inval
        self.container_size = container_size.into();
    }

    /// Returns the SVG dom container size.
    ///
    /// If the client specified a container size via [`set_container_size`](Self::set_container_size),
    /// then the same size is returned.
    ///
    /// When unspecified by clients, this returns the intrinsic size of the root element, as
    /// defined by its width/height attributes. If either width or height is specified in relative
    /// units (e.g. "100%"), then the corresponding intrinsic size dimension is zero.
    // Port of: modules/svg/src/SkSVGDOM.cpp#L502-L504 (chrome/m156)
    #[doc(alias = "containerSize")]
    #[must_use]
    pub fn container_size(&self) -> &Size {
        &self.container_size
    }

    /// Returns the node with the given id, or `None` if not found.
    // Port of: modules/svg/src/SkSVGDOM.cpp#L511-L514 (chrome/m156)
    #[doc(alias = "findNodeById")]
    #[must_use]
    pub fn find_node_by_id(&self, id: &str) -> Option<Node> {
        self.id_mapper.find(id)
    }

    // Port of: modules/svg/src/SkSVGDOM.cpp#L466-L482 (chrome/m156)
    pub fn render(&self, canvas: &Canvas) {
        let lctx = LengthContext::new(self.container_size);
        let pctx = PresentationContext::default();
        let ctx = RenderContext::new(canvas, &self.id_mapper, &lctx, &pctx, ObbScope::none());
        let root: &dyn SvgNode = &*self.root;
        root.render(&ctx);
    }

    /// Renders the node with the given id as if it were the only child of the root.
    // Port of: modules/svg/src/SkSVGDOM.cpp#L484-L500 (chrome/m156)
    #[doc(alias = "renderNode")]
    pub fn render_node(&self, canvas: &Canvas, pctx: &PresentationContext, id: &str) {
        let lctx = LengthContext::new(self.container_size);
        let ctx = RenderContext::new(canvas, &self.id_mapper, &lctx, pctx, ObbScope::none());
        self.root
            .render_node(&ctx, &Iri::new(crate::types::IriType::Local, id));
    }
}
