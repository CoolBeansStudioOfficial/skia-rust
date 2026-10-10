// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/SkColorMatrix.h, src/effects/SkColorMatrix.cpp
//

//! `SkColorMatrix`: a 5x4 color matrix (row-major, 4 rows of 5 coefficients), used by the matrix
//! color filters.

use crate::image_info::YUVColorSpace;

/// Index of the red scale coefficient (`kR_Scale`).
const R_SCALE: usize = 0;
/// Index of the green scale coefficient (`kG_Scale`).
const G_SCALE: usize = 6;
/// Index of the blue scale coefficient (`kB_Scale`).
const B_SCALE: usize = 12;
/// Index of the alpha scale coefficient (`kA_Scale`).
const A_SCALE: usize = 18;
/// Index of the red translation (`kR_Trans`).
const R_TRANS: usize = 4;
/// Index of the green translation (`kG_Trans`).
const G_TRANS: usize = 9;
/// Index of the blue translation (`kB_Trans`).
const B_TRANS: usize = 14;
/// Index of the alpha translation (`kA_Trans`).
const A_TRANS: usize = 19;

/// Luminance-like weights used by `setSaturation` (`kHueR`, `kHueG`, `kHueB`).
// Port of: src/effects/SkColorMatrix.cpp#L101-L103 (chrome/m156)
const HUE_R: f32 = 0.213;
const HUE_G: f32 = 0.715;
const HUE_B: f32 = 0.072;

/// A 5x4 color matrix (`SkColorMatrix`): `fMat` in row-major order.
// Port of: include/effects/SkColorMatrix.h#L18-L55 (chrome/m156)
#[doc(alias = "SkColorMatrix")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorMatrix {
    mat: [f32; 20],
}

impl Default for ColorMatrix {
    /// The identity matrix.
    fn default() -> Self {
        Self::new(
            1.0, 0.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 0.0, 1.0, 0.0,
        )
    }
}

impl ColorMatrix {
    /// Builds a matrix from its 20 coefficients, in row-major order.
    #[doc(alias = "SkColorMatrix")]
    #[allow(clippy::too_many_arguments)] // mirrors SkColorMatrix's 20-argument constructor
    #[must_use]
    pub const fn new(
        m00: f32,
        m01: f32,
        m02: f32,
        m03: f32,
        m04: f32,
        m10: f32,
        m11: f32,
        m12: f32,
        m13: f32,
        m14: f32,
        m20: f32,
        m21: f32,
        m22: f32,
        m23: f32,
        m24: f32,
        m30: f32,
        m31: f32,
        m32: f32,
        m33: f32,
        m34: f32,
    ) -> Self {
        Self {
            mat: [
                m00, m01, m02, m03, m04, m10, m11, m12, m13, m14, m20, m21, m22, m23, m24, m30,
                m31, m32, m33, m34,
            ],
        }
    }

    /// Sets the matrix to the identity (`setIdentity`).
    // Port of: src/effects/SkColorMatrix.cpp#L67-L70 (chrome/m156)
    #[doc(alias = "setIdentity")]
    pub fn set_identity(&mut self) {
        self.mat = [0.0; 20];
        self.mat[R_SCALE] = 1.0;
        self.mat[G_SCALE] = 1.0;
        self.mat[B_SCALE] = 1.0;
        self.mat[A_SCALE] = 1.0;
    }

    /// Sets the matrix to a diagonal scale (`setScale`).
    // Port of: src/effects/SkColorMatrix.cpp#L72-L78 (chrome/m156)
    #[doc(alias = "setScale")]
    pub fn set_scale(&mut self, r_scale: f32, g_scale: f32, b_scale: f32, a_scale: f32) {
        self.mat = [0.0; 20];
        self.mat[R_SCALE] = r_scale;
        self.mat[G_SCALE] = g_scale;
        self.mat[B_SCALE] = b_scale;
        self.mat[A_SCALE] = a_scale;
    }

    /// Adds a translation to the matrix's offset column (`postTranslate`).
    // Port of: src/effects/SkColorMatrix.cpp#L80-L85 (chrome/m156)
    #[doc(alias = "postTranslate")]
    pub fn post_translate(&mut self, dr: f32, dg: f32, db: f32, da: f32) {
        self.mat[R_TRANS] += dr;
        self.mat[G_TRANS] += dg;
        self.mat[B_TRANS] += db;
        self.mat[A_TRANS] += da;
    }

    /// Sets `self` to `a * b` (`setConcat`).
    // Port of: src/effects/SkColorMatrix.cpp#L89-L91 (chrome/m156)
    #[doc(alias = "setConcat")]
    pub fn set_concat(&mut self, a: &ColorMatrix, b: &ColorMatrix) {
        set_concat(&mut self.mat, &a.mat, &b.mat);
    }

    /// `self = self * mat` (`preConcat`).
    // Port of: include/effects/SkColorMatrix.h#L43 (chrome/m156)
    #[doc(alias = "preConcat")]
    pub fn pre_concat(&mut self, mat: &ColorMatrix) {
        let this = *self;
        self.set_concat(&this, mat);
    }

    /// `self = mat * self` (`postConcat`).
    // Port of: include/effects/SkColorMatrix.h#L44 (chrome/m156)
    #[doc(alias = "postConcat")]
    pub fn post_concat(&mut self, mat: &ColorMatrix) {
        let this = *self;
        self.set_concat(mat, &this);
    }

    /// Sets the matrix to a saturation change (`setSaturation`).
    // Port of: src/effects/SkColorMatrix.cpp#L105-L116 (chrome/m156)
    #[doc(alias = "setSaturation")]
    pub fn set_saturation(&mut self, sat: f32) {
        self.mat = [0.0; 20];
        let r = HUE_R * (1.0 - sat);
        let g = HUE_G * (1.0 - sat);
        let b = HUE_B * (1.0 - sat);
        setrow(&mut self.mat, 0, r + sat, g, b);
        setrow(&mut self.mat, 5, r, g + sat, b);
        setrow(&mut self.mat, 10, r, g, b + sat);
        self.mat[A_SCALE] = 1.0;
    }

    /// Replaces the matrix with 20 row-major coefficients (`setRowMajor`).
    #[doc(alias = "setRowMajor")]
    pub fn set_row_major(&mut self, src: &[f32; 20]) {
        self.mat = *src;
    }

    /// Copies the 20 row-major coefficients out (`getRowMajor`).
    #[doc(alias = "getRowMajor")]
    pub fn get_row_major(&self, dst: &mut [f32; 20]) {
        *dst = self.mat;
    }

    /// The coefficients, row-major. Crate-internal: the filters copy them into the pipeline.
    pub(crate) fn as_row_major(&self) -> &[f32; 20] {
        &self.mat
    }
}

/// Writes `r`, `g`, `b` into the three coefficients of row `start` (`setrow`).
// Port of: src/effects/SkColorMatrix.cpp#L95-L99 (chrome/m156)
fn setrow(row: &mut [f32; 20], start: usize, r: f32, g: f32, b: f32) {
    row[start] = r;
    row[start + 1] = g;
    row[start + 2] = b;
}

/// `result = outer * inner` for two row-major 5x4 matrices (`set_concat`).
///
/// The sums are evaluated left to right, as in the C++, and the result is written only after
/// both inputs are read, so `result` may alias either input.
// Port of: src/effects/SkColorMatrix.cpp#L37-L65 (chrome/m156)
fn set_concat(result: &mut [f32; 20], outer: &[f32; 20], inner: &[f32; 20]) {
    let mut target = [0.0_f32; 20];
    let mut index = 0;
    for j in (0..20).step_by(5) {
        for i in 0..4 {
            target[index] = outer[j] * inner[i]
                + outer[j + 1] * inner[i + 5]
                + outer[j + 2] * inner[i + 10]
                + outer[j + 3] * inner[i + 15];
            index += 1;
        }
        target[index] = outer[j] * inner[4]
            + outer[j + 1] * inner[9]
            + outer[j + 2] * inner[14]
            + outer[j + 3] * inner[19]
            + outer[j + 4];
        index += 1;
    }
    *result = target;
}

impl ColorMatrix {
    /// The RGB to YUV matrix of a colour space (`SkColorMatrix::RGBtoYUV`).
    // Port of: src/effects/SkColorMatrix.cpp#L13-L17 (chrome/m156)
    #[doc(alias = "SkColorMatrix::RGBtoYUV")]
    #[must_use]
    pub fn rgb_to_yuv(cs: YUVColorSpace) -> Self {
        Self {
            mat: crate::yuv_math::color_matrix_rgb2yuv(cs),
        }
    }

    /// The YUV to RGB matrix of a colour space (`SkColorMatrix::YUVtoRGB`).
    // Port of: src/effects/SkColorMatrix.cpp#L19-L23 (chrome/m156)
    #[doc(alias = "SkColorMatrix::YUVtoRGB")]
    #[must_use]
    pub fn yuv_to_rgb(cs: YUVColorSpace) -> Self {
        Self {
            mat: crate::yuv_math::color_matrix_yuv2rgb(cs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_the_default() {
        let mut m = ColorMatrix { mat: [0.5; 20] };
        m.set_identity();
        assert_eq!(m, ColorMatrix::default());
    }
}
