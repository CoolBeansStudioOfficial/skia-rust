// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathRawShapesTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::path::Path;
use skia_rust_core::path_enums::ResolveConvexity;
use skia_rust_core::path_priv;
use skia_rust_core::path_raw::PathRaw;
use skia_rust_core::path_raw_shapes;
use skia_rust_core::path_types::{PathDirection, PathSegmentMask};
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

const DIRECTIONS: [PathDirection; 2] = [PathDirection::CW, PathDirection::CCW];

// Port of: tests/PathRawShapesTest.cpp#L25-L27 (chrome/m156)
fn path_from_raw(raw: &PathRaw<'_>) -> Path {
    Path::raw(
        raw.points(),
        raw.verbs(),
        raw.conics(),
        raw.fill_type(),
        None,
    )
}

// Port of: tests/PathRawShapesTest.cpp#L29-L34 (chrome/m156)
fn span_eq<T: PartialEq>(a: &[T], b: &[T]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).all(|(x, y)| x == y)
}

// Port of: tests/PathRawShapesTest.cpp#L36-L45 (chrome/m156)
fn check_path_is_raw(reporter: &mut Reporter, path: &Path, raw: &PathRaw<'_>) {
    let praw = path_priv::raw(path, ResolveConvexity::No);
    reporter_assert!(reporter, praw.is_some());
    let Some(praw) = praw else { return };

    reporter_assert!(reporter, span_eq(praw.points, raw.points));
    reporter_assert!(reporter, span_eq(praw.verbs, raw.verbs));
    reporter_assert!(reporter, span_eq(praw.conics, raw.conics));
    reporter_assert!(reporter, praw.bounds == raw.bounds);
}

// skia-rust: the C++ shapes derive from `SkPathRaw`; here `shape.raw()` is the `SkPathRaw` view.

// Port of: tests/PathRawShapesTest.cpp#L47-L61 (chrome/m156)
def_test!(pathrawshapes_rect, |reporter| {
    let r = Rect::new(1.0, 2.0, 3.0, 4.0);

    for dir in DIRECTIONS {
        let shape = path_raw_shapes::Rect::new(&r, dir, 0);
        let raw = shape.raw();

        reporter_assert!(reporter, raw.bounds() == r);
        reporter_assert!(reporter, raw.is_known_to_be_convex());
        reporter_assert!(
            reporter,
            raw.segment_masks() == PathSegmentMask::LINE.bits()
        );

        let path = path_from_raw(&raw);

        check_path_is_raw(reporter, &path, &raw);
    }
});

// Port of: tests/PathRawShapesTest.cpp#L63-L77 (chrome/m156)
def_test!(pathrawshapes_oval, |reporter| {
    let r = Rect::new(1.0, 2.0, 3.0, 4.0);

    for dir in DIRECTIONS {
        let shape = path_raw_shapes::Oval::new(&r, dir, 1);
        let raw = shape.raw();

        reporter_assert!(reporter, raw.bounds() == r);
        reporter_assert!(reporter, raw.is_known_to_be_convex());
        reporter_assert!(
            reporter,
            raw.segment_masks() == PathSegmentMask::CONIC.bits()
        );

        let path = Path::oval(r, dir);

        check_path_is_raw(reporter, &path, &raw);
    }
});

// Port of: tests/PathRawShapesTest.cpp#L79-L96 (chrome/m156)
def_test!(pathrawshapes_rrect, |reporter| {
    let r = Rect::new(0.0, 0.0, 4.0, 4.0);
    let rr = RRect::new_rect_xy(r, 1.0, 1.0);

    for dir in DIRECTIONS {
        let shape = path_raw_shapes::RRect::with_dir(&rr, dir);
        let raw = shape.raw();

        reporter_assert!(reporter, raw.bounds() == r);
        reporter_assert!(reporter, raw.is_known_to_be_convex());
        reporter_assert!(
            reporter,
            raw.segment_masks() == (PathSegmentMask::LINE | PathSegmentMask::CONIC).bits()
        );

        let path = Path::rrect(rr, dir);

        check_path_is_raw(reporter, &path, &raw);
    }
});
