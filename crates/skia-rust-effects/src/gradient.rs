// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/SkGradient.h

//! `SkGradient`: the description of a gradient's colors and how they are interpolated, and the
//! `SkShaders` factories that make gradient shaders from it. The API follows `skia-safe`'s
//! `gradient` module ([`Gradient`], [`Colors`], [`Interpolation`], [`shaders`]).

use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::scalar::scalar;
use skia_rust_core::tile_mode::TileMode;

/// Gradient interpolation settings (`SkGradient::Interpolation`): in which color space, and
/// whether premultiplied, the colors are interpolated.
// Port of: include/effects/SkGradient.h#L21-L83 (chrome/m156)
#[doc(alias = "SkGradient::Interpolation")]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub struct Interpolation {
    /// `fInPremul`.
    pub in_premul: interpolation::InPremul,
    /// `fColorSpace`.
    pub color_space: interpolation::ColorSpace,
    /// `fHueMethod`: only relevant for LCH, OKLCH, HSL, or HWB.
    pub hue_method: interpolation::HueMethod,
}

pub mod interpolation {
    //! The settings of [`Interpolation`](super::Interpolation): the interpolation color space,
    //! premultiplication mode and hue method.

    /// Whether to interpolate colors in premultiplied alpha space
    /// (`SkGradient::Interpolation::InPremul`).
    #[doc(alias = "SkGradient::Interpolation::InPremul")]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
    pub enum InPremul {
        /// `kNo`.
        #[default]
        No = 0,
        /// `kYes`.
        Yes = 1,
    }

    /// Color space for gradient interpolation
    /// (`SkGradient::Interpolation::ColorSpace`). See
    /// <https://www.w3.org/TR/css-color-4/#interpolation-space>.
    #[doc(alias = "SkGradient::Interpolation::ColorSpace")]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
    pub enum ColorSpace {
        /// Default Skia behavior: interpolate in the color space of the destination surface
        /// (`kDestination`).
        #[default]
        Destination = 0,
        /// `kSRGBLinear`.
        SRGBLinear,
        /// `kLab`.
        Lab,
        /// `kOKLab`.
        OKLab,
        /// The same as `OKLab`, except it has a simplified version of the CSS gamut mapping
        /// algorithm into Rec2020 space applied to it (`kOKLabGamutMap`). Experimental.
        OKLabGamutMap,
        /// `kLCH`.
        LCH,
        /// `kOKLCH`.
        OKLCH,
        /// The same as `OKLCH`, except it has the same gamut mapping applied to it as
        /// `OKLabGamutMap` does (`kOKLCHGamutMap`). Experimental.
        OKLCHGamutMap,
        /// `kSRGB`.
        SRGB,
        /// `kHSL`.
        HSL,
        /// `kHWB`.
        HWB,
        /// `kDisplayP3`.
        DisplayP3,
        /// `kRec2020`.
        Rec2020,
        /// `kProphotoRGB`.
        ProphotoRGB,
        /// `kA98RGB` (`kLastColorSpace`).
        A98RGB,
    }

    /// `kColorSpaceCount`.
    #[doc(alias = "kColorSpaceCount")]
    pub const COLOR_SPACE_COUNT: usize = ColorSpace::A98RGB as usize + 1;

    /// Hue interpolation method for cylindrical color spaces (LCH, OKLCH, HSL, HWB)
    /// (`SkGradient::Interpolation::HueMethod`). See
    /// <https://www.w3.org/TR/css-color-4/#hue-interpolation>.
    #[doc(alias = "SkGradient::Interpolation::HueMethod")]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
    pub enum HueMethod {
        /// `kShorter`.
        #[default]
        Shorter = 0,
        /// `kLonger`.
        Longer,
        /// `kIncreasing`.
        Increasing,
        /// `kDecreasing` (`kLastHueMethod`).
        Decreasing,
    }

    /// `kHueMethodCount`.
    #[doc(alias = "kHueMethodCount")]
    pub const HUE_METHOD_COUNT: usize = HueMethod::Decreasing as usize + 1;
}

impl Interpolation {
    /// Interpolation settings from the legacy `SkGradientShader` flags (`FromFlags`): bit 0 is
    /// "interpolate colors in premul".
    // Port of: include/effects/SkGradient.h#L76-L81 (chrome/m156)
    #[doc(alias = "FromFlags")]
    #[must_use]
    pub fn from_flags(flags: u32) -> Self {
        Self {
            in_premul: if flags & 1 != 0 {
                interpolation::InPremul::Yes
            } else {
                interpolation::InPremul::No
            },
            color_space: interpolation::ColorSpace::Destination,
            hue_method: interpolation::HueMethod::Shorter,
        }
    }
}

/// Specification for the colors in a gradient (`SkGradient::Colors`): the colors, their
/// positions, the tile mode and the color space of the colors.
///
/// The positions, if given, are relative positions of each color across the gradient and must
/// lie between 0.0 and 1.0 and be strictly increasing. If the first value is not 0.0, an
/// additional color stop is added at position 0.0 with the same color as the first color; if the
/// last value is less than 1.0, one is added at 1.0 with the last color.
// Port of: include/effects/SkGradient.h#L85-L127 (chrome/m156)
#[doc(alias = "SkGradient::Colors")]
#[derive(Debug, Clone)]
#[allow(clippy::struct_field_names)] // mirrors SkGradient::Colors::fColors
pub struct Colors<'a> {
    colors: &'a [Color4f],
    pos: Option<&'a [scalar]>,
    color_space: Option<ColorSpace>,
    tile_mode: TileMode,
}

impl Default for Colors<'_> {
    // Port of: include/effects/SkGradient.h#L88 (chrome/m156)
    fn default() -> Self {
        Colors {
            colors: &[],
            pos: None,
            color_space: None,
            tile_mode: TileMode::Clamp,
        }
    }
}

impl<'a> Colors<'a> {
    /// Colors with explicit positions (`Colors(colors, pos, mode, cs)`).
    ///
    /// - `colors`: the colors for the gradient.
    /// - `pos`: relative positions of each color (0.0 to 1.0, strictly increasing); `None` to
    ///   distribute the colors evenly. Skia asserts that the lengths match and throws away
    ///   positions of another length.
    /// - `tile_mode`: tiling mode for the gradient.
    /// - `color_space`: optional color space of the colors; sRGB if `None`.
    // Port of: include/effects/SkGradient.h#L89-L99 (chrome/m156)
    #[must_use]
    pub fn new(
        colors: &'a [Color4f],
        pos: Option<&'a [scalar]>,
        tile_mode: TileMode,
        color_space: impl Into<Option<ColorSpace>>,
    ) -> Self {
        debug_assert!(pos.is_none_or(|pos| pos.is_empty() || pos.len() == colors.len()));

        // throw away inconsistent inputs
        let pos = pos.filter(|pos| pos.len() == colors.len());
        Self {
            colors,
            pos,
            color_space: color_space.into(),
            tile_mode,
        }
    }

    /// Colors distributed evenly (`Colors(colors, tm, cs)`).
    // Port of: include/effects/SkGradient.h#L101-L103 (chrome/m156)
    #[must_use]
    pub fn new_evenly_spaced(
        colors: &'a [Color4f],
        tile_mode: TileMode,
        color_space: impl Into<Option<ColorSpace>>,
    ) -> Self {
        Self::new(colors, None, tile_mode, color_space)
    }

    /// The colors (`colors`).
    #[must_use]
    pub fn colors(&self) -> &'a [Color4f] {
        self.colors
    }

    /// The positions, if they were given (`positions`).
    #[must_use]
    pub fn positions(&self) -> Option<&'a [scalar]> {
        self.pos
    }

    /// The color space of the colors, if one was given (`colorSpace`).
    #[must_use]
    pub fn color_space(&self) -> Option<&ColorSpace> {
        self.color_space.as_ref()
    }

    /// The tile mode (`tileMode`).
    #[must_use]
    pub fn tile_mode(&self) -> TileMode {
        self.tile_mode
    }
}

/// A gradient's colors and interpolation settings (`SkGradient`).
// Port of: include/effects/SkGradient.h#L19-L137 (chrome/m156)
#[doc(alias = "SkGradient")]
#[derive(Debug, Clone, Default)]
pub struct Gradient<'a> {
    colors: Colors<'a>,
    interpolation: Interpolation,
}

impl<'a> Gradient<'a> {
    /// `SkGradient(colors, interp)`.
    #[must_use]
    pub fn new(colors: Colors<'a>, interpolation: impl Into<Interpolation>) -> Self {
        Self {
            colors,
            interpolation: interpolation.into(),
        }
    }

    /// The colors (`colors`).
    #[must_use]
    pub fn colors(&self) -> &Colors<'a> {
        &self.colors
    }

    /// The interpolation settings (`interpolation`).
    #[must_use]
    pub fn interpolation(&self) -> &Interpolation {
        &self.interpolation
    }
}

pub mod shaders {
    //! The gradient shader factories of `SkShaders` (`SkShaders::LinearGradient` and friends).

    use skia_rust_core::matrix::Matrix;
    use skia_rust_core::point::Point;
    use skia_rust_core::scalar::scalar;
    use skia_rust_core::shader::Shader;

    use super::Gradient;
    use crate::conical_gradient::two_point_conical_gradient as make_two_point_conical;
    use crate::linear_gradient::linear_gradient as make_linear;
    use crate::radial_gradient::radial_gradient as make_radial;
    use crate::sweep_gradient::sweep_gradient as make_sweep;

    /// A shader that generates a linear gradient between the two specified points
    /// (`SkShaders::LinearGradient`). `None` if the inputs are invalid.
    ///
    /// - `points`: the end-points of the line segment.
    /// - `gradient`: the colors and interpolation method.
    /// - `local_matrix`: optional local matrix.
    #[doc(alias = "LinearGradient")]
    #[must_use]
    pub fn linear_gradient<'a>(
        points: (impl Into<Point>, impl Into<Point>),
        gradient: &Gradient<'_>,
        local_matrix: impl Into<Option<&'a Matrix>>,
    ) -> Option<Shader> {
        let points = [points.0.into(), points.1.into()];
        make_linear(&points, gradient, local_matrix.into())
    }

    /// A shader that generates a radial gradient given the center and radius
    /// (`SkShaders::RadialGradient`). `None` if the inputs are invalid.
    ///
    /// - `center`, `radius`: the circle of the gradient; the radius must not be negative.
    /// - `gradient`: the colors and interpolation method.
    /// - `local_matrix`: optional local matrix.
    #[doc(alias = "RadialGradient")]
    #[must_use]
    pub fn radial_gradient<'a>(
        (center, radius): (impl Into<Point>, scalar),
        gradient: &Gradient<'_>,
        local_matrix: impl Into<Option<&'a Matrix>>,
    ) -> Option<Shader> {
        make_radial(center.into(), radius, gradient, local_matrix.into())
    }

    /// A shader that generates a conical gradient given two circles, or `None` if the inputs
    /// are invalid (`SkShaders::TwoPointConicalGradient`). The gradient interprets the two
    /// circles according to the HTML spec
    /// <http://dev.w3.org/html5/2dcontext/#dom-context-2d-createradialgradient>.
    ///
    /// - `start`, `start_radius`: the start circle; the radius must not be negative.
    /// - `end`, `end_radius`: the end circle; the radius must not be negative.
    /// - `gradient`: the colors and interpolation method.
    /// - `local_matrix`: optional local matrix.
    #[doc(alias = "TwoPointConicalGradient")]
    #[must_use]
    pub fn two_point_conical_gradient<'a>(
        (start, start_radius): (impl Into<Point>, scalar),
        (end, end_radius): (impl Into<Point>, scalar),
        gradient: &Gradient<'_>,
        local_matrix: impl Into<Option<&'a Matrix>>,
    ) -> Option<Shader> {
        make_two_point_conical(
            start.into(),
            start_radius,
            end.into(),
            end_radius,
            gradient,
            local_matrix.into(),
        )
    }

    /// A shader that generates a sweep gradient given a center (`SkShaders::SweepGradient`).
    ///
    /// The shader accepts negative angles and angles larger than 360, draws between 0 and 360
    /// degrees, similar to the CSS conic-gradient semantics. 0 degrees means horizontal
    /// positive x axis. The start angle must not be greater than the end angle, otherwise
    /// `None` is returned. If color stops do not contain 0 and 1 but are within this range, the
    /// respective outer color stop is repeated for 0 and 1. Color stops less than 0 are clamped
    /// to 0, and greater than 1 are clamped to 1.
    ///
    /// - `center`: the center of the sweep.
    /// - `start_angle`, `end_angle`: the angular range, corresponding to positions 0 and 1.
    /// - `gradient`: the colors and interpolation method.
    /// - `local_matrix`: optional local matrix.
    #[doc(alias = "SweepGradient")]
    #[must_use]
    pub fn sweep_gradient<'a>(
        center: impl Into<Point>,
        (start_angle, end_angle): (scalar, scalar),
        gradient: &Gradient<'_>,
        local_matrix: impl Into<Option<&'a Matrix>>,
    ) -> Option<Shader> {
        make_sweep(
            center.into(),
            start_angle,
            end_angle,
            gradient,
            local_matrix.into(),
        )
    }
}
