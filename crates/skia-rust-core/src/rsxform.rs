// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkRSXform.h, src/core/SkRSXform.cpp

//! `SkRSXform`: a compressed form of a rotation+scale matrix.

use crate::point::{Point, Vector};
use crate::scalar::{scalar, scalar_cos, scalar_sin};
use crate::size::Size;

/// A compressed form of a rotation+scale matrix.
///
/// ```text
/// [ fSCos     -fSSin    fTx ]
/// [ fSSin      fSCos    fTy ]
/// [     0          0      1 ]
/// ```
// Port of: include/core/SkRSXform.h#L23-L68 (chrome/m156)
#[doc(alias = "SkRSXform")]
#[derive(Copy, Clone, PartialEq, Debug, Default)]
#[repr(C)]
pub struct RSXform {
    /// `fSCos`.
    pub scos: scalar,
    /// `fSSin`.
    pub ssin: scalar,
    /// `fTx`. (Not a `Vector`, to keep this struct Skia-like.)

// Copyright 2018 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkRSXform.h (chrome/m156), the fields and `Make` only.

//! `SkRSXform`: a rotation-scale transform plus translation, used by `drawGlyphs` with `RSXform`
//! positioning.

use crate::scalar::scalar;

/// A rotation-scale transform: `(s_cos, s_sin)` is the scale times the cosine and sine of the
/// rotation, and `(tx, ty)` the translation.
// Port of: include/core/SkRSXform.h#L23-L45 (chrome/m156)
#[doc(alias = "SkRSXform")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RSXform {
    /// `fSCos`.
    pub s_cos: scalar,
    /// `fSSin`.
    pub s_sin: scalar,
    /// `fTx`.
    pub tx: scalar,
    /// `fTy`.
    pub ty: scalar,
}

impl RSXform {
    /// `SkRSXform::Make`.
    // Port of: include/core/SkRSXform.h#L24-L27 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn new(scos: scalar, ssin: scalar, t: impl Into<Vector>) -> Self {
        let t = t.into();
        Self {
            scos,
            ssin,
            tx: t.x,
            ty: t.y,
        }
    }

    /// Initializes a new xform based on the scale, rotation (in radians), final `t` location
    /// and anchor point `a` within the src quad (`MakeFromRadians`).
    ///
    /// Note: the anchor point is not normalized (e.g. 0...1) but is in pixels of the src image.
    // Port of: include/core/SkRSXform.h#L35-L40 (chrome/m156)
    #[doc(alias = "MakeFromRadians")]
    #[must_use]
    pub fn from_radians(
        scale: scalar,
        radians: scalar,
        t: impl Into<Vector>,
        a: impl Into<Point>,
    ) -> Self {
        let t = t.into();
        let a = a.into();
        let s = scalar_sin(radians) * scale;
        let c = scalar_cos(radians) * scale;
        Self::new(c, s, (t.x + -c * a.x + s * a.y, t.y + -s * a.x - c * a.y))
    }

    /// `rectStaysRect`.
    // Port of: include/core/SkRSXform.h#L47-L49 (chrome/m156)
    #[doc(alias = "rectStaysRect")]
    #[must_use]
    #[allow(clippy::float_cmp)] // mirrors `0 == fSCos || 0 == fSSin`
    pub fn rect_stays_rect(&self) -> bool {
        0.0 == self.scos || 0.0 == self.ssin
    }

    /// `setIdentity`.
    // Port of: include/core/SkRSXform.h#L51-L54 (chrome/m156)
    #[doc(alias = "setIdentity")]
    pub fn set_identity(&mut self) {
        self.scos = 1.0;
        self.ssin = 0.0;
        self.tx = 0.0;
        self.ty = 0.0;
    }

    /// `set`.
    // Port of: include/core/SkRSXform.h#L56-L61 (chrome/m156)
    pub fn set(&mut self, scos: scalar, ssin: scalar, t: impl Into<Vector>) {
        let t = t.into();
        self.scos = scos;
        self.ssin = ssin;
        self.tx = t.x;
        self.ty = t.y;
    }

    /// The four corners of a `size` rectangle transformed by this xform, clockwise from the
    /// top left (`toQuad`).
    // Port of: src/core/SkRSXform.cpp#L9-L31 (chrome/m156)
    #[doc(alias = "toQuad")]
    #[must_use]
    pub fn to_quad(self, size: impl Into<Size>) -> [Point; 4] {
        let size = size.into();
        let (width, height) = (size.width, size.height);
        let m00 = self.scos;
        let m01 = -self.ssin;
        let m02 = self.tx;
        let m10 = -m01;
        let m11 = m00;
        let m12 = self.ty;

        [
            Point::new(m02, m12),
            Point::new(m00 * width + m02, m10 * width + m12),
            Point::new(
                m00 * width + m01 * height + m02,
                m10 * width + m11 * height + m12,
            ),
            Point::new(m01 * height + m02, m11 * height + m12),
        ]
    }

    /// The four corners of a `size` rectangle transformed by this xform, in triangle strip
    /// order (`toTriStrip`).
    // Port of: src/core/SkRSXform.cpp#L33-L44 (chrome/m156)
    #[doc(alias = "toTriStrip")]
    #[must_use]
    pub fn to_tri_strip(self, size: impl Into<Size>) -> [Point; 4] {
        let size = size.into();
        let (width, height) = (size.width, size.height);
        let m00 = self.scos;
        let m01 = -self.ssin;
        let m02 = self.tx;
        let m10 = -m01;
        let m11 = m00;
        let m12 = self.ty;

        [
            Point::new(m02, m12),
            Point::new(m01 * height + m02, m11 * height + m12),
            Point::new(m00 * width + m02, m10 * width + m12),
            Point::new(
                m00 * width + m01 * height + m02,
                m10 * width + m11 * height + m12,
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::Matrix;

    #[test]
    fn quads_follow_the_matrix() {
        // Values that make every product exact.
        let x = RSXform::new(2.0, 1.0, (5.0, 7.0));
        let quad = x.to_quad((3.0, 4.0));
        let mut m = Matrix::new_identity();
        m.set_rsxform(&x);
        let corners = [
            Point::new(0.0, 0.0),
            Point::new(3.0, 0.0),
            Point::new(3.0, 4.0),
            Point::new(0.0, 4.0),
        ];
        for (q, c) in quad.iter().zip(corners) {
            assert_eq!(*q, m.map_point(c));
        }

        // The triangle strip is the quad in another order.
        let strip = x.to_tri_strip((3.0, 4.0));
        assert_eq!(strip, [quad[0], quad[3], quad[1], quad[2]]);
    }

    #[test]
    fn matrix_of_an_xform() {
        let x = RSXform::new(0.5, -0.25, (3.0, 4.0));
        let mut m = Matrix::new_identity();
        m.set_rsxform(&x);
        assert_eq!(m.scale_x(), 0.5);
        assert_eq!(m.skew_x(), 0.25);
        assert_eq!(m.skew_y(), -0.25);
        assert_eq!(m.scale_y(), 0.5);
        assert_eq!((m.translate_x(), m.translate_y()), (3.0, 4.0));
        assert!(!m.has_perspective());
    }

    #[test]
    fn construction() {
        let mut x = RSXform::default();
        x.set_identity();
        assert_eq!(x, RSXform::new(1.0, 0.0, (0.0, 0.0)));
        assert!(x.rect_stays_rect());
        x.set(0.0, 2.0, (1.0, 2.0));
        assert!(x.rect_stays_rect());
        assert_eq!((x.scos, x.ssin, x.tx, x.ty), (0.0, 2.0, 1.0, 2.0));
        assert!(!RSXform::new(1.0, 1.0, (0.0, 0.0)).rect_stays_rect());

        // Scale 2, a quarter turn about the anchor (1, 0) of the source, placed at (10, 20).
        let x = RSXform::from_radians(2.0, std::f32::consts::FRAC_PI_2, (10.0, 20.0), (1.0, 0.0));
        assert!((x.scos - 0.0).abs() < 1e-6 && (x.ssin - 2.0).abs() < 1e-6);
        // The anchor lands on the translation.
        let mut m = Matrix::new_identity();
        m.set_rsxform(&x);
        let p = m.map_point((1.0, 0.0));
        assert!((p.x - 10.0).abs() < 1e-5 && (p.y - 20.0).abs() < 1e-5);
    }

    /// `SkRSXform::Make(scos, ssin, tx, ty)`.
    // Port of: include/core/SkRSXform.h#L24-L27 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub const fn new(s_cos: scalar, s_sin: scalar, tx: scalar, ty: scalar) -> Self {
        Self {
            s_cos,
            s_sin,
            tx,
            ty,
        }
    }
}
