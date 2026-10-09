// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/TessellateCurvesRenderStep.h, TessellateCurvesRenderStep.cpp

//! [`TessellateCurvesRenderStep`]: the curve patches of a filled path, drawn as instances of a
//! fixed-count template (the static vertex and index buffers), with the curve evaluated in the
//! vertex shader. Used by the stencil-then-cover tessellating renderers.

use skia_rust_core::path_priv;
use skia_rust_core::path_types::PathVerb;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::scalar;

use crate::graphite::attribute::Attribute;
use crate::graphite::buffer_manager::{StaticBufferBinding, StaticBufferManager};
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{PrimitiveType, VertexAttribType};
use crate::graphite::draw_writer::DrawWriter;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::common_depth_stencil_settings::{
    EVEN_ODD_STENCIL_PASS, WINDING_STENCIL_PASS,
};
use crate::graphite::render::dynamic_instances_patch_allocator::{
    DynamicInstancesPatchAllocator, FixedCountVariant,
};
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::Layout;
use crate::graphite::uniform::Uniform;
use crate::sksl_type_shared::SkSLType;
use crate::tessellate::fixed_count_buffer_utils::FixedCountCurves;
use crate::tessellate::linear_tolerances::LinearTolerances;
use crate::tessellate::patch_writer::{GraphiteCurvesTraits, PatchWriter};
use crate::tessellate::tessellation::PatchAttribs;
use crate::tessellate::wangs_formula::VectorXform;

// Port of: src/gpu/graphite/render/TessellateCurvesRenderStep.cpp#L49-L51 (chrome/m156)
// No fan point or stroke params, since this is for filled curves (not strokes or wedges). No
// explicit curve type on platforms that support infinity.
const ATTRIBS: PatchAttribs = PatchAttribs::PAINT_DEPTH.union(PatchAttribs::SSBO_INDEX);
const ATTRIBS_WITH_CURVE_TYPE: PatchAttribs = ATTRIBS.union(PatchAttribs::EXPLICIT_CURVE_TYPE);

// Port of: src/gpu/graphite/render/TessellateCurvesRenderStep.cpp#L62-L76 (chrome/m156)
// The order of the attribute declarations must match the order used by
// PatchWriter::emitPatchAttribs, i.e.: join << fanPoint << stroke << color << depth << curveType
// << ssboIndex
const BASE_ATTRIBUTES: [Attribute; 4] = [
    Attribute::new("p01", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("p23", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("depth", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
];
const ATTRIBUTES_WITH_CURVE_TYPE: [Attribute; 5] = [
    Attribute::new("p01", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("p23", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("depth", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("curveType", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
];

// Port of: src/gpu/graphite/render/TessellateCurvesRenderStep.cpp#L93-L96 (chrome/m156)
const STATIC_ATTRS: [Attribute; 1] = [Attribute::new(
    "resolveLevel_and_idx",
    VertexAttribType::Float2,
    SkSLType::Float2,
)];

// Port of: src/gpu/graphite/render/TessellateCurvesRenderStep.cpp#L90 (chrome/m156)
const UNIFORMS: [Uniform; 1] = [Uniform::new("localToDevice", SkSLType::Float4x4)];

// The literal pieces of `TessellateCurvesRenderStep::vertexSkSL`, concatenated in C++ order. The
// `%s` takes the curve type expression.
// Port of: src/gpu/graphite/render/TessellateCurvesRenderStep.cpp#L118-L130 (chrome/m156)
const VERTEX_SKSL_FORMAT: &str = concat!(
    "float2x2 vectorXform = float2x2(localToDevice[0].xy, localToDevice[1].xy);\n",
    "float2 localCoord = tessellate_filled_curve(",
    "vectorXform, resolveLevel_and_idx.x, resolveLevel_and_idx.y, p01, p23, %s);\n",
    "float4 devPosition = localToDevice * float4(localCoord, 0.0, 1.0);\n",
    "devPosition.z = depth;\n",
    "stepLocalCoords = localCoord;\n",
);

/// The `TessellateCurvesRenderStep`: one of the even-odd or winding stencil variants.
// Port of: src/gpu/graphite/render/TessellateCurvesRenderStep.h#L16-L44 (chrome/m156)
#[doc(alias = "skgpu::graphite::TessellateCurvesRenderStep")]
#[derive(Debug)]
pub struct TessellateCurvesRenderStep {
    base: RenderStepBase,
    // Points to the static buffers holding the fixed indexed vertex template for drawing instances.
    vertex_buffer: StaticBufferBinding,
    index_buffer: StaticBufferBinding,
    infinity_support: bool,
}

// `FixedCountCurves` as the variant of `DynamicInstancesPatchAllocator`.
// Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L51-L58 (chrome/m156), `VertexCount`
impl FixedCountVariant for FixedCountCurves {
    fn vertex_count(tolerances: &LinearTolerances) -> u32 {
        u32::try_from(FixedCountCurves::vertex_count(tolerances))
            .expect("the vertex count is not negative")
    }
}

impl TessellateCurvesRenderStep {
    /// `TessellateCurvesRenderStep(layout, evenOdd, infinitySupport, bufferManager)`.
    // Port of: src/gpu/graphite/render/TessellateCurvesRenderStep.cpp#L80-L114 (chrome/m156)
    #[must_use]
    pub fn new(
        layout: Layout,
        even_odd: bool,
        infinity_support: bool,
        buffer_manager: &mut StaticBufferManager,
    ) -> Self {
        let render_step_id = if even_odd {
            RenderStepID::TessellateCurves_EvenOdd
        } else {
            RenderStepID::TessellateCurves_Winding
        };
        let depth_stencil = if even_odd {
            EVEN_ODD_STENCIL_PASS
        } else {
            WINDING_STENCIL_PASS
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
                | RenderStepFlags::IGNORE_INVERSE_FILL,
            &UNIFORMS,
            PrimitiveType::Triangles,
            depth_stencil,
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
            FixedCountCurves::vertex_buffer_vertex_count(),
            FixedCountCurves::vertex_buffer_stride(),
            &vertex_buffer,
        ) {
            FixedCountCurves::write_vertex_buffer(
                &mut vertex_data,
                FixedCountCurves::vertex_buffer_size(),
            );
        } // otherwise static buffer creation failed, so do nothing; Context initialization will fail.

        let index_buffer = StaticBufferBinding::new();
        let index_size = FixedCountCurves::index_buffer_size();
        if let Some(mut index_data) = buffer_manager.get_index_writer(index_size, &index_buffer) {
            FixedCountCurves::write_index_buffer(&mut index_data, index_size);
        } // otherwise static buffer creation failed, so do nothing; Context initialization will fail.

        Self {
            base,
            vertex_buffer,
            index_buffer,
            infinity_support,
        }
    }
}

impl RenderStep for TessellateCurvesRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/TessellateCurvesRenderStep.cpp#L118-L130 (chrome/m156)
    fn vertex_sksl(&self, _roots: &crate::graphite::paint_params_key::RootNodesInfo) -> String {
        // TODO (Skia): Approximate perspective scaling to match how PatchWriter is configured (or
        // provide explicit tessellation level in instance data instead of replicating work).
        let curve_type = if self.infinity_support {
            "curve_type_using_inf_support(p23)"
        } else {
            "curveType"
        };
        VERTEX_SKSL_FORMAT.replacen("%s", curve_type, 1)
    }

    // Port of: src/gpu/graphite/render/TessellateCurvesRenderStep.cpp#L132-L173 (chrome/m156)
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32) {
        // TODO (Skia): Iterate the Shape directly.
        let path = params.geometry().shape().as_path();

        let patch_reserve_count =
            FixedCountCurves::prealloc_count(i32::try_from(path.count_verbs()).unwrap_or(i32::MAX));
        let attribs = if self.infinity_support {
            ATTRIBS
        } else {
            ATTRIBS_WITH_CURVE_TYPE
        };
        let vertex_buffer = self.vertex_buffer.get();
        let index_buffer = self.index_buffer.get();
        let mut patches = PatchWriter::<
            GraphiteCurvesTraits,
            DynamicInstancesPatchAllocator<'_, '_, FixedCountCurves>,
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
        // TODO (Skia): This doesn't account for perspective division yet.
        debug_assert!(!matches!(
            params.transform().type_(),
            crate::graphite::geom::transform::Type::Perspective
        ));
        patches.set_shader_transform(
            VectorXform::from(params.transform().matrix()),
            params.transform().max_scale_factor(),
        );

        // TODO (Skia): Copy the logic from PathCurveTessellator::write_patches if the shape
        // iterator is needed here.
        for (verb, pts, w) in path_priv::iterate(&path) {
            match verb {
                PathVerb::Quad => patches.write_quadratic(fixed_pts::<3>(pts)),
                PathVerb::Conic => {
                    patches.write_conic(fixed_pts::<3>(pts), conic_weight(w));
                }
                PathVerb::Cubic => patches.write_cubic(fixed_pts::<4>(pts)),
                _ => {}
            }
        }
    }

    // Port of: src/gpu/graphite/render/TessellateCurvesRenderStep.cpp#L175-L181 (chrome/m156)
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

/// The first `N` points of a verb's points, as the fixed-size array the `PatchWriter` takes (the
/// `pts` argument of the C++ `write*` calls).
// Port of: src/core/SkPathPriv.h#L221-L227 (chrome/m156), the `pts` of `SkPathPriv::Iterate`
pub(crate) fn fixed_pts<const N: usize>(pts: &[Point]) -> &[Point; N] {
    <&[Point; N]>::try_from(&pts[..N]).expect("the verb has at least N points")
}

/// The weight of a conic verb (`*w` of `SkPathPriv::Iterate`).
// Port of: src/core/SkPathPriv.h#L221-L227 (chrome/m156), the `w` of `SkPathPriv::Iterate`
pub(crate) fn conic_weight(w: Option<scalar>) -> f32 {
    w.expect("a conic verb has a weight")
}
