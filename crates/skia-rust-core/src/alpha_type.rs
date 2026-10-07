// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkAlphaType.h

//! [`AlphaType`]: how to interpret the alpha component of a pixel.

/// Describes how to interpret the alpha component of a pixel. A pixel may be opaque, or alpha,
/// describing multiple levels of transparency.
///
/// In simple blending, alpha weights the draw color and the destination color to create a new
/// color. If alpha describes a weight from zero to one:
///
/// `new color = draw color * alpha + destination color * (1 - alpha)`
///
/// In practice alpha is encoded in two or more bits, where 1.0 equals all bits set.
///
/// RGB may have alpha included in each component value; the stored value is the original RGB
/// multiplied by alpha. Premultiplied color components improve performance.
// Port of: include/core/SkAlphaType.h#L26-L32 (chrome/m156)
#[doc(alias = "SkAlphaType")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum AlphaType {
    /// Uninitialized.
    #[doc(alias = "kUnknown_SkAlphaType")]
    #[default]
    Unknown = 0,
    /// Pixel is opaque.
    #[doc(alias = "kOpaque_SkAlphaType")]
    Opaque = 1,
    /// Pixel components are premultiplied by alpha.
    #[doc(alias = "kPremul_SkAlphaType")]
    Premul = 2,
    /// Pixel components are independent of alpha.
    #[doc(alias = "kUnpremul_SkAlphaType")]
    Unpremul = 3,
}

impl AlphaType {
    /// The last valid value.
    #[doc(alias = "kLastEnum_SkAlphaType")]
    pub const LAST_ENUM: Self = Self::Unpremul;

    /// Returns true if `self` is [`AlphaType::Opaque`].
    ///
    /// [`AlphaType::Opaque`] is a hint that the color type is opaque, or that all alpha values
    /// are set to their 1.0 equivalent. If the alpha type is [`AlphaType::Opaque`], and the color
    /// type is not opaque, then the result of drawing any pixel with a alpha value less than 1.0
    /// is undefined.
    // Port of: include/core/SkAlphaType.h#L41-L43 (chrome/m156)
    #[doc(alias = "SkAlphaTypeIsOpaque")]
    #[must_use]
    pub fn is_opaque(self) -> bool {
        self == Self::Opaque
    }
}
