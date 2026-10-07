// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/tessellate/Tessellation.h, src/gpu/tessellate/Tessellation.cpp

//! Curve helpers from Skia's GPU tessellator (`skgpu::tess`) that are exercised by CPU-side unit
//! tests. They live in `skia-rust-core` until the GPU crate exists.

use skia_rust_simd::vx::{self, Float2};

use crate::floating_point::ieee_float_divide;
use crate::geometry::from_point;
use crate::point::Point;

/// Finds the (up to 2) T values where a cubic should be chopped so that every piece rotates at
/// most 180 degrees ("convex-180"). If the curve inflects, these are the inflection points;
/// otherwise it is the T where rotation reaches 180 degrees. `are_cusps` is set to whether the
/// chops are cusps (or near-cusps) rather than regular inflections. Returns the number of chops
/// written to `t`.
// Port of: src/gpu/tessellate/Tessellation.cpp#L207-L348 (chrome/m156)
#[doc(alias = "FindCubicConvex180Chops")]
#[allow(clippy::many_single_char_names)] // names follow the C++ derivation in the comments
#[allow(clippy::float_cmp)] // exact float comparisons, as in Skia
pub fn find_cubic_convex_180_chops(pts: &[Point], t: &mut [f32; 2], are_cusps: &mut bool) -> usize {
    // If a chop falls within a distance of "kEpsilon" from 0 or 1, throw it out. Tangents become
    // unstable when we chop too close to the boundary. This works out because the tessellation
    // shaders don't allow more than 2^10 parametric segments, and they snap the beginning and
    // ending edges at 0 and 1. So if we overstep an inflection or point of 180-degree rotation by
    // a fraction of a tessellation segment, it just gets snapped.
    const K_EPSILON: f32 = 1.0f32 / 2048.0; // 1 / (1 << 11)
    // Floating-point representation of "1 - 2*kEpsilon".
    const K_IEEE_ONE_MINUS_2_EPSILON: u32 = (127 << 23) - 2 * (1 << (24 - 11));
    // Unfortunately we don't have a way to static_assert this, but we can runtime assert that the
    // kIEEE_one_minus_2_epsilon bits are correct.
    debug_assert_eq!(
        f32::from_bits(K_IEEE_ONE_MINUS_2_EPSILON),
        1.0 - 2.0 * K_EPSILON
    );

    let p0 = from_point(pts[0]);
    let p1 = from_point(pts[1]);
    let p2 = from_point(pts[2]);
    let p3 = from_point(pts[3]);

    // Find the cubic's power basis coefficients. These define the bezier curve as:
    //
    //                                    |T^3|
    //     Cubic(T) = x,y = |A  3B  3C| * |T^2| + P0
    //                      |.   .   .|   |T  |
    //
    // And the tangent direction (scaled by a uniform 1/3) will be:
    //
    //                                                 |T^2|
    //     Tangent_Direction(T) = dx,dy = |A  2B  C| * |T  |
    //                                    |.   .  .|   |1  |
    //
    let c = p1 - p0;
    let d = p2 - p1;
    let e = p3 - p0;
    let b = d - c;
    let a = -3.0f32 * d + e;

    // Now find the cubic's inflection function. There are inflections where F' x F'' == 0.
    // We formulate this as a quadratic equation:  F' x F'' == aT^2 + bT + c == 0.
    // See: https://www.microsoft.com/en-us/research/wp-content/uploads/2005/01/p1000-loop.pdf
    // NOTE: We only need the roots, so a uniform scale factor does not affect the solution.
    let mut qa = vx::cross(a, b);
    let qb = vx::cross(a, c);
    let mut qc = vx::cross(b, c);
    let mut b_over_minus_2 = -0.5f32 * qb;
    let mut discr_over_4 = b_over_minus_2 * b_over_minus_2 - qa * qc;

    // If -cuspThreshold <= discr_over_4 <= cuspThreshold, it means the two roots are within
    // kEpsilon of one another (in parametric space). This is close enough for our purposes to
    // consider them a single cusp.
    let mut cusp_threshold = qa * (K_EPSILON / 2.0);
    cusp_threshold *= cusp_threshold;

    if discr_over_4 < -cusp_threshold {
        // The curve does not inflect or cusp. This means it might rotate more than 180 degrees
        // instead. Chop were rotation == 180 deg. (This is the 2nd root where the tangent is
        // parallel to tan0.)
        //
        //      Tangent_Direction(T) x tan0 == 0
        //      (AT^2 x tan0) + (2BT x tan0) + (C x tan0) == 0
        //      (A x C)T^2 + (2B x C)T + (C x C) == 0  [[because tan0 == P1 - P0 == C]]
        //      bT^2 + 2cT + 0 == 0  [[because A x C == b, B x C == c]]
        //      T = [0, -2c/b]
        //
        // NOTE: if C == 0, then C != tan0. But this is fine because the curve is definitely
        // convex-180 if any points are colocated, and T[0] will equal NaN which returns 0 chops.
        *are_cusps = false;
        let root = ieee_float_divide(qc, b_over_minus_2);
        // Is "root" inside the range [kEpsilon, 1 - kEpsilon)?
        if (root - K_EPSILON).to_bits() < K_IEEE_ONE_MINUS_2_EPSILON {
            t[0] = root;
            return 1;
        }
        return 0;
    }

    *are_cusps = discr_over_4 <= cusp_threshold;
    if *are_cusps {
        // The two roots are close enough that we can consider them a single cusp.
        if qa != 0.0 || b_over_minus_2 != 0.0 || qc != 0.0 {
            // Pick the average of both roots.
            let root = ieee_float_divide(b_over_minus_2, qa);
            // Is "root" inside the range [kEpsilon, 1 - kEpsilon)?
            if (root - K_EPSILON).to_bits() < K_IEEE_ONE_MINUS_2_EPSILON {
                t[0] = root;
                return 1;
            }
            return 0;
        }

        // The curve is a flat line. The standard inflection function doesn't detect cusps from
        // flat lines. Find cusps by searching instead for points where the tangent is
        // perpendicular to tan0. This will find any cusp point.
        //
        //     dot(tan0, Tangent_Direction(T)) == 0
        //
        //                         |T^2|
        //     tan0 * |A  2B  C| * |T  | == 0
        //            |.   .  .|   |1  |
        //
        let tan0 = vx::if_then_else(c.ne_mask(0.0), c, p2 - p0);
        qa = vx::dot(tan0, a);
        b_over_minus_2 = -vx::dot(tan0, b);
        qc = vx::dot(tan0, c);
        discr_over_4 = b_over_minus_2 * b_over_minus_2 - qa * qc;
        if discr_over_4 < -cusp_threshold {
            // With the updated discriminant, this line actually wouldn't have cusps (e.g. it never
            // turns back on itself).
            return 0;
        }

        // std::max(discr_over_4, 0.f) is `(discr_over_4 < 0) ? 0 : discr_over_4`.
        discr_over_4 = if discr_over_4 < 0.0 {
            0.0
        } else {
            discr_over_4
        };
    }

    // Solve our quadratic equation to find where to chop. See the quadratic formula from
    // Numerical Recipes in C.
    let mut q = discr_over_4.sqrt();
    q = q.copysign(b_over_minus_2);
    q += b_over_minus_2;
    let mut roots = Float2::from_list(&[q, qc]) / Float2::from_list(&[qa, q]);

    let inside = roots.gt_mask(K_EPSILON) & roots.lt_mask(1.0 - K_EPSILON);
    if inside[0] != 0 {
        if inside[1] != 0 && roots[0] != roots[1] {
            if roots[0] > roots[1] {
                roots = vx::shuffle::<2, 2, f32>(roots, [1, 0]); // Sort.
            }
            roots.store(t);
            return 2;
        }
        t[0] = roots[0];
        return 1;
    }
    if inside[1] != 0 {
        t[0] = roots[1];
        return 1;
    }
    0
}
