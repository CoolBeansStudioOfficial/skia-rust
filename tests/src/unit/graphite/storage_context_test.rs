// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/StorageContextTest.cpp (chrome/m156)

#![cfg(test)]
// Mirrors the C++ tests, which write small integers into float slots and declare similarly named
// bindings in long functions.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    // There are no tolerances in this project: the C++ compares the floats exactly.
    clippy::float_cmp,
    clippy::items_after_statements,
    clippy::manual_is_multiple_of,
    // The loops index several arrays in parallel, as the C++ does.
    clippy::manual_memcpy,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::too_many_lines
)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::colors;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::point::Point;
use skia_rust_core::size::ISize;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation};
use skia_rust_effects::linear_gradient::LinearGradient;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped, Protected, Renderable};
use skia_rust_gpu::graphite::buffer::BindBufferInfo;
use skia_rust_gpu::graphite::buffer_manager::buffer_aligner;
use skia_rust_gpu::graphite::draw_context::DrawContext;
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::storage_context::{StorageContext, StorageContextResult};
use skia_rust_gpu::graphite::texture_proxy::TextureProxy;

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

// `make_draw_context()` of the C++ file's anonymous namespace.
fn make_draw_context(recorder: &Recorder) -> Option<DrawContext> {
    let priv_ = recorder.priv_();
    let caps = &**priv_.caps();
    let color_info = ColorInfo::new(ColorType::RGBA8888, AlphaType::Premul, None);
    let tex_info = caps.get_default_sampled_texture_info(
        color_info.color_type(),
        Mipmapped::No,
        Protected::No,
        Renderable::Yes,
    );
    let target = TextureProxy::make(
        caps,
        &mut priv_.resource_provider().lock().unwrap(),
        ISize::new(16, 16),
        &tex_info,
        Budgeted::Yes,
        "StorageContextTestTarget",
    );
    DrawContext::make(
        caps,
        target,
        ISize::new(16, 16),
        &color_info,
        &SurfaceProps::default(),
        /* allow_unpremul= */ false,
    )
}

// `SkLinearGradient` between the two points of the tests with `colors` and `tile_mode`.
fn make_gradient(
    colors: &[skia_rust_core::color::Color4f],
    pos: Option<&[f32]>,
    tile_mode: TileMode,
) -> LinearGradient {
    let pts = [Point::new(0.0, 0.0), Point::new(100.0, 100.0)];
    LinearGradient::new(
        &pts,
        &Gradient::new(
            Colors::new(colors, pos, tile_mode, None),
            Interpolation::default(),
        ),
    )
}

// `buffer->map()` plus the bind's offset: the bytes of the bound range.
fn mapped_bytes(bind: &BindBufferInfo) -> Vec<u8> {
    let buffer = bind.buffer.as_ref().expect("a buffer");
    let data = buffer.map().expect("a mapped buffer");
    data[bind.offset as usize..].to_vec()
}

fn floats(bytes: &[u8], count: usize) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .take(count)
        .map(|c| f32::from_ne_bytes(*c))
        .collect()
}

fn float_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|f| f.to_ne_bytes()).collect()
}

// Port of: tests/graphite/StorageContextTest.cpp#L50-L123 (chrome/m156)
def_graphite_test_for_all_contexts!(StorageContextAlignmentTest, |reporter, context| {
    let recorder = context.make_recorder(None);

    let colors = [colors::RED, colors::BLUE];
    let grad1 = make_gradient(&colors, None, TileMode::Clamp);
    let grad2 = make_gradient(&colors, None, TileMode::Repeat);

    let caps = recorder.priv_().caps().clone();
    let use_storage = caps.storage_buffer_support();
    let mut ctx_storage = StorageContext::new(
        caps.resource_binding_requirements()
            .max_fallback_texture_size,
        use_storage,
    );
    let ctx_handle = &mut ctx_storage;

    let k_float_count: i32 = if use_storage { 10 } else { 12 };

    // 1. Allocate gradient data for shader1
    let (ptr1, offset1) = ctx_handle.allocate_gradient_data(2, grad1.base());
    reporter_assert!(reporter, ptr1.is_some());
    reporter_assert!(reporter, offset1 == 0);
    if let Some(ptr1) = ptr1 {
        for i in 0..k_float_count {
            ptr1[i as usize] = 10.0 + i as f32;
        }
    }

    // 2. Allocate gradient data again for shader1 (deduplication check)
    let (ptr1_dup, offset1_dup) = ctx_handle.allocate_gradient_data(2, grad1.base());
    reporter_assert!(reporter, ptr1_dup.is_none());
    reporter_assert!(reporter, offset1_dup == offset1);

    // 3. Allocate gradient data for shader2
    let (ptr2, offset2) = ctx_handle.allocate_gradient_data(2, grad2.base());
    reporter_assert!(reporter, ptr2.is_some());
    reporter_assert!(reporter, offset2 == k_float_count);
    if let Some(ptr2) = ptr2 {
        for i in 0..k_float_count {
            ptr2[i as usize] = 30.0 + i as f32;
        }
    }

    // Record vertex requirement with stride 16 and align 16, setting running LCM to 16
    ctx_handle.record_alignment(/* stride= */ 16, /* align= */ 16);

    ctx_handle.finalize_precached_storage_data();

    // Finalize storage buffer allocation and check 16-byte alignment
    let mut draw_context = make_draw_context(&recorder).expect("a draw context");
    let storage_result = ctx_handle.finalize(&recorder.priv_(), &mut |t| {
        draw_context.record_dependency(t);
    });
    reporter_assert!(reporter, storage_result.is_some());
    let Some(storage_result) = storage_result else {
        return;
    };
    match storage_result {
        StorageContextResult::Buffer(bind_info) => {
            reporter_assert!(reporter, bind_info.buffer.is_some());
            reporter_assert!(reporter, bind_info.size == (2 * k_float_count * 4) as u32);
            reporter_assert!(reporter, bind_info.size % 16 == 0);

            if !caps.draw_buffer_can_be_mapped() {
                return;
            }

            let buffer_data = mapped_bytes(&bind_info);
            let float_data = floats(&buffer_data, 2 * k_float_count as usize);
            for i in 0..k_float_count as usize {
                reporter_assert!(reporter, float_data[i] == 10.0 + i as f32);
                reporter_assert!(
                    reporter,
                    float_data[k_float_count as usize + i] == 30.0 + i as f32
                );
            }
        }
        StorageContextResult::Texture(_proxy) => {}
    }
});

// Port of: tests/graphite/StorageContextTest.cpp#L125-L185 (chrome/m156)
def_graphite_test_for_all_contexts!(StorageContextPaddingAlignmentTest, |reporter, context| {
    let recorder = context.make_recorder(None);

    let colors = [colors::RED, colors::BLUE];
    let grad = make_gradient(&colors, None, TileMode::Clamp);

    let caps = recorder.priv_().caps().clone();
    let use_storage = caps.storage_buffer_support();
    let mut ctx_storage = StorageContext::new(
        caps.resource_binding_requirements()
            .max_fallback_texture_size,
        use_storage,
    );
    let ctx_handle = &mut ctx_storage;

    let k_float_count: i32 = if use_storage { 10 } else { 12 };
    let k_bytes = k_float_count as usize * 4;

    // Allocate gradient data for 1 shader with 2 stops
    let (ptr, offset) = ctx_handle.allocate_gradient_data(2, grad.base());
    reporter_assert!(reporter, ptr.is_some());
    reporter_assert!(reporter, offset == 0);
    if let Some(ptr) = ptr {
        for i in 0..k_float_count {
            ptr[i as usize] = 30.0 + i as f32;
        }
    }

    // Record vertex requirement with stride 32 and align 16, setting running LCM to 32
    ctx_handle.record_alignment(/* stride= */ 32, /* align= */ 16);

    ctx_handle.finalize_precached_storage_data();

    // Finalize: gradient bytes should be padded to 64 bytes (aligned to 32 bytes)
    let mut draw_context = make_draw_context(&recorder).expect("a draw context");
    let storage_result = ctx_handle.finalize(&recorder.priv_(), &mut |t| {
        draw_context.record_dependency(t);
    });
    reporter_assert!(reporter, storage_result.is_some());
    let Some(storage_result) = storage_result else {
        return;
    };
    match storage_result {
        StorageContextResult::Buffer(bind_info) => {
            reporter_assert!(reporter, bind_info.buffer.is_some());
            reporter_assert!(reporter, bind_info.size == 64);
            reporter_assert!(reporter, bind_info.size % 32 == 0);

            if !caps.draw_buffer_can_be_mapped() {
                return;
            }

            let buffer_data = mapped_bytes(&bind_info);
            let float_data = floats(&buffer_data, k_float_count as usize);
            for i in 0..k_float_count as usize {
                reporter_assert!(reporter, float_data[i] == 30.0 + i as f32);
            }
            for i in k_bytes..64 {
                reporter_assert!(reporter, buffer_data[i] == 0);
            }
        }
        StorageContextResult::Texture(_proxy) => {}
    }
});

// Port of: tests/graphite/StorageContextTest.cpp#L187-L258 (chrome/m156)
def_graphite_test_for_all_contexts!(StorageContextAppendVertexTest, |reporter, context| {
    let recorder = context.make_recorder(None);

    let colors = [colors::RED, colors::BLUE];
    let grad = make_gradient(&colors, None, TileMode::Clamp);

    let caps = recorder.priv_().caps().clone();
    let use_storage = caps.storage_buffer_support();
    let mut ctx_storage = StorageContext::new(
        caps.resource_binding_requirements()
            .max_fallback_texture_size,
        use_storage,
    );
    let ctx_handle = &mut ctx_storage;

    let k_float_count: i32 = if use_storage { 10 } else { 12 };
    let k_bytes = k_float_count as usize * 4;

    // 1. Allocate gradient data
    let (grad_ptr, grad_offset) = ctx_handle.allocate_gradient_data(2, grad.base());
    reporter_assert!(reporter, grad_ptr.is_some());
    reporter_assert!(reporter, grad_offset == 0);
    if let Some(grad_ptr) = grad_ptr {
        for i in 0..k_float_count {
            grad_ptr[i as usize] = 40.0 + i as f32;
        }
    }

    // 2. Record vertex alignment requirement: stride 24, align 16 -> running LCM = 48
    ctx_handle.record_alignment(/* stride= */ 24, /* align= */ 16);

    // 3. Finalize precached storage data: aligned to running LCM (48 bytes)
    ctx_handle.finalize_precached_storage_data();

    // 4. Append vertices with stride 24, align 16, count 2
    // With storage: stride 24, align 16 -> LCM = 48. Padded grad size = 48.
    // Without storage: stride 24 -> 32, align 16 -> LCM = 32. Padded grad size = 64.
    let verts: [f32; 12] = [0., 1., 2., 3., 4., 5., 6., 7., 8., 9., 10., 11.];
    let verts_bytes = float_bytes(&verts);
    let v_offset = ctx_handle.append_vertices(
        &verts_bytes,
        /* count= */ 2,
        /* stride= */ 24,
        /* align= */ 16,
    );
    let expected_v_offset: u32 = if use_storage { 48 } else { 64 };
    reporter_assert!(reporter, v_offset == expected_v_offset);

    // 5. Finalize storage buffer
    let mut draw_context = make_draw_context(&recorder).expect("a draw context");
    let storage_result = ctx_handle.finalize(&recorder.priv_(), &mut |t| {
        draw_context.record_dependency(t);
    });
    reporter_assert!(reporter, storage_result.is_some());
    let Some(storage_result) = storage_result else {
        return;
    };
    match storage_result {
        StorageContextResult::Buffer(bind_info) => {
            reporter_assert!(reporter, bind_info.buffer.is_some());
            // Total size = 48 (aligned gradient) + 48 (vertices) = 96 bytes
            reporter_assert!(reporter, bind_info.size == 96);
            reporter_assert!(reporter, bind_info.size % 48 == 0);

            if !caps.draw_buffer_can_be_mapped() {
                return;
            }

            let buffer_data = mapped_bytes(&bind_info);
            let float_data = floats(&buffer_data, k_float_count as usize);
            for i in 0..k_float_count as usize {
                reporter_assert!(reporter, float_data[i] == 40.0 + i as f32);
            }
            for i in k_bytes..48 {
                reporter_assert!(reporter, buffer_data[i] == 0);
            }
            reporter_assert!(
                reporter,
                buffer_data[48..48 + verts_bytes.len()] == verts_bytes[..]
            );
        }
        StorageContextResult::Texture(_proxy) => {}
    }
});

// Port of: tests/graphite/StorageContextTest.cpp#L260-L345 (chrome/m156)
def_graphite_test_for_all_contexts!(
    StorageContextMultipleRenderStepsTest,
    |reporter, context| {
        let recorder = context.make_recorder(None);

        let colors = [colors::RED, colors::BLUE];
        let grad = make_gradient(&colors, None, TileMode::Clamp);

        let caps = recorder.priv_().caps().clone();
        let use_storage = caps.storage_buffer_support();
        let mut ctx_storage = StorageContext::new(
            caps.resource_binding_requirements()
                .max_fallback_texture_size,
            use_storage,
        );
        let ctx_handle = &mut ctx_storage;

        let k_float_count: i32 = if use_storage { 10 } else { 12 };
        let k_bytes = k_float_count as usize * 4;

        // 1. Allocate gradient data
        let (grad_ptr, grad_offset) = ctx_handle.allocate_gradient_data(2, grad.base());
        reporter_assert!(reporter, grad_ptr.is_some());
        reporter_assert!(reporter, grad_offset == 0);
        if let Some(grad_ptr) = grad_ptr {
            for i in 0..k_float_count {
                grad_ptr[i as usize] = 50.0 + i as f32;
            }
        }

        // 2. Record alignments from multiple render steps:
        // Step A: stride 24, align 16 -> LCM = 48
        // Step B: stride 32, align 16 -> LCM = 32
        // Running LCM = LCM(48, 32) = 96
        ctx_handle.record_alignment(/* stride= */ 24, /* align= */ 16);
        ctx_handle.record_alignment(/* stride= */ 32, /* align= */ 16);

        // 3. Finalize precached storage data: aligned to running LCM (96 bytes)
        ctx_handle.finalize_precached_storage_data();

        // 4. Step A appends 1 vertex of 24 bytes (stride 24, align 16)
        let data_a: [f32; 6] = [1., 2., 3., 4., 5., 6.];
        let data_a_bytes = float_bytes(&data_a);
        let offset_a = ctx_handle.append_vertices(
            &data_a_bytes,
            /* count= */ 1,
            /* stride= */ 24,
            /* align= */ 16,
        );
        let expected_offset_a: u32 = if use_storage { 96 } else { 64 };
        reporter_assert!(reporter, offset_a == expected_offset_a);

        // 5. Step B appends 1 vertex of 32 bytes (stride 32, align 16)
        // With storage: Local vertex size is 24; next 32-byte aligned offset is 32 (8 bytes
        // zero-padding) Without storage: Step A is already padded to 32 bytes; next aligned offset
        // is 32.
        let data_b: [f32; 8] = [1., 2., 3., 4., 5., 6., 7., 8.];
        let data_b_bytes = float_bytes(&data_b);
        let offset_b = ctx_handle.append_vertices(
            &data_b_bytes,
            /* count= */ 1,
            /* stride= */ 32,
            /* align= */ 16,
        );
        let expected_offset_b: u32 = if use_storage { 128 } else { 96 };
        reporter_assert!(reporter, offset_b == expected_offset_b);

        // 6. Finalize storage buffer: Total size = 96 (gradient) + 32 (local offset) + 32 (dataB) = 160
        let mut draw_context = make_draw_context(&recorder).expect("a draw context");
        let storage_result = ctx_handle.finalize(&recorder.priv_(), &mut |t| {
            draw_context.record_dependency(t);
        });
        reporter_assert!(reporter, storage_result.is_some());
        let Some(storage_result) = storage_result else {
            return;
        };
        match storage_result {
            StorageContextResult::Buffer(bind_info) => {
                reporter_assert!(reporter, bind_info.buffer.is_some());
                reporter_assert!(reporter, bind_info.size == 160);
                reporter_assert!(reporter, bind_info.size % 32 == 0);

                if !caps.draw_buffer_can_be_mapped() {
                    return;
                }

                let buffer_data = mapped_bytes(&bind_info);
                let float_data = floats(&buffer_data, k_float_count as usize);
                for i in 0..k_float_count as usize {
                    reporter_assert!(reporter, float_data[i] == 50.0 + i as f32);
                }
                for i in k_bytes..96 {
                    reporter_assert!(reporter, buffer_data[i] == 0);
                }
                reporter_assert!(
                    reporter,
                    buffer_data[96..96 + data_a_bytes.len()] == data_a_bytes[..]
                );
                for i in 120..128 {
                    reporter_assert!(reporter, buffer_data[i] == 0);
                }
                reporter_assert!(
                    reporter,
                    buffer_data[128..128 + data_b_bytes.len()] == data_b_bytes[..]
                );
            }
            StorageContextResult::Texture(_proxy) => {}
        }
    }
);

// Port of: tests/graphite/StorageContextTest.cpp#L347-L445 (chrome/m156)
def_graphite_test_for_all_contexts!(StorageContextLCMVariantsTest, |reporter, context| {
    let recorder = context.make_recorder(None);

    struct TestCase {
        stride: usize,
        align: usize,
        expected_lcm: u32,
    }

    let test_cases = [
        TestCase {
            stride: 12,
            align: 16,
            expected_lcm: 48,
        },
        TestCase {
            stride: 20,
            align: 16,
            expected_lcm: 80,
        },
        TestCase {
            stride: 24,
            align: 16,
            expected_lcm: 48,
        },
        TestCase {
            stride: 28,
            align: 16,
            expected_lcm: 112,
        },
        TestCase {
            stride: 36,
            align: 16,
            expected_lcm: 144,
        },
        TestCase {
            stride: 40,
            align: 16,
            expected_lcm: 80,
        },
        TestCase {
            stride: 64,
            align: 16,
            expected_lcm: 64,
        },
    ];

    let colors = [colors::RED, colors::BLUE];
    let grad = make_gradient(&colors, None, TileMode::Clamp);

    let caps = recorder.priv_().caps().clone();
    let use_storage = caps.storage_buffer_support();
    let k_float_count: i32 = if use_storage { 10 } else { 12 };
    let k_bytes = k_float_count as usize * 4;

    for tc in &test_cases {
        let mut ctx = StorageContext::new(
            caps.resource_binding_requirements()
                .max_fallback_texture_size,
            use_storage,
        );
        // Allocate 2 gradient stops
        let (grad_ptr, grad_offset) = ctx.allocate_gradient_data(2, grad.base());
        reporter_assert!(reporter, grad_ptr.is_some());
        reporter_assert!(reporter, grad_offset == 0);
        if let Some(grad_ptr) = grad_ptr {
            for i in 0..k_float_count {
                grad_ptr[i as usize] = 60.0 + i as f32;
            }
        }

        ctx.record_alignment(tc.stride, tc.align);
        ctx.finalize_precached_storage_data();

        let (padded_align, expected_lcm) = if use_storage {
            (tc.align, tc.expected_lcm)
        } else {
            let padded_stride = tc.stride.next_multiple_of(16);
            let padded_align = tc.align.max(16);
            (
                padded_align,
                buffer_aligner::lcm_alignment(padded_align as u32, padded_stride as u32),
            )
        };

        let expected_padded_grad_size =
            skia_rust_core::align::align_non_pow2(k_bytes as u32, expected_lcm);

        // Append 2 vertices
        let mut vert = vec![0_u8; tc.stride * 2];
        for (i, v) in vert.iter_mut().enumerate() {
            *v = ((i + 1) & 0x7F) as u8;
        }
        let v_offset = ctx.append_vertices(&vert, /* count= */ 2, tc.stride, tc.align);
        reporter_assert!(reporter, v_offset == expected_padded_grad_size);
        reporter_assert!(reporter, v_offset as usize % padded_align == 0);
        reporter_assert!(reporter, v_offset % expected_lcm == 0);

        let mut draw_context = make_draw_context(&recorder).expect("a draw context");
        let storage_result = ctx.finalize(&recorder.priv_(), &mut |t| {
            draw_context.record_dependency(t);
        });
        reporter_assert!(reporter, storage_result.is_some());
        let Some(storage_result) = storage_result else {
            return;
        };
        match storage_result {
            StorageContextResult::Buffer(bind_info) => {
                reporter_assert!(reporter, bind_info.buffer.is_some());
                reporter_assert!(
                    reporter,
                    bind_info.size as usize == expected_padded_grad_size as usize + tc.stride * 2
                );

                if !caps.draw_buffer_can_be_mapped() {
                    continue;
                }

                let buffer_data = mapped_bytes(&bind_info);
                let float_data = floats(&buffer_data, k_float_count as usize);
                for i in 0..k_float_count as usize {
                    reporter_assert!(reporter, float_data[i] == 60.0 + i as f32);
                }
                for i in k_bytes..expected_padded_grad_size as usize {
                    reporter_assert!(reporter, buffer_data[i] == 0);
                }
                let start = expected_padded_grad_size as usize;
                reporter_assert!(reporter, buffer_data[start..start + vert.len()] == vert[..]);
            }
            StorageContextResult::Texture(_proxy) => {}
        }
    }
});

// Port of: tests/graphite/StorageContextTest.cpp#L447-L531 (chrome/m156)
def_graphite_test_for_all_contexts!(StorageContextVertexOnlyAndResetTest, |reporter, context| {
    let recorder = context.make_recorder(None);
    let caps = recorder.priv_().caps().clone();
    let use_storage = caps.storage_buffer_support();
    let mut ctx = StorageContext::new(
        caps.resource_binding_requirements()
            .max_fallback_texture_size,
        use_storage,
    );

    reporter_assert!(reporter, ctx.is_empty());

    let mut draw_context = make_draw_context(&recorder).expect("a draw context");

    // 1. Finalize on empty context returns nullopt
    ctx.finalize_precached_storage_data();
    let empty_result = ctx.finalize(&recorder.priv_(), &mut |t| {
        draw_context.record_dependency(t);
    });
    reporter_assert!(reporter, empty_result.is_none());

    // 2. Vertex-only allocation without gradients
    ctx.record_alignment(/* stride= */ 24, /* align= */ 16);
    ctx.finalize_precached_storage_data();

    let vert: [f32; 6] = [1., 2., 3., 4., 5., 6.];
    let vert_bytes = float_bytes(&vert);
    let offset = ctx.append_vertices(
        &vert_bytes,
        /* count= */ 1,
        /* stride= */ 24,
        /* align= */ 16,
    );
    reporter_assert!(reporter, offset == 0);
    reporter_assert!(reporter, !ctx.is_empty());

    let vertex_result = ctx.finalize(&recorder.priv_(), &mut |t| {
        draw_context.record_dependency(t);
    });
    reporter_assert!(reporter, vertex_result.is_some());
    match vertex_result {
        Some(StorageContextResult::Buffer(bind_info)) => {
            reporter_assert!(reporter, bind_info.buffer.is_some());
            reporter_assert!(reporter, bind_info.size == 24);

            if caps.draw_buffer_can_be_mapped() {
                let buffer_data = mapped_bytes(&bind_info);
                reporter_assert!(reporter, buffer_data[..vert_bytes.len()] == vert_bytes[..]);
            }
        }
        Some(StorageContextResult::Texture(_proxy)) => {}
        None => return,
    }

    // 3. Reset cache and verify clean state
    ctx.reset_cache();
    reporter_assert!(reporter, ctx.is_empty());

    // 4. Subsequent allocation after reset starts at offset 0
    let colors = [colors::RED, colors::BLUE];
    let grad = make_gradient(&colors, None, TileMode::Clamp);
    let k_float_count: i32 = if use_storage { 10 } else { 12 };
    let (grad_ptr, grad_offset) = ctx.allocate_gradient_data(2, grad.base());
    reporter_assert!(reporter, grad_ptr.is_some());
    reporter_assert!(reporter, grad_offset == 0);
    if let Some(grad_ptr) = grad_ptr {
        for i in 0..k_float_count {
            grad_ptr[i as usize] = 70.0 + i as f32;
        }
    }

    ctx.finalize_precached_storage_data();
    let post_reset_result = ctx.finalize(&recorder.priv_(), &mut |t| {
        draw_context.record_dependency(t);
    });
    reporter_assert!(reporter, post_reset_result.is_some());
    match post_reset_result {
        Some(StorageContextResult::Buffer(reset_info)) => {
            reporter_assert!(reporter, reset_info.buffer.is_some());

            if !caps.draw_buffer_can_be_mapped() {
                return;
            }

            let reset_buffer_data = mapped_bytes(&reset_info);
            let reset_floats = floats(&reset_buffer_data, k_float_count as usize);
            for i in 0..k_float_count as usize {
                reporter_assert!(reporter, reset_floats[i] == 70.0 + i as f32);
            }
        }
        Some(StorageContextResult::Texture(_proxy)) => {}
        None => {}
    }
});

// Port of: tests/graphite/StorageContextTest.cpp#L533-L645 (chrome/m156)
def_graphite_test_for_all_contexts!(StorageContextMultiStopGradientTest, |reporter, context| {
    let recorder = context.make_recorder(None);
    let caps = recorder.priv_().caps().clone();
    let use_storage = caps.storage_buffer_support();
    let mut ctx = StorageContext::new(
        caps.resource_binding_requirements()
            .max_fallback_texture_size,
        use_storage,
    );

    let aligned_offsets9: usize = if use_storage { 9 } else { 12 };
    let count9: usize = if use_storage {
        9 * 5
    } else {
        (aligned_offsets9 + 9 * 4).next_multiple_of(4)
    };

    let colors9 = vec![colors::RED; 9];
    let pos9: Vec<f32> = vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 1.0];
    let grad9 = make_gradient(&colors9, Some(&pos9), TileMode::Clamp);

    let (ptr9, offset9) = ctx.allocate_gradient_data(9, grad9.base());
    reporter_assert!(reporter, ptr9.is_some());
    reporter_assert!(reporter, offset9 == 0);

    // Populate offsets and colors
    if let Some(ptr9) = ptr9 {
        for i in 0..9 {
            ptr9[i] = pos9[i];
        }
        for i in 9..aligned_offsets9 {
            ptr9[i] = 0.0; // padding
        }
        for i in 0..9 {
            ptr9[aligned_offsets9 + i * 4] = i as f32;
            ptr9[aligned_offsets9 + i * 4 + 1] = i as f32 * 0.1;
            ptr9[aligned_offsets9 + i * 4 + 2] = 0.5;
            ptr9[aligned_offsets9 + i * 4 + 3] = 1.0;
        }
    }

    let aligned_offsets17: usize = if use_storage { 17 } else { 20 };
    let count17: usize = if use_storage {
        17 * 5
    } else {
        (aligned_offsets17 + 17 * 4).next_multiple_of(4)
    };

    let colors17 = vec![colors::BLUE; 17];
    let pos17: Vec<f32> = (0..17).map(|i| i as f32 / 16.0).collect();
    let grad17 = make_gradient(&colors17, Some(&pos17), TileMode::Repeat);

    let (ptr17, offset17) = ctx.allocate_gradient_data(17, grad17.base());
    reporter_assert!(reporter, ptr17.is_some());
    reporter_assert!(reporter, offset17 == count9 as i32);

    if let Some(ptr17) = ptr17 {
        for i in 0..17 {
            ptr17[i] = pos17[i];
        }
        for i in 17..aligned_offsets17 {
            ptr17[i] = 0.0; // padding
        }
        for i in 0..17 {
            ptr17[aligned_offsets17 + i * 4] = i as f32;
            ptr17[aligned_offsets17 + i * 4 + 1] = i as f32 * 0.05;
            ptr17[aligned_offsets17 + i * 4 + 2] = 0.25;
            ptr17[aligned_offsets17 + i * 4 + 3] = 1.0;
        }
    }

    ctx.finalize_precached_storage_data();

    let mut draw_context = make_draw_context(&recorder).expect("a draw context");
    let storage_result = ctx.finalize(&recorder.priv_(), &mut |t| {
        draw_context.record_dependency(t);
    });
    reporter_assert!(reporter, storage_result.is_some());
    let Some(storage_result) = storage_result else {
        return;
    };
    match storage_result {
        StorageContextResult::Buffer(bind_info) => {
            reporter_assert!(reporter, bind_info.buffer.is_some());
            reporter_assert!(reporter, bind_info.size as usize == (count9 + count17) * 4);
            reporter_assert!(reporter, bind_info.size % 16 == 0 || use_storage);

            if !caps.draw_buffer_can_be_mapped() {
                return;
            }

            let buffer_data = mapped_bytes(&bind_info);
            let float_data = floats(&buffer_data, count9 + count17);

            // Check grad9 data
            for i in 0..9 {
                reporter_assert!(reporter, float_data[i] == pos9[i]);
            }
            for i in 0..9 {
                reporter_assert!(reporter, float_data[aligned_offsets9 + i * 4] == i as f32);
                reporter_assert!(
                    reporter,
                    float_data[aligned_offsets9 + i * 4 + 1] == i as f32 * 0.1
                );
            }

            // Check grad17 data
            let float17 = &float_data[count9..];
            for i in 0..17 {
                reporter_assert!(reporter, float17[i] == pos17[i]);
            }
            for i in 0..17 {
                reporter_assert!(reporter, float17[aligned_offsets17 + i * 4] == i as f32);
                reporter_assert!(
                    reporter,
                    float17[aligned_offsets17 + i * 4 + 1] == i as f32 * 0.05
                );
            }
        }
        StorageContextResult::Texture(_proxy) => {}
    }
});

// Port of: tests/graphite/StorageContextTest.cpp#L647-L690 (chrome/m156)
def_graphite_test_for_all_contexts!(
    StorageContextFallbackStridedCopyTest,
    |reporter, context| {
        let recorder = context.make_recorder(None);
        let mut ctx = StorageContext::new(
            recorder
                .priv_()
                .caps()
                .resource_binding_requirements()
                .max_fallback_texture_size,
            /* storage_buffer_support= */ false,
        );

        // Record alignment for stride 24, align 16. Because storageBufferSupport is false,
        // stride is rounded up to 32 and align to at least 16. LCM = 32.
        ctx.record_alignment(/* stride= */ 24, /* align= */ 16);
        ctx.finalize_precached_storage_data();

        // Append 2 vertices with original stride 24 (6 floats = 24 bytes per vertex; 48 bytes total)
        let verts: [f32; 12] = [1., 2., 3., 4., 5., 6., 7., 8., 9., 10., 11., 12.];
        let offset = ctx.append_vertices(
            &float_bytes(&verts),
            /* count= */ 2,
            /* stride= */ 24,
            /* align= */ 16,
        );
        reporter_assert!(reporter, offset == 0);

        let draw_context = make_draw_context(&recorder);
        reporter_assert!(reporter, draw_context.is_some());
        let Some(mut draw_context) = draw_context else {
            return;
        };

        // Finalize: padded to 32 bytes per vertex -> 2 * 32 = 64 bytes total
        let storage_result = ctx.finalize(&recorder.priv_(), &mut |t| {
            draw_context.record_dependency(t);
        });
        reporter_assert!(reporter, storage_result.is_some());
        let Some(StorageContextResult::Texture(proxy)) = storage_result else {
            reporter_assert!(reporter, false);
            return;
        };
        reporter_assert!(reporter, proxy.dimensions() == ISize::new(4, 1));
        reporter_assert!(reporter, draw_context.snap_draw_task().is_some());
    }
);
