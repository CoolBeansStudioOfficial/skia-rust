// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Checks the offsets and sizes of `StorageContext` against the expectations of
// tests/graphite/StorageContextTest.cpp. The 1:1 port is tests/src/unit/graphite/
// storage_context_test.rs; these checks are this crate's own and also check the dependency count.

#![cfg(not(target_arch = "wasm32"))]

use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::point::Point;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation};
use skia_rust_effects::linear_gradient::LinearGradient;
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::context_priv::ContextPriv;
use skia_rust_gpu::graphite::storage_context::{StorageContext, StorageContextResult};
use skia_rust_gpu::graphite::wgpu::{make_context, noop_backend_context};

fn gradient() -> LinearGradient {
    let colors = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(0.0, 0.0, 1.0, 1.0),
    ];
    let desc = Gradient::new(
        Colors::new(&colors, None, TileMode::Clamp, None::<ColorSpace>),
        Interpolation::default(),
    );
    LinearGradient::new(&[Point::new(0.0, 0.0), Point::new(100.0, 100.0)], &desc)
}

// The test writes small integers into f32 slots, which f32 holds exactly.
#[allow(clippy::cast_precision_loss)]
#[test]
fn storage_context_offsets_and_sizes() {
    let context = make_context(&noop_backend_context(), &ContextOptions::default())
        .expect("a context on the noop device");
    let recorder = context.make_recorder(None);
    let caps = ContextPriv::caps(&context);
    let use_storage = caps.storage_buffer_support();
    let max_fallback = caps
        .resource_binding_requirements()
        .max_fallback_texture_size;

    // Port of: tests/graphite/StorageContextTest.cpp#L50-L123 (the first alignment test), with the
    // offsets it checks: gradient 1 at 0, its duplicate at 0 with no new data, gradient 2 after it.
    let grad1 = gradient();
    let grad2 = gradient();
    let float_count: i32 = if use_storage { 10 } else { 12 };
    let mut ctx = StorageContext::new(max_fallback, use_storage);

    let (ptr1, offset1) = ctx.allocate_gradient_data(2, grad1.base());
    assert!(ptr1.is_some());
    assert_eq!(offset1, 0);
    if let Some(ptr1) = ptr1 {
        for (i, v) in ptr1.iter_mut().enumerate() {
            *v = 10.0 + i as f32;
        }
    }

    let (ptr1_dup, offset1_dup) = ctx.allocate_gradient_data(2, grad1.base());
    assert!(ptr1_dup.is_none());
    assert_eq!(offset1_dup, offset1);

    let (ptr2, offset2) = ctx.allocate_gradient_data(2, grad2.base());
    assert!(ptr2.is_some());
    assert_eq!(offset2, float_count);

    ctx.record_alignment(16, 16);
    ctx.finalize_precached_storage_data();
    assert_eq!(ctx.running_lcm(), 16);

    let mut dependencies = 0;
    let result = ctx.finalize(&recorder.priv_(), &mut |_| dependencies += 1);
    assert!(result.is_some());
    match result {
        Some(StorageContextResult::Buffer(bind)) => {
            assert!(bind.buffer.is_some());
            assert_eq!(bind.size, (2 * float_count * 4).cast_unsigned());
            assert_eq!(bind.size % 16, 0);
        }
        Some(StorageContextResult::Texture(_)) => {}
        None => panic!("finalize returned no result for non-empty data"),
    }
    // Only the texture path records an upload.
    assert_eq!(dependencies, usize::from(!use_storage));
}

#[test]
fn storage_context_append_vertices_offsets() {
    let context = make_context(&noop_backend_context(), &ContextOptions::default())
        .expect("a context on the noop device");
    let recorder = context.make_recorder(None);
    let caps = ContextPriv::caps(&context);
    let use_storage = caps.storage_buffer_support();
    let max_fallback = caps
        .resource_binding_requirements()
        .max_fallback_texture_size;
    let grad = gradient();
    let mut ctx = StorageContext::new(max_fallback, use_storage);

    // Port of: tests/graphite/StorageContextTest.cpp#L187-L258 (StorageContextAppendVertexTest).
    let (grad_ptr, grad_offset) = ctx.allocate_gradient_data(2, grad.base());
    assert!(grad_ptr.is_some());
    assert_eq!(grad_offset, 0);
    ctx.record_alignment(24, 16);
    ctx.finalize_precached_storage_data();

    let verts = [
        0.0_f32, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0,
    ];
    let verts_bytes: Vec<u8> = verts.iter().flat_map(|f| f.to_ne_bytes()).collect();
    let v_offset = ctx.append_vertices(&verts_bytes, 2, 24, 16);
    let expected_v_offset = if use_storage { 48 } else { 64 };
    assert_eq!(v_offset, expected_v_offset);

    let mut dependencies = 0;
    let result = ctx.finalize(&recorder.priv_(), &mut |_| dependencies += 1);
    if let Some(StorageContextResult::Buffer(bind)) = result {
        assert_eq!(bind.size, 96);
        assert_eq!(bind.size % 48, 0);
    }
}

#[test]
fn storage_context_multiple_render_steps_offsets() {
    let context = make_context(&noop_backend_context(), &ContextOptions::default())
        .expect("a context on the noop device");
    let caps = ContextPriv::caps(&context);
    let use_storage = caps.storage_buffer_support();
    let max_fallback = caps
        .resource_binding_requirements()
        .max_fallback_texture_size;
    let grad = gradient();
    let mut ctx = StorageContext::new(max_fallback, use_storage);

    // Port of: tests/graphite/StorageContextTest.cpp#L260-L345 (StorageContextMultipleRenderStepsTest).
    let (grad_ptr, _) = ctx.allocate_gradient_data(2, grad.base());
    assert!(grad_ptr.is_some());
    ctx.record_alignment(24, 16);
    ctx.record_alignment(32, 16);
    ctx.finalize_precached_storage_data();
    assert_eq!(ctx.running_lcm(), 96);

    let data_a = [1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0];
    let bytes_a: Vec<u8> = data_a.iter().flat_map(|f| f.to_ne_bytes()).collect();
    let offset_a = ctx.append_vertices(&bytes_a, 1, 24, 16);
    assert_eq!(offset_a, if use_storage { 96 } else { 64 });

    let data_b = [1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let bytes_b: Vec<u8> = data_b.iter().flat_map(|f| f.to_ne_bytes()).collect();
    let offset_b = ctx.append_vertices(&bytes_b, 1, 32, 16);
    assert_eq!(offset_b, if use_storage { 128 } else { 96 });
}
