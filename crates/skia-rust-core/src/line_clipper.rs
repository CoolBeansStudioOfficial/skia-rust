// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkLineClipper.h, src/core/SkLineClipper.cpp

//! Clipping line segments against rectangles (`SkLineClipper.h`).

use crate::floating_point::float_midpoint;
use crate::point::Point;
use crate::rect::Rect;
use crate::scalar::{Scalar, scalar};

/// `SkLineClipper::kMaxPoints`.
pub const MAX_POINTS: usize = 4;
/// `SkLineClipper::kMaxClippedLineSegments`.
pub const MAX_CLIPPED_LINE_SEGMENTS: usize = MAX_POINTS - 1;

// Port of: src/core/SkLineClipper.cpp#L19-L33 (chrome/m156)
fn pin_unsorted<T: PartialOrd + Copy>(value: T, limit0: T, limit1: T) -> T {
    let (mut limit0, mut limit1) = (limit0, limit1);
    if limit1 < limit0 {
        std::mem::swap(&mut limit0, &mut limit1);
    }
    // now the limits are sorted
    let mut value = value;
    if value < limit0 {
        value = limit0;
    } else if value > limit1 {
        value = limit1;
    }
    value
}

// return X coordinate of intersection with horizontal line at Y
// Port of: src/core/SkLineClipper.cpp#L36-L54 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // (float) of the double result, as in C++
fn sect_with_horizontal(src: &[Point; 2], y: scalar) -> scalar {
    let dy = src[1].y - src[0].y;
    if dy.nearly_zero(None) {
        float_midpoint(src[0].x, src[1].x)
    } else {
        // need the extra precision so we don't compute a value that exceeds
        // our original limits
        let x0 = f64::from(src[0].x);
        let y0 = f64::from(src[0].y);
        let x1 = f64::from(src[1].x);
        let y1 = f64::from(src[1].y);
        let result = x0 + (f64::from(y) - y0) * (x1 - x0) / (y1 - y0);
        // The computed X value might still exceed [X0..X1] due to quantum flux
        // when the doubles were added and subtracted, so we have to pin the
        // answer :(
        pin_unsorted(result, x0, x1) as f32
    }
}

// return Y coordinate of intersection with vertical line at X
// Port of: src/core/SkLineClipper.cpp#L57-L72 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // (float) of the double result, as in C++
fn sect_with_vertical(src: &[Point; 2], x: scalar) -> scalar {
    let dx = src[1].x - src[0].x;
    if dx.nearly_zero(None) {
        float_midpoint(src[0].y, src[1].y)
    } else {
        // need the extra precision so we don't compute a value that exceeds
        // our original limits
        let x0 = f64::from(src[0].x);
        let y0 = f64::from(src[0].y);
        let x1 = f64::from(src[1].x);
        let y1 = f64::from(src[1].y);
        let result = y0 + (f64::from(x) - x0) * (y1 - y0) / (x1 - x0);
        result as f32
    }
}

// Port of: src/core/SkLineClipper.cpp#L74-L81 (chrome/m156)
fn sect_clamp_with_vertical(src: &[Point; 2], x: scalar) -> scalar {
    let y = sect_with_vertical(src, x);
    // Our caller expects y to be between src[0].fY and src[1].fY (unsorted), but due to the
    // numerics of floats/doubles, we might have computed a value slightly outside of that,
    // so we have to manually clamp afterwards.
    // See skbug.com/40038736
    pin_unsorted(y, src[0].y, src[1].y)
}

// Port of: src/core/SkLineClipper.cpp#L85-L87 (chrome/m156)
fn nested_lt(a: scalar, b: scalar, dim: scalar) -> bool {
    a <= b && (a < b || dim > 0.0)
}

// returns true if outer contains inner, even if inner is empty.
// note: outer.contains(inner) always returns false if inner is empty.
// Port of: src/core/SkLineClipper.cpp#L91-L95 (chrome/m156)
fn contains_no_empty_check(outer: &Rect, inner: &Rect) -> bool {
    outer.left <= inner.left
        && outer.top <= inner.top
        && outer.right >= inner.right
        && outer.bottom >= inner.bottom
}

/// Intersects the segment `src` with `clip`. Returns the non-empty resulting segment, if any.
///
/// [`clip_line`] is specialized for scan-conversion, as it adds vertical segments on the sides
/// to show where the line extended beyond the left or right sides. This does not.
// Port of: src/core/SkLineClipper.cpp#L97-L165 (chrome/m156)
#[doc(alias = "IntersectLine")]
#[must_use]
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
pub fn intersect_line(src: &[Point; 2], clip: &Rect) -> Option<[Point; 2]> {
    let mut bounds = Rect::default();
    bounds.set_bounds2(src[0], src[1]);
    if contains_no_empty_check(clip, &bounds) {
        return Some(*src);
    }
    // check for no overlap, and only permit coincident edges if the line
    // and the edge are colinear
    if nested_lt(bounds.right, clip.left, bounds.width())
        || nested_lt(clip.right, bounds.left, bounds.width())
        || nested_lt(bounds.bottom, clip.top, bounds.height())
        || nested_lt(clip.bottom, bounds.top, bounds.height())
    {
        return None;
    }

    let (mut index0, mut index1) = if src[0].y < src[1].y { (0, 1) } else { (1, 0) };

    let mut tmp = *src;

    // now compute Y intersections
    if tmp[index0].y < clip.top {
        tmp[index0].set(sect_with_horizontal(src, clip.top), clip.top);
    }
    if tmp[index1].y > clip.bottom {
        tmp[index1].set(sect_with_horizontal(src, clip.bottom), clip.bottom);
    }

    if tmp[0].x < tmp[1].x {
        index0 = 0;
        index1 = 1;
    } else {
        index0 = 1;
        index1 = 0;
    }

    // check for quick-reject in X again, now that we may have been chopped
    if (tmp[index1].x <= clip.left || tmp[index0].x >= clip.right)
        // usually we will return false, but we don't if the line is vertical and coincident
        // with the clip.
        && (tmp[0].x != tmp[1].x || tmp[0].x < clip.left || tmp[0].x > clip.right)
    {
        return None;
    }

    if tmp[index0].x < clip.left {
        let y = sect_with_vertical(&tmp, clip.left);
        tmp[index0].set(clip.left, y);
    }
    if tmp[index1].x > clip.right {
        let y = sect_with_vertical(&tmp, clip.right);
        tmp[index1].set(clip.right, y);
    }
    #[cfg(debug_assertions)]
    {
        bounds.set_bounds2(tmp[0], tmp[1]);
        debug_assert!(contains_no_empty_check(clip, &bounds));
    }
    Some(tmp)
}

/// Clips the line `pts[0]..pts[1]` against `clip`, ignoring segments that lie completely above
/// or below the clip. Portions to the left or right become vertical segments aligned to the edge
/// of the clip.
///
/// Returns the number of line segments; their end-points are stored sequentially in `lines`
/// (`lines[0]..lines[1]`, `lines[1]..lines[2]`, `lines[2]..lines[3]`).
// Port of: src/core/SkLineClipper.cpp#L180-L283 (chrome/m156)
#[doc(alias = "ClipLine")]
pub fn clip_line(
    pts: &[Point; 2],
    clip: &Rect,
    lines: &mut [Point; MAX_POINTS],
    can_cull_to_the_right: bool,
) -> usize {
    let (mut index0, mut index1) = if pts[0].y < pts[1].y { (0, 1) } else { (1, 0) };

    // Check if we're completely clipped out in Y (above or below

    if pts[index1].y <= clip.top {
        // we're above the clip
        return 0;
    }
    if pts[index0].y >= clip.bottom {
        // we're below the clip
        return 0;
    }

    // Chop in Y to produce a single segment, stored in tmp[0..1]

    let mut tmp = *pts;

    // now compute intersections
    if pts[index0].y < clip.top {
        tmp[index0].set(sect_with_horizontal(pts, clip.top), clip.top);
    }
    if tmp[index1].y > clip.bottom {
        tmp[index1].set(sect_with_horizontal(pts, clip.bottom), clip.bottom);
    }

    // Chop it into 1..3 segments that are wholly within the clip in X.

    // temp storage for up to 3 segments
    let mut result_storage = [Point::default(); MAX_POINTS];
    let result: &[Point];
    let mut line_count = 1;
    let mut reverse;

    if pts[0].x < pts[1].x {
        index0 = 0;
        index1 = 1;
        reverse = false;
    } else {
        index0 = 1;
        index1 = 0;
        reverse = true;
    }

    if tmp[index1].x <= clip.left {
        // wholly to the left
        tmp[0].x = clip.left;
        tmp[1].x = clip.left;
        result = &tmp;
        reverse = false;
    } else if tmp[index0].x >= clip.right {
        // wholly to the right
        if can_cull_to_the_right {
            return 0;
        }
        tmp[0].x = clip.right;
        tmp[1].x = clip.right;
        result = &tmp;
        reverse = false;
    } else {
        let mut r = 0;
        if tmp[index0].x < clip.left {
            result_storage[r].set(clip.left, tmp[index0].y);
            r += 1;
            result_storage[r].set(clip.left, sect_clamp_with_vertical(&tmp, clip.left));
        } else {
            result_storage[r] = tmp[index0];
        }
        r += 1;

        if tmp[index1].x > clip.right {
            result_storage[r].set(clip.right, sect_clamp_with_vertical(&tmp, clip.right));
            r += 1;
            result_storage[r].set(clip.right, tmp[index1].y);
        } else {
            result_storage[r] = tmp[index1];
        }

        line_count = r;
        result = &result_storage;
    }

    // Now copy the results into the caller's lines[] parameter
    if reverse {
        // copy the pts in reverse order to maintain winding order
        for i in 0..=line_count {
            lines[line_count - i] = result[i];
        }
    } else {
        lines[..=line_count].copy_from_slice(&result[..=line_count]);
    }
    line_count
}
