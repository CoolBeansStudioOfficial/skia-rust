// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Sanity tests of the path effects that are not ports of Skia tests.

use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::PathEffect;
use skia_rust_core::path_utils::fill_path_with_paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::stroke_rec::StrokeRec;

use crate::dash_path_effect::new;

fn stroke_paint(width: scalar) -> Paint {
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(width);
    paint
}

fn stroke_rec(width: scalar) -> StrokeRec {
    StrokeRec::from_paint(&stroke_paint(width), None, None)
}

// The path-effect part of what `DashPathEffectTest_asPoints_limit` does through
// `Canvas::drawLine` (which needs D6): a huge stroke width makes the cull rect outset by a
// large amount, and the dash count must stay bounded.
#[test]
fn long_line_with_many_dashes_is_bounded() {
    let dash = new(&[1.0, 1.0], 0.0).unwrap();
    let path = Path::line((1.0, 1.0), (1.0, 5.0e10));
    let cull = Rect::from_wh(256.0, 256.0);
    let mut builder = PathBuilder::new();
    let mut paint = stroke_paint(5.0e10);
    paint.set_path_effect(dash);
    let _ = fill_path_with_paint(&path, &paint, &mut builder, Some(&cull), None);
}

#[test]
fn dashes_a_horizontal_line() {
    let dash = new(&[2.0, 2.0], 0.0).unwrap();
    let path = Path::line((0.0, 0.0), (10.0, 0.0));
    let (builder, rec) = dash.filter_path(&path, &stroke_rec(1.0), None).unwrap();
    // The special line fast path strokes the dashes itself.
    assert!(rec.is_fill_style());
    let dashed = builder.snapshot();
    // dashes [0,2], [4,6] and [8,10]: three quads of four points
    assert_eq!(dashed.count_points(), 12);
    assert_eq!(dashed.bounds(), &Rect::new(0.0, -0.5, 10.0, 0.5));
}

#[test]
fn invalid_dashes_are_rejected() {
    assert!(new(&[], 0.0).is_none());
    assert!(new(&[1.0], 0.0).is_none());
    assert!(new(&[1.0, -1.0], 0.0).is_none());
    assert!(new(&[0.0, 0.0], 0.0).is_none());
    assert!(new(&[1.0, 1.0], scalar::NAN).is_none());
}

#[test]
fn sum_and_compose_filter() {
    let dash = new(&[2.0, 2.0], 0.0).unwrap();
    let sum = PathEffect::sum(dash.clone(), dash.clone());
    let path = Path::line((0.0, 0.0), (10.0, 0.0));
    let (builder, _) = sum.filter_path(&path, &stroke_rec(1.0), None).unwrap();
    // The first dash turns the shared rec into a fill, so the second one does not apply.
    assert_eq!(builder.snapshot().count_points(), 12);
    assert!(PathEffect::compose(dash.clone(), dash).compute_fast_bounds(None));
}
