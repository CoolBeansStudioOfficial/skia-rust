// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/rrectclipdrawpaint.cpp (chrome/m156)

// Exercises code in skgpu::V1::SurfaceDrawContext that attempts to replace a rrect clip/draw
// paint with draw rrect.

use crate::prelude::*;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::Color4f;
use skia_rust_core::color::colors;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/rrectclipdrawpaint.cpp#L16-L62 (chrome/m156), DEF_SIMPLE_GM(rrect_clip_draw_paint)
crate::def_simple_gm!(rrect_clip_draw_paint, canvas, 256, 256, {
    let rrect = RRect::new_rect_xy(Rect::from_xywh(10.0, 10.0, 236.0, 236.0), 30.0, 40.0);

    let mut p = Paint::default();
    p.set_color(Color::RED);

    let mut zoom_out = Matrix::new_identity();
    zoom_out.set_scale((0.7, 0.7), Point::new(128.0, 128.0));

    let layer_rect = Rect::from_wh(256.0, 256.0);
    canvas.save_layer(&SaveLayerRec::default().bounds(&layer_rect));
    canvas.clip_rrect(rrect, None, true);
    canvas.draw_paint(&p);
    canvas.restore();

    canvas.concat(&zoom_out);
    p.set_color(Color::BLUE);
    canvas.save_layer(&SaveLayerRec::default().bounds(&layer_rect));
    canvas.clip_rrect(rrect, None, false);
    canvas.draw_paint(&p);
    canvas.restore();

    let pts = [Point::new(0.0, 0.0), Point::new(256.0, 256.0)];
    let k_colors1 = [colors::CYAN, colors::GREEN];
    let grad1 = Gradient::new(
        Colors::new(&k_colors1, None, TileMode::Clamp, None),
        Interpolation::default(),
    );
    p.set_shader(shaders::linear_gradient((pts[0], pts[1]), &grad1, None));
    canvas.concat(&zoom_out);
    canvas.save_layer(&SaveLayerRec::default().bounds(&layer_rect));
    canvas.clip_rrect(rrect, None, true);
    canvas.draw_paint(&p);
    canvas.restore();

    // compat_gray = 0x88 / 255.f (136 / 255 as f32)
    let compat_gray: f32 = 136.0 / 255.0;
    let k_colors2 = [
        colors::MAGENTA,
        Color4f::new(compat_gray, compat_gray, compat_gray, 1.0),
    ];
    let grad2 = Gradient::new(
        Colors::new(&k_colors2, None, TileMode::Clamp, None),
        Interpolation::default(),
    );
    p.set_shader(shaders::radial_gradient(
        (Point::new(128.0, 128.0), 128.0),
        &grad2,
        None,
    ));
    canvas.concat(&zoom_out);
    canvas.save_layer(&SaveLayerRec::default().bounds(&layer_rect));
    canvas.clip_rrect(rrect, None, false);
    canvas.draw_paint(&p);
    canvas.restore();
});
