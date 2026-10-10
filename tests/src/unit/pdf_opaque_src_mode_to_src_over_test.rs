// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PDFOpaqueSrcModeToSrcOverTest.cpp (chrome/m156)

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_pdf::jpeg;
use skia_rust_pdf::new_document;

use crate::{def_test, reporter_assert};

// Port of: tests/PDFOpaqueSrcModeToSrcOverTest.cpp#L18-L33 (chrome/m156)
fn run_test(out: &mut dyn WStream, mode: BlendMode, alpha: u8) {
    let mut pdf_doc = new_document(out, Some(&jpeg::metadata_with_callbacks()));
    let c = pdf_doc.begin_page(612.0, 792.0, None).expect("a canvas");
    let black = Paint::default();
    let mut background = Paint::default();
    background.set_color(Color::WHITE);
    background.set_alpha(alpha);
    background.set_blend_mode(mode);
    c.draw_rect(Rect::from_wh(612.0, 792.0), &background);
    c.draw_rect(Rect::from_xywh(36.0, 36.0, 9.0, 9.0), &black);
    c.draw_rect(Rect::from_xywh(72.0, 72.0, 468.0, 648.0), &background);
    c.draw_rect(Rect::from_xywh(108.0, 108.0, 9.0, 9.0), &black);
    pdf_doc.close();
}

// Port of: tests/PDFOpaqueSrcModeToSrcOverTest.cpp#L35-L58 (chrome/m156)
// http://crbug.com/473572
def_test!(SkPDF_OpaqueSrcModeToSrcOver, |r| {
    let mut src_mode = DynamicMemoryWStream::new();
    let mut src_over_mode = DynamicMemoryWStream::new();

    let mut alpha = 0xFF; // SK_AlphaOPAQUE
    run_test(&mut src_mode, BlendMode::Src, alpha);
    run_test(&mut src_over_mode, BlendMode::SrcOver, alpha);
    reporter_assert!(r, src_mode.bytes_written() == src_over_mode.bytes_written());
    // The two PDFs should be equal because they have an opaque alpha.

    src_mode.reset();
    src_over_mode.reset();

    alpha = 0x80;
    run_test(&mut src_mode, BlendMode::Src, alpha);
    run_test(&mut src_over_mode, BlendMode::SrcOver, alpha);
    reporter_assert!(r, src_mode.bytes_written() > src_over_mode.bytes_written());
    // The two PDFs should not be equal because they have a non-opaque alpha.
});
