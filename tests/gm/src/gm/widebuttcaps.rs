// Copyright 2019 Google LLC.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/widebuttcaps.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::paint::{Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::random::Random;

// Port of: gm/widebuttcaps.cpp#L15-L17 (chrome/m156)
const STROKE_WIDTH: f32 = 100.0;
const TEST_WIDTH: i32 = 120 * 4;
const TEST_HEIGHT: i32 = 120 * 3 + 140;

// Port of: gm/widebuttcaps.cpp#L19-L43 (chrome/m156)
fn draw_strokes(canvas: &Canvas, rand: &mut Random, path: &Path, cubic: &Path) {
    let mut stroke_paint = Paint::default();
    stroke_paint.set_anti_alias(true);
    stroke_paint.set_stroke_width(STROKE_WIDTH);
    stroke_paint.set_style(Style::Stroke);

    let _arc = AutoCanvasRestore::guard(canvas, true);
    stroke_paint.set_stroke_join(Join::Bevel);
    stroke_paint.set_color(Color::new(rand.next_u() | 0xff808080));
    canvas.draw_path(path, &stroke_paint);

    canvas.translate((120.0, 0.0));
    stroke_paint.set_stroke_join(Join::Round);
    stroke_paint.set_color(Color::new(rand.next_u() | 0xff808080));
    canvas.draw_path(path, &stroke_paint);

    canvas.translate((120.0, 0.0));
    stroke_paint.set_stroke_join(Join::Miter);
    stroke_paint.set_color(Color::new(rand.next_u() | 0xff808080));
    canvas.draw_path(path, &stroke_paint);

    canvas.translate((120.0, 0.0));
    stroke_paint.set_color(Color::new(rand.next_u() | 0xff808080));
    canvas.draw_path(cubic, &stroke_paint);
}

// Port of: gm/widebuttcaps.cpp#L45-L73 (chrome/m156)
fn draw_test(canvas: &Canvas) {
    let mut rand = Random::default();

    canvas.clear(Color::BLACK);

    let _arc = AutoCanvasRestore::guard(canvas, true);
    canvas.translate((60.0, 60.0));

    draw_strokes(
        canvas,
        &mut rand,
        &PathBuilder::new()
            .line_to((10.0, 0.0))
            .line_to((10.0, 10.0))
            .detach(),
        &PathBuilder::new()
            .cubic_to((10.0, 0.0), (10.0, 0.0), (10.0, 10.0))
            .detach(),
    );
    canvas.translate((0.0, 120.0));

    draw_strokes(
        canvas,
        &mut rand,
        &PathBuilder::new()
            .line_to((0.0, -10.0))
            .line_to((0.0, 10.0))
            .detach(),
        &PathBuilder::new()
            .cubic_to((0.0, -10.0), (0.0, -10.0), (0.0, 10.0))
            .detach(),
    );
    canvas.translate((0.0, 120.0));

    draw_strokes(
        canvas,
        &mut rand,
        &PathBuilder::new()
            .line_to((0.0, -10.0))
            .line_to((10.0, -10.0))
            .line_to((10.0, 10.0))
            .line_to((0.0, 10.0))
            .detach(),
        &PathBuilder::new()
            .cubic_to((0.0, -10.0), (10.0, 10.0), (0.0, 10.0))
            .detach(),
    );
    canvas.translate((0.0, 140.0));

    draw_strokes(
        canvas,
        &mut rand,
        &PathBuilder::new()
            .line_to((0.0, -10.0))
            .line_to((10.0, -10.0))
            .line_to((10.0, 0.0))
            .line_to((0.0, 0.0))
            .detach(),
        &PathBuilder::new()
            .cubic_to((0.0, -10.0), (10.0, 0.0), (0.0, 0.0))
            .detach(),
    );
    canvas.translate((0.0, 120.0));
}

// Port of: gm/widebuttcaps.cpp#L75-L78 (chrome/m156)
crate::def_simple_gm!(widebuttcaps, canvas, TEST_WIDTH, TEST_HEIGHT, {
    canvas.clear(Color::BLACK);
    draw_test(canvas);
});
