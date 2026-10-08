// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkMatrix.h, src/core/SkMatrix.cpp

//! A 3x3 matrix for transforming coordinates (`SkMatrix.h`).
//!
//! [`Matrix`] holds a 3x3 matrix for transforming coordinates. This allows mapping [`Point`] and
//! vectors with translation, scaling, skewing, rotation, and perspective.
//!
//! Matrix elements are in row-major order. [`Matrix`] default constructs to identity.
//!
//! [`Matrix`] includes a hidden variable that classifies the type of matrix to improve
//! performance, like `SkMatrix`'s `fTypeMask`. The cache is updated through `&self` (as in C++,
//! where it is `mutable`), so [`Matrix`] stores it in an atomic: it is `Send + Sync`, but it is
//! `Clone` rather than `Copy`.
//!
//! # Not ported yet
//! * `SkMatrix::mapRect` for matrices with perspective needs `SkPathBuilder::transform` and
//!   `SkPathPriv::PerspectiveClip` (`SkPath`, `SkEdgeClipper`); [`Matrix::map_rect`] panics there.
//! * `SkMatrix::dump` needs `SkString` / `SkDebugf`.
//! * `SkTreatAsSprite` needs `SkSamplingOptions`.

use crate::float_bits::float_as_2s_compliment;
use crate::floating_point::{double_to_float, ieee_float_divide, is_finite, is_finite_all};
use crate::floating_point::{float_radians_to_degrees, is_finite_array};
use crate::path_builder::PathBuilder;
use crate::point::{Point, Vector};
use crate::point3::Point3;
use crate::rect::Rect;
use crate::rsxform::RSXform;
use crate::scalar::{
    SCALAR_MAX, SCALAR_NEARLY_ZERO, Scalar, degrees_to_radians, double_to_scalar, scalar,
    scalar_abs, scalar_cos_snap_to_zero, scalar_invert, scalar_sin_snap_to_zero, scalar_sqrt,
    scalar_square,
};
use crate::size::Size;
use bitflags::bitflags;
use skia_rust_simd::vx::{Float4, shuffle};
use std::ops::{Index, IndexMut, Mul};
use std::sync::atomic::{AtomicU32, Ordering};

bitflags! {
    /// Bit masks describing the transformations a [`Matrix`] performs
    /// (`SkMatrix::TypeMask`).
    ///
    /// The identity matrix has no bits set.
    // Port of: include/core/SkMatrix.h#L165-L171 (chrome/m156)
    #[doc(alias = "SkMatrix::TypeMask")]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TypeMask: u32 {
        /// Identity matrix; all bits clear (`kIdentity_Mask`).
        const IDENTITY = 0;
        /// Translation matrix (`kTranslate_Mask`).
        const TRANSLATE = 0x01;
        /// Scale matrix (`kScale_Mask`).
        const SCALE = 0x02;
        /// Skew or rotate matrix (`kAffine_Mask`).
        const AFFINE = 0x04;
        /// Perspective matrix (`kPerspective_Mask`).
        const PERSPECTIVE = 0x08;
    }
}

/// How [`Matrix::rect_2_rect`] fits the source rectangle into the destination
/// (`SkMatrix::ScaleToFit`).
// Port of: include/core/SkMatrix.h#L129-L134 (chrome/m156)
#[doc(alias = "SkMatrix::ScaleToFit")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ScaleToFit {
    /// Scales in x and y to fill the destination rectangle (`kFill_ScaleToFit`).
    Fill,
    /// Scales and aligns to left and top (`kStart_ScaleToFit`).
    Start,
    /// Scales and aligns to center (`kCenter_ScaleToFit`).
    Center,
    /// Scales and aligns to right and bottom (`kEnd_ScaleToFit`).
    End,
}

/// Indices of the nine matrix members, in row-major order (`SkMatrix::kMScaleX`, ...).
// Port of: include/core/SkMatrix.h#L316-L324 (chrome/m156)
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Member {
    /// Horizontal scale factor (`kMScaleX`).
    ScaleX = 0,
    /// Horizontal skew factor (`kMSkewX`).
    SkewX = 1,
    /// Horizontal translation (`kMTransX`).
    TransX = 2,
    /// Vertical skew factor (`kMSkewY`).
    SkewY = 3,
    /// Vertical scale factor (`kMScaleY`).
    ScaleY = 4,
    /// Vertical translation (`kMTransY`).
    TransY = 5,
    /// Input x perspective factor (`kMPersp0`).
    Persp0 = 6,
    /// Input y perspective factor (`kMPersp1`).
    Persp1 = 7,
    /// Perspective bias (`kMPersp2`).
    Persp2 = 8,
}

impl From<Member> for usize {
    fn from(m: Member) -> Self {
        m as usize
    }
}

/// Indices of the six members of the affine form, in the order of [`Matrix::to_affine`]
/// (`SkMatrix::kAScaleX`, ...).
// Port of: include/core/SkMatrix.h#L326-L331 (chrome/m156)
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum AffineMember {
    /// Horizontal scale factor (`kAScaleX`).
    ScaleX = 0,
    /// Vertical skew factor (`kASkewY`).
    SkewY = 1,
    /// Horizontal skew factor (`kASkewX`).
    SkewX = 2,
    /// Vertical scale factor (`kAScaleY`).
    ScaleY = 3,
    /// Horizontal translation (`kATransX`).
    TransX = 4,
    /// Vertical translation (`kATransY`).
    TransY = 5,
}

impl From<AffineMember> for usize {
    fn from(m: AffineMember) -> Self {
        m as usize
    }
}

// Private type-mask bits (`SkMatrix` private constants).
// Port of: include/core/SkMatrix.h#L1740-L1790 (chrome/m156)
const RECT_STAYS_RECT_MASK: u32 = 0x10;
const ONLY_PERSPECTIVE_VALID_MASK: u32 = 0x40;
const UNKNOWN_MASK: u32 = 0x80;
const ORABLE_MASKS: u32 = 0x01 | 0x02 | 0x04 | 0x08;
const ALL_MASKS: u32 = ORABLE_MASKS | RECT_STAYS_RECT_MASK;

const TRANSLATE_MASK: u32 = 0x01;
const SCALE_MASK: u32 = 0x02;
const AFFINE_MASK: u32 = 0x04;
const PERSPECTIVE_MASK: u32 = 0x08;

// this aligns with the masks, so we can compute a mask from a variable 0/1
const RECT_STAYS_RECT_SHIFT: u32 = 4;

// Port of: src/core/SkMatrix.cpp#L85 (chrome/m156)
const SCALAR_1_INT: i32 = 0x3f80_0000;

const M_SCALE_X: usize = 0;
const M_SKEW_X: usize = 1;
const M_TRANS_X: usize = 2;
const M_SKEW_Y: usize = 3;
const M_SCALE_Y: usize = 4;
const M_TRANS_Y: usize = 5;
const M_PERSP_0: usize = 6;
const M_PERSP_1: usize = 7;
const M_PERSP_2: usize = 8;

// `std::min` / `std::max` (not `f32::min` / `f32::max`: they differ for NaN).
fn std_min(a: scalar, b: scalar) -> scalar {
    if b < a { b } else { a }
}

fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

/// A 3x3 matrix for transforming coordinates, in row-major order.
///
/// Like `SkMatrix`, it caches a classification of the matrix (see [`Matrix::get_type`]).
// Port of: include/core/SkMatrix.h#L60-L1860 (chrome/m156)
#[doc(alias = "SkMatrix")]
#[derive(Debug)]
pub struct Matrix {
    mat: [scalar; 9],
    // `SkMatrix::fTypeMask`; `mutable` in C++, so atomic here.
    type_mask: AtomicU32,
}

impl Clone for Matrix {
    fn clone(&self) -> Self {
        Self {
            mat: self.mat,
            type_mask: AtomicU32::new(self.mask()),
        }
    }
}

impl Default for Matrix {
    fn default() -> Self {
        Self::new_identity()
    }
}

impl PartialEq for Matrix {
    // Port of: src/core/SkMatrix.cpp#L165-L172 (chrome/m156)
    #[allow(clippy::float_cmp)] // SkMatrix::operator== compares floats with ==
    fn eq(&self, rhs: &Self) -> bool {
        let (ma, mb) = (&self.mat, &rhs.mat);
        ma[0] == mb[0]
            && ma[1] == mb[1]
            && ma[2] == mb[2]
            && ma[3] == mb[3]
            && ma[4] == mb[4]
            && ma[5] == mb[5]
            && ma[6] == mb[6]
            && ma[7] == mb[7]
            && ma[8] == mb[8]
    }
}

impl Mul for Matrix {
    type Output = Self;
    // Port of: include/core/SkMatrix.h#L1710-L1712 (chrome/m156)
    fn mul(self, rhs: Matrix) -> Self::Output {
        Matrix::concat(&self, &rhs)
    }
}

impl Mul for &Matrix {
    type Output = Matrix;
    fn mul(self, rhs: &Matrix) -> Self::Output {
        Matrix::concat(self, rhs)
    }
}

impl Index<usize> for Matrix {
    type Output = scalar;

    // Port of: include/core/SkMatrix.h#L354-L357 (chrome/m156)
    fn index(&self, index: usize) -> &Self::Output {
        &self.mat[index]
    }
}

impl IndexMut<usize> for Matrix {
    // Port of: include/core/SkMatrix.h#L450-L453 (chrome/m156)
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.set_type_mask(UNKNOWN_MASK);
        &mut self.mat[index]
    }
}

impl Index<Member> for Matrix {
    type Output = scalar;

    fn index(&self, index: Member) -> &Self::Output {
        &self[index as usize]
    }
}

impl IndexMut<Member> for Matrix {
    fn index_mut(&mut self, index: Member) -> &mut Self::Output {
        self.index_mut(index as usize)
    }
}

/// The identity matrix (`SkMatrix::I()`).
// Port of: src/core/SkMatrix.cpp#L1465-L1469 (chrome/m156)
#[doc(alias = "SkMatrix::I")]
pub static IDENTITY: Matrix = Matrix::new_identity();

// The matrix returned by `SkMatrix::InvalidMatrix()`.
// Port of: src/core/SkMatrix.cpp#L1471-L1478 (chrome/m156)
static INVALID_MATRIX: Matrix = Matrix::from_parts(
    [SCALAR_MAX; 9],
    TRANSLATE_MASK | SCALE_MASK | AFFINE_MASK | PERSPECTIVE_MASK,
);

#[allow(clippy::float_cmp)] // the ported code compares floats exactly, as Skia does
impl Matrix {
    // Port of: include/core/SkMatrix.h#L1790-L1800 (chrome/m156)
    const fn from_parts(mat: [scalar; 9], type_mask: u32) -> Self {
        Self {
            mat,
            type_mask: AtomicU32::new(type_mask),
        }
    }

    /// Creates the identity matrix.
    // Port of: include/core/SkMatrix.h#L61 (chrome/m156)
    #[doc(alias = "SkMatrix")]
    #[must_use]
    pub const fn new_identity() -> Self {
        Self::from_parts(
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            RECT_STAYS_RECT_MASK,
        )
    }

    /// Sets the matrix to scale by `(sx, sy)`.
    // Port of: include/core/SkMatrix.h#L66-L70 (chrome/m156)
    #[doc(alias = "Scale")]
    #[must_use]
    pub fn scale((sx, sy): (scalar, scalar)) -> Self {
        let mut m = Self::new_identity();
        m.set_scale((sx, sy), None);
        m
    }

    /// Sets the matrix to translate by `d`.
    // Port of: include/core/SkMatrix.h#L82-L88 (chrome/m156)
    #[doc(alias = "Translate")]
    #[must_use]
    pub fn translate(d: impl Into<Vector>) -> Self {
        let d = d.into();
        let mut m = Self::new_identity();
        m.set_translate(d);
        m
    }

    /// Sets the matrix to scale by `(sx, sy)` and then translate by `t`.
    // Port of: src/core/SkMatrix.cpp#L543-L558 (chrome/m156)
    #[doc(alias = "ScaleTranslate")]
    #[must_use]
    pub fn scale_translate((sx, sy): (scalar, scalar), t: impl Into<Vector>) -> Self {
        let t = t.into();
        Self::scale_translate_xy(sx, sy, t.x, t.y)
    }

    fn scale_translate_xy(sx: f32, sy: f32, tx: f32, ty: f32) -> Self {
        let mut mask = 0;
        if sx != 1.0 || sy != 1.0 {
            mask |= SCALE_MASK;
        }
        if tx != 0.0 || ty != 0.0 {
            mask |= TRANSLATE_MASK;
        }
        if sx != 0.0 && sy != 0.0 {
            mask |= RECT_STAYS_RECT_MASK;
        }
        Self::from_parts([sx, 0.0, tx, 0.0, sy, ty, 0.0, 0.0, 1.0], mask)
    }

    /// Sets the matrix to rotate by `deg` degrees about the origin.
    // Port of: include/core/SkMatrix.h#L97-L101 (chrome/m156)
    #[doc(alias = "RotateDeg")]
    #[must_use]
    pub fn rotate_deg(deg: scalar) -> Self {
        let mut m = Self::new_identity();
        m.set_rotate(deg, None);
        m
    }

    /// Sets the matrix to rotate by `deg` degrees about `pivot`.
    // Port of: include/core/SkMatrix.h#L102-L106 (chrome/m156)
    #[doc(alias = "RotateDeg")]
    #[must_use]
    pub fn rotate_deg_pivot(deg: scalar, pivot: impl Into<Point>) -> Self {
        let mut m = Self::new_identity();
        m.set_rotate(deg, pivot.into());
        m
    }

    /// Sets the matrix to rotate by `rad` radians about the origin.
    // Port of: include/core/SkMatrix.h#L107-L109 (chrome/m156)
    #[doc(alias = "RotateRad")]
    #[must_use]
    pub fn rotate_rad(rad: scalar) -> Self {
        Self::rotate_deg(float_radians_to_degrees(rad))
    }

    /// Sets the matrix to skew by `(kx, ky)` about the origin.
    // Port of: include/core/SkMatrix.h#L117-L121 (chrome/m156)
    #[doc(alias = "Skew")]
    #[must_use]
    pub fn skew((kx, ky): (scalar, scalar)) -> Self {
        let mut m = Self::new_identity();
        m.set_skew((kx, ky), None);
        m
    }

    /// Sets the matrix to the nine given members, in row-major order (`SkMatrix::MakeAll`).
    // Port of: include/core/SkMatrix.h#L153-L162 (chrome/m156)
    #[doc(alias = "MakeAll")]
    #[must_use]
    #[allow(clippy::too_many_arguments)] // mirrors SkMatrix::MakeAll
    pub fn new_all(
        scale_x: scalar,
        skew_x: scalar,
        trans_x: scalar,
        skew_y: scalar,
        scale_y: scalar,
        trans_y: scalar,
        pers_0: scalar,
        pers_1: scalar,
        pers_2: scalar,
    ) -> Self {
        let mut m = Self::new_identity();
        m.set_all(
            scale_x, skew_x, trans_x, skew_y, scale_y, trans_y, pers_0, pers_1, pers_2,
        );
        m
    }

    /// Returns the identity matrix (`SkMatrix::I()`).
    // Port of: src/core/SkMatrix.cpp#L1465-L1469 (chrome/m156)
    #[doc(alias = "I")]
    #[must_use]
    pub fn i() -> &'static Matrix {
        &IDENTITY
    }

    /// Returns a matrix with every member set to the largest scalar and all type bits set
    /// (`SkMatrix::InvalidMatrix()`).
    // Port of: src/core/SkMatrix.cpp#L1471-L1478 (chrome/m156)
    #[doc(alias = "InvalidMatrix")]
    #[must_use]
    pub fn invalid_matrix() -> &'static Matrix {
        &INVALID_MATRIX
    }

    // ---- type mask cache -------------------------------------------------------------------

    fn mask(&self) -> u32 {
        self.type_mask.load(Ordering::Relaxed)
    }

    // `SkMatrix::setTypeMask`
    // Port of: include/core/SkMatrix.h#L1840-L1850 (chrome/m156)
    fn set_type_mask(&self, mask: u32) {
        debug_assert!(
            UNKNOWN_MASK == mask
                || (mask & ALL_MASKS) == mask
                || ((UNKNOWN_MASK | ONLY_PERSPECTIVE_VALID_MASK) & mask)
                    == (UNKNOWN_MASK | ONLY_PERSPECTIVE_VALID_MASK)
        );
        self.type_mask.store(mask, Ordering::Relaxed);
    }

    // Port of: include/core/SkMatrix.h#L1851-L1856 (chrome/m156)
    fn or_type_mask(&self, mask: u32) {
        debug_assert_eq!(mask & ORABLE_MASKS, mask);
        self.type_mask.fetch_or(mask, Ordering::Relaxed);
    }

    // Port of: include/core/SkMatrix.h#L1857-L1862 (chrome/m156)
    fn clear_type_mask(&self, mask: u32) {
        debug_assert_eq!(mask & ALL_MASKS, mask);
        self.type_mask.fetch_and(!mask, Ordering::Relaxed);
    }

    // Port of: include/core/SkMatrix.h#L1805-L1815 (chrome/m156)
    fn get_perspective_type_mask_only(&self) -> u32 {
        let mut mask = self.mask();
        if (mask & UNKNOWN_MASK) != 0 && (mask & ONLY_PERSPECTIVE_VALID_MASK) == 0 {
            mask = self.compute_perspective_type_mask();
            self.type_mask.store(mask, Ordering::Relaxed);
        }
        mask & 0xF
    }

    // Port of: include/core/SkMatrix.h#L1816-L1822 (chrome/m156)
    #[allow(clippy::verbose_bit_mask)] // mirrors `(fTypeMask & 0xF) == 0`
    fn is_trivially_identity(&self) -> bool {
        let mask = self.mask();
        if (mask & UNKNOWN_MASK) != 0 {
            return false;
        }
        (mask & 0xF) == 0
    }

    // Port of: include/core/SkMatrix.h#L1823-L1829 (chrome/m156)
    fn update_translate_mask(&self) {
        if self.mat[M_TRANS_X] != 0.0 || self.mat[M_TRANS_Y] != 0.0 {
            self.type_mask.fetch_or(TRANSLATE_MASK, Ordering::Relaxed);
        } else {
            self.type_mask.fetch_and(!TRANSLATE_MASK, Ordering::Relaxed);
        }
    }

    // Port of: src/core/SkMatrix.cpp#L87-L100 (chrome/m156)
    fn compute_perspective_type_mask(&self) -> u32 {
        // Benchmarking suggests that replacing this set of SkScalarAs2sCompliment
        // is a win, but replacing those below is not. We don't yet understand
        // that result.
        if self.mat[M_PERSP_0] != 0.0 || self.mat[M_PERSP_1] != 0.0 || self.mat[M_PERSP_2] != 1.0 {
            // If this is a perspective transform, we return true for all other
            // transform flags - this does not disable any optimizations, respects
            // the rule that the type mask must be conservative, and speeds up
            // type mask computation.
            return ORABLE_MASKS;
        }

        ONLY_PERSPECTIVE_VALID_MASK | UNKNOWN_MASK
    }

    // Port of: src/core/SkMatrix.cpp#L102-L161 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // the shifted values are 0 or 1, as in the C++ `int` math
    fn compute_type_mask(&self) -> u32 {
        let fmat = &self.mat;
        let mut mask: u32 = 0;

        if fmat[M_PERSP_0] != 0.0 || fmat[M_PERSP_1] != 0.0 || fmat[M_PERSP_2] != 1.0 {
            // Once it is determined that that this is a perspective transform,
            // all other flags are moot as far as optimizations are concerned.
            return ORABLE_MASKS;
        }

        if fmat[M_TRANS_X] != 0.0 || fmat[M_TRANS_Y] != 0.0 {
            mask |= TRANSLATE_MASK;
        }

        let mut m00 = float_as_2s_compliment(fmat[M_SCALE_X]);
        let mut m01 = float_as_2s_compliment(fmat[M_SKEW_X]);
        let mut m10 = float_as_2s_compliment(fmat[M_SKEW_Y]);
        let mut m11 = float_as_2s_compliment(fmat[M_SCALE_Y]);

        if (m01 | m10) != 0 {
            // The skew components may be scale-inducing, unless we are dealing
            // with a pure rotation.  Testing for a pure rotation is expensive,
            // so we opt for being conservative by always setting the scale bit.
            // along with affine.
            // By doing this, we are also ensuring that matrices have the same
            // type masks as their inverses.
            mask |= AFFINE_MASK | SCALE_MASK;

            // For rectStaysRect, in the affine case, we only need check that
            // the primary diagonal is all zeros and that the secondary diagonal
            // is all non-zero.

            // map non-zero to 1
            m01 = i32::from(m01 != 0);
            m10 = i32::from(m10 != 0);

            let dp0 = i32::from(0 == (m00 | m11)); // true if both are 0
            let ds1 = m01 & m10; // true if both are 1

            // (dp0 & ds1) is 0 or 1
            mask |= ((dp0 & ds1) as u32) << RECT_STAYS_RECT_SHIFT;
        } else {
            // Only test for scale explicitly if not affine, since affine sets the
            // scale bit.
            if ((m00 ^ SCALAR_1_INT) | (m11 ^ SCALAR_1_INT)) != 0 {
                mask |= SCALE_MASK;
            }

            // Not affine, therefore we already know secondary diagonal is
            // all zeros, so we just need to check that primary diagonal is
            // all non-zero.

            // map non-zero to 1
            m00 = i32::from(m00 != 0);
            m11 = i32::from(m11 != 0);

            // record if the (p)rimary diagonal is all non-zero
            // (m00 & m11) is 0 or 1
            mask |= ((m00 & m11) as u32) << RECT_STAYS_RECT_SHIFT;
        }

        mask
    }

    /// Returns a bit field describing the transformations the matrix may perform.
    ///
    /// The bit field is computed lazily and cached; it is conservative (a set bit does not
    /// guarantee the matrix really does that transformation).
    // Port of: include/core/SkMatrix.h#L181-L186 (chrome/m156)
    #[doc(alias = "getType")]
    #[must_use]
    pub fn get_type(&self) -> TypeMask {
        let mut mask = self.mask();
        if (mask & UNKNOWN_MASK) != 0 {
            mask = self.compute_type_mask();
            self.type_mask.store(mask, Ordering::Relaxed);
        }
        TypeMask::from_bits_truncate(mask & 0xF)
    }

    // `getType()` as raw bits, for the `mask <= kTranslate_Mask` style tests of the C++.
    fn type_bits(&self) -> u32 {
        self.get_type().bits()
    }

    /// Returns true if the matrix is identity.
    // Port of: include/core/SkMatrix.h#L197-L199 (chrome/m156)
    #[doc(alias = "isIdentity")]
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.type_bits() == 0
    }

    /// Returns true if the matrix contains only scale and translation (`SkMatrix::isScaleTranslate`).
    // Port of: include/core/SkMatrix.h#L210-L212 (chrome/m156)
    #[doc(alias = "isScaleTranslate")]
    #[must_use]
    pub fn is_scale_translate(&self) -> bool {
        (self.type_bits() & !(SCALE_MASK | TRANSLATE_MASK)) == 0
    }

    /// Returns true if the matrix is identity, or translates.
    // Port of: include/core/SkMatrix.h#L222 (chrome/m156)
    #[doc(alias = "isTranslate")]
    #[must_use]
    pub fn is_translate(&self) -> bool {
        (self.type_bits() & !TRANSLATE_MASK) == 0
    }

    /// Returns true if the matrix maps an axis-aligned rectangle to an axis-aligned rectangle.
    // Port of: include/core/SkMatrix.h#L245-L250 (chrome/m156)
    #[doc(alias = "rectStaysRect")]
    #[must_use]
    pub fn rect_stays_rect(&self) -> bool {
        let mut mask = self.mask();
        if (mask & UNKNOWN_MASK) != 0 {
            mask = self.compute_type_mask();
            self.type_mask.store(mask, Ordering::Relaxed);
        }
        (mask & RECT_STAYS_RECT_MASK) != 0
    }

    /// Alias for [`Matrix::rect_stays_rect`].
    // Port of: include/core/SkMatrix.h#L261 (chrome/m156)
    #[doc(alias = "preservesAxisAlignment")]
    #[must_use]
    pub fn preserves_axis_alignment(&self) -> bool {
        self.rect_stays_rect()
    }

    /// Returns true if the matrix contains perspective elements.
    // Port of: include/core/SkMatrix.h#L286-L289 (chrome/m156)
    #[doc(alias = "hasPerspective")]
    #[must_use]
    pub fn has_perspective(&self) -> bool {
        (self.get_perspective_type_mask_only() & PERSPECTIVE_MASK) != 0
    }

    /// Returns true if the matrix contains only translation, rotation, reflection, and uniform
    /// scale (within [`SCALAR_NEARLY_ZERO`]).
    #[doc(alias = "isSimilarity")]
    #[must_use]
    pub fn is_similarity(&self) -> bool {
        self.is_similarity_tol(SCALAR_NEARLY_ZERO)
    }

    /// [`Matrix::is_similarity`] with an explicit tolerance.
    // Port of: src/core/SkMatrix.cpp#L185-L212 (chrome/m156)
    #[doc(alias = "isSimilarity")]
    #[must_use]
    pub fn is_similarity_tol(&self, tol: scalar) -> bool {
        // if identity or translate matrix
        let mask = self.type_bits();
        if mask <= TRANSLATE_MASK {
            return true;
        }
        if (mask & PERSPECTIVE_MASK) != 0 {
            return false;
        }

        let mx = self.mat[M_SCALE_X];
        let my = self.mat[M_SCALE_Y];
        // if no skew, can just compare scale factors
        if (mask & AFFINE_MASK) == 0 {
            return !mx.nearly_zero(None)
                && scalar::nearly_equal(scalar_abs(mx), scalar_abs(my), None);
        }
        let sx = self.mat[M_SKEW_X];
        let sy = self.mat[M_SKEW_Y];

        if is_degenerate_2x2(mx, sx, sy, my) {
            return false;
        }

        // upper 2x2 is rotation/reflection + uniform scale if basis vectors
        // are 90 degree rotations of each other
        (scalar::nearly_equal(mx, my, tol) && scalar::nearly_equal(sx, -sy, tol))
            || (scalar::nearly_equal(mx, -my, tol) && scalar::nearly_equal(sx, sy, tol))
    }

    /// Returns true if the matrix maps perpendicular lines to perpendicular lines
    /// (within [`SCALAR_NEARLY_ZERO`]).
    #[doc(alias = "preservesRightAngles")]
    #[must_use]
    pub fn preserves_right_angles(&self) -> bool {
        self.preserves_right_angles_tol(SCALAR_NEARLY_ZERO)
    }

    /// [`Matrix::preserves_right_angles`] with an explicit tolerance.
    // Port of: src/core/SkMatrix.cpp#L214-L242 (chrome/m156)
    #[doc(alias = "preservesRightAngles")]
    #[must_use]
    pub fn preserves_right_angles_tol(&self, tol: scalar) -> bool {
        let mask = self.type_bits();

        if mask <= TRANSLATE_MASK {
            // identity, translate and/or scale
            return true;
        }
        if (mask & PERSPECTIVE_MASK) != 0 {
            return false;
        }

        debug_assert!((mask & (AFFINE_MASK | SCALE_MASK)) != 0);

        let mx = self.mat[M_SCALE_X];
        let my = self.mat[M_SCALE_Y];
        let sx = self.mat[M_SKEW_X];
        let sy = self.mat[M_SKEW_Y];

        if is_degenerate_2x2(mx, sx, sy, my) {
            return false;
        }

        // upper 2x2 is scale + rotation/reflection if basis vectors are orthogonal
        let vec = [Point::new(mx, sy), Point::new(sx, my)];

        vec[0].dot(vec[1]).nearly_zero(scalar_square(tol))
    }

    // ---- member access ---------------------------------------------------------------------

    /// Returns member `index`, in row-major order (`SkMatrix::get`).
    // Port of: include/core/SkMatrix.h#L362-L365 (chrome/m156)
    #[must_use]
    pub fn get(&self, index: impl Into<usize>) -> scalar {
        self.mat[index.into()]
    }

    /// Returns the member at row `r`, column `c`.
    ///
    /// # Panics
    /// If `r` or `c` is greater than 2.
    // Port of: include/core/SkMatrix.h#L378-L382 (chrome/m156)
    #[must_use]
    pub fn rc(&self, r: usize, c: usize) -> scalar {
        assert!(r <= 2);
        assert!(c <= 2);
        self.mat[r * 3 + c]
    }

    /// Returns the horizontal scale factor.
    // Port of: include/core/SkMatrix.h#L390 (chrome/m156)
    #[doc(alias = "getScaleX")]
    #[must_use]
    pub fn scale_x(&self) -> scalar {
        self.mat[M_SCALE_X]
    }

    /// Returns the vertical scale factor.
    #[doc(alias = "getScaleY")]
    #[must_use]
    pub fn scale_y(&self) -> scalar {
        self.mat[M_SCALE_Y]
    }

    /// Returns the vertical skew factor.
    #[doc(alias = "getSkewY")]
    #[must_use]
    pub fn skew_y(&self) -> scalar {
        self.mat[M_SKEW_Y]
    }

    /// Returns the horizontal skew factor.
    #[doc(alias = "getSkewX")]
    #[must_use]
    pub fn skew_x(&self) -> scalar {
        self.mat[M_SKEW_X]
    }

    /// Returns the horizontal translation factor.
    #[doc(alias = "getTranslateX")]
    #[must_use]
    pub fn translate_x(&self) -> scalar {
        self.mat[M_TRANS_X]
    }

    /// Returns the vertical translation factor.
    #[doc(alias = "getTranslateY")]
    #[must_use]
    pub fn translate_y(&self) -> scalar {
        self.mat[M_TRANS_Y]
    }

    /// Returns the input x perspective factor.
    #[doc(alias = "getPerspX")]
    #[must_use]
    pub fn persp_x(&self) -> scalar {
        self.mat[M_PERSP_0]
    }

    /// Returns the input y perspective factor.
    #[doc(alias = "getPerspY")]
    #[must_use]
    pub fn persp_y(&self) -> scalar {
        self.mat[M_PERSP_1]
    }

    /// Sets member `index` to `value` and invalidates the cached type.
    // Port of: include/core/SkMatrix.h#L463-L467 (chrome/m156)
    pub fn set(&mut self, index: impl Into<usize>, value: scalar) -> &mut Self {
        self.mat[index.into()] = value;
        self.set_type_mask(UNKNOWN_MASK);
        self
    }

    /// Sets the horizontal scale factor.
    #[doc(alias = "setScaleX")]
    pub fn set_scale_x(&mut self, v: scalar) -> &mut Self {
        self.set(Member::ScaleX, v)
    }

    /// Sets the vertical scale factor.
    #[doc(alias = "setScaleY")]
    pub fn set_scale_y(&mut self, v: scalar) -> &mut Self {
        self.set(Member::ScaleY, v)
    }

    /// Sets the vertical skew factor.
    #[doc(alias = "setSkewY")]
    pub fn set_skew_y(&mut self, v: scalar) -> &mut Self {
        self.set(Member::SkewY, v)
    }

    /// Sets the horizontal skew factor.
    #[doc(alias = "setSkewX")]
    pub fn set_skew_x(&mut self, v: scalar) -> &mut Self {
        self.set(Member::SkewX, v)
    }

    /// Sets the horizontal translation.
    #[doc(alias = "setTranslateX")]
    pub fn set_translate_x(&mut self, v: scalar) -> &mut Self {
        self.set(Member::TransX, v)
    }

    /// Sets the vertical translation.
    #[doc(alias = "setTranslateY")]
    pub fn set_translate_y(&mut self, v: scalar) -> &mut Self {
        self.set(Member::TransY, v)
    }

    /// Sets the input x perspective factor.
    #[doc(alias = "setPerspX")]
    pub fn set_persp_x(&mut self, v: scalar) -> &mut Self {
        self.set(Member::Persp0, v)
    }

    /// Sets the input y perspective factor.
    #[doc(alias = "setPerspY")]
    pub fn set_persp_y(&mut self, v: scalar) -> &mut Self {
        self.set(Member::Persp1, v)
    }

    /// Sets all members, in row-major order.
    // Port of: include/core/SkMatrix.h#L536-L553 (chrome/m156)
    #[doc(alias = "setAll")]
    #[allow(clippy::too_many_arguments)] // mirrors SkMatrix::setAll
    pub fn set_all(
        &mut self,
        scale_x: scalar,
        skew_x: scalar,
        trans_x: scalar,
        skew_y: scalar,
        scale_y: scalar,
        trans_y: scalar,
        persp_0: scalar,
        persp_1: scalar,
        persp_2: scalar,
    ) -> &mut Self {
        self.mat[M_SCALE_X] = scale_x;
        self.mat[M_SKEW_X] = skew_x;
        self.mat[M_TRANS_X] = trans_x;
        self.mat[M_SKEW_Y] = skew_y;
        self.mat[M_SCALE_Y] = scale_y;
        self.mat[M_TRANS_Y] = trans_y;
        self.mat[M_PERSP_0] = persp_0;
        self.mat[M_PERSP_1] = persp_1;
        self.mat[M_PERSP_2] = persp_2;
        self.set_type_mask(UNKNOWN_MASK);
        self
    }

    /// Copies the nine members, in row-major order, into `buffer`.
    // Port of: include/core/SkMatrix.h#L558-L560 (chrome/m156)
    #[doc(alias = "get9")]
    pub fn get_9(&self, buffer: &mut [scalar; 9]) {
        *buffer = self.mat;
    }

    /// Sets the nine members from `buffer`, in row-major order.
    // Port of: src/core/SkMatrix.cpp#L56-L60 (chrome/m156)
    #[doc(alias = "set9")]
    pub fn set_9(&mut self, buffer: &[scalar; 9]) -> &mut Self {
        self.mat = *buffer;
        self.set_type_mask(UNKNOWN_MASK);
        self
    }

    /// Sets the matrix to the transform of an [`RSXform`] (`SkMatrix::setRSXform`).
    // Port of: src/core/SkMatrix.cpp#L425-L439 (chrome/m156)
    #[doc(alias = "setRSXform")]
    pub fn set_rsxform(&mut self, xform: &RSXform) -> &mut Self {
        self.mat[M_SCALE_X] = xform.scos;
        self.mat[M_SKEW_X] = -xform.ssin;
        self.mat[M_TRANS_X] = xform.tx;

        self.mat[M_SKEW_Y] = xform.ssin;
        self.mat[M_SCALE_Y] = xform.scos;
        self.mat[M_TRANS_Y] = xform.ty;

        self.mat[M_PERSP_0] = 0.0;
        self.mat[M_PERSP_1] = 0.0;
        self.mat[M_PERSP_2] = 1.0;

        self.set_type_mask(UNKNOWN_MASK | ONLY_PERSPECTIVE_VALID_MASK);
        self
    }

    /// Sets the matrix to identity.
    // Port of: src/core/SkMatrix.cpp#L54 (chrome/m156)
    pub fn reset(&mut self) -> &mut Self {
        *self = Self::new_identity();
        self
    }

    /// Alias for [`Matrix::reset`].
    // Port of: include/core/SkMatrix.h#L600 (chrome/m156)
    #[doc(alias = "setIdentity")]
    pub fn set_identity(&mut self) -> &mut Self {
        self.reset()
    }

    // ---- translate -------------------------------------------------------------------------

    /// Sets the matrix to translate by `v`.
    // Port of: src/core/SkMatrix.cpp#L259-L266 (chrome/m156)
    #[doc(alias = "setTranslate")]
    pub fn set_translate(&mut self, v: impl Into<Vector>) -> &mut Self {
        let v = v.into();
        let (dx, dy) = (v.x, v.y);
        *self = Self::from_parts(
            [1.0, 0.0, dx, 0.0, 1.0, dy, 0.0, 0.0, 1.0],
            if dx != 0.0 || dy != 0.0 {
                TRANSLATE_MASK | RECT_STAYS_RECT_MASK
            } else {
                RECT_STAYS_RECT_MASK
            },
        );
        self
    }

    /// Sets the matrix to its own value concatenated with a translation by `delta`
    /// (`M' = M * T(dx, dy)`).
    // Port of: src/core/SkMatrix.cpp#L268-L284 (chrome/m156)
    #[doc(alias = "preTranslate")]
    pub fn pre_translate(&mut self, delta: impl Into<Vector>) -> &mut Self {
        let delta = delta.into();
        let (dx, dy) = (delta.x, delta.y);
        let mask = self.type_bits();

        if mask <= TRANSLATE_MASK {
            self.mat[M_TRANS_X] += dx;
            self.mat[M_TRANS_Y] += dy;
        } else if (mask & PERSPECTIVE_MASK) != 0 {
            let mut m = Self::new_identity();
            m.set_translate(Point::new(dx, dy));
            return self.pre_concat(&m);
        } else {
            self.mat[M_TRANS_X] += sdot(self.mat[M_SCALE_X], dx, self.mat[M_SKEW_X], dy);
            self.mat[M_TRANS_Y] += sdot(self.mat[M_SKEW_Y], dx, self.mat[M_SCALE_Y], dy);
        }
        self.update_translate_mask();
        self
    }

    /// Sets the matrix to a translation by `delta` concatenated with its own value
    /// (`M' = T(dx, dy) * M`).
    // Port of: src/core/SkMatrix.cpp#L286-L297 (chrome/m156)
    #[doc(alias = "postTranslate")]
    pub fn post_translate(&mut self, delta: impl Into<Vector>) -> &mut Self {
        let delta = delta.into();
        let (dx, dy) = (delta.x, delta.y);
        if self.has_perspective() {
            let mut m = Self::new_identity();
            m.set_translate(Point::new(dx, dy));
            self.post_concat(&m);
        } else {
            self.mat[M_TRANS_X] += dx;
            self.mat[M_TRANS_Y] += dy;
            self.update_translate_mask();
        }
        self
    }

    // ---- scale -----------------------------------------------------------------------------

    /// Sets the matrix to scale by `(sx, sy)` about `pivot` (the origin when `None`).
    // Port of: src/core/SkMatrix.cpp#L301-L318 (chrome/m156)
    #[doc(alias = "setScale")]
    pub fn set_scale(
        &mut self,
        (sx, sy): (scalar, scalar),
        pivot: impl Into<Option<Point>>,
    ) -> &mut Self {
        if let Some(pivot) = pivot.into() {
            let (px, py) = (pivot.x, pivot.y);
            if 1.0 == sx && 1.0 == sy {
                self.reset();
            } else {
                self.set_scale_translate((sx, sy), Point::new(px - sx * px, py - sy * py));
            }
        } else {
            let rect_mask = if sx == 0.0 || sy == 0.0 {
                0
            } else {
                RECT_STAYS_RECT_MASK
            };
            *self = Self::from_parts(
                [sx, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 1.0],
                if sx == 1.0 && sy == 1.0 {
                    rect_mask
                } else {
                    SCALE_MASK | rect_mask
                },
            );
        }
        self
    }

    /// Sets the matrix to scale by `(sx, sy)` and then translate by `t`.
    // Port of: include/core/SkMatrix.h#L1732-L1734 (chrome/m156)
    #[doc(alias = "setScaleTranslate")]
    pub fn set_scale_translate(&mut self, s: (scalar, scalar), t: impl Into<Vector>) -> &mut Self {
        *self = Self::scale_translate(s, t);
        self
    }

    /// Sets the matrix to its own value concatenated with a scale about `pivot`
    /// (the origin when `None`).
    // Port of: src/core/SkMatrix.cpp#L320-L363 (chrome/m156)
    #[doc(alias = "preScale")]
    pub fn pre_scale(
        &mut self,
        (sx, sy): (scalar, scalar),
        pivot: impl Into<Option<Point>>,
    ) -> &mut Self {
        if 1.0 == sx && 1.0 == sy {
            return self;
        }

        if let Some(pivot) = pivot.into() {
            let mut m = Self::new_identity();
            m.set_scale((sx, sy), pivot);
            return self.pre_concat(&m);
        }

        // the assumption is that these multiplies are very cheap, and that
        // a full concat and/or just computing the matrix type is more expensive.
        // Also, the fixed-point case checks for overflow, but the float doesn't,
        // so we can get away with these blind multiplies.

        self.mat[M_SCALE_X] *= sx;
        self.mat[M_SKEW_Y] *= sx;
        self.mat[M_PERSP_0] *= sx;

        self.mat[M_SKEW_X] *= sy;
        self.mat[M_SCALE_Y] *= sy;
        self.mat[M_PERSP_1] *= sy;

        // Attempt to simplify our type when applying an inverse scale.
        // TODO: The persp/affine preconditions are in place to keep the mask consistent with
        //       what computeTypeMask() would produce (persp/skew always implies kScale).
        //       We should investigate whether these flag dependencies are truly needed.
        if self.mat[M_SCALE_X] == 1.0
            && self.mat[M_SCALE_Y] == 1.0
            && (self.mask() & (PERSPECTIVE_MASK | AFFINE_MASK)) == 0
        {
            self.clear_type_mask(SCALE_MASK);
        } else {
            self.or_type_mask(SCALE_MASK);
            // Remove kRectStaysRect if the preScale factors were 0
            if sx == 0.0 || sy == 0.0 {
                self.clear_type_mask(RECT_STAYS_RECT_MASK);
            }
        }
        self
    }

    /// Sets the matrix to a scale about `pivot` (the origin when `None`) concatenated with its
    /// own value.
    // Port of: src/core/SkMatrix.cpp#L365-L381 (chrome/m156)
    #[doc(alias = "postScale")]
    pub fn post_scale(
        &mut self,
        (sx, sy): (scalar, scalar),
        pivot: impl Into<Option<Point>>,
    ) -> &mut Self {
        if 1.0 == sx && 1.0 == sy {
            return self;
        }
        let mut m = Self::new_identity();
        m.set_scale((sx, sy), pivot);
        self.post_concat(&m)
    }

    /// Divides the matrix's x and y members by `divx` and `divy`; returns false (leaving the
    /// matrix unchanged) if either is zero (`SkMatrix::postIDiv`, private in C++).
    // Port of: src/core/SkMatrix.cpp#L385-L403 (chrome/m156)
    #[allow(clippy::similar_names)] // inv_x / inv_y follow the C++ invX / invY
    pub(crate) fn post_i_div(&mut self, divx: i32, divy: i32) -> bool {
        if divx == 0 || divy == 0 {
            return false;
        }

        // C++ `1.f / divx`: the int converts to float
        #[allow(clippy::cast_precision_loss)] // mirrors the implicit int -> float conversion
        let inv_x = 1.0f32 / divx as f32;
        #[allow(clippy::cast_precision_loss)] // mirrors the implicit int -> float conversion
        let inv_y = 1.0f32 / divy as f32;

        self.mat[M_SCALE_X] *= inv_x;
        self.mat[M_SKEW_X] *= inv_x;
        self.mat[M_TRANS_X] *= inv_x;

        self.mat[M_SCALE_Y] *= inv_y;
        self.mat[M_SKEW_Y] *= inv_y;
        self.mat[M_TRANS_Y] *= inv_y;

        self.set_type_mask(UNKNOWN_MASK);
        true
    }

    // ---- rotate ----------------------------------------------------------------------------

    /// Sets the matrix to rotate by the given sine and cosine about `pivot` (the origin when
    /// `None`).
    // Port of: src/core/SkMatrix.cpp#L407-L455 (chrome/m156)
    #[doc(alias = "setSinCos")]
    pub fn set_sin_cos(
        &mut self,
        (sin_value, cos_value): (scalar, scalar),
        pivot: impl Into<Option<Point>>,
    ) -> &mut Self {
        if let Some(pivot) = pivot.into() {
            let (px, py) = (pivot.x, pivot.y);
            let one_minus_cos_v = 1.0 - cos_value;

            self.mat[M_SCALE_X] = cos_value;
            self.mat[M_SKEW_X] = -sin_value;
            self.mat[M_TRANS_X] = sdot(sin_value, py, one_minus_cos_v, px);

            self.mat[M_SKEW_Y] = sin_value;
            self.mat[M_SCALE_Y] = cos_value;
            self.mat[M_TRANS_Y] = sdot(-sin_value, px, one_minus_cos_v, py);
        } else {
            self.mat[M_SCALE_X] = cos_value;
            self.mat[M_SKEW_X] = -sin_value;
            self.mat[M_TRANS_X] = 0.0;

            self.mat[M_SKEW_Y] = sin_value;
            self.mat[M_SCALE_Y] = cos_value;
            self.mat[M_TRANS_Y] = 0.0;
        }

        self.mat[M_PERSP_0] = 0.0;
        self.mat[M_PERSP_1] = 0.0;
        self.mat[M_PERSP_2] = 1.0;

        self.set_type_mask(UNKNOWN_MASK | ONLY_PERSPECTIVE_VALID_MASK);
        self
    }

    /// Sets the matrix to rotate by `degrees` about `pivot` (the origin when `None`).
    /// Positive degrees rotate clockwise.
    // Port of: src/core/SkMatrix.cpp#L457-L465 (chrome/m156)
    #[doc(alias = "setRotate")]
    pub fn set_rotate(&mut self, degrees: scalar, pivot: impl Into<Option<Point>>) -> &mut Self {
        let rad = degrees_to_radians(degrees);
        self.set_sin_cos(
            (scalar_sin_snap_to_zero(rad), scalar_cos_snap_to_zero(rad)),
            pivot,
        )
    }

    /// Sets the matrix to its own value concatenated with a rotation about `pivot`.
    // Port of: src/core/SkMatrix.cpp#L467-L477 (chrome/m156)
    #[doc(alias = "preRotate")]
    pub fn pre_rotate(&mut self, degrees: scalar, pivot: impl Into<Option<Point>>) -> &mut Self {
        let mut m = Self::new_identity();
        m.set_rotate(degrees, pivot);
        self.pre_concat(&m)
    }

    /// Sets the matrix to a rotation about `pivot` concatenated with its own value.
    // Port of: src/core/SkMatrix.cpp#L479-L489 (chrome/m156)
    #[doc(alias = "postRotate")]
    pub fn post_rotate(&mut self, degrees: scalar, pivot: impl Into<Option<Point>>) -> &mut Self {
        let mut m = Self::new_identity();
        m.set_rotate(degrees, pivot);
        self.post_concat(&m)
    }

    // ---- skew ------------------------------------------------------------------------------

    /// Sets the matrix to skew by `(kx, ky)` about `pivot` (the origin when `None`).
    // Port of: src/core/SkMatrix.cpp#L493-L515 (chrome/m156)
    #[doc(alias = "setSkew")]
    pub fn set_skew(
        &mut self,
        (kx, ky): (scalar, scalar),
        pivot: impl Into<Option<Point>>,
    ) -> &mut Self {
        if let Some(pivot) = pivot.into() {
            let (px, py) = (pivot.x, pivot.y);
            *self = Self::from_parts(
                [1.0, kx, -kx * py, ky, 1.0, -ky * px, 0.0, 0.0, 1.0],
                UNKNOWN_MASK | ONLY_PERSPECTIVE_VALID_MASK,
            );
        } else {
            self.mat[M_SCALE_X] = 1.0;
            self.mat[M_SKEW_X] = kx;
            self.mat[M_TRANS_X] = 0.0;

            self.mat[M_SKEW_Y] = ky;
            self.mat[M_SCALE_Y] = 1.0;
            self.mat[M_TRANS_Y] = 0.0;

            self.mat[M_PERSP_0] = 0.0;
            self.mat[M_PERSP_1] = 0.0;
            self.mat[M_PERSP_2] = 1.0;

            self.set_type_mask(UNKNOWN_MASK | ONLY_PERSPECTIVE_VALID_MASK);
        }
        self
    }

    /// Sets the matrix to its own value concatenated with a skew about `pivot`.
    // Port of: src/core/SkMatrix.cpp#L517-L527 (chrome/m156)
    #[doc(alias = "preSkew")]
    pub fn pre_skew(
        &mut self,
        (kx, ky): (scalar, scalar),
        pivot: impl Into<Option<Point>>,
    ) -> &mut Self {
        let mut m = Self::new_identity();
        m.set_skew((kx, ky), pivot);
        self.pre_concat(&m)
    }

    /// Sets the matrix to a skew about `pivot` concatenated with its own value.
    // Port of: src/core/SkMatrix.cpp#L529-L539 (chrome/m156)
    #[doc(alias = "postSkew")]
    pub fn post_skew(
        &mut self,
        (kx, ky): (scalar, scalar),
        pivot: impl Into<Option<Point>>,
    ) -> &mut Self {
        let mut m = Self::new_identity();
        m.set_skew((kx, ky), pivot);
        self.post_concat(&m)
    }

    // ---- rect to rect ----------------------------------------------------------------------

    /// Returns the matrix that maps `src` to `dst` according to `stf`, or `None` if `src` is
    /// empty (`SkMatrix::Rect2Rect`).
    // Port of: src/core/SkMatrix.cpp#L560-L600 (chrome/m156)
    #[doc(alias = "Rect2Rect")]
    #[must_use]
    pub fn rect_2_rect(
        src: impl AsRef<Rect>,
        dst: impl AsRef<Rect>,
        stf: impl Into<Option<ScaleToFit>>,
    ) -> Option<Self> {
        let (src, dst) = (src.as_ref(), dst.as_ref());
        let stf = stf.into().unwrap_or(ScaleToFit::Fill);
        if src.is_empty() {
            return None;
        }

        let mut sx = ieee_float_divide(dst.width(), src.width());
        let mut sy = ieee_float_divide(dst.height(), src.height());
        let mut x_larger = false;

        if stf != ScaleToFit::Fill {
            if sx > sy {
                x_larger = true;
                sx = sy;
            } else {
                sy = sx;
            }
        }

        let mut tx = dst.left - src.left * sx;
        let mut ty = dst.top - src.top * sy;
        if stf == ScaleToFit::Center || stf == ScaleToFit::End {
            let mut diff = if x_larger {
                dst.width() - src.width() * sy
            } else {
                dst.height() - src.height() * sy
            };

            if stf == ScaleToFit::Center {
                diff /= 2.0;
            }

            if x_larger {
                tx += diff;
            } else {
                ty += diff;
            }
        }
        Some(Self::scale_translate_xy(sx, sy, tx, ty))
    }

    /// Like [`Matrix::rect_2_rect`], but returns the identity matrix on failure
    /// (`SkMatrix::RectToRectOrIdentity`).
    // Port of: include/core/SkMatrix.h#L1120-L1123 (chrome/m156)
    #[doc(alias = "RectToRectOrIdentity")]
    #[must_use]
    pub fn rect_to_rect_or_identity(
        src: impl AsRef<Rect>,
        dst: impl AsRef<Rect>,
        stf: impl Into<Option<ScaleToFit>>,
    ) -> Self {
        Self::rect_2_rect(src, dst, stf).unwrap_or_else(Self::new_identity)
    }

    /// Sets the matrix to [`Matrix::rect_2_rect`]; on failure resets it to identity and returns
    /// false (`SkMatrix::setRectToRect`, behind `SK_SUPPORT_LEGACY_MATRIX_RECTTORECT`).
    // Port of: include/core/SkMatrix.h#L1126-L1133 (chrome/m156)
    #[doc(alias = "setRectToRect")]
    pub fn set_rect_to_rect(
        &mut self,
        src: impl AsRef<Rect>,
        dst: impl AsRef<Rect>,
        stf: ScaleToFit,
    ) -> bool {
        if let Some(mx) = Self::rect_2_rect(src, dst, stf) {
            *self = mx;
            return true;
        }
        self.reset();
        false
    }

    /// Returns [`Matrix::rect_2_rect`], or the identity matrix on failure
    /// (`SkMatrix::MakeRectToRect` / `RectToRect`, behind `SK_SUPPORT_LEGACY_MATRIX_RECTTORECT`).
    // Port of: include/core/SkMatrix.h#L1135-L1145 (chrome/m156)
    #[doc(alias = "MakeRectToRect")]
    #[doc(alias = "RectToRect")]
    #[must_use]
    pub fn make_rect_to_rect(
        src: impl AsRef<Rect>,
        dst: impl AsRef<Rect>,
        stf: ScaleToFit,
    ) -> Self {
        Self::rect_to_rect_or_identity(src, dst, stf)
    }

    // ---- concat ----------------------------------------------------------------------------

    /// Sets the matrix to `a` concatenated with `b` (`a * b`).
    // Port of: src/core/SkMatrix.cpp#L616-L685 (chrome/m156)
    #[doc(alias = "setConcat")]
    pub fn set_concat(&mut self, a: &Self, b: &Self) -> &mut Self {
        let a_type = a.type_bits();
        let b_type = b.type_bits();

        if a.is_trivially_identity() {
            *self = b.clone();
        } else if b.is_trivially_identity() {
            *self = a.clone();
        } else if only_scale_and_translate(a_type | b_type) {
            self.set_scale_translate(
                (
                    a.mat[M_SCALE_X] * b.mat[M_SCALE_X],
                    a.mat[M_SCALE_Y] * b.mat[M_SCALE_Y],
                ),
                Point::new(
                    a.mat[M_SCALE_X] * b.mat[M_TRANS_X] + a.mat[M_TRANS_X],
                    a.mat[M_SCALE_Y] * b.mat[M_TRANS_Y] + a.mat[M_TRANS_Y],
                ),
            );
        } else {
            let mut tmp = Self::new_identity();

            if ((a_type | b_type) & PERSPECTIVE_MASK) != 0 {
                tmp.mat[M_SCALE_X] = rowcol3(&a.mat, 0, &b.mat, 0);
                tmp.mat[M_SKEW_X] = rowcol3(&a.mat, 0, &b.mat, 1);
                tmp.mat[M_TRANS_X] = rowcol3(&a.mat, 0, &b.mat, 2);
                tmp.mat[M_SKEW_Y] = rowcol3(&a.mat, 3, &b.mat, 0);
                tmp.mat[M_SCALE_Y] = rowcol3(&a.mat, 3, &b.mat, 1);
                tmp.mat[M_TRANS_Y] = rowcol3(&a.mat, 3, &b.mat, 2);
                tmp.mat[M_PERSP_0] = rowcol3(&a.mat, 6, &b.mat, 0);
                tmp.mat[M_PERSP_1] = rowcol3(&a.mat, 6, &b.mat, 1);
                tmp.mat[M_PERSP_2] = rowcol3(&a.mat, 6, &b.mat, 2);

                tmp.set_type_mask(UNKNOWN_MASK);
            } else {
                tmp.mat[M_SCALE_X] = muladdmul(
                    a.mat[M_SCALE_X],
                    b.mat[M_SCALE_X],
                    a.mat[M_SKEW_X],
                    b.mat[M_SKEW_Y],
                );

                tmp.mat[M_SKEW_X] = muladdmul(
                    a.mat[M_SCALE_X],
                    b.mat[M_SKEW_X],
                    a.mat[M_SKEW_X],
                    b.mat[M_SCALE_Y],
                );

                tmp.mat[M_TRANS_X] = muladdmul(
                    a.mat[M_SCALE_X],
                    b.mat[M_TRANS_X],
                    a.mat[M_SKEW_X],
                    b.mat[M_TRANS_Y],
                ) + a.mat[M_TRANS_X];

                tmp.mat[M_SKEW_Y] = muladdmul(
                    a.mat[M_SKEW_Y],
                    b.mat[M_SCALE_X],
                    a.mat[M_SCALE_Y],
                    b.mat[M_SKEW_Y],
                );

                tmp.mat[M_SCALE_Y] = muladdmul(
                    a.mat[M_SKEW_Y],
                    b.mat[M_SKEW_X],
                    a.mat[M_SCALE_Y],
                    b.mat[M_SCALE_Y],
                );

                tmp.mat[M_TRANS_Y] = muladdmul(
                    a.mat[M_SKEW_Y],
                    b.mat[M_TRANS_X],
                    a.mat[M_SCALE_Y],
                    b.mat[M_TRANS_Y],
                ) + a.mat[M_TRANS_Y];

                tmp.mat[M_PERSP_0] = 0.0;
                tmp.mat[M_PERSP_1] = 0.0;
                tmp.mat[M_PERSP_2] = 1.0;
                tmp.set_type_mask(UNKNOWN_MASK | ONLY_PERSPECTIVE_VALID_MASK);
            }
            *self = tmp;
        }
        self
    }

    /// Returns `a` concatenated with `b` (`a * b`) (`SkMatrix::Concat`).
    // Port of: include/core/SkMatrix.h#L1704-L1708 (chrome/m156)
    #[doc(alias = "Concat")]
    #[must_use]
    pub fn concat(a: &Matrix, b: &Matrix) -> Matrix {
        let mut result = Self::new_identity();
        result.set_concat(a, b);
        result
    }

    /// Sets the matrix to `self * other`.
    // Port of: src/core/SkMatrix.cpp#L687-L694 (chrome/m156)
    #[doc(alias = "preConcat")]
    pub fn pre_concat(&mut self, other: &Self) -> &mut Self {
        // check for identity first, so we don't do a needless copy of ourselves
        // to ourselves inside setConcat()
        if !other.is_identity() {
            let this = self.clone();
            self.set_concat(&this, other);
        }
        self
    }

    /// Sets the matrix to `other * self`.
    // Port of: src/core/SkMatrix.cpp#L696-L703 (chrome/m156)
    #[doc(alias = "postConcat")]
    pub fn post_concat(&mut self, other: &Self) -> &mut Self {
        // check for identity first, so we don't do a needless copy of ourselves
        // to ourselves inside setConcat()
        if !other.is_identity() {
            let this = self.clone();
            self.set_concat(other, &this);
        }
        self
    }

    // ---- poly to poly ----------------------------------------------------------------------

    // Port of: src/core/SkMatrix.cpp#L1215-L1229 (chrome/m156)
    fn poly_2_proc(src_pt: &[Point], dst: &mut Matrix) -> bool {
        dst.mat[M_SCALE_X] = src_pt[1].y - src_pt[0].y;
        dst.mat[M_SKEW_Y] = src_pt[0].x - src_pt[1].x;
        dst.mat[M_PERSP_0] = 0.0;

        dst.mat[M_SKEW_X] = src_pt[1].x - src_pt[0].x;
        dst.mat[M_SCALE_Y] = src_pt[1].y - src_pt[0].y;
        dst.mat[M_PERSP_1] = 0.0;

        dst.mat[M_TRANS_X] = src_pt[0].x;
        dst.mat[M_TRANS_Y] = src_pt[0].y;
        dst.mat[M_PERSP_2] = 1.0;
        dst.set_type_mask(UNKNOWN_MASK);
        true
    }

    // Port of: src/core/SkMatrix.cpp#L1231-L1245 (chrome/m156)
    fn poly_3_proc(src_pt: &[Point], dst: &mut Matrix) -> bool {
        dst.mat[M_SCALE_X] = src_pt[2].x - src_pt[0].x;
        dst.mat[M_SKEW_Y] = src_pt[2].y - src_pt[0].y;
        dst.mat[M_PERSP_0] = 0.0;

        dst.mat[M_SKEW_X] = src_pt[1].x - src_pt[0].x;
        dst.mat[M_SCALE_Y] = src_pt[1].y - src_pt[0].y;
        dst.mat[M_PERSP_1] = 0.0;

        dst.mat[M_TRANS_X] = src_pt[0].x;
        dst.mat[M_TRANS_Y] = src_pt[0].y;
        dst.mat[M_PERSP_2] = 1.0;
        dst.set_type_mask(UNKNOWN_MASK);
        true
    }

    // Port of: src/core/SkMatrix.cpp#L1247-L1301 (chrome/m156)
    #[allow(clippy::many_single_char_names, clippy::similar_names)] // names follow the C++
    fn poly_4_proc(src_pt: &[Point], dst: &mut Matrix) -> bool {
        let x0 = src_pt[2].x - src_pt[0].x;
        let y0 = src_pt[2].y - src_pt[0].y;
        let x1 = src_pt[2].x - src_pt[1].x;
        let y1 = src_pt[2].y - src_pt[1].y;
        let x2 = src_pt[2].x - src_pt[3].x;
        let y2 = src_pt[2].y - src_pt[3].y;

        /* check if abs(x2) > abs(y2) */
        let x2_gt_y2 = if x2 > 0.0 {
            if y2 > 0.0 { x2 > y2 } else { x2 > -y2 }
        } else if y2 > 0.0 {
            -x2 > y2
        } else {
            x2 < y2
        };
        let a1 = if x2_gt_y2 {
            let denom = ieee_float_divide(x1 * y2, x2) - y1;
            if check_for_zero(denom) {
                return false;
            }
            (((x0 - x1) * y2 / x2) - y0 + y1) / denom
        } else {
            let denom = x1 - ieee_float_divide(y1 * x2, y2);
            if check_for_zero(denom) {
                return false;
            }
            (x0 - x1 - ieee_float_divide((y0 - y1) * x2, y2)) / denom
        };

        /* check if abs(x1) > abs(y1) */
        let x1_gt_y1 = if x1 > 0.0 {
            if y1 > 0.0 { x1 > y1 } else { x1 > -y1 }
        } else if y1 > 0.0 {
            -x1 > y1
        } else {
            x1 < y1
        };
        let a2 = if x1_gt_y1 {
            let denom = y2 - ieee_float_divide(x2 * y1, x1);
            if check_for_zero(denom) {
                return false;
            }
            (y0 - y2 - ieee_float_divide((x0 - x2) * y1, x1)) / denom
        } else {
            let denom = ieee_float_divide(y2 * x1, y1) - x2;
            if check_for_zero(denom) {
                return false;
            }
            (ieee_float_divide((y0 - y2) * x1, y1) - x0 + x2) / denom
        };

        dst.mat[M_SCALE_X] = a2 * src_pt[3].x + src_pt[3].x - src_pt[0].x;
        dst.mat[M_SKEW_Y] = a2 * src_pt[3].y + src_pt[3].y - src_pt[0].y;
        dst.mat[M_PERSP_0] = a2;

        dst.mat[M_SKEW_X] = a1 * src_pt[1].x + src_pt[1].x - src_pt[0].x;
        dst.mat[M_SCALE_Y] = a1 * src_pt[1].y + src_pt[1].y - src_pt[0].y;
        dst.mat[M_PERSP_1] = a1;

        dst.mat[M_TRANS_X] = src_pt[0].x;
        dst.mat[M_TRANS_Y] = src_pt[0].y;
        dst.mat[M_PERSP_2] = 1.0;
        dst.set_type_mask(UNKNOWN_MASK);
        true
    }

    /// Returns the matrix that maps the points of `src` to the points of `dst`, or `None` if
    /// the lengths differ, there are more than four points, or the mapping does not exist
    /// (`SkMatrix::PolyToPoly`).
    ///
    /// Originally adapted from Rob Johnson's original sample code in `QuickDraw` GX.
    // Port of: src/core/SkMatrix.cpp#L1307-L1339 (chrome/m156)
    #[doc(alias = "PolyToPoly")]
    #[must_use]
    pub fn poly_to_poly(src: &[Point], dst: &[Point]) -> Option<Matrix> {
        if src.len() != dst.len() || src.len() > 4 {
            return None;
        }

        match src.len() {
            0 => Some(Self::new_identity()),
            1 => Some(Self::translate(dst[0] - src[0])),
            n => {
                let proc: fn(&[Point], &mut Matrix) -> bool = match n {
                    2 => Self::poly_2_proc,
                    3 => Self::poly_3_proc,
                    _ => Self::poly_4_proc,
                };

                let mut temp_map = Self::new_identity();
                if !proc(src, &mut temp_map) {
                    return None;
                }
                let inverse = temp_map.invert()?;
                if !proc(dst, &mut temp_map) {
                    return None;
                }
                Some(Matrix::concat(&temp_map, &inverse))
            }
        }
    }

    /// Sets the matrix to [`Matrix::poly_to_poly`]; returns false (leaving the matrix
    /// unchanged) on failure (`SkMatrix::setPolyToPoly`).
    // Port of: include/core/SkMatrix.h#L1158-L1164 (chrome/m156)
    #[doc(alias = "setPolyToPoly")]
    pub fn set_poly_to_poly(&mut self, src: &[Point], dst: &[Point]) -> bool {
        if let Some(mx) = Self::poly_to_poly(src, dst) {
            *self = mx;
            return true;
        }
        false
    }

    // ---- invert ----------------------------------------------------------------------------

    // Port of: src/core/SkMatrix.cpp#L788-L817 (chrome/m156)
    fn compute_inv(dst: &mut [scalar; 9], src: &[scalar; 9], inv_det: f64, is_persp: bool) {
        if is_persp {
            dst[M_SCALE_X] = scross_dscale(
                src[M_SCALE_Y],
                src[M_PERSP_2],
                src[M_TRANS_Y],
                src[M_PERSP_1],
                inv_det,
            );
            dst[M_SKEW_X] = scross_dscale(
                src[M_TRANS_X],
                src[M_PERSP_1],
                src[M_SKEW_X],
                src[M_PERSP_2],
                inv_det,
            );
            dst[M_TRANS_X] = scross_dscale(
                src[M_SKEW_X],
                src[M_TRANS_Y],
                src[M_TRANS_X],
                src[M_SCALE_Y],
                inv_det,
            );

            dst[M_SKEW_Y] = scross_dscale(
                src[M_TRANS_Y],
                src[M_PERSP_0],
                src[M_SKEW_Y],
                src[M_PERSP_2],
                inv_det,
            );
            dst[M_SCALE_Y] = scross_dscale(
                src[M_SCALE_X],
                src[M_PERSP_2],
                src[M_TRANS_X],
                src[M_PERSP_0],
                inv_det,
            );
            dst[M_TRANS_Y] = scross_dscale(
                src[M_TRANS_X],
                src[M_SKEW_Y],
                src[M_SCALE_X],
                src[M_TRANS_Y],
                inv_det,
            );

            dst[M_PERSP_0] = scross_dscale(
                src[M_SKEW_Y],
                src[M_PERSP_1],
                src[M_SCALE_Y],
                src[M_PERSP_0],
                inv_det,
            );
            dst[M_PERSP_1] = scross_dscale(
                src[M_SKEW_X],
                src[M_PERSP_0],
                src[M_SCALE_X],
                src[M_PERSP_1],
                inv_det,
            );
            dst[M_PERSP_2] = scross_dscale(
                src[M_SCALE_X],
                src[M_SCALE_Y],
                src[M_SKEW_X],
                src[M_SKEW_Y],
                inv_det,
            );
        } else {
            // not perspective
            dst[M_SCALE_X] = double_to_scalar(f64::from(src[M_SCALE_Y]) * inv_det);
            dst[M_SKEW_X] = double_to_scalar(f64::from(-src[M_SKEW_X]) * inv_det);
            dst[M_TRANS_X] = dcross_dscale(
                f64::from(src[M_SKEW_X]),
                f64::from(src[M_TRANS_Y]),
                f64::from(src[M_SCALE_Y]),
                f64::from(src[M_TRANS_X]),
                inv_det,
            );

            dst[M_SKEW_Y] = double_to_scalar(f64::from(-src[M_SKEW_Y]) * inv_det);
            dst[M_SCALE_Y] = double_to_scalar(f64::from(src[M_SCALE_X]) * inv_det);
            dst[M_TRANS_Y] = dcross_dscale(
                f64::from(src[M_SKEW_Y]),
                f64::from(src[M_TRANS_X]),
                f64::from(src[M_SCALE_X]),
                f64::from(src[M_TRANS_Y]),
                inv_det,
            );

            dst[M_PERSP_0] = 0.0;
            dst[M_PERSP_1] = 0.0;
            dst[M_PERSP_2] = 1.0;
        }
    }

    /// Returns the inverse of the matrix, or `None` if it is not invertible (or the inverse is
    /// not finite).
    // Port of: src/core/SkMatrix.cpp#L819-L882 (chrome/m156)
    #[must_use]
    #[allow(clippy::similar_names)] // inv_sx / inv_sy, inv_tx / inv_ty follow the C++
    pub fn invert(&self) -> Option<Matrix> {
        let mask = self.type_bits();

        if mask == 0 {
            return Some(self.clone());
        }

        // Optimized invert for only scale and/or translation matrices.
        if 0 == (mask & !(SCALE_MASK | TRANSLATE_MASK)) {
            if (mask & SCALE_MASK) != 0 {
                // Scale + (optional) Translate
                let inv_sx = ieee_float_divide(1.0, self.mat[M_SCALE_X]);
                let inv_sy = ieee_float_divide(1.0, self.mat[M_SCALE_Y]);
                // Denormalized (non-zero) scale factors will overflow when inverted, in which case
                // the inverse matrix would not be finite, so return false.
                if !is_finite_all(inv_sx, &[inv_sy]) {
                    return None;
                }
                let inv_tx = -self.mat[M_TRANS_X] * inv_sx;
                let inv_ty = -self.mat[M_TRANS_Y] * inv_sy;
                // Make sure inverse translation didn't overflow/underflow after dividing by scale.
                // Also catches cases where the original matrix's translation values are not
                // finite.
                if !is_finite_all(inv_tx, &[inv_ty]) {
                    return None;
                }

                let mut inv = Self::new_identity();
                inv.mat[M_SKEW_X] = 0.0;
                inv.mat[M_SKEW_Y] = 0.0;
                inv.mat[M_PERSP_0] = 0.0;
                inv.mat[M_PERSP_1] = 0.0;

                inv.mat[M_SCALE_X] = inv_sx;
                inv.mat[M_SCALE_Y] = inv_sy;
                inv.mat[M_PERSP_2] = 1.0;
                inv.mat[M_TRANS_X] = inv_tx;
                inv.mat[M_TRANS_Y] = inv_ty;

                inv.set_type_mask(mask | RECT_STAYS_RECT_MASK);
                return Some(inv);
            }

            // Translate-only
            if !is_finite_all(self.mat[M_TRANS_X], &[self.mat[M_TRANS_Y]]) {
                // Translation components aren't finite, so inverse isn't possible
                return None;
            }

            return Some(Self::translate(Point::new(
                -self.mat[M_TRANS_X],
                -self.mat[M_TRANS_Y],
            )));
        }

        let is_persp = (mask & PERSPECTIVE_MASK) != 0;
        let inv_det = sk_inv_determinant(&self.mat, is_persp);

        if inv_det == 0.0 {
            // underflow
            return None;
        }

        let mut inv = Self::new_identity();
        Self::compute_inv(&mut inv.mat, &self.mat, inv_det, is_persp);
        if !inv.is_finite() {
            return None;
        }
        inv.set_type_mask(self.mask());
        Some(inv)
    }

    // ---- affine ----------------------------------------------------------------------------

    /// Sets `affine` to the identity, in the order of [`AffineMember`].
    // Port of: src/core/SkMatrix.cpp#L759-L766 (chrome/m156)
    #[doc(alias = "SetAffineIdentity")]
    pub fn set_affine_identity(affine: &mut [scalar; 6]) {
        affine[AffineMember::ScaleX as usize] = 1.0;
        affine[AffineMember::SkewY as usize] = 0.0;
        affine[AffineMember::SkewX as usize] = 0.0;
        affine[AffineMember::ScaleY as usize] = 1.0;
        affine[AffineMember::TransX as usize] = 0.0;
        affine[AffineMember::TransY as usize] = 0.0;
    }

    /// Returns the six affine members (in the order of [`AffineMember`]), or `None` if the
    /// matrix has perspective (`SkMatrix::asAffine`).
    // Port of: src/core/SkMatrix.cpp#L768-L781 (chrome/m156)
    #[doc(alias = "asAffine")]
    #[must_use]
    pub fn to_affine(&self) -> Option<[scalar; 6]> {
        if self.has_perspective() {
            return None;
        }
        let mut affine = [0.0; 6];
        affine[AffineMember::ScaleX as usize] = self.mat[M_SCALE_X];
        affine[AffineMember::SkewY as usize] = self.mat[M_SKEW_Y];
        affine[AffineMember::SkewX as usize] = self.mat[M_SKEW_X];
        affine[AffineMember::ScaleY as usize] = self.mat[M_SCALE_Y];
        affine[AffineMember::TransX as usize] = self.mat[M_TRANS_X];
        affine[AffineMember::TransY as usize] = self.mat[M_TRANS_Y];
        Some(affine)
    }

    /// Sets the matrix from the six affine members (in the order of [`AffineMember`]).
    // Port of: src/core/SkMatrix.cpp#L62-L74 (chrome/m156)
    #[doc(alias = "setAffine")]
    pub fn set_affine(&mut self, affine: &[scalar; 6]) -> &mut Self {
        self.mat[M_SCALE_X] = affine[AffineMember::ScaleX as usize];
        self.mat[M_SKEW_X] = affine[AffineMember::SkewX as usize];
        self.mat[M_TRANS_X] = affine[AffineMember::TransX as usize];
        self.mat[M_SKEW_Y] = affine[AffineMember::SkewY as usize];
        self.mat[M_SCALE_Y] = affine[AffineMember::ScaleY as usize];
        self.mat[M_TRANS_Y] = affine[AffineMember::TransY as usize];
        self.mat[M_PERSP_0] = 0.0;
        self.mat[M_PERSP_1] = 0.0;
        self.mat[M_PERSP_2] = 1.0;
        self.set_type_mask(UNKNOWN_MASK);
        self
    }

    /// Returns the matrix for the six affine members.
    #[doc(alias = "setAffine")]
    #[must_use]
    pub fn from_affine(affine: &[scalar; 6]) -> Matrix {
        let mut m = Self::new_identity();
        m.set_affine(affine);
        m
    }

    /// If the bottom row is `[0, 0, not_one]`, divides everything by `not_one`, so the matrix
    /// is no longer treated as perspective.
    // Port of: include/core/SkMatrix.h#L1235-L1239 and src/core/SkMatrix.cpp#L35-L52 (chrome/m156)
    #[doc(alias = "normalizePerspective")]
    pub fn normalize_perspective(&mut self) {
        if self.mat[8] != 1.0 {
            self.do_normalize_perspective();
        }
    }

    fn do_normalize_perspective(&mut self) {
        // If the bottom row of the matrix is [0, 0, not_one], we will treat the matrix as if it
        // is in perspective, even though it stills behaves like its affine. If we divide everything
        // by the not_one value, then it will behave the same, but will be treated as affine,
        // and therefore faster (e.g. clients can forward-difference calculations).
        if 0.0 == self.mat[M_PERSP_0] && 0.0 == self.mat[M_PERSP_1] {
            let p2 = self.mat[M_PERSP_2];
            if p2 != 0.0 && p2 != 1.0 {
                let inv = 1.0 / f64::from(p2);
                for v in &mut self.mat[..6] {
                    *v = double_to_scalar(f64::from(*v) * inv);
                }
                self.mat[M_PERSP_2] = 1.0;
            }
            self.set_type_mask(UNKNOWN_MASK);
        }
    }

    // ---- map points ------------------------------------------------------------------------

    /// Maps `src` into `dst` (the shorter of the two lengths is mapped); `dst` and `src` may
    /// not overlap.
    // Port of: src/core/SkMatrix.cpp#L783-L786 (chrome/m156)
    #[doc(alias = "mapPoints")]
    pub fn map_points(&self, dst: &mut [Point], src: &[Point]) {
        self.map_points_proc(dst, Some(src));
    }

    /// Maps `pts` in place.
    // Port of: include/core/SkMatrix.h#L1294-L1296 (chrome/m156)
    #[doc(alias = "mapPoints")]
    pub fn map_points_inplace(&self, pts: &mut [Point]) {
        self.map_points_proc(pts, None);
    }

    // Dispatches on the matrix type like `gMapPtsProcs`. A `None` source reads the points from
    // `dst` (the C++ passes the same pointer for both).
    // Port of: src/core/SkMatrix.cpp#L1021-L1031 (chrome/m156)
    fn map_points_proc(&self, dst: &mut [Point], src: Option<&[Point]>) {
        let count = src.map_or(dst.len(), |s| dst.len().min(s.len()));
        match self.type_bits() {
            0 => identity_pts(dst, src, count),
            1 => trans_pts(self, dst, src, count),
            2 | 3 => scale_pts(self, dst, src, count),
            4..=7 => affine_vpts(self, dst, src, count),
            // repeat the persp proc 8 times
            _ => persp_pts(self, dst, src, count),
        }
    }

    /// Maps the homogeneous points of `src` into `dst` (the shorter of the two lengths).
    // Port of: src/core/SkMatrix.cpp#L1035-L1079 (chrome/m156)
    #[doc(alias = "mapHomogeneousPoints")]
    pub fn map_homogeneous_points(&self, dst: &mut [Point3], src: &[Point3]) {
        let count = dst.len().min(src.len());
        if count == 0 {
            return;
        }
        if self.is_identity() {
            dst[..count].copy_from_slice(&src[..count]);
            return;
        }
        for i in 0..count {
            let sx = src[i].x;
            let sy = src[i].y;
            let sw = src[i].z;
            let mat = &self.mat;
            let x = sdot3(sx, mat[M_SCALE_X], sy, mat[M_SKEW_X], sw, mat[M_TRANS_X]);
            let y = sdot3(sx, mat[M_SKEW_Y], sy, mat[M_SCALE_Y], sw, mat[M_TRANS_Y]);
            let w = sdot3(sx, mat[M_PERSP_0], sy, mat[M_PERSP_1], sw, mat[M_PERSP_2]);

            dst[i].set(x, y, w);
        }
    }

    /// Maps a single homogeneous point.
    // Port of: include/core/SkMatrix.h#L1320-L1324 (chrome/m156)
    #[doc(alias = "mapHomogeneousPoint")]
    #[must_use]
    pub fn map_homogeneous_point(&self, src: Point3) -> Point3 {
        let mut dst = [Point3::default()];
        self.map_homogeneous_points(&mut dst, &[src]);
        dst[0]
    }

    /// Maps the points of `src` to homogeneous coordinates in `dst` (the shorter of the two
    /// lengths).
    // Port of: src/core/SkMatrix.cpp#L1081-L1105 (chrome/m156)
    #[doc(alias = "mapPointsToHomogeneous")]
    pub fn map_points_to_homogeneous(&self, dst: &mut [Point3], src: &[Point]) {
        let count = dst.len().min(src.len());
        let fmat = &self.mat;

        if self.is_identity() {
            for i in 0..count {
                dst[i] = Point3::new(src[i].x, src[i].y, 1.0);
            }
        } else if self.has_perspective() {
            for i in 0..count {
                dst[i] = Point3::new(
                    fmat[0] * src[i].x + fmat[1] * src[i].y + fmat[2],
                    fmat[3] * src[i].x + fmat[4] * src[i].y + fmat[5],
                    fmat[6] * src[i].x + fmat[7] * src[i].y + fmat[8],
                );
            }
        } else {
            // affine
            for i in 0..count {
                dst[i] = Point3::new(
                    fmat[0] * src[i].x + fmat[1] * src[i].y + fmat[2],
                    fmat[3] * src[i].x + fmat[4] * src[i].y + fmat[5],
                    1.0,
                );
            }
        }
    }

    /// Maps a single point to homogeneous coordinates.
    // Port of: include/core/SkMatrix.h#L1334-L1338 (chrome/m156)
    #[doc(alias = "mapPointToHomogeneous")]
    #[must_use]
    pub fn map_point_to_homogeneous(&self, src: impl Into<Point>) -> Point3 {
        let mut dst = [Point3::default()];
        self.map_points_to_homogeneous(&mut dst, &[src.into()]);
        dst[0]
    }

    /// Maps `point`, performing the perspective divide if the matrix has perspective.
    // Port of: include/core/SkMatrix.h#L1355-L1361 (chrome/m156)
    #[doc(alias = "mapPoint")]
    #[must_use]
    pub fn map_point(&self, point: impl Into<Point>) -> Point {
        let point = point.into();
        if self.has_perspective() {
            self.map_point_perspective(point)
        } else {
            self.map_point_affine(point)
        }
    }

    /// Maps `(x, y)` (see [`Matrix::map_point`]).
    #[doc(alias = "mapXY")]
    #[must_use]
    pub fn map_xy(&self, x: scalar, y: scalar) -> Point {
        self.map_point((x, y))
    }

    /// Maps `point` ignoring any perspective members; the matrix must not have perspective.
    // Port of: include/core/SkMatrix.h#L1367-L1373 (chrome/m156)
    #[doc(alias = "mapPointAffine")]
    #[must_use]
    pub fn map_point_affine(&self, point: impl Into<Point>) -> Point {
        debug_assert!(!self.has_perspective());
        let p = point.into();
        let fmat = &self.mat;
        Point::new(
            (p.x * fmat[0] + p.y * fmat[1]) + fmat[2],
            (p.x * fmat[3] + p.y * fmat[4]) + fmat[5],
        )
    }

    // Port of: src/core/SkMatrix.cpp#L884-L892 (chrome/m156)
    pub(crate) fn map_point_perspective(&self, p: Point) -> Point {
        let fmat = &self.mat;
        let x = sdot(p.x, fmat[M_SCALE_X], p.y, fmat[M_SKEW_X]) + fmat[M_TRANS_X];
        let y = sdot(p.x, fmat[M_SKEW_Y], p.y, fmat[M_SCALE_Y]) + fmat[M_TRANS_Y];
        let mut z = sdot(p.x, fmat[M_PERSP_0], p.y, fmat[M_PERSP_1]) + fmat[M_PERSP_2];
        if z != 0.0 {
            z = 1.0 / z;
        }
        Point::new(x * z, y * z)
    }

    /// Maps the origin `(0, 0)`.
    // Port of: include/core/SkMatrix.h#L1389-L1399 (chrome/m156)
    #[doc(alias = "mapOrigin")]
    #[must_use]
    pub fn map_origin(&self) -> Point {
        let mut x = self.translate_x();
        let mut y = self.translate_y();
        if self.has_perspective() {
            let mut w = self.mat[M_PERSP_2];
            if w != 0.0 {
                w = 1.0 / w;
            }
            x *= w;
            y *= w;
        }
        Point::new(x, y)
    }

    /// Maps the vectors of `src` into `dst` (the shorter of the two lengths), ignoring
    /// translation.
    // Port of: src/core/SkMatrix.cpp#L1109-L1123 (chrome/m156)
    #[doc(alias = "mapVectors")]
    pub fn map_vectors(&self, dst: &mut [Vector], src: &[Vector]) {
        self.map_vectors_proc(dst, Some(src));
    }

    /// Maps `vecs` in place, ignoring translation.
    // Port of: include/core/SkMatrix.h#L1465-L1467 (chrome/m156)
    #[doc(alias = "mapVectors")]
    pub fn map_vectors_inplace(&self, vecs: &mut [Vector]) {
        self.map_vectors_proc(vecs, None);
    }

    fn map_vectors_proc(&self, dst: &mut [Vector], src: Option<&[Vector]>) {
        if self.has_perspective() {
            let origin = self.map_point_perspective(Point::new(0.0, 0.0));

            let count = src.map_or(dst.len(), |s| dst.len().min(s.len()));
            for i in (0..count).rev() {
                dst[i] = self.map_point_perspective(src_at(dst, src, i)) - origin;
            }
        } else {
            let mut tmp = self.clone();

            tmp.mat[M_TRANS_X] = 0.0;
            tmp.mat[M_TRANS_Y] = 0.0;
            tmp.clear_type_mask(TRANSLATE_MASK);
            tmp.map_points_proc(dst, src);
        }
    }

    /// Maps the vector `vec`, ignoring translation.
    // Port of: include/core/SkMatrix.h#L1476-L1483 (chrome/m156)
    #[doc(alias = "mapVector")]
    #[must_use]
    pub fn map_vector(&self, vec: impl Into<Vector>) -> Vector {
        let mut vec = [vec.into()];
        self.map_vectors_inplace(&mut vec);
        vec[0]
    }

    /// Sets `dst` to the bounds of `src` mapped by the matrix; also returns true if the mapped
    /// rectangle is still an axis-aligned rectangle (see [`Matrix::rect_stays_rect`]). With
    /// perspective, the rectangle is clipped to the `w > 0` half-space first.
    // Port of: src/core/SkMatrix.cpp#L1147-L1172 (chrome/m156)
    #[doc(alias = "mapRect")]
    #[must_use]
    pub fn map_rect(&self, src: impl AsRef<Rect>) -> (Rect, bool) {
        let src = src.as_ref();

        if self.type_bits() <= TRANSLATE_MASK {
            let tx = self.mat[M_TRANS_X];
            let ty = self.mat[M_TRANS_Y];
            let trans = Float4::new(tx, ty, tx, ty);
            let dst = rect_from_float4(sort_as_rect(rect_to_float4(src) + trans));
            return (dst, true);
        }
        if self.is_scale_translate() {
            return (self.map_rect_scale_translate_unchecked(src), true);
        } else if self.has_perspective() {
            let mut builder = PathBuilder::new();
            builder.add_rect(src, None, None);
            builder.transform(self);
            return (builder.compute_bounds(), false);
        }
        let mut quad = to_quad_cw(src);
        self.map_points_inplace(&mut quad);
        let mut dst = Rect::default();
        dst.set_bounds_no_check(&quad);
        (dst, self.rect_stays_rect()) // might still return true if rotated by 90, etc.
    }

    /// Returns the four corners of `rect` mapped by the matrix (in `SkRect::toQuad` order).
    // Port of: include/core/SkMatrix.h#L1550-L1552 (chrome/m156)
    #[doc(alias = "mapRectToQuad")]
    #[must_use]
    pub fn map_rect_to_quad(&self, rect: impl AsRef<Rect>) -> [Point; 4] {
        let mut quad = to_quad_cw(rect.as_ref());
        self.map_points_inplace(&mut quad);
        quad
    }

    /// Maps `src` by a scale-translate matrix; returns `None` if the matrix is not
    /// scale-translate (`SkMatrix::mapRectScaleTranslate`).
    // Port of: src/core/SkMatrix.cpp#L1134-L1145 (chrome/m156)
    #[doc(alias = "mapRectScaleTranslate")]
    #[must_use]
    pub fn map_rect_scale_translate(&self, src: impl AsRef<Rect>) -> Option<Rect> {
        if self.is_scale_translate() {
            Some(self.map_rect_scale_translate_unchecked(src.as_ref()))
        } else {
            None
        }
    }

    fn map_rect_scale_translate_unchecked(&self, src: &Rect) -> Rect {
        debug_assert!(self.is_scale_translate());

        let sx = self.mat[M_SCALE_X];
        let sy = self.mat[M_SCALE_Y];
        let tx = self.mat[M_TRANS_X];
        let ty = self.mat[M_TRANS_Y];
        let scale = Float4::new(sx, sy, sx, sy);
        let trans = Float4::new(tx, ty, tx, ty);
        rect_from_float4(sort_as_rect(rect_to_float4(src) * scale + trans))
    }

    /// Returns the geometric mean of the radii of the ellipse that results from mapping a
    /// circle of radius `radius` by the matrix (`SkMatrix::mapRadius`).
    // Port of: src/core/SkMatrix.cpp#L1174-L1186 (chrome/m156)
    #[doc(alias = "mapRadius")]
    #[must_use]
    pub fn map_radius(&self, radius: scalar) -> scalar {
        let mut vec = [Vector::new(radius, 0.0), Vector::new(0.0, radius)];
        self.map_vectors_inplace(&mut vec);

        let d0 = vec[0].length();
        let d1 = vec[1].length();

        // return geometric mean
        scalar_sqrt(d0 * d1)
    }

    // ---- scale factors ---------------------------------------------------------------------

    /// Returns the minimum scaling factor, or -1 if the matrix has perspective or the factor
    /// is not finite (`SkMatrix::getMinScale`).
    // Port of: src/core/SkMatrix.cpp#L1443-L1450 (chrome/m156)
    #[doc(alias = "getMinScale")]
    #[must_use]
    pub fn min_scale(&self) -> scalar {
        match get_scale_factor(MinMaxOrBoth::Min, self.get_type(), &self.mat) {
            Some((factor, _)) => factor,
            None => -1.0,
        }
    }

    /// Returns the maximum scaling factor, or -1 if the matrix has perspective or the factor
    /// is not finite (`SkMatrix::getMaxScale`).
    // Port of: src/core/SkMatrix.cpp#L1452-L1459 (chrome/m156)
    #[doc(alias = "getMaxScale")]
    #[must_use]
    pub fn max_scale(&self) -> scalar {
        match get_scale_factor(MinMaxOrBoth::Max, self.get_type(), &self.mat) {
            Some((factor, _)) => factor,
            None => -1.0,
        }
    }

    /// Returns the `(min, max)` scaling factors, or `None` if the matrix has perspective or a
    /// factor is not finite (`SkMatrix::getMinMaxScales`).
    // Port of: src/core/SkMatrix.cpp#L1461-L1463 (chrome/m156)
    #[doc(alias = "getMinMaxScales")]
    #[must_use]
    pub fn min_max_scales(&self) -> Option<(scalar, scalar)> {
        get_scale_factor(MinMaxOrBoth::Both, self.get_type(), &self.mat)
    }

    /// Decomposes the matrix into a scale (returned) followed by `remaining`, if the matrix
    /// has no perspective and the scales are finite and non-zero (`SkMatrix::decomposeScale`).
    // Port of: src/core/SkMatrix.cpp#L1480-L1500 (chrome/m156)
    #[doc(alias = "decomposeScale")]
    #[must_use]
    pub fn decompose_scale(&self, remaining: Option<&mut Matrix>) -> Option<Size> {
        if self.has_perspective() {
            return None;
        }

        let sx = Point::length_xy(self.scale_x(), self.skew_y());
        let sy = Point::length_xy(self.skew_x(), self.scale_y());
        if !is_finite_all(sx, &[sy]) || sx.nearly_zero(None) || sy.nearly_zero(None) {
            return None;
        }

        if let Some(remaining) = remaining {
            *remaining = self.clone();
            remaining.pre_scale((scalar_invert(sx), scalar_invert(sy)), None);
        }
        Some(Size::new(sx, sy))
    }

    /// Invalidates the cached matrix type; call after members changed through a path other
    /// than the setters (`SkMatrix::dirtyMatrixTypeCache`).
    // Port of: include/core/SkMatrix.h#L1717-L1719 (chrome/m156)
    #[doc(alias = "dirtyMatrixTypeCache")]
    pub fn dirty_matrix_type_cache(&mut self) {
        self.set_type_mask(UNKNOWN_MASK);
    }

    /// Returns true if all nine members are finite.
    // Port of: include/core/SkMatrix.h#L1741 (chrome/m156)
    #[doc(alias = "isFinite")]
    #[must_use]
    pub fn is_finite(&self) -> bool {
        is_finite_array(&self.mat)
    }
}

// Private helpers shared with `matrix_priv`.
impl Matrix {
    /// The nine members.
    pub(crate) fn members(&self) -> &[scalar; 9] {
        &self.mat
    }
}

// ---- file-static helpers of SkMatrix.cpp ---------------------------------------------------

// helper function to determine if upper-left 2x2 of matrix is degenerate
// Port of: src/core/SkMatrix.cpp#L176-L181 (chrome/m156)
pub(crate) fn is_degenerate_2x2(
    scale_x: scalar,
    skew_x: scalar,
    skew_y: scalar,
    scale_y: scalar,
) -> bool {
    let perp_dot = scale_x * scale_y - skew_x * skew_y;
    perp_dot.nearly_zero(SCALAR_NEARLY_ZERO * SCALAR_NEARLY_ZERO)
}

// Port of: src/core/SkMatrix.cpp#L246-L248 (chrome/m156)
fn sdot(a: scalar, b: scalar, c: scalar, d: scalar) -> scalar {
    a * b + c * d
}

// Port of: src/core/SkMatrix.cpp#L250-L253 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn sdot3(a: scalar, b: scalar, c: scalar, d: scalar, e: scalar, f: scalar) -> scalar {
    a * b + c * d + e * f
}

// Port of: src/core/SkMatrix.cpp#L255-L257 (chrome/m156)
fn scross(a: scalar, b: scalar, c: scalar, d: scalar) -> scalar {
    a * b - c * d
}

// Port of: src/core/SkMatrix.cpp#L604-L606 (chrome/m156)
fn muladdmul(a: f32, b: f32, c: f32, d: f32) -> f32 {
    double_to_float(f64::from(a) * f64::from(b) + f64::from(c) * f64::from(d))
}

// Port of: src/core/SkMatrix.cpp#L608-L610 (chrome/m156)
fn rowcol3(row: &[f32; 9], row_start: usize, col: &[f32; 9], col_start: usize) -> f32 {
    row[row_start] * col[col_start]
        + row[row_start + 1] * col[col_start + 3]
        + row[row_start + 2] * col[col_start + 6]
}

// Port of: src/core/SkMatrix.cpp#L612-L614 (chrome/m156)
fn only_scale_and_translate(mask: u32) -> bool {
    0 == (mask & (AFFINE_MASK | PERSPECTIVE_MASK))
}

// Port of: src/core/SkMatrix.cpp#L713-L716 (chrome/m156)
fn scross_dscale(a: scalar, b: scalar, c: scalar, d: scalar, scale: f64) -> scalar {
    double_to_scalar(f64::from(scross(a, b, c, d)) * scale)
}

// Port of: src/core/SkMatrix.cpp#L718-L720 (chrome/m156)
fn dcross(a: f64, b: f64, c: f64, d: f64) -> f64 {
    a * b - c * d
}

// Port of: src/core/SkMatrix.cpp#L722-L725 (chrome/m156)
fn dcross_dscale(a: f64, b: f64, c: f64, d: f64, scale: f64) -> scalar {
    double_to_scalar(dcross(a, b, c, d) * scale)
}

// Port of: src/core/SkMatrix.cpp#L727-L744 (chrome/m156)
pub(crate) fn sk_determinant(mat: &[f32; 9], is_perspective: bool) -> f64 {
    let m = |i: usize| f64::from(mat[i]);
    if is_perspective {
        m(M_SCALE_X) * dcross(m(M_SCALE_Y), m(M_PERSP_2), m(M_TRANS_Y), m(M_PERSP_1))
            + m(M_SKEW_X) * dcross(m(M_TRANS_Y), m(M_PERSP_0), m(M_SKEW_Y), m(M_PERSP_2))
            + m(M_TRANS_X) * dcross(m(M_SKEW_Y), m(M_PERSP_1), m(M_SCALE_Y), m(M_PERSP_0))
    } else {
        dcross(m(M_SCALE_X), m(M_SCALE_Y), m(M_SKEW_X), m(M_SKEW_Y))
    }
}

// Port of: src/core/SkMatrix.cpp#L746-L757 (chrome/m156)
fn sk_inv_determinant(mat: &[f32; 9], is_perspective: bool) -> f64 {
    let det = sk_determinant(mat, is_perspective);

    // Since the determinant is on the order of the cube of the matrix members,
    // compare to the cube of the default nearly-zero constant (although an
    // estimate of the condition number would be better if it wasn't so expensive).
    if double_to_float(det)
        .nearly_zero(SCALAR_NEARLY_ZERO * SCALAR_NEARLY_ZERO * SCALAR_NEARLY_ZERO)
    {
        return 0.0;
    }
    1.0 / det
}

// Port of: src/core/SkMatrix.cpp#L1211-L1213 (chrome/m156)
fn check_for_zero(x: f32) -> bool {
    x * x == 0.0
}

// `src[i]` of a possibly in-place source.
fn src_at(dst: &[Point], src: Option<&[Point]>, i: usize) -> Point {
    match src {
        Some(s) => s[i],
        None => dst[i],
    }
}

// Loads two points as `skvx::float4::Load(src)`.
fn load2(dst: &[Point], src: Option<&[Point]>, i: usize) -> Float4 {
    let (p0, p1) = (src_at(dst, src, i), src_at(dst, src, i + 1));
    Float4::new(p0.x, p0.y, p1.x, p1.y)
}

// Stores two points as `float4.store(dst)`.
fn store2(dst: &mut [Point], i: usize, v: Float4) {
    dst[i] = Point::new(v[0], v[1]);
    dst[i + 1] = Point::new(v[2], v[3]);
}

// Port of: src/core/SkMatrix.cpp#L896-L902 (chrome/m156)
fn identity_pts(dst: &mut [Point], src: Option<&[Point]>, count: usize) {
    if let Some(src) = src {
        dst[..count].copy_from_slice(&src[..count]);
    }
}

// Port of: src/core/SkMatrix.cpp#L904-L930 (chrome/m156)
fn trans_pts(m: &Matrix, dst: &mut [Point], src: Option<&[Point]>, count: usize) {
    debug_assert!(m.type_bits() <= TRANSLATE_MASK);
    if count > 0 {
        let tx = m.translate_x();
        let ty = m.translate_y();
        let mut i = 0;
        let mut count = count;
        if (count & 1) != 0 {
            let p = src_at(dst, src, i);
            dst[i] = Point::new(p.x + tx, p.y + ty);
            i += 1;
        }
        let trans4 = Float4::new(tx, ty, tx, ty);
        count >>= 1;
        if (count & 1) != 0 {
            let v = load2(dst, src, i) + trans4;
            store2(dst, i, v);
            i += 2;
        }
        count >>= 1;
        for _ in 0..count {
            let v0 = load2(dst, src, i) + trans4;
            store2(dst, i, v0);
            let v1 = load2(dst, src, i + 2) + trans4;
            store2(dst, i + 2, v1);
            i += 4;
        }
    }
}

// Port of: src/core/SkMatrix.cpp#L932-L963 (chrome/m156)
fn scale_pts(m: &Matrix, dst: &mut [Point], src: Option<&[Point]>, count: usize) {
    debug_assert!(m.type_bits() <= (SCALE_MASK | TRANSLATE_MASK));
    if count > 0 {
        let tx = m.translate_x();
        let ty = m.translate_y();
        let sx = m.scale_x();
        let sy = m.scale_y();
        let trans4 = Float4::new(tx, ty, tx, ty);
        let scale4 = Float4::new(sx, sy, sx, sy);
        let mut i = 0;
        let mut count = count;
        if (count & 1) != 0 {
            let s = src_at(dst, src, i);
            let p = Float4::new(s.x, s.y, 0.0, 0.0);
            let p = p * scale4 + trans4;
            dst[i] = Point::new(p[0], p[1]);
            i += 1;
        }
        count >>= 1;
        if (count & 1) != 0 {
            let v = load2(dst, src, i) * scale4 + trans4;
            store2(dst, i, v);
            i += 2;
        }
        count >>= 1;
        for _ in 0..count {
            let v0 = load2(dst, src, i) * scale4 + trans4;
            store2(dst, i, v0);
            let v1 = load2(dst, src, i + 2) * scale4 + trans4;
            store2(dst, i + 2, v1);
            i += 4;
        }
    }
}

// Port of: src/core/SkMatrix.cpp#L965-L987 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn persp_pts(m: &Matrix, dst: &mut [Point], src: Option<&[Point]>, count: usize) {
    debug_assert!(m.has_perspective());

    for i in 0..count {
        let s = src_at(dst, src, i);
        let (sy, sx) = (s.y, s.x);
        let fmat = m.members();

        let x = sdot(sx, fmat[M_SCALE_X], sy, fmat[M_SKEW_X]) + fmat[M_TRANS_X];
        let y = sdot(sx, fmat[M_SKEW_Y], sy, fmat[M_SCALE_Y]) + fmat[M_TRANS_Y];
        let mut z = sdot(sx, fmat[M_PERSP_0], sy, fmat[M_PERSP_1]) + fmat[M_PERSP_2];
        if z != 0.0 {
            z = 1.0 / z;
        }

        dst[i] = Point::new(x * z, y * z);
    }
}

// Port of: src/core/SkMatrix.cpp#L989-L1019 (chrome/m156)
fn affine_vpts(m: &Matrix, dst: &mut [Point], src: Option<&[Point]>, count: usize) {
    debug_assert!(m.type_bits() != PERSPECTIVE_MASK);
    if count > 0 {
        let tx = m.translate_x();
        let ty = m.translate_y();
        let sx = m.scale_x();
        let sy = m.scale_y();
        let kx = m.skew_x();
        let ky = m.skew_y();
        let trans4 = Float4::new(tx, ty, tx, ty);
        let scale4 = Float4::new(sx, sy, sx, sy);
        let skew4 = Float4::new(kx, ky, kx, ky); // applied to swizzle of src4
        let trailing_element = (count & 1) != 0;
        let pairs = count >> 1;
        let mut i = 0;
        for _ in 0..pairs {
            let src4 = load2(dst, src, i);
            let swz4 = shuffle(src4, [1, 0, 3, 2]); // y0 x0, y1 x1
            let v = src4 * scale4 + swz4 * skew4 + trans4;
            store2(dst, i, v);
            i += 2;
        }
        if trailing_element {
            // We use the same logic here to ensure that the math stays consistent throughout, even
            // though the high float2 is ignored.
            let s = src_at(dst, src, i);
            let src4 = Float4::new(s.x, s.y, 0.0, 0.0);
            let swz4 = shuffle(src4, [1, 0, 3, 2]); // y0 x0, y1 x1
            let v = src4 * scale4 + swz4 * skew4 + trans4;
            dst[i] = Point::new(v[0], v[1]);
        }
    }
}

// Port of: src/core/SkMatrix.cpp#L1125-L1132 (chrome/m156)
pub(crate) fn sort_as_rect(ltrb: Float4) -> Float4 {
    let rblt = Float4::new(ltrb[2], ltrb[3], ltrb[0], ltrb[1]);
    let min = ltrb.min(rblt);
    let max = ltrb.max(rblt);
    // We can extract either pair [0,1] or [2,3] from min and max and be correct, but on
    // ARM this sequence generates the fastest (a single instruction).
    Float4::new(min[2], min[3], max[0], max[1])
}

// `skvx::float4::Load(&rect.fLeft)`.
pub(crate) fn rect_to_float4(r: &Rect) -> Float4 {
    Float4::new(r.left, r.top, r.right, r.bottom)
}

// `float4.store(&rect.fLeft)`.
pub(crate) fn rect_from_float4(v: Float4) -> Rect {
    Rect::new(v[0], v[1], v[2], v[3])
}

// `SkRect::toQuad(SkPathDirection::kCW)`; `SkRect::toQuad` itself needs `SkPathDirection`.
// Port of: src/core/SkRect.cpp#L61-L82 (chrome/m156)
pub(crate) fn to_quad_cw(r: &Rect) -> [Point; 4] {
    [
        Point::new(r.left, r.top),
        Point::new(r.right, r.top),
        Point::new(r.right, r.bottom),
        Point::new(r.left, r.bottom),
    ]
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum MinMaxOrBoth {
    Min,
    Max,
    Both,
}

// Port of: src/core/SkMatrix.cpp#L1349-L1441 (chrome/m156)
// Returns `(results[0], results[1])`; for `Min`/`Max` only the first is meaningful.
#[allow(clippy::many_single_char_names)] // names follow the C++
#[allow(clippy::manual_midpoint)] // `(a + c) / 2.f` must be evaluated exactly like this
fn get_scale_factor(
    min_max_or_both: MinMaxOrBoth,
    type_mask: TypeMask,
    m: &[scalar; 9],
) -> Option<(scalar, scalar)> {
    let type_bits = type_mask.bits();
    if (type_bits & PERSPECTIVE_MASK) != 0 {
        return None;
    }
    if 0 == type_bits {
        return Some((1.0, 1.0));
    }
    let mut results = [0.0f32; 2];
    if (type_bits & AFFINE_MASK) == 0 {
        match min_max_or_both {
            MinMaxOrBoth::Min => {
                results[0] = std_min(scalar_abs(m[M_SCALE_X]), scalar_abs(m[M_SCALE_Y]));
            }
            MinMaxOrBoth::Max => {
                results[0] = std_max(scalar_abs(m[M_SCALE_X]), scalar_abs(m[M_SCALE_Y]));
            }
            MinMaxOrBoth::Both => {
                results[0] = scalar_abs(m[M_SCALE_X]);
                results[1] = scalar_abs(m[M_SCALE_Y]);
                if results[0] > results[1] {
                    results.swap(0, 1);
                }
            }
        }
        return Some((results[0], results[1]));
    }
    // ignore the translation part of the matrix, just look at 2x2 portion.
    // compute singular values, take largest or smallest abs value.
    // [a b; b c] = A^T*A
    let a = sdot(m[M_SCALE_X], m[M_SCALE_X], m[M_SKEW_Y], m[M_SKEW_Y]);
    let b = sdot(m[M_SCALE_X], m[M_SKEW_X], m[M_SCALE_Y], m[M_SKEW_Y]);
    let c = sdot(m[M_SKEW_X], m[M_SKEW_X], m[M_SCALE_Y], m[M_SCALE_Y]);
    // eigenvalues of A^T*A are the squared singular values of A.
    // characteristic equation is det((A^T*A) - l*I) = 0
    // l^2 - (a + c)l + (ac-b^2)
    // solve using quadratic equation (divisor is non-zero since l^2 has 1 coeff
    // and roots are guaranteed to be pos and real).
    let b_sqd = b * b;
    // if upper left 2x2 is orthogonal save some math
    if b_sqd <= SCALAR_NEARLY_ZERO * SCALAR_NEARLY_ZERO {
        match min_max_or_both {
            MinMaxOrBoth::Min => results[0] = std_min(a, c),
            MinMaxOrBoth::Max => results[0] = std_max(a, c),
            MinMaxOrBoth::Both => {
                results[0] = a;
                results[1] = c;
                if results[0] > results[1] {
                    results.swap(0, 1);
                }
            }
        }
    } else {
        let aminusc = a - c;
        let apluscdiv2 = (a + c) / 2.0;
        let x = (aminusc * aminusc + 4.0 * b_sqd).sqrt() / 2.0;
        match min_max_or_both {
            MinMaxOrBoth::Min => results[0] = apluscdiv2 - x,
            MinMaxOrBoth::Max => results[0] = apluscdiv2 + x,
            MinMaxOrBoth::Both => {
                results[0] = apluscdiv2 - x;
                results[1] = apluscdiv2 + x;
            }
        }
    }
    if !is_finite(results[0]) {
        return None;
    }
    // Due to the floating point inaccuracy, there might be an error in a, b, c
    // calculated by sdot, further deepened by subsequent arithmetic operations
    // on them. Therefore, we allow and cap the nearly-zero negative values.
    if results[0] < 0.0 {
        results[0] = 0.0;
    }
    results[0] = scalar_sqrt(results[0]);
    if min_max_or_both == MinMaxOrBoth::Both {
        if !is_finite(results[1]) {
            return None;
        }
        if results[1] < 0.0 {
            results[1] = 0.0;
        }
        results[1] = scalar_sqrt(results[1]);
    }
    Some((results[0], results[1]))
}
