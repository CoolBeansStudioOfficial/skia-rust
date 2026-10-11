// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkShaderImageFilter.cpp

//! `SkImageFilters::Shader`: a leaf filter whose output is an (infinite) shader.

use skia_rust_core::flattenable::FlattenableRegistry;
use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter::{flatten_common, unflatten_common};
use skia_rust_core::image_filter_result::FilterResult;
use skia_rust_core::image_filter_types::{Context, Mapping, MatrixCapability};
use skia_rust_core::picture_priv;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::{IRect, Rect, rect_priv};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::write_buffer::BinaryWriteBuffer;

use skia_rust_core::tile_mode::TileMode;

use crate::image_filters::crop_filter::{crop, empty};

/// `SkImageFilters::Dither`: whether the shader output is dithered.
// Port of: include/effects/SkImageFilters.h (chrome/m156), SkImageFilters::Dither
#[doc(alias = "SkImageFilters::Dither")]
#[doc(alias = "SkImageFilters_Dither")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Dither {
    /// `kNo`.
    #[default]
    No,
    /// `kYes`.
    Yes,
}

/// The shader filter (`SkShaderImageFilter`).
// Port of: src/effects/imagefilters/SkShaderImageFilter.cpp#L29-L66 (chrome/m156)
#[doc(alias = "SkShaderImageFilter")]
#[derive(Debug)]
pub struct ShaderImageFilter {
    common: ImageFilterCommon,
    shader: Shader,
    dither: Dither,
}

impl ShaderImageFilter {
    /// `SkShaderImageFilter(shader, dither)`.
    // Port of: src/effects/imagefilters/SkShaderImageFilter.cpp#L31-L36 (chrome/m156)
    #[must_use]
    pub fn new(shader: Shader, dither: Dither) -> Self {
        ShaderImageFilter {
            common: ImageFilterCommon::new(Vec::new(), None),
            shader,
            dither,
        }
    }
}

impl ImageFilterBase for ShaderImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkShaderImageFilter.cpp#L47 (chrome/m156), SK_FLATTENABLE_HOOKS
    fn type_name(&self) -> &'static str {
        "SkShaderImageFilter"
    }

    // Port of: src/effects/imagefilters/SkShaderImageFilter.cpp#L128-L133 (chrome/m156), flatten
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        flatten_common(&self.common, buffer);
        buffer.write_shader(Some(&self.shader));
        buffer.write_bool(self.dither == Dither::Yes);
    }

    // Port of: src/effects/imagefilters/SkShaderImageFilter.cpp#L119-L122 (chrome/m156)
    fn on_filter_image(&self, ctx: &Context<'_>) -> FilterResult {
        let dither = self.dither == Dither::Yes;
        FilterResult::make_from_shader(ctx, self.shader.clone(), dither)
    }

    // Port of: src/effects/imagefilters/SkShaderImageFilter.cpp#L124-L130 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        _mapping: &Mapping,
        _desired_output: IRect,
        _content_bounds: Option<IRect>,
    ) -> IRect {
        // This is a leaf filter, it requires no input and no further recursion
        IRect::new_empty()
    }

    // Port of: src/effects/imagefilters/SkShaderImageFilter.cpp#L132-L138 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        _mapping: &Mapping,
        _content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        // The output of a shader is infinite, unless we were to inspect the shader for a decal
        // tile mode around a gradient or image.
        None
    }

    // Port of: src/effects/imagefilters/SkShaderImageFilter.cpp#L38-L40 (chrome/m156)
    fn compute_fast_bounds(&self, _bounds: &Rect) -> Rect {
        rect_priv::make_large_s32()
    }

    // Port of: src/effects/imagefilters/SkShaderImageFilter.cpp#L49 (chrome/m156)
    fn on_affects_transparent_black(&self) -> bool {
        true
    }

    // Port of: src/effects/imagefilters/SkShaderImageFilter.cpp#L51 (chrome/m156)
    fn on_get_ctm_capability(&self) -> MatrixCapability {
        MatrixCapability::Complex
    }
}

/// `SkImageFilters::Shader(shader, dither, cropRect)`: the empty filter if `shader` is `None`,
/// cropped to `crop_rect` if given.
// Port of: src/effects/imagefilters/SkShaderImageFilter.cpp#L70-L82 (chrome/m156)
#[doc(alias = "Shader")]
#[must_use]
pub fn shader(
    shader: Option<Shader>,
    dither: Dither,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    let Some(shader) = shader else {
        return Some(empty());
    };
    let filter = Some(ImageFilter::from_base(ShaderImageFilter::new(
        shader, dither,
    )));
    match crop_rect {
        Some(rect) => crop(&rect, TileMode::Decal, filter),
        None => filter,
    }
}

/// `SkShaderImageFilter::CreateProc`.
// Port of: src/effects/imagefilters/SkShaderImageFilter.cpp#L89-L115 (chrome/m156)
pub fn create_proc(
    buffer: &mut ReadBuffer<'_>,
    registry: &FlattenableRegistry,
) -> Option<ImageFilter> {
    let (_, crop_rect) = unflatten_common(buffer, 0, registry)?;
    let (filter_shader, dither): (Option<Shader>, bool);
    if buffer.is_version_lt(picture_priv::VERSION_SHADER_IMAGE_FILTER_SERIALIZE_SHADER) {
        // The old implementation stored an entire SkPaint, but we only need the SkShader and dither
        // boolean.
        let paint = buffer.read_paint(registry);
        filter_shader = paint
            .shader()
            .or_else(|| shaders::color_in_space(paint.color4f(), None));
        dither = paint.is_dither();
    } else {
        filter_shader = buffer.read_shader(registry);
        dither = buffer.read_bool();
    }
    let dither = if dither { Dither::Yes } else { Dither::No };
    shader(filter_shader, dither, crop_rect)
}
