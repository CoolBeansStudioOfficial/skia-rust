// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/RecordingOrderTest.cpp (chrome/m156)
//
// The test reads the surface back, so it runs on a real adapter (`def_graphite_adapter_test!`).

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::font::Font;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::graphite_types::{InsertRecordingInfo, SubmitInfo, SyncToCpu};
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::wgpu::WgpuContext;
use skia_rust_tools::font_tool_utils::create_portable_typeface;

use crate::tools::compare_pixels::compare_pixels;
use crate::{Reporter, def_graphite_adapter_test, errorf};

const K_SURFACE_SIZE: (i32, i32) = (64, 64);
const K_BACKGROUND_COLOR: Color = Color::WHITE;

// Port of: tests/graphite/RecordingOrderTest.cpp#L19-L125 (chrome/m156)
fn run_test(reporter: &mut Reporter, context: &mut WgpuContext, recorder: &mut Recorder) -> bool {
    let ii = ImageInfo::new(
        K_SURFACE_SIZE,
        ColorType::RGBA8888,
        skia_rust_core::alpha_type::AlphaType::Premul,
        None,
    );
    let mut result0 = Bitmap::new();
    let mut result1 = Bitmap::new();
    result0.alloc_pixels_info(&ii, None);
    result1.alloc_pixels_info(&ii, None);

    let Some(surface) = Surface::render_target(recorder, &ii, Mipmapped::No, None, "") else {
        errorf!(reporter, "Surface creation failed");
        return false;
    };
    let canvas = surface.canvas();

    // Set up a recording to clear the surface
    canvas.clear(K_BACKGROUND_COLOR);
    let Some(mut clear_recording) = recorder.snap() else {
        errorf!(reporter, "Recording creation failed");
        return false;
    };

    // Draw some text and get recording
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let mut font = Font::new(
        create_portable_typeface(Some("serif"), FontStyle::default()),
        12.0,
        1.0,
        0.0,
    );
    font.set_subpixel(true);
    let text0 = "Hamburge";
    canvas.draw_simple_text(
        text0,
        TextEncoding::UTF8,
        Point::new(3.0, 20.0),
        &font,
        &paint,
    );
    let Some(mut text0_recording) = recorder.snap() else {
        errorf!(reporter, "Recording creation failed");
        return false;
    };

    // Draw some more text and get recording
    let text1 = "burgefons";
    canvas.draw_simple_text(
        text1,
        TextEncoding::UTF8,
        Point::new(3.0, 40.0),
        &font,
        &paint,
    );
    let Some(mut text1_recording) = recorder.snap() else {
        errorf!(reporter, "Recording creation failed");
        return false;
    };

    // Playback 0, then 1, and read pixels
    let _ = context.insert_recording(InsertRecordingInfo::new(&mut clear_recording));
    let _ = context.insert_recording(InsertRecordingInfo::new(&mut text0_recording));
    let _ = context.insert_recording(InsertRecordingInfo::new(&mut text1_recording));
    let _ = context.submit(SubmitInfo::new(SyncToCpu::No));
    {
        let mut pm0 = result0.peek_pixels_mut().expect("pixels");
        if !context.read_surface_pixels(&surface, &mut pm0, 0, 0) {
            errorf!(reporter, "readPixels failed");
            return false;
        }
    }

    // Playback 1, then 0, and read pixels
    let _ = context.insert_recording(InsertRecordingInfo::new(&mut clear_recording));
    let _ = context.insert_recording(InsertRecordingInfo::new(&mut text1_recording));
    let _ = context.insert_recording(InsertRecordingInfo::new(&mut text0_recording));
    let _ = context.submit(SubmitInfo::new(SyncToCpu::No));
    {
        let mut pm1 = result1.peek_pixels_mut().expect("pixels");
        if !context.read_surface_pixels(&surface, &mut pm1, 0, 0) {
            errorf!(reporter, "readPixels failed");
            return false;
        }
    }

    // Compare and contrast
    let tol = 1.0f32 / 256.0;
    let tols = [tol, tol, tol, tol];
    let pm0 = result0.peek_pixels().expect("pixels");
    let pm1 = result1.peek_pixels().expect("pixels");
    let mut error = |x: i32, y: i32, diffs: &[f32; 4]| {
        // SkASSERT(x >= 0 && y >= 0);
        errorf!(
            reporter,
            "Error at {}, {}. Diff in floats: ({:.6}, {:.6}, {:.6}, {:.6})",
            x,
            y,
            diffs[0],
            diffs[1],
            diffs[2],
            diffs[3]
        );
    };
    compare_pixels(&pm0, &pm1, &tols, &mut error);
    true
}

// This test captures two recordings A and B, plays them back as A then B, and B then A,
// and verifies that the result is the same.
// Port of: tests/graphite/RecordingOrderTest.cpp#L127-L132 (chrome/m156)
def_graphite_adapter_test!(RecordingOrderTest_Graphite, |reporter, context| {
    let mut recorder = context.make_recorder(None);
    let _ = run_test(reporter, context, &mut recorder);
});
