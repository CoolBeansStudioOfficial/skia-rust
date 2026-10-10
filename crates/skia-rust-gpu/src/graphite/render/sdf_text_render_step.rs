// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/SDFTextRenderStep.h,
// src/gpu/graphite/render/SDFTextRenderStep.cpp

//! [`SDFTextRenderStep`]: draws signed distance field glyphs from the text atlas, with the
//! distance mapped to single channel coverage.

use skia_rust_core::mask_gamma::compute_luminance;
use skia_rust_core::sampling_options::FilterMode;

use crate::graphite::attribute::{Attribute, Interpolation, Varying};
use crate::graphite::caps::ResourceBindingRequirements;
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::PrimitiveType;
use crate::graphite::draw_writer::DrawWriter;
use crate::graphite::paint_params_key::RootNodesInfo;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::bitmap_text_render_step::{
    TEXT_APPEND_ATTRS, add_atlas_textures, atlas_textures_and_samplers_sksl,
};
use crate::graphite::render::common_depth_stencil_settings::DIRECT_DEPTH_LEQUAL_PASS;
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::Layout;
use crate::graphite::uniform::Uniform;
use crate::sksl_type_shared::SkSLType;
use crate::text_gpu::distance_field_adjust_table::DistanceFieldAdjustTable;

/// `SK_GAMMA_EXPONENT`: 0 is sRGB.
const GAMMA_EXPONENT: f32 = 0.0;

const UNIFORMS: [Uniform; 4] = [
    Uniform::new("maskToDevice", SkSLType::Float4x4),
    Uniform::new("localToDevice", SkSLType::Float4x4),
    Uniform::new("atlasSizeInv", SkSLType::Float2),
    Uniform::new("gammaParams", SkSLType::Half2),
];

const APPEND_ATTRS: [Attribute; 7] = TEXT_APPEND_ATTRS;

/// The vertex SkSL of the distance field text steps.
// Port of: src/gpu/graphite/render/SDFTextRenderStep.cpp#L90-L106 (chrome/m156)
pub(crate) const SDF_VERTEX_SKSL: &str = "texIndex = half(indexAndFlags.x);\
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
                                         stepLocalCoords);";

/// The varyings of the distance field text steps.
// Port of: src/gpu/graphite/render/SDFTextRenderStep.cpp#L68-L71 (chrome/m156)
pub(crate) const SDF_VARYINGS: [Varying; 3] = [
    Varying::new("unormTexCoords", SkSLType::Float2, Interpolation::Perspective),
    Varying::new("textureCoords", SkSLType::Float2, Interpolation::Perspective),
    Varying::new("texIndex", SkSLType::Float, Interpolation::Perspective),
];

/// `SDFTextRenderStep`: draws the glyphs of a distance field sub run with single channel
/// coverage.
// Port of: src/gpu/graphite/render/SDFTextRenderStep.h#L22-L40 (chrome/m156)
#[doc(alias = "skgpu::graphite::SDFTextRenderStep")]
#[derive(Debug)]
pub struct SDFTextRenderStep {
    base: RenderStepBase,
}

impl SDFTextRenderStep {
    /// `SDFTextRenderStep(layout)`.
    // Port of: src/gpu/graphite/render/SDFTextRenderStep.cpp#L53-L84 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout) -> Self {
        let base = RenderStepBase::new(
            layout,
            RenderStepID::SDFText,
            RenderStepFlags::PERFORMS_SHADING
                | RenderStepFlags::HAS_TEXTURES
                | RenderStepFlags::EMITS_COVERAGE
                | RenderStepFlags::APPEND_INSTANCES,
            &UNIFORMS,
            PrimitiveType::TriangleStrip,
            DIRECT_DEPTH_LEQUAL_PASS,
            &[],
            &APPEND_ATTRS,
            &[],
            &SDF_VARYINGS,
        );
        Self { base }
    }
}

impl RenderStep for SDFTextRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/SDFTextRenderStep.cpp#L88-L106 (chrome/m156)
    fn vertex_sksl(&self, _roots: &RootNodesInfo) -> String {
        // Returns the body of a vertex function, which must define a float4 devPosition variable
        // and must write to an already-defined float2 stepLocalCoords variable.
        SDF_VERTEX_SKSL.to_owned()
    }

    // Port of: src/gpu/graphite/render/SDFTextRenderStep.cpp#L108-L120 (chrome/m156)
    fn textures_and_samplers_sksl(
        &self,
        binding_reqs: &ResourceBindingRequirements,
        next_binding_index: &mut i32,
    ) -> String {
        atlas_textures_and_samplers_sksl("sdf_atlas", binding_reqs, next_binding_index)
    }

    // Port of: src/gpu/graphite/render/SDFTextRenderStep.cpp#L122-L141 (chrome/m156)
    fn fragment_coverage_sksl(&self) -> &'static str {
        // The returned SkSL must write its coverage into a 'half4 outputCoverage' variable
        // (defined in the calling code) with the actual coverage splatted out into all four
        // channels.

        // TODO: To minimize the number of shaders generated this is the full affine shader.
        // For best performance it may be worth creating the uniform scale shader as well,
        // as that's the most common case.
        // TODO: Need to add 565 support.
        // TODO: Need aliased and possibly sRGB support.
        "outputCoverage = sdf_text_coverage_fn(sample_indexed_atlas(textureCoords, \
                                                                    int(texIndex), \
                                                                    sdf_atlas_0, \
                                                                    sdf_atlas_1, \
                                                                    sdf_atlas_2, \
                                                                    sdf_atlas_3).r, \
                                               gammaParams, \
                                               unormTexCoords);"
    }

    // Port of: src/gpu/graphite/render/SDFTextRenderStep.cpp#L143-L158 (chrome/m156)
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

    // Port of: src/gpu/graphite/render/SDFTextRenderStep.cpp#L160-L204 (chrome/m156)
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

        let dims = proxies[0].dimensions();
        {
            let uniforms = gatherer.uniform_manager();
            #[cfg(debug_assertions)]
            uniforms.set_expected_uniforms(self.uniforms(), false);

            // write uniforms
            // TODO(b/238753996): The maskToDevice should be adjusted similar to
            // CoverageMaskRenderStep so that the integer translation is pulled into the instance
            // data and this uniform is less likely to change.
            // TODO(b/307766179): Similarly, we should discard the local-to-device matrix uniform
            // value (and just set identity) if the paint doesn't actually require local coords.
            // TODO(b/351923375): Precompute the 3x3 inverse of the local-to-device since it's
            // shared by all instances? We can derive it from the Transform's existing 4x4
            // inverse.
            uniforms.write_m44(sub_run_data.mask_to_device());
            uniforms.write_m44(params.transform().matrix()); // local-to-device
            #[allow(clippy::cast_precision_loss)] // atlas dimensions are far below 2^24
            uniforms.write_vec([1.0 / dims.width as f32, 1.0 / dims.height as f32]);

            // TODO: generate LCD adjustment
            // SK_GAMMA_APPLY_TO_A8 is defined in Skia's own build.
            let df_adjust_table = DistanceFieldAdjustTable::get();
            // TODO: don't do this for aliased text
            #[allow(clippy::cast_possible_wrap)] // a luminance is at most 255
            let lum = compute_luminance(GAMMA_EXPONENT, sub_run_data.luminance_color()) as i32;
            let gamma_adjustment = df_adjust_table
                .get_adjustment(lum, sub_run_data.use_gamma_correct_distance_table());
            let gamma_params = [
                gamma_adjustment,
                if sub_run_data.use_gamma_correct_distance_table() {
                    1.0
                } else {
                    0.0
                },
            ];
            uniforms.write_half_vec(gamma_params);

            #[cfg(debug_assertions)]
            uniforms.done_with_expected_uniforms();
        }

        add_atlas_textures(gatherer, &proxies, FilterMode::Linear);
    }
}
