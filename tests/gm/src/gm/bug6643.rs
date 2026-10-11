// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bug6643.cpp (chrome/m156)

//! A sweep gradient, drawn through a picture shader that tiles the picture of a paint.

use crate::prelude::*;
use skia_rust_core::color::Color4f;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::interpolation::InPremul;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_raster::picture_shader::PictureShaderExt;

// Port of: gm/bug6643.cpp#L20-L39 (chrome/m156), DEF_SIMPLE_GM(bug6643, canvas, 200, 200)
crate::def_simple_gm!(bug6643, canvas, 200, 200, {
    let colors = [
        Color4f::new(0.0, 0.0, 0.0, 0.0), // kTransparent
        Color4f::new(0.0, 1.0, 0.0, 1.0), // kGreen
        Color4f::new(0.0, 0.0, 0.0, 0.0), // kTransparent
    ];

    let mut p = Paint::default();
    p.set_anti_alias(true);
    let gradient = Gradient::new(
        Colors::new(&colors, None, TileMode::Clamp, None),
        Interpolation {
            in_premul: InPremul::Yes,
            ..Interpolation::default()
        },
    );
    p.set_shader(gradient_shaders::sweep_gradient(
        Point::new(100.0, 100.0),
        (0.0, 360.0),
        &gradient,
        None,
    ));

    let mut recorder = PictureRecorder::new();
    recorder
        .begin_recording(Rect::from_wh(200.0, 200.0), false)
        .draw_paint(&p);

    let picture = recorder.finish_recording_as_picture(None);
    p.set_shader(picture.and_then(|picture| {
        picture.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            FilterMode::Nearest,
            None,
            None,
        )
    }));
    canvas.draw_color(Color::WHITE, None);
    canvas.draw_paint(&p);
});
