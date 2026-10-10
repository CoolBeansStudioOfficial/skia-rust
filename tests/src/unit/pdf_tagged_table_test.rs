// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PDFTaggedTableTest.cpp (chrome/m156)

use skia_rust_core::color::Color;
use skia_rust_core::font::Font;
use skia_rust_core::paint::Paint;
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

// Port of: tests/PDFTaggedTableTest.cpp#L20-L151 (chrome/m156)
// Test building a tagged PDF containing a table.
// Add this to args.gn to output the PDF to a file:
//   extra_cflags = [ "-DSK_PDF_TEST_TAGS_OUTPUT_PATH=\"/tmp/table.pdf\"" ]
def_test!(SkPDF_tagged_table, |_r| {
    let mut output_stream = DynamicMemoryWStream::new();

    let page_size = Size::new(612.0, 792.0); // U.S. Letter

    let mut metadata = Metadata::default();
    metadata.title = "Example Tagged Table PDF".to_owned();
    metadata.creator = "Skia".to_owned();
    let now = get_date_time();
    metadata.creation = now;
    metadata.modified = now;
    metadata.jpeg_decoder = Some(jpeg::decode);
    metadata.jpeg_encoder = Some(jpeg::encode);

    const ROW_COUNT: usize = 5;
    const COL_COUNT: usize = 4;
    let cell_data: [Option<&str>; ROW_COUNT * COL_COUNT] = [
        Some("Car Make and Model"),
        Some("Engine"),
        Some("City MPG"),
        Some("Highway MPG"),
        Some("Mitsubishi Mirage ES"),
        Some("Gas"),
        Some("28"),
        Some("47"),
        Some("Toyota Prius Three"),
        Some("Hybrid"),
        Some("43"),
        Some("59"),
        Some("Nissan Leaf SL"),
        Some("Electric"),
        Some("N/A"),
        None,
        Some("Tesla Model 3"),
        None,
        Some("N/A"),
        None,
    ];

    // The document tag.
    let mut root = PdfTag::default();
    root.node_id = 1;
    root.type_string = "Document".to_owned();
    root.lang = "en-US".to_owned();

    // Heading.
    let mut h1 = PdfTag::default();
    h1.node_id = 2;
    h1.type_string = "H1".to_owned();
    h1.alt = "Tagged PDF Table Alt Text".to_owned();
    root.child_vector.push(h1);

    // Table.
    let mut table = PdfTag::default();
    table.node_id = 3;
    table.type_string = "Table".to_owned();
    table
        .attributes
        .append_float_array("Layout", "BBox", &[72.0, 72.0, 360.0, 360.0]);
    table
        .attributes
        .append_text_string("Table", "Summary", "Fuel efficiency");

    for row_index in 0..ROW_COUNT {
        let mut row = PdfTag::default();
        row.node_id = i32::try_from(4 + row_index).expect("fits");
        row.type_string = "TR".to_owned();
        for col_index in 0..COL_COUNT {
            let mut cell = PdfTag::default();
            let cell_index = row_index * COL_COUNT + col_index;
            cell.node_id = i32::try_from(10 + cell_index).expect("fits");
            if cell_data[cell_index].is_none() {
                cell.type_string = "NonStruct".to_owned();
            } else if row_index == 0 || col_index == 0 {
                cell.type_string = "TH".to_owned();
                cell.attributes.append_text_string("Table", "Short", "Car");
            } else {
                cell.type_string = "TD".to_owned();
                let header_ids = vec![
                    i32::try_from(10 + row_index * COL_COUNT).expect("fits"), // Row header
                    i32::try_from(10 + col_index).expect("fits"),             // Col header.
                ];
                cell.attributes
                    .append_node_id_array("Table", "Headers", &header_ids);
            }

            if cell_index == 13 {
                cell.attributes.append_int("Table", "RowSpan", 2);
            } else if cell_index == 14 || cell_index == 18 {
                cell.attributes.append_int("Table", "ColSpan", 2);
            } else if row_index == 0 || col_index == 0 {
                cell.attributes.append_name(
                    "Table",
                    "Scope",
                    if row_index == 0 { "Column" } else { "Row" },
                );
            }
            row.child_vector.push(cell);
        }
        table.child_vector.push(row);
    }
    root.child_vector.push(table);

    metadata.structure_element_tree_root = Some(root);
    let mut document = new_document(&mut output_stream, Some(&metadata));

    let mut paint = Paint::default();
    paint.set_color(Color::BLACK);

    let canvas = document
        .begin_page(page_size.width, page_size.height, None)
        .expect("a canvas");
    set_node_id(canvas, 2);
    let mut font = Font::from_size(default_typeface(), 36.0);
    canvas.draw_str("Tagged PDF Table", (72.0, 72.0), &font, &paint);

    font.set_size(14.0);
    for row_index in 0..ROW_COUNT {
        for col_index in 0..COL_COUNT {
            let cell_index = row_index * COL_COUNT + col_index;
            let Some(str_) = cell_data[cell_index] else {
                continue;
            };

            let x = 72 + col_index * 108 + if col_index > 0 { 72 } else { 0 };
            let y = 144 + row_index * 48;

            set_node_id(canvas, i32::try_from(10 + cell_index).expect("fits"));
            canvas.draw_str(str_, (x as f32, y as f32), &font, &paint);
        }
    }

    document.end_page();
    document.close();
});
