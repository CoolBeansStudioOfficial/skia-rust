// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/shaderpath.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: gm/shaderpath.cpp#L16-L37 (chrome/m156), makebm
fn makebm(bm: &mut Bitmap, w: i32, h: i32) {
    bm.alloc_n32_pixels((w, h), None);
    bm.erase_color(Color::TRANSPARENT);
    let canvas = Canvas::from_bitmap(bm, None).expect("a canvas");
    let s = int_to_scalar(w.min(h));
    let pts0 = [Point::new(0.0, 0.0), Point::new(s, s)];
    let pts1 = [Point::new(s / 2.0, 0.0), Point::new(s / 2.0, s)];
    let pos: [f32; 3] = [0.0, 1.0 / 2.0, 1.0];

    let mut paint = Paint::default();
    let conv0 = color_converter(&[
        Color::from(0x80F0_0080),
        Color::from(0xF0F0_8000),
        Color::from(0x8000_80F0),
    ]);
    paint.set_shader(shaders::linear_gradient(
        (pts0[0], pts0[1]),
        &Gradient::new(
            Colors::new(&conv0, Some(&pos[..]), TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));
    canvas.draw_paint(&paint);

    let conv1 = color_converter(&[
        Color::from(0xF080_00F0),
        Color::from(0x8080_F000),
        Color::from(0xF000_F080),
    ]);
    paint.set_shader(shaders::linear_gradient(
        (pts1[0], pts1[1]),
        &Gradient::new(
            Colors::new(&conv1, Some(&pos[..]), TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));
    canvas.draw_paint(&paint);
}

// `SkColorConverter`: `SkColor4f::FromColor` of each colour.
// Port of: src/core/SkColorPriv.h (chrome/m156), SkColorConverter::colors4f
fn color_converter(src: &[Color]) -> Vec<Color4f> {
    src.iter().map(|&c| Color4f::from_color(c)).collect()
}

const K_POINT_SIZE: i32 = 300;

// Port of: gm/shaderpath.cpp#L46-L107 (chrome/m156), ShaderPathGM
struct ShaderPathGm {
    bmp: Bitmap,
}

impl ShaderPathGm {
    fn new() -> Self {
        Self {
            bmp: Bitmap::new(),
        }
    }
}

impl GM for ShaderPathGm {
    fn name(&self) -> String {
        "shaderpath".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(820, 930)
    }

    // Port of: gm/shaderpath.cpp#L49-L51 (chrome/m156), setBGColor(0xFFDDDDDD)
    fn bg_color(&self) -> Color {
        Color::from(0xFFDD_DDDD)
    }

    // Port of: gm/shaderpath.cpp#L56-L58 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        makebm(&mut self.bmp, K_POINT_SIZE / 4, K_POINT_SIZE / 4);
    }

    // Port of: gm/shaderpath.cpp#L60-L100 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut bmp_paint = Paint::default();
        bmp_paint.set_anti_alias(true);
        bmp_paint.set_alpha_f(0.5);
        let sampling = SamplingOptions::from(FilterMode::Linear);
        if let Some(image) = self.bmp.as_image() {
            canvas.draw_image_with_sampling_options(
                &image,
                (5.0, 5.0),
                sampling,
                Some(&bmp_paint),
            );
        }

        let mut outline_paint = Paint::default();
        outline_paint.set_style(Style::Stroke);
        outline_paint.set_stroke_width(0.0);

        canvas.translate((15.0, 15.0));
        canvas.scale((2.0, 2.0));

        let tile_modes = [TileMode::Repeat, TileMode::Mirror];

        // position the baseline of the first path
        canvas.translate((0.0, 2.25));
        let mut builder = PathBuilder::new();
        builder
            .move_to((0.0, 40.0))
            .cubic_to((10.0, 70.0), (20.0, 10.0), (30.0, 40.0));
        let path: Path = builder.detach();

        canvas.save();
        let mut i = 0;
        for &tm0 in &tile_modes {
            for &tm1 in &tile_modes {
                let mut local_m = Matrix::new_identity();
                local_m.set_translate((5.0, 5.0));
                local_m.post_rotate(20.0, None);
                local_m.post_scale((1.15, 0.85), None);

                let mut fill_paint = Paint::default();
                fill_paint.set_anti_alias(true);
                fill_paint.set_shader(self.bmp.to_shader(
                    (tm0, tm1),
                    sampling,
                    &local_m,
                ));
                canvas.draw_path(&path, &fill_paint);
                canvas.draw_path(&path, &outline_paint);
                canvas.translate((50.0, 0.0));
                i += 1;
                if i % 2 == 0 {
                    canvas.restore();
                    canvas.translate((0.0, 22.5));
                    canvas.save();
                }
            }
        }
        canvas.restore();
    }
}

// Port of: gm/shaderpath.cpp#L139 (chrome/m156)
crate::def_gm!(ShaderPathGM, ShaderPathGm::new());
