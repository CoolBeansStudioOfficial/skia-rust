// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/Skbug5221.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::canvas::Canvas;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_font;

use crate::def_test;

// This passes by not crashing.
// Port of: tests/Skbug5221.cpp#L23-L27 (chrome/m156)
fn test(canvas: &Canvas) {
    canvas.scale((63.0, 0.0));
    // `SkCanvas::drawString`, which is `drawSimpleText` with `SkTextEncoding::kUTF8`.
    canvas.draw_simple_text(
        "A",
        TextEncoding::UTF8,
        (50.0, 50.0),
        &default_font(),
        &Paint::default(),
    );
}

// Port of: tests/Skbug5221.cpp#L29-L33 (chrome/m156)
def_test!(skbug5221, |_reporter| {
    let mut surface = surfaces::raster_n32_premul((256, 256)).expect("surface");
    test(surface.canvas());
});
