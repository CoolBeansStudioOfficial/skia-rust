// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PDFDocumentTest.cpp (chrome/m156)

#![allow(clippy::field_reassign_with_default)] // the tests assign the fields one by one, as the C++ does
#![allow(clippy::cast_precision_loss)] // mirrors the C++ casts of the test drawing code
#![allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the test drawing code
#![allow(clippy::cast_sign_loss)] // mirrors the C++ casts of the test drawing code

use std::io::Read as _;

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::paint::Paint;
use skia_rust_core::stream::{DynamicMemoryWStream, FileWStream, NullWStream, WStream};
use skia_rust_pdf::date_time::DateTime;
use skia_rust_pdf::jpeg;
use skia_rust_pdf::metadata::Metadata;
use skia_rust_pdf::new_document;
use skia_rust_tools::font_tool_utils::default_font;

use crate::tmp_dir::get_tmp_dir;
use crate::{Reporter, def_test, errorf, reporter_assert};

// Port of: tests/PDFDocumentTest.cpp#L29-L36 (chrome/m156)
fn test_empty(reporter: &mut Reporter) {
    let mut stream = DynamicMemoryWStream::new();
    {
        let mut doc = new_document(&mut stream, Some(&jpeg::metadata_with_callbacks()));

        doc.close();
    }

    reporter_assert!(reporter, stream.bytes_written() == 0);
}

// Port of: tests/PDFDocumentTest.cpp#L38-L53 (chrome/m156)
fn test_abort(reporter: &mut Reporter) {
    let mut stream = DynamicMemoryWStream::new();
    {
        let mut doc = new_document(&mut stream, Some(&jpeg::metadata_with_callbacks()));

        let canvas = doc.begin_page(100.0, 100.0, None);
        canvas.expect("a canvas").draw_color(Color::RED, None);
        doc.end_page();

        doc.abort();
    }

    // Test that only the header is written, not the full document.
    reporter_assert!(reporter, stream.bytes_written() < 256);
}

// Port of: tests/PDFDocumentTest.cpp#L55-L90 (chrome/m156)
fn test_abort_with_file(reporter: &mut Reporter) {
    let Some(tmp_dir) = get_tmp_dir() else {
        errorf!(reporter, "missing tmpDir.");
        return;
    };

    let path = tmp_dir.join("aborted.pdf");
    if !FileWStream::new(&path).is_valid() {
        errorf!(reporter, "unable to write to: {}", path.display());
        return;
    }

    // Make sure doc's destructor is called to flush.
    {
        let mut stream = FileWStream::new(&path);
        let mut doc = new_document(&mut stream, Some(&jpeg::metadata_with_callbacks()));

        let canvas = doc.begin_page(100.0, 100.0, None);
        canvas.expect("a canvas").draw_color(Color::RED, None);
        doc.end_page();

        doc.abort();
    }

    let mut file = std::fs::File::open(&path).expect("the file was written");
    // Test that only the header is written, not the full document.
    let mut buffer = [0u8; 256];
    let mut read = 0;
    while read < buffer.len() {
        match file.read(&mut buffer[read..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => read += n,
        }
    }
    reporter_assert!(reporter, read < buffer.len());
}

// Port of: tests/PDFDocumentTest.cpp#L92-L124 (chrome/m156)
fn test_file(reporter: &mut Reporter) {
    let Some(tmp_dir) = get_tmp_dir() else {
        errorf!(reporter, "missing tmpDir.");
        return;
    };

    let path = tmp_dir.join("file.pdf");
    if !FileWStream::new(&path).is_valid() {
        errorf!(reporter, "unable to write to: {}", path.display());
        return;
    }

    {
        let mut stream = FileWStream::new(&path);
        let mut doc = new_document(&mut stream, Some(&jpeg::metadata_with_callbacks()));
        let canvas = doc.begin_page(100.0, 100.0, None);

        canvas.expect("a canvas").draw_color(Color::RED, None);
        doc.end_page();
        doc.close();
    }

    let file = std::fs::File::open(&path);
    reporter_assert!(reporter, file.is_ok());
    let mut header = [0u8; 4];
    if let Ok(mut file) = file {
        reporter_assert!(reporter, file.read_exact(&mut header).is_ok());
    }
    reporter_assert!(reporter, &header == b"%PDF");
}

// Port of: tests/PDFDocumentTest.cpp#L126-L137 (chrome/m156)
fn test_close(reporter: &mut Reporter) {
    let mut stream = DynamicMemoryWStream::new();
    {
        let mut doc = new_document(&mut stream, Some(&jpeg::metadata_with_callbacks()));

        let canvas = doc.begin_page(100.0, 100.0, None);
        canvas.expect("a canvas").draw_color(Color::RED, None);
        doc.end_page();

        doc.close();
    }

    reporter_assert!(reporter, stream.bytes_written() != 0);
}

// Port of: tests/PDFDocumentTest.cpp#L139-L146 (chrome/m156)
def_test!(SkPDF_document_tests, |reporter| {
    test_empty(reporter);
    test_abort(reporter);
    test_abort_with_file(reporter);
    test_file(reporter);
    test_close(reporter);
});

// Port of: tests/PDFDocumentTest.cpp#L148-L158 (chrome/m156)
def_test!(SkPDF_document_skbug_4734, |_r| {
    let mut stream = DynamicMemoryWStream::new();
    let mut doc = new_document(&mut stream, Some(&jpeg::metadata_with_callbacks()));
    let canvas = doc.begin_page(64.0, 64.0, None).expect("a canvas");
    canvas.scale((10000.0, 10000.0));
    canvas.translate((20.0, 10.0));
    canvas.rotate(30.0, None);
    let text = "HELLO";
    canvas.draw_str(text, (0.0, 0.0), &default_font(), &Paint::default());
});

// Port of: tests/PDFDocumentTest.cpp#L160-L169 (chrome/m156)
fn contains(result: &[u8], expectation: &str) -> bool {
    let expectation = expectation.as_bytes();
    result
        .windows(expectation.len())
        .any(|window| window == expectation)
}

// Port of: tests/PDFDocumentTest.cpp#L171-L214 (chrome/m156)
// verify that the PDFA flag does something.
def_test!(SkPDF_pdfa_document, |r| {
    let mut pdf_metadata = Metadata::default();
    pdf_metadata.title = "test document".to_owned();
    pdf_metadata.creation = DateTime {
        time_zone_minutes: 0,
        year: 1999,
        month: 12,
        day_of_week: 5,
        day: 31,
        hour: 23,
        minute: 59,
        second: 59,
    };
    pdf_metadata.pdfa = true;
    pdf_metadata.jpeg_decoder = Some(jpeg::decode);
    pdf_metadata.jpeg_encoder = Some(jpeg::encode);

    let mut buffer = DynamicMemoryWStream::new();
    {
        let mut doc = new_document(&mut buffer, Some(&pdf_metadata));
        doc.begin_page(64.0, 64.0, None)
            .expect("a canvas")
            .draw_color(Color::RED, None);
        doc.close();
    }
    let data = buffer.detach_as_vector();

    let expectations = [
        "sRGB IEC61966-2.1",
        "<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">test document",
        "<xmp:CreateDate>1999-12-31T23:59:59+00:00</xmp:CreateDate>",
        "/Subtype /XML",
        "/CreationDate (D:19991231235959+00'00')>>",
    ];
    for expectation in expectations {
        if !contains(&data, expectation) {
            errorf!(r, "PDFA expectation missing: '{}'.", expectation);
        }
    }
    pdf_metadata.producer = "phoney library".to_owned();
    pdf_metadata.pdfa = true;
    {
        let mut doc = new_document(&mut buffer, Some(&pdf_metadata));
        doc.begin_page(64.0, 64.0, None)
            .expect("a canvas")
            .draw_color(Color::RED, None);
        doc.close();
    }
    let data = buffer.detach_as_vector();

    let more_expectations = [
        "/Producer (phoney library)",
        "<pdf:Producer>phoney library</pdf:Producer>",
    ];
    for expectation in more_expectations {
        if !contains(&data, expectation) {
            errorf!(r, "PDFA expectation missing: '{}'.", expectation);
        }
    }
});

// Port of: tests/PDFDocumentTest.cpp#L217-L246 (chrome/m156)
def_test!(SkPDF_unicode_metadata, |r| {
    let mut pdf_metadata = Metadata::default();
    pdf_metadata.title = "𝓐𝓑𝓒𝓓𝓔 𝓕𝓖𝓗𝓘𝓙".to_owned(); // Out of basic multilingual plane
    pdf_metadata.author = "ABCDE FGHIJ".to_owned(); // ASCII
    pdf_metadata.subject = "αβγδε ζηθικ".to_owned(); // inside  basic multilingual plane
    pdf_metadata.pdfa = true;
    pdf_metadata.jpeg_decoder = Some(jpeg::decode);
    pdf_metadata.jpeg_encoder = Some(jpeg::encode);

    let mut w_stream = DynamicMemoryWStream::new();
    {
        let mut doc = new_document(&mut w_stream, Some(&pdf_metadata));
        doc.begin_page(612.0, 792.0, None)
            .expect("a canvas")
            .draw_color(Color::CYAN, None);
    }
    let data = w_stream.detach_as_vector();
    let expectations = [
        concat!(
            "<</Title <FEFFD835DCD0D835DCD1D835DCD2D835DCD3D835DCD40020",
            "D835DCD5D835DCD6D835DCD7D835DCD8D835DCD9>"
        ),
        "/Author (ABCDE FGHIJ)",
        "Subject <FEFF03B103B203B303B403B5002003B603B703B803B903BA>",
        "/ViewerPreferences",
        "/DisplayDocTitle true",
    ];
    for expectation in expectations {
        if !contains(&data, expectation) {
            errorf!(r, "PDF expectation missing: '{}'.", expectation);
        }
    }
});

// Port of: tests/PDFDocumentTest.cpp#L248-L265 (chrome/m156)
// Make sure we excercise the multi-page functionality without problems.
// Add this to args.gn to output the PDF to a file:
//   extra_cflags = [ "-DSK_PDF_TEST_MULTIPAGE=\"/tmp/skpdf_test_multipage.pdf\"" ]
def_test!(SkPDF_multiple_pages, |_r| {
    let n = 100;
    let mut w_stream = DynamicMemoryWStream::new();
    let mut doc = new_document(&mut w_stream, Some(&jpeg::metadata_with_callbacks()));
    for i in 0..n {
        doc.begin_page(612.0, 792.0, None)
            .expect("a canvas")
            .draw_color(
                Color4f::from_color(Color::from_argb(
                    0xFF,
                    0x00,
                    (255.0f32 * i as f32 / (n - 1) as f32) as u8,
                    0x00,
                )),
                None,
            );
    }
});

// Port of: tests/PDFDocumentTest.cpp#L267-L279 (chrome/m156)
// Test to make sure that jobs launched by PDF backend don't cause a segfault
// after calling abort().
def_test!(SkPDF_abort_jobs, |_rep| {
    let mut b = Bitmap::new();
    b.alloc_n32_pixels((612, 792), None);
    b.erase_color(Color::new(0x4F96_43A0));
    let mut metadata = jpeg::metadata_with_callbacks();
    let executor: std::sync::Arc<dyn skia_rust_core::executor::Executor> =
        std::sync::Arc::new(skia_rust_core::executor::ThreadPool::fifo(0, true));
    metadata.executor = Some(executor);
    let mut dst = NullWStream::new();
    let mut doc = new_document(&mut dst, Some(&metadata));
    doc.begin_page(612.0, 792.0, None)
        .expect("a canvas")
        .draw_image(b.as_image().expect("an image"), (0.0, 0.0), None);
    doc.abort();
});
