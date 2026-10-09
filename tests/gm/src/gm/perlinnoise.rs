// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/perlinnoise.cpp (chrome/m156)

// `SkIntToScalar` of the GM sizes and offsets, which are small integers (exact in a float).
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use NoiseType::{FractalNoise as Fr, Turbulence as Tu};
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_effects::image_filters::shader_filter::Dither;
use skia_rust_effects::image_filters::{color_filter, shader};
use skia_rust_effects::perlin_noise_shader::shaders;

// Port of: gm/perlinnoise.cpp#L31-L43 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NoiseType {
    FractalNoise,
    Turbulence,
}

// Port of: gm/perlinnoise.cpp#L45-L59 (chrome/m156)
fn noise_shader(
    noise_type: NoiseType,
    base_frequency_x: f32,
    base_frequency_y: f32,
    num_octaves: usize,
    seed: f32,
    stitch_tiles: bool,
    size: ISize,
) -> Option<Shader> {
    let tile_size = stitch_tiles.then_some(size);
    let base_frequency = (base_frequency_x, base_frequency_y);
    match noise_type {
        NoiseType::FractalNoise => {
            shaders::fractal_noise(base_frequency, num_octaves, seed, tile_size)
        }
        NoiseType::Turbulence => shaders::turbulence(base_frequency, num_octaves, seed, tile_size),
    }
}

// Port of: gm/perlinnoise.cpp#L21-L23 (chrome/m156)
const K_SIZE: ISize = ISize::new(80, 80);

// Port of: gm/perlinnoise.cpp#L30-L37 (chrome/m156)
struct PerlinNoiseGm;

impl PerlinNoiseGm {
    // Port of: gm/perlinnoise.cpp#L34-L41 (chrome/m156)
    fn draw_rect(canvas: &Canvas, pt: Point, paint: &Paint, size: ISize) {
        canvas.save();
        canvas.translate((pt.x, pt.y));
        let r = Rect::from_wh(size.width as f32, size.height as f32);
        canvas.draw_rect(r, paint);
        canvas.restore();
    }

    // Port of: gm/perlinnoise.cpp#L43-L70 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    fn test(
        canvas: &Canvas,
        mut pt: Point,
        noise_type: NoiseType,
        stitch: bool,
        base_frequency: (f32, f32),
        num_octaves: usize,
        seed: f32,
        tile_size: ISize,
    ) {
        let shader = noise_shader(
            noise_type,
            base_frequency.0,
            base_frequency.1,
            num_octaves,
            seed,
            stitch,
            tile_size,
        );
        let mut paint = Paint::default();
        paint.set_shader(shader);
        if stitch {
            Self::draw_rect(canvas, pt, &paint, tile_size);
            pt.x += tile_size.width as f32;
            Self::draw_rect(canvas, pt, &paint, tile_size);
            pt.y += tile_size.height as f32;
            Self::draw_rect(canvas, pt, &paint, tile_size);
            pt.x -= tile_size.width as f32;
            Self::draw_rect(canvas, pt, &paint, tile_size);
        } else {
            Self::draw_rect(canvas, pt, &paint, K_SIZE);
        }
    }
}

impl GM for PerlinNoiseGm {
    fn name(&self) -> String {
        "perlinnoise".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(220, 620)
    }

    // Port of: gm/perlinnoise.cpp#L72-L127 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let tile = ISize::new(40, 40);
        let p = |x: f32, y: f32| Point::new(x, y);

        Self::test(canvas, p(0.0, 0.0), Fr, false, (0.1, 0.1), 0, 0.0, tile);
        Self::test(canvas, p(100.0, 0.0), Tu, false, (0.1, 0.1), 0, 0.0, tile);

        Self::test(canvas, p(0.0, 100.0), Fr, false, (0.1, 0.1), 2, 0.0, tile);
        Self::test(canvas, p(100.0, 100.0), Fr, true, (0.05, 0.1), 1, 0.0, tile);

        Self::test(canvas, p(0.0, 200.0), Tu, true, (0.1, 0.1), 1, 0.0, tile);
        Self::test(canvas, p(100.0, 200.0), Tu, false, (0.2, 0.4), 5, 0.0, tile);

        Self::test(canvas, p(0.0, 300.0), Fr, false, (0.1, 0.1), 3, 1.0, tile);
        Self::test(canvas, p(100.0, 300.0), Fr, false, (0.1, 0.1), 3, 4.0, tile);

        canvas.save();
        canvas.scale((0.75, 1.0));

        Self::test(canvas, p(0.0, 400.0), Fr, false, (0.1, 0.1), 2, 0.0, tile);
        Self::test(canvas, p(100.0, 400.0), Fr, true, (0.1, 0.05), 1, 0.0, tile);

        canvas.restore();

        // Matches Chromium test case in svg/filters/feTurbulence-tiled.svg
        Self::test(
            canvas,
            p(0.0, 500.0),
            Tu,
            true,
            (0.03, 0.03),
            1,
            0.0,
            ISize::new(50, 50),
        );

        // Matches Chromium test case in css3/filters/effect-reference.html
        Self::test(
            canvas,
            p(120.0, 500.0),
            Tu,
            false,
            (0.05, 0.05),
            2,
            0.0,
            tile,
        );
    }
}

// Port of: gm/perlinnoise.cpp#L129-L166 (chrome/m156)
struct PerlinNoiseLocalMatrixGm;

impl GM for PerlinNoiseLocalMatrixGm {
    fn name(&self) -> String {
        "perlinnoise_localmatrix".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((10.0, 10.0));

        let mut paint = Paint::default();
        paint.set_shader(noise_shader(
            NoiseType::FractalNoise,
            0.1,
            0.1,
            2,
            0.0,
            false,
            K_SIZE,
        ));

        let w = K_SIZE.width as f32;
        let h = K_SIZE.height as f32;

        let mut r = Rect::from_wh(w, h);
        canvas.draw_rect(r, &paint);

        canvas.save();
        canvas.translate((w * 5.0 / 4.0, 0.0));
        canvas.draw_rect(r, &paint);
        canvas.restore();

        canvas.save();
        canvas.translate((0.0, h + 10.0));
        canvas.scale((2.0, 2.0));
        canvas.draw_rect(r, &paint);
        canvas.restore();

        canvas.save();
        canvas.translate((w + 100.0, h + 10.0));
        canvas.scale((2.0, 2.0));
        canvas.draw_rect(r, &paint);
        canvas.restore();

        // The next row should draw the same as the previous, even though we are using a local
        // matrix instead of the canvas.

        canvas.translate((0.0, h * 2.0 + 10.0));

        let lm = Matrix::scale((2.0, 2.0));
        let shader = paint.shader().map(|s| s.with_local_matrix(&lm));
        paint.set_shader(shader);
        r.right += r.width();
        r.bottom += r.height();

        canvas.save();
        canvas.translate((0.0, h + 10.0));
        canvas.draw_rect(r, &paint);
        canvas.restore();

        canvas.save();
        canvas.translate((w + 100.0, h + 10.0));
        canvas.draw_rect(r, &paint);
        canvas.restore();
    }
}

// Demonstrate skbug.com/40045243 (Perlin noise shader doesn't rotate correctly)
// Port of: gm/perlinnoise.cpp#L168-L223 (chrome/m156)
struct PerlinNoiseRotatedGm;

const K_CELL_SIZE: ISize = ISize::new(100, 100);
const K_RECT_SIZE: ISize = ISize::new(60, 60);
const K_PAD: i32 = 10;
const K_CELLS_X: i32 = 3;
const K_CELLS_Y: i32 = 2;

impl GM for PerlinNoiseRotatedGm {
    fn name(&self) -> String {
        "perlinnoise_rotated".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(
            2 * K_PAD + K_CELLS_X * K_CELL_SIZE.width,
            2 * K_PAD + K_CELLS_Y * K_CELL_SIZE.height,
        )
    }

    // Port of: gm/perlinnoise.cpp#L184-L221 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut outline = Paint::default();
        outline.set_color(Color::BLACK);
        outline.set_stroke_width(2.0);
        outline.set_style(Style::Stroke);
        outline.set_anti_alias(true);

        let rect_to_draw = Rect::from_wh(K_RECT_SIZE.width as f32, K_RECT_SIZE.height as f32);
        let marker = Rect::from_wh(5.0, 5.0);

        let mut y_offset = K_PAD as f32;
        for noise_type in [NoiseType::FractalNoise, NoiseType::Turbulence] {
            let mut x_offset = K_PAD as f32;

            let mut p = Paint::default();
            p.set_shader(noise_shader(
                noise_type,
                0.05,
                0.05,
                1,
                0.0,
                false,
                K_RECT_SIZE,
            ));

            for rotation in [0.0f32, 10.0, 80.0] {
                let save_count = canvas.save();
                canvas.translate((x_offset, y_offset));

                canvas.draw_rect(
                    Rect::from_wh(K_CELL_SIZE.width as f32, K_CELL_SIZE.height as f32),
                    &outline,
                );

                canvas.save();

                canvas.translate((
                    K_CELL_SIZE.width as f32 / 2.0,
                    K_CELL_SIZE.height as f32 / 2.0,
                ));
                canvas.rotate(rotation, None);
                canvas.translate((
                    -(K_RECT_SIZE.width as f32) / 2.0,
                    -(K_RECT_SIZE.height as f32) / 2.0,
                ));

                canvas.draw_rect(rect_to_draw, &p);

                canvas.draw_rect(rect_to_draw, &outline);
                canvas.draw_rect(marker, &outline);

                canvas.restore_to_count(save_count);

                x_offset += K_CELL_SIZE.width as f32;
            }

            y_offset += K_CELL_SIZE.height as f32;
        }
    }
}

// Demonstrate skbug.com/40045485 (Intel GPUs show artifacts when applying perlin noise to layers)
// Port of: gm/perlinnoise.cpp#L225-L255 (chrome/m156)
struct PerlinNoiseLayeredGm;

impl GM for PerlinNoiseLayeredGm {
    fn name(&self) -> String {
        "perlinnoise_layered".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(500, 500)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let perlin = color_filter(
            Some(
                // `SkColorFilters::Matrix(const SkColorMatrix&)` defaults to `Clamp::kYes`.
                color_filters::matrix(&ColorMatrix::default(), Clamp::Yes)
                    .expect("an identity color matrix is valid"),
            ),
            shader(
                shaders::fractal_noise((0.3, 0.3), 1, 4.0, None),
                Dither::default(),
                None,
            ),
            None,
        );

        let paint = Paint::default();
        canvas.save_layer(&SaveLayerRec::default().paint(&paint));
        {
            let mut p = Paint::default();
            p.set_image_filter(perlin.clone());
            canvas.draw_paint(&p);
        }
        canvas.restore();

        canvas.save_layer(&SaveLayerRec::default());
        {
            let mut p = Paint::default();
            p.set_image_filter(perlin);
            canvas.draw_paint(&p);
        }
        canvas.restore();
    }
}

crate::def_gm!(PerlinNoiseGM, PerlinNoiseGm);
crate::def_gm!(PerlinNoiseLocalMatrixGM, PerlinNoiseLocalMatrixGm);
crate::def_gm!(PerlinNoiseRotatedGM, PerlinNoiseRotatedGm);
crate::def_gm!(PerlinNoiseLayeredGM, PerlinNoiseLayeredGm);
