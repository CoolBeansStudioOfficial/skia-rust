// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/AnnotationTest.cpp (chrome/m156), the PDF and SVG tests. `Annotation_NoDraw` is
// not ported yet.

use std::cell::RefCell;
use std::rc::Rc;

use skia_rust_core::data::Data;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::stream::DynamicMemoryWStream;
use skia_rust_pdf::jpeg;
use skia_rust_pdf::new_document;
use skia_rust_svg::canvas as svg_canvas;
use skia_rust_svg::canvas::Options;

use crate::{def_test, reporter_assert};

/// Returns true if data (may contain null characters) contains needle (null terminated).
// Port of: tests/AnnotationTest.cpp#L21-L30 (chrome/m156)
fn contains_string(data: &[u8], needle: &str) -> bool {
    let needle = needle.as_bytes();
    let n_size = needle.len();
    for i in 0..data.len().saturating_sub(n_size) {
        if &data[i..i + n_size] == needle {
            return true;
        }
    }
    false
}

// Port of: tests/AnnotationTest.cpp#L50-L69 (chrome/m156)
def_test!(Annotation_PdfLink, |reporter| {
    let mut out_stream = DynamicMemoryWStream::new();
    {
        let mut doc = new_document(&mut out_stream, Some(&jpeg::metadata_with_callbacks()));
        let canvas = doc.begin_page(612.0, 792.0, None);
        reporter_assert!(reporter, canvas.is_some());
        let canvas = canvas.expect("a canvas");

        let r = Rect::from_xywh(72.0, 72.0, 288.0, 72.0);
        let data = Data::new_with_cstring(Some(c"http://www.gooogle.com"));
        canvas.draw_url_annotation(r, &data);

        doc.close();
    }
    let out = out_stream.detach_as_vector();

    reporter_assert!(reporter, contains_string(&out, "/Annots "));
});

// Port of: tests/AnnotationTest.cpp#L71-L89 (chrome/m156)
def_test!(Annotation_PdfDefineNamedDestination, |reporter| {
    let mut out_stream = DynamicMemoryWStream::new();
    {
        let mut doc = new_document(&mut out_stream, Some(&jpeg::metadata_with_callbacks()));
        let canvas = doc.begin_page(612.0, 792.0, None);
        reporter_assert!(reporter, canvas.is_some());
        let canvas = canvas.expect("a canvas");

        let p = Point::new(72.0, 72.0);
        let data = Data::new_with_cstring(Some(c"example"));
        canvas.draw_named_destination_annotation(p, &data);

        doc.close();
    }
    let out = out_stream.detach_as_vector();

    reporter_assert!(reporter, contains_string(&out, "/example "));
});

// Port of: tests/AnnotationTest.cpp#L97-L112 (chrome/m156)
def_test!(Annotation_SvgLink, |reporter| {
    let out_stream = Rc::new(RefCell::new(DynamicMemoryWStream::new()));
    let bounds = Rect::from_iwh(400, 400);
    let canvas = svg_canvas::make(&bounds, out_stream.clone(), Options::default());

    let r = Rect::from_xywh(72.0, 72.0, 288.0, 72.0);
    let data = Data::new_with_cstring(Some(c"http://www.gooogle.com"));
    canvas.draw_url_annotation(r, &data);

    let out = out_stream.borrow_mut().detach_as_vector();

    reporter_assert!(
        reporter,
        contains_string(&out, "a xlink:href=\"http://www.gooogle.com\"")
    );
});

// Port of: tests/AnnotationTest.cpp#L114-L130 (chrome/m156)
def_test!(Annotation_SvgLinkNamedDestination, |reporter| {
    let out_stream = Rc::new(RefCell::new(DynamicMemoryWStream::new()));
    let bounds = Rect::from_iwh(400, 400);
    let canvas = svg_canvas::make(&bounds, out_stream.clone(), Options::default());

    let r = Rect::from_xywh(72.0, 72.0, 288.0, 72.0);
    let data = Data::new_with_cstring(Some(c"http://www.gooogle.com/#NamedDestination"));
    canvas.draw_link_destination_annotation(r, &data);

    let out = out_stream.borrow_mut().detach_as_vector();

    reporter_assert!(
        reporter,
        contains_string(
            &out,
            "a xlink:href=\"http://www.gooogle.com/#NamedDestination\""
        )
    );
});
