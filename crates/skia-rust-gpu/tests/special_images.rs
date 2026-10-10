// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Graphite special images, the image filter backend and image-to-device links
//! (`docs/design/gpu.md` §5.5, §5.6) over wgpu's noop adapter: what the devices snap and record,
//! and which tasks reach the recorder's root task list. The pixel checks are in
//! `special_image_pixels.rs`.

#![cfg(not(target_arch = "wasm32"))]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::device::Device as CoreDevice;
use skia_rust_core::image::RequiredProperties;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::size::Size;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_gpu::gpu::backing_fit::BackingFit;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped};
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::device::Device;
use skia_rust_gpu::graphite::image_factories::texture_from_image;
use skia_rust_gpu::graphite::image_graphite::Image as GraphiteImage;
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::resource_types::LoadOp;
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::wgpu::{WgpuContext, make_context, noop_backend_context};

fn context() -> WgpuContext {
    let backend = noop_backend_context();
    make_context(&backend, &ContextOptions::default()).expect("a context on the noop device")
}

fn info(w: i32, h: i32) -> ImageInfo {
    ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Premul, None)
}

fn make_device(recorder: &Recorder, w: i32, h: i32) -> Device {
    Device::make_with_info(
        Some(recorder),
        &info(w, h),
        Budgeted::Yes,
        Mipmapped::No,
        BackingFit::Exact,
        &SurfaceProps::default(),
        LoadOp::Clear,
        "SpecialImagesTest",
        true,
        false,
    )
    .expect("a device on the noop context")
}

fn solid() -> Paint {
    let mut paint = Paint::new(Color4f::new(1.0, 0.0, 0.0, 1.0), None);
    paint.set_anti_alias(false);
    paint
}

// snapSpecial() without a copy flushes the device and wraps its target: the special image is
// texture backed, over the requested subset of the whole target.
#[test]
fn a_snapped_special_image_wraps_the_device_target() {
    let context = context();
    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder, 32, 32);
    device.draw_rect(&Rect::new(0.0, 0.0, 16.0, 16.0), &solid());
    assert!(device.testing_only_pending_render_steps() > 0);
    let roots = recorder.priv_().num_root_tasks();

    let special = device
        .snap_special(&IRect::new(4, 4, 20, 20), false)
        .expect("a special image");
    assert!(special.is_texture_backed());
    assert_eq!(special.subset(), IRect::new(4, 4, 20, 20));
    assert_eq!(special.backing_store_dimensions(), (32, 32).into());
    assert!(special.as_bitmap().is_none());
    let image = special.as_image().expect("the image of a special image");
    assert!(image.as_base().is_graphite_backed());
    // The pending draws went to the root task list.
    assert_eq!(device.testing_only_pending_render_steps(), 0);
    assert_eq!(recorder.priv_().num_root_tasks(), roots + 1);

    // A subset of the special image shares the texture.
    let sub = special
        .make_subset(&IRect::new(2, 2, 6, 6))
        .expect("a subset");
    assert_eq!(sub.subset(), IRect::new(6, 6, 10, 10));
    assert!(sub.is_texture_backed());
    assert_eq!(sub.make_pixel_outset().subset(), IRect::new(5, 5, 11, 11));
}

// snapSpecial(forceCopy) copies the subset into a new texture, whose subset starts at the origin.
#[test]
fn a_forced_copy_snaps_a_subset_at_the_origin() {
    let context = context();
    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder, 32, 32);
    device.draw_rect(&Rect::new(0.0, 0.0, 16.0, 16.0), &solid());
    let special = device
        .snap_special(&IRect::new(4, 4, 20, 24), true)
        .expect("a copied special image");
    assert!(special.is_texture_backed());
    assert_eq!(special.subset(), IRect::new(0, 0, 16, 20));
}

// drawSpecial() draws the special image as an image rect with the given transform.
#[test]
fn draw_special_records_a_draw() {
    let context = context();
    let recorder = context.make_recorder(None);
    let mut src = make_device(&recorder, 16, 16);
    src.draw_rect(&Rect::new(0.0, 0.0, 16.0, 16.0), &solid());
    let special = src.snap_special_all().expect("a special image");

    let mut dst = make_device(&recorder, 32, 32);
    assert_eq!(dst.testing_only_pending_render_steps(), 0);
    dst.draw_special(
        &special,
        &Matrix::translate((8.0, 8.0)),
        &SamplingOptions::default(),
        &Paint::default(),
        SrcRectConstraint::Fast,
    );
    assert!(dst.testing_only_pending_render_steps() > 0);
}

// A device of the recorder has an image filtering backend whose devices are Graphite devices and
// whose blur is the shader blur algorithm (a texture-backed result of the requested size).
#[test]
fn the_image_filter_backend_blurs_on_the_gpu() {
    let context = context();
    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder, 32, 32);
    device.draw_rect(&Rect::new(8.0, 8.0, 24.0, 24.0), &solid());
    let special = device.snap_special_all().expect("a special image");

    let backend = device
        .create_image_filtering_backend(&SurfaceProps::default(), ColorType::RGBA8888)
        .expect("a Graphite backend");
    assert_eq!(backend.color_type(), ColorType::RGBA8888);
    let made = backend
        .make_device((10, 12).into(), None, None)
        .expect("a backend device");
    assert_eq!(made.state().width(), 10);

    let engine = backend.blur_engine().expect("a blur engine");
    for sigma in [
        Size::new(2.0, 2.0),
        Size::new(0.0, 3.0),
        Size::new(4.0, 1.0),
    ] {
        let algorithm = engine
            .find_algorithm(sigma, ColorType::RGBA8888)
            .expect("the shader blur handles every color type");
        assert!(!algorithm.supports_only_decal_tiling());
        assert_eq!(algorithm.max_sigma().to_bits(), 4.0f32.to_bits());
        let blurred = algorithm
            .blur(
                sigma,
                &special,
                IRect::new(0, 0, 32, 32),
                TileMode::Decal,
                IRect::new(-4, -4, 36, 36),
            )
            .expect("a blurred image");
        assert!(blurred.is_texture_backed());
        assert_eq!(blurred.dimensions(), (40, 40).into());
    }

    // `MakeGraphite` converts a raster image with the recorder's image provider; the default one
    // makes nothing (as in Skia), so only an uploaded image becomes a special image.
    let mut bitmap = skia_rust_core::bitmap::Bitmap::new();
    assert!(bitmap.try_alloc_pixels_info(&info(4, 4), None));
    bitmap.erase_color_4f(Color4f::new(0.0, 1.0, 0.0, 1.0));
    bitmap.set_immutable();
    let raster = bitmap.as_image().expect("a raster image");
    assert!(
        backend
            .make_image(&IRect::new(1, 1, 3, 3), &raster)
            .is_none()
    );
    let uploaded = texture_from_image(&recorder, &raster, RequiredProperties::default())
        .expect("an uploaded image");
    let converted = backend
        .make_image(&IRect::new(1, 1, 3, 3), &uploaded)
        .expect("a special image of a Graphite image");
    assert!(converted.is_texture_backed());
    let cached = backend
        .get_cached_bitmap(&bitmap)
        .expect("a cached texture");
    assert!(cached.as_base().is_graphite_backed());
}

// Q-B: an image taken from a surface is linked to the surface's device. Drawing the image flushes
// the surface's pending draws, including draws made after `as_image()` (Skia's
// `NotifyInUseTestAsImage` task order: A1, B1, A2, B2).
#[test]
fn a_surface_image_flushes_later_draws_when_it_is_drawn() {
    let context = context();
    let recorder = context.make_recorder(None);
    let a = Surface::render_target(&recorder, &info(10, 10), Mipmapped::No, None, "").unwrap();
    let b = Surface::render_target(&recorder, &info(20, 10), Mipmapped::No, None, "").unwrap();
    let roots = || recorder.priv_().num_root_tasks();

    a.canvas().clear(Color4f::new(0.0, 0.0, 1.0, 1.0));
    let image_a = a.as_image();
    let gi = GraphiteImage::from_core(&image_a).expect("a Graphite image");
    assert!(gi.has_linked_devices());
    // as_image() does not flush.
    assert_eq!(roots(), 0);

    b.canvas().clear(Color4f::new(0.0, 0.0, 0.0, 1.0));
    b.canvas().draw_image(&image_a, (0.0, 0.0), None);
    // A1 was flushed before B's draw.
    assert_eq!(roots(), 1);

    a.canvas().clear(Color4f::new(1.0, 0.0, 0.0, 1.0));
    b.canvas().draw_image(&image_a, (10.0, 0.0), None);
    // B1 (which read A1) and then A2 were flushed before B2: A has a dependent (B).
    assert_eq!(roots(), 3);

    // Dropping the surface makes its device immutable: the image unlinks on its next use.
    drop(a);
    b.canvas().draw_image(&image_a, (0.0, 0.0), None);
    assert!(!gi.has_linked_devices());
}

// A surface that draws its own image flushes itself first (the image samples the earlier draws).
#[test]
fn a_surface_drawing_its_own_image_flushes_itself() {
    let context = context();
    let recorder = context.make_recorder(None);
    let a = Surface::render_target(&recorder, &info(16, 16), Mipmapped::No, None, "").unwrap();
    a.canvas()
        .draw_rect(Rect::new(0.0, 0.0, 8.0, 8.0), &solid());
    let image_a = a.as_image();
    assert_eq!(recorder.priv_().num_root_tasks(), 0);
    a.canvas().draw_image(&image_a, (8.0, 8.0), None);
    assert_eq!(recorder.priv_().num_root_tasks(), 1);
}

// A layer is restored through snapSpecial() and drawSpecial() (drawDevice): the layer's draws
// reach the root task list and the parent device records the layer image draw.
#[test]
fn a_restored_layer_is_drawn_into_its_parent() {
    let context = context();
    let recorder = context.make_recorder(None);
    let a = Surface::render_target(&recorder, &info(16, 16), Mipmapped::No, None, "").unwrap();
    a.canvas().save_layer_alpha(None, 128);
    a.canvas()
        .draw_rect(Rect::new(0.0, 0.0, 8.0, 8.0), &solid());
    a.canvas().restore();
    // The layer's draw task.
    assert_eq!(recorder.priv_().num_root_tasks(), 1);
    a.flush();
    // The parent's draw task, which samples the layer.
    assert_eq!(recorder.priv_().num_root_tasks(), 2);
}
