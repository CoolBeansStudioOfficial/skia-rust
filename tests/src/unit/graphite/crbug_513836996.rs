// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/crbug_513836996.cpp (chrome/m156)
//
// The test reads the surface back, so it runs on a real adapter (`def_graphite_adapter_test!`).

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{ImageSetEntry, QuadAAFlags, SrcRectConstraint};
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::RequiredProperties;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::graphite_types::InsertRecordingInfo;
use skia_rust_gpu::graphite::image_factories::texture_from_image;
use skia_rust_gpu::graphite::surface_graphite::Surface;

use crate::{def_graphite_adapter_test, reporter_assert};

const K_W: i32 = 200;
const K_H: i32 = 100;

// Port of: tests/graphite/crbug_513836996.cpp#L23-L89 (chrome/m156)
def_graphite_adapter_test!(crbug_513836996, |reporter, context| {
    if context.supports_protected_content() {
        // Require readback, so exit if not possible
        return;
    }
    let ii = ImageInfo::new(
        (K_W, K_H),
        ColorType::RGBA8888,
        skia_rust_core::alpha_type::AlphaType::Premul,
        None,
    );
    let mut recorder = context.make_recorder(None);
    let Some(surface) =
        Surface::render_target(&recorder, &ii, Mipmapped::No, None, "crbug_513836996")
    else {
        panic!("a render target surface");
    };
    let surface_canvas = surface.canvas();
    surface_canvas.clear(Color::BLACK);

    let src_ii = ImageInfo::new(
        (32, 32),
        ColorType::RGBA8888,
        skia_rust_core::alpha_type::AlphaType::Premul,
        None,
    );
    let mut src_bm = Bitmap::new();
    src_bm.alloc_pixels_info(&src_ii, None);
    src_bm.erase_color(Color::GREEN);
    let raster_image = src_bm.as_image().expect("a raster image");
    let green = texture_from_image(&recorder, &raster_image, RequiredProperties::default())
        .expect("a texture image");

    let dst_clips = [
        Point::new(10.0, 10.0),
        Point::new(60.0, 10.0),
        Point::new(60.0, 60.0),
        Point::new(10.0, 60.0), // LEFT
        Point::new(110.0, 10.0),
        Point::new(190.0, 10.0),
        Point::new(190.0, 90.0),
        Point::new(110.0, 90.0), // RIGHT
    ];

    let entries = [
        ImageSetEntry::new(
            green.clone(),
            Rect::from_ltrb(-100.0, -100.0, -50.0, -50.0),
            Rect::from_ltrb(0.0, 0.0, 200.0, 100.0),
            None,
            1.0,
            QuadAAFlags::NONE,
            true,
        ),
        ImageSetEntry::new(
            green,
            Rect::from_ltrb(0.0, 0.0, 32.0, 32.0),
            Rect::from_ltrb(0.0, 0.0, 200.0, 100.0),
            None,
            1.0,
            QuadAAFlags::NONE,
            true,
        ),
    ];
    surface_canvas.experimental_draw_edge_aa_image_set(
        &entries,
        &dst_clips,
        &[],
        SamplingOptions::default(),
        None,
        SrcRectConstraint::Fast,
    );

    let mut recording = recorder.snap().expect("a recording");
    let _ = context.insert_recording(InsertRecordingInfo::new(&mut recording));

    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(&ii, None);
    let mut pixmap = bm.peek_pixels_mut().expect("pixels");
    let read_ok = context.read_surface_pixels(&surface, &mut pixmap, 0, 0);
    reporter_assert!(reporter, read_ok, "Failed to read pixels");

    let left_px = bm.get_color((35, 35));
    let right_px = bm.get_color((150, 50));
    reporter_assert!(
        reporter,
        left_px == Color::BLACK,
        "Left pixel should be BLACK, got {:?}",
        left_px
    );
    reporter_assert!(
        reporter,
        right_px == Color::GREEN,
        "Right pixel should be GREEN, got {:?}",
        right_px
    );
});
