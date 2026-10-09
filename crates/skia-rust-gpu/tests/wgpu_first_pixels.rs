// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! G11c: the first pixels. Draws recorded on a Graphite `Device` are inserted into a context on a
//! real adapter, run by the wgpu command buffer, and read back with `Context::read_pixels`.
//!
//! The tests need an adapter that renders: Mesa's lavapipe or WARP (`docs/design/gpu.md` §2). On a
//! machine without one they report that and pass, unless `SKIA_RUST_REQUIRE_ADAPTER` is set (the
//! GPU CI jobs set it). The context's caps are computed from the adapter (`ShaderF16` stays off),
//! so the pipelines are the all-f32 ones the D3D12 goldens used.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use skia_rust_core::color::Color4f;
use skia_rust_core::device::Device as CoreDevice;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect as SkRect};
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_gpu::gpu::backing_fit::BackingFit;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, CallbackResult, Mipmapped};
use skia_rust_core::size::ISize;
use skia_rust_gpu::graphite::async_read::PixelTransferResult;
use skia_rust_gpu::graphite::context_priv::ContextPriv;
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::device::Device;
use skia_rust_gpu::graphite::graphite_types::{
    InsertRecordingInfo, InsertStatus, SubmitInfo, SyncToCpu,
};
use skia_rust_gpu::graphite::resource_types::{AccessPattern, BufferType, LoadOp};
use skia_rust_gpu::graphite::task::copy_task::CopyBufferToBufferTask;
use skia_rust_gpu::graphite::task::synchronize_to_cpu_task::SynchronizeToCpuTask;
use skia_rust_gpu::graphite::wgpu::{WgpuContext, adapter_backend_context, make_context};

/// A context on a real adapter, or `None` (after saying so) if the machine has none.
fn real_context() -> Option<WgpuContext> {
    let Some((backend_context, info)) = adapter_backend_context(wgpu::Backends::all()) else {
        assert!(
            std::env::var_os("SKIA_RUST_REQUIRE_ADAPTER").is_none(),
            "SKIA_RUST_REQUIRE_ADAPTER is set but there is no adapter"
        );
        eprintln!("no adapter that renders: skipping");
        return None;
    };
    eprintln!(
        "adapter: {} ({:?}, {:?})",
        info.name, info.backend, info.device_type
    );
    Some(make_context(&backend_context, &ContextOptions::default()).expect("a context"))
}

fn solid(r: f32, g: f32, b: f32) -> Paint {
    let mut paint = Paint::new(Color4f::new(r, g, b, 1.0), None);
    paint.set_anti_alias(false);
    paint
}

const SIZE: i32 = 32;

/// Draws `rects` into a cleared `SIZE` x `SIZE` N32 target and returns its pixels as `(RGBA
/// bytes, row bytes)`.
fn render(context: &mut WgpuContext, rects: &[(SkRect, Paint)]) -> Option<(Vec<u8>, usize)> {
    let mut recorder = context.make_recorder(None);
    let image_info = ImageInfo::new_n32_premul((SIZE, SIZE), None);
    let mut device = Device::make_with_info(
        Some(&recorder),
        &image_info,
        Budgeted::Yes,
        Mipmapped::No,
        BackingFit::Exact,
        &SurfaceProps::default(),
        LoadOp::Clear,
        "FirstPixels",
        true,
        false,
    )
    .expect("a device");
    for (rect, paint) in rects {
        device.draw_rect(rect, paint);
    }
    let target = device.target();
    device.flush_pending_work();

    let mut recording = recorder.snap().expect("a recording");
    let status = context.insert_recording(InsertRecordingInfo::new(&mut recording));
    assert_eq!(status, InsertStatus::Success);
    assert!(context.submit(SubmitInfo::new(SyncToCpu::Yes)));
    drop(device);

    context.read_pixels(
        &target,
        image_info.color_info(),
        IRect::from_size((SIZE, SIZE)),
        &image_info,
    )
}

fn pixel(pixels: &[u8], row_bytes: usize, x: usize, y: usize) -> [u8; 4] {
    let i = y * row_bytes + x * 4;
    pixels[i..i + 4].try_into().unwrap()
}

#[test]
fn a_cleared_target_reads_back_transparent() {
    let Some(mut context) = real_context() else {
        return;
    };
    let (pixels, row_bytes) = render(&mut context, &[]).expect("the pixels");
    // An empty device draws nothing, but its clear is a render pass.
    for y in 0..SIZE as usize {
        for x in 0..SIZE as usize {
            assert_eq!(pixel(&pixels, row_bytes, x, y), [0, 0, 0, 0]);
        }
    }
}

#[test]
fn a_rect_draws_its_color_and_only_there() {
    let Some(mut context) = real_context() else {
        return;
    };
    let rects = [
        (SkRect::new(4.0, 4.0, 20.0, 12.0), solid(1.0, 0.0, 0.0)),
        (SkRect::new(8.0, 16.0, 28.0, 28.0), solid(0.0, 0.0, 1.0)),
    ];
    let (pixels, row_bytes) = render(&mut context, &rects).expect("the pixels");
    for y in 0..SIZE as usize {
        for x in 0..SIZE as usize {
            let expected = if (4..20).contains(&x) && (4..12).contains(&y) {
                [255, 0, 0, 255]
            } else if (8..28).contains(&x) && (16..28).contains(&y) {
                [0, 0, 255, 255]
            } else {
                [0, 0, 0, 0]
            };
            assert_eq!(pixel(&pixels, row_bytes, x, y), expected, "pixel ({x}, {y})");
        }
    }
}

#[test]
fn a_buffer_copy_is_read_back_through_the_async_map() {
    let Some(mut context) = real_context() else {
        return;
    };
    let mut recorder = context.make_recorder(None);
    let (src, dst) = {
        let mut provider = ContextPriv::resource_provider(&context).lock().unwrap();
        (
            provider
                .find_or_create_non_shareable_buffer(
                    64,
                    BufferType::XferCpuToGpu,
                    AccessPattern::HostVisible,
                    "Src",
                )
                .unwrap(),
            provider
                .find_or_create_non_shareable_buffer(
                    64,
                    BufferType::XferGpuToCpu,
                    AccessPattern::HostVisible,
                    "Dst",
                )
                .unwrap(),
        )
    };
    let mut data = src.map().expect("the upload buffer is mapped");
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = u8::try_from(i).unwrap();
    }
    src.unmap_with(&data);
    recorder.priv_().add(CopyBufferToBufferTask::make(
        src.as_arc(),
        0,
        dst.clone(),
        0,
        64,
    ));
    recorder
        .priv_()
        .add(SynchronizeToCpuTask::make(dst.clone()));
    let mut recording = recorder.snap().expect("a recording");

    let finished = Arc::new(AtomicBool::new(false));
    let signal = finished.clone();
    let mut info = InsertRecordingInfo::new(&mut recording);
    info.finished_proc = Some(Box::new(move |result| {
        assert_eq!(result, CallbackResult::Success);
        signal.store(true, Ordering::Release);
    }));
    assert_eq!(context.insert_recording(info), InsertStatus::Success);

    // The transfer buffer is mapped when the submission finishes, and its bytes go to the
    // callback.
    let read: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
    let slot = read.clone();
    context.finalize_async_read_pixels(
        None,
        vec![PixelTransferResult {
            transfer_buffer: Some(dst),
            size: ISize::new(64, 1),
            row_bytes: 64,
            pixel_converter: None,
        }],
        Box::new(move |result| {
            let result = result.expect("the read succeeded");
            assert_eq!(result.count(), 1);
            assert_eq!(result.row_bytes(0), 64);
            *slot.lock().unwrap() = Some(result.data(0).to_vec());
        }),
    );
    assert!(context.submit(SubmitInfo::new(SyncToCpu::Yes)));
    assert!(finished.load(Ordering::Acquire));

    let expected: Vec<u8> = (0..64u8).collect();
    assert_eq!(read.lock().unwrap().as_deref(), Some(&expected[..]));
}
