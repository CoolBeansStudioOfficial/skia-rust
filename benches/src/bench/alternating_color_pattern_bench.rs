// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/AlternatingColorPatternBench.cpp

//! A 5 x 5 grid of rects or filled paths alternating between two color patterns: solid white,
//! solid blue, an opaque bitmap or a partly transparent bitmap (a rendering bench).

// The int-to-scalar and int-index casts mirror the C++ `SkIntToScalar` and `int` arithmetic.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::def_bench;
use crate::prelude::*;

const NX: usize = 5;
const NY: usize = 5;
const NUM_DRAWS: usize = NX * NY;

/// `enum ColorPattern`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColorPattern {
    White,
    Blue,
    OpaqueBitmap,
    AlphaBitmap,
}

/// `struct ColorPatternData`.
struct ColorPatternData {
    color: Color,
    is_bitmap: bool,
    name: &'static str,
}

// Keep this in same order as ColorPattern enum.
// Port of: bench/AlternatingColorPatternBench.cpp#L24-L30 (chrome/m156)
fn color_pattern_data(pattern: ColorPattern) -> ColorPatternData {
    match pattern {
        ColorPattern::White => ColorPatternData {
            color: Color::WHITE,
            is_bitmap: false,
            name: "white",
        },
        ColorPattern::Blue => ColorPatternData {
            color: Color::BLUE,
            is_bitmap: false,
            name: "blue",
        },
        // The misspelling is Skia's own name for this pattern.
        ColorPattern::OpaqueBitmap => ColorPatternData {
            color: Color::WHITE,
            is_bitmap: true,
            name: "obaqueBitMap",
        },
        ColorPattern::AlphaBitmap => ColorPatternData {
            color: Color::from(0x1000_0000_u32),
            is_bitmap: true,
            name: "alphaBitmap",
        },
    }
}

/// `enum DrawType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DrawType {
    Rect,
    Path,
}

/// `makebm(SkBitmap* bm, int w, int h)`: a 40 x 40 bitmap of two linear gradients.
// Port of: bench/AlternatingColorPatternBench.cpp#L32-L60 (chrome/m156)
fn makebm(bm: &mut Bitmap, w: i32, h: i32) {
    bm.alloc_n32_pixels((w, h), None);
    bm.erase_color(Color::TRANSPARENT);

    let canvas = <Canvas as RasterCanvas>::from_bitmap(bm, None)
        .expect("a bitmap just allocated is ready to draw");
    let s: scalar = w.min(h) as scalar;
    let pts0 = [Point::new(0.0, 0.0), Point::new(s, s)];
    let pts1 = [Point::new(s / 2.0, 0.0), Point::new(s / 2.0, s)];
    let pos: [scalar; 3] = [0.0, 1.0 / 2.0, 1.0];
    let colors0 = [
        Color4f::from_color(Color::from(0x80F0_0080_u32)),
        Color4f::from_color(Color::from(0xF0F0_8000_u32)),
        Color4f::from_color(Color::from(0x8000_80F0_u32)),
    ];
    let colors1 = [
        Color4f::from_color(Color::from(0xF080_00F0_u32)),
        Color4f::from_color(Color::from(0x8080_F000_u32)),
        Color4f::from_color(Color::from(0xF000_F080_u32)),
    ];

    let mut paint = Paint::default();

    let gradient0 = Gradient::new(
        Colors::new(&colors0, Some(&pos), TileMode::Clamp, None),
        Interpolation::default(),
    );
    paint.set_shader(shaders::linear_gradient(
        (pts0[0], pts0[1]),
        &gradient0,
        None,
    ));
    canvas.draw_paint(&paint);
    let gradient1 = Gradient::new(
        Colors::new(&colors1, Some(&pos), TileMode::Clamp, None),
        Interpolation::default(),
    );
    paint.set_shader(shaders::linear_gradient(
        (pts1[0], pts1[1]),
        &gradient1,
        None,
    ));
    canvas.draw_paint(&paint);
}

/// `class AlternatingColorPatternBench`.
// Port of: bench/AlternatingColorPatternBench.cpp#L62-L157 (chrome/m156)
struct AlternatingColorPatternBench {
    bm_shader: Option<Shader>,
    paths: Vec<Path>,
    rects: Vec<Rect>,
    colors: [Color; NUM_DRAWS],
    shaders: Vec<Option<Shader>>,
    name: String,
    pattern1: ColorPatternData,
    pattern2: ColorPatternData,
    draw_type: DrawType,
    bmp: Bitmap,
}

impl AlternatingColorPatternBench {
    // Port of: bench/AlternatingColorPatternBench.cpp#L79-L89 (chrome/m156)
    fn new(pattern1: ColorPattern, pattern2: ColorPattern, draw_type: DrawType) -> Self {
        let p1 = color_pattern_data(pattern1);
        let p2 = color_pattern_data(pattern2);
        let name = format!(
            "colorPattern_{}_{}_{}",
            p1.name,
            p2.name,
            if draw_type == DrawType::Rect {
                "rect"
            } else {
                "path"
            }
        );
        Self {
            bm_shader: None,
            paths: vec![Path::default(); NUM_DRAWS],
            rects: vec![Rect::default(); NUM_DRAWS],
            colors: [Color::default(); NUM_DRAWS],
            shaders: vec![None; NUM_DRAWS],
            name,
            pattern1: p1,
            pattern2: p2,
            draw_type,
            bmp: Bitmap::new(),
        }
    }
}

impl Benchmark for AlternatingColorPatternBench {
    // Port of: bench/AlternatingColorPatternBench.cpp#L93-L96 (chrome/m156)
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/AlternatingColorPatternBench.cpp#L98-L134 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        let w: i32 = 40;
        let h: i32 = 40;
        makebm(&mut self.bmp, w, h);
        self.bm_shader = self.bmp.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            SamplingOptions::from(FilterMode::Linear),
            None,
        );
        let offset: i32 = 2;
        let mut count = 0;
        for j in 0..NY {
            for i in 0..NX {
                let x = (w + offset) * i as i32;
                let y = (h * offset) * j as i32;
                if self.draw_type == DrawType::Rect {
                    self.rects[count].set_xywh(x as scalar, y as scalar, w as scalar, h as scalar);
                } else {
                    let mut builder = PathBuilder::new();
                    builder.move_to((x as scalar, y as scalar));
                    builder.r_line_to((w as scalar, 0.0));
                    builder.r_line_to((0.0, h as scalar));
                    builder.r_line_to(((-w + 1) as scalar, 0.0));
                    self.paths[count] = builder.detach();
                }
                if count % 2 == 0 {
                    self.colors[count] = self.pattern1.color;
                    self.shaders[count] = if self.pattern1.is_bitmap {
                        self.bm_shader.clone()
                    } else {
                        None
                    };
                } else {
                    self.colors[count] = self.pattern2.color;
                    self.shaders[count] = if self.pattern2.is_bitmap {
                        self.bm_shader.clone()
                    } else {
                        None
                    };
                }
                count += 1;
            }
        }
    }

    // Port of: bench/AlternatingColorPatternBench.cpp#L136-L152 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("AlternatingColorPatternBench is a rendering bench");
        let mut paint = Paint::default();
        paint.set_anti_alias(false);

        for _ in 0..loops {
            for j in 0..NUM_DRAWS {
                paint.set_color(self.colors[j]);
                paint.set_shader(self.shaders[j].clone());
                if self.draw_type == DrawType::Rect {
                    canvas.draw_rect(self.rects[j], &paint);
                } else {
                    canvas.draw_path(&self.paths[j], &paint);
                }
            }
        }
    }
}

// Port of: bench/AlternatingColorPatternBench.cpp#L159-L161 (chrome/m156)
def_bench!(
    alternating_color_pattern_bench_white_white_path =
        "AlternatingColorPatternBench(kWhite_ColorPattern, kWhite_ColorPattern, kPath_DrawType)",
    AlternatingColorPatternBench::new(ColorPattern::White, ColorPattern::White, DrawType::Path)
);
// Port of: bench/AlternatingColorPatternBench.cpp#L162-L164 (chrome/m156)
def_bench!(
    alternating_color_pattern_bench_blue_blue_path =
        "AlternatingColorPatternBench(kBlue_ColorPattern, kBlue_ColorPattern, kPath_DrawType)",
    AlternatingColorPatternBench::new(ColorPattern::Blue, ColorPattern::Blue, DrawType::Path)
);
// Port of: bench/AlternatingColorPatternBench.cpp#L165-L167 (chrome/m156)
def_bench!(
    alternating_color_pattern_bench_white_blue_path =
        "AlternatingColorPatternBench(kWhite_ColorPattern, kBlue_ColorPattern, kPath_DrawType)",
    AlternatingColorPatternBench::new(ColorPattern::White, ColorPattern::Blue, DrawType::Path)
);
// Port of: bench/AlternatingColorPatternBench.cpp#L169-L171 (chrome/m156)
def_bench!(
    alternating_color_pattern_bench_opaque_opaque_path = "AlternatingColorPatternBench(kOpaqueBitmap_ColorPattern, kOpaqueBitmap_ColorPattern, kPath_DrawType)",
    AlternatingColorPatternBench::new(
        ColorPattern::OpaqueBitmap,
        ColorPattern::OpaqueBitmap,
        DrawType::Path
    )
);
// Port of: bench/AlternatingColorPatternBench.cpp#L172-L174 (chrome/m156)
def_bench!(
    alternating_color_pattern_bench_alpha_alpha_path = "AlternatingColorPatternBench(kAlphaBitmap_ColorPattern, kAlphaBitmap_ColorPattern, kPath_DrawType)",
    AlternatingColorPatternBench::new(
        ColorPattern::AlphaBitmap,
        ColorPattern::AlphaBitmap,
        DrawType::Path
    )
);
// Port of: bench/AlternatingColorPatternBench.cpp#L175-L177 (chrome/m156)
def_bench!(
    alternating_color_pattern_bench_opaque_alpha_path = "AlternatingColorPatternBench(kOpaqueBitmap_ColorPattern, kAlphaBitmap_ColorPattern, kPath_DrawType)",
    AlternatingColorPatternBench::new(
        ColorPattern::OpaqueBitmap,
        ColorPattern::AlphaBitmap,
        DrawType::Path
    )
);
// Port of: bench/AlternatingColorPatternBench.cpp#L179-L181 (chrome/m156)
def_bench!(
    alternating_color_pattern_bench_opaque_opaque_rect = "AlternatingColorPatternBench(kOpaqueBitmap_ColorPattern, kOpaqueBitmap_ColorPattern, kRect_DrawType)",
    AlternatingColorPatternBench::new(
        ColorPattern::OpaqueBitmap,
        ColorPattern::OpaqueBitmap,
        DrawType::Rect
    )
);
// Port of: bench/AlternatingColorPatternBench.cpp#L182-L184 (chrome/m156)
def_bench!(
    alternating_color_pattern_bench_alpha_alpha_rect = "AlternatingColorPatternBench(kAlphaBitmap_ColorPattern, kAlphaBitmap_ColorPattern, kRect_DrawType)",
    AlternatingColorPatternBench::new(
        ColorPattern::AlphaBitmap,
        ColorPattern::AlphaBitmap,
        DrawType::Rect
    )
);
// Port of: bench/AlternatingColorPatternBench.cpp#L185-L187 (chrome/m156)
def_bench!(
    alternating_color_pattern_bench_opaque_alpha_rect = "AlternatingColorPatternBench(kOpaqueBitmap_ColorPattern, kAlphaBitmap_ColorPattern, kRect_DrawType)",
    AlternatingColorPatternBench::new(
        ColorPattern::OpaqueBitmap,
        ColorPattern::AlphaBitmap,
        DrawType::Rect
    )
);
