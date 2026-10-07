// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/DashPathEffectTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_test, reporter_assert};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, DEFAULT_MITER_LIMIT, Join, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::{PathEffect, PointData};
use skia_rust_core::path_utils::fill_path_with_stroke_rec_and_effect;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{SCALAR_INFINITY, scalar};
use skia_rust_core::stroke_rec::StrokeRec;
use skia_rust_effects::dash_path_effect::DashPathEffectExt;

// crbug.com/348821 was rooted in SkDashPathEffect refusing to flatten and unflatten itself when
// the effect is nonsense.  Here we test that it fails when passed nonsense parameters.

// Port of: tests/DashPathEffectTest.cpp#L32-L38 (chrome/m156)
def_test!(
    #[allow(clippy::excessive_precision)] // float literals are copied verbatim from the C++
    DashPathEffectTest_crbug_348821,
    |r| {
        let intervals: [scalar; 2] = [1.769_343_6e36, 2.802_596_9e-45]; // Values from bug.
        let phase: scalar = SCALAR_INFINITY; // Used to force a nonsense effect.
        let dash = PathEffect::dash(&intervals, phase);

        reporter_assert!(r, dash.is_none());
    }
);

// Test out the asPoint culling behavior.
// Port of: tests/DashPathEffectTest.cpp#L41-L98 (chrome/m156)
def_test!(DashPathEffectTest_asPoints, |r| {
    const K_NUM_MATS: usize = 3;

    struct TestCases {
        pts: [Point; 2],
        expected_result: bool,
    }

    fn tc(pts: [(scalar, scalar); 2], expected_result: bool) -> TestCases {
        TestCases {
            pts: [
                Point::new(pts[0].0, pts[0].1),
                Point::new(pts[1].0, pts[1].1),
            ],
            expected_result,
        }
    }

    let intervals: [scalar; 2] = [1.0, 1.0];
    let dash = PathEffect::dash(&intervals, 0.0).unwrap();

    let cull = Rect::from_wh(1.0, 1.0);

    let test_cases = [
        tc([(-5.0, 0.5), (-4.0, 0.5)], false), // off to the left
        tc([(4.0, 0.5), (5.0, 0.5)], false),   // off to the right
        tc([(0.5, 4.0), (0.5, 5.0)], false),   // off the bottom
        tc([(0.5, -5.0), (0.5, -4.0)], false), // off the top
        tc([(0.5, 0.2), (0.5, 0.8)], true),    // entirely inside vertical
        tc([(0.2, 0.5), (0.8, 0.5)], true),    // entirely inside horizontal
        tc([(0.5, -5.0), (0.5, 5.0)], true),   // straddles both sides vertically
        tc([(-5.0, 0.5), (5.0, 0.5)], true),   // straddles both sides horizontally
        tc([(0.5, -5.0), (0.5, 0.5)], true),   // straddles top
        tc([(0.5, 5.0), (0.5, 0.5)], true),    // straddles bottom
        tc([(-5.0, 0.5), (0.5, 0.5)], true),   // straddles left
        tc([(5.0, 0.5), (0.5, 0.5)], true),    // straddles right
        tc([(0.5, 0.5), (0.5, 0.5)], false),   // zero length
    ];

    // SkPaint paint; paint.setStyle(kStroke_Style); paint.setStrokeWidth(1.0f);
    // SkStrokeRec rec(paint);
    let rec = StrokeRec::from_paint_params(
        Style::Stroke,
        1.0,
        DEFAULT_MITER_LIMIT,
        Cap::Butt,
        Join::Miter,
        1.0,
    );

    let mut mats = [const { Matrix::new_identity() }; K_NUM_MATS];
    mats[0].reset();
    mats[1].set_rotate(90.0, Point::new(0.5, 0.5));
    mats[2].set_translate((10.0, 10.0));

    for (i, mat) in mats.iter().enumerate() {
        for test_case in &test_cases {
            for k in 0..2 {
                // exercise alternating endpoints
                let mut results = PointData::default();
                let src = Path::line(test_case.pts[k], test_case.pts[(k + 1) % 2]);

                let actual_result = dash.as_points(&mut results, &src, &rec, mat, Some(&cull));
                if i < 2 {
                    reporter_assert!(r, actual_result == test_case.expected_result);
                } else {
                    // On the third pass all the lines should be outside the translated cull rect
                    reporter_assert!(r, !actual_result);
                }
            }
        }
    }
});

// Port of: tests/DashPathEffectTest.cpp#L100-L115 (chrome/m156)
def_test!(DashPath_bug4871, |_r| {
    let path = PathBuilder::new()
        .move_to((30.0, 24.0))
        .cubic_to((30.002, 24.0), (30.0, 24.0), (30.0, 24.0))
        .close()
        .detach();

    let intervals: [scalar; 2] = [1.0, 1.0];
    let dash = PathEffect::dash(&intervals, 0.0);

    // SkPaint paint; paint.setStyle(kStroke_Style); paint.setPathEffect(dash);
    let rec = StrokeRec::from_paint_params(
        Style::Stroke,
        0.0,
        DEFAULT_MITER_LIMIT,
        Cap::Butt,
        Join::Miter,
        1.0,
    );

    // (void)skpathutils::FillPathWithPaint(path, paint);
    let mut builder = PathBuilder::new();
    let _ = fill_path_with_stroke_rec_and_effect(
        &path,
        rec,
        dash.as_ref(),
        &mut builder,
        None,
        Matrix::i(),
    );
});

// Verify that long lines with many dashes don't cause overflows/OOMs.
// Port of: tests/DashPathEffectTest.cpp#L118-L129 (chrome/m156)
def_test!(
    #[ignore = "needs Canvas and Surface (D6): drawLine is not ported yet"]
    DashPathEffectTest_asPoints_limit,
    |_r| {
        // TODO(D6): sk_sp<SkSurface> surface(SkSurfaces::Raster(SkImageInfo::MakeN32Premul(256,
        // 256))); SkCanvas* canvas = surface->getCanvas(); SkPaint p; p.setStyle(kStroke_Style);
        // p.setStrokeWidth(5.0e10f) (force the bounds to outset by a large amount);
        // const SkScalar intervals[] = { 1, 1 }; p.setPathEffect(SkDashPathEffect::Make(
        // intervals, 0)); canvas->drawLine(1, 1, 1, 5.0e10f, p);
    }
);

// This used to cause SkDashImpl to walk off the end of the intervals array, due to underflow
// trying to substract a smal value from a large one in floats.
// Port of: tests/DashPathEffectTest.cpp#L133-L144 (chrome/m156)
def_test!(DashCrazy_crbug_875494, |_r| {
    let vals: [scalar; 6] = [98.0, 94.0, 2_888_458_849.0, 227.0, 0.0, 197.0];

    let cull = Rect::from_xywh(43.0, 236.0, 57.0, 149.0);
    let path = Path::rect(cull, None);

    let mut builder = PathBuilder::new();
    // SkPaint paint; paint.setStyle(kStroke_Style); paint.setPathEffect(Make(vals, 222));
    let dash = PathEffect::dash(&vals, 222.0);
    let rec = StrokeRec::from_paint_params(
        Style::Stroke,
        0.0,
        DEFAULT_MITER_LIMIT,
        Cap::Butt,
        Join::Miter,
        1.0,
    );
    fill_path_with_stroke_rec_and_effect(
        &path,
        rec,
        dash.as_ref(),
        &mut builder,
        Some(&cull),
        Matrix::i(),
    );
});
