// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/TessellateStrokesRenderStep.h, TessellateStrokesRenderStep.cpp

//! [`TessellateStrokesRenderStep`]: the stroke of a path, drawn as one instance per stroke patch
//! (a curve, line, join or cap) with the vertex shader generating the triangle strip. The strokes
//! need no static buffers: the vertex ID selects the edge.

use skia_rust_core::geometry::{
    Conic, chop_cubic_at, chop_cubic_at_t0_t1, eval_quad_at, find_quad_mid_tangent,
};
use skia_rust_core::path_priv;
use skia_rust_core::path_types::PathVerb;
use skia_rust_core::point::Point;
use skia_rust_core::tessellation::find_cubic_convex_180_chops;

use crate::graphite::attribute::Attribute;
use crate::graphite::buffer::BindBufferInfo;
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{PrimitiveType, VertexAttribType};
use crate::graphite::draw_writer::DrawWriter;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::common_depth_stencil_settings::{
    DIRECT_DEPTH_LESS_PASS, INCREMENT_STENCIL_PASS,
};
use crate::graphite::render::dynamic_instances_patch_allocator::{
    DynamicInstancesPatchAllocator, FixedCountVariant,
};
use crate::graphite::render::tessellate_curves_render_step::{conic_weight, fixed_pts};
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::Layout;
use crate::graphite::uniform::Uniform;
use crate::sksl_type_shared::SkSLType;
use crate::tessellate::fixed_count_buffer_utils::FixedCountStrokes;
use crate::tessellate::linear_tolerances::LinearTolerances;
use crate::tessellate::patch_writer::{GraphiteStrokesTraits, PatchWriter};
use crate::tessellate::tessellation::{PatchAttribs, StrokeParams, conic_has_cusp};
use crate::tessellate::wangs_formula::VectorXform;

// Port of: src/gpu/graphite/render/TessellateStrokesRenderStep.cpp#L46-L56 (chrome/m156)
// Always use dynamic stroke params and join control points, track the join control point in
// PatchWriter and replicate line end points (match Ganesh's shader behavior). No explicit curve
// type on platforms that support infinity. No color or wide color attribs.
const ATTRIBS: PatchAttribs = PatchAttribs::JOIN_CONTROL_POINT
    .union(PatchAttribs::STROKE_PARAMS)
    .union(PatchAttribs::PAINT_DEPTH)
    .union(PatchAttribs::SSBO_INDEX);
const ATTRIBS_WITH_CURVE_TYPE: PatchAttribs = ATTRIBS.union(PatchAttribs::EXPLICIT_CURVE_TYPE);

// Port of: src/gpu/graphite/render/TessellateStrokesRenderStep.cpp#L66-L84 (chrome/m156)
// The order of the attribute declarations must match the order used by
// PatchWriter::emitPatchAttribs, i.e.: join << fanPoint << stroke << color << depth << curveType
// << ssboIndex
const BASE_ATTRIBUTES: [Attribute; 6] = [
    Attribute::new("p01", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("p23", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("prevPoint", VertexAttribType::Float2, SkSLType::Float2),
    Attribute::new("stroke", VertexAttribType::Float2, SkSLType::Float2),
    Attribute::new("depth", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
];
const ATTRIBUTES_WITH_CURVE_TYPE: [Attribute; 7] = [
    Attribute::new("p01", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("p23", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("prevPoint", VertexAttribType::Float2, SkSLType::Float2),
    Attribute::new("stroke", VertexAttribType::Float2, SkSLType::Float2),
    Attribute::new("depth", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("curveType", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
];

// Port of: src/gpu/graphite/render/TessellateStrokesRenderStep.cpp#L98-L100 (chrome/m156)
const UNIFORMS: [Uniform; 3] = [
    Uniform::new("affineMatrix", SkSLType::Float4),
    Uniform::new("translate", SkSLType::Float2),
    Uniform::new("maxScale", SkSLType::Float),
];

// The literal pieces of `TessellateStrokesRenderStep::vertexSkSL`, concatenated in C++ order. The
// `%s` takes the curve type expression. The edge count 16383 is `FixedCountStrokes::kMaxEdges`.
// Port of: src/gpu/graphite/render/TessellateStrokesRenderStep.cpp#L113-L123 (chrome/m156)
const VERTEX_SKSL_FORMAT: &str = concat!(
    "float edgeID = float(sk_VertexID >> 1);\n",
    "if ((sk_VertexID & 1) != 0) {",
    "edgeID = -edgeID;",
    "}\n",
    "float2x2 affine = float2x2(affineMatrix.xy, affineMatrix.zw);\n",
    "float4 devAndLocalCoords = tessellate_stroked_curve(",
    "edgeID, 16383, affine, translate, maxScale, p01, p23, prevPoint,",
    "stroke, %s);\n",
    "float4 devPosition = float4(devAndLocalCoords.xy, depth, 1.0);\n",
    "stepLocalCoords = devAndLocalCoords.zw;\n",
);

/// The `TessellateStrokesRenderStep`: the fill of a stroke, or the inverse fill of a stroke that
/// increments the stencil.
// Port of: src/gpu/graphite/render/TessellateStrokesRenderStep.h#L22-L37 (chrome/m156)
#[doc(alias = "skgpu::graphite::TessellateStrokesRenderStep")]
#[derive(Debug)]
pub struct TessellateStrokesRenderStep {
    base: RenderStepBase,
    infinity_support: bool,
}

// `FixedCountStrokes` as the variant of `DynamicInstancesPatchAllocator`.
// Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L168-L170 (chrome/m156), `VertexCount`
impl FixedCountVariant for FixedCountStrokes {
    fn vertex_count(tolerances: &LinearTolerances) -> u32 {
        u32::try_from(FixedCountStrokes::vertex_count(tolerances))
            .expect("the vertex count is not negative")
    }
}

impl TessellateStrokesRenderStep {
    /// `TessellateStrokesRenderStep(layout, infinitySupport, inverseFill)`.
    // Port of: src/gpu/graphite/render/TessellateStrokesRenderStep.cpp#L91-L106 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout, infinity_support: bool, inverse_fill: bool) -> Self {
        let (render_step_id, flags, depth_stencil) = if inverse_fill {
            (
                RenderStepID::TessellateStrokes_InverseFill,
                RenderStepFlags::NONE,
                INCREMENT_STENCIL_PASS,
            )
        } else {
            (
                RenderStepID::TessellateStrokes_Fill,
                RenderStepFlags::PERFORMS_SHADING,
                DIRECT_DEPTH_LESS_PASS,
            )
        };
        let append_attrs: &[Attribute] = if infinity_support {
            &BASE_ATTRIBUTES
        } else {
            &ATTRIBUTES_WITH_CURVE_TYPE
        };
        let base = RenderStepBase::new(
            layout,
            render_step_id,
            flags | RenderStepFlags::REQUIRES_MSAA | RenderStepFlags::APPEND_DYNAMIC_INSTANCES,
            &UNIFORMS,
            PrimitiveType::TriangleStrip,
            depth_stencil,
            &[],
            append_attrs,
            &[],
            &[],
        );
        Self {
            base,
            infinity_support,
        }
    }
}

impl RenderStep for TessellateStrokesRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/TessellateStrokesRenderStep.cpp#L113-L125 (chrome/m156)
    fn vertex_sksl(&self) -> String {
        // TODO (Skia): Assumes vertex ID support for now, max edges must equal
        // skgpu::tess::FixedCountStrokes::kMaxEdges -> (2^14 - 1) -> 16383.
        let curve_type = if self.infinity_support {
            "curve_type_using_inf_support(p23)"
        } else {
            "curveType"
        };
        VERTEX_SKSL_FORMAT.replacen("%s", curve_type, 1)
    }

    // Port of: src/gpu/graphite/render/TessellateStrokesRenderStep.cpp#L127-L248 (chrome/m156)
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32) {
        // TODO (Skia): Iterate the Shape directly.
        let path = params.geometry().shape().as_path();

        let patch_reserve_count = FixedCountStrokes::prealloc_count(
            i32::try_from(path.count_verbs()).unwrap_or(i32::MAX),
        );
        // Stroke tessellation does not use fixed indices or vertex data, and only needs the vertex
        // ID.
        let attribs = if self.infinity_support {
            ATTRIBS
        } else {
            ATTRIBS_WITH_CURVE_TYPE
        };
        let mut patches = PatchWriter::<
            GraphiteStrokesTraits,
            DynamicInstancesPatchAllocator<'_, '_, FixedCountStrokes>,
        >::new(attribs, |stride| {
            DynamicInstancesPatchAllocator::new(
                stride,
                writer,
                BindBufferInfo::default(),
                BindBufferInfo::default(),
                u32::try_from(patch_reserve_count).expect("the reserve count is not negative"),
            )
        });
        patches.update_paint_depth_attrib(params.order().depth_as_float());
        patches.update_ssbo_index_attrib(ssbo_index);

        // The vector xform approximates how the control points are transformed by the shader to
        // more accurately compute how many *parametric* segments are needed.
        // getMaxScale() returns -1 if it can't compute a scale factor (e.g. perspective), taking
        // the absolute value automatically converts that to an identity scale factor for our
        // purposes.
        patches.set_shader_transform(
            VectorXform::from(params.transform().matrix()),
            params.transform().max_scale_factor(),
        );

        debug_assert!(params.is_stroke());
        let style = params.stroke_style();
        patches
            .update_stroke_params_attrib(StrokeParams::new(style.half_width(), style.join_limit()));

        let cap = style.cap();
        for (verb, pts, w) in path_priv::iterate(&path) {
            match verb {
                // This automatically joins the last contour with the first contour (deferred) if
                // the contour is closed. If the contour is not closed, it automatically adds
                // additional patches for the end cap of the last patch and the beginning cap of
                // the deferred patch. This does nothing if this is the beginning of the first
                // contour.
                PathVerb::Move => patches.write_deferred_stroke_patch(pts[0], Some(cap)),
                // Draws a line back to the starting point of the contour and writes any deferred
                // patch with a join (instead of caps). Or if the contour was empty, draws a cap.
                PathVerb::Close => patches.close_deferred_stroke_patch(cap),
                PathVerb::Line => patches.write_line(pts[0], pts[1]),
                PathVerb::Quad => {
                    let quad = fixed_pts::<3>(pts);
                    if conic_has_cusp(quad) {
                        // The cusp is always at the midtangent.
                        let cusp = eval_quad_at(quad, find_quad_mid_tangent(quad));
                        patches.write_circle(cusp);
                        // A quad can only have a cusp if it's flat with a 180-degree turnaround.
                        patches.write_line(quad[0], cusp);
                        patches.write_line(cusp, quad[2]);
                    } else {
                        patches.write_quadratic(quad);
                    }
                }
                PathVerb::Conic => {
                    let conic_pts = fixed_pts::<3>(pts);
                    let weight = conic_weight(w);
                    if conic_has_cusp(conic_pts) {
                        // The cusp is always at the midtangent.
                        let conic = Conic::new(conic_pts[0], conic_pts[1], conic_pts[2], weight);
                        let cusp = conic.eval_at(conic.find_mid_tangent());
                        patches.write_circle(cusp);
                        // A conic can only have a cusp if it's flat with a 180-degree turnaround.
                        patches.write_line(conic_pts[0], cusp);
                        patches.write_line(cusp, conic_pts[2]);
                    } else {
                        patches.write_conic(conic_pts, weight);
                    }
                }
                PathVerb::Cubic => {
                    let cubic = fixed_pts::<4>(pts);
                    let mut chops = [Point::default(); 10];
                    let mut t = [0.0_f32; 2];
                    let mut are_cusps = false;
                    let num_chops = find_cubic_convex_180_chops(cubic, &mut t, &mut are_cusps);
                    match num_chops {
                        0 => patches.write_cubic(cubic),
                        1 => {
                            chop_cubic_at(cubic, &mut chops, t[0]);
                            if are_cusps {
                                patches.write_circle(chops[3]);
                                // In a perfect world, these 3 points would be equal after chopping
                                // on a cusp.
                                chops[2] = chops[3];
                                chops[4] = chops[3];
                            }
                            patches.write_cubic(fixed_pts::<4>(&chops[..]));
                            patches.write_cubic(fixed_pts::<4>(&chops[3..]));
                        }
                        _ => {
                            debug_assert_eq!(num_chops, 2);
                            chop_cubic_at_t0_t1(cubic, &mut chops, t[0], t[1]);
                            if are_cusps {
                                patches.write_circle(chops[3]);
                                patches.write_circle(chops[6]);
                                // Two cusps are only possible if it's a flat line with two
                                // 180-degree turnarounds.
                                patches.write_line(chops[0], chops[3]);
                                patches.write_line(chops[3], chops[6]);
                                patches.write_line(chops[6], chops[9]);
                            } else {
                                patches.write_cubic(fixed_pts::<4>(&chops[..]));
                                patches.write_cubic(fixed_pts::<4>(&chops[3..]));
                                patches.write_cubic(fixed_pts::<4>(&chops[6..]));
                            }
                        }
                    }
                }
            }
        }

        // Finish the last contour (next moveTo point doesn't matter).
        patches.write_deferred_stroke_patch(Point::new(0.0, 0.0), Some(cap));
    }

    // Port of: src/gpu/graphite/render/TessellateStrokesRenderStep.cpp#L250-L268 (chrome/m156)
    fn write_uniforms_and_textures(
        &self,
        params: &DrawParams,
        gatherer: &mut PipelineDataGatherer,
    ) {
        // TODO (Skia): Implement perspective.
        debug_assert!(!matches!(
            params.transform().type_(),
            crate::graphite::geom::transform::Type::Perspective
        ));

        let m = params.transform().matrix();
        // affineMatrix = float4 (2x2 of transform), translate = float2, maxScale = float.
        // Column-major 2x2 of the transform.
        let upper = [m.rc(0, 0), m.rc(1, 0), m.rc(0, 1), m.rc(1, 1)];
        let uniforms = gatherer.uniform_manager();
        uniforms.write_vec(upper);
        uniforms.write_vec([m.rc(0, 3), m.rc(1, 3)]);
        uniforms.write_f32(params.transform().max_scale_factor());
    }
}
