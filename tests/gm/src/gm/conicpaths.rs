// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/conicpaths.cpp (chrome/m156)

#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::excessive_precision,
    clippy::float_cmp,
    clippy::inconsistent_digit_grouping,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

// Port of: gm/conicpaths.cpp#L24-L143 (chrome/m156)
struct ConicPathsGm {
    paths: Vec<Path>,
    giant_circle: Path,
}

impl ConicPathsGm {
    // Port of: gm/conicpaths.cpp#L30-L97 (chrome/m156)
    fn new() -> Self {
        let mut paths = Vec::new();

        {
            let w = 2f32.sqrt() / 2.0;
            let mut conic_circle = PathBuilder::new();
            conic_circle.move_to((0.0, 0.0));
            conic_circle.conic_to((0.0, 50.0), (50.0, 50.0), w);
            conic_circle.r_conic_to((50.0, 0.0), (50.0, -50.0), w);
            conic_circle.r_conic_to((0.0, -50.0), (-50.0, -50.0), w);
            conic_circle.r_conic_to((-50.0, 0.0), (-50.0, 50.0), w);
            paths.push(conic_circle.detach());
        }

        {
            let mut hyperbola = PathBuilder::new();
            hyperbola.move_to((0.0, 0.0));
            hyperbola.conic_to((0.0, 100.0), (100.0, 100.0), 2.0);
            paths.push(hyperbola.detach());
        }

        {
            let mut thin_hyperbola = PathBuilder::new();
            thin_hyperbola.move_to((0.0, 0.0));
            thin_hyperbola.conic_to((100.0, 100.0), (5.0, 0.0), 2.0);
            paths.push(thin_hyperbola.detach());
        }

        {
            let mut very_thin_hyperbola = PathBuilder::new();
            very_thin_hyperbola.move_to((0.0, 0.0));
            very_thin_hyperbola.conic_to((100.0, 100.0), (1.0, 0.0), 2.0);
            paths.push(very_thin_hyperbola.detach());
        }

        {
            let mut closed_hyperbola = PathBuilder::new();
            closed_hyperbola.move_to((0.0, 0.0));
            closed_hyperbola.conic_to((100.0, 100.0), (0.0, 0.0), 2.0);
            paths.push(closed_hyperbola.detach());
        }

        {
            // using 1 as weight defaults to using quadTo
            let mut near_parabola = PathBuilder::new();
            near_parabola.move_to((0.0, 0.0));
            near_parabola.conic_to((0.0, 100.0), (100.0, 100.0), 0.999);
            paths.push(near_parabola.detach());
        }

        {
            let mut thin_ellipse = PathBuilder::new();
            thin_ellipse.move_to((0.0, 0.0));
            thin_ellipse.conic_to((100.0, 100.0), (5.0, 0.0), 0.5);
            paths.push(thin_ellipse.detach());
        }

        {
            let mut very_thin_ellipse = PathBuilder::new();
            very_thin_ellipse.move_to((0.0, 0.0));
            very_thin_ellipse.conic_to((100.0, 100.0), (1.0, 0.0), 0.5);
            paths.push(very_thin_ellipse.detach());
        }

        {
            let mut closed_ellipse = PathBuilder::new();
            closed_ellipse.move_to((0.0, 0.0));
            closed_ellipse.conic_to((100.0, 100.0), (0.0, 0.0), 0.5);
            paths.push(closed_ellipse.detach());
        }

        let giant_circle = {
            let mut b = PathBuilder::new();
            let w = 2f32.sqrt() / 2.0;
            b.move_to((2.1e+11, -1.05e+11));
            b.conic_to((2.1e+11, 0.0), (1.05e+11, 0.0), w);
            b.conic_to((0.0, 0.0), (0.0, -1.05e+11), w);
            b.conic_to((0.0, -2.1e+11), (1.05e+11, -2.1e+11), w);
            b.conic_to((2.1e+11, -2.1e+11), (2.1e+11, -1.05e+11), w);
            b.detach()
        };

        Self {
            paths,
            giant_circle,
        }
    }

    // Port of: gm/conicpaths.cpp#L99-L102 (chrome/m156)
    fn draw_giant_circle(&self, canvas: &Canvas) {
        let paint = Paint::default();
        canvas.draw_path(&self.giant_circle, &paint);
    }
}

impl GM for ConicPathsGm {
    fn name(&self) -> String {
        "conicpaths".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(920, 960)
    }

    // Port of: gm/conicpaths.cpp#L104-L136 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        const ALPHA_VALUE: [u8; 2] = [0xFF, 0x40];

        let margin = 15.0;
        canvas.translate((margin, margin));

        let mut paint = Paint::default();
        for path in &self.paths {
            canvas.save();
            for alpha in ALPHA_VALUE {
                paint.set_argb(alpha, 0, 0, 0);
                for aa in 0..2 {
                    paint.set_anti_alias(aa != 0);
                    for fh in 0..2 {
                        paint.set_stroke(fh != 0);

                        let bounds: Rect = *path.bounds();
                        canvas.save();
                        canvas.translate((-bounds.left, -bounds.top));
                        canvas.draw_path(path, &paint);
                        canvas.restore();

                        canvas.translate((110.0, 0.0));
                    }
                }
            }
            canvas.restore();
            canvas.translate((0.0, 110.0));
        }
        canvas.restore();

        self.draw_giant_circle(canvas);
    }
}

crate::def_gm!(ConicPathsGM, ConicPathsGm::new());

// Port of: gm/conicpaths.cpp#L147-L161 (chrome/m156)
crate::def_simple_gm!(arccirclegap, canvas, 250, 250, {
    canvas.translate((50.0, 100.0));
    let c = Point::new(1_052.539_062_5, 506.876_097_803_471_1);
    let radius: f32 = 1_096.702_150_363_923;
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_stroke(true);
    canvas.draw_circle(c, radius, &paint);
    let mut b = PathBuilder::new();
    b.move_to((288.888_847_106_541_33, -280.266_808_626_09));
    b.arc_to_tangent(
        (0.0, 0.0),
        (-39.002_164_433_064_11, 400.605_892_579_647_6),
        radius,
    );
    let path = b.detach();
    paint.set_color(Color::new(0xff00_7f00));
    canvas.draw_path(&path, &paint);
});

// Port of: gm/conicpaths.cpp#L163-L172 (chrome/m156)
crate::def_simple_gm!(largecircle, canvas, 250, 250, {
    canvas.translate((50.0, 100.0));
    let c = Point::new(1_052.539_062_5, 506.876_097_803_471_1);
    let radius: f32 = 1_096.702_150_363_923;
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_stroke(true);
    canvas.draw_circle(c, radius, &paint);
});

// Port of: gm/conicpaths.cpp#L174-L202 (chrome/m156)
crate::def_simple_gm!(largeovals, canvas, 250, 250, {
    // Test EllipseOp
    let mut r = Rect::from_xywh(-520.0, -520.0, 5000.0, 4000.0);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_stroke(true);
    paint.set_stroke_width(100.0);
    canvas.draw_oval(r, &paint);
    r.offset((-15.0, -15.0));
    paint.set_color(Color::DARK_GRAY);
    // we use stroke and fill to avoid falling into the SimpleFill path
    paint.set_style(Style::StrokeAndFill);
    paint.set_stroke_width(1.0);
    canvas.draw_oval(r, &paint);

    // Test DIEllipseOp
    canvas.rotate(1.0, None);
    r.offset((55.0, 55.0));
    paint.set_color(Color::GRAY);
    paint.set_stroke(true);
    paint.set_stroke_width(100.0);
    canvas.draw_oval(r, &paint);
    r.offset((-15.0, -15.0));
    paint.set_color(Color::LIGHT_GRAY);
    paint.set_style(Style::StrokeAndFill);
    paint.set_stroke_width(1.0);
    canvas.draw_oval(r, &paint);
});

// Port of: gm/conicpaths.cpp#L204-L217 (chrome/m156)
crate::def_simple_gm!(crbug_640176, canvas, 250, 250, {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x0000_0000), f32::from_bits(0x0000_0000))); // 0, 0
    path.line_to((f32::from_bits(0x42cf_d89a), f32::from_bits(0xc270_0000))); // 103.923f, -60
    path.line_to((f32::from_bits(0x42cf_d899), f32::from_bits(0xc270_0006))); // 103.923f, -60
    path.conic_to(
        (f32::from_bits(0x42f0_0000), f32::from_bits(0xc200_9d9c)),
        (f32::from_bits(0x42f0_0001), f32::from_bits(0x0000_0000)),
        f32::from_bits(0x3f77_46ea),
    ); // 120, -32.1539f, 120, 0, 0.965926f

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    canvas.translate((125.0, 125.0));
    canvas.draw_path(&path.detach(), &paint);
});
