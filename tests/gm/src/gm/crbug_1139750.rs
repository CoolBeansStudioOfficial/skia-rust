// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_1139750.cpp (chrome/m156)

use skia_rust_core::color::Color;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

// Port of: gm/crbug_1139750.cpp#L14-L29 (chrome/m156)
crate::def_simple_gm_bg!(crbug_1139750, canvas, 50, 50, Color::WHITE, {
    // Draw a round-rect with a (slightly) non-square scale. This forces the GPU backend to use
    // the elliptical round-rect op. We set the stroke width to exactly double the radii, which
    // makes the inner radii exactly zero. The shader uses the inverse inner radii to compute the
    // coverage ramp, so this would end up producing infinity, and the geometry would disappear.
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_style(Style::Stroke);
    p.set_stroke_width(2.0);
    let r = Rect::from_xywh(1.0, 1.0, 19.0, 19.0);
    let rr = RRect::new_rect_xy(r, 1.0, 1.0);
    canvas.translate((10.0, 10.0));
    canvas.scale((1.47619, 1.52381));
    canvas.draw_rrect(rr, &p);
});
