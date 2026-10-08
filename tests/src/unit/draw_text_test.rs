// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/DrawTextTest.cpp (chrome/m156)
//
// Not ported yet:
// - `DrawText_dashout`: its second draw uses a dash path effect, whose strike descriptor entry
//   (`kEffects_SkDescriptorTag`, `writeFlattenable`) is not ported, so the glyph painter panics on
//   it. The first draw alone would not make the test meaningful.
// - `DrawText_noglyphs`: it draws `SkTextBlob::MakeFromText` (`TextBlob`, T15a).

use skia_rust_core::font::Edging;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_font;

use crate::def_test;

// Test drawing text at some unusual coordinates.
// We measure success by not crashing or asserting.
// Port of: tests/DrawTextTest.cpp#L117-L132 (chrome/m156)
def_test!(DrawText_weirdCoordinates, |_reporter| {
    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul((10, 10), None), None, None).unwrap();
    let canvas = surface.canvas();

    let font = default_font();
    let oddballs = [0.0_f32, f32::INFINITY, f32::NAN, 34_359_738_368.0];
    for x in oddballs {
        canvas.draw_str("a", (x, 0.0), &font, &Paint::default());
        canvas.draw_str("a", (-x, 0.0), &font, &Paint::default());
    }
    for y in oddballs {
        canvas.draw_str("a", (0.0, y), &font, &Paint::default());
        canvas.draw_str("a", (0.0, -y), &font, &Paint::default());
    }
});

// Test drawing text with some unusual matrices.
// We measure success by not crashing or asserting.
// Port of: tests/DrawTextTest.cpp#L136-L169 (chrome/m156)
def_test!(DrawText_weirdMatricies, |_reporter| {
    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul((100, 100), None), None, None).unwrap();
    let canvas = surface.canvas();

    let mut font = default_font();
    font.set_edging(Edging::SubpixelAntiAlias);

    // (textSize, matrix) pairs.
    let test_cases: [(f32, [f32; 9]); 10] = [
        // 2x2 singular
        (10.0, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [1.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        // See https://bugzilla.mozilla.org/show_bug.cgi?id=1305085 .
        (1.0, [10.0, 20.0, 0.0, 20.0, 40.0, 0.0, 0.0, 0.0, 1.0]),
    ];
    for (text_size, m) in test_cases {
        font.set_size(text_size);
        let mut mat = Matrix::default();
        mat.set_all(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8]);
        canvas.set_matrix(&M44::from(mat));
        canvas.draw_str("Hamburgefons", (10.0, 10.0), &font, &Paint::default());
    }
});
