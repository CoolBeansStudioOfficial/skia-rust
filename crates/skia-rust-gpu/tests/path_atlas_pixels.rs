// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! G12a: path draws through the raster path atlas, inserted into a context on a real adapter, run
//! by the wgpu command buffer and read back with `Context::read_pixels`. The coverage masks are
//! rasterized on the CPU and sampled by the `CoverageMask` render step.
//!
//! The pixels are exact where the paint is not anti-aliased (the interior and the exterior of the
//! path), and partial at an anti-aliased edge. The checks hold on lavapipe (Linux) and WARP
//! (Windows). The noop-adapter checks of the same draws are in `path_atlas.rs`.
//!
//! Like the other real-adapter tests these are `#[ignore]`d, so CI counts them as not run. Run
//! them with `cargo test -p skia-rust-gpu --test path_atlas_pixels -- --ignored`. A machine with
//! no adapter that renders reports that and passes, unless `SKIA_RUST_REQUIRE_ADAPTER` is set.
#![cfg(not(target_arch = "wasm32"))]

use skia_rust_core::color::Color4f;
use skia_rust_core::device::Device as CoreDevice;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::IRect;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_gpu::gpu::backing_fit::BackingFit;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped};
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::device::Device;
use skia_rust_gpu::graphite::graphite_types::{
    InsertRecordingInfo, InsertStatus, SubmitInfo, SyncToCpu,
};
use skia_rust_gpu::graphite::renderer_provider::PathRendererStrategy;
use skia_rust_gpu::graphite::resource_types::LoadOp;
use skia_rust_gpu::graphite::wgpu::{WgpuContext, adapter_backend_context, make_context};

const SIZE: i32 = 128;

/// A context on a real adapter with the raster path atlas strategy, or `None` (after saying so) if
/// the machine has no adapter that renders.
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
    let options = ContextOptions {
        path_renderer_strategy: Some(PathRendererStrategy::RasterAtlas),
        ..ContextOptions::default()
    };
    Some(make_context(&backend_context, &options).expect("a context"))
}

/// A paint of an opaque red, with or without antialiasing.
fn paint(anti_alias: bool) -> Paint {
    let mut paint = Paint::new(Color4f::new(1.0, 0.0, 0.0, 1.0), None);
    paint.set_anti_alias(anti_alias);
    paint.set_style(Style::Fill);
    paint
}

/// A convex hexagon centred in the target.
fn hexagon(fill_type: PathFillType) -> Path {
    let points = [
        Point::new(64.0, 20.0),
        Point::new(104.0, 42.0),
        Point::new(104.0, 86.0),
        Point::new(64.0, 108.0),
        Point::new(24.0, 86.0),
        Point::new(24.0, 42.0),
    ];
    let mut builder = PathBuilder::new_with_fill_type(fill_type);
    builder.move_to(points[0]);
    for point in &points[1..] {
        builder.line_to(*point);
    }
    builder.close();
    builder.detach()
}

/// A rectangle with fractional edges: the column `10` is half covered.
fn fractional_rect() -> Path {
    let mut builder = PathBuilder::new();
    builder.add_rect(
        skia_rust_core::rect::Rect::new(10.5, 20.0, 100.0, 100.0),
        None,
        None,
    );
    builder.detach()
}

/// Records what `draw` draws into a cleared target, runs it and reads the target back.
fn render_with(context: &mut WgpuContext, draw: impl FnOnce(&mut Device)) -> (Vec<u8>, usize) {
    let recorder = context.make_recorder(None);
    // An explicit RGBA8 format, never N32: the readback is compared channel by channel.
    let image_info = ImageInfo::new(
        (SIZE, SIZE),
        skia_rust_core::color_type::ColorType::RGBA8888,
        skia_rust_core::alpha_type::AlphaType::Premul,
        None,
    );
    let mut device = Device::make_with_info(
        Some(&recorder),
        &image_info,
        Budgeted::Yes,
        Mipmapped::No,
        BackingFit::Exact,
        &SurfaceProps::default(),
        LoadOp::Clear,
        "PathAtlasPixels",
        true,
        false,
    )
    .expect("a device");
    draw(&mut device);
    let target = device.target();
    device.flush_pending_work();

    let mut recorder = recorder;
    let mut recording = recorder.snap().expect("a recording");
    let status = context.insert_recording(InsertRecordingInfo::new(&mut recording));
    assert_eq!(status, InsertStatus::Success);
    assert!(context.submit(SubmitInfo::new(SyncToCpu::Yes)));
    drop(device);

    context
        .read_pixels(
            &target,
            image_info.color_info(),
            IRect::from_size((SIZE, SIZE)),
            &image_info,
        )
        .expect("the pixels")
}

fn pixel(pixels: &(Vec<u8>, usize), x: usize, y: usize) -> [u8; 4] {
    let (pixels, row_bytes) = pixels;
    let i = y * row_bytes + x * 4;
    pixels[i..i + 4].try_into().unwrap()
}

const RED: [u8; 4] = [255, 0, 0, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

// A filled path drawn through the raster atlas covers its inside exactly (without antialiasing)
// and nothing outside.
// Port of: src/gpu/graphite/RasterPathAtlas.cpp#L50-L115 (chrome/m156), the atlas draw of a fill
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn a_fill_through_the_raster_atlas_covers_its_inside_only() {
    let Some(mut context) = real_context() else {
        return;
    };
    let pixels = render_with(&mut context, |device| {
        device.draw_path(&hexagon(PathFillType::Winding), &paint(false));
    });
    assert_eq!(pixel(&pixels, 64, 64), RED);
    assert_eq!(pixel(&pixels, 30, 64), RED);
    assert_eq!(pixel(&pixels, 100, 64), RED);
    assert_eq!(pixel(&pixels, 10, 64), CLEAR);
    assert_eq!(pixel(&pixels, 64, 10), CLEAR);
    assert_eq!(pixel(&pixels, 2, 2), CLEAR);
}

// The inverse fill of a path through the raster atlas covers everything outside it.
// Port of: src/gpu/graphite/RasterPathAtlas.cpp#L120-L137 (chrome/m156), the inverse mask
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn an_inverse_fill_through_the_raster_atlas_covers_the_outside() {
    let Some(mut context) = real_context() else {
        return;
    };
    let pixels = render_with(&mut context, |device| {
        device.draw_path(&hexagon(PathFillType::InverseWinding), &paint(false));
    });
    assert_eq!(pixel(&pixels, 64, 64), CLEAR);
    assert_eq!(pixel(&pixels, 2, 2), RED);
    assert_eq!(pixel(&pixels, 120, 120), RED);
}

// An anti-aliased edge through the raster atlas is partly covered: the half-covered column is
// neither clear nor solid, and its colour is red at that alpha.
// Port of: src/gpu/graphite/RasterPathAtlas.cpp#L127-L137 (chrome/m156), the AA mask
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn an_antialiased_edge_through_the_raster_atlas_is_partly_covered() {
    let Some(mut context) = real_context() else {
        return;
    };
    let pixels = render_with(&mut context, |device| {
        device.draw_path(&fractional_rect(), &paint(true));
    });
    // Column 10 is covered for half of its width.
    let [r, g, b, a] = pixel(&pixels, 10, 64);
    assert!(a > 64 && a < 192, "the half-covered column has alpha {a}");
    assert_eq!((r, g, b), (a, 0, 0), "premultiplied red at alpha {a}");
    // Fully covered and fully clear pixels are exact.
    assert_eq!(pixel(&pixels, 50, 64), RED);
    assert_eq!(pixel(&pixels, 2, 64), CLEAR);
}
