// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/RecordingSurfacesTest.cpp (chrome/m156)
//
// The tests read the surface back, so they run on a real adapter (`def_graphite_adapter_test!`).

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::graphite_types::{InsertRecordingInfo, InsertStatus};
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::wgpu::WgpuContext;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::{Reporter, def_graphite_adapter_test, errorf, reporter_assert};

// `SkColors::kRed`, `kGreen`, `kTransparent`, as SkColor4f.
const K_RED: Color4f = Color4f::new(1.0, 0.0, 0.0, 1.0);
const K_GREEN: Color4f = Color4f::new(0.0, 1.0, 0.0, 1.0);
const K_TRANSPARENT: Color4f = Color4f::new(0.0, 0.0, 0.0, 0.0);

// `kNoOffset` and `kEmptyClip`.
const K_NO_OFFSET: IPoint = IPoint::new(0, 0);
const K_EMPTY_CLIP: IRect = IRect::new_empty();

// `struct Expectation`.
struct Expectation {
    x: i32,
    y: i32,
    color: Color4f,
}

fn expect(x: i32, y: i32, color: Color4f) -> Expectation {
    Expectation { x, y, color }
}

// Port of: tests/graphite/RecordingSurfacesTest.cpp#L35-L106 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn run_test(
    reporter: &mut Reporter,
    context: &mut WgpuContext,
    surface_size: ISize,
    recording_size: ISize,
    replay_offset: IPoint,
    replay_clip: IRect,
    canvas_mipmapped: Mipmapped,
    target_mipmapped: Mipmapped,
    draw: impl Fn(&Canvas),
    expectations: &[Expectation],
) {
    let surface_image_info =
        ImageInfo::new(surface_size, ColorType::RGBA8888, AlphaType::Premul, None);

    let mut surface_recorder = context.make_recorder(None);
    let Some(mut surface) = Surface::render_target(
        &surface_recorder,
        &surface_image_info,
        canvas_mipmapped,
        None,
        "",
    ) else {
        errorf!(reporter, "Surface creation failed");
        return;
    };
    let Some(texture_info) = surface
        .target()
        .proxy()
        .map(|proxy| proxy.texture_info().clone())
    else {
        errorf!(reporter, "Surface has no target proxy");
        return;
    };

    // Flush the initial clear added by MakeGraphite.
    let Some(mut surface_recording) = surface_recorder.snap() else {
        errorf!(reporter, "Recording creation failed");
        return;
    };
    let _ = context.insert_recording(InsertRecordingInfo::new(&mut surface_recording));

    // Snap a recording without a bound target.
    let recording_image_info = surface_image_info.with_dimensions(recording_size);
    let mut recorder = context.make_recorder(None);
    let Some(canvas) = recorder.make_deferred_canvas(&recording_image_info, &texture_info) else {
        errorf!(reporter, "Deferred canvas creation failed");
        return;
    };
    draw(canvas);
    let Some(mut recording) = recorder.snap() else {
        errorf!(reporter, "Recording creation failed");
        return;
    };

    // Play back recording. If the mipmap settings don't match, we have to create a new surface.
    if canvas_mipmapped != target_mipmapped {
        let Some(new_surface) = Surface::render_target(
            &surface_recorder,
            &surface_image_info,
            target_mipmapped,
            None,
            "",
        ) else {
            errorf!(reporter, "Surface creation failed");
            return;
        };
        surface = new_surface;
        // Flush the initial clear for this new surface.
        let Some(mut new_surface_recording) = surface_recorder.snap() else {
            errorf!(reporter, "Recording creation failed");
            return;
        };
        let _ = context.insert_recording(InsertRecordingInfo::new(&mut new_surface_recording));
    }
    let mut insert = InsertRecordingInfo::new(&mut recording);
    insert.target_surface = Some(&surface);
    insert.target_translation = replay_offset;
    insert.target_clip = replay_clip;
    let _ = context.insert_recording(insert);

    // Read pixels.
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&surface_image_info, None);
    {
        let Some(mut pixmap) = bitmap.peek_pixels_mut() else {
            errorf!(reporter, "Bitmap has no pixels");
            return;
        };
        if !context.read_surface_pixels(&surface, &mut pixmap, 0, 0) {
            errorf!(reporter, "readPixels failed");
            return;
        }
    }
    let Some(pixmap) = bitmap.peek_pixels() else {
        errorf!(reporter, "Bitmap has no pixels");
        return;
    };

    // Verify expectations are met and recording is uninstantiated.
    reporter_assert!(reporter, !recording.priv_().is_target_proxy_instantiated());
    for e in expectations {
        let color = pixmap.get_color_4f(IPoint::new(e.x, e.y));
        reporter_assert!(reporter, color == e.color);
    }
}

// Tests that clear does not clear an entire replayed-to surface if recorded onto a smaller surface.
// Port of: tests/graphite/RecordingSurfacesTest.cpp#L109-L129 (chrome/m156)
def_graphite_adapter_test!(RecordingSurfacesTestClear, |reporter, context| {
    let surface_size = ISize::new(8, 4);
    let recording_size = ISize::new(4, 4);

    let draw = |canvas: &Canvas| {
        canvas.clear(K_RED);
    };

    let expectations = [expect(0, 0, K_RED), expect(4, 0, K_TRANSPARENT)];

    run_test(
        reporter,
        context,
        surface_size,
        recording_size,
        K_NO_OFFSET,
        K_EMPTY_CLIP,
        Mipmapped::No,
        Mipmapped::No,
        draw,
        &expectations,
    );
});

// Tests that a draw is translated correctly when replayed with an offset.
// Port of: tests/graphite/RecordingSurfacesTest.cpp#L132-L156 (chrome/m156)
def_graphite_adapter_test!(RecordingSurfacesTestDraw, |reporter, context| {
    let surface_size = ISize::new(8, 4);
    let recording_size = ISize::new(4, 4);
    let replay_offset = IPoint::new(4, 0);

    let draw = |canvas: &Canvas| {
        let mut paint = Paint::default();
        paint.set_color4f(K_RED, None);
        canvas.draw_irect(IRect::from_xywh(0, 0, 4, 4), &paint);
    };

    let expectations = [expect(0, 0, K_TRANSPARENT), expect(4, 0, K_RED)];

    run_test(
        reporter,
        context,
        surface_size,
        recording_size,
        replay_offset,
        K_EMPTY_CLIP,
        Mipmapped::No,
        Mipmapped::No,
        draw,
        &expectations,
    );
});

// Tests that writePixels is translated correctly when replayed with an offset.
// Port of: tests/graphite/RecordingSurfacesTest.cpp#L159-L187 (chrome/m156)
def_graphite_adapter_test!(RecordingSurfacesTestWritePixels, |reporter, context| {
    let mut bitmap = Bitmap::new();
    bitmap.alloc_n32_pixels((4, 4), true);
    {
        let Some(bitmap_canvas) = Canvas::from_bitmap(&mut bitmap, None) else {
            errorf!(reporter, "Bitmap canvas creation failed");
            return;
        };
        let mut paint = Paint::default();
        paint.set_color4f(K_RED, None);
        bitmap_canvas.draw_irect(IRect::from_xywh(0, 0, 4, 4), &paint);
    }

    let surface_size = ISize::new(8, 4);
    let recording_size = ISize::new(4, 4);
    let replay_offset = IPoint::new(4, 0);

    let draw = |canvas: &Canvas| {
        let _ = canvas.write_pixels_from_bitmap(&bitmap, IPoint::new(0, 0));
    };

    let expectations = [expect(0, 0, K_TRANSPARENT), expect(4, 0, K_RED)];

    run_test(
        reporter,
        context,
        surface_size,
        recording_size,
        replay_offset,
        K_EMPTY_CLIP,
        Mipmapped::No,
        Mipmapped::No,
        draw,
        &expectations,
    );
});

// Tests that the result of writePixels is cropped correctly when offscreen.
// Port of: tests/graphite/RecordingSurfacesTest.cpp#L190-L219 (chrome/m156)
def_graphite_adapter_test!(
    RecordingSurfacesTestWritePixelsOffscreen,
    |reporter, context| {
        let mut bitmap = Bitmap::new();
        bitmap.alloc_n32_pixels((4, 4), true);
        {
            let Some(bitmap_canvas) = Canvas::from_bitmap(&mut bitmap, None) else {
                errorf!(reporter, "Bitmap canvas creation failed");
                return;
            };
            let mut paint = Paint::default();
            paint.set_color4f(K_RED, None);
            bitmap_canvas.draw_irect(IRect::from_xywh(0, 0, 4, 4), &paint);
            paint.set_color4f(K_GREEN, None);
            bitmap_canvas.draw_irect(IRect::from_xywh(2, 2, 2, 2), &paint);
        }

        let surface_size = ISize::new(4, 4);
        let recording_size = ISize::new(4, 4);
        let replay_offset = IPoint::new(-2, -2);

        let draw = |canvas: &Canvas| {
            let _ = canvas.write_pixels_from_bitmap(&bitmap, IPoint::new(0, 0));
        };

        let expectations = [expect(0, 0, K_GREEN)];

        run_test(
            reporter,
            context,
            surface_size,
            recording_size,
            replay_offset,
            K_EMPTY_CLIP,
            Mipmapped::No,
            Mipmapped::No,
            draw,
            &expectations,
        );
    }
);

// Tests that the result of a draw is cropped correctly with a provided clip on replay.
// Port of: tests/graphite/RecordingSurfacesTest.cpp#L222-L270 (chrome/m156)
def_graphite_adapter_test!(RecordingSurfacesTestDrawWithClip, |reporter, context| {
    let surface_size = ISize::new(8, 4);
    let recording_size = ISize::new(8, 4);

    let draw = |canvas: &Canvas| {
        let mut paint = Paint::default();
        paint.set_color4f(K_RED, None);
        canvas.draw_irect(IRect::from_xywh(0, 0, 4, 4), &paint);
        paint.set_color4f(K_GREEN, None);
        canvas.draw_irect(IRect::from_xywh(4, 0, 4, 4), &paint);
    };

    {
        // Test that the clip applies in translated space.
        let replay_offset = IPoint::new(4, 0);
        let replay_clip = IRect::from_xywh(0, 0, 2, 4);

        let expectations = [
            expect(0, 0, K_TRANSPARENT),
            expect(4, 0, K_RED),
            expect(6, 0, K_TRANSPARENT),
        ];

        run_test(
            reporter,
            context,
            surface_size,
            recording_size,
            replay_offset,
            replay_clip,
            Mipmapped::No,
            Mipmapped::No,
            draw,
            &expectations,
        );
    }

    {
        // Test that the draw is not translated by the clip.
        let replay_clip = IRect::from_xywh(4, 0, 2, 4);

        let expectations = [expect(4, 0, K_GREEN)];

        run_test(
            reporter,
            context,
            surface_size,
            recording_size,
            K_NO_OFFSET,
            replay_clip,
            Mipmapped::No,
            Mipmapped::No,
            draw,
            &expectations,
        );
    }
});

// Tests that a scissor translated to negative coordinates is applied correctly.
// Port of: tests/graphite/RecordingSurfacesTest.cpp#L273-L300 (chrome/m156)
def_graphite_adapter_test!(RecordingSurfacesTestNegativeClip, |reporter, context| {
    let surface_size = ISize::new(4, 4);
    let recording_size = ISize::new(4, 4);

    let draw = |canvas: &Canvas| {
        canvas.clip_irect(IRect::from_xywh(0, 0, 4, 4), None);
        let mut paint = Paint::default();
        paint.set_color4f(K_RED, None);
        canvas.draw_irect(IRect::from_xywh(0, 0, 4, 4), &paint);
    };

    let replay_offset = IPoint::new(-2, 0);

    let expectations = [expect(0, 0, K_RED), expect(2, 0, K_TRANSPARENT)];

    run_test(
        reporter,
        context,
        surface_size,
        recording_size,
        replay_offset,
        K_EMPTY_CLIP,
        Mipmapped::No,
        Mipmapped::No,
        draw,
        &expectations,
    );
});

// Tests that the result of writePixels is cropped correctly with a provided clip on replay.
// Port of: tests/graphite/RecordingSurfacesTest.cpp#L303-L333 (chrome/m156)
def_graphite_adapter_test!(
    RecordingSurfacesTestWritePixelsWithClip,
    |reporter, context| {
        let mut bitmap = Bitmap::new();
        bitmap.alloc_n32_pixels((4, 4), true);
        {
            let Some(bitmap_canvas) = Canvas::from_bitmap(&mut bitmap, None) else {
                errorf!(reporter, "Bitmap canvas creation failed");
                return;
            };
            let mut paint = Paint::default();
            paint.set_color4f(K_RED, None);
            bitmap_canvas.draw_irect(IRect::from_xywh(0, 0, 4, 4), &paint);
        }

        let surface_size = ISize::new(8, 4);
        let recording_size = ISize::new(4, 4);
        let replay_offset = IPoint::new(4, 0);
        let replay_clip = IRect::from_xywh(0, 0, 2, 4);

        let draw = |canvas: &Canvas| {
            let _ = canvas.write_pixels_from_bitmap(&bitmap, IPoint::new(0, 0));
        };

        let expectations = [
            expect(0, 0, K_TRANSPARENT),
            expect(4, 0, K_RED),
            expect(6, 0, K_TRANSPARENT),
        ];

        run_test(
            reporter,
            context,
            surface_size,
            recording_size,
            replay_offset,
            replay_clip,
            Mipmapped::No,
            Mipmapped::No,
            draw,
            &expectations,
        );
    }
);

// Port of: tests/graphite/RecordingSurfacesTest.cpp#L335-L442 (chrome/m156)
def_graphite_adapter_test!(RecordingSurfacesTestMipmapped, |reporter, context| {
    let recording_size = ISize::new(1, 1);

    let draw = |canvas: &Canvas| {
        canvas.clear(K_RED);
    };

    {
        let surface_size = ISize::new(1, 1);

        let expectations = [expect(0, 0, K_RED)];

        run_test(
            reporter,
            context,
            surface_size,
            recording_size,
            K_NO_OFFSET,
            K_EMPTY_CLIP,
            Mipmapped::Yes,
            Mipmapped::Yes,
            draw,
            &expectations,
        );
    }

    {
        // Test that draw fails if canvas and surface mipmap settings don't match.
        let surface_size = ISize::new(1, 1);

        let expectations = [expect(0, 0, K_TRANSPARENT)];

        run_test(
            reporter,
            context,
            surface_size,
            recording_size,
            K_NO_OFFSET,
            K_EMPTY_CLIP,
            Mipmapped::Yes,
            Mipmapped::No,
            draw,
            &expectations,
        );

        run_test(
            reporter,
            context,
            surface_size,
            recording_size,
            K_NO_OFFSET,
            K_EMPTY_CLIP,
            Mipmapped::No,
            Mipmapped::Yes,
            draw,
            &expectations,
        );
    }

    {
        // Test that draw fails if canvas and surface dimensions don't match.
        let surface_size = ISize::new(2, 1);

        let expectations = [expect(0, 0, K_TRANSPARENT)];

        run_test(
            reporter,
            context,
            surface_size,
            recording_size,
            K_NO_OFFSET,
            K_EMPTY_CLIP,
            Mipmapped::Yes,
            Mipmapped::Yes,
            draw,
            &expectations,
        );
    }

    {
        // Test that draw fails if offset is provided.
        let surface_size = ISize::new(1, 1);
        let replay_offset = IPoint::new(1, 0);

        let expectations = [expect(0, 0, K_TRANSPARENT)];

        run_test(
            reporter,
            context,
            surface_size,
            recording_size,
            replay_offset,
            K_EMPTY_CLIP,
            Mipmapped::Yes,
            Mipmapped::Yes,
            draw,
            &expectations,
        );
    }

    {
        // Test that draw fails if clip is provided.
        let surface_size = ISize::new(1, 1);
        let replay_clip = IRect::from_xywh(0, 0, 1, 1);

        let expectations = [expect(0, 0, K_TRANSPARENT)];

        run_test(
            reporter,
            context,
            surface_size,
            recording_size,
            K_NO_OFFSET,
            replay_clip,
            Mipmapped::Yes,
            Mipmapped::Yes,
            draw,
            &expectations,
        );
    }
});

// Port of: tests/graphite/RecordingSurfacesTest.cpp#L444-L470 (chrome/m156)
def_graphite_adapter_test!(
    RecordingSurfacesTestMipmappedWritePixels,
    |reporter, context| {
        let mut bitmap = Bitmap::new();
        bitmap.alloc_n32_pixels((1, 1), true);
        {
            let Some(bitmap_canvas) = Canvas::from_bitmap(&mut bitmap, None) else {
                errorf!(reporter, "Bitmap canvas creation failed");
                return;
            };
            let mut paint = Paint::default();
            paint.set_color4f(K_RED, None);
            bitmap_canvas.draw_irect(IRect::from_xywh(0, 0, 1, 1), &paint);
        }

        let recording_size = ISize::new(1, 1);
        let surface_size = ISize::new(1, 1);

        let draw = |canvas: &Canvas| {
            let _ = canvas.write_pixels_from_bitmap(&bitmap, IPoint::new(0, 0));
        };

        let expectations = [expect(0, 0, K_RED)];

        run_test(
            reporter,
            context,
            surface_size,
            recording_size,
            K_NO_OFFSET,
            K_EMPTY_CLIP,
            Mipmapped::Yes,
            Mipmapped::Yes,
            draw,
            &expectations,
        );
    }
);

// Tests that you can't create two deferred canvases before snapping the first.
// Port of: tests/graphite/RecordingSurfacesTest.cpp#L473-L488 (chrome/m156)
def_graphite_adapter_test!(RecordingSurfacesTestTwoCanvases, |reporter, context| {
    let image_info = ImageInfo::new(
        ISize::new(1, 1),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut recorder = context.make_recorder(None);
    let Some(surface) = Surface::render_target(&recorder, &image_info, Mipmapped::No, None, "")
    else {
        errorf!(reporter, "Surface creation failed");
        return;
    };
    let Some(texture_info) = surface
        .target()
        .proxy()
        .map(|proxy| proxy.texture_info().clone())
    else {
        errorf!(reporter, "Surface has no target proxy");
        return;
    };

    // First canvas is created successfully.
    reporter_assert!(
        reporter,
        recorder
            .make_deferred_canvas(&image_info, &texture_info)
            .is_some()
    );
    // Second canvas fails to be created.
    reporter_assert!(
        reporter,
        recorder
            .make_deferred_canvas(&image_info, &texture_info)
            .is_none()
    );
});

// Tests that inserting a recording with a surface does not crash even if no draws to a deferred
// canvas were recorded.
// Port of: tests/graphite/RecordingSurfacesTest.cpp#L492-L503 (chrome/m156)
def_graphite_adapter_test!(
    RecordingSurfacesTestUnnecessarySurface,
    |reporter, context| {
        let image_info = ImageInfo::new(
            ISize::new(1, 1),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
        let mut recorder = context.make_recorder(None);

        let Some(surface) = Surface::render_target(&recorder, &image_info, Mipmapped::No, None, "")
        else {
            errorf!(reporter, "Surface creation failed");
            return;
        };
        let Some(mut recording) = recorder.snap() else {
            errorf!(reporter, "Recording creation failed");
            return;
        };
        let mut insert = InsertRecordingInfo::new(&mut recording);
        insert.target_surface = Some(&surface);
        reporter_assert!(
            reporter,
            context.insert_recording(insert) == InsertStatus::Success
        );
    }
);
