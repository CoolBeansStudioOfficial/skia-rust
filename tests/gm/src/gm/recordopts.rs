// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/recordopts.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::colors::TRANSPARENT;
use skia_rust_core::color_filters::table_argb;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_effects::image_filters::{color_filter, picture};
use skia_rust_raster::raster_canvas::RasterCanvas;

const TEST_RECT_SIZE: i32 = 50;
const DETECTOR_GREEN_VALUE: u8 = 50;

// This detector detects that color filter phase of the pixel pipeline receives the correct value.
// Below are few functions to install "detector" color filters. The filter is there to assert that
// the color value it sees is the expected. It will trigger only with kDetectorGreenValue, and
// turn that value into full green. The idea is that if an optimization incorrectly changes
// kDetectorGreenValue and then the incorrect value is observable by some part of the drawing
// pipeline, that pixel will remain empty.
// Port of: gm/recordopts.cpp#L36-L45 (chrome/m156), make_detector_color_filter
fn make_detector_color_filter() -> Option<skia_rust_core::color_filter::ColorFilter> {
    let mut table_a = [0_u8; 256];
    let table_r = [0_u8; 256];
    let mut table_g = [0_u8; 256];
    let table_b = [0_u8; 256];
    table_a[255] = 255;
    table_g[usize::from(DETECTOR_GREEN_VALUE)] = 255;
    table_argb(Some(&table_a), Some(&table_r), Some(&table_g), Some(&table_b))
}

// Port of: gm/recordopts.cpp#L47-L50 (chrome/m156), install_detector_color_filter
fn install_detector_color_filter(draw_paint: &mut Paint) {
    draw_paint.set_color_filter(make_detector_color_filter());
}

// This detector detects that image filter phase of the pixel pipeline receives the correct value.
// Port of: gm/recordopts.cpp#L52-L55 (chrome/m156), install_detector_image_filter
fn install_detector_image_filter(draw_paint: &mut Paint) {
    let input = draw_paint.image_filter();
    draw_paint.set_image_filter(color_filter(make_detector_color_filter(), input, None));
}

// Port of: gm/recordopts.cpp#L57-L58 (chrome/m156), no_detector_install
fn no_detector_install(_draw_paint: &mut Paint) {}

// Port of: gm/recordopts.cpp#L60 (chrome/m156), InstallDetectorFunc
type InstallDetectorFunc = fn(&mut Paint);

// Draws an pattern that can be optimized by alpha folding outer savelayer alpha value to
// inner draw. Since we know that folding will happen to the inner draw, install a detector
// to make sure that optimization does not change anything observable.
// Port of: gm/recordopts.cpp#L66-L78 (chrome/m156), draw_save_layer_draw_rect_restore_sequence
fn draw_save_layer_draw_rect_restore_sequence(
    canvas: &Canvas,
    shape_color: Color,
    install_detector: InstallDetectorFunc,
) {
    let test_rect_size = int_to_scalar(TEST_RECT_SIZE);
    let target_rect = Rect::from_wh(test_rect_size, test_rect_size);
    let mut layer_paint = Paint::default();
    layer_paint.set_color(Color::from_argb(128, 0, 0, 0));
    canvas.save_layer(
        &SaveLayerRec::default()
            .bounds(&target_rect)
            .paint(&layer_paint),
    );
    let mut draw_paint = Paint::default();
    draw_paint.set_color(shape_color);
    install_detector(&mut draw_paint);
    canvas.draw_rect(&target_rect, &draw_paint);
    canvas.restore();
}

// Draws an pattern that can be optimized by alpha folding outer savelayer alpha value to
// inner draw. A variant where the draw is not uniform color.
// Port of: gm/recordopts.cpp#L81-L105 (chrome/m156), draw_save_layer_draw_bitmap_restore_sequence
fn draw_save_layer_draw_bitmap_restore_sequence(
    canvas: &Canvas,
    shape_color: Color,
    install_detector: InstallDetectorFunc,
) {
    let mut bitmap = Bitmap::new();
    bitmap.alloc_n32_pixels((TEST_RECT_SIZE, TEST_RECT_SIZE), None);
    bitmap.erase_color(shape_color);
    {
        // Make the bitmap non-uniform color, so that it can not be optimized as uniform drawRect.
        if let Some(bitmap_canvas) = Canvas::from_bitmap(&mut bitmap, None) {
            let mut p = Paint::default();
            p.set_color(Color::WHITE);
            debug_assert_ne!(shape_color, Color::WHITE);
            bitmap_canvas.draw_rect(&Rect::from_wh(7.0, 7.0), &p);
        }
    }

    let test_rect_size = int_to_scalar(TEST_RECT_SIZE);
    let target_rect = Rect::from_wh(test_rect_size, test_rect_size);
    let mut layer_paint = Paint::default();
    layer_paint.set_color(Color::from_argb(129, 0, 0, 0));
    canvas.save_layer(
        &SaveLayerRec::default()
            .bounds(&target_rect)
            .paint(&layer_paint),
    );
    let mut draw_paint = Paint::default();
    install_detector(&mut draw_paint);
    if let Some(image) = bitmap.as_image() {
        canvas.draw_image_with_sampling_options(
            &image,
            (0.0, 0.0),
            SamplingOptions::default(),
            Some(&draw_paint),
        );
    }
    canvas.restore();
}

// Draws an pattern that can be optimized by alpha folding outer savelayer alpha value to
// inner savelayer. We know that alpha folding happens to inner savelayer, so add detector there.
// Port of: gm/recordopts.cpp#L107-L140 (chrome/m156), draw_svg_opacity_and_filter_layer_sequence
fn draw_svg_opacity_and_filter_layer_sequence(
    canvas: &Canvas,
    shape_color: Color,
    install_detector: InstallDetectorFunc,
) {
    let test_rect_size = int_to_scalar(TEST_RECT_SIZE);
    let target_rect = Rect::from_wh(test_rect_size, test_rect_size);
    let shape: Option<Picture> = {
        let mut recorder = PictureRecorder::new();
        let record_canvas = recorder.begin_recording(
            Rect::from_wh(
                int_to_scalar(TEST_RECT_SIZE + 2),
                int_to_scalar(TEST_RECT_SIZE + 2),
            ),
            false,
        );
        let mut shape_paint = Paint::default();
        shape_paint.set_color(shape_color);
        record_canvas.draw_rect(&target_rect, &shape_paint);
        recorder.finish_recording_as_picture(None)
    };

    let mut layer_paint = Paint::default();
    layer_paint.set_color(Color::from_argb(130, 0, 0, 0));
    canvas.save_layer(
        &SaveLayerRec::default()
            .bounds(&target_rect)
            .paint(&layer_paint),
    );
    canvas.save();
    canvas.clip_rect(target_rect, ClipOp::Intersect, false);
    let mut draw_paint = Paint::default();
    draw_paint.set_image_filter(picture(shape, None));
    install_detector(&mut draw_paint);
    canvas.save_layer(
        &SaveLayerRec::default()
            .bounds(&target_rect)
            .paint(&draw_paint),
    );
    canvas.restore();
    canvas.restore();
    canvas.restore();
}

// Type of the three sequences above, as used in the GM's table.
type TestVariantSequence = fn(&Canvas, Color, InstallDetectorFunc);

// Draws two columns of rectangles. The test is correct when:
//  - Left and right columns always identical
//  - First 3 rows are green, with a white dent in the middle row
//  - Next 6 rows are green, with a grey dent in the middle row
//    (the grey dent is from the color filter removing everything but the "good" green, see below)
//  - Last 6 rows are grey
// Port of: gm/recordopts.cpp#L142-L230 (chrome/m156), DEF_SIMPLE_GM(recordopts, ...)
crate::def_simple_gm!(
    recordopts,
    canvas,
    (TEST_RECT_SIZE + 1) * 2,
    (TEST_RECT_SIZE + 1) * 15,
    {
        // The flushes of the C++ are GPU-only (`GrAsDirectContext`), so there is nothing to flush.
        canvas.clear(TRANSPARENT);

        let funcs: [TestVariantSequence; 3] = [
            draw_save_layer_draw_rect_restore_sequence,
            draw_save_layer_draw_bitmap_restore_sequence,
            draw_svg_opacity_and_filter_layer_sequence,
        ];

        // Draw layer-related sequences that can be optimized by folding the opacity layer alpha to
        // the inner draw operation. This tries to trigger the optimization, and relies on gm diffs
        // to keep the color value correct over time.
        // Draws two green rects side by side: one is without the optimization, the other is with
        // the optimization applied.
        let shape_color = Color::from_argb(255, 0, 255, 0);
        for draw_test_sequence in funcs {
            canvas.save();
            draw_test_sequence(canvas, shape_color, no_detector_install);
            canvas.translate((int_to_scalar(TEST_RECT_SIZE) + 1.0, 0.0));
            {
                let mut recorder = PictureRecorder::new();
                draw_test_sequence(
                    recorder.begin_recording(
                        Rect::from_wh(
                            int_to_scalar(TEST_RECT_SIZE),
                            int_to_scalar(TEST_RECT_SIZE),
                        ),
                        false,
                    ),
                    shape_color,
                    no_detector_install,
                );
                if let Some(pic) = recorder.finish_recording_as_picture(None) {
                    pic.playback(canvas);
                }
            }
            canvas.restore();
            canvas.translate((0.0, int_to_scalar(TEST_RECT_SIZE) + 1.0));
        }

        // Draw the same layer related sequences, but manipulate the sequences so that the result is
        // incorrect if the alpha is folded or folded incorrectly. These test the observable state
        // throughout the pixel pipeline, and thus may turn off the optimizations (this is why we
        // trigger the optimizations above).
        // Draws two green rects side by side: one is without the optimization, the other is with
        // the possibility that optimization is applied.
        // At the end, draws the same patterns in translucent black. This tests that the detectors
        // work, eg. that if the value the detector sees is wrong, the resulting image shows this.
        let shape_colors = [
            Color::from_argb(255, 0, DETECTOR_GREEN_VALUE, 0),
            // This tests that detectors work.
            Color::from_argb(255, 0, DETECTOR_GREEN_VALUE + 1, 0),
        ];
        let detector_install_funcs: [InstallDetectorFunc; 2] =
            [install_detector_image_filter, install_detector_color_filter];
        for shape_color in shape_colors {
            for detector_install_func in detector_install_funcs {
                for draw_test_sequence in funcs {
                    canvas.save();
                    draw_test_sequence(canvas, shape_color, detector_install_func);
                    canvas.translate((int_to_scalar(TEST_RECT_SIZE + 1), 0.0));
                    {
                        let mut recorder = PictureRecorder::new();
                        draw_test_sequence(
                            recorder.begin_recording(
                                Rect::from_wh(
                                    int_to_scalar(TEST_RECT_SIZE),
                                    int_to_scalar(TEST_RECT_SIZE),
                                ),
                                false,
                            ),
                            shape_color,
                            detector_install_func,
                        );
                        if let Some(pic) = recorder.finish_recording_as_picture(None) {
                            pic.playback(canvas);
                        }
                    }
                    canvas.restore();
                    canvas.translate((0.0, int_to_scalar(TEST_RECT_SIZE + 1)));
                }
            }
        }
    }
);
