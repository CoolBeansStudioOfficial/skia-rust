// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/CircularArcRenderStep.h, CircularArcRenderStep.cpp

//! [`CircularArcRenderStep`]: filled circular arcs (with or without the center), and stroked
//! circular arcs with butt or round caps that do not include the center point. Each draw is one
//! instance of an 18-vertex octagon template; the fragment shader evaluates the arc.

use skia_rust_core::paint::Cap;
use skia_rust_core::scalar::{
    SCALAR_PI, Scalar, degrees_to_radians, scalar_abs, scalar_cos, scalar_sin,
};
use skia_rust_simd::vx::{Float2, Float4};

use crate::graphite::attribute::{Attribute, Interpolation, Varying};
use crate::graphite::buffer::BindBufferInfo;
use crate::graphite::buffer_manager::{StaticBufferBinding, StaticBufferManager};
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{PrimitiveType, VertexAttribType};
use crate::graphite::draw_writer::{DrawWriter, Instances};
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::common_depth_stencil_settings::DIRECT_DEPTH_LESS_PASS;
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::Layout;
use crate::sksl_type_shared::SkSLType;

/// `SK_ScalarHalf`.
// Port of: include/core/SkScalar.h (SK_ScalarHalf)
const SK_SCALAR_HALF: f32 = 0.5;

/// Vertices in the octagon template (`kVertexCount`).
// Port of: src/gpu/graphite/render/CircularArcRenderStep.cpp#L66 (chrome/m156)
const VERTEX_COUNT: usize = 18;

// Intersection flags of the fragment shader (`kIntersection_NoRoundCaps` and `_RoundCaps`).
// Port of: src/gpu/graphite/render/CircularArcRenderStep.cpp#L199-L200 (chrome/m156)
const INTERSECTION_NO_ROUND_CAPS: f32 = 1.0;
const INTERSECTION_ROUND_CAPS: f32 = 2.0;

/// The `CircularArcRenderStep`.
// Port of: src/gpu/graphite/render/CircularArcRenderStep.h#L17-L44 (chrome/m156)
#[doc(alias = "skgpu::graphite::CircularArcRenderStep")]
#[derive(Debug)]
pub struct CircularArcRenderStep {
    base: RenderStepBase,
    vertex_buffer: StaticBufferBinding,
}

const STATIC_ATTRS: [Attribute; 1] = [Attribute::new(
    "position",
    VertexAttribType::Float3,
    SkSLType::Float3,
)];

const APPEND_ATTRS: [Attribute; 12] = [
    Attribute::new("centerScales", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("radiiAndFlags", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("geoClipPlane", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("fragClipPlane0", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("fragClipPlane1", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("inRoundCapPos", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("inRoundCapRadius", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("depth", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
    Attribute::new("mat0", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("mat1", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("mat2", VertexAttribType::Float3, SkSLType::Float3),
];

/// The octagon vertices of the static template: `(x, y, AA offset)` (`kOctagonVertices`).
// Port of: src/gpu/graphite/render/CircularArcRenderStep.cpp#L66-L99 (chrome/m156)
fn octagon_vertices() -> [[f32; 3]; VERTEX_COUNT] {
    const K_OCT_OFFSET: f32 = 0.414_213_56_f32; // sqrt(2) - 1
    const K_COS_PI8: f32 = 0.923_579_5_f32;
    const K_SIN_PI8: f32 = 0.382_683_43_f32;
    const K_OUTER_AA_OFFSET: f32 = 0.5;
    const K_INNER_AA_OFFSET: f32 = -0.5;
    [
        [-K_OCT_OFFSET, -1.0, K_OUTER_AA_OFFSET],
        [-K_SIN_PI8, -K_COS_PI8, K_INNER_AA_OFFSET],
        [K_OCT_OFFSET, -1.0, K_OUTER_AA_OFFSET],
        [K_SIN_PI8, -K_COS_PI8, K_INNER_AA_OFFSET],
        [1.0, -K_OCT_OFFSET, K_OUTER_AA_OFFSET],
        [K_COS_PI8, -K_SIN_PI8, K_INNER_AA_OFFSET],
        [1.0, K_OCT_OFFSET, K_OUTER_AA_OFFSET],
        [K_COS_PI8, K_SIN_PI8, K_INNER_AA_OFFSET],
        [K_OCT_OFFSET, 1.0, K_OUTER_AA_OFFSET],
        [K_SIN_PI8, K_COS_PI8, K_INNER_AA_OFFSET],
        [-K_OCT_OFFSET, 1.0, K_OUTER_AA_OFFSET],
        [-K_SIN_PI8, K_COS_PI8, K_INNER_AA_OFFSET],
        [-1.0, K_OCT_OFFSET, K_OUTER_AA_OFFSET],
        [-K_COS_PI8, K_SIN_PI8, K_INNER_AA_OFFSET],
        [-1.0, -K_OCT_OFFSET, K_OUTER_AA_OFFSET],
        [-K_COS_PI8, -K_SIN_PI8, K_INNER_AA_OFFSET],
        [-K_OCT_OFFSET, -1.0, K_OUTER_AA_OFFSET],
        [-K_SIN_PI8, -K_COS_PI8, K_INNER_AA_OFFSET],
    ]
}

impl CircularArcRenderStep {
    /// `CircularArcRenderStep(layout, bufferManager)`: writes the octagon template into
    /// `buffer_manager`.
    // Port of: src/gpu/graphite/render/CircularArcRenderStep.cpp#L101-L129 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout, buffer_manager: &mut StaticBufferManager) -> Self {
        let base = RenderStepBase::new(
            layout,
            RenderStepID::CircularArc,
            RenderStepFlags::PERFORMS_SHADING
                | RenderStepFlags::EMITS_COVERAGE
                | RenderStepFlags::OUTSET_BOUNDS_FOR_AA
                | RenderStepFlags::APPEND_INSTANCES,
            &[],
            PrimitiveType::TriangleStrip,
            DIRECT_DEPTH_LESS_PASS,
            &STATIC_ATTRS,
            &APPEND_ATTRS,
            &[],
            &[
                Varying::new("circleEdge", SkSLType::Float4, Interpolation::Perspective),
                Varying::new("clipPlane", SkSLType::Float3, Interpolation::Perspective),
                Varying::new("isectPlane", SkSLType::Float3, Interpolation::Perspective),
                Varying::new("unionPlane", SkSLType::Float3, Interpolation::Perspective),
                Varying::new(
                    "roundCapRadius",
                    SkSLType::Float,
                    Interpolation::Perspective,
                ),
                Varying::new("roundCapPos", SkSLType::Float4, Interpolation::Perspective),
            ],
        );

        // `Vertex` is one SkV3 position: 12 bytes.
        let vertex_buffer = StaticBufferBinding::new();
        if let Some(mut writer) = buffer_manager.get_vertex_writer(
            VERTEX_COUNT,
            3 * std::mem::size_of::<f32>(),
            &vertex_buffer,
        ) {
            writer.put(&octagon_vertices());
        }

        Self {
            base,
            vertex_buffer,
        }
    }
}

// The vector helpers below are the `SkV2`/`SkV3` operations used by the C++ code, written with
// the same operation order: `Normalize(v) = v * (1 / length(v))`, with `lengthSquared` as the
// dot product in `x*x + y*y` order.
// Port of: include/core/SkM44.h#L27 and #L68 (chrome/m156), SkV2/SkV3::Normalize

/// `SkV2::Normalize`.
#[must_use]
fn v2_normalize(v: [f32; 2]) -> [f32; 2] {
    let length = (v[0] * v[0] + v[1] * v[1]).sqrt();
    let s = 1.0_f32 / length;
    [v[0] * s, v[1] * s]
}

/// `SkV2 * s`.
#[must_use]
fn v2_scale(v: [f32; 2], s: f32) -> [f32; 2] {
    [v[0] * s, v[1] * s]
}

/// `-v`.
#[must_use]
fn v2_neg(v: [f32; 2]) -> [f32; 2] {
    [-v[0], -v[1]]
}

/// `SkV2::dot`.
#[must_use]
fn v2_dot(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

impl RenderStep for CircularArcRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/CircularArcRenderStep.cpp#L131-L142 (chrome/m156)
    fn vertex_sksl(&self) -> String {
        "float4 devPosition = circular_arc_vertex_fn(\
         position, \
         centerScales, radiiAndFlags, geoClipPlane, fragClipPlane0, fragClipPlane1, \
         inRoundCapPos, inRoundCapRadius, depth, float3x3(mat0, mat1, mat2), \
         circleEdge, clipPlane, isectPlane, unionPlane, \
         roundCapRadius, roundCapPos, \
         stepLocalCoords);\n"
            .to_owned()
    }

    // Port of: src/gpu/graphite/render/CircularArcRenderStep.cpp#L144-L150 (chrome/m156)
    fn fragment_coverage_sksl(&self) -> &'static str {
        "outputCoverage = circular_arc_coverage_fn(circleEdge, \
         clipPlane, \
         isectPlane, \
         unionPlane, \
         roundCapRadius, \
         roundCapPos);"
    }

    // Port of: src/gpu/graphite/render/CircularArcRenderStep.cpp#L152-L269 (chrome/m156)
    // The body follows `CircularArcRenderStep::writeVertices` line by line, so that the arithmetic
    // keeps its order; it is longer than the lint's limit for that reason.
    #[allow(clippy::too_many_lines)]
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32) {
        let mut instance = Instances::new(
            writer,
            self.vertex_buffer.get(),
            BindBufferInfo::default(),
            u32::try_from(VERTEX_COUNT).expect("the octagon template fits in u32"),
        );
        let mut vw = instance.append(1);

        let shape = params.geometry().shape();
        let arc = shape.arc();
        let oval = &arc.oval;
        let local_center = [oval.center_x(), oval.center_y()];
        let mut local_outer_radius: f32 = (oval.right - oval.left) / 2.0;
        let mut local_inner_radius: f32 = 0.0;

        let transform = params.transform();
        let radius = local_outer_radius * transform.max_scale_factor();

        let is_stroke = params.is_stroke();
        let mut inner_radius: f32 = -SK_SCALAR_HALF;
        let mut outer_radius: f32 = radius;
        if is_stroke {
            let mut local_half_width = params.stroke_style().half_width();
            let mut half_width = local_half_width * transform.max_scale_factor();
            if half_width.nearly_zero(None) {
                half_width = SK_SCALAR_HALF;
                local_half_width = half_width / transform.max_scale_factor();
            }
            outer_radius += half_width;
            inner_radius = radius - half_width;
            local_inner_radius = local_outer_radius - local_half_width;
            local_outer_radius += local_half_width;
        }
        outer_radius += SK_SCALAR_HALF;
        inner_radius -= SK_SCALAR_HALF;

        let start_angle_radians = degrees_to_radians(arc.start_angle);
        let sweep_angle_radians = degrees_to_radians(arc.sweep_angle);
        let mut local_points = [[0.0_f32; 2]; 3];
        local_points[0][1] = scalar_sin(start_angle_radians);
        local_points[0][0] = scalar_cos(start_angle_radians);
        let end_angle = start_angle_radians + sweep_angle_radians;
        local_points[1][1] = scalar_sin(end_angle);
        local_points[1][0] = scalar_cos(end_angle);
        local_points[2] = [0.0, 0.0];

        let local_in: [Float2; 3] = local_points.map(|p| Float2::new(p[0], p[1]));
        let mut dev_points = [Float4::new(0.0, 0.0, 0.0, 0.0); 3];
        transform.map_points_v2(&local_in, &mut dev_points);
        let mut start_point = [
            dev_points[0].x() - dev_points[2].x(),
            dev_points[0].y() - dev_points[2].y(),
        ];
        let mut stop_point = [
            dev_points[1].x() - dev_points[2].x(),
            dev_points[1].y() - dev_points[2].y(),
        ];
        start_point = v2_normalize(start_point);
        stop_point = v2_normalize(stop_point);

        let m = transform.matrix();
        let upper_left_det = m.rc(0, 0) * m.rc(1, 1) - m.rc(0, 1) * m.rc(1, 0);
        if upper_left_det < 0.0 {
            std::mem::swap(&mut start_point, &mut stop_point);
        }

        let abs_sweep = scalar_abs(sweep_angle_radians);
        let use_center =
            (arc.is_wedge() || is_stroke) && !(abs_sweep - SCALAR_PI).nearly_zero(None);

        let mut geo_clip_plane = [0.0_f32, 0.0, 1.0];
        // Both planes are set on every path below.
        let clip_plane0: [f32; 3];
        let clip_plane1: [f32; 3];
        let mut round_cap_pos0 = [0.0_f32; 2];
        let mut round_cap_pos1 = [0.0_f32; 2];
        let mut round_cap_radius: f32 = 0.0;
        let mut flags: f32 = INTERSECTION_NO_ROUND_CAPS;
        if is_stroke
            && params.stroke_style().half_width() > 0.0
            && params.stroke_style().cap() == Cap::Round
        {
            let mid_radius = (inner_radius + outer_radius) / (2.0 * outer_radius);
            round_cap_pos0 = v2_scale(start_point, mid_radius);
            round_cap_pos1 = v2_scale(stop_point, mid_radius);
            flags = INTERSECTION_ROUND_CAPS;
            round_cap_radius = (outer_radius - inner_radius) / (2.0 * outer_radius);
        }

        if use_center {
            let mut norm0 = [start_point[1], -start_point[0]];
            let mut norm1 = [stop_point[1], -stop_point[0]];
            if sweep_angle_radians < 0.0 {
                std::mem::swap(&mut norm0, &mut norm1);
            }
            norm0 = v2_neg(norm0);
            clip_plane0 = [norm0[0], norm0[1], 0.5];
            clip_plane1 = [norm1[0], norm1[1], 0.5];
            if abs_sweep > SCALAR_PI {
                flags = -flags;
            } else if !is_stroke && abs_sweep < 0.5 * SCALAR_PI {
                let mut local_norm0 = [local_points[0][1], -local_points[0][0]];
                let mut local_norm1 = [local_points[1][1], -local_points[1][0]];
                if sweep_angle_radians < 0.0 {
                    std::mem::swap(&mut local_norm0, &mut local_norm1);
                }
                let clip_norm = [
                    -local_norm0[1] - local_norm1[1],
                    local_norm1[0] + local_norm0[0],
                ];
                let clip_norm = v2_normalize(clip_norm);
                let dist = 0.5 / radius / transform.max_scale_factor();
                geo_clip_plane = [clip_norm[0], clip_norm[1], dist];
            }
        } else {
            start_point = v2_scale(start_point, radius);
            stop_point = v2_scale(stop_point, radius);
            let mut norm = v2_normalize([
                start_point[1] - stop_point[1],
                stop_point[0] - start_point[0],
            ]);
            if sweep_angle_radians > 0.0 {
                norm = v2_neg(norm);
            }
            let d = -v2_dot(norm, start_point) + 0.5;
            clip_plane0 = [norm[0], norm[1], d];
            clip_plane1 = [0.0, 0.0, 1.0]; // No clipping.
        }

        if is_stroke && inner_radius < -SK_SCALAR_HALF {
            inner_radius = -SK_SCALAR_HALF;
            local_inner_radius = 0.0;
        }
        inner_radius /= outer_radius;

        let center_scales = [
            local_center[0],
            local_center[1],
            local_outer_radius,
            local_inner_radius,
        ];
        let radii_and_flags = [outer_radius, inner_radius, flags];
        let in_round_cap_pos = [
            round_cap_pos0[0],
            round_cap_pos0[1],
            round_cap_pos1[0],
            round_cap_pos1[1],
        ];
        let depth = params.order().depth_as_float();
        vw.put(&center_scales)
            .put(&radii_and_flags)
            .put(&geo_clip_plane)
            .put(&clip_plane0)
            .put(&clip_plane1)
            .put(&in_round_cap_pos)
            .put(&round_cap_radius)
            .put(&depth)
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

    // Port of: src/gpu/graphite/render/CircularArcRenderStep.cpp#L271-L274 (chrome/m156)
    fn write_uniforms_and_textures(
        &self,
        _params: &DrawParams,
        _gatherer: &mut PipelineDataGatherer,
    ) {
        // All data is uploaded as instance attributes, so no uniforms are needed.
    }
}
