// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/strokes.cpp (chrome/m156)

// Float literals are copied verbatim from the C++ source.
#![allow(clippy::excessive_precision)]

use crate::prelude::*;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::path_utils::fill_path_with_paint_to_path;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::SCALAR_PI;
use skia_rust_core::utils::parse_path;
use skia_rust_effects::dash_path_effect;

// Port of: gm/strokes.cpp#L39-L41 (chrome/m156)
const W: i32 = 400;
const H: i32 = 400;
const N: i32 = 50;

// Port of: gm/strokes.cpp#L43-L44 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // SkIntToScalar
const SW: f32 = W as f32;
#[allow(clippy::cast_precision_loss)] // SkIntToScalar
const SH: f32 = H as f32;

// Port of: gm/strokes.cpp#L46-L58 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // int to float conversions as in C++
fn rnd_rect(r: &mut Rect, paint: &mut Paint, rand: &mut Random) {
    let x = rand.next_u_scalar1() * W as f32;
    let y = rand.next_u_scalar1() * H as f32;
    let w = rand.next_u_scalar1() * (W >> 2) as f32;
    let h = rand.next_u_scalar1() * (H >> 2) as f32;
    let hoffset = rand.next_s_scalar1();
    let woffset = rand.next_s_scalar1();

    r.set_xywh(x, y, w, h);
    r.offset((-w / 2.0 + woffset, -h / 2.0 + hoffset));

    paint.set_color(rand.next_u());
    paint.set_alpha_f(1.0);
}

// Port of: gm/strokes.cpp#L61-L96 (chrome/m156)
struct StrokesGm;

impl GM for StrokesGm {
    fn name(&self) -> String {
        "strokes_round".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(W, H * 2)
    }

    #[allow(clippy::cast_precision_loss)] // SH * y
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(9.0 / 2.0);

        for y in 0..2 {
            paint.set_anti_alias(y != 0);
            let _acr = AutoCanvasRestore::guard(canvas, true);
            canvas.translate((0.0, SH * y as f32));
            canvas.clip_rect(Rect::from_ltrb(2.0, 2.0, SW - 2.0, SH - 2.0), None, None);

            let mut rand = Random::default();
            for _ in 0..N {
                let mut r = Rect::new_empty();
                rnd_rect(&mut r, &mut paint, &mut rand);
                canvas.draw_oval(r, &paint);
                rnd_rect(&mut r, &mut paint, &mut rand);
                canvas.draw_round_rect(r, r.width() / 4.0, r.height() / 4.0, &paint);
                rnd_rect(&mut r, &mut paint, &mut rand);
            }
        }
    }
}

// See https://code.google.com/p/chromium/issues/detail?id=422974 and
// http://jsfiddle.net/1xnku3sg/2/
// Port of: gm/strokes.cpp#L102-L194 (chrome/m156)
#[derive(Default)]
struct ZeroLenStrokesGm {
    move_hf_path: Path,
    move_zf_path: Path,
    dashedf_path: Path,
    ref_path: [Path; 4],
    cubic_path: Path,
    quad_path: Path,
    line_path: Path,
}

impl GM for ZeroLenStrokesGm {
    fn name(&self) -> String {
        "zeroPath".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(W, H * 2)
    }

    #[allow(clippy::cast_precision_loss)] // i * 10.f
    fn on_once_before_draw(&mut self) {
        let parse_assert_result = |s: &str| parse_path::from_svg(s).expect("SkAssertResult");

        self.move_hf_path = parse_assert_result("M0,0h0M10,0h0M20,0h0");
        self.move_zf_path = parse_assert_result("M0,0zM10,0zM20,0z");
        self.dashedf_path = parse_assert_result("M0,0h25");
        self.cubic_path = parse_assert_result("M 0 0 C 0 0 0 0 0 0");
        self.quad_path = parse_assert_result("M 0 0 Q 0 0 0 0");
        self.line_path = parse_assert_result("M 0 0 L 0 0");

        let mut builders = [
            PathBuilder::new(),
            PathBuilder::new(),
            PathBuilder::new(),
            PathBuilder::new(),
        ];
        for i in 0..3 {
            let f = i as f32 * 10.0;
            builders[0].add_circle((f, 0.0), 5.0, None);
            builders[1].add_circle((f, 0.0), 10.0, None);
            builders[2].add_rect(Rect::new(f - 4.0, -2.0, f + 4.0, 6.0), None, None);
            builders[3].add_rect(Rect::new(f - 10.0, -10.0, f + 10.0, 10.0), None, None);
        }
        for (i, builder) in builders.iter_mut().enumerate() {
            self.ref_path[i] = builder.detach();
        }
    }

    #[allow(clippy::cast_precision_loss)] // i * 100.f
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut fill_paint = Paint::default();
        fill_paint.set_anti_alias(true);
        let mut stroke_paint = fill_paint.clone();
        stroke_paint.set_style(Style::Stroke);
        for i in 0..2usize {
            fill_paint.set_alpha_f(1.0);
            stroke_paint.set_alpha_f(1.0);
            stroke_paint.set_stroke_width(if i != 0 { 8.0 } else { 10.0 });
            stroke_paint.set_stroke_cap(if i != 0 { Cap::Square } else { Cap::Round });
            canvas.save();
            canvas.translate((10.0 + i as f32 * 100.0, 10.0));
            canvas.draw_path(&self.move_hf_path, &stroke_paint);
            canvas.translate((0.0, 20.0));
            canvas.draw_path(&self.move_zf_path, &stroke_paint);
            let mut dash_paint = stroke_paint.clone();
            let intervals = [0.0, 10.0];
            dash_paint.set_path_effect(dash_path_effect::new(&intervals, 0.0));
            let _ = fill_path_with_paint_to_path(&self.dashedf_path, &dash_paint);
            canvas.translate((0.0, 20.0));
            canvas.draw_path(&self.dashedf_path, &dash_paint);
            canvas.translate((0.0, 20.0));
            canvas.draw_path(&self.ref_path[i * 2], &fill_paint);
            stroke_paint.set_stroke_width(20.0);
            stroke_paint.set_alpha_f(0.5);
            canvas.translate((0.0, 50.0));
            canvas.draw_path(&self.move_hf_path, &stroke_paint);
            canvas.translate((0.0, 30.0));
            canvas.draw_path(&self.move_zf_path, &stroke_paint);
            canvas.translate((0.0, 30.0));
            fill_paint.set_alpha_f(0.5);
            canvas.draw_path(&self.ref_path[1 + i * 2], &fill_paint);
            canvas.translate((0.0, 30.0));
            canvas.draw_path(&self.cubic_path, &stroke_paint);
            canvas.translate((0.0, 30.0));
            canvas.draw_path(&self.quad_path, &stroke_paint);
            canvas.translate((0.0, 30.0));
            canvas.draw_path(&self.line_path, &stroke_paint);
            canvas.restore();
        }
    }
}

// Port of: gm/strokes.cpp#L196-L228 (chrome/m156)
struct TeenyStrokesGm;

impl TeenyStrokesGm {
    // Port of: gm/strokes.cpp#L200-L212 (chrome/m156)
    fn line(scale: f32, canvas: &Canvas, color: Color) {
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_style(Style::Stroke);
        p.set_color(color);
        canvas.translate((50.0, 0.0));
        canvas.save();
        p.set_stroke_width(scale * 5.0);
        canvas.scale((1.0 / scale, 1.0 / scale));
        canvas.draw_line((20.0 * scale, 20.0 * scale), (20.0 * scale, 100.0 * scale), &p);
        canvas.draw_line((20.0 * scale, 20.0 * scale), (100.0 * scale, 100.0 * scale), &p);
        canvas.restore();
    }
}

impl GM for TeenyStrokesGm {
    fn name(&self) -> String {
        "teenyStrokes".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(W, H * 2)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        Self::line(0.00005, canvas, Color::BLACK);
        Self::line(0.000045, canvas, Color::RED);
        Self::line(0.0000035, canvas, Color::GREEN);
        Self::line(0.000003, canvas, Color::BLUE);
        Self::line(0.000002, canvas, Color::BLACK);
    }
}

// Port of: gm/strokes.cpp#L230-L251 (chrome/m156)
crate::def_simple_gm!(CubicStroke, canvas, 384, 384, {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_style(Style::Stroke);
    p.set_stroke_width(1.0720);
    let path = PathBuilder::new()
        .move_to((-6000.0, -6000.0))
        .cubic_to((-3500.0, 5500.0), (-500.0, 5500.0), (2500.0, -6500.0))
        .detach();
    canvas.draw_path(&path, &p);
    p.set_stroke_width(1.0721);
    canvas.translate((10.0, 10.0));
    canvas.draw_path(&path, &p);
    p.set_stroke_width(1.0722);
    canvas.translate((10.0, 10.0));
    canvas.draw_path(&path, &p);
});

// Port of: gm/strokes.cpp#L253-L278 (chrome/m156)
crate::def_simple_gm!(zerolinestroke, canvas, 90, 120, {
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(20.0);
    paint.set_anti_alias(true);
    paint.set_stroke_cap(Cap::Round);

    let mut path = PathBuilder::new()
        .move_to((30.0, 90.0))
        .line_to((30.0, 90.0))
        .line_to((60.0, 90.0))
        .line_to((60.0, 90.0))
        .detach();
    canvas.draw_path(&path, &paint);

    path = Path::line((30.0, 30.0), (60.0, 30.0));
    canvas.draw_path(&path, &paint);

    path = PathBuilder::new()
        .move_to((30.0, 60.0))
        .line_to((30.0, 60.0))
        .line_to((60.0, 60.0))
        .detach();
    canvas.draw_path(&path, &paint);
});

// Port of: gm/strokes.cpp#L280-L311 (chrome/m156)
crate::def_simple_gm!(quadcap, canvas, 200, 200, {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_style(Style::Stroke);
    p.set_stroke_width(0.0);
    let pts = [
        Point::new(105.738571, 13.126318),
        Point::new(105.738571, 13.126318),
        Point::new(123.753784, 1.0),
    ];
    let mut tangent: Vector = pts[1] - pts[2];
    tangent.normalize();
    let mut pts2 = pts;
    let cap_outset = SCALAR_PI / 8.0;
    pts2[0].x += tangent.x * cap_outset;
    pts2[0].y += tangent.y * cap_outset;
    pts2[1].x += tangent.x * cap_outset;
    pts2[1].y += tangent.y * cap_outset;
    pts2[2].x += -tangent.x * cap_outset;
    pts2[2].y += -tangent.y * cap_outset;

    let mut path = PathBuilder::new()
        .move_to(pts2[0])
        .quad_to(pts2[1], pts2[2])
        .detach();
    canvas.draw_path(&path, &p);

    path = PathBuilder::new()
        .move_to(pts[0])
        .quad_to(pts[1], pts[2])
        .detach();
    p.set_stroke_cap(Cap::Round);
    canvas.translate((30.0, 0.0));
    canvas.draw_path(&path, &p);
});

// Port of: gm/strokes.cpp#L313-L356 (chrome/m156)
#[derive(Default)]
struct Strokes2Gm {
    path: Path,
}

impl GM for Strokes2Gm {
    fn name(&self) -> String {
        "strokes_poly".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(W, H * 2)
    }

    #[allow(clippy::cast_precision_loss)] // W >> 1
    fn on_once_before_draw(&mut self) {
        let mut rand = Random::default();
        let mut builder = PathBuilder::new();
        builder.move_to((0.0, 0.0));
        for _ in 0..13 {
            let x = rand.next_u_scalar1() * (W >> 1) as f32;
            let y = rand.next_u_scalar1() * (H >> 1) as f32;
            builder.line_to((x, y));
        }
        self.path = builder.detach();
    }

    #[allow(clippy::cast_precision_loss)] // SH * y
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.draw_color(Color::WHITE, None);

        let mut paint = Paint::default();
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(9.0 / 2.0);

        for y in 0..2 {
            paint.set_anti_alias(y != 0);
            let _acr = AutoCanvasRestore::guard(canvas, true);
            canvas.translate((0.0, SH * y as f32));
            canvas.clip_rect(Rect::from_ltrb(2.0, 2.0, SW - 2.0, SH - 2.0), None, None);

            let mut rand = Random::default();
            for _ in 0..N / 2 {
                let mut r = Rect::new_empty();
                rnd_rect(&mut r, &mut paint, &mut rand);
                canvas.rotate(15.0, Some(Point::new(SW / 2.0, SH / 2.0)));
                canvas.draw_path(&self.path, &paint);
            }
        }
    }
}

// Port of: gm/strokes.cpp#L360-L364 (chrome/m156)
fn inset(r: &Rect) -> Rect {
    let mut rr = *r;
    rr.inset((r.width() / 10.0, r.height() / 10.0));
    rr
}

// Port of: gm/strokes.cpp#L366-L487 (chrome/m156)
struct Strokes3Gm;

impl Strokes3Gm {
    // Port of: gm/strokes.cpp#L367-L373 (chrome/m156)
    fn make0(bounds: &Rect) -> Path {
        PathBuilder::new()
            .add_rect(bounds, PathDirection::CW, None)
            .add_rect(inset(bounds), PathDirection::CW, None)
            .detach()
    }

    // Port of: gm/strokes.cpp#L375-L381 (chrome/m156)
    fn make1(bounds: &Rect) -> Path {
        PathBuilder::new()
            .add_rect(bounds, PathDirection::CW, None)
            .add_rect(inset(bounds), PathDirection::CCW, None)
            .detach()
    }

    // Port of: gm/strokes.cpp#L383-L389 (chrome/m156)
    fn make2(bounds: &Rect) -> Path {
        PathBuilder::new()
            .add_oval(bounds, PathDirection::CW, None)
            .add_oval(inset(bounds), PathDirection::CW, None)
            .detach()
    }

    // Port of: gm/strokes.cpp#L391-L397 (chrome/m156)
    fn make3(bounds: &Rect) -> Path {
        PathBuilder::new()
            .add_oval(bounds, PathDirection::CW, None)
            .add_oval(inset(bounds), PathDirection::CCW, None)
            .detach()
    }

    // Port of: gm/strokes.cpp#L399-L407 (chrome/m156)
    fn make4(bounds: &Rect) -> Path {
        let mut r = *bounds;
        r.inset((bounds.width() / 10.0, -bounds.height() / 10.0));
        PathBuilder::new()
            .add_rect(bounds, PathDirection::CW, None)
            .add_oval(r, PathDirection::CW, None)
            .detach()
    }

    // Port of: gm/strokes.cpp#L409-L417 (chrome/m156)
    fn make5(bounds: &Rect) -> Path {
        let mut r = *bounds;
        r.inset((bounds.width() / 10.0, -bounds.height() / 10.0));
        PathBuilder::new()
            .add_rect(bounds, PathDirection::CW, None)
            .add_oval(r, PathDirection::CCW, None)
            .detach()
    }
}

impl GM for Strokes3Gm {
    fn name(&self) -> String {
        "strokes3".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1500, 1500)
    }

    #[allow(clippy::cast_precision_loss)] // SK_Scalar1 * j * j
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut orig_paint = Paint::default();
        orig_paint.set_anti_alias(true);
        orig_paint.set_style(Style::Stroke);
        let mut fill_paint = orig_paint.clone();
        fill_paint.set_color(Color::RED);
        let mut stroke_paint = orig_paint.clone();
        stroke_paint.set_color(color_to_565(0xFF4444FF));

        let procs: [fn(&Rect) -> Path; 6] = [
            Self::make0,
            Self::make1,
            Self::make2,
            Self::make3,
            Self::make4,
            Self::make5,
        ];

        canvas.translate((20.0, 80.0));

        let bounds = Rect::from_wh(50.0, 50.0);
        let dx = bounds.width() * 4.0 / 3.0;
        let dy = bounds.height() * 5.0;

        for make in procs {
            let orig = make(&bounds);

            canvas.save();
            for j in 0..13 {
                stroke_paint.set_stroke_width(1.0 * j as f32 * j as f32);
                canvas.draw_path(&orig, &stroke_paint);
                canvas.draw_path(&orig, &orig_paint);
                let (fill, _) = fill_path_with_paint_to_path(&orig, &stroke_paint);
                canvas.draw_path(&fill, &fill_paint);
                canvas.translate((dx + stroke_paint.stroke_width(), 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, dy));
        }
    }
}

// `ToolUtils::color_to_565`.
// Port of: tools/ToolUtils.cpp#L142-L151 (chrome/m156)
fn color_to_565(color: u32) -> Color {
    use skia_rust_core::color::pre_multiply_color;
    use skia_rust_core::color_data::{pixel16_to_color, pixel32_to_pixel16};
    let pm_color = pre_multiply_color(Color::new(color));
    let color16 = pixel32_to_pixel16(pm_color);
    pixel16_to_color(color16)
}

// Port of: gm/strokes.cpp#L489-L508 (chrome/m156)
struct Strokes4Gm;

impl GM for Strokes4Gm {
    fn name(&self) -> String {
        "strokes_zoomed".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(W, H * 2)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(0.055);

        canvas.scale((1000.0, 1000.0));
        canvas.draw_circle((0.0, 2.0), 1.97, &paint);
    }
}

// Test stroking for curves that produce degenerate tangents when t is 0 or 1
// (skbug.com/40035337)
// Port of: gm/strokes.cpp#L510-L564 (chrome/m156)
struct Strokes5Gm;

impl GM for Strokes5Gm {
    fn name(&self) -> String {
        "zero_control_stroke".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(W, H * 2)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut p = Paint::default();
        p.set_color(Color::RED);
        p.set_anti_alias(true);
        p.set_style(Style::Stroke);
        p.set_stroke_width(40.0);
        p.set_stroke_cap(Cap::Butt);

        let mut path = PathBuilder::new()
            .move_to((157.474, 111.753))
            .cubic_to((128.5, 111.5), (35.5, 29.5), (35.5, 29.5))
            .detach();
        canvas.draw_path(&path, &p);
        path = PathBuilder::new()
            .move_to((250.0, 50.0))
            .quad_to((280.0, 80.0), (280.0, 80.0))
            .detach();
        canvas.draw_path(&path, &p);
        path = PathBuilder::new()
            .move_to((150.0, 50.0))
            .conic_to((180.0, 80.0), (180.0, 80.0), 0.707)
            .detach();
        canvas.draw_path(&path, &p);

        path = PathBuilder::new()
            .move_to((157.474, 311.753))
            .cubic_to((157.474, 311.753), (85.5, 229.5), (35.5, 229.5))
            .detach();
        canvas.draw_path(&path, &p);
        path = PathBuilder::new()
            .move_to((280.0, 250.0))
            .quad_to((280.0, 250.0), (310.0, 280.0))
            .detach();
        canvas.draw_path(&path, &p);
        path = PathBuilder::new()
            .move_to((180.0, 250.0))
            .conic_to((180.0, 250.0), (210.0, 280.0), 0.707)
            .detach();
        canvas.draw_path(&path, &p);
    }
}

// Port of: gm/strokes.cpp#L570-L574 (chrome/m156)
crate::def_gm!(StrokesGM, StrokesGm);
crate::def_gm!(Strokes2GM, Strokes2Gm::default());
crate::def_gm!(Strokes3GM, Strokes3Gm);
crate::def_gm!(Strokes4GM, Strokes4Gm);
crate::def_gm!(Strokes5GM, Strokes5Gm);

// Port of: gm/strokes.cpp#L576-L577 (chrome/m156)
crate::def_gm!(ZeroLenStrokesGM, ZeroLenStrokesGm::default());
crate::def_gm!(TeenyStrokesGM, TeenyStrokesGm);

// Port of: gm/strokes.cpp#L579-L592 (chrome/m156)
crate::def_simple_gm!(zerolinedash, canvas, 256, 256, {
    canvas.clear(Color::WHITE);

    let mut paint = Paint::default();
    paint.set_color(Color::from_argb(255, 0, 0, 0));
    paint.set_stroke_width(11.0);
    paint.set_stroke_cap(Cap::Round);
    paint.set_stroke_join(Join::Bevel);

    let dash_pattern = [1.0, 5.0];
    paint.set_path_effect(dash_path_effect::new(&dash_pattern, 0.0));

    canvas.draw_line((100.0, 100.0), (100.0, 100.0), &paint);
});

// Port of: gm/strokes.cpp#L621-L655 (chrome/m156)
crate::def_simple_gm!(inner_join_geometry, canvas, 1000, 700, {
    // These paths trigger cases where we must add inner join geometry.
    // skbug.com/40043052
    #[rustfmt::skip]
    let path_points: [Point; 24] = [
        // moveTo               lineTo                  lineTo
        Point::new(119.0,  71.0), Point::new(129.0, 151.0), Point::new(230.0,  24.0),
        Point::new(200.0, 144.0), Point::new(129.0, 151.0), Point::new(230.0,  24.0),
        Point::new(192.0, 176.0), Point::new(224.0, 175.0), Point::new(281.0, 103.0),
        Point::new(233.0, 205.0), Point::new(224.0, 175.0), Point::new(281.0, 103.0),
        Point::new(121.0, 216.0), Point::new(234.0, 189.0), Point::new(195.0, 147.0),
        Point::new(141.0, 216.0), Point::new(254.0, 189.0), Point::new(238.0, 250.0),
        Point::new(159.0, 202.0), Point::new(269.0, 197.0), Point::new(289.0, 165.0),
        Point::new(159.0, 202.0), Point::new(269.0, 197.0), Point::new(287.0, 227.0),
    ];

    let mut path_paint = Paint::default();
    path_paint.set_stroke(true);
    path_paint.set_anti_alias(true);
    path_paint.set_stroke_width(100.0);

    let mut skeleton_paint = Paint::default();
    skeleton_paint.set_stroke(true);
    skeleton_paint.set_anti_alias(true);
    skeleton_paint.set_stroke_width(0.0);
    skeleton_paint.set_color(Color::RED);

    canvas.translate((0.0, 50.0));
    for i in 0..path_points.len() / 3 {
        let path = Path::polygon(&path_points[i * 3..i * 3 + 3], false, None, None);
        canvas.draw_path(&path, &path_paint);

        let (fill_path, _) = fill_path_with_paint_to_path(&path, &path_paint);
        canvas.draw_path(&fill_path, &skeleton_paint);

        canvas.translate((200.0, 0.0));
        if (i + 1) % 4 == 0 {
            canvas.translate((-800.0, 200.0));
        }
    }
});

// Port of: gm/strokes.cpp#L657-L682 (chrome/m156)
crate::def_simple_gm!(skbug12244, canvas, 150, 150, {
    // Should look like a stroked triangle; these vertices are the results of the SkStroker
    // but we draw as a filled path in order to highlight that it's the GPU triangulating path
    // renderer that's the source of the problem, and not the stroking operation. The original
    // path was a simple:
    // m(0,0), l(100, 40), l(0, 80), l(0,0) with a stroke width of 15px
    let path = PathBuilder::new()
        .move_to((2.7854299545288085938, -6.9635753631591796875))
        .line_to((120.194366455078125, 40.0))
        .line_to((-7.5000004768371582031, 91.07775115966796875))
        .line_to((-7.5000004768371582031, -11.077748298645019531))
        .line_to((2.7854299545288085938, -6.9635753631591796875))
        .move_to((-2.7854299545288085938, 6.9635753631591796875))
        .line_to((0.0, 0.0))
        .line_to((7.5, 0.0))
        .line_to((7.5000004768371582031, 68.92224884033203125))
        .line_to((79.805633544921875, 40.0))
        .line_to((-2.7854299545288085938, 6.9635753631591796875))
        .detach();

    let mut p = Paint::default();
    p.set_color(Color::GREEN);

    canvas.translate((20.0, 20.0));
    canvas.draw_path(&path, &p);
});

// Port of: gm/strokes.cpp#L684-L (chrome/m156)
crate::def_simple_gm!(b_340982297, canvas, 80, 50, {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    let mut path = PathBuilder::new()
        .move_to((30.23983, 48.5674667))
        .line_to((1.30884242, 45.5222702))
        .line_to((2.97688866, 29.6749554))
        .line_to((17.4423828, 31.1975555))
        .line_to((2.94269657, 30.0452003))
        .line_to((4.38597536, 11.8849154))
        .line_to((33.3853493, 14.1896257))
        .close()
        .detach();

    canvas.draw_path(&path, &paint);

    path = PathBuilder::new()
        .move_to((73.3853455, 4.18963623))
        .line_to((69.995636, 39.1360626))
        .line_to((42.83145142, 21.056778))
        .line_to((42.97689819, 19.6749573))
        .line_to((57.4423828, 21.1975555))
        .line_to((42.94268799, 20.0451965))
        .line_to((44.38595581, 1.88491821))
        .close()
        .detach();

    canvas.draw_path(&path, &paint);
});
