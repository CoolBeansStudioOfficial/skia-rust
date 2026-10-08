// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/polygons.cpp (chrome/m156)

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
use skia_rust_core::paint::{Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::scalar::{SCALAR_PI, scalar_cos, scalar_sin};

const NUM_POLYGONS: usize = 8;
const CELL_SIZE: i32 = 100;
const NUM_EXTRA_STYLES: i32 = 2;
const NUM_STROKE_WIDTHS: i32 = 3;
const NUM_JOINS: i32 = 3;

// This GM tests a grab-bag of convex and concave polygons. They are triangles,
// trapezoid, diamond, polygons with lots of edges, several concave polygons...
// But rectangles are excluded.
// Port of: gm/polygons.cpp#L23-L170 (chrome/m156)
struct PolygonsGm {
    polygons: Vec<Path>,
}

impl PolygonsGm {
    fn new() -> Self {
        Self {
            polygons: Vec::new(),
        }
    }
}

// Set the location for the current test on the canvas
// Port of: gm/polygons.cpp#L89-L93 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // int cell coordinates, as in C++
fn set_location(canvas: &Canvas, counter: usize, line_num: usize) {
    let x = (CELL_SIZE as f32) * ((counter % line_num) as f32) + 30.0 + 1.0 / 4.0;
    let y = (CELL_SIZE as f32) * ((counter / line_num) as f32) + 30.0 + 3.0 / 4.0;
    canvas.translate((x, y));
}

// Port of: gm/polygons.cpp#L95-L102 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparison with the stroke width 40, as in C++
fn set_color_and_alpha(paint: &mut Paint, rand: &mut Random) {
    let mut color = rand.next_u();
    color |= 0xff00_0000;
    paint.set_color(Color::new(color));
    if 40.0 == paint.stroke_width() {
        paint.set_alpha(0xA0);
    }
}

impl GM for PolygonsGm {
    fn name(&self) -> String {
        "polygons".to_string()
    }

    fn size(&mut self) -> ISize {
        let width = (NUM_POLYGONS as i32) * CELL_SIZE + 40;
        let height = (NUM_JOINS * NUM_STROKE_WIDTHS + NUM_EXTRA_STYLES) * CELL_SIZE + 40;
        ISize::new(width, height)
    }

    // Construct all polygons
    // Port of: gm/polygons.cpp#L42-L86 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // int loop index, as in C++
    #[allow(clippy::too_many_lines)]
    fn on_once_before_draw(&mut self) {
        let p0 = [
            Point::new(0.0, 0.0),
            Point::new(60.0, 0.0),
            Point::new(90.0, 40.0),
        ]; // triangle
        let p1 = [
            Point::new(0.0, 0.0),
            Point::new(0.0, 40.0),
            Point::new(60.0, 40.0),
            Point::new(40.0, 0.0),
        ]; // trapezoid
        let p2 = [
            Point::new(0.0, 0.0),
            Point::new(40.0, 40.0),
            Point::new(80.0, 40.0),
            Point::new(40.0, 0.0),
        ]; // diamond
        let p3 = [
            Point::new(10.0, 0.0),
            Point::new(50.0, 0.0),
            Point::new(60.0, 10.0),
            Point::new(60.0, 30.0),
            Point::new(50.0, 40.0),
            Point::new(10.0, 40.0),
            Point::new(0.0, 30.0),
            Point::new(0.0, 10.0),
        ]; // octagon
        // circle-like polygons with 32-edges.
        let mut p4 = [Point::default(); 32];
        for (i, pt) in p4.iter_mut().enumerate() {
            let angle = 2.0 * SCALAR_PI * (i as f32) / 32.0;
            *pt = Point::new(
                20.0 * scalar_cos(angle) + 20.0,
                20.0 * scalar_sin(angle) + 20.0,
            );
        }
        let p5 = [
            Point::new(0.0, 0.0),
            Point::new(20.0, 20.0),
            Point::new(0.0, 40.0),
            Point::new(60.0, 20.0),
        ]; // concave polygon with 4 edges
        let p6 = [
            Point::new(0.0, 40.0),
            Point::new(0.0, 30.0),
            Point::new(15.0, 30.0),
            Point::new(15.0, 20.0),
            Point::new(30.0, 20.0),
            Point::new(30.0, 10.0),
            Point::new(45.0, 10.0),
            Point::new(45.0, 0.0),
            Point::new(60.0, 0.0),
            Point::new(60.0, 40.0),
        ]; // stairs-like polygon
        let p7 = [
            Point::new(0.0, 20.0),
            Point::new(20.0, 20.0),
            Point::new(30.0, 0.0),
            Point::new(40.0, 20.0),
            Point::new(60.0, 20.0),
            Point::new(45.0, 30.0),
            Point::new(55.0, 50.0),
            Point::new(30.0, 40.0),
            Point::new(5.0, 50.0),
            Point::new(15.0, 30.0),
        ]; // five-point stars

        let pgs: [&[Point]; NUM_POLYGONS] = [&p0, &p1, &p2, &p3, &p4, &p5, &p6, &p7];

        for pts in pgs {
            let mut b = PathBuilder::new();
            b.move_to(pts[0]);
            for pt in &pts[1..] {
                b.line_to(*pt);
            }
            b.close();
            self.polygons.push(b.detach());
        }
    }

    // Port of: gm/polygons.cpp#L104-L159 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar of the stroke widths
    fn on_draw(&mut self, canvas: &Canvas) {
        // Stroke widths are:
        // 0(may use hairline rendering), 10(common case for stroke-style)
        // 40(>= geometry width/height, make the contour filled in fact)
        const STROKE_WIDTHS: [i32; 3] = [0, 10, 40];

        const JOINS: [Join; 3] = [Join::Miter, Join::Round, Join::Bevel];

        let mut counter: usize = 0;
        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        let mut rand = Random::default();
        // For stroke style painter
        paint.set_style(Style::Stroke);
        for join in JOINS {
            for width in STROKE_WIDTHS {
                for i in 0..self.polygons.len() {
                    canvas.save();
                    set_location(canvas, counter, self.polygons.len());

                    set_color_and_alpha(&mut paint, &mut rand);
                    paint.set_stroke_join(join);
                    paint.set_stroke_width(width as f32);

                    canvas.draw_path(&self.polygons[i], &paint);
                    canvas.restore();
                    counter += 1;
                }
            }
        }

        // For stroke-and-fill style painter and fill style painter
        const STYLES: [Style; 2] = [Style::StrokeAndFill, Style::Fill];

        paint.set_stroke_join(Join::Miter);
        paint.set_stroke_width(20.0);
        for style in STYLES {
            paint.set_style(style);
            for i in 0..self.polygons.len() {
                canvas.save();
                set_location(canvas, counter, self.polygons.len());
                set_color_and_alpha(&mut paint, &mut rand);
                canvas.draw_path(&self.polygons[i], &paint);
                canvas.restore();
                counter += 1;
            }
        }
    }
}

crate::def_gm!(PolygonsGM, PolygonsGm::new());

// see crbug.com/1197461
// Port of: gm/polygons.cpp#L177-L191 (chrome/m156)
crate::def_simple_gm!(conjoined_polygons, canvas, 400, 400, {
    let mut b = PathBuilder::new();
    b.move_to((0.0, 120.0));
    b.line_to((0.0, 0.0));
    b.line_to((50.0, 330.0));
    b.line_to((90.0, 0.0));
    b.line_to((340.0, 0.0));
    b.line_to((90.0, 330.0));
    b.line_to((50.0, 330.0));
    b.close();

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    canvas.draw_path(&b.detach(), &paint);
});
