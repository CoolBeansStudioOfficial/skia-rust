// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/hardstop_gradients_many.cpp (chrome/m156)

// This GM presents different gradients with an increasing number of hardstops, from 1 to 100.

use crate::prelude::*;
use skia_rust_core::color::colors;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/hardstop_gradients_many.cpp#L28-L33 (chrome/m156)
const K_WIDTH: i32 = 1000;
const K_HEIGHT: i32 = 2000;
const K_NUM_ROWS: i32 = 100;
const K_CELL_HEIGHT: i32 = K_HEIGHT / K_NUM_ROWS;
const K_PAD_HEIGHT: i32 = 1;
const K_RECT_HEIGHT: i32 = K_CELL_HEIGHT - (2 * K_PAD_HEIGHT);

// Port of: gm/hardstop_gradients_many.cpp#L35-L77 (chrome/m156)
struct HardstopGradientsManyGm;

impl GM for HardstopGradientsManyGm {
    fn name(&self) -> String {
        "hardstop_gradients_many".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(K_WIDTH, K_HEIGHT)
    }

    #[allow(clippy::cast_precision_loss)] // SkScalar(int) of small values
    fn on_draw(&mut self, canvas: &Canvas) {
        let points = [
            Point::new(0.0, (K_RECT_HEIGHT / 2) as f32),
            Point::new(K_WIDTH as f32, (K_RECT_HEIGHT / 2) as f32),
        ];

        let mut colors: Vec<Color4f> = Vec::new();
        let mut positions: Vec<f32>;

        for row in 1..=K_NUM_ROWS {
            // Assemble a gradient containing a blue-to-white blend, repeated N times per row.
            colors.push(colors::BLUE);
            colors.push(colors::WHITE);

            positions = vec![0.0];
            for pos in 1..row {
                let place = (pos as f32) / (row as f32);
                positions.push(place);
                positions.push(place);
            }
            positions.push(1.0);
            assert_eq!(positions.len(), colors.len());

            // Draw it.
            let shader = shaders::linear_gradient(
                (points[0], points[1]),
                &Gradient::new(
                    Colors::new(&colors, Some(&positions), TileMode::Clamp, None),
                    Interpolation::default(),
                ),
                None,
            );
            let mut paint = Paint::default();
            paint.set_shader(shader);
            canvas.draw_rect(
                Rect::from_xywh(
                    0.0,
                    K_PAD_HEIGHT as f32,
                    K_WIDTH as f32,
                    K_RECT_HEIGHT as f32,
                ),
                &paint,
            );

            canvas.translate((0.0, K_CELL_HEIGHT as f32));
        }
    }
}

// Port of: gm/hardstop_gradients_many.cpp#L82 (chrome/m156)
crate::def_gm!(HardstopGradientsManyGM, HardstopGradientsManyGm);
