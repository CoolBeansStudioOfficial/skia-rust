// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFDocument.cpp, src/pdf/SkPDFDocumentPriv.h (SkPDFDocument, SkPDFOffsetMap,
// SkPDF::MakeDocument), src/core/SkDocument.cpp and the object-stream helpers of
// src/pdf/SkPDFTypes.cpp (chrome/m156)

//! The PDF document: the header, the indirect objects with their cross-reference offsets, the
//! pages drawn through [`Document::begin_page`], the page tree, the catalog and the trailer.
//!
//! `SkDocument` (the page state machine) and `SkPDFDocument` are one type here. The pages are
//! drawn by the PDF device ([`PdfDevice`](crate::device::PdfDevice)); the document is shared
//! with every device it makes (a layer, a pattern cell, a soft mask) through a [`DocHandle`], so
//! the canonicalized objects (graphic states, images, shaders) are written once per document.
//!
//! skia-rust: the objects are written to a buffer in the shared state, and the document passes
//! the buffer to the stream it was made with at the end of each page, at `close` and at `abort`.
//! A device must be `'static`, so it cannot borrow the stream. The bytes, and the offsets in the
//! cross-reference table, are those of Skia. Not ported: the executor, so that streams are
//! deflated serially and the output is reproducible; the font subsets (`modules.md` M26), which
//! `close` emits before the trailer.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::data::Data;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::size::{ISize, Size};
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_core::utf::count_utf8;

use crate::bitmap::IccProfileKey;
use crate::deflate::DeflateWStream;
use crate::device::{ContentHandle, PdfDevice};
use crate::graphic_state::{FillGraphicState, StrokeGraphicState};
use crate::gradient_shader::GradientKey;
use crate::jpeg;
use crate::keyed_image::BitmapKey;
use crate::metadata::{
    CompressionLevel, Metadata, Uuid, create_uuid, make_document_information_dict, make_pdf_id,
    xmp_packet,
};
use crate::shader::ImageShaderKey;
use crate::srgb_icc::SRGB_PROFILE;
use crate::tag::{Mark, StructTree};
use crate::types::{
    PdfArray, PdfDict, PdfIndirectReference, PdfObject, PdfParentTreeKey,
};
use crate::utils::rect_to_array;

/// `SK_ScalarDefaultRasterDPI`.
pub(crate) const DEFAULT_RASTER_DPI: f32 = 72.0;

/// `SkPDFGetElemIdKey`: the annotation key that sets the node ID of the following drawing.
// Port of: src/pdf/SkPDFDocument.cpp#L53-L57 (chrome/m156)
#[doc(alias = "SkPDFGetElemIdKey")]
#[must_use]
pub fn elem_id_key() -> &'static str {
    "PDF_Node_Key"
}

/// `SkPDF::SetNodeId`: associates a node ID with subsequent drawing commands in `canvas`. The
/// same node ID can appear in a `StructureElementNode` in order to associate a document's
/// structure element tree with its content. A node ID of zero indicates no node ID. Negative
/// node IDs are reserved.
// Port of: src/pdf/SkPDFDocument.cpp#L705-L709 (chrome/m156)
#[doc(alias = "SkPDF::SetNodeId")]
pub fn set_node_id(canvas: &Canvas, node_id: i32) {
    let payload = Data::new_copy(&node_id.to_ne_bytes());
    canvas.draw_annotation(Rect::new(0.0, 0.0, 0.0, 0.0), elem_id_key(), Some(&payload));
}

/// `SkPDFOffsetMap`: the byte offset of every indirect object, for the cross-reference table.
// Port of: src/pdf/SkPDFDocumentPriv.h#L38-L52, src/pdf/SkPDFDocument.cpp#L83-L117 (chrome/m156)
#[doc(alias = "SkPDFOffsetMap")]
#[derive(Debug, Default)]
pub struct PdfOffsetMap {
    /// `fOffsets[i]` is the offset of object `i + 1`.
    offsets: Vec<i32>,
    /// The position of the `%PDF` header, relative to which the offsets are taken.
    base_offset: Option<usize>,
}

impl PdfOffsetMap {
    /// `markStartOfDocument`: offsets are relative to `position`, the stream's position.
    // Port of: src/pdf/SkPDFDocument.cpp#L83 (chrome/m156)
    pub fn mark_start_of_document(&mut self, position: usize) {
        self.base_offset = Some(position);
    }

    /// `markStartOfObject`: records the offset of object `reference_number`, which starts at
    /// `position`.
    // Port of: src/pdf/SkPDFDocument.cpp#L89-L96 (chrome/m156)
    pub fn mark_start_of_object(&mut self, reference_number: i32, position: usize) {
        debug_assert!(reference_number > 0);
        let index = usize::try_from(reference_number - 1).unwrap_or(0);
        if index >= self.offsets.len() {
            self.offsets.resize(index + 1, 0);
        }
        self.offsets[index] = i32::try_from(self.difference(position)).unwrap_or(i32::MAX);
    }

    /// `objectCount`: the number of entries, including the free zeroth object.
    // Port of: src/pdf/SkPDFDocument.cpp#L98-L101 (chrome/m156)
    #[must_use]
    pub fn object_count(&self) -> i32 {
        // Include the special zeroth object in the count.
        i32::try_from(self.offsets.len() + 1).unwrap_or(i32::MAX)
    }

    /// `emitCrossReferenceTable`: writes the `xref` table to `s`, which is at `position`, and
    /// returns the offset of the table.
    // Port of: src/pdf/SkPDFDocument.cpp#L102-L117 (chrome/m156)
    pub fn emit_cross_reference_table(&self, s: &mut dyn WStream, position: usize) -> i32 {
        let x_ref_file_offset = i32::try_from(self.difference(position)).unwrap_or(i32::MAX);
        s.write_text("xref\n0 ");
        s.write_dec_as_text(self.object_count());
        s.write_text("\n0000000000 65535 f \n");
        for &offset in &self.offsets {
            debug_assert!(offset > 0); // Offset was set.
            s.write_big_dec_as_text(i64::from(offset), 10);
            s.write_text(" 00000 n \n");
        }
        x_ref_file_offset
    }

    /// `difference`: the position relative to the start of the document.
    // Port of: src/pdf/SkPDFDocument.cpp#L85-L87 (chrome/m156)
    fn difference(&self, position: usize) -> usize {
        let base = self.base_offset.unwrap_or(0);
        debug_assert!(position >= base);
        position.saturating_sub(base)
    }
}

/// `SkPDFNamedDestination`.
// Port of: src/pdf/SkPDFDocumentPriv.h#L55-L59 (chrome/m156)
#[doc(alias = "SkPDFNamedDestination")]
#[derive(Debug, Clone)]
pub struct PdfNamedDestination {
    /// `fName`.
    pub name: Data,
    /// `fPoint`.
    pub point: Point,
    /// `fPage`.
    pub page: PdfIndirectReference,
}

/// `SkPDFLink::Type`.
// Port of: src/pdf/SkPDFDocumentPriv.h#L63-L67 (chrome/m156)
#[doc(alias = "SkPDFLink::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkType {
    /// `kNone`.
    None,
    /// `kUrl`.
    Url,
    /// `kNamedDestination`.
    NamedDestination,
}

/// `SkPDFLink`.
// Port of: src/pdf/SkPDFDocumentPriv.h#L62-L80 (chrome/m156)
#[doc(alias = "SkPDFLink")]
#[derive(Debug, Clone)]
pub struct PdfLink {
    /// `fType`.
    pub link_type: LinkType,
    /// `fData`: the url or named destination, depending on the type.
    pub data: Data,
    /// `fRect`.
    pub rect: Rect,
    /// `fElemId`.
    pub elem_id: i32,
}

/// `ToValidUtf8String`: the C string in `d`, if it is a valid UTF-8 string with its terminator.
// Port of: src/pdf/SkPDFDocument.cpp#L59-L79 (chrome/m156)
fn to_valid_utf8_string(d: &Data) -> Vec<u8> {
    if d.is_empty() {
        debug_assert!(false, "Not a valid string, data length is zero.");
        return Vec::new();
    }

    let bytes = d.as_bytes();
    if bytes[bytes.len() - 1] != 0 {
        debug_assert!(false, "Not a valid string, not null-terminated.");
        return Vec::new();
    }

    // CountUTF8 returns -1 if there's an invalid UTF-8 byte sequence.
    let valid_utf8_chars_count = count_utf8(&bytes[..bytes.len() - 1]);
    if valid_utf8_chars_count == -1 {
        debug_assert!(false, "Not a valid UTF-8 string.");
        return Vec::new();
    }

    bytes[..bytes.len() - 1].to_vec()
}

/// `serializeHeader`: `%PDF-1.4` and the binary comment that marks the file as binary.
// Port of: src/pdf/SkPDFDocument.cpp#L124-L131 (chrome/m156)
fn serialize_header(offset_map: &mut PdfOffsetMap, w_stream: &mut DynamicMemoryWStream, flushed: usize) {
    offset_map.mark_start_of_document(flushed + w_stream.bytes_written());
    w_stream.write_text("%PDF-1.4\n%");
    // The PDF spec recommends a comment with four bytes, all with their high bits set. "\xD3\xEB
    // \xE9\xE1" is "Skia" with the high bits set.
    w_stream.write(&[0xD3, 0xEB, 0xE9, 0xE1]);
    w_stream.write_text("\n");
}

/// `serialize_footer`: the cross-reference table and the trailer.
// Port of: src/pdf/SkPDFDocument.cpp#L144-L164 (chrome/m156)
fn serialize_footer(
    offset_map: &PdfOffsetMap,
    w_stream: &mut DynamicMemoryWStream,
    flushed: usize,
    info_dict: PdfIndirectReference,
    doc_catalog: PdfIndirectReference,
    uuid: Uuid,
) {
    let position = flushed + w_stream.bytes_written();
    let x_ref_file_offset = offset_map.emit_cross_reference_table(w_stream, position);
    let mut trailer_dict = PdfDict::new(None);
    trailer_dict.insert_int("Size", offset_map.object_count());
    debug_assert!(doc_catalog.is_valid());
    trailer_dict.insert_ref("Root", doc_catalog);
    debug_assert!(info_dict.is_valid());
    trailer_dict.insert_ref("Info", info_dict);
    if uuid != Uuid::default() {
        trailer_dict.insert_object("ID", Box::new(make_pdf_id(uuid, uuid)));
    }
    w_stream.write_text("trailer\n");
    trailer_dict.emit_object(w_stream);
    w_stream.write_text("\nstartxref\n");
    w_stream.write_big_dec_as_text(i64::from(x_ref_file_offset), 0);
    w_stream.write_text("\n%%EOF\n");
}

/// A node of the page tree, with the reference it is written under.
struct PageTreeNode {
    node: PdfDict,
    reserved_ref: PdfIndirectReference,
    page_object_descendant_count: i32,
}

/// The number of children of an internal node of the page tree.
const K_MAX_NODE_SIZE: usize = 8;

impl PageTreeNode {
    /// Builds one layer of the tree from the layer below it.
    // Port of: src/pdf/SkPDFDocument.cpp#L166-L225 (`PageTreeNode::Layer`, chrome/m156)
    fn layer(vec: Vec<PageTreeNode>, doc: &mut DocInner) -> Vec<PageTreeNode> {
        let n = vec.len();
        debug_assert!(!vec.is_empty());
        let result_len = (n - 1) / K_MAX_NODE_SIZE + 1;
        debug_assert!(result_len >= 1);
        debug_assert!(n == 1 || result_len < n);
        let mut result = Vec::with_capacity(result_len);
        let mut nodes = vec.into_iter();
        let mut index = 0;
        for _ in 0..result_len {
            if n != 1 && index + 1 == n {
                // No need to create a new node.
                result.push(nodes.next().expect("index < n"));
                index += 1;
                continue;
            }
            let parent = doc.reserve_ref();
            let mut kids_list = PdfArray::new();
            let mut descendant_count: i32 = 0;
            let mut j = 0;
            while j < K_MAX_NODE_SIZE && index < n {
                let mut node = nodes.next().expect("index < n");
                index += 1;
                node.node.insert_ref("Parent", parent);
                kids_list.append_ref(doc.emit(&node.node, node.reserved_ref));
                descendant_count += node.page_object_descendant_count;
                j += 1;
            }
            let mut next = PdfDict::new(Some("Pages"));
            next.insert_int("Count", descendant_count);
            next.insert_object("Kids", Box::new(kids_list));
            result.push(PageTreeNode {
                node: next,
                reserved_ref: parent,
                page_object_descendant_count: descendant_count,
            });
        }
        result
    }
}

/// `generate_page_tree`: a tree describing all the pages, with at most 8 children per node. The
/// tree is built bottom up, and internal nodes that would have only one child are skipped.
// Port of: src/pdf/SkPDFDocument.cpp#L166-L229 (chrome/m156)
fn generate_page_tree(
    doc: &mut DocInner,
    pages: Vec<PdfDict>,
    page_refs: &[PdfIndirectReference],
) -> PdfIndirectReference {
    debug_assert!(!pages.is_empty());
    debug_assert_eq!(pages.len(), page_refs.len());
    let mut current_layer: Vec<PageTreeNode> = pages
        .into_iter()
        .zip(page_refs.iter().copied())
        .map(|(node, reserved_ref)| PageTreeNode {
            node,
            reserved_ref,
            page_object_descendant_count: 1,
        })
        .collect();
    current_layer = PageTreeNode::layer(current_layer, doc);
    while current_layer.len() > 1 {
        current_layer = PageTreeNode::layer(current_layer, doc);
    }
    debug_assert_eq!(current_layer.len(), 1);
    let root = current_layer.pop().expect("one root");
    doc.emit(&root.node, root.reserved_ref)
}

/// `make_srgb_color_profile`: the sRGB ICC profile as a stream.
// Port of: src/pdf/SkPDFDocument.cpp#L527-L533 (chrome/m156)
fn make_srgb_color_profile(doc: &mut DocInner) -> PdfIndirectReference {
    let mut dict = PdfDict::new(None);
    dict.insert_int("N", 3);
    let mut range = PdfArray::new();
    for v in [0, 1, 0, 1, 0, 1] {
        range.append_int(v);
    }
    dict.insert_object("Range", Box::new(range));
    doc.stream_out(Some(dict), SRGB_PROFILE, true)
}

/// `make_srgb_output_intents`: the sRGB output intent of a PDF/A document.
// Port of: src/pdf/SkPDFDocument.cpp#L535-L546 (chrome/m156)
fn make_srgb_output_intents(doc: &mut DocInner) -> PdfArray {
    // sRGB is specified by HTML, CSS, and SVG.
    let mut output_intent = PdfDict::new(Some("OutputIntent"));
    output_intent.insert_name("S", "GTS_PDFA1");
    output_intent.insert_text_string("RegistryName", "http://www.color.org");
    output_intent.insert_text_string("OutputConditionIdentifier", "Custom");
    output_intent.insert_text_string("Info", "sRGB IEC61966-2.1");
    let profile = make_srgb_color_profile(doc);
    output_intent.insert_ref("DestOutputProfile", profile);
    let mut intent_array = PdfArray::new();
    intent_array.append_object(Box::new(output_intent));
    intent_array
}

// Port of: src/pdf/SkPDFDocument.cpp#L289-L299 (populate_link_annotation, chrome/m156)
fn populate_link_annotation(annotation: &mut PdfDict, r: &Rect) {
    annotation.insert_name("Subtype", "Link");
    annotation.insert_int("F", 4); // required by ISO 19005
    // Border: 0 = Horizontal corner radius.
    //         0 = Vertical corner radius.
    //         0 = Width, 0 = no border.
    annotation.insert_object("Border", Box::new(crate::utils::make_int_array(&[0, 0, 0])));
    annotation.insert_object(
        "Rect",
        Box::new(crate::utils::make_scalar_array(&[
            r.left, r.top, r.right, r.bottom,
        ])),
    );
}

/// The state of a document shared by the document and its devices (the data members of
/// `SkPDFDocument`, without the stream and the canvas).
// Port of: src/pdf/SkPDFDocumentPriv.h#L84-L231 (chrome/m156)
pub(crate) struct DocInner {
    /// The objects written so far and not yet passed to the document's stream.
    pub(crate) out: DynamicMemoryWStream,
    /// The bytes that were passed to the stream already.
    flushed: usize,
    /// Whether writing is over (`getStream()` is null after `close` or `abort`).
    finished: bool,
    offset_map: PdfOffsetMap,
    pub(crate) metadata: Rc<Metadata>,
    /// The document ID (and instance ID), for PDF/A documents only; zero otherwise.
    uuid: Uuid,
    info_dict: PdfIndirectReference,
    xmp: PdfIndirectReference,
    /// The next free object number (`fNextObjectNumber`).
    next_object_number: i32,
    /// The page dictionaries, in order (`fPages`).
    pages: Vec<PdfDict>,
    /// The reserved reference of each page (`fPageRefs`).
    page_refs: Vec<PdfIndirectReference>,
    /// Whether there is a page device (`hasCurrentPage`).
    has_current_page: bool,
    /// The initial transform of the page device (`currentPageTransform`).
    current_page_transform: Matrix,
    /// The raster scale and its inverse.
    raster_scale: f32,
    inverse_raster_scale: f32,

    // Canonicalized objects.
    pub(crate) image_shader_map: HashMap<ImageShaderKey, PdfIndirectReference>,
    pub(crate) gradient_pattern_map: HashMap<GradientKey, PdfIndirectReference>,
    pub(crate) pdf_bitmap_map: HashMap<BitmapKey, PdfIndirectReference>,
    pub(crate) icc_profile_map: HashMap<IccProfileKey, PdfIndirectReference>,
    pub(crate) stroke_gs_map: HashMap<StrokeGraphicState, PdfIndirectReference>,
    pub(crate) fill_gs_map: HashMap<FillGraphicState, PdfIndirectReference>,
    pub(crate) invert_function: PdfIndirectReference,
    pub(crate) no_smask_graphic_state: PdfIndirectReference,
    pub(crate) current_page_links: Vec<PdfLink>,
    pub(crate) named_destinations: Vec<PdfNamedDestination>,

    /// For tagged PDFs (`fStructTree`).
    pub(crate) struct_tree: StructTree,
}

impl std::fmt::Debug for DocInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocInner")
            .field("next_object_number", &self.next_object_number)
            .field("pages", &self.pages.len())
            .finish_non_exhaustive()
    }
}

impl DocInner {
    fn new(metadata: Metadata) -> Self {
        let raster_scale = metadata.raster_dpi / DEFAULT_RASTER_DPI;
        let inverse_raster_scale = DEFAULT_RASTER_DPI / metadata.raster_dpi;
        let struct_tree = StructTree::new(
            metadata.structure_element_tree_root.as_ref(),
            metadata.outline,
        );
        Self {
            out: DynamicMemoryWStream::new(),
            flushed: 0,
            finished: false,
            offset_map: PdfOffsetMap::default(),
            metadata: Rc::new(metadata),
            uuid: Uuid::default(),
            info_dict: PdfIndirectReference::default(),
            xmp: PdfIndirectReference::default(),
            next_object_number: 1,
            pages: Vec::new(),
            page_refs: Vec::new(),
            has_current_page: false,
            current_page_transform: Matrix::new_identity(),
            raster_scale,
            inverse_raster_scale,
            image_shader_map: HashMap::new(),
            gradient_pattern_map: HashMap::new(),
            pdf_bitmap_map: HashMap::new(),
            icc_profile_map: HashMap::new(),
            stroke_gs_map: HashMap::new(),
            fill_gs_map: HashMap::new(),
            invert_function: PdfIndirectReference::default(),
            no_smask_graphic_state: PdfIndirectReference::default(),
            current_page_links: Vec::new(),
            named_destinations: Vec::new(),
            struct_tree,
        }
    }

    /// The position in the document of the next byte written.
    fn position(&self) -> usize {
        self.flushed + self.out.bytes_written()
    }

    /// `reserveRef`: a fresh object number. Every reference returned must be passed to `emit`
    /// exactly once.
    // Port of: src/pdf/SkPDFDocumentPriv.h#L174 (chrome/m156)
    pub(crate) fn reserve_ref(&mut self) -> PdfIndirectReference {
        let value = self.next_object_number;
        self.next_object_number += 1;
        PdfIndirectReference { value }
    }

    /// `beginObject`: `N 0 obj`, and the offset of it.
    // Port of: src/pdf/SkPDFDocument.cpp#L133-L139 (begin_indirect_object, chrome/m156)
    fn begin_object(&mut self, reference: PdfIndirectReference) {
        let position = self.position();
        self.offset_map
            .mark_start_of_object(reference.value, position);
        self.out.write_dec_as_text(reference.value);
        self.out.write_text(" 0 obj\n"); // Generation number is always 0.
    }

    /// `endObject`.
    // Port of: src/pdf/SkPDFDocument.cpp#L141 (end_indirect_object, chrome/m156)
    fn end_object(&mut self) {
        self.out.write_text("\nendobj\n");
    }

    /// `emit`: writes `object` as the indirect object `reference`, and returns the reference.
    // Port of: src/pdf/SkPDFDocument.cpp#L252-L257 (chrome/m156)
    pub(crate) fn emit(
        &mut self,
        object: &dyn PdfObject,
        reference: PdfIndirectReference,
    ) -> PdfIndirectReference {
        if self.finished {
            return reference;
        }
        self.begin_object(reference);
        object.emit_object(&mut self.out);
        self.end_object();
        reference
    }

    /// `emit(object)`: reserves a reference and writes the object under it.
    pub(crate) fn emit_new(&mut self, object: &dyn PdfObject) -> PdfIndirectReference {
        let reference = self.reserve_ref();
        self.emit(object, reference)
    }

    /// `emitStream`: writes a stream object, its dictionary, `stream`, and `endstream`.
    // Port of: src/pdf/SkPDFDocumentPriv.h#L144-L153 (chrome/m156)
    pub(crate) fn emit_stream(
        &mut self,
        dict: &PdfDict,
        content: &[u8],
        reference: PdfIndirectReference,
    ) {
        if self.finished {
            return;
        }
        self.begin_object(reference);
        dict.emit_object(&mut self.out);
        self.out.write_text(" stream\n");
        self.out.write(content);
        self.out.write_text("\nendstream");
        self.end_object();
    }

    /// `SkPDFStreamOut`: writes `content` as a stream object (deflated when the metadata asks
    /// for compression and `compress` is set, and that saves space), and returns its reference.
    // Port of: src/pdf/SkPDFTypes.cpp#L436-L470 (SkPDFStreamOut, serialize_stream, chrome/m156)
    pub(crate) fn stream_out(
        &mut self,
        dict: Option<PdfDict>,
        content: &[u8],
        compress: bool,
    ) -> PdfIndirectReference {
        // "/Filter_/FlateDecode_": the bytes a compressed stream must save to be worth it.
        const MINIMUM_SAVINGS: usize = 21;
        let reference = self.reserve_ref();
        let mut dict = dict.unwrap_or_else(|| PdfDict::new(None));
        let mut data: Vec<u8> = content.to_vec();
        if self.metadata.compression_level != CompressionLevel::None
            && compress
            && content.len() > MINIMUM_SAVINGS
        {
            let mut compressed_data = DynamicMemoryWStream::new();
            {
                let level = self.metadata.compression_level as i32;
                let mut deflate = DeflateWStream::new(Some(&mut compressed_data), level, false);
                deflate.write(content);
                deflate.finalize();
            }
            let compressed = compressed_data.detach_as_vector();
            if content.len() > compressed.len() + MINIMUM_SAVINGS {
                data = compressed;
                dict.insert_name("Filter", "FlateDecode");
            }
        }
        dict.insert_int_usize("Length", data.len());
        self.emit_stream(&dict, &data, reference);
        reference
    }

    /// `getPage`: the reference of page `page_index`.
    // Port of: src/pdf/SkPDFDocument.cpp#L548-L551 (chrome/m156)
    pub(crate) fn get_page(&self, page_index: usize) -> PdfIndirectReference {
        debug_assert!(page_index < self.page_refs.len());
        self.page_refs[page_index]
    }

    /// `hasCurrentPage`.
    pub(crate) fn has_current_page(&self) -> bool {
        self.has_current_page
    }

    /// `currentPage`.
    pub(crate) fn current_page(&self) -> PdfIndirectReference {
        debug_assert!(self.has_current_page && !self.page_refs.is_empty());
        self.page_refs[self.page_refs.len() - 1]
    }

    /// `currentPageTransform`: the initial transform of the page device, or the identity when
    /// not on a page (like when emitting a Type3 glyph).
    // Port of: src/pdf/SkPDFDocument.cpp#L553-L560 (chrome/m156)
    pub(crate) fn current_page_transform(&self) -> Matrix {
        if !self.has_current_page {
            return Matrix::new_identity();
        }
        self.current_page_transform.clone()
    }

    /// `createMarkForElemId`.
    // Port of: src/pdf/SkPDFDocument.cpp#L562-L571 (chrome/m156)
    pub(crate) fn create_mark_for_elem_id(
        &mut self,
        elem_id: i32,
        struct_parents_key: &mut PdfParentTreeKey,
    ) -> Mark {
        // If the mark isn't on a page (like when emitting a Type3 glyph)
        // return a temporary mark not attached to the page or a structure element.
        if !self.has_current_page {
            return Mark::default();
        }
        let page_index = u32::try_from(self.pages.len()).expect("fits");
        self.struct_tree
            .create_mark_for_elem_id(elem_id, page_index, struct_parents_key)
    }

    /// `createStructParentKeyForElemId`.
    // Port of: src/pdf/SkPDFDocument.cpp#L584-L594 (chrome/m156)
    pub(crate) fn create_struct_parent_key_for_elem_id(
        &mut self,
        elem_id: i32,
        content_item_ref: PdfIndirectReference,
    ) -> PdfParentTreeKey {
        // Structure elements are tied to pages, so don't emit one if not on a page.
        if !self.has_current_page {
            return PdfParentTreeKey::default();
        }
        let page_index = u32::try_from(self.pages.len()).expect("fits");
        self.struct_tree
            .create_struct_parent_key_for_elem_id(elem_id, page_index, content_item_ref)
    }

    /// `getAnnotations`: writes the links of the current page, and returns the array of them.
    // Port of: src/pdf/SkPDFDocument.cpp#L301-L342 (chrome/m156)
    fn get_annotations(&mut self) -> Option<PdfArray> {
        let count = self.current_page_links.len();
        if 0 == count {
            return None;
        }
        let mut array = PdfArray::new();
        array.reserve(count);
        let links = std::mem::take(&mut self.current_page_links);
        for link in &links {
            let mut annotation = PdfDict::new(Some("Annot"));
            populate_link_annotation(&mut annotation, &link.rect);
            if link.link_type == LinkType::Url {
                let mut action = PdfDict::new(Some("Action"));
                action.insert_name("S", "URI");
                // This is documented to be a 7 bit ASCII (byte) string.
                action.insert_byte_string("URI", to_valid_utf8_string(&link.data));
                annotation.insert_object("A", Box::new(action));
            } else if link.link_type == LinkType::NamedDestination {
                annotation.insert_name_escaped("Dest", to_valid_utf8_string(&link.data));
            } else {
                debug_assert!(false, "Unknown link type.");
            }

            let annotation_ref = self.reserve_ref();
            if link.elem_id != 0 {
                let struct_parent_key =
                    self.create_struct_parent_key_for_elem_id(link.elem_id, annotation_ref);
                if struct_parent_key.is_valid() {
                    annotation.insert_int("StructParent", struct_parent_key.value);
                }
            }

            self.emit(&annotation, annotation_ref);
            array.append_ref(annotation_ref);
        }
        // `fCurrentPageLinks.clear()` follows in `onEndPage`.
        Some(array)
    }

    /// `append_destinations`.
    // Port of: src/pdf/SkPDFDocument.cpp#L275-L287 (chrome/m156)
    fn append_destinations(
        &mut self,
        named_destinations: &[PdfNamedDestination],
    ) -> PdfIndirectReference {
        let mut destinations = PdfDict::new(None);
        for dest in named_destinations {
            let mut pdf_dest = PdfArray::new();
            pdf_dest.reserve(5);
            pdf_dest.append_ref(dest.page);
            pdf_dest.append_name("XYZ");
            pdf_dest.append_scalar(dest.point.x);
            pdf_dest.append_scalar(dest.point.y);
            pdf_dest.append_int(0); // Leave zoom unchanged
            let name = to_valid_utf8_string(&dest.name);
            destinations.insert_object_escaped_key(&name, Box::new(pdf_dest));
        }
        self.emit_new(&destinations)
    }

    /// `SkPDFMetadata::MakeXMPObject`: the XMP packet of a PDF/A document, as an uncompressed
    /// stream.
    // Port of: src/pdf/SkPDFMetadata.cpp#L228-L329 (MakeXMPObject, chrome/m156)
    fn make_xmp_object(&mut self, doc: Uuid, instance: Uuid) -> PdfIndirectReference {
        let value = xmp_packet(&self.metadata, doc, instance);
        let mut dict = PdfDict::new(Some("Metadata"));
        dict.insert_name("Subtype", "XML");
        // Not compressed: a program without PDF support must be able to grep for "<?xpacket".
        self.stream_out(Some(dict), value.as_bytes(), false)
    }

    /// The first-page part of `SkPDFDocument::onBeginPage`: the header, the information
    /// dictionary, and for PDF/A documents the UUID and the XMP packet.
    // Port of: src/pdf/SkPDFDocument.cpp#L271-L285 (onBeginPage, first page, chrome/m156)
    fn start_document(&mut self) {
        let flushed = self.flushed;
        serialize_header(&mut self.offset_map, &mut self.out, flushed);
        let info = make_document_information_dict(&self.metadata);
        self.info_dict = self.emit_new(&info);
        if self.metadata.pdfa {
            self.uuid = create_uuid(&self.metadata);
            // The same UUID is the document ID and the instance ID, since this is the first
            // revision of the document (Skia does not revise PDF documents). Without PDF/A, no
            // UUID is used, so that output is reproducible.
            self.xmp = self.make_xmp_object(self.uuid, self.uuid);
        }
    }

    /// `SkPDFDocument::onClose`: writes the catalog, the page tree and the trailer. Writes
    /// nothing when there is no page, as in Skia.
    // Port of: src/pdf/SkPDFDocument.cpp#L628-L689 (onClose, chrome/m156)
    fn on_close(&mut self) {
        if self.pages.is_empty() {
            return;
        }
        let mut doc_catalog = PdfDict::new(Some("Catalog"));
        if self.metadata.pdfa {
            debug_assert!(self.xmp.is_valid());
            doc_catalog.insert_ref("Metadata", self.xmp);
            // Don't specify OutputIntents if we are not in PDF/A mode since
            // no one has ever asked for this feature.
            let intents = make_srgb_output_intents(self);
            doc_catalog.insert_object("OutputIntents", Box::new(intents));
        }

        let pages = std::mem::take(&mut self.pages);
        let page_refs = self.page_refs.clone();
        let page_tree = generate_page_tree(self, pages, &page_refs);
        doc_catalog.insert_ref("Pages", page_tree);

        if !self.named_destinations.is_empty() {
            let named_destinations = std::mem::take(&mut self.named_destinations);
            let dests = self.append_destinations(&named_destinations);
            doc_catalog.insert_ref("Dests", dests);
        }

        // Handle tagged PDFs. The tree is taken out of the document while it writes to it.
        let mut struct_tree = std::mem::take(&mut self.struct_tree);
        let root = struct_tree.emit_struct_tree_root(self);
        if root.is_valid() {
            // In the document catalog, indicate that this PDF is tagged.
            let mut mark_info = PdfDict::new(Some("MarkInfo"));
            mark_info.insert_bool("Marked", true);
            doc_catalog.insert_object("MarkInfo", Box::new(mark_info));
            doc_catalog.insert_ref("StructTreeRoot", root);

            let outline = struct_tree.make_outline(self);
            if outline.is_valid() {
                doc_catalog.insert_ref("Outlines", outline);
            }
        }
        let root_language = struct_tree.root_language();
        self.struct_tree = struct_tree;

        // If ViewerPreferences DisplayDocTitle isn't set to true, accessibility checks will fail.
        if !self.metadata.title.is_empty() {
            let mut viewer_prefs = PdfDict::new(Some("ViewerPreferences"));
            viewer_prefs.insert_bool("DisplayDocTitle", true);
            doc_catalog.insert_object("ViewerPreferences", Box::new(viewer_prefs));
        }

        let mut lang = self.metadata.lang.clone();
        if lang.is_empty() {
            lang = root_language;
        }
        if !lang.is_empty() {
            doc_catalog.insert_text_string("Lang", lang.as_str());
        }

        let doc_catalog_ref = self.emit_new(&doc_catalog);

        // TODO(M26): `for f in get_fonts() { f.emitSubset(self) }`: the font subsets are written
        // here, before the trailer, once `SkPDFFont` is ported.

        let flushed = self.flushed;
        serialize_footer(
            &self.offset_map,
            &mut self.out,
            flushed,
            self.info_dict,
            doc_catalog_ref,
            self.uuid,
        );
    }
}

/// A shared handle to the state of a document (what `SkPDFDocument*` is to a device).
#[derive(Clone)]
pub struct DocHandle(pub(crate) Rc<RefCell<DocInner>>);

impl std::fmt::Debug for DocHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocHandle").finish_non_exhaustive()
    }
}

impl DocHandle {
    /// Runs `f` on the state. `f` must not call anything that borrows the state again.
    pub(crate) fn with<R>(&self, f: impl FnOnce(&mut DocInner) -> R) -> R {
        f(&mut self.0.borrow_mut())
    }

    /// `reserveRef`.
    #[must_use]
    pub fn reserve_ref(&self) -> PdfIndirectReference {
        self.with(DocInner::reserve_ref)
    }

    /// `emit(object, ref)`.
    pub fn emit(
        &self,
        object: &dyn PdfObject,
        reference: PdfIndirectReference,
    ) -> PdfIndirectReference {
        self.with(|d| d.emit(object, reference))
    }

    /// `emit(object)`.
    #[must_use]
    pub fn emit_new(&self, object: &dyn PdfObject) -> PdfIndirectReference {
        self.with(|d| d.emit_new(object))
    }

    /// `SkPDFStreamOut`.
    #[must_use]
    pub fn stream_out(
        &self,
        dict: Option<PdfDict>,
        content: &[u8],
        compress: bool,
    ) -> PdfIndirectReference {
        self.with(|d| d.stream_out(dict, content, compress))
    }

    /// `emitStream`.
    pub fn emit_stream(&self, dict: &PdfDict, content: &[u8], reference: PdfIndirectReference) {
        self.with(|d| d.emit_stream(dict, content, reference));
    }

    /// `metadata`.
    #[must_use]
    pub fn metadata(&self) -> Rc<Metadata> {
        self.with(|d| Rc::clone(&d.metadata))
    }

    /// `hasCurrentPage`.
    #[must_use]
    pub fn has_current_page(&self) -> bool {
        self.with(|d| d.has_current_page())
    }
}

/// The state of a document's page (`SkDocument::State`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageState {
    BetweenPages,
    InPage,
    Closed,
}

/// `SkPDF::MakeDocument` (`pdf::new_document` in skia-safe): a PDF document that writes to
/// `writer`. `metadata` defaults to [`Metadata::default`]; a non-positive raster DPI becomes 72,
/// a negative encoding quality becomes 0, and missing JPEG callbacks become Skia's, as
/// `MakeDocument` does.
// Port of: src/pdf/SkPDFDocument.cpp#L711-L735 (`SkPDF::MakeDocument`, chrome/m156)
#[doc(alias = "SkPDF::MakeDocument")]
#[must_use]
pub fn new_document<'a>(writer: &'a mut dyn WStream, metadata: Option<&Metadata>) -> Document<'a> {
    let mut meta = metadata.cloned().unwrap_or_default();
    if meta.raster_dpi <= 0.0 {
        meta.raster_dpi = DEFAULT_RASTER_DPI;
    }
    if meta.encoding_quality < 0 {
        meta.encoding_quality = 0;
    }
    if meta.jpeg_decoder.is_none() {
        meta.jpeg_decoder = Some(jpeg::decode);
    }
    if meta.jpeg_encoder.is_none() {
        meta.jpeg_encoder = Some(jpeg::encode);
    }
    Document::new(writer, meta)
}

/// `SkPDFDocument` (with `SkDocument`): a PDF document written to a stream.
///
/// Pages are drawn between [`begin_page`](Self::begin_page) and [`end_page`](Self::end_page).
/// [`close`](Self::close) writes the catalog, the page tree and the trailer, and
/// [`abort`](Self::abort) stops writing without a trailer. Dropping an open document closes it,
/// as Skia's destructor does.
// Port of: src/pdf/SkPDFDocumentPriv.h#L84-L231, src/core/SkDocument.cpp (chrome/m156)
#[doc(alias = "SkPDFDocument")]
#[doc(alias = "SkDocument")]
pub struct Document<'a> {
    /// The destination. `None` once the document is closed or aborted.
    stream: Option<&'a mut dyn WStream>,
    inner: DocHandle,
    state: PageState,
    /// The canvas of the current page (`fCanvas`).
    canvas: Option<Canvas>,
    /// The content of the page device (`fPageDevice`).
    page_device: Option<ContentHandle>,
}

impl std::fmt::Debug for Document<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("open", &self.stream.is_some())
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl<'a> Document<'a> {
    // Port of: src/pdf/SkPDFDocument.cpp#L238-L245, src/core/SkDocument.cpp#L16 (chrome/m156)
    fn new(stream: &'a mut dyn WStream, metadata: Metadata) -> Self {
        Self {
            stream: Some(stream),
            inner: DocHandle(Rc::new(RefCell::new(DocInner::new(metadata)))),
            state: PageState::BetweenPages,
            canvas: None,
            page_device: None,
        }
    }

    /// `metadata()`.
    #[must_use]
    pub fn metadata(&self) -> Rc<Metadata> {
        self.inner.metadata()
    }

    /// Passes the bytes written so far to the stream.
    fn flush(&mut self) {
        let Some(stream) = self.stream.as_deref_mut() else {
            return;
        };
        self.inner.with(|d| {
            let n = d.out.bytes_written();
            d.out.write_to_and_reset(stream);
            d.flushed += n;
        });
    }

    /// `SkDocument::beginPage`: begins a new page and returns the canvas that draws into it. The
    /// document owns the canvas, and it goes out of scope when [`end_page`](Self::end_page) or
    /// [`close`](Self::close) is called, or the document is dropped. Calls `end_page` if there
    /// is a current page. `content` is the area of the page that the canvas draws to. Returns
    /// `None` if the size is empty, the document is closed, or `content` is outside the page.
    // Port of: src/core/SkDocument.cpp#L28-L58 (chrome/m156)
    #[doc(alias = "beginPage")]
    pub fn begin_page(&mut self, width: f32, height: f32, content: Option<&Rect>) -> Option<&Canvas> {
        if width <= 0.0 || height <= 0.0 || PageState::Closed == self.state {
            return None;
        }
        if PageState::InPage == self.state {
            self.end_page();
        }
        debug_assert_eq!(self.state, PageState::BetweenPages);
        self.state = PageState::InPage;
        self.on_begin_page(width, height);
        let canvas = self.canvas.as_ref()?;
        // `trim`
        if let Some(content) = content {
            let mut inner = *content;
            if !inner.intersect(Rect::new(0.0, 0.0, width, height)) {
                return None;
            }
            canvas.clip_rect(inner, None, None);
            canvas.translate((inner.x(), inner.y()));
        }
        Some(canvas)
    }

    /// `SkPDFDocument::onBeginPage`.
    // Port of: src/pdf/SkPDFDocument.cpp#L271-L322 (chrome/m156)
    fn on_begin_page(&mut self, width: f32, height: f32) {
        debug_assert!(self.canvas.is_none());
        let (raster_scale, inverse_raster_scale) = self.inner.with(|d| {
            if d.pages.is_empty() {
                // if this is the first page if the document.
                d.start_document();
            }
            (d.raster_scale, d.inverse_raster_scale)
        });
        // By scaling the page at the device level, we will create bitmap layer
        // devices at the rasterized scale, not the 72dpi scale.  Bitmap layer
        // devices are created when saveLayer is called with an ImageFilter;  see
        // SkPDFDevice::createDevice().
        let page_size: ISize =
            Size::new(width * raster_scale, height * raster_scale).to_round();
        let mut initial_transform = Matrix::new_identity();
        // Skia uses the top left as the origin but PDF natively has the origin at the
        // bottom left. This matrix corrects for that, as well as the raster scale.
        initial_transform.set_scale_translate(
            (inverse_raster_scale, -inverse_raster_scale),
            (0.0, inverse_raster_scale * page_size.height as f32),
        );
        let device = PdfDevice::new(page_size, &self.inner, &initial_transform);
        self.page_device = Some(device.content_handle());
        self.inner.with(|d| {
            d.has_current_page = true;
            d.current_page_transform = initial_transform;
        });
        let canvas = Canvas::from_device(Box::new(device));
        canvas.scale((raster_scale, raster_scale));
        self.canvas = Some(canvas);
        self.inner.with(|d| {
            let page_ref = d.reserve_ref();
            d.page_refs.push(page_ref);
        });
    }

    /// `SkDocument::endPage`: call when the content for the current page has been drawn into the
    /// canvas returned by [`begin_page`](Self::begin_page). After this call that canvas is gone.
    // Port of: src/core/SkDocument.cpp#L60-L65 (chrome/m156)
    #[doc(alias = "endPage")]
    pub fn end_page(&mut self) {
        if PageState::InPage == self.state {
            self.state = PageState::BetweenPages;
            self.on_end_page();
        }
    }

    /// `SkPDFDocument::onEndPage`.
    // Port of: src/pdf/SkPDFDocument.cpp#L344-L395 (chrome/m156)
    fn on_end_page(&mut self) {
        // `reset_object(&fCanvas)`
        self.canvas = None;
        let page_device = self.page_device.take().expect("a page device");

        let mut page = PdfDict::new(Some("Page"));

        let (page_content, resource_dict, media_size, struct_parents_key) = {
            let mut content = page_device.borrow_mut();
            let inverse_raster_scale = self.inner.with(|d| d.inverse_raster_scale);
            let size = content.size();
            let media_size = Size::new(
                size.width as f32 * inverse_raster_scale,
                size.height as f32 * inverse_raster_scale,
            );
            let page_content = content.content();
            let resource_dict = content.make_resource_dict();
            (
                page_content,
                resource_dict,
                media_size,
                content.struct_parents_key(),
            )
        };

        page.insert_object("Resources", Box::new(resource_dict));
        page.insert_object(
            "MediaBox",
            Box::new(rect_to_array(&Rect::from_size(media_size))),
        );

        self.inner.with(|d| {
            if let Some(annotations) = d.get_annotations() {
                page.insert_object("Annots", Box::new(annotations));
                d.current_page_links.clear();
            }

            let contents = d.stream_out(None, &page_content, true);
            page.insert_ref("Contents", contents);
            if struct_parents_key.is_valid() {
                page.insert_int("StructParents", struct_parents_key.value);
                d.struct_tree.set_content_stream_ref_for_struct_parents_key(
                    struct_parents_key,
                    crate::tag::PAGE_CONTENT_STREAM_REF,
                );
            }

            // Tabs is PDF 1.5, but setting it checks an accessibility box.
            page.insert_name("Tabs", "S");

            d.pages.push(page);
            d.has_current_page = false;
        });
        self.flush();
    }

    /// `SkDocument::close`: call when all pages have been drawn. This closes the stream. After
    /// `close` the document can no longer add new pages. Dropping the document calls `close` if
    /// need be.
    // Port of: src/core/SkDocument.cpp#L67-L83 (chrome/m156)
    pub fn close(&mut self) {
        loop {
            match self.state {
                PageState::BetweenPages => {
                    self.state = PageState::Closed;
                    self.inner.with(DocInner::on_close);
                    self.flush();
                    // we don't own the stream, but we mark it nullptr since we can
                    // no longer write to it.
                    self.stream = None;
                    self.inner.with(|d| d.finished = true);
                    return;
                }
                PageState::InPage => self.end_page(),
                PageState::Closed => return,
            }
        }
    }

    /// `SkDocument::abort`: stops writing. No trailer is written.
    // Port of: src/core/SkDocument.cpp#L85-L91, src/pdf/SkPDFDocument.cpp#L397-L399 (chrome/m156)
    pub fn abort(&mut self) {
        self.flush();
        self.state = PageState::Closed;
        // we don't own the stream, but we mark it nullptr since we can
        // no longer write to it.
        self.stream = None;
        self.inner.with(|d| d.finished = true);
    }
}

impl Drop for Document<'_> {
    // "subclasses of SkDocument must call close() in their destructors."
    fn drop(&mut self) {
        self.close();
    }
}
