// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/TableBench.cpp

//! A spreadsheet-style table of 1-pixel-bordered cells, drawn as three `drawRect`s per cell (a
//! rendering bench).

use skia_rust_core::color::Color;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::int_to_scalar;

use crate::def_bench;
use crate::prelude::*;

// Port of: bench/TableBench.cpp#L10-L11 (chrome/m156)
const K_CELL_WIDTH: f32 = 20.0; // SkIntToScalar(20)
const K_CELL_HEIGHT: f32 = 10.0; // SkIntToScalar(10)

/// `class TableBench`.
// Port of: bench/TableBench.cpp#L24-L70 (chrome/m156)
struct TableBench;

impl TableBench {
    const NUM_ROWS: i32 = 48;
    const NUM_COLS: i32 = 32;
}

impl Benchmark for TableBench {
    fn name(&self) -> String {
        "tablebench".to_owned()
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("TableBench is a rendering bench");
        // SkPaint cellPaint; cellPaint.setColor(0xFFFFFFF);
        let mut cell_paint = Paint::default();
        cell_paint.set_color(Color::new(0x0FFF_FFFF));

        // SkPaint borderPaint; borderPaint.setColor(0xFFCCCCCC);
        let mut border_paint = Paint::default();
        border_paint.set_color(Color::new(0xFFCC_CCCC));

        for _ in 0..loops {
            for row in 0..Self::NUM_ROWS {
                for col in 0..Self::NUM_COLS {
                    let cell = Rect::from_ltrb(
                        int_to_scalar(col) * K_CELL_WIDTH,
                        int_to_scalar(row) * K_CELL_HEIGHT,
                        int_to_scalar(col + 1) * K_CELL_WIDTH,
                        int_to_scalar(row + 1) * K_CELL_HEIGHT,
                    );
                    canvas.draw_rect(cell, &cell_paint);

                    let bottom = Rect::from_ltrb(
                        int_to_scalar(col) * K_CELL_WIDTH,
                        int_to_scalar(row) * K_CELL_HEIGHT + (K_CELL_HEIGHT - 1.0),
                        int_to_scalar(col + 1) * K_CELL_WIDTH,
                        int_to_scalar(row + 1) * K_CELL_HEIGHT,
                    );
                    canvas.draw_rect(bottom, &border_paint);

                    let right = Rect::from_ltrb(
                        int_to_scalar(col) * K_CELL_WIDTH + (K_CELL_WIDTH - 1.0),
                        int_to_scalar(row) * K_CELL_HEIGHT,
                        int_to_scalar(col + 1) * K_CELL_WIDTH,
                        int_to_scalar(row + 1) * K_CELL_HEIGHT,
                    );
                    canvas.draw_rect(right, &border_paint);
                }
            }
        }
    }
}

// Port of: bench/TableBench.cpp#L72-L72 (chrome/m156)
def_bench!(table_bench = "TableBench()", TableBench);
