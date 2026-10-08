// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/dashcircle.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: literals, short names, local constants, int/float
// conversions, index loops and long bodies are kept as they are there.
#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::trivially_copy_pass_by_ref,
    clippy::write_with_newline,
    clippy::excessive_precision,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::SCALAR_PI;
use skia_rust_effects::dash_path_effect;

// Port of: gm/dashcircle.cpp#L24-L38 (chrome/m156)
const DASH1: [i32; 2] = [1, 1];
const DASH2: [i32; 2] = [1, 3];
const DASH3: [i32; 4] = [1, 1, 3, 3];
const DASH4: [i32; 4] = [1, 3, 2, 4];

const DASH_EXAMPLES: [&[i32]; 4] = [&DASH1, &DASH2, &DASH3, &DASH4];

// Port of: gm/dashcircle.cpp#L41-L116 (chrome/m156)
struct DashCircleGm {
    rotation: f32,
}

impl GM for DashCircleGm {
    fn name(&self) -> String {
        "dashcircle".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(900, 1200)
    }

    #[allow(clippy::cast_precision_loss)] // int to float conversions as in C++
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut ref_paint = Paint::default();
        ref_paint.set_anti_alias(true);
        ref_paint.set_color(Color::new(0xFFbf3f7f));
        ref_paint.set_stroke(true);
        ref_paint.set_stroke_width(1.0);
        let radius: f32 = 125.0;
        let oval = Rect::from_ltrb(-radius - 20.0, -radius - 20.0, radius + 20.0, radius + 20.0);
        let circle = Path::circle((0.0, 0.0), radius, None);
        let circumference = radius * SCALAR_PI * 2.0;
        let wedges = [6, 12, 36];
        canvas.translate((radius + 20.0, radius + 20.0));
        for wedge in wedges {
            let arc_length = 360.0 / wedge as f32;
            canvas.save();
            for dash_example in DASH_EXAMPLES {
                let length = dash_example.len();
                let mut ref_path = PathBuilder::new();
                let mut dash_units = 0;
                for index in 0..length {
                    dash_units += dash_example[index];
                }
                let unit_length = arc_length / dash_units as f32;
                let mut angle: f32 = 0.0;
                for _ in 0..wedge {
                    for i2 in (0..length).step_by(2) {
                        let span = dash_example[i2] as f32 * unit_length;
                        ref_path.move_to((0.0, 0.0));
                        ref_path.arc_to(oval, angle, span, false);
                        ref_path.close();
                        angle += span + (dash_example[i2 + 1] as f32) * unit_length;
                    }
                }
                canvas.save();
                canvas.rotate(self.rotation, None);
                canvas.draw_path(&ref_path.detach(), &ref_paint);
                canvas.restore();
                let mut p = Paint::default();
                p.set_anti_alias(true);
                p.set_stroke(true);
                p.set_stroke_width(10.0);
                let mut intervals = [0.0f32; 4];
                let interval_count = length;
                let dash_length = circumference / wedge as f32 / dash_units as f32;
                for index in 0..length {
                    intervals[index] = dash_example[index] as f32 * dash_length;
                }
                p.set_path_effect(dash_path_effect::new(&intervals[..interval_count], 0.0));
                canvas.save();
                canvas.rotate(self.rotation, None);
                canvas.draw_path(&circle, &p);
                canvas.restore();
                canvas.translate((0.0, radius * 2.0 + 50.0));
            }
            canvas.restore();
            canvas.translate((radius * 2.0 + 50.0, 0.0));
        }
    }
}
crate::def_gm!(DashCircleGM, DashCircleGm { rotation: 0.0 });

// Port of: gm/dashcircle.cpp#L120-L240 (chrome/m156)
struct DashCircle2Gm {
    // Init with a non-zero phase for when run as a non-animating GM.
    phase_degrees: f32,
}

impl GM for DashCircle2Gm {
    fn name(&self) -> String {
        "dashcircle2".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(635, 900)
    }

    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn on_draw(&mut self, canvas: &Canvas) {
        // These intervals are defined relative to tau.
        const INTERVALS: [[f32; 2]; 10] = [
            [0.333, 0.333],
            [0.015, 0.015],
            [0.01, 0.09],
            [0.097, 0.003],
            [0.02, 0.04],
            [0.1, 0.2],
            [0.25, 0.25],
            [0.6, 0.7], // adds to > 1
            [1.2, 0.8], // on is > 1
            [0.1, 1.1], // off is > 1*/
        ];

        const RADIUS: f32 = 20.0;
        const STROKE_WIDTH: f32 = 15.0;
        const PAD: f32 = 5.0;
        const CIRCLE: Rect = Rect::new(-RADIUS, -RADIUS, RADIUS, RADIUS);

        // `kRadius * 1.5` is evaluated in double and converted to float.
        #[allow(clippy::cast_possible_truncation)]
        const THIN_RADIUS: f32 = (RADIUS as f64 * 1.5) as f32;
        const THIN_CIRCLE: Rect = Rect::new(-THIN_RADIUS, -THIN_RADIUS, THIN_RADIUS, THIN_RADIUS);
        const THIN_STROKE_WIDTH: f32 = 0.4;

        let mut deffects = Vec::new();
        let mut thin_deffects = Vec::new();
        for interval in &INTERVALS {
            const TAU: f32 = 2.0 * SCALAR_PI;
            const CIRCUMFERENCE: f32 = RADIUS * TAU;
            let mut scaled_intervals = [CIRCUMFERENCE * interval[0], CIRCUMFERENCE * interval[1]];
            deffects.push(dash_path_effect::new(
                &scaled_intervals,
                CIRCUMFERENCE * self.phase_degrees * TAU / 360.0,
            ));
            const THIN_CIRCUMFERENCE: f32 = THIN_RADIUS * TAU;
            scaled_intervals[0] = THIN_CIRCUMFERENCE * interval[0];
            scaled_intervals[1] = THIN_CIRCUMFERENCE * interval[1];
            thin_deffects.push(dash_path_effect::new(
                &scaled_intervals,
                THIN_CIRCUMFERENCE * self.phase_degrees * TAU / 360.0,
            ));
        }

        let mut rotate = Matrix::new_identity();
        rotate.set_rotate(25.0, None);
        let matrices = [
            Matrix::i().clone(),
            Matrix::scale((1.2, 1.2)),
            Matrix::new_all(1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0), // y flipper
            Matrix::new_all(-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0), // x flipper
            Matrix::scale((0.7, 0.7)),
            rotate.clone(),
            Matrix::concat(
                &Matrix::concat(
                    &Matrix::new_all(-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
                    &rotate,
                ),
                &rotate,
            ),
        ];

        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_stroke_width(STROKE_WIDTH);
        paint.set_stroke(true);

        // Compute the union of bounds of all of our test cases.
        let mut bounds = Rect::new_empty();
        let k_bounds = THIN_CIRCLE.with_outset((THIN_STROKE_WIDTH / 2.0, THIN_STROKE_WIDTH / 2.0));
        for m in &matrices {
            let (dev_bounds, _) = m.map_rect(k_bounds);
            bounds.join(dev_bounds);
        }

        canvas.save();
        canvas.translate((-bounds.left + PAD, -bounds.top + PAD));
        for i in 0..deffects.len() {
            canvas.save();
            for m in &matrices {
                canvas.save();
                canvas.concat(m);

                paint.set_path_effect(deffects[i].clone());
                paint.set_stroke_width(STROKE_WIDTH);
                canvas.draw_oval(CIRCLE, &paint);

                paint.set_path_effect(thin_deffects[i].clone());
                paint.set_stroke_width(THIN_STROKE_WIDTH);
                canvas.draw_oval(THIN_CIRCLE, &paint);

                canvas.restore();
                canvas.translate((bounds.width() + PAD, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, bounds.height() + PAD));
        }
        canvas.restore();
    }
}
crate::def_gm!(
    DashCircle2GM,
    DashCircle2Gm {
        phase_degrees: 12.0
    }
);

// Port of: gm/dashcircle.cpp#L242-L272 (chrome/m156)
crate::def_simple_gm!(maddash, canvas, 1600, 1600, {
    canvas.draw_rect(Rect::new(0.0, 0.0, 1600.0, 1600.0), &Paint::default());
    let mut p = Paint::default();
    p.set_color(Color::RED);
    p.set_anti_alias(true);
    p.set_stroke(true);
    p.set_stroke_width(380.0);

    let intvls = [2.5, 10.0 /* 1200 */];
    p.set_path_effect(dash_path_effect::new(&intvls, 0.0));

    canvas.draw_circle((400.0, 400.0), 200.0, &p);

    let mut path = PathBuilder::new();
    path.move_to((800.0, 400.0));
    path.quad_to((1000.0, 400.0), (1000.0, 600.0));
    path.quad_to((1000.0, 800.0), (800.0, 800.0));
    path.quad_to((600.0, 800.0), (600.0, 600.0));
    path.quad_to((600.0, 400.0), (800.0, 400.0));
    path.close();
    canvas.translate((350.0, 150.0));
    p.set_stroke_width(320.0);
    canvas.draw_path(&path.detach(), &p);

    path.move_to((800.0, 400.0));
    path.cubic_to((900.0, 400.0), (1000.0, 500.0), (1000.0, 600.0));
    path.cubic_to((1000.0, 700.0), (900.0, 800.0), (800.0, 800.0));
    path.cubic_to((700.0, 800.0), (600.0, 700.0), (600.0, 600.0));
    path.cubic_to((600.0, 500.0), (700.0, 400.0), (800.0, 400.0));
    path.close();
    canvas.translate((-550.0, 500.0));
    p.set_stroke_width(300.0);
    canvas.draw_path(&path.detach(), &p);
});
