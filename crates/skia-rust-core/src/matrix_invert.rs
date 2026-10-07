// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkMatrixInvert.h, src/core/SkMatrixInvert.cpp

//! Inversion of 2x2, 3x3 and 4x4 matrices in column-major order (`SkMatrixInvert.h`).
//!
//! The calculations are always performed in doubles, to avoid prematurely losing precision.
//! Each function returns the determinant of the input matrix; if zero is returned, the matrix
//! was non-invertible, and the output was left in an indeterminate state. Passing `None` for
//! the output only computes the determinant.

use crate::floating_point::{double_to_float, ieee_double_divide, is_finite_array};
use crate::scalar::scalar;

/// Computes the inverse of the 2x2 matrix `in_matrix` into `out_matrix`.
// Port of: src/core/SkMatrixInvert.cpp#L12-L35 (chrome/m156)
#[doc(alias = "SkInvert2x2Matrix")]
#[must_use]
pub fn invert_2x2_matrix(in_matrix: &[scalar; 4], out_matrix: Option<&mut [scalar; 4]>) -> scalar {
    let a00 = f64::from(in_matrix[0]);
    let a01 = f64::from(in_matrix[1]);
    let a10 = f64::from(in_matrix[2]);
    let a11 = f64::from(in_matrix[3]);

    // Calculate the determinant
    let mut determinant = a00 * a11 - a01 * a10;
    if let Some(out_matrix) = out_matrix {
        let invdet = ieee_double_divide(1.0, determinant);
        out_matrix[0] = double_to_float(a11 * invdet);
        out_matrix[1] = double_to_float(-a01 * invdet);
        out_matrix[2] = double_to_float(-a10 * invdet);
        out_matrix[3] = double_to_float(a00 * invdet);
        // If 1/det overflows to infinity (i.e. det is denormalized) or any of the inverted matrix
        // values is non-finite, return zero to indicate a non-invertible matrix.
        if !is_finite_array(&out_matrix[..]) {
            determinant = 0.0;
        }
    }
    double_to_float(determinant)
}

/// Computes the inverse of the 3x3 matrix `in_matrix` into `out_matrix`.
// Port of: src/core/SkMatrixInvert.cpp#L37-L78 (chrome/m156)
#[doc(alias = "SkInvert3x3Matrix")]
#[must_use]
pub fn invert_3x3_matrix(in_matrix: &[scalar; 9], out_matrix: Option<&mut [scalar; 9]>) -> scalar {
    let a00 = f64::from(in_matrix[0]);
    let a01 = f64::from(in_matrix[1]);
    let a02 = f64::from(in_matrix[2]);
    let a10 = f64::from(in_matrix[3]);
    let a11 = f64::from(in_matrix[4]);
    let a12 = f64::from(in_matrix[5]);
    let a20 = f64::from(in_matrix[6]);
    let a21 = f64::from(in_matrix[7]);
    let a22 = f64::from(in_matrix[8]);

    let b01 = a22 * a11 - a12 * a21;
    let b11 = -a22 * a10 + a12 * a20;
    let b21 = a21 * a10 - a11 * a20;

    // Calculate the determinant
    let mut determinant = a00 * b01 + a01 * b11 + a02 * b21;
    if let Some(out_matrix) = out_matrix {
        let invdet = ieee_double_divide(1.0, determinant);
        out_matrix[0] = double_to_float(b01 * invdet);
        out_matrix[1] = double_to_float((-a22 * a01 + a02 * a21) * invdet);
        out_matrix[2] = double_to_float((a12 * a01 - a02 * a11) * invdet);
        out_matrix[3] = double_to_float(b11 * invdet);
        out_matrix[4] = double_to_float((a22 * a00 - a02 * a20) * invdet);
        out_matrix[5] = double_to_float((-a12 * a00 + a02 * a10) * invdet);
        out_matrix[6] = double_to_float(b21 * invdet);
        out_matrix[7] = double_to_float((-a21 * a00 + a01 * a20) * invdet);
        out_matrix[8] = double_to_float((a11 * a00 - a01 * a10) * invdet);

        // If 1/det overflows to infinity (i.e. det is denormalized) or any of the inverted matrix
        // values is non-finite, return zero to indicate a non-invertible matrix.
        if !is_finite_array(&out_matrix[..]) {
            determinant = 0.0;
        }
    }
    double_to_float(determinant)
}

/// Computes the inverse of the 4x4 matrix `in_matrix` into `out_matrix`.
// Port of: src/core/SkMatrixInvert.cpp#L80-L166 (chrome/m156)
#[doc(alias = "SkInvert4x4Matrix")]
#[must_use]
#[allow(clippy::many_single_char_names, clippy::similar_names)] // names follow the C++
pub fn invert_4x4_matrix(
    in_matrix: &[scalar; 16],
    out_matrix: Option<&mut [scalar; 16]>,
) -> scalar {
    let a00 = f64::from(in_matrix[0]);
    let a01 = f64::from(in_matrix[1]);
    let a02 = f64::from(in_matrix[2]);
    let a03 = f64::from(in_matrix[3]);
    let a10 = f64::from(in_matrix[4]);
    let a11 = f64::from(in_matrix[5]);
    let a12 = f64::from(in_matrix[6]);
    let a13 = f64::from(in_matrix[7]);
    let a20 = f64::from(in_matrix[8]);
    let a21 = f64::from(in_matrix[9]);
    let a22 = f64::from(in_matrix[10]);
    let a23 = f64::from(in_matrix[11]);
    let a30 = f64::from(in_matrix[12]);
    let a31 = f64::from(in_matrix[13]);
    let a32 = f64::from(in_matrix[14]);
    let a33 = f64::from(in_matrix[15]);

    let mut b00 = a00 * a11 - a01 * a10;
    let mut b01 = a00 * a12 - a02 * a10;
    let mut b02 = a00 * a13 - a03 * a10;
    let mut b03 = a01 * a12 - a02 * a11;
    let mut b04 = a01 * a13 - a03 * a11;
    let mut b05 = a02 * a13 - a03 * a12;
    let mut b06 = a20 * a31 - a21 * a30;
    let mut b07 = a20 * a32 - a22 * a30;
    let mut b08 = a20 * a33 - a23 * a30;
    let mut b09 = a21 * a32 - a22 * a31;
    let mut b10 = a21 * a33 - a23 * a31;
    let mut b11 = a22 * a33 - a23 * a32;

    // Calculate the determinant
    let mut determinant = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;

    if let Some(out_matrix) = out_matrix {
        let invdet = ieee_double_divide(1.0, determinant);
        b00 *= invdet;
        b01 *= invdet;
        b02 *= invdet;
        b03 *= invdet;
        b04 *= invdet;
        b05 *= invdet;
        b06 *= invdet;
        b07 *= invdet;
        b08 *= invdet;
        b09 *= invdet;
        b10 *= invdet;
        b11 *= invdet;

        out_matrix[0] = double_to_float(a11 * b11 - a12 * b10 + a13 * b09);
        out_matrix[1] = double_to_float(a02 * b10 - a01 * b11 - a03 * b09);
        out_matrix[2] = double_to_float(a31 * b05 - a32 * b04 + a33 * b03);
        out_matrix[3] = double_to_float(a22 * b04 - a21 * b05 - a23 * b03);
        out_matrix[4] = double_to_float(a12 * b08 - a10 * b11 - a13 * b07);
        out_matrix[5] = double_to_float(a00 * b11 - a02 * b08 + a03 * b07);
        out_matrix[6] = double_to_float(a32 * b02 - a30 * b05 - a33 * b01);
        out_matrix[7] = double_to_float(a20 * b05 - a22 * b02 + a23 * b01);
        out_matrix[8] = double_to_float(a10 * b10 - a11 * b08 + a13 * b06);
        out_matrix[9] = double_to_float(a01 * b08 - a00 * b10 - a03 * b06);
        out_matrix[10] = double_to_float(a30 * b04 - a31 * b02 + a33 * b00);
        out_matrix[11] = double_to_float(a21 * b02 - a20 * b04 - a23 * b00);
        out_matrix[12] = double_to_float(a11 * b07 - a10 * b09 - a12 * b06);
        out_matrix[13] = double_to_float(a00 * b09 - a01 * b07 + a02 * b06);
        out_matrix[14] = double_to_float(a31 * b01 - a30 * b03 - a32 * b00);
        out_matrix[15] = double_to_float(a20 * b03 - a21 * b01 + a22 * b00);

        // If 1/det overflows to infinity (i.e. det is denormalized) or any of the inverted matrix
        // values is non-finite, return zero to indicate a non-invertible matrix.
        if !is_finite_array(&out_matrix[..]) {
            determinant = 0.0;
        }
    }
    double_to_float(determinant)
}
