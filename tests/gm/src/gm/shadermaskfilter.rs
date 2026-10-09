// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/shadermaskfilter.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_effects::shader_mask_filter;

// Port of: gm/shadermaskfilter.cpp#L43-L49 (chrome/m156), make_shader
fn make_shader(r: Rect) -> Option<skia_rust_core::shader::Shader> {
    let colors = [
        Color4f::new(0.0, 0.0, 0.0, 0.0),
        Color4f::from(Color::WHITE),
    ];
    let gradient = Gradient::new(
        Colors::new(&colors, None, TileMode::Repeat, None),
        Interpolation::default(),
    );
    gradient_shaders::linear_gradient(((r.left, r.top), (r.right, r.bottom)), &gradient, None)
}

// Port of: gm/shadermaskfilter.cpp#L51-L63 (chrome/m156), shadermaskfilter_gradient
crate::def_simple_gm!(shadermaskfilter_gradient, canvas, 512, 512, {
    let r = Rect::from_ltrb(0.0, 0.0, 100.0, 150.0);
    let Some(shader) = make_shader(r) else {
        return;
    };
    let mf = shader_mask_filter::new(shader);

    canvas.translate((20.0, 20.0));
    canvas.scale((2.0, 2.0));

    let mut paint = Paint::default();
    paint.set_mask_filter(Some(mf));
    paint.set_color(Color::RED);
    paint.set_anti_alias(true);
    canvas.draw_oval(r, &paint);
});
