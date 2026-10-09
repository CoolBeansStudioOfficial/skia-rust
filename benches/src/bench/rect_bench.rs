// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/RectBench.cpp

//! Rects, ovals, round rects and points drawn from a fixed random set of 300 rectangles, the
//! hairline points, blit-mask points, and `SkRect::Bounds` over random points.
//!
//! `--strokeWidth` (`FLAGS_strokeWidth`, default -1) is not ported: its default leaves the
//! stroke widths of `PointsBench` and `BlitMaskBench` as the C++ defaults.

// The int-to-scalar and int-index casts mirror the C++ `SkIntToScalar` and `int` arithmetic.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::PointMode;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, Style};
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

use crate::def_bench;
use crate::prelude::*;

const W: u32 = 640;
const H: u32 = 480;
const N: usize = 300;

/// `RectBench`'s data and the parts shared by its subclasses: `fShift`, `fStroke`, `fAA`,
/// `fPerspective`, `fRects`, `fColors`.
// Port of: bench/RectBench.cpp#L20-L112 (chrome/m156)
struct RectBase {
    shift: i32,
    stroke: i32,
    aa: bool,
    perspective: bool,
    rects: Vec<Rect>,
    colors: Vec<u32>,
}

impl RectBase {
    // Port of: bench/RectBench.cpp#L33-L37 (chrome/m156)
    fn new(shift: i32, stroke: i32, aa: bool, perspective: bool) -> Self {
        Self {
            shift,
            stroke,
            aa,
            perspective,
            rects: vec![Rect::default(); N],
            colors: vec![0; N],
        }
    }

    /// `computeName(root)`: `root_shift[_stroke_w]_aa|_bw[_persp]`.
    // Port of: bench/RectBench.cpp#L39-L53 (chrome/m156)
    fn compute_name(&self, root: &str) -> String {
        let stroke = if self.stroke > 0 {
            format!("_stroke_{}", self.stroke)
        } else {
            String::new()
        };
        let aa = if self.aa { "_aa" } else { "_bw" };
        let persp = if self.perspective { "_persp" } else { "" };
        format!("{root}_{}{stroke}{aa}{persp}", self.shift)
    }

    // Port of: bench/RectBench.cpp#L63-L80 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        let mut rand = Random::default();
        let offset: scalar = 1.0 / 3.0; // SK_Scalar1/3
        for i in 0..N {
            let mut x = (rand.next_u() % W) as i32;
            let mut y = (rand.next_u() % H) as i32;
            let mut w = (rand.next_u() % W) as i32;
            let mut h = (rand.next_u() % H) as i32;
            w >>= self.shift;
            h >>= self.shift;
            x -= w / 2;
            y -= h / 2;
            self.rects[i].set_xywh(x as scalar, y as scalar, w as scalar, h as scalar);
            self.rects[i].offset((offset, offset));
            self.colors[i] = rand.next_u() | 0xFF80_8080;
        }
    }

    /// `RectBench::setupPaint`: `Benchmark::setupPaint` (anti-aliasing on), then `fAA`.
    // Port of: bench/RectBench.cpp#L104-L107 (chrome/m156)
    fn rect_setup_paint(&self, paint: &mut Paint) {
        paint.set_anti_alias(self.aa);
    }

    /// `RectBench::onDraw`, with the virtual `setupPaint` and `drawThisRect` as closures.
    // Port of: bench/RectBench.cpp#L82-L102 (chrome/m156)
    fn draw(
        &self,
        canvas: &Canvas,
        loops: i32,
        setup_paint: impl Fn(&mut Paint),
        draw_this_rect: impl Fn(&Canvas, &Rect, &Paint),
    ) {
        let mut paint = Paint::default();
        if self.stroke > 0 {
            paint.set_style(Style::Stroke);
            paint.set_stroke_width(self.stroke as scalar);
        }
        if self.perspective {
            // Apply some fixed perspective to change how ops may draw the rects.
            canvas.concat(&perspective_matrix());
        }
        for i in 0..loops {
            let idx = i as usize % N;
            paint.set_color(Color::from(self.colors[idx]));
            setup_paint(&mut paint);
            draw_this_rect(canvas, &self.rects[idx], &paint);
        }
    }
}

/// The `SkMatrix` `RectBench::onDraw` concatenates for perspective rects.
// Port of: bench/RectBench.cpp#L88-L95 (chrome/m156)
fn perspective_matrix() -> Matrix {
    let mut perspective = Matrix::new_identity();
    perspective.set_identity();
    perspective.set_persp_x(1e-4);
    perspective.set_persp_y(1e-3);
    perspective.set_skew_x(0.1);
    perspective
}

/// `RectBench`: `drawRect` of each rect.
// Port of: bench/RectBench.cpp#L20-L112 (chrome/m156)
struct RectBench {
    base: RectBase,
}

impl Benchmark for RectBench {
    // Port of: bench/RectBench.cpp#L61 (chrome/m156)
    fn name(&self) -> String {
        self.base.compute_name("rects")
    }

    // Port of: bench/RectBench.cpp#L63-L80 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        self.base.on_delayed_setup();
    }

    // Port of: bench/RectBench.cpp#L82-L102 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("RectBench is a rendering bench");
        let base = &self.base;
        base.draw(
            canvas,
            loops,
            |paint| base.rect_setup_paint(paint),
            |c, r, p| {
                c.draw_rect(r, p);
            },
        );
    }
}

/// `SrcModeRectBench`: source-mode blending at half alpha.
// Port of: bench/RectBench.cpp#L114-L139 (chrome/m156)
struct SrcModeRectBench {
    base: RectBase,
    mode: BlendMode,
}

impl Benchmark for SrcModeRectBench {
    // Port of: bench/RectBench.cpp#L128-L132 (chrome/m156)
    fn name(&self) -> String {
        format!("srcmode_{}", self.base.compute_name("rects"))
    }

    fn on_delayed_setup(&mut self) {
        self.base.on_delayed_setup();
    }

    // Port of: bench/RectBench.cpp#L120-L126 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("SrcModeRectBench is a rendering bench");
        let base = &self.base;
        let mode = self.mode;
        base.draw(
            canvas,
            loops,
            |paint| {
                base.rect_setup_paint(paint);
                // srcmode is most interesting when we're not opaque
                paint.set_alpha(0x80);
                paint.set_blend_mode(mode);
            },
            |c, r, p| {
                c.draw_rect(r, p);
            },
        );
    }
}

/// `TransparentRectBench`: a non-opaque rect.
// Port of: bench/RectBench.cpp#L141-L161 (chrome/m156)
struct TransparentRectBench {
    base: RectBase,
}

impl Benchmark for TransparentRectBench {
    // Port of: bench/RectBench.cpp#L152-L156 (chrome/m156)
    fn name(&self) -> String {
        format!("transparent_{}", self.base.compute_name("rects"))
    }

    fn on_delayed_setup(&mut self) {
        self.base.on_delayed_setup();
    }

    // Port of: bench/RectBench.cpp#L146-L150 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("TransparentRectBench is a rendering bench");
        let base = &self.base;
        base.draw(
            canvas,
            loops,
            |paint| {
                base.rect_setup_paint(paint);
                // draw non opaque rect
                paint.set_alpha(0x80);
            },
            |c, r, p| {
                c.draw_rect(r, p);
            },
        );
    }
}

/// `LocalCoordsRectBench`: adds a shader that requires local coordinates.
// Port of: bench/RectBench.cpp#L163-L193 (chrome/m156)
struct LocalCoordsRectBench {
    base: RectBase,
    shader: Option<Shader>,
}

impl LocalCoordsRectBench {
    // Port of: bench/RectBench.cpp#L166 (chrome/m156)
    fn new(aa: bool, perspective: bool) -> Self {
        Self {
            base: RectBase::new(1, 0, aa, perspective),
            shader: None,
        }
    }
}

impl Benchmark for LocalCoordsRectBench {
    // Port of: bench/RectBench.cpp#L182-L186 (chrome/m156)
    fn name(&self) -> String {
        format!("{}_localcoords", self.base.compute_name("rects"))
    }

    // Port of: bench/RectBench.cpp#L169-L175 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        self.base.on_delayed_setup();
        // Create the shader once, so that isn't included in the timing.
        let pts = [Point::new(0.0, 0.0), Point::new(50.0, 50.0)];
        let colors = [
            Color4f::new(1.0, 1.0, 1.0, 1.0),
            Color4f::new(0.0, 0.0, 1.0, 1.0),
        ];
        let gradient = Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        );
        self.shader = shaders::linear_gradient((pts[0], pts[1]), &gradient, None);
    }

    // Port of: bench/RectBench.cpp#L177-L180 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("LocalCoordsRectBench is a rendering bench");
        let base = &self.base;
        let shader = &self.shader;
        base.draw(
            canvas,
            loops,
            |paint| {
                base.rect_setup_paint(paint);
                paint.set_shader(shader.clone());
            },
            |c, r, p| {
                c.draw_rect(r, p);
            },
        );
    }
}

/// `OvalBench`: `drawOval` of each rect.
// Port of: bench/RectBench.cpp#L196-L204 (chrome/m156)
struct OvalBench {
    base: RectBase,
}

impl Benchmark for OvalBench {
    // Port of: bench/RectBench.cpp#L203 (chrome/m156)
    fn name(&self) -> String {
        self.base.compute_name("ovals")
    }

    fn on_delayed_setup(&mut self) {
        self.base.on_delayed_setup();
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("OvalBench is a rendering bench");
        let base = &self.base;
        base.draw(
            canvas,
            loops,
            |paint| base.rect_setup_paint(paint),
            |c, r, p| {
                c.draw_oval(r, p);
            },
        );
    }
}

/// `RRectBench`: `drawRoundRect` of each rect, with a quarter-size radius.
// Port of: bench/RectBench.cpp#L206-L214 (chrome/m156)
struct RRectBench {
    base: RectBase,
}

impl Benchmark for RRectBench {
    // Port of: bench/RectBench.cpp#L213 (chrome/m156)
    fn name(&self) -> String {
        self.base.compute_name("rrects")
    }

    fn on_delayed_setup(&mut self) {
        self.base.on_delayed_setup();
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("RRectBench is a rendering bench");
        let base = &self.base;
        base.draw(
            canvas,
            loops,
            |paint| base.rect_setup_paint(paint),
            |c, r, p| {
                c.draw_round_rect(r, r.width() / 4.0, r.height() / 4.0, p);
            },
        );
    }
}

/// The 600 points `reinterpret_cast<SkPoint*>(fRects)` views: each rect's (left, top) and
/// (right, bottom), in memory order.
// Port of: bench/RectBench.cpp#L245 (chrome/m156)
fn rects_as_points(rects: &[Rect]) -> Vec<Point> {
    rects
        .iter()
        .flat_map(|r| [Point::new(r.left, r.top), Point::new(r.right, r.bottom)])
        .collect()
}

/// `PointsBench`: `drawPoints` with a round cap at widths 7 and 0.
// Port of: bench/RectBench.cpp#L216-L255 (chrome/m156)
struct PointsBench {
    base: RectBase,
    mode: PointMode,
    name: String,
}

impl Benchmark for PointsBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_delayed_setup(&mut self) {
        self.base.on_delayed_setup();
    }

    // Port of: bench/RectBench.cpp#L226-L249 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("PointsBench is a rendering bench");
        // gSizes = {7, 0}; FLAGS_strokeWidth is unset (-1), so both sizes are drawn.
        let sizes: [scalar; 2] = [7.0, 0.0];
        let pts = rects_as_points(&self.base.rects);

        let mut paint = Paint::default();
        paint.set_stroke_cap(Cap::Round);

        for _ in 0..loops {
            for (i, size) in sizes.iter().enumerate() {
                paint.set_stroke_width(*size);
                self.base.rect_setup_paint(&mut paint);
                canvas.draw_points(self.mode, &pts, &paint);
                paint.set_color(Color::from(self.base.colors[i % N]));
            }
        }
    }
}

/// `HairPointsBench`: `drawPoints(kPoints)` of 300 random points, 1000 times per loop.
// Port of: bench/RectBench.cpp#L257-L307 (chrome/m156)
struct HairPointsBench {
    bm: BlendMode,
    alpha: f32,
    pts: Vec<Point>,
    name: String,
}

impl HairPointsBench {
    // Port of: bench/RectBench.cpp#L268-L272 (chrome/m156)
    fn new(bm: BlendMode, alpha: f32) -> Self {
        Self {
            bm,
            alpha,
            pts: vec![Point::default(); N],
            name: format!("hair_points_mode_{}_alpha_{alpha}", bm.name()),
        }
    }
}

const HAIR_W: scalar = 640.0;
const HAIR_H: scalar = 480.0;

impl Benchmark for HairPointsBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/RectBench.cpp#L275-L283 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        if backend == Backend::NonRendering {
            return false;
        }
        // seems to be a bug on graphic (mali) + src_mode
        let shows_bug = self.bm == BlendMode::Src && backend == Backend::Graphite;
        !shows_bug
    }

    // Port of: bench/RectBench.cpp#L287-L294 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        let mut rand = Random::default();
        for p in &mut self.pts {
            let x = rand.next_f() * HAIR_W;
            let y = rand.next_f() * HAIR_H;
            *p = Point::new(x, y);
        }
    }

    // Port of: bench/RectBench.cpp#L296-L306 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("HairPointsBench is a rendering bench");
        let mut paint = Paint::default();
        paint.set_blend_mode(self.bm);
        paint.set_alpha_f(self.alpha);
        paint.set_stroke_width(0.0); // we're hairpoints

        for _ in 0..loops {
            for _ in 0..1000 {
                canvas.draw_points(PointMode::Points, &self.pts, &paint);
            }
        }
    }
}

/// `BlitMaskBench::kMaskType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaskType {
    Opaque,
    Black,
    Color,
    Shader,
}

/// `BlitMaskBench`: `drawPoints` of the rect points with a mask of each type.
// Port of: bench/RectBench.cpp#L314-L385 (chrome/m156)
struct BlitMaskBench {
    base: RectBase,
    mode: PointMode,
    mask_type: MaskType,
    name: String,
}

impl Benchmark for BlitMaskBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_delayed_setup(&mut self) {
        self.base.on_delayed_setup();
    }

    // Port of: bench/RectBench.cpp#L331-L378 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("BlitMaskBench is a rendering bench");
        // gSizes = {13, 24}; FLAGS_strokeWidth is unset (-1), so both sizes are drawn.
        let sizes: [scalar; 2] = [13.0, 24.0];
        let pts = rects_as_points(&self.base.rects);
        let mut rand = Random::default();
        let mut color: u32 = 0xFF00_0000;
        let mut alpha: u8 = 0xFF;
        let mut paint = Paint::default();
        paint.set_stroke_cap(Cap::Round);
        if self.mask_type == MaskType::Shader {
            let mut src_bm = Bitmap::new();
            src_bm.alloc_n32_pixels((10, 1), None);
            src_bm.erase_color(Color::from(0xFF00_FF00_u32));
            paint.set_shader(src_bm.to_shader(None, SamplingOptions::default(), None));
        }
        for _ in 0..loops {
            for (i, size) in sizes.iter().enumerate() {
                match self.mask_type {
                    MaskType::Opaque => {
                        color = self.base.colors[i];
                        alpha = 0xFF;
                    }
                    MaskType::Black => {
                        alpha = 0xFF;
                        color = 0xFF00_0000;
                    }
                    MaskType::Color => {
                        color = self.base.colors[i];
                        alpha = (rand.next_u() & 255) as u8;
                    }
                    MaskType::Shader => {}
                }
                paint.set_stroke_width(*size);
                self.base.rect_setup_paint(&mut paint);
                paint.set_color(Color::from(color));
                paint.set_alpha(alpha);
                canvas.draw_points(self.mode, &pts, &paint);
            }
        }
    }
}

/// `RectBoundsBench`: `SkRect::Bounds` of `count` random points (non-rendering).
// Port of: bench/RectBench.cpp#L444-L478 (chrome/m156)
struct RectBoundsBench {
    name: String,
    points: Vec<Point>,
}

impl RectBoundsBench {
    // Port of: bench/RectBench.cpp#L449-L452 (chrome/m156)
    fn new(count: usize) -> Self {
        Self {
            name: format!("rect_bounds_{count}"),
            points: vec![Point::default(); count],
        }
    }
}

impl Benchmark for RectBoundsBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/RectBench.cpp#L469-L471 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/RectBench.cpp#L460-L467 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        let mut rand = Random::default();
        for p in &mut self.points {
            let x = rand.next_f();
            let y = rand.next_f();
            *p = Point::new(x, y);
        }
    }

    // Port of: bench/RectBench.cpp#L473-L477 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            // (void)SkRect::Bounds(fPoints): the result is discarded; black_box keeps the call.
            std::hint::black_box(Rect::bounds(&self.points));
        }
    }
}

// Port of: bench/RectBench.cpp#L388-L388 (chrome/m156)
def_bench!(
    rect_bench_1_0_true = "RectBench(1, 0, true)",
    RectBench {
        base: RectBase::new(1, 0, true, false)
    }
);
// Port of: bench/RectBench.cpp#L389-L389 (chrome/m156)
def_bench!(
    rect_bench_1_4_true = "RectBench(1, 4, true)",
    RectBench {
        base: RectBase::new(1, 4, true, false)
    }
);
// Port of: bench/RectBench.cpp#L390-L390 (chrome/m156)
def_bench!(
    rect_bench_3_0_true = "RectBench(3, 0, true)",
    RectBench {
        base: RectBase::new(3, 0, true, false)
    }
);
// Port of: bench/RectBench.cpp#L391-L391 (chrome/m156)
def_bench!(
    rect_bench_3_4_true = "RectBench(3, 4, true)",
    RectBench {
        base: RectBase::new(3, 4, true, false)
    }
);
// Port of: bench/RectBench.cpp#L393-L393 (chrome/m156)
def_bench!(
    rect_bench_1_0_false = "RectBench(1, 0, false)",
    RectBench {
        base: RectBase::new(1, 0, false, false)
    }
);
// Port of: bench/RectBench.cpp#L394-L394 (chrome/m156)
def_bench!(
    rect_bench_1_4_false = "RectBench(1, 4, false)",
    RectBench {
        base: RectBase::new(1, 4, false, false)
    }
);
// Port of: bench/RectBench.cpp#L395-L395 (chrome/m156)
def_bench!(
    rect_bench_3_0_false = "RectBench(3, 0, false)",
    RectBench {
        base: RectBase::new(3, 0, false, false)
    }
);
// Port of: bench/RectBench.cpp#L396-L396 (chrome/m156)
def_bench!(
    rect_bench_3_4_false = "RectBench(3, 4, false)",
    RectBench {
        base: RectBase::new(3, 4, false, false)
    }
);

// Port of: bench/RectBench.cpp#L398-L398 (chrome/m156)
def_bench!(
    rect_bench_oval_1 = "OvalBench(1)",
    OvalBench {
        base: RectBase::new(1, 0, true, false)
    }
);
// Port of: bench/RectBench.cpp#L399-L399 (chrome/m156)
def_bench!(
    rect_bench_oval_3 = "OvalBench(3)",
    OvalBench {
        base: RectBase::new(3, 0, true, false)
    }
);
// Port of: bench/RectBench.cpp#L400-L400 (chrome/m156)
def_bench!(
    rect_bench_oval_1_4 = "OvalBench(1, 4)",
    OvalBench {
        base: RectBase::new(1, 4, true, false)
    }
);
// Port of: bench/RectBench.cpp#L401-L401 (chrome/m156)
def_bench!(
    rect_bench_oval_3_4 = "OvalBench(3, 4)",
    OvalBench {
        base: RectBase::new(3, 4, true, false)
    }
);
// Port of: bench/RectBench.cpp#L402-L402 (chrome/m156)
def_bench!(
    rect_bench_rrect_1 = "RRectBench(1)",
    RRectBench {
        base: RectBase::new(1, 0, true, false)
    }
);
// Port of: bench/RectBench.cpp#L403-L403 (chrome/m156)
def_bench!(
    rect_bench_rrect_1_4 = "RRectBench(1, 4)",
    RRectBench {
        base: RectBase::new(1, 4, true, false)
    }
);
// Port of: bench/RectBench.cpp#L404-L404 (chrome/m156)
def_bench!(
    rect_bench_rrect_3 = "RRectBench(3)",
    RRectBench {
        base: RectBase::new(3, 0, true, false)
    }
);
// Port of: bench/RectBench.cpp#L405-L405 (chrome/m156)
def_bench!(
    rect_bench_rrect_3_4 = "RRectBench(3, 4)",
    RRectBench {
        base: RectBase::new(3, 4, true, false)
    }
);

// Port of: bench/RectBench.cpp#L407-L407 (chrome/m156)
def_bench!(
    rect_bench_hair_points_src_over_half = "HairPointsBench(SkBlendMode::kSrcOver, 0.5f)",
    HairPointsBench::new(BlendMode::SrcOver, 0.5)
);
// Port of: bench/RectBench.cpp#L408-L408 (chrome/m156)
def_bench!(
    rect_bench_hair_points_src_over_1 = "HairPointsBench(SkBlendMode::kSrcOver, 1)",
    HairPointsBench::new(BlendMode::SrcOver, 1.0)
);
// Port of: bench/RectBench.cpp#L409-L409 (chrome/m156)
def_bench!(
    rect_bench_hair_points_src_half = "HairPointsBench(SkBlendMode::kSrc, 0.5f)",
    HairPointsBench::new(BlendMode::Src, 0.5)
);
// Port of: bench/RectBench.cpp#L410-L410 (chrome/m156)
def_bench!(
    rect_bench_hair_points_src_1 = "HairPointsBench(SkBlendMode::kSrc, 1)",
    HairPointsBench::new(BlendMode::Src, 1.0)
);

// Port of: bench/RectBench.cpp#L412-L412 (chrome/m156)
def_bench!(
    rect_bench_points = r#"PointsBench(SkCanvas::kPoints_PointMode, "points")"#,
    PointsBench {
        base: RectBase::new(2, 0, true, false),
        mode: PointMode::Points,
        name: "points".to_owned(),
    }
);
// Port of: bench/RectBench.cpp#L413-L413 (chrome/m156)
def_bench!(
    rect_bench_lines = r#"PointsBench(SkCanvas::kLines_PointMode, "lines")"#,
    PointsBench {
        base: RectBase::new(2, 0, true, false),
        mode: PointMode::Lines,
        name: "lines".to_owned(),
    }
);
// Port of: bench/RectBench.cpp#L414-L414 (chrome/m156)
def_bench!(
    rect_bench_polygon = r#"PointsBench(SkCanvas::kPolygon_PointMode, "polygon")"#,
    PointsBench {
        base: RectBase::new(2, 0, true, false),
        mode: PointMode::Polygon,
        name: "polygon".to_owned(),
    }
);

// Port of: bench/RectBench.cpp#L416-L416 (chrome/m156)
def_bench!(
    rect_bench_src_mode = "SrcModeRectBench()",
    SrcModeRectBench {
        base: RectBase::new(1, 0, true, false),
        mode: BlendMode::Src,
    }
);

// Port of: bench/RectBench.cpp#L418-L418 (chrome/m156)
def_bench!(
    rect_bench_transparent = "TransparentRectBench()",
    TransparentRectBench {
        base: RectBase::new(1, 0, true, false)
    }
);

// Port of: bench/RectBench.cpp#L420-L420 (chrome/m156)
def_bench!(
    rect_bench_local_coords_true = "LocalCoordsRectBench(true)",
    LocalCoordsRectBench::new(true, false)
);
// Port of: bench/RectBench.cpp#L421-L421 (chrome/m156)
def_bench!(
    rect_bench_local_coords_false = "LocalCoordsRectBench(false)",
    LocalCoordsRectBench::new(false, false)
);

// Perspective rects.
// Port of: bench/RectBench.cpp#L424-L424 (chrome/m156)
def_bench!(
    rect_bench_1_0_true_persp = "RectBench(1, 0, true, true)",
    RectBench {
        base: RectBase::new(1, 0, true, true)
    }
);
// Port of: bench/RectBench.cpp#L425-L425 (chrome/m156)
def_bench!(
    rect_bench_1_0_false_persp = "RectBench(1, 0, false, true)",
    RectBench {
        base: RectBase::new(1, 0, false, true)
    }
);
// Port of: bench/RectBench.cpp#L426-L426 (chrome/m156)
def_bench!(
    rect_bench_local_coords_true_persp = "LocalCoordsRectBench(true, true)",
    LocalCoordsRectBench::new(true, true)
);
// Port of: bench/RectBench.cpp#L427-L427 (chrome/m156)
def_bench!(
    rect_bench_local_coords_false_persp = "LocalCoordsRectBench(false, true)",
    LocalCoordsRectBench::new(false, true)
);

// Port of: bench/RectBench.cpp#L431-L433 (chrome/m156)
def_bench!(
    rect_bench_mask_opaque =
        r#"BlitMaskBench(SkCanvas::kPoints_PointMode, BlitMaskBench::kMaskOpaque, "maskopaque")"#,
    BlitMaskBench {
        base: RectBase::new(2, 0, true, false),
        mode: PointMode::Points,
        mask_type: MaskType::Opaque,
        name: "maskopaque".to_owned(),
    }
);
// Port of: bench/RectBench.cpp#L434-L436 (chrome/m156)
def_bench!(
    rect_bench_mask_black =
        r#"BlitMaskBench(SkCanvas::kPoints_PointMode, BlitMaskBench::kMaskBlack, "maskblack")"#,
    BlitMaskBench {
        base: RectBase::new(2, 0, true, false),
        mode: PointMode::Points,
        mask_type: MaskType::Black,
        name: "maskblack".to_owned(),
    }
);
// Port of: bench/RectBench.cpp#L437-L439 (chrome/m156)
def_bench!(
    rect_bench_mask_color =
        r#"BlitMaskBench(SkCanvas::kPoints_PointMode, BlitMaskBench::kMaskColor, "maskcolor")"#,
    BlitMaskBench {
        base: RectBase::new(2, 0, true, false),
        mode: PointMode::Points,
        mask_type: MaskType::Color,
        name: "maskcolor".to_owned(),
    }
);
// Port of: bench/RectBench.cpp#L440-L442 (chrome/m156)
def_bench!(
    rect_bench_mask_shader =
        r#"BlitMaskBench(SkCanvas::kPoints_PointMode, BlitMaskBench::KMaskShader, "maskshader")"#,
    BlitMaskBench {
        base: RectBase::new(2, 0, true, false),
        mode: PointMode::Points,
        mask_type: MaskType::Shader,
        name: "maskshader".to_owned(),
    }
);

// Port of: bench/RectBench.cpp#L480-L480 (chrome/m156)
def_bench!(
    rect_bench_rect_bounds_4 = "RectBoundsBench(4)",
    RectBoundsBench::new(4)
);
// Port of: bench/RectBench.cpp#L481-L481 (chrome/m156)
def_bench!(
    rect_bench_rect_bounds_400 = "RectBoundsBench(400)",
    RectBoundsBench::new(400)
);
