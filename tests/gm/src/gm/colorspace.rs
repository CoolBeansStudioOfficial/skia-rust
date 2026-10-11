// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/colorspace.cpp (chrome/m156)
//
// skia-rust: `draw_colorspace_gm` only has a result when the canvas is colour-managed. The harness
// canvas has no colour space, so the GM draws its "only makes sense with color-managed drawing"
// message, which is the only branch that can run here. The colour-managed branches (which call
// `SkImage::makeColorSpace` and `SkCanvas::makeSurface`) are not ported, and panic if reached.

use crate::prelude::*;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_tools::font_tool_utils::default_portable_font;

const W: f32 = 128.0;
const H: f32 = 128.0;

// Port of: gm/colorspace.cpp#L69-L79 (chrome/m156), draw_colorspace_gm: the no-colour-space path
fn draw_colorspace_gm(canvas: &Canvas) {
    let font = default_portable_font();
    if canvas.image_info().color_space().is_none() {
        canvas.draw_simple_text(
            b"This GM only makes sense with color-managed drawing.",
            TextEncoding::UTF8,
            (W, H),
            &font,
            &Paint::default(),
        );
        return;
    }
    panic!("the colour-managed branches of colorspace.cpp are not ported");
}

// Port of: gm/colorspace.cpp#L118-L120 (chrome/m156)
crate::def_simple_gm!(colorspace, canvas, 128 * 7, 128 * 5, {
    draw_colorspace_gm(canvas);
});

// Port of: gm/colorspace.cpp#L122-L124 (chrome/m156)
crate::def_simple_gm!(colorspace2, canvas, 128 * 7, 128 * 5, {
    draw_colorspace_gm(canvas);
});
