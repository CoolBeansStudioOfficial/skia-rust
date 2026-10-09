// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/MultisampleTest.cpp (chrome/m156)

#![cfg(test)]
// Mirrors the C++ test, which declares constants inline and converts integer coordinates.
#![allow(clippy::items_after_statements, clippy::cast_precision_loss)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::graphite_types::{InsertRecordingInfo, InsertStatus};
use skia_rust_gpu::graphite::surface_graphite::Surface;

use crate::{def_graphite_adapter_test, errorf, reporter_assert};

const K_RED: Color4f = Color4f::new(1.0, 0.0, 0.0, 1.0);
const K_BLUE: Color4f = Color4f::new(0.0, 0.0, 1.0, 1.0);

// Tests that a drawing with MSAA will have contents retained between recordings.
// This is for testing MSAA load from resolve feature.
// TODO(b/296420752): enable in CTS after debugging failure at coordinates 32,30.
// Port of: tests/graphite/MultisampleTest.cpp#L26-L90 (chrome/m156)
def_graphite_adapter_test!(MultisampleRetainTest, |reporter, context| {
    let surface_image_info = ImageInfo::new((33, 33), ColorType::RGBA8888, AlphaType::Premul, None);

    let mut surface_recorder = context.make_recorder(None);
    let Some(surface) = Surface::render_target(
        &surface_recorder,
        &surface_image_info,
        Mipmapped::No,
        None,
        "",
    ) else {
        errorf!(reporter, "Failed to create surface");
        return;
    };

    // Clear entire surface to red
    let surface_canvas = surface.canvas();
    surface_canvas.clear(K_RED);
    let mut surface_recording = surface_recorder.snap().expect("a recording");
    // Flush the clearing
    let _ = context.insert_recording(InsertRecordingInfo::new(&mut surface_recording));

    // Draw a blue path. The old red background should be retained between recordings.
    let mut paint = Paint::default();
    paint.set_stroke_width(3.0);
    paint.set_color4f(K_BLUE, None);
    paint.set_style(Style::Stroke);

    const K_PATH_POINTS: [[i32; 2]; 4] = [[9, 8], [9, 12], [14, 16], [10, 32]];

    let mut builder = PathBuilder::new();
    builder.move_to((K_PATH_POINTS[0][0] as f32, K_PATH_POINTS[0][1] as f32));
    for point in &K_PATH_POINTS {
        builder.line_to((point[0] as f32, point[1] as f32));
    }
    let path = builder.detach();

    surface_canvas.draw_path(&path, &paint);

    let mut surface_recording2 = surface_recorder.snap().expect("a recording");
    // Play back recording.
    let status = context.insert_recording(InsertRecordingInfo::new(&mut surface_recording2));
    reporter_assert!(reporter, status == InsertStatus::Success);

    // Read pixels.
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&surface_image_info, None);
    let mut pixmap = bitmap.peek_pixels_mut().expect("pixels");
    if !context.read_surface_pixels(&surface, &mut pixmap, 0, 0) {
        errorf!(reporter, "readPixels failed");
        return;
    }

    // Verify recording was replayed.
    reporter_assert!(reporter, pixmap.get_color_4f((16, 0)) == K_RED);
    reporter_assert!(reporter, pixmap.get_color_4f((0, 16)) == K_RED);
    reporter_assert!(reporter, pixmap.get_color_4f((32, 30)) == K_RED);

    // Verify points on the path have blue color. We don't verify last point because it is on the
    // edge of the path thus might have blurry color.
    for point in &K_PATH_POINTS[..K_PATH_POINTS.len() - 1] {
        reporter_assert!(
            reporter,
            pixmap.get_color_4f((point[0], point[1])) == K_BLUE
        );
    }
});

// Tests that multisampled rendering with LoadOp::Clear in one pass and LoadOp::Load in another pass
// works. With the Vulkan backend in particular, without
// VK_EXT_multisampled_render_to_single_sampled, the render pass with LoadOp::Load has an extra
// "unresolve" pass at the start.
// Port of: tests/graphite/MultisampleTest.cpp#L92-L160 (chrome/m156)
def_graphite_adapter_test!(MultisampleClearThenLoad, |reporter, context| {
    let surface_image_info = ImageInfo::new((33, 33), ColorType::RGBA8888, AlphaType::Premul, None);

    let mut surface_recorder = context.make_recorder(None);
    let Some(surface) = Surface::render_target(
        &surface_recorder,
        &surface_image_info,
        Mipmapped::No,
        None,
        "",
    ) else {
        errorf!(reporter, "Failed to create surface");
        return;
    };

    // Clear entire surface to red
    let surface_canvas = surface.canvas();
    surface_canvas.clear(K_RED);

    // Draw a blue path over it. This render pass will use LoadOp::Clear because of the clear above.
    let mut paint = Paint::default();
    paint.set_stroke_width(3.0);
    paint.set_color4f(K_BLUE, None);
    paint.set_style(Style::Stroke);
    paint.set_anti_alias(true);

    // Note: The path is not a line, since that's optimized not to take the multisampling path.
    let path = PathBuilder::new()
        .move_to((0.0, 0.0))
        .line_to((15.0, 15.0))
        .line_to((33.0, 33.0))
        .detach();

    surface_canvas.draw_path(&path, &paint);

    let mut surface_recording = surface_recorder.snap().expect("a recording");
    // Play back recording. This breaks the render pass.
    let _ = context.insert_recording(InsertRecordingInfo::new(&mut surface_recording));

    // Draw another path. Because the contents of the surface need to be retained, this render pass
    // will use LoadOp::Load.
    let path2 = PathBuilder::new()
        .move_to((33.0, 0.0))
        .line_to((15.0, 15.0))
        .line_to((0.0, 33.0))
        .detach();

    surface_canvas.draw_path(&path2, &paint);

    let mut surface_recording2 = surface_recorder.snap().expect("a recording");
    // Play back recording. Break the render pass again.
    let _ = context.insert_recording(InsertRecordingInfo::new(&mut surface_recording2));

    // Verify results
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&surface_image_info, None);
    let mut pixmap = bitmap.peek_pixels_mut().expect("pixels");
    if !context.read_surface_pixels(&surface, &mut pixmap, 0, 0) {
        errorf!(reporter, "readPixels failed");
        return;
    }

    // Some points outside the paths
    reporter_assert!(reporter, pixmap.get_color_4f((1, 16)) == K_RED);
    reporter_assert!(reporter, pixmap.get_color_4f((16, 1)) == K_RED);
    reporter_assert!(reporter, pixmap.get_color_4f((31, 16)) == K_RED);
    reporter_assert!(reporter, pixmap.get_color_4f((16, 31)) == K_RED);

    // Some points on the paths
    reporter_assert!(reporter, pixmap.get_color_4f((1, 1)) == K_BLUE);
    reporter_assert!(reporter, pixmap.get_color_4f((31, 1)) == K_BLUE);
    reporter_assert!(reporter, pixmap.get_color_4f((1, 31)) == K_BLUE);
    reporter_assert!(reporter, pixmap.get_color_4f((31, 31)) == K_BLUE);
    reporter_assert!(reporter, pixmap.get_color_4f((16, 16)) == K_BLUE);
});
