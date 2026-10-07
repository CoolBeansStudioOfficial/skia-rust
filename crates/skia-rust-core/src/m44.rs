// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkM44.h, src/core/SkM44.cpp

//! 4x4 matrices and 2, 3 and 4 component vectors (`SkM44.h`).
//!
//! `SkM44::dump` needs `SkDebugf` and is not ported. `SkMatrixPriv::MapRect(const SkM44&, ...)`
//! (defined in `SkM44.cpp`) lives in [`matrix_priv::map_rect`](crate::matrix_priv::map_rect).

use crate::floating_point::{double_to_float, ieee_float_divide, is_finite_array};
use crate::matrix::{Matrix, Member};
use crate::matrix_invert::invert_4x4_matrix;
use crate::rect::Rect;
use crate::scalar::{SCALAR_1, Scalar, scalar, scalar_cos, scalar_sin, scalar_sqrt};
use skia_rust_simd::vx::Float4;
use std::ops::{
    Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign,
};

/// A 2 component vector (`SkV2`).
// Port of: include/core/SkM44.h#L19-L54 (chrome/m156)
#[doc(alias = "SkV2")]
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct V2 {
    /// The x component.
    pub x: f32,
    /// The y component.
    pub y: f32,
}

impl V2 {
    /// Creates a vector.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Returns the dot product (`SkV2::Dot`, `SkV2::dot`).
    // Port of: include/core/SkM44.h#L24-L25 (chrome/m156)
    #[doc(alias = "Dot")]
    #[must_use]
    pub fn dot(self, b: Self) -> scalar {
        self.x * b.x + self.y * b.y
    }

    /// Returns the 2D cross product (`SkV2::Cross`, `SkV2::cross`).
    // Port of: include/core/SkM44.h#L26-L27 (chrome/m156)
    #[doc(alias = "Cross")]
    #[must_use]
    pub fn cross(self, b: Self) -> scalar {
        self.x * b.y - self.y * b.x
    }

    /// Returns the vector scaled to unit length (`SkV2::Normalize`, `SkV2::normalize`).
    // Port of: include/core/SkM44.h#L28 (chrome/m156)
    #[doc(alias = "Normalize")]
    #[must_use]
    pub fn normalize(self) -> Self {
        self * (1.0 / self.length())
    }

    /// Returns the squared length.
    // Port of: include/core/SkM44.h#L47 (chrome/m156)
    #[doc(alias = "lengthSquared")]
    #[must_use]
    pub fn length_squared(self) -> scalar {
        Self::dot(self, self)
    }

    /// Returns the length.
    // Port of: include/core/SkM44.h#L48 (chrome/m156)
    #[must_use]
    pub fn length(self) -> scalar {
        scalar_sqrt(self.length_squared())
    }

    /// Returns the components as an array (`SkV2::ptr`).
    #[doc(alias = "ptr")]
    #[must_use]
    pub fn to_array(self) -> [f32; 2] {
        [self.x, self.y]
    }
}

impl Neg for V2 {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y)
    }
}

impl Add for V2 {
    type Output = Self;
    fn add(self, v: Self) -> Self::Output {
        Self::new(self.x + v.x, self.y + v.y)
    }
}

impl Sub for V2 {
    type Output = Self;
    fn sub(self, v: Self) -> Self::Output {
        Self::new(self.x - v.x, self.y - v.y)
    }
}

impl Mul for V2 {
    type Output = Self;
    fn mul(self, v: Self) -> Self::Output {
        Self::new(self.x * v.x, self.y * v.y)
    }
}

impl Mul<scalar> for V2 {
    type Output = Self;
    fn mul(self, s: scalar) -> Self::Output {
        Self::new(self.x * s, self.y * s)
    }
}

impl Mul<V2> for scalar {
    type Output = V2;
    fn mul(self, v: V2) -> Self::Output {
        V2::new(v.x * self, v.y * self)
    }
}

impl Div<V2> for scalar {
    type Output = V2;
    fn div(self, v: V2) -> Self::Output {
        V2::new(self / v.x, self / v.y)
    }
}

impl Div<scalar> for V2 {
    type Output = V2;
    fn div(self, s: scalar) -> Self::Output {
        V2::new(self.x / s, self.y / s)
    }
}

impl AddAssign for V2 {
    fn add_assign(&mut self, v: Self) {
        *self = *self + v;
    }
}

impl SubAssign for V2 {
    fn sub_assign(&mut self, v: Self) {
        *self = *self - v;
    }
}

impl MulAssign for V2 {
    fn mul_assign(&mut self, v: Self) {
        *self = *self * v;
    }
}

impl MulAssign<scalar> for V2 {
    fn mul_assign(&mut self, s: scalar) {
        *self = *self * s;
    }
}

impl DivAssign<scalar> for V2 {
    fn div_assign(&mut self, s: scalar) {
        *self = *self / s;
    }
}

/// A 3 component vector (`SkV3`).
// Port of: include/core/SkM44.h#L56-L96 (chrome/m156)
#[doc(alias = "SkV3")]
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct V3 {
    /// The x component.
    pub x: f32,
    /// The y component.
    pub y: f32,
    /// The z component.
    pub z: f32,
}

impl V3 {
    /// Creates a vector.
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Returns the dot product (`SkV3::Dot`, `SkV3::dot`).
    // Port of: include/core/SkM44.h#L66-L67 (chrome/m156)
    #[doc(alias = "Dot")]
    #[must_use]
    pub fn dot(&self, b: &Self) -> scalar {
        self.x * b.x + self.y * b.y + self.z * b.z
    }

    /// Returns the cross product (`SkV3::Cross`, `SkV3::cross`).
    // Port of: include/core/SkM44.h#L68-L70 (chrome/m156)
    #[doc(alias = "Cross")]
    #[must_use]
    pub fn cross(&self, b: &Self) -> Self {
        Self::new(
            self.y * b.z - self.z * b.y,
            self.z * b.x - self.x * b.z,
            self.x * b.y - self.y * b.x,
        )
    }

    /// Returns the vector scaled to unit length (`SkV3::Normalize`, `SkV3::normalize`).
    // Port of: include/core/SkM44.h#L71 (chrome/m156)
    #[doc(alias = "Normalize")]
    #[must_use]
    pub fn normalize(&self) -> Self {
        *self * (1.0 / self.length())
    }

    /// Returns the squared length.
    // Port of: include/core/SkM44.h#L90 (chrome/m156)
    #[doc(alias = "lengthSquared")]
    #[must_use]
    pub fn length_squared(&self) -> scalar {
        Self::dot(self, self)
    }

    /// Returns the length.
    // Port of: include/core/SkM44.h#L91 (chrome/m156)
    #[must_use]
    pub fn length(&self) -> scalar {
        scalar_sqrt(Self::dot(self, self))
    }

    /// Returns the components as an array (`SkV3::ptr`).
    #[doc(alias = "ptr")]
    #[must_use]
    pub fn to_array(&self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

impl Neg for V3 {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y, -self.z)
    }
}

impl Add for V3 {
    type Output = Self;
    fn add(self, v: Self) -> Self::Output {
        Self::new(self.x + v.x, self.y + v.y, self.z + v.z)
    }
}

impl Sub for V3 {
    type Output = Self;
    fn sub(self, v: Self) -> Self::Output {
        Self::new(self.x - v.x, self.y - v.y, self.z - v.z)
    }
}

impl Mul for V3 {
    type Output = Self;
    fn mul(self, v: Self) -> Self::Output {
        Self::new(self.x * v.x, self.y * v.y, self.z * v.z)
    }
}

impl Mul<scalar> for V3 {
    type Output = Self;
    fn mul(self, s: scalar) -> Self::Output {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Mul<V3> for scalar {
    type Output = V3;
    fn mul(self, v: V3) -> Self::Output {
        v * self
    }
}

impl AddAssign for V3 {
    fn add_assign(&mut self, v: Self) {
        *self = *self + v;
    }
}

impl SubAssign for V3 {
    fn sub_assign(&mut self, v: Self) {
        *self = *self - v;
    }
}

impl MulAssign for V3 {
    fn mul_assign(&mut self, v: Self) {
        *self = *self * v;
    }
}

impl MulAssign<scalar> for V3 {
    fn mul_assign(&mut self, s: scalar) {
        *self = *self * s;
    }
}

/// A 4 component vector (`SkV4`).
// Port of: include/core/SkM44.h#L98-L148 (chrome/m156)
#[doc(alias = "SkV4")]
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct V4 {
    /// The x component.
    pub x: f32,
    /// The y component.
    pub y: f32,
    /// The z component.
    pub z: f32,
    /// The w component.
    pub w: f32,
}

impl V4 {
    /// Creates a vector.
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// Returns the squared length.
    // Port of: include/core/SkM44.h#L124 (chrome/m156)
    #[doc(alias = "lengthSquared")]
    #[must_use]
    pub fn length_squared(&self) -> scalar {
        Self::dot(self, self)
    }

    /// Returns the length.
    // Port of: include/core/SkM44.h#L125 (chrome/m156)
    #[must_use]
    pub fn length(&self) -> scalar {
        scalar_sqrt(Self::dot(self, self))
    }

    /// Returns the dot product (`SkV4::Dot`, `SkV4::dot`).
    // Port of: include/core/SkM44.h#L103-L105 (chrome/m156)
    #[doc(alias = "Dot")]
    #[must_use]
    pub fn dot(&self, b: &Self) -> scalar {
        self.x * b.x + self.y * b.y + self.z * b.z + self.w * b.w
    }

    /// Returns the vector scaled to unit length (`SkV4::Normalize`, `SkV4::normalize`).
    // Port of: include/core/SkM44.h#L106 (chrome/m156)
    #[doc(alias = "Normalize")]
    #[must_use]
    pub fn normalize(&self) -> Self {
        *self * (1.0 / self.length())
    }

    /// Returns the components as an array (`SkV4::ptr`).
    #[doc(alias = "ptr")]
    #[must_use]
    pub fn to_array(&self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }
}

impl Neg for V4 {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y, -self.z, -self.w)
    }
}

impl Add for V4 {
    type Output = Self;
    fn add(self, v: Self) -> Self::Output {
        Self::new(self.x + v.x, self.y + v.y, self.z + v.z, self.w + v.w)
    }
}

impl Sub for V4 {
    type Output = Self;
    fn sub(self, v: Self) -> Self::Output {
        Self::new(self.x - v.x, self.y - v.y, self.z - v.z, self.w - v.w)
    }
}

impl Mul for V4 {
    type Output = Self;
    fn mul(self, v: Self) -> Self::Output {
        Self::new(self.x * v.x, self.y * v.y, self.z * v.z, self.w * v.w)
    }
}

impl Mul<scalar> for V4 {
    type Output = Self;
    fn mul(self, s: scalar) -> Self::Output {
        Self::new(self.x * s, self.y * s, self.z * s, self.w * s)
    }
}

impl Mul<V4> for scalar {
    type Output = V4;
    fn mul(self, v: V4) -> Self::Output {
        v * self
    }
}

impl Index<usize> for V4 {
    type Output = f32;

    // Port of: include/core/SkM44.h#L129-L132 (chrome/m156)
    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            3 => &self.w,
            _ => panic!("V4 index out of range: {index}"),
        }
    }
}

impl IndexMut<usize> for V4 {
    // Port of: include/core/SkM44.h#L133-L136 (chrome/m156)
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match index {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            3 => &mut self.w,
            _ => panic!("V4 index out of range: {index}"),
        }
    }
}

/// A 4x4 matrix, stored in column-major order (`SkM44`).
///
/// The constructor [`M44::new`] takes its sixteen arguments in row-major order, as in C++.
// Port of: include/core/SkM44.h#L150-L433 (chrome/m156)
#[doc(alias = "SkM44")]
#[derive(Copy, Clone, Debug)]
pub struct M44 {
    mat: [scalar; 16],
}

impl Default for M44 {
    fn default() -> Self {
        Self::new_identity()
    }
}

impl PartialEq for M44 {
    // Port of: src/core/SkM44.cpp#L19-L37 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        self.mat == other.mat
    }
}

// Port of: src/core/SkM44.cpp#L39-L44 (chrome/m156)
fn transpose_arrays(dst: &mut [scalar; 16], src: &[scalar; 16]) {
    dst[0] = src[0];
    dst[1] = src[4];
    dst[2] = src[8];
    dst[3] = src[12];
    dst[4] = src[1];
    dst[5] = src[5];
    dst[6] = src[9];
    dst[7] = src[13];
    dst[8] = src[2];
    dst[9] = src[6];
    dst[10] = src[10];
    dst[11] = src[14];
    dst[12] = src[3];
    dst[13] = src[7];
    dst[14] = src[11];
    dst[15] = src[15];
}

// Loads the column `i` of a column-major array (`skvx::float4::Load(fMat + 4*i)`).
fn load_col(mat: &[scalar; 16], i: usize) -> Float4 {
    Float4::load(&mat[i * 4..i * 4 + 4])
}

// Stores `v` as column `i` (`v.store(fMat + 4*i)`).
fn store_col(mat: &mut [scalar; 16], i: usize, v: Float4) {
    v.store(&mut mat[i * 4..i * 4 + 4]);
}

impl M44 {
    /// Creates the identity matrix (`SkM44()`).
    // Port of: include/core/SkM44.h#L157-L161 (chrome/m156)
    #[must_use]
    pub const fn new_identity() -> Self {
        Self {
            mat: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    /// Returns `a` concatenated with `b` (`SkM44(a, b)`, `a * b`).
    // Port of: include/core/SkM44.h#L163-L165 (chrome/m156)
    #[doc(alias = "Concat")]
    #[must_use]
    pub fn concat(a: &Self, b: &Self) -> Self {
        let mut m = Self::new_identity();
        m.set_concat(a, b);
        m
    }

    /// Creates a matrix with every member NaN (`SkM44(kNaN_Constructor)`).
    // Port of: include/core/SkM44.h#L175-L180 (chrome/m156)
    #[must_use]
    pub const fn nan() -> Self {
        Self {
            mat: [f32::NAN; 16],
        }
    }

    /// Creates a matrix from sixteen members given in row-major order, as in
    /// `SkM44(m0, m4, m8, m12, ...)`.
    // Port of: include/core/SkM44.h#L182-L193 (chrome/m156)
    #[must_use]
    #[allow(clippy::too_many_arguments)] // mirrors the SkM44 constructor
    pub const fn new(
        m0: scalar,
        m4: scalar,
        m8: scalar,
        m12: scalar,
        m1: scalar,
        m5: scalar,
        m9: scalar,
        m13: scalar,
        m2: scalar,
        m6: scalar,
        m10: scalar,
        m14: scalar,
        m3: scalar,
        m7: scalar,
        m11: scalar,
        m15: scalar,
    ) -> Self {
        Self {
            mat: [
                m0, m1, m2, m3, m4, m5, m6, m7, m8, m9, m10, m11, m12, m13, m14, m15,
            ],
        }
    }

    /// Creates a matrix from its four rows.
    // Port of: include/core/SkM44.h#L195-L202 (chrome/m156)
    #[doc(alias = "Rows")]
    #[must_use]
    pub fn rows(r0: &V4, r1: &V4, r2: &V4, r3: &V4) -> Self {
        let mut m = Self::new_identity();
        m.set_row(0, r0);
        m.set_row(1, r1);
        m.set_row(2, r2);
        m.set_row(3, r3);
        m
    }

    /// Creates a matrix from its four columns.
    // Port of: include/core/SkM44.h#L203-L210 (chrome/m156)
    #[doc(alias = "Cols")]
    #[must_use]
    pub fn cols(c0: &V4, c1: &V4, c2: &V4, c3: &V4) -> Self {
        let mut m = Self::new_identity();
        m.set_col(0, c0);
        m.set_col(1, c1);
        m.set_col(2, c2);
        m.set_col(3, c3);
        m
    }

    /// Creates a matrix from sixteen members in row-major order.
    // Port of: include/core/SkM44.h#L212-L217 (chrome/m156)
    #[doc(alias = "RowMajor")]
    #[must_use]
    pub fn row_major(r: &[scalar; 16]) -> Self {
        Self::new(
            r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8], r[9], r[10], r[11], r[12], r[13],
            r[14], r[15],
        )
    }

    /// Creates a matrix from sixteen members in column-major order.
    // Port of: include/core/SkM44.h#L218-L223 (chrome/m156)
    #[doc(alias = "ColMajor")]
    #[must_use]
    pub fn col_major(c: &[scalar; 16]) -> Self {
        Self::new(
            c[0], c[4], c[8], c[12], c[1], c[5], c[9], c[13], c[2], c[6], c[10], c[14], c[3], c[7],
            c[11], c[15],
        )
    }

    /// Creates a translation matrix.
    // Port of: include/core/SkM44.h#L225-L230 (chrome/m156)
    #[doc(alias = "Translate")]
    #[must_use]
    pub fn translate(x: scalar, y: scalar, z: scalar) -> Self {
        Self::new(
            1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z, 0.0, 0.0, 0.0, 1.0,
        )
    }

    /// Creates a scale matrix.
    // Port of: include/core/SkM44.h#L232-L237 (chrome/m156)
    #[doc(alias = "Scale")]
    #[must_use]
    pub fn scale(x: scalar, y: scalar, z: scalar) -> Self {
        Self::new(
            x, 0.0, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 0.0, z, 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    /// Creates a matrix that rotates `radians` about `axis` (which need not be a unit vector).
    // Port of: include/core/SkM44.h#L239-L243 (chrome/m156)
    #[doc(alias = "Rotate")]
    #[must_use]
    pub fn rotate(axis: V3, radians: scalar) -> Self {
        let mut m = Self::new_identity();
        m.set_rotate(axis, radians);
        m
    }

    /// Creates a matrix that maps `src` to `dst`.
    // Port of: src/core/SkM44.cpp#L291-L308 (chrome/m156)
    #[doc(alias = "RectToRect")]
    #[must_use]
    pub fn rect_to_rect(src: impl AsRef<Rect>, dst: impl AsRef<Rect>) -> Self {
        let (src, dst) = (src.as_ref(), dst.as_ref());
        if src.is_empty() {
            return Self::new_identity();
        } else if dst.is_empty() {
            return Self::scale(0.0, 0.0, 0.0);
        }

        let sx = dst.width() / src.width();
        let sy = dst.height() / src.height();

        let tx = dst.left - sx * src.left;
        let ty = dst.top - sy * src.top;

        Self::new(
            sx, 0.0, 0.0, tx, 0.0, sy, 0.0, ty, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    /// Creates a view matrix that looks from `eye` towards `center`, with `up` pointing up.
    // Port of: src/core/SkM44.cpp#L310-L339 (chrome/m156)
    #[doc(alias = "LookAt")]
    #[must_use]
    pub fn look_at(eye: &V3, center: &V3, up: &V3) -> Self {
        let f = normalize_v3(*center - *eye);
        let u = normalize_v3(*up);
        let s = normalize_v3(f.cross(&u));

        let mut m = Self::new_identity();
        let cols = Self::cols(
            &v4(s, 0.0),
            &v4(s.cross(&f), 0.0),
            &v4(-f, 0.0),
            &v4(*eye, 1.0),
        );
        if let Some(inverse) = cols.invert() {
            m = inverse;
        } else {
            m.set_identity();
        }
        m
    }

    /// Creates a perspective matrix with the given clip distances and field of view
    /// (`angle`, in radians).
    // Port of: src/core/SkM44.cpp#L341-L358 (chrome/m156)
    #[doc(alias = "Perspective")]
    #[must_use]
    pub fn perspective(near: f32, far: f32, angle: f32) -> Self {
        debug_assert!(far > near);

        let denom_inv = ieee_float_divide(1.0, far - near);
        let half_angle = angle * 0.5;
        debug_assert!(half_angle != 0.0);
        // skia-rust: libm (tan)
        let cot = ieee_float_divide(1.0, half_angle.tan());

        let mut m = Self::new_identity();
        m.set_rc(0, 0, cot);
        m.set_rc(1, 1, cot);
        m.set_rc(2, 2, (far + near) * denom_inv);
        m.set_rc(2, 3, 2.0 * far * near * denom_inv);
        m.set_rc(3, 2, -1.0);
        m
    }

    /// Copies the members in column-major order.
    // Port of: include/core/SkM44.h#L256-L258 (chrome/m156)
    #[doc(alias = "getColMajor")]
    pub fn get_col_major(&self, v: &mut [scalar; 16]) {
        *v = self.mat;
    }

    /// Copies the members in row-major order.
    // Port of: src/core/SkM44.cpp#L46-L48 (chrome/m156)
    #[doc(alias = "getRowMajor")]
    pub fn get_row_major(&self, v: &mut [scalar; 16]) {
        transpose_arrays(v, &self.mat);
    }

    /// Returns the member at row `r` and column `c`.
    ///
    /// # Panics
    /// If `r` or `c` is greater than 3.
    // Port of: include/core/SkM44.h#L261-L265 (chrome/m156)
    #[must_use]
    pub fn rc(&self, r: usize, c: usize) -> scalar {
        assert!(r <= 3);
        assert!(c <= 3);
        self.mat[c * 4 + r]
    }

    /// Sets the member at row `r` and column `c`.
    ///
    /// # Panics
    /// If `r` or `c` is greater than 3.
    // Port of: include/core/SkM44.h#L266-L270 (chrome/m156)
    #[doc(alias = "setRC")]
    pub fn set_rc(&mut self, r: usize, c: usize, value: scalar) {
        assert!(r <= 3);
        assert!(c <= 3);
        self.mat[c * 4 + r] = value;
    }

    /// Returns row `i`.
    ///
    /// # Panics
    /// If `i` is greater than 3.
    // Port of: include/core/SkM44.h#L272-L275 (chrome/m156)
    #[must_use]
    pub fn row(&self, i: usize) -> V4 {
        assert!(i <= 3);
        V4::new(
            self.mat[i],
            self.mat[i + 4],
            self.mat[i + 8],
            self.mat[i + 12],
        )
    }

    /// Returns column `i`.
    ///
    /// # Panics
    /// If `i` is greater than 3.
    // Port of: include/core/SkM44.h#L276-L279 (chrome/m156)
    #[must_use]
    pub fn col(&self, i: usize) -> V4 {
        assert!(i <= 3);
        V4::new(
            self.mat[i * 4],
            self.mat[i * 4 + 1],
            self.mat[i * 4 + 2],
            self.mat[i * 4 + 3],
        )
    }

    /// Sets row `i`.
    ///
    /// # Panics
    /// If `i` is greater than 3.
    // Port of: include/core/SkM44.h#L281-L287 (chrome/m156)
    #[doc(alias = "setRow")]
    pub fn set_row(&mut self, i: usize, v: &V4) {
        assert!(i <= 3);
        self.mat[i] = v.x;
        self.mat[i + 4] = v.y;
        self.mat[i + 8] = v.z;
        self.mat[i + 12] = v.w;
    }

    /// Sets column `i`.
    ///
    /// # Panics
    /// If `i` is greater than 3.
    // Port of: include/core/SkM44.h#L288-L292 (chrome/m156)
    #[doc(alias = "setCol")]
    pub fn set_col(&mut self, i: usize, v: &V4) {
        assert!(i <= 3);
        self.mat[i * 4..i * 4 + 4].copy_from_slice(&v.to_array());
    }

    /// Sets the matrix to identity.
    // Port of: include/core/SkM44.h#L293-L299 (chrome/m156)
    #[doc(alias = "setIdentity")]
    pub fn set_identity(&mut self) -> &mut Self {
        *self = Self::new_identity();
        self
    }

    /// Sets the matrix to a translation.
    // Port of: include/core/SkM44.h#L301-L307 (chrome/m156)
    #[doc(alias = "setTranslate")]
    pub fn set_translate(&mut self, x: scalar, y: scalar, z: scalar) -> &mut Self {
        *self = Self::translate(x, y, z);
        self
    }

    /// Sets the matrix to a scale.
    // Port of: include/core/SkM44.h#L309-L315 (chrome/m156)
    #[doc(alias = "setScale")]
    pub fn set_scale(&mut self, x: scalar, y: scalar, z: scalar) -> &mut Self {
        *self = Self::scale(x, y, z);
        self
    }

    /// Sets the matrix to the rotation about the unit vector `axis` by the angle with the given
    /// sine and cosine.
    // Port of: src/core/SkM44.cpp#L163-L179 (chrome/m156)
    #[doc(alias = "setRotateUnitSinCos")]
    #[allow(clippy::many_single_char_names)] // names follow the C++
    pub fn set_rotate_unit_sin_cos(
        &mut self,
        axis: V3,
        sin_angle: scalar,
        cos_angle: scalar,
    ) -> &mut Self {
        // Taken from "Essential Mathematics for Games and Interactive Applications"
        //             James M. Van Verth and Lars M. Bishop -- third edition
        let x = axis.x;
        let y = axis.y;
        let z = axis.z;
        let c = cos_angle;
        let s = sin_angle;
        let t = 1.0 - c;

        *self = Self::new(
            t * x * x + c,
            t * x * y - s * z,
            t * x * z + s * y,
            0.0,
            t * x * y + s * z,
            t * y * y + c,
            t * y * z - s * x,
            0.0,
            t * x * z - s * y,
            t * y * z + s * x,
            t * z * z + c,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        self
    }

    /// Sets the matrix to the rotation about the unit vector `axis` by `radians`.
    // Port of: include/core/SkM44.h#L332-L334 (chrome/m156)
    #[doc(alias = "setRotateUnit")]
    pub fn set_rotate_unit(&mut self, axis: V3, radians: scalar) -> &mut Self {
        self.set_rotate_unit_sin_cos(axis, scalar_sin(radians), scalar_cos(radians))
    }

    /// Sets the matrix to the rotation about `axis` by `radians`; the identity if `axis` has no
    /// finite, positive length.
    // Port of: src/core/SkM44.cpp#L181-L189 (chrome/m156)
    #[doc(alias = "setRotate")]
    pub fn set_rotate(&mut self, axis: V3, radians: scalar) -> &mut Self {
        let len = axis.length();
        if len > 0.0 && crate::floating_point::is_finite(len) {
            self.set_rotate_unit(axis * (SCALAR_1 / len), radians);
        } else {
            self.set_identity();
        }
        self
    }

    /// Sets the matrix to `a` concatenated with `b` (`a * b`).
    // Port of: src/core/SkM44.cpp#L50-L71 (chrome/m156)
    #[doc(alias = "setConcat")]
    pub fn set_concat(&mut self, a: &Self, b: &Self) -> &mut Self {
        let c0 = load_col(&a.mat, 0);
        let c1 = load_col(&a.mat, 1);
        let c2 = load_col(&a.mat, 2);
        let c3 = load_col(&a.mat, 3);

        let compute = |r: Float4| c0 * r[0] + (c1 * r[1] + (c2 * r[2] + c3 * r[3]));

        let m0 = compute(load_col(&b.mat, 0));
        let m1 = compute(load_col(&b.mat, 1));
        let m2 = compute(load_col(&b.mat, 2));
        let m3 = compute(load_col(&b.mat, 3));

        store_col(&mut self.mat, 0, m0);
        store_col(&mut self.mat, 1, m1);
        store_col(&mut self.mat, 2, m2);
        store_col(&mut self.mat, 3, m3);
        self
    }

    /// Sets the matrix to `self * m`.
    // Port of: include/core/SkM44.h#L362-L364 (chrome/m156)
    #[doc(alias = "preConcat")]
    pub fn pre_concat(&mut self, m: &Self) -> &mut Self {
        let this = *self;
        self.set_concat(&this, m)
    }

    /// Sets the matrix to `m * self`.
    // Port of: include/core/SkM44.h#L366-L368 (chrome/m156)
    #[doc(alias = "postConcat")]
    pub fn post_concat(&mut self, m: &Self) -> &mut Self {
        let this = *self;
        self.set_concat(m, &this)
    }

    /// Sets the matrix to `self * b`, where `b` is a 3x3 matrix applied to x, y and w
    /// (`SkM44::preConcat(const SkMatrix&)`).
    // Port of: src/core/SkM44.cpp#L73-L90 (chrome/m156)
    #[doc(alias = "preConcat")]
    pub fn pre_concat_matrix(&mut self, b: &Matrix) -> &mut Self {
        let c0 = load_col(&self.mat, 0);
        let c1 = load_col(&self.mat, 1);
        let c3 = load_col(&self.mat, 3);

        let compute = |r0: f32, r1: f32, r3: f32| c0 * r0 + (c1 * r1 + c3 * r3);

        let m0 = compute(b[0], b[3], b[6]);
        let m1 = compute(b[1], b[4], b[7]);
        let m3 = compute(b[2], b[5], b[8]);

        store_col(&mut self.mat, 0, m0);
        store_col(&mut self.mat, 1, m1);
        store_col(&mut self.mat, 3, m3);
        self
    }

    /// If the bottom row is `[0, 0, 0, not_one]`, divides everything by `not_one`, so the
    /// matrix is no longer treated as perspective.
    // Port of: src/core/SkM44.cpp#L260-L274 (chrome/m156)
    #[doc(alias = "normalizePerspective")]
    pub fn normalize_perspective(&mut self) {
        // If the bottom row of the matrix is [0, 0, 0, not_one], we will treat the matrix as if it
        // is in perspective, even though it stills behaves like its affine. If we divide
        // everything by the not_one value, then it will behave the same, but will be treated as
        // affine, and therefore faster (e.g. clients can forward-difference calculations).
        #[allow(clippy::float_cmp)] // mirrors the C++ exact comparisons
        let needs_normalizing = self.mat[15] != 1.0
            && self.mat[15] != 0.0
            && self.mat[3] == 0.0
            && self.mat[7] == 0.0
            && self.mat[11] == 0.0;
        if needs_normalizing {
            let inv = 1.0 / f64::from(self.mat[15]);
            // `skvx::float4 * double` converts the double to float first
            let inv = double_to_float(inv);
            for i in 0..4 {
                let v = load_col(&self.mat, i) * inv;
                store_col(&mut self.mat, i, v);
            }
            self.mat[15] = 1.0;
        }
    }

    /// Returns true if all sixteen members are finite.
    // Port of: include/core/SkM44.h#L378 (chrome/m156)
    #[doc(alias = "isFinite")]
    #[must_use]
    pub fn is_finite(&self) -> bool {
        is_finite_array(&self.mat)
    }

    /// Returns the inverse of the matrix, or `None` if it is not invertible.
    // Port of: src/core/SkM44.cpp#L276-L284 (chrome/m156)
    #[must_use]
    pub fn invert(&self) -> Option<M44> {
        let mut tmp = [0.0; 16];
        #[allow(clippy::float_cmp)] // mirrors `== 0.0f`
        if invert_4x4_matrix(&self.mat, Some(&mut tmp)) == 0.0 {
            return None;
        }
        Some(Self { mat: tmp })
    }

    /// Returns the transpose of the matrix.
    // Port of: src/core/SkM44.cpp#L286-L290 (chrome/m156)
    #[must_use]
    pub fn transpose(&self) -> Self {
        let mut trans = Self::new_identity();
        transpose_arrays(&mut trans.mat, &self.mat);
        trans
    }

    /// Maps the homogeneous point `(x, y, z, w)`.
    // Port of: src/core/SkM44.cpp#L125-L136 (chrome/m156)
    #[must_use]
    #[allow(clippy::many_single_char_names)] // names follow the C++
    pub fn map(&self, x: f32, y: f32, z: f32, w: f32) -> V4 {
        let c0 = load_col(&self.mat, 0);
        let c1 = load_col(&self.mat, 1);
        let c2 = load_col(&self.mat, 2);
        let c3 = load_col(&self.mat, 3);

        let r = c0 * x + (c1 * y + (c2 * z + c3 * w));
        V4::new(r[0], r[1], r[2], r[3])
    }

    /// Returns the 3x3 matrix of the members that affect x, y and w
    /// (`SkM44::asM33`).
    // Port of: include/core/SkM44.h#L409-L413 (chrome/m156)
    #[doc(alias = "asM33")]
    #[must_use]
    pub fn to_m33(&self) -> Matrix {
        let m = &self.mat;
        Matrix::new_all(m[0], m[4], m[12], m[1], m[5], m[13], m[3], m[7], m[15])
    }

    /// Sets the matrix to its own value concatenated with a translation (`M' = M * T`).
    // Port of: src/core/SkM44.cpp#L92-L100 (chrome/m156)
    #[doc(alias = "preTranslate")]
    pub fn pre_translate(
        &mut self,
        x: scalar,
        y: scalar,
        z: impl Into<Option<scalar>>,
    ) -> &mut Self {
        let z = z.into().unwrap_or(0.0);
        let c0 = load_col(&self.mat, 0);
        let c1 = load_col(&self.mat, 1);
        let c2 = load_col(&self.mat, 2);
        let c3 = load_col(&self.mat, 3);

        // only need to update the last column
        let v = c0 * x + (c1 * y + (c2 * z + c3));
        store_col(&mut self.mat, 3, v);
        self
    }

    /// Sets the matrix to a translation concatenated with its own value (`M' = T * M`).
    // Port of: src/core/SkM44.cpp#L102-L109 (chrome/m156)
    #[doc(alias = "postTranslate")]
    pub fn post_translate(
        &mut self,
        x: scalar,
        y: scalar,
        z: impl Into<Option<scalar>>,
    ) -> &mut Self {
        let z = z.into().unwrap_or(0.0);
        let t = Float4::new(x, y, z, 0.0);
        let v0 = t * self.mat[3] + load_col(&self.mat, 0);
        store_col(&mut self.mat, 0, v0);
        let v1 = t * self.mat[7] + load_col(&self.mat, 1);
        store_col(&mut self.mat, 1, v1);
        let v2 = t * self.mat[11] + load_col(&self.mat, 2);
        store_col(&mut self.mat, 2, v2);
        let v3 = t * self.mat[15] + load_col(&self.mat, 3);
        store_col(&mut self.mat, 3, v3);
        self
    }

    /// Sets the matrix to its own value concatenated with a scale in x and y.
    // Port of: src/core/SkM44.cpp#L111-L118 (chrome/m156)
    #[doc(alias = "preScale")]
    pub fn pre_scale(&mut self, x: scalar, y: scalar) -> &mut Self {
        let c0 = load_col(&self.mat, 0);
        let c1 = load_col(&self.mat, 1);

        store_col(&mut self.mat, 0, c0 * x);
        store_col(&mut self.mat, 1, c1 * y);
        self
    }

    /// Sets the matrix to its own value concatenated with a scale in x, y and z.
    // Port of: src/core/SkM44.cpp#L120-L129 (chrome/m156)
    #[doc(alias = "preScale")]
    pub fn pre_scale_xyz(&mut self, x: scalar, y: scalar, z: scalar) -> &mut Self {
        let c0 = load_col(&self.mat, 0);
        let c1 = load_col(&self.mat, 1);
        let c2 = load_col(&self.mat, 2);

        store_col(&mut self.mat, 0, c0 * x);
        store_col(&mut self.mat, 1, c1 * y);
        store_col(&mut self.mat, 2, c2 * z);
        self
    }

    /// The members in column-major order (`SkMatrixPriv::M44ColMajor`).
    pub(crate) fn members(&self) -> &[scalar; 16] {
        &self.mat
    }
}

// Port of: src/core/SkM44.cpp#L318-L322 (chrome/m156)
fn normalize_v3(v: V3) -> V3 {
    let vlen = v.length();

    if vlen.nearly_zero(None) {
        v
    } else {
        v * (1.0 / vlen)
    }
}

// Port of: src/core/SkM44.cpp#L324 (chrome/m156)
fn v4(v: V3, w: scalar) -> V4 {
    V4::new(v.x, v.y, v.z, w)
}

impl Mul for &M44 {
    type Output = M44;
    // Port of: include/core/SkM44.h#L358-L360 (chrome/m156)
    fn mul(self, m: Self) -> Self::Output {
        M44::concat(self, m)
    }
}

impl Mul<V4> for &M44 {
    type Output = V4;
    // Port of: include/core/SkM44.h#L392-L394 (chrome/m156)
    fn mul(self, v: V4) -> Self::Output {
        self.map(v.x, v.y, v.z, v.w)
    }
}

impl Mul<V3> for &M44 {
    type Output = V3;
    // Port of: include/core/SkM44.h#L400-L403 (chrome/m156)
    fn mul(self, v: V3) -> Self::Output {
        let v4 = self.map(v.x, v.y, v.z, 0.0);
        V3::new(v4.x, v4.y, v4.z)
    }
}

impl From<&Matrix> for M44 {
    // Port of: include/core/SkM44.h#L415-L420 (chrome/m156)
    fn from(src: &Matrix) -> Self {
        Self::new(
            src[Member::ScaleX],
            src[Member::SkewX],
            0.0,
            src[Member::TransX],
            src[Member::SkewY],
            src[Member::ScaleY],
            0.0,
            src[Member::TransY],
            0.0,
            0.0,
            1.0,
            0.0,
            src[Member::Persp0],
            src[Member::Persp1],
            0.0,
            src[Member::Persp2],
        )
    }
}

impl From<Matrix> for M44 {
    fn from(m: Matrix) -> Self {
        M44::from(&m)
    }
}
