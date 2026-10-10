// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// API shape of: third_party/rust-skia/skia-safe/src/docs/pdf_document.rs (`skia_safe::pdf`)

//! Generating PDF (Portable Document Format) output via Skia's PDF backend (see
//! [`crate::Document`]), `skia_safe::pdf`.
//!
//! Differences from skia-safe, which wraps the C++ objects: [`Metadata`] has no lifetime, and its
//! structure tree ([`StructureElementNode`], [`AttributeList`]) owns its strings and takes `&str`
//! where skia-safe takes `&CString`. The document has the JPEG decoder and encoder callbacks of
//! `SkPDF::Metadata` inside, and they default to the codec crate's, as in `SkPDF::MakeDocument`.

use std::io;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::stream::WStream;

pub use skia_rust_pdf::date_time::DateTime;
pub use skia_rust_pdf::metadata::{CompressionLevel, Outline};
pub use skia_rust_pdf::tag::{AttributeList, StructureElementNode, node_id};

use crate::document::Document;
use crate::document::state;

/// Optional metadata to be passed into the PDF factory function (`SkPDF::Metadata`).
#[derive(Debug, Clone)]
pub struct Metadata {
    /// The document's title.
    pub title: String,
    /// The name of the person who created the document.
    pub author: String,
    /// The subject of the document.
    pub subject: String,
    /// Keywords associated with the document. Commas may be used to delineate keywords within
    /// the string.
    pub keywords: String,
    /// If the document was converted to PDF from another format, the name of the conforming
    /// product that created the original document from which it was converted.
    pub creator: String,
    /// The product that is converting this document to PDF.
    pub producer: String,
    /// The date and time the document was created. The zero default value represents an
    /// unknown/unset time.
    pub creation: Option<DateTime>,
    /// The date and time the document was most recently modified. The zero default value
    /// represents an unknown/unset time.
    pub modified: Option<DateTime>,
    /// The natural language of the text in the PDF. If `lang` is empty, the root
    /// [`StructureElementNode`]'s `lang` will be used (if not empty). Text not in this language
    /// should be marked with [`StructureElementNode`]'s `lang`.
    pub lang: String,
    /// The DPI (pixels-per-inch) at which features without native PDF support will be rasterized
    /// (e.g. draw image with perspective, draw text with perspective, ...) A larger DPI would
    /// create a PDF that reflects the original intent with better fidelity, but it can make for
    /// larger PDF files too, which would use more memory while rendering, and it would be slower
    /// to be processed or sent online or to printer.
    pub raster_dpi: Option<f32>,
    /// If `true`, include XMP metadata, a document UUID, and sRGB output intent information.
    /// This adds length to the document and makes it non-reproducible, but are necessary features
    /// for PDF/A-2b conformance.
    pub pdf_a: bool,
    /// Encoding quality controls the trade-off between size and quality. By default this is set
    /// to 101 percent, which corresponds to lossless encoding. If this value is set to a value
    /// <= 100, and the image is opaque, it will be encoded (using JPEG) with that quality
    /// setting.
    pub encoding_quality: Option<i32>,
    /// If `true`, rasterize gradients with non-opaque stops for print compatibility.
    ///
    /// Intended only for physical printing; it trades vector fidelity and file size to avoid
    /// PDF-to-PostScript converter bugs.
    pub rasterize_alpha_gradients_for_printing: bool,
    /// An optional tree of structured document tags that provide a semantic representation of
    /// the content.
    pub structure_element_tree_root: Option<StructureElementNode>,
    /// The outline of the document.
    pub outline: Outline,
    /// PDF streams may be compressed to save space. Use this to specify the desired compression
    /// vs time tradeoff.
    pub compression_level: CompressionLevel,
}

impl Default for Metadata {
    fn default() -> Self {
        let inner = skia_rust_pdf::metadata::Metadata::default();
        Self {
            title: String::new(),
            author: String::new(),
            subject: String::new(),
            keywords: String::new(),
            creator: String::new(),
            producer: inner.producer,
            creation: None,
            modified: None,
            lang: String::new(),
            raster_dpi: None,
            pdf_a: false,
            encoding_quality: None,
            rasterize_alpha_gradients_for_printing: false,
            structure_element_tree_root: None,
            outline: Outline::None,
            compression_level: CompressionLevel::Default,
        }
    }
}

impl Metadata {
    /// The metadata of the PDF backend.
    fn to_inner(&self) -> skia_rust_pdf::metadata::Metadata {
        let mut md = skia_rust_pdf::metadata::Metadata {
            title: self.title.clone(),
            author: self.author.clone(),
            subject: self.subject.clone(),
            keywords: self.keywords.clone(),
            creator: self.creator.clone(),
            producer: self.producer.clone(),
            lang: self.lang.clone(),
            pdfa: self.pdf_a,
            rasterize_alpha_gradients_for_printing: self.rasterize_alpha_gradients_for_printing,
            structure_element_tree_root: self.structure_element_tree_root.clone(),
            outline: self.outline,
            compression_level: self.compression_level,
            ..skia_rust_pdf::metadata::Metadata::default()
        };
        if let Some(creation) = self.creation {
            md.creation = creation;
        }
        if let Some(modified) = self.modified {
            md.modified = modified;
        }
        if let Some(raster_dpi) = self.raster_dpi {
            md.raster_dpi = raster_dpi;
        }
        if let Some(encoding_quality) = self.encoding_quality {
            md.encoding_quality = encoding_quality;
        }
        md
    }
}

/// A [`WStream`] that writes to an [`io::Write`] (`RustWStream`).
struct IoWStream<'a, W: io::Write + ?Sized> {
    writer: &'a mut W,
    bytes_written: usize,
}

impl<W: io::Write + ?Sized> WStream for IoWStream<'_, W> {
    fn write(&mut self, buffer: &[u8]) -> bool {
        if self.writer.write_all(buffer).is_err() {
            return false;
        }
        self.bytes_written += buffer.len();
        true
    }

    fn flush(&mut self) {
        let _ = self.writer.flush();
    }

    fn bytes_written(&self) -> usize {
        self.bytes_written
    }
}

/// Create a PDF-backed document, writing the results into a writer.
///
/// PDF pages are sized in point units. 1 pt == 1/72 inch == 127/360 mm.
///
/// - `writer` - A PDF document will be written to this writer. The document may write to the
///   writer at anytime during its lifetime, until either [`crate::Document::close`] is called or
///   the document is deleted.
/// - `metadata` - a PDF metadata object. Some fields may be left empty.
#[doc(alias = "SkPDF::MakeDocument")]
pub fn new_document<'a, W: io::Write + ?Sized>(
    writer: &'a mut W,
    metadata: Option<&Metadata>,
) -> Document<'a> {
    let stream = IoWStream {
        writer,
        bytes_written: 0,
    };
    let inner_metadata = metadata.map(Metadata::to_inner);
    let document =
        skia_rust_pdf::document::new_document_boxed(Box::new(stream), inner_metadata.as_ref());
    Document::new(document, state::Open { pages: 0 })
}

/// Associate a node ID with subsequent drawing commands in a [`Canvas`].
///
/// The same node ID can appear in a [`StructureElementNode`] in order to associate a document's
/// structure element tree with its content.
///
/// A node ID of zero indicates no node ID. Negative node IDs are reserved.
///
/// - `canvas` - The canvas used to draw to the PDF.
/// - `node_id` - The node ID for subsequent drawing commands.
#[doc(alias = "SkPDF::SetNodeId")]
pub fn set_node_id(canvas: &Canvas, node_id: i32) {
    skia_rust_pdf::set_node_id(canvas, node_id);
}

#[cfg(test)]
mod tests {
    use skia_rust_core::paint::Paint;
    use skia_rust_core::rect::Rect;

    use super::{Metadata, new_document};

    // Port of: rust-skia skia-safe/src/docs/pdf_document.rs (`generate_pdf_with_structure_and_attributes`
    // shape: a document with a structure tree, written through `std::io::Write`).
    #[test]
    fn writes_a_pdf_through_io_write() {
        let mut bytes: Vec<u8> = Vec::new();
        {
            let mut root = super::StructureElementNode::new("Document");
            root.node_id = 1;
            let metadata = Metadata {
                title: "Facade".to_owned(),
                structure_element_tree_root: Some(root),
                ..Metadata::default()
            };
            let document = new_document(&mut bytes, Some(&metadata));
            let mut document = document.begin_page((200.0, 200.0), None);
            super::set_node_id(document.canvas(), 1);
            document
                .canvas()
                .draw_rect(Rect::from_wh(50.0, 50.0), &Paint::default());
            assert_eq!(document.page(), 1);
            let document = document.end_page();
            assert_eq!(document.pages(), 1);
            document.close();
        }
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(bytes.ends_with(b"%%EOF\n"));
        assert!(
            bytes
                .windows(b"/StructTreeRoot".len())
                .any(|w| w == b"/StructTreeRoot")
        );
    }
}
