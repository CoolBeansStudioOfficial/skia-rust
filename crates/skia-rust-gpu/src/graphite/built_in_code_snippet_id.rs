// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/BuiltInCodeSnippetID.h

//! [`BuiltInCodeSnippetID`]: the ids of the code snippets Graphite builds into every
//! `ShaderCodeDictionary`.
//!
//! The variants are in the order of Skia's enum: the ids appear in serialized `PaintParamsKey`s.
//! The C++ names with an underscore (`kCSXform_sRGB`, `kFixedBlend_Clear`) lose it here.

/// The ids of the built-in code snippets (`BuiltInCodeSnippetID`).
///
/// This isn't just a signal for a failure during paintparams key creation: `Error` also
/// implements the default behavior for an erroneous draw. The fixed blend ids are contiguous,
/// last, and ordered like `BlendMode`, so `id - FIRST_FIXED_BLEND` is the blend mode.
// Port of: src/gpu/graphite/BuiltInCodeSnippetID.h#L15-L194 (chrome/m156)
#[doc(alias = "skgpu::graphite::BuiltInCodeSnippetID")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u32)]
pub enum BuiltInCodeSnippetID {
    #[doc(alias = "kError")]
    Error,
    #[doc(alias = "kPriorOutput")]
    PriorOutput,
    #[doc(alias = "kSolidColorShader")]
    SolidColorShader,
    #[doc(alias = "kRGBPaintColor")]
    RGBPaintColor,
    #[doc(alias = "kAlphaOnlyPaintColor")]
    AlphaOnlyPaintColor,
    #[doc(alias = "kLinearGradientShader4")]
    LinearGradientShader4,
    #[doc(alias = "kLinearGradientShader8")]
    LinearGradientShader8,
    #[doc(alias = "kLinearGradientShaderTexture")]
    LinearGradientShaderTexture,
    #[doc(alias = "kLinearGradientShaderBuffer")]
    LinearGradientShaderBuffer,
    #[doc(alias = "kRadialGradientShader4")]
    RadialGradientShader4,
    #[doc(alias = "kRadialGradientShader8")]
    RadialGradientShader8,
    #[doc(alias = "kRadialGradientShaderTexture")]
    RadialGradientShaderTexture,
    #[doc(alias = "kRadialGradientShaderBuffer")]
    RadialGradientShaderBuffer,
    #[doc(alias = "kSweepGradientShader4")]
    SweepGradientShader4,
    #[doc(alias = "kSweepGradientShader8")]
    SweepGradientShader8,
    #[doc(alias = "kSweepGradientShaderTexture")]
    SweepGradientShaderTexture,
    #[doc(alias = "kSweepGradientShaderBuffer")]
    SweepGradientShaderBuffer,
    #[doc(alias = "kConicalGradientShader4")]
    ConicalGradientShader4,
    #[doc(alias = "kConicalGradientShader8")]
    ConicalGradientShader8,
    #[doc(alias = "kConicalGradientShaderTexture")]
    ConicalGradientShaderTexture,
    #[doc(alias = "kConicalGradientShaderBuffer")]
    ConicalGradientShaderBuffer,
    #[doc(alias = "kLocalMatrixShader")]
    LocalMatrixShader,
    #[doc(alias = "kLocalMatrixShaderPersp")]
    LocalMatrixShaderPersp,
    #[doc(alias = "kImageShader")]
    ImageShader,
    #[doc(alias = "kImageShaderClamp")]
    ImageShaderClamp,
    #[doc(alias = "kCubicImageShader")]
    CubicImageShader,
    #[doc(alias = "kHWImageShader")]
    HWImageShader,
    #[doc(alias = "kYUVImageShader")]
    YUVImageShader,
    #[doc(alias = "kCubicYUVImageShader")]
    CubicYUVImageShader,
    #[doc(alias = "kHWYUVImageShader")]
    HWYUVImageShader,
    #[doc(alias = "kHWYUVNoSwizzleImageShader")]
    HWYUVNoSwizzleImageShader,
    #[doc(alias = "kCoordNormalizeShader")]
    CoordNormalizeShader,
    #[doc(alias = "kCoordClampShader")]
    CoordClampShader,
    #[doc(alias = "kDitherShader")]
    DitherShader,
    #[doc(alias = "kPerlinNoiseShader")]
    PerlinNoiseShader,
    #[doc(alias = "kMatrixColorFilter")]
    MatrixColorFilter,
    #[doc(alias = "kHSLMatrixColorFilter")]
    HSLMatrixColorFilter,
    #[doc(alias = "kTableColorFilter")]
    TableColorFilter,
    #[doc(alias = "kGaussianColorFilter")]
    GaussianColorFilter,
    #[doc(alias = "kCSXform_AlphaOnly")]
    CSXformAlphaOnly,
    #[doc(alias = "kCSXform_PreAlpha")]
    CSXformPreAlpha,
    #[doc(alias = "kCSXform_Unpremul")]
    CSXformUnpremul,
    #[doc(alias = "kCSXform_ForceOpaque")]
    CSXformForceOpaque,
    #[doc(alias = "kCSXform_sRGB")]
    CSXformsRGB,
    #[doc(alias = "kCSXform_PQ")]
    CSXformPQ,
    #[doc(alias = "kCSXform_HLG")]
    CSXformHLG,
    #[doc(alias = "kCSXform_HLGInv")]
    CSXformHLGInv,
    #[doc(alias = "kCSXform_Gamut")]
    CSXformGamut,
    #[doc(alias = "kCSXform_PostAlpha")]
    CSXformPostAlpha,
    #[doc(alias = "kCSXform_Premul")]
    CSXformPremul,
    #[doc(alias = "kPrimitiveColor")]
    PrimitiveColor,
    #[doc(alias = "kAnalyticClip")]
    AnalyticClip,
    #[doc(alias = "kAnalyticAndAtlasClip")]
    AnalyticAndAtlasClip,
    #[doc(alias = "kCompose")]
    Compose,
    #[doc(alias = "kBlendCompose")]
    BlendCompose,
    #[doc(alias = "kPorterDuffBlender")]
    PorterDuffBlender,
    #[doc(alias = "kHSLCBlender")]
    HSLCBlender,
    #[doc(alias = "kFixedBlend_Clear")]
    FixedBlendClear,
    #[doc(alias = "kFixedBlend_Src")]
    FixedBlendSrc,
    #[doc(alias = "kFixedBlend_Dst")]
    FixedBlendDst,
    #[doc(alias = "kFixedBlend_SrcOver")]
    FixedBlendSrcOver,
    #[doc(alias = "kFixedBlend_DstOver")]
    FixedBlendDstOver,
    #[doc(alias = "kFixedBlend_SrcIn")]
    FixedBlendSrcIn,
    #[doc(alias = "kFixedBlend_DstIn")]
    FixedBlendDstIn,
    #[doc(alias = "kFixedBlend_SrcOut")]
    FixedBlendSrcOut,
    #[doc(alias = "kFixedBlend_DstOut")]
    FixedBlendDstOut,
    #[doc(alias = "kFixedBlend_SrcATop")]
    FixedBlendSrcATop,
    #[doc(alias = "kFixedBlend_DstATop")]
    FixedBlendDstATop,
    #[doc(alias = "kFixedBlend_Xor")]
    FixedBlendXor,
    #[doc(alias = "kFixedBlend_Plus")]
    FixedBlendPlus,
    #[doc(alias = "kFixedBlend_Modulate")]
    FixedBlendModulate,
    #[doc(alias = "kFixedBlend_Screen")]
    FixedBlendScreen,
    #[doc(alias = "kFixedBlend_Overlay")]
    FixedBlendOverlay,
    #[doc(alias = "kFixedBlend_Darken")]
    FixedBlendDarken,
    #[doc(alias = "kFixedBlend_Lighten")]
    FixedBlendLighten,
    #[doc(alias = "kFixedBlend_ColorDodge")]
    FixedBlendColorDodge,
    #[doc(alias = "kFixedBlend_ColorBurn")]
    FixedBlendColorBurn,
    #[doc(alias = "kFixedBlend_HardLight")]
    FixedBlendHardLight,
    #[doc(alias = "kFixedBlend_SoftLight")]
    FixedBlendSoftLight,
    #[doc(alias = "kFixedBlend_Difference")]
    FixedBlendDifference,
    #[doc(alias = "kFixedBlend_Exclusion")]
    FixedBlendExclusion,
    #[doc(alias = "kFixedBlend_Multiply")]
    FixedBlendMultiply,
    #[doc(alias = "kFixedBlend_Hue")]
    FixedBlendHue,
    #[doc(alias = "kFixedBlend_Saturation")]
    FixedBlendSaturation,
    #[doc(alias = "kFixedBlend_Color")]
    FixedBlendColor,
    #[doc(alias = "kFixedBlend_Luminosity")]
    FixedBlendLuminosity,
}

impl BuiltInCodeSnippetID {
    /// `kFirstFixedBlend`.
    #[doc(alias = "kFirstFixedBlend")]
    pub const FIRST_FIXED_BLEND: Self = Self::FixedBlendClear;
    /// `kLast`.
    #[doc(alias = "kLast")]
    pub const LAST: Self = Self::FixedBlendLuminosity;

    /// The id with the number `id`, if it is a built-in one.
    #[must_use]
    pub const fn from_u32(id: u32) -> Option<Self> {
        Some(match id {
            0 => Self::Error,
            1 => Self::PriorOutput,
            2 => Self::SolidColorShader,
            3 => Self::RGBPaintColor,
            4 => Self::AlphaOnlyPaintColor,
            5 => Self::LinearGradientShader4,
            6 => Self::LinearGradientShader8,
            7 => Self::LinearGradientShaderTexture,
            8 => Self::LinearGradientShaderBuffer,
            9 => Self::RadialGradientShader4,
            10 => Self::RadialGradientShader8,
            11 => Self::RadialGradientShaderTexture,
            12 => Self::RadialGradientShaderBuffer,
            13 => Self::SweepGradientShader4,
            14 => Self::SweepGradientShader8,
            15 => Self::SweepGradientShaderTexture,
            16 => Self::SweepGradientShaderBuffer,
            17 => Self::ConicalGradientShader4,
            18 => Self::ConicalGradientShader8,
            19 => Self::ConicalGradientShaderTexture,
            20 => Self::ConicalGradientShaderBuffer,
            21 => Self::LocalMatrixShader,
            22 => Self::LocalMatrixShaderPersp,
            23 => Self::ImageShader,
            24 => Self::ImageShaderClamp,
            25 => Self::CubicImageShader,
            26 => Self::HWImageShader,
            27 => Self::YUVImageShader,
            28 => Self::CubicYUVImageShader,
            29 => Self::HWYUVImageShader,
            30 => Self::HWYUVNoSwizzleImageShader,
            31 => Self::CoordNormalizeShader,
            32 => Self::CoordClampShader,
            33 => Self::DitherShader,
            34 => Self::PerlinNoiseShader,
            35 => Self::MatrixColorFilter,
            36 => Self::HSLMatrixColorFilter,
            37 => Self::TableColorFilter,
            38 => Self::GaussianColorFilter,
            39 => Self::CSXformAlphaOnly,
            40 => Self::CSXformPreAlpha,
            41 => Self::CSXformUnpremul,
            42 => Self::CSXformForceOpaque,
            43 => Self::CSXformsRGB,
            44 => Self::CSXformPQ,
            45 => Self::CSXformHLG,
            46 => Self::CSXformHLGInv,
            47 => Self::CSXformGamut,
            48 => Self::CSXformPostAlpha,
            49 => Self::CSXformPremul,
            50 => Self::PrimitiveColor,
            51 => Self::AnalyticClip,
            52 => Self::AnalyticAndAtlasClip,
            53 => Self::Compose,
            54 => Self::BlendCompose,
            55 => Self::PorterDuffBlender,
            56 => Self::HSLCBlender,
            57 => Self::FixedBlendClear,
            58 => Self::FixedBlendSrc,
            59 => Self::FixedBlendDst,
            60 => Self::FixedBlendSrcOver,
            61 => Self::FixedBlendDstOver,
            62 => Self::FixedBlendSrcIn,
            63 => Self::FixedBlendDstIn,
            64 => Self::FixedBlendSrcOut,
            65 => Self::FixedBlendDstOut,
            66 => Self::FixedBlendSrcATop,
            67 => Self::FixedBlendDstATop,
            68 => Self::FixedBlendXor,
            69 => Self::FixedBlendPlus,
            70 => Self::FixedBlendModulate,
            71 => Self::FixedBlendScreen,
            72 => Self::FixedBlendOverlay,
            73 => Self::FixedBlendDarken,
            74 => Self::FixedBlendLighten,
            75 => Self::FixedBlendColorDodge,
            76 => Self::FixedBlendColorBurn,
            77 => Self::FixedBlendHardLight,
            78 => Self::FixedBlendSoftLight,
            79 => Self::FixedBlendDifference,
            80 => Self::FixedBlendExclusion,
            81 => Self::FixedBlendMultiply,
            82 => Self::FixedBlendHue,
            83 => Self::FixedBlendSaturation,
            84 => Self::FixedBlendColor,
            85 => Self::FixedBlendLuminosity,
            _ => return None,
        })
    }
}

impl From<BuiltInCodeSnippetID> for u32 {
    fn from(id: BuiltInCodeSnippetID) -> u32 {
        id as u32
    }
}

/// `kBuiltInCodeSnippetIDCount`.
// Port of: src/gpu/graphite/BuiltInCodeSnippetID.h#L195 (chrome/m156)
#[doc(alias = "kBuiltInCodeSnippetIDCount")]
pub const BUILT_IN_CODE_SNIPPET_ID_COUNT: i32 = BuiltInCodeSnippetID::LAST as i32 + 1;

/// `kFixedBlendIDOffset`.
// Port of: src/gpu/graphite/BuiltInCodeSnippetID.h#L196-L197 (chrome/m156)
#[doc(alias = "kFixedBlendIDOffset")]
pub const FIXED_BLEND_ID_OFFSET: i32 = BuiltInCodeSnippetID::FIRST_FIXED_BLEND as i32;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_u32_round_trips() {
        for id in 0..BUILT_IN_CODE_SNIPPET_ID_COUNT as u32 {
            assert_eq!(
                BuiltInCodeSnippetID::from_u32(id).map(|i| i as u32),
                Some(id)
            );
        }
        assert_eq!(
            BuiltInCodeSnippetID::from_u32(BUILT_IN_CODE_SNIPPET_ID_COUNT as u32),
            None
        );
    }

    #[test]
    fn counts() {
        assert_eq!(BUILT_IN_CODE_SNIPPET_ID_COUNT, 86);
        assert_eq!(FIXED_BLEND_ID_OFFSET, 57);
    }
}
