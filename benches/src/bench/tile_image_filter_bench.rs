// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/TileImageFilterBench.cpp

//! `TileImageFilterBench`: `SkImageFilters::Tile` of a 50×50 source over a 512×512 destination,
//! drawn as one rect or in square tiles of `tile_size` (a rendering bench).

use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::int_to_scalar;
use skia_rust_effects::image_filters;

use crate::def_bench;
use crate::prelude::*;

/// `#define WIDTH 512`.
// Port of: bench/TileImageFilterBench.cpp#L14-L14 (chrome/m156)
const WIDTH: i32 = 512;
/// `#define HEIGHT 512`.
// Port of: bench/TileImageFilterBench.cpp#L15-L15 (chrome/m156)
const HEIGHT: i32 = 512;

/// `class TileImageFilterBench`.
// Port of: bench/TileImageFilterBench.cpp#L23-L66 (chrome/m156)
struct TileImageFilterBench {
    tile_size: i32,
    name: String,
}

impl TileImageFilterBench {
    // TileImageFilterBench(int tileSize) : fTileSize(tileSize)
    // Port of: bench/TileImageFilterBench.cpp#L23-L66 (chrome/m156)
    fn new(tile_size: i32) -> Self {
        // if (tileSize > 0) fName.printf("tile_image_filter_tiled_%d", tileSize);
        // else fName.printf("tile_image_filter");
        let name = if tile_size > 0 {
            format!("tile_image_filter_tiled_{tile_size}")
        } else {
            "tile_image_filter".to_owned()
        };
        Self { tile_size, name }
    }
}

impl Benchmark for TileImageFilterBench {
    // onGetName()
    fn name(&self) -> String {
        self.name.clone()
    }

    // onDraw(int loops, SkCanvas* canvas)
    // Port of: bench/TileImageFilterBench.cpp#L23-L66 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("TileImageFilterBench is a rendering bench");
        // SkPaint paint;
        let mut paint = Paint::default();
        // paint.setImageFilter(SkImageFilters::Tile(SkRect::MakeWH(50, 50),
        //                                           SkRect::MakeWH(WIDTH, HEIGHT), nullptr));
        let tile: Option<ImageFilter> = image_filters::tile(
            &Rect::from_xywh(0.0, 0.0, 50.0, 50.0),
            &Rect::from_xywh(0.0, 0.0, int_to_scalar(WIDTH), int_to_scalar(HEIGHT)),
            None,
        );
        paint.set_image_filter(tile);

        // for (int i = 0; i < loops; i++) { ... }
        for _ in 0..loops {
            if self.tile_size > 0 {
                // for (int y = 0; y < HEIGHT; y += fTileSize)
                for y in (0..HEIGHT)
                    .step_by(usize::try_from(self.tile_size).expect("tile size is positive"))
                {
                    // for (int x = 0; x < WIDTH; x += fTileSize)
                    for x in (0..WIDTH)
                        .step_by(usize::try_from(self.tile_size).expect("tile size is positive"))
                    {
                        // canvas->save();
                        canvas.save();
                        // SkIRect clipIRect = SkIRect::MakeXYWH(x, y, fTileSize, fTileSize);
                        // canvas->clipRect(SkRect::Make(clipIRect));
                        canvas.clip_rect(
                            Rect::from_xywh(
                                int_to_scalar(x),
                                int_to_scalar(y),
                                int_to_scalar(self.tile_size),
                                int_to_scalar(self.tile_size),
                            ),
                            None,
                            None,
                        );
                        // canvas->drawRect(SkRect::MakeWH(WIDTH, HEIGHT), paint);
                        canvas.draw_rect(
                            Rect::from_xywh(0.0, 0.0, int_to_scalar(WIDTH), int_to_scalar(HEIGHT)),
                            &paint,
                        );
                        // canvas->restore();
                        canvas.restore();
                    }
                }
            } else {
                // canvas->drawRect(SkRect::MakeWH(WIDTH, HEIGHT), paint);
                canvas.draw_rect(
                    Rect::from_xywh(0.0, 0.0, int_to_scalar(WIDTH), int_to_scalar(HEIGHT)),
                    &paint,
                );
            }
        }
    }
}

// Port of: bench/TileImageFilterBench.cpp#L68-L68 (chrome/m156)
def_bench!(
    tile_image_filter_bench_0 = "TileImageFilterBench(0)",
    TileImageFilterBench::new(0)
);
// Port of: bench/TileImageFilterBench.cpp#L69-L69 (chrome/m156)
def_bench!(
    tile_image_filter_bench_32 = "TileImageFilterBench(32)",
    TileImageFilterBench::new(32)
);
// Port of: bench/TileImageFilterBench.cpp#L70-L70 (chrome/m156)
def_bench!(
    tile_image_filter_bench_64 = "TileImageFilterBench(64)",
    TileImageFilterBench::new(64)
);
