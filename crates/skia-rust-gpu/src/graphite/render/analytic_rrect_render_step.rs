// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/AnalyticRRectRenderStep.h, AnalyticRRectRenderStep.cpp

//! [`AnalyticRRectRenderStep`]: one step for filled rounded rectangles with elliptical corners,
//! filled quads with per-edge AA, stroked rectangles, rounded rectangles and lines with any join
//! and cap, and hairlines of all of those. Each draw is one instance of a 36-vertex template;
//! the instance attributes encode the shape (see the encoding table in the C++ header comment).

use skia_rust_core::paint::Cap;
use skia_rust_core::rrect::{Corner, RRect};
use skia_rust_simd::vx::{Float4, dot};

use crate::graphite::attribute::{Attribute, Interpolation, Varying};
use crate::graphite::buffer_manager::{StaticBufferBinding, StaticBufferManager};
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{PrimitiveType, VertexAttribType};
use crate::graphite::draw_writer::{DrawWriter, Instances};
use crate::graphite::geom::edge_aa_quad::{EdgeAAQuad, Flags as AAFlags};
use crate::graphite::geom::geometry::Geometry;
use crate::graphite::geom::rect::Rect;
use crate::graphite::geom::shape::Shape;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::common_depth_stencil_settings::DIRECT_DEPTH_LESS_PASS;
use crate::graphite::render::per_edge_aa_quad_render_step::is_clockwise;
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::Layout;
use crate::sksl_type_shared::SkSLType;

// Allowed values for the center weight instance value. When (instance weight > vertex weight) the
// vertex is snapped to the center instead of its regular calculation.
// Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L210-L214 (chrome/m156)
const K_SOLID_INTERIOR: f32 = 1.0;
const K_STROKE_INTERIOR: f32 = 0.0;
const K_FILLED_STROKE_INTERIOR: f32 = -1.0;

// Special local AA radius that signals the self-intersections of a stroke interior need extra
// calculations in the vertex shader.
// Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L216-L217 (chrome/m156)
const K_COMPLEX_AA_INSETS: f32 = -1.0;

// sk_VertexID is divided by the corner vertex count in SkSL.
// Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L219-L222 (chrome/m156)
const K_CORNER_VERTEX_COUNT: u16 = 9;
const VERTEX_COUNT: usize = 4 * K_CORNER_VERTEX_COUNT as usize;
const INDEX_COUNT: usize = 69;

/// One vertex of the static template (`struct Vertex`). Its fields are written in order, which is
/// the 28-byte layout of the C++ struct.
// Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L226-L232 (chrome/m156)
#[derive(Clone, Copy)]
struct Vertex {
    corner_id: u32,
    position: [f32; 2],
    normal: [f32; 2],
    normal_scale: f32,
    center_weight: f32,
}

/// The 9 vertices of one corner (`get_per_corner_vertex_attrs<kCornerID>`).
// Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L234-L271 (chrome/m156)
fn per_corner_vertex_attrs(corner_id: u32) -> [Vertex; K_CORNER_VERTEX_COUNT as usize] {
    // Allowed values for the normal scale: +1 is a device-space outset along the normal away
    // from the outer edge of the stroke, 0 is no outset on the outer edge, -1 is a local inset
    // from the inner edge.
    const K_OUTSET: f32 = 1.0;
    const K_INSET: f32 = -1.0;
    const K_CENTER: f32 = 1.0; // "true" as a float
    const ZERO: f32 = 0.0; // Named this way to help call out non-zero parameters.
    // "half root 2" (`0.5f * SK_FloatSqrt2`, with the same bits as `f32::consts::SQRT_2`).
    let hr2 = 0.5_f32 * std::f32::consts::SQRT_2;
    let v = |position, normal, normal_scale, center_weight| Vertex {
        corner_id,
        position,
        normal,
        normal_scale,
        center_weight,
    };
    [
        // Device-space AA outsets from the outer curve.
        v([1.0, 0.0], [1.0, 0.0], K_OUTSET, ZERO),
        v([1.0, 0.0], [hr2, hr2], K_OUTSET, ZERO),
        v([0.0, 1.0], [hr2, hr2], K_OUTSET, ZERO),
        v([0.0, 1.0], [0.0, 1.0], K_OUTSET, ZERO),
        // Outer anchors (no local or device-space normal outset).
        v([1.0, 0.0], [hr2, hr2], ZERO, ZERO),
        v([0.0, 1.0], [hr2, hr2], ZERO, ZERO),
        // Inner curve, with an additional AA inset in the common case.
        v([1.0, 0.0], [1.0, 0.0], K_INSET, ZERO),
        v([0.0, 1.0], [0.0, 1.0], K_INSET, ZERO),
        // Center filling vertices (equal to the inner AA insets unless 'center' triggers a fill).
        v([1.0, 0.0], [1.0, 0.0], K_INSET, K_CENTER),
    ]
}

/// The 69 indices of the template (`write_index_buffer`). The corners are TL, TR, BR, BL.
// Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L273-L296 (chrome/m156)
fn index_buffer() -> [u16; INDEX_COUNT] {
    const TL: u16 = 0;
    const TR: u16 = K_CORNER_VERTEX_COUNT;
    const BR: u16 = 2 * K_CORNER_VERTEX_COUNT;
    const BL: u16 = 3 * K_CORNER_VERTEX_COUNT;
    [
        // Exterior AA ramp outset.
        TL,
        TL + 4,
        TL + 1,
        TL + 5,
        TL + 2,
        TL + 3,
        TL + 5,
        TR,
        TR + 4,
        TR + 1,
        TR + 5,
        TR + 2,
        TR + 3,
        TR + 5,
        BR,
        BR + 4,
        BR + 1,
        BR + 5,
        BR + 2,
        BR + 3,
        BR + 5,
        BL,
        BL + 4,
        BL + 1,
        BL + 5,
        BL + 2,
        BL + 3,
        BL + 5,
        TL,
        TL + 4, // Close and jump to the next strip.
        // Outer to inner edges.
        TL + 4,
        TL + 6,
        TL + 5,
        TL + 7,
        TR + 4,
        TR + 6,
        TR + 5,
        TR + 7,
        BR + 4,
        BR + 6,
        BR + 5,
        BR + 7,
        BL + 4,
        BL + 6,
        BL + 5,
        BL + 7,
        TL + 4,
        TL + 6, // Close and jump to the next strip.
        // Fill triangles.
        TL + 6,
        TL + 8,
        TL + 7,
        TL + 7,
        TR + 8,
        TR + 6,
        TR + 8,
        TR + 7,
        TR + 7,
        BR + 8,
        BR + 6,
        BR + 8,
        BR + 7,
        BR + 7,
        BL + 8,
        BL + 6,
        BL + 8,
        BL + 7,
        BL + 7,
        TL + 8,
        TL + 6, // Close.
    ]
}

/// The `load_x_radii` helper: the X radii in top-left, clockwise order.
// Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L131-L136 (chrome/m156)
fn load_x_radii(rrect: &RRect) -> [f32; 4] {
    [
        rrect.radii(Corner::UpperLeft).x,
        rrect.radii(Corner::UpperRight).x,
        rrect.radii(Corner::LowerRight).x,
        rrect.radii(Corner::LowerLeft).x,
    ]
}

/// The `load_y_radii` helper: the Y radii in top-left, clockwise order.
// Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L137-L142 (chrome/m156)
fn load_y_radii(rrect: &RRect) -> [f32; 4] {
    [
        rrect.radii(Corner::UpperLeft).y,
        rrect.radii(Corner::UpperRight).y,
        rrect.radii(Corner::LowerRight).y,
        rrect.radii(Corner::LowerLeft).y,
    ]
}

/// `opposite_insets_intersect(const SkRRect&, ...)`: whether one inset per side would meet the
/// opposite corner's curve.
// Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L143-L160 (chrome/m156)
fn opposite_insets_intersect_rrect(rrect: &RRect, stroke_radius: f32, aa_radius: f32) -> bool {
    // One AA inset per side.
    let max_inset = stroke_radius + 2.0 * aa_radius;
    let width = rrect.width();
    let height = rrect.height();
    // Horizontal insets would intersect the opposite corner's curve.
    max_inset >= width - rrect.radii(Corner::LowerLeft).x
        || max_inset >= width - rrect.radii(Corner::LowerRight).x
        || max_inset >= width - rrect.radii(Corner::UpperLeft).x
        || max_inset >= width - rrect.radii(Corner::UpperRight).x
        // Vertical insets would intersect the opposite corner's curve.
        || max_inset >= height - rrect.radii(Corner::LowerLeft).y
        || max_inset >= height - rrect.radii(Corner::LowerRight).y
        || max_inset >= height - rrect.radii(Corner::UpperLeft).y
        || max_inset >= height - rrect.radii(Corner::UpperRight).y
}

/// `opposite_insets_intersect(const Rect&, ...)`: `any(rect.size() <= 2 * (strokeRadius +
/// aaRadius))`.
// Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L161-L164 (chrome/m156)
fn opposite_insets_intersect_rect(rect: &Rect, stroke_radius: f32, aa_radius: f32) -> bool {
    let size = rect.size();
    let limit = 2.0 * (stroke_radius + aa_radius);
    size.x() <= limit || size.y() <= limit
}

/// `opposite_insets_intersect(const Geometry&, ...)`.
// Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L165-L193 (chrome/m156)
fn opposite_insets_intersect_geometry(
    geometry: &Geometry,
    stroke_radius: f32,
    aa_radius: f32,
) -> bool {
    if geometry.is_edge_aa_quad() {
        debug_assert_eq!(stroke_radius, 0.0);
        let quad = geometry.edge_aa_quad();
        if quad.edge_flags() == AAFlags::NONE {
            // If all edges are non-AA there is no insetting, so completely non-AA quads can use
            // the fill triangles for simpler fragment shader work.
            false
        } else if quad.is_rect() && quad.edge_flags() == AAFlags::ALL {
            opposite_insets_intersect_rect(&quad.bounds(), 0.0, aa_radius)
        } else {
            // Quads with mixed AA edges are tiles where non-AA edges must seam perfectly together,
            // so the insets snap to the center, and the fill triangles cannot be used.
            true
        }
    } else {
        let shape: &Shape = geometry.shape();
        if shape.is_line() {
            stroke_radius <= aa_radius
        } else if shape.is_rect() {
            opposite_insets_intersect_rect(shape.rect(), stroke_radius, aa_radius)
        } else {
            opposite_insets_intersect_rrect(shape.rrect(), stroke_radius, aa_radius)
        }
    }
}

/// `quad_center(quad)`: the average of the four corners. The center of the bounding box is not
/// used, since it is biased for triangles.
// Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L212-L216 (chrome/m156)
fn quad_center(quad: &EdgeAAQuad) -> [f32; 2] {
    let quarter = Float4::new(0.25, 0.25, 0.25, 0.25);
    [dot(quad.xs(), quarter), dot(quad.ys(), quarter)]
}

/// The four lanes of a `Float4`.
fn lanes(v: Float4) -> [f32; 4] {
    [v.x(), v.y(), v.z(), v.w()]
}

/// The `AnalyticRRectRenderStep`.
// Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.h#L16-L45 (chrome/m156)
#[doc(alias = "skgpu::graphite::AnalyticRRectRenderStep")]
#[derive(Debug)]
pub struct AnalyticRRectRenderStep {
    base: RenderStepBase,
    vertex_buffer: StaticBufferBinding,
    index_buffer: StaticBufferBinding,
}

// Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L298-L334 (chrome/m156), the
// attribute lists
const STATIC_ATTRS: [Attribute; 5] = [
    Attribute::new("cornerID", VertexAttribType::UInt, SkSLType::UInt),
    Attribute::new("position", VertexAttribType::Float2, SkSLType::Float2),
    Attribute::new("normal", VertexAttribType::Float2, SkSLType::Float2),
    Attribute::new("normalScale", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("centerWeight", VertexAttribType::Float, SkSLType::Float),
];

const APPEND_ATTRS: [Attribute; 9] = [
    Attribute::new("xRadiiOrFlags", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("radiiOrQuadXs", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("ltrbOrQuadYs", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("center", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("depth", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
    Attribute::new("mat0", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("mat1", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("mat2", VertexAttribType::Float3, SkSLType::Float3),
];

impl AnalyticRRectRenderStep {
    /// `AnalyticRRectRenderStep(layout, bufferManager)`: writes the static template into
    /// `buffer_manager`.
    // Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L336-L372 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout, buffer_manager: &mut StaticBufferManager) -> Self {
        let base = RenderStepBase::new(
            layout,
            RenderStepID::AnalyticRRect,
            RenderStepFlags::PERFORMS_SHADING
                | RenderStepFlags::EMITS_COVERAGE
                | RenderStepFlags::OUTSET_BOUNDS_FOR_AA
                | RenderStepFlags::USE_NON_AA_INNER_FILL
                | RenderStepFlags::APPEND_INSTANCES,
            &[],
            PrimitiveType::TriangleStrip,
            DIRECT_DEPTH_LESS_PASS,
            &STATIC_ATTRS,
            &APPEND_ATTRS,
            &[],
            &[
                // float2x2
                Varying::new("jacobian", SkSLType::Float4, Interpolation::Perspective),
                // distance to the LTRB edges
                Varying::new(
                    "edgeDistances",
                    SkSLType::Float4,
                    Interpolation::Perspective,
                ),
                Varying::new("xRadii", SkSLType::Float4, Interpolation::Perspective),
                Varying::new("yRadii", SkSLType::Float4, Interpolation::Perspective),
                Varying::new("strokeParams", SkSLType::Float2, Interpolation::Perspective),
                Varying::new(
                    "perPixelControl",
                    SkSLType::Float2,
                    Interpolation::Perspective,
                ),
            ],
        );

        // `Vertex` is 28 bytes: a u32 corner ID, two float2, and two floats.
        let vertex_buffer = StaticBufferBinding::new();
        if let Some(mut writer) = buffer_manager.get_vertex_writer(
            VERTEX_COUNT,
            std::mem::size_of::<u32>() + 6 * std::mem::size_of::<f32>(),
            &vertex_buffer,
        ) {
            for corner in 0..4_u32 {
                for v in per_corner_vertex_attrs(corner) {
                    writer
                        .put(&v.corner_id)
                        .put(&v.position)
                        .put(&v.normal)
                        .put(&v.normal_scale)
                        .put(&v.center_weight);
                }
            }
        }
        let index_binding = StaticBufferBinding::new();
        if let Some(mut writer) = buffer_manager
            .get_index_writer(std::mem::size_of::<u16>() * INDEX_COUNT, &index_binding)
        {
            writer.put(&index_buffer());
        }

        Self {
            base,
            vertex_buffer,
            index_buffer: index_binding,
        }
    }
}

impl RenderStep for AnalyticRRectRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L374-L386 (chrome/m156)
    fn vertex_sksl(&self) -> String {
        concat!(
            "float4 devPosition = analytic_rrect_vertex_fn(",
            "cornerID, position, normal, normalScale, centerWeight, ",
            "xRadiiOrFlags, radiiOrQuadXs, ltrbOrQuadYs, center, depth, ",
            "float3x3(mat0, mat1, mat2), ",
            "jacobian, edgeDistances, xRadii, yRadii, strokeParams, perPixelControl, ",
            "stepLocalCoords);\n"
        )
        .to_owned()
    }

    // Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L388-L396 (chrome/m156)
    fn fragment_coverage_sksl(&self) -> &'static str {
        concat!(
            "outputCoverage = analytic_rrect_coverage_fn(sk_FragCoord, ",
            "jacobian, ",
            "edgeDistances, ",
            "xRadii, ",
            "yRadii, ",
            "strokeParams, ",
            "perPixelControl);"
        )
    }

    // Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L398-L552 (chrome/m156)
    // The body follows `writeVertices` line by line. The debug-only `SkASSERT`s on the shape's
    // circularity (`SkRRectPriv::AllCornersRelativelyCircular`) are not ported.
    #[allow(clippy::too_many_lines)]
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32) {
        let mut instance = Instances::new(
            writer,
            self.vertex_buffer.get(),
            self.index_buffer.get(),
            u32::try_from(INDEX_COUNT).expect("the index count fits in u32"),
        );
        let mut vw = instance.append(1);

        let geometry = params.geometry();
        let bounds = geometry.bounds();
        let mut aa_radius = params.transform().local_aa_radius(&bounds);
        let mut stroke_inset = 0.0_f32;
        let mut center_weight = K_SOLID_INTERIOR;

        // The three float4 instance attributes that describe the shape. Each is set on every path.
        let x_radii_or_flags: [f32; 4];
        let radii_or_quad_xs: [f32; 4];
        let ltrb_or_quad_ys: [f32; 4];

        if params.is_stroke() {
            let shape = geometry.shape();
            let stroke_radius = params.stroke_style().half_width();
            // Rect or [r]rect: the bounds' size. A line uses its length and zero.
            let size: [f32; 2] = if shape.is_line() {
                let p0 = shape.p0();
                let p1 = shape.p1();
                let d = [p1.x() - p0.x(), p1.y() - p0.y()];
                [(d[0] * d[0] + d[1] * d[1]).sqrt(), 0.0]
            } else {
                [bounds.size().x(), bounds.size().y()]
            };
            let inner_gap = [
                size[0] - 2.0 * params.stroke_style().half_width(),
                size[1] - 2.0 * params.stroke_style().half_width(),
            ];
            if (inner_gap[0] <= 0.0 || inner_gap[1] <= 0.0) && stroke_radius > 0.0 {
                stroke_inset = -stroke_radius;
            } else {
                center_weight = K_STROKE_INTERIOR;
                stroke_inset = stroke_radius;
            }

            let x_radii = if shape.is_rrect() {
                load_x_radii(shape.rrect())
            } else {
                [0.0; 4]
            };
            if stroke_radius > 0.0 || shape.is_line() {
                let mut join_style = params.stroke_style().join_limit();
                let line_flag = if shape.is_line() { 1.0 } else { 0.0 };
                let empty = [size[0] == 0.0, size[1] == 0.0];
                if shape.is_line() || (empty[0] && empty[1]) {
                    debug_assert!(shape.is_line() || params.stroke_style().cap() != Cap::Butt);
                    join_style = match params.stroke_style().cap() {
                        Cap::Round => -1.0, // Round cap == round join.
                        Cap::Butt => 0.0,   // Butt cap == bevel join.
                        Cap::Square => 1.0, // Square cap == miter join.
                    };
                } else if params.stroke_style().is_miter_join() {
                    if params.stroke_style().miter_limit() < std::f32::consts::SQRT_2
                        || (empty[0] || empty[1])
                    {
                        join_style = 0.0; // Bevel (or butt if the width or height is zero).
                    } else {
                        join_style = 1.0;
                    }
                }
                // Otherwise no join correction is needed for non-empty geometry or round joins.
                x_radii_or_flags = [-2.0, line_flag, stroke_radius, join_style];
                radii_or_quad_xs = x_radii;
                ltrb_or_quad_ys = if shape.is_line() {
                    lanes(shape.line())
                } else {
                    lanes(bounds.ltrb())
                };
            } else {
                let y_radii = if shape.is_rrect() {
                    load_y_radii(shape.rrect())
                } else {
                    [0.0; 4]
                };
                x_radii_or_flags = [
                    -2.0 - x_radii[0],
                    -2.0 - x_radii[1],
                    -2.0 - x_radii[2],
                    -2.0 - x_radii[3],
                ];
                radii_or_quad_xs = y_radii;
                ltrb_or_quad_ys = lanes(bounds.ltrb());
            }
        } else {
            debug_assert!(!bounds.is_empty_negative_or_nan());
            if geometry.is_edge_aa_quad() {
                let quad = geometry.edge_aa_quad();
                if quad.edge_flags() == AAFlags::NONE {
                    aa_radius = 0.0;
                }
                let flags = quad.edge_flags();
                let sign = |flag: AAFlags| if flags.contains(flag) { -1.0 } else { 0.0 };
                let edge_signs = [
                    sign(AAFlags::LEFT),
                    sign(AAFlags::TOP),
                    sign(AAFlags::RIGHT),
                    sign(AAFlags::BOTTOM),
                ];
                let xs = lanes(quad.xs());
                let ys = lanes(quad.ys());
                if is_clockwise(quad) {
                    x_radii_or_flags = edge_signs;
                    radii_or_quad_xs = xs;
                    ltrb_or_quad_ys = ys;
                } else {
                    // Swap left and right AA bits, swap TL with TR and BL with BR.
                    x_radii_or_flags = [edge_signs[2], edge_signs[1], edge_signs[0], edge_signs[3]];
                    radii_or_quad_xs = [xs[1], xs[0], xs[3], xs[2]];
                    ltrb_or_quad_ys = [ys[1], ys[0], ys[3], ys[2]];
                }
            } else {
                let shape = geometry.shape();
                debug_assert!(!shape.is_line());
                if shape.is_rect() || (shape.is_rrect() && shape.rrect().is_rect()) {
                    let ltrb = lanes(bounds.ltrb());
                    // Edge flags: -1 on every edge. Xs are [L, R, R, L] and Ys are [T, T, B, B].
                    x_radii_or_flags = [-1.0; 4];
                    radii_or_quad_xs = [ltrb[0], ltrb[2], ltrb[2], ltrb[0]];
                    ltrb_or_quad_ys = [ltrb[1], ltrb[1], ltrb[3], ltrb[3]];
                } else {
                    debug_assert!(load_x_radii(shape.rrect()).iter().any(|&r| r > 0.0));
                    x_radii_or_flags = load_x_radii(shape.rrect());
                    radii_or_quad_xs = load_y_radii(shape.rrect());
                    ltrb_or_quad_ys = lanes(bounds.ltrb());
                }
            }
        }

        if opposite_insets_intersect_geometry(geometry, stroke_inset, aa_radius) {
            aa_radius = K_COMPLEX_AA_INSETS;
            if center_weight == K_STROKE_INTERIOR {
                center_weight = K_FILLED_STROKE_INTERIOR;
            }
        }

        let m = params.transform().matrix();
        let center = if geometry.is_edge_aa_quad() {
            quad_center(geometry.edge_aa_quad())
        } else {
            let c = bounds.center();
            [c.x(), c.y()]
        };
        let center_attr = [center[0], center[1], center_weight, aa_radius];
        let depth = params.order().depth_as_float();
        vw.put(&x_radii_or_flags)
            .put(&radii_or_quad_xs)
            .put(&ltrb_or_quad_ys)
            .put(&center_attr)
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

    // Port of: src/gpu/graphite/render/AnalyticRRectRenderStep.cpp#L554-L557 (chrome/m156)
    fn write_uniforms_and_textures(
        &self,
        _params: &DrawParams,
        _gatherer: &mut PipelineDataGatherer,
    ) {
        // All data is uploaded as instance attributes, so no uniforms are needed.
    }
}
