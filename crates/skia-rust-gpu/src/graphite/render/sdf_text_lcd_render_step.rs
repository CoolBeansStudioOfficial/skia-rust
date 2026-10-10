// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/SDFTextLCDRenderStep.h,
// src/gpu/graphite/render/SDFTextLCDRenderStep.cpp

//! [`SDFTextLCDRenderStep`]: draws signed distance field glyphs from the text atlas with
//! per-channel (LCD) coverage.

use skia_rust_core::distance_field_gen::DISTANCE_FIELD_INSET;
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::surface_props::PixelGeometry;

use crate::graphite::attribute::Attribute;
use crate::graphite::caps::ResourceBindingRequirements;
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::PrimitiveType;
use crate::graphite::draw_writer::DrawWriter;
use crate::graphite::geom::transform::Transform;
use crate::graphite::paint_params_key::RootNodesInfo;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::bitmap_text_render_step::{
    TEXT_APPEND_ATTRS, add_atlas_textures, atlas_textures_and_samplers_sksl,
};
use crate::graphite::render::common_depth_stencil_settings::DIRECT_DEPTH_LEQUAL_PASS;
use crate::graphite::render::sdf_text_render_step::{SDF_VARYINGS, SDF_VERTEX_SKSL};
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::Layout;
use crate::graphite::uniform::Uniform;
use crate::sksl_type_shared::SkSLType;
use crate::text_gpu::distance_field_adjust_table::DistanceFieldAdjustTable;

const UNIFORMS: [Uniform; 5] = [
    Uniform::new("maskToDevice", SkSLType::Float4x4),
    Uniform::new("localToDevice", SkSLType::Float4x4),
    Uniform::new("atlasSizeInv", SkSLType::Float2),
    Uniform::new("pixelGeometryDelta", SkSLType::Half2),
    Uniform::new("gammaParams", SkSLType::Half4),
];

const APPEND_ATTRS: [Attribute; 7] = TEXT_APPEND_ATTRS;

/// `SkPixelGeometryIsH(geometry)`.
fn pixel_geometry_is_h(geometry: PixelGeometry) -> bool {
    matches!(geometry, PixelGeometry::RGBH | PixelGeometry::BGRH)
}

/// `SkPixelGeometryIsV(geometry)`.
fn pixel_geometry_is_v(geometry: PixelGeometry) -> bool {
    matches!(geometry, PixelGeometry::RGBV | PixelGeometry::BGRV)
}

/// `SkPixelGeometryIsBGR(geometry)`.
fn pixel_geometry_is_bgr(geometry: PixelGeometry) -> bool {
    matches!(geometry, PixelGeometry::BGRH | PixelGeometry::BGRV)
}

/// `SDFTextLCDRenderStep`: draws the glyphs of a distance field sub run with per-channel
/// coverage.
// Port of: src/gpu/graphite/render/SDFTextLCDRenderStep.h#L22-L38 (chrome/m156)
#[doc(alias = "skgpu::graphite::SDFTextLCDRenderStep")]
#[derive(Debug)]
pub struct SDFTextLCDRenderStep {
    base: RenderStepBase,
}

impl SDFTextLCDRenderStep {
    /// `SDFTextLCDRenderStep(layout)`.
    // Port of: src/gpu/graphite/render/SDFTextLCDRenderStep.cpp#L48-L83 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout) -> Self {
        let base = RenderStepBase::new(
            layout,
            RenderStepID::SDFTextLCD,
            RenderStepFlags::PERFORMS_SHADING
                | RenderStepFlags::HAS_TEXTURES
                | RenderStepFlags::EMITS_COVERAGE
                | RenderStepFlags::LCD_COVERAGE
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

impl RenderStep for SDFTextLCDRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/SDFTextLCDRenderStep.cpp#L87-L105 (chrome/m156)
    fn vertex_sksl(&self, _roots: &RootNodesInfo) -> String {
        // Returns the body of a vertex function, which must define a float4 devPosition variable
        // and must write to an already-defined float2 stepLocalCoords variable.
        SDF_VERTEX_SKSL.to_owned()
    }

    // Port of: src/gpu/graphite/render/SDFTextLCDRenderStep.cpp#L107-L119 (chrome/m156)
    fn textures_and_samplers_sksl(
        &self,
        binding_reqs: &ResourceBindingRequirements,
        next_binding_index: &mut i32,
    ) -> String {
        atlas_textures_and_samplers_sksl("sdf_atlas", binding_reqs, next_binding_index)
    }

    // Port of: src/gpu/graphite/render/SDFTextLCDRenderStep.cpp#L121-L146 (chrome/m156)
    fn fragment_coverage_sksl(&self) -> &'static str {
        // The returned SkSL must write its coverage into a 'half4 outputCoverage' variable
        // (defined in the calling code) with the actual coverage splatted out into all four
        // channels.

        // TODO: To minimize the number of shaders generated this is the full affine shader.
        // For best performance it may be worth creating the uniform scale shader as well,
        // as that's the most common case.
        // TODO: Need to add 565 support.
        // TODO: Need aliased and possibly sRGB support.
        "outputCoverage = sdf_text_lcd_coverage_fn(textureCoords, \
                                                   pixelGeometryDelta, \
                                                   gammaParams, \
                                                   unormTexCoords, \
                                                   texIndex, \
                                                   sdf_atlas_0, \
                                                   sdf_atlas_1, \
                                                   sdf_atlas_2, \
                                                   sdf_atlas_3);"
    }

    // Port of: src/gpu/graphite/render/SDFTextLCDRenderStep.cpp#L148-L163 (chrome/m156)
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

    // Port of: src/gpu/graphite/render/SDFTextLCDRenderStep.cpp#L165-L221 (chrome/m156)
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
            let (width, height) = (dims.width as f32, dims.height as f32);
            uniforms.write_vec([1.0 / width, 1.0 / height]);

            // compute and write pixelGeometry vector
            let mut pixel_geometry_delta = [0.0f32, 0.0];

            // There is 2px padding of each glyph (SK_DistanceFieldInset). We can't allow
            // offsetting to go outside of our padding (e.g. 2*SK_DistanceFieldInset if two SDF
            // glyphs were next to each other is theoretically ok). This is because SDF and
            // regular A8 masks are shared in the same atlas, so an adjacent glyph may not
            // actually have its own padding.
            //
            // NOTE: kLCDOffsetLimit is multiplied by 3 to account for the scale added to
            // pixelGeometryDelta
            #[allow(clippy::cast_precision_loss)] // the inset is 2
            let lcd_offset_limit: f32 = 3.0 * (DISTANCE_FIELD_INSET as f32 - 0.5);
            let max_lcd_offset = Transform::new(*sub_run_data.mask_to_device())
                .local_aa_radius(&sub_run_data.bounds());
            if max_lcd_offset < lcd_offset_limit {
                let pixel_geometry = sub_run_data.pixel_geometry();
                if pixel_geometry_is_h(pixel_geometry) {
                    pixel_geometry_delta = [1.0 / (3.0 * width), 0.0];
                } else if pixel_geometry_is_v(pixel_geometry) {
                    pixel_geometry_delta = [0.0, 1.0 / (3.0 * height)];
                }
                if pixel_geometry_is_bgr(pixel_geometry) {
                    pixel_geometry_delta = [-pixel_geometry_delta[0], -pixel_geometry_delta[1]];
                }
            }

            uniforms.write_half_vec(pixel_geometry_delta);

            // compute and write gamma adjustment
            let df_adjust_table = DistanceFieldAdjustTable::get();
            let luminance_color = sub_run_data.luminance_color();
            let use_gamma = sub_run_data.use_gamma_correct_distance_table();
            let red_correction =
                df_adjust_table.get_adjustment(i32::from(luminance_color.r()), use_gamma);
            let green_correction =
                df_adjust_table.get_adjustment(i32::from(luminance_color.g()), use_gamma);
            let blue_correction =
                df_adjust_table.get_adjustment(i32::from(luminance_color.b()), use_gamma);
            let gamma_params = [
                red_correction,
                green_correction,
                blue_correction,
                if use_gamma { 1.0 } else { 0.0 },
            ];
            uniforms.write_half_vec(gamma_params);

            #[cfg(debug_assertions)]
            uniforms.done_with_expected_uniforms();
        }

        add_atlas_textures(gatherer, &proxies, FilterMode::Linear);
    }
}
