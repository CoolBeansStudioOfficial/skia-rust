// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/sparse_strips/Flatten.h, src/gpu/graphite/sparse_strips/Flatten.cpp

//! [`Flatten`]: flattens the curves of a path into a [`Polyline`].
//!
//! This is a translation of the work of the Vello authors:
//! <https://raphlinus.github.io/graphics/curves/2019/12/23/flatten-quadbez.html>,
//! <https://github.com/linebender/vello/blob/main/sparse_strips/vello_common/src/flatten.rs>.
//!
//! The naive approach to flattening a Bezier curve evaluates points at evenly spaced parametric
//! `t` values (e.g., t=0.1, 0.2, ..., 1.0) and recursively subdivides the resulting lines until
//! each segment is within the desired tolerance. However, the rate of change in both arc length
//! and curvature is not evenly distributed across `t`. A uniform step in `t` might span a long,
//! mostly flat distance (needing few segments), or it might whip around a tight loop (needing
//! many).
//!
//! Instead, this algorithm maps the quadratic curve into an "arc-length" or uniform-curvature
//! space by projecting it onto a standard `y = x^2` parabola. By dividing the curve uniformly in
//! this parabolic space, we can determine the parametric steps required to satisfy a given
//! tolerance upfront and without recursion. This allows the exact `t` values to be calculated
//! without data dependencies, increasing ILP and enabling SIMD.
//!
//! 1. `estimate_lines_from_quad`: analyzes the control points and transforms the quadratic into
//!    the parabolic space. It calculates the total "parabolic distance" the curve covers. By
//!    comparing this distance against the tolerance, it determines how many subdivisions are
//!    required to safely approximate the curve.
//!
//! 2. `determine_quad_subdiv_t`: finds where to divide the curve. Using the inverse integral of
//!    the parabolic approximation, the `N` evenly spaced parabolic steps are mapped back into the
//!    curve's parametric space (`t`).
//!
//! 3. Culling, Simplification, and Winding: Curves completely above, below, or to the right of the
//!    viewport are culled, producing no lines. Curves to the left of the viewport are simplified
//!    instead of culled because they may still contribute winding for left-to-right scanline
//!    renderers (sparse strips). Curves that are sufficiently similar to lines are also simplified
//!    instead of subdivided. This simplification avoids subdivision and reduces the number of
//!    tiles generated downstream, while preserving the winding the curve would have otherwise
//!    contributed.
//!
//! 4. Lowering Conics: Conics (rational quadratics) are degree-reduced to standard quadratics
//!    using `AutoConicToQuads` (scalar), or to cubics (SIMD).
//!
//! 5. Lowering Cubics: Cubics are approximated by a series of quadratics. The required number of
//!    segments is estimated by measuring the curve's geometric deviation from a pure quadratic.
//!    The cubic is then sliced into equal parametric intervals, and the resulting quadratics are
//!    subdivided.
//!
//! The casts between the integer and floating point types below mirror the C++ conversions of the
//! ported arithmetic, including the `double` widening of `float` operands.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::suboptimal_flops,
    clippy::excessive_precision,
    // The C++ declares function-local constants mid-function.
    clippy::items_after_statements,
    // `(1 + w) * 0.5f` is the ported arithmetic, not `midpoint`.
    clippy::manual_midpoint
)]

use skia_rust_core::geometry::AutoConicToQuads;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::{Iter, Path, Verb};
use skia_rust_core::point::Point;
use skia_rust_simd::vx::{
    Float2, Float4, Float8, Int4, abs, all, fma, if_then_else, join, length, shuffle, sqrt,
};

use crate::graphite::sparse_strips::polyline::Polyline;
use crate::graphite::sparse_strips::types::FlattenMode;
use crate::tessellate::wangs_formula::nextlog64;

/// `std::max(a, b)`: `(a < b) ? b : a`, which differs from `f32::max` for NaN.
fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// `FlattenParams`.
// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L22-L35 (chrome/m156)
#[derive(Clone, Copy, Debug)]
struct FlattenParams {
    a0: f64,
    da: f64,
    u0: f64,
    /// Normalization factor (1.0 / (u2 - u0)) to scale the final 't' into the [0, 1] range.
    u_scale: f64,
    /// Total integrated curvature/error metric for the segment. Used to compute the required
    /// number of line segments.
    curvature_integral: f64,
}

// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L35-L39 (chrome/m156)
fn approx_parabola_integral(x: f64) -> f64 {
    const K_D: f64 = 0.67;
    const K_D_POW4: f64 = K_D * K_D * K_D * K_D;
    x / (1.0 - K_D + (K_D_POW4 + 0.25 * x * x).sqrt().sqrt())
}

// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L41-L44 (chrome/m156)
fn approx_parabola_inv_integral(x: f64) -> f64 {
    const K_B: f64 = 0.39;
    x * (1.0 - K_B + (K_B * K_B + 0.25 * x * x).sqrt())
}

/// Determining the number of quads from a cubic:
///
/// 1. Error Budgeting (`kCubicAccuracy`): Flattening cubics happens in two steps: Cubic -> Quads,
///    then Quads -> Lines. `kCubicAccuracy` allocates only 10% (`kCubicErrTolerance`) of the
///    total allowed pixel error to the Cubic -> Quads steps, saving 90% for the final lines.
///
/// 2. Wang's Formula and the Vector Error (`errV`): Since we are flattening a cubic to a
///    *quadratic*, we shift the math up one degree and evaluate the third derivative instead:
///    `(3*P2 - P3) - (3*P1 - P0)`. If this evaluates to zero, the second derivative is constant,
///    meaning the curve is a perfect parabola. We use this expression `errV` as a measure of how
///    much the cubic "bulges" away from an ideal quadratic shape.
///
/// 3. Error Quantization and Lookup Table: From Wang's formula, the maximum geometric error of
///    the un-chopped cubic is `||errV|| / (12*sqrt(3))`. By Taylor's theorem, if we chop the curve
///    into n segments, the approximation error shrinks cubically (Base Error / n^3). Thus, to
///    satisfy our desired tolerance, we must find the minimum n where
///    `((||errV|| / (12*sqrt(3)) / n^3) <= kCubicErrTolerance`. To avoid calling sqrt, we square
///    the equation: `(12*sqrt(3))^2 = 432`. Correspondingly, `(n^3)^2 = n^6`, so errDiv scales at
///    `n^6`. To avoid calculating 6th roots, and because we know we can only have an integer
///    number of subdivisions, we precompute a lookup table (`kPowSixLut`) of 6th powers to find
///    the required number of subdivisions.
///
/// 4. Subdivision limit: As a guard against pathological inputs, and to avoid dynamic memory
///    allocation, we bound the maximum number of quads that can be produced by a cubic. If
///    errDiv exceeds the table, the function returns `kMaxQuadsFromCubic`.
// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L89-L130 (chrome/m156)
fn estimate_num_quads_from_cubic(pts: &[Point]) -> u32 {
    const K_WANG_ERR_THRESHOLD: f64 =
        1.0 / (432.0 * Flatten::K_CUBIC_ERR_TOLERANCE * Flatten::K_CUBIC_ERR_TOLERANCE);

    let left_quad_ctrl_x2 = pts[1] * 3.0f32 - pts[0];
    let right_quad_ctrl_x2 = pts[2] * 3.0f32 - pts[3];
    let err_v = right_quad_ctrl_x2 - left_quad_ctrl_x2;
    let err_sq = f64::from(Point::dot_product(err_v, err_v));
    let err_div = err_sq * K_WANG_ERR_THRESHOLD;

    const K_POW_SIX_LUT: [f64; Flatten::K_MAX_QUADS_FROM_CUBIC as usize] = [
        1.0,
        64.0,
        729.0,
        4096.0,
        15625.0,
        46656.0,
        117_649.0,
        262_144.0,
        531_441.0,
        1_000_000.0,
        1_771_561.0,
        2_985_984.0,
        4_826_809.0,
        7_529_536.0,
        11_390_625.0,
        16_777_216.0,
    ];

    for (i, pow_six) in K_POW_SIX_LUT.iter().enumerate() {
        if err_div <= *pow_six {
            return i as u32 + 1;
        }
    }
    Flatten::K_MAX_QUADS_FROM_CUBIC
}

// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L132-L176 (chrome/m156)
fn estimate_lines_from_quad(pts: &[Point], k_sqrt_err_tolerance: f64) -> FlattenParams {
    let d01 = pts[1] - pts[0];
    let d12 = pts[2] - pts[1];
    let dd = d01 - d12;

    let cross = f64::from(Point::cross_product(pts[2] - pts[0], dd));
    if cross.abs() < Flatten::K_EPSILON_D {
        return FlattenParams {
            a0: 0.0,
            da: 0.0,
            u0: 0.0,
            u_scale: 1.0,
            curvature_integral: 0.0,
        };
    }

    let x0 = f64::from(Point::dot_product(d01, dd)) / cross;
    let x2 = f64::from(Point::dot_product(d12, dd)) / cross;
    let scale = (cross / (f64::from(dd.length()) * (x2 - x0))).abs();

    let a0 = approx_parabola_integral(x0);
    let a2 = approx_parabola_integral(x2);
    let da = a2 - a0;

    let mut val = 0.0f64;
    if scale.is_finite() {
        let abs_da = da.abs();
        let sqrt_scale = scale.sqrt();
        if x0.is_sign_negative() == x2.is_sign_negative() {
            val = abs_da * sqrt_scale;
        } else {
            // Handle cusps by computing the integral over the shortest cusp-free section.
            let x_min = k_sqrt_err_tolerance / sqrt_scale;
            val = k_sqrt_err_tolerance * abs_da / approx_parabola_integral(x_min);
        }
    }

    let u0 = approx_parabola_inv_integral(a0);
    let u2 = approx_parabola_inv_integral(a2);
    let u_scale = 1.0 / (u2 - u0);

    FlattenParams {
        a0,
        da,
        u0,
        u_scale,
        curvature_integral: val,
    }
}

/// Splits a conic into cubics: the conic is bisected as many times as its error requires, and each
/// piece is approximated by a cubic.
///
/// 1. Error Proxy: The vector `errV = p0 - 2*p1 + p2` is the second difference of the control
///    points. If the curve were a standard polynomial quadratic (w = 1), this vector's length
///    would directly bound the deviation between the curve and its chord. We use this as our
///    baseline metric for the polygon's curvature. Because conics are rational, weights
///    (0 < w < 1) cause the curve to parameterize non-uniformly. We multiply by `algFactor`
///    (1 / w^2) to heuristically overestimate the distortion and force enough subdivisions to
///    hide visual errors.
///
/// 2. Midpoint Approximation: This implementation uses the G1 mid-point approximation from
///    Proposition 5 of Floater, M. S. (1996). Floater demonstrates that the error of this
///    approximation is O(h^6), where h is the parametric interval. Thus, bisecting the curve at
///    t=0.5 reduces the worst-case error bound by a factor of 64 (2^6).
///
/// Returns the total number of generated cubics (1 << bisections).
// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L196-L255 (chrome/m156)
fn split_conics_to_cubics(
    pts: &[Point],
    w: f32,
    out_cubics: &mut [[Point; 4]; Flatten::K_MAX_CUBICS_FROM_CONIC as usize],
) -> u32 {
    let p0 = Float2::new(pts[0].x, pts[0].y);
    let p1 = Float2::new(pts[1].x, pts[1].y);
    let p2 = Float2::new(pts[2].x, pts[2].y);

    let alg_factor = if w < 1.0 && w > 0.0 {
        1.0 / (w * w)
    } else {
        1.0
    };
    let err_v = p0 - p1 * 2.0f32 + p2;
    let err = alg_factor * length(err_v);

    // `1.0f / kConicToCubicErrTolerance` is computed in `double`, then rounded to a `float`.
    const INV_TOL: f32 = (1.0 / Flatten::K_CONIC_TO_CUBIC_ERR_TOLERANCE) as f32;
    let bisections = nextlog64(err * INV_TOL).min(Flatten::K_MAX_BISECTIONS as i32);

    #[derive(Clone, Copy)]
    struct ConicState {
        p0: Float2,
        p1: Float2,
        p2: Float2,
        w: f32,
        level: i32,
    }

    let empty = ConicState {
        p0,
        p1,
        p2,
        w,
        level: 0,
    };
    let mut stack = [empty; Flatten::K_MAX_BISECTIONS as usize + 1];
    let mut stack_idx = 0usize;
    stack[stack_idx] = ConicState {
        p0,
        p1,
        p2,
        w,
        level: bisections,
    };
    stack_idx += 1;

    let mut cubic_idx = 0usize;
    // The tree of a full bisection has this many nodes.
    const K_MAX_TREE_NODES: i32 = (Flatten::K_MAX_CUBICS_FROM_CONIC as i32 * 2) - 1;
    for _ in 0..K_MAX_TREE_NODES {
        if stack_idx == 0 {
            break;
        }
        stack_idx -= 1;
        let c = stack[stack_idx];

        if c.level == 0 {
            let k = (4.0f32 / 3.0f32) * (c.w / (1.0 + c.w));
            let c1 = c.p0 + (c.p1 - c.p0) * k;
            let c2 = c.p2 + (c.p1 - c.p2) * k;

            out_cubics[cubic_idx][0] = Point::new(c.p0.x(), c.p0.y());
            out_cubics[cubic_idx][1] = Point::new(c1.x(), c1.y());
            out_cubics[cubic_idx][2] = Point::new(c2.x(), c2.y());
            out_cubics[cubic_idx][3] = Point::new(c.p2.x(), c.p2.y());
            cubic_idx += 1;
        } else {
            let inv = 1.0 / (1.0 + c.w);
            let p01 = (c.p0 + c.p1 * c.w) * inv;
            let p12 = (c.p1 * c.w + c.p2) * inv;
            let m = (p01 + p12) * 0.5f32;
            let new_weight = ((1.0 + c.w) * 0.5f32).sqrt();

            stack[stack_idx] = ConicState {
                p0: m,
                p1: p12,
                p2: c.p2,
                w: new_weight,
                level: c.level - 1,
            };
            stack_idx += 1;
            stack[stack_idx] = ConicState {
                p0: c.p0,
                p1: p01,
                p2: m,
                w: new_weight,
                level: c.level - 1,
            };
            stack_idx += 1;
        }
    }

    1u32 << bisections
}

// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L257-L261 (chrome/m156)
fn determine_quad_subdiv_t(params: &FlattenParams, x: f64) -> f64 {
    let a = params.a0 + params.da * x;
    let u = approx_parabola_inv_integral(a);
    (u - params.u0) * params.u_scale
}

// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L263-L266 (chrome/m156)
fn eval_quad_scalar(pts: &[Point], t: f32) -> Point {
    let mt = 1.0f32 - t;
    pts[0] * (mt * mt) + pts[1] * (2.0f32 * mt * t) + pts[2] * (t * t)
}

// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L268-L274 (chrome/m156)
fn eval_cubic_scalar(pts: &[Point], t: f32) -> Point {
    let mt = 1.0f32 - t;
    let mt2 = mt * mt;
    let t2 = t * t;
    pts[0] * (mt2 * mt)
        + pts[1] * (3.0f32 * mt2 * t)
        + pts[2] * (3.0f32 * mt * t2)
        + pts[3] * (t2 * t)
}

// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L276-L287 (chrome/m156)
fn is_completely_culled(pts: &[Point], width: f32, height: f32) -> bool {
    let mut top = true;
    let mut right = true;
    let mut bottom = true;
    for pt in pts {
        top &= pt.y < 0.0;
        right &= pt.x > width;
        bottom &= pt.y > height;
    }
    top || right || bottom
}

// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L289-L296 (chrome/m156)
fn is_completely_left(pts: &[Point]) -> bool {
    let mut left = true;
    for pt in pts {
        left &= pt.x < 0.0;
    }
    left
}

// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L298-L315 (chrome/m156)
fn is_within_dist_sq(p: Point, a: Point, b: Point, k_err_tolerance: f32) -> bool {
    let ab = b - a;
    let ap = p - a;

    let len_sq = Point::dot_product(ab, ab);
    let t_num = Point::dot_product(ap, ab);

    if len_sq == 0.0 || t_num <= 0.0 {
        return Point::dot_product(ap, ap) <= k_err_tolerance;
    }

    if t_num >= len_sq {
        let bp = p - b;
        return Point::dot_product(bp, bp) <= k_err_tolerance;
    }

    let cross = Point::cross_product(ap, ab);
    (cross * cross) <= (k_err_tolerance * len_sq)
}

/// `kIota`.
// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L324-L329 (chrome/m156)
fn k_iota() -> Float8 {
    Float8::from_list(&[0.0, 0.0, 2.0, 2.0, 1.0, 1.0, 3.0, 3.0])
}

/// `kIota2`.
// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L330-L331 (chrome/m156)
fn k_iota2() -> Float8 {
    Float8::from_list(&[0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0])
}

// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L324-L327 (chrome/m156)
fn splat_pt_simd(pt: Point) -> Float8 {
    Float8::from_list(&[pt.x, pt.y, pt.x, pt.y, pt.x, pt.y, pt.x, pt.y])
}

// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L329-L338 (chrome/m156)
fn approx_parabola_integral_simd(x: Float4) -> Float4 {
    const K_D: f32 = 0.67;
    const K_D_POW4: f32 = K_D * K_D * K_D * K_D;
    let x2 = x * x;
    let temp = sqrt(sqrt(fma(x2, Float4::splat(0.25), Float4::splat(K_D_POW4))));
    let denom = temp + (1.0f32 - K_D);
    x / denom
}

// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L340-L350 (chrome/m156)
fn approx_parabola_inv_integral_simd(x: Float8) -> Float8 {
    const K_B: f32 = 0.39;
    const K_ONE_MINUS_B: f32 = 1.0f32 - K_B;
    const K_B2: f32 = K_B * K_B;
    let x2 = x * x;
    let temp = sqrt(fma(x2, Float8::splat(0.25), Float8::splat(K_B2)));
    let factor = temp + K_ONE_MINUS_B;
    x * factor
}

// Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L352-L355 (chrome/m156)
fn is_finite_simd(x: Float4) -> Int4 {
    // x is guaranteed positive area/length, no abs() needed to clear sign bit
    x.bit_cast::<4, i32>().lt_mask(Int4::splat(0x7f80_0000))
}

/// The scratch values used during cubic flattening. With the exception of `flattened_cubics`, all
/// members are overwritten during `flattenCubic*()` calls. Consequently, only `flattened_cubics`
/// requires manual state management (clearing) at the start of the function.
// Port of: src/gpu/graphite/sparse_strips/Flatten.h#L139-L170 (chrome/m156)
#[derive(Clone, Debug)]
struct CubicFlattenCtx {
    /// The on-curve endpoints (p0 and p2) of the approximated quadratic segments, where p2 of the
    /// prior quad is p0 of the next quad, as `x, y` pairs (the C++ stores `SkPoint`s and writes
    /// them through `float*`). Sized +4 to safely handle the final point insertion and potential
    /// SIMD padding.
    even_pts: Vec<f32>,
    /// The off-curve control points (p1) of the approximated quadratic segments, as `x, y` pairs.
    /// The C++ array has no padding; the one here has a few points of it, because the stores of
    /// the SIMD evaluation write whole chunks.
    odd_pts: Vec<f32>,
    /// The starting value of the parabolic integral for each quadratic segment. Maps the start of
    /// the curve into uniform arc-length space.
    a0: Vec<f32>,
    /// The delta of the parabolic integral (a2 - a0) for each quadratic. Used to linearly
    /// interpolate the integral 'a' across the segment.
    da: Vec<f32>,
    /// The starting value of the inverse parabolic integral for each quadratic.
    u0: Vec<f32>,
    /// The scale factor for the inverse integral (1.0 / (u2 - u0)). Used to map the evenly spaced
    /// parabolic steps back into parametric 't' space.
    u_scale: Vec<f32>,
    /// Total integrated curvature/error metric for each quadratic. This dictates how many flat
    /// line segments each quad needs to be broken into.
    curvature_integral: Vec<f32>,
    /// The actual number of quadratics the current cubic was split into. Guaranteed to be <=
    /// `kMaxQuadsInCtx`.
    num_quads: u32,
    /// The final dynamic array where the flattened points of the cubic are gathered before being
    /// appended to the main polyline.
    flattened_cubics: Vec<Point>,
}

impl CubicFlattenCtx {
    fn new() -> Self {
        let quads = Flatten::K_MAX_QUADS_IN_CTX as usize;
        Self {
            even_pts: vec![0.0; (quads + 4) * 2],
            odd_pts: vec![0.0; (quads + 4) * 2],
            a0: vec![0.0; quads + 4],
            da: vec![0.0; quads + 4],
            u0: vec![0.0; quads + 4],
            u_scale: vec![0.0; quads + 4],
            curvature_integral: vec![0.0; quads + 4],
            num_quads: 0,
            flattened_cubics: Vec::new(),
        }
    }

    fn even_pt(&self, i: usize) -> Point {
        Point::new(self.even_pts[2 * i], self.even_pts[2 * i + 1])
    }

    fn odd_pt(&self, i: usize) -> Point {
        Point::new(self.odd_pts[2 * i], self.odd_pts[2 * i + 1])
    }
}

/// Stores `p` (four points of `x, y` pairs) at `flattened[start_idx..start_idx + 4]`.
fn store_points(p: Float8, flattened: &mut [Point], start_idx: usize) {
    for q in 0..4 {
        flattened[start_idx + q] = Point::new(p[2 * q], p[2 * q + 1]);
    }
}

/// `Flatten`: flattens paths into [`Polyline`]s.
// Port of: src/gpu/graphite/sparse_strips/Flatten.h#L75-L208 (chrome/m156)
#[doc(alias = "skgpu::graphite::Flatten")]
#[derive(Debug)]
pub struct Flatten {
    context: CubicFlattenCtx,
    conic_to_quad: AutoConicToQuads,
}

impl Default for Flatten {
    fn default() -> Self {
        Self::new()
    }
}

impl Flatten {
    /// `kEpsilonD`.
    pub const K_EPSILON_D: f64 = 1e-6;
    /// `kEpsilonF`.
    pub const K_EPSILON_F: f32 = Self::K_EPSILON_D as f32;
    /// `kQuadErrTolerance`.
    pub const K_QUAD_ERR_TOLERANCE: f64 = 0.25;
    /// `kSqrtQuadTolerance`.
    pub const K_SQRT_QUAD_TOLERANCE: f64 = 0.5;
    /// `kQuadTolerance2`.
    pub const K_QUAD_TOLERANCE2: f64 = Self::K_QUAD_ERR_TOLERANCE * Self::K_QUAD_ERR_TOLERANCE;
    /// When subdividing a cubic, we have to budget our error between the lowering from
    /// Cubics->Quad, and flattenning from Quads->Lines. `kCubicAccuracy` is the amount we
    /// allocate to Cubics->Quad.
    pub const K_CUBIC_ACCURACY: f32 = 0.1;
    /// `kCubicErrTolerance`.
    pub const K_CUBIC_ERR_TOLERANCE: f64 =
        Self::K_CUBIC_ACCURACY as f64 * Self::K_QUAD_ERR_TOLERANCE;
    /// `kQuadSubdivThreshold`.
    pub const K_QUAD_SUBDIV_THRESHOLD: f32 = (4.0f32 as f64 * Self::K_QUAD_TOLERANCE2) as f32;
    /// `kCubicSubdivThreshold`.
    pub const K_CUBIC_SUBDIV_THRESHOLD: f32 =
        ((16.0f32 / 9.0f32) as f64 * Self::K_QUAD_TOLERANCE2) as f32;
    /// `kMaxQuadsFromCubic`.
    pub const K_MAX_QUADS_FROM_CUBIC: u32 = 16;

    /// `kSqrtQuadFromCubicTol`: the square root of the error the Quad->Line conversion gets of a
    /// cubic, `(1.0f - kCubicAccuracy) * kQuadErrTolerance` (`0x1.e5b9d136c6d96p-2`). Since
    /// `std::sqrt` is not constexpr, the value is calculated offline; if `kCubicAccuracy` changes,
    /// this needs to be recalculated.
    pub const K_SQRT_QUAD_FROM_CUBIC_TOL: f64 = f64::from_bits(0x3FDE_5B9D_136C_6D96);

    /// When subdividing a conic we have to budget our error tolerance across the entire
    /// Conics->Cubics->Quad->Line conversion. In this scheme we give Conics 10% of the error,
    /// Cubics 10%, and Quads 80%.
    pub const K_CONIC_ACCURACY: f32 = 0.1;
    /// `kConicToCubicErrTolerance`.
    pub const K_CONIC_TO_CUBIC_ERR_TOLERANCE: f64 =
        Self::K_CONIC_ACCURACY as f64 * Self::K_QUAD_ERR_TOLERANCE;
    /// `kMaxBisections`.
    pub const K_MAX_BISECTIONS: u32 = 4;
    /// `kMaxCubicsFromConic`.
    pub const K_MAX_CUBICS_FROM_CONIC: u32 = 1 << Self::K_MAX_BISECTIONS;

    /// `kSqrtQuadFromCubicTolForConic`: the square root of the error the Quad->Line conversion
    /// gets of a conic, `(1.0f - kConicAccuracy - kCubicAccuracy) * kQuadErrTolerance`
    /// (`0x1.c9f25c264ef86p-2`).
    pub const K_SQRT_QUAD_FROM_CUBIC_TOL_FOR_CONIC: f64 = f64::from_bits(0x3FDC_9F25_C264_EF86);

    /// The total amount of quads we can accumulate from flattening a single "higher level" curve.
    /// In practice, conics very rarely exceed 64.
    pub const K_MAX_QUADS_IN_CTX: u32 =
        Self::K_MAX_QUADS_FROM_CUBIC * Self::K_MAX_CUBICS_FROM_CONIC;

    /// `Flatten()`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            context: CubicFlattenCtx::new(),
            conic_to_quad: AutoConicToQuads::new(),
        }
    }

    /// `processPaths<kMode>(path, ctm, width, height, polyline)`.
    ///
    /// # Panics
    /// In debug builds, if `ctm` has perspective.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.h#L120-L132 (chrome/m156)
    #[doc(alias = "processPaths")]
    pub fn process_paths(
        &mut self,
        mode: FlattenMode,
        path: &Path,
        ctm: &Matrix,
        width: f32,
        height: f32,
        polyline: &mut Polyline,
    ) {
        debug_assert!(!ctm.has_perspective());
        match mode {
            FlattenMode::Simd => self.process_paths_simd(path, ctm, width, height, polyline),
            FlattenMode::Scalar => self.process_paths_scalar(path, ctm, width, height, polyline),
        }
    }

    /// `processPathsSimd`.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L1025-L1029 (chrome/m156)
    fn process_paths_simd(
        &mut self,
        path: &Path,
        ctm: &Matrix,
        width: f32,
        height: f32,
        polyline: &mut Polyline,
    ) {
        self.process_paths_simd_impl::<true, true>(path, ctm, width, height, polyline);
    }

    /// `processPathsScalar`.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L1031-L1035 (chrome/m156)
    fn process_paths_scalar(
        &mut self,
        path: &Path,
        ctm: &Matrix,
        width: f32,
        height: f32,
        polyline: &mut Polyline,
    ) {
        self.process_paths_scalar_impl::<true, true>(path, ctm, width, height, polyline);
    }

    /// `processPathsSimdTest`: the non-simplifying, non-culling flattening, to be used as a
    /// source of truth.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L1038-L1042 (chrome/m156)
    #[doc(alias = "processPathsSimdTest")]
    pub fn process_paths_simd_test(
        &mut self,
        path: &Path,
        ctm: &Matrix,
        width: f32,
        height: f32,
        polyline: &mut Polyline,
    ) {
        self.process_paths_simd_impl::<false, false>(path, ctm, width, height, polyline);
    }

    /// `processPathsScalarTest`: the non-simplifying, non-culling flattening, to be used as a
    /// source of truth.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L1044-L1048 (chrome/m156)
    #[doc(alias = "processPathsScalarTest")]
    pub fn process_paths_scalar_test(
        &mut self,
        path: &Path,
        ctm: &Matrix,
        width: f32,
        height: f32,
        polyline: &mut Polyline,
    ) {
        self.process_paths_scalar_impl::<false, false>(path, ctm, width, height, polyline);
    }

    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L496-L514 (chrome/m156)
    /// `flattenQuadScalar(pts, polyline)`.
    fn flatten_quad_scalar(pts: &[Point], polyline: &mut Polyline) {
        let params = estimate_lines_from_quad(pts, Self::K_SQRT_QUAD_TOLERANCE);

        // The error of a flat segment spanning W of parabolic distance is (a * W^2) / 4. Taking
        // the square root makes this easier: sqrt(Error) = (sqrt(a) * W) / 2
        // n = fCurvatureIntegral / (kSqrtQuadTolerance * 2).
        let num_segments =
            1u32.max((0.5 / Self::K_SQRT_QUAD_TOLERANCE * params.curvature_integral).ceil() as u32);
        let step = 1.0 / f64::from(num_segments);
        for i in 1..num_segments {
            let u = f64::from(i) * step;
            let t = determine_quad_subdiv_t(&params, u);
            polyline.append_point(eval_quad_scalar(pts, t as f32));
        }
        polyline.append_point(pts[2]);
    }

    /// `flattenCubicScalar<kSqrtQuadTol>(pts)`: returns the number of points gathered in
    /// `flattened_cubics`.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L516-L607 (chrome/m156)
    fn flatten_cubic_scalar(&mut self, pts: &[Point], k_sqrt_quad_tol: f64) -> u32 {
        let num_quads = estimate_num_quads_from_cubic(pts);
        self.context.num_quads = num_quads;

        const K_HALF_OVER_N: [f32; Flatten::K_MAX_QUADS_FROM_CUBIC as usize + 1] = [
            0.0,
            0.5 / 1.0,
            0.5 / 2.0,
            0.5 / 3.0,
            0.5 / 4.0,
            0.5 / 5.0,
            0.5 / 6.0,
            0.5 / 7.0,
            0.5 / 8.0,
            0.5 / 9.0,
            0.5 / 10.0,
            0.5 / 11.0,
            0.5 / 12.0,
            0.5 / 13.0,
            0.5 / 14.0,
            0.5 / 15.0,
            0.5 / 16.0,
        ];
        let dt = K_HALF_OVER_N[num_quads as usize];

        let mut curvature_integral_sum = 0.0f32;
        for i in 0..num_quads {
            let t0 = (2 * i) as f32 * dt;
            let t1 = (2 * i + 1) as f32 * dt;
            let t2 = (2 * i + 2) as f32 * dt;

            let p0 = eval_cubic_scalar(pts, t0);
            let p_one_half = eval_cubic_scalar(pts, t1);
            let p2 = eval_cubic_scalar(pts, t2);

            let p1 = p_one_half * 2.0f32 - (p0 + p2) * 0.5f32;

            let iu = i as usize;
            self.context.even_pts[2 * iu] = p0.x;
            self.context.even_pts[2 * iu + 1] = p0.y;
            self.context.odd_pts[2 * iu] = p1.x;
            self.context.odd_pts[2 * iu + 1] = p1.y;
            self.context.even_pts[2 * (iu + 1)] = p2.x;
            self.context.even_pts[2 * (iu + 1) + 1] = p2.y;

            let quad = [p0, p1, p2];
            let params = estimate_lines_from_quad(&quad, k_sqrt_quad_tol);

            self.context.a0[iu] = params.a0 as f32;
            self.context.da[iu] = params.da as f32;
            self.context.u0[iu] = params.u0 as f32;
            self.context.u_scale[iu] = params.u_scale as f32;
            self.context.curvature_integral[iu] =
                std_max(params.curvature_integral as f32, Self::K_EPSILON_F);
            // `float += double`: the sum is computed in `double`, then rounded to `float`.
            curvature_integral_sum =
                (f64::from(curvature_integral_sum) + params.curvature_integral) as f32;
        }

        let num_segments =
            1u32.max(((0.5 * f64::from(curvature_integral_sum)) / k_sqrt_quad_tol).ceil() as u32);

        let target_len = num_segments + 4;
        self.context
            .flattened_cubics
            .resize(target_len as usize, Point::default());

        let step = curvature_integral_sum / num_segments as f32;
        let step_recip = 1.0f32 / step;
        let mut cumulative_curvature = 0.0f32;
        let mut last_n = 0u32;
        let mut x0_base = 0.0f32;

        for i in 0..num_quads {
            let iu = i as usize;
            cumulative_curvature += self.context.curvature_integral[iu];
            let this_n = cumulative_curvature * step_recip;
            let this_n_next = 1.0f32 + this_n.floor();

            let dn = (this_n_next as u32).wrapping_sub(last_n); // thisNNext always > lastN
            if dn > 0 {
                let dx = step / self.context.curvature_integral[iu];
                let x0 = x0_base * dx;

                let p0 = self.context.even_pt(iu);
                let p1 = self.context.odd_pt(iu);
                let p2 = self.context.even_pt(iu + 1);

                let a0 = self.context.a0[iu];
                let da = self.context.da[iu];
                let u0 = self.context.u0[iu];
                let u_scale = self.context.u_scale[iu];

                for j in 0..dn {
                    let x = x0 + j as f32 * dx;
                    let a = a0 + da * x;
                    let u = approx_parabola_inv_integral(f64::from(a)) as f32;
                    let t = (u - u0) * u_scale;
                    let mt = 1.0f32 - t;
                    let p = p0 * (mt * mt) + p1 * (2.0f32 * t * mt) + p2 * (t * t);
                    self.context.flattened_cubics[(last_n + j) as usize] = p;
                }
            }

            x0_base = this_n_next - this_n;
            last_n = this_n_next as u32;
        }

        self.context.flattened_cubics[num_segments as usize] =
            self.context.even_pt(num_quads as usize);
        num_segments + 1
    }

    /// `flattenQuadSimd(pts, polyline)`.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L609-L650 (chrome/m156)
    #[doc(alias = "flattenQuadSimd")]
    pub fn flatten_quad_simd(pts: &[Point], polyline: &mut Polyline) {
        let params = estimate_lines_from_quad(pts, Self::K_SQRT_QUAD_TOLERANCE);

        let num_segments =
            1u32.max((0.5 / Self::K_SQRT_QUAD_TOLERANCE * params.curvature_integral).ceil() as u32);
        let step = 1.0f32 / num_segments as f32;

        let p0 = splat_pt_simd(pts[0]);
        let p1 = splat_pt_simd(pts[1]);
        let p2 = splat_pt_simd(pts[2]);

        let a = fma(p1, Float8::splat(-2.0), p0) + p2;
        let b = (p1 - p0) * 2.0f32;
        let c = p0;

        let v_da = Float8::splat(params.da as f32);
        let v_a0 = Float8::splat(params.a0 as f32);
        let v_u0 = Float8::splat(params.u0 as f32);
        let v_u_scale = Float8::splat(params.u_scale as f32);

        let mut out = [0.0f32; 8];
        let mut i = 1u32;
        while i < num_segments {
            let x = (k_iota2() + Float8::splat(i as f32)) * step;
            let a_vec = fma(v_da, x, v_a0);
            let u = approx_parabola_inv_integral_simd(a_vec);
            let t = (u - v_u0) * v_u_scale;

            let p = fma(fma(a, t, b), t, c);
            p.store(&mut out);

            polyline.append_point(Point::new(out[0], out[1]));
            if i + 1 < num_segments {
                polyline.append_point(Point::new(out[2], out[3]));
            }
            if i + 2 < num_segments {
                polyline.append_point(Point::new(out[4], out[5]));
            }
            if i + 3 < num_segments {
                polyline.append_point(Point::new(out[6], out[7]));
            }
            i += 4;
        }
        polyline.append_point(pts[2]);
    }

    /// `evalCubicsSimd(pts, numQuads, startIdx)`: evaluates the end and mid points of the
    /// quadratics that approximate a cubic.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L652-L682 (chrome/m156)
    fn eval_cubics_simd(&mut self, pts: &[Point], num_quads: u32, start_idx: u32) {
        let dt = 0.5f32 / num_quads as f32;

        let p0 = splat_pt_simd(pts[0]);
        let p1 = splat_pt_simd(pts[1]);
        let p2 = splat_pt_simd(pts[2]);
        let p3 = splat_pt_simd(pts[3]);

        let a = fma(p1 - p2, Float8::splat(3.0), p3 - p0);
        let b = fma(p1, Float8::splat(-2.0), p0 + p2) * 3.0f32;
        let c = (p1 - p0) * 3.0f32;
        let d = p0;

        let step = k_iota() * dt;
        let mut t = step;
        let t_inc = Float8::splat(4.0f32 * dt);

        let even_base = start_idx as usize * 2;
        let loop_count = num_quads.div_ceil(2);
        for i in 0..loop_count as usize {
            let evaluated = fma(fma(fma(a, t, b), t, c), t, d);
            evaluated
                .lo()
                .store(&mut self.context.even_pts[even_base + i * 4..]);
            evaluated
                .hi()
                .store(&mut self.context.odd_pts[even_base + i * 4..]);
            t += t_inc;
        }
        p3.store(&mut self.context.even_pts[even_base + num_quads as usize * 2..]);
    }

    /// `estimateLinesFromQuadSimd<kSqrtQuadTol>()`: `estimate_lines_from_quad` of four quads at a
    /// time.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L684-L754 (chrome/m156)
    fn estimate_lines_from_quad_simd(&mut self, k_sqrt_quad_tol: f64) {
        let num_quads = self.context.num_quads;
        let chunks = num_quads.div_ceil(4);

        for i in 0..chunks as usize {
            let p0 = Float8::load(&self.context.even_pts[i * 8..]);
            let p_one_half = Float8::load(&self.context.odd_pts[i * 8..]);
            let p2 = Float8::load(&self.context.even_pts[i * 8 + 2..]);

            let p1 = fma(p_one_half, Float8::splat(2.0), (p0 + p2) * -0.5f32);
            p1.store(&mut self.context.odd_pts[i * 8..]);

            let d01 = p1 - p0;
            let d12 = p2 - p1;

            let d01x: Float4 = shuffle(d01, [0, 2, 4, 6]);
            let d01y: Float4 = shuffle(d01, [1, 3, 5, 7]);
            let d12x: Float4 = shuffle(d12, [0, 2, 4, 6]);
            let d12y: Float4 = shuffle(d12, [1, 3, 5, 7]);

            let ddx = d01x - d12x;
            let ddy = d01y - d12y;

            let cross = (d01x + d12x) * ddy - (d01y + d12y) * ddx;

            let abs_cross = abs(cross);
            let collinear_mask: Int4 = abs_cross.lt_mask(Self::K_EPSILON_F);

            let inv_cross = 1.0f32 / cross;
            let x0 = fma(d01x, ddx, d01y * ddy) * inv_cross;
            let x2 = fma(d12x, ddx, d12y * ddy) * inv_cross;

            let dd_sq = fma(ddx, ddx, ddy * ddy);
            let dd_hypot = sqrt(dd_sq);
            let scale = (cross * cross) / (dd_hypot * dd_sq);

            let a0 = approx_parabola_integral_simd(x0);
            let a2 = approx_parabola_integral_simd(x2);
            let da = a2 - a0;

            let abs_da = abs(da);
            let sqrt_scale = sqrt(scale);

            let sign_x0 = x0.bit_cast::<4, i32>() & i32::MIN;
            let sign_x2 = x2.bit_cast::<4, i32>() & i32::MIN;
            let mask: Int4 = sign_x0.eq_mask(sign_x2);

            let non_cusp = abs_da * sqrt_scale;

            let x_min = (k_sqrt_quad_tol as f32) / sqrt_scale;
            let approx_int = approx_parabola_integral_simd(x_min);
            let cusp = ((k_sqrt_quad_tol as f32) * abs_da) / approx_int;

            let mut val_raw = if_then_else(mask, non_cusp, cusp);
            val_raw = if_then_else(collinear_mask, Float4::splat(0.0), val_raw);

            let finite_mask = is_finite_simd(val_raw);
            let val = if_then_else(finite_mask, val_raw, Float4::splat(0.0));

            let u0_u2 = approx_parabola_inv_integral_simd(join(a0, a2));
            let mut u0 = u0_u2.lo();
            let u2 = u0_u2.hi();
            let mut u_scale = 1.0f32 / (u2 - u0);

            u0 = if_then_else(collinear_mask, Float4::splat(0.0), u0);
            u_scale = if_then_else(collinear_mask, Float4::splat(1.0), u_scale);

            a0.store(&mut self.context.a0[i * 4..]);
            da.store(&mut self.context.da[i * 4..]);
            u0.store(&mut self.context.u0[i * 4..]);
            u_scale.store(&mut self.context.u_scale[i * 4..]);
            val.store(&mut self.context.curvature_integral[i * 4..]);
        }
    }

    /// `outputLinesFromQuadSimd(quadIdx, x0, dx, numSegments, startIdx)`.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L756-L782 (chrome/m156)
    fn output_lines_from_quad_simd(
        &mut self,
        quad_idx: usize,
        x0: f32,
        dx: f32,
        num_segments: u32,
        start_idx: u32,
    ) {
        let p0 = splat_pt_simd(self.context.even_pt(quad_idx));
        let p1 = splat_pt_simd(self.context.odd_pt(quad_idx));
        let p2 = splat_pt_simd(self.context.even_pt(quad_idx + 1));

        let x = fma(k_iota2(), Float8::splat(dx), Float8::splat(x0));
        let da = Float8::splat(self.context.da[quad_idx]);
        let mut a = fma(x, da, Float8::splat(self.context.a0[quad_idx]));
        let a_inc = Float8::splat(4.0f32 * dx * self.context.da[quad_idx]);

        let u_scale = Float8::splat(self.context.u_scale[quad_idx]);
        let u0 = Float8::splat(self.context.u0[quad_idx]);

        let a_coef = fma(p1, Float8::splat(-2.0), p0) + p2;
        let b_coef = (p1 - p0) * 2.0f32;
        let c_coef = p0;

        let chunks = num_segments.div_ceil(4);
        for j in 0..chunks {
            let u = approx_parabola_inv_integral_simd(a);
            let t = (u - u0) * u_scale;

            let p = fma(fma(a_coef, t, b_coef), t, c_coef);
            store_points(
                p,
                &mut self.context.flattened_cubics,
                (start_idx + j * 4) as usize,
            );
            a += a_inc;
        }
    }

    /// `flattenCubicSimd<kSqrtQuadTol>(pts)`: returns the number of points gathered in
    /// `flattened_cubics`.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L784-L788 (chrome/m156)
    #[doc(alias = "flattenCubicSimd")]
    fn flatten_cubic_simd(&mut self, pts: &[Point], k_sqrt_quad_tol: f64) -> u32 {
        self.flatten_cubics_simd(&[[pts[0], pts[1], pts[2], pts[3]]], k_sqrt_quad_tol)
    }

    /// `flattenCubicsSimd<kSqrtQuadTol>(cubics)`: returns the number of points gathered in
    /// `flattened_cubics`.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L790-L840 (chrome/m156)
    #[doc(alias = "flattenCubicsSimd")]
    fn flatten_cubics_simd(&mut self, cubics: &[[Point; 4]], k_sqrt_quad_tol: f64) -> u32 {
        let mut total_quads = 0u32;
        for cubic in cubics {
            let num_quads = estimate_num_quads_from_cubic(cubic);
            self.eval_cubics_simd(cubic, num_quads, total_quads);
            total_quads += num_quads;
        }
        debug_assert!(total_quads <= Self::K_MAX_QUADS_IN_CTX);
        self.context.num_quads = total_quads;

        self.estimate_lines_from_quad_simd(k_sqrt_quad_tol);

        let mut curvature_integral_sum = 0.0f32;
        for i in 0..total_quads as usize {
            let val = std_max(self.context.curvature_integral[i], Self::K_EPSILON_F);
            self.context.curvature_integral[i] = val;
            curvature_integral_sum += val;
        }

        let num_segments =
            1u32.max((f64::from(0.5f32 * curvature_integral_sum) / k_sqrt_quad_tol).ceil() as u32);

        let target_len = num_segments + 4;
        self.context
            .flattened_cubics
            .resize(target_len as usize, Point::default());

        let step = curvature_integral_sum / num_segments as f32;
        let step_recip = 1.0f32 / step;
        let mut cumulative_curvature = 0.0f32;
        let mut last_n = 0u32;
        let mut x0_base = 0.0f32;

        for i in 0..total_quads as usize {
            let val = self.context.curvature_integral[i];
            cumulative_curvature += val;
            let this_n = cumulative_curvature * step_recip;
            let this_n_next = 1.0f32 + this_n.floor();

            let dn = (this_n_next as u32).wrapping_sub(last_n);
            if dn > 0 {
                let dx = step / val;
                let x0 = x0_base * dx;
                self.output_lines_from_quad_simd(i, x0, dx, dn, last_n);
            }

            x0_base = this_n_next - this_n;
            last_n = this_n_next as u32;
        }

        self.context.flattened_cubics[num_segments as usize] =
            self.context.even_pt(total_quads as usize);
        num_segments + 1
    }

    /// `processPathsImpl`: walks the verbs of `path` and appends their flattened points to
    /// `polyline`, with the sentinels between sub-paths.
    ///
    /// `SIMD` selects the flattening of the curves. The C++ passes the three curve processors as
    /// lambdas; here they are `process_quad`, `process_conic` and `process_cubic`.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L361-L492, L895-L1015 (chrome/m156)
    fn process_paths_impl<
        const SIMD: bool,
        const K_SHOULD_CULL: bool,
        const K_SHOULD_SIMPLIFY: bool,
    >(
        &mut self,
        path: &Path,
        ctm: &Matrix,
        width: f32,
        height: f32,
        polyline: &mut Polyline,
    ) {
        self.context.flattened_cubics.clear();

        let is_identity = ctm.is_identity();

        let mut closed = true;
        let mut need_start_pt = false;
        let mut start_pt = Point::new(0.0, 0.0);
        let mut last_pt = Point::new(0.0, 0.0);

        let mut iter = Iter::new(path, false);
        let mut local_pts = [Point::default(); 4];
        let mut mapped_pts = [Point::default(); 4];
        loop {
            let verb = iter.next_verb(&mut local_pts);
            if verb == Verb::Done {
                break;
            }
            let count = match verb {
                Verb::Move => 1,
                Verb::Line => 2,
                Verb::Quad | Verb::Conic => 3,
                Verb::Cubic => 4,
                Verb::Close | Verb::Done => 0,
            };
            let pts: &[Point] = if is_identity {
                &local_pts[..count]
            } else {
                ctm.map_points(&mut mapped_pts[..count], &local_pts[..count]);
                &mapped_pts[..count]
            };

            match verb {
                Verb::Move => {
                    if !closed && last_pt != start_pt {
                        if need_start_pt {
                            polyline.append_point(last_pt);
                        }
                        polyline.append_point(start_pt);
                    }
                    polyline.append_sentinel();

                    closed = false;
                    need_start_pt = false;
                    last_pt = pts[0];
                    start_pt = pts[0];
                    polyline.append_point(pts[0]);
                }
                Verb::Line => {
                    debug_assert!(!closed);
                    if need_start_pt {
                        polyline.append_point(pts[0]);
                        need_start_pt = false;
                    }
                    last_pt = pts[1];
                    polyline.append_point(pts[1]);
                }
                Verb::Quad => {
                    debug_assert!(!closed);
                    if Self::process_quad::<K_SHOULD_CULL, K_SHOULD_SIMPLIFY>(
                        pts,
                        need_start_pt,
                        width,
                        height,
                        polyline,
                    ) {
                        need_start_pt = false;
                    } else if !need_start_pt {
                        polyline.append_sentinel();
                        need_start_pt = true;
                    }
                    last_pt = pts[2];
                }
                Verb::Conic => {
                    debug_assert!(!closed);
                    let weight = iter.conic_weight().unwrap_or(0.0);
                    if self.process_conic::<SIMD, K_SHOULD_CULL, K_SHOULD_SIMPLIFY>(
                        pts,
                        weight,
                        need_start_pt,
                        width,
                        height,
                        polyline,
                    ) {
                        need_start_pt = false;
                    } else if !need_start_pt {
                        polyline.append_sentinel();
                        need_start_pt = true;
                    }
                    last_pt = pts[2];
                }
                Verb::Cubic => {
                    debug_assert!(!closed);
                    if self.process_cubic::<SIMD, K_SHOULD_CULL, K_SHOULD_SIMPLIFY>(
                        pts,
                        need_start_pt,
                        width,
                        height,
                        polyline,
                    ) {
                        need_start_pt = false;
                    } else if !need_start_pt {
                        polyline.append_sentinel();
                        need_start_pt = true;
                    }
                    last_pt = pts[3];
                }
                Verb::Close => {
                    closed = true;
                    if last_pt != start_pt {
                        if need_start_pt {
                            polyline.append_point(last_pt);
                        }
                        polyline.append_point(start_pt);
                    }
                    need_start_pt = false;
                }
                Verb::Done => {}
            }
        }

        if !closed && last_pt != start_pt {
            if need_start_pt {
                polyline.append_point(last_pt);
            }
            polyline.append_point(start_pt);
        }
        polyline.append_sentinel();
    }

    /// `processQuad`: the quad lambda of `processPathsSimdImpl` / `processPathsScalarImpl` (the
    /// SIMD one flattens with `flattenQuadScalar` as well). Returns false if the quad is culled.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L664-L690, L953-L972 (chrome/m156)
    fn process_quad<const K_SHOULD_CULL: bool, const K_SHOULD_SIMPLIFY: bool>(
        pts: &[Point],
        need_start_pt: bool,
        width: f32,
        height: f32,
        polyline: &mut Polyline,
    ) -> bool {
        if K_SHOULD_CULL && is_completely_culled(&pts[..3], width, height) {
            return false;
        }

        if need_start_pt {
            polyline.append_point(pts[0]);
        }

        if K_SHOULD_SIMPLIFY
            && (is_completely_left(&pts[..3])
                || is_within_dist_sq(pts[1], pts[0], pts[2], Self::K_QUAD_SUBDIV_THRESHOLD))
        {
            polyline.append_point(pts[2]);
            return true;
        }

        Self::flatten_quad_scalar(pts, polyline);
        true
    }

    /// `processConic`: the conic lambda of `processPathsSimdImpl` (SIMD: the conic is split into
    /// cubics) / `processPathsScalarImpl` (the conic is split into quads). Returns false if the
    /// conic is culled.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L692-L722, L974-L1000 (chrome/m156)
    fn process_conic<const SIMD: bool, const K_SHOULD_CULL: bool, const K_SHOULD_SIMPLIFY: bool>(
        &mut self,
        pts: &[Point],
        weight: f32,
        need_start_pt: bool,
        width: f32,
        height: f32,
        polyline: &mut Polyline,
    ) -> bool {
        // The SIMD lambda culls and simplifies with the same predicates as the scalar one, on
        // `skvx::float4`s that repeat the last point.
        if K_SHOULD_CULL && Self::is_culled(SIMD, &pts[..3], width, height) {
            return false;
        }

        if need_start_pt {
            polyline.append_point(pts[0]);
        }

        if K_SHOULD_SIMPLIFY
            && (Self::is_left(SIMD, &pts[..3])
                || is_within_dist_sq(pts[1], pts[0], pts[2], Self::K_QUAD_SUBDIV_THRESHOLD))
        {
            polyline.append_point(pts[2]);
            return true;
        }

        if SIMD {
            let mut cubics = [[Point::default(); 4]; Self::K_MAX_CUBICS_FROM_CONIC as usize];
            let num_cubics = split_conics_to_cubics(pts, weight, &mut cubics);
            let num_segments = self.flatten_cubics_simd(
                &cubics[..num_cubics as usize],
                Self::K_SQRT_QUAD_FROM_CUBIC_TOL_FOR_CONIC,
            );
            polyline.append_points(&self.context.flattened_cubics[1..num_segments as usize]);
        } else {
            let quad_pts = self
                .conic_to_quad
                .compute_quads_with_weight(pts, weight, Self::K_QUAD_ERR_TOLERANCE as f32)
                .to_vec();
            let quad_count = self.conic_to_quad.count_quads();
            for i in 0..quad_count {
                Self::flatten_quad_scalar(&quad_pts[i * 2..i * 2 + 3], polyline);
            }
        }
        true
    }

    /// `processCubic`: the cubic lambda of `processPathsSimdImpl` / `processPathsScalarImpl`.
    /// Returns false if the cubic is culled.
    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L724-L755, L1002-L1015 (chrome/m156)
    fn process_cubic<const SIMD: bool, const K_SHOULD_CULL: bool, const K_SHOULD_SIMPLIFY: bool>(
        &mut self,
        pts: &[Point],
        need_start_pt: bool,
        width: f32,
        height: f32,
        polyline: &mut Polyline,
    ) -> bool {
        if K_SHOULD_CULL && Self::is_culled(SIMD, &pts[..4], width, height) {
            return false;
        }

        if need_start_pt {
            polyline.append_point(pts[0]);
        }

        if K_SHOULD_SIMPLIFY
            && (Self::is_left(SIMD, &pts[..4])
                || (is_within_dist_sq(pts[1], pts[0], pts[3], Self::K_CUBIC_SUBDIV_THRESHOLD)
                    && is_within_dist_sq(pts[2], pts[0], pts[3], Self::K_CUBIC_SUBDIV_THRESHOLD)))
        {
            polyline.append_point(pts[3]);
            return true;
        }

        let num_segments = if SIMD {
            self.flatten_cubic_simd(pts, Self::K_SQRT_QUAD_FROM_CUBIC_TOL)
        } else {
            self.flatten_cubic_scalar(pts, Self::K_SQRT_QUAD_FROM_CUBIC_TOL)
        };
        polyline.append_points(&self.context.flattened_cubics[1..num_segments as usize]);
        true
    }

    /// The culling test: `skvx::all(X > width) || skvx::all(Y < 0.0f) || skvx::all(Y > height)`
    /// of the SIMD lambdas (`float4`s, which repeat the last point of a quad or conic), or
    /// `is_completely_culled` of the scalar ones. They are the same predicate.
    fn is_culled(simd: bool, pts: &[Point], width: f32, height: f32) -> bool {
        if simd {
            let x = Self::float4_of(pts, |p| p.x);
            let y = Self::float4_of(pts, |p| p.y);
            all(x.gt_mask(width)) || all(y.lt_mask(0.0f32)) || all(y.gt_mask(height))
        } else {
            is_completely_culled(pts, width, height)
        }
    }

    /// The left-of-viewport test: `skvx::all(X < 0.0f)` of the SIMD lambdas, or
    /// `is_completely_left` of the scalar ones.
    fn is_left(simd: bool, pts: &[Point]) -> bool {
        if simd {
            all(Self::float4_of(pts, |p| p.x).lt_mask(0.0f32))
        } else {
            is_completely_left(pts)
        }
    }

    /// `skvx::float4 X(pts[0].fX, pts[1].fX, pts[2].fX, pts[2].fX)` (a quad or conic: the last
    /// point is duplicated), or the four points of a cubic.
    fn float4_of(pts: &[Point], coord: impl Fn(&Point) -> f32) -> Float4 {
        let last = pts.len() - 1;
        Float4::new(
            coord(&pts[0]),
            coord(&pts[1]),
            coord(&pts[2]),
            coord(&pts[last.min(3)]),
        )
    }

    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L842-L935 (chrome/m156)
    fn process_paths_simd_impl<const K_SHOULD_CULL: bool, const K_SHOULD_SIMPLIFY: bool>(
        &mut self,
        path: &Path,
        ctm: &Matrix,
        width: f32,
        height: f32,
        polyline: &mut Polyline,
    ) {
        self.process_paths_impl::<true, K_SHOULD_CULL, K_SHOULD_SIMPLIFY>(
            path, ctm, width, height, polyline,
        );
    }

    // Port of: src/gpu/graphite/sparse_strips/Flatten.cpp#L937-L1023 (chrome/m156)
    fn process_paths_scalar_impl<const K_SHOULD_CULL: bool, const K_SHOULD_SIMPLIFY: bool>(
        &mut self,
        path: &Path,
        ctm: &Matrix,
        width: f32,
        height: f32,
        polyline: &mut Polyline,
    ) {
        self.process_paths_impl::<false, K_SHOULD_CULL, K_SHOULD_SIMPLIFY>(
            path, ctm, width, height, polyline,
        );
    }
}
