// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/Blend.h, src/gpu/Blend.cpp

//! `skgpu::BlendEquation`, `skgpu::BlendCoeff` and the predicates on them, plus the mapping from
//! Porter-Duff and advanced blend modes to shader functions.
//!
//! The SK_DEBUG-only dump helpers of `Blend.cpp` (`equation_string`, `coeff_string`, ...) are
//! not ported: they only print.

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::PMColor4f;

/// How source and destination are combined.
// Port of: src/gpu/Blend.h#L26-L53 (chrome/m156)
#[doc(alias = "skgpu::BlendEquation")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum BlendEquation {
    // Basic blend equations.
    /// `Cs*S + Cd*D`.
    Add,
    /// `Cs*S - Cd*D`.
    Subtract,
    /// `Cd*D - Cs*S`.
    ReverseSubtract,

    // Advanced blend equations. These are described in the SVG and PDF specs.
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Multiply,
    HslHue,
    HslSaturation,
    HslColor,
    HslLuminosity,
    Illegal,
}

impl BlendEquation {
    /// `kFirstAdvanced`.
    pub const FIRST_ADVANCED: BlendEquation = BlendEquation::Screen;
    /// `kLast`.
    pub const LAST: BlendEquation = BlendEquation::Illegal;
}

// Port of: src/gpu/Blend.h#L55 (chrome/m156)
/// `kBlendEquationCnt`.
pub const K_BLEND_EQUATION_CNT: usize = BlendEquation::LAST as usize + 1;

/// Coefficients of the source and destination in a blend equation.
// Port of: src/gpu/Blend.h#L60-L81 (chrome/m156)
#[doc(alias = "skgpu::BlendCoeff")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum BlendCoeff {
    /// 0
    Zero,
    /// 1
    One,
    /// src color
    SC,
    /// one minus src color
    ISC,
    /// dst color
    DC,
    /// one minus dst color
    IDC,
    /// src alpha
    SA,
    /// one minus src alpha
    ISA,
    /// dst alpha
    DA,
    /// one minus dst alpha
    IDA,
    /// constant color
    ConstC,
    /// one minus constant color
    IConstC,
    S2C,
    IS2C,
    S2A,
    IS2A,
    Illegal,
}

impl BlendCoeff {
    /// `kLast`.
    pub const LAST: BlendCoeff = BlendCoeff::Illegal;
}

// Port of: src/gpu/Blend.h#L101 (chrome/m156)
/// `kBlendCoeffCnt`.
pub const K_BLEND_COEFF_CNT: usize = BlendCoeff::LAST as usize + 1;

/// The blend state of a draw: equation, coefficients, blend constant and whether color is
/// written.
// Port of: src/gpu/Blend.h#L83-L99 (chrome/m156)
#[doc(alias = "skgpu::BlendInfo")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlendInfo {
    pub equation: BlendEquation,
    pub src_blend: BlendCoeff,
    pub dst_blend: BlendCoeff,
    pub blend_constant: PMColor4f,
    pub writes_color: bool,
}

impl Default for BlendInfo {
    // Port of: src/gpu/Blend.h#L90-L95 (chrome/m156)
    /// The C++ default member initializers: `kAdd`, `kOne`, `kZero`, `SK_PMColor4fTRANSPARENT`,
    /// `true`.
    fn default() -> Self {
        BlendInfo {
            equation: BlendEquation::Add,
            src_blend: BlendCoeff::One,
            dst_blend: BlendCoeff::Zero,
            blend_constant: PMColor4f::default(),
            writes_color: true,
        }
    }
}

// Port of: src/gpu/Blend.h#L103-L106 (chrome/m156)
/// Does `coeff` reference the source color or alpha?
#[must_use]
pub const fn blend_coeff_refs_src(coeff: BlendCoeff) -> bool {
    matches!(
        coeff,
        BlendCoeff::SC | BlendCoeff::ISC | BlendCoeff::SA | BlendCoeff::ISA
    )
}

// Port of: src/gpu/Blend.h#L108-L111 (chrome/m156)
/// Does `coeff` reference the destination color or alpha?
#[must_use]
pub const fn blend_coeff_refs_dst(coeff: BlendCoeff) -> bool {
    matches!(
        coeff,
        BlendCoeff::DC | BlendCoeff::IDC | BlendCoeff::DA | BlendCoeff::IDA
    )
}

// Port of: src/gpu/Blend.h#L113-L116 (chrome/m156)
/// Does `coeff` reference the second source (dual-source blending)?
#[must_use]
pub const fn blend_coeff_refs_src2(coeff: BlendCoeff) -> bool {
    matches!(
        coeff,
        BlendCoeff::S2C | BlendCoeff::IS2C | BlendCoeff::S2A | BlendCoeff::IS2A
    )
}

// Port of: src/gpu/Blend.h#L118-L120 (chrome/m156)
/// Does the formula read the source color?
#[must_use]
pub const fn blend_coeffs_use_src_color(src_coeff: BlendCoeff, dst_coeff: BlendCoeff) -> bool {
    !matches!(src_coeff, BlendCoeff::Zero) || blend_coeff_refs_src(dst_coeff)
}

// Port of: src/gpu/Blend.h#L122-L127 (chrome/m156)
/// Does the formula read the destination color? A source with opaque color makes `ISA` a
/// no-op for the destination.
#[must_use]
#[allow(clippy::nonminimal_bool)] // mirrors the C++ expression term by term
pub const fn blend_coeffs_use_dst_color(
    src_coeff: BlendCoeff,
    dst_coeff: BlendCoeff,
    src_color_is_opaque: bool,
) -> bool {
    blend_coeff_refs_dst(src_coeff)
        || (!matches!(dst_coeff, BlendCoeff::Zero)
            && !(matches!(dst_coeff, BlendCoeff::ISA) && src_color_is_opaque))
}

// Port of: src/gpu/Blend.h#L129-L132 (chrome/m156)
/// Is `equation` one of the advanced (SVG/PDF) equations?
#[must_use]
pub const fn blend_equation_is_advanced(equation: BlendEquation) -> bool {
    // `equation >= kFirstAdvanced && equation != kIllegal`: the variants are ordered.
    (equation as u8) >= (BlendEquation::FIRST_ADVANCED as u8)
        && !matches!(equation, BlendEquation::Illegal)
}

// Port of: src/gpu/Blend.h#L134-L139 (chrome/m156)
/// Does the equation with these coefficients write the destination?
#[must_use]
pub const fn blend_modifies_dst(
    equation: BlendEquation,
    src_coeff: BlendCoeff,
    dst_coeff: BlendCoeff,
) -> bool {
    (!matches!(
        equation,
        BlendEquation::Add | BlendEquation::ReverseSubtract
    )) || !matches!(src_coeff, BlendCoeff::Zero)
        || !matches!(dst_coeff, BlendCoeff::One)
}

// Port of: src/gpu/Blend.h#L141-L143 (chrome/m156)
/// Does `coeff` reference the blend constant?
#[must_use]
pub const fn blend_coeff_refs_constant(coeff: BlendCoeff) -> bool {
    matches!(coeff, BlendCoeff::ConstC | BlendCoeff::IConstC)
}

// Port of: src/gpu/Blend.h#L145-L150 (chrome/m156)
/// Can blending be disabled altogether (`src + dst` with `One`/`Zero`)?
#[must_use]
pub const fn blend_should_disable(
    equation: BlendEquation,
    src_coeff: BlendCoeff,
    dst_coeff: BlendCoeff,
) -> bool {
    matches!(equation, BlendEquation::Add | BlendEquation::Subtract)
        && matches!(src_coeff, BlendCoeff::One)
        && matches!(dst_coeff, BlendCoeff::Zero)
}

// Port of: src/gpu/Blend.h#L179-L190 (chrome/m156)
/// May coverage be applied as the alpha of the source (instead of a separate multiply)?
#[must_use]
pub const fn blend_allows_coverage_as_alpha(
    equation: BlendEquation,
    src_coeff: BlendCoeff,
    dst_coeff: BlendCoeff,
) -> bool {
    blend_equation_is_advanced(equation)
        || !blend_modifies_dst(equation, src_coeff, dst_coeff)
        || ((matches!(
            equation,
            BlendEquation::Add | BlendEquation::ReverseSubtract
        )) && !blend_coeff_refs_src(src_coeff)
            && matches!(
                dst_coeff,
                BlendCoeff::One | BlendCoeff::ISC | BlendCoeff::ISA
            ))
}

// Port of: src/gpu/Blend.cpp#L18-L51 (chrome/m156)
/// `BlendFuncName(mode)`: the name of the shader function that implements `mode`.
#[must_use]
pub fn blend_func_name(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Clear => "blend_clear",
        BlendMode::Src => "blend_src",
        BlendMode::Dst => "blend_dst",
        BlendMode::SrcOver => "blend_src_over",
        BlendMode::DstOver => "blend_dst_over",
        BlendMode::SrcIn => "blend_src_in",
        BlendMode::DstIn => "blend_dst_in",
        BlendMode::SrcOut => "blend_src_out",
        BlendMode::DstOut => "blend_dst_out",
        BlendMode::SrcATop => "blend_src_atop",
        BlendMode::DstATop => "blend_dst_atop",
        BlendMode::Xor => "blend_xor",
        BlendMode::Plus => "blend_plus",
        BlendMode::Modulate => "blend_modulate",
        BlendMode::Screen => "blend_screen",
        BlendMode::Overlay => "blend_overlay",
        BlendMode::Darken => "blend_darken",
        BlendMode::Lighten => "blend_lighten",
        BlendMode::ColorDodge => "blend_color_dodge",
        BlendMode::ColorBurn => "blend_color_burn",
        BlendMode::HardLight => "blend_hard_light",
        BlendMode::SoftLight => "blend_soft_light",
        BlendMode::Difference => "blend_difference",
        BlendMode::Exclusion => "blend_exclusion",
        BlendMode::Multiply => "blend_multiply",
        BlendMode::Hue => "blend_hue",
        BlendMode::Saturation => "blend_saturation",
        BlendMode::Color => "blend_color",
        BlendMode::Luminosity => "blend_luminosity",
    }
}

// Port of: src/gpu/Blend.cpp#L53-L83 (chrome/m156)
/// `GetPorterDuffBlendConstants(mode)`: the constants the `blend_porter_duff` shader function
/// takes for the Porter-Duff modes; empty for every other mode.
#[must_use]
pub fn get_porter_duff_blend_constants(mode: BlendMode) -> &'static [f32] {
    // See sksl_gpu.sksl's blend_porter_duff function for explanation of values
    const K_CLEAR: &[f32] = &[0.0, 0.0, 0.0, 0.0];
    const K_SRC: &[f32] = &[1.0, 0.0, 0.0, 0.0];
    const K_DST: &[f32] = &[0.0, 1.0, 0.0, 0.0];
    const K_SRC_OVER: &[f32] = &[1.0, 1.0, 0.0, -1.0];
    const K_DST_OVER: &[f32] = &[1.0, 1.0, -1.0, 0.0];
    const K_SRC_IN: &[f32] = &[0.0, 0.0, 1.0, 0.0];
    const K_DST_IN: &[f32] = &[0.0, 0.0, 0.0, 1.0];
    const K_SRC_OUT: &[f32] = &[1.0, 0.0, -1.0, 0.0];
    const K_DST_OUT: &[f32] = &[0.0, 1.0, 0.0, -1.0];
    const K_SRC_ATOP: &[f32] = &[0.0, 1.0, 1.0, -1.0];
    const K_DST_ATOP: &[f32] = &[1.0, 0.0, -1.0, 1.0];
    const K_XOR: &[f32] = &[1.0, 1.0, -1.0, -1.0];
    match mode {
        BlendMode::Clear => K_CLEAR,
        BlendMode::Src => K_SRC,
        BlendMode::Dst => K_DST,
        BlendMode::SrcOver => K_SRC_OVER,
        BlendMode::DstOver => K_DST_OVER,
        BlendMode::SrcIn => K_SRC_IN,
        BlendMode::DstIn => K_DST_IN,
        BlendMode::SrcOut => K_SRC_OUT,
        BlendMode::DstOut => K_DST_OUT,
        BlendMode::SrcATop => K_SRC_ATOP,
        BlendMode::DstATop => K_DST_ATOP,
        BlendMode::Xor => K_XOR,
        _ => &[],
    }
}

/// The shader function for a blend mode, and the uniform data it takes.
// Port of: src/gpu/Blend.h#L206-L209 (chrome/m156)
#[doc(alias = "skgpu::ReducedBlendModeInfo")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReducedBlendModeInfo {
    /// `fFunction`: the name of the shader function.
    pub function: &'static str,
    /// `fUniformData`: the constants the function takes.
    pub uniform_data: &'static [f32],
}

// Port of: src/gpu/Blend.cpp#L85-L126 (chrome/m156)
/// `GetReducedBlendModeInfo(mode)`.
#[must_use]
pub fn get_reduced_blend_mode_info(mode: BlendMode) -> ReducedBlendModeInfo {
    const K_HUE: &[f32] = &[0.0, 1.0];
    const K_SATURATION: &[f32] = &[1.0, 1.0];
    const K_COLOR: &[f32] = &[0.0, 0.0];
    const K_LUMINOSITY: &[f32] = &[1.0, 0.0];
    const K_OVERLAY: &[f32] = &[0.0];
    const K_HARD_LIGHT: &[f32] = &[1.0];
    const K_DARKEN: &[f32] = &[1.0];
    const K_LIGHTEN: &[f32] = &[-1.0];
    const EMPTY: &[f32] = &[];

    // This switch must be kept in sync with BlendKey() in src/ganesh/glsl/GrGLSLBlend.cpp.
    // Clear/src/dst are intentionally omitted; using the built-in blend_xxxxx functions is
    // preferable, since that gives us an opportunity to eliminate the src/dst entirely.
    let (function, uniform_data) = match mode {
        BlendMode::SrcOver
        | BlendMode::DstOver
        | BlendMode::SrcIn
        | BlendMode::DstIn
        | BlendMode::SrcOut
        | BlendMode::DstOut
        | BlendMode::SrcATop
        | BlendMode::DstATop
        | BlendMode::Xor => ("blend_porter_duff", get_porter_duff_blend_constants(mode)),
        BlendMode::Hue => ("blend_hslc", K_HUE),
        BlendMode::Saturation => ("blend_hslc", K_SATURATION),
        BlendMode::Color => ("blend_hslc", K_COLOR),
        BlendMode::Luminosity => ("blend_hslc", K_LUMINOSITY),
        BlendMode::Overlay => ("blend_overlay", K_OVERLAY),
        BlendMode::HardLight => ("blend_overlay", K_HARD_LIGHT),
        BlendMode::Darken => ("blend_darken", K_DARKEN),
        BlendMode::Lighten => ("blend_darken", K_LIGHTEN),
        _ => (blend_func_name(mode), EMPTY),
    };
    ReducedBlendModeInfo {
        function,
        uniform_data,
    }
}
