// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/clipshader.cpp (chrome/m156)

use skia_rust_core::color::colors;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::surfaces;

// This tests using clip shader and then changing the canvas matrix before drawing. It also verifies
// that we don't incorrectly disable linear filtering of a clip image shader.
// Port of: gm/clipshader.cpp#L22-L61 (chrome/m156), clipshadermatrix
crate::def_simple_gm!(clipshadermatrix, canvas, 145, 128, {
    let mut clip_surface =
        surfaces::raster(&ImageInfo::new_a8((70, 60)), None, None).expect("a clip surface");
    // Hard edged oval clip
    clip_surface
        .canvas()
        .draw_oval(Rect::from_xywh(0.0, 10.0, 64.0, 44.0), &Paint::default());
    let clip_shader = clip_surface
        .image_snapshot()
        .expect("a clip snapshot")
        .to_shader(
            (TileMode::Decal, TileMode::Decal),
            SamplingOptions::from(FilterMode::Linear),
            None,
        )
        .expect("a clip shader");

    canvas.translate((5.0, 0.0));
    for tx in [0.0_f32, 68.5] {
        for ty in [0.0_f32, 66.5] {
            canvas.save();

            canvas.translate((tx, ty));
            canvas.clip_shader(clip_shader.clone(), None);
            canvas.translate((-tx, -ty));

            let mut m = Matrix::new_identity();
            m.set_skew((0.03, 0.0), None);
            m.set_persp_y(0.0007);
            m.set_persp_x(-0.002);
            m.set_scale_x(1.2);
            m.set_scale_y(0.8);
            m.pre_rotate(30.0, None);
            canvas.concat(&m);

            let mut center = Point::new(64.0, 64.0);
            let m_inv = m.invert().expect("an invertible matrix");
            center = m_inv.map_point(center);
            let gradient_colors = [
                colors::YELLOW,
                colors::GREEN,
                colors::BLUE,
                colors::MAGENTA,
                colors::CYAN,
                colors::YELLOW,
            ];
            let gradient = shaders::radial_gradient(
                (center, 32.0),
                &Gradient::new(
                    Colors::new(&gradient_colors, None, TileMode::Mirror, None),
                    Interpolation::default(),
                ),
                None,
            );

            let mut paint = Paint::default();
            paint.set_shader(gradient);
            canvas.draw_paint(&paint);

            canvas.restore();
        }
    }
});
