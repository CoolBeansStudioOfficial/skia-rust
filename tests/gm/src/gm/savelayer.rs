// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/savelayer.cpp (chrome/m156)
//
// Only `savelayer_f16` is ported here. `savelayer_initfromprev` and `skbug_14554` decode
// mandrill images, and `save_behind` needs SkCanvasPriv::SaveBehind/DrawBehind; none of those is
// on main yet.

// The int-to-scalar cast of the layer count mirrors the C++ arithmetic (exact in f32).
#![allow(clippy::cast_precision_loss)]

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{SaveLayerFlags, SaveLayerRec};
use skia_rust_core::color::colors;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/savelayer.cpp#L179-L199 (chrome/m156), DEF_SIMPLE_GM(savelayer_f16)
crate::def_simple_gm!(savelayer_f16, canvas, 900, 300, {
    let n = 15;
    let r = Rect::from_ltrb(0.0, 0.0, 300.0, 300.0);
    let mut paint = Paint::default();

    let grad_colors = [colors::RED, colors::GREEN, colors::BLUE, colors::RED];
    let grad = Gradient::new(
        Colors::new(&grad_colors, None, TileMode::Clamp, None),
        Interpolation::default(),
    );
    // SkShaders::SweepGradient(center, colors) sweeps the full circle.
    paint.set_shader(shaders::sweep_gradient(
        (r.center_x(), r.center_y()),
        (0.0, 360.0),
        &grad,
        None,
    ));

    canvas.draw_oval(r, &paint);

    paint.set_alpha_f(1.0 / n as f32);
    paint.set_blend_mode(BlendMode::Plus);

    for flags in [SaveLayerFlags::empty(), SaveLayerFlags::F16_COLOR_TYPE] {
        canvas.translate((r.width(), 0.0));

        canvas.save_layer(&SaveLayerRec::default().flags(flags));
        for _ in 0..n {
            canvas.draw_oval(r, &paint);
        }
        canvas.restore();
    }
});
