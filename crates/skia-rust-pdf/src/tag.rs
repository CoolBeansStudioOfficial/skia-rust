// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFTag.{h,cpp}, include/docs/SkPDFDocument.h#L30-L85 (chrome/m156)

//! The structure tree of a tagged PDF (`SkPDFStructTree`): the semantic tree the caller gives in
//! the metadata, the marked content and annotations that belong to its nodes, the parent tree and
//! ID tree that tie them together, and the optional outline.
//!
//! skia-rust: Skia's tree of `SkPDFStructElem` pointers in an arena is a `Vec` of elements that
//! refer to each other by index.

use std::collections::HashMap;

use skia_rust_core::point::Point;

use crate::document::DocInner;
use crate::metadata::Outline;
use crate::types::{PdfArray, PdfDict, PdfIndirectReference, PdfOptionalArray, PdfParentTreeKey};

/// `SkPDF::NodeID`: the reserved node IDs for artifacts.
// Port of: include/docs/SkPDFDocument.h#L276-L285 (chrome/m156)
pub mod node_id {
    /// `Nothing`.
    pub const NOTHING: i32 = 0;
    /// `OtherArtifact`.
    pub const OTHER_ARTIFACT: i32 = -1;
    /// `PaginationArtifact`.
    pub const PAGINATION_ARTIFACT: i32 = -2;
    /// `PaginationHeaderArtifact`.
    pub const PAGINATION_HEADER_ARTIFACT: i32 = -3;
    /// `PaginationFooterArtifact`.
    pub const PAGINATION_FOOTER_ARTIFACT: i32 = -4;
    /// `PaginationWatermarkArtifact`.
    pub const PAGINATION_WATERMARK_ARTIFACT: i32 = -5;
    /// `LayoutArtifact`.
    pub const LAYOUT_ARTIFACT: i32 = -6;
    /// `PageArtifact`.
    pub const PAGE_ARTIFACT: i32 = -7;
    /// `BackgroundArtifact`.
    pub const BACKGROUND_ARTIFACT: i32 = -8;
}

/// One attribute of an [`AttributeList`], with the PDF object it makes.
#[derive(Debug, Clone, PartialEq)]
enum AttributeValue {
    Int(i32),
    Float(f32),
    Name(String),
    TextString(Vec<u8>),
    FloatArray(Vec<f32>),
    NodeIdArray(Vec<i32>),
}

/// An attribute with its owner and name.
#[derive(Debug, Clone, PartialEq)]
struct Attribute {
    owner: String,
    name: String,
    value: AttributeValue,
}

/// `SkPDF::AttributeList`: attributes for nodes in the PDF tree.
///
/// Each attribute must have an owner (e.g. "Layout", "List", "Table", etc) and an attribute name
/// (e.g. "`BBox`", "`RowSpan`", etc.) from `PDF32000_2008` 14.8.5, and then a value of the proper type
/// according to the spec.
///
/// skia-rust: the strings are copied. The attributes are kept as data and made into PDF objects
/// when the document is created from the metadata.
// Port of: include/docs/SkPDFDocument.h#L30-L56, src/pdf/SkPDFTag.cpp#L143-L224 (chrome/m156)
#[doc(alias = "SkPDF::AttributeList")]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AttributeList {
    attrs: Vec<Attribute>,
    /// `fElemIds`: element identifiers referenced by the attributes.
    elem_ids: Vec<i32>,
}

impl AttributeList {
    /// An empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, owner: &str, name: &str, value: AttributeValue) -> &mut Self {
        self.attrs.push(Attribute {
            owner: owner.to_owned(),
            name: name.to_owned(),
            value,
        });
        self
    }

    /// `appendInt`.
    #[doc(alias = "appendInt")]
    pub fn append_int(&mut self, owner: &str, name: &str, value: i32) -> &mut Self {
        self.push(owner, name, AttributeValue::Int(value))
    }

    /// `appendFloat`.
    #[doc(alias = "appendFloat")]
    pub fn append_float(&mut self, owner: &str, name: &str, value: f32) -> &mut Self {
        self.push(owner, name, AttributeValue::Float(value))
    }

    /// `appendName`.
    #[doc(alias = "appendName")]
    pub fn append_name(&mut self, owner: &str, name: &str, value: &str) -> &mut Self {
        self.push(owner, name, AttributeValue::Name(value.to_owned()))
    }

    /// `appendTextString`.
    #[doc(alias = "appendTextString")]
    pub fn append_text_string(
        &mut self,
        owner: &str,
        name: &str,
        value: impl AsRef<[u8]>,
    ) -> &mut Self {
        self.push(
            owner,
            name,
            AttributeValue::TextString(value.as_ref().to_vec()),
        )
    }

    /// `appendFloatArray`.
    #[doc(alias = "appendFloatArray")]
    pub fn append_float_array(&mut self, owner: &str, name: &str, value: &[f32]) -> &mut Self {
        self.push(owner, name, AttributeValue::FloatArray(value.to_vec()))
    }

    /// `appendNodeIdArray`.
    // Port of: src/pdf/SkPDFTag.cpp#L208-L224 (chrome/m156)
    #[doc(alias = "appendNodeIdArray")]
    pub fn append_node_id_array(&mut self, owner: &str, name: &str, node_ids: &[i32]) -> &mut Self {
        // Keep the element identifiers so we can mark their targets as used (and needing /ID) later.
        self.elem_ids.extend_from_slice(node_ids);
        self.push(owner, name, AttributeValue::NodeIdArray(node_ids.to_vec()))
    }

    /// The attributes as the PDF array of dictionaries (`fAttrs`), or `None` if there are none.
    fn to_pdf_array(&self) -> Option<PdfArray> {
        if self.attrs.is_empty() {
            return None;
        }
        let mut attrs = PdfArray::new();
        for attr in &self.attrs {
            let mut attr_dict = PdfDict::new(None);
            attr_dict.insert_name("O", &attr.owner);
            match &attr.value {
                AttributeValue::Int(v) => attr_dict.insert_int(&attr.name, *v),
                AttributeValue::Float(v) => attr_dict.insert_scalar(&attr.name, *v),
                AttributeValue::Name(v) => attr_dict.insert_name(&attr.name, v),
                AttributeValue::TextString(v) => attr_dict.insert_text_string(&attr.name, v),
                AttributeValue::FloatArray(v) => {
                    let mut pdf_array = PdfArray::new();
                    for element in v {
                        pdf_array.append_scalar(*element);
                    }
                    attr_dict.insert_object(&attr.name, Box::new(pdf_array));
                }
                AttributeValue::NodeIdArray(v) => {
                    let mut pdf_array = PdfArray::new();
                    for elem_id in v {
                        pdf_array.append_byte_string(string_from_elem_id(*elem_id));
                    }
                    attr_dict.insert_object(&attr.name, Box::new(pdf_array));
                }
            }
            attrs.append_object(Box::new(attr_dict));
        }
        Some(attrs)
    }
}

/// `SkPDF::StructureElementNode`: a node in a PDF structure tree, giving a semantic
/// representation of the content. Each node ID is associated with content by passing the canvas
/// and node ID to [`set_node_id`](crate::document::set_node_id) when drawing. Node IDs should be
/// unique within each tree.
// Port of: include/docs/SkPDFDocument.h#L58-L65 (chrome/m156)
#[doc(alias = "SkPDF::StructureElementNode")]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StructureElementNode {
    /// `fTypeString`.
    pub type_string: String,
    /// `fChildVector`.
    pub child_vector: Vec<StructureElementNode>,
    /// `fNodeId`.
    pub node_id: i32,
    /// `fAttributes`.
    pub attributes: AttributeList,
    /// `fAlt`.
    pub alt: String,
    /// `fLang`.
    pub lang: String,
}

impl StructureElementNode {
    /// A node of the given type.
    #[must_use]
    pub fn new(type_string: impl Into<String>) -> Self {
        Self {
            type_string: type_string.into(),
            ..Self::default()
        }
    }
}

/// `SkPDFStructElem::StringFromElemId`: structure elements (/`StructElem`) may have an element
/// identifier (/ID) which is a byte string. Element identifiers are used by attributes
/// (/`StructElem` /A) to refer to structure elements. The mapping from element identifier to
/// structure element is emitted in the /`IDTree`. Element identifiers are stored as an integer
/// (elemId) and this creates a byte string. Since the /`IDTree` is a name tree the element
/// identifier keys must be ordered; the digits are zero-padded so that lexicographic order
/// matches numeric order.
// Port of: src/pdf/SkPDFTag.cpp#L52-L56 (chrome/m156)
fn string_from_elem_id(elem_id: i32) -> String {
    format!("node{elem_id:08}")
}

/// `Location`: where a mark or an element starts, the earliest page and the top left of it.
// Port of: src/pdf/SkPDFTag.cpp#L25-L47 (chrome/m156)
#[derive(Debug, Clone, Copy)]
struct Location {
    point: Point,
    page_index: u32,
}

impl Default for Location {
    fn default() -> Self {
        Self {
            point: Point::new(f32::NAN, f32::NAN),
            page_index: 0,
        }
    }
}

impl Location {
    fn accumulate(&mut self, child: &Location) {
        if !child.point.is_finite() {
            return;
        }
        if !self.point.is_finite() {
            *self = *child;
            return;
        }
        if child.page_index < self.page_index {
            *self = *child;
            return;
        }
        if child.page_index == self.page_index {
            self.point.x = child.point.x.min(self.point.x);
            self.point.y = child.point.y.max(self.point.y); // PDF y-up
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MarkedContentInfo {
    location: Location,
    mcid: i32,
    struct_parents_key: PdfParentTreeKey,
}

#[derive(Debug, Clone, Copy)]
struct ContentItemInfo {
    page_index: u32,
    struct_parent_key: PdfParentTreeKey,
}

/// `SkPDFStructElem::ContentIndex`.
// Port of: src/pdf/SkPDFTag.cpp#L129-L144 (chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
struct ContentIndex {
    parent_id: PdfParentTreeKey,
    mcid: i32,
}

impl ContentIndex {
    fn from_marked_content(mci: &MarkedContentInfo) -> Self {
        Self {
            parent_id: mci.struct_parents_key,
            mcid: mci.mcid,
        }
    }

    #[allow(clippy::trivially_copy_pass_by_ref)] // mirrors the C++ const reference
    fn from_content_item(cii: &ContentItemInfo) -> Self {
        Self {
            parent_id: cii.struct_parent_key,
            mcid: 0,
        }
    }

    #[allow(clippy::trivially_copy_pass_by_ref)] // mirrors the C++ const method
    fn valid(&self) -> bool {
        self.parent_id.is_valid()
    }
}

/// `SkPDFStructElem::ContentSpan`.
// Port of: src/pdf/SkPDFTag.cpp#L145-L185 (chrome/m156)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ContentSpan {
    data: Option<(ContentIndex, ContentIndex)>,
}

impl ContentSpan {
    fn empty(&self) -> bool {
        self.data.is_none()
    }

    fn first(&self) -> ContentIndex {
        self.data.expect("a non-empty span").0
    }

    fn last(&self) -> ContentIndex {
        self.data.expect("a non-empty span").1
    }

    #[allow(clippy::trivially_copy_pass_by_ref)] // mirrors the C++ const reference
    fn accumulate_index(&mut self, ci: &ContentIndex) {
        if !ci.valid() {
            return;
        }
        let Some((first, last)) = &mut self.data else {
            self.data = Some((*ci, *ci));
            return;
        };
        if *ci < *first {
            *first = *ci;
        }
        if *last < *ci {
            *last = *ci;
        }
    }

    fn accumulate_span(&mut self, cs: &ContentSpan) {
        if cs.empty() {
            return;
        }
        self.accumulate_index(&cs.first());
        self.accumulate_index(&cs.last());
    }
}

/// `SkPDFStructElem`: a node of the tree, as the tree built from the caller's nodes.
// Port of: src/pdf/SkPDFTag.cpp#L49-L128 (chrome/m156)
#[derive(Debug, Default)]
struct StructElem {
    parent: Option<usize>,
    children: Vec<usize>,
    marked_content: Vec<MarkedContentInfo>,
    elem_id: i32,
    want_title: bool,
    used: bool,
    used_in_id_tree: bool,
    struct_type: String,
    title: Vec<u8>,
    alt: String,
    lang: String,
    reference: PdfIndirectReference,
    attributes: Option<PdfArray>,
    attribute_elem_ids: Vec<i32>,
    content_items: Vec<ContentItemInfo>,
}

/// `SkPDFStructTree::Mark`: a marked-content sequence of a structure element.
// Port of: src/pdf/SkPDFTag.h#L32-L52 (chrome/m156)
#[doc(alias = "SkPDFStructTree::Mark")]
#[derive(Debug, Clone, Copy, Default)]
pub struct Mark {
    struct_elem: Option<usize>,
    mark_index: usize,
}

impl Mark {
    /// `explicit operator bool`.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.struct_elem.is_some()
    }

    /// `mcid`: a negative value means no active mark; if valid, always >= 0.
    // Port of: src/pdf/SkPDFTag.cpp#L275-L277 (chrome/m156)
    #[must_use]
    pub fn mcid(&self, tree: &StructTree) -> i32 {
        self.struct_elem
            .map_or(-1, |e| tree.elems[e].marked_content[self.mark_index].mcid)
    }

    /// `elemId`: 0 means no active structure element.
    // Port of: src/pdf/SkPDFTag.cpp#L263-L265 (chrome/m156)
    #[must_use]
    pub fn elem_id(&self, tree: &StructTree) -> i32 {
        self.struct_elem.map_or(0, |e| tree.elems[e].elem_id)
    }

    /// `structType`: only call when valid.
    // Port of: src/pdf/SkPDFTag.cpp#L267-L270 (chrome/m156)
    #[must_use]
    #[allow(clippy::missing_panics_doc)] // asserts the C++ preconditions
    pub fn struct_type(&self, tree: &StructTree) -> String {
        let e = self.struct_elem.expect("a valid mark");
        tree.elems[e].struct_type.clone()
    }

    /// `accumulate`: only call when valid.
    // Port of: src/pdf/SkPDFTag.cpp#L279-L283 (chrome/m156)
    #[allow(clippy::missing_panics_doc)] // asserts the C++ preconditions
    pub fn accumulate(&self, tree: &mut StructTree, point: Point) {
        let e = self.struct_elem.expect("a valid mark");
        let location = &mut tree.elems[e].marked_content[self.mark_index].location;
        let page_index = location.page_index;
        location.accumulate(&Location { point, page_index });
    }
}

/// An entry in an ordered map from an element identifier to an indirect reference to its
/// corresponding structure element (`SkPDFStructTree::IDTreeEntry`).
#[derive(Debug, Clone, Copy)]
struct IdTreeEntry {
    elem_id: i32,
    struct_elem_ref: PdfIndirectReference,
}

/// An entry of the parent tree: a content item (an annotation) or a stream of marked content.
#[derive(Debug)]
enum ParentTreeEntry {
    Item {
        struct_elem: usize,
        content_item_ref: PdfIndirectReference,
    },
    Stream {
        /// Indexed by MCID.
        children: Vec<usize>,
        /// The non-page content stream reference, `PAGE_CONTENT_STREAM_REF`, or empty.
        content_stream_ref: PdfIndirectReference,
    },
}

/// `SkPDFStructTree::kPageContentStreamRef`.
// Port of: src/pdf/SkPDFTag.h#L59 (chrome/m156)
pub const PAGE_CONTENT_STREAM_REF: PdfIndirectReference = PdfIndirectReference { value: -2 };

/// `SkPDFStructTree`.
// Port of: src/pdf/SkPDFTag.h#L22-L98 (chrome/m156)
#[doc(alias = "SkPDFStructTree")]
#[derive(Debug, Default)]
pub struct StructTree {
    elems: Vec<StructElem>,
    struct_elem_for_elem_id: HashMap<i32, usize>,
    root: Option<usize>,
    outline: Outline,
    /// Indexed by ?`::StructParent` or ?`::StructParents`.
    parent_tree: Vec<ParentTreeEntry>,
}

impl StructTree {
    /// `SkPDFStructTree(StructureElementNode*, Outline)`: builds the tree from the caller's.
    // Port of: src/pdf/SkPDFTag.cpp#L226-L270 (chrome/m156)
    #[must_use]
    pub fn new(node: Option<&StructureElementNode>, outline: Outline) -> Self {
        let mut tree = StructTree::default();
        if let Some(node) = node {
            tree.elems.push(StructElem::default());
            tree.root = Some(0);
            tree.outline = outline;
            tree.move_node(node, 0, false);
        }
        tree
    }

    // Port of: src/pdf/SkPDFTag.cpp#L232-L270 (SkPDFStructTree::move, chrome/m156)
    fn move_node(&mut self, node: &StructureElementNode, index: usize, mut want_title: bool) {
        self.elems[index].elem_id = node.node_id;
        self.struct_elem_for_elem_id.insert(node.node_id, index);

        // Accumulate title text, need to be in sync with create_outline_from_headers
        let ty = node.type_string.as_bytes();
        want_title |= self.outline == Outline::StructureElementHeaders
            && ty.len() == 2
            && ty[0] == b'H'
            && (b'1'..=b'6').contains(&ty[1]);
        self.elems[index].want_title = want_title;

        self.elems[index].struct_type = if node.type_string.is_empty() {
            "NonStruct".to_owned()
        } else {
            node.type_string.clone()
        };
        self.elems[index].alt.clone_from(&node.alt);
        self.elems[index].lang.clone_from(&node.lang);

        let mut children = Vec::with_capacity(node.child_vector.len());
        for node_child in &node.child_vector {
            let child_index = self.elems.len();
            self.elems.push(StructElem {
                parent: Some(index),
                ..StructElem::default()
            });
            children.push(child_index);
            self.move_node(node_child, child_index, want_title);
        }
        self.elems[index].children = children;

        self.elems[index].attributes = node.attributes.to_pdf_array();
        self.elems[index]
            .attribute_elem_ids
            .clone_from(&node.attributes.elem_ids);
    }

    // Port of: src/pdf/SkPDFTag.cpp#L84-L108 (SkPDFStructElem::setUsed, chrome/m156)
    fn set_used(&mut self, index: usize) {
        if self.elems[index].used {
            return;
        }
        // First to avoid possible cycles.
        self.elems[index].used = true;
        // Any StructElem referenced by an attribute is used.
        let attribute_elem_ids = self.elems[index].attribute_elem_ids.clone();
        for elem_id in attribute_elem_ids {
            let Some(&struct_elem) = self.struct_elem_for_elem_id.get(&elem_id) else {
                continue;
            };
            self.set_used(struct_elem);
            self.elems[struct_elem].used_in_id_tree = true;
        }
        // The parent StructElem is used.
        if let Some(parent) = self.elems[index].parent {
            self.set_used(parent);
        }
    }

    /// `createMarkForElemId`: creates a new marked-content identifier (MCID) to be used with a
    /// marked-content sequence parented by the structure element (`StructElem`) with the given
    /// element identifier (elemId). The `StructTreeRoot::ParentTree`[?`::StructParents`][mcid] will
    /// refer to the structure element. The structure element will add this MCID as its next
    /// child (in `StructElem::K`). Returns a false Mark if elemId does not refer to a `StructElem`.
    ///
    /// If a true Mark is returned and `struct_parents_key` is false, the Mark will be added to a
    /// new `StructParents` and `struct_parents_key` will be updated to reference the `StructParents`
    /// entry.
    // Port of: src/pdf/SkPDFTag.cpp#L285-L324 (chrome/m156)
    #[allow(clippy::missing_panics_doc)] // asserts the C++ preconditions
    pub fn create_mark_for_elem_id(
        &mut self,
        elem_id: i32,
        page_index: u32,
        struct_parents_key: &mut PdfParentTreeKey,
    ) -> Mark {
        if self.root.is_none() {
            return Mark::default();
        }
        let Some(&struct_elem) = self.struct_elem_for_elem_id.get(&elem_id) else {
            return Mark::default();
        };

        if self.parent_tree.len() <= usize::try_from(struct_parents_key.value).unwrap_or(0)
            && struct_parents_key.value >= 0
        {
            return Mark::default();
        }
        if !struct_parents_key.is_valid() {
            struct_parents_key.value = i32::try_from(self.parent_tree.len()).expect("fits");
            self.parent_tree.push(ParentTreeEntry::Stream {
                children: Vec::new(),
                content_stream_ref: PdfIndirectReference::default(),
            });
        }
        let entry_index = usize::try_from(struct_parents_key.value).expect("a valid key");
        if !matches!(
            self.parent_tree[entry_index],
            ParentTreeEntry::Stream { .. }
        ) {
            return Mark::default();
        }

        self.set_used(struct_elem);

        let ParentTreeEntry::Stream { children, .. } = &mut self.parent_tree[entry_index] else {
            unreachable!("checked above");
        };
        let mcid = i32::try_from(children.len()).expect("fits");
        debug_assert!(
            self.elems[struct_elem].marked_content.is_empty()
                || self.elems[struct_elem]
                    .marked_content
                    .last()
                    .is_some_and(|m| m.location.page_index <= page_index)
        );
        self.elems[struct_elem]
            .marked_content
            .push(MarkedContentInfo {
                location: Location {
                    point: Point::new(f32::NAN, f32::NAN),
                    page_index,
                },
                mcid,
                struct_parents_key: *struct_parents_key,
            });
        children.push(struct_elem);
        Mark {
            struct_elem: Some(struct_elem),
            mark_index: self.elems[struct_elem].marked_content.len() - 1,
        }
    }

    /// `setContentStreamRefForStructParentsKey`.
    // Port of: src/pdf/SkPDFTag.cpp#L326-L338 (chrome/m156)
    pub fn set_content_stream_ref_for_struct_parents_key(
        &mut self,
        struct_parents_key: PdfParentTreeKey,
        content_stream_ref: PdfIndirectReference,
    ) {
        let Ok(index) = usize::try_from(struct_parents_key.value) else {
            return;
        };
        let Some(entry) = self.parent_tree.get_mut(index) else {
            return;
        };
        if let ParentTreeEntry::Stream {
            content_stream_ref: slot,
            ..
        } = entry
        {
            *slot = content_stream_ref;
        }
    }

    /// `getContentStreamRefForStructParentsKey`.
    // Port of: src/pdf/SkPDFTag.cpp#L340-L353 (chrome/m156)
    #[must_use]
    pub fn content_stream_ref_for_struct_parents_key(
        &self,
        struct_parents_key: PdfParentTreeKey,
    ) -> PdfIndirectReference {
        let Ok(index) = usize::try_from(struct_parents_key.value) else {
            return PdfIndirectReference::default();
        };
        match self.parent_tree.get(index) {
            Some(ParentTreeEntry::Stream {
                content_stream_ref, ..
            }) => *content_stream_ref,
            _ => PdfIndirectReference::default(),
        }
    }

    /// `createStructParentKeyForElemId`: creates a key to use with /`StructParent` in a content
    /// item (usually an annotation) which refers to the structure element (`StructElem`) with the
    /// given element identifier (elemId). The `StructTreeRoot` `ParentTree` will map from this key
    /// to the structure element. The structure element will add the content item as its next
    /// child (as `StructElem::K::OBJR`). Returns a false key if elemId does not refer to a
    /// `StructElem`.
    // Port of: src/pdf/SkPDFTag.cpp#L355-L376 (chrome/m156)
    #[allow(clippy::missing_panics_doc)] // asserts the C++ preconditions
    pub fn create_struct_parent_key_for_elem_id(
        &mut self,
        elem_id: i32,
        page_index: u32,
        content_item_ref: PdfIndirectReference,
    ) -> PdfParentTreeKey {
        if self.root.is_none() {
            return PdfParentTreeKey::default();
        }
        let Some(&struct_elem) = self.struct_elem_for_elem_id.get(&elem_id) else {
            return PdfParentTreeKey::default();
        };

        self.set_used(struct_elem);

        let struct_parent_key = PdfParentTreeKey {
            value: i32::try_from(self.parent_tree.len()).expect("fits"),
        };
        self.elems[struct_elem].content_items.push(ContentItemInfo {
            page_index,
            struct_parent_key,
        });
        self.parent_tree.push(ParentTreeEntry::Item {
            struct_elem,
            content_item_ref,
        });
        struct_parent_key
    }

    /// `getContentItemRefForStructParentKey`.
    // Port of: src/pdf/SkPDFTag.cpp#L378-L391 (chrome/m156)
    #[must_use]
    pub fn content_item_ref_for_struct_parent_key(
        &self,
        struct_parent_key: PdfParentTreeKey,
    ) -> PdfIndirectReference {
        let Ok(index) = usize::try_from(struct_parent_key.value) else {
            return PdfIndirectReference::default();
        };
        match self.parent_tree.get(index) {
            Some(ParentTreeEntry::Item {
                content_item_ref, ..
            }) => *content_item_ref,
            _ => PdfIndirectReference::default(),
        }
    }

    /// `addStructElemTitle`.
    // Port of: src/pdf/SkPDFTag.cpp#L609-L627 (chrome/m156)
    pub fn add_struct_elem_title(&mut self, elem_id: i32, title: &[u8]) {
        if self.root.is_none() {
            return;
        }
        let Some(&struct_elem) = self.struct_elem_for_elem_id.get(&elem_id) else {
            return;
        };
        let elem = &mut self.elems[struct_elem];
        if elem.want_title {
            elem.title.extend_from_slice(title);
            // Arbitrary cutoff for size.
            if elem.title.len() > 1023 {
                elem.want_title = false;
            }
        }
    }

    /// `getRootLanguage`.
    // Port of: src/pdf/SkPDFTag.cpp#L976-L978 (chrome/m156)
    #[must_use]
    pub fn root_language(&self) -> String {
        self.root
            .map_or_else(String::new, |r| self.elems[r].lang.clone())
    }

    /// `emitStructElem`.
    // Port of: src/pdf/SkPDFTag.cpp#L393-L605 (chrome/m156)
    #[allow(clippy::too_many_lines)] // one function in Skia
    fn emit_struct_elem(
        &mut self,
        index: usize,
        parent: PdfIndirectReference,
        id_tree: &mut Vec<IdTreeEntry>,
        doc: &mut DocInner,
        content_span: &mut ContentSpan,
    ) -> PdfIndirectReference {
        let reference = doc.reserve_ref();
        self.elems[index].reference = reference;

        let mut dict = PdfDict::new(Some("StructElem"));
        dict.insert_name_escaped("S", &self.elems[index].struct_type);

        if !self.elems[index].alt.is_empty() {
            dict.insert_text_string("Alt", &self.elems[index].alt);
        }
        if !self.elems[index].lang.is_empty() {
            dict.insert_text_string("Lang", &self.elems[index].lang);
        }
        dict.insert_ref("P", parent);

        {
            // K
            // Need to emit the kids in order. There are three kinds of kids:
            //   1. children (structure elements, in user order, have marked content and content
            //      items)
            //   2. marked content (drawing, sort by {struct parent key, marked content id})
            //   3. content items (currently just annotations, {struct parent key, 0})
            // The children must be emitted in the order specified by the user. The marked content
            // and content items must be emitted in the order they were drawn. If all the kid
            // content is well ordered (no child span overlapping with anything else) and that
            // order matches the user specified order of children then there is a "good" order.
            // But any form of overlap is possible so there may not be a "good" order. In other
            // words, the structure tree is an ordered hierarchy but the user can draw items and
            // associate them with structure tree entries in any order. If the content isn't
            // hierarchical it won't fit well into the structure tree. So try to find a
            // least-bad order.
            //
            // The strategy used here is:
            // 1. Merge all overlapping child spans to order the children.
            // 2. Emit the each next child, marked content, or content item.
            //    Empty children are emitted first then compare by ContentIndex.

            // Emit the children, collect their spans, then adjust the spans
            struct ChildSpan {
                content_span: ContentSpan,
                reference: PdfIndirectReference,
            }
            let mut child_spans: Vec<ChildSpan> = Vec::new();
            let children = self.elems[index].children.clone();
            for child in children {
                if self.elems[child].used {
                    let mut child_span = ChildSpan {
                        content_span: ContentSpan::default(),
                        reference: PdfIndirectReference::default(),
                    };
                    child_span.reference = self.emit_struct_elem(
                        child,
                        reference,
                        id_tree,
                        doc,
                        &mut child_span.content_span,
                    );
                    child_spans.push(child_span);
                }
            }
            if child_spans.len() > 1 {
                let mut min_first_after: Option<ContentIndex> = None;
                for child_span in child_spans.iter_mut().rev() {
                    if child_span.content_span.empty() {
                        // Let empty child spans remain empty
                        continue;
                    }
                    if min_first_after.is_none_or(|m| child_span.content_span.first() <= m) {
                        // This child span starts before all subsequent child spans, everything
                        // is fine.
                        min_first_after = Some(child_span.content_span.first());
                        continue;
                    }
                    // This is a non-empty span which currently starts after a subsequent child
                    // span.
                    child_span
                        .content_span
                        .accumulate_index(&min_first_after.expect("checked above"));
                }
            }
            #[cfg(debug_assertions)]
            {
                // Postcondition: spans are empty or start after the all previous spans.
                let mut max_first_seen_so_far: Option<ContentIndex> = None;
                for child_span in &child_spans {
                    if child_span.content_span.empty() {
                        continue;
                    }
                    let max = *max_first_seen_so_far.get_or_insert(child_span.content_span.first());
                    debug_assert!(max <= child_span.content_span.first());
                    max_first_seen_so_far = Some(max.max(child_span.content_span.first()));
                }
            }

            // Setup the marked content
            let mut longest_page: u32 = 0;
            if !self.elems[index].marked_content.is_empty() {
                // Use the mode page as /Pg and use integer mcid for marks on that page.
                // SkPDFStructElem::fMarkedContent is already sorted by page, since it is append
                // only in createMarkForElemId where pageIndex is the monotonically increasing
                // current page.
                let mut longest_run: usize = 0;
                let mut current_run: usize = 0;
                let mut current_page: u32 = 0;
                for info in &self.elems[index].marked_content {
                    let this_page = info.location.page_index;
                    if current_page != this_page {
                        debug_assert!(current_page < this_page);
                        current_page = this_page;
                        current_run = 0;
                    }
                    current_run += 1;
                    if longest_run < current_run {
                        longest_run = current_run;
                        longest_page = current_page;
                    }
                }
                dict.insert_ref("Pg", doc.get_page(longest_page as usize));
            }

            let mut kids = PdfOptionalArray::default();
            let marked_content = self.elems[index].marked_content.clone();
            let content_items = self.elems[index].content_items.clone();
            let mut marked_content_iter = 0usize;
            let mut content_item_iter = 0usize;
            let mut child_span_iter = 0usize;
            while marked_content_iter != marked_content.len()
                || content_item_iter != content_items.len()
                || child_span_iter != child_spans.len()
            {
                let mci = marked_content
                    .get(marked_content_iter)
                    .map_or_else(ContentIndex::default, ContentIndex::from_marked_content);
                let cii = content_items
                    .get(content_item_iter)
                    .map_or_else(ContentIndex::default, ContentIndex::from_content_item);

                if let Some(child_span) = child_spans.get(child_span_iter)
                    && (child_span.content_span.empty()
                        || ((!mci.valid() || child_span.content_span.first() <= mci)
                            && (!cii.valid() || child_span.content_span.first() <= cii)))
                {
                    kids.array_mut().append_ref(child_span.reference);
                    content_span.accumulate_span(&child_span.content_span);
                    child_span_iter += 1;
                    continue;
                }

                if mci.valid() && (!cii.valid() || mci <= cii) {
                    let info = &marked_content[marked_content_iter];
                    let content_stream_ref =
                        self.content_stream_ref_for_struct_parents_key(info.struct_parents_key);
                    if info.location.page_index == longest_page
                        && content_stream_ref == PAGE_CONTENT_STREAM_REF
                    {
                        kids.array_mut().append_int(info.mcid);
                        content_span.accumulate_index(&ContentIndex::from_marked_content(info));
                    } else if content_stream_ref.is_valid()
                        || content_stream_ref == PAGE_CONTENT_STREAM_REF
                    {
                        let mut mcr = PdfDict::new(Some("MCR"));
                        if info.location.page_index != longest_page {
                            mcr.insert_ref("Pg", doc.get_page(info.location.page_index as usize));
                        }
                        if content_stream_ref.is_valid() {
                            mcr.insert_ref("Stm", content_stream_ref);
                        }
                        mcr.insert_int("MCID", info.mcid);
                        kids.array_mut().append_object(Box::new(mcr));
                        content_span.accumulate_index(&ContentIndex::from_marked_content(info));
                    }

                    marked_content_iter += 1;
                    continue;
                }

                if cii.valid() && (!mci.valid() || cii <= mci) {
                    let info = &content_items[content_item_iter];
                    let content_item_ref =
                        self.content_item_ref_for_struct_parent_key(info.struct_parent_key);
                    let mut content_item_dict = PdfDict::new(Some("OBJR"));
                    content_item_dict.insert_ref("Obj", content_item_ref);
                    content_item_dict.insert_ref("Pg", doc.get_page(info.page_index as usize));
                    kids.array_mut().append_object(Box::new(content_item_dict));
                    content_span.accumulate_index(&ContentIndex::from_content_item(info));

                    content_item_iter += 1;
                    continue;
                }

                debug_assert!(false);
                break;
            }
            dict.insert_object("K", Box::new(kids));
        }

        if let Some(attributes) = self.elems[index].attributes.take() {
            dict.insert_object("A", Box::new(attributes));
        }

        // If this StructElem ID was referenced, add /ID and add it to the IDTree.
        if self.elems[index].used_in_id_tree {
            dict.insert_byte_string("ID", string_from_elem_id(self.elems[index].elem_id));
            id_tree.push(IdTreeEntry {
                elem_id: self.elems[index].elem_id,
                struct_elem_ref: reference,
            });
        }

        doc.emit(&dict, reference)
    }

    /// `emitStructTreeRoot`: the reference of the /`StructTreeRoot`, or none if nothing is used.
    // Port of: src/pdf/SkPDFTag.cpp#L629-L735 (chrome/m156)
    pub(crate) fn emit_struct_tree_root(&mut self, doc: &mut DocInner) -> PdfIndirectReference {
        let Some(root) = self.root else {
            return PdfIndirectReference::default();
        };
        if !self.elems[root].used {
            return PdfIndirectReference::default();
        }

        let struct_tree_root_ref = doc.reserve_ref();

        // Build the StructTreeRoot.
        let mut struct_tree_root = PdfDict::new(Some("StructTreeRoot"));
        let mut id_tree: Vec<IdTreeEntry> = Vec::new();
        let mut root_content_span = ContentSpan::default();
        let k = self.emit_struct_elem(
            root,
            struct_tree_root_ref,
            &mut id_tree,
            doc,
            &mut root_content_span,
        );
        struct_tree_root.insert_ref("K", k);
        struct_tree_root.insert_int(
            "ParentTreeNextKey",
            i32::try_from(self.parent_tree.len()).expect("fits"),
        );

        // Build the parent tree, a number tree which consists of two things:
        // For each Page or FormXObject with marked content:
        //   key: ?::StructParents
        //   value: array of structure element ref indexed by the page's marked-content identifiers
        // For each content item (usually an annotation)
        //   key: ?::StructParent
        //   value: structure element ref
        let mut parent_tree = PdfDict::new(Some("ParentTree"));
        let mut parent_tree_nums = PdfArray::new();

        for (struct_parent_key, entry) in self.parent_tree.iter().enumerate() {
            let key = i32::try_from(struct_parent_key).expect("fits");
            match entry {
                ParentTreeEntry::Item { struct_elem, .. } => {
                    parent_tree_nums.append_int(key); // /StructParent
                    parent_tree_nums.append_ref(self.elems[*struct_elem].reference);
                }
                ParentTreeEntry::Stream {
                    children,
                    content_stream_ref,
                } => {
                    if content_stream_ref.is_valid()
                        || *content_stream_ref == PAGE_CONTENT_STREAM_REF
                    {
                        let mut struct_elem_for_mcid_array = PdfArray::new();
                        for struct_elem in children {
                            debug_assert!(self.elems[*struct_elem].reference.is_valid());
                            struct_elem_for_mcid_array
                                .append_ref(self.elems[*struct_elem].reference);
                        }
                        parent_tree_nums.append_int(key); // /StructParents
                        parent_tree_nums.append_ref(doc.emit_new(&struct_elem_for_mcid_array));
                    }
                }
            }
        }

        parent_tree.insert_object("Nums", Box::new(parent_tree_nums));
        let parent_tree_ref = doc.emit_new(&parent_tree);
        struct_tree_root.insert_ref("ParentTree", parent_tree_ref);

        // Build the IDTree, a mapping from every unique element identifier byte string to
        // a reference to its corresponding structure element.
        if !id_tree.is_empty() {
            id_tree.sort_by_key(|e| e.elem_id);

            let mut id_tree_leaf = PdfDict::new(None);
            let mut limits = PdfArray::new();
            limits.append_byte_string(string_from_elem_id(id_tree[0].elem_id));
            limits.append_byte_string(string_from_elem_id(id_tree[id_tree.len() - 1].elem_id));
            id_tree_leaf.insert_object("Limits", Box::new(limits));
            let mut names = PdfArray::new();
            for entry in &id_tree {
                names.append_byte_string(string_from_elem_id(entry.elem_id));
                names.append_ref(entry.struct_elem_ref);
            }
            id_tree_leaf.insert_object("Names", Box::new(names));
            let mut id_tree_kids = PdfArray::new();
            id_tree_kids.append_ref(doc.emit_new(&id_tree_leaf));

            let mut id_tree_root = PdfDict::new(None);
            id_tree_root.insert_object("Kids", Box::new(id_tree_kids));
            let id_tree_ref = doc.emit_new(&id_tree_root);
            struct_tree_root.insert_ref("IDTree", id_tree_ref);
        }

        doc.emit(&struct_tree_root, struct_tree_root_ref)
    }

    /// `makeOutline`: the reference of the /Outlines, or none if there is no outline.
    // Port of: src/pdf/SkPDFTag.cpp#L938-L974 (chrome/m156)
    pub(crate) fn make_outline(&self, doc: &mut DocInner) -> PdfIndirectReference {
        let Some(root) = self.root else {
            return PdfIndirectReference::default();
        };
        if !self.elems[root].used || self.outline == Outline::None {
            return PdfIndirectReference::default();
        }

        let outline_ref;
        let mut outline = PdfDict::new(Some("Outlines"));
        if self.outline == Outline::StructureElements {
            outline_ref = doc.reserve_ref();
            let entry_ref = doc.reserve_ref();
            let none = PdfIndirectReference::default();
            let entry = self.structelem_outline_emit(doc, root, outline_ref, none, entry_ref, none);
            outline.insert_ref("First", entry_ref);
            outline.insert_ref("Last", entry_ref);
            outline.insert_int(
                "Count",
                i32::try_from(entry.descendant_count).expect("fits"),
            );
        } else {
            let mut top = HeaderEntry {
                content: HeaderContent::default(),
                header_level: 0,
                structure_ref: PdfIndirectReference::default(),
                reference: PdfIndirectReference::default(),
                children: Vec::new(),
                descendents_emitted: 0,
            };
            let mut stack: Vec<Vec<usize>> = vec![Vec::new()];
            self.header_outline_make(root, &mut top, &mut stack);
            if top.children.is_empty() {
                return PdfIndirectReference::default();
            }
            outline_ref = doc.reserve_ref();
            top.set_all_refs(doc, outline_ref);
            top.emit_descendents(doc);
            outline.insert_ref("First", top.children[0].reference);
            outline.insert_ref("Last", top.children[top.children.len() - 1].reference);
            outline.insert_int(
                "Count",
                i32::try_from(top.descendents_emitted).expect("fits"),
            );
        }

        doc.emit(&outline, outline_ref)
    }

    // Port of: src/pdf/SkPDFTag.cpp#L798-L824 (header_outline::create_header_content,
    // chrome/m156)
    fn create_header_content(&self, struct_elem: usize) -> HeaderContent {
        let elem = &self.elems[struct_elem];
        let text: Vec<u8> = if !elem.title.is_empty() {
            elem.title.clone()
        } else if !elem.alt.is_empty() {
            elem.alt.as_bytes().to_vec()
        } else {
            Vec::new()
        };

        // The uppermost/leftmost point on the earliest page of this StructElem's marks.
        let mut struct_elem_location = Location::default();
        for mark in &elem.marked_content {
            struct_elem_location.accumulate(&mark.location);
        }

        let mut content = HeaderContent {
            text,
            location: struct_elem_location,
        };

        // Accumulate children
        for &child in &elem.children {
            if self.elems[child].used {
                let child_content = self.create_header_content(child);
                content.accumulate(&child_content);
            }
        }
        content
    }

    // Port of: src/pdf/SkPDFTag.cpp#L826-L850 (header_outline::make, chrome/m156)
    fn header_outline_make(
        &self,
        struct_elem: usize,
        top: &mut HeaderEntry,
        stack: &mut Vec<Vec<usize>>,
    ) {
        let ty = self.elems[struct_elem].struct_type.as_bytes();
        if ty.len() == 2 && ty[0] == b'H' && (b'1'..=b'6').contains(&ty[1]) {
            let level = i32::from(ty[1] - b'0');
            while level <= top.entry_at(stack.last().expect("a stack")).header_level {
                stack.pop();
            }
            let content = self.create_header_content(struct_elem);
            if !content.text.is_empty() {
                let e = HeaderEntry {
                    content,
                    header_level: level,
                    structure_ref: self.elems[struct_elem].reference,
                    reference: PdfIndirectReference::default(),
                    children: Vec::new(),
                    descendents_emitted: 0,
                };
                let mut path = stack.last().expect("a stack").clone();
                let parent = top.entry_at_mut(&path);
                parent.children.push(e);
                path.push(parent.children.len() - 1);
                stack.push(path);
                return;
            }
        }

        for &child in &self.elems[struct_elem].children {
            if self.elems[child].used {
                self.header_outline_make(child, top, stack);
            }
        }
    }

    // Port of: src/pdf/SkPDFTag.cpp#L861-L936 (structelem_outline::emit, chrome/m156)
    fn structelem_outline_emit(
        &self,
        doc: &mut DocInner,
        struct_elem: usize,
        parent_ref: PdfIndirectReference,
        prev_sibling_ref: PdfIndirectReference,
        self_ref: PdfIndirectReference,
        next_sibling_ref: PdfIndirectReference,
    ) -> StructElemOutlineEntry {
        let mut self_entry = StructElemOutlineEntry::default();

        // Emit any child entries.
        let mut child_refs: Vec<PdfIndirectReference> = Vec::new();
        for &child in &self.elems[struct_elem].children {
            if !self.elems[child].used {
                continue;
            }
            child_refs.push(doc.reserve_ref());
        }
        let mut child_refs_index = 0usize;
        let mut prev_child_ref = PdfIndirectReference::default(); // Starts out as "none".
        child_refs.push(PdfIndirectReference::default()); // Put an extra "none" on the end for the last "next".
        for &child in &self.elems[struct_elem].children {
            if !self.elems[child].used {
                continue;
            }
            let curr_child_ref = child_refs[child_refs_index];
            let next_child_ref = child_refs[child_refs_index + 1];
            let child_entry = self.structelem_outline_emit(
                doc,
                child,
                self_ref,
                prev_child_ref,
                curr_child_ref,
                next_child_ref,
            );
            self_entry.accumulate(&child_entry);
            prev_child_ref = curr_child_ref;
            child_refs_index += 1;
        }
        child_refs.pop(); // Remove the "none" on the end.

        // Emit self entry.
        let elem = &self.elems[struct_elem];
        let mut entry = PdfDict::new(None);
        if !elem.title.is_empty() {
            entry.insert_text_string("Title", &elem.title);
        } else if !elem.alt.is_empty() {
            entry.insert_text_string("Title", &elem.alt);
        } else {
            entry.insert_text_string("Title", &elem.struct_type);
        }

        // The uppermost/leftmost point on the earliest page of this structure element's marks.
        let mut struct_elem_location = Location::default();
        for mark in &elem.marked_content {
            struct_elem_location.accumulate(&mark.location);
        }
        if struct_elem_location.point.is_finite() {
            let mut destination = PdfArray::new();
            destination.append_ref(doc.get_page(struct_elem_location.page_index as usize));
            destination.append_name("XYZ");
            destination.append_scalar(struct_elem_location.point.x);
            destination.append_scalar(struct_elem_location.point.y);
            destination.append_int(0);
            entry.insert_object("Dest", Box::new(destination));

            self_entry.location.accumulate(&struct_elem_location);
        } else if self_entry.location.point.is_finite() {
            // The uppermost/leftmost point on the earliest page of any child.
            let mut destination = PdfArray::new();
            destination.append_ref(doc.get_page(self_entry.location.page_index as usize));
            destination.append_name("XYZ");
            destination.append_scalar(self_entry.location.point.x);
            destination.append_scalar(self_entry.location.point.y);
            destination.append_int(0);
            entry.insert_object("Dest", Box::new(destination));
        }
        if elem.reference.is_valid() {
            entry.insert_ref("SE", elem.reference);
        }
        entry.insert_ref("Parent", parent_ref);
        if prev_sibling_ref.is_valid() {
            entry.insert_ref("Prev", prev_sibling_ref);
        }
        if next_sibling_ref.is_valid() {
            entry.insert_ref("Next", next_sibling_ref);
        }
        if !child_refs.is_empty() {
            entry.insert_ref("First", child_refs[0]);
            entry.insert_ref("Last", child_refs[child_refs.len() - 1]);
            entry.insert_int(
                "Count",
                i32::try_from(self_entry.descendant_count).expect("fits"),
            );
        }
        doc.emit(&entry, self_ref);
        self_entry.descendant_count += 1;
        self_entry
    }
}

/// `structelem_outline::Entry`.
#[derive(Debug, Default)]
struct StructElemOutlineEntry {
    descendant_count: usize,
    location: Location,
}

impl StructElemOutlineEntry {
    fn accumulate(&mut self, child: &StructElemOutlineEntry) {
        self.descendant_count += child.descendant_count;
        self.location.accumulate(&child.location);
    }
}

/// `header_outline::Entry::Content`.
#[derive(Debug, Default)]
struct HeaderContent {
    text: Vec<u8>,
    location: Location,
}

impl HeaderContent {
    fn accumulate(&mut self, child: &HeaderContent) {
        self.text.extend_from_slice(&child.text);
        self.location.accumulate(&child.location);
    }
}

/// `header_outline::Entry`.
#[derive(Debug)]
struct HeaderEntry {
    content: HeaderContent,
    header_level: i32,
    structure_ref: PdfIndirectReference,
    reference: PdfIndirectReference,
    children: Vec<HeaderEntry>,
    descendents_emitted: usize,
}

impl HeaderEntry {
    /// The entry at `path` (child indices from this one).
    fn entry_at(&self, path: &[usize]) -> &HeaderEntry {
        let mut entry = self;
        for &i in path {
            entry = &entry.children[i];
        }
        entry
    }

    fn entry_at_mut(&mut self, path: &[usize]) -> &mut HeaderEntry {
        let mut entry = self;
        for &i in path {
            entry = &mut entry.children[i];
        }
        entry
    }

    // Port of: src/pdf/SkPDFTag.cpp#L762-L768 (chrome/m156)
    fn set_all_refs(&mut self, doc: &mut DocInner, reference: PdfIndirectReference) {
        self.reference = reference;
        for child in &mut self.children {
            let child_ref = doc.reserve_ref();
            child.set_all_refs(doc, child_ref);
        }
    }

    // Port of: src/pdf/SkPDFTag.cpp#L770-L802 (chrome/m156)
    fn emit_descendents(&mut self, doc: &mut DocInner) {
        self.descendents_emitted = self.children.len();
        for i in 0..self.children.len() {
            self.children[i].emit_descendents(doc);
            self.descendents_emitted += self.children[i].descendents_emitted;

            let child = &self.children[i];
            let mut entry = PdfDict::new(None);
            entry.insert_text_string("Title", &child.content.text);

            let mut destination = PdfArray::new();
            destination.append_ref(doc.get_page(child.content.location.page_index as usize));
            destination.append_name("XYZ");
            destination.append_scalar(child.content.location.point.x);
            destination.append_scalar(child.content.location.point.y);
            destination.append_int(0);
            entry.insert_object("Dest", Box::new(destination));

            entry.insert_ref("Parent", self.reference);
            if child.structure_ref.is_valid() {
                entry.insert_ref("SE", child.structure_ref);
            }
            if 0 < i {
                entry.insert_ref("Prev", self.children[i - 1].reference);
            }
            if i + 1 < self.children.len() {
                entry.insert_ref("Next", self.children[i + 1].reference);
            }
            if !child.children.is_empty() {
                entry.insert_ref("First", child.children[0].reference);
                entry.insert_ref("Last", child.children[child.children.len() - 1].reference);
                entry.insert_int(
                    "Count",
                    i32::try_from(child.descendents_emitted).expect("fits"),
                );
            }
            doc.emit(&entry, child.reference);
        }
    }
}
