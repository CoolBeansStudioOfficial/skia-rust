// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkSurfaceProps.h, src/image/SkSurface.cpp (`SkSurfaceProps::SkSurfaceProps`)

//! [`SurfaceProps`]: properties and constraints of a surface (pixel geometry, flags, text gamma).
//!
//! skia-rust: ported with D5 because `SkDevice` stores one. Nothing in the raster path reads the
//! text contrast and gamma yet (text is a later phase).

use bitflags::bitflags;

use crate::scalar::scalar;

/// How the LCD strips are arranged for each pixel (`SkPixelGeometry`). If this is unknown, or the
/// pixels are meant to be "portable" and/or transformed before showing (e.g. rotated, scaled)
/// then use [`PixelGeometry::Unknown`].
// Port of: include/core/SkSurfaceProps.h#L16-L22 (chrome/m156)
#[doc(alias = "SkPixelGeometry")]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Hash)]
#[repr(i32)]
pub enum PixelGeometry {
    /// `kUnknown_SkPixelGeometry`.
    #[default]
    Unknown = 0,
    /// `kRGB_H_SkPixelGeometry`.
    RGBH = 1,
    /// `kBGR_H_SkPixelGeometry`.
    BGRH = 2,
    /// `kRGB_V_SkPixelGeometry`.
    RGBV = 3,
    /// `kBGR_V_SkPixelGeometry`.
    BGRV = 4,
}

impl PixelGeometry {
    /// True iff the geometry is known and is RGB (`SkPixelGeometryIsRGB`).
    // Port of: include/core/SkSurfaceProps.h#L25-L27 (chrome/m156)
    #[doc(alias = "SkPixelGeometryIsRGB")]
    #[must_use]
    pub fn is_rgb(self) -> bool {
        self == PixelGeometry::RGBH || self == PixelGeometry::RGBV
    }

    /// True iff the geometry is known and is BGR (`SkPixelGeometryIsBGR`).
    // Port of: include/core/SkSurfaceProps.h#L30-L32 (chrome/m156)
    #[doc(alias = "SkPixelGeometryIsBGR")]
    #[must_use]
    pub fn is_bgr(self) -> bool {
        self == PixelGeometry::BGRH || self == PixelGeometry::BGRV
    }

    /// True iff the geometry is known and is horizontal (`SkPixelGeometryIsH`).
    // Port of: include/core/SkSurfaceProps.h#L35-L37 (chrome/m156)
    #[doc(alias = "SkPixelGeometryIsH")]
    #[must_use]
    pub fn is_h(self) -> bool {
        self == PixelGeometry::RGBH || self == PixelGeometry::BGRH
    }

    /// True iff the geometry is known and is vertical (`SkPixelGeometryIsV`).
    // Port of: include/core/SkSurfaceProps.h#L40-L42 (chrome/m156)
    #[doc(alias = "SkPixelGeometryIsV")]
    #[must_use]
    pub fn is_v(self) -> bool {
        self == PixelGeometry::RGBV || self == PixelGeometry::BGRV
    }
}

bitflags! {
    /// `SkSurfaceProps::Flags`.
    // Port of: include/core/SkSurfaceProps.h#L54-L65 (chrome/m156)
    #[doc(alias = "SkSurfaceProps::Flags")]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
    pub struct SurfacePropsFlags: u32 {
        /// `kUseDeviceIndependentFonts_Flag`.
        const USE_DEVICE_INDEPENDENT_FONTS = 1 << 0;
        /// `kDynamicMSAA_Flag`: use internal MSAA to render to non-MSAA GPU surfaces.
        const DYNAMIC_MSAA = 1 << 1;
        /// `kAlwaysDither_Flag`: all rendering will have dithering enabled (GPU backends only).
        const ALWAYS_DITHER = 1 << 2;
        /// `kPreservesTransparentDraws_Flag`: the surface will preserve transparent draws.
        const PRESERVES_TRANSPARENT_DRAWS = 1 << 3;
    }
}

impl SurfacePropsFlags {
    /// `kDefault_Flag` (no flags).
    pub const DEFAULT: SurfacePropsFlags = SurfacePropsFlags::empty();
}

/// The default text contrast (`SK_GAMMA_CONTRAST`).
const GAMMA_CONTRAST: scalar = 0.5;
/// The default text gamma (`SK_GAMMA_EXPONENT`, 0 meaning sRGB).
const GAMMA_EXPONENT: scalar = 0.0;

/// Properties and constraints of a surface (`SkSurfaceProps`). The rendering engine can parse
/// these during drawing, and can sometimes optimize its performance (e.g. disabling an expensive
/// feature).
// Port of: include/core/SkSurfaceProps.h#L47-L117 (chrome/m156)
#[doc(alias = "SkSurfaceProps")]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SurfaceProps {
    flags: SurfacePropsFlags,
    pixel_geometry: PixelGeometry,
    text_contrast: scalar,
    text_gamma: scalar,
}

impl Default for SurfaceProps {
    /// No flags, unknown pixel geometry, platform-default contrast and gamma.
    // Port of: src/image/SkSurface.cpp#L37-L41 (chrome/m156)
    fn default() -> Self {
        Self::new(SurfacePropsFlags::DEFAULT, PixelGeometry::Unknown)
    }
}

impl SurfaceProps {
    /// `kMaxContrastInclusive`.
    pub const MAX_CONTRAST_INCLUSIVE: scalar = 1.0;
    /// `kMinContrastInclusive`.
    pub const MIN_CONTRAST_INCLUSIVE: scalar = 0.0;
    /// `kMaxGammaExclusive`.
    pub const MAX_GAMMA_EXCLUSIVE: scalar = 4.0;
    /// `kMinGammaInclusive`.
    pub const MIN_GAMMA_INCLUSIVE: scalar = 0.0;

    /// The given flags and pixel geometry, with the platform-default text contrast and gamma.
    // Port of: src/image/SkSurface.cpp#L43-L47 (chrome/m156)
    #[must_use]
    pub fn new(flags: SurfacePropsFlags, pixel_geometry: PixelGeometry) -> SurfaceProps {
        SurfaceProps {
            flags,
            pixel_geometry,
            text_contrast: GAMMA_CONTRAST,
            text_gamma: GAMMA_EXPONENT,
        }
    }

    /// The given flags, pixel geometry, text contrast and gamma.
    // Port of: src/image/SkSurface.cpp#L49-L53 (chrome/m156)
    #[must_use]
    pub fn new_with_text_properties(
        flags: SurfacePropsFlags,
        pixel_geometry: PixelGeometry,
        text_contrast: scalar,
        text_gamma: scalar,
    ) -> SurfaceProps {
        SurfaceProps {
            flags,
            pixel_geometry,
            text_contrast,
            text_gamma,
        }
    }

    /// A copy with another pixel geometry (`cloneWithPixelGeometry`).
    // Port of: include/core/SkSurfaceProps.h#L74-L76 (chrome/m156)
    #[doc(alias = "cloneWithPixelGeometry")]
    #[must_use]
    pub fn clone_with_pixel_geometry(&self, new_pixel_geometry: PixelGeometry) -> SurfaceProps {
        SurfaceProps::new_with_text_properties(
            self.flags,
            new_pixel_geometry,
            self.text_contrast,
            self.text_gamma,
        )
    }

    /// The flags.
    #[must_use]
    pub fn flags(&self) -> SurfacePropsFlags {
        self.flags
    }

    /// The pixel geometry (`pixelGeometry`).
    #[doc(alias = "pixelGeometry")]
    #[must_use]
    pub fn pixel_geometry(&self) -> PixelGeometry {
        self.pixel_geometry
    }

    /// The text contrast (`textContrast`).
    #[doc(alias = "textContrast")]
    #[must_use]
    pub fn text_contrast(&self) -> scalar {
        self.text_contrast
    }

    /// The text gamma (`textGamma`).
    #[doc(alias = "textGamma")]
    #[must_use]
    pub fn text_gamma(&self) -> scalar {
        self.text_gamma
    }

    /// `isUseDeviceIndependentFonts`.
    #[doc(alias = "isUseDeviceIndependentFonts")]
    #[must_use]
    pub fn is_use_device_independent_fonts(&self) -> bool {
        self.flags
            .contains(SurfacePropsFlags::USE_DEVICE_INDEPENDENT_FONTS)
    }

    /// `isAlwaysDither`.
    #[doc(alias = "isAlwaysDither")]
    #[must_use]
    pub fn is_always_dither(&self) -> bool {
        self.flags.contains(SurfacePropsFlags::ALWAYS_DITHER)
    }

    /// `preservesTransparentDraws`.
    #[doc(alias = "preservesTransparentDraws")]
    #[must_use]
    pub fn preserves_transparent_draws(&self) -> bool {
        self.flags
            .contains(SurfacePropsFlags::PRESERVES_TRANSPARENT_DRAWS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::float_cmp)] // the defaults are exact constants
    fn defaults_and_clone_with_pixel_geometry() {
        let p = SurfaceProps::default();
        assert_eq!(p.flags(), SurfacePropsFlags::DEFAULT);
        assert_eq!(p.pixel_geometry(), PixelGeometry::Unknown);
        assert_eq!(p.text_contrast(), 0.5);
        assert_eq!(p.text_gamma(), 0.0);

        let q = SurfaceProps::new_with_text_properties(
            SurfacePropsFlags::ALWAYS_DITHER,
            PixelGeometry::RGBH,
            0.25,
            2.0,
        )
        .clone_with_pixel_geometry(PixelGeometry::BGRV);
        assert!(q.is_always_dither() && !q.is_use_device_independent_fonts());
        assert!(q.pixel_geometry().is_bgr() && q.pixel_geometry().is_v());
        assert_eq!((q.text_contrast(), q.text_gamma()), (0.25, 2.0));
        assert_ne!(p, q);
    }
}
