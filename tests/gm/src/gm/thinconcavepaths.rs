// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/thinconcavepaths.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;

// Test thin stroked rect (stroked "by hand", not by stroking).
// Port of: gm/thinconcavepaths.cpp#L21-L33 (chrome/m156)
fn draw_thin_stroked_rect(canvas: &Canvas, paint: &Paint, width: f32) {
    let path = PathBuilder::new()
        .move_to((10.0 + width, 10.0 + width))
        .line_to((40.0, 10.0 + width))
        .line_to((40.0, 20.0))
        .line_to((10.0 + width, 20.0))
        .move_to((10.0, 10.0))
        .line_to((10.0, 20.0 + width))
        .line_to((40.0 + width, 20.0 + width))
        .line_to((40.0 + width, 10.0))
        .detach();
    canvas.draw_path(&path, paint);
}

// Port of: gm/thinconcavepaths.cpp#L35-L44 (chrome/m156)
fn draw_thin_right_angle(canvas: &Canvas, paint: &Paint, width: f32) {
    let path = PathBuilder::new()
        .move_to((10.0 + width, 10.0 + width))
        .line_to((40.0, 10.0 + width))
        .line_to((40.0, 20.0))
        .line_to((40.0 + width, 20.0 + width))
        .line_to((40.0 + width, 10.0))
        .line_to((10.0, 10.0))
        .detach();
    canvas.draw_path(&path, paint);
}

// Test thin horizontal line (<1 pixel) which should give lower alpha.
// Port of: gm/thinconcavepaths.cpp#L47-L56 (chrome/m156)
fn draw_golf_club(canvas: &Canvas, paint: &Paint, width: f32) {
    let path = PathBuilder::new()
        .move_to((20.0, 10.0))
        .line_to((80.0, 10.0))
        .line_to((80.0, 10.0 + width))
        .line_to((30.0, 10.0 + width))
        .line_to((30.0, 20.0))
        .line_to((20.0, 20.0))
        .detach();
    canvas.draw_path(&path, paint);
}

// Test thin lines between two filled regions. The outer edges overlap, but
// there are no inverted edges to fix.
// Port of: gm/thinconcavepaths.cpp#L60-L72 (chrome/m156)
fn draw_barbell(canvas: &Canvas, paint: &Paint, width: f32) {
    let offset = width * 0.5;
    let path = PathBuilder::new()
        .move_to((30.0, 5.0))
        .line_to((40.0 - offset, 15.0 - offset))
        .line_to((60.0 + offset, 15.0 - offset))
        .line_to((70.0, 5.0))
        .line_to((70.0, 25.0))
        .line_to((60.0 + offset, 15.0 + offset))
        .line_to((40.0 - offset, 15.0 + offset))
        .line_to((30.0, 25.0))
        .detach();
    canvas.draw_path(&path, paint);
}

// Test a thin rectangle and triangle. The top and bottom inner edges of the
// rectangle and all inner edges of the triangle invert on stroking.
// Port of: gm/thinconcavepaths.cpp#L76-L87 (chrome/m156)
fn draw_thin_rect_and_triangle(canvas: &Canvas, paint: &Paint, width: f32) {
    let path = PathBuilder::new()
        .move_to((30.0, 5.0))
        .line_to((30.0 + width, 5.0))
        .line_to((30.0 + width, 25.0))
        .line_to((30.0, 25.0))
        .move_to((40.0, 5.0))
        .line_to((40.0 + width, 5.0))
        .line_to((40.0, 25.0))
        .detach();
    canvas.draw_path(&path, paint);
}

// Two triangles joined by a very thin bridge. The tiny triangle formed
// by the inner edges at the bridge is inverted.
// (These are actually now more phat pants than hipster pants.)
// Port of: gm/thinconcavepaths.cpp#L91-L100 (chrome/m156)
fn draw_hipster_pants(canvas: &Canvas, paint: &Paint, width: f32) {
    let path = PathBuilder::new()
        .move_to((10.0, 10.0))
        .line_to((10.0, 20.0))
        .line_to((50.0, 10.0 + width))
        .line_to((90.0, 20.0))
        .line_to((90.0, 10.0))
        .detach();
    canvas.draw_path(&path, paint);
}

// A thin z-shape whose interior inverts on stroking. The top and bottom inner edges invert, and
// the connector edges at the "elbows" intersect the inner edges.
// Port of: gm/thinconcavepaths.cpp#L104-L115 (chrome/m156)
fn draw_skinny_snake(canvas: &Canvas, paint: &Paint, width: f32) {
    let path = PathBuilder::new()
        .move_to((20.0 + width, 10.0))
        .line_to((20.0 + width, 20.0))
        .line_to((10.0 + width, 30.0))
        .line_to((10.0 + width, 40.0))
        .line_to((10.0 - width, 40.0))
        .line_to((10.0 - width, 30.0))
        .line_to((20.0 - width, 20.0))
        .line_to((20.0 - width, 10.0))
        .detach();
    canvas.draw_path(&path, paint);
}

// Test pointy features whose outer edges extend far to the right on stroking.
// Port of: gm/thinconcavepaths.cpp#L118-L127 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // double arithmetic narrowed to float, as in C++
fn draw_pointy_golf_club(canvas: &Canvas, paint: &Paint, width: f32) {
    let path = PathBuilder::new()
        .move_to((20.0, 10.0))
        .line_to((80.0, (10.0 + f64::from(width) * 0.5) as f32))
        .line_to((30.0, 10.0 + width))
        .line_to((30.0, 20.0))
        .line_to((20.0, 20.0))
        .detach();
    canvas.draw_path(&path, paint);
}

// Port of: gm/thinconcavepaths.cpp#L129-L141 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // double arithmetic narrowed to float, as in C++
fn draw_small_i(canvas: &Canvas, paint: &Paint, width: f32) {
    let w = f64::from(width);
    let f = |x: f64| x as f32;
    let path = PathBuilder::new()
        .move_to((f(1.25 - w), f(18.75 + w)))
        .line_to((f(1.25 - w), f(12.25 - w)))
        .line_to((f(2.50 + w), f(12.25 - w)))
        .line_to((f(2.50 + w), f(18.75 + w)))
        .move_to((f(1.25 - w), f(11.75 + w)))
        .line_to((f(1.25 - w), f(10.25 - w)))
        .line_to((f(2.50 + w), f(10.25 - w)))
        .line_to((f(2.50 + w), f(11.75 + w)))
        .detach();
    canvas.draw_path(&path, paint);
}

// Port of: gm/thinconcavepaths.cpp#L147-L218 (chrome/m156)
crate::def_simple_gm!(thinconcavepaths, canvas, 550, 400, {
    let mut paint = Paint::default();

    paint.set_anti_alias(true);
    paint.set_style(Style::Fill);

    canvas.save();
    let mut width: f32 = 0.5;
    while width < 2.05 {
        draw_thin_stroked_rect(canvas, &paint, width);
        canvas.translate((0.0, 25.0));
        width += 0.25;
    }
    canvas.restore();
    canvas.translate((50.0, 0.0));
    canvas.save();
    width = 0.5;
    while width < 2.05 {
        draw_thin_right_angle(canvas, &paint, width);
        canvas.translate((0.0, 25.0));
        width += 0.25;
    }
    canvas.restore();
    canvas.translate((40.0, 0.0));
    canvas.save();
    width = 0.2;
    while width < 2.1 {
        draw_golf_club(canvas, &paint, width);
        canvas.translate((0.0, 30.0));
        width += 0.2;
    }
    canvas.restore();
    canvas.translate((70.0, 0.0));
    canvas.save();
    width = 0.2;
    while width < 2.1 {
        draw_thin_rect_and_triangle(canvas, &paint, width);
        canvas.translate((0.0, 30.0));
        width += 0.2;
    }
    canvas.restore();
    canvas.translate((30.0, 0.0));
    canvas.save();

    width = 0.2;
    while width < 2.1 {
        draw_barbell(canvas, &paint, width);
        canvas.translate((0.0, 30.0));
        width += 0.2;
    }
    canvas.restore();
    canvas.translate((80.0, 0.0));
    canvas.save();
    width = 0.2;
    while width < 2.1 {
        draw_hipster_pants(canvas, &paint, width);
        canvas.translate((0.0, 30.0));
        width += 0.2;
    }
    canvas.restore();
    canvas.translate((100.0, 0.0));
    canvas.save();
    width = 0.2;
    while width < 2.1 {
        draw_skinny_snake(canvas, &paint, width);
        canvas.translate((0.0, 30.0));
        width += 0.2;
    }
    canvas.restore();
    canvas.translate((30.0, 0.0));
    canvas.save();
    width = 0.2;
    while width < 2.1 {
        draw_pointy_golf_club(canvas, &paint, width);
        canvas.translate((0.0, 30.0));
        width += 0.2;
    }
    canvas.restore();
    canvas.translate((100.0, 0.0));
    canvas.save();
    width = 0.0;
    while width < 0.5 {
        draw_small_i(canvas, &paint, width);
        canvas.translate((0.0, 30.0));
        width += 0.05;
    }
    canvas.restore();
    canvas.translate((100.0, 0.0));
});
