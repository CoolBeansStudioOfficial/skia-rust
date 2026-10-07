// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkStrokerPriv.h, src/core/SkStrokerPriv.cpp

//! `SkStrokerPriv`: the cappers and joiners used by the stroker.

use crate::geometry::{Conic, MAX_CONICS_FOR_ARC};
use crate::matrix::Matrix;
use crate::paint::{Cap, Join};
use crate::path_builder::PathBuilder;
use crate::path_types::PathDirection;
use crate::point::{Point, Vector, point_priv};
use crate::scalar::{SCALAR_1, SCALAR_ROOT_2_OVER_2, SCALAR_SQRT2, Scalar, scalar, scalar_sqrt};

/// `CUBIC_ARC_FACTOR`.
// Port of: src/core/SkStrokerPriv.h#L21 (chrome/m156)
pub const CUBIC_ARC_FACTOR: scalar = (SCALAR_SQRT2 - SCALAR_1) * 4.0 / 3.0;

/// Draws the cap at one end of a contour (`SkStrokerPriv::CapProc`): `pivot` is the end point on
/// the original path, `normal` the offset normal, `stop` the point the cap must end at.
// Port of: src/core/SkStrokerPriv.h#L28-L32 (chrome/m156)
#[doc(alias = "SkStrokerPriv::CapProc")]
pub type CapProc =
    fn(path: &mut PathBuilder, pivot: Point, normal: Vector, stop: Point, extend_last_pt: bool);

/// Draws the join between two segments (`SkStrokerPriv::JoinProc`).
// Port of: src/core/SkStrokerPriv.h#L34-L39 (chrome/m156)
#[doc(alias = "SkStrokerPriv::JoinProc")]
pub type JoinProc = fn(
    outer: &mut PathBuilder,
    inner: &mut PathBuilder,
    before_unit_normal: Vector,
    pivot: Point,
    after_unit_normal: Vector,
    radius: scalar,
    inv_miter_limit: scalar,
    prev_is_line: bool,
    curr_is_line: bool,
);

// Port of: src/core/SkStrokerPriv.cpp#L18-L21 (chrome/m156)
fn butt_capper(
    sink: &mut PathBuilder,
    _pivot: Point,
    _normal: Vector,
    stop: Point,
    _extend_last_pt: bool,
) {
    sink.line_to((stop.x, stop.y));
}

// Port of: src/core/SkStrokerPriv.cpp#L23-L32 (chrome/m156)
fn round_capper(
    sink: &mut PathBuilder,
    pivot: Point,
    normal: Vector,
    stop: Point,
    _extend_last_pt: bool,
) {
    let parallel = point_priv::rotate_cw(normal);

    let projected_center = pivot + parallel;

    sink.conic_to(
        projected_center + normal,
        projected_center,
        SCALAR_ROOT_2_OVER_2,
    );
    sink.conic_to(projected_center - normal, stop, SCALAR_ROOT_2_OVER_2);
}

// Port of: src/core/SkStrokerPriv.cpp#L34-L47 (chrome/m156)
fn square_capper(
    sink: &mut PathBuilder,
    pivot: Point,
    normal: Vector,
    stop: Point,
    extend_last_pt: bool,
) {
    let parallel = point_priv::rotate_cw(normal);

    if extend_last_pt {
        sink.set_last_point(pivot + normal + parallel);
        sink.line_to(pivot - normal + parallel);
    } else {
        sink.line_to(pivot + normal + parallel);
        sink.line_to(pivot - normal + parallel);
        sink.line_to(stop);
    }
}

/////////////////////////////////////////////////////////////////////////////

// Port of: src/core/SkStrokerPriv.cpp#L51-L53 (chrome/m156)
fn is_clockwise(before: Vector, after: Vector) -> bool {
    before.x * after.y > before.y * after.x
}

// Port of: src/core/SkStrokerPriv.cpp#L55-L60 (chrome/m156)
#[derive(Copy, Clone, PartialEq, Eq)]
enum AngleType {
    Nearly180,
    Sharp,
    Shallow,
    NearlyLine,
}

// Port of: src/core/SkStrokerPriv.cpp#L62-L71 (chrome/m156)
fn dot_2_angle_type(dot: scalar) -> AngleType {
    // need more precise fixed normalization
    //  SkASSERT(SkScalarAbs(dot) <= SK_Scalar1 + SK_ScalarNearlyZero);

    if dot >= 0.0 {
        // shallow or line
        if (SCALAR_1 - dot).nearly_zero(None) {
            AngleType::NearlyLine
        } else {
            AngleType::Shallow
        }
    } else if (SCALAR_1 + dot).nearly_zero(None) {
        // sharp or 180
        AngleType::Nearly180
    } else {
        AngleType::Sharp
    }
}

// Port of: src/core/SkStrokerPriv.cpp#L73-L85 (chrome/m156)
fn handle_inner_join(inner: &mut PathBuilder, pivot: Point, after: Vector) {
    // In the degenerate case that the stroke radius is larger than our segments
    // just connecting the two inner segments may "show through" as a funny
    // diagonal. To pseudo-fix this, we go through the pivot point. This adds
    // an extra point/edge, but I can't see a cheap way to know when this is
    // not needed :(
    inner.line_to((pivot.x, pivot.y));

    inner.line_to((pivot.x - after.x, pivot.y - after.y));
}

// Port of: src/core/SkStrokerPriv.cpp#L87-L102 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors SkStrokerPriv::JoinProc
fn blunt_joiner(
    outer: &mut PathBuilder,
    inner: &mut PathBuilder,
    before_unit_normal: Vector,
    pivot: Point,
    after_unit_normal: Vector,
    radius: scalar,
    _inv_miter_limit: scalar,
    _prev_is_line: bool,
    _curr_is_line: bool,
) {
    let (mut outer, mut inner) = (outer, inner);
    let mut after = after_unit_normal.scaled(radius);

    if !is_clockwise(before_unit_normal, after_unit_normal) {
        (outer, inner) = (inner, outer);
        after.negate();
    }

    outer.line_to((pivot.x + after.x, pivot.y + after.y));
    handle_inner_join(inner, pivot, after);
}

// Port of: src/core/SkStrokerPriv.cpp#L104-L138 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors SkStrokerPriv::JoinProc
fn round_joiner(
    outer: &mut PathBuilder,
    inner: &mut PathBuilder,
    before_unit_normal: Vector,
    pivot: Point,
    after_unit_normal: Vector,
    radius: scalar,
    _inv_miter_limit: scalar,
    _prev_is_line: bool,
    _curr_is_line: bool,
) {
    let (mut outer, mut inner) = (outer, inner);
    let dot_prod = Point::dot_product(before_unit_normal, after_unit_normal);
    let angle_type = dot_2_angle_type(dot_prod);

    if angle_type == AngleType::NearlyLine {
        return;
    }

    let mut before = before_unit_normal;
    let mut after = after_unit_normal;
    let mut dir = PathDirection::CW;

    if !is_clockwise(before, after) {
        (outer, inner) = (inner, outer);
        before.negate();
        after.negate();
        dir = PathDirection::CCW;
    }

    let mut matrix = Matrix::scale((radius, radius));
    matrix.post_translate((pivot.x, pivot.y));
    let mut conics = [Conic::default(); MAX_CONICS_FOR_ARC];
    let count = Conic::build_unit_arc(before, after, dir, Some(&matrix), &mut conics);
    if count > 0 {
        for conic in &conics[..count] {
            outer.conic_to(conic.pts[1], conic.pts[2], conic.w);
        }
        after.scale(radius);
        handle_inner_join(inner, pivot, after);
    }
}

// Port of: src/core/SkStrokerPriv.cpp#L148 (chrome/m156)
#[allow(clippy::excessive_precision, clippy::approx_constant)] // verbatim C++ literal
const ONE_OVER_SQRT2: scalar = 0.707_106_781;

// Port of: src/core/SkStrokerPriv.cpp#L142-L221 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors SkStrokerPriv::JoinProc
#[allow(clippy::float_cmp)] // exact comparison with 0, as in Skia
#[allow(clippy::manual_midpoint)] // mirrors the C++ `(1 + dot) / 2.f` arithmetic
fn miter_joiner(
    outer: &mut PathBuilder,
    inner: &mut PathBuilder,
    before_unit_normal: Vector,
    pivot: Point,
    after_unit_normal: Vector,
    radius: scalar,
    inv_miter_limit: scalar,
    prev_is_line: bool,
    mut curr_is_line: bool,
) {
    let (mut outer, mut inner) = (outer, inner);
    // negate the dot since we're using normals instead of tangents
    let dot_prod = Point::dot_product(before_unit_normal, after_unit_normal);
    let angle_type = dot_2_angle_type(dot_prod);
    let mut before = before_unit_normal;
    let mut after = after_unit_normal;

    if angle_type == AngleType::NearlyLine {
        return;
    }

    // `break 'blunt` is C++'s `goto DO_BLUNT`.
    'blunt: {
        if angle_type == AngleType::Nearly180 {
            curr_is_line = false;
            break 'blunt;
        }

        let ccw = !is_clockwise(before, after);
        if ccw {
            (outer, inner) = (inner, outer);
            before.negate();
            after.negate();
        }

        // Before we enter the world of square-roots and divides,
        // check if we're trying to join an upright right angle
        // (common case for stroking rectangles). If so, special case
        // that (for speed an accuracy).
        // Note: we only need to check one normal if dot==0
        let mid = if 0.0 == dot_prod && inv_miter_limit <= ONE_OVER_SQRT2 {
            // goto DO_MITER
            (before + after) * radius
        } else {
            // midLength = radius / sinHalfAngle
            // if (midLength > miterLimit * radius) abort
            // if (radius / sinHalf > miterLimit * radius) abort
            // if (1 / sinHalf > miterLimit) abort
            // if (1 / miterLimit > sinHalf) abort
            // My dotProd is opposite sign, since it is built from normals and not tangents
            // hence 1 + dot instead of 1 - dot in the formula
            let sin_half_angle = scalar_sqrt((SCALAR_1 + dot_prod) / 2.0);
            if sin_half_angle < inv_miter_limit {
                curr_is_line = false;
                break 'blunt;
            }

            // choose the most accurate way to form the initial mid-vector
            let mut mid = if angle_type == AngleType::Sharp {
                let mut mid = Point::new(after.y - before.y, before.x - after.x);
                if ccw {
                    mid.negate();
                }
                mid
            } else {
                before + after
            };

            let _ = mid.set_length(radius / sin_half_angle);
            mid
        };

        // DO_MITER:
        if prev_is_line {
            outer.set_last_point(pivot + mid);
        } else {
            outer.line_to(pivot + mid);
        }
    }

    // DO_BLUNT:
    after.scale(radius);
    if !curr_is_line {
        outer.line_to((pivot.x + after.x, pivot.y + after.y));
    }
    handle_inner_join(inner, pivot, after);
}

/// The cappers and joiners (`SkStrokerPriv`).
// Port of: src/core/SkStrokerPriv.h#L27-L46 (chrome/m156)
#[doc(alias = "SkStrokerPriv")]
#[derive(Debug)]
pub struct StrokerPriv;

impl StrokerPriv {
    /// The capper that draws `cap` (`SkStrokerPriv::CapFactory`).
    // Port of: src/core/SkStrokerPriv.cpp#L225-L232 (chrome/m156)
    #[doc(alias = "CapFactory")]
    #[must_use]
    pub fn cap_factory(cap: Cap) -> CapProc {
        match cap {
            Cap::Butt => butt_capper,
            Cap::Round => round_capper,
            Cap::Square => square_capper,
        }
    }

    /// The joiner that draws `join` (`SkStrokerPriv::JoinFactory`).
    // Port of: src/core/SkStrokerPriv.cpp#L234-L241 (chrome/m156)
    #[doc(alias = "JoinFactory")]
    #[must_use]
    pub fn join_factory(join: Join) -> JoinProc {
        match join {
            Join::Miter => miter_joiner,
            Join::Round => round_joiner,
            Join::Bevel => blunt_joiner,
        }
    }
}
