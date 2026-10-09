// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/shadertext3.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::color_priv::ColorConverter;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::int_to_scalar;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/shadertext3.cpp#L27-L44 (chrome/m156), makebm
fn makebm(bm: &mut Bitmap, w: i32, h: i32) {
    bm.alloc_n32_pixels((w, h), None);
    bm.erase_color(Color::new(0x0000_0000));

    {
        let canvas = CoreCanvas::from_bitmap(bm, None).expect("a canvas");
        let s = int_to_scalar(w.min(h));
        let pts0 = [Point::new(0.0, 0.0), Point::new(s, s)];
        let pts1 = [Point::new(s / 2.0, 0.0), Point::new(s / 2.0, s)];
        let pos: [f32; 3] = [0.0, 1.0 / 2.0, 1.0];

        let mut paint = Paint::default();
        let conv0 = ColorConverter::new(&[
            Color::new(0x80F0_0080),
            Color::new(0xF0F0_8000),
            Color::new(0x8000_80F0),
        ]);
        paint.set_shader(gradient_shaders::linear_gradient(
            (pts0[0], pts0[1]),
            &Gradient::new(
                Colors::new(conv0.colors4f(), Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));
        canvas.draw_paint(&paint);
        let conv1 = ColorConverter::new(&[
            Color::new(0xF080_00F0),
            Color::new(0x8080_F000),
            Color::new(0xF000_F080),
        ]);
        paint.set_shader(gradient_shaders::linear_gradient(
            (pts1[0], pts1[1]),
            &Gradient::new(
                Colors::new(conv1.colors4f(), Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));
        canvas.draw_paint(&paint);
    }
}

// Port of: gm/shadertext3.cpp#L46-L52 (chrome/m156), LabeledMatrix
#[allow(dead_code)] // the C++ struct is declared but never used by the GM
struct LabeledMatrix {
    matrix: Matrix,
    label: &'static str,
}

// Port of: gm/shadertext3.cpp#L55 (chrome/m156), kPointSize
const K_POINT_SIZE: i32 = 300;

// Port of: gm/shadertext3.cpp#L110 (chrome/m156), kTileModes
const K_TILE_MODES: [TileMode; 2] = [TileMode::Repeat, TileMode::Mirror];

// Port of: gm/shadertext3.cpp#L57-L120 (chrome/m156), ShaderText3GM
struct ShaderText3Gm {
    bmp: Bitmap,
    bg_color: Color,
}

impl ShaderText3Gm {
    // Port of: gm/shadertext3.cpp#L59-L61 (chrome/m156), the constructor
    fn new() -> Self {
        Self {
            bmp: Bitmap::default(),
            bg_color: Color::new(0xFFDD_DDDD),
        }
    }
}

impl GM for ShaderText3Gm {
    // Port of: gm/shadertext3.cpp#L63 (chrome/m156), getName
    fn name(&self) -> String {
        "shadertext3".to_owned()
    }

    // Port of: gm/shadertext3.cpp#L65 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(820, 930)
    }

    fn bg_color(&self) -> Color {
        self.bg_color
    }

    // Port of: gm/shadertext3.cpp#L68-L70 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        makebm(&mut self.bmp, K_POINT_SIZE / 4, K_POINT_SIZE / 4);
    }

    // Port of: gm/shadertext3.cpp#L72-L118 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut bmp_paint = Paint::default();
        bmp_paint.set_anti_alias(true);
        bmp_paint.set_alpha_f(0.5);
        let sampling = SamplingOptions::from(FilterMode::Linear);

        let image = self.bmp.as_image().expect("the bitmap is allocated");
        canvas.draw_image_with_sampling_options(&image, (5.0, 5.0), sampling, Some(&bmp_paint));

        let font = Font::from_size(default_portable_typeface(), int_to_scalar(K_POINT_SIZE));
        let mut outline_paint = Paint::default();
        outline_paint.set_style(Style::Stroke);
        outline_paint.set_stroke_width(0.0);

        canvas.translate((15.0, 15.0));

        // draw glyphs scaled up
        canvas.scale((2.0, 2.0));



        // position the baseline of the first run
        canvas.translate((0.0, 0.75 * int_to_scalar(K_POINT_SIZE)));

        canvas.save();
        let mut i = 0;
        for &tm0 in &K_TILE_MODES {
            for &tm1 in &K_TILE_MODES {
                let mut local_m = Matrix::default();
                local_m.set_translate((5.0, 5.0));
                local_m.post_rotate(20.0, None);
                local_m.post_scale((1.15, 0.85), None);

                let mut fill_paint = Paint::default();
                fill_paint.set_anti_alias(true);
                fill_paint.set_shader(self.bmp.to_shader((tm0, tm1), sampling, &local_m));

                let text = "B";
                canvas.draw_str(text, (0.0, 0.0), &font, &fill_paint);
                canvas.draw_str(text, (0.0, 0.0), &font, &outline_paint);
                let (w, _) = font.measure_text(text.as_bytes(), TextEncoding::UTF8, None);
                canvas.translate((w + 10.0, 0.0));
                i += 1;
                if i % 2 == 0 {
                    canvas.restore();
                    canvas.translate((0.0, 0.75 * int_to_scalar(K_POINT_SIZE)));
                    canvas.save();
                }
            }
        }
        canvas.restore();
    }
}

// Port of: gm/shadertext3.cpp#L138 (chrome/m156)
crate::def_gm!(ShaderText3GM, ShaderText3Gm::new());
