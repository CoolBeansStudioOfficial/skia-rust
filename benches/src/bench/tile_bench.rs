// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/TileBench.cpp

//! `ConstXTileBench`: a 1×300 gradient bitmap used as a tiled shader (the special case of a tiled
//! 1×N texture), with optional translucency and a 2× scale (a rendering bench).

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::int_to_scalar;
use skia_rust_core::tile_mode::TileMode;

use crate::def_bench;
use crate::prelude::*;

/// `ConstXTileBench::kWidth`.
// Port of: bench/TileBench.cpp#L109-L110 (chrome/m156)
const WIDTH: i32 = 1;
/// `ConstXTileBench::kHeight`.
const HEIGHT: i32 = 300;

/// `create_gradient(SkBitmap* bm)`: a blue ramp down the single column.
// Port of: bench/TileBench.cpp#L17-L28 (chrome/m156)
// The float-to-u8 cast is C++'s (U8CPU) conversion, which the port mirrors on purpose.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn create_gradient(bm: &mut Bitmap) {
    debug_assert_eq!(bm.width(), 1);
    let height = bm.height();

    let delta_b: f32 = 255.0 / int_to_scalar(height);
    let mut blue: f32 = 255.0;

    for y in 0..height {
        // *bm->getAddr32(0, y) = SkColorSetRGB(0, 0, (U8CPU) blue);
        // The float-to-u8 conversion saturates in Rust, where C++ truncates an in-range value;
        // `blue` stays in [0, 255] here, so the two agree.
        let color: u32 = 0xFF00_0000 | u32::from(blue as u8);
        bm.set_addr32(0, y, color);
        blue -= delta_b;
    }
}

/// `ConstXTileBench::name` table: `gTileModeStr`, indexed by `(unsigned)tile`.
// Port of: bench/TileBench.cpp#L32-L120 (chrome/m156)
fn tile_mode_str(tile: TileMode) -> &'static str {
    match tile {
        TileMode::Clamp => "C",
        TileMode::Repeat => "R",
        TileMode::Mirror => "M",
        TileMode::Decal => "D",
    }
}

/// `class ConstXTileBench`.
// Port of: bench/TileBench.cpp#L32-L120 (chrome/m156)
struct ConstXTileBench {
    filter_mode: FilterMode,
    x_tile: TileMode,
    y_tile: TileMode,
    do_trans: bool,
    do_scale: bool,
    paint: Paint,
    name: String,
}

impl ConstXTileBench {
    // ConstXTileBench(SkTileMode xTile, SkTileMode yTile, SkFilterMode fm, bool doTrans, bool doScale)
    // Port of: bench/TileBench.cpp#L32-L120 (chrome/m156)
    fn new(
        x_tile: TileMode,
        y_tile: TileMode,
        fm: FilterMode,
        do_trans: bool,
        do_scale: bool,
    ) -> Self {
        // fName.printf("constXTile_");
        let mut name = String::from("constXTile_");
        // fName.append(gTileModeStr[(unsigned)xTile]);
        name.push_str(tile_mode_str(x_tile));
        // fName.append(gTileModeStr[(unsigned)yTile]);
        name.push_str(tile_mode_str(y_tile));
        // if (fm != SkFilterMode::kNearest) fName.append("_filter");
        if fm != FilterMode::Nearest {
            name.push_str("_filter");
        }
        // if (doTrans) fName.append("_trans");
        if do_trans {
            name.push_str("_trans");
        }
        // if (doScale) fName.append("_scale");
        if do_scale {
            name.push_str("_scale");
        }

        Self {
            filter_mode: fm,
            x_tile,
            y_tile,
            do_trans,
            do_scale,
            paint: Paint::default(),
            name,
        }
    }
}

impl Benchmark for ConstXTileBench {
    // onGetName()
    fn name(&self) -> String {
        self.name.clone()
    }

    // onDelayedSetup()
    // Port of: bench/TileBench.cpp#L32-L120 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        // SkBitmap bm;
        let mut bm = Bitmap::new();
        // bm.allocN32Pixels(kWidth, kHeight, true);
        bm.alloc_n32_pixels((WIDTH, HEIGHT), true);
        // bm.eraseColor(SK_ColorWHITE);
        bm.erase_color(Color::new(0xFFFF_FFFF));

        create_gradient(&mut bm);

        // fPaint.setShader(bm.makeShader(fXTile, fYTile, SkSamplingOptions(fFilterMode)));
        self.paint.set_shader(bm.to_shader(
            (self.x_tile, self.y_tile),
            SamplingOptions::from(self.filter_mode),
            None,
        ));
    }

    // onDraw(int loops, SkCanvas* canvas)
    // Port of: bench/TileBench.cpp#L32-L120 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("ConstXTileBench is a rendering bench");
        // SkPaint paint(fPaint);
        let mut paint = self.paint.clone();
        // this->setupPaint(&paint);
        self.setup_paint(&mut paint);
        if self.do_trans {
            // paint.setColor(SkColorSetARGB(0x80, 0xFF, 0xFF, 0xFF));
            paint.set_color(Color::new(0x80FF_FFFF));
        }

        // SkRect r;
        let r = if self.do_scale {
            // r = SkRect::MakeWH(SkIntToScalar(2 * 640), SkIntToScalar(2 * 480));
            let r = Rect::from_xywh(0.0, 0.0, 2.0 * 640.0, 2.0 * 480.0);
            // canvas->scale(SK_ScalarHalf, SK_ScalarHalf);
            canvas.scale((0.5, 0.5));
            r
        } else {
            // r = SkRect::MakeWH(SkIntToScalar(640), SkIntToScalar(480));
            Rect::from_xywh(0.0, 0.0, 640.0, 480.0)
        };

        // SkPaint bgPaint;
        // bgPaint.setColor(SK_ColorWHITE);
        let mut bg_paint = Paint::default();
        bg_paint.set_color(Color::new(0xFFFF_FFFF));

        // for (int i = 0; i < loops; i++) { ... }
        for _ in 0..loops {
            if self.do_trans {
                canvas.draw_rect(r, &bg_paint);
            }

            canvas.draw_rect(r, &paint);
        }
    }
}

// Port of: bench/TileBench.cpp#L129-L129 (chrome/m156)
def_bench!(
    const_x_tile_bench_clamp_clamp_nn =
        "ConstXTileBench(SkTileMode::kClamp, SkTileMode::kClamp, gNN, false, false)",
    ConstXTileBench::new(
        TileMode::Clamp,
        TileMode::Clamp,
        FilterMode::Nearest,
        false,
        false
    )
);
// Port of: bench/TileBench.cpp#L132-L132 (chrome/m156)
def_bench!(
    const_x_tile_bench_repeat_repeat_li =
        "ConstXTileBench(SkTileMode::kRepeat, SkTileMode::kRepeat, gLI, false, false)",
    ConstXTileBench::new(
        TileMode::Repeat,
        TileMode::Repeat,
        FilterMode::Linear,
        false,
        false
    )
);
// Port of: bench/TileBench.cpp#L134-L134 (chrome/m156)
def_bench!(
    const_x_tile_bench_mirror_mirror_li =
        "ConstXTileBench(SkTileMode::kMirror, SkTileMode::kMirror, gLI, false, false)",
    ConstXTileBench::new(
        TileMode::Mirror,
        TileMode::Mirror,
        FilterMode::Linear,
        false,
        false
    )
);
// Port of: bench/TileBench.cpp#L137-L137 (chrome/m156)
def_bench!(
    const_x_tile_bench_clamp_clamp_nn_trans =
        "ConstXTileBench(SkTileMode::kClamp, SkTileMode::kClamp, gNN, true, false)",
    ConstXTileBench::new(
        TileMode::Clamp,
        TileMode::Clamp,
        FilterMode::Nearest,
        true,
        false
    )
);
// Port of: bench/TileBench.cpp#L140-L140 (chrome/m156)
def_bench!(
    const_x_tile_bench_repeat_repeat_li_trans =
        "ConstXTileBench(SkTileMode::kRepeat, SkTileMode::kRepeat, gLI, true, false)",
    ConstXTileBench::new(
        TileMode::Repeat,
        TileMode::Repeat,
        FilterMode::Linear,
        true,
        false
    )
);
// Port of: bench/TileBench.cpp#L142-L142 (chrome/m156)
def_bench!(
    const_x_tile_bench_mirror_mirror_li_trans =
        "ConstXTileBench(SkTileMode::kMirror, SkTileMode::kMirror, gLI, true, false)",
    ConstXTileBench::new(
        TileMode::Mirror,
        TileMode::Mirror,
        FilterMode::Linear,
        true,
        false
    )
);
