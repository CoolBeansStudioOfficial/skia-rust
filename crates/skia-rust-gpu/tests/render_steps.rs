// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Vertex and instance data of the ported `RenderStep`s, read back through `DrawWriter`.
//!
//! Skia has no unit tests for these steps. The expectations here are written by hand from the C++
//! of `CoverBoundsRenderStep.cpp`, `PerEdgeAAQuadRenderStep.cpp`, `CircularArcRenderStep.cpp` and
//! `DrawWriter.cpp`: the instance layout (the attribute order of each step's constructor), the
//! inverse-fill bounds (`shuffle<2,3,0,1>` of the scissor), the clockwise swap of `EdgeAAQuad`
//! corners, and the arc constants. Floats are compared as their native-endian bytes, the form in
//! which `BufferWriter` stores them.

mod support;

use skia_rust_core::arc::Arc;
use skia_rust_core::color::Color;
use skia_rust_core::m44::M44;
use skia_rust_core::paint::{Cap, Join};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect as SkRect};
use skia_rust_core::rrect::RRect;
use skia_rust_core::vertices::{VertexMode, Vertices as SkVertices};
use skia_rust_gpu::gpu::gpu_types::Protected;
use skia_rust_gpu::graphite::buffer::{BindBufferInfo, Buffer};
use skia_rust_gpu::graphite::buffer_manager::{
    StaticBufferHost, StaticBufferManager, StaticFinishResult,
};
use skia_rust_gpu::graphite::draw_order::{DrawOrder, PaintersDepth};
use skia_rust_gpu::graphite::draw_params::{Clip, DrawParams, StrokeStyle};
use skia_rust_gpu::graphite::draw_types::{
    BarrierType, DrawTypeFlags, PrimitiveType, RenderStateFlags,
};
use skia_rust_gpu::graphite::draw_writer::{DrawPassCommandList, DrawWriter, Vertices};
use skia_rust_gpu::graphite::geom::edge_aa_quad::{EdgeAAQuad, Flags as AAFlags};
use skia_rust_gpu::graphite::geom::geometry::Geometry;
use skia_rust_gpu::graphite::geom::non_msaa_clip::NonMSAAClip;
use skia_rust_gpu::graphite::geom::rect::Rect;
use skia_rust_gpu::graphite::geom::shape::Shape;
use skia_rust_gpu::graphite::geom::transform::Transform;
use skia_rust_gpu::graphite::graphite_types::DepthStencilFlags;
use skia_rust_gpu::graphite::paint_params_key::RootNodesInfo;
use skia_rust_gpu::graphite::render::analytic_rrect_render_step::AnalyticRRectRenderStep;
use skia_rust_gpu::graphite::render::circular_arc_render_step::CircularArcRenderStep;
use skia_rust_gpu::graphite::render::common_depth_stencil_settings::{
    DIRECT_DEPTH_LESS_PASS, REGULAR_COVER_PASS,
};
use skia_rust_gpu::graphite::render::cover_bounds_render_step::CoverBoundsRenderStep;
use skia_rust_gpu::graphite::render::middle_out_fan_render_step::MiddleOutFanRenderStep;
use skia_rust_gpu::graphite::render::per_edge_aa_quad_render_step::PerEdgeAAQuadRenderStep;
use skia_rust_gpu::graphite::render::tessellate_curves_render_step::TessellateCurvesRenderStep;
use skia_rust_gpu::graphite::render::tessellate_strokes_render_step::TessellateStrokesRenderStep;
use skia_rust_gpu::graphite::render::tessellate_wedges_render_step::TessellateWedgesRenderStep;
use skia_rust_gpu::graphite::render::vertices_render_step::VerticesRenderStep;
use skia_rust_gpu::graphite::render_step::{Coverage, RenderStep, RenderStepFlags, RenderStepID};
use skia_rust_gpu::graphite::renderer_provider::RendererProvider;
use skia_rust_gpu::graphite::resource::ResourceRef;
use skia_rust_gpu::graphite::resource_types::Layout;
use skia_rust_gpu::graphite::task::TaskRef;
use skia_rust_gpu::graphite::upload_buffer_manager::UploadBufferManager;

use support::{MockCaps, committed_bytes, make_recorder, shared_provider};

/// A static buffer manager on the mock back end, as the render steps' constructors need.
fn static_manager() -> StaticBufferManager {
    let (provider, _) = shared_provider();
    StaticBufferManager::new(provider, &MockCaps::default())
}

/// The `Context` side of `StaticBufferManager::finalize()`: the copy tasks are accepted and
/// dropped, since these tests read the bindings and not the copied bytes.
struct NoopStaticHost;

impl StaticBufferHost for NoopStaticHost {
    fn add_upload_buffer_manager_refs(&mut self, _upload_manager: &mut UploadBufferManager) {}

    fn add_task(&mut self, _task: &TaskRef, is_protected: Protected) -> bool {
        assert_eq!(is_protected, Protected::No);
        true
    }

    fn add_static_resource(&mut self, _buffer: ResourceRef<Buffer>) {}
}

/// Binds the static templates the steps wrote: `finalize()` is what makes their bindings valid.
fn finalize(manager: &mut StaticBufferManager) {
    assert_eq!(
        manager.finalize(&mut NoopStaticHost),
        StaticFinishResult::Success
    );
}

/// One command the `DrawWriter` recorded.
// (The C++ `DrawPassCommands::List` calls that DrawWriter.cpp makes, recorded for inspection.)
#[derive(Debug, Clone, PartialEq)]
enum Call {
    BindAppend(BindBufferInfo),
    BindStatic(BindBufferInfo),
    BindIndex(BindBufferInfo),
    Barrier(BarrierType),
    Draw {
        primitive: PrimitiveType,
        base_vertex: u32,
        vertex_count: u32,
    },
    DrawIndexed {
        primitive: PrimitiveType,
        base_index: u32,
        index_count: u32,
        base_vertex: u32,
    },
    DrawInstanced {
        primitive: PrimitiveType,
        base_vertex: u32,
        vertex_count: u32,
        base_instance: u32,
        instance_count: u32,
    },
    DrawIndexedInstanced {
        primitive: PrimitiveType,
        base_index: u32,
        index_count: u32,
        base_vertex: u32,
        base_instance: u32,
        instance_count: u32,
    },
}

#[derive(Default)]
struct RecordingList(Vec<Call>);

impl DrawPassCommandList for RecordingList {
    fn bind_append_data_buffer(&mut self, append_attribs: BindBufferInfo) {
        self.0.push(Call::BindAppend(append_attribs));
    }
    fn bind_static_data_buffer(&mut self, static_attribs: BindBufferInfo) {
        self.0.push(Call::BindStatic(static_attribs));
    }
    fn bind_index_buffer(&mut self, indices: BindBufferInfo) {
        self.0.push(Call::BindIndex(indices));
    }
    fn add_barrier(&mut self, barrier: BarrierType) {
        self.0.push(Call::Barrier(barrier));
    }
    fn draw(&mut self, primitive: PrimitiveType, base_vertex: u32, vertex_count: u32) {
        self.0.push(Call::Draw {
            primitive,
            base_vertex,
            vertex_count,
        });
    }
    fn draw_indexed(
        &mut self,
        primitive: PrimitiveType,
        base_index: u32,
        index_count: u32,
        base_vertex: u32,
    ) {
        self.0.push(Call::DrawIndexed {
            primitive,
            base_index,
            index_count,
            base_vertex,
        });
    }
    fn draw_instanced(
        &mut self,
        primitive: PrimitiveType,
        base_vertex: u32,
        vertex_count: u32,
        base_instance: u32,
        instance_count: u32,
    ) {
        self.0.push(Call::DrawInstanced {
            primitive,
            base_vertex,
            vertex_count,
            base_instance,
            instance_count,
        });
    }
    fn draw_indexed_instanced(
        &mut self,
        primitive: PrimitiveType,
        base_index: u32,
        index_count: u32,
        base_vertex: u32,
        base_instance: u32,
        instance_count: u32,
    ) {
        self.0.push(Call::DrawIndexedInstanced {
            primitive,
            base_index,
            index_count,
            base_vertex,
            base_instance,
            instance_count,
        });
    }
}

/// The native-endian bytes of a slice of `f32`, as `BufferWriter` stores them.
fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

/// The native-endian bytes of a slice of `u32`.
fn u32_bytes(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

/// The `DrawParams` of a draw with an identity transform and a clip that covers `bounds`.
fn params_for(geometry: Geometry, bounds: SkRect, scissor: IRect) -> DrawParams {
    let clip = Clip::new(
        Rect::from_sk_rect(&bounds),
        Rect::from_sk_rect(&bounds),
        scissor,
        NonMSAAClip::default(),
        false,
    );
    // Painter's depth 1: `depthAsFloat()` is then `1 - 1/65535`.
    let order = DrawOrder::new(PaintersDepth::first().next());
    DrawParams::new(
        Transform::new(M44::new_identity()),
        geometry,
        &clip,
        order,
        None,
        BarrierType::None,
    )
}

/// Records the instances a step writes with `new_pipeline_state(...)` and one flush, then
/// returns the calls and the bytes of the first appended instance block.
fn record_instances(
    step: &dyn RenderStep,
    params: &DrawParams,
    ssbo_index: u32,
    instance_stride: usize,
    byte_count: usize,
) -> (Vec<Call>, Vec<u8>) {
    // The tessellation steps append dynamic instances, whose count the tolerances decide.
    let append_flags = if step
        .base()
        .flags()
        .contains(RenderStepFlags::APPEND_DYNAMIC_INSTANCES)
    {
        RenderStateFlags::APPEND_DYNAMIC_INSTANCES
    } else {
        RenderStateFlags::APPEND_INSTANCES
    };
    let (mut recorder, _shared) = make_recorder(MockCaps::default());
    let binding;
    let calls;
    {
        let priv_ = recorder.priv_();
        let manager = priv_.draw_buffer_manager();
        let mut list = RecordingList::default();
        {
            let mut writer = DrawWriter::new(&mut list, manager);
            writer.new_pipeline_state(
                step.base().primitive_type(),
                step.base().static_data_stride(),
                instance_stride,
                append_flags,
                BarrierType::None,
            );
            step.write_vertices(&mut writer, params, ssbo_index);
            writer.flush();
        }
        calls = list.0;
        // The append binding of the one draw: the first BindAppend in the list.
        binding = calls
            .iter()
            .find_map(|c| match c {
                Call::BindAppend(b) => Some(b.clone()),
                _ => None,
            })
            .expect("the instance draw binds its append buffer");
    }
    let buffer = binding.buffer.clone().unwrap();
    let _recording = recorder.snap().unwrap();
    let bytes = committed_bytes(&buffer).unwrap();
    let start = binding.offset as usize;
    (calls, bytes[start..start + byte_count].to_vec())
}

#[test]
fn draw_writer_zero_pads_vertex_appends_to_a_multiple_of_four() {
    let (mut recorder, _shared) = make_recorder(MockCaps::default());
    let binding;
    let calls;
    {
        let priv_ = recorder.priv_();
        let manager = priv_.draw_buffer_manager();
        let mut list = RecordingList::default();
        {
            let mut writer = DrawWriter::new(&mut list, manager);
            // Vertices of 8 bytes: a u32 index and a u32 value.
            writer.new_pipeline_state(
                PrimitiveType::TriangleStrip,
                0,
                8,
                RenderStateFlags::APPEND_VERTICES,
                BarrierType::None,
            );
            {
                let mut vertices = Vertices::new(&mut writer);
                let mut vw = vertices.append(3);
                for i in 0..3_u32 {
                    vw.put(&i).put(&(i * 10));
                }
            }
            writer.flush();
        }
        calls = list.0;
        binding = calls
            .iter()
            .find_map(|c| match c {
                Call::BindAppend(b) => Some(b.clone()),
                _ => None,
            })
            .unwrap();
    }
    // Three vertices are drawn, from the start of the buffer.
    assert!(calls.contains(&Call::Draw {
        primitive: PrimitiveType::TriangleStrip,
        base_vertex: 0,
        vertex_count: 3,
    }));
    let buffer = binding.buffer.clone().unwrap();
    let _recording = recorder.snap().unwrap();
    let bytes = committed_bytes(&buffer).unwrap();
    // Vertices 0..3 are written; the fourth, the ARM padding, is zero (ARM hardware, b/399631317).
    let expected = [
        u32_bytes(&[0, 0, 1, 10, 2, 20]),
        vec![0; 8], // the padding vertex
    ]
    .concat();
    assert_eq!(&bytes[..32], &expected[..]);
}

#[test]
fn cover_bounds_instance_bytes_for_a_rect() {
    let rect = SkRect {
        left: 10.0,
        top: 20.0,
        right: 30.0,
        bottom: 40.0,
    };
    let params = params_for(
        Geometry::Shape(Shape::from_rect(Rect::from_sk_rect(&rect))),
        rect,
        IRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        },
    );
    let step = CoverBoundsRenderStep::new(
        Layout::Std140,
        RenderStepID::CoverBounds_RegularCover,
        REGULAR_COVER_PASS,
    );
    // The instance: bounds (ltrb), depth, ssboIndex, then the 3x3 of the matrix as
    // [m00 m10 m30], [m01 m11 m31], [m03 m13 m33]. The identity gives the columns below.
    let depth = 1.0_f32 - 1.0_f32 / 65535.0_f32;
    let mut expected = f32_bytes(&[10.0, 20.0, 30.0, 40.0]);
    expected.extend(f32_bytes(&[depth]));
    expected.extend(u32_bytes(&[7]));
    expected.extend(f32_bytes(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]));
    let (calls, bytes) = record_instances(&step, &params, 7, 60, 60);
    assert_eq!(bytes, expected);
    assert!(calls.contains(&Call::DrawInstanced {
        primitive: PrimitiveType::TriangleStrip,
        base_vertex: 0,
        vertex_count: 4,
        base_instance: 0,
        instance_count: 1,
    }));
}

#[test]
fn cover_bounds_inverse_fill_uses_the_inverted_device_scissor() {
    let rect = SkRect {
        left: 10.0,
        top: 20.0,
        right: 30.0,
        bottom: 40.0,
    };
    let mut shape = Shape::from_rect(Rect::from_sk_rect(&rect));
    shape.set_inverted(true);
    let params = params_for(
        Geometry::Shape(shape),
        rect,
        IRect {
            left: 1,
            top: 2,
            right: 3,
            bottom: 4,
        },
    );
    let step = CoverBoundsRenderStep::new(
        Layout::Std140,
        RenderStepID::CoverBounds_InverseCover,
        REGULAR_COVER_PASS,
    );
    // An inverse fill uploads the scissor as [R, B, L, T]: shuffle<2,3,0,1> of [L, T, R, B].
    let (_, bytes) = record_instances(&step, &params, 0, 60, 16);
    assert_eq!(bytes, f32_bytes(&[3.0, 4.0, 1.0, 2.0]));
}

#[test]
fn per_edge_aa_quad_instance_bytes_for_a_clockwise_rect() {
    let rect = Rect::new(10.0, 20.0, 30.0, 40.0);
    let quad = EdgeAAQuad::from_rect(rect, AAFlags::ALL);
    let params = params_for(
        Geometry::EdgeAAQuad(quad),
        SkRect {
            left: 10.0,
            top: 20.0,
            right: 30.0,
            bottom: 40.0,
        },
        IRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        },
    );
    // edgeFlags (4 x u8, 255 = AA on), quadXs, quadYs, depth, ssboIndex, the 3x3 matrix. The rect
    // is clockwise by construction: xs = [L, R, R, L], ys = [T, T, B, B].
    let depth = 1.0_f32 - 1.0_f32 / 65535.0_f32;
    let mut expected = vec![255_u8, 255, 255, 255];
    expected.extend(f32_bytes(&[10.0, 30.0, 30.0, 10.0]));
    expected.extend(f32_bytes(&[20.0, 20.0, 40.0, 40.0]));
    expected.extend(f32_bytes(&[depth]));
    expected.extend(u32_bytes(&[3]));
    expected.extend(f32_bytes(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]));
    let mut manager = static_manager();
    let step = PerEdgeAAQuadRenderStep::new(Layout::Std140, &mut manager);
    finalize(&mut manager);
    let (calls, bytes) = record_instances(&step, &params, 3, 80, 80);
    assert_eq!(bytes, expected);
    // The template is indexed: 29 indices of the static index buffer, one instance.
    assert!(calls.contains(&Call::DrawIndexedInstanced {
        primitive: PrimitiveType::TriangleStrip,
        base_index: 0,
        index_count: 29,
        base_vertex: 0,
        base_instance: 0,
        instance_count: 1,
    }));
}

#[test]
fn per_edge_aa_quad_swaps_counter_clockwise_corners() {
    // TL(0,0), BL(0,10), BR(10,12), TR(10,0): counter-clockwise (the cross product is -100), and
    // not a rectangle, so `is_clockwise` is false and the corners are swapped: left and right
    // trade places, TL with TR, and BL with BR.
    let points = [
        Point::new(0.0, 0.0),
        Point::new(0.0, 10.0),
        Point::new(10.0, 12.0),
        Point::new(10.0, 0.0),
    ];
    let quad = EdgeAAQuad::from_points(&points, AAFlags::LEFT);
    let params = params_for(
        Geometry::EdgeAAQuad(quad),
        SkRect {
            left: 0.0,
            top: 0.0,
            right: 10.0,
            bottom: 12.0,
        },
        IRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        },
    );
    // Only the left edge is anti-aliased: signs [L, T, R, B] = [255, 0, 0, 0] become [0, 0, 255, 0].
    // xs = [0, 0, 10, 10] becomes [x1, x0, x3, x2] = [0, 0, 10, 10]; ys = [0, 10, 12, 0] becomes
    // [y1, y0, y3, y2] = [10, 0, 0, 12].
    let depth = 1.0_f32 - 1.0_f32 / 65535.0_f32;
    let mut expected = vec![0_u8, 0, 255, 0];
    expected.extend(f32_bytes(&[0.0, 0.0, 10.0, 10.0]));
    expected.extend(f32_bytes(&[10.0, 0.0, 0.0, 12.0]));
    expected.extend(f32_bytes(&[depth]));
    expected.extend(u32_bytes(&[0]));
    expected.extend(f32_bytes(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]));
    let mut manager = static_manager();
    let step = PerEdgeAAQuadRenderStep::new(Layout::Std140, &mut manager);
    finalize(&mut manager);
    let (_, bytes) = record_instances(&step, &params, 0, 80, 80);
    assert_eq!(bytes, expected);
}

#[test]
fn circular_arc_fill_instance_bytes() {
    let oval = SkRect {
        left: 0.0,
        top: 0.0,
        right: 20.0,
        bottom: 20.0,
    };
    let params = params_for(
        Geometry::Shape(Shape::from_arc(Arc::new(oval, 0.0, 90.0, false))),
        oval,
        IRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        },
    );
    let mut manager = static_manager();
    let step = CircularArcRenderStep::new(Layout::Std140, &mut manager);
    finalize(&mut manager);
    // A filled arc without its center: centerScales = [cx, cy, outer, inner] = [10, 10, 10, 0];
    // radiiAndFlags = [outer, inner / outer, flags] with outer = 10 + 1/2, inner = -1/2 - 1/2, and
    // flags = kIntersection_NoRoundCaps = 1; geoClipPlane keeps its default [0, 0, 1].
    let outer = 10.0_f32 + 0.5_f32;
    let inner = (-0.5_f32 - 0.5_f32) / outer;
    let mut expected = f32_bytes(&[10.0, 10.0, 10.0, 0.0]);
    expected.extend(f32_bytes(&[outer, inner, 1.0]));
    expected.extend(f32_bytes(&[0.0, 0.0, 1.0]));
    let (calls, bytes) = record_instances(&step, &params, 5, 128, 40);
    assert_eq!(bytes, expected);
    assert!(calls.contains(&Call::DrawInstanced {
        primitive: PrimitiveType::TriangleStrip,
        base_vertex: 0,
        vertex_count: 18,
        base_instance: 0,
        instance_count: 1,
    }));
}

#[test]
fn render_step_sksl_text_is_byte_identical() {
    // Each expectation concatenates the C++ literal pieces of the step's vertexSkSL or
    // fragmentCoverageSkSL, in the same order.
    let mut manager = static_manager();
    let cover = CoverBoundsRenderStep::new(
        Layout::Std140,
        RenderStepID::CoverBounds_RegularCover,
        REGULAR_COVER_PASS,
    );
    assert_eq!(
        cover.vertex_sksl(&RootNodesInfo::default()),
        concat!(
            "float4 devPosition = cover_bounds_vertex_fn(",
            "float2(sk_VertexID / 2, sk_VertexID % 2), ",
            "bounds, depth, float3x3(mat0, mat1, mat2), ",
            "stepLocalCoords);\n"
        )
    );

    let quad = PerEdgeAAQuadRenderStep::new(Layout::Std140, &mut manager);
    assert_eq!(
        quad.vertex_sksl(&RootNodesInfo::default()),
        concat!(
            "float4 devPosition = per_edge_aa_quad_vertex_fn(",
            "cornerID, normal, ",
            "edgeFlags, quadXs, quadYs, depth, ",
            "float3x3(mat0, mat1, mat2), ",
            "edgeDistances, ",
            "stepLocalCoords);\n"
        )
    );
    assert_eq!(
        quad.fragment_coverage_sksl(),
        "outputCoverage = per_edge_aa_quad_coverage_fn(sk_FragCoord, edgeDistances);"
    );

    let arc = CircularArcRenderStep::new(Layout::Std140, &mut manager);
    assert_eq!(
        arc.vertex_sksl(&RootNodesInfo::default()),
        concat!(
            "float4 devPosition = circular_arc_vertex_fn(",
            "position, ",
            "centerScales, radiiAndFlags, geoClipPlane, fragClipPlane0, fragClipPlane1, ",
            "inRoundCapPos, inRoundCapRadius, depth, float3x3(mat0, mat1, mat2), ",
            "circleEdge, clipPlane, isectPlane, unionPlane, ",
            "roundCapRadius, roundCapPos, ",
            "stepLocalCoords);\n"
        )
    );
    assert_eq!(
        arc.fragment_coverage_sksl(),
        concat!(
            "outputCoverage = circular_arc_coverage_fn(circleEdge, ",
            "clipPlane, ",
            "isectPlane, ",
            "unionPlane, ",
            "roundCapRadius, ",
            "roundCapPos);"
        )
    );
}

#[test]
fn renderer_provider_names_draw_types_and_depth_stencil_flags() {
    let mut manager = static_manager();
    let provider = RendererProvider::new(Layout::Std140, true, &mut manager);

    let quad = provider.per_edge_aa_quad();
    assert_eq!(quad.name(), "SingleStep[PerEdgeAAQuadRenderStep]");
    assert_eq!(quad.draw_types(), DrawTypeFlags::PER_EDGE_AA_QUAD);
    assert_eq!(quad.coverage(), Coverage::SingleChannel);
    assert!(quad.use_non_aa_inner_fill());

    let fill = provider.non_aa_bounds_fill();
    assert_eq!(fill.name(), "SingleStep[CoverBoundsRenderStep[NonAAFill]]");
    assert_eq!(fill.coverage(), Coverage::None);
    // A depth-only LESS pass: no stencil, depth test and write.
    assert_eq!(fill.depth_stencil_flags(), DepthStencilFlags::Depth);

    assert_eq!(
        provider.circular_arc().name(),
        "SingleStep[CircularArcRenderStep]"
    );
    assert_eq!(
        provider.cover_fill().name(),
        "CoverBoundsRenderStep[RegularCover]"
    );
    assert_eq!(
        provider.cover_inverse().name(),
        "CoverBoundsRenderStep[InverseCover]"
    );
    assert_eq!(
        provider.cover_fill().base().depth_stencil_settings(),
        &REGULAR_COVER_PASS
    );
    assert!(
        provider
            .cover_fill()
            .base()
            .depth_stencil_settings()
            .depth_test_enabled
    );

    let rrect = provider.analytic_rrect();
    assert_eq!(rrect.name(), "SingleStep[AnalyticRRectRenderStep]");
    assert_eq!(rrect.draw_types(), DrawTypeFlags::ANALYTIC_RRECT);
    assert_eq!(rrect.coverage(), Coverage::SingleChannel);
}

/// The `DrawParams` of a stroked draw with an identity transform.
fn stroked_params(geometry: Geometry, bounds: SkRect, stroke: StrokeStyle) -> DrawParams {
    let clip = Clip::new(
        Rect::from_sk_rect(&bounds),
        Rect::from_sk_rect(&bounds),
        IRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        },
        NonMSAAClip::default(),
        false,
    );
    DrawParams::new(
        Transform::new(M44::new_identity()),
        geometry,
        &clip,
        DrawOrder::new(PaintersDepth::first().next()),
        Some(&stroke),
        BarrierType::None,
    )
}

/// The instance of an `AnalyticRRectRenderStep` draw: 108 bytes, as the attribute list gives.
const ANALYTIC_RRECT_STRIDE: usize = 108;

#[test]
fn analytic_rrect_fill_rect_instance_bytes() {
    let rect = SkRect {
        left: 10.0,
        top: 20.0,
        right: 30.0,
        bottom: 40.0,
    };
    let params = params_for(
        Geometry::Shape(Shape::from_rect(Rect::from_sk_rect(&rect))),
        rect,
        IRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        },
    );
    let mut manager = static_manager();
    let step = AnalyticRRectRenderStep::new(Layout::Std140, &mut manager);
    finalize(&mut manager);
    // The identity transform has an AA radius of 1 (1 / min scale). A 20 x 20 rect does not reach
    // its opposite insets (2 * 1 = 2), so the center weight stays solid (1).
    // xRadiiOrFlags = [-1, -1, -1, -1] (all edges AA); radiiOrQuadXs = [L, R, R, L];
    // ltrbOrQuadYs = [T, T, B, B]; center = [cx, cy, centerWeight, aaRadius].
    let depth = 1.0_f32 - 1.0_f32 / 65535.0_f32;
    let mut expected = f32_bytes(&[-1.0, -1.0, -1.0, -1.0]);
    expected.extend(f32_bytes(&[10.0, 30.0, 30.0, 10.0]));
    expected.extend(f32_bytes(&[20.0, 20.0, 40.0, 40.0]));
    expected.extend(f32_bytes(&[20.0, 30.0, 1.0, 1.0]));
    expected.extend(f32_bytes(&[depth]));
    expected.extend(u32_bytes(&[5]));
    expected.extend(f32_bytes(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]));
    let (calls, bytes) = record_instances(
        &step,
        &params,
        5,
        ANALYTIC_RRECT_STRIDE,
        ANALYTIC_RRECT_STRIDE,
    );
    assert_eq!(bytes, expected);
    assert!(calls.contains(&Call::DrawIndexedInstanced {
        primitive: PrimitiveType::TriangleStrip,
        base_index: 0,
        index_count: 69,
        base_vertex: 0,
        base_instance: 0,
        instance_count: 1,
    }));
}

#[test]
fn analytic_rrect_clockwise_quad_matches_the_rect_encoding() {
    // A clockwise quad with every edge AA is the same shape as the rect above, and encodes the
    // same instance bytes.
    let quad = EdgeAAQuad::from_rect(Rect::new(10.0, 20.0, 30.0, 40.0), AAFlags::ALL);
    let params = params_for(
        Geometry::EdgeAAQuad(quad),
        SkRect {
            left: 10.0,
            top: 20.0,
            right: 30.0,
            bottom: 40.0,
        },
        IRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        },
    );
    let mut manager = static_manager();
    let step = AnalyticRRectRenderStep::new(Layout::Std140, &mut manager);
    finalize(&mut manager);
    let depth = 1.0_f32 - 1.0_f32 / 65535.0_f32;
    let mut expected = f32_bytes(&[-1.0, -1.0, -1.0, -1.0]);
    expected.extend(f32_bytes(&[10.0, 30.0, 30.0, 10.0]));
    expected.extend(f32_bytes(&[20.0, 20.0, 40.0, 40.0]));
    // quad_center: dot(xs, 0.25) = 20, dot(ys, 0.25) = 30.
    expected.extend(f32_bytes(&[20.0, 30.0, 1.0, 1.0]));
    expected.extend(f32_bytes(&[depth]));
    expected.extend(u32_bytes(&[0]));
    expected.extend(f32_bytes(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]));
    let (_, bytes) = record_instances(
        &step,
        &params,
        0,
        ANALYTIC_RRECT_STRIDE,
        ANALYTIC_RRECT_STRIDE,
    );
    assert_eq!(bytes, expected);
}

#[test]
fn analytic_rrect_stroked_rect_instance_bytes() {
    let rect = SkRect {
        left: 10.0,
        top: 20.0,
        right: 30.0,
        bottom: 40.0,
    };
    // A 4-wide miter stroke: half width 2, miter limit 4.
    let stroke = StrokeStyle::new(4.0, 4.0, Join::Miter, Cap::Butt);
    let params = stroked_params(
        Geometry::Shape(Shape::from_rect(Rect::from_sk_rect(&rect))),
        rect,
        stroke,
    );
    let mut manager = static_manager();
    let step = AnalyticRRectRenderStep::new(Layout::Std140, &mut manager);
    finalize(&mut manager);
    // The inner gap is 20 - 4 = 16 > 0, so the center weight is the stroke interior (0), and the
    // inset is the stroke radius (2). The join is the miter rule: the limit (4) is not below
    // sqrt 2 and the shape is not empty, so the join style is 1. The radii are zero.
    // xRadiiOrFlags = [-2, lineFlag = 0, strokeRadius = 2, join = 1]. The inset does not reach the
    // opposite sides (2 * (2 + 1) = 6 < 20), so the AA radius stays 1.
    let depth = 1.0_f32 - 1.0_f32 / 65535.0_f32;
    let mut expected = f32_bytes(&[-2.0, 0.0, 2.0, 1.0]);
    expected.extend(f32_bytes(&[0.0, 0.0, 0.0, 0.0]));
    expected.extend(f32_bytes(&[10.0, 20.0, 30.0, 40.0]));
    expected.extend(f32_bytes(&[20.0, 30.0, 0.0, 1.0]));
    expected.extend(f32_bytes(&[depth]));
    expected.extend(u32_bytes(&[2]));
    expected.extend(f32_bytes(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]));
    let (_, bytes) = record_instances(
        &step,
        &params,
        2,
        ANALYTIC_RRECT_STRIDE,
        ANALYTIC_RRECT_STRIDE,
    );
    assert_eq!(bytes, expected);
}

#[test]
fn analytic_rrect_rounded_rect_instance_bytes() {
    let rect = SkRect {
        left: 0.0,
        top: 0.0,
        right: 20.0,
        bottom: 20.0,
    };
    let round = RRect::new_rect_xy(rect, 5.0, 5.0);
    let params = params_for(
        Geometry::Shape(Shape::from_rrect(round)),
        rect,
        IRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        },
    );
    let mut manager = static_manager();
    let step = AnalyticRRectRenderStep::new(Layout::Std140, &mut manager);
    finalize(&mut manager);
    // A filled rounded rect: the X radii, then the Y radii, then the bounds. The radii are 5 at
    // every corner, and the insets (2) do not reach the opposite curves (20 - 5 = 15).
    let depth = 1.0_f32 - 1.0_f32 / 65535.0_f32;
    let mut expected = f32_bytes(&[5.0, 5.0, 5.0, 5.0]);
    expected.extend(f32_bytes(&[5.0, 5.0, 5.0, 5.0]));
    expected.extend(f32_bytes(&[0.0, 0.0, 20.0, 20.0]));
    expected.extend(f32_bytes(&[10.0, 10.0, 1.0, 1.0]));
    expected.extend(f32_bytes(&[depth]));
    expected.extend(u32_bytes(&[1]));
    expected.extend(f32_bytes(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]));
    let (_, bytes) = record_instances(
        &step,
        &params,
        1,
        ANALYTIC_RRECT_STRIDE,
        ANALYTIC_RRECT_STRIDE,
    );
    assert_eq!(bytes, expected);
}

/// Records the vertices a step appends with `APPEND_VERTICES`, and returns the calls and the bytes
/// of the appended block, including the padding that the draw adds to a multiple of four.
fn record_vertices(
    step: &dyn RenderStep,
    params: &DrawParams,
    ssbo_index: u32,
    vertex_stride: usize,
    byte_count: usize,
) -> (Vec<Call>, Vec<u8>) {
    let (mut recorder, _shared) = make_recorder(MockCaps::default());
    let binding;
    let calls;
    {
        let priv_ = recorder.priv_();
        let manager = priv_.draw_buffer_manager();
        let mut list = RecordingList::default();
        {
            let mut writer = DrawWriter::new(&mut list, manager);
            writer.new_pipeline_state(
                step.base().primitive_type(),
                step.base().static_data_stride(),
                vertex_stride,
                RenderStateFlags::APPEND_VERTICES,
                BarrierType::None,
            );
            step.write_vertices(&mut writer, params, ssbo_index);
            writer.flush();
        }
        calls = list.0;
        binding = calls
            .iter()
            .find_map(|c| match c {
                Call::BindAppend(b) => Some(b.clone()),
                _ => None,
            })
            .expect("the vertex draw binds its append buffer");
    }
    let buffer = binding.buffer.clone().unwrap();
    let _recording = recorder.snap().unwrap();
    let bytes = committed_bytes(&buffer).unwrap();
    let start = binding.offset as usize;
    (calls, bytes[start..start + byte_count].to_vec())
}

/// The `DrawParams` of a `SkVertices` draw with an identity transform.
fn vertices_params(vertices: SkVertices) -> DrawParams {
    let bounds = SkRect {
        left: 0.0,
        top: 0.0,
        right: 10.0,
        bottom: 10.0,
    };
    params_for(
        Geometry::Vertices(vertices),
        bounds,
        IRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        },
    )
}

#[test]
fn vertices_position_only_triangle_bytes() {
    let positions = [
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(0.0, 10.0),
    ];
    let vertices = SkVertices::new_copy(VertexMode::Triangles, &positions, None, None, None)
        .expect("a valid triangle list");
    let params = vertices_params(vertices);
    let step = VerticesRenderStep::new(Layout::Std140, false, false);
    // Position and ssboIndex: 12 bytes per vertex. Three vertices are drawn, and the fourth
    // (the padding to a multiple of four) is zero.
    let mut expected = f32_bytes(&[0.0, 0.0]);
    expected.extend(u32_bytes(&[7]));
    expected.extend(f32_bytes(&[10.0, 0.0]));
    expected.extend(u32_bytes(&[7]));
    expected.extend(f32_bytes(&[0.0, 10.0]));
    expected.extend(u32_bytes(&[7]));
    expected.extend([0_u8; 12]);
    let (calls, bytes) = record_vertices(&step, &params, 7, 12, 48);
    assert_eq!(bytes, expected);
    assert!(calls.contains(&Call::Draw {
        primitive: PrimitiveType::Triangles,
        base_vertex: 0,
        vertex_count: 3,
    }));
}

#[test]
fn vertices_color_and_tex_coords_strip_bytes() {
    let positions = [
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(0.0, 10.0),
    ];
    let colors = [
        Color::new(0xFFFF_0000),
        Color::new(0x8000_FF00),
        Color::new(0x4000_0000),
    ];
    let tex_coords = [
        Point::new(0.0, 0.0),
        Point::new(1.0, 0.0),
        Point::new(0.0, 1.0),
    ];
    let vertices = SkVertices::new_copy(
        VertexMode::Triangles,
        &positions,
        Some(&tex_coords[..]),
        Some(&colors[..]),
        None,
    )
    .expect("a valid triangle list");
    let params = vertices_params(vertices);
    let step = VerticesRenderStep::new(Layout::Std140, true, true);
    // position (8), vertColor as the SkColor's four bytes (4), texCoords (8), ssboIndex (4).
    let mut expected = Vec::new();
    for (i, (p, t)) in positions.iter().zip(tex_coords.iter()).enumerate() {
        expected.extend(f32_bytes(&[p.x, p.y]));
        expected.extend(u32_bytes(&[u32::from(colors[i])]));
        expected.extend(f32_bytes(&[t.x, t.y]));
        expected.extend(u32_bytes(&[9]));
    }
    expected.extend([0_u8; 24]); // The padding vertex.
    let (calls, bytes) = record_vertices(&step, &params, 9, 24, 96);
    assert_eq!(bytes, expected);
    assert!(calls.contains(&Call::Draw {
        primitive: PrimitiveType::Triangles,
        base_vertex: 0,
        vertex_count: 3,
    }));
}

#[test]
fn vertices_indexed_triangles_are_expanded_in_index_order() {
    let positions = [
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(0.0, 10.0),
        Point::new(10.0, 10.0),
    ];
    let indices: [u16; 6] = [0, 1, 2, 2, 1, 3];
    let vertices = SkVertices::new_copy(
        VertexMode::Triangles,
        &positions,
        None,
        None,
        Some(&indices[..]),
    )
    .expect("a valid indexed triangle list");
    let params = vertices_params(vertices);
    let step = VerticesRenderStep::new(Layout::Std140, false, false);
    // The triangles visit the positions in index order: p0 p1 p2 p2 p1 p3. Six vertices are
    // drawn, and two padding vertices follow.
    let order = [0_usize, 1, 2, 2, 1, 3];
    let mut expected = Vec::new();
    for &v in &order {
        expected.extend(f32_bytes(&[positions[v].x, positions[v].y]));
        expected.extend(u32_bytes(&[3]));
    }
    expected.extend([0_u8; 24]);
    let (calls, bytes) = record_vertices(&step, &params, 3, 12, 96);
    assert_eq!(bytes, expected);
    assert!(calls.contains(&Call::Draw {
        primitive: PrimitiveType::Triangles,
        base_vertex: 0,
        vertex_count: 6,
    }));
}

#[test]
fn renderer_provider_vertices_variants() {
    let mut manager = static_manager();
    let provider = RendererProvider::new(Layout::Std140, true, &mut manager);
    assert_eq!(
        provider.vertices(false, false).name(),
        "SingleStep[VerticesRenderStep[Pos]]"
    );
    assert_eq!(
        provider.vertices(true, true).name(),
        "SingleStep[VerticesRenderStep[PosColorTexCoords]]"
    );
    // Color without texture coordinates is also the drop-shadow variant.
    assert_eq!(
        provider.vertices(true, false).draw_types(),
        DrawTypeFlags::DRAW_VERTICES | DrawTypeFlags::DROP_SHADOWS
    );
    assert_eq!(provider.vertices(true, true).coverage(), Coverage::None);
    assert!(
        provider
            .vertices(true, true)
            .step(0)
            .emits_primitive_color()
    );
}

#[test]
fn analytic_rrect_sksl_text_is_byte_identical() {
    let mut manager = static_manager();
    let step = AnalyticRRectRenderStep::new(Layout::Std140, &mut manager);
    assert_eq!(
        step.vertex_sksl(&RootNodesInfo::default()),
        concat!(
            "float4 devPosition = analytic_rrect_vertex_fn(",
            "cornerID, position, normal, normalScale, centerWeight, ",
            "xRadiiOrFlags, radiiOrQuadXs, ltrbOrQuadYs, center, depth, ",
            "float3x3(mat0, mat1, mat2), ",
            "jacobian, edgeDistances, xRadii, yRadii, strokeParams, perPixelControl, ",
            "stepLocalCoords);\n"
        )
    );
    assert_eq!(
        step.fragment_coverage_sksl(),
        concat!(
            "outputCoverage = analytic_rrect_coverage_fn(sk_FragCoord, ",
            "jacobian, ",
            "edgeDistances, ",
            "xRadii, ",
            "yRadii, ",
            "strokeParams, ",
            "perPixelControl);"
        )
    );
}

// ---- Tessellation render steps (G7b). ----
//
// The SkSL pieces below are the string literals of each step's `vertexSkSL` in the C++, in
// order (extracted mechanically from `third_party/skia/src/gpu/graphite/render/*.cpp`); the `%s`
// of each `printf` is the curve-type expression, which the test fills in.

/// The `depthAsFloat()` of `params_for` (painter's depth 1).
fn tess_depth() -> f32 {
    1.0_f32 - 1.0_f32 / 65535.0_f32
}

/// The curve-type expression of `vertexSkSL` for a step with and without infinity support.
const CURVE_TYPE_INF: &str = "curve_type_using_inf_support(p23)";
const CURVE_TYPE_EXPLICIT: &str = "curveType";

#[test]
fn tessellate_sksl_text_is_byte_identical() {
    // TessellateCurvesRenderStep.cpp#L118-L130: the `%s` is the curve type.
    for (infinity, curve_type) in [(true, CURVE_TYPE_INF), (false, CURVE_TYPE_EXPLICIT)] {
        let mut manager = static_manager();
        let curves = TessellateCurvesRenderStep::new(Layout::Std140, true, infinity, &mut manager);
        let expected = [
            "float2x2 vectorXform = float2x2(localToDevice[0].xy, localToDevice[1].xy);\n",
            "float2 localCoord = tessellate_filled_curve(",
            "vectorXform, resolveLevel_and_idx.x, resolveLevel_and_idx.y, p01, p23, ",
            curve_type,
            ");\n",
            "float4 devPosition = localToDevice * float4(localCoord, 0.0, 1.0);\n",
            "devPosition.z = depth;\n",
            "stepLocalCoords = localCoord;\n",
        ]
        .concat();
        assert_eq!(curves.vertex_sksl(&RootNodesInfo::default()), expected);
    }

    // TessellateWedgesRenderStep.cpp#L122-L140: the `%s` is the curve type.
    for (infinity, curve_type) in [(true, CURVE_TYPE_INF), (false, CURVE_TYPE_EXPLICIT)] {
        let mut manager = static_manager();
        let wedges = TessellateWedgesRenderStep::new(
            Layout::Std140,
            RenderStepID::TessellateWedges_Convex,
            infinity,
            DIRECT_DEPTH_LESS_PASS,
            &mut manager,
        );
        let expected = [
            "float2 localCoord;\n",
            "if (resolveLevel_and_idx.x < 0) {\n",
            "localCoord = fanPointAttrib;\n",
            "} else {\n",
            "float2x2 vectorXform = float2x2(localToDevice[0].xy, localToDevice[1].xy);\n",
            "localCoord = tessellate_filled_curve(",
            "vectorXform, resolveLevel_and_idx.x, resolveLevel_and_idx.y, p01, p23, ",
            curve_type,
            ");\n",
            "}\n",
            "float4 devPosition = localToDevice * float4(localCoord, 0.0, 1.0);\n",
            "devPosition.z = depth;\n",
            "stepLocalCoords = localCoord;\n",
        ]
        .concat();
        assert_eq!(wedges.vertex_sksl(&RootNodesInfo::default()), expected);
    }

    // TessellateStrokesRenderStep.cpp#L113-L124: the `%s` is the curve type, for fill and inverse.
    for inverse in [false, true] {
        for (infinity, curve_type) in [(true, CURVE_TYPE_INF), (false, CURVE_TYPE_EXPLICIT)] {
            let strokes = TessellateStrokesRenderStep::new(Layout::Std140, infinity, inverse);
            let expected = [
                "float edgeID = float(sk_VertexID >> 1);\n",
                "if ((sk_VertexID & 1) != 0) {",
                "edgeID = -edgeID;",
                "}\n",
                "float2x2 affine = float2x2(affineMatrix.xy, affineMatrix.zw);\n",
                "float4 devAndLocalCoords = tessellate_stroked_curve(",
                "edgeID, 16383, affine, translate, maxScale, p01, p23, prevPoint,",
                "stroke, ",
                curve_type,
                ");\n",
                "float4 devPosition = float4(devAndLocalCoords.xy, depth, 1.0);\n",
                "stepLocalCoords = devAndLocalCoords.zw;\n",
            ]
            .concat();
            assert_eq!(strokes.vertex_sksl(&RootNodesInfo::default()), expected);
        }
    }

    // MiddleOutFanRenderStep.cpp#L47-L52.
    let fan = MiddleOutFanRenderStep::new(Layout::Std140, true);
    assert_eq!(
        fan.vertex_sksl(&RootNodesInfo::default()),
        concat!(
            "float4 devPosition = localToDevice * float4(position, 0.0, 1.0);\n",
            "devPosition.z = depth;\n",
            "stepLocalCoords = position;\n",
        )
    );
}

/// The closed square (0,0), (100,0), (100,100), (0,100): convex, with midpoint (50, 50).
fn square_path() -> Path {
    let mut builder = PathBuilder::new();
    builder
        .move_to(Point::new(0.0, 0.0))
        .line_to(Point::new(100.0, 0.0))
        .line_to(Point::new(100.0, 100.0))
        .line_to(Point::new(0.0, 100.0))
        .close();
    builder.detach()
}

/// The bounds and scissor of the tessellation tests' draws.
fn tess_bounds() -> SkRect {
    SkRect {
        left: 0.0,
        top: 0.0,
        right: 100.0,
        bottom: 100.0,
    }
}

#[test]
fn middle_out_fan_convex_square_vertex_bytes() {
    // The middle-out triangulation of the square, traced through MiddleOutPolygonTriangulator
    // (five verbs, so the stack starts with (0, 0)): [(0,0), (100,0), (100,100)] is popped by the
    // second line, then [(100,100), (0,100), (0,0)] by the close.
    let params = params_for(
        Geometry::Shape(Shape::from_path(square_path())),
        tess_bounds(),
        IRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        },
    );
    let step = MiddleOutFanRenderStep::new(Layout::Std140, false);
    let depth = tess_depth();
    let triangles = [
        [(0.0_f32, 0.0_f32), (100.0, 0.0), (100.0, 100.0)],
        [(100.0, 100.0), (0.0, 100.0), (0.0, 0.0)],
    ];
    // Position, depth and ssboIndex: 16 bytes per vertex.
    let mut expected = Vec::new();
    for triangle in triangles {
        for (x, y) in triangle {
            expected.extend(f32_bytes(&[x, y, depth]));
            expected.extend(u32_bytes(&[3]));
        }
    }
    let (calls, bytes) = record_vertices(&step, &params, 3, 16, 96);
    assert_eq!(bytes, expected);
    assert!(calls.contains(&Call::Draw {
        primitive: PrimitiveType::Triangles,
        base_vertex: 0,
        vertex_count: 6,
    }));
}

#[test]
fn tessellate_curves_cubic_patch_bytes() {
    // No chop and no discard: the control points, the depth and the ssboIndex. Infinity support
    // needs no curve-type attribute, so the stride is 32 + 4 + 4.
    let cubic = [
        Point::new(0.0, 0.0),
        Point::new(0.0, 100.0),
        Point::new(100.0, 100.0),
        Point::new(100.0, 0.0),
    ];
    let mut builder = PathBuilder::new();
    builder
        .move_to(cubic[0])
        .cubic_to(cubic[1], cubic[2], cubic[3]);
    let params = params_for(
        Geometry::Shape(Shape::from_path(builder.detach())),
        tess_bounds(),
        IRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        },
    );
    let mut manager = static_manager();
    let step = TessellateCurvesRenderStep::new(Layout::Std140, false, true, &mut manager);
    finalize(&mut manager);
    let mut expected = f32_bytes(&[0.0, 0.0, 0.0, 100.0, 100.0, 100.0, 100.0, 0.0]);
    expected.extend(f32_bytes(&[tess_depth()]));
    expected.extend(u32_bytes(&[5]));
    let (_calls, bytes) = record_instances(&step, &params, 5, 40, 40);
    assert_eq!(bytes, expected);
}

#[test]
fn tessellate_curves_quad_patch_bytes() {
    // writeQuadPatch: the quad becomes the cubic (p0, mix(p0 | p2, p1, 2/3), p2), where
    // mix(a, b, T) = (b - a) * T + a in each lane (PatchWriter.h#L706 and #L732).
    let (p0, p1, p2) = (
        (0.0_f32, 0.0_f32),
        (50.0_f32, 100.0_f32),
        (100.0_f32, 0.0_f32),
    );
    let t = 2.0_f32 / 3.0_f32;
    let cp1 = [(p1.0 - p0.0) * t + p0.0, (p1.1 - p0.1) * t + p0.1];
    let cp2 = [(p1.0 - p2.0) * t + p2.0, (p1.1 - p2.1) * t + p2.1];
    let mut builder = PathBuilder::new();
    builder
        .move_to(Point::new(p0.0, p0.1))
        .quad_to(Point::new(p1.0, p1.1), Point::new(p2.0, p2.1));
    let params = params_for(
        Geometry::Shape(Shape::from_path(builder.detach())),
        tess_bounds(),
        IRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        },
    );
    let mut manager = static_manager();
    let step = TessellateCurvesRenderStep::new(Layout::Std140, false, true, &mut manager);
    finalize(&mut manager);
    let mut expected = f32_bytes(&[p0.0, p0.1, cp1[0], cp1[1], cp2[0], cp2[1], p2.0, p2.1]);
    expected.extend(f32_bytes(&[tess_depth()]));
    expected.extend(u32_bytes(&[5]));
    let (_calls, bytes) = record_instances(&step, &params, 5, 40, 40);
    assert_eq!(bytes, expected);
}

#[test]
fn tessellate_curves_conic_patch_bytes() {
    // writeConicPatch: p0, p1, p2 and {w, +inf} as the last control point. With infinity support
    // the +inf in p23.w is the conic signal, so no curve-type attribute is written.
    let mut builder = PathBuilder::new();
    builder.move_to(Point::new(0.0, 0.0)).conic_to(
        Point::new(50.0, 100.0),
        Point::new(100.0, 0.0),
        0.5,
    );
    let params = params_for(
        Geometry::Shape(Shape::from_path(builder.detach())),
        tess_bounds(),
        IRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        },
    );
    let mut manager = static_manager();
    let step = TessellateCurvesRenderStep::new(Layout::Std140, false, true, &mut manager);
    finalize(&mut manager);
    let mut expected = f32_bytes(&[0.0, 0.0, 50.0, 100.0, 100.0, 0.0, 0.5, f32::INFINITY]);
    expected.extend(f32_bytes(&[tess_depth()]));
    expected.extend(u32_bytes(&[5]));
    let (_calls, bytes) = record_instances(&step, &params, 5, 40, 40);
    assert_eq!(bytes, expected);
}

/// The control points of a line written by `PatchWriter::writeLine` for wedges, which is the
/// cubic (p0, (p1 - p0) / 3 + p0, (p0 - p1) / 3 + p1, p1) (PatchWriter.h#L577-L600).
fn wedge_line_cubic(a: (f32, f32), b: (f32, f32)) -> [f32; 8] {
    let third = 1.0_f32 / 3.0_f32;
    [
        a.0,
        a.1,
        (b.0 - a.0) * third + a.0,
        (b.1 - a.1) * third + a.1,
        (a.0 - b.0) * third + b.0,
        (a.1 - b.1) * third + b.1,
        b.0,
        b.1,
    ]
}

#[test]
fn tessellate_wedges_convex_square_patch_bytes() {
    // Four line patches, each with the fan point at the contour midpoint (50, 50): the three
    // lines of the path and the closing line from (0, 100) back to the start (0, 0).
    let params = params_for(
        Geometry::Shape(Shape::from_path(square_path())),
        tess_bounds(),
        IRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        },
    );
    let mut manager = static_manager();
    let step = TessellateWedgesRenderStep::new(
        Layout::Std140,
        RenderStepID::TessellateWedges_Convex,
        true,
        DIRECT_DEPTH_LESS_PASS,
        &mut manager,
    );
    finalize(&mut manager);
    let corners = [
        (0.0_f32, 0.0_f32),
        (100.0, 0.0),
        (100.0, 100.0),
        (0.0, 100.0),
    ];
    let edges = [
        (corners[0], corners[1]),
        (corners[1], corners[2]),
        (corners[2], corners[3]),
        (corners[3], corners[0]),
    ];
    let mut expected = Vec::new();
    for (a, b) in edges {
        expected.extend(f32_bytes(&wedge_line_cubic(a, b)));
        expected.extend(f32_bytes(&[50.0, 50.0, tess_depth()]));
        expected.extend(u32_bytes(&[3]));
    }
    // Patch: 32 bytes of control points, the fan point (8), depth (4) and ssboIndex (4).
    let (_calls, bytes) = record_instances(&step, &params, 3, 48, 4 * 48);
    assert_eq!(bytes, expected);
}

/// The open polyline (0,0), (100,0), (100,100) stroked with half width 5 and a miter join of
/// limit 4. The stroke attributes are the join control point, then (radius, joinLimit).
fn open_polyline_stroke_params(cap: Cap) -> DrawParams {
    let mut builder = PathBuilder::new();
    builder
        .move_to(Point::new(0.0, 0.0))
        .line_to(Point::new(100.0, 0.0))
        .line_to(Point::new(100.0, 100.0));
    let stroke = StrokeStyle::new(10.0, 4.0, Join::Miter, cap);
    stroked_params(
        Geometry::Shape(Shape::from_path(builder.detach())),
        tess_bounds(),
        stroke,
    )
}

#[test]
fn tessellate_strokes_open_polyline_butt_cap_patch_bytes() {
    // Two line patches. The first is deferred until the join is known, so it is written last,
    // with its join control point set to the contour's first point (the butt cap needs no more).
    // The second is written directly, with the join of the first line's incoming tangent (0, 0).
    // Each patch: four control points, the join (8), the stroke params (radius 5, joinLimit 4),
    // depth and ssboIndex: 56 bytes.
    let params = open_polyline_stroke_params(Cap::Butt);
    // The strokes use no static buffers, so there is nothing to finalize.
    let step = TessellateStrokesRenderStep::new(Layout::Std140, true, false);
    let stroke = [5.0_f32, 4.0_f32];
    let mut expected = f32_bytes(&[100.0, 0.0, 100.0, 0.0, 100.0, 100.0, 100.0, 100.0]);
    expected.extend(f32_bytes(&[0.0, 0.0]));
    expected.extend(f32_bytes(&stroke));
    expected.extend(f32_bytes(&[tess_depth()]));
    expected.extend(u32_bytes(&[3]));
    expected.extend(f32_bytes(&[0.0, 0.0, 0.0, 0.0, 100.0, 0.0, 100.0, 0.0]));
    expected.extend(f32_bytes(&[0.0, 0.0]));
    expected.extend(f32_bytes(&stroke));
    expected.extend(f32_bytes(&[tess_depth()]));
    expected.extend(u32_bytes(&[3]));
    let (_calls, bytes) = record_instances(&step, &params, 3, 56, 112);
    assert_eq!(bytes, expected);
}

#[test]
fn tessellate_strokes_open_polyline_round_cap_patch_bytes() {
    // With a round cap, the end of the contour adds a circle at the last control point and one at
    // the first (before the deferred first patch, which keeps the first point as its join).
    // A circle is a cubic with four copies of its point, and that point as its join.
    let params = open_polyline_stroke_params(Cap::Round);
    let step = TessellateStrokesRenderStep::new(Layout::Std140, true, false);
    let stroke = [5.0_f32, 4.0_f32];
    let circle = |p: [f32; 2]| {
        let mut bytes = f32_bytes(&[p[0], p[1], p[0], p[1], p[0], p[1], p[0], p[1]]);
        bytes.extend(f32_bytes(&p));
        bytes.extend(f32_bytes(&stroke));
        bytes.extend(f32_bytes(&[tess_depth()]));
        bytes.extend(u32_bytes(&[3]));
        bytes
    };
    let mut expected = f32_bytes(&[100.0, 0.0, 100.0, 0.0, 100.0, 100.0, 100.0, 100.0]);
    expected.extend(f32_bytes(&[0.0, 0.0]));
    expected.extend(f32_bytes(&stroke));
    expected.extend(f32_bytes(&[tess_depth()]));
    expected.extend(u32_bytes(&[3]));
    expected.extend(circle([100.0, 100.0]));
    expected.extend(circle([0.0, 0.0]));
    expected.extend(f32_bytes(&[0.0, 0.0, 0.0, 0.0, 100.0, 0.0, 100.0, 0.0]));
    expected.extend(f32_bytes(&[0.0, 0.0]));
    expected.extend(f32_bytes(&stroke));
    expected.extend(f32_bytes(&[tess_depth()]));
    expected.extend(u32_bytes(&[3]));
    let (_calls, bytes) = record_instances(&step, &params, 3, 56, 224);
    assert_eq!(bytes, expected);
}

#[test]
fn renderer_provider_tessellated_renderers_names() {
    let mut manager = static_manager();
    let provider = RendererProvider::new(Layout::Std140, true, &mut manager);
    assert_eq!(
        provider.convex_tessellated_wedges().name(),
        "SingleStep[TessellateWedgesRenderStep[Convex]]"
    );
    // Each stencil renderer is named by its fill type, in `PathFillType` order.
    assert_eq!(
        provider
            .stencil_tessellated_curves_and_tris(PathFillType::Winding)
            .name(),
        "StencilTessellatedCurvesAndTris[winding]"
    );
    assert_eq!(
        provider
            .stencil_tessellated_curves_and_tris(PathFillType::EvenOdd)
            .name(),
        "StencilTessellatedCurvesAndTris[evenodd]"
    );
    assert_eq!(
        provider
            .stencil_tessellated_wedges(PathFillType::InverseWinding)
            .name(),
        "StencilTessellatedWedges[inverse-winding]"
    );
    assert_eq!(
        provider
            .stencil_tessellated_wedges(PathFillType::InverseEvenOdd)
            .name(),
        "StencilTessellatedWedges[inverse-evenodd]"
    );
    assert_eq!(
        provider.tessellated_strokes(false).name(),
        "SingleStep[TessellateStrokesRenderStep[Fill]]"
    );
    assert_eq!(
        provider.tessellated_strokes(true).name(),
        "TessellatedStrokesInverseFill"
    );
}
