// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFDocument.cpp, src/pdf/SkPDFDocumentPriv.h (SkPDFDocument, SkPDFOffsetMap,
// SkPDF::MakeDocument) and the object-stream helpers of src/pdf/SkPDFTypes.cpp (chrome/m156)

//! The PDF document skeleton: the header, the indirect objects with their cross-reference
//! offsets, the page tree, the catalog and the trailer.
//!
//! What is here: object serialization (`emit`, `emit_stream`, `SkPDFStreamOut`), the offset map and
//! the cross-reference table, the trailer, the page tree (`generate_page_tree`), the document
//! information dictionary, and the PDF/A metadata and output intent.
//!
//! Not here yet, and waiting for the device (`modules.md` M25): `begin_page` and `end_page`, which
//! draw into an `SkPDFDevice` and produce each page's resources and content stream; annotations,
//! structure tree, named destinations and the outline; and the font subsets (M26), which
//! `close` emits before the trailer. Until M25 a document can have no page, and `close` then
//! writes nothing, as in Skia. The tests that draw are left `todo`.

use skia_rust_core::stream::{DynamicMemoryWStream, WStream};

use crate::deflate::DeflateWStream;
use crate::metadata::{
    CompressionLevel, Metadata, Uuid, create_uuid, make_document_information_dict, make_pdf_id,
    xmp_packet,
};
use crate::srgb_icc::SRGB_PROFILE;
use crate::types::{PdfArray, PdfDict, PdfIndirectReference, PdfObject};

/// `SK_ScalarDefaultRasterDPI`.
pub(crate) const DEFAULT_RASTER_DPI: f32 = 72.0;

/// `SkPDFOffsetMap`: the byte offset of every indirect object, for the cross-reference table.
// Port of: src/pdf/SkPDFDocumentPriv.h#L38-L52, src/pdf/SkPDFDocument.cpp#L83-L117 (chrome/m156)
#[doc(alias = "SkPDFOffsetMap")]
#[derive(Debug, Default)]
pub struct PdfOffsetMap {
    /// `fOffsets[i]` is the offset of object `i + 1`.
    offsets: Vec<i32>,
    /// The offset of the `%PDF` header, relative to which the offsets are taken.
    base_offset: Option<usize>,
}

impl PdfOffsetMap {
    /// `markStartOfDocument`: offsets are relative to the stream's current position.
    // Port of: src/pdf/SkPDFDocument.cpp#L83 (chrome/m156)
    pub fn mark_start_of_document(&mut self, s: &dyn WStream) {
        self.base_offset = Some(s.bytes_written());
    }

    /// `markStartOfObject`: records the offset of object `reference_number`.
    // Port of: src/pdf/SkPDFDocument.cpp#L89-L96 (chrome/m156)
    pub fn mark_start_of_object(&mut self, reference_number: i32, s: &dyn WStream) {
        debug_assert!(reference_number > 0);
        let index = usize::try_from(reference_number - 1).unwrap_or(0);
        if index >= self.offsets.len() {
            self.offsets.resize(index + 1, 0);
        }
        self.offsets[index] = i32::try_from(self.difference(s.bytes_written())).unwrap_or(i32::MAX);
    }

    /// `objectCount`: the number of entries, including the free zeroth object.
    // Port of: src/pdf/SkPDFDocument.cpp#L98-L101 (chrome/m156)
    #[must_use]
    pub fn object_count(&self) -> i32 {
        // Include the special zeroth object in the count.
        i32::try_from(self.offsets.len() + 1).unwrap_or(i32::MAX)
    }

    /// `emitCrossReferenceTable`: writes the `xref` table and returns the offset of the table.
    // Port of: src/pdf/SkPDFDocument.cpp#L102-L117 (chrome/m156)
    pub fn emit_cross_reference_table(&self, s: &mut dyn WStream) -> i32 {
        let x_ref_file_offset =
            i32::try_from(self.difference(s.bytes_written())).unwrap_or(i32::MAX);
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

    /// `difference`: the stream position relative to the start of the document.
    // Port of: src/pdf/SkPDFDocument.cpp#L85-L87 (chrome/m156)
    fn difference(&self, position: usize) -> usize {
        let base = self.base_offset.unwrap_or(0);
        debug_assert!(position >= base);
        position.saturating_sub(base)
    }
}

/// `serializeHeader`: `%PDF-1.4` and the binary comment that marks the file as binary.
// Port of: src/pdf/SkPDFDocument.cpp#L124-L131 (chrome/m156)
fn serialize_header(offset_map: &mut PdfOffsetMap, w_stream: &mut dyn WStream) {
    offset_map.mark_start_of_document(w_stream);
    w_stream.write_text("%PDF-1.4\n%");
    // The PDF spec recommends a comment with four bytes, all with their high bits set. "\xD3\xEB
    // \xE9\xE1" is "Skia" with the high bits set.
    w_stream.write(&[0xD3, 0xEB, 0xE9, 0xE1]);
    w_stream.write_text("\n");
}

/// `begin_indirect_object`: `N 0 obj`, and the offset of it.
// Port of: src/pdf/SkPDFDocument.cpp#L133-L139 (chrome/m156)
fn begin_indirect_object(
    offset_map: &mut PdfOffsetMap,
    reference: PdfIndirectReference,
    s: &mut dyn WStream,
) {
    offset_map.mark_start_of_object(reference.value, s);
    s.write_dec_as_text(reference.value);
    s.write_text(" 0 obj\n"); // Generation number is always 0.
}

/// `end_indirect_object`.
// Port of: src/pdf/SkPDFDocument.cpp#L141 (chrome/m156)
fn end_indirect_object(s: &mut dyn WStream) {
    s.write_text("\nendobj\n");
}

/// `serialize_footer`: the cross-reference table and the trailer.
// Port of: src/pdf/SkPDFDocument.cpp#L144-L164 (chrome/m156)
fn serialize_footer(
    offset_map: &PdfOffsetMap,
    w_stream: &mut dyn WStream,
    info_dict: PdfIndirectReference,
    doc_catalog: PdfIndirectReference,
    uuid: Uuid,
) {
    let x_ref_file_offset = offset_map.emit_cross_reference_table(w_stream);
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
    fn layer(vec: Vec<PageTreeNode>, doc: &mut Document<'_>) -> Vec<PageTreeNode> {
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
    doc: &mut Document<'_>,
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
fn make_srgb_color_profile(doc: &mut Document<'_>) -> PdfIndirectReference {
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
fn make_srgb_output_intents(doc: &mut Document<'_>) -> PdfArray {
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

/// `SkPDF::MakeDocument` (`pdf::new_document` in skia-safe): a PDF document that writes to
/// `writer`. `metadata` defaults to [`Metadata::default`]; a non-positive raster DPI becomes 72,
/// and a negative encoding quality becomes 0, as `MakeDocument` does.
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
    Document::new(writer, meta)
}

/// `SkPDFDocument`: a PDF document written to a stream.
///
/// The document is written as it goes. `close` writes the catalog, the page tree and the
/// trailer, and `abort` stops writing without a trailer. Dropping an open document closes it,
/// as Skia's destructor does.
// Port of: src/pdf/SkPDFDocumentPriv.h#L84-L140, src/pdf/SkPDFDocument.cpp#L238-L266 (chrome/m156)
#[doc(alias = "SkPDFDocument")]
pub struct Document<'a> {
    /// The destination. `None` once the document is closed or aborted.
    stream: Option<&'a mut dyn WStream>,
    offset_map: PdfOffsetMap,
    metadata: Metadata,
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
}

impl std::fmt::Debug for Document<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("open", &self.stream.is_some())
            .field("next_object_number", &self.next_object_number)
            .field("pages", &self.pages.len())
            .finish_non_exhaustive()
    }
}

impl<'a> Document<'a> {
    // Port of: src/pdf/SkPDFDocument.cpp#L238-L245 (chrome/m156)
    fn new(stream: &'a mut dyn WStream, metadata: Metadata) -> Self {
        Self {
            stream: Some(stream),
            offset_map: PdfOffsetMap::default(),
            metadata,
            uuid: Uuid::default(),
            info_dict: PdfIndirectReference::default(),
            xmp: PdfIndirectReference::default(),
            next_object_number: 1,
            pages: Vec::new(),
            page_refs: Vec::new(),
        }
    }

    /// `metadata()`.
    #[must_use]
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// `reserveRef`: a fresh object number.
    // Port of: src/pdf/SkPDFDocumentPriv.h#L118 (chrome/m156)
    pub fn reserve_ref(&mut self) -> PdfIndirectReference {
        let value = self.next_object_number;
        self.next_object_number += 1;
        PdfIndirectReference { value }
    }

    /// Whether the document is still open for writing.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.stream.is_some()
    }

    /// `emit`: writes `object` as the indirect object `reference`, and returns the reference.
    // Port of: src/pdf/SkPDFDocument.cpp#L252-L257 (chrome/m156)
    pub fn emit(
        &mut self,
        object: &dyn PdfObject,
        reference: PdfIndirectReference,
    ) -> PdfIndirectReference {
        let Some(s) = self.stream.as_deref_mut() else {
            return reference;
        };
        begin_indirect_object(&mut self.offset_map, reference, s);
        object.emit_object(s);
        end_indirect_object(s);
        reference
    }

    /// `emitStream`: writes a stream object, its dictionary, `stream`, and `endstream`.
    // Port of: src/pdf/SkPDFDocumentPriv.h#L92-L101 (chrome/m156)
    pub fn emit_stream(&mut self, dict: &PdfDict, content: &[u8], reference: PdfIndirectReference) {
        let Some(s) = self.stream.as_deref_mut() else {
            return;
        };
        begin_indirect_object(&mut self.offset_map, reference, s);
        dict.emit_object(s);
        s.write_text(" stream\n");
        s.write(content);
        s.write_text("\nendstream");
        end_indirect_object(s);
    }

    /// `SkPDFStreamOut` with no executor: writes `content` as a stream object (deflated when the
    /// metadata asks for compression and `compress` is set, and that saves space), and returns
    /// its reference.
    // Port of: src/pdf/SkPDFTypes.cpp#L436-L470 (SkPDFStreamOut, serialize_stream, chrome/m156)
    pub fn stream_out(
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
    /// dictionary, and for PDF/A documents the UUID and the XMP packet. Called once, before the
    /// first page (or, until the device exists, before the first object).
    // Port of: src/pdf/SkPDFDocument.cpp#L271-L285 (onBeginPage, first page, chrome/m156)
    pub fn start_document(&mut self) {
        if let Some(s) = self.stream.as_deref_mut() {
            serialize_header(&mut self.offset_map, s);
        }
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

    /// `emit(object)`: reserves a reference and writes the object under it.
    pub fn emit_new(&mut self, object: &dyn PdfObject) -> PdfIndirectReference {
        let reference = self.reserve_ref();
        self.emit(object, reference)
    }

    /// Adds a finished page (its dictionary and reserved reference). The device builds the page
    /// dictionary in `end_page` (`modules.md` M25); until then, callers pass the dictionaries.
    pub fn add_page(&mut self, page: PdfDict, reference: PdfIndirectReference) {
        self.pages.push(page);
        self.page_refs.push(reference);
    }

    /// `SkDocument::close` for `SkPDFDocument::onClose`: writes the catalog, the page tree and
    /// the trailer. Writes nothing when there is no page, as in Skia.
    // Port of: src/pdf/SkPDFDocument.cpp#L628-L689 (onClose, chrome/m156)
    pub fn close(&mut self) {
        if self.stream.is_none() {
            return;
        }
        if self.pages.is_empty() {
            self.stream = None;
            return;
        }
        let mut doc_catalog = PdfDict::new(Some("Catalog"));
        if self.metadata.pdfa {
            debug_assert!(self.xmp.is_valid());
            doc_catalog.insert_ref("Metadata", self.xmp);
            // Output intents are written only in PDF/A mode.
            let intents = make_srgb_output_intents(self);
            doc_catalog.insert_object("OutputIntents", Box::new(intents));
        }
        let pages = std::mem::take(&mut self.pages);
        let page_refs = std::mem::take(&mut self.page_refs);
        let page_tree = generate_page_tree(self, pages, &page_refs);
        doc_catalog.insert_ref("Pages", page_tree);
        // ViewerPreferences: accessibility checks need DisplayDocTitle when there is a title.
        if !self.metadata.title.is_empty() {
            let mut viewer_prefs = PdfDict::new(Some("ViewerPreferences"));
            viewer_prefs.insert_bool("DisplayDocTitle", true);
            doc_catalog.insert_object("ViewerPreferences", Box::new(viewer_prefs));
        }
        if !self.metadata.lang.is_empty() {
            doc_catalog.insert_text_string("Lang", self.metadata.lang.as_str());
        }
        let doc_catalog_ref = self.emit_new(&doc_catalog);
        if let Some(s) = self.stream.as_deref_mut() {
            serialize_footer(
                &self.offset_map,
                s,
                self.info_dict,
                doc_catalog_ref,
                self.uuid,
            );
        }
        self.stream = None;
    }

    /// `SkDocument::abort`: stops writing. No trailer is written.
    // Port of: src/pdf/SkPDFDocument.cpp#L409-L411 (onAbort, chrome/m156)
    pub fn abort(&mut self) {
        self.stream = None;
    }
}

impl Drop for Document<'_> {
    // "subclasses of SkDocument must call close() in their destructors."
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document with one bare page, closed: the header, the objects, the xref table and the
    /// trailer, with the `startxref` offset pointing at the table.
    #[test]
    fn skeleton_has_header_xref_and_trailer() {
        let mut out = DynamicMemoryWStream::new();
        {
            let metadata = Metadata {
                title: "T".to_owned(),
                ..Metadata::default()
            };
            let mut doc = new_document(&mut out, Some(&metadata));
            doc.start_document();
            let reference = doc.reserve_ref();
            doc.add_page(PdfDict::new(Some("Page")), reference);
            doc.close();
        }
        let bytes = out.detach_as_vector();
        assert!(bytes.starts_with(b"%PDF-1.4\n%\xD3\xEB\xE9\xE1\n"));
        assert!(bytes.ends_with(b"\n%%EOF\n"));
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Title (T)"));
        assert!(text.contains("/Type /Catalog"));
        assert!(text.contains("/Type /Page"));
        assert!(text.contains("/Type /Pages"));
        // The startxref offset points at the cross-reference table, measured from the start of
        // the document.
        let tail = text.rsplit("startxref\n").next().unwrap_or("");
        let offset: usize = tail.split('\n').next().unwrap_or("").parse().unwrap_or(0);
        assert!(bytes[offset..].starts_with(b"xref\n0 "));
    }

    /// Closing a document with no page writes nothing, as in Skia.
    #[test]
    fn close_without_pages_writes_nothing() {
        let mut out = DynamicMemoryWStream::new();
        {
            let mut doc = new_document(&mut out, None);
            doc.close();
        }
        assert_eq!(out.bytes_written(), 0);
    }

    /// An aborted document writes no trailer.
    #[test]
    fn abort_writes_no_trailer() {
        let mut out = DynamicMemoryWStream::new();
        {
            let mut doc = new_document(&mut out, None);
            doc.start_document();
            let reference = doc.reserve_ref();
            doc.add_page(PdfDict::new(Some("Page")), reference);
            doc.abort();
        }
        assert!(out.bytes_written() < 256);
    }
}
