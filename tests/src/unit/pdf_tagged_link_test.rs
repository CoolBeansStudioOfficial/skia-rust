// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PDFTaggedLinkTest.cpp (chrome/m156)

use skia_rust_core::color::Color;
use skia_rust_core::data::Data;
use skia_rust_core::font::Font;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::size::Size;
use skia_rust_core::stream::DynamicMemoryWStream;
use skia_rust_pdf::jpeg;
use skia_rust_pdf::metadata::Metadata;
use skia_rust_pdf::tag::StructureElementNode;
use skia_rust_pdf::utils::get_date_time;
use skia_rust_pdf::{new_document, set_node_id};
use skia_rust_tools::font_tool_utils::default_typeface;

use crate::def_test;

type PdfTag = StructureElementNode;

// Port of: tests/PDFTaggedLinkTest.cpp#L19-L69 (chrome/m156)
// Test building a tagged PDF with links.
// Add this to args.gn to output the PDF to a file:
//   extra_cflags = [ "-DSK_PDF_TEST_TAGS_OUTPUT_PATH=\"/tmp/links.pdf\"" ]
def_test!(SkPDF_tagged_links, |_r| {
    let mut output_stream = DynamicMemoryWStream::new();

    let page_size = Size::new(612.0, 792.0); // U.S. Letter

    let mut metadata = Metadata::default();
    metadata.title = "Example Tagged PDF With Links".to_owned();
    metadata.creator = "Skia".to_owned();
    let now = get_date_time();
    metadata.creation = now;
    metadata.modified = now;
    metadata.jpeg_decoder = Some(jpeg::decode);
    metadata.jpeg_encoder = Some(jpeg::encode);

    // The document tag.
    let mut root = PdfTag::default();
    root.node_id = 1;
    root.type_string = "Document".to_owned();
    root.lang = "en-US".to_owned();

    // A link.
    let mut l1 = PdfTag::default();
    l1.node_id = 2;
    l1.type_string = "Link".to_owned();
    root.child_vector.push(l1);

    metadata.structure_element_tree_root = Some(root);
    let mut document = new_document(&mut output_stream, Some(&metadata));

    let mut paint = Paint::default();
    paint.set_color(Color::BLUE);

    let canvas = document
        .begin_page(page_size.width, page_size.height, None)
        .expect("a canvas");
    let font = Font::from_size(default_typeface(), 20.0);

    // The node ID should cover both the text and the annotation.
    set_node_id(canvas, 2);
    canvas.draw_str("Click to visit Google.com", (72.0, 72.0), &font, &paint);
    let link_rect = Rect::from_xywh(72.0, 54.0, 218.0, 24.0);
    let url = Data::new_with_cstring(Some(c"http://www.google.com"));
    canvas.draw_url_annotation(link_rect, &url);

    document.end_page();
    document.close();
});
