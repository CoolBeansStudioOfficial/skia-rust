// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/BitmapTextRenderStep.h,
// src/gpu/graphite/render/BitmapTextRenderStep.cpp

//! [`BitmapTextRenderStep`]: draws glyph masks (A8, LCD or color) from the text atlas, as a
//! four-vertex triangle strip per glyph instance.

use std::sync::Arc;

use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;

use crate::gpu::mask_format::MaskFormat;
use crate::graphite::attribute::{Attribute, Varying};
use crate::graphite::caps::ResourceBindingRequirements;
use crate::graphite::context_utils::emit_sampler_layout;
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{PrimitiveType, VertexAttribType};
use crate::graphite::draw_writer::DrawWriter;
use crate::graphite::paint_params_key::RootNodesInfo;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::common_depth_stencil_settings::DIRECT_DEPTH_LEQUAL_PASS;
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::{Layout, SamplerDesc};
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::uniform::Uniform;
use crate::sksl_type_shared::SkSLType;

/// We are expecting to sample from up to 4 textures.
// Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L43 (chrome/m156)
pub const NUM_TEXT_ATLAS_TEXTURES: usize = 4;

/// `variant_id(variant)`.
// Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L45-L53 (chrome/m156)
fn variant_id(variant: MaskFormat) -> RenderStepID {
    match variant {
        MaskFormat::A8 => RenderStepID::BitmapText_Mask,
        MaskFormat::A565 => RenderStepID::BitmapText_LCD,
        MaskFormat::Argb => RenderStepID::BitmapText_Color,
    }
}

/// `BitmapTextRenderStep::Flags(variant)`.
// Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L81-L93 (chrome/m156)
fn flags(variant: MaskFormat) -> RenderStepFlags {
    match variant {
        MaskFormat::A8 => {
            RenderStepFlags::PERFORMS_SHADING
                | RenderStepFlags::HAS_TEXTURES
                | RenderStepFlags::EMITS_COVERAGE
        }
        MaskFormat::A565 => {
            RenderStepFlags::PERFORMS_SHADING
                | RenderStepFlags::HAS_TEXTURES
                | RenderStepFlags::EMITS_COVERAGE
                | RenderStepFlags::LCD_COVERAGE
        }
        MaskFormat::Argb => {
            RenderStepFlags::PERFORMS_SHADING
                | RenderStepFlags::HAS_TEXTURES
                | RenderStepFlags::EMITS_PRIMITIVE_COLOR
        }
    }
}

const UNIFORMS: [Uniform; 3] = [
    Uniform::new("maskToDevice", SkSLType::Float4x4),
    Uniform::new("localToDevice", SkSLType::Float4x4),
    Uniform::new("atlasSizeInv", SkSLType::Float2),
];

/// The per-glyph instance attributes shared by the text steps.
// Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L66-L73 (chrome/m156)
pub(crate) const TEXT_APPEND_ATTRS: [Attribute; 7] = [
    Attribute::new("size", VertexAttribType::UShort2, SkSLType::UShort2),
    Attribute::new("uvPos", VertexAttribType::UShort2, SkSLType::UShort2),
    Attribute::new("xyPos", VertexAttribType::Float2, SkSLType::Float2),
    Attribute::new("indexAndFlags", VertexAttribType::UShort2, SkSLType::UShort2),
    Attribute::new(
        "strikeToSourceScale",
        VertexAttribType::Float,
        SkSLType::Float,
    ),
    Attribute::new("depth", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
];

/// The texture bindings of the text atlas: four samplers named `{prefix}_{i}`.
// Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L113-L123 (chrome/m156)
pub(crate) fn atlas_textures_and_samplers_sksl(
    prefix: &str,
    binding_reqs: &ResourceBindingRequirements,
    next_binding_index: &mut i32,
) -> String {
    let mut result = String::new();

    for i in 0..NUM_TEXT_ATLAS_TEXTURES {
        result += &emit_sampler_layout(binding_reqs, next_binding_index);
        result += &format!(" sampler2D {prefix}_{i};\n");
    }

    result
}

/// Adds the atlas textures to the gatherer with `filter` sampling: the active pages, then the
/// first one again for the samplers of the shader that have no active page.
// Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L193-L208 (chrome/m156)
pub(crate) fn add_atlas_textures(
    gatherer: &mut PipelineDataGatherer,
    proxies: &[Arc<TextureProxy>],
    filter: FilterMode,
) {
    let sampler = SamplerDesc::new(&SamplingOptions::from(filter), TileMode::Clamp);
    // write textures and samplers
    for proxy in proxies.iter().take(NUM_TEXT_ATLAS_TEXTURES) {
        gatherer.add(Some(Arc::clone(proxy)), sampler.clone());
    }
    // If the atlas has less than 4 active proxies we still need to set up samplers for the
    // shader.
    for _ in proxies.len()..NUM_TEXT_ATLAS_TEXTURES {
        gatherer.add(Some(Arc::clone(&proxies[0])), sampler.clone());
    }
}

/// `BitmapTextRenderStep`: draws the glyphs of an atlas sub run from the glyph atlas, with the
/// mask as coverage or, for color glyphs, as the primitive color.
// Port of: src/gpu/graphite/render/BitmapTextRenderStep.h#L22-L52 (chrome/m156)
#[doc(alias = "skgpu::graphite::BitmapTextRenderStep")]
#[derive(Debug)]
pub struct BitmapTextRenderStep {
    base: RenderStepBase,
}

impl BitmapTextRenderStep {
    /// `BitmapTextRenderStep(layout, variant)`.
    // Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L55-L79 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout, variant: MaskFormat) -> Self {
        let base = RenderStepBase::new(
            layout,
            variant_id(variant),
            flags(variant) | RenderStepFlags::APPEND_INSTANCES,
            &UNIFORMS,
            PrimitiveType::TriangleStrip,
            DIRECT_DEPTH_LEQUAL_PASS,
            &[],
            &TEXT_APPEND_ATTRS,
            &[],
            &[
                Varying::new(
                    "textureCoords",
                    SkSLType::Float2,
                    crate::graphite::attribute::Interpolation::Perspective,
                ),
                Varying::new(
                    "texIndex",
                    SkSLType::Half,
                    crate::graphite::attribute::Interpolation::Perspective,
                ),
                Varying::new(
                    "maskFormat",
                    SkSLType::Half,
                    crate::graphite::attribute::Interpolation::Perspective,
                ),
            ],
        );
        Self { base }
    }
}

impl RenderStep for BitmapTextRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L95-L111 (chrome/m156)
    fn vertex_sksl(&self, _roots: &RootNodesInfo) -> String {
        // Returns the body of a vertex function, which must define a float4 devPosition variable
        // and must write to an already-defined float2 stepLocalCoords variable.
        "texIndex = half(indexAndFlags.x);\
         maskFormat = half(indexAndFlags.y);\
         float2 unormTexCoords;\
         float4 devPosition = text_vertex_fn(float2(sk_VertexID >> 1, sk_VertexID & 1), \
                                             maskToDevice, \
                                             localToDevice, \
                                             atlasSizeInv, \
                                             float2(size), \
                                             float2(uvPos), \
                                             xyPos, \
                                             strikeToSourceScale, \
                                             depth, \
                                             textureCoords, \
                                             unormTexCoords, \
                                             stepLocalCoords);"
            .to_owned()
    }

    // Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L113-L123 (chrome/m156)
    fn textures_and_samplers_sksl(
        &self,
        binding_reqs: &ResourceBindingRequirements,
        next_binding_index: &mut i32,
    ) -> String {
        atlas_textures_and_samplers_sksl("text_atlas", binding_reqs, next_binding_index)
    }

    // Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L126-L137 (chrome/m156)
    fn fragment_color_sksl(&self, _roots: &RootNodesInfo) -> String {
        // The returned SkSL must write its color into a 'half4 primitiveColor' variable (defined
        // in the calling code).
        "primitiveColor = sample_indexed_atlas(textureCoords, \
                                               int(texIndex), \
                                               text_atlas_0, \
                                               text_atlas_1, \
                                               text_atlas_2, \
                                               text_atlas_3);"
            .to_owned()
    }

    // Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L139-L152 (chrome/m156)
    fn fragment_coverage_sksl(&self) -> &'static str {
        // The returned SkSL must write its coverage into a 'half4 outputCoverage' variable
        // (defined in the calling code) with the actual coverage splatted out into all four
        // channels.
        "outputCoverage = bitmap_text_coverage_fn(sample_indexed_atlas(textureCoords, \
                                                                       int(texIndex), \
                                                                       text_atlas_0, \
                                                                       text_atlas_1, \
                                                                       text_atlas_2, \
                                                                       text_atlas_3), \
                                                  int(maskFormat));"
    }

    // Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L154 (chrome/m156)
    fn uses_uniforms_in_fragment_sksl(&self) -> bool {
        false
    }

    // Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L156-L170 (chrome/m156)
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32) {
        let sub_run_data = params.geometry().sub_run_data();
        let sub_run = sub_run_data.sub_run();
        let backend = sub_run.glyph_vector().backend();
        let glyph_data = backend
            .as_ref()
            .expect("the sub run has backend data when it is drawn");
        glyph_data.fill_instance_data(
            sub_run.vertex_filler(),
            writer,
            sub_run_data.start_glyph_index(),
            sub_run_data.glyph_count(),
            sub_run.instance_flags(),
            ssbo_index,
            params.order().depth_as_float(),
        );
    }

    // Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L172-L210 (chrome/m156)
    fn write_uniforms_and_textures(
        &self,
        params: &DrawParams,
        gatherer: &mut PipelineDataGatherer,
    ) {
        #[cfg(debug_assertions)]
        gatherer.check_rewind();

        let sub_run_data = params.geometry().sub_run_data();
        let proxies = sub_run_data
            .atlas_proxies()
            .expect("the text atlas has a texture when the sub run is drawn");

        // write uniforms
        // TODO(b/238753996): The maskToDevice should be adjusted similar to CoverageMaskRenderStep
        // so that the integer translation is pulled into the instance data and this uniform is
        // less likely to change.
        // TODO(b/307766179): Similarly, we should discard the local-to-device matrix uniform
        // value (and just set identity) if the paint doesn't actually require local coords.
        // TODO(b/351923375): Precompute the 3x3 inverse of the local-to-device since it's shared
        // by all instances? We can derive it from the Transform's existing 4x4 inverse.
        let dims = proxies[0].dimensions();
        {
            let uniforms = gatherer.uniform_manager();
            #[cfg(debug_assertions)]
            uniforms.set_expected_uniforms(self.uniforms(), false);
            uniforms.write_m44(sub_run_data.mask_to_device());
            uniforms.write_m44(params.transform().matrix()); // local-to-device
            #[allow(clippy::cast_precision_loss)] // atlas dimensions are far below 2^24
            uniforms.write_vec([1.0 / dims.width as f32, 1.0 / dims.height as f32]);
            #[cfg(debug_assertions)]
            uniforms.done_with_expected_uniforms();
        }

        add_atlas_textures(gatherer, &proxies, FilterMode::Nearest);
    }
}
