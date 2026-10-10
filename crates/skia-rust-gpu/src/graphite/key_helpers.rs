// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/KeyHelpers.{h,cpp} (lines 1-1500 of the .cpp: the shader
// blocks, and the core-shader half of `AddToKey`)

//! `KeyHelpers` part I: the blocks that build a [`PaintParamsKey`] for solid colors, gradients,
//! images, YUV images, coordinate clamping and normalization, dithering, perlin noise and local
//! matrices, together with the uniform data each block gathers.
//!
//! Every block writes its uniforms inside a `ScopedUniformWriter`, which opens the snippet's
//! uniform struct (if it has one) and, in debug builds, checks the written uniforms against the
//! snippet's declared uniform list. The packing goes through the [`UniformManager`] of the
//! [`PipelineDataGatherer`], so the bytes follow the layout the gatherer was created with.
//!
//! Textures are bound through the gatherer (`PipelineDataGatherer::add`) by the image, YUV,
//! dither, perlin noise, table color filter and gradient blocks. The proxies are the caller's:
//! without one (the pre-compile path) the binding is a `None` proxy with its sampler.
//!
//! Not ported yet:
//!
//! - gradients with more than 8 stops in a storage buffer (`StorageContext`, the `use_storage_buffer`
//!   path). Those add an error block; the color-and-offset texture path is ported.
//! - the image shader's `AddToKey` case is ported (`add_image_to_key`, G10d). The YUV image case
//!   (`add_yuv_image_to_key`, G15) and the picture shader (`Surface::Make` for its tile, G15) add an
//!   error block.
//! - the runtime effect shader `AddToKey` case. Its block (`RuntimeEffectBlock`) is in
//!   `key_helpers_ii`, but this shader dispatch does not route to it yet. It adds an error block.

use std::any::Any;
use std::cell::RefMut;
use std::sync::{Arc, LazyLock};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::{Color4f, PMColor4f};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_space_priv::srgb_singleton;
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::floating_point::ieee_float_divide;
use skia_rust_core::image::Image;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::raster_pipeline::contexts::PerlinNoiseShaderType;
use skia_rust_core::rect::{Contains, Rect, RoundOut};
use skia_rust_core::runtime_effect_priv;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::SCALAR_NEARLY_ZERO;
use skia_rust_core::scalar::scalar_is_int;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::blend_shader::BlendShader;
use skia_rust_core::shaders::color_filter_shader::ColorFilterShader;
use skia_rust_core::shaders::color_shader::ColorShader;
use skia_rust_core::shaders::coord_clamp_shader::CoordClampShader;
use skia_rust_core::shaders::ctm_shader::CtmShader;
use skia_rust_core::shaders::empty_shader::EmptyShader;
use skia_rust_core::shaders::image_shader::ImageShader;
use skia_rust_core::shaders::local_matrix_shader::LocalMatrixShader;
use skia_rust_core::shaders::runtime_shader::RuntimeShader;
use skia_rust_core::shaders::shader_base::{GradientType, ShaderBase, ShaderType};
use skia_rust_core::size::{ISize, Size};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::conical_gradient::{ConicalGradient, ConicalType};
use skia_rust_effects::gradient::Interpolation;
use skia_rust_effects::gradient::interpolation::ColorSpace as InterpolationColorSpace;
use skia_rust_effects::gradient::interpolation::InPremul;
use skia_rust_effects::gradient_base_shader::{Color4fXformer, GradientBaseShader};
use skia_rust_effects::linear_gradient::LinearGradient;
use skia_rust_effects::perlin_noise_shader::PerlinNoiseShader;
use skia_rust_effects::radial_gradient::RadialGradient;
use skia_rust_effects::sweep_gradient::SweepGradient;

use crate::gpu::dither_utils::{dither_range_for_config, make_dither_lut};
use crate::gpu::gpu_types::Origin;
use crate::gpu::gradient_bitmap::create_gradient_color_and_offset_bitmap;
use crate::gpu::sk_log::skia_log_w;
use crate::graphite::built_in_code_snippet_id::BuiltInCodeSnippetID;
use crate::graphite::caps::Caps;
use crate::graphite::image_graphite::Image as GraphiteImage;
use crate::graphite::key_context::{KeyContext, KeyGenFlags};
use crate::graphite::key_helpers_ii::{
    ColorSpaceTransformBlock, ColorSpaceTransformData, RuntimeEffectBlock, RuntimeEffectShaderData,
    ScopedUniformWriter, add_blend_mode, add_children_to_key, add_fixed_blend_mode,
    add_to_key_color_filter, blend, compose, solid_color_shader_add_block,
};
use crate::graphite::paint_params_key::PaintParamsKeyBuilder;
use crate::graphite::recorder::RecorderPriv;
use crate::graphite::resource_types::{ImmutableSamplerInfo, SamplerDesc};
use crate::graphite::storage_context::StorageContext;
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::texture_utils::{as_view, get_graphite_backed};
use crate::graphite::uniform_manager::UniformManager;

/// The key builder, borrowed for one call.
fn builder<'a>(key_context: &KeyContext<'a>) -> RefMut<'a, PaintParamsKeyBuilder> {
    key_context.paint_params_key_builder().borrow_mut()
}

/// `SkPMColor4f` as the components the uniform manager writes.
fn pm_components(c: &PMColor4f) -> [f32; 4] {
    [c.r, c.g, c.b, c.a]
}

/// `SkSize::Make(1.f / size.width(), 1.f / size.height())` for an `SkISize`.
// The image sizes are small integers, so the conversion to float is exact.
#[allow(clippy::cast_precision_loss)]
fn inverse_dimensions(size: ISize) -> [f32; 2] {
    [1.0 / size.width as f32, 1.0 / size.height as f32]
}

/// `SkSize::Make(SkISize)`.
// The image sizes are small integers, so the conversion to float is exact.
#[allow(clippy::cast_precision_loss)]
fn isize_to_size(size: ISize) -> Size {
    Size::new(size.width as f32, size.height as f32)
}

/// `SkRect::Make(SkISize)`.
fn rect_from_isize(size: ISize) -> Rect {
    Rect::from_iwh(size.width, size.height)
}

/// `SkTileMode::kDecal` becomes `kClamp`; every other mode is kept.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L794-L799 (chrome/m156), the UV tile mode choice
fn substitute_decal_with_clamp(tile_mode: TileMode) -> TileMode {
    if tile_mode == TileMode::Decal {
        TileMode::Clamp
    } else {
        tile_mode
    }
}

/// Downcasts a shader to its concrete type, through `Any`.
fn downcast_shader<T: 'static>(base: &dyn ShaderBase) -> Option<&T> {
    let any: &dyn Any = base;
    any.downcast_ref::<T>()
}

// ==================================================================
// Solid and paint colors
// ==================================================================

/// Adds a block with a premultiplied, solid color (`SolidColorShaderBlock`).
// Port of: src/gpu/graphite/KeyHelpers.h#L30-L32 (chrome/m156)
#[derive(Debug)]
pub struct SolidColorShaderBlock;

impl SolidColorShaderBlock {
    /// `SolidColorShaderBlock::AddBlock`.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L126-L136 (chrome/m156)
    #[doc(alias = "AddBlock")]
    pub fn add_block(key_context: &KeyContext<'_>, premul_color: &PMColor4f) {
        solid_color_shader_add_block(key_context, premul_color);
    }
}

/// Adds a block that takes the paint color from the uniforms (`RGBPaintColorBlock`).
// Port of: src/gpu/graphite/KeyHelpers.h#L34-L36 (chrome/m156)
#[derive(Debug)]
pub struct RGBPaintColorBlock;

impl RGBPaintColorBlock {
    /// `RGBPaintColorBlock::AddBlock`.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L142-L157 (chrome/m156)
    #[doc(alias = "AddBlock")]
    pub fn add_block(key_context: &KeyContext<'_>) {
        let mut scope = ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::RGBPaintColor);
        scope
            .uniforms()
            .write_paint_color(key_context.paint_color());
        builder(key_context).add_block(BuiltInCodeSnippetID::RGBPaintColor);
    }
}

/// Adds a block that takes the paint color from the uniforms, alpha only
/// (`AlphaOnlyPaintColorBlock`).
// Port of: src/gpu/graphite/KeyHelpers.h#L38-L40 (chrome/m156)
#[derive(Debug)]
pub struct AlphaOnlyPaintColorBlock;

impl AlphaOnlyPaintColorBlock {
    /// `AlphaOnlyPaintColorBlock::AddBlock`.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L159-L162 (chrome/m156)
    #[doc(alias = "AddBlock")]
    pub fn add_block(key_context: &KeyContext<'_>) {
        let mut scope =
            ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::AlphaOnlyPaintColor);
        scope
            .uniforms()
            .write_paint_color(key_context.paint_color());
        builder(key_context).add_block(BuiltInCodeSnippetID::AlphaOnlyPaintColor);
    }
}

// ==================================================================
// Gradients
// ==================================================================

/// The data a gradient block needs (`GradientShaderBlocks::GradientData`).
///
/// The inline stop limit is [`GradientData::NUM_INTERNAL_STORAGE_STOPS`]. Stop data above it
/// lives in a storage buffer or a color-and-offset texture (bound through the gatherer).
// Port of: src/gpu/graphite/KeyHelpers.h#L42-L107 (chrome/m156)
#[doc(alias = "GradientShaderBlocks::GradientData")]
#[derive(Clone, Debug)]
pub struct GradientData {
    /// `fType`.
    pub r#type: GradientType,
    /// `fPoints`.
    pub points: [Point; 2],
    /// `fRadii`.
    pub radii: [f32; 2],
    /// `fBias`.
    pub bias: f32,
    /// `fScale`.
    pub scale: f32,
    /// `fTM`.
    pub tm: TileMode,
    /// `fNumStops`.
    pub num_stops: usize,
    /// `fUseStorageBuffer`.
    pub use_storage_buffer: bool,
    /// `fColors`: the inline stops, padded out to `NUM_INTERNAL_STORAGE_STOPS`.
    pub colors: [PMColor4f; 8],
    /// `fOffsets`: the inline offsets, packed as two `SkV4`s and padded out like `colors`.
    pub offsets: [[f32; 4]; 2],
    /// `fColorsAndOffsetsProxy`. Set by the caller for stop counts above the inline limit when
    /// storage buffers are not used.
    pub colors_and_offsets_proxy: Option<Arc<TextureProxy>>,
    /// `fSrcColors`: the stops, kept for stop counts above the inline limit when storage buffers
    /// are used, to be copied into the storage buffer.
    pub src_colors: Vec<PMColor4f>,
    /// `fSrcOffsets`: the offsets of `src_colors` (`None` for evenly spaced stops).
    pub src_offsets: Option<Vec<f32>>,
    /// `fSrcShader`: the address of the gradient shader, which identifies its data in the
    /// `StorageContext`.
    pub src_shader: usize,
    /// `fInterpolation`.
    pub interpolation: Interpolation,
}

impl GradientData {
    /// `kNumInternalStorageStops`: the number of stops stored internal to this data structure
    /// before falling back to bitmap storage.
    // Port of: src/gpu/graphite/KeyHelpers.h#L50 (chrome/m156)
    pub const NUM_INTERNAL_STORAGE_STOPS: usize = 8;

    /// Used during pre-compilation, when there is not enough information to extract uniform data.
    /// It still provides enough data to make the decisions about which snippets to use.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L323-L338 (chrome/m156), the pre-compile constructor
    #[must_use]
    pub fn new_precompile(
        gradient_type: GradientType,
        num_stops: usize,
        use_storage_buffer: bool,
    ) -> Self {
        Self {
            r#type: gradient_type,
            points: [Point::new(0.0, 0.0), Point::new(0.0, 0.0)],
            radii: [0.0, 0.0],
            bias: 0.0,
            scale: 0.0,
            tm: TileMode::Clamp,
            num_stops,
            use_storage_buffer,
            colors: [PMColor4f {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }; 8],
            offsets: [[0.0; 4]; 2],
            colors_and_offsets_proxy: None,
            src_colors: Vec::new(),
            src_offsets: None,
            src_shader: 0,
            interpolation: Interpolation::default(),
        }
    }

    /// Used when extracting information from a `PaintParams`; it provides the data the selected
    /// snippet needs to write its uniforms. `offsets` of `None` means the stops are evenly spaced.
    #[must_use]
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L340-L392 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ constructor
    #[allow(clippy::cast_precision_loss)] // the stop index and count are small integers
    pub fn new(
        gradient_type: GradientType,
        point0: Point,
        point1: Point,
        radius0: f32,
        radius1: f32,
        bias: f32,
        scale: f32,
        tm: TileMode,
        num_stops: usize,
        colors: &[PMColor4f],
        offsets: Option<&[f32]>,
        shader: &GradientBaseShader,
        colors_and_offsets_proxy: Option<Arc<TextureProxy>>,
        use_storage_buffer: bool,
        interpolation: Interpolation,
    ) -> Self {
        debug_assert!(num_stops >= 1);
        let mut data = Self {
            r#type: gradient_type,
            points: [point0, point1],
            radii: [radius0, radius1],
            bias,
            scale,
            tm,
            num_stops,
            use_storage_buffer,
            colors: [PMColor4f {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }; 8],
            offsets: [[0.0; 4]; 2],
            colors_and_offsets_proxy: None,
            src_colors: if num_stops > Self::NUM_INTERNAL_STORAGE_STOPS && use_storage_buffer {
                colors[..num_stops].to_vec()
            } else {
                Vec::new()
            },
            src_offsets: if num_stops > Self::NUM_INTERNAL_STORAGE_STOPS && use_storage_buffer {
                offsets.map(|offsets| offsets[..num_stops].to_vec())
            } else {
                None
            },
            src_shader: std::ptr::from_ref(shader) as usize,
            interpolation,
        };

        if num_stops <= Self::NUM_INTERNAL_STORAGE_STOPS {
            data.colors[..num_stops].copy_from_slice(&colors[..num_stops]);
            // `rawOffsets` is the flat view of `fOffsets`.
            let mut raw_offsets = [0.0_f32; 8];
            for (i, raw) in raw_offsets.iter_mut().enumerate().take(num_stops) {
                *raw = match offsets {
                    Some(offsets) => offsets[i],
                    None => (i as f32) / ((num_stops - 1) as f32),
                };
            }
            // Extend the colors and offsets, if necessary, to fill out the arrays. The unrolled
            // binary search implementation assumes excess stops match the last real value.
            for i in num_stops..Self::NUM_INTERNAL_STORAGE_STOPS {
                data.colors[i] = data.colors[num_stops - 1];
                raw_offsets[i] = raw_offsets[num_stops - 1];
            }
            data.offsets = [
                [
                    raw_offsets[0],
                    raw_offsets[1],
                    raw_offsets[2],
                    raw_offsets[3],
                ],
                [
                    raw_offsets[4],
                    raw_offsets[5],
                    raw_offsets[6],
                    raw_offsets[7],
                ],
            ];
        } else if !use_storage_buffer {
            data.colors_and_offsets_proxy = colors_and_offsets_proxy;
            debug_assert!(data.colors_and_offsets_proxy.is_some());
        }
        data
    }
}

/// Writes the color and offset data directly in the gatherer gradient buffer and returns the
/// offset the data begins at in the buffer.
///
/// Returns a negative offset to signal failure, in which case the paint key must be poisoned
/// to drop the draw.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L297-L321 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // the stop index and count are small integers
fn write_color_and_offset_bufdata(
    storage_context: &mut StorageContext,
    num_stops: usize,
    colors: &[PMColor4f],
    offsets: Option<&[f32]>,
    shader_key: usize,
) -> i32 {
    let (dst_data, buffer_offset) = storage_context.allocate_gradient_data_for(
        i32::try_from(num_stops).expect("gradient stop count fits an int"),
        shader_key,
    );
    if let Some(dst_data) = dst_data {
        debug_assert!(buffer_offset >= 0);
        // Data doesn't already exist so we need to write it. Writes all offset data, then color
        // data. This way when binary searching through the offsets, there is better cache
        // locality.
        let mut color_idx = num_stops;
        for i in 0..num_stops {
            let offset = match offsets {
                Some(offsets) => offsets[i],
                None => (i as f32) / ((num_stops - 1) as f32),
            };
            debug_assert!((0.0..=1.0).contains(&offset));

            dst_data[i] = offset;
            dst_data[color_idx] = colors[i].r;
            dst_data[color_idx + 1] = colors[i].g;
            dst_data[color_idx + 2] = colors[i].b;
            dst_data[color_idx + 3] = colors[i].a;
            color_idx += 4;
        }
    }

    buffer_offset
}

/// Adds the gradient blocks (`GradientShaderBlocks`).
// Port of: src/gpu/graphite/KeyHelpers.h#L108-L110 (chrome/m156)
#[derive(Debug)]
pub struct GradientShaderBlocks;

impl GradientShaderBlocks {
    /// `GradientShaderBlocks::AddBlock`. Stop counts above the inline limit need a storage buffer
    /// (not ported: an error block) or a color-and-offset texture, which is bound through the
    /// gatherer and needs a recorder; without its proxy the block adds an error block.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L394-L465 (chrome/m156)
    #[doc(alias = "AddBlock")]
    pub fn add_block(key_context: &KeyContext<'_>, grad_data: &GradientData) {
        // The buffer offset is only non-zero on the storage-buffer path.
        let mut buffer_offset = 0;
        if grad_data.num_stops > GradientData::NUM_INTERNAL_STORAGE_STOPS
            && key_context.recorder().is_some()
        {
            let has_storage;
            if grad_data.use_storage_buffer {
                debug_assert!(key_context.storage_context().is_some());
                if let Some(storage_context) = key_context.storage_context() {
                    buffer_offset = write_color_and_offset_bufdata(
                        &mut storage_context.borrow_mut(),
                        grad_data.num_stops,
                        &grad_data.src_colors,
                        grad_data.src_offsets.as_deref(),
                        grad_data.src_shader,
                    );
                    has_storage = buffer_offset >= 0;
                } else {
                    has_storage = false;
                }
            } else {
                // The color-and-offset texture is bound with nearest filtering and clamped
                // tiling.
                key_context.pipeline_data_gatherer().borrow_mut().add(
                    grad_data.colors_and_offsets_proxy.clone(),
                    SamplerDesc::new(&SamplingOptions::from(FilterMode::Nearest), TileMode::Clamp),
                );
                has_storage = grad_data.colors_and_offsets_proxy.is_some();
            }

            if !has_storage {
                builder(key_context).add_error_block();
                skia_log_w!("Couldn't upload large gradient color stop data");
                return;
            }
        }

        let n = grad_data.num_stops;
        let storage = grad_data.use_storage_buffer;
        let code_snippet_id = match grad_data.r#type {
            GradientType::Linear => {
                let id = if n <= 4 {
                    BuiltInCodeSnippetID::LinearGradientShader4
                } else if n <= 8 {
                    BuiltInCodeSnippetID::LinearGradientShader8
                } else if storage {
                    BuiltInCodeSnippetID::LinearGradientShaderBuffer
                } else {
                    BuiltInCodeSnippetID::LinearGradientShaderTexture
                };
                add_linear_gradient_uniform_data(key_context, id, grad_data, buffer_offset);
                id
            }
            GradientType::Radial => {
                let id = if n <= 4 {
                    BuiltInCodeSnippetID::RadialGradientShader4
                } else if n <= 8 {
                    BuiltInCodeSnippetID::RadialGradientShader8
                } else if storage {
                    BuiltInCodeSnippetID::RadialGradientShaderBuffer
                } else {
                    BuiltInCodeSnippetID::RadialGradientShaderTexture
                };
                add_radial_gradient_uniform_data(key_context, id, grad_data, buffer_offset);
                id
            }
            GradientType::Sweep => {
                let id = if n <= 4 {
                    BuiltInCodeSnippetID::SweepGradientShader4
                } else if n <= 8 {
                    BuiltInCodeSnippetID::SweepGradientShader8
                } else if storage {
                    BuiltInCodeSnippetID::SweepGradientShaderBuffer
                } else {
                    BuiltInCodeSnippetID::SweepGradientShaderTexture
                };
                add_sweep_gradient_uniform_data(key_context, id, grad_data, buffer_offset);
                id
            }
            GradientType::Conical => {
                let id = if n <= 4 {
                    BuiltInCodeSnippetID::ConicalGradientShader4
                } else if n <= 8 {
                    BuiltInCodeSnippetID::ConicalGradientShader8
                } else if storage {
                    BuiltInCodeSnippetID::ConicalGradientShaderBuffer
                } else {
                    BuiltInCodeSnippetID::ConicalGradientShaderTexture
                };
                add_conical_gradient_uniform_data(key_context, id, grad_data, buffer_offset);
                id
            }
            GradientType::None => {
                // `SkDEBUGFAIL`; the C++ code then falls through to the solid color snippet with
                // no uniforms written.
                debug_assert!(false, "Expected a gradient shader, but it wasn't one.");
                BuiltInCodeSnippetID::SolidColorShader
            }
        };

        builder(key_context).add_block(code_snippet_id);
    }
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L166-L186 (chrome/m156), `add_gradient_preamble`
fn add_gradient_preamble(grad_data: &GradientData, gatherer: &mut UniformManager) {
    if grad_data.num_stops <= GradientData::NUM_INTERNAL_STORAGE_STOPS {
        if grad_data.num_stops <= 4 {
            // Round up to 4 stops.
            let colors: [[f32; 4]; 4] =
                std::array::from_fn(|i| pm_components(&grad_data.colors[i]));
            gatherer.write_array_vec(&colors[..]);
            gatherer.write_vec(grad_data.offsets[0]);
        } else {
            // Round up to 8 stops.
            let colors: [[f32; 4]; 8] =
                std::array::from_fn(|i| pm_components(&grad_data.colors[i]));
            gatherer.write_array_vec(&colors[..]);
            gatherer.write_array_vec(&grad_data.offsets[..]);
        }
    }
}

// All the gradients share a common postamble of:
//   numStops - for texture-based gradients
//   tilemode
//   colorSpace
//   doUnPremul
// Port of: src/gpu/graphite/KeyHelpers.cpp#L188-L221 (chrome/m156), `add_gradient_postamble`
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // the stop count fits i32
fn add_gradient_postamble(
    grad_data: &GradientData,
    buffer_offset: i32,
    gatherer: &mut UniformManager,
) {
    use InterpolationColorSpace as ColorSpaceEnum;

    // The C++ static_asserts pin the enum values the shaders expect.
    debug_assert_eq!(ColorSpaceEnum::Lab as i32, 2);
    debug_assert_eq!(ColorSpaceEnum::OKLab as i32, 3);
    debug_assert_eq!(ColorSpaceEnum::HSL as i32, 9);
    debug_assert_eq!(ColorSpaceEnum::HWB as i32, 10);

    let input_premul = grad_data.interpolation.in_premul as i32;

    if grad_data.num_stops > GradientData::NUM_INTERNAL_STORAGE_STOPS {
        gatherer.write_i32(grad_data.num_stops as i32);
        if grad_data.use_storage_buffer {
            gatherer.write_i32(buffer_offset);
        }
    }

    gatherer.write_i32(grad_data.tm as i32);
    gatherer.write_i32(grad_data.interpolation.color_space as i32);
    gatherer.write_i32(input_premul);
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L223-L231 (chrome/m156)
fn add_linear_gradient_uniform_data(
    key_context: &KeyContext<'_>,
    code_snippet_id: BuiltInCodeSnippetID,
    grad_data: &GradientData,
    buffer_offset: i32,
) {
    let mut scope = ScopedUniformWriter::new(key_context, code_snippet_id);
    add_gradient_preamble(grad_data, scope.uniforms());
    add_gradient_postamble(grad_data, buffer_offset, scope.uniforms());
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L233-L241 (chrome/m156)
fn add_radial_gradient_uniform_data(
    key_context: &KeyContext<'_>,
    code_snippet_id: BuiltInCodeSnippetID,
    grad_data: &GradientData,
    buffer_offset: i32,
) {
    let mut scope = ScopedUniformWriter::new(key_context, code_snippet_id);
    add_gradient_preamble(grad_data, scope.uniforms());
    add_gradient_postamble(grad_data, buffer_offset, scope.uniforms());
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L243-L253 (chrome/m156)
fn add_sweep_gradient_uniform_data(
    key_context: &KeyContext<'_>,
    code_snippet_id: BuiltInCodeSnippetID,
    grad_data: &GradientData,
    buffer_offset: i32,
) {
    let mut scope = ScopedUniformWriter::new(key_context, code_snippet_id);
    add_gradient_preamble(grad_data, scope.uniforms());
    scope.uniforms().write_f32(grad_data.bias);
    scope.uniforms().write_f32(grad_data.scale);
    add_gradient_postamble(grad_data, buffer_offset, scope.uniforms());
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L255-L288 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // the C++ stores the double quotient as a float
fn add_conical_gradient_uniform_data(
    key_context: &KeyContext<'_>,
    code_snippet_id: BuiltInCodeSnippetID,
    grad_data: &GradientData,
    buffer_offset: i32,
) {
    let mut scope = ScopedUniformWriter::new(key_context, code_snippet_id);

    let mut d_radius = grad_data.radii[1] - grad_data.radii[0];
    let is_radial = Point::distance(grad_data.points[1], grad_data.points[0]) < SCALAR_NEARLY_ZERO;

    // When a == 0, encode invA == 1 for radial case, and invA == 0 for linear edge case.
    let mut a: f32 = 0.0;
    let mut inv_a: f32 = 1.0;
    if is_radial {
        // Since radius0 is being scaled by 1 / dRadius, and the original radius is always
        // positive, this gives us the original sign of dRadius.
        d_radius = if grad_data.radii[0] > 0.0 { 1.0 } else { -1.0 };
    } else {
        a = 1.0 - d_radius * d_radius;
        if a.abs() > SCALAR_NEARLY_ZERO {
            // The C++ computes `1.0 / (2.0 * a)` in double precision and stores it as a float.
            inv_a = (1.0_f64 / (2.0_f64 * f64::from(a))) as f32;
        } else {
            a = 0.0;
            inv_a = 0.0;
        }
    }

    add_gradient_preamble(grad_data, scope.uniforms());
    scope.uniforms().write_f32(grad_data.radii[0]);
    scope.uniforms().write_f32(d_radius);
    scope.uniforms().write_f32(a);
    scope.uniforms().write_f32(inv_a);
    add_gradient_postamble(grad_data, buffer_offset, scope.uniforms());
}

// ==================================================================
// Local matrices and coordinate blocks
// ==================================================================

/// The matrix a local-matrix block applies (`LocalMatrixShaderBlock::LMShaderData`).
// Port of: src/gpu/graphite/KeyHelpers.h#L122-L134 (chrome/m156)
#[derive(Clone, Debug)]
pub struct LMShaderData {
    /// `fLocalMatrix`. Applied to `coords.xy01`, so a 4x4 is flattened to a 3x3 here.
    pub local_matrix: Matrix,
}

impl LMShaderData {
    /// Wraps `local_matrix` (`LMShaderData(const SkMatrix&)`).
    // Port of: src/gpu/graphite/KeyHelpers.h#L124-L125 (chrome/m156)
    #[must_use]
    pub fn new(local_matrix: Matrix) -> Self {
        Self { local_matrix }
    }
}

/// Begins a local matrix block (`LocalMatrixShaderBlock`).
// Port of: src/gpu/graphite/KeyHelpers.h#L121-L135 (chrome/m156)
#[derive(Debug)]
pub struct LocalMatrixShaderBlock;

impl LocalMatrixShaderBlock {
    /// `LocalMatrixShaderBlock::BeginBlock`. The caller ends the block.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L469-L490 (chrome/m156)
    #[doc(alias = "BeginBlock")]
    pub fn begin_block(key_context: &KeyContext<'_>, lm_shader_data: &LMShaderData) {
        let m = &lm_shader_data.local_matrix;

        if m.has_perspective() {
            // Perspective local matrices are rare enough and add enough extra instructions that
            // it's worth specializing since it has to perform a per-pixel division.
            builder(key_context).begin_block(BuiltInCodeSnippetID::LocalMatrixShaderPersp);
            let mut scope =
                ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::LocalMatrixShaderPersp);
            scope.uniforms().write_matrix(m);
        } else {
            // For an affine 2D transform, we only need to upload the upper 2x2 and XY translation.
            builder(key_context).begin_block(BuiltInCodeSnippetID::LocalMatrixShader);

            let mut scope =
                ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::LocalMatrixShader);
            // The upper 2x2 is expected to be in column major order, but SkMatrix is 3x3 row major.
            scope
                .uniforms()
                .write_vec([m.scale_x(), m.skew_y(), m.skew_x(), m.scale_y()]);
            scope
                .uniforms()
                .write_vec([m.translate_x(), m.translate_y()]);
        }
    }
}

/// The normalization of coordinates by the inverse of an image's dimensions
/// (`CoordNormalizeShaderBlock::CoordNormalizeData`).
// Port of: src/gpu/graphite/KeyHelpers.h#L136-L146 (chrome/m156)
#[derive(Clone, Copy, Debug)]
pub struct CoordNormalizeData {
    /// `fInvDimensions`.
    pub inv_dimensions: Size,
}

impl CoordNormalizeData {
    /// Stores `1 / dimensions` (`CoordNormalizeData(SkSize)`).
    // Port of: src/gpu/graphite/KeyHelpers.h#L138-L140 (chrome/m156)
    #[must_use]
    pub fn new(dimensions: Size) -> Self {
        Self {
            inv_dimensions: Size::new(1.0 / dimensions.width, 1.0 / dimensions.height),
        }
    }
}

/// Begins a coordinate normalization block (`CoordNormalizeShaderBlock`).
// Port of: src/gpu/graphite/KeyHelpers.h#L148-L150 (chrome/m156)
#[derive(Debug)]
pub struct CoordNormalizeShaderBlock;

impl CoordNormalizeShaderBlock {
    /// `CoordNormalizeShaderBlock::BeginBlock`. The caller ends the block.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L843-L857 (chrome/m156)
    #[doc(alias = "BeginBlock")]
    pub fn begin_block(key_context: &KeyContext<'_>, data: &CoordNormalizeData) {
        let mut scope =
            ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::CoordNormalizeShader);
        scope
            .uniforms()
            .write_vec([data.inv_dimensions.width, data.inv_dimensions.height]);
        builder(key_context).begin_block(BuiltInCodeSnippetID::CoordNormalizeShader);
    }
}

/// The subset that coordinates are clamped to (`CoordClampShaderBlock::CoordClampData`).
// Port of: src/gpu/graphite/KeyHelpers.h#L152-L158 (chrome/m156)
#[derive(Clone, Copy, Debug)]
pub struct CoordClampData {
    /// `fSubset`.
    pub subset: Rect,
}

impl CoordClampData {
    /// Wraps `subset` (`CoordClampData(SkRect)`).
    // Port of: src/gpu/graphite/KeyHelpers.h#L154 (chrome/m156)
    #[must_use]
    pub fn new(subset: Rect) -> Self {
        Self { subset }
    }
}

/// Begins a coordinate clamp block (`CoordClampShaderBlock`).
// Port of: src/gpu/graphite/KeyHelpers.h#L160-L162 (chrome/m156)
#[derive(Debug)]
pub struct CoordClampShaderBlock;

impl CoordClampShaderBlock {
    /// `CoordClampShaderBlock::BeginBlock`. The caller ends the block.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L871-L875 (chrome/m156)
    #[doc(alias = "BeginBlock")]
    pub fn begin_block(key_context: &KeyContext<'_>, clamp_data: &CoordClampData) {
        let mut scope =
            ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::CoordClampShader);
        scope.uniforms().write_rect(&clamp_data.subset);
        builder(key_context).begin_block(BuiltInCodeSnippetID::CoordClampShader);
    }
}

// ==================================================================
// Images
// ==================================================================

/// The data of an image shader block (`ImageShaderBlock::ImageData`).
// Port of: src/gpu/graphite/KeyHelpers.h#L160-L185 (chrome/m156)
#[doc(alias = "ImageShaderBlock::ImageData")]
#[derive(Clone, Debug)]
pub struct ImageData {
    /// `fSampling`.
    pub sampling: SamplingOptions,
    /// `fTileModes`.
    pub tile_modes: (TileMode, TileMode),
    /// `fImgSize`.
    pub img_size: ISize,
    /// `fSubset`.
    pub subset: Rect,
    /// `fTextureProxy`. Set when the key comes from an actual image; `None` for pre-compilation.
    pub texture_proxy: Option<Arc<TextureProxy>>,
    /// `fImmutableSamplerInfo`, used when there is no texture proxy.
    pub immutable_sampler_info: ImmutableSamplerInfo,
}

impl ImageData {
    /// `ImageShaderBlock::ImageData(sampling, tileModeX, tileModeY, imgSize, subset, info)`.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L569-L580 (chrome/m156)
    #[must_use]
    pub fn new(
        sampling: SamplingOptions,
        tile_mode_x: TileMode,
        tile_mode_y: TileMode,
        img_size: ISize,
        subset: Rect,
        immutable_sampler_info: ImmutableSamplerInfo,
    ) -> Self {
        Self {
            sampling,
            tile_modes: (tile_mode_x, tile_mode_y),
            img_size,
            subset,
            texture_proxy: None,
            immutable_sampler_info,
        }
    }
}

/// Adds the image shader blocks (`ImageShaderBlock`).
// Port of: src/gpu/graphite/KeyHelpers.h#L187-L189 (chrome/m156)
#[derive(Debug)]
pub struct ImageShaderBlock;

// Port of: src/gpu/graphite/KeyHelpers.cpp#L496-L507 (chrome/m156)
fn add_image_uniform_data(key_context: &KeyContext<'_>, img_data: &ImageData) {
    debug_assert!(!img_data.sampling.use_cubic);
    let mut scope = ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::ImageShader);

    scope
        .uniforms()
        .write_vec(inverse_dimensions(img_data.img_size));
    scope.uniforms().write_rect(&img_data.subset);
    scope.uniforms().write_i32(img_data.tile_modes.0 as i32);
    scope.uniforms().write_i32(img_data.tile_modes.1 as i32);
    scope.uniforms().write_i32(img_data.sampling.filter as i32);
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L509-L529 (chrome/m156)
fn add_clamp_image_uniform_data(key_context: &KeyContext<'_>, img_data: &ImageData) {
    // Matches GrTextureEffect::kLinearInset, to make sure we don't touch an outer row or column
    // with a weight of 0 when linear filtering.
    const LINEAR_INSET: f32 = 0.5 + 0.000_01;

    debug_assert!(!img_data.sampling.use_cubic);
    let mut scope = ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::ImageShaderClamp);

    scope
        .uniforms()
        .write_vec(inverse_dimensions(img_data.img_size));

    // The subset should clamp texel coordinates to an inset subset to prevent sampling neighboring
    // texels when coords fall exactly at texel boundaries.
    let mut subset_inset_clamp = img_data.subset;
    if img_data.sampling.filter == FilterMode::Nearest {
        subset_inset_clamp = subset_inset_clamp.round_out();
    }
    subset_inset_clamp.inset(Point::new(LINEAR_INSET, LINEAR_INSET));
    scope.uniforms().write_rect(&subset_inset_clamp);
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L531-L544 (chrome/m156)
fn add_cubic_image_uniform_data(key_context: &KeyContext<'_>, img_data: &ImageData) {
    debug_assert!(img_data.sampling.use_cubic);
    let mut scope = ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::CubicImageShader);

    scope
        .uniforms()
        .write_vec(inverse_dimensions(img_data.img_size));
    scope.uniforms().write_rect(&img_data.subset);
    scope.uniforms().write_i32(img_data.tile_modes.0 as i32);
    scope.uniforms().write_i32(img_data.tile_modes.1 as i32);
    let cubic = &img_data.sampling.cubic;
    scope
        .uniforms()
        .write_half_m44(&cubic_resampler_matrix(cubic.b, cubic.c));
}

/// `SkImageShader::CubicResamplerMatrix`.
fn cubic_resampler_matrix(b: f32, c: f32) -> M44 {
    ImageShader::cubic_resampler_matrix(b, c)
}

// If clampToBorderSupport is unavailable, kDecal will be substituted to clamp in most cases.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L546-L550 (chrome/m156)
fn should_substitute_decal(tile_modes: (TileMode, TileMode), caps: &dyn Caps) -> bool {
    !caps.clamp_to_border_support()
        && (tile_modes.0 == TileMode::Decal || tile_modes.1 == TileMode::Decal)
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L552-L555 (chrome/m156)
fn can_do_tiling_in_hw(img_data: &ImageData, caps: &dyn Caps) -> bool {
    !should_substitute_decal(img_data.tile_modes, caps)
        && img_data.subset.contains(rect_from_isize(img_data.img_size))
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L557-L565 (chrome/m156)
fn add_sampler_data_to_key(key_context: &KeyContext<'_>, sampler_desc: &SamplerDesc) {
    if sampler_desc.is_immutable() {
        builder(key_context).add_data(&sampler_desc.as_span());
    } else {
        // Means we have a regular dynamic sampler. Append a default SamplerDesc to convey this,
        // allowing the key to maintain and convey sampler binding order.
        builder(key_context).add_data(&[]);
    }
}

impl ImageShaderBlock {
    /// `ImageShaderBlock::AddBlock`. The texture is bound through the gatherer.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L583-L634 (chrome/m156)
    #[doc(alias = "AddBlock")]
    pub fn add_block(key_context: &KeyContext<'_>, img_data: &ImageData) {
        if key_context.recorder().is_some() && img_data.texture_proxy.is_none() {
            builder(key_context).add_error_block();
            return;
        }

        let caps = key_context.caps();
        let do_tiling_in_hw = !img_data.sampling.use_cubic && can_do_tiling_in_hw(img_data, caps);

        if do_tiling_in_hw {
            let data = CoordNormalizeData::new(isize_to_size(img_data.img_size));
            CoordNormalizeShaderBlock::begin_block(key_context, &data);
            builder(key_context).begin_block(BuiltInCodeSnippetID::HWImageShader);
        } else if img_data.sampling.use_cubic {
            add_cubic_image_uniform_data(key_context, img_data);
            builder(key_context).begin_block(BuiltInCodeSnippetID::CubicImageShader);
        } else if img_data.tile_modes.0 == TileMode::Clamp
            && img_data.tile_modes.1 == TileMode::Clamp
        {
            add_clamp_image_uniform_data(key_context, img_data);
            builder(key_context).begin_block(BuiltInCodeSnippetID::ImageShaderClamp);
        } else {
            add_image_uniform_data(key_context, img_data);
            builder(key_context).begin_block(BuiltInCodeSnippetID::ImageShader);
        }

        // Image shaders must append immutable sampler data (or '0' in the more common case where
        // regular samplers are used).
        let info = match &img_data.texture_proxy {
            Some(proxy) => caps.get_immutable_sampler_info(proxy.texture_info()),
            None => img_data.immutable_sampler_info,
        };
        let tile_mode_with_substitution = if do_tiling_in_hw {
            img_data.tile_modes
        } else {
            (TileMode::Clamp, TileMode::Clamp)
        };

        let sampler_desc =
            SamplerDesc::new_with_tile_modes(&img_data.sampling, tile_mode_with_substitution, info);
        key_context
            .pipeline_data_gatherer()
            .borrow_mut()
            .add(img_data.texture_proxy.clone(), sampler_desc);
        add_sampler_data_to_key(key_context, &sampler_desc);

        builder(key_context).end_block();

        if do_tiling_in_hw {
            // Additional block for coord normalization.
            builder(key_context).end_block();
        }
    }
}

// ==================================================================
// YUV images
// ==================================================================

/// The data of a YUV image shader block (`YUVImageShaderBlock::ImageData`).
// Port of: src/gpu/graphite/KeyHelpers.h#L191-L221 (chrome/m156)
#[doc(alias = "YUVImageShaderBlock::ImageData")]
#[derive(Clone, Debug)]
pub struct YUVImageData {
    /// `fSampling`.
    pub sampling: SamplingOptions,
    /// `fSamplingUV`.
    pub sampling_uv: SamplingOptions,
    /// `fTileModes`.
    pub tile_modes: (TileMode, TileMode),
    /// `fImgSize`.
    pub img_size: ISize,
    /// `fImgSizeUV`: size of the UV planes relative to Y's texel space.
    pub img_size_uv: ISize,
    /// `fSubset`.
    pub subset: Rect,
    /// `fLinearFilterUVInset`.
    pub linear_filter_uv_inset: Point,
    /// `fChannelSelect`.
    pub channel_select: [[f32; 4]; 4],
    /// `fAlphaParam`.
    pub alpha_param: f32,
    /// `fYUVtoRGBMatrix`.
    pub yuv_to_rgb_matrix: Matrix,
    /// `fYUVtoRGBTranslate`.
    pub yuv_to_rgb_translate: [f32; 3],
    /// `fTextureProxies`. Set when the key comes from an actual image.
    pub texture_proxies: [Option<Arc<TextureProxy>>; 4],
}

impl YUVImageData {
    /// `YUVImageShaderBlock::ImageData(sampling, tileModeX, tileModeY, imgSize, subset)`.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L756-L767 (chrome/m156)
    #[must_use]
    pub fn new(
        sampling: SamplingOptions,
        tile_mode_x: TileMode,
        tile_mode_y: TileMode,
        img_size: ISize,
        subset: Rect,
    ) -> Self {
        Self {
            sampling,
            sampling_uv: sampling,
            tile_modes: (tile_mode_x, tile_mode_y),
            img_size,
            img_size_uv: img_size,
            subset,
            // `{ 0.50001f, 0.50001f }`
            linear_filter_uv_inset: Point::new(0.50001, 0.50001),
            channel_select: [[0.0; 4]; 4],
            alpha_param: 0.0,
            yuv_to_rgb_matrix: Matrix::default(),
            yuv_to_rgb_translate: [0.0; 3],
            texture_proxies: [None, None, None, None],
        }
    }
}

/// Adds the YUV image shader blocks (`YUVImageShaderBlock`).
// Port of: src/gpu/graphite/KeyHelpers.h#L223-L225 (chrome/m156)
#[derive(Debug)]
pub struct YUVImageShaderBlock;

// Port of: src/gpu/graphite/KeyHelpers.cpp#L642-L662 (chrome/m156)
fn add_yuv_image_uniform_data(key_context: &KeyContext<'_>, img_data: &YUVImageData) {
    let mut scope = ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::YUVImageShader);

    scope
        .uniforms()
        .write_vec(inverse_dimensions(img_data.img_size));
    scope
        .uniforms()
        .write_vec(inverse_dimensions(img_data.img_size_uv));
    scope.uniforms().write_rect(&img_data.subset);
    scope.uniforms().write_vec([
        img_data.linear_filter_uv_inset.x,
        img_data.linear_filter_uv_inset.y,
    ]);
    scope.uniforms().write_i32(img_data.tile_modes.0 as i32);
    scope.uniforms().write_i32(img_data.tile_modes.1 as i32);
    scope.uniforms().write_i32(img_data.sampling.filter as i32);
    scope
        .uniforms()
        .write_i32(img_data.sampling_uv.filter as i32);

    for channel in img_data.channel_select {
        scope.uniforms().write_half_vec(channel);
    }
    scope
        .uniforms()
        .write_half_matrix(&img_data.yuv_to_rgb_matrix);
    scope
        .uniforms()
        .write_half_vec(img_data.yuv_to_rgb_translate);
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L664-L683 (chrome/m156)
fn add_cubic_yuv_image_uniform_data(key_context: &KeyContext<'_>, img_data: &YUVImageData) {
    let mut scope =
        ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::CubicYUVImageShader);

    scope
        .uniforms()
        .write_vec(inverse_dimensions(img_data.img_size));
    scope
        .uniforms()
        .write_vec(inverse_dimensions(img_data.img_size_uv));
    scope.uniforms().write_rect(&img_data.subset);
    scope.uniforms().write_i32(img_data.tile_modes.0 as i32);
    scope.uniforms().write_i32(img_data.tile_modes.1 as i32);
    let cubic = &img_data.sampling.cubic;
    scope
        .uniforms()
        .write_half_m44(&cubic_resampler_matrix(cubic.b, cubic.c));

    for channel in img_data.channel_select {
        scope.uniforms().write_half_vec(channel);
    }
    scope
        .uniforms()
        .write_half_matrix(&img_data.yuv_to_rgb_matrix);
    scope
        .uniforms()
        .write_half_vec(img_data.yuv_to_rgb_translate);
}

/// The sign-encoded linear-filter UV inset shared by the hardware-tiled YUV blocks.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L685-L716 (chrome/m156), the inset encoding
fn hw_linear_filter_uv_inset(img_data: &YUVImageData) -> Point {
    let mut inset = img_data.linear_filter_uv_inset;
    // We sign-encode whether we need to adjust the UV coords by applying `fLinearFilterUVInset`
    // for nearest neighbor filtering in `linearFilterUVInset.fX`.
    if img_data.sampling.filter == FilterMode::Nearest {
        inset.x = -inset.x;
    }
    // We sign-encode whether we need clamping for subset or mismatched Y/UV plane size draws in
    // `linearFilterUVInset.fY` - only clamp tiling modes are supported though.
    if !img_data.subset.contains(rect_from_isize(img_data.img_size))
        || img_data.img_size != img_data.img_size_uv
    {
        debug_assert!(
            img_data.tile_modes.0 == TileMode::Clamp && img_data.tile_modes.1 == TileMode::Clamp
        );
        inset.y = -inset.y;
    }
    inset
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L685-L716 (chrome/m156)
fn add_hw_yuv_image_uniform_data(key_context: &KeyContext<'_>, img_data: &YUVImageData) {
    let mut scope = ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::HWYUVImageShader);

    scope
        .uniforms()
        .write_vec(inverse_dimensions(img_data.img_size));
    scope
        .uniforms()
        .write_vec(inverse_dimensions(img_data.img_size_uv));
    scope.uniforms().write_rect(&img_data.subset);

    let inset = hw_linear_filter_uv_inset(img_data);
    scope.uniforms().write_vec([inset.x, inset.y]);

    for channel in img_data.channel_select {
        scope.uniforms().write_half_vec(channel);
    }
    scope
        .uniforms()
        .write_half_matrix(&img_data.yuv_to_rgb_matrix);
    scope
        .uniforms()
        .write_half_vec(img_data.yuv_to_rgb_translate);
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L718-L752 (chrome/m156)
fn add_hw_yuv_no_swizzle_image_uniform_data(key_context: &KeyContext<'_>, img_data: &YUVImageData) {
    let mut scope =
        ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::HWYUVNoSwizzleImageShader);

    scope
        .uniforms()
        .write_vec(inverse_dimensions(img_data.img_size));
    scope
        .uniforms()
        .write_vec(inverse_dimensions(img_data.img_size_uv));
    scope.uniforms().write_rect(&img_data.subset);

    let inset = hw_linear_filter_uv_inset(img_data);
    scope.uniforms().write_vec([inset.x, inset.y]);

    scope
        .uniforms()
        .write_half_matrix(&img_data.yuv_to_rgb_matrix);
    let translate_alpha = [
        img_data.yuv_to_rgb_translate[0],
        img_data.yuv_to_rgb_translate[1],
        img_data.yuv_to_rgb_translate[2],
        img_data.alpha_param,
    ];
    scope.uniforms().write_half_vec(translate_alpha);
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L769-L780 (chrome/m156)
fn can_do_yuv_tiling_in_hw(img_data: &YUVImageData, caps: &dyn Caps) -> bool {
    if should_substitute_decal(img_data.tile_modes, caps) {
        return false;
    }
    // Use the HW tiling shader variant if we're drawing the full rect with matched Y and UV plane
    // sizes and any tiling mode, or if we're drawing a subset with clamp tiling mode.
    (img_data.subset.contains(rect_from_isize(img_data.img_size))
        && img_data.img_size == img_data.img_size_uv)
        || (img_data.tile_modes.0 == TileMode::Clamp && img_data.tile_modes.1 == TileMode::Clamp)
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L782-L792 (chrome/m156)
#[allow(clippy::float_cmp)] // the C++ compares the exact channel value with 1
fn no_yuv_swizzle(img_data: &YUVImageData) -> bool {
    // Y_U_V or U_Y_V format, reading from R channel for each texture
    img_data
        .channel_select
        .iter()
        .all(|channel| channel[0] == 1.0)
}

impl YUVImageShaderBlock {
    /// `YUVImageShaderBlock::AddBlock`. The four textures are bound through the gatherer (see
    /// the module docs for the image views).
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L794-L839 (chrome/m156)
    #[doc(alias = "AddBlock")]
    pub fn add_block(key_context: &KeyContext<'_>, img_data: &YUVImageData) {
        if key_context.recorder().is_some() && img_data.texture_proxies.iter().any(Option::is_none)
        {
            builder(key_context).add_error_block();
            return;
        }

        let caps = key_context.caps();
        let do_tiling_in_hw =
            !img_data.sampling.use_cubic && can_do_yuv_tiling_in_hw(img_data, caps);
        let no_swizzle = no_yuv_swizzle(img_data);

        // uvs are never SkTileMode::kDecal
        let uv_tile_modes = (
            substitute_decal_with_clamp(img_data.tile_modes.0),
            substitute_decal_with_clamp(img_data.tile_modes.1),
        );
        let y_alpha_tile_modes = if do_tiling_in_hw {
            img_data.tile_modes
        } else {
            (TileMode::Clamp, TileMode::Clamp)
        };
        {
            let mut gatherer = key_context.pipeline_data_gatherer().borrow_mut();
            let proxies = &img_data.texture_proxies;
            gatherer.add(
                proxies[0].clone(),
                SamplerDesc::new_with_tile_modes(
                    &img_data.sampling,
                    y_alpha_tile_modes,
                    ImmutableSamplerInfo::default(),
                ),
            );
            gatherer.add(
                proxies[1].clone(),
                SamplerDesc::new_with_tile_modes(
                    &img_data.sampling_uv,
                    uv_tile_modes,
                    ImmutableSamplerInfo::default(),
                ),
            );
            gatherer.add(
                proxies[2].clone(),
                SamplerDesc::new_with_tile_modes(
                    &img_data.sampling_uv,
                    uv_tile_modes,
                    ImmutableSamplerInfo::default(),
                ),
            );
            gatherer.add(
                proxies[3].clone(),
                SamplerDesc::new_with_tile_modes(
                    &img_data.sampling,
                    y_alpha_tile_modes,
                    ImmutableSamplerInfo::default(),
                ),
            );
        }

        if do_tiling_in_hw && no_swizzle {
            add_hw_yuv_no_swizzle_image_uniform_data(key_context, img_data);
            builder(key_context).add_block(BuiltInCodeSnippetID::HWYUVNoSwizzleImageShader);
        } else if do_tiling_in_hw {
            add_hw_yuv_image_uniform_data(key_context, img_data);
            builder(key_context).add_block(BuiltInCodeSnippetID::HWYUVImageShader);
        } else if img_data.sampling.use_cubic {
            add_cubic_yuv_image_uniform_data(key_context, img_data);
            builder(key_context).add_block(BuiltInCodeSnippetID::CubicYUVImageShader);
        } else {
            add_yuv_image_uniform_data(key_context, img_data);
            builder(key_context).add_block(BuiltInCodeSnippetID::YUVImageShader);
        }
    }
}

// ==================================================================
// Dither and perlin noise
// ==================================================================

/// The data of a dither block (`DitherShaderBlock::DitherData`).
// Port of: src/gpu/graphite/KeyHelpers.h#L227-L236 (chrome/m156)
#[derive(Clone, Debug)]
pub struct DitherData {
    /// `fRange`.
    pub range: f32,
    /// `fLUTProxy`. Bound through the gatherer.
    pub lut_proxy: Option<Arc<TextureProxy>>,
}

impl DitherData {
    /// Wraps `range` and the look-up table (`DitherData(float, sk_sp<TextureProxy>)`).
    // Port of: src/gpu/graphite/KeyHelpers.h#L229-L231 (chrome/m156)
    #[must_use]
    pub fn new(range: f32, lut_proxy: Option<Arc<TextureProxy>>) -> Self {
        Self { range, lut_proxy }
    }
}

/// Adds the dither block (`DitherShaderBlock`).
// Port of: src/gpu/graphite/KeyHelpers.h#L238-L240 (chrome/m156)
#[derive(Debug)]
pub struct DitherShaderBlock;

impl DitherShaderBlock {
    /// `DitherShaderBlock::AddBlock`. The look-up table is bound through the gatherer.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L890-L898 (chrome/m156)
    #[doc(alias = "AddBlock")]
    pub fn add_block(key_context: &KeyContext<'_>, data: &DitherData) {
        let mut scope = ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::DitherShader);
        scope.uniforms().write_half(data.range);

        debug_assert!(data.lut_proxy.is_some() || key_context.recorder().is_none());
        scope.add_texture(
            data.lut_proxy.clone(),
            SamplerDesc::new(
                &SamplingOptions::from(FilterMode::Nearest),
                TileMode::Repeat,
            ),
        );
        drop(scope);

        builder(key_context).add_block(BuiltInCodeSnippetID::DitherShader);
    }
}

/// The dither look-up table, made once: Skia's `static const SkBitmap gLUT`. Its pixel ref is
/// the same for every draw, so the recorder's proxy cache holds a single texture for it.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2727 (chrome/m156)
static DITHER_LUT: LazyLock<Bitmap> = LazyLock::new(make_dither_lut);

/// `AddDitherBlock`: the dither block of a draw whose target has `color_type`. With a recorder
/// that cannot make the look-up table, the input color passes through (`kPriorOutput`).
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2726-L2740 (chrome/m156)
#[doc(alias = "AddDitherBlock")]
pub fn add_dither_block(key_context: &KeyContext<'_>, color_type: ColorType) {
    let proxy = RecorderPriv::create_cached_proxy(key_context.recorder(), &DITHER_LUT, "DitherLUT");
    if key_context.recorder().is_some() && proxy.is_none() {
        skia_log_w!("Couldn't create dither shader's LUT");
        builder(key_context).add_block(BuiltInCodeSnippetID::PriorOutput);
        return;
    }

    let data = DitherData::new(dither_range_for_config(color_type), proxy);
    DitherShaderBlock::add_block(key_context, &data);
}

/// The kind of noise (`PerlinNoiseShaderBlock::Type`). The values match `SkPerlinNoiseShaderType`.
// Port of: src/gpu/graphite/KeyHelpers.h#L242-L245 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum PerlinNoiseType {
    /// `kFractalNoise`.
    FractalNoise = 0,
    /// `kTurbulence`.
    Turbulence = 1,
}

/// The data of a perlin noise block (`PerlinNoiseShaderBlock::PerlinNoiseData`).
// Port of: src/gpu/graphite/KeyHelpers.h#L247-L275 (chrome/m156)
#[derive(Clone, Debug)]
pub struct PerlinNoiseData {
    /// `fType`.
    pub r#type: PerlinNoiseType,
    /// `fBaseFrequency`.
    pub base_frequency: Point,
    /// `fNumOctaves`.
    pub num_octaves: i32,
    /// `fStitchData`: the stitch tile size as floats.
    pub stitch_data: Point,
    /// `fPermutationsProxy`. Bound through the gatherer.
    pub permutations_proxy: Option<Arc<TextureProxy>>,
    /// `fNoiseProxy`. Bound through the gatherer.
    pub noise_proxy: Option<Arc<TextureProxy>>,
}

impl PerlinNoiseData {
    /// `PerlinNoiseData(type, baseFrequency, numOctaves, stitchData)`.
    // Port of: src/gpu/graphite/KeyHelpers.h#L255-L262 (chrome/m156)
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // the stitch tile size is a small integer
    pub fn new(
        r#type: PerlinNoiseType,
        base_frequency: Point,
        num_octaves: i32,
        stitch_data: ISize,
    ) -> Self {
        Self {
            r#type,
            base_frequency,
            num_octaves,
            stitch_data: Point::new(stitch_data.width as f32, stitch_data.height as f32),
            permutations_proxy: None,
            noise_proxy: None,
        }
    }

    /// Whether the noise tiles (`stitching()`).
    // Port of: src/gpu/graphite/KeyHelpers.h#L264 (chrome/m156)
    #[must_use]
    pub fn stitching(&self) -> bool {
        !self.stitch_data.is_zero()
    }
}

/// Adds the perlin noise block (`PerlinNoiseShaderBlock`).
// Port of: src/gpu/graphite/KeyHelpers.h#L276-L278 (chrome/m156)
#[derive(Debug)]
pub struct PerlinNoiseShaderBlock;

impl PerlinNoiseShaderBlock {
    /// `PerlinNoiseShaderBlock::AddBlock`. The two tables are bound through the gatherer.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L902-L928 (chrome/m156)
    #[doc(alias = "AddBlock")]
    pub fn add_block(key_context: &KeyContext<'_>, noise_data: &PerlinNoiseData) {
        let mut scope =
            ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::PerlinNoiseShader);

        scope
            .uniforms()
            .write_vec([noise_data.base_frequency.x, noise_data.base_frequency.y]);
        scope
            .uniforms()
            .write_vec([noise_data.stitch_data.x, noise_data.stitch_data.y]);
        scope.uniforms().write_i32(noise_data.r#type as i32);
        scope.uniforms().write_i32(noise_data.num_octaves);
        scope
            .uniforms()
            .write_i32(i32::from(noise_data.stitching()));

        // Both tables repeat in x and clamp in y.
        let repeat_x_tile_modes = (TileMode::Repeat, TileMode::Clamp);
        scope.add_texture(
            noise_data.permutations_proxy.clone(),
            SamplerDesc::new_with_tile_modes(
                &SamplingOptions::from(FilterMode::Nearest),
                repeat_x_tile_modes,
                ImmutableSamplerInfo::default(),
            ),
        );
        scope.add_texture(
            noise_data.noise_proxy.clone(),
            SamplerDesc::new_with_tile_modes(
                &SamplingOptions::from(FilterMode::Nearest),
                repeat_x_tile_modes,
                ImmutableSamplerInfo::default(),
            ),
        );
        drop(scope);

        builder(key_context).add_block(BuiltInCodeSnippetID::PerlinNoiseShader);
    }
}

// ==================================================================
// AddToKey for the core shaders served by the blocks above
// ==================================================================

// `map_color`: the color's transform from its source color space to the destination.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1704-L1712 (chrome/m156)
fn map_color(
    c: &Color4f,
    src: Option<&ColorSpace>,
    dst: Option<&ColorSpace>,
    dst_alpha_type: AlphaType,
) -> PMColor4f {
    let mut color = [c.r, c.g, c.b, c.a];
    ColorSpaceXformSteps::new(src, AlphaType::Unpremul, dst, dst_alpha_type).apply(&mut color);
    PMColor4f {
        r: color[0],
        g: color[1],
        b: color[2],
        a: color[3],
    }
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1911-L1919 (chrome/m156)
fn add_color_shader_to_key(key_context: &KeyContext<'_>, shader: &ColorShader) {
    let srgb = ColorSpace::new_srgb();
    let dst = key_context.dst_color_info();
    let color = map_color(
        &shader.color(),
        Some(&srgb),
        dst.color_space_ref(),
        dst.alpha_type(),
    );
    SolidColorShaderBlock::add_block(key_context, &color);
}

// `add_local_matrix_to_key`: wraps the inner key in a local matrix block.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1881-L1898 (chrome/m156)
fn add_local_matrix_to_key(
    key_context: &KeyContext<'_>,
    local_matrix: &Matrix,
    post_inverse_matrix: &Matrix,
    add_inner_to_key: impl FnOnce(&KeyContext<'_>),
) {
    let Some(mut lm_inverse) = local_matrix.invert() else {
        builder(key_context).add_error_block();
        return;
    };

    lm_inverse.post_concat(post_inverse_matrix);

    LocalMatrixShaderBlock::begin_block(key_context, &LMShaderData::new(lm_inverse));
    let lm_context = key_context.with_local_matrix(local_matrix);
    add_inner_to_key(&lm_context);
    builder(key_context).end_block();
}

/// `get_image_origin_matrix(image)`: the flip a Graphite-backed image with a bottom-left origin
/// needs; the identity otherwise. YUVA images carry their own origin matrix, which is not ported
/// (G15) and is treated as the identity here.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2238-L2262 (chrome/m156)
fn get_image_origin_matrix(image: &Image) -> Matrix {
    // If the image is not graphite backed then we can assume the origin will be TopLeft as we
    // require that in the ImageProvider utility.
    if image.as_base().is_graphite_backed()
        && let Some(graphite) = GraphiteImage::from_core(image)
    {
        let view = graphite.texture_proxy_view();
        if view.origin() == Origin::BottomLeft {
            // Pixel heights are far below 2^24, so the conversion is exact.
            #[allow(clippy::cast_precision_loss)]
            let height = view.height() as f32;
            return Matrix::scale_translate((1.0, -1.0), (0.0, height));
        }
    }
    // Otherwise no modification required
    Matrix::default()
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L2283-L2303 (chrome/m156)
fn add_local_matrix_shader_to_key(key_context: &KeyContext<'_>, shader: &LocalMatrixShader) {
    let wrapped = shader.wrapped_shader();
    let wrapped_base = wrapped.as_base();
    let xtra_matrix = match wrapped_base.shader_type() {
        ShaderType::Image => {
            let Some(image_shader) = downcast_shader::<ImageShader>(wrapped_base) else {
                builder(key_context).add_error_block();
                return;
            };
            get_image_origin_matrix(image_shader.image())
        }
        ShaderType::GradientBase => {
            let Some(gradient_matrix) = get_gradient_matrix(wrapped_base) else {
                builder(key_context).add_error_block();
                return;
            };
            gradient_matrix
        }
        _ => Matrix::default(),
    };

    add_local_matrix_to_key(
        key_context,
        shader.local_matrix(),
        &xtra_matrix,
        |child_ctx| add_to_key_shader(child_ctx, Some(wrapped)),
    );
}

/// The `SkGradientBaseShader` of a gradient shader, whichever subclass it is.
fn gradient_base_of(base: &dyn ShaderBase) -> Option<&GradientBaseShader> {
    if let Some(s) = downcast_shader::<LinearGradient>(base) {
        return Some(s.base());
    }
    if let Some(s) = downcast_shader::<RadialGradient>(base) {
        return Some(s.base());
    }
    if let Some(s) = downcast_shader::<SweepGradient>(base) {
        return Some(s.base());
    }
    downcast_shader::<ConicalGradient>(base).map(ConicalGradient::base)
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L2261-L2281 (chrome/m156), `get_gradient_matrix`
// Conical gradients override the matrix, since Graphite uses a different algorithm than the
// raster and Ganesh backends. `None` if the conical's centers have no unit mapping.
fn get_gradient_matrix(base: &dyn ShaderBase) -> Option<Matrix> {
    if base.as_gradient(None, None) == GradientType::Conical {
        let conical = downcast_shader::<ConicalGradient>(base)?;
        if conical.get_type() == ConicalType::Radial {
            let mut conical_matrix = Matrix::translate(-conical.get_start_center());
            let scale = ieee_float_divide(1.0, conical.get_diff_radius());
            conical_matrix.post_scale((scale, scale), None);
            Some(conical_matrix)
        } else {
            ConicalGradient::map_to_unit_x(conical.get_start_center(), conical.get_end_center())
        }
    } else {
        // Use the standard gradient matrix for other types.
        Some(gradient_base_of(base)?.gradient_matrix().clone())
    }
}

// Please see GrGradientShader.cpp::make_interpolated_to_dst for substantial comments as to why this
// code is structured this way.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2481-L2524 (chrome/m156), `make_interpolated_to_dst`
fn make_interpolated_to_dst(
    key_context: &KeyContext<'_>,
    grad_data: &GradientData,
    interp: Interpolation,
    intermediate_cs: Option<&ColorSpace>,
) {
    use InterpolationColorSpace as CS;

    let mut input_premul = interp.in_premul == InPremul::Yes;
    if matches!(
        interp.color_space,
        CS::Lab
            | CS::OKLab
            | CS::OKLabGamutMap
            | CS::LCH
            | CS::OKLCH
            | CS::OKLCHGamutMap
            | CS::HSL
            | CS::HWB
    ) {
        input_premul = false;
    }

    let dst_color_info = key_context.dst_color_info();
    let dst_color_space = dst_color_info
        .color_space_ref()
        .unwrap_or_else(|| srgb_singleton());
    let intermediate_alpha_type = if input_premul {
        AlphaType::Premul
    } else {
        AlphaType::Unpremul
    };

    let data = ColorSpaceTransformData::from_color_spaces(
        intermediate_cs,
        intermediate_alpha_type,
        Some(dst_color_space),
        dst_color_info.alpha_type(),
    );

    // The gradient block and colorSpace conversion block need to be combined (via the Compose
    // block) so that the localMatrix block can treat them as one child.
    compose(
        key_context,
        || GradientShaderBlocks::add_block(key_context, grad_data),
        || ColorSpaceTransformBlock::add_block(key_context, &data),
    );
}

/// The shared gradient key: the stops transformed for the destination, uploaded inline or, with
/// more than [`GradientData::NUM_INTERNAL_STORAGE_STOPS`] stops and no storage buffers, as a
/// cached color-and-offset texture.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2526-L2584 (chrome/m156), `add_gradient_to_key`
#[allow(clippy::too_many_arguments)] // mirrors the C++ helper
fn add_gradient_to_key(
    key_context: &KeyContext<'_>,
    shader: &GradientBaseShader,
    gradient_type: GradientType,
    point0: Point,
    point1: Point,
    radius0: f32,
    radius1: f32,
    bias: f32,
    scale: f32,
) {
    let xformed_colors =
        Color4fXformer::new(shader, key_context.dst_color_info().color_space_ref());
    let colors = &xformed_colors.colors;
    let positions = xformed_colors.positions.as_deref();
    let color_count = colors.len();

    let mut proxy: Option<Arc<TextureProxy>> = None;

    let grad_uses_storage = key_context.caps().storage_buffer_support();
    if color_count > GradientData::NUM_INTERNAL_STORAGE_STOPS && !grad_uses_storage {
        if shader.cached_bitmap().is_none() {
            let colors_and_offsets_bitmap = create_gradient_color_and_offset_bitmap(
                i32::try_from(color_count).expect("gradient color count fits an int"),
                colors,
                positions,
            );
            if colors_and_offsets_bitmap.is_empty() {
                skia_log_w!("Couldn't create GradientShader's color and offset bitmap");
                builder(key_context).add_error_block();
                return;
            }
            shader.set_cached_bitmap(colors_and_offsets_bitmap);
        }

        let Some(cached) = shader.cached_bitmap() else {
            builder(key_context).add_error_block();
            return;
        };
        proxy =
            RecorderPriv::create_cached_proxy(key_context.recorder(), cached, "GradientTexture");
        if proxy.is_none() {
            skia_log_w!("Couldn't create GradientShader's color and offset bitmap proxy");
            builder(key_context).add_error_block();
            return;
        }
    }

    let data = GradientData::new(
        gradient_type,
        point0,
        point1,
        radius0,
        radius1,
        bias,
        scale,
        shader.tile_mode(),
        color_count,
        colors,
        positions,
        shader,
        proxy,
        grad_uses_storage,
        *shader.interpolation(),
    );

    make_interpolated_to_dst(
        key_context,
        &data,
        *shader.interpolation(),
        xformed_colors.intermediate_color_space.as_ref(),
    );
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L2586-L2609 (chrome/m156), `add_gradient_to_key` for
// conical gradients: the radii are scaled to the unit mapping of the centers.
fn add_conical_gradient_to_key(key_context: &KeyContext<'_>, shader: &ConicalGradient) {
    let mut r0 = shader.get_start_radius();
    let mut r1 = shader.get_end_radius();

    if shader.get_type() == ConicalType::Radial {
        r0 /= shader.get_diff_radius();
        r1 /= shader.get_diff_radius();
    } else {
        // Since we map the centers to be (0,0) and (1,0) in the gradient matrix, there is a
        // scale of 1/distance-between-centers that has to be applied to the radii.
        r0 /= shader.get_center_x1();
        r1 /= shader.get_center_x1();
    }

    add_gradient_to_key(
        key_context,
        shader.base(),
        GradientType::Conical,
        shader.get_start_center(),
        shader.get_end_center(),
        r0,
        r1,
        0.0,
        0.0,
    );
}

/// `AddToKey` for a gradient shader, dispatched on its subclass (`add_to_key(SkGradientBaseShader*)`).
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2647-L2663 (chrome/m156), and the `add_gradient_to_key`
// overloads for linear, radial and sweep gradients (#L2611-L2645)
fn add_gradient_base_shader_to_key(key_context: &KeyContext<'_>, base: &dyn ShaderBase) {
    if let Some(s) = downcast_shader::<ConicalGradient>(base) {
        add_conical_gradient_to_key(key_context, s);
    } else if let Some(s) = downcast_shader::<LinearGradient>(base) {
        add_gradient_to_key(
            key_context,
            s.base(),
            GradientType::Linear,
            s.start(),
            s.end(),
            0.0,
            0.0,
            0.0,
            0.0,
        );
    } else if let Some(s) = downcast_shader::<RadialGradient>(base) {
        add_gradient_to_key(
            key_context,
            s.base(),
            GradientType::Radial,
            s.center(),
            Point::new(0.0, 0.0),
            s.radius(),
            0.0,
            0.0,
            0.0,
        );
    } else if let Some(s) = downcast_shader::<SweepGradient>(base) {
        add_gradient_to_key(
            key_context,
            s.base(),
            GradientType::Sweep,
            s.center(),
            Point::new(0.0, 0.0),
            0.0,
            0.0,
            s.t_bias(),
            s.t_scale(),
        );
    } else {
        // SkDEBUGFAIL: a gradient shader that is none of the four subclasses.
        builder(key_context).add_error_block();
    }
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1865-L1877 (chrome/m156), the blend shader's
// `Blend` composition of its src and dst children under its blend mode.
fn add_blend_shader_to_key(key_context: &KeyContext<'_>, shader: &BlendShader) {
    blend(
        key_context,
        || add_blend_mode(key_context, shader.mode()),
        || add_to_key_shader(key_context, Some(shader.src())),
        || add_to_key_shader(key_context, Some(shader.dst())),
    );
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1900-L1909 (chrome/m156), `add_to_key(SkCTMShader*)`
fn add_ctm_shader_to_key(key_context: &KeyContext<'_>, shader: &CtmShader) {
    // CTM shaders are always given device coordinates, so we don't have to modify the CTM itself
    // with keyContext's local transform.
    add_local_matrix_to_key(key_context, shader.ctm(), &Matrix::default(), |child_ctx| {
        add_to_key_shader(child_ctx, Some(shader.proxy_shader()));
    });
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1921-L1931 (chrome/m156), `add_to_key(SkColorFilterShader*)`
fn add_color_filter_shader_to_key(key_context: &KeyContext<'_>, shader: &ColorFilterShader) {
    compose(
        key_context,
        || add_to_key_shader(key_context, Some(shader.shader())),
        || add_to_key_color_filter(key_context, Some(shader.filter())),
    );
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1933-L1946 (chrome/m156), `add_to_key(SkCoordClampShader*)`
fn add_coord_clamp_shader_to_key(key_context: &KeyContext<'_>, shader: &CoordClampShader) {
    CoordClampShaderBlock::begin_block(key_context, &CoordClampData::new(shader.subset()));

    // Subtleties in clamping implementation can lead to texture samples at non pixel aligned
    // coordinates, particularly if clamped to non-texel centers.
    let child_ctx = key_context.with_extra_flags(KeyGenFlags::DISABLE_SAMPLING_OPTIMIZATION);
    add_to_key_shader(&child_ctx, Some(shader.shader()));

    builder(key_context).end_block();
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L2312-L2345 (chrome/m156), `add_to_key(SkPerlinNoiseShader*)`
fn add_perlin_noise_shader_to_key(key_context: &KeyContext<'_>, shader: &PerlinNoiseShader) {
    debug_assert!(shader.num_octaves() != 0);

    let tables = shader.painting_tables();
    let recorder = key_context.recorder();
    let permutations =
        RecorderPriv::create_cached_proxy(recorder, &tables.permutations, "PerlinNoisePermTable");
    let noise = RecorderPriv::create_cached_proxy(recorder, &tables.noise, "PerlinNoiseNoiseTable");

    if permutations.is_none() || noise.is_none() {
        skia_log_w!("Couldn't create tables for PerlinNoiseShader");
        builder(key_context).add_error_block();
        return;
    }

    let noise_type = match shader.noise_type() {
        PerlinNoiseShaderType::FractalNoise => PerlinNoiseType::FractalNoise,
        PerlinNoiseShaderType::Turbulence => PerlinNoiseType::Turbulence,
    };
    let mut perlin_data = PerlinNoiseData::new(
        noise_type,
        tables.base_frequency,
        shader.num_octaves(),
        tables.stitch_data_init,
    );
    perlin_data.permutations_proxy = permutations;
    perlin_data.noise_proxy = noise;

    PerlinNoiseShaderBlock::add_block(key_context, &perlin_data);
}

/// `add_to_key(SkRuntimeShader*)`: a runtime effect's uniforms (transformed to the dst color
/// space) and its children, between the runtime effect's begin and end blocks. A runtime effect
/// that cannot be keyed becomes its no-op stand-in.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2415-L2434 (chrome/m156)
fn add_runtime_shader_to_key(key_context: &KeyContext<'_>, shader: &RuntimeShader) {
    let effect = shader.effect();
    let dst_cs = key_context.dst_color_info().color_space_ref();
    let uniforms = runtime_effect_priv::transform_uniforms(
        effect.uniforms(),
        &shader.uniform_data(dst_cs),
        dst_cs,
    );

    let shader_data = RuntimeEffectShaderData {
        effect: effect.clone(),
        uniforms: Some(uniforms),
    };
    if !RuntimeEffectBlock::begin_block(key_context, &shader_data) {
        RuntimeEffectBlock::add_no_op_effect(key_context, effect);
        return;
    }

    add_children_to_key(key_context, shader.children(), effect);

    builder(key_context).end_block();
}

/// `add_image_to_key(keyContext, image, subset, sampling, tileModeX, tileModeY, isRaw)`: the
/// image shader's block, with the image converted to a Graphite-backed one first. A draw whose
/// image cannot be converted adds an error block (the draw is dropped).
///
/// The YUVA branch (`add_yuv_image_to_key`) is not ported (G15): a YUVA image adds an error block.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2106-L2236 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn add_image_to_key(
    key_context: &KeyContext<'_>,
    image: &Image,
    mut subset: Rect,
    sampling: SamplingOptions,
    tile_mode_x: TileMode,
    tile_mode_y: TileMode,
    is_raw: bool,
) {
    let Some(recorder) = key_context.recorder() else {
        builder(key_context).add_error_block();
        return;
    };
    let (image_to_draw, mut new_sampling) = get_graphite_backed(recorder, image, sampling);
    let Some(image_to_draw) = image_to_draw else {
        skia_log_w!("Couldn't convert SkImage to a Graphite-backed representation");
        builder(key_context).add_error_block();
        return;
    };

    // We must call notifyInUse() here to link the final, Graphite-backed 'imageToDraw'
    // to the DrawContext that will sample it.
    //
    // This is necessary for two primary cases:
    // 1. The original image was not Graphite-backed.
    // 2. The original image was already Graphite-backed, but produced through Image::Copy,
    //    possibly from a different DrawContext.
    //
    // skia-rust: the key context records the image and the device that draws notifies it once
    // the key is built, since it holds the `DrawContext` mutably (`docs/design/gpu.md` §5.6).
    debug_assert!(image_to_draw.as_base().is_graphite_backed());
    key_context.notify_in_use(image_to_draw.clone());

    // Here we detect pixel aligned blit-like image draws. Some devices have low precision filtering
    // and will produce degraded (blurry) images unexpectedly for sequential exact pixel blits when
    // not using nearest filtering. This is common for canvas scrolling implementations. Forcing
    // nearest filtering when possible can also be a minor perf/power optimization depending on the
    // hardware.
    if !(key_context
        .flags()
        .contains(KeyGenFlags::DISABLE_SAMPLING_OPTIMIZATION)
        || new_sampling.use_cubic)
    {
        let mut total_m = key_context.local2dev().to_m33();
        if let Some(local_matrix) = key_context.local_matrix() {
            total_m.pre_concat(local_matrix);
        }
        total_m.normalize_perspective();
        // The matrix should be translation with only pixel aligned 2d translation.
        let sampling_has_no_effect = total_m.is_translate()
            && scalar_is_int(total_m.translate_x())
            && scalar_is_int(total_m.translate_y());
        if sampling_has_no_effect {
            new_sampling = SamplingOptions::from(FilterMode::Nearest);
        }

        if sampling_has_no_effect && !key_context.clip_draw_bounds().is_empty() {
            let mut local_draw_bounds = *key_context.clip_draw_bounds();
            local_draw_bounds.offset((-total_m.translate_x(), -total_m.translate_y()));
            if subset.contains(&local_draw_bounds) {
                // The draw is strictly within the subset, so we don't need to clamp.
                subset = Rect::from_size(image_to_draw.dimensions());
            }
        }
    }

    if image_to_draw.as_base().is_yuva() {
        // add_yuv_image_to_key is G15.
        builder(key_context).add_error_block();
        return;
    }

    // `AsView`: a non-YUVA Graphite-backed image always has a texture view.
    let view = as_view(Some(&image_to_draw));
    if view.proxy().is_none() {
        builder(key_context).add_error_block();
        return;
    }
    let mut img_data = ImageData::new(
        new_sampling,
        tile_mode_x,
        tile_mode_y,
        view.dimensions(),
        subset,
        ImmutableSamplerInfo::default(),
    );
    img_data.texture_proxy = view.ref_proxy();
    let read_swizzle = view.swizzle();
    let mut color_xform_data = ColorSpaceTransformData::from_steps(ColorSpaceXformSteps::default());
    color_xform_data.read_swizzle = read_swizzle;
    color_xform_data.is_alpha_only = image_to_draw.is_alpha_only();

    if !is_raw {
        let dst = key_context.dst_color_info();
        color_xform_data.steps = ColorSpaceXformSteps::new(
            image_to_draw.color_space().as_ref(),
            image_to_draw.alpha_type(),
            dst.color_space_ref(),
            dst.alpha_type(),
        );

        if image_to_draw.is_alpha_only()
            && !key_context
                .flags()
                .contains(KeyGenFlags::DISABLE_ALPHA_ONLY_IMAGE_COLORIZATION)
        {
            // NOTE: Alpha is not affected by colorspace conversion to the dst, and the paint color
            // is already xformed to the dst, but the ColorSpaceTransformBlock is necessary to apply
            // any read swizzle, which is often necessary for alpha-only color types.
            blend(
                key_context,
                || add_fixed_blend_mode(key_context, BlendMode::DstIn),
                || {
                    compose(
                        key_context,
                        || ImageShaderBlock::add_block(key_context, &img_data),
                        || ColorSpaceTransformBlock::add_block(key_context, &color_xform_data),
                    );
                },
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

/// `AddToKey(PaintParams::SimpleImage)`: an image shader on a paint is always a local matrix
/// shader composed with an image shader; this makes the same call sequence with the decomposed
/// objects.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2686-L2701 (chrome/m156)
#[doc(alias = "AddToKey")]
pub fn add_simple_image_to_key(
    key_context: &KeyContext<'_>,
    simple_image: &crate::graphite::paint_params::SimpleImage,
) {
    add_local_matrix_to_key(
        key_context,
        simple_image.local_matrix.as_ref().unwrap_or(Matrix::i()),
        &get_image_origin_matrix(&simple_image.image),
        |child_ctx| {
            add_image_to_key(
                child_ctx,
                &simple_image.image,
                simple_image.subset,
                simple_image.sampling_options,
                TileMode::Clamp,
                TileMode::Clamp,
                /* is_raw= */ false,
            );
        },
    );
}

/// `AddToKey(SkImageShader)`: the image shader's fields go to `add_image_to_key`.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2232-L2236 (chrome/m156)
fn add_image_shader_to_key(key_context: &KeyContext<'_>, shader: &ImageShader) {
    add_image_to_key(
        key_context,
        shader.image(),
        shader.subset(),
        shader.sampling(),
        shader.tile_mode_x(),
        shader.tile_mode_y(),
        shader.is_raw(),
    );
}

/// Adds the implementation of `shader` to `key_context`'s key (`AddToKey(SkShader)`). A `None`
/// shader is a programming error: a fixed transparent solid color keeps the key's structure.
///
/// Shaders whose blocks are not ported add an error block.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2665-L2684 (chrome/m156), `AddToKey(SkShader)`
#[doc(alias = "AddToKey")]
pub fn add_to_key_shader(key_context: &KeyContext<'_>, shader: Option<&Shader>) {
    let Some(shader) = shader else {
        // Calling code assumes a block will be appended. Add a fixed block to preserve shader and
        // PaintParamsKey structure in release builds but assert since this should either not
        // happen or should be changing high-level logic within PaintParams::toKey().
        debug_assert!(false, "AddToKey called with a null shader");
        SolidColorShaderBlock::add_block(
            key_context,
            &PMColor4f {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
        );
        return;
    };

    let base = shader.as_base();
    match base.shader_type() {
        ShaderType::Color => match downcast_shader::<ColorShader>(base) {
            Some(s) => add_color_shader_to_key(key_context, s),
            None => builder(key_context).add_error_block(),
        },
        ShaderType::Empty => {
            debug_assert!(downcast_shader::<EmptyShader>(base).is_some());
            builder(key_context).add_block(BuiltInCodeSnippetID::PriorOutput);
        }
        ShaderType::LocalMatrix => match downcast_shader::<LocalMatrixShader>(base) {
            Some(s) => add_local_matrix_shader_to_key(key_context, s),
            None => builder(key_context).add_error_block(),
        },
        ShaderType::GradientBase => add_gradient_base_shader_to_key(key_context, base),
        ShaderType::PerlinNoise => match downcast_shader::<PerlinNoiseShader>(base) {
            Some(s) => add_perlin_noise_shader_to_key(key_context, s),
            None => builder(key_context).add_error_block(),
        },
        ShaderType::Runtime => match downcast_shader::<RuntimeShader>(base) {
            Some(s) => add_runtime_shader_to_key(key_context, s),
            None => builder(key_context).add_error_block(),
        },
        ShaderType::Blend => match downcast_shader::<BlendShader>(base) {
            Some(s) => add_blend_shader_to_key(key_context, s),
            None => builder(key_context).add_error_block(),
        },
        ShaderType::ColorFilter => match downcast_shader::<ColorFilterShader>(base) {
            Some(s) => add_color_filter_shader_to_key(key_context, s),
            None => builder(key_context).add_error_block(),
        },
        ShaderType::CoordClamp => match downcast_shader::<CoordClampShader>(base) {
            Some(s) => add_coord_clamp_shader_to_key(key_context, s),
            None => builder(key_context).add_error_block(),
        },
        ShaderType::CTM => match downcast_shader::<CtmShader>(base) {
            Some(s) => add_ctm_shader_to_key(key_context, s),
            None => builder(key_context).add_error_block(),
        },
        ShaderType::Image => match downcast_shader::<ImageShader>(base) {
            Some(s) => add_image_shader_to_key(key_context, s),
            None => builder(key_context).add_error_block(),
        },
        // Not ported yet: the YUV, picture and dictionary-less shaders (see the module docs).
        _ => builder(key_context).add_error_block(),
    }
}
