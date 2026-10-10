// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! G12b: text drawn into a context on a real adapter, run by the wgpu command buffer and read back
//! with `Context::read_pixels`: direct mask text, distance field text and text drawn as paths,
//! from the glyph atlases.
//!
//! The assertions hold on lavapipe (Linux) and WARP (Windows): they check the shape of the result,
//! not exact antialiased coverage values. The text is red on a clear target, so every pixel is a
//! premultiplied red whose green and blue are 0 and whose red equals its alpha. The noop-adapter
//! checks of the same draws are in `text_gpu.rs`.
//!
//! Like the other real-adapter tests these are `#[ignore]`d, so CI counts them as not run. Run
//! them with `cargo test -p skia-rust-gpu --test text_pixels -- --ignored`. A machine with no
//! adapter that renders reports that and passes, unless `SKIA_RUST_REQUIRE_ADAPTER` is set.
#![cfg(not(target_arch = "wasm32"))]
// Pixel coordinates are far below 2^24.
#![allow(clippy::cast_precision_loss)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::device::{Device as CoreDevice, draw_glyph_run_list};
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::glyph_run::GlyphRunBuilder;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::IRect;
use skia_rust_core::surface_props::{PixelGeometry, SurfaceProps, SurfacePropsFlags};
use skia_rust_core::text_blob::TextBlobBuilder;
use skia_rust_gpu::gpu::backing_fit::BackingFit;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped};
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::device::Device;
use skia_rust_gpu::graphite::graphite_types::{
    InsertRecordingInfo, InsertStatus, SubmitInfo, SyncToCpu,
};
use skia_rust_gpu::graphite::resource_types::LoadOp;
use skia_rust_gpu::graphite::wgpu::{WgpuContext, adapter_backend_context, make_context};
use skia_rust_tools::font_tool_utils::{add_to_text_blob, default_typeface};

const SIZE: i32 = 512;

/// A context on a real adapter, or `None` (after saying so) if the machine has no adapter that
/// renders.
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

fn font(size: f32) -> Font {
    let mut font = Font::from_size(default_typeface(), size);
    font.set_edging(Edging::AntiAlias);
    font
}

fn red_paint() -> Paint {
    let mut paint = Paint::new(Color4f::new(1.0, 0.0, 0.0, 1.0), None);
    paint.set_anti_alias(true);
    paint
}

/// The pixels of the target.
struct Pixels {
    bytes: Vec<u8>,
    row_bytes: usize,
}

impl Pixels {
    fn at(&self, x: usize, y: usize) -> [u8; 4] {
        let i = y * self.row_bytes + x * 4;
        self.bytes[i..i + 4].try_into().unwrap()
    }

    /// The pixels with any coverage.
    fn covered(&self) -> Vec<(usize, usize)> {
        let mut covered = Vec::new();
        for y in 0..SIZE as usize {
            for x in 0..SIZE as usize {
                if self.at(x, y)[3] != 0 {
                    covered.push((x, y));
                }
            }
        }
        covered
    }

    /// The bounds of the pixels with any coverage: `(left, top, right, bottom)` inclusive.
    fn covered_bounds(&self) -> Option<(usize, usize, usize, usize)> {
        let covered = self.covered();
        let first = *covered.first()?;
        let mut bounds = (first.0, first.1, first.0, first.1);
        for (x, y) in covered {
            bounds = (
                bounds.0.min(x),
                bounds.1.min(y),
                bounds.2.max(x),
                bounds.3.max(y),
            );
        }
        Some(bounds)
    }

    /// Every pixel is a premultiplied red: green and blue are 0 and red equals alpha.
    fn assert_all_premultiplied_red(&self) {
        for y in 0..SIZE as usize {
            for x in 0..SIZE as usize {
                let [r, g, b, a] = self.at(x, y);
                assert!(
                    g == 0 && b == 0 && r == a,
                    "pixel ({x}, {y}) is {:?}",
                    [r, g, b, a]
                );
            }
        }
    }

    fn count_with_alpha(&self, predicate: impl Fn(u8) -> bool) -> usize {
        self.covered()
            .into_iter()
            .filter(|&(x, y)| predicate(self.at(x, y)[3]))
            .count()
    }
}

/// Records what `draw` draws into a cleared target, runs it and reads the target back.
fn render_with(
    context: &mut WgpuContext,
    props: &SurfaceProps,
    draw: impl FnOnce(&mut Device),
) -> Pixels {
    let recorder = context.make_recorder(None);
    // An explicit RGBA8 format, never N32: the readback is compared channel by channel.
    let image_info = ImageInfo::new((SIZE, SIZE), ColorType::RGBA8888, AlphaType::Premul, None);
    let mut device = Device::make_with_info(
        Some(&recorder),
        &image_info,
        Budgeted::Yes,
        Mipmapped::No,
        BackingFit::Exact,
        props,
        LoadOp::Clear,
        "TextPixels",
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

    let (bytes, row_bytes) = context
        .read_pixels(
            &target,
            image_info.color_info(),
            IRect::from_size((SIZE, SIZE)),
            &image_info,
        )
        .expect("the pixels");
    // `TEXT_PIXELS_DUMP_DIR` saves the readback as a raw RGBA file for a look at the result.
    if let Some(dir) = std::env::var_os("TEXT_PIXELS_DUMP_DIR") {
        let name = std::thread::current().name().unwrap_or("text").to_owned();
        let path = std::path::Path::new(&dir).join(format!("{name}.rgba"));
        std::fs::create_dir_all(&dir).expect("the dump directory");
        std::fs::write(path, &bytes).expect("the dump file");
    }
    Pixels { bytes, row_bytes }
}

fn draw_text(device: &mut Device, font: &Font, paint: &Paint, text: &str, origin: Point) {
    let mut builder = GlyphRunBuilder::new();
    let list =
        builder.text_to_glyph_run_list(font, paint, text.as_bytes(), origin, TextEncoding::UTF8);
    draw_glyph_run_list(device, &list, paint);
}

// Small text is drawn from the A8 glyph atlas: the pixels it covers are within the bounds of the
// text, some are fully covered, and none are outside it.
// Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L19-L212 (chrome/m156)
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn small_text_is_drawn_from_the_atlas() {
    let Some(mut context) = real_context() else {
        return;
    };
    let font = font(40.0);
    let origin = Point::new(20.0, 100.0);
    let text = "Hamburgefons";
    let pixels = render_with(&mut context, &SurfaceProps::default(), |device| {
        draw_text(device, &font, &red_paint(), text, origin);
    });
    pixels.assert_all_premultiplied_red();

    let (left, top, right, bottom) = pixels.covered_bounds().expect("the text drew");
    let (width, _) = font.measure_text(text.as_bytes(), TextEncoding::UTF8, None);
    // The text starts at the origin, goes up to the ascent and down to the descent.
    assert!(left as f32 >= origin.x - 4.0, "left {left}");
    assert!(right as f32 <= origin.x + width + 4.0, "right {right}");
    assert!(top as f32 >= origin.y - 40.0, "top {top}");
    assert!(bottom as f32 <= origin.y + 20.0, "bottom {bottom}");
    // Enough pixels are covered to be text, and the strokes of the glyphs are solid.
    assert!(pixels.covered().len() > 400);
    assert!(pixels.count_with_alpha(|a| a == 255) > 100);
    // Nothing is drawn where there is no text.
    assert_eq!(pixels.at(5, 5), [0, 0, 0, 0]);
    assert_eq!(
        pixels.at(SIZE as usize - 5, SIZE as usize - 5),
        [0, 0, 0, 0]
    );
}

// The same blob drawn at two whole-pixel positions is drawn from the same atlas entries, and the
// two copies are identical.
// Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L58-L91 (chrome/m156)
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn a_blob_drawn_twice_makes_identical_copies() {
    let Some(mut context) = real_context() else {
        return;
    };
    let font = font(32.0);
    let mut blob_builder = TextBlobBuilder::new();
    add_to_text_blob(&mut blob_builder, "Hamburgefons", &font, 0.0, 0.0);
    let blob = blob_builder.make().expect("a blob");
    let pixels = render_with(&mut context, &SurfaceProps::default(), |device| {
        for origin in [Point::new(20.0, 60.0), Point::new(20.0, 160.0)] {
            let mut builder = GlyphRunBuilder::new();
            let list = builder.blob_to_glyph_run_list(&blob, origin);
            draw_glyph_run_list(device, &list, &red_paint());
        }
    });
    pixels.assert_all_premultiplied_red();
    assert!(pixels.covered().len() > 400);
    // Rows 0..110 are the first copy; the second is 100 rows below it.
    for y in 0..100usize {
        for x in 0..SIZE as usize {
            assert_eq!(pixels.at(x, y), pixels.at(x, y + 100), "({x}, {y})");
        }
    }
}

// Large text is drawn from the distance field in the atlas: the inside of a stem is solid and the
// text has the size asked for.
// Port of: src/gpu/graphite/render/SDFTextRenderStep.cpp#L53-L204 (chrome/m156)
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn large_text_is_drawn_from_distance_fields() {
    let Some(mut context) = real_context() else {
        return;
    };
    let font = font(200.0);
    let origin = Point::new(20.0, 250.0);
    let pixels = render_with(&mut context, &SurfaceProps::default(), |device| {
        draw_text(device, &font, &red_paint(), "Hg", origin);
    });
    pixels.assert_all_premultiplied_red();
    let (left, top, right, bottom) = pixels.covered_bounds().expect("the text drew");
    assert!(left as f32 >= origin.x - 8.0, "left {left}");
    assert!(top as f32 >= origin.y - 220.0, "top {top}");
    assert!(bottom as f32 <= origin.y + 80.0, "bottom {bottom}");
    assert!(right > left + 100 && bottom > top + 100);
    // A large glyph has a large solid interior.
    assert!(pixels.count_with_alpha(|a| a == 255) > 3000);
    assert_eq!(pixels.at(5, 5), [0, 0, 0, 0]);
}

// Text beyond the distance field sizes is drawn as paths and covers pixels like text does.
// Port of: src/text/gpu/SubRunContainer.cpp#L1357-L1659 (chrome/m156), the path case
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn huge_text_is_drawn_as_paths() {
    let Some(mut context) = real_context() else {
        return;
    };
    let font = font(400.0);
    let pixels = render_with(&mut context, &SurfaceProps::default(), |device| {
        draw_text(device, &font, &red_paint(), "H", Point::new(40.0, 420.0));
    });
    pixels.assert_all_premultiplied_red();
    assert!(pixels.count_with_alpha(|a| a == 255) > 8000);
    assert_eq!(pixels.at(5, 5), [0, 0, 0, 0]);
}

// A slug draws the same pixels as the blob it was made of.
// Port of: src/gpu/graphite/Device.cpp#L2573-L2585 (chrome/m156)
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn a_slug_draws_the_same_pixels_as_its_text() {
    let Some(mut context) = real_context() else {
        return;
    };
    let font = font(32.0);
    let origin = Point::new(20.0, 60.0);
    let text = "Hamburgefons";
    let direct = render_with(&mut context, &SurfaceProps::default(), |device| {
        draw_text(device, &font, &red_paint(), text, origin);
    });
    let slugged = render_with(&mut context, &SurfaceProps::default(), |device| {
        let mut builder = GlyphRunBuilder::new();
        let list = builder.text_to_glyph_run_list(
            &font,
            &red_paint(),
            text.as_bytes(),
            origin,
            TextEncoding::UTF8,
        );
        let slug = device
            .convert_glyph_run_list_to_slug(&list, &red_paint())
            .expect("a slug");
        device.draw_slug(&slug, &red_paint());
    });
    assert!(direct.covered().len() > 300);
    assert_eq!(direct.bytes, slugged.bytes);
}

// LCD text on a surface with a pixel geometry covers pixels with per-channel coverage, so the
// colors are not the gray-coverage premultiplied reds of A8 text.
// Port of: src/gpu/graphite/render/BitmapTextRenderStep.cpp#L19-L212 (chrome/m156), the LCD variant
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn lcd_text_covers_pixels_with_per_channel_coverage() {
    let Some(mut context) = real_context() else {
        return;
    };
    let mut lcd_font = font(40.0);
    lcd_font.set_edging(Edging::SubpixelAntiAlias);
    let props = SurfaceProps::new(SurfacePropsFlags::DEFAULT, PixelGeometry::RGBH);
    let pixels = render_with(&mut context, &props, |device| {
        draw_text(
            device,
            &lcd_font,
            &red_paint(),
            "Hamburgefons",
            Point::new(20.0, 100.0),
        );
    });
    assert!(pixels.covered().len() > 400);
    // The red paint only has red, so with LCD coverage no pixel has green or blue.
    for (x, y) in pixels.covered() {
        let [_, g, b, _] = pixels.at(x, y);
        assert_eq!((g, b), (0, 0), "({x}, {y})");
    }
}
