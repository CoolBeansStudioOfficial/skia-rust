// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/Transform.h, src/gpu/graphite/geom/Transform.cpp

//! `skgpu::graphite::Transform`: an `SkM44` with its inverse and cached properties.

use skia_rust_core::floating_point::{FLOAT_INFINITY, ieee_float_divide, is_finite, is_finite_all};
use skia_rust_core::m44::{M44, V4};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::matrix_invert::invert_2x2_matrix;
use skia_rust_core::matrix_priv;
use skia_rust_simd::vx::{Float2, Float4};

use crate::graphite::geom::rect::Rect;

// Port of: src/gpu/graphite/geom/Transform.cpp#L25-L31 (chrome/m156)
// The (-tx,-ty) terms preserve the calculated values in (l,t,-r,-b) form so that the return value
// can be passed directly into FromVals() to avoid extra negation operations in ltrb().
fn scale_translate_rect(rect_vals: Float4, sx: f32, sy: f32, tx: f32, ty: f32) -> Float4 {
    rect_vals * Float4::new(sx, sy, sx, sy) + Float4::new(tx, ty, -tx, -ty)
}

// Port of: src/gpu/graphite/geom/Transform.cpp#L33-L63 (chrome/m156)
// The exact `== 0.0` test selects the anti-diagonal case, as in the C++.
#[allow(clippy::float_cmp)]
fn map_rect(ty: Type, m: &M44, r: &Rect) -> Rect {
    match ty {
        Type::Identity => *r,
        Type::SimpleRectStaysRect => {
            // Since scale factors are positive, the returned rectangle is already sorted
            Rect::from_vals(scale_translate_rect(
                r.vals(),
                m.rc(0, 0),
                m.rc(1, 1),
                m.rc(0, 3),
                m.rc(1, 3),
            ))
        }
        Type::RectStaysRect => {
            // Which is not the case for general rect-stays-rect transforms
            let mut xformed = r.vals();
            if m.rc(0, 0) == 0.0 {
                // Anti-diagonal matrix (90/270 rotation), so scale L+R by m10 and T+B by m01 and
                // then swizzle so that the transformed values swap X and Y components and then
                // sort
                xformed =
                    scale_translate_rect(xformed, m.rc(1, 0), m.rc(0, 1), m.rc(1, 3), m.rc(0, 3))
                        .yxwz();
            } else {
                // Mirror or 180 rotation, so X and/or Y edges may be flipped so just sort after.
                xformed =
                    scale_translate_rect(xformed, m.rc(0, 0), m.rc(1, 1), m.rc(0, 3), m.rc(1, 3));
            }
            let mut out = Rect::from_vals(xformed);
            out.sort();
            out
        }
        Type::Affine | Type::Perspective => {
            // SkMatrixPriv::MapRect(m, r.asSkRect())
            Rect::from(matrix_priv::map_rect(m, &r.as_sk_rect()))
        }
        Type::Invalid => Rect::infinite_inverted(),
    }
}

// Port of: src/gpu/graphite/geom/Transform.cpp#L65-L76 (chrome/m156)
fn map_points_m44(m: &M44, input: &[Float4], out: &mut [Float4]) {
    let mut cm = [0.0_f32; 16];
    m.get_col_major(&mut cm);
    let c0 = Float4::from_list(&cm[0..4]);
    let c1 = Float4::from_list(&cm[4..8]);
    let c2 = Float4::from_list(&cm[8..12]);
    let c3 = Float4::from_list(&cm[12..16]);

    for (o, i) in out.iter_mut().zip(input) {
        let p = (c0 * i.x()) + (c1 * i.y()) + (c2 * i.z()) + (c3 * i.w());
        *o = p;
    }
}

// Returns singular value decomposition of the 2x2 matrix [m00 m01] as {min, max}
//                                                        [m10 m11]
// Port of: src/gpu/graphite/geom/Transform.cpp#L78-L92 (chrome/m156)
// The `0.5 * (a + b)` forms are kept as written in the C++: `f32::midpoint` rounds differently.
#[allow(clippy::manual_midpoint)]
fn compute_svd(m00: f32, m01: f32, m10: f32, m11: f32) -> (f32, f32) {
    // no-persp, these are the singular values of [m00,m01][m10,m11], which is just the upper 2x2
    // and equivalent to SkMatrix::getMinmaxScales().
    let s1 = m00 * m00 + m01 * m01 + m10 * m10 + m11 * m11;

    let e = m00 * m00 + m01 * m01 - m10 * m10 - m11 * m11;
    let f = m00 * m10 + m01 * m11;
    let s2 = (e * e + 4.0 * f * f).sqrt();

    // s2 >= 0, so (s1 - s2) <= (s1 + s2) so this always returns {min, max}.
    ((0.5 * (s1 - s2)).sqrt(), (0.5 * (s1 + s2)).sqrt())
}

// Port of: src/gpu/graphite/geom/Transform.cpp#L94-L102 (chrome/m156)
fn sort_scale(sx: f32, sy: f32) -> (f32, f32) {
    let min = sx.abs();
    let max = sy.abs();
    if min > max { (max, min) } else { (min, max) }
}

// `std::min(a, b)` is `(b < a) ? b : a`.
fn std_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

/// Type classifies the transform into coarse categories so that certain optimizations or
/// properties can be queried efficiently.
#[doc(alias = "skgpu::graphite::Transform::Type")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Type {
    /// Applying the matrix to a vector or point is a no-op, so could be skipped entirely.
    Identity,
    /// The matrix transforms a rect to another rect, without mirrors or rotations, so both
    /// pre-and-post transform coordinates can be exactly represented as rects.
    SimpleRectStaysRect,
    /// The matrix transforms a rect to another rect, but may mirror or rotate the corners
    /// relative to each other. This means that the post-transformed rect completely fills that
    /// space.
    RectStaysRect,
    /// The matrix transform may have skew or rotation, so a mapped rect does not fill space, but
    /// there is no need to perform perspective division or w-plane clipping. This also includes
    /// orthographic projections.
    Affine,
    /// The matrix includes perspective and requires further projection to 2D, so care must be
    /// taken when w is less than or near 0, and homogeneous division and perspective-correct
    /// interpolation are needed when rendering.
    Perspective,
    /// The matrix is not invertible or not finite, so should not be used to draw.
    Invalid,
}

/// `Transform` encapsulates an `SkM44` matrix, its inverse, and other properties dependent on the
/// original matrix value that are useful when rendering.
// Port of: src/gpu/graphite/geom/Transform.h#L24-L174 (chrome/m156)
#[doc(alias = "skgpu::graphite::Transform")]
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    m: M44,
    inv_m: M44, // M^-1
    ty: Type,

    // These are cached for non-projection transforms since they are constant; projection matrices
    // must be computed per point, and these values are ignored.
    min_scale_factor: f32,
    max_scale_factor: f32,
}

impl Transform {
    // Used for static factories that have known properties.
    // Port of: src/gpu/graphite/geom/Transform.h#L158-L164 (chrome/m156)
    const fn with_known(m: M44, inv_m: M44, ty: Type, min_scale: f32, max_scale: f32) -> Self {
        Self {
            m,
            inv_m,
            ty,
            min_scale_factor: min_scale,
            max_scale_factor: max_scale,
        }
    }

    /// `Transform::Identity()`.
    // Port of: src/gpu/graphite/geom/Transform.h#L58-L60 (chrome/m156)
    #[must_use]
    pub const fn identity() -> Self {
        Self::with_known(
            M44::new_identity(),
            M44::new_identity(),
            Type::Identity,
            1.0,
            1.0,
        )
    }

    /// `Transform::Invalid()`.
    // Port of: src/gpu/graphite/geom/Transform.h#L61-L64 (chrome/m156)
    #[must_use]
    pub const fn invalid() -> Self {
        Self::with_known(M44::nan(), M44::nan(), Type::Invalid, 1.0, 1.0)
    }

    /// `Transform::Translate(x, y)`.
    // Port of: src/gpu/graphite/geom/Transform.h#L66-L75 (chrome/m156)
    #[must_use]
    pub fn translate(x: f32, y: f32) -> Self {
        if x == 0.0 && y == 0.0 {
            Self::identity()
        } else if is_finite_all(x, &[y]) {
            Self::with_known(
                M44::translate(x, y, 0.0),
                M44::translate(-x, -y, 0.0),
                Type::SimpleRectStaysRect,
                1.0,
                1.0,
            )
        } else {
            Self::invalid()
        }
    }

    /// `Transform::Inverse(t)`.
    // Port of: src/gpu/graphite/geom/Transform.h#L77-L79 (chrome/m156)
    #[must_use]
    pub fn inverse(t: &Transform) -> Self {
        Self::with_known(
            t.inv_m,
            t.m,
            t.ty,
            1.0 / t.max_scale_factor,
            1.0 / t.min_scale_factor,
        )
    }

    /// `operator const SkM44&()`: the matrix.
    #[must_use]
    pub fn matrix(&self) -> &M44 {
        &self.m
    }

    /// `inverse()`: the inverse matrix.
    #[must_use]
    pub fn inverse_matrix(&self) -> &M44 {
        &self.inv_m
    }

    /// `operator SkMatrix()`.
    #[must_use]
    pub fn to_matrix(&self) -> Matrix {
        self.m.to_m33()
    }

    /// `type()`.
    #[must_use]
    pub fn type_(&self) -> Type {
        self.ty
    }

    /// `valid()`.
    #[must_use]
    pub fn valid(&self) -> bool {
        self.ty != Type::Invalid
    }

    /// `maxScaleFactor()`: valid for non-projection types and 1.0 for projection matrices.
    #[must_use]
    pub fn max_scale_factor(&self) -> f32 {
        debug_assert!(self.valid());
        self.max_scale_factor
    }

    /// `scaleFactors(p)`: the `{min,max}` scale factor at the pre-transformed location `p`.
    // Port of: src/gpu/graphite/geom/Transform.cpp#L198-L248 (chrome/m156)
    #[must_use]
    // The derivative names (dxdu, dfdv, ...) mirror the C++ derivation.
    #[allow(clippy::similar_names)]
    pub fn scale_factors(&self, p: Float2) -> (f32, f32) {
        debug_assert!(self.valid());
        if self.ty < Type::Perspective {
            return (self.min_scale_factor, self.max_scale_factor);
        }

        // Singular values of [df/du df/dv] define perspective correct minimum and maximum scale
        // factors for M evaluated at (u,v). See the C++ source for the derivation.
        let dev_p: V4 = self.m.map(p.x(), p.y(), 0.0, 1.0);

        let dxdu = self.m.rc(0, 0);
        let dxdv = self.m.rc(0, 1);
        let dydu = self.m.rc(1, 0);
        let dydv = self.m.rc(1, 1);
        let dwdu = self.m.rc(3, 0);
        let dwdv = self.m.rc(3, 1);

        let inv_w2 = ieee_float_divide(1.0, dev_p.w * dev_p.w);
        // non-persp has invW2 = 1, devP.w = 1, dwdu = 0, dwdv = 0
        let dfdu = (dev_p.w * dxdu - dev_p.x * dwdu) * inv_w2; // non-persp -> dxdu -> m00
        let dfdv = (dev_p.w * dxdv - dev_p.x * dwdv) * inv_w2; // non-persp -> dxdv -> m01
        let dgdu = (dev_p.w * dydu - dev_p.y * dwdu) * inv_w2; // non-persp -> dydu -> m10
        let dgdv = (dev_p.w * dydv - dev_p.y * dwdv) * inv_w2; // non-persp -> dydv -> m11

        // no-persp, these are the singular values of [m00,m01][m10,m11]
        compute_svd(dfdu, dfdv, dgdu, dgdv)
    }

    /// Return the minimum distance needed to move in local (pre-transform) space to ensure that
    /// the transformed coordinates are at least 1px away from the original mapped point.
    ///
    /// If the bounds would be clipped by the w=0 plane or otherwise is ill-conditioned, this will
    /// return positive infinity.
    // Port of: src/gpu/graphite/geom/Transform.cpp#L250-L276 (chrome/m156)
    #[must_use]
    pub fn local_aa_radius(&self, bounds: &Rect) -> f32 {
        debug_assert!(self.valid());

        let min = if self.ty < Type::Perspective {
            // The scale factor is constant
            self.min_scale_factor
        } else {
            // Calculate the minimum scale factor over the 4 corners of the bounding box
            let tl = self
                .scale_factors(Float2::new(bounds.left(), bounds.top()))
                .0;
            let tr = self
                .scale_factors(Float2::new(bounds.right(), bounds.top()))
                .0;
            let br = self
                .scale_factors(Float2::new(bounds.right(), bounds.bot()))
                .0;
            let bl = self
                .scale_factors(Float2::new(bounds.left(), bounds.bot()))
                .0;
            std_min(std_min(tl, tr), std_min(br, bl))
        };

        // Moving 1 from 'p' before transforming will move at least 'min' and at most 'max' from
        // the transformed point. Thus moving between [1/max, 1/min] pre-transformation means post
        // transformation moves between [1,max/min] so using 1/min as the local AA radius ensures
        // that the post-transformed point is at least 1px away from the original.
        let aa_radius = ieee_float_divide(1.0, min);
        if is_finite(aa_radius) {
            aa_radius
        } else {
            FLOAT_INFINITY
        }
    }

    /// `mapRect(rect)`.
    #[must_use]
    pub fn map_rect(&self, rect: &Rect) -> Rect {
        debug_assert!(self.valid());
        map_rect(self.ty, &self.m, rect)
    }

    /// `inverseMapRect(rect)`.
    #[must_use]
    pub fn inverse_map_rect(&self, rect: &Rect) -> Rect {
        debug_assert!(self.valid());
        map_rect(self.ty, &self.inv_m, rect)
    }

    /// `mapPoints(localRect, deviceOut[4])`: the four corners, transformed.
    // Port of: src/gpu/graphite/geom/Transform.cpp#L287-L294 (chrome/m156)
    #[must_use]
    pub fn map_points_rect(&self, local_rect: &Rect) -> [Float4; 4] {
        debug_assert!(self.valid());
        let local_corners = [
            Float2::new(local_rect.left(), local_rect.top()),
            Float2::new(local_rect.right(), local_rect.top()),
            Float2::new(local_rect.right(), local_rect.bot()),
            Float2::new(local_rect.left(), local_rect.bot()),
        ];
        let mut device_out = [Float4::default(); 4];
        self.map_points_v2(&local_corners, &mut device_out);
        device_out
    }

    /// `mapPoints(const SkV2* localIn, SkV4* deviceOut, int count)`: the z of the input is 0.
    // Port of: src/gpu/graphite/geom/Transform.cpp#L296-L308 (chrome/m156)
    pub fn map_points_v2(&self, local_in: &[Float2], device_out: &mut [Float4]) {
        debug_assert!(self.valid());
        // TODO: These maybe should go into SkM44, since bulk point mapping seems generally useful
        let mut cm = [0.0_f32; 16];
        self.m.get_col_major(&mut cm);
        let c0 = Float4::from_list(&cm[0..4]);
        let c1 = Float4::from_list(&cm[4..8]);
        // skip c2 since localIn's z is assumed to be 0
        let c3 = Float4::from_list(&cm[12..16]);

        for (o, i) in device_out.iter_mut().zip(local_in) {
            // + c2*0.f is skipped, as in the C++
            *o = c0 * i.x() + c1 * i.y() + c3;
        }
    }

    /// `mapPoints(const SkV4* localIn, SkV4* deviceOut, int count)`.
    // Port of: src/gpu/graphite/geom/Transform.cpp#L310-L313 (chrome/m156)
    pub fn map_points_v4(&self, local_in: &[Float4], device_out: &mut [Float4]) {
        debug_assert!(self.valid());
        map_points_m44(&self.m, local_in, device_out);
    }

    /// `inverseMapPoints(deviceIn, localOut, count)`.
    // Port of: src/gpu/graphite/geom/Transform.cpp#L315-L318 (chrome/m156)
    pub fn inverse_map_points(&self, device_in: &[Float4], local_out: &mut [Float4]) {
        debug_assert!(self.valid());
        map_points_m44(&self.inv_m, device_in, local_out);
    }

    /// `preTranslate(x, y)`.
    #[must_use]
    pub fn pre_translate(&self, x: f32, y: f32) -> Self {
        self.concat_m44(&M44::translate(x, y, 0.0))
    }

    /// `postTranslate(x, y)`.
    #[must_use]
    pub fn post_translate(&self, x: f32, y: f32) -> Self {
        Self::translate(x, y).concat(self)
    }

    /// `concat(t)`: returns `this * t`.
    #[must_use]
    pub fn concat(&self, t: &Transform) -> Self {
        debug_assert!(self.valid());
        Self::new(M44::concat(&self.m, &t.m))
    }

    /// `concat(const SkM44& t)`: returns `this * t`.
    #[must_use]
    pub fn concat_m44(&self, t: &M44) -> Self {
        debug_assert!(self.valid());
        Self::new(M44::concat(&self.m, t))
    }

    /// `concatInverse(t)`: returns `this * t^-1`.
    #[must_use]
    pub fn concat_inverse(&self, t: &Transform) -> Self {
        debug_assert!(self.valid());
        Self::new(M44::concat(&self.m, &t.inv_m))
    }

    /// `concatInverse(const SkM44& t)`.
    #[must_use]
    pub fn concat_inverse_m44(&self, t: &M44) -> Self {
        debug_assert!(self.valid());
        // Saves a multiply compared to inverting just 't' and calculating both fM*t^-1 and t*fInvM
        // (t * this^-1)^-1 = this * t^-1
        Self::inverse(&Self::new(M44::concat(t, &self.inv_m)))
    }
}

impl PartialEq for Transform {
    // Port of: src/gpu/graphite/geom/Transform.h#L87-L89 (chrome/m156)
    fn eq(&self, other: &Transform) -> bool {
        self.valid() == other.valid() && (!self.valid() || self.m == other.m)
    }
}

impl Transform {
    /// `Transform(const SkM44& m)`: computes the inverse and the type of `m`.
    // Port of: src/gpu/graphite/geom/Transform.cpp#L106-L196 (chrome/m156)
    #[must_use]
    // The exact comparisons with 0 and 1 are the C++ classification of matrix types.
    #[allow(clippy::float_cmp)]
    // One function as in the C++ constructor, so it keeps its length for line-by-line review.
    #[allow(clippy::too_many_lines)]
    pub fn new(m: M44) -> Self {
        let k_no_perspective = V4::new(0.0, 0.0, 0.0, 1.0);
        let k_no_z = V4::new(0.0, 0.0, 1.0, 0.0);
        let mut t = Self {
            m,
            inv_m: M44::new_identity(),
            ty: Type::Invalid,
            min_scale_factor: 1.0,
            max_scale_factor: 1.0,
        };
        if m.row(3) != k_no_perspective {
            // Perspective matrices will have per-location scale factors calculated, so cached
            // scale factors will not be used.
            if let Some(inv) = m.invert() {
                t.inv_m = inv;
                t.ty = Type::Perspective;
            } else {
                t.ty = Type::Invalid;
            }
            return t;
        } else if m.col(2) != k_no_z || m.row(2) != k_no_z {
            // Orthographic matrices are lumped into the kAffine type although we use invert()
            // instead of taking short cuts.
            if let Some(inv) = m.invert() {
                t.inv_m = inv;
                t.ty = Type::Affine;
                // These scale factors are valid for the case where Z=0, which is the case for all
                // local geometry that's drawn.
                (t.min_scale_factor, t.max_scale_factor) =
                    compute_svd(m.rc(0, 0), m.rc(0, 1), m.rc(1, 0), m.rc(1, 1));
            } else {
                t.ty = Type::Invalid;
            }
            return t;
        }

        //                                              [sx kx 0 tx]
        // At this point, we know that m is of the form [ky sy 0 ty]
        //                                              [0  0  1 0 ]
        //                                              [0  0  0 1 ]
        // Other than kIdentity, none of the types depend on (tx, ty). The remaining types are
        // identified by considering the upper 2x2 (tx and ty are still used to compute the
        // inverse).
        let sx = m.rc(0, 0);
        let sy = m.rc(1, 1);
        let kx = m.rc(0, 1);
        let ky = m.rc(1, 0);
        let tx = m.rc(0, 3);
        let ty = m.rc(1, 3);
        if kx == 0.0 && ky == 0.0 {
            // 2x2 is a diagonal matrix
            if sx == 0.0 || sy == 0.0 {
                // Not invertible
                t.ty = Type::Invalid;
            } else if sx == 1.0 && sy == 1.0 && tx == 0.0 && ty == 0.0 {
                t.ty = Type::Identity;
                t.inv_m = M44::new_identity();
            } else {
                let ix = 1.0 / sx;
                let iy = 1.0 / sy;
                t.ty = if sx > 0.0 && sy > 0.0 {
                    Type::SimpleRectStaysRect
                } else {
                    Type::RectStaysRect
                };
                t.inv_m = M44::new(
                    ix,
                    0.0,
                    0.0,
                    -ix * tx,
                    0.0,
                    iy,
                    0.0,
                    -iy * ty,
                    0.0,
                    0.0,
                    1.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                );
                (t.min_scale_factor, t.max_scale_factor) = sort_scale(sx, sy);
            }
        } else if sx == 0.0 && sy == 0.0 {
            // 2x2 is an anti-diagonal matrix and represents a 90 or 270 degree rotation plus
            // optional scale and translate.
            if kx == 0.0 || ky == 0.0 {
                // Not invertible
                t.ty = Type::Invalid;
            } else {
                let ix = 1.0 / kx;
                let iy = 1.0 / ky;
                t.ty = Type::RectStaysRect;
                t.inv_m = M44::new(
                    0.0,
                    iy,
                    0.0,
                    -iy * ty,
                    ix,
                    0.0,
                    0.0,
                    -ix * tx,
                    0.0,
                    0.0,
                    1.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                );
                (t.min_scale_factor, t.max_scale_factor) = sort_scale(kx, ky);
            }
        } else {
            // Invert just the upper 2x2 and derive inverse translation from that
            let upper = [sx, ky, kx, sy]; // col-major
            let mut inv_upper = [0.0_f32; 4];
            if invert_2x2_matrix(&upper, Some(&mut inv_upper)) == 0.0 {
                // 2x2 was not invertible, so the original matrix won't be invertible either
                t.ty = Type::Invalid;
            } else {
                t.ty = Type::Affine;
                t.inv_m = M44::new(
                    inv_upper[0],
                    inv_upper[2],
                    0.0,
                    -inv_upper[0] * tx - inv_upper[2] * ty,
                    inv_upper[1],
                    inv_upper[3],
                    0.0,
                    -inv_upper[1] * tx - inv_upper[3] * ty,
                    0.0,
                    0.0,
                    1.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                );
                (t.min_scale_factor, t.max_scale_factor) = compute_svd(sx, kx, ky, sy);
            }
        }
        t
    }
}
