// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: fuzz/FuzzPolyUtils.cpp (chrome/m156)
//
// skia-rust builds without `SK_ENABLE_OPTIMIZE_SIZE`, so this is the full fuzzer; the empty
// `PolyUtils` of the other branch (manifest `PolyUtils#2`) is excluded.

use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{scalar, scalar_round_to_scalar};
use skia_rust_core::utils::poly_utils::{
    get_polygon_winding, inset_convex_polygon, is_convex_polygon, is_simple_polygon,
    offset_simple_polygon, triangulate_simple_polygon,
};

use crate::def_fuzz;

// clamps the point to the nearest 16th of a pixel
// Port of: fuzz/FuzzPolyUtils.cpp#L19-L25 (chrome/m156)
fn sanitize_point(input: Point) -> Point {
    Point::new(
        scalar_round_to_scalar(16.0 * input.x) * 0.0625,
        scalar_round_to_scalar(16.0 * input.y) * 0.0625,
    )
}

// Port of: fuzz/FuzzPolyUtils.cpp#L27-L62 (chrome/m156)
def_fuzz!(PolyUtils, fuzz_PolyUtils, |fuzz| {
    let count: i32 = fuzz.next_range(0, 512);
    #[allow(clippy::cast_sign_loss)] // `count` is in [0, 512]
    let count = count as usize;
    let mut polygon = vec![Point::default(); count];
    for p in &mut polygon {
        let (x, y): (scalar, scalar) = (fuzz.next(), fuzz.next());
        *p = sanitize_point(Point::new(x, y));
    }
    let bounds = Rect::bounds_or_empty(&polygon);

    let _ = get_polygon_winding(&polygon);
    let is_convex = is_convex_polygon(&polygon);
    let is_simple = is_simple_polygon(&polygon);

    let mut output: Vec<Point> = Vec::new();
    if is_convex {
        let inset: scalar = fuzz.next();
        let _ = inset_convex_polygon(&polygon, inset, &mut output);
    }

    if is_simple {
        // Limit this to prevent timeouts.
        // This should be fine, as this is roughly the range we expect from the shadow algorithm.
        // `nextRange(&offset, -1000, 1000)` has int bounds: the clamping template, not the
        // float overload.
        let offset: scalar = fuzz.next_range(-1000.0, 1000.0);
        let _ = offset_simple_polygon(&polygon, &bounds, offset, &mut output, None);

        let index_map: Vec<u16> = fuzz.next_n(count);
        let mut output_indices: Vec<u16> = Vec::new();
        let _ = triangulate_simple_polygon(&polygon, &index_map, &mut output_indices);
    }
});
