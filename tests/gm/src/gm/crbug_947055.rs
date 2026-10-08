// Copyright 2019 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_947055.cpp (chrome/m156)

use skia_rust_core::color::Color;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;

// Port of: gm/crbug_947055.cpp#L13-L29 (chrome/m156)
crate::def_simple_gm_bg!(crbug_947055, canvas, 200, 50, Color::BLUE, {
    // Green 2D rectangle to highlight the red rectangle. Isn't necessary
    // to trigger problem, but helps show the extreme corner outsets.
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::GREEN);
    canvas.draw_rect(Rect::from_xywh(19.0, 7.0, 180.0, 10.0), &paint);
    // Red perspective rectangle with bad AA on Ganesh
    paint.set_color(Color::RED);
    canvas.concat(&Matrix::new_all(
        1.0, 2.4520, 19.0, 0.0, 0.3528, 9.5, 0.0, 0.0225, 1.0,
    ));
    canvas.draw_rect(Rect::from_wh(180.0, 500.0), &paint);
});
