// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.h, PerEdgeAAQuadRenderStep.cpp

//! [`PerEdgeAAQuadRenderStep`]: filled convex quadrilaterals with per-edge anti-aliasing. Each
//! draw is one instance; the four corners come from a static vertex template.

use crate::graphite::attribute::{Attribute, Interpolation, Varying};
use crate::graphite::buffer_manager::{StaticBufferBinding, StaticBufferManager};
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{PrimitiveType, VertexAttribType};
use crate::graphite::draw_writer::{DrawWriter, Instances};
use crate::graphite::geom::edge_aa_quad::{EdgeAAQuad, Flags as AAFlags};
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::common_depth_stencil_settings::DIRECT_DEPTH_LESS_PASS;
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::Layout;
use crate::sksl_type_shared::SkSLType;

/// Coverage sign of an anti-aliased edge (`kAAOn`).
const AA_ON: u8 = 255;
/// Coverage sign of a non-anti-aliased edge (`kAAOff`).
const AA_OFF: u8 = 0;

/// Vertices per corner (`kCornerVertexCount`).
// Port of: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.cpp (kCornerVertexCount)
const CORNER_VERTEX_COUNT: u16 = 4;
/// Vertices in the template (`kVertexCount`).
// Port of: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.cpp (kVertexCount)
const VERTEX_COUNT: usize = 4 * CORNER_VERTEX_COUNT as usize;
/// Indices in the template (`kIndexCount`).
// Port of: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.cpp (kIndexCount)
const INDEX_COUNT: usize = 29;

/// `Vertex` in the static vertex template: a corner ID and the normal of one vertex.
// Port of: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.cpp (struct Vertex)
#[derive(Clone, Copy)]
struct Vertex {
    corner_id: u32,
    normal: [f32; 2],
}

/// `get_per_corner_vertex_attrs<kCornerID>()`.
// Port of: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.cpp (get_per_corner_vertex_attrs)
fn per_corner_vertex_attrs(corner_id: u32) -> [Vertex; CORNER_VERTEX_COUNT as usize] {
    // "half root 2".
    // `SK_FloatSqrt2` (1.41421356f) has the same bits as `f32::consts::SQRT_2`.
    let hr2 = 0.5_f32 * std::f32::consts::SQRT_2;
    [
        // Normals for device-space AA outsets from the outer curve.
        Vertex {
            corner_id,
            normal: [1.0, 0.0],
        },
        Vertex {
            corner_id,
            normal: [hr2, hr2],
        },
        Vertex {
            corner_id,
            normal: [0.0, 1.0],
        },
        // Normal for the outer anchor (zero length: no local or device-space normal outset).
        Vertex {
            corner_id,
            normal: [0.0, 0.0],
        },
    ]
}

/// `write_index_buffer`: the 29 indices of the template.
// Port of: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.cpp (write_index_buffer)
fn index_buffer() -> [u16; INDEX_COUNT] {
    let tl: u16 = 0;
    let tr: u16 = CORNER_VERTEX_COUNT;
    let br: u16 = 2 * CORNER_VERTEX_COUNT;
    let bl: u16 = 3 * CORNER_VERTEX_COUNT;
    [
        // Exterior AA ramp outset.
        tl + 1,
        tl + 2,
        tl + 3,
        tr,
        tr + 3,
        tr + 1,
        tr + 1,
        tr + 2,
        tr + 3,
        br,
        br + 3,
        br + 1,
        br + 1,
        br + 2,
        br + 3,
        bl,
        bl + 3,
        bl + 1,
        bl + 1,
        bl + 2,
        bl + 3,
        tl,
        tl + 3,
        tl + 1,
        tl + 3,
        // Fill triangles.
        tl + 3,
        tr + 3,
        bl + 3,
        br + 3,
    ]
}

// Port of: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.cpp#L20-L60 (chrome/m156), the
// per-edge quad attributes (`appendAttrs`, `staticAttrs`) and the varying
const STATIC_ATTRS: [Attribute; 2] = [
    Attribute::new("cornerID", VertexAttribType::UInt, SkSLType::UInt),
    Attribute::new("normal", VertexAttribType::Float2, SkSLType::Float2),
];

const APPEND_ATTRS: [Attribute; 8] = [
    Attribute::new("edgeFlags", VertexAttribType::UByte4Norm, SkSLType::Float4),
    Attribute::new("quadXs", VertexAttribType::Float4, SkSLType::Float4),
    Attribute::new("quadYs", VertexAttribType::Float4, SkSLType::Float4),
    // TODO (Skia): pack depth and ssbo index into one 32-bit attribute, if we can go without
    // needing both render step and paint ssbo index attributes.
    Attribute::new("depth", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
    Attribute::new("mat0", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("mat1", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("mat2", VertexAttribType::Float3, SkSLType::Float3),
];

/// `PerEdgeAAQuadRenderStep`: draws filled quads with per-edge AA.
// Port of: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.h#L17-L47 (chrome/m156)
#[doc(alias = "skgpu::graphite::PerEdgeAAQuadRenderStep")]
#[derive(Debug)]
pub struct PerEdgeAAQuadRenderStep {
    base: RenderStepBase,
    vertex_buffer: StaticBufferBinding,
    index_buffer: StaticBufferBinding,
}

impl PerEdgeAAQuadRenderStep {
    /// `PerEdgeAAQuadRenderStep(layout, bufferManager)`: writes the static vertex and index
    /// templates into `buffer_manager`.
    // Port of: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.cpp#L178-L200 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout, buffer_manager: &mut StaticBufferManager) -> Self {
        let base = RenderStepBase::new(
            layout,
            RenderStepID::PerEdgeAAQuad,
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
            &[Varying::new(
                "edgeDistances", // distance to LTRB edges
                SkSLType::Float4,
                Interpolation::Perspective,
            )],
        );

        // Initialize the static buffers used when recording draw calls. Each instance of this step
        // gets its own copy of the data.
        let vertex_buffer = StaticBufferBinding::new();
        let index_buffer_binding = StaticBufferBinding::new();
        let vertex_stride = std::mem::size_of::<u32>() + 2 * std::mem::size_of::<f32>();
        if let Some(mut writer) =
            buffer_manager.get_vertex_writer(VERTEX_COUNT, vertex_stride, &vertex_buffer)
        {
            // `write_vertex_buffer`: the four corner templates, TL -> TR -> BR -> BL.
            for corner in 0..4_u32 {
                for v in per_corner_vertex_attrs(corner) {
                    writer.put(&v.corner_id).put(&v.normal);
                }
            }
        }
        if let Some(mut writer) = buffer_manager.get_index_writer(
            std::mem::size_of::<u16>() * INDEX_COUNT,
            &index_buffer_binding,
        ) {
            writer.put(&index_buffer());
        }

        Self {
            base,
            vertex_buffer,
            index_buffer: index_buffer_binding,
        }
    }
}

/// `is_clockwise(quad)`: whether the quad's vertices are already clockwise.
// Port of: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.cpp#L100-L119 (chrome/m156)
fn is_clockwise(quad: &EdgeAAQuad) -> bool {
    if quad.is_rect() {
        return true; // By construction, these are always locally clockwise.
    }
    // This assumes that each corner has a consistent winding, which holds for convex inputs. Check
    // the sign of the cross product between the first two edges.
    let xs = quad.xs();
    let ys = quad.ys();
    let (x, y) = (lanes(xs), lanes(ys));
    let mut winding = (x[0] - x[3]) * (y[1] - y[0]) - (y[0] - y[3]) * (x[1] - x[0]);
    if winding == 0.0 {
        // The input possibly forms a triangle with duplicate vertices, so check the opposite
        // corner.
        winding = (x[2] - x[1]) * (y[3] - y[2]) - (y[2] - y[1]) * (x[3] - x[2]);
    }
    // If winding is < 0 the vertices are CCW. If it is still 0 they form a line, in which case the
    // vertex shader constructs a correct CW winding. Otherwise the winding is positive: CW.
    winding >= 0.0
}

/// The four lanes of a `Float4` as an array.
fn lanes(v: skia_rust_simd::vx::Float4) -> [f32; 4] {
    [v.x(), v.y(), v.z(), v.w()]
}

impl RenderStep for PerEdgeAAQuadRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.cpp#L202-L213 (chrome/m156)
    fn vertex_sksl(&self) -> String {
        // The body of a vertex function: it defines `float4 devPosition` and writes the
        // already-defined `float2 stepLocalCoords`.
        "float4 devPosition = per_edge_aa_quad_vertex_fn(\
         cornerID, normal, \
         edgeFlags, quadXs, quadYs, depth, \
         float3x3(mat0, mat1, mat2), \
         edgeDistances, \
         stepLocalCoords);\n"
            .to_owned()
    }

    // Port of: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.cpp#L215-L220 (chrome/m156)
    fn fragment_coverage_sksl(&self) -> &'static str {
        // The returned SkSL must write its coverage into a 'half4 outputCoverage' variable (defined
        // in the calling code) with the actual coverage splatted out into all four channels.
        "outputCoverage = per_edge_aa_quad_coverage_fn(sk_FragCoord, edgeDistances);"
    }

    // Port of: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.cpp#L222-L255 (chrome/m156)
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32) {
        let quad = params.geometry().edge_aa_quad();
        let mut instance = Instances::new(
            writer,
            self.vertex_buffer.get(),
            self.index_buffer.get(),
            u32::try_from(INDEX_COUNT).expect("index count fits in u32"),
        );
        let mut vw = instance.append(1);

        // Empty fills should not have been recorded at all.
        let flags = quad.edge_flags();
        let sign = |flag: AAFlags| if flags.contains(flag) { AA_ON } else { AA_OFF };
        let edge_signs: [u8; 4] = [
            sign(AAFlags::LEFT),
            sign(AAFlags::TOP),
            sign(AAFlags::RIGHT),
            sign(AAFlags::BOTTOM),
        ];
        let xs = lanes(quad.xs());
        let ys = lanes(quad.ys());

        // The vertex shader expects points in clockwise order. EdgeAAQuad is the only shape that
        // might have counter-clockwise input.
        if is_clockwise(quad) {
            vw.put(&edge_signs).put(&xs).put(&ys);
        } else {
            // Swap left and right AA bits, swap TL with TR and BL with BR.
            vw.put(&[edge_signs[2], edge_signs[1], edge_signs[0], edge_signs[3]])
                .put(&[xs[1], xs[0], xs[3], xs[2]])
                .put(&[ys[1], ys[0], ys[3], ys[2]]);
        }

        // All instance types share the remaining instance attribute definitions.
        let m = params.transform().matrix();
        vw.put(&params.order().depth_as_float())
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

    // Port of: src/gpu/graphite/render/PerEdgeAAQuadRenderStep.cpp#L257-L262 (chrome/m156)
    fn write_uniforms_and_textures(
        &self,
        _params: &DrawParams,
        _gatherer: &mut PipelineDataGatherer,
    ) {
        // All data is uploaded as instance attributes, so no uniforms are needed.
    }
}
