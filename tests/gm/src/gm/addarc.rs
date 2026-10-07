// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/addarc.cpp (chrome/m156)

// Float literals are copied verbatim from the C++ source.
#![allow(clippy::excessive_precision)]

use crate::prelude::*;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_measure::PathMeasure;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{
    SCALAR_PI, degrees_to_radians, scalar_cos, scalar_sin, scalar_sqrt,
};

// `ToolUtils::color_to_565`.
// Port of: tools/ToolUtils.cpp#L142-L151 (chrome/m156)
fn color_to_565(color: u32) -> Color {
    use skia_rust_core::color::pre_multiply_color;
    use skia_rust_core::color_data::{pixel16_to_color, pixel32_to_pixel16};
    let pm_color = pre_multiply_color(Color::new(color));
    let color16 = pixel32_to_pixel16(pm_color);
    pixel16_to_color(color16)
}

// Port of: gm/addarc.cpp#L26-L72 (chrome/m156)
struct AddArcGm {
    rotate: f32,
}

impl GM for AddArcGm {
    fn name(&self) -> String {
        "addarc".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1040, 1040)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((20.0, 20.0));

        let mut r = Rect::from_wh(1000.0, 1000.0);

        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_stroke(true);
        paint.set_stroke_width(15.0);

        let inset = paint.stroke_width() + 4.0;
        let sweep_angle: f32 = 345.0;
        let mut rand = Random::default();

        let mut sign: f32 = 1.0;
        while r.width() > paint.stroke_width() * 3.0 {
            paint.set_color(color_to_565(rand.next_u() | (0xFF << 24)));
            let mut start_angle = rand.next_u_scalar1() * 360.0;

            let speed = scalar_sqrt(16.0 / r.width()) * 0.5;
            start_angle += self.rotate * 360.0 * speed * sign;

            let mut path = PathBuilder::new();
            path.add_arc(r, start_angle, sweep_angle);
            canvas.draw_path(&path.detach().with_is_volatile(true), &paint);

            r.inset((inset, inset));
            sign = -sign;
        }
    }
}
crate::def_gm!(AddArcGM, AddArcGm { rotate: 0.0 });

// Port of: gm/addarc.cpp#L74-L108 (chrome/m156)
const R: i32 = 400;

crate::def_simple_gm!(addarc_meas, canvas, 2 * R + 40, 2 * R + 40, {
    #[allow(clippy::cast_precision_loss)] // SkScalar from int
    let r = R as f32;
    canvas.translate((r + 20.0, r + 20.0));

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_stroke(true);

    let mut meas_paint = Paint::default();
    meas_paint.set_anti_alias(true);
    meas_paint.set_color(Color::RED);

    let oval = Rect::from_ltrb(-r, -r, r, r);
    canvas.draw_oval(oval, &paint);

    let mut deg: f32 = 0.0;
    while deg < 360.0 {
        let rad = degrees_to_radians(deg);
        let rx = scalar_cos(rad) * r;
        let ry = scalar_sin(rad) * r;

        canvas.draw_line((0.0, 0.0), (rx, ry), &paint);

        let meas = PathMeasure::new(
            &PathBuilder::new().add_arc(oval, 0.0, deg).detach(),
            false,
            None,
        );
        let arc_len = rad * r;
        if let Some((pos, _)) = meas.pos_tan(arc_len) {
            canvas.draw_line(Point::new(0.0, 0.0), pos, &meas_paint);
        }
        deg += 10.0;
    }
});

// Emphasize drawing a stroked oval (containing conics) and then scaling the results up,
// to ensure that we compute the stroke taking the CTM into account
// Port of: gm/addarc.cpp#L110-L150 (chrome/m156)
struct StrokeCircleGm {
    rotate: f32,
}

impl GM for StrokeCircleGm {
    fn name(&self) -> String {
        "strokecircle".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(520, 520)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.scale((20.0, 20.0));
        canvas.translate((13.0, 13.0));

        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_stroke(true);
        paint.set_stroke_width(1.0 / 2.0);

        let delta = paint.stroke_width() * 3.0 / 2.0;
        let mut r = Rect::from_xywh(-12.0, -12.0, 24.0, 24.0);
        let mut rand = Random::default();

        let mut sign: f32 = 1.0;
        while r.width() > paint.stroke_width() * 2.0 {
            let _acr = AutoCanvasRestore::guard(canvas, true);
            canvas.rotate(self.rotate * sign, None);

            paint.set_color(color_to_565(rand.next_u() | (0xFF << 24)));
            canvas.draw_oval(r, &paint);
            r.inset((delta, delta));
            sign = -sign;
        }
    }
}
crate::def_gm!(StrokeCircleGM, StrokeCircleGm { rotate: 0.0 });

// Fill circles and rotate them to test our Analytic Anti-Aliasing.
// This test is based on StrokeCircleGM.
// Port of: gm/addarc.cpp#L152-L196 (chrome/m156)
struct FillCircleGm {
    rotate: f32,
}

impl GM for FillCircleGm {
    fn name(&self) -> String {
        "fillcircle".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(520, 520)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.scale((20.0, 20.0));
        canvas.translate((13.0, 13.0));

        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_stroke(true);
        paint.set_stroke_width(1.0 / 2.0);

        let stroke_width = paint.stroke_width();
        let delta = stroke_width * 3.0 / 2.0;
        let mut r = Rect::from_xywh(-12.0, -12.0, 24.0, 24.0);
        let mut rand = Random::default();

        // Reset style to fill. We only need stroke stype for producing delta and strokeWidth
        paint.set_stroke(false);

        let mut sign: f32 = 1.0;
        while r.width() > stroke_width * 2.0 {
            let _acr = AutoCanvasRestore::guard(canvas, true);
            canvas.rotate(self.rotate * sign, None);
            paint.set_color(color_to_565(rand.next_u() | (0xFF << 24)));
            canvas.draw_oval(r, &paint);
            r.inset((delta, delta));
            sign = -sign;
        }
    }
}
crate::def_gm!(FillCircleGM, FillCircleGm { rotate: 0.0 });

// Port of: gm/addarc.cpp#L198-L207 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ function
fn html_canvas_arc(
    path: &mut PathBuilder,
    x: f32,
    y: f32,
    r: f32,
    start: f32,
    end: f32,
    ccw: bool,
    call_arc_to: bool,
) {
    let bounds = Rect::new(x - r, y - r, x + r, y + r);
    let sweep = if ccw { end - start } else { start - end };
    if call_arc_to {
        path.arc_to(bounds, start, sweep, false);
    } else {
        path.add_arc(bounds, start, sweep);
    }
}

// Lifted from canvas-arc-circumference-fill-diffs.html
// Port of: gm/addarc.cpp#L209-L258 (chrome/m156)
crate::def_simple_gm!(manyarcs, canvas, 620, 330, {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_stroke(true);

    canvas.translate((10.0, 10.0));

    // 20 angles.
    let mut sweep_angles: [f32; 20] = [
        -123.7, -2.3, -2.0, -1.0, -0.3, -0.000001, 0.0, 0.000001, 0.3, 0.7, 1.0, 1.3, 1.5, 1.7,
        1.99999, 2.0, 2.00001, 2.3, 4.3, 3934723942837.3,
    ];
    for angle in &mut sweep_angles {
        *angle *= 180.0;
    }

    let mut start_angles: [f32; 4] = [-1.0, -0.5, 0.0, 0.5];
    for angle in &mut start_angles {
        *angle *= 180.0;
    }

    let mut anticlockwise = false;
    let mut sign: f32 = 1.0;
    for i in 0..start_angles.len() * 2 {
        if i == start_angles.len() {
            anticlockwise = true;
            sign = -1.0;
        }
        let start_angle = start_angles[i % start_angles.len()] * sign;
        canvas.save();
        for sweep_angle in sweep_angles {
            let mut path = PathBuilder::new();
            path.move_to((0.0, 2.0));
            html_canvas_arc(
                &mut path,
                18.0,
                15.0,
                10.0,
                start_angle,
                start_angle + (sweep_angle * sign),
                anticlockwise,
                true,
            );
            path.line_to((0.0, 28.0));
            canvas.draw_path(&path.detach().with_is_volatile(true), &paint);
            canvas.translate((30.0, 0.0));
        }
        canvas.restore();
        canvas.translate((0.0, 40.0));
    }
});

// Lifted from https://bugs.chromium.org/p/chromium/issues/detail?id=640031
// Port of: gm/addarc.cpp#L260-L309 (chrome/m156)
crate::def_simple_gm!(tinyanglearcs, canvas, 620, 330, {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_stroke(true);

    canvas.translate((50.0, 50.0));

    let outer_radius: f32 = 100000.0;
    let inner_radius = outer_radius - 20.0;
    let center_x: f32 = 50.0;
    let center_y = outer_radius;
    let start_angles: [f32; 2] = [1.5 * SCALAR_PI, 1.501 * SCALAR_PI];
    let sweep_angle = 10.0 / outer_radius;

    for start_angle in start_angles {
        let mut path = PathBuilder::new();
        let end_angle = start_angle + sweep_angle;
        path.move_to((
            center_x + inner_radius * start_angle.cos(),
            center_y + inner_radius * start_angle.sin(),
        ));
        path.line_to((
            center_x + outer_radius * start_angle.cos(),
            center_y + outer_radius * start_angle.sin(),
        ));
        // A combination of tiny sweepAngle + large radius, we should draw a line.
        html_canvas_arc(
            &mut path,
            center_x,
            outer_radius,
            outer_radius,
            start_angle * 180.0 / SCALAR_PI,
            end_angle * 180.0 / SCALAR_PI,
            true,
            true,
        );
        path.line_to((
            center_x + inner_radius * end_angle.cos(),
            center_y + inner_radius * end_angle.sin(),
        ));
        html_canvas_arc(
            &mut path,
            center_x,
            outer_radius,
            inner_radius,
            end_angle * 180.0 / SCALAR_PI,
            start_angle * 180.0 / SCALAR_PI,
            true,
            false,
        );
        canvas.draw_path(&path.detach(), &paint);
        canvas.translate((20.0, 0.0));
    }
});
