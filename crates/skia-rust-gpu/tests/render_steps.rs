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
use skia_rust_core::m44::M44;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect as SkRect};
use skia_rust_gpu::gpu::gpu_types::Protected;
use skia_rust_gpu::graphite::buffer::{BindBufferInfo, Buffer};
use skia_rust_gpu::graphite::buffer_manager::{
    StaticBufferHost, StaticBufferManager, StaticFinishResult,
};
use skia_rust_gpu::graphite::draw_order::{DrawOrder, PaintersDepth};
use skia_rust_gpu::graphite::draw_params::{Clip, DrawParams};
use skia_rust_gpu::graphite::draw_types::{
    BarrierType, DrawTypeFlags, PrimitiveType, RenderStateFlags,
};
use skia_rust_gpu::graphite::draw_writer::{DrawPassCommandList, DrawWriter, Vertices};
use skia_rust_gpu::graphite::geom::edge_aa_quad::{EdgeAAQuad, Flags as AAFlags};
use skia_rust_gpu::graphite::geom::geometry::Geometry;
use skia_rust_gpu::graphite::geom::non_msaa_clip::AnalyticClip;
use skia_rust_gpu::graphite::geom::rect::Rect;
use skia_rust_gpu::graphite::geom::shape::Shape;
use skia_rust_gpu::graphite::geom::transform::Transform;
use skia_rust_gpu::graphite::graphite_types::DepthStencilFlags;
use skia_rust_gpu::graphite::render::circular_arc_render_step::CircularArcRenderStep;
use skia_rust_gpu::graphite::render::common_depth_stencil_settings::REGULAR_COVER_PASS;
use skia_rust_gpu::graphite::render::cover_bounds_render_step::CoverBoundsRenderStep;
use skia_rust_gpu::graphite::render::per_edge_aa_quad_render_step::PerEdgeAAQuadRenderStep;
use skia_rust_gpu::graphite::render_step::{Coverage, RenderStep, RenderStepID};
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
        AnalyticClip::default(),
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
                RenderStateFlags::APPEND_INSTANCES,
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
        cover.vertex_sksl(),
        concat!(
            "float4 devPosition = cover_bounds_vertex_fn(",
            "float2(sk_VertexID / 2, sk_VertexID % 2), ",
            "bounds, depth, float3x3(mat0, mat1, mat2), ",
            "stepLocalCoords);\n"
        )
    );

    let quad = PerEdgeAAQuadRenderStep::new(Layout::Std140, &mut manager);
    assert_eq!(
        quad.vertex_sksl(),
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
        arc.vertex_sksl(),
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
    let provider = RendererProvider::new(Layout::Std140, &mut manager);

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
}
