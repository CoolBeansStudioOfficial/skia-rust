// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: fuzz/FuzzPathMeasure.cpp (chrome/m156)

use skia_rust_core::path::Verb;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_measure::PathMeasure;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::scalar::scalar;

use crate::common::fuzz_evil_path;
use crate::def_fuzz;

// Port of: fuzz/FuzzPathMeasure.cpp#L17-L40 (chrome/m156)
def_fuzz!(PathMeasure, fuzz_PathMeasure, |fuzz| {
    let bits: u8 = fuzz.next();
    let mut distance: [scalar; 6] = [0.0; 6];
    for d in &mut distance {
        *d = fuzz.next();
    }
    let path = fuzz_evil_path(fuzz, Verb::Done);
    let bounds = path.bounds();
    // std::max(a, b) is `(a < b) ? b : a`.
    let (w, h) = (bounds.width(), bounds.height());
    let max_dim = if w < h { h } else { w };
    if max_dim > 1_000_000.0 {
        return;
    }
    let res_scale = max_dim / 1000.0;
    let mut measure = PathMeasure::new(&path, bits & 1 != 0, res_scale);
    let mut position = Point::default();
    let mut tangent = Vector::default();
    let _ = measure.get_pos_tan(distance[0], Some(&mut position), Some(&mut tangent));
    let mut dst = PathBuilder::new();
    let _ = measure.get_segment(distance[1], distance[2], &mut dst, (bits >> 1) & 1 != 0);
    let _ = measure.next_contour();
    let _ = measure.get_pos_tan(distance[3], Some(&mut position), Some(&mut tangent));
    let _ = measure.get_segment(distance[4], distance[5], &mut dst, (bits >> 2) & 1 != 0);
});
