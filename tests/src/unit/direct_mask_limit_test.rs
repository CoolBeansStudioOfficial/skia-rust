// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/DirectMaskLimitTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::graphite_types::{
    InsertRecordingInfo, InsertStatus, SubmitInfo, SyncToCpu,
};
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::wgpu::WgpuContext;
use skia_rust_text::utils::custom_typeface::CustomTypefaceBuilder;

use crate::{def_graphite_adapter_test_with_options, reporter_assert};

// Port of: tests/DirectMaskLimitTest.cpp#L33-L41 (chrome/m156)
fn disable_sdft_options(options: &mut ContextOptions) {
    options.min_distance_field_font_size = 384.0;
    options.glyphs_as_paths_font_size = 384.0;
    options.support_bilerp_from_glyph_atlas = true;
    options.max_texture_size_override = 1024;
}

// Port of: tests/DirectMaskLimitTest.cpp#L43-L87 (chrome/m156)
fn test_direct_mask_limit(
    reporter: &mut crate::Reporter,
    surface: &Surface,
    context: &mut WgpuContext,
) {
    let canvas = surface.canvas();
    canvas.clear(Color::BLACK);

    let mut builder = CustomTypefaceBuilder::new();
    let path = Path::rect(Rect::new(0.0, -1.0, 1.0, 0.0), None);
    builder.set_glyph(1, 1.0, &path);

    let mut font = Font::from_size(builder.detach().expect("a typeface"), 253.0);
    font.set_edging(Edging::SubpixelAntiAlias);

    let mut paint = Paint::default();
    paint.set_color(Color::WHITE);

    let glyph = 1;
    let position = Point::new(0.0, 0.0);
    canvas.draw_glyphs_at(
        &[glyph],
        [position].as_slice(),
        Point::new(100.0, 300.0),
        &font,
        &paint,
    );

    // The submit of the C++ test: snap the recorder, insert the recording and submit.
    let mut recorder = surface.recorder().expect("the surface has a recorder");
    let mut recording = recorder.snap().expect("a recording");
    reporter_assert!(
        reporter,
        context.insert_recording(InsertRecordingInfo::new(&mut recording)) == InsertStatus::Success
    );
    reporter_assert!(reporter, context.submit(SubmitInfo::new(SyncToCpu::Yes)));

    let mut bitmap = Bitmap::new();
    bitmap.alloc_n32_pixels((512, 512), None);
    let read = {
        let Some(mut pixmap) = bitmap.peek_pixels_mut() else {
            reporter_assert!(reporter, false, "peekPixels failed");
            return;
        };
        context.read_surface_pixels(surface, &mut pixmap, 0, 0)
    };
    reporter_assert!(reporter, read);

    let mut has_white_pixel = false;
    'rows: for y in 0..512 {
        for x in 0..512 {
            if bitmap.get_color((x, y)) != Color::BLACK {
                has_white_pixel = true;
                break 'rows;
            }
        }
    }
    reporter_assert!(
        reporter,
        has_white_pixel,
        "Draw failed (all pixels are black)"
    );
}

// Port of: tests/DirectMaskLimitTest.cpp#L109-L135 (chrome/m156)
def_graphite_adapter_test_with_options!(
    DirectMaskLimitTest_Graphite,
    |options| {
        disable_sdft_options(options);
    },
    |reporter, context| {
        let recorder = context.make_recorder(None);
        let ii = ImageInfo::new_n32_premul((512, 512), None);
        let Some(surface) = Surface::render_target(&recorder, &ii, Mipmapped::No, None, "") else {
            return;
        };
        test_direct_mask_limit(reporter, &surface, context);
    }
);
