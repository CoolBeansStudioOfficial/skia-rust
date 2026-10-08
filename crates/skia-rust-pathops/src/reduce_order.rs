// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkReduceOrder.h, src/pathops/SkReduceOrder.cpp

//! Reduces the order of a line, quad or cubic whose control points make it degenerate
//! (`SkReduceOrder`).

use skia_rust_core::path::Verb;
use skia_rust_core::point::Point;

use crate::cubic::DCubic;
use crate::line::DLine;
use crate::point::DPoint;
use crate::quad::DQuad;
use crate::types::{
    almost_equal_ulps, almost_equal_ulps_pin, approximately_equal, approximately_equal_half,
    approximately_zero, points_to_verb, std_max,
};

/// `SkReduceOrder::Quadratics`: whether a cubic may reduce to a quad.
#[doc(alias = "SkReduceOrder::Quadratics")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Quadratics {
    /// `kNo_Quadratics`.
    No,
    /// `kAllow_Quadratics`.
    Allow,
}

/// `SkReduceOrder`: the reduced line, quad or cubic of the last `reduce_*` call.
// Port of: src/pathops/SkReduceOrder.h (chrome/m156)
#[doc(alias = "SkReduceOrder")]
#[derive(Copy, Clone, Debug, Default)]
pub struct ReduceOrder {
    /// `SkDLine fLine`.
    pub line: DLine,
    /// `SkDQuad fQuad`.
    pub quad: DQuad,
    /// `SkDCubic fCubic`.
    pub cubic: DCubic,
}

/// Port of `coincident_line` (quad).
// Port of: src/pathops/SkReduceOrder.cpp (chrome/m156)
fn coincident_line_quad(quad: &DQuad, reduction: &mut DQuad) -> usize {
    reduction.pts[0] = quad.pts[0];
    reduction.pts[1] = quad.pts[0];
    1
}

/// Port of `reductionLineCount` for a quad reduction.
// Port of: src/pathops/SkReduceOrder.cpp (chrome/m156)
fn reduction_line_count_quad(reduction: &DQuad) -> usize {
    1 + usize::from(!reduction.pts[0].approximately_equal(reduction.pts[1]))
}

/// Port of `vertical_line` / `horizontal_line` (quad).
// Port of: src/pathops/SkReduceOrder.cpp (chrome/m156)
fn line_quad(quad: &DQuad, reduction: &mut DQuad) -> usize {
    reduction.pts[0] = quad.pts[0];
    reduction.pts[1] = quad.pts[2];
    reduction_line_count_quad(reduction)
}

/// Port of `check_linear` (quad).
// Port of: src/pathops/SkReduceOrder.cpp (chrome/m156)
fn check_linear_quad(quad: &DQuad, reduction: &mut DQuad) -> usize {
    if !quad.is_linear(0, 2) {
        return 0;
    }
    reduction.pts[0] = quad.pts[0];
    reduction.pts[1] = quad.pts[2];
    reduction_line_count_quad(reduction)
}

/// Port of `coincident_line` (cubic).
// Port of: src/pathops/SkReduceOrder.cpp (chrome/m156)
fn coincident_line_cubic(cubic: &DCubic, reduction: &mut DCubic) -> usize {
    reduction.pts[0] = cubic.pts[0];
    reduction.pts[1] = cubic.pts[0];
    1
}

/// Port of `reductionLineCount` for a cubic reduction.
// Port of: src/pathops/SkReduceOrder.cpp (chrome/m156)
fn reduction_line_count_cubic(reduction: &DCubic) -> usize {
    1 + usize::from(!reduction.pts[0].approximately_equal(reduction.pts[1]))
}

/// Port of `vertical_line` / `horizontal_line` (cubic).
// Port of: src/pathops/SkReduceOrder.cpp (chrome/m156)
fn line_cubic(cubic: &DCubic, reduction: &mut DCubic) -> usize {
    reduction.pts[0] = cubic.pts[0];
    reduction.pts[1] = cubic.pts[3];
    reduction_line_count_cubic(reduction)
}

/// Port of `check_quadratic`: a cubic that is a quad in disguise reduces to three points.
// Port of: src/pathops/SkReduceOrder.cpp#L132-L156 (chrome/m156)
#[allow(clippy::similar_names)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
fn check_quadratic(cubic: &DCubic, reduction: &mut DCubic) -> usize {
    let p = &cubic.pts;
    let dx10 = p[1].x - p[0].x;
    let dx23 = p[2].x - p[3].x;
    let mid_x = p[0].x + dx10 * 3.0 / 2.0;
    let side_ax = mid_x - p[3].x;
    let side_bx = dx23 * 3.0 / 2.0;
    let x_fails = if approximately_zero(side_ax) {
        !approximately_equal(side_ax, side_bx)
    } else {
        !almost_equal_ulps_pin(side_ax, side_bx)
    };
    if x_fails {
        return 0;
    }
    let dy10 = p[1].y - p[0].y;
    let dy23 = p[2].y - p[3].y;
    let mid_y = p[0].y + dy10 * 3.0 / 2.0;
    let side_ay = mid_y - p[3].y;
    let side_by = dy23 * 3.0 / 2.0;
    let y_fails = if approximately_zero(side_ay) {
        !approximately_equal(side_ay, side_by)
    } else {
        !almost_equal_ulps_pin(side_ay, side_by)
    };
    if y_fails {
        return 0;
    }
    reduction.pts[0] = cubic.pts[0];
    reduction.pts[1] = DPoint::new(mid_x, mid_y);
    reduction.pts[2] = cubic.pts[3];
    3
}

/// Port of `check_linear` (cubic).
// Port of: src/pathops/SkReduceOrder.cpp (chrome/m156)
fn check_linear_cubic(cubic: &DCubic, reduction: &mut DCubic) -> usize {
    if !cubic.is_linear(0, 3) {
        return 0;
    }
    reduction.pts[0] = cubic.pts[0];
    reduction.pts[1] = cubic.pts[3];
    reduction_line_count_cubic(reduction)
}

impl ReduceOrder {
    /// `int reduce(const SkDLine& line)`.
    // Port of: src/pathops/SkReduceOrder.cpp (chrome/m156)
    pub fn reduce_line(&mut self, line: &DLine) -> usize {
        self.line.pts[0] = line[0];
        let different = usize::from(line[0] != line[1]);
        self.line.pts[1] = line[different];
        1 + different
    }

    /// `int reduce(const SkDQuad& quad)`.
    // Port of: src/pathops/SkReduceOrder.cpp (chrome/m156)
    pub fn reduce_quad(&mut self, quad: &DQuad) -> usize {
        let mut min_x = 0usize;
        let mut max_x = 0usize;
        let mut min_y = 0usize;
        let mut max_y = 0usize;
        let mut min_x_set = 0u32;
        let mut min_y_set = 0u32;
        for index in 1..3 {
            if quad.pts[min_x].x > quad.pts[index].x {
                min_x = index;
            }
            if quad.pts[min_y].y > quad.pts[index].y {
                min_y = index;
            }
            if quad.pts[max_x].x < quad.pts[index].x {
                max_x = index;
            }
            if quad.pts[max_y].y < quad.pts[index].y {
                max_y = index;
            }
        }
        for index in 0..3 {
            if almost_equal_ulps(quad.pts[index].x, quad.pts[min_x].x) {
                min_x_set |= 1 << index;
            }
            if almost_equal_ulps(quad.pts[index].y, quad.pts[min_y].y) {
                min_y_set |= 1 << index;
            }
        }
        if (min_x_set & 0x05) == 0x5 && (min_y_set & 0x05) == 0x5 {
            // test for degenerate
            return coincident_line_quad(quad, &mut self.quad);
        }
        if min_x_set == 0x7 {
            // test for vertical line
            return line_quad(quad, &mut self.quad);
        }
        if min_y_set == 0x7 {
            // test for horizontal line
            return line_quad(quad, &mut self.quad);
        }
        let result = check_linear_quad(quad, &mut self.quad);
        if result != 0 {
            return result;
        }
        self.quad = *quad;
        3
    }

    /// `int reduce(const SkDCubic& cubic, Quadratics allowQuadratics)`.
    // Port of: src/pathops/SkReduceOrder.cpp (chrome/m156)
    pub fn reduce_cubic(&mut self, cubic: &DCubic, allow_quadratics: Quadratics) -> usize {
        let mut min_x = 0usize;
        let mut max_x = 0usize;
        let mut min_y = 0usize;
        let mut max_y = 0usize;
        let mut min_x_set = 0u32;
        let mut min_y_set = 0u32;
        for index in 1..4 {
            if cubic.pts[min_x].x > cubic.pts[index].x {
                min_x = index;
            }
            if cubic.pts[min_y].y > cubic.pts[index].y {
                min_y = index;
            }
            if cubic.pts[max_x].x < cubic.pts[index].x {
                max_x = index;
            }
            if cubic.pts[max_y].y < cubic.pts[index].y {
                max_y = index;
            }
        }
        for index in 0..4 {
            let cx = cubic.pts[index].x;
            let cy = cubic.pts[index].y;
            let denom = std_max(
                cx.abs(),
                std_max(
                    cy.abs(),
                    std_max(cubic.pts[min_x].x.abs(), cubic.pts[min_y].y.abs()),
                ),
            );
            if denom == 0.0 {
                min_x_set |= 1 << index;
                min_y_set |= 1 << index;
                continue;
            }
            let inv = 1.0 / denom;
            if approximately_equal_half(cx * inv, cubic.pts[min_x].x * inv) {
                min_x_set |= 1 << index;
            }
            if approximately_equal_half(cy * inv, cubic.pts[min_y].y * inv) {
                min_y_set |= 1 << index;
            }
        }
        if min_x_set == 0xF {
            // test for vertical line
            if min_y_set == 0xF {
                // return 1 if all four are coincident
                return coincident_line_cubic(cubic, &mut self.cubic);
            }
            return line_cubic(cubic, &mut self.cubic);
        }
        if min_y_set == 0xF {
            // test for horizontal line
            return line_cubic(cubic, &mut self.cubic);
        }
        let result = check_linear_cubic(cubic, &mut self.cubic);
        if result != 0 {
            return result;
        }
        if allow_quadratics == Quadratics::Allow {
            let result = check_quadratic(cubic, &mut self.cubic);
            if result != 0 {
                return result;
            }
        }
        self.cubic = *cubic;
        4
    }

    /// `static SkPath::Verb Quad(const SkPoint pts[3], SkPoint* reducePts)`.
    // Port of: src/pathops/SkReduceOrder.cpp (chrome/m156)
    #[doc(alias = "Quad")]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::needless_range_loop
    )] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn quad_verb(pts: [Point; 3], reduce_pts: &mut [Point]) -> Verb {
        let mut quad = DQuad::default();
        quad.set(pts);
        let mut reducer = Self::default();
        let order = reducer.reduce_quad(&quad);
        if order == 2 {
            // quad became line
            for index in 0..order {
                reduce_pts[index] = reducer.quad.pts[index].as_sk_point();
            }
        }
        points_to_verb(order as i32 - 1)
    }

    /// `static SkPath::Verb Conic(const SkConic& conic, SkPoint* reducePts)`: `pts` and `weight`
    /// of the conic.
    // Port of: src/pathops/SkReduceOrder.cpp (chrome/m156)
    #[doc(alias = "Conic")]
    #[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn conic_verb(pts: [Point; 3], weight: f32, reduce_pts: &mut [Point]) -> Verb {
        let verb = Self::quad_verb(pts, reduce_pts);
        if verb as i32 > Verb::Line as i32 && weight == 1.0 {
            return Verb::Quad;
        }
        if verb == Verb::Quad {
            Verb::Conic
        } else {
            verb
        }
    }

    /// `static SkPath::Verb Cubic(const SkPoint a[4], SkPoint* reducePts)`.
    // Port of: src/pathops/SkReduceOrder.cpp (chrome/m156)
    #[doc(alias = "Cubic")]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::needless_range_loop
    )] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn cubic_verb(a: [Point; 4], reduce_pts: &mut [Point]) -> Verb {
        if DPoint::approximately_equal_points(a[0], a[1])
            && DPoint::approximately_equal_points(a[0], a[2])
            && DPoint::approximately_equal_points(a[0], a[3])
        {
            reduce_pts[0] = a[0];
            return Verb::Move;
        }
        let mut cubic = DCubic::default();
        cubic.set(a);
        let mut reducer = Self::default();
        let order = reducer.reduce_cubic(&cubic, Quadratics::Allow);
        if order == 2 || order == 3 {
            // cubic became line or quad
            for index in 0..order {
                reduce_pts[index] = reducer.cubic.pts[index].as_sk_point();
            }
        }
        points_to_verb(order as i32 - 1)
    }
}
