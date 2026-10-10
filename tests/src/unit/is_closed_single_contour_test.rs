// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/IsClosedSingleContourTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_priv::is_closed_single_contour_path;

use crate::{def_test, reporter_assert};

// Port of: tests/IsClosedSingleContourTest.cpp#L11-L53 (chrome/m156)
def_test!(IsClosedSingleContourTest, |reporter| {
    let mut p = PathBuilder::new();
    reporter_assert!(reporter, !is_closed_single_contour_path(&p.detach()));
    p.close();
    reporter_assert!(reporter, !is_closed_single_contour_path(&p.detach()));
    p.move_to((10, 10));
    p.close();
    reporter_assert!(reporter, is_closed_single_contour_path(&p.detach()));
    p.move_to((10, 10));
    p.line_to((20, 20));
    p.close();
    reporter_assert!(reporter, is_closed_single_contour_path(&p.detach()));
    p.move_to((10, 10));
    p.line_to((20, 20));
    p.quad_to((30, 30), (40, 40));
    p.cubic_to((50, 50), (60, 60), (70, 70));
    p.conic_to((30, 30), (40, 40), 0.5);
    p.close();
    reporter_assert!(reporter, is_closed_single_contour_path(&p.detach()));
    p.move_to((10, 10));
    p.line_to((20, 20));
    p.line_to((20, 30));
    reporter_assert!(reporter, !is_closed_single_contour_path(&p.detach()));
    p.move_to((10, 10));
    p.line_to((20, 20));
    p.move_to((10, 10));
    p.line_to((20, 30));
    p.close();
    reporter_assert!(reporter, !is_closed_single_contour_path(&p.detach()));
    p.move_to((10, 10));
    p.line_to((20, 20));
    p.close();
    p.line_to((20, 30));
    p.close();
    reporter_assert!(reporter, !is_closed_single_contour_path(&p.detach()));
});
