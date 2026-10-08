// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/SkGradientShader.h (the pre-`SkGradient` factories, which m156
// no longer has: they are what `skia-safe`'s `gradient_shader` module wraps around `SkShaders`)

//! The older gradient factories of `skia-safe`'s `gradient_shader` module: free functions that
//! take the colors, positions, tile mode and flags separately, and make the shader with the
//! `SkGradient` API ([`crate::gradient`]). Prefer [`gradient::shaders`](crate::gradient::shaders).
//!
//! skia-rust: `skia-safe` also has these as associated functions of `Shader`
//! (`Shader::linear_gradient`, ...); `Shader` is defined in `skia-rust-core`, which cannot call
//! into this crate, so they are only free functions here.

use bitflags::bitflags;

use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;

use crate::gradient::{Gradient, Interpolation, interpolation, shaders};

pub use crate::gradient::Colors as GradientColors;

bitflags! {
    /// `SkGradientShader::Flags`.
    #[doc(alias = "SkGradientShader::Flags")]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
    pub struct Flags: u32 {
        /// `kInterpolateColorsInPremul_Flag`.
        const INTERPOLATE_COLORS_IN_PREMUL = 1 << 0;
    }
}

impl From<Flags> for Interpolation {
    fn from(flags: Flags) -> Self {
        let in_premul = if flags.contains(Flags::INTERPOLATE_COLORS_IN_PREMUL) {
            interpolation::InPremul::Yes
        } else {
            interpolation::InPremul::No
        };
        Self {
            in_premul,
            color_space: interpolation::ColorSpace::Destination,
            hue_method: interpolation::HueMethod::Shorter,
        }
    }
}

/// Either a slice of [`Color`], or a slice of [`Color4f`] and a color space. Whenever this type
/// is expected, it's either possible to directly pass a `&[Color]`, or a tuple of type
/// `(&[Color4f], ColorSpace)`.
#[derive(Debug)]
pub enum GradientShaderColors<'a> {
    /// 8-bit sRGB colors.
    Colors(&'a [Color]),
    /// Float colors in a color space (sRGB if `None`).
    ColorsInSpace(&'a [Color4f], Option<ColorSpace>),
}

impl GradientShaderColors<'_> {
    /// The number of colors.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            GradientShaderColors::Colors(colors) => colors.len(),
            GradientShaderColors::ColorsInSpace(colors, _) => colors.len(),
        }
    }

    /// Whether there are no colors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<'a> From<&'a [Color]> for GradientShaderColors<'a> {
    fn from(colors: &'a [Color]) -> Self {
        GradientShaderColors::Colors(colors)
    }
}

impl<'a> From<(&'a [Color4f], ColorSpace)> for GradientShaderColors<'a> {
    fn from(c: (&'a [Color4f], ColorSpace)) -> Self {
        GradientShaderColors::ColorsInSpace(c.0, Some(c.1))
    }
}

impl<'a> From<(&'a [Color4f], Option<ColorSpace>)> for GradientShaderColors<'a> {
    fn from(c: (&'a [Color4f], Option<ColorSpace>)) -> Self {
        GradientShaderColors::ColorsInSpace(c.0, c.1)
    }
}

impl<'a> From<&'a [Color4f]> for GradientShaderColors<'a> {
    fn from(c: &'a [Color4f]) -> Self {
        GradientShaderColors::ColorsInSpace(c, None)
    }
}

/// Makes the `SkGradient` of the arguments of the older factories and calls `make` with it.
fn with_gradient<R>(
    colors: GradientShaderColors<'_>,
    pos: Option<&[scalar]>,
    mode: TileMode,
    flags: Flags,
    make: impl FnOnce(&Gradient<'_>) -> R,
) -> R {
    match colors {
        GradientShaderColors::Colors(colors) => {
            // Convert Color to Color4f
            let colors4f: Vec<Color4f> = colors.iter().map(|c| Color4f::from(*c)).collect();
            let grad_colors = GradientColors::new(&colors4f, pos, mode, None);
            make(&Gradient::new(grad_colors, flags))
        }
        GradientShaderColors::ColorsInSpace(colors, color_space) => {
            let grad_colors = GradientColors::new(colors, pos, mode, color_space);
            make(&Gradient::new(grad_colors, flags))
        }
    }
}

/// A linear gradient between two points (`SkGradientShader::MakeLinear`).
#[doc(alias = "MakeLinear")]
#[must_use]
pub fn linear<'a>(
    points: (impl Into<Point>, impl Into<Point>),
    colors: impl Into<GradientShaderColors<'a>>,
    pos: impl Into<Option<&'a [scalar]>>,
    mode: TileMode,
    flags: impl Into<Option<Flags>>,
    local_matrix: impl Into<Option<&'a Matrix>>,
) -> Option<Shader> {
    let local_matrix = local_matrix.into();
    with_gradient(
        colors.into(),
        pos.into(),
        mode,
        flags.into().unwrap_or_default(),
        |grad| shaders::linear_gradient(points, grad, local_matrix),
    )
}

/// A linear gradient between two points with explicit interpolation settings.
#[must_use]
pub fn linear_with_interpolation<'a>(
    points: (impl Into<Point>, impl Into<Point>),
    (colors, color_space): (&'a [Color4f], impl Into<Option<ColorSpace>>),
    pos: impl Into<Option<&'a [scalar]>>,
    mode: TileMode,
    interpolation: impl Into<Interpolation>,
    local_matrix: impl Into<Option<&'a Matrix>>,
) -> Option<Shader> {
    let grad_colors = GradientColors::new(colors, pos.into(), mode, color_space.into());
    let grad = Gradient::new(grad_colors, interpolation);
    shaders::linear_gradient(points, &grad, local_matrix)
}

/// A radial gradient (`SkGradientShader::MakeRadial`).
#[doc(alias = "MakeRadial")]
#[must_use]
pub fn radial<'a>(
    center: impl Into<Point>,
    radius: scalar,
    colors: impl Into<GradientShaderColors<'a>>,
    pos: impl Into<Option<&'a [scalar]>>,
    mode: TileMode,
    flags: impl Into<Option<Flags>>,
    local_matrix: impl Into<Option<&'a Matrix>>,
) -> Option<Shader> {
    let local_matrix = local_matrix.into();
    with_gradient(
        colors.into(),
        pos.into(),
        mode,
        flags.into().unwrap_or_default(),
        |grad| shaders::radial_gradient((center, radius), grad, local_matrix),
    )
}

/// A radial gradient with explicit interpolation settings.
#[must_use]
pub fn radial_with_interpolation<'a>(
    (center, radius): (impl Into<Point>, scalar),
    (colors, color_space): (&'a [Color4f], impl Into<Option<ColorSpace>>),
    pos: impl Into<Option<&'a [scalar]>>,
    mode: TileMode,
    interpolation: impl Into<Interpolation>,
    local_matrix: impl Into<Option<&'a Matrix>>,
) -> Option<Shader> {
    let grad_colors = GradientColors::new(colors, pos.into(), mode, color_space.into());
    let grad = Gradient::new(grad_colors, interpolation);
    shaders::radial_gradient((center, radius), &grad, local_matrix)
}

/// A two-point conical gradient (`SkGradientShader::MakeTwoPointConical`).
#[doc(alias = "MakeTwoPointConical")]
#[must_use]
#[allow(clippy::too_many_arguments)] // mirrors the skia-safe signature
pub fn two_point_conical<'a>(
    start: impl Into<Point>,
    start_radius: scalar,
    end: impl Into<Point>,
    end_radius: scalar,
    colors: impl Into<GradientShaderColors<'a>>,
    pos: impl Into<Option<&'a [scalar]>>,
    mode: TileMode,
    flags: impl Into<Option<Flags>>,
    local_matrix: impl Into<Option<&'a Matrix>>,
) -> Option<Shader> {
    let local_matrix = local_matrix.into();
    with_gradient(
        colors.into(),
        pos.into(),
        mode,
        flags.into().unwrap_or_default(),
        |grad| {
            shaders::two_point_conical_gradient(
                (start, start_radius),
                (end, end_radius),
                grad,
                local_matrix,
            )
        },
    )
}

/// A two-point conical gradient with explicit interpolation settings.
#[must_use]
pub fn two_point_conical_with_interpolation<'a>(
    (start, start_radius): (impl Into<Point>, scalar),
    (end, end_radius): (impl Into<Point>, scalar),
    (colors, color_space): (&'a [Color4f], impl Into<Option<ColorSpace>>),
    pos: impl Into<Option<&'a [scalar]>>,
    mode: TileMode,
    interpolation: impl Into<Interpolation>,
    local_matrix: impl Into<Option<&'a Matrix>>,
) -> Option<Shader> {
    let grad_colors = GradientColors::new(colors, pos.into(), mode, color_space.into());
    let grad = Gradient::new(grad_colors, interpolation);
    shaders::two_point_conical_gradient(
        (start, start_radius),
        (end, end_radius),
        &grad,
        local_matrix,
    )
}

/// A sweep gradient (`SkGradientShader::MakeSweep`); the angles default to 0 and 360.
#[doc(alias = "MakeSweep")]
#[must_use]
pub fn sweep<'a>(
    center: impl Into<Point>,
    colors: impl Into<GradientShaderColors<'a>>,
    pos: impl Into<Option<&'a [scalar]>>,
    mode: TileMode,
    angles: impl Into<Option<(scalar, scalar)>>,
    flags: impl Into<Option<Flags>>,
    local_matrix: impl Into<Option<&'a Matrix>>,
) -> Option<Shader> {
    let angles = angles.into().unwrap_or((0.0, 360.0));
    let local_matrix = local_matrix.into();
    with_gradient(
        colors.into(),
        pos.into(),
        mode,
        flags.into().unwrap_or_default(),
        |grad| shaders::sweep_gradient(center, angles, grad, local_matrix),
    )
}

/// A sweep gradient with explicit interpolation settings.
#[must_use]
pub fn sweep_with_interpolation<'a>(
    center: impl Into<Point>,
    (colors, color_space): (&'a [Color4f], impl Into<Option<ColorSpace>>),
    pos: impl Into<Option<&'a [scalar]>>,
    mode: TileMode,
    angles: impl Into<Option<(scalar, scalar)>>,
    interpolation: impl Into<Interpolation>,
    local_matrix: impl Into<Option<&'a Matrix>>,
) -> Option<Shader> {
    let angles = angles.into().unwrap_or((0.0, 360.0));
    let grad_colors = GradientColors::new(colors, pos.into(), mode, color_space.into());
    let grad = Gradient::new(grad_colors, interpolation);
    shaders::sweep_gradient(center, angles, &grad, local_matrix)
}
