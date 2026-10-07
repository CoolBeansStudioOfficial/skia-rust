// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/hairlines.cpp (chrome/m156)

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
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{SCALAR_PI, degrees_to_radians, scalar_cos, scalar_sin};

// Port of: gm/hairlines.cpp#L28-L186 (chrome/m156)
#[derive(Default)]
struct HairlinesGm {
    paths: Vec<Path>,
}

impl GM for HairlinesGm {
    fn name(&self) -> String {
        "hairlines".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1250, 1250)
    }

    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar, kRadius * ...
    fn on_once_before_draw(&mut self) {
        {
            let mut line_angles = PathBuilder::new();
            const NUM_ANGLES: i32 = 15;
            const RADIUS: i32 = 40;

            for i in 0..NUM_ANGLES {
                let angle = SCALAR_PI * i as f32 / NUM_ANGLES as f32;
                let x = RADIUS as f32 * scalar_cos(angle);
                let y = RADIUS as f32 * scalar_sin(angle);
                line_angles.move_to((x, y)).line_to((-x, -y));
            }
            self.paths.push(line_angles.detach());
        }

        self.paths.push(
            PathBuilder::new()
                .move_to((0.0, -10.0))
                .quad_to((100.0, 100.0), (-10.0, 0.0))
                .detach(),
        );

        self.paths.push(
            PathBuilder::new()
                .move_to((0.0, -5.0))
                .quad_to((100.0, 100.0), (-5.0, 0.0))
                .detach(),
        );

        self.paths.push(
            PathBuilder::new()
                .move_to((0.0, -2.0))
                .quad_to((100.0, 100.0), (-2.0, 0.0))
                .detach(),
        );

        self.paths.push(
            PathBuilder::new()
                .move_to((0.0, -1.0))
                .quad_to((100.0, 100.0), (-2.0 + 306.0f32 / 4.0, 75.0))
                .detach(),
        );

        self.paths.push(
            PathBuilder::new()
                .move_to((0.0, -1.0))
                .quad_to((100.0, 100.0), (-1.0, 0.0))
                .detach(),
        );

        self.paths.push(
            PathBuilder::new()
                .move_to((0.0, -0.0))
                .quad_to((100.0, 100.0), (0.0, 0.0))
                .detach(),
        );

        self.paths.push(
            PathBuilder::new()
                .move_to((0.0, -0.0))
                .quad_to((100.0, 100.0), (75.0, 75.0))
                .detach(),
        );

        // Two problem cases for gpu hairline renderer found by shapeops testing. These used
        // to assert that the computed bounding box didn't contain all the vertices.

        self.paths.push(
            PathBuilder::new()
                .move_to((4.0, 6.0))
                .cubic_to((5.0, 6.0), (5.0, 4.0), (4.0, 0.0))
                .close()
                .detach(),
        );

        self.paths.push(
            PathBuilder::new()
                .move_to((5.0, 1.0))
                .line_to((4.32787323, 1.67212653))
                .cubic_to(
                    (2.75223875, 3.24776125),
                    (3.00581908, 4.51236057),
                    (3.7580452, 4.37367964),
                )
                .cubic_to((4.66472578, 3.888381), (5.0, 2.875), (5.0, 1.0))
                .close()
                .detach(),
        );

        // Three paths that show the same bug (missing end caps)

        self.paths.push(
            PathBuilder::new()
                .move_to((6.5, 5.5))
                .line_to((3.5, 0.5))
                .move_to((0.5, 5.5))
                .line_to((3.5, 0.5))
                .detach(),
        );

        // An X (crbug.com/137317)
        self.paths.push(
            PathBuilder::new()
                .move_to((1.0, 1.0))
                .line_to((6.0, 6.0))
                .move_to((1.0, 6.0))
                .line_to((6.0, 1.0))
                .detach(),
        );

        // A right angle (crbug.com/137465 and crbug.com/256776)
        self.paths.push(
            PathBuilder::new()
                .move_to((5.5, 5.5))
                .line_to((5.5, 0.5))
                .line_to((0.5, 0.5))
                .detach(),
        );

        {
            // Arc example to test imperfect truncation bug (crbug.com/295626)
            const RAD: f32 = 2000.0;
            const START_ANGLE: f32 = 262.59717;
            const SWEEP_ANGLE: f32 = 17.188717 / 2.0;

            let mut bug = PathBuilder::new();

            // Add a circular arc
            let circle = Rect::from_ltrb(-RAD, -RAD, RAD, RAD);
            bug.add_arc(circle, START_ANGLE, SWEEP_ANGLE);

            // Now add the chord that should cap the circular arc
            let p0 = Point::new(
                RAD * scalar_cos(degrees_to_radians(START_ANGLE)),
                RAD * scalar_sin(degrees_to_radians(START_ANGLE)),
            );

            let p1 = Point::new(
                RAD * scalar_cos(degrees_to_radians(START_ANGLE + SWEEP_ANGLE)),
                RAD * scalar_sin(degrees_to_radians(START_ANGLE + SWEEP_ANGLE)),
            );

            bug.move_to(p0);
            bug.line_to(p1);
            self.paths.push(bug.detach());
        }
    }

    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    fn on_draw(&mut self, canvas: &Canvas) {
        const ALPHA_VALUE: [u8; 2] = [0xFF, 0x40];
        const WIDTHS: [f32; 3] = [0.0, 0.5, 1.5];

        const MARGIN: i32 = 5;
        let wrap_x = 1250 - MARGIN;

        let mut max_h: f32 = 0.0;
        canvas.translate((MARGIN as f32, MARGIN as f32));
        canvas.save();

        let mut x = MARGIN as f32;
        for path in &self.paths {
            for alpha in ALPHA_VALUE {
                for aa in 0..2 {
                    for width in WIDTHS {
                        let bounds = path.bounds();

                        if x + bounds.width() > wrap_x as f32 {
                            canvas.restore();
                            canvas.translate((0.0, max_h + MARGIN as f32));
                            canvas.save();
                            max_h = 0.0;
                            x = MARGIN as f32;
                        }

                        let mut paint = Paint::default();
                        paint.set_argb(alpha, 0, 0, 0);
                        paint.set_anti_alias(aa != 0);
                        paint.set_style(Style::Stroke);
                        paint.set_stroke_width(width);

                        canvas.save();
                        canvas.translate((-bounds.left, -bounds.top));
                        canvas.draw_path(path, &paint);
                        canvas.restore();

                        max_h = max_h.max(bounds.height());

                        let dx = bounds.width() + MARGIN as f32;
                        x += dx;
                        canvas.translate((dx, 0.0));
                    }
                }
            }
        }
        canvas.restore();
    }
}

// Port of: gm/hairlines.cpp#L188-L215 (chrome/m156)
fn draw_squarehair_tests(canvas: &Canvas, paint: &Paint) {
    let mut paint = paint.clone();
    paint.set_style(Style::Stroke);
    canvas.draw_line((10.0, 10.0), (20.0, 10.0), &paint);
    // degenerate move, line, close to make sure we still draw the cap on both ends.
    let p = PathBuilder::new()
        .move_to((10.0, 15.0))
        .line_to((20.0, 15.0))
        .close()
        .detach();
    canvas.draw_path(&p, &paint);
    canvas.draw_line((10.0, 20.5), (20.0, 20.5), &paint);
    canvas.draw_line((30.0, 10.0), (30.0, 20.0), &paint);
    canvas.draw_line((35.5, 10.0), (35.5, 20.0), &paint);
    canvas.draw_line((40.0, 10.0), (50.0, 20.0), &paint);
    let mut path = PathBuilder::new();
    path.move_to((60.0, 10.0));
    path.quad_to((60.0, 20.0), (70.0, 20.0));
    path.conic_to((70.0, 10.0), (80.0, 10.0), 0.707);
    canvas.draw_path(&path.detach(), &paint);

    path.move_to((90.0, 10.0));
    path.cubic_to((90.0, 20.0), (100.0, 20.0), (100.0, 10.0));
    path.line_to((110.0, 10.0));
    canvas.draw_path(&path.detach(), &paint);
}

// Port of: gm/hairlines.cpp#L217-L238 (chrome/m156)
crate::def_simple_gm!(squarehair, canvas, 240, 360, {
    let aliases = [false, true];
    let widths = [0.0, 0.999, 1.0, 1.001];
    let caps = [Cap::Butt, Cap::Square, Cap::Round];
    // draw aliased on left side, and anti-aliased on right side
    for alias in aliases {
        canvas.save();
        for width in widths {
            for cap in caps {
                let mut paint = Paint::default();
                paint.set_stroke_cap(cap);
                paint.set_stroke_width(width);
                paint.set_anti_alias(alias);
                paint.set_color(Color::BLACK);
                draw_squarehair_tests(canvas, &paint);
                canvas.translate((0.0, 30.0));
            }
        }
        canvas.restore();
        canvas.translate((120.0, 0.0));
    }
});

// GM to test subdivision of hairlines
// Port of: gm/hairlines.cpp#L283-L297 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // SkIntToScalar
fn draw_subdivided_quad(canvas: &Canvas, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
    let mut paint = Paint::default();
    paint.set_stroke_width(1.0);
    paint.set_anti_alias(true);
    paint.set_style(Style::Stroke);
    paint.set_color(color);

    canvas.draw_path(
        &PathBuilder::new()
            .move_to((0.0, 0.0))
            .quad_to((x0 as f32, y0 as f32), (x1 as f32, y1 as f32))
            .detach(),
        &paint,
    );
}

// Port of: gm/hairlines.cpp#L299-L315 (chrome/m156)
crate::def_simple_gm!(hairline_subdiv, canvas, 512, 256, {
    // no subdivisions
    canvas.translate((45.0, -25.0));
    draw_subdivided_quad(canvas, 334, 334, 467, 267, Color::BLACK);

    // one subdivision
    canvas.translate((-185.0, -150.0));
    draw_subdivided_quad(canvas, 472, 472, 660, 378, Color::RED);

    // two subdivisions
    canvas.translate((-275.0, -200.0));
    draw_subdivided_quad(canvas, 668, 668, 934, 535, Color::GREEN);

    // three subdivisions
    canvas.translate((-385.0, -260.0));
    draw_subdivided_quad(canvas, 944, 944, 1320, 756, Color::BLUE);
});

// Port of: gm/hairlines.cpp#L319 (chrome/m156)
crate::def_gm!(HairlinesGM, HairlinesGm::default());
