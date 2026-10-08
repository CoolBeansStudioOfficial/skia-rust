// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/labyrinth.cpp (chrome/m156)

#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::excessive_precision,
    clippy::float_cmp,
    clippy::inconsistent_digit_grouping,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::path_builder::PathBuilder;

/// Repro case for <https://bugs.chromium.org/p/chromium/issues/detail?id=913223>
///
/// The original bug was filed against square caps, but here we also draw the labyrinth using round
/// and butt caps.
///
/// Square and round caps expose over-coverage on overlaps when using coverage counting.
///
/// Butt caps expose under-coverage on abutted strokes when using a `max()` coverage function.
// Port of: gm/labyrinth.cpp#L26-L85 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // int cell coordinates, as in C++
fn draw_labyrinth(canvas: &Canvas, cap: Cap) {
    const K_ROWS: [[u8; 12]; 11] = [
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        [0, 1, 0, 1, 0, 1, 0, 0, 0, 0, 1, 1],
        [0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1, 1],
        [1, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0],
        [0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1],
        [1, 0, 0, 1, 0, 0, 0, 0, 1, 1, 1, 0],
        [0, 1, 0, 1, 1, 1, 0, 0, 1, 1, 1, 0],
        [1, 0, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1],
        [0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 1],
        [0, 1, 1, 1, 0, 0, 1, 1, 1, 1, 0, 0],
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    ];

    const K_COLS: [[u8; 10]; 13] = [
        [1, 1, 1, 1, 0, 1, 1, 1, 1, 1],
        [0, 0, 1, 0, 0, 0, 1, 1, 1, 0],
        [0, 1, 1, 0, 1, 1, 1, 0, 0, 1],
        [1, 1, 0, 0, 0, 0, 1, 0, 1, 0],
        [0, 0, 1, 0, 1, 0, 0, 0, 0, 1],
        [0, 0, 1, 1, 1, 0, 0, 0, 1, 0],
        [0, 1, 0, 1, 1, 1, 0, 0, 0, 0],
        [1, 1, 1, 0, 1, 1, 1, 0, 1, 0],
        [1, 1, 0, 1, 1, 0, 0, 0, 1, 0],
        [0, 0, 1, 0, 0, 0, 0, 0, 0, 1],
        [0, 0, 1, 1, 0, 0, 0, 0, 1, 0],
        [0, 0, 0, 0, 0, 0, 1, 0, 0, 1],
        [1, 1, 1, 1, 1, 1, 0, 1, 1, 1],
    ];

    let mut maze = PathBuilder::new();
    for (y, row) in K_ROWS.iter().enumerate() {
        for (x, cell) in row.iter().enumerate() {
            if *cell != 0 {
                maze.move_to((x as f32, y as f32));
                maze.line_to(((x + 1) as f32, y as f32));
            }
        }
    }
    for (x, col) in K_COLS.iter().enumerate() {
        for (y, cell) in col.iter().enumerate() {
            if *cell != 0 {
                maze.move_to((x as f32, y as f32));
                maze.line_to((x as f32, (y + 1) as f32));
            }
        }
    }

    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(0.1);
    paint.set_color(Color::new(0xff40_6060));
    paint.set_anti_alias(true);
    paint.set_stroke_cap(cap);

    canvas.translate((10.5, 10.5));
    canvas.scale((40.0, 40.0));
    canvas.draw_path(&maze.detach(), &paint);
}

// Port of: gm/labyrinth.cpp#L87-L100 (chrome/m156)
const K_WIDTH: i32 = 500;
const K_HEIGHT: i32 = 420;

// Port of: gm/labyrinth.cpp#L90-L92 (chrome/m156)
crate::def_simple_gm!(labyrinth_square, canvas, K_WIDTH, K_HEIGHT, {
    draw_labyrinth(canvas, Cap::Square);
});

// Port of: gm/labyrinth.cpp#L94-L96 (chrome/m156)
crate::def_simple_gm!(labyrinth_round, canvas, K_WIDTH, K_HEIGHT, {
    draw_labyrinth(canvas, Cap::Round);
});

// Port of: gm/labyrinth.cpp#L98-L100 (chrome/m156)
crate::def_simple_gm!(labyrinth_butt, canvas, K_WIDTH, K_HEIGHT, {
    draw_labyrinth(canvas, Cap::Butt);
});
