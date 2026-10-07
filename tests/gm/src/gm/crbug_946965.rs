// Copyright 2019 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_946965.cpp (chrome/m156)

use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

// Exposed a bug in ellipse rendering where the radii were wrong under 90 degree rotation.
// Port of: gm/crbug_946965.cpp#L13-L27 (chrome/m156)
crate::def_simple_gm!(crbug_946965, canvas, 75, 150, {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    canvas.translate((25.0, 80.0));
    canvas.rotate(90.0, None);
    canvas.scale((1.5, 1.0));
    let rrect = RRect::new_rect_xy(Rect::from_ltrb(-20.0, -5.0, 20.0, 5.0), 10.0, 10.0);
    canvas.draw_rrect(rrect, &paint);
    canvas.translate((0.0, -20.0));
    paint.set_stroke_width(3.0);
    paint.set_style(Style::Stroke);
    canvas.draw_rrect(rrect, &paint);
});
