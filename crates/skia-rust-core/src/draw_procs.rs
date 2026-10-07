// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkDrawProcs.h, src/core/SkDraw.cpp (`DrawTreatAAStrokeAsHairline`)

//! `SkDrawProcs.h`: when a thin stroke can be drawn as a modulated hairline.

use crate::floating_point::float_midpoint;
use crate::matrix::Matrix;
use crate::paint::{Paint, Style};
use crate::point::Vector;
use crate::scalar::{SCALAR_1, scalar};

/// `fast_len`: the length of `vec`, approximated.
// Port of: src/core/SkDraw.cpp#L807-L815 (chrome/m156)
fn fast_len(vec: Vector) -> f32 {
    let mut x = vec.x.abs();
    let mut y = vec.y.abs();
    if x < y {
        std::mem::swap(&mut x, &mut y);
    }
    x + (y / 2.0)
}

/// If an anti-aliased stroke of `stroke_width` under `matrix` is at most one device pixel wide,
/// returns the coverage to modulate a hairline with (`DrawTreatAAStrokeAsHairline`).
///
/// `stroke_width` must be positive.
// Port of: src/core/SkDraw.cpp#L817-L838 (chrome/m156)
#[doc(alias = "DrawTreatAAStrokeAsHairline")]
#[must_use]
pub fn draw_treat_aa_stroke_as_hairline(stroke_width: scalar, matrix: &Matrix) -> Option<scalar> {
    debug_assert!(stroke_width > 0.0);
    // We need to try to fake a thick-stroke with a modulated hairline.

    if matrix.has_perspective() {
        return None;
    }

    let src = [
        Vector::new(stroke_width, 0.0),
        Vector::new(0.0, stroke_width),
    ];
    let mut dst = [Vector::default(); 2];
    matrix.map_vectors(&mut dst, &src);
    let len0 = fast_len(dst[0]);
    let len1 = fast_len(dst[1]);
    if len0 <= SCALAR_1 && len1 <= SCALAR_1 {
        return Some(float_midpoint(len0, len1));
    }
    None
}

/// If `paint` strokes and its stroke width under `matrix` is at most one pixel, returns the
/// coverage to simulate it with a hairline (`DrawTreatAsHairline`).
// Port of: src/core/SkDrawProcs.h#L24-L40 (chrome/m156)
#[doc(alias = "DrawTreatAsHairline")]
#[must_use]
pub fn draw_treat_as_hairline(paint: &Paint, matrix: &Matrix) -> Option<scalar> {
    if Style::Stroke != paint.style() {
        return None;
    }

    let stroke_width = paint.stroke_width();
    if 0.0 == stroke_width {
        return Some(SCALAR_1);
    }

    if !paint.is_anti_alias() {
        return None;
    }

    draw_treat_aa_stroke_as_hairline(stroke_width, matrix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::float_cmp)] // exact results of a few multiplications
    fn hairline_coverage() {
        let mut p = Paint::default();
        p.set_style(Style::Stroke);
        p.set_stroke_width(0.0);
        assert_eq!(
            draw_treat_as_hairline(&p, &Matrix::new_identity()),
            Some(1.0)
        );
        p.set_stroke_width(0.5);
        assert_eq!(draw_treat_as_hairline(&p, &Matrix::new_identity()), None); // not AA
        p.set_anti_alias(true);
        assert_eq!(
            draw_treat_as_hairline(&p, &Matrix::new_identity()),
            Some(0.5)
        );
        p.set_stroke_width(1.5);
        assert_eq!(draw_treat_as_hairline(&p, &Matrix::new_identity()), None);
        assert_eq!(
            draw_treat_as_hairline(&p, &Matrix::scale((0.5, 0.5))),
            Some(0.75)
        );
        p.set_style(Style::Fill);
        assert_eq!(draw_treat_as_hairline(&p, &Matrix::new_identity()), None);
    }
}
