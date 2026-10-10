// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PDFTaggedTest.cpp (chrome/m156), all but `SkPDF_tagged_saveLayer`, which looks
// for the marked content of text and waits for the PDF fonts (modules.md M26).

#![allow(clippy::field_reassign_with_default)] // the tests assign the fields one by one, as the C++ does

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::font::Font;
use skia_rust_core::paint::Paint;
use skia_rust_core::size::Size;
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_pdf::jpeg;
use skia_rust_pdf::metadata::{Metadata, Outline};
use skia_rust_pdf::tag::{StructureElementNode, node_id};
use skia_rust_pdf::utils::get_date_time;
use skia_rust_pdf::{new_document, set_node_id};
use skia_rust_tools::font_tool_utils::default_typeface;

use crate::def_test;

type PdfTag = StructureElementNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmitHeader {
    No = 0,
    Yes = 1,
}

// Port of: tests/PDFTaggedTest.cpp#L26-L212 (chrome/m156)
#[allow(clippy::too_many_lines)] // one function in Skia
fn write_structured_document(
    output_stream: &mut dyn WStream,
    outline: Outline,
    emit_header: EmitHeader,
) {
    let page_size = Size::new(612.0, 792.0); // U.S. Letter

    let mut metadata = Metadata::default();
    metadata.title = "Example Tagged PDF".to_owned();
    metadata.creator = "Skia".to_owned();
    metadata.outline = outline;
    let now = get_date_time();
    metadata.creation = now;
    metadata.modified = now;
    metadata.jpeg_decoder = Some(jpeg::decode);
    metadata.jpeg_encoder = Some(jpeg::encode);

    // The document tag.
    let mut root = PdfTag::default();
    root.node_id = 1;
    root.type_string = "Document".to_owned();

    // Heading.
    if emit_header == EmitHeader::Yes {
        let mut h1 = PdfTag::default();
        h1.node_id = 2;
        h1.type_string = "H1".to_owned();
        h1.alt = "A Header".to_owned();
        root.child_vector.push(h1);
    }

    // Initial paragraph.
    let mut p = PdfTag::default();
    p.node_id = 3;
    p.type_string = "P".to_owned();
    root.child_vector.push(p);

    // Hidden div. This is never referenced by marked content
    // so it should not appear in the resulting PDF.
    let mut div = PdfTag::default();
    div.node_id = 4;
    div.type_string = "Div".to_owned();
    root.child_vector.push(div);

    // A bulleted list of two items.
    let mut l = PdfTag::default();
    l.node_id = 5;
    l.type_string = "L".to_owned();

    let mut lm1 = PdfTag::default();
    lm1.node_id = 6;
    lm1.type_string = "Lbl".to_owned();
    l.child_vector.push(lm1);

    let mut li1 = PdfTag::default();
    li1.node_id = 7;
    li1.type_string = "LI".to_owned();
    l.child_vector.push(li1);

    let mut lm2 = PdfTag::default();
    lm2.node_id = 8;
    lm2.type_string = "Lbl".to_owned();
    l.child_vector.push(lm2);
    let mut li2 = PdfTag::default();
    li2.node_id = 9;
    li2.type_string = "LI".to_owned();
    l.child_vector.push(li2);

    root.child_vector.push(l);

    // Paragraph spanning two pages.
    let mut p2 = PdfTag::default();
    p2.node_id = 10;
    p2.type_string = "P".to_owned();
    root.child_vector.push(p2);

    // Image with alt text.
    let mut img = PdfTag::default();
    img.node_id = 11;
    img.type_string = "Figure".to_owned();
    img.alt = "Red box".to_owned();
    root.child_vector.push(img);

    metadata.structure_element_tree_root = Some(root);
    let mut document = new_document(output_stream, Some(&metadata));

    let mut paint = Paint::default();
    paint.set_color(Color::BLACK);

    // First page.
    let canvas = document
        .begin_page(page_size.width, page_size.height, None)
        .expect("a canvas");
    set_node_id(canvas, 2);
    let mut font = Font::from_size(default_typeface(), 36.0);
    let mut message = "This is the title";
    canvas.translate((72.0, 72.0));
    canvas.draw_str(message, (0.0, 0.0), &font, &paint);

    set_node_id(canvas, 1);
    font.set_size(12.0);
    message = "Some document text";
    canvas.translate((0.0, 20.0));
    canvas.draw_str(message, (0.0, 0.0), &font, &paint);

    set_node_id(canvas, 3);
    font.set_size(14.0);
    message = "This is a simple paragraph.";
    canvas.translate((0.0, 72.0));
    canvas.draw_str(message, (0.0, 0.0), &font, &paint);

    set_node_id(canvas, 6);
    font.set_size(14.0);
    message = "*";
    canvas.translate((0.0, 72.0));
    canvas.draw_str(message, (0.0, 0.0), &font, &paint);

    set_node_id(canvas, 7);
    message = "List item 1";
    canvas.translate((36.0, 0.0));
    canvas.draw_str(message, (0.0, 0.0), &font, &paint);

    set_node_id(canvas, 8);
    message = "*";
    canvas.translate((-36.0, 36.0));
    canvas.draw_str(message, (0.0, 0.0), &font, &paint);

    set_node_id(canvas, 9);
    message = "List item 2";
    canvas.translate((36.0, 0.0));
    canvas.draw_str(message, (0.0, 0.0), &font, &paint);

    set_node_id(canvas, 1);
    font.set_size(12.0);
    message = "Some document text";
    canvas.translate((0.0, 20.0));
    canvas.draw_str(message, (0.0, 0.0), &font, &paint);

    set_node_id(canvas, 10);
    message = "This is a paragraph that starts on one page";
    canvas.translate((-36.0, 6.0 * 72.0));
    canvas.draw_str(message, (0.0, 0.0), &font, &paint);

    document.end_page();

    // Second page.
    let canvas = document
        .begin_page(page_size.width, page_size.height, None)
        .expect("a canvas");
    let mut bg_paint = Paint::default();
    bg_paint.set_color(Color::new(0xFFCC_CCCC)); // SK_ColorLTGRAY
    set_node_id(canvas, node_id::BACKGROUND_ARTIFACT);
    canvas.draw_paint(&bg_paint);

    set_node_id(canvas, 10);
    message = "and finishes on the second page.";
    canvas.translate((72.0, 72.0));
    canvas.draw_str(message, (0.0, 0.0), &font, &paint);

    set_node_id(canvas, 1);
    font.set_size(12.0);
    message = "Some document text";
    canvas.translate((0.0, 20.0));
    canvas.draw_str(message, (0.0, 0.0), &font, &paint);

    // Test a tagged image with alt text.
    set_node_id(canvas, 11);
    let mut test_bitmap = Bitmap::new();
    test_bitmap.alloc_n32_pixels((72, 72), None);
    test_bitmap.erase_color(Color::RED);
    canvas.draw_image(
        test_bitmap.as_image().expect("an image"),
        (72.0, 144.0),
        None,
    );

    set_node_id(canvas, 10);
    message = "and finishes on the second page.";
    canvas.translate((72.0, 72.0));
    canvas.draw_str(message, (0.0, 0.0), &font, &paint);

    // This has a node ID but never shows up in the tag tree so it
    // won't be tagged.
    set_node_id(canvas, 999);
    canvas.draw_str(
        "Page",
        (page_size.width - 100.0, page_size.height - 30.0),
        &font,
        &paint,
    );

    set_node_id(canvas, node_id::PAGINATION_FOOTER_ARTIFACT);
    canvas.draw_str(
        "2",
        (page_size.width - 30.0, page_size.height - 30.0),
        &font,
        &paint,
    );

    document.end_page();

    document.close();

    drop(document);
    output_stream.flush();
}

// To log the output, replace the SkDynamicMemoryWStream with something like
// SkFILEWStream outputStream("test.pdf");

// Port of: tests/PDFTaggedTest.cpp#L218-L225 (chrome/m156)
// Test building a tagged PDF with headers and a header outline.
def_test!(SkPDF_structelem_header_outline_doc, |_r| {
    let mut output_stream = DynamicMemoryWStream::new();
    write_structured_document(
        &mut output_stream,
        Outline::StructureElementHeaders,
        EmitHeader::Yes,
    );
});

// Port of: tests/PDFTaggedTest.cpp#L227-L234 (chrome/m156)
// Test building a tagged PDF with a structure element outline.
def_test!(SkPDF_structelem_outline_doc, |_r| {
    let mut output_stream = DynamicMemoryWStream::new();
    write_structured_document(
        &mut output_stream,
        Outline::StructureElements,
        EmitHeader::Yes,
    );
});

// Port of: tests/PDFTaggedTest.cpp#L236-L243 (chrome/m156)
// Test building a tagged PDF with no headers with a header outline.
def_test!(SkPDF_structelem_header_outline_doc_noheader, |_r| {
    let mut output_stream = DynamicMemoryWStream::new();
    write_structured_document(
        &mut output_stream,
        Outline::StructureElementHeaders,
        EmitHeader::No,
    );
});
