// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/analytic_gradients.cpp (chrome/m156)

// This GM presents a variety of gradients meant to test the correctness of the analytic unrolled
// binary gradient colorizer, which can handle arbitrary gradients with 1 to 8 interpolation
// intervals. These intervals can be either hardstops or smooth color transitions.
//
// It produces an image similar to that of GM_hardstop_gradients, but is arranged as follows:
//
//            | Clamp          |
//            |________________|
//            | M1  M2  M3  M4 |
// ___________|________________|
//      1     |
//      2     |
//      3     |
//      4     |
//      5     |
//      6     |
//      7     |
//      8     |
// The M-modes are different ways of interlveaving hardstops with smooth transitions:
//   - M1 = All smooth transitions
//   - M2 = All hard stops
//   - M5 = Alternating smooth then hard
//   - M6 = Alternating hard then smooth
//
// Only clamping is tested since this is focused more on within the interpolation region behavior,
// compared to overall behavior.

// int casts and index loops mirror the C++
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::needless_range_loop
)]

use crate::prelude::*;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// All positions must be divided by the target interval count, which will produce the expected
// normalized position array for that interval number (assuming an appropriate color count is
// provided).
// Port of: gm/analytic_gradients.cpp#L47-L50 (chrome/m156)
const M1_POSITIONS: &[i32] = &[0, 1, 2, 3, 4, 5, 6, 7, 8];
const M2_POSITIONS: &[i32] = &[0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8];
const M3_POSITIONS: &[i32] = &[0, 1, 2, 2, 3, 4, 4, 5, 6, 6, 7, 8];
const M4_POSITIONS: &[i32] = &[0, 1, 1, 2, 3, 3, 4, 5, 5, 6, 7, 7, 8];

// Color count = index of first occurrence of interval count value in Mx_POSITIONS array.
// Port of: gm/analytic_gradients.cpp#L52-L60 (chrome/m156)
const INT1_COLOR_COUNTS: [usize; 4] = [2, 2, 2, 2];
const INT2_COLOR_COUNTS: [usize; 4] = [3, 4, 3, 4];
const INT3_COLOR_COUNTS: [usize; 4] = [4, 6, 5, 5];
const INT4_COLOR_COUNTS: [usize; 4] = [5, 8, 6, 7];
const INT5_COLOR_COUNTS: [usize; 4] = [6, 10, 8, 8];
const INT6_COLOR_COUNTS: [usize; 4] = [7, 12, 9, 10];
const INT7_COLOR_COUNTS: [usize; 4] = [8, 14, 11, 11];
const INT8_COLOR_COUNTS: [usize; 4] = [9, 16, 12, 13];

// Cycle through defined colors for positions 0 through 8.
// Port of: gm/analytic_gradients.cpp#L62-L72 (chrome/m156)
const COLORS: [Color; 9] = [
    Color::DARK_GRAY,
    Color::RED,
    Color::YELLOW,
    Color::GREEN,
    Color::CYAN,
    Color::BLUE,
    Color::MAGENTA,
    Color::BLACK,
    Color::LIGHT_GRAY,
];

// Port of: gm/analytic_gradients.cpp#L74-L83 (chrome/m156)
const INTERVAL_COLOR_COUNTS: [&[usize; 4]; 8] = [
    &INT1_COLOR_COUNTS,
    &INT2_COLOR_COUNTS,
    &INT3_COLOR_COUNTS,
    &INT4_COLOR_COUNTS,
    &INT5_COLOR_COUNTS,
    &INT6_COLOR_COUNTS,
    &INT7_COLOR_COUNTS,
    &INT8_COLOR_COUNTS,
];
const COLOR_COUNT: usize = COLORS.len();

// Port of: gm/analytic_gradients.cpp#L86-L91 (chrome/m156)
const M_POSITIONS: [&[i32]; 4] = [M1_POSITIONS, M2_POSITIONS, M3_POSITIONS, M4_POSITIONS];

// Port of: gm/analytic_gradients.cpp#L93-L106 (chrome/m156)
const WIDTH: i32 = 500;
const HEIGHT: i32 = 500;

const NUM_ROWS: usize = 8;
const NUM_COLS: usize = 4;

const CELL_WIDTH: i32 = WIDTH / NUM_COLS as i32;
const CELL_HEIGHT: i32 = HEIGHT / NUM_ROWS as i32;

const PAD_WIDTH: i32 = 3;
const PAD_HEIGHT: i32 = 3;

const RECT_WIDTH: i32 = CELL_WIDTH - (2 * PAD_WIDTH);
const RECT_HEIGHT: i32 = CELL_HEIGHT - (2 * PAD_HEIGHT);

// Port of: gm/analytic_gradients.cpp#L108-L120 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // SkIntToScalar of small values
fn shade_rect(canvas: &Canvas, shader: Option<Shader>, cell_row: usize, cell_col: usize) {
    let mut paint = Paint::default();
    paint.set_shader(shader);

    canvas.save();
    canvas.translate((
        (cell_col as i32 * CELL_WIDTH + PAD_WIDTH) as f32,
        (cell_row as i32 * CELL_HEIGHT + PAD_HEIGHT) as f32,
    ));

    let rect = Rect::from_wh(RECT_WIDTH as f32, RECT_HEIGHT as f32);
    canvas.draw_rect(rect, &paint);
    canvas.restore();
}

// Port of: gm/analytic_gradients.cpp#L122-L170 (chrome/m156)
struct AnalyticGradientShaderGm;

impl GM for AnalyticGradientShaderGm {
    fn name(&self) -> String {
        "analytic_gradients".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1024, 512)
    }

    #[allow(clippy::cast_precision_loss)] // SkIntToScalar of small values
    fn on_draw(&mut self, canvas: &Canvas) {
        let points = [Point::new(0.0, 0.0), Point::new(RECT_WIDTH as f32, 0.0)];

        for cell_row in 0..NUM_ROWS {
            // Each interval has 4 different color counts, one per mode
            let color_counts = INTERVAL_COLOR_COUNTS[cell_row]; // Has len = 4

            for cell_col in 0..NUM_COLS {
                // create_gradient_points(cellRow, cellCol, points);

                // Get the color count dependent on interval and mode
                let color_count = color_counts[cell_col];
                // Get the positions given the mode
                let layout = M_POSITIONS[cell_col];

                // Collect positions and colors specific to the interval+mode normalizing the
                // position based on the interval count (== cellRow+1)
                let mut colors: Vec<Color4f> = vec![Color4f::default(); color_count];
                let mut positions: Vec<f32> = vec![0.0; color_count];
                for i in 0..color_count {
                    positions[i] = (layout[i] as f32) / ((cell_row + 1) as f32);
                    colors[i] = Color4f::from_color(COLORS[i % COLOR_COUNT]);
                }

                let grad = Gradient::new(
                    Colors::new(&colors, Some(&positions), TileMode::Clamp, None),
                    Interpolation::default(),
                );
                let shader = shaders::linear_gradient((points[0], points[1]), &grad, None);

                shade_rect(canvas, shader, cell_row, cell_col);
            }
        }
    }
}

// Port of: gm/analytic_gradients.cpp#L182 (chrome/m156)
crate::def_gm!(AnalyticGradientShaderGM, AnalyticGradientShaderGm);
