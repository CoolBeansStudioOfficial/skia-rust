// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/MiddleOutFanRenderStep.h, MiddleOutFanRenderStep.cpp

//! [`MiddleOutFanRenderStep`]: the triangles of a path's middle-out fan, written as plain vertices
//! into the stencil pass of a stencil-then-cover renderer. Unlike the curve steps, it draws no
//! curves, so it needs no tessellation.

use crate::graphite::attribute::Attribute;
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{DepthStencilSettings, PrimitiveType, VertexAttribType};
use crate::graphite::draw_writer::{DrawWriter, Vertices};
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::common_depth_stencil_settings::{
    EVEN_ODD_STENCIL_PASS, WINDING_STENCIL_PASS,
};
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::Layout;
use crate::graphite::uniform::Uniform;
use crate::sksl_type_shared::SkSLType;
use crate::tessellate::middle_out_polygon_triangulator::PathMiddleOutFanIter;

// Port of: src/gpu/graphite/render/MiddleOutFanRenderStep.cpp#L37-L43 (chrome/m156)
const APPEND_ATTRS: [Attribute; 3] = [
    Attribute::new("position", VertexAttribType::Float2, SkSLType::Float2),
    Attribute::new("depth", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
];

// Port of: src/gpu/graphite/render/MiddleOutFanRenderStep.cpp#L35 (chrome/m156)
const UNIFORMS: [Uniform; 1] = [Uniform::new("localToDevice", SkSLType::Float4x4)];

/// The `MiddleOutFanRenderStep`: the even-odd or winding stencil variant.
// Port of: src/gpu/graphite/render/MiddleOutFanRenderStep.h#L22-L37 (chrome/m156)
#[doc(alias = "skgpu::graphite::MiddleOutFanRenderStep")]
#[derive(Debug)]
pub struct MiddleOutFanRenderStep {
    base: RenderStepBase,
}

impl MiddleOutFanRenderStep {
    /// `MiddleOutFanRenderStep(layout, evenOdd)`.
    // Port of: src/gpu/graphite/render/MiddleOutFanRenderStep.cpp#L30-L43 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout, even_odd: bool) -> Self {
        let (render_step_id, depth_stencil): (RenderStepID, DepthStencilSettings) = if even_odd {
            (RenderStepID::MiddleOutFan_EvenOdd, EVEN_ODD_STENCIL_PASS)
        } else {
            (RenderStepID::MiddleOutFan_Winding, WINDING_STENCIL_PASS)
        };
        let base = RenderStepBase::new(
            layout,
            render_step_id,
            RenderStepFlags::REQUIRES_MSAA | RenderStepFlags::APPEND_VERTICES,
            &UNIFORMS,
            PrimitiveType::Triangles,
            depth_stencil,
            &[],
            &APPEND_ATTRS,
            &[],
            &[],
        );
        Self { base }
    }
}

impl RenderStep for MiddleOutFanRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/MiddleOutFanRenderStep.cpp#L47-L52 (chrome/m156)
    fn vertex_sksl(&self, _roots: &crate::graphite::paint_params_key::RootNodesInfo) -> String {
        concat!(
            "float4 devPosition = localToDevice * float4(position, 0.0, 1.0);\n",
            "devPosition.z = depth;\n",
            "stepLocalCoords = position;\n",
        )
        .to_owned()
    }

    // Port of: src/gpu/graphite/render/MiddleOutFanRenderStep.cpp#L54-L75 (chrome/m156)
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32) {
        // TODO (Skia): Have Shape provide a path-like iterator so we don't actually have to
        // convert non paths to SkPath just to iterate their pts/verbs.
        let path = params.geometry().shape().as_path();

        let max_triangles_in_fans = path.count_verbs().saturating_sub(2);
        let depth = params.order().depth_as_float();

        let mut verts = Vertices::new(writer);
        verts.reserve(
            u32::try_from(max_triangles_in_fans * 3).expect("the triangle count fits in u32"),
        );
        let mut it = PathMiddleOutFanIter::new(&path);
        while !it.done() {
            for (p0, p1, p2) in &it.next_stack() {
                let mut vw = verts.append(3);
                vw.put(&p0)
                    .put(&depth)
                    .put(&ssbo_index)
                    .put(&p1)
                    .put(&depth)
                    .put(&ssbo_index)
                    .put(&p2)
                    .put(&depth)
                    .put(&ssbo_index);
            }
        }
    }

    // Port of: src/gpu/graphite/render/MiddleOutFanRenderStep.cpp#L77-L83 (chrome/m156)
    fn write_uniforms_and_textures(
        &self,
        params: &DrawParams,
        gatherer: &mut PipelineDataGatherer,
    ) {
        #[cfg(debug_assertions)]
        gatherer.check_rewind();
        let uniforms = gatherer.uniform_manager();
        // `UniformExpectationsValidator uev(gatherer, this->uniforms())`
        #[cfg(debug_assertions)]
        uniforms.set_expected_uniforms(self.uniforms(), false);
        uniforms.write_m44(params.transform().matrix());
        #[cfg(debug_assertions)]
        uniforms.done_with_expected_uniforms();
    }
}
