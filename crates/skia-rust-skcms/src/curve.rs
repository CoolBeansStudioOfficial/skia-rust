// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skcms/skcms.cc

//! Evaluating curves, comparing them with transfer functions, and fitting a parametric
//! [`TransferFunction`] to a table-based [`Curve`].

use crate::math::{
    Vector3, cvt_i32, fabsf_, fmaxf_, fminf_, isfinitef_, log2f_, logf_, minus_1_ulp, mv_mul, powf_,
};
use crate::public::{Curve, Matrix3x3, TfType, TransferFunction};

// See https://crbug.com/492744328 for how this was determined.
// Port of: modules/skcms/skcms.cc#L662 (chrome/m156)
pub(crate) const MAX_TABLE_ENTRIES: u32 = 1 << 24; // 16,777,216

// Port of: modules/skcms/skcms.cc#L308-L332 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(table_entries - 1)
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // table indices are in range
pub(crate) fn eval_curve(curve: &Curve, x: f32) -> f32 {
    let (entries, table_8, table_16) = match curve {
        Curve::Parametric(tf) => return tf.eval(x),
        Curve::Table8 { entries, table } => (*entries, Some(table), None),
        Curve::Table16 { entries, table } => (*entries, None, Some(table)),
    };

    let ix = fmaxf_(0.0, fminf_(x, 1.0)) * (entries.wrapping_sub(1)) as f32;
    let lo = cvt_i32(ix);
    let hi = cvt_i32(minus_1_ulp(ix + 1.0));
    let t = ix - lo as f32;

    let (l, h);
    if let Some(table_8) = table_8 {
        l = f32::from(table_8.byte(lo as usize)) * (1.0 / 255.0f32);
        h = f32::from(table_8.byte(hi as usize)) * (1.0 / 255.0f32);
    } else {
        let table_16 = table_16.expect("a table curve has either table_8 or table_16");
        // The table holds big-endian 16-bit values.
        let be = |i: i32| {
            let off = 2usize.wrapping_mul(i as usize);
            u16::from_be_bytes([table_16.byte(off), table_16.byte(off.wrapping_add(1))])
        };
        l = f32::from(be(lo)) * (1.0 / 65535.0f32);
        h = f32::from(be(hi)) * (1.0 / 65535.0f32);
    }
    l + (h - l) * t
}

/// The largest error found when round-tripping `curve` through `inv_tf`.
// Port of: modules/skcms/skcms.cc#L334-L344 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(N - 1)
#[must_use]
pub fn max_roundtrip_error(curve: &Curve, inv_tf: &TransferFunction) -> f32 {
    let n = if curve.table_entries() > 256 {
        curve.table_entries()
    } else {
        256
    };
    let dx = 1.0f32 / (n - 1) as f32;
    let mut err = 0.0f32;
    for i in 0..n {
        let x = i as f32 * dx;
        let y = eval_curve(curve, x);
        err = fmaxf_(err, fabsf_(x - inv_tf.eval(y)));
    }
    err
}

/// Practical test that answers: Is `curve` roughly the inverse of `inv_tf`? Typically used by
/// passing the inverse of a known parametric transfer function (like sRGB), to determine if a
/// particular curve is very close to sRGB.
// Port of: modules/skcms/skcms.cc#L346-L348 (chrome/m156)
#[doc(alias = "skcms_AreApproximateInverses")]
#[must_use]
pub fn are_approximate_inverses(curve: &Curve, inv_tf: &TransferFunction) -> bool {
    max_roundtrip_error(curve, inv_tf) < (1.0 / 512.0f32)
}

// If you pass f, we'll fit a possibly-non-zero value for *f.
// If you pass nullptr, we'll assume you want *f to be treated as zero.
// Port of: modules/skcms/skcms.cc#L1241-L1291 (chrome/m156)
#[allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)] // mirrors static_cast<float>(int)
#[allow(clippy::many_single_char_names)] // mirrors the C++ variable names
fn fit_linear(
    curve: &Curve,
    n: i32,
    tol: f32,
    c: &mut f32,
    d: &mut f32,
    f: Option<&mut f32>,
) -> i32 {
    debug_assert!(n > 1);
    // We iteratively fit the first points to the TF's linear piece.
    // We want the cx + f line to pass through the first and last points we fit exactly.
    //
    // As we walk along the points we find the minimum and maximum slope of the line before the
    // error would exceed our tolerance.  We stop when the range [slope_min, slope_max] becomes
    // emtpy, when we definitely can't add any more points.
    //
    // Some points' error intervals may intersect the running interval but not lie fully
    // within it.  So we keep track of the last point we saw that is a valid end point candidate,
    // and once the search is done, back up to build the line through *that* point.
    let dx = 1.0f32 / (n - 1) as f32;

    let mut lin_points = 1;

    let mut f_zero = 0.0f32;
    let f = match f {
        Some(f) => {
            *f = eval_curve(curve, 0.0);
            f
        }
        None => &mut f_zero,
    };

    let mut slope_min = -f32::INFINITY;
    let mut slope_max = f32::INFINITY;
    for i in 1..n {
        let x = i as f32 * dx;
        let y = eval_curve(curve, x);

        let slope_max_i = (y + tol - *f) / x;
        let slope_min_i = (y - tol - *f) / x;
        if slope_max_i < slope_min || slope_max < slope_min_i {
            // Slope intervals would no longer overlap.
            break;
        }
        slope_max = fminf_(slope_max, slope_max_i);
        slope_min = fmaxf_(slope_min, slope_min_i);

        let cur_slope = (y - *f) / x;
        if slope_min <= cur_slope && cur_slope <= slope_max {
            lin_points = i + 1;
            *c = cur_slope;
        }
    }

    // Set D to the last point that met our tolerance.
    *d = (lin_points - 1) as f32 * dx;
    lin_points
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //

// From here below we're approximating an skcms_Curve with an skcms_TransferFunction{g,a,b,c,d,e,f}:
//
//   tf(x) =  cx + f          x < d
//   tf(x) = (ax + b)^g + e   x ≥ d
//
// When fitting, we add the additional constraint that both pieces meet at d:
//
//   cd + f = (ad + b)^g + e
//
// Solving for e and folding it through gives an alternate formulation of the non-linear piece:
//
//   tf(x) =                           cx + f   x < d
//   tf(x) = (ax + b)^g - (ad + b)^g + cd + f   x ≥ d
//
// Our overall strategy is then:
//    For a couple tolerances,
//       - fit_linear():    fit c,d,f iteratively to as many points as our tolerance allows
//       - invert c,d,f
//       - fit_nonlinear(): fit g,a,b using Gauss-Newton given those inverted c,d,f
//                          (and by constraint, inverted e) to the inverse of the table.
//    Return the parameters with least maximum error.
//
// To run Gauss-Newton to find g,a,b, we'll also need the gradient of the residuals
// of round-trip f_inv(x), the inverse of the non-linear piece of f(x).
//
//    let y = Table(x)
//    r(x) = x - f_inv(y)
//
//    ∂r/∂g = ln(ay + b)*(ay + b)^g
//          - ln(ad + b)*(ad + b)^g
//    ∂r/∂a = yg(ay + b)^(g-1)
//          - dg(ad + b)^(g-1)
//    ∂r/∂b =  g(ay + b)^(g-1)
//          -  g(ad + b)^(g-1)

// Return the residual of roundtripping skcms_Curve(x) through f_inv(y) with parameters P,
// and fill out the gradient of the residual into dfdP.
// Port of: modules/skcms/skcms.cc#L2174-L2200 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors the C++ variable names
fn rg_nonlinear(x: f32, curve: &Curve, tf: &TransferFunction, dfdp: &mut [f32; 3]) -> f32 {
    let y = eval_curve(curve, x);

    let (g, a, b, c, d, f) = (tf.g, tf.a, tf.b, tf.c, tf.d, tf.f);

    let big_y = fmaxf_(a * y + b, 0.0);
    let big_d = a * d + b;
    debug_assert!(big_d >= 0.0);

    // The gradient.
    dfdp[0] = logf_(big_y) * powf_(big_y, g) - logf_(big_d) * powf_(big_d, g);
    dfdp[1] = y * g * powf_(big_y, g - 1.0) - d * g * powf_(big_d, g - 1.0);
    dfdp[2] = g * powf_(big_y, g - 1.0) - g * powf_(big_d, g - 1.0);

    // The residual.
    let f_inv = powf_(big_y, g) - powf_(big_d, g) + c * d + f;
    x - f_inv
}

// Port of: modules/skcms/skcms.cc#L2202-L2281 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(i)
fn gauss_newton_step(curve: &Curve, tf: &mut TransferFunction, x0: f32, dx: f32, n: i32) -> bool {
    // We'll sample x from the range [x0,x1] (both inclusive) N times with even spacing.
    //
    // Let P = [ tf->g, tf->a, tf->b ] (the three terms that we're adjusting).
    //
    // We want to do P' = P + (Jf^T Jf)^-1 Jf^T r(P),
    //   where r(P) is the residual vector
    //   and Jf is the Jacobian matrix of f(), ∂r/∂P.
    //
    // Let's review the shape of each of these expressions:
    //   r(P)   is [N x 1], a column vector with one entry per value of x tested
    //   Jf     is [N x 3], a matrix with an entry for each (x,P) pair
    //   Jf^T   is [3 x N], the transpose of Jf
    //
    //   Jf^T Jf   is [3 x N] * [N x 3] == [3 x 3], a 3x3 matrix,
    //                                              and so is its inverse (Jf^T Jf)^-1
    //   Jf^T r(P) is [3 x N] * [N x 1] == [3 x 1], a column vector with the same shape as P
    //
    // Our implementation strategy to get to the final ∆P is
    //   1) evaluate Jf^T Jf,   call that lhs
    //   2) evaluate Jf^T r(P), call that rhs
    //   3) invert lhs
    //   4) multiply inverse lhs by rhs
    //
    // This is a friendly implementation strategy because we don't have to have any
    // buffers that scale with N, and equally nice don't have to perform any matrix
    // operations that are variable size.
    //
    // Other implementation strategies could trade this off, e.g. evaluating the
    // pseudoinverse of Jf ( (Jf^T Jf)^-1 Jf^T ) directly, then multiplying that by
    // the residuals.  That would probably require implementing singular value
    // decomposition, and would create a [3 x N] matrix to be multiplied by the
    // [N x 1] residual vector, but on the upside I think that'd eliminate the
    // possibility of this gauss_newton_step() function ever failing.

    // 0) start off with lhs and rhs safely zeroed.
    let mut lhs = Matrix3x3 {
        vals: [[0.0; 3]; 3],
    };
    let mut rhs = Vector3 { vals: [0.0; 3] };

    // 1,2) evaluate lhs and evaluate rhs
    //   We want to evaluate Jf only once, but both lhs and rhs involve Jf^T,
    //   so we'll have to update lhs and rhs at the same time.
    for i in 0..n {
        let x = x0 + i as f32 * dx;

        let mut dfdp = [0.0f32; 3];
        let resid = rg_nonlinear(x, curve, tf, &mut dfdp);

        for r in 0..3 {
            for c in 0..3 {
                lhs.vals[r][c] += dfdp[r] * dfdp[c];
            }
            rhs.vals[r] += dfdp[r] * resid;
        }
    }

    // If any of the 3 P parameters are unused, this matrix will be singular.
    // Detect those cases and fix them up to indentity instead, so we can invert.
    #[allow(clippy::float_cmp)] // mirrors the C++ exact zero tests
    for k in 0..3 {
        if lhs.vals[0][k] == 0.0
            && lhs.vals[1][k] == 0.0
            && lhs.vals[2][k] == 0.0
            && lhs.vals[k][0] == 0.0
            && lhs.vals[k][1] == 0.0
            && lhs.vals[k][2] == 0.0
        {
            lhs.vals[k][k] = 1.0;
        }
    }

    // 3) invert lhs
    let Some(lhs_inv) = lhs.invert() else {
        return false;
    };

    // 4) multiply inverse lhs by rhs
    let dp = mv_mul(&lhs_inv, &rhs);
    tf.g += dp.vals[0];
    tf.a += dp.vals[1];
    tf.b += dp.vals[2];
    isfinitef_(tf.g) && isfinitef_(tf.a) && isfinitef_(tf.b)
}

// Port of: modules/skcms/skcms.cc#L2283-L2296 (chrome/m156)
fn max_roundtrip_error_checked(curve: &Curve, tf_inv: &TransferFunction) -> f32 {
    let Some(tf) = tf_inv.invert() else {
        return f32::INFINITY;
    };
    if tf.tf_type() != TfType::SRGBish {
        return f32::INFINITY;
    }

    let Some(tf_inv_again) = tf.invert() else {
        return f32::INFINITY;
    };

    max_roundtrip_error(curve, &tf_inv_again)
}

// Fit the points in [L,N) to the non-linear piece of tf, or return false if we can't.
// Port of: modules/skcms/skcms.cc#L2299-L2356 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(int)
fn fit_nonlinear(curve: &Curve, l: i32, n: i32, tf: &mut TransferFunction) -> bool {
    // This enforces a few constraints that are not modeled in gauss_newton_step()'s optimization.
    let fixup_tf = |tf: &mut TransferFunction| -> bool {
        // a must be non-negative. That ensures the function is monotonically increasing.
        // We don't really know how to fix up a if it goes negative.
        if tf.a < 0.0 {
            return false;
        }
        // ad+b must be non-negative. That ensures we don't end up with complex numbers in powf.
        // We feel just barely not uneasy enough to tweak b so ad+b is zero in this case.
        if tf.a * tf.d + tf.b < 0.0 {
            tf.b = -tf.a * tf.d;
        }
        debug_assert!(tf.a >= 0.0 && tf.a * tf.d + tf.b >= 0.0);

        // cd+f must be ~= (ad+b)^g+e. That ensures the function is continuous. We keep e as a free
        // parameter so we can guarantee this.
        tf.e = tf.c * tf.d + tf.f - powf_(tf.a * tf.d + tf.b, tf.g);

        isfinitef_(tf.e)
    };

    if !fixup_tf(tf) {
        return false;
    }

    // No matter where we start, dx should always represent N even steps from 0 to 1.
    let dx = 1.0f32 / (n - 1) as f32;

    let mut best_tf = *tf;
    let mut best_max_error = f32::INFINITY;

    // Need this or several curves get worse... *sigh*
    let init_error = max_roundtrip_error_checked(curve, tf);
    if init_error < best_max_error {
        best_max_error = init_error;
        best_tf = *tf;
    }

    // As far as we can tell, 1 Gauss-Newton step won't converge, and 3 steps is no better than 2.
    for _ in 0..8 {
        if !gauss_newton_step(curve, tf, l as f32 * dx, dx, n - l) || !fixup_tf(tf) {
            *tf = best_tf;
            return isfinitef_(best_max_error);
        }

        let max_error = max_roundtrip_error_checked(curve, tf);
        if max_error < best_max_error {
            best_max_error = max_error;
            best_tf = *tf;
        }
    }

    *tf = best_tf;
    isfinitef_(best_max_error)
}

/// Approximates a table-based `curve` with a parametric [`TransferFunction`]. On success,
/// returns the approximation and its maximum round-trip error.
// Port of: modules/skcms/skcms.cc#L2358-L2457 (chrome/m156)
#[doc(alias = "skcms_ApproximateCurve")]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)] // mirrors static_cast<float>(int)
#[must_use]
pub fn approximate_curve(curve: &Curve) -> Option<(TransferFunction, f32)> {
    if curve.table_entries() == 0 {
        // No point approximating an skcms_TransferFunction with an skcms_TransferFunction!
        return None;
    }

    if curve.table_entries() == 1 || curve.table_entries() > MAX_TABLE_ENTRIES {
        // We need at least two points, and must put some reasonable cap on the maximum number.
        return None;
    }

    let n = curve.table_entries() as i32;
    let dx = 1.0f32 / (n - 1) as f32;

    let mut max_error = f32::INFINITY;
    let mut approx = TransferFunction::default();
    let k_tolerances = [1.5f32 / 65535.0f32, 1.0f32 / 512.0f32];
    for &tolerance in &k_tolerances {
        // It's problematic to fit curves with non-zero f, so always force it to zero explicitly.
        let mut tf = TransferFunction {
            f: 0.0,
            ..TransferFunction::default()
        };
        let (mut c, mut d) = (0.0f32, 0.0f32);
        let l = fit_linear(curve, n, tolerance, &mut c, &mut d, None);
        tf.c = c;
        tf.d = d;

        if l == n {
            // If the entire data set was linear, move the coefficients to the nonlinear portion
            // with G == 1.  This lets use a canonical representation with d == 0.
            tf.g = 1.0;
            tf.a = tf.c;
            tf.b = tf.f;
            tf.c = 0.0;
            tf.d = 0.0;
            tf.e = 0.0;
            tf.f = 0.0;
        } else if l == n - 1 {
            // Degenerate case with only two points in the nonlinear segment. Solve directly.
            tf.g = 1.0;
            tf.a = (eval_curve(curve, (n - 1) as f32 * dx)
                - eval_curve(curve, (n - 2) as f32 * dx))
                / dx;
            tf.b = eval_curve(curve, (n - 2) as f32 * dx) - tf.a * (n - 2) as f32 * dx;
            tf.e = 0.0;
        } else {
            // Start by guessing a gamma-only curve through the midpoint.
            #[allow(clippy::manual_midpoint)] // mirrors (L + N) / 2
            let mid = (l + n) / 2;
            let mid_x = mid as f32 / (n - 1) as f32;
            let mid_y = eval_curve(curve, mid_x);
            tf.g = log2f_(mid_y) / log2f_(mid_x);
            tf.a = 1.0;
            tf.b = 0.0;
            tf.e = tf.c * tf.d + tf.f - powf_(tf.a * tf.d + tf.b, tf.g);

            let Some(mut tf_inv) = tf.invert() else {
                continue;
            };
            if !fit_nonlinear(curve, l, n, &mut tf_inv) {
                continue;
            }

            // We fit tf_inv, so calculate tf to keep in sync.
            // fit_nonlinear() should guarantee invertibility.
            let Some(back) = tf_inv.invert() else {
                debug_assert!(false);
                continue;
            };
            tf = back;
        }

        // We'd better have a sane, sRGB-ish TF by now.
        // Other non-Bad TFs would be fine, but we know we've only ever tried to fit sRGBish;
        // anything else is just some accident of math and the way we pun tf.g as a type flag.
        // fit_nonlinear() should guarantee this, but the special cases may fail this test.
        if tf.tf_type() != TfType::SRGBish {
            continue;
        }

        // We find our error by roundtripping the table through tf_inv.
        //
        // (The most likely use case for this approximation is to be inverted and
        // used as the transfer function for a destination color space.)
        //
        // We've kept tf and tf_inv in sync above, but we can't guarantee that tf is
        // invertible, so re-verify that here (and use the new inverse for testing).
        // fit_nonlinear() should guarantee this, but the special cases that don't use
        // it may fail this test.
        let Some(tf_inv) = tf.invert() else {
            continue;
        };

        let err = max_roundtrip_error(curve, &tf_inv);
        if max_error > err {
            max_error = err;
            approx = tf;
        }
    }
    if isfinitef_(max_error) {
        Some((approx, max_error))
    } else {
        None
    }
}
