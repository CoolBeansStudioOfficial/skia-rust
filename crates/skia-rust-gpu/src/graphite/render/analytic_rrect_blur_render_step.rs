// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.h,
// src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp

//! [`AnalyticRRectBlurRenderStep`]: a blurred rounded rect, drawn as a 25-cell grid of the rect
//! (`write_vertex_buffer`). Each instance carries the cell bounds of its rect, and the fragment
//! shader evaluates the blur's CDF look-up table only in the cells the blur reaches.

use skia_rust_core::m44::V2;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;

use crate::graphite::attribute::{Attribute, Interpolation, Varying};
use crate::graphite::buffer_manager::{StaticBufferBinding, StaticBufferManager};
use crate::graphite::caps::ResourceBindingRequirements;
use crate::graphite::context_utils::emit_sampler_layout;
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{PrimitiveType, VertexAttribType};
use crate::graphite::draw_writer::{DrawWriter, Instances};
use crate::graphite::paint_params_key::RootNodesInfo;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::common_depth_stencil_settings::DIRECT_DEPTH_LESS_PASS;
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::{Layout, SamplerDesc};
use crate::graphite::uniform::Uniform;
use crate::sksl_type_shared::SkSLType;

/// `kVertexCount`: the 4 corners of 5 vertices and the 21 quads of 4 vertices.
// Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L28-L29 (chrome/m156)
const VERTEX_COUNT: usize = 104;
/// `kIndexCount`.
// Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L29 (chrome/m156)
const INDEX_COUNT: usize = 162;

/// `std::max(a, b)`: `(a < b) ? b : a`, which is not `f32::max` for NaN.
fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// `std::min(a, b)`: `(b < a) ? b : a`.
fn std_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

/// One vertex of the static grid (`struct Vertex`): `fGridAndBevel[4]` and `fCellID`.
// Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L28-L43 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Vertex {
    grid_and_bevel: [i32; 4],
    cell_id: u32,
}

/// `write_vertex_buffer`: the 104 vertices of the grid, corners first (TL, TR, BR, BL), then the
/// 21 remaining cells.
// Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L93-L177 (chrome/m156)
fn vertex_buffer() -> Vec<Vertex> {
    let v = |grid_and_bevel: [i32; 4], cell_id: u32| Vertex {
        grid_and_bevel,
        cell_id,
    };
    let mut vertices = Vec::with_capacity(VERTEX_COUNT);

    // Corner 0: TL, cell 0.
    vertices.push(v([0, 0, 1, 0], 0)); // v0
    vertices.push(v([1, 0, 0, 0], 0)); // v1
    vertices.push(v([1, 1, 0, 0], 0)); // v2
    vertices.push(v([0, 1, 0, 0], 0)); // v3
    vertices.push(v([0, 0, 0, 1], 0)); // v4

    // Corner 1: TR, cell 4.
    vertices.push(v([4, 0, 0, 0], 4)); // v5
    vertices.push(v([5, 0, -1, 0], 4)); // v6
    vertices.push(v([5, 0, 0, 1], 4)); // v7
    vertices.push(v([5, 1, 0, 0], 4)); // v8
    vertices.push(v([4, 1, 0, 0], 4)); // v9

    // Corner 2: BR, cell 24.
    vertices.push(v([4, 4, 0, 0], 24)); // v10
    vertices.push(v([5, 4, 0, 0], 24)); // v11
    vertices.push(v([5, 5, 0, -1], 24)); // v12
    vertices.push(v([5, 5, -1, 0], 24)); // v13
    vertices.push(v([4, 5, 0, 0], 24)); // v14

    // Corner 3: BL, cell 20.
    vertices.push(v([0, 4, 0, 0], 20)); // v15
    vertices.push(v([1, 4, 0, 0], 20)); // v16
    vertices.push(v([1, 5, 0, 0], 20)); // v17
    vertices.push(v([0, 5, 1, 0], 20)); // v18
    vertices.push(v([0, 5, 0, -1], 20)); // v19

    // 21 quads for the remaining cells.
    for row in 0..5_i32 {
        for col in 0..5_i32 {
            // Skip corners.
            if (row == 0 && col == 0)
                || (row == 0 && col == 4)
                || (row == 4 && col == 0)
                || (row == 4 && col == 4)
            {
                continue;
            }

            let mut c_id = (row * 5 + col) as u32;
            let x0 = col;
            let x1 = col + 1;
            let y0 = row;
            let y1 = row + 1;

            // Encode the corner safe edge bit.
            if row == 2 {
                if col == 0 {
                    c_id |= 1 << 28; // Left
                } else if col == 4 {
                    c_id |= 1 << 29; // Right
                }
            } else if col == 2 {
                if row == 0 {
                    c_id |= 1 << 30; // Top
                } else if row == 4 {
                    c_id |= 1 << 31; // Bottom
                }
            }

            vertices.push(v([x0, y0, 0, 0], c_id));
            vertices.push(v([x1, y0, 0, 0], c_id));
            vertices.push(v([x1, y1, 0, 0], c_id));
            vertices.push(v([x0, y1, 0, 0], c_id));
        }
    }
    vertices
}

/// `write_index_buffer`: the 162 indices of the grid.
// Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L179-L213 (chrome/m156)
fn index_buffer() -> Vec<u16> {
    let mut indices = vec![
        // Corner 0: TL Corner, fan from v2.
        2, 0, 1, 2, 4, 0, 2, 3, 4, // Corner 1: TR Corner, fan from v9.
        9, 5, 6, 9, 6, 7, 9, 7, 8, // Corner 2: BR Corner, fan from v10.
        10, 11, 12, 10, 12, 13, 10, 13, 14, // Corner 3: BL Corner, fan from v16.
        16, 17, 18, 16, 18, 19, 16, 19, 15,
    ];

    // Create remaining quads.
    let mut base: u16 = 20;
    for _ in 0..21 {
        indices.extend_from_slice(&[base, base + 1, base + 3, base + 1, base + 2, base + 3]);
        base += 4;
    }
    indices
}

/// The static attributes (`gridAndBevel`, `cellID`).
// Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L232-L233 (chrome/m156)
const STATIC_ATTRS: [Attribute; 2] = [
    Attribute::new("gridAndBevel", VertexAttribType::Int4, SkSLType::Int4),
    Attribute::new("cellID", VertexAttribType::UInt, SkSLType::UInt),
];

/// The per-instance attributes.
// Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L342-L353 (chrome/m156)
const APPEND_ATTRS: [Attribute; 10] = [
    Attribute::new("bounds0", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("bounds1", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("bounds2", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new(
        "cornerSafeBounds",
        VertexAttribType::Float4,
        SkSLType::Float4,
    ),
    Attribute::new("canSaturateEdge", VertexAttribType::UInt, SkSLType::UInt),
    Attribute::new("localToDevice0", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("localToDevice1", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("localToDevice2", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("depth", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
];

/// The uniforms (`rect`, `drawPad`, `sqrtHalfOverSigma`, `rrectRadii[2]`, `blurRadius`).
// Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L334-L340 (chrome/m156)
const UNIFORMS: [Uniform; 5] = [
    Uniform::new("rect", SkSLType::Float4),
    Uniform::new("drawPad", SkSLType::Float2),
    Uniform::new("sqrtHalfOverSigma", SkSLType::Half2),
    Uniform::new_array("rrectRadii", SkSLType::Float4, 2),
    Uniform::new("blurRadius", SkSLType::Float2),
];

/// The varyings: the scaled shape coordinates and the cell flags.
// Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L354-L356 (chrome/m156)
const VARYINGS: [Varying; 2] = [
    Varying::new(
        "scaledShapeCoords",
        SkSLType::Float2,
        Interpolation::Perspective,
    ),
    Varying::new("vFlags", SkSLType::Half4, Interpolation::Perspective),
];

/// `AnalyticRRectBlurRenderStep`: a blurred rounded rect, one instance per draw.
// Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.h#L16-L46 (chrome/m156)
#[doc(alias = "skgpu::graphite::AnalyticRRectBlurRenderStep")]
#[derive(Debug)]
pub struct AnalyticRRectBlurRenderStep {
    base: RenderStepBase,
    vertex_buffer: StaticBufferBinding,
    index_buffer: StaticBufferBinding,
}

impl AnalyticRRectBlurRenderStep {
    /// `AnalyticRRectBlurRenderStep(layout, bufferManager)`: writes the static grid into
    /// `buffer_manager`.
    // Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L215-L262 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout, buffer_manager: &mut StaticBufferManager) -> Self {
        let base = RenderStepBase::new(
            layout,
            RenderStepID::AnalyticRRectBlur,
            RenderStepFlags::PERFORMS_SHADING
                | RenderStepFlags::HAS_TEXTURES
                | RenderStepFlags::EMITS_COVERAGE
                | RenderStepFlags::APPEND_INSTANCES,
            &UNIFORMS,
            PrimitiveType::Triangles,
            DIRECT_DEPTH_LESS_PASS,
            &STATIC_ATTRS,
            &APPEND_ATTRS,
            &[],
            &VARYINGS,
        );

        // `Vertex` is 20 bytes: four i32 and a u32.
        let vertex_binding = StaticBufferBinding::new();
        if let Some(mut writer) = buffer_manager.get_vertex_writer(
            VERTEX_COUNT,
            std::mem::size_of::<Vertex>(),
            &vertex_binding,
        ) {
            for vertex in vertex_buffer() {
                writer.put(&vertex.grid_and_bevel).put(&vertex.cell_id);
            }
        }
        let index_binding = StaticBufferBinding::new();
        if let Some(mut writer) = buffer_manager
            .get_index_writer(std::mem::size_of::<u16>() * INDEX_COUNT, &index_binding)
        {
            let indices: [u16; INDEX_COUNT] = index_buffer()
                .try_into()
                .expect("the index buffer has INDEX_COUNT indices");
            writer.put(&indices);
        }

        Self {
            base,
            vertex_buffer: vertex_binding,
            index_buffer: index_binding,
        }
    }
}

impl RenderStep for AnalyticRRectBlurRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L264-L272 (chrome/m156)
    fn vertex_sksl(&self, _roots: &RootNodesInfo) -> String {
        "float4 devPosition = analytic_rrect_blur_vertex_fn(\
         gridAndBevel, cellID, rect, \
         bounds0, bounds1, bounds2, \
         cornerSafeBounds, canSaturateEdge, drawPad, rrectRadii, depth, \
         localToDevice0, localToDevice1, localToDevice2, \
         scaledShapeCoords, vFlags, stepLocalCoords);\n"
            .to_owned()
    }

    // Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L274-L277 (chrome/m156)
    fn textures_and_samplers_sksl(
        &self,
        binding_reqs: &ResourceBindingRequirements,
        next_binding_index: &mut i32,
    ) -> String {
        emit_sampler_layout(binding_reqs, next_binding_index) + " sampler2D cdfLut;"
    }

    // Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L279-L288 (chrome/m156)
    fn fragment_coverage_sksl(&self) -> &'static str {
        "outputCoverage = analytic_rrect_blur_coverage_fn(scaledShapeCoords, \
                                                          vFlags, \
                                                          rect, \
                                                          sqrtHalfOverSigma, \
                                                          rrectRadii, \
                                                          blurRadius, \
                                                          cdfLut);"
    }

    // Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L290-L392 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32) {
        let blur = params.geometry().analytic_rrect_blur_mask();
        let rrect = blur.rrect();
        let rect = *rrect.bounds();
        let radii = rrect.radii_ref();

        let draw_pad_x = blur.draw_pad_x();
        let draw_pad_y = blur.draw_pad_y();
        let local_sigma = blur.local_sigma();
        let sat_pad_x = (3.5_f32 * local_sigma.x).ceil();
        let sat_pad_y = (3.5_f32 * local_sigma.y).ceil();

        // Calculate distance from edge where the corner curvature has ended and is saturated.
        let safe_offset_left = std_max(radii[0].x, radii[3].x) + draw_pad_x;
        let safe_offset_right = std_max(radii[1].x, radii[2].x) + draw_pad_x;
        let safe_offset_top = std_max(radii[0].y, radii[1].y) + draw_pad_y;
        let safe_offset_bottom = std_max(radii[2].y, radii[3].y) + draw_pad_y;

        let corner_safe_x_min = rect.left + safe_offset_left;
        let corner_safe_x_max = rect.right - safe_offset_right;
        let corner_safe_y_min = rect.top + safe_offset_top;
        let corner_safe_y_max = rect.bottom - safe_offset_bottom;

        // Innermost safe bounds that we define as our fully saturated bounds.
        let mut ins_x_min = rect.left + std_max(sat_pad_x, safe_offset_left);
        let mut ins_x_max = rect.right - std_max(sat_pad_x, safe_offset_right);
        let mut ins_y_min = rect.top + std_max(sat_pad_y, safe_offset_top);
        let mut ins_y_max = rect.bottom - std_max(sat_pad_y, safe_offset_bottom);

        // Snap innermost inset bounds to the center if they are overlapping.
        if ins_x_min >= ins_x_max {
            ins_x_min = (rect.left + rect.right) * 0.5;
            ins_x_max = ins_x_min;
        }
        if ins_y_min >= ins_y_max {
            ins_y_min = (rect.top + rect.bottom) * 0.5;
            ins_y_max = ins_y_min;
        }

        // Our outermost edge safe inset bounds. This allows us to assume full coverage when we are
        // far enough from an edge on one axis and within the corner radius safe limits on the other
        // axis.
        let edge_ins_x_min = std_min(ins_x_min, rect.left + std_min(sat_pad_x, safe_offset_left));
        let edge_ins_x_max = std_max(
            ins_x_max,
            rect.right - std_min(sat_pad_x, safe_offset_right),
        );
        let edge_ins_y_min = std_min(ins_y_min, rect.top + std_min(sat_pad_y, safe_offset_top));
        let edge_ins_y_max = std_max(
            ins_y_max,
            rect.bottom - std_min(sat_pad_y, safe_offset_bottom),
        );

        // Bounds mapping to each row and columns of our 5x5 vertex grid.
        let x_bounds = [
            rect.left - draw_pad_x,
            edge_ins_x_min,
            ins_x_min,
            ins_x_max,
            edge_ins_x_max,
            rect.right + draw_pad_x,
        ];
        let y_bounds = [
            rect.top - draw_pad_y,
            edge_ins_y_min,
            ins_y_min,
            ins_y_max,
            edge_ins_y_max,
            rect.bottom + draw_pad_y,
        ];

        let mat = params.transform().matrix();

        // Bitmask determining which edges we can assume are fully saturated by the opposite edge.
        // Dependent on the current edge cell's blur not going past the opposite edge of the rect.
        // In order from least significant to most significant: left, right, top, and bottom.
        let mut can_saturate_edge: u32 = 0;
        if edge_ins_x_min + draw_pad_x < rect.right {
            can_saturate_edge |= 1;
        }
        if edge_ins_x_max - draw_pad_x > rect.left {
            can_saturate_edge |= 2;
        }
        if edge_ins_y_min + draw_pad_y < rect.bottom {
            can_saturate_edge |= 4;
        }
        if edge_ins_y_max - draw_pad_y > rect.top {
            can_saturate_edge |= 8;
        }

        let mut instances = Instances::new(
            writer,
            self.vertex_buffer.get(),
            self.index_buffer.get(),
            u32::try_from(INDEX_COUNT).expect("the index count fits in u32"),
        );
        let mut vw = instances.append(1);
        // bounds0, bounds1, bounds2 (the two bound arrays, back to back)
        vw.put(&x_bounds);
        vw.put(&y_bounds);
        // cornerSafeBounds
        vw.put(&[
            corner_safe_x_min,
            corner_safe_y_min,
            corner_safe_x_max,
            corner_safe_y_max,
        ]);
        vw.put(&can_saturate_edge);
        // localToDevice0, localToDevice1, localToDevice2
        vw.put(&[mat.rc(0, 0), mat.rc(1, 0), mat.rc(3, 0)]);
        vw.put(&[mat.rc(0, 1), mat.rc(1, 1), mat.rc(3, 1)]);
        vw.put(&[mat.rc(0, 3), mat.rc(1, 3), mat.rc(3, 3)]);
        vw.put(&params.order().depth_as_float());
        vw.put(&ssbo_index);
    }

    // Port of: src/gpu/graphite/render/AnalyticRRectBlurRenderStep.cpp#L394-L421 (chrome/m156)
    fn write_uniforms_and_textures(
        &self,
        params: &DrawParams,
        gatherer: &mut PipelineDataGatherer,
    ) {
        #[cfg(debug_assertions)]
        gatherer.check_rewind();

        let blur = params.geometry().analytic_rrect_blur_mask();
        let rrect = blur.rrect();
        let radii = rrect.radii_ref();

        let local_sigma = blur.local_sigma();
        // `(1.f / SK_FloatSqrt2) / localSigma.x`
        let sqrt_half_over_sigma = V2::new(
            (1.0_f32 / std::f32::consts::SQRT_2) / local_sigma.x,
            (1.0_f32 / std::f32::consts::SQRT_2) / local_sigma.y,
        );
        // `std::floor(std::ceil(6.f * localSigma.x) / 2.0)`: the division is in double.
        let blur_radius = V2::new(
            (f64::from((6.0_f32 * local_sigma.x).ceil()) / 2.0).floor() as f32,
            (f64::from((6.0_f32 * local_sigma.y).ceil()) / 2.0).floor() as f32,
        );

        {
            let uniforms = gatherer.uniform_manager();
            #[cfg(debug_assertions)]
            uniforms.set_expected_uniforms(self.uniforms(), false);

            uniforms.write_rect(rrect.bounds());
            let draw_pad = blur.draw_pad();
            uniforms.write_vec([draw_pad.x, draw_pad.y]);
            uniforms.write_half_vec([sqrt_half_over_sigma.x, sqrt_half_over_sigma.y]);
            let radii_arr = [
                [radii[0].x, radii[0].y, radii[1].x, radii[1].y],
                [radii[2].x, radii[2].y, radii[3].x, radii[3].y],
            ];
            uniforms.write_array_vec(&radii_arr);
            uniforms.write_vec([blur_radius.x, blur_radius.y]);

            #[cfg(debug_assertions)]
            uniforms.done_with_expected_uniforms();
        }

        gatherer.add(
            Some(blur.ref_cdf_proxy()),
            SamplerDesc::new(&SamplingOptions::from(FilterMode::Linear), TileMode::Clamp),
        );
    }
}
