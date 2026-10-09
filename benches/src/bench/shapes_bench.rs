// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/ShapesBench.cpp

//! Thousands of rects, ovals, round rects, or donuts (a rect or round rect with an inner shape cut
//! out with `drawDRRect`), each under its own random transform and color (a rendering bench).
//!
//! `ENABLE_COMMAND_LINE_SHAPES_BENCH` is 0 in Skia, so the flag-driven `ShapesBench()` constructor
//! is compiled out there and is not ported.

// The int-to-scalar casts mirror the C++ `SkIntToScalar` and `int` arithmetic.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use skia_rust_core::color::Color;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::scalar::scalar;

use crate::def_bench;
use crate::prelude::*;

const BENCH_WIDTH: i32 = 1000;
const BENCH_HEIGHT: i32 = 1000;

/// `ShapesBench::ShapesType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShapesType {
    None,
    Rect,
    Oval,
    RRect,
    Mixed,
}

impl ShapesType {
    /// `shapeTypeNames[type]`.
    // Port of: bench/ShapesBench.cpp#L117-L124 (chrome/m156)
    fn name(self) -> &'static str {
        match self {
            ShapesType::None => "none",
            ShapesType::Rect => "rect",
            ShapesType::Oval => "oval",
            ShapesType::RRect => "rrect",
            ShapesType::Mixed => "mixed",
        }
    }

    /// The `int` `nextRangeU(kRect_ShapesType, kRRect_ShapesType)` produces, back as a shape.
    fn from_range_value(v: u32) -> Self {
        match v {
            1 => ShapesType::Rect,
            2 => ShapesType::Oval,
            3 => ShapesType::RRect,
            _ => unreachable!("nextRangeU(kRect, kRRect) is in 1..=3"),
        }
    }
}

/// `ShapeInfo::fDraw`: the `std::bind` of the `SkCanvas` draw call and the shape it captures.
#[derive(Clone, Copy, Debug)]
enum ShapeDraw {
    Rect(Rect),
    Oval(Rect),
    RRect(RRect),
    DRRect(RRect, RRect),
}

impl ShapeDraw {
    fn draw(&self, canvas: &Canvas, paint: &Paint) {
        match self {
            ShapeDraw::Rect(r) => {
                canvas.draw_rect(r, paint);
            }
            ShapeDraw::Oval(r) => {
                canvas.draw_oval(r, paint);
            }
            ShapeDraw::RRect(rr) => {
                canvas.draw_rrect(rr, paint);
            }
            ShapeDraw::DRRect(outer, inner) => {
                canvas.draw_drrect(outer, inner, paint);
            }
        }
    }
}

/// `ShapesBench::ShapeInfo`.
// Port of: bench/ShapesBench.cpp#L233-L237 (chrome/m156)
struct ShapeInfo {
    matrix: Matrix,
    color: u32,
    draw: Option<ShapeDraw>,
}

/// `class ShapesBench`.
// Port of: bench/ShapesBench.cpp#L43-L255 (chrome/m156)
struct ShapesBench {
    shapes_type: ShapesType,
    inner_shapes_type: ShapesType,
    num_shapes: i32,
    shapes_size: ISize,
    perspective: bool,
    name: String,
    rect: RRect,
    oval: RRect,
    rrect: RRect,
    inner_rect: RRect,
    inner_oval: RRect,
    inner_rrect: RRect,
    shapes: Vec<ShapeInfo>,
}

impl ShapesBench {
    // Port of: bench/ShapesBench.cpp#L53-L61 (chrome/m156)
    fn new(
        shapes_type: ShapesType,
        inner_shapes_type: ShapesType,
        num_shapes: i32,
        shapes_size: (i32, i32),
        perspective: bool,
    ) -> Self {
        let mut bench = Self {
            shapes_type,
            inner_shapes_type,
            num_shapes,
            shapes_size: ISize::new(shapes_size.0, shapes_size.1),
            perspective,
            name: String::new(),
            rect: RRect::default(),
            oval: RRect::default(),
            rrect: RRect::default(),
            inner_rect: RRect::default(),
            inner_oval: RRect::default(),
            inner_rrect: RRect::default(),
            shapes: Vec::new(),
        };
        bench.clamp_shape_size();
        bench.name = bench.make_name();
        bench
    }

    /// `clampShapeSize()`: scales the shape down so its diagonal fits the canvas.
    // Port of: bench/ShapesBench.cpp#L107-L115 (chrome/m156)
    fn clamp_shape_size(&mut self) {
        let max_diagonal = BENCH_WIDTH.min(BENCH_HEIGHT) as f32;
        let w = self.shapes_size.width;
        let h = self.shapes_size.height;
        let diagonal = ((w * w) as f32 + (h * h) as f32).sqrt();
        if diagonal > max_diagonal {
            self.shapes_size.width = (w as f32 * max_diagonal / diagonal) as i32;
            self.shapes_size.height = (h as f32 * max_diagonal / diagonal) as i32;
        }
    }

    /// `onGetName()`.
    // Port of: bench/ShapesBench.cpp#L117-L139 (chrome/m156)
    fn make_name(&self) -> String {
        let inner = match self.inner_shapes_type {
            ShapesType::None => String::new(),
            inner => format!("_inner_{}", inner.name()),
        };
        let persp = if self.perspective { "_persp" } else { "" };
        format!(
            "shapes_{}{inner}_{}_{}x{}{persp}",
            self.shapes_type.name(),
            self.num_shapes,
            self.shapes_size.width,
            self.shapes_size.height
        )
    }
}

/// The `sqrtf(W*W + H*H)` the placement pads by.
fn pad_for(size: ISize) -> scalar {
    ((size.width * size.width) as f32 + (size.height * size.height) as f32).sqrt()
}

impl Benchmark for ShapesBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/ShapesBench.cpp#L140 (chrome/m156)
    fn size(&mut self) -> ISize {
        ISize::new(BENCH_WIDTH, BENCH_HEIGHT)
    }

    // Port of: bench/ShapesBench.cpp#L142-L214 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        let w = self.shapes_size.width as scalar;
        let h = self.shapes_size.height as scalar;

        self.rect
            .set_rect(Rect::from_xywh(-w / 2.0, -h / 2.0, w, h));
        self.oval.set_oval(self.rect.rect());
        self.rrect
            .set_nine_patch(self.rect.rect(), w / 8.0, h / 13.0, w / 11.0, h / 7.0);

        if self.inner_shapes_type != ShapesType::None {
            self.inner_rect = self.rect.with_inset((w / 7.0, h / 11.0));
            self.inner_rect.offset((w / 28.0, h / 44.0));
            self.inner_oval.set_oval(self.inner_rect.rect());
            self.inner_rrect
                .set_rect_xy(self.inner_rect.rect(), w / 13.0, w / 7.0);
        }

        let mut rand = Random::default();
        let n = self.num_shapes as usize;
        self.shapes = Vec::with_capacity(n);
        for _ in 0..n {
            let pad = pad_for(self.shapes_size);
            // The two translate components draw from `rand` left to right.
            let tx = 0.5 * pad + rand.next_f() * (BENCH_WIDTH as scalar - pad);
            let ty = 0.5 * pad + rand.next_f() * (BENCH_HEIGHT as scalar - pad);
            let mut matrix = Matrix::new_identity();
            matrix.set_translate((tx, ty));
            matrix.pre_rotate(rand.next_f() * 360.0, None::<Point>);
            if self.perspective {
                matrix.set_persp_x(0.00015);
                matrix.set_persp_y(-0.00015);
            }
            let color = rand.next_u() | 0xff80_8080;
            self.shapes.push(ShapeInfo {
                matrix,
                color,
                draw: None,
            });
        }

        // Do this in a separate loop so mixed shapes get the same random numbers during
        // placement as non-mixed do.
        for i in 0..n {
            let mut shape_type = self.shapes_type;
            if shape_type == ShapesType::Mixed {
                shape_type = ShapesType::from_range_value(rand.next_range_u(1, 3));
            }
            let mut inner_shape_type = self.inner_shapes_type;
            if inner_shape_type == ShapesType::Mixed {
                inner_shape_type = ShapesType::from_range_value(rand.next_range_u(1, 3));
            }
            let draw = if inner_shape_type == ShapesType::None {
                match shape_type {
                    ShapesType::Rect => Some(ShapeDraw::Rect(*self.rect.rect())),
                    ShapesType::Oval => Some(ShapeDraw::Oval(*self.oval.rect())),
                    ShapesType::RRect => Some(ShapeDraw::RRect(self.rrect)),
                    ShapesType::None | ShapesType::Mixed => None,
                }
            } else {
                let outer = match shape_type {
                    ShapesType::Rect => Some(self.rect),
                    ShapesType::Oval => Some(self.oval),
                    ShapesType::RRect => Some(self.rrect),
                    ShapesType::None | ShapesType::Mixed => None,
                };
                let inner = match inner_shape_type {
                    ShapesType::Rect => Some(self.inner_rect),
                    ShapesType::Oval => Some(self.inner_oval),
                    ShapesType::RRect => Some(self.inner_rrect),
                    ShapesType::None | ShapesType::Mixed => None,
                };
                // `*outer` and `*inner` are dereferenced in C++ whatever the switch picked.
                match (outer, inner) {
                    (Some(o), Some(inn)) => Some(ShapeDraw::DRRect(o, inn)),
                    _ => unreachable!("shape types are resolved to rect, oval or rrect"),
                }
            };
            self.shapes[i].draw = draw;
        }
    }

    // Port of: bench/ShapesBench.cpp#L216-L228 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("ShapesBench is a rendering bench");
        let mut paint = Paint::default();
        self.setup_paint(&mut paint);
        for _ in 0..loops {
            for shape in &self.shapes {
                canvas.save();
                canvas.set_matrix(&M44::from(&shape.matrix));
                paint.set_color(Color::from(shape.color));
                if let Some(draw) = &shape.draw {
                    draw.draw(canvas, &paint);
                }
                canvas.restore();
            }
        }
    }
}

// Small primitives (CPU bound, in theory).
// Port of: bench/ShapesBench.cpp#L261-L262 (chrome/m156)
def_bench!(
    shapes_bench_rect_none_10000_32x32 = "ShapesBench(ShapesBench::kRect_ShapesType, ShapesBench::kNone_ShapesType, 10000, SkISize::Make(32, 32), false)",
    ShapesBench::new(ShapesType::Rect, ShapesType::None, 10000, (32, 32), false)
);
// Port of: bench/ShapesBench.cpp#L263-L264 (chrome/m156)
def_bench!(
    shapes_bench_oval_none_10000_32x32 = "ShapesBench(ShapesBench::kOval_ShapesType, ShapesBench::kNone_ShapesType, 10000, SkISize::Make(32, 32), false)",
    ShapesBench::new(ShapesType::Oval, ShapesType::None, 10000, (32, 32), false)
);
// Port of: bench/ShapesBench.cpp#L265-L266 (chrome/m156)
def_bench!(
    shapes_bench_oval_none_10000_32x33 = "ShapesBench(ShapesBench::kOval_ShapesType, ShapesBench::kNone_ShapesType, 10000, SkISize::Make(32, 33), false)",
    ShapesBench::new(ShapesType::Oval, ShapesType::None, 10000, (32, 33), false)
);
// Port of: bench/ShapesBench.cpp#L267-L268 (chrome/m156)
def_bench!(
    shapes_bench_rrect_none_10000_32x32 = "ShapesBench(ShapesBench::kRRect_ShapesType, ShapesBench::kNone_ShapesType, 10000, SkISize::Make(32, 32), false)",
    ShapesBench::new(ShapesType::RRect, ShapesType::None, 10000, (32, 32), false)
);
// Port of: bench/ShapesBench.cpp#L269-L270 (chrome/m156)
def_bench!(
    shapes_bench_mixed_none_10000_32x33 = "ShapesBench(ShapesBench::kMixed_ShapesType, ShapesBench::kNone_ShapesType, 10000, SkISize::Make(32, 33), false)",
    ShapesBench::new(ShapesType::Mixed, ShapesType::None, 10000, (32, 33), false)
);

// Large primitives (GPU bound, in theory).
// Port of: bench/ShapesBench.cpp#L273-L274 (chrome/m156)
def_bench!(
    shapes_bench_rect_none_100_500x500 = "ShapesBench(ShapesBench::kRect_ShapesType, ShapesBench::kNone_ShapesType, 100, SkISize::Make(500, 500), false)",
    ShapesBench::new(ShapesType::Rect, ShapesType::None, 100, (500, 500), false)
);
// Port of: bench/ShapesBench.cpp#L275-L276 (chrome/m156)
def_bench!(
    shapes_bench_oval_none_100_500x500 = "ShapesBench(ShapesBench::kOval_ShapesType, ShapesBench::kNone_ShapesType, 100, SkISize::Make(500, 500), false)",
    ShapesBench::new(ShapesType::Oval, ShapesType::None, 100, (500, 500), false)
);
// Port of: bench/ShapesBench.cpp#L277-L278 (chrome/m156)
def_bench!(
    shapes_bench_oval_none_100_500x501 = "ShapesBench(ShapesBench::kOval_ShapesType, ShapesBench::kNone_ShapesType, 100, SkISize::Make(500, 501), false)",
    ShapesBench::new(ShapesType::Oval, ShapesType::None, 100, (500, 501), false)
);
// Port of: bench/ShapesBench.cpp#L279-L280 (chrome/m156)
def_bench!(
    shapes_bench_rrect_none_100_500x500 = "ShapesBench(ShapesBench::kRRect_ShapesType, ShapesBench::kNone_ShapesType, 100, SkISize::Make(500, 500), false)",
    ShapesBench::new(ShapesType::RRect, ShapesType::None, 100, (500, 500), false)
);
// Port of: bench/ShapesBench.cpp#L281-L282 (chrome/m156)
def_bench!(
    shapes_bench_mixed_none_100_500x501 = "ShapesBench(ShapesBench::kMixed_ShapesType, ShapesBench::kNone_ShapesType, 100, SkISize::Make(500, 501), false)",
    ShapesBench::new(ShapesType::Mixed, ShapesType::None, 100, (500, 501), false)
);

// Donuts (small and large). These fall-back to path rendering due to non-orthogonal rotation
// making them quite slow. Thus, reduce the counts substantially.
// Port of: bench/ShapesBench.cpp#L286-L287 (chrome/m156)
def_bench!(
    shapes_bench_rect_rect_500_32x32 = "ShapesBench(ShapesBench::kRect_ShapesType, ShapesBench::kRect_ShapesType, 500, SkISize::Make(32, 32), false)",
    ShapesBench::new(ShapesType::Rect, ShapesType::Rect, 500, (32, 32), false)
);
// Port of: bench/ShapesBench.cpp#L288-L289 (chrome/m156)
def_bench!(
    shapes_bench_rrect_rrect_500_32x32 = "ShapesBench(ShapesBench::kRRect_ShapesType, ShapesBench::kRRect_ShapesType, 500, SkISize::Make(32, 32), false)",
    ShapesBench::new(ShapesType::RRect, ShapesType::RRect, 500, (32, 32), false)
);
// Port of: bench/ShapesBench.cpp#L290-L291 (chrome/m156)
def_bench!(
    shapes_bench_rect_rect_50_500x500 = "ShapesBench(ShapesBench::kRect_ShapesType, ShapesBench::kRect_ShapesType, 50, SkISize::Make(500, 500), false)",
    ShapesBench::new(ShapesType::Rect, ShapesType::Rect, 50, (500, 500), false)
);
// Port of: bench/ShapesBench.cpp#L292-L293 (chrome/m156)
def_bench!(
    shapes_bench_rrect_rrect_50_500x500 = "ShapesBench(ShapesBench::kRRect_ShapesType, ShapesBench::kRRect_ShapesType, 50, SkISize::Make(500, 500), false)",
    ShapesBench::new(ShapesType::RRect, ShapesType::RRect, 50, (500, 500), false)
);
