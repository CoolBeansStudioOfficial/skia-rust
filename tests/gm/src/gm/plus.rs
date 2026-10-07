// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/plus.cpp (chrome/m156)

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::Color;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::Rect;

// Port of: gm/plus.cpp#L14-L53 (chrome/m156)
crate::def_simple_gm!(PlusMergesAA, canvas, 256, 256, {
    let mut p = Paint::default();
    p.set_color(Color::RED);
    p.set_anti_alias(true); //  <-- crucial to the test that we use AA

    // Draw a two red squares.
    canvas.draw_rect(Rect::from_wh(100.0, 100.0), &p);
    canvas.draw_rect(Rect::from_xywh(150.0, 0.0, 100.0, 100.0), &p);

    p.set_color(Color::from(0xf000_ff00));

    // We'll draw a green square on top of each using two triangles.
    let upper_left = PathBuilder::new()
        .line_to((100.0, 0.0))
        .line_to((0.0, 100.0))
        .line_to((0.0, 0.0))
        .detach();

    let bottom_right = PathBuilder::new()
        .move_to((100.0, 0.0))
        .line_to((100.0, 100.0))
        .line_to((0.0, 100.0))
        .line_to((100.0, 0.0))
        .detach();

    // The left square is drawn simply with SrcOver.  It will show a red seam.
    canvas.draw_path(&upper_left, &p);
    canvas.draw_path(&bottom_right, &p);

    // Using Plus on the right should merge the AA of seam together completely covering the red.
    canvas.save_layer(&SaveLayerRec::default());
    p.set_blend_mode(BlendMode::Plus);
    canvas.translate((150.0, 0.0));
    canvas.draw_path(&upper_left, &p);
    canvas.draw_path(&bottom_right, &p);
    canvas.restore();
});
