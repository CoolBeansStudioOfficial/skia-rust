// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Graphite special images and image filters on a real adapter (`docs/design/gpu.md` §5.5,
//! §5.6): a blurred draw (the Graphite image filter backend and the shader blur algorithm), a
//! restored layer (`snapSpecial` and `drawSpecial`), and a surface image drawn after more draws to
//! its surface (the image-to-device link). The noop-adapter checks are in `special_images.rs`.
//!
//! Like the other real-adapter tests these are `#[ignore]`d, so CI counts them as not run. Run
//! them with `cargo test -p skia-rust-gpu --test special_image_pixels -- --ignored`. A machine
//! with no adapter that renders (lavapipe on Linux, WARP on Windows) reports that and passes,
//! unless `SKIA_RUST_REQUIRE_ADAPTER` is set. The targets are `RGBA_8888` (never N32), so the byte
//! order is the same on every host.

#![cfg(not(target_arch = "wasm32"))]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::wgpu::{WgpuContext, adapter_backend_context, make_context};

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

fn rgba(w: i32, h: i32) -> ImageInfo {
    ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Premul, None)
}

fn solid(r: f32, g: f32, b: f32) -> Paint {
    let mut paint = Paint::new(Color4f::new(r, g, b, 1.0), None);
    paint.set_anti_alias(false);
    paint
}

/// Reads `surface` back as RGBA bytes (`[r, g, b, a]` per pixel, rows of `width` pixels).
fn read(context: &mut WgpuContext, surface: &Surface) -> Vec<[u8; 4]> {
    let info = surface.image_info().clone();
    let (w, h) = (info.width(), info.height());
    let row_bytes = usize::try_from(w).unwrap() * 4;
    let mut bytes = vec![0xCD_u8; row_bytes * usize::try_from(h).unwrap()];
    let mut pixmap = Pixmap::new(&info, &mut bytes, row_bytes).unwrap();
    assert!(context.read_surface_pixels(surface, &mut pixmap, 0, 0));
    bytes.as_chunks::<4>().0.to_vec()
}

fn at(pixels: &[[u8; 4]], width: i32, x: i32, y: i32) -> [u8; 4] {
    pixels[usize::try_from(y * width + x).unwrap()]
}

const RED: [u8; 4] = [255, 0, 0, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

// A rect drawn with a blur image filter: the canvas evaluates the filter with the Graphite
// backend, whose blur is the shader blur algorithm. Far inside the rect the color is unchanged,
// far outside it is clear, and around the edge it is a partial, premultiplied red that falls off
// away from the rect.
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn a_blur_image_filter_blurs_on_the_gpu() {
    let Some(mut context) = real_context() else {
        return;
    };
    // sigma 2 is a 2D pass; sigma (3, 0.5) is two 1D passes (13 x 5 samples > 28).
    for (sx, sy) in [(2.0, 2.0), (3.0, 0.5)] {
        let recorder = context.make_recorder(None);
        let surface = Surface::render_target(&recorder, &rgba(64, 64), Mipmapped::No, None, "")
            .expect("a surface");
        let mut paint = solid(1.0, 0.0, 0.0);
        paint.set_image_filter(image_filters::blur(sx, sy, TileMode::Decal, None, None));
        surface
            .canvas()
            .draw_rect(Rect::new(16.0, 16.0, 48.0, 48.0), &paint);
        let pixels = read(&mut context, &surface);

        assert_eq!(at(&pixels, 64, 32, 32), RED, "sigma ({sx}, {sy}) center");
        assert_eq!(at(&pixels, 64, 2, 2), CLEAR, "sigma ({sx}, {sy}) corner");
        assert_eq!(at(&pixels, 64, 32, 2), CLEAR, "sigma ({sx}, {sy}) above");
        // Across the left edge: partial, premultiplied red, decreasing away from the rect.
        let inside = at(&pixels, 64, 16, 32);
        let outside = at(&pixels, 64, 14, 32);
        for p in [inside, outside] {
            assert!(p[3] > 0 && p[3] < 255, "sigma ({sx}, {sy}) edge {p:?}");
            assert_eq!(p[0], p[3], "sigma ({sx}, {sy}) premultiplied red {p:?}");
            assert_eq!((p[1], p[2]), (0, 0));
        }
        assert!(inside[3] > outside[3], "{inside:?} {outside:?}");
    }
}

// A layer drawn with half alpha is restored into its parent through snapSpecial() and
// drawSpecial().
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn a_restored_layer_is_composited_into_its_parent() {
    let Some(mut context) = real_context() else {
        return;
    };
    let recorder = context.make_recorder(None);
    let surface =
        Surface::render_target(&recorder, &rgba(16, 16), Mipmapped::No, None, "").unwrap();
    let canvas = surface.canvas();
    canvas.clear(Color4f::new(0.0, 0.0, 1.0, 1.0));
    canvas.save_layer_alpha(None, 0x80);
    canvas.draw_rect(Rect::new(0.0, 0.0, 8.0, 16.0), &solid(1.0, 0.0, 0.0));
    canvas.restore();
    let pixels = read(&mut context, &surface);
    // Left: red at alpha 0x80 over blue. Right: the blue the layer left alone.
    let left = at(&pixels, 16, 4, 8);
    assert_eq!(left[3], 255);
    assert_eq!(left[1], 0);
    assert!(left[0] >= 0x7F && left[0] <= 0x81, "{left:?}");
    assert!(left[2] >= 0x7E && left[2] <= 0x80, "{left:?}");
    assert_eq!(at(&pixels, 16, 12, 8), [0, 0, 255, 255]);
}

// Q-B: an image of surface A drawn into B sees A's draws up to the moment it is drawn, including
// draws made to A after `as_image()`; a second draw of the image after more draws to A sees those.
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn a_surface_image_sees_draws_made_after_it_was_taken() {
    let Some(mut context) = real_context() else {
        return;
    };
    let recorder = context.make_recorder(None);
    let a = Surface::render_target(&recorder, &rgba(8, 8), Mipmapped::No, None, "").unwrap();
    let b = Surface::render_target(&recorder, &rgba(24, 8), Mipmapped::No, None, "").unwrap();

    a.canvas().clear(Color4f::new(0.0, 0.0, 1.0, 1.0));
    let image_a = a.as_image();
    // Drawn after the image was taken, before it is first used.
    a.canvas()
        .draw_rect(Rect::new(0.0, 0.0, 4.0, 8.0), &solid(0.0, 1.0, 0.0));
    b.canvas().clear(Color4f::new(0.0, 0.0, 0.0, 1.0));
    b.canvas().draw_image(&image_a, (0.0, 0.0), None);
    // More draws to A, then the image again.
    a.canvas().clear(Color4f::new(1.0, 0.0, 0.0, 1.0));
    b.canvas().draw_image(&image_a, (8.0, 0.0), None);

    let pixels = read(&mut context, &b);
    assert_eq!(at(&pixels, 24, 2, 4), [0, 255, 0, 255]);
    assert_eq!(at(&pixels, 24, 6, 4), [0, 0, 255, 255]);
    assert_eq!(at(&pixels, 24, 10, 4), RED);
    assert_eq!(at(&pixels, 24, 14, 4), RED);
    assert_eq!(at(&pixels, 24, 20, 4), [0, 0, 0, 255]);
}
