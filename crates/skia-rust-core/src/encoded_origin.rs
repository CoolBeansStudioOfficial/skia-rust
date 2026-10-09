// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/codec/SkEncodedOrigin.h#L14-L68 (chrome/m156)
// Ported from: include/codec/SkEncodedOrigin.h

//! The orientation of encoded image data (the EXIF orientation values).

/// The orientation of an encoded image. The values match the EXIF orientation tag
/// (www.exif.org/Exif2-2.PDF). Port of `SkEncodedOrigin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
#[doc(alias = "SkEncodedOrigin")]
pub enum EncodedOrigin {
    /// Default.
    #[default]
    #[doc(alias = "kTopLeft_SkEncodedOrigin")]
    TopLeft = 1,
    /// Reflected across y-axis.
    #[doc(alias = "kTopRight_SkEncodedOrigin")]
    TopRight = 2,
    /// Rotated 180.
    #[doc(alias = "kBottomRight_SkEncodedOrigin")]
    BottomRight = 3,
    /// Reflected across x-axis.
    #[doc(alias = "kBottomLeft_SkEncodedOrigin")]
    BottomLeft = 4,
    /// Reflected across x-axis, rotated 90 CCW.
    #[doc(alias = "kLeftTop_SkEncodedOrigin")]
    LeftTop = 5,
    /// Rotated 90 CW.
    #[doc(alias = "kRightTop_SkEncodedOrigin")]
    RightTop = 6,
    /// Reflected across x-axis, rotated 90 CW.
    #[doc(alias = "kRightBottom_SkEncodedOrigin")]
    RightBottom = 7,
    /// Rotated 90 CCW.
    #[doc(alias = "kLeftBottom_SkEncodedOrigin")]
    LeftBottom = 8,
}

impl EncodedOrigin {
    /// Port of `kDefault_SkEncodedOrigin`.
    pub const DEFAULT: Self = Self::TopLeft;
    /// Port of `kLast_SkEncodedOrigin`.
    pub const LAST: Self = Self::LeftBottom;

    /// Port of `SkEncodedOriginSwapsWidthHeight`: true for the four origins that rotate by 90
    /// degrees, which swap the image's width and height.
    // Port of: include/codec/SkEncodedOrigin.h#L67-L69 (chrome/m156)
    #[doc(alias = "SkEncodedOriginSwapsWidthHeight")]
    #[must_use]
    pub fn swaps_width_height(self) -> bool {
        self as i32 >= Self::LeftTop as i32
    }

    /// Port of `SkEncodedOriginToMatrix`: the matrix that maps an image of width `w` and height
    /// `h` in its encoded orientation to the upright orientation.
    // Port of: include/codec/SkEncodedOrigin.h#L32-L44 (chrome/m156)
    #[doc(alias = "SkEncodedOriginToMatrix")]
    #[must_use]
    // mirrors the implicit int-to-float conversion of the C++ `MakeAll(..., w, ..., h, ...)` calls
    #[allow(clippy::cast_precision_loss)]
    pub fn to_matrix(self, w: i32, h: i32) -> crate::matrix::Matrix {
        use crate::matrix::Matrix;
        let (w, h) = (w as f32, h as f32);
        match self {
            Self::TopLeft => Matrix::new_identity(),
            Self::TopRight => Matrix::new_all(-1.0, 0.0, w, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
            Self::BottomRight => Matrix::new_all(-1.0, 0.0, w, 0.0, -1.0, h, 0.0, 0.0, 1.0),
            Self::BottomLeft => Matrix::new_all(1.0, 0.0, 0.0, 0.0, -1.0, h, 0.0, 0.0, 1.0),
            Self::LeftTop => Matrix::new_all(0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0),
            Self::RightTop => Matrix::new_all(0.0, -1.0, w, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0),
            Self::RightBottom => Matrix::new_all(0.0, -1.0, w, -1.0, 0.0, h, 0.0, 0.0, 1.0),
            Self::LeftBottom => Matrix::new_all(0.0, 1.0, 0.0, -1.0, 0.0, h, 0.0, 0.0, 1.0),
        }
    }

    /// Port of `SkEncodedOriginToMatrixInverse`: a matrix equal to `to_matrix(w, h).invert()`,
    /// written out directly, as the C++ header does.
    // Port of: include/codec/SkEncodedOrigin.h#L49-L61 (chrome/m156)
    #[doc(alias = "SkEncodedOriginToMatrixInverse")]
    #[must_use]
    // mirrors the implicit int-to-float conversion of the C++ `MakeAll(..., w, ..., h, ...)` calls
    #[allow(clippy::cast_precision_loss)]
    pub fn to_matrix_inverse(self, w: i32, h: i32) -> crate::matrix::Matrix {
        use crate::matrix::Matrix;
        let (w, h) = (w as f32, h as f32);
        match self {
            Self::TopLeft => Matrix::new_identity(),
            Self::TopRight => Matrix::new_all(-1.0, 0.0, w, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
            Self::BottomRight => Matrix::new_all(-1.0, 0.0, w, 0.0, -1.0, h, 0.0, 0.0, 1.0),
            Self::BottomLeft => Matrix::new_all(1.0, 0.0, 0.0, 0.0, -1.0, h, 0.0, 0.0, 1.0),
            Self::LeftTop => Matrix::new_all(0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0),
            Self::RightTop => Matrix::new_all(0.0, 1.0, 0.0, -1.0, 0.0, w, 0.0, 0.0, 1.0),
            Self::RightBottom => Matrix::new_all(0.0, -1.0, h, -1.0, 0.0, w, 0.0, 0.0, 1.0),
            Self::LeftBottom => Matrix::new_all(0.0, -1.0, h, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0),
        }
    }
}
