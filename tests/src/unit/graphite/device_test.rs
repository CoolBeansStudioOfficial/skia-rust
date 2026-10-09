// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/DeviceTest.cpp (chrome/m156)

#![cfg(test)]
// Mirrors the C++ test, which declares constants inline and converts integer coordinates.
#![allow(clippy::items_after_statements)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::vertices::{VertexMode, Vertices};
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::surface_graphite::Surface;

use crate::{def_graphite_adapter_test, errorf, reporter_assert};

// Tests that vertex transparency will affect draw order.
// Port of: tests/graphite/DeviceTest.cpp#L20-L82 (chrome/m156)
def_graphite_adapter_test!(DeviceTestVertexTransparency, |reporter, context| {
    // Set up transparent vertices, in a 5px wide by 10px tall rectangle.
    const K_VERTEX_COUNT: usize = 5;
    let positions = [
        Point::new(2.5, 5.0),
        Point::new(0.0, 0.0),
        Point::new(5.0, 0.0),
        Point::new(5.0, 10.0),
        Point::new(0.0, 10.0),
    ];

    const K_INDICES: [u16; 6] = [0, 1, 2, 3, 4, 1];

    let colors = [Color::from(0x7F00_FF00_u32); K_VERTEX_COUNT];

    let v = Vertices::new_copy(
        VertexMode::TriangleFan,
        &positions,
        None,
        Some(&colors),
        Some(&K_INDICES),
    )
    .expect("vertices");

    // Draw vertices at x = 0.
    let recorder = context.make_recorder(None);
    let ii = ImageInfo::new((10, 10), ColorType::RGBA8888, AlphaType::Premul, None);
    let Some(surface) = Surface::render_target(&recorder, &ii, Mipmapped::No, None, "") else {
        errorf!(reporter, "Failed to create surface");
        return;
    };
    let canvas = surface.canvas();
    canvas.draw_vertices(&v, BlendMode::Dst, &Paint::default());

    // Draw a square that will overlap both vertex draws.
    let mut red_paint = Paint::default();
    red_paint.set_color(Color::RED);
    canvas.draw_rect(Rect::new(0.0, 0.0, 10.0, 10.0), &red_paint);

    // Draw vertices at x = 5.
    canvas.translate((5.0, 0.0));
    canvas.draw_vertices(&v, BlendMode::Dst, &Paint::default());

    // Read pixels.
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&ii, None);
    let Some(mut pixmap) = bitmap.peek_pixels_mut() else {
        errorf!(reporter, "peekPixels failed");
        return;
    };
    if !context.read_surface_pixels(&surface, &mut pixmap, 0, 0) {
        errorf!(reporter, "readPixels failed");
        return;
    }

    // Check that draws weren't reordered to put vertex draws together.
    // The second vertex draw should have been 50% green on top of red.
    let color = pixmap.get_color((9, 5));
    let expected = Color::from(0xFF80_7F00_u32);
    reporter_assert!(
        reporter,
        color == expected,
        "Wrong color, expected {:08x}, found {:08x}",
        u32::from(expected),
        u32::from(color)
    );
});
