// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PreChopPathCurvesTest.cpp (chrome/m156)

#![cfg(test)]
// The float literals are copied digit for digit from the C++ test, so they stay unseparated.
#![allow(clippy::unreadable_literal, clippy::excessive_precision)]

use skia_rust_core::matrix::Matrix;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::Rect;
use skia_rust_gpu::tessellate::tessellation::pre_chop_path_curves;

use crate::def_test;

// Port of: tests/PreChopPathCurvesTest.cpp#L16-L45 (chrome/m156), `PreChopPathCurves`.
def_test!(PreChopPathCurves, |_reporter| {
    let mut builder = PathBuilder::new();
    // These particular test cases can get stuck in infinite recursion due to limited fp32
    // precision. (Although they will not with the provided tessellationPrecision values; we had to
    // lower precision in order to avoid the "viewport size" assert in PreChopPathCurves.) Bump the
    // tessellationPreciion up to 4 and run these tests in order to verify our bail condition for
    // infinite recursion caused by bad fp32 precision. If the test completes, it passed.
    builder
        .move_to((11.171727877046647_f32, -11.78621173228717_f32))
        .quad_to(
            (11.171727877046647_f32, -11.78621173228717_f32),
            (8.33583747124031_f32, 77.27177002747368_f32),
        )
        .cubic_to(
            (8.33583747124031_f32, 77.27177002747368_f32),
            (8.33583747124031_f32, 77.27177002747368_f32),
            (11.171727877046647_f32, -11.78621173228717_f32),
        )
        .conic_to(
            (11.171727877046647_f32, -11.78621173228717_f32),
            (8.33583747124031_f32, 77.27177002747368_f32),
            1e-6_f32,
        )
        .conic_to(
            (8.33583747124031_f32, 77.27177002747368_f32),
            (11.171727877046647_f32, -11.78621173228717_f32),
            1e6_f32,
        );
    let p = builder.detach();

    let m = Matrix::scale((138.68622826903837_f32, 74192976757580.44189_f32));
    // The C++ test discards the chopped path: it only checks that the call completes.
    let _ = pre_chop_path_curves(
        1.0 / 16.0,
        &p,
        &m,
        &Rect::new(1000.0, -74088852800000.0, 3000.0, -74088852700000.0),
    );

    let m = Matrix::scale((138.68622826903837_f32, 74192976757580.44189_f32 * 0.3_f32));
    // The C++ test discards the chopped path: it only checks that the call completes.
    let _ = pre_chop_path_curves(
        0.25,
        &p,
        &m,
        &Rect::new(1000.0, -22226658140000.0, 3000.0, -22226658130000.0),
    );

    let m = Matrix::scale((138.68622826903837_f32, 74192976757580.44189_f32 / 4.0_f32));
    // The C++ test discards the chopped path: it only checks that the call completes.
    let _ = pre_chop_path_curves(
        0.25,
        &p,
        &m,
        &Rect::new(1000.0, -18522213200000.0, 3000.0, -18522213100000.0),
    );
});
