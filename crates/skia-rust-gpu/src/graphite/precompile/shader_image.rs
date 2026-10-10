// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/precompile/PrecompileShader.h (image factories),
// src/gpu/graphite/precompile/PrecompileImageShader.h, and the image and YUV-image parts of
// src/gpu/graphite/precompile/PrecompileShader.cpp

//! The image, raw-image, YUV-image and picture precompile shaders.

use std::sync::Arc;

use bitflags::bitflags;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;

use crate::gpu::gpu_types::{Mipmapped, Protected, Renderable};
use crate::graphite::key_context::{KeyContext, KeyGenFlags};
use crate::graphite::key_helpers::{
    ImageData, ImageShaderBlock, RGBPaintColorBlock, YUVImageData, YUVImageShaderBlock,
};
use crate::graphite::key_helpers_ii::{
    ColorSpaceTransformBlock, ColorSpaceTransformData, add_fixed_blend_mode, blend, compose,
};
use crate::graphite::precompile::base::PrecompileBaseImpl;
use crate::graphite::precompile::shader::{PrecompileShader, PrecompileShaders, ShaderImpl};
use crate::graphite::resource_types::ImmutableSamplerInfo;
use crate::graphite::texture_format::read_swizzle_for_color_type;

use crate::graphite::texture_info::texture_info_priv;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::image_info_priv::color_type_is_alpha_only;
use skia_rust_core::size::ISize;

bitflags! {
    /// `PrecompileShaders::ImageShaderFlags`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct ImageShaderFlags: u16 {
        /// `kNone`.
        const NONE = 0;
        /// `kCubicSampling`.
        const CUBIC_SAMPLING = 1 << 1;
        /// `kIncludeAlphaOnly`.
        const INCLUDE_ALPHA_ONLY = 1 << 2;
        /// `kAll`.
        const ALL = Self::CUBIC_SAMPLING.bits() | Self::INCLUDE_ALPHA_ONLY.bits();
        /// `kExcludeCubic`.
        const EXCLUDE_CUBIC = Self::INCLUDE_ALPHA_ONLY.bits();
        /// `kNoAlphaNoCubic`.
        const NO_ALPHA_NO_CUBIC = Self::NONE.bits();
    }
}

bitflags! {
    /// `PrecompileShaders::YUVImageShaderFlags`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct YUVImageShaderFlags: u16 {
        /// `kNone`.
        const NONE = 0;
        /// `kHardwareSamplingNoSwizzle`.
        const HARDWARE_SAMPLING_NO_SWIZZLE = 1 << 1;
        /// `kHardwareSampling`.
        const HARDWARE_SAMPLING = 1 << 2;
        /// `kShaderBasedSampling`.
        const SHADER_BASED_SAMPLING = 1 << 3;
        /// `kCubicSampling`.
        const CUBIC_SAMPLING = 1 << 4;
        /// `kExcludeCubic`.
        const EXCLUDE_CUBIC = Self::HARDWARE_SAMPLING_NO_SWIZZLE.bits()
            | Self::HARDWARE_SAMPLING.bits()
            | Self::SHADER_BASED_SAMPLING.bits();
        /// `kNoCubicNoNonSwizzledHW`.
        const NO_CUBIC_NO_NON_SWIZZLED_HW = Self::HARDWARE_SAMPLING_NO_SWIZZLE.bits()
            | Self::SHADER_BASED_SAMPLING.bits();
    }
}

/// `kAllTileModes`: the tile modes a default image shader covers.
pub const ALL_TILE_MODES: [TileMode; 4] = [
    TileMode::Clamp,
    TileMode::Repeat,
    TileMode::Mirror,
    TileMode::Decal,
];

/// `PrecompileImageShader::kExtraNumSamplingTilingCombos`.
const EXTRA_NUM_SAMPLING_TILING_COMBOS: i32 = 2;
/// `PrecompileImageShader::kCubicSampled`.
const CUBIC_SAMPLED: i32 = 1;
/// `PrecompileImageShader::kHWTiled`.
const HW_TILED: i32 = 0;

/// `SkSamplingOptions(SkCubicResampler::Mitchell())`.
fn cubic_sampling() -> SamplingOptions {
    SamplingOptions {
        use_cubic: true,
        cubic: CubicResampler::mitchell(),
        ..SamplingOptions::default()
    }
}

/// `SkSamplingOptions(SkFilterMode::kLinear)`.
fn linear_sampling() -> SamplingOptions {
    SamplingOptions {
        filter: FilterMode::Linear,
        ..SamplingOptions::default()
    }
}

/// The red channel of a YUV plane's channel selector.
const RED_CHANNEL: [f32; 4] = [1.0, 0.0, 0.0, 0.0];

/// `PrecompileImageShader::DefaultColorInfoPremul()`.
fn default_color_info_premul() -> ColorInfo {
    ColorInfo::new(
        ColorType::RGBA8888,
        AlphaType::Premul,
        ColorSpace::new_srgb(),
    )
}

/// `PrecompileImageShader::DefaultColorInfoSRGB()`.
fn default_color_info_srgb() -> ColorInfo {
    ColorInfo::new(
        ColorType::RGBA8888,
        AlphaType::Premul,
        skia_rust_core::color_space_priv::srgb_singleton().with_color_spin(),
    )
}

/// `PrecompileImageShader::DefaultColorInfoGeneral()`.
fn default_color_info_general() -> ColorInfo {
    ColorInfo::new(
        ColorType::RGBA8888,
        AlphaType::Premul,
        ColorSpace::new_srgb_linear(),
    )
}

/// `PrecompileImageShader::DefaultColorInfoAlphaOnly()`.
fn default_color_info_alpha_only() -> ColorInfo {
    ColorInfo::new(
        ColorType::Alpha8,
        AlphaType::Premul,
        ColorSpace::new_srgb_linear(),
    )
}

/// `PrecompileImageShader::DefaultColorInfos()`.
fn default_color_infos() -> Vec<ColorInfo> {
    vec![
        default_color_info_premul(),
        default_color_info_srgb(),
        default_color_info_general(),
        default_color_info_alpha_only(),
    ]
}

/// `PrecompileImageShader::NonAlphaOnlyDefaultColorInfos()`.
fn non_alpha_only_default_color_infos() -> Vec<ColorInfo> {
    vec![
        default_color_info_premul(),
        default_color_info_srgb(),
        default_color_info_general(),
    ]
}

/// `PrecompileImageShader::RawImageDefaultColorInfos()`.
fn raw_image_default_color_infos() -> Vec<ColorInfo> {
    vec![default_color_info_premul(), default_color_info_alpha_only()]
}

/// `PrecompileImageShader`.
// Port of: src/gpu/graphite/precompile/PrecompileImageShader.h#L17-L70 (chrome/m156)
pub(crate) struct ImageShader {
    num_extra_sampling_tiling_combos: i32,
    color_infos: Vec<ColorInfo>,
    tile_modes: Vec<TileMode>,
    use_dst_color_info: bool,
    raw: bool,
    immutable_sampler_info: ImmutableSamplerInfo,
}

impl ImageShader {
    // Port of: PrecompileImageShader::PrecompileImageShader (chrome/m156)
    fn new(
        flags: ImageShaderFlags,
        color_infos: &[ColorInfo],
        tile_modes: &[TileMode],
        raw: bool,
        immutable_sampler_info: ImmutableSamplerInfo,
    ) -> Self {
        Self {
            num_extra_sampling_tiling_combos: if flags.contains(ImageShaderFlags::CUBIC_SAMPLING) {
                EXTRA_NUM_SAMPLING_TILING_COMBOS
            } else {
                1 // Just kHWTiled
            },
            color_infos: if !color_infos.is_empty() {
                color_infos.to_vec()
            } else if raw {
                raw_image_default_color_infos()
            } else if flags.contains(ImageShaderFlags::INCLUDE_ALPHA_ONLY) {
                default_color_infos()
            } else {
                non_alpha_only_default_color_infos()
            },
            tile_modes: tile_modes.to_vec(),
            use_dst_color_info: !color_infos.is_empty(),
            raw,
            immutable_sampler_info,
        }
    }

    fn num_sampling_tiling_combos(&self) -> i32 {
        i32::try_from(self.tile_modes.len()).unwrap_or(i32::MAX)
            + self.num_extra_sampling_tiling_combos
    }
}

impl PrecompileBaseImpl for ImageShader {
    // Port of: PrecompileImageShader::numIntrinsicCombinations (chrome/m156)
    fn num_intrinsic_combinations(&self) -> i32 {
        i32::try_from(self.color_infos.len()).unwrap_or(i32::MAX)
            * self.num_sampling_tiling_combos()
    }

    // Port of: PrecompileImageShader::addToKey (chrome/m156)
    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        debug_assert!(desired_combination < self.num_intrinsic_combinations());
        let num_sampling_tiling_combos = self.num_sampling_tiling_combos();
        let desired_sampling_tiling_combo = desired_combination % num_sampling_tiling_combos;
        let desired_color_info =
            usize::try_from(desired_combination / num_sampling_tiling_combos).unwrap_or(0);
        let desired_sampling_tiling_combo =
            usize::try_from(desired_sampling_tiling_combo).unwrap_or(0);
        let num_tile_modes = self.tile_modes.len();
        let tile_mode = if desired_sampling_tiling_combo < num_tile_modes {
            self.tile_modes[desired_sampling_tiling_combo]
        } else {
            TileMode::Clamp
        };
        // `desiredSamplingTilingCombo - numTileModes` for the non-tile-mode combinations.
        let extra_index = if desired_sampling_tiling_combo >= num_tile_modes {
            i32::try_from(desired_sampling_tiling_combo - num_tile_modes).unwrap_or(-1)
        } else {
            -1
        };
        let img_size = if extra_index == HW_TILED {
            ISize::new(1, 1) // kHWTileableSize
        } else {
            ISize::new(2, 2) // kShaderTileableSize
        };
        let sampling = if extra_index == CUBIC_SAMPLED {
            cubic_sampling()
        } else {
            linear_sampling()
        };
        let subset = Rect::from_xywh(0.0, 0.0, 1.0, 1.0); // kSubset = SkRect::MakeWH(1, 1)
        let img_data = ImageData::new(
            sampling,
            tile_mode,
            tile_mode,
            img_size,
            subset,
            self.immutable_sampler_info,
        );

        let color_info = &self.color_infos[desired_color_info];
        let alpha_only = color_type_is_alpha_only(color_info.color_type());
        let caps = key_context.caps();
        let format = texture_info_priv::view_format(&caps.get_default_sampled_texture_info(
            color_info.color_type(),
            Mipmapped::No,
            Protected::No,
            Renderable::No,
        ));
        let read_swizzle = read_swizzle_for_color_type(color_info.color_type(), format);
        let mut color_xform_data = ColorSpaceTransformData::from_swizzle(read_swizzle);
        color_xform_data.is_alpha_only = alpha_only;

        if !self.raw {
            let mut dst_color_space = skia_rust_core::color_space_priv::srgb_singleton().clone();
            let mut dst_at = color_info.alpha_type();
            if self.use_dst_color_info {
                let dst_info = key_context.dst_color_info();
                dst_color_space = dst_info
                    .color_space_ref()
                    .cloned()
                    .unwrap_or_else(|| skia_rust_core::color_space_priv::srgb_singleton().clone());
                dst_at = dst_info.alpha_type();
            }
            color_xform_data.steps = ColorSpaceXformSteps::new(
                color_info.color_space_ref(),
                color_info.alpha_type(),
                Some(&dst_color_space),
                dst_at,
            );
            if alpha_only
                && !key_context
                    .flags()
                    .contains(KeyGenFlags::DISABLE_ALPHA_ONLY_IMAGE_COLORIZATION)
            {
                blend(
                    key_context,
                    // addBlendToKey
                    || add_fixed_blend_mode(key_context, BlendMode::DstIn),
                    // addSrcToKey
                    || {
                        compose(
                            key_context,
                            || ImageShaderBlock::add_block(key_context, &img_data),
                            || ColorSpaceTransformBlock::add_block(key_context, &color_xform_data),
                        );
                    },
                    // addDstToKey
                    || RGBPaintColorBlock::add_block(key_context),
                );
                return;
            }
        }

        compose(
            key_context,
            || ImageShaderBlock::add_block(key_context, &img_data),
            || ColorSpaceTransformBlock::add_block(key_context, &color_xform_data),
        );
    }
}

impl ShaderImpl for ImageShader {
    // Port of: PrecompileImageShader::isOpaque (chrome/m156)
    fn is_opaque(&self, desired_combination: i32) -> bool {
        debug_assert!(desired_combination < self.num_intrinsic_combinations());
        let desired_color_info =
            usize::try_from(desired_combination / self.num_sampling_tiling_combos()).unwrap_or(0);
        self.color_infos[desired_color_info].is_opaque()
    }
}

/// `PrecompileYUVImageShader`.
// Port of: src/gpu/graphite/precompile/PrecompileShader.cpp#L285-L420 (chrome/m156)
pub(crate) struct YuvImageShader {
    color_infos: Vec<ColorInfo>,
    use_dst_color_space: bool,
    tiling_modes: Vec<i32>,
}

const K_SHADER_TILED: i32 = 0;
const K_HW_TILED_NO_SWIZZLE: i32 = 1;
const K_HW_TILED_WITH_SWIZZLE: i32 = 2;
const K_CUBIC_SHADER_TILED: i32 = 3;

impl YuvImageShader {
    // Port of: PrecompileYUVImageShader::PrecompileYUVImageShader + setupTilingModes (chrome/m156)
    fn new(flags: YUVImageShaderFlags, color_infos: &[ColorInfo]) -> Self {
        let mut tiling_modes = Vec::new();
        if flags.contains(YUVImageShaderFlags::HARDWARE_SAMPLING_NO_SWIZZLE) {
            tiling_modes.push(K_HW_TILED_NO_SWIZZLE);
        }
        if flags.contains(YUVImageShaderFlags::HARDWARE_SAMPLING) {
            tiling_modes.push(K_HW_TILED_WITH_SWIZZLE);
        }
        if flags.contains(YUVImageShaderFlags::SHADER_BASED_SAMPLING) {
            tiling_modes.push(K_SHADER_TILED);
        }
        if flags.contains(YUVImageShaderFlags::CUBIC_SAMPLING) {
            tiling_modes.push(K_CUBIC_SHADER_TILED);
        }
        debug_assert_eq!(tiling_modes.len(), flags.bits().count_ones() as usize);
        Self {
            color_infos: if color_infos.is_empty() {
                non_alpha_only_default_color_infos()
            } else {
                color_infos.to_vec()
            },
            use_dst_color_space: !color_infos.is_empty(),
            tiling_modes,
        }
    }
}

impl PrecompileBaseImpl for YuvImageShader {
    fn num_intrinsic_combinations(&self) -> i32 {
        i32::try_from(self.tiling_modes.len()).unwrap_or(i32::MAX)
            * i32::try_from(self.color_infos.len()).unwrap_or(i32::MAX)
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        let num_tiling = self.tiling_modes.len();
        let desired_tiling = usize::try_from(desired_combination).unwrap_or(0) % num_tiling;
        let desired_color_info = usize::try_from(desired_combination).unwrap_or(0) / num_tiling;
        let tiling = self.tiling_modes[desired_tiling];

        let sampling = if tiling == K_CUBIC_SHADER_TILED {
            cubic_sampling()
        } else {
            SamplingOptions::default()
        };
        let mut img_data = YUVImageData::new(
            sampling,
            TileMode::Clamp,
            if tiling == K_SHADER_TILED {
                TileMode::Repeat
            } else {
                TileMode::Clamp
            },
            ISize::new(1, 1),
            if tiling == K_SHADER_TILED {
                Rect::from_xywh(0.0, 0.0, 0.0, 0.0)
            } else {
                Rect::from_xywh(0.0, 0.0, 1.0, 1.0)
            },
        );
        img_data.channel_select[0] = RED_CHANNEL;
        img_data.channel_select[1] = RED_CHANNEL;
        if tiling == K_HW_TILED_NO_SWIZZLE {
            img_data.channel_select[2] = RED_CHANNEL;
        } else {
            img_data.channel_select[2] = [0.0, 1.0, 0.0, 0.0];
        }
        img_data.channel_select[3] = RED_CHANNEL;
        img_data.yuv_to_rgb_matrix = Matrix::new_all(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0);
        img_data.yuv_to_rgb_translate = [0.0, 0.0, 0.0];

        let color_info = &self.color_infos[desired_color_info];
        let caps = key_context.caps();
        let format = texture_info_priv::view_format(&caps.get_default_sampled_texture_info(
            color_info.color_type(),
            Mipmapped::No,
            Protected::No,
            Renderable::No,
        ));
        let read_swizzle = read_swizzle_for_color_type(color_info.color_type(), format);
        let mut color_xform_data = ColorSpaceTransformData::from_swizzle(read_swizzle);
        let dst_color_space = if self.use_dst_color_space {
            key_context.dst_color_info().color_space_ref().cloned()
        } else {
            Some(skia_rust_core::color_space_priv::srgb_singleton().clone())
        };
        color_xform_data.steps = ColorSpaceXformSteps::new(
            color_info.color_space_ref(),
            color_info.alpha_type(),
            dst_color_space.as_ref(),
            color_info.alpha_type(),
        );
        compose(
            key_context,
            || YUVImageShaderBlock::add_block(key_context, &img_data),
            || ColorSpaceTransformBlock::add_block(key_context, &color_xform_data),
        );
    }
}

impl ShaderImpl for YuvImageShader {
    fn is_opaque(&self, desired_combination: i32) -> bool {
        let desired_color_info =
            usize::try_from(desired_combination).unwrap_or(0) / self.tiling_modes.len();
        self.color_infos[desired_color_info].is_opaque()
    }
}

/// `PrecompileShaders::Image(shaderFlags, colorInfos, tileModes)`.
#[must_use]
pub(crate) fn image_shader_node(
    flags: ImageShaderFlags,
    color_infos: &[ColorInfo],
    tile_modes: &[TileMode],
    raw: bool,
    immutable_sampler_info: ImmutableSamplerInfo,
) -> PrecompileShader {
    PrecompileShader::from_imp(Arc::new(ImageShader::new(
        flags,
        color_infos,
        tile_modes,
        raw,
        immutable_sampler_info,
    )))
}

impl PrecompileShaders {
    /// `PrecompileShaders::Image(shaderFlags, colorInfos, tileModes)`.
    #[must_use]
    pub fn image(
        flags: ImageShaderFlags,
        color_infos: &[ColorInfo],
        tile_modes: &[TileMode],
    ) -> PrecompileShader {
        Self::image_with_sampler(
            flags,
            color_infos,
            tile_modes,
            ImmutableSamplerInfo::default(),
        )
    }

    /// `PrecompileShaders::Image` with an immutable sampler (`setImmutableSamplerInfo`).
    #[must_use]
    pub fn image_with_sampler(
        flags: ImageShaderFlags,
        color_infos: &[ColorInfo],
        tile_modes: &[TileMode],
        immutable_sampler_info: ImmutableSamplerInfo,
    ) -> PrecompileShader {
        Self::local_matrix(
            &[image_shader_node(
                flags,
                color_infos,
                tile_modes,
                false,
                immutable_sampler_info,
            )],
            false,
        )
    }

    /// `PrecompileShaders::RawImage(shaderFlags, colorInfos, tileModes)`.
    #[must_use]
    pub fn raw_image(
        flags: ImageShaderFlags,
        color_infos: &[ColorInfo],
        tile_modes: &[TileMode],
    ) -> PrecompileShader {
        let new_flags = flags & !ImageShaderFlags::CUBIC_SAMPLING;
        Self::local_matrix(
            &[image_shader_node(
                new_flags,
                color_infos,
                tile_modes,
                true,
                ImmutableSamplerInfo::default(),
            )],
            false,
        )
    }

    /// `PrecompileShaders::YUVImage(shaderFlags, colorInfos)`.
    #[must_use]
    pub fn yuv_image(flags: YUVImageShaderFlags, color_infos: &[ColorInfo]) -> PrecompileShader {
        Self::local_matrix(
            &[PrecompileShader::from_imp(Arc::new(YuvImageShader::new(
                flags,
                color_infos,
            )))],
            false,
        )
    }

    /// `PrecompileShadersPriv::Picture(withLM)`: the image shader, with a local matrix when
    /// `with_lm`.
    // Port of: src/gpu/graphite/precompile/PrecompileShader.cpp#L580-L593 (chrome/m156)
    #[must_use]
    #[doc(hidden)]
    pub fn picture_priv(with_lm: bool) -> PrecompileShader {
        let s = Self::image(ImageShaderFlags::ALL, &[], &ALL_TILE_MODES);
        if with_lm {
            return Self::local_matrix(&[s], false);
        }
        s
    }

    /// `PrecompileShaders::Picture()`: the picture shader, with and without a local matrix.
    #[must_use]
    pub fn picture() -> PrecompileShader {
        Self::local_matrix_both_variants(&[Self::image(
            ImageShaderFlags::ALL,
            &[],
            &ALL_TILE_MODES,
        )])
    }
}
