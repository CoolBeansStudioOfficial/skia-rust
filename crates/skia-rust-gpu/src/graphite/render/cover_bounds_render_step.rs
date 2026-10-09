// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/CoverBoundsRenderStep.h, CoverBoundsRenderStep.cpp

//! [`CoverBoundsRenderStep`]: covers the bounds of a shape with one instance per draw. It is the
//! cover pass of the stencil-then-cover algorithms, and the non-AA fill of rectangles.

use crate::graphite::attribute::Attribute;
use crate::graphite::buffer::BindBufferInfo;
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{DepthStencilSettings, PrimitiveType, VertexAttribType};
use crate::graphite::draw_writer::{DrawWriter, Instances};
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::Layout;
use crate::sksl_type_shared::SkSLType;

/// The `CoverBoundsRenderStep`: one instance per draw, as a 4-vertex triangle strip.
// Port of: src/gpu/graphite/render/CoverBoundsRenderStep.h#L17-L35 (chrome/m156)
#[doc(alias = "skgpu::graphite::CoverBoundsRenderStep")]
#[derive(Debug)]
pub struct CoverBoundsRenderStep {
    base: RenderStepBase,
}

// Port of: src/gpu/graphite/render/CoverBoundsRenderStep.cpp#L22-L40 (chrome/m156), `appendAttrs`
const APPEND_ATTRS: [Attribute; 6] = [
    Attribute::new("bounds", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("depth", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
    Attribute::new("mat0", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("mat1", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("mat2", VertexAttribType::Float3, SkSLType::Float3),
];

impl CoverBoundsRenderStep {
    /// `CoverBoundsRenderStep(layout, renderStepID, dsSettings)`.
    // Port of: src/gpu/graphite/render/CoverBoundsRenderStep.cpp#L22-L40 (chrome/m156)
    #[must_use]
    pub fn new(
        layout: Layout,
        render_step_id: RenderStepID,
        ds_settings: DepthStencilSettings,
    ) -> Self {
        Self {
            base: RenderStepBase::new(
                layout,
                render_step_id,
                RenderStepFlags::PERFORMS_SHADING
                    | RenderStepFlags::APPEND_INSTANCES
                    | RenderStepFlags::INVERSE_FILLS_SCISSOR,
                &[],
                PrimitiveType::TriangleStrip,
                ds_settings,
                &[],
                &APPEND_ATTRS,
                &[],
                &[],
            ),
        }
    }
}

impl RenderStep for CoverBoundsRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/CoverBoundsRenderStep.cpp#L42-L49 (chrome/m156)
    fn vertex_sksl(&self) -> String {
        // Returns the body of a vertex function, which must define a float4 devPosition variable
        // and must write to an already-defined float2 stepLocalCoords variable.
        "float4 devPosition = cover_bounds_vertex_fn(\
         float2(sk_VertexID / 2, sk_VertexID % 2), \
         bounds, depth, float3x3(mat0, mat1, mat2), \
         stepLocalCoords);\n"
            .to_owned()
    }

    // Port of: src/gpu/graphite/render/CoverBoundsRenderStep.cpp#L51-L80 (chrome/m156)
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32) {
        // Each instance is 4 vertices, forming 2 triangles from a single triangle strip, so no
        // indices are needed. sk_VertexID places the vertices, so no vertex buffer is needed.
        let mut instances = Instances::new(
            writer,
            BindBufferInfo::default(),
            BindBufferInfo::default(),
            4,
        );

        // `skvx::cast<float>` of the scissor's ints, as the C++ converts them.
        #[allow(clippy::cast_precision_loss)]
        let bounds: [f32; 4] =
            if params.geometry().is_shape() && params.geometry().shape().inverted() {
                // Normally bounding boxes are sorted so that l<r and t<b. An inverse fill uploads the
                // inverted rectangle [r,b,l,t] to encode that the bounds are already in device space;
                // the VS then uses the inverse transform to compute local coordinates.
                let s = params.scissor();
                [s.right as f32, s.bottom as f32, s.left as f32, s.top as f32]
            } else {
                let ltrb = params.geometry().bounds().ltrb();
                [ltrb.x(), ltrb.y(), ltrb.z(), ltrb.w()]
            };

        // Since the local coords always have Z=0, the 3rd row and column of the matrix are unused.
        let m = params.transform().matrix();
        let mut vw = instances.append(1);
        vw.put(&bounds)
            .put(&params.order().depth_as_float())
            .put(&ssbo_index)
            .put(&m.rc(0, 0))
            .put(&m.rc(1, 0))
            .put(&m.rc(3, 0))
            .put(&m.rc(0, 1))
            .put(&m.rc(1, 1))
            .put(&m.rc(3, 1))
            .put(&m.rc(0, 3))
            .put(&m.rc(1, 3))
            .put(&m.rc(3, 3));
    }

    // Port of: src/gpu/graphite/render/CoverBoundsRenderStep.cpp#L82-L88 (chrome/m156)
    fn write_uniforms_and_textures(
        &self,
        _params: &DrawParams,
        _gatherer: &mut PipelineDataGatherer,
    ) {
        // All data is uploaded as instance attributes, so no uniforms are needed.
    }
}
