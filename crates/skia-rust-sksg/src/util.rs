// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Helpers shared by the sksg modules: the scalar and rect operations that Skia's sksg uses.

use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

/// `sk_float_round2int`: rounds to the nearest integer, halves up (`floor(x + 0.5)`).
// Port of: include/private/base/SkFloatingPoint.h (chrome/m156) (`sk_float_round2int`)
#[must_use]
pub fn float_round_to_int(x: f32) -> i32 {
    #[allow(clippy::cast_possible_truncation)] // the callers keep the value in the int range
    let rounded = (x + 0.5_f32).floor() as i32;
    rounded
}

/// `SkTPin(x, lo, hi)` for scalars.
// Port of: include/private/base/SkTPin.h (chrome/m156) (`SkTPin`)
#[must_use]
pub fn pin(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

/// `SkRect::contains(x, y)`: half-open on the right and bottom edges.
// Port of: include/core/SkRect.h (chrome/m156) (`SkRect::contains`)
#[must_use]
pub fn rect_contains(r: &Rect, p: Point) -> bool {
    p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom
}

/// `SK_MaxS32 >> 1`-scale large rect of `SkRectPriv::MakeLargeS32`: `MakeILarge` as an `SkRect`.
// Port of: src/core/SkRectPriv.h#L22-L28 (chrome/m156) (`MakeILarge`) and #L33-L37 (`MakeLargeS32`)
#[must_use]
pub fn make_large_s32() -> skia_rust_core::rect::Rect {
    // 1 << 29 is exactly representable as a float.
    const LARGE: f32 = 536_870_912.0;
    skia_rust_core::rect::Rect::new(-LARGE, -LARGE, LARGE, LARGE)
}
