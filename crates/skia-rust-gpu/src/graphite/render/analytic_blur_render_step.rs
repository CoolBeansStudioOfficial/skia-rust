// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/AnalyticBlurRenderStep.h, AnalyticBlurRenderStep.cpp

//! [`AnalyticBlurRenderStep`]: a blurred rect, rounded rect or circle, evaluated per pixel from
//! the blur's look-up texture. The step draws two triangles over the blur's draw bounds.

use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;

use crate::graphite::attribute::{Attribute, Interpolation, Varying};
use crate::graphite::caps::ResourceBindingRequirements;
use crate::graphite::context_utils::emit_sampler_layout;
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{PrimitiveType, VertexAttribType};
use crate::graphite::draw_writer::{DrawWriter, Vertices};
use crate::graphite::geom::analytic_blur_mask::ShapeType;
use crate::graphite::paint_params_key::RootNodesInfo;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::common_depth_stencil_settings::DIRECT_DEPTH_LESS_PASS;
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::{Layout, SamplerDesc};
use crate::graphite::uniform::Uniform;
use crate::sksl_type_shared::SkSLType;

/// The uniforms of the blur step (`AnalyticBlurRenderStep`'s constructor).
// Port of: src/gpu/graphite/render/AnalyticBlurRenderStep.cpp#L25-L38 (chrome/m156)
const UNIFORMS: [Uniform; 6] = [
    Uniform::new("localToDevice", SkSLType::Float4x4),
    Uniform::new("deviceToScaledShape", SkSLType::Float3x3),
    Uniform::new("shapeData", SkSLType::Float4),
    Uniform::new("blurData", SkSLType::Half2),
    Uniform::new("shapeType", SkSLType::Int),
    Uniform::new("depth", SkSLType::Float),
];

/// The appended vertex attributes: the position and the SSBO index.
// Port of: src/gpu/graphite/render/AnalyticBlurRenderStep.cpp#L39-L42 (chrome/m156)
const APPEND_ATTRS: [Attribute; 2] = [
    Attribute::new("position", VertexAttribType::Float2, SkSLType::Float2),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
];

/// The varyings: the fragment coordinates in the local shape space, scaled to device space but
/// not translated or rotated.
// Port of: src/gpu/graphite/render/AnalyticBlurRenderStep.cpp#L43-L51 (chrome/m156)
const VARYINGS: [Varying; 1] = [Varying::new(
    "scaledShapeCoords",
    SkSLType::Float2,
    Interpolation::Perspective,
)];

/// `AnalyticBlurRenderStep`: draws a blurred shape from its analytic blur mask.
// Port of: src/gpu/graphite/render/AnalyticBlurRenderStep.h#L19-L40 (chrome/m156)
#[doc(alias = "skgpu::graphite::AnalyticBlurRenderStep")]
#[derive(Debug)]
pub struct AnalyticBlurRenderStep {
    base: RenderStepBase,
}

impl AnalyticBlurRenderStep {
    /// `AnalyticBlurRenderStep(layout)`.
    // Port of: src/gpu/graphite/render/AnalyticBlurRenderStep.cpp#L25-L51 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout) -> Self {
        let base = RenderStepBase::new(
            layout,
            RenderStepID::AnalyticBlur,
            RenderStepFlags::PERFORMS_SHADING
                | RenderStepFlags::HAS_TEXTURES
                | RenderStepFlags::EMITS_COVERAGE
                | RenderStepFlags::APPEND_VERTICES,
            &UNIFORMS,
            PrimitiveType::Triangles,
            DIRECT_DEPTH_LESS_PASS,
            &[],
            &APPEND_ATTRS,
            &[],
            &VARYINGS,
        );
        Self { base }
    }
}

impl RenderStep for AnalyticBlurRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/AnalyticBlurRenderStep.cpp#L53-L61 (chrome/m156)
    fn vertex_sksl(&self, _roots: &RootNodesInfo) -> String {
        "float4 devPosition = localToDevice * float4(position, depth, 1.0);\n\
         stepLocalCoords = position;\n\
         scaledShapeCoords = (deviceToScaledShape * devPosition.xy1).xy;\n"
            .to_owned()
    }

    // Port of: src/gpu/graphite/render/AnalyticBlurRenderStep.cpp#L63-L66 (chrome/m156)
    fn textures_and_samplers_sksl(
        &self,
        binding_reqs: &ResourceBindingRequirements,
        next_binding_index: &mut i32,
    ) -> String {
        emit_sampler_layout(binding_reqs, next_binding_index) + " sampler2D s;"
    }

    // Port of: src/gpu/graphite/render/AnalyticBlurRenderStep.cpp#L68-L76 (chrome/m156)
    fn fragment_coverage_sksl(&self) -> &'static str {
        "outputCoverage = blur_coverage_fn(scaledShapeCoords, \
                                           shapeData, \
                                           blurData, \
                                           shapeType, \
                                           s);"
    }

    // Port of: src/gpu/graphite/render/AnalyticBlurRenderStep.cpp#L78-L92 (chrome/m156)
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32) {
        let r = params.geometry().analytic_blur_mask().draw_bounds();
        let (left, top, right, bot) = (r.left(), r.top(), r.right(), r.bot());
        let mut verts = Vertices::new(writer);
        let mut vw = verts.append(6);
        vw.put(&[left, top]).put(&ssbo_index);
        vw.put(&[right, top]).put(&ssbo_index);
        vw.put(&[left, bot]).put(&ssbo_index);
        vw.put(&[right, top]).put(&ssbo_index);
        vw.put(&[right, bot]).put(&ssbo_index);
        vw.put(&[left, bot]).put(&ssbo_index);
    }

    // Port of: src/gpu/graphite/render/AnalyticBlurRenderStep.cpp#L94-L110 (chrome/m156)
    fn write_uniforms_and_textures(
        &self,
        params: &DrawParams,
        gatherer: &mut PipelineDataGatherer,
    ) {
        #[cfg(debug_assertions)]
        gatherer.check_rewind();

        let blur = params.geometry().analytic_blur_mask();
        {
            let uniforms = gatherer.uniform_manager();
            #[cfg(debug_assertions)]
            uniforms.set_expected_uniforms(self.uniforms(), false);

            uniforms.write_m44(params.transform().matrix());
            uniforms.write_matrix(&blur.device_to_scaled_shape().to_m33());
            uniforms.write_rect(&blur.shape_data().as_sk_rect());
            let blur_data = blur.blur_data();
            uniforms.write_half_vec([blur_data.x, blur_data.y]);
            // The shape type is an `int` in the shader.
            uniforms.write_i32(blur.shape_type() as i32);
            uniforms.write_f32(params.order().depth_as_float());

            #[cfg(debug_assertions)]
            uniforms.done_with_expected_uniforms();
        }

        let filter = if blur.shape_type() == ShapeType::Rect {
            FilterMode::Linear
        } else {
            FilterMode::Nearest
        };
        gatherer.add(
            Some(blur.ref_proxy()),
            SamplerDesc::new(&SamplingOptions::from(filter), TileMode::Clamp),
        );
    }
}
