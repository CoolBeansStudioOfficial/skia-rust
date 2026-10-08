// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/overstroke.cpp (chrome/m156)

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

// This GM exercises stroking of paths with large stroke lengths, which is
// referred to as "overstroke" for brevity. In Skia as of 8/2016 we offset
// each part of the curve the request amount even if it makes the offsets
// overlap and create holes. There is not a really great algorithm for this
// and several other 2D graphics engines have the same bug.
//
// The old Nvidia Path Renderer used to yield correct results, so a possible
// direction of attack is to use the GPU and a completely different algorithm.
//
// See crbug.com/589769 skbug.com/40036571 skbug.com/40036572

use crate::prelude::*;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_measure::PathMeasure;
use skia_rust_core::path_utils::{fill_path_with_paint, fill_path_with_paint_to_path};
use skia_rust_core::point::point_priv;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;

const OVERSTROKE_WIDTH: f32 = 500.0;
const NORMALSTROKE_WIDTH: f32 = 3.0;

// Port of: gm/overstroke.cpp#L41-L49 (chrome/m156)
fn make_normal_paint() -> Paint {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_style(Style::Stroke);
    p.set_stroke_width(NORMALSTROKE_WIDTH);
    p.set_color(Color::BLUE);

    p
}

// Port of: gm/overstroke.cpp#L51-L58 (chrome/m156)
fn make_overstroke_paint() -> Paint {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_style(Style::Stroke);
    p.set_stroke_width(OVERSTROKE_WIDTH);

    p
}

// Port of: gm/overstroke.cpp#L60-L66 (chrome/m156)
fn quad_path() -> Path {
    PathBuilder::new()
        .move_to((0.0, 0.0))
        .line_to((100.0, 0.0))
        .quad_to((50.0, -40.0), (0.0, 0.0))
        .close()
        .detach()
}

// Port of: gm/overstroke.cpp#L68-L73 (chrome/m156)
fn cubic_path() -> Path {
    PathBuilder::new()
        .move_to((0.0, 0.0))
        .cubic_to((25.0, 75.0), (75.0, -50.0), (100.0, 0.0))
        .detach()
}

// Port of: gm/overstroke.cpp#L75-L79 (chrome/m156)
fn oval_path() -> Path {
    let oval = Rect::from_xywh(0.0, -25.0, 100.0, 50.0);

    PathBuilder::new()
        .arc_to(oval, 0.0, 359.0, true)
        .close()
        .detach()
}

// Port of: gm/overstroke.cpp#L81-L101 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // int loop index, as in C++
fn ribs_path(path: &Path, radius: f32) -> Path {
    let mut ribs = PathBuilder::new();

    let spacing: f32 = 5.0;
    let mut accum: f32 = 0.0;

    let meas = PathMeasure::new(path, false, None);
    let length = meas.length();
    let mut pos = Point::default();
    let mut tan = Vector::default();
    while accum < length {
        if meas.get_pos_tan(accum, Some(&mut pos), Some(&mut tan)) {
            tan.scale(radius);
            point_priv::rotate_ccw_in_place(&mut tan);
            ribs.add_line(pos + tan, pos - tan);
        }
        accum += spacing;
    }

    ribs.detach()
}

// Port of: gm/overstroke.cpp#L103-L111 (chrome/m156)
fn draw_ribs(canvas: &Canvas, path: &Path) {
    let ribs = ribs_path(path, OVERSTROKE_WIDTH / 2.0);
    let mut p = make_normal_paint();
    p.set_stroke_width(1.0);
    p.set_color(Color::BLUE);
    p.set_color(Color::GREEN);

    canvas.draw_path(&ribs, &p);
}

// Port of: gm/overstroke.cpp#L115-L124 (chrome/m156)
fn draw_small_quad(canvas: &Canvas) {
    // scaled so it's visible
    // canvas->scale(8, 8);

    let p = make_normal_paint();
    let path = quad_path();

    draw_ribs(canvas, &path);
    canvas.draw_path(&path, &p);
}

// Port of: gm/overstroke.cpp#L126-L132 (chrome/m156)
fn draw_large_quad(canvas: &Canvas) {
    let p = make_overstroke_paint();
    let path = quad_path();

    canvas.draw_path(&path, &p);
    draw_ribs(canvas, &path);
}

// Port of: gm/overstroke.cpp#L134-L145 (chrome/m156)
fn draw_quad_fillpath(canvas: &Canvas) {
    let path = quad_path();
    let p = make_overstroke_paint();

    let mut fillp = make_normal_paint();
    fillp.set_color(Color::MAGENTA);

    let mut fillpath = PathBuilder::new();
    fill_path_with_paint(&path, &p, &mut fillpath, None::<&Rect>, None::<Matrix>);

    canvas.draw_path(&fillpath.detach(), &fillp);
}

// Port of: gm/overstroke.cpp#L147-L151 (chrome/m156)
fn draw_stroked_quad(canvas: &Canvas) {
    canvas.translate((400.0, 0.0));
    draw_large_quad(canvas);
    draw_quad_fillpath(canvas);
}

// Port of: gm/overstroke.cpp#L155-L161 (chrome/m156)
fn draw_small_cubic(canvas: &Canvas) {
    let p = make_normal_paint();
    let path = cubic_path();

    draw_ribs(canvas, &path);
    canvas.draw_path(&path, &p);
}

// Port of: gm/overstroke.cpp#L163-L169 (chrome/m156)
fn draw_large_cubic(canvas: &Canvas) {
    let p = make_overstroke_paint();
    let path = cubic_path();

    canvas.draw_path(&path, &p);
    draw_ribs(canvas, &path);
}

// Port of: gm/overstroke.cpp#L171-L182 (chrome/m156)
fn draw_cubic_fillpath(canvas: &Canvas) {
    let path = cubic_path();
    let p = make_overstroke_paint();

    let mut fillp = make_normal_paint();
    fillp.set_color(Color::MAGENTA);

    let mut fillpath = PathBuilder::new();
    fill_path_with_paint(&path, &p, &mut fillpath, None::<&Rect>, None::<Matrix>);

    canvas.draw_path(&fillpath.detach(), &fillp);
}

// Port of: gm/overstroke.cpp#L184-L188 (chrome/m156)
fn draw_stroked_cubic(canvas: &Canvas) {
    canvas.translate((400.0, 0.0));
    draw_large_cubic(canvas);
    draw_cubic_fillpath(canvas);
}

// Port of: gm/overstroke.cpp#L192-L199 (chrome/m156)
fn draw_small_oval(canvas: &Canvas) {
    let p = make_normal_paint();

    let path = oval_path();

    draw_ribs(canvas, &path);
    canvas.draw_path(&path, &p);
}

// Port of: gm/overstroke.cpp#L201-L207 (chrome/m156)
fn draw_large_oval(canvas: &Canvas) {
    let p = make_overstroke_paint();
    let path = oval_path();

    canvas.draw_path(&path, &p);
    draw_ribs(canvas, &path);
}

// Port of: gm/overstroke.cpp#L209-L219 (chrome/m156)
fn draw_oval_fillpath(canvas: &Canvas) {
    let path = oval_path();
    let p = make_overstroke_paint();

    let mut fillp = make_normal_paint();
    fillp.set_color(Color::MAGENTA);

    let (fillpath, _) = fill_path_with_paint_to_path(&path, &p);

    canvas.draw_path(&fillpath, &fillp);
}

// Port of: gm/overstroke.cpp#L221-L225 (chrome/m156)
fn draw_stroked_oval(canvas: &Canvas) {
    canvas.translate((400.0, 0.0));
    draw_large_oval(canvas);
    draw_oval_fillpath(canvas);
}

// Port of: gm/overstroke.cpp#L229-L232 (chrome/m156)
const EXAMPLES: [fn(&Canvas); 6] = [
    draw_small_quad,
    draw_stroked_quad,
    draw_small_cubic,
    draw_stroked_cubic,
    draw_small_oval,
    draw_stroked_oval,
];

// Port of: gm/overstroke.cpp#L234-L251 (chrome/m156)
crate::def_simple_gm!(OverStroke, canvas, 500, 500, {
    let length = EXAMPLES.len();
    let width = 2;

    for i in 0..length {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // small indices
        let x = (i % width) as i32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // small indices
        let y = (i / width) as i32;

        canvas.save();
        canvas.translate((150.0 * x as f32, 150.0 * y as f32));
        canvas.scale((0.2, 0.2));
        canvas.translate((300.0, 400.0));

        EXAMPLES[i](canvas);

        canvas.restore();
    }
});
