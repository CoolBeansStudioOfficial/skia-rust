// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsTightBounds.cpp (chrome/m156)
//! Tight bounds of a path (`ComputeTightBounds`): the bounds of the curves' extrema, computed
//! from the op graph when the curves are not well behaved.

use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathVerb;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

use crate::op_edge_builder::EdgeBuilder;
use crate::op_state::OpState;
use crate::rect::Bounds;
use crate::types::between;

/// `between(a, b, c)` on the `SkScalar` coordinates of a point, widened to double as Skia does.
// Port of: src/pathops/SkPathOpsTypes.h (chrome/m156), the `between` used by ComputeTightBounds
fn between_scalar(a: f32, b: f32, c: f32) -> bool {
    between(f64::from(a), f64::from(b), f64::from(c))
}

/// `ComputeTightBounds(const SkPath& path, SkRect* result)`: the tight bounds of `path`, or
/// `None` if the edge builder cannot parse the path.
// Port of: src/pathops/SkPathOpsTightBounds.cpp#L15-L82 (chrome/m156)
#[doc(alias = "ComputeTightBounds")]
#[must_use]
pub fn tight_bounds(path: &Path) -> Option<Rect> {
    // SkRect moveBounds = { SK_ScalarMax, SK_ScalarMax, SK_ScalarMin, SK_ScalarMin };
    let mut move_bounds = Rect::new(f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut well_behaved = true;
    for rec in path.iter() {
        let pts: &[Point] = rec.points();
        match rec.verb() {
            PathVerb::Move => {
                move_bounds.left = move_bounds.left.min(pts[0].x);
                move_bounds.top = move_bounds.top.min(pts[0].y);
                move_bounds.right = move_bounds.right.max(pts[0].x);
                move_bounds.bottom = move_bounds.bottom.max(pts[0].y);
            }
            PathVerb::Quad | PathVerb::Conic if well_behaved => {
                well_behaved &= between_scalar(pts[0].x, pts[1].x, pts[2].x);
                well_behaved &= between_scalar(pts[0].y, pts[1].y, pts[2].y);
            }
            PathVerb::Cubic if well_behaved => {
                well_behaved &= between_scalar(pts[0].x, pts[1].x, pts[3].x);
                well_behaved &= between_scalar(pts[0].y, pts[1].y, pts[3].y);
                well_behaved &= between_scalar(pts[0].x, pts[2].x, pts[3].x);
                well_behaved &= between_scalar(pts[0].y, pts[2].y, pts[3].y);
            }
            _ => {}
        }
    }
    if well_behaved {
        return Some(*path.bounds());
    }
    // turn path into list of segments
    let mut state = OpState::new();
    let mut contour_list = state.contour_head();
    let mut builder = EdgeBuilder::new(path, contour_list);
    if !builder.finish(&mut state) {
        return None;
    }
    if !state.sort_contour_list(&mut contour_list, false, false) {
        return Some(move_bounds);
    }
    let mut current = contour_list;
    let mut bounds: Bounds = state.contour_bounds(current);
    while let Some(next) = state.contour_next(current) {
        current = next;
        bounds.add_bounds(&state.contour_bounds(current));
    }
    let mut result = Rect::new(bounds.left, bounds.top, bounds.right, bounds.bottom);
    if !move_bounds.is_empty() {
        result.join(move_bounds);
    }
    Some(result)
}
