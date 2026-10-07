// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkMatrixUtils.h, src/core/SkMatrix.cpp

//! Matrix helpers declared in `SkMatrixUtils.h`.
//!
//! `SkTreatAsSprite` is not ported yet: it needs `SkSamplingOptions`.

use crate::matrix::{Matrix, Member, is_degenerate_2x2};
use crate::point::Point;
use crate::scalar::{Scalar, double_to_scalar, scalar, scalar_invert, scalar_sqrt};

/// Decomposes the upper-left 2x2 of the matrix into a rotation (represented by the cosine and
/// sine of the rotation angle), followed by a non-uniform scale, followed by another rotation.
/// If there is a reflection, one of the scale factors will be negative.
///
/// Returns true if successful. Returns false if the matrix is degenerate.
///
/// A square matrix M can be decomposed (via polar decomposition) into two matrices -- an
/// orthogonal matrix Q and a symmetric matrix S. In turn we can decompose S into U*W*U^T, where
/// U is another orthogonal matrix and W is a scale matrix. These can be recombined to give
/// M = (Q*U)*W*U^T, i.e., the product of two orthogonal matrices and a scale matrix.
///
/// The one wrinkle is that traditionally Q may contain a reflection -- the calculation has been
/// rejiggered to put that reflection into W.
// Port of: src/core/SkMatrix.cpp#L1603-L1703 (chrome/m156)
#[doc(alias = "SkDecomposeUpper2x2")]
#[allow(clippy::many_single_char_names)] // names follow the C++
#[allow(clippy::manual_midpoint)] // `0.5*(trace + discriminant)` must be evaluated exactly like this
pub fn decompose_upper_2x2(
    matrix: &Matrix,
    rotation1: Option<&mut Point>,
    scale: Option<&mut Point>,
    rotation2: Option<&mut Point>,
) -> bool {
    let a = matrix[Member::ScaleX];
    let b = matrix[Member::SkewX];
    let c = matrix[Member::SkewY];
    let d = matrix[Member::ScaleY];

    if is_degenerate_2x2(a, b, c, d) {
        return false;
    }

    let w1: f64;
    let w2: f64;
    let mut cos1: scalar;
    let mut sin1: scalar;
    let cos2: scalar;
    let sin2: scalar;

    // do polar decomposition (M = Q*S)
    let cos_q: scalar;
    let sin_q: scalar;
    let sa: f64;
    let sb: f64;
    let sd: f64;
    // if M is already symmetric (i.e., M = I*S)
    if scalar::nearly_equal(b, c, None) {
        cos_q = 1.0;
        sin_q = 0.0;

        sa = f64::from(a);
        sb = f64::from(b);
        sd = f64::from(d);
    } else {
        let mut cq = a + d;
        let mut sq = c - b;
        let reciplen = scalar_invert(scalar_sqrt(cq * cq + sq * sq));
        cq *= reciplen;
        sq *= reciplen;
        cos_q = cq;
        sin_q = sq;

        // S = Q^-1*M
        // we don't calc Sc since it's symmetric
        // (the products are evaluated in float, as in C++)
        sa = f64::from(a * cos_q + c * sin_q);
        sb = f64::from(b * cos_q + d * sin_q);
        sd = f64::from(-b * sin_q + d * cos_q);
    }

    // Now we need to compute eigenvalues of S (our scale factors)
    // and eigenvectors (bases for our rotation)
    // From this, should be able to reconstruct S as U*W*U^T
    if double_to_scalar(sb).nearly_zero(None) {
        // already diagonalized
        cos1 = 1.0;
        sin1 = 0.0;
        w1 = sa;
        w2 = sd;
        cos2 = cos_q;
        sin2 = sin_q;
    } else {
        let diff = sa - sd;
        let discriminant = (diff * diff + 4.0 * sb * sb).sqrt();
        let trace = sa + sd;
        if diff > 0.0 {
            w1 = 0.5 * (trace + discriminant);
            w2 = 0.5 * (trace - discriminant);
        } else {
            w1 = 0.5 * (trace - discriminant);
            w2 = 0.5 * (trace + discriminant);
        }

        cos1 = double_to_scalar(sb);
        sin1 = double_to_scalar(w1 - sa);
        let reciplen = scalar_invert(scalar_sqrt(cos1 * cos1 + sin1 * sin1));
        cos1 *= reciplen;
        sin1 *= reciplen;

        // rotation 2 is composition of Q and U
        cos2 = cos1 * cos_q - sin1 * sin_q;
        sin2 = sin1 * cos_q + cos1 * sin_q;

        // rotation 1 is U^T
        sin1 = -sin1;
    }

    if let Some(scale) = scale {
        scale.x = double_to_scalar(w1);
        scale.y = double_to_scalar(w2);
    }
    if let Some(rotation1) = rotation1 {
        rotation1.x = cos1;
        rotation1.y = sin1;
    }
    if let Some(rotation2) = rotation2 {
        rotation2.x = cos2;
        rotation2.y = sin2;
    }

    true
}
