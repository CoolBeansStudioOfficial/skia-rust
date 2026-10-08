// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/hardstop_gradients.cpp (chrome/m156)

// This GM presents a variety of different gradients with different tile modes. Each entry in the
// table is a rectangle with a linear gradient that spans from its left edge to its right edge.
// The rows in the table represent different color/position configurations, while the columns in
// the table represent different tile modes. In order to highlight the differences between tile
// modes, the gradient starts and ends at 30 pixel inset from either side of the rectangle.
//
//                              | Clamp         Repeat          Mirror
// _____________________________|___________________________________________
// 2-color                      | rect00        rect01          rect02
// 3-color even                 | rect10        rect11          rect12
// 3-color texture              | rect20        rect21          rect22
// 5-color hard stop            | rect30        rect31          rect32
// 4-color hard stop centered   | rect40        rect41          rect42
// 3-color hard stop 001        | rect50        rect51          rect52
// 3-color hard stop 011        | rect60        rect61          rect62
// 4-color hard stop off-center | rect70        rect71          rect72
//
// The first three rows are cases covered by pre-hard-stop code; simple 2-color gradients, 3-color
// gradients with the middle color centered, and general gradients that are rendered from a
// texture atlas.
//
// The next four rows all deal with hard stop gradients. The fourth row is a generic hard stop
// gradient, while the three subsequent rows deal with special cases of hard stop gradients;
// centered hard stop gradients (with t-values 0, 0.5, 0.5, 1), and two edge cases (with t-values
// 0, 0, 1 and 0, 1, 1). The final row has a single off-center hard stop.

// int casts and index loops mirror the C++
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::needless_range_loop
)]

use crate::prelude::*;
use skia_rust_core::color::colors;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/hardstop_gradients.cpp#L48-L60 (chrome/m156)
const WIDTH: i32 = 500;
const HEIGHT: i32 = 500;

const NUM_ROWS: usize = 8;
const NUM_COLS: usize = 3;

const CELL_WIDTH: i32 = WIDTH / NUM_COLS as i32;
const CELL_HEIGHT: i32 = HEIGHT / NUM_ROWS as i32;

const PAD_WIDTH: i32 = 3;
const PAD_HEIGHT: i32 = 3;

const RECT_WIDTH: i32 = CELL_WIDTH - (2 * PAD_WIDTH);
const RECT_HEIGHT: i32 = CELL_HEIGHT - (2 * PAD_HEIGHT);

// Port of: gm/hardstop_gradients.cpp#L62-L73 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // SkIntToScalar of small values
fn shade_rect(canvas: &Canvas, shader: Option<Shader>, cell_row: usize, cell_col: usize) {
    let mut paint = Paint::default();
    paint.set_shader(shader);

    let rect = Rect::from_xywh(
        (cell_col as i32 * CELL_WIDTH + PAD_WIDTH) as f32,
        (cell_row as i32 * CELL_HEIGHT + PAD_HEIGHT) as f32,
        RECT_WIDTH as f32,
        RECT_HEIGHT as f32,
    );

    canvas.draw_rect(rect, &paint);
}

// Port of: gm/hardstop_gradients.cpp#L75-L84 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // SkIntToScalar of small values
fn create_gradient_points(cell_row: usize, cell_col: usize) -> [Point; 2] {
    const X_OFFSET: i32 = 30;

    let x0 = (cell_col as i32 * CELL_WIDTH + PAD_WIDTH + X_OFFSET) as f32;
    let x1 = ((cell_col as i32 + 1) * CELL_WIDTH - PAD_WIDTH - X_OFFSET) as f32;
    let y = (cell_row as i32 * CELL_HEIGHT + PAD_HEIGHT + RECT_HEIGHT / 2) as f32;

    [Point::new(x0, y), Point::new(x1, y)]
}

// Port of: gm/hardstop_gradients.cpp#L86-L150 (chrome/m156)
struct HardstopGradientShaderGm;

impl GM for HardstopGradientShaderGm {
    fn name(&self) -> String {
        "hardstop_gradients".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(512, 512)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let colors = [
            colors::RED,
            colors::GREEN,
            colors::BLUE,
            colors::YELLOW,
            colors::MAGENTA,
        ];

        let row3 = [0.00f32, 0.25, 1.00];
        let row4 = [0.00f32, 0.25, 0.50, 0.50, 1.00];
        let row5 = [0.00f32, 0.50, 0.50, 1.00];
        let row6 = [0.00f32, 0.00, 1.00];
        let row7 = [0.00f32, 1.00, 1.00];
        let row8 = [0.00f32, 0.30, 0.30, 1.00];

        let positions: [Option<&[f32]>; NUM_ROWS] = [
            None,
            None,
            Some(&row3),
            Some(&row4),
            Some(&row5),
            Some(&row6),
            Some(&row7),
            Some(&row8),
        ];

        let num_gradient_colors: [usize; NUM_ROWS] = [2, 3, 3, 5, 4, 3, 3, 4];

        let tilemodes: [TileMode; NUM_COLS] = [TileMode::Clamp, TileMode::Repeat, TileMode::Mirror];

        for cell_row in 0..NUM_ROWS {
            for cell_col in 0..NUM_COLS {
                let points = create_gradient_points(cell_row, cell_col);

                let n = num_gradient_colors[cell_row];
                let pos = positions[cell_row].map(|p| &p[..n]);
                let shader = shaders::linear_gradient(
                    (points[0], points[1]),
                    &Gradient::new(
                        Colors::new(&colors[..n], pos, tilemodes[cell_col], None),
                        Interpolation::default(),
                    ),
                    None,
                );

                shade_rect(canvas, shader, cell_row, cell_col);
            }
        }
    }
}

// Port of: gm/hardstop_gradients.cpp#L171 (chrome/m156)
crate::def_gm!(HardstopGradientShaderGM, HardstopGradientShaderGm);
