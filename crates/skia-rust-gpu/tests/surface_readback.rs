// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! G11c wiring: Graphite surfaces and images are read back through the context
//! (`Device::onReadPixels` / `ContextPriv::readPixels`), and a source that needs a draw to be
//! read (`CopyAsDraw` in `asyncReadPixels`) is converted by the GPU.
//!
//! Like `wgpu_first_pixels`, the tests need an adapter that renders and otherwise report that and
//! pass, unless `SKIA_RUST_REQUIRE_ADAPTER` is set.

#![cfg(not(target_arch = "wasm32"))]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::Rect as SkRect;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::graphite_types::{InsertRecordingInfo, InsertStatus};
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::wgpu::{WgpuContext, adapter_backend_context, make_context};

fn real_context() -> Option<WgpuContext> {
    let Some((backend_context, _)) = adapter_backend_context(wgpu::Backends::all()) else {
        assert!(
            std::env::var_os("SKIA_RUST_REQUIRE_ADAPTER").is_none(),
            "SKIA_RUST_REQUIRE_ADAPTER is set but there is no adapter"
        );
        eprintln!("no adapter that renders: skipping");
        return None;
    };
    Some(make_context(&backend_context, &ContextOptions::default()).expect("a context"))
}

fn solid(r: f32, g: f32, b: f32) -> Paint {
    let mut paint = Paint::new(Color4f::new(r, g, b, 1.0), None);
    paint.set_anti_alias(false);
    paint
}

// The bytes of an N32 pixel of the given channels (the native byte order of the host).
fn n32(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    if ColorType::N32 == ColorType::BGRA8888 {
        [b, g, r, a]
    } else {
        [r, g, b, a]
    }
}

// A half float to f32.
fn half_to_f32(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = i32::from((bits >> 10) & 0x1F);
    let mantissa = f32::from(bits & 0x3FF);
    match exponent {
        0 => sign * mantissa * 2f32.powi(-24),
        31 => sign * f32::INFINITY,
        _ => sign * (1.0 + mantissa / 1024.0) * 2f32.powi(exponent - 15),
    }
}

#[test]
fn a_surface_reads_back_its_draws_through_the_context() {
    let Some(mut context) = real_context() else {
        return;
    };
    let recorder = context.make_recorder(None);
    let info = ImageInfo::new_n32_premul((8, 4), None);
    let surface = Surface::render_target(&recorder, &info, Mipmapped::No, None, "").unwrap();
    surface
        .canvas()
        .draw_rect(SkRect::new(0.0, 0.0, 4.0, 4.0), &solid(1.0, 0.0, 0.0));

    let mut pixels = vec![0xCD_u8; 8 * 4 * 4];
    let mut pixmap = Pixmap::new(&info, &mut pixels, 8 * 4).unwrap();
    assert!(context.read_surface_pixels(&surface, &mut pixmap, 0, 0));
    for y in 0..4 {
        for x in 0..8 {
            let i = (y * 8 + x) * 4;
            let expected = if x < 4 {
                n32(255, 0, 0, 255)
            } else {
                [0, 0, 0, 0]
            };
            assert_eq!(pixels[i..i + 4], expected, "({x}, {y})");
        }
    }
}

#[test]
fn a_surface_reads_back_a_subrect_at_an_offset() {
    let Some(mut context) = real_context() else {
        return;
    };
    let recorder = context.make_recorder(None);
    let info = ImageInfo::new_n32_premul((8, 4), None);
    let surface = Surface::render_target(&recorder, &info, Mipmapped::No, None, "").unwrap();
    surface
        .canvas()
        .draw_rect(SkRect::new(4.0, 0.0, 8.0, 4.0), &solid(0.0, 1.0, 0.0));

    let sub_info = ImageInfo::new_n32_premul((2, 2), None);
    let mut pixels = vec![0_u8; 2 * 2 * 4];
    let mut pixmap = Pixmap::new(&sub_info, &mut pixels, 2 * 4).unwrap();
    assert!(context.read_surface_pixels(&surface, &mut pixmap, 3, 1));
    let green = n32(0, 255, 0, 255);
    assert_eq!(pixels[0..4], [0, 0, 0, 0]);
    assert_eq!(pixels[4..8], green);
    assert_eq!(pixels[8..12], [0, 0, 0, 0]);
    assert_eq!(pixels[12..16], green);
}

#[test]
fn an_image_of_a_flushed_surface_reads_back() {
    let Some(mut context) = real_context() else {
        return;
    };
    let mut recorder = context.make_recorder(None);
    let info = ImageInfo::new_n32_premul((4, 4), None);
    let surface = Surface::render_target(&recorder, &info, Mipmapped::No, None, "").unwrap();
    surface
        .canvas()
        .draw_rect(SkRect::new(0.0, 0.0, 4.0, 4.0), &solid(0.0, 0.0, 1.0));
    let image = surface.as_image();

    // Surface flush, then insert and submit the recording, then read the image.
    surface.flush();
    let mut recording = recorder.snap().expect("a recording");
    assert_eq!(
        context.insert_recording(InsertRecordingInfo::new(&mut recording)),
        InsertStatus::Success
    );
    let mut pixels = vec![0_u8; 4 * 4 * 4];
    let mut pixmap = Pixmap::new(&info, &mut pixels, 4 * 4).unwrap();
    assert!(context.read_image_pixels(&image, &mut pixmap, 0, 0));
    for px in pixels.chunks(4) {
        assert_eq!(px, n32(0, 0, 255, 255));
    }
}

#[test]
fn a_read_that_needs_a_transfer_function_is_converted_by_a_draw() {
    let Some(mut context) = real_context() else {
        return;
    };
    let recorder = context.make_recorder(None);
    let info = ImageInfo::new(
        (4, 4),
        ColorType::N32,
        AlphaType::Premul,
        Some(ColorSpace::new_srgb()),
    );
    let surface = Surface::render_target(&recorder, &info, Mipmapped::No, None, "").unwrap();
    surface
        .canvas()
        .draw_rect(SkRect::new(0.0, 0.0, 4.0, 4.0), &solid(0.5, 0.5, 0.5));

    // A linear destination: the sRGB-encoded 0.5 is about 0.214, which `CopyAsDraw` computes on
    // the GPU (the transfer function is not the identity).
    let linear = ImageInfo::new(
        (4, 4),
        ColorType::RGBAF16,
        AlphaType::Premul,
        Some(ColorSpace::new_srgb_linear()),
    );
    let mut pixels = vec![0_u8; 4 * 4 * 8];
    let mut pixmap = Pixmap::new(&linear, &mut pixels, 4 * 8).unwrap();
    assert!(context.read_surface_pixels(&surface, &mut pixmap, 0, 0));
    let channel = |i: usize| half_to_f32(u16::from_le_bytes([pixels[i], pixels[i + 1]]));
    let red = channel(0);
    assert!(
        (0.21..0.22).contains(&red),
        "linear red {red} is not about 0.214"
    );
    assert_eq!(channel(6), 1.0);
}
