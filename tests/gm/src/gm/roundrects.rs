// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/roundrects.cpp (chrome/m156)

// names and int casts mirror the C++
#![allow(
    clippy::similar_names,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]

use crate::prelude::*;
use skia_rust_core::color::{Color, HSV, colors};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/roundrects.cpp#L24-L32 (chrome/m156)
fn gen_color(rand: &mut Random) -> Color {
    let hsv = HSV {
        h: rand.next_range_f(0.0, 360.0),
        s: rand.next_range_f(0.75, 1.0),
        v: rand.next_range_f(0.75, 1.0),
    };

    crate::tool_utils::color_to_565(hsv.to_color(0xFF))
}

// Port of: gm/roundrects.cpp#L34-L381 (chrome/m156)
struct RoundRectGm {
    paints: Vec<Paint>,
    matrices: Vec<Matrix>,
}

impl RoundRectGm {
    fn new() -> Self {
        let mut gm = RoundRectGm {
            paints: Vec::new(),
            matrices: Vec::new(),
        };
        gm.make_paints();
        gm.make_matrices();
        gm
    }

    fn make_paints(&mut self) {
        // no AA
        self.paints.push(Paint::default());

        // AA
        let mut p = Paint::default();
        p.set_anti_alias(true);
        self.paints.push(p);

        // AA with stroke style
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_style(Style::Stroke);
        p.set_stroke_width(5.0);
        self.paints.push(p);

        // AA with stroke style, width = 0
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_style(Style::Stroke);
        self.paints.push(p);

        // AA with stroke and fill style
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_style(Style::StrokeAndFill);
        p.set_stroke_width(3.0);
        self.paints.push(p);
    }

    fn make_matrices(&mut self) {
        let mut m = Matrix::new_identity();
        m.set_identity();
        self.matrices.push(m);

        for (sx, sy) in [(3.0, 2.0), (2.0, 2.0), (1.0, 2.0), (4.0, 1.0)] {
            let mut m = Matrix::new_identity();
            m.set_scale((sx, sy), None);
            self.matrices.push(m);
        }

        let mut m = Matrix::new_identity();
        m.set_rotate(90.0, None);
        self.matrices.push(m);

        let mut m = Matrix::new_identity();
        m.set_skew((2.0, 3.0), None);
        self.matrices.push(m);

        let mut m = Matrix::new_identity();
        m.set_rotate(60.0, None);
        self.matrices.push(m);
    }
}

impl GM for RoundRectGm {
    fn name(&self) -> String {
        "roundrects".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1200, 900)
    }

    fn bg_color(&self) -> Color {
        Color::new(0xFF00_0000)
    }

    // Port of: gm/roundrects.cpp#L119-L366 (chrome/m156)
    #[allow(clippy::too_many_lines, clippy::cast_precision_loss)] // mirrors the C++ body
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut rand = Random::new(1);
        canvas.translate((20.0, 20.0));
        let k_rect = Rect::new(-20.0, -30.0, 20.0, 30.0);
        let mut circle_rrect = RRect::default();
        circle_rrect.set_rect_xy(k_rect, 5.0, 5.0);

        let k_x_start = 60.0f32;
        let k_y_start = 80.0f32;
        let k_x_step = 150i32;
        let k_y_step = 160i32;
        let max_x = self.matrices.len() as i32;

        let mut rect_paint = Paint::default();
        rect_paint.set_anti_alias(true);
        rect_paint.set_style(Style::Stroke);
        rect_paint.set_stroke_width(0.0);
        rect_paint.set_color(Color::LIGHT_GRAY);

        let mut test_count = 0i32;
        for i in 0..self.paints.len() {
            for j in 0..self.matrices.len() {
                canvas.save();
                let mut mat = self.matrices[j].clone();
                // position the roundrect, and make it at off-integer coords.
                mat.post_translate((
                    k_x_start + 1.0 * k_x_step as f32 * (test_count % max_x) as f32 + 1.0 / 4.0,
                    k_y_start
                        + 1.0 * k_y_step as f32 * (test_count / max_x) as f32
                        + 3.0 * 1.0 / 4.0,
                ));
                canvas.concat(&mat);

                let color = gen_color(&mut rand);
                self.paints[i].set_color(color);

                canvas.draw_rect(k_rect, &rect_paint);
                canvas.draw_rrect(circle_rrect, &self.paints[i]);

                canvas.restore();

                test_count += 1;
            }
        }

        let xs = k_x_step as f32;
        let ys = k_y_step as f32;

        // special cases

        // non-scaled tall and skinny roundrect
        for i in 0..self.paints.len() {
            let rect = Rect::new(-20.0, -60.0, 20.0, 60.0);
            let mut ellipse_rect = RRect::default();
            ellipse_rect.set_rect_xy(rect, 5.0, 10.0);

            canvas.save();
            // position the roundrect, and make it at off-integer coords.
            canvas.translate((
                k_x_start + 1.0 * xs * 2.55 + 1.0 / 4.0,
                k_y_start + 1.0 * ys * i as f32 + 3.0 * 1.0 / 4.0,
            ));

            let color = gen_color(&mut rand);
            self.paints[i].set_color(color);

            canvas.draw_rect(rect, &rect_paint);
            canvas.draw_rrect(ellipse_rect, &self.paints[i]);
            canvas.restore();
        }

        // non-scaled wide and short roundrect
        for i in 0..self.paints.len() {
            let rect = Rect::new(-80.0, -30.0, 80.0, 30.0);
            let mut ellipse_rect = RRect::default();
            ellipse_rect.set_rect_xy(rect, 20.0, 5.0);

            canvas.save();
            // position the roundrect, and make it at off-integer coords.
            canvas.translate((
                k_x_start + 1.0 * xs * 4.0 + 1.0 / 4.0,
                k_y_start + 1.0 * ys * i as f32 + 3.0 * 1.0 / 4.0 + 0.5 * ys,
            ));

            let color = gen_color(&mut rand);
            self.paints[i].set_color(color);

            canvas.draw_rect(rect, &rect_paint);
            canvas.draw_rrect(ellipse_rect, &self.paints[i]);
            canvas.restore();
        }

        // super skinny roundrect
        for i in 0..self.paints.len() {
            let rect = Rect::new(0.0, -60.0, 1.0, 60.0);
            let mut circle_rect = RRect::default();
            circle_rect.set_rect_xy(rect, 5.0, 5.0);

            canvas.save();
            // position the roundrect, and make it at off-integer coords.
            canvas.translate((
                k_x_start + 1.0 * xs * 3.25 + 1.0 / 4.0,
                k_y_start + 1.0 * ys * i as f32 + 3.0 * 1.0 / 4.0,
            ));

            let color = gen_color(&mut rand);
            self.paints[i].set_color(color);

            canvas.draw_rrect(circle_rect, &self.paints[i]);
            canvas.restore();
        }

        // super short roundrect
        for i in 0..self.paints.len() {
            let rect = Rect::new(-80.0, -1.0, 80.0, 0.0);
            let mut circle_rect = RRect::default();
            circle_rect.set_rect_xy(rect, 5.0, 5.0);

            canvas.save();
            // position the roundrect, and make it at off-integer coords.
            canvas.translate((
                k_x_start + 1.0 * xs * 2.5 + 1.0 / 4.0,
                k_y_start + 1.0 * ys * i as f32 + 3.0 * 1.0 / 4.0 + 0.5 * ys,
            ));

            let color = gen_color(&mut rand);
            self.paints[i].set_color(color);

            canvas.draw_rrect(circle_rect, &self.paints[i]);
            canvas.restore();
        }

        // radial gradient
        let center = Point::new(0.0, 0.0);
        let gcolors = [colors::BLUE, colors::RED, colors::GREEN];
        let pos = [0.0, 0.5, 1.0];
        let shader = shaders::radial_gradient(
            (center, 20.0),
            &Gradient::new(
                Colors::new(&gcolors, Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        );

        for i in 0..self.paints.len() {
            canvas.save();
            // position the path, and make it at off-integer coords.
            canvas.translate((
                k_x_start + 1.0 * xs * 0.0 + 1.0 / 4.0,
                k_y_start + 1.0 * ys * i as f32 + 3.0 * 1.0 / 4.0 + 0.5 * ys,
            ));

            let color = gen_color(&mut rand);
            self.paints[i].set_color(color);
            self.paints[i].set_shader(shader.clone());

            canvas.draw_rect(k_rect, &rect_paint);
            canvas.draw_rrect(circle_rrect, &self.paints[i]);

            self.paints[i].set_shader(None);

            canvas.restore();
        }

        // strokes and radii
        {
            let radii: [[f32; 2]; 4] = [[10.0, 10.0], [5.0, 15.0], [5.0, 15.0], [5.0, 15.0]];

            let stroke_widths: [f32; 4] = [20.0, 10.0, 20.0, 40.0];

            for i in 0..4 {
                let mut circle_rect = RRect::default();
                circle_rect.set_rect_xy(k_rect, radii[i][0], radii[i][1]);

                canvas.save();
                // position the roundrect, and make it at off-integer coords.
                canvas.translate((
                    k_x_start + 1.0 * xs * 5.0 + 1.0 / 4.0,
                    k_y_start + 1.0 * ys * i as f32 + 3.0 * 1.0 / 4.0 + 0.5 * ys,
                ));

                let color = gen_color(&mut rand);

                let mut p = Paint::default();
                p.set_anti_alias(true);
                p.set_style(Style::Stroke);
                p.set_stroke_width(stroke_widths[i]);
                p.set_color(color);

                canvas.draw_rrect(circle_rect, &p);
                canvas.restore();
            }
        }

        // test old entry point ( skbug.com/40034920 )
        {
            canvas.save();

            canvas.translate((
                k_x_start + 1.0 * xs * 5.0 + 1.0 / 4.0,
                k_y_start + 1.0 * ys * 4.0 + 1.0 / 4.0 + 0.5 * ys,
            ));

            let color = gen_color(&mut rand);

            let mut p = Paint::default();
            p.set_color(color);

            let ooo_rect = Rect::new(20.0, 30.0, -20.0, -30.0); // intentionally out of order
            canvas.draw_round_rect(ooo_rect, 10.0, 10.0, &p);

            canvas.restore();
        }

        // rrect with stroke > radius/2
        {
            let small_rect = Rect::new(-30.0, -20.0, 30.0, 20.0);
            let mut circle_rect = RRect::default();
            circle_rect.set_rect_xy(small_rect, 5.0, 5.0);

            canvas.save();
            // position the roundrect, and make it at off-integer coords.
            canvas.translate((
                k_x_start + 1.0 * xs * 5.0 + 1.0 / 4.0,
                k_y_start - 1.0 * ys + 73.0 * 1.0 / 4.0 + 0.5 * ys,
            ));

            let color = gen_color(&mut rand);

            let mut p = Paint::default();
            p.set_anti_alias(true);
            p.set_style(Style::Stroke);
            p.set_stroke_width(25.0);
            p.set_color(color);

            canvas.draw_rrect(circle_rect, &p);
            canvas.restore();
        }
    }
}

//////////////////////////////////////////////////////////////////////////////

// Port of: gm/roundrects.cpp#L378 (chrome/m156)
crate::def_gm!(RoundRectGM, RoundRectGm::new());
