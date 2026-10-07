// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_892988.cpp (chrome/m156)

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;

// The root cause of this bug was that when dual source blending was not supported and we made a
// copy of the destination to perform blending we would clip the copy bounds to the current clip.
// However, it is possible for anti-aliased that are fully contained by the clip in a geometric
// sense to actually draw outside the clip in pixel space because we don't consider aa bloat when
// determining if the draw is contained by the clip.
// Port of: gm/crbug_892988.cpp#L18-L33 (chrome/m156)
crate::def_simple_gm!(crbug_892988, canvas, 256, 256, {
    let mut paint1 = Paint::default();
    paint1.set_style(Style::Stroke);
    paint1.set_stroke_width(1.0);
    paint1.set_anti_alias(true);
    canvas.draw_rect(Rect::from_ltrb(11.5, 0.5, 245.5, 245.5), &paint1);
    canvas.clip_rect(Rect::from_ltrb(12.0, 1.0, 244.0, 244.0), None, true);
    let mut paint2 = Paint::default();
    // Use src mode with a non-opaque color to produce a blend that can't be handled with
    // simple blend coefficients.
    paint2.set_color(Color::from(0xF0FF_FFFF));
    paint2.set_blend_mode(BlendMode::Src);
    paint2.set_anti_alias(true);
    canvas.draw_rect(Rect::from_ltrb(12.0, 1.0, 244.0, 244.0), &paint2);
});
