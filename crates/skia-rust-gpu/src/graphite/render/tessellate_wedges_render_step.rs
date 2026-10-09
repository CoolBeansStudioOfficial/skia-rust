// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/TessellateWedgesRenderStep.h, TessellateWedgesRenderStep.cpp

//! [`TessellateWedgesRenderStep`]: the curve patches of a filled path, each joined to the midpoint
//! of its contour (the fan point), so that the wedges fill the contour. Used by the convex
//! renderer and the stencil-then-cover renderers.

use skia_rust_core::path_types::PathVerb;
use skia_rust_core::point::Point;

use crate::graphite::attribute::Attribute;
use crate::graphite::buffer_manager::{StaticBufferBinding, StaticBufferManager};
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{DepthStencilSettings, PrimitiveType, VertexAttribType};
use crate::graphite::draw_writer::DrawWriter;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::dynamic_instances_patch_allocator::{
    DynamicInstancesPatchAllocator, FixedCountVariant,
};
use crate::graphite::render::tessellate_curves_render_step::{conic_weight, fixed_pts};
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::Layout;
use crate::graphite::uniform::Uniform;
use crate::sksl_type_shared::SkSLType;
use crate::tessellate::fixed_count_buffer_utils::FixedCountWedges;
use crate::tessellate::linear_tolerances::LinearTolerances;
use crate::tessellate::midpoint_contour_parser::MidpointContourParser;
use crate::tessellate::patch_writer::{GraphiteWedgesTraits, PatchWriter};
use crate::tessellate::tessellation::PatchAttribs;
use crate::tessellate::wangs_formula::VectorXform;

// Port of: src/gpu/graphite/render/TessellateWedgesRenderStep.cpp#L44-L51 (chrome/m156)
// Only kFanPoint, no stroke params, since this is for filled wedges. No color or wide color attribs,
// since it might always be part of the PaintParams or we'll add a color-only fast path to RenderStep
// later. No explicit curve type on platforms that support infinity.
const ATTRIBS: PatchAttribs = PatchAttribs::FAN_POINT
    .union(PatchAttribs::PAINT_DEPTH)
    .union(PatchAttribs::SSBO_INDEX);
const ATTRIBS_WITH_CURVE_TYPE: PatchAttribs = ATTRIBS.union(PatchAttribs::EXPLICIT_CURVE_TYPE);

// Port of: src/gpu/graphite/render/TessellateWedgesRenderStep.cpp#L59-L77 (chrome/m156)
// The order of the attribute declarations must match the order used by
// PatchWriter::emitPatchAttribs, i.e.: join << fanPoint << stroke << color << depth << curveType
// << ssboIndex
const BASE_ATTRIBUTES: [Attribute; 5] = [
    Attribute::new("p01", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("p23", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("fanPointAttrib", VertexAttribType::Float2, SkSLType::Float2),
    Attribute::new("depth", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
];
const ATTRIBUTES_WITH_CURVE_TYPE: [Attribute; 6] = [
    Attribute::new("p01", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("p23", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("fanPointAttrib", VertexAttribType::Float2, SkSLType::Float2),
    Attribute::new("depth", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("curveType", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
];

// Port of: src/gpu/graphite/render/TessellateWedgesRenderStep.cpp#L97-L100 (chrome/m156)
const STATIC_ATTRS: [Attribute; 1] = [Attribute::new(
    "resolveLevel_and_idx",
    VertexAttribType::Float2,
    SkSLType::Float2,
)];

// Port of: src/gpu/graphite/render/TessellateWedgesRenderStep.cpp#L94 (chrome/m156)
const UNIFORMS: [Uniform; 1] = [Uniform::new("localToDevice", SkSLType::Float4x4)];

// The literal pieces of `TessellateWedgesRenderStep::vertexSkSL`, concatenated in C++ order. The
// `%s` takes the curve type expression.
// Port of: src/gpu/graphite/render/TessellateWedgesRenderStep.cpp#L122-L139 (chrome/m156)
const VERTEX_SKSL_FORMAT: &str = concat!(
    "float2 localCoord;\n",
    "if (resolveLevel_and_idx.x < 0) {\n",
    // A negative resolve level means this is the fan point.
    "localCoord = fanPointAttrib;\n",
    "} else {\n",
    // TODO (Skia): Approximate perspective scaling to match how PatchWriter is configured (or
    // provide explicit tessellation level in instance data instead of replicating work).
    "float2x2 vectorXform = float2x2(localToDevice[0].xy, localToDevice[1].xy);\n",
    "localCoord = tessellate_filled_curve(",
    "vectorXform, resolveLevel_and_idx.x, resolveLevel_and_idx.y, p01, p23, %s);\n",
    "}\n",
    "float4 devPosition = localToDevice * float4(localCoord, 0.0, 1.0);\n",
    "devPosition.z = depth;\n",
    "stepLocalCoords = localCoord;\n",
);

/// The `TessellateWedgesRenderStep`: the convex variant, or the even-odd or winding stencil
/// variant, chosen by the `RenderStepID` and depth-stencil settings it is given.
// Port of: src/gpu/graphite/render/TessellateWedgesRenderStep.h#L26-L49 (chrome/m156)
#[doc(alias = "skgpu::graphite::TessellateWedgesRenderStep")]
#[derive(Debug)]
pub struct TessellateWedgesRenderStep {
    base: RenderStepBase,
    // Points to the static buffers holding the fixed indexed vertex template for drawing instances.
    vertex_buffer: StaticBufferBinding,
    index_buffer: StaticBufferBinding,
    infinity_support: bool,
}

// `FixedCountWedges` as the variant of `DynamicInstancesPatchAllocator`.
// Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L168-L170 (chrome/m156), `VertexCount`
impl FixedCountVariant for FixedCountWedges {
    fn vertex_count(tolerances: &LinearTolerances) -> u32 {
        u32::try_from(FixedCountWedges::vertex_count(tolerances))
            .expect("the vertex count is not negative")
    }
}

impl TessellateWedgesRenderStep {
    /// `TessellateWedgesRenderStep(layout, renderStepID, infinitySupport, depthStencilSettings,
    /// bufferManager)`.
    // Port of: src/gpu/graphite/render/TessellateWedgesRenderStep.cpp#L82-L118 (chrome/m156)
    #[must_use]
    pub fn new(
        layout: Layout,
        render_step_id: RenderStepID,
        infinity_support: bool,
        depth_stencil_settings: DepthStencilSettings,
        buffer_manager: &mut StaticBufferManager,
    ) -> Self {
        let performs_shading = if depth_stencil_settings.depth_write_enabled {
            RenderStepFlags::PERFORMS_SHADING
        } else {
            RenderStepFlags::NONE
        };
        let append_attrs: &[Attribute] = if infinity_support {
            &BASE_ATTRIBUTES
        } else {
            &ATTRIBUTES_WITH_CURVE_TYPE
        };
        let base = RenderStepBase::new(
            layout,
            render_step_id,
            RenderStepFlags::REQUIRES_MSAA
                | RenderStepFlags::APPEND_DYNAMIC_INSTANCES
                | RenderStepFlags::IGNORE_INVERSE_FILL
                | performs_shading,
            &UNIFORMS,
            PrimitiveType::Triangles,
            depth_stencil_settings,
            &STATIC_ATTRS,
            append_attrs,
            &[],
            &[],
        );

        // Initialize the static buffers we'll use when recording draw calls.
        // NOTE: Each instance of this RenderStep gets its own copy of the data. If this ends up
        // causing problems, we can modify StaticBufferManager to de-duplicate requests.
        let vertex_buffer = StaticBufferBinding::new();
        if let Some(mut vertex_data) = buffer_manager.get_vertex_writer(
            FixedCountWedges::vertex_buffer_vertex_count(),
            FixedCountWedges::vertex_buffer_stride(),
            &vertex_buffer,
        ) {
            FixedCountWedges::write_vertex_buffer(
                &mut vertex_data,
                FixedCountWedges::vertex_buffer_size(),
            );
        } // otherwise static buffer creation failed, so do nothing; Context initialization will fail.

        let index_buffer = StaticBufferBinding::new();
        let index_size = FixedCountWedges::index_buffer_size();
        if let Some(mut index_data) = buffer_manager.get_index_writer(index_size, &index_buffer) {
            FixedCountWedges::write_index_buffer(&mut index_data, index_size);
        } // otherwise static buffer creation failed, so do nothing; Context initialization will fail.

        Self {
            base,
            vertex_buffer,
            index_buffer,
            infinity_support,
        }
    }
}

impl RenderStep for TessellateWedgesRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/TessellateWedgesRenderStep.cpp#L122-L140 (chrome/m156)
    fn vertex_sksl(&self) -> String {
        let curve_type = if self.infinity_support {
            "curve_type_using_inf_support(p23)"
        } else {
            "curveType"
        };
        VERTEX_SKSL_FORMAT.replacen("%s", curve_type, 1)
    }

    // Port of: src/gpu/graphite/render/TessellateWedgesRenderStep.cpp#L142-L210 (chrome/m156)
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32) {
        // TODO (Skia): Iterate the Shape directly.
        let path = params.geometry().shape().as_path();

        let patch_reserve_count =
            FixedCountWedges::prealloc_count(i32::try_from(path.count_verbs()).unwrap_or(i32::MAX));
        let attribs = if self.infinity_support {
            ATTRIBS
        } else {
            ATTRIBS_WITH_CURVE_TYPE
        };
        let vertex_buffer = self.vertex_buffer.get();
        let index_buffer = self.index_buffer.get();
        let mut patches = PatchWriter::<
            GraphiteWedgesTraits,
            DynamicInstancesPatchAllocator<'_, '_, FixedCountWedges>,
        >::new(attribs, |stride| {
            DynamicInstancesPatchAllocator::new(
                stride,
                writer,
                vertex_buffer,
                index_buffer,
                u32::try_from(patch_reserve_count).expect("the reserve count is not negative"),
            )
        });
        patches.update_paint_depth_attrib(params.order().depth_as_float());
        patches.update_ssbo_index_attrib(ssbo_index);

        // The vector xform approximates how the control points are transformed by the shader to
        // more accurately compute how many *parametric* segments are needed.
        debug_assert!(!matches!(
            params.transform().type_(),
            crate::graphite::geom::transform::Type::Perspective
        ));
        patches.set_shader_transform(
            VectorXform::from(params.transform().matrix()),
            params.transform().max_scale_factor(),
        );

        // For wedges, we iterate over each contour explicitly, using a fan point position that is
        // in the midpoint of the current contour.
        let mut parser = MidpointContourParser::new(&path);
        while parser.parse_next_contour() {
            patches.update_fan_point_attrib(parser.current_midpoint());
            let mut last_point = Point::new(0.0, 0.0);
            let mut start_point = Point::new(0.0, 0.0);
            for (verb, pts, w) in parser.current_contour() {
                match verb {
                    PathVerb::Move => {
                        start_point = pts[0];
                        last_point = pts[0];
                    }
                    // Unlike curve tessellation, wedges have to handle lines as part of the patch,
                    // effectively forming a single triangle with the fan point.
                    PathVerb::Line => {
                        patches.write_line(pts[0], pts[1]);
                        last_point = pts[1];
                    }
                    PathVerb::Quad => {
                        patches.write_quadratic(fixed_pts::<3>(pts));
                        last_point = pts[2];
                    }
                    PathVerb::Conic => {
                        patches.write_conic(fixed_pts::<3>(pts), conic_weight(w));
                        last_point = pts[2];
                    }
                    PathVerb::Cubic => {
                        patches.write_cubic(fixed_pts::<4>(pts));
                        last_point = pts[3];
                    }
                    PathVerb::Close => {}
                }
            }

            // Explicitly close the contour with another line segment, which also differs from
            // curve tessellation since that approach's triangle step automatically closes the
            // contour.
            #[allow(clippy::float_cmp)] // SkPoint's operator!= is an exact comparison
            if last_point != start_point {
                patches.write_line(last_point, start_point);
            }
        }
    }

    // Port of: src/gpu/graphite/render/TessellateWedgesRenderStep.cpp#L212-L218 (chrome/m156)
    fn write_uniforms_and_textures(
        &self,
        params: &DrawParams,
        gatherer: &mut PipelineDataGatherer,
    ) {
        gatherer
            .uniform_manager()
            .write_m44(params.transform().matrix());
    }
}
