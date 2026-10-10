// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PDFMetadataAttributeTest.cpp (chrome/m156)

#![allow(clippy::field_reassign_with_default)] // the tests assign the fields one by one, as the C++ does

use skia_rust_core::stream::DynamicMemoryWStream;
use skia_rust_pdf::jpeg;
use skia_rust_pdf::metadata::Metadata;
use skia_rust_pdf::new_document;
use skia_rust_pdf::utils::get_date_time;

use crate::{def_test, errorf};

// Port of: tests/PDFMetadataAttributeTest.cpp#L18-L68 (chrome/m156)
def_test!(SkPDF_Metadata, |r| {
    let now = get_date_time();
    let mut metadata = Metadata::default();
    metadata.title = "A1".to_owned();
    metadata.author = "A2".to_owned();
    metadata.subject = "A3".to_owned();
    metadata.keywords = "A4".to_owned();
    metadata.creator = "A5".to_owned();
    metadata.creation = now;
    metadata.modified = now;
    metadata.jpeg_decoder = Some(jpeg::decode);
    metadata.jpeg_encoder = Some(jpeg::encode);

    let mut pdf = DynamicMemoryWStream::new();
    {
        let mut doc = new_document(&mut pdf, Some(&metadata));
        let _ = doc.begin_page(612.0, 792.0, None);
        doc.close();
    }
    let data = pdf.detach_as_vector();
    let expectations = [
        "/Title (A1)",
        "/Author (A2)",
        "/Subject (A3)",
        "/Keywords (A4)",
        "/Creator (A5)",
        "/Producer (Skia/PDF ",
        "/CreationDate (D:",
        "/ModDate (D:",
    ];
    for expectation in expectations {
        let expectation_bytes = expectation.as_bytes();
        let found = data
            .windows(expectation_bytes.len())
            .any(|window| window == expectation_bytes);
        if !found {
            errorf!(r, "expectation missing: '{}'.", expectation);
        }
    }
});
