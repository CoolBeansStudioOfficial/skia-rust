// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkGlyphTest.cpp (chrome/m156), the cases that need only the glyph rect

#![cfg(test)]

use skia_rust_core::glyph::GlyphRect;
use skia_rust_core::glyph::skglyph::{empty_rect, full_rect, rect_intersection, rect_union};
use skia_rust_core::rect::Rect;

use crate::{def_test, reporter_assert};

/// Converts a small integer test coordinate to a scalar. The values are well inside f32's exact
/// integer range.
#[allow(clippy::cast_precision_loss)] // test coordinates are small integers, exact in f32
fn sc(v: i32) -> f32 {
    v as f32
}

// Port of: tests/SkGlyphTest.cpp#L36-L64 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)] // exact comparisons, as in the C++ test
    SkGlyphRectBasic,
    |reporter| {
        let r = GlyphRect::new(1.0, 1.0, 10.0, 10.0);
        reporter_assert!(reporter, !r.is_empty());

        let mut a = rect_union(r, empty_rect());
        reporter_assert!(reporter, a.rect() == Rect::from_ltrb(1.0, 1.0, 10.0, 10.0));
        let width_height = a.width_height();
        reporter_assert!(reporter, width_height.x == 9.0 && width_height.y == 9.0);

        a = rect_intersection(r, full_rect());
        reporter_assert!(reporter, a.rect() == Rect::from_ltrb(1.0, 1.0, 10.0, 10.0));

        let mut acc = full_rect();
        for x in -10..10 {
            for y in -10..10 {
                acc = rect_intersection(acc, GlyphRect::new(sc(x), sc(y), sc(x + 20), sc(y + 20)));
            }
        }
        reporter_assert!(
            reporter,
            acc.rect() == Rect::from_ltrb(9.0, 9.0, 10.0, 10.0)
        );

        acc = empty_rect();
        for x in -10..10 {
            for y in -10..10 {
                acc = rect_union(acc, GlyphRect::new(sc(x), sc(y), sc(x + 20), sc(y + 20)));
            }
        }
        reporter_assert!(
            reporter,
            acc.rect() == Rect::from_ltrb(-10.0, -10.0, 29.0, 29.0)
        );
    }
);
