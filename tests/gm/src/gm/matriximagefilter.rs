// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/matriximagefilter.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{Canvas as CoreCanvas, SaveLayerRec};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_effects::image_filters::matrix_transform;
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: gm/matriximagefilter.cpp#L8-L16 (chrome/m156)
fn draw(canvas: &Canvas, rect: Rect, bitmap: &Bitmap, matrix: &Matrix, sampling: SamplingOptions) {
    let mut paint = Paint::default();
    paint.set_image_filter(matrix_transform(matrix, sampling, None));
    canvas.save_layer(&SaveLayerRec::default().bounds(&rect).paint(&paint));
    canvas.draw_image_with_sampling_options(
        bitmap.as_image().expect("an image"),
        (0.0, 0.0),
        sampling,
        None,
    );
    canvas.restore();
}

// Port of: gm/matriximagefilter.cpp#L18-L40 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // coordinates are below 64: exact in f32
fn make_checkerboard() -> Bitmap {
    let mut bitmap = Bitmap::new();
    bitmap.alloc_n32_pixels((64, 64), None);
    {
        let canvas = CoreCanvas::from_bitmap(&mut bitmap, None).expect("a canvas");
        let mut dark_paint = Paint::default();
        dark_paint.set_color(Color::new(0xFF40_4040));
        let mut light_paint = Paint::default();
        light_paint.set_color(Color::new(0xFFA0_A0A0));
        for y in (0..64).step_by(32) {
            for x in (0..64).step_by(32) {
                canvas.save();
                canvas.translate((x as f32, y as f32));
                canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 16.0, 16.0), &dark_paint);
                canvas.draw_rect(Rect::from_xywh(16.0, 0.0, 16.0, 16.0), &light_paint);
                canvas.draw_rect(Rect::from_xywh(0.0, 16.0, 16.0, 16.0), &light_paint);
                canvas.draw_rect(Rect::from_xywh(16.0, 16.0, 16.0, 16.0), &dark_paint);
                canvas.restore();
            }
        }
    }
    bitmap
}

// Port of: gm/matriximagefilter.cpp#L42-L55 (chrome/m156)
crate::def_simple_gm_bg!(matriximagefilter, canvas, 420, 100, Color::BLACK, {
    let margin = 10.0f32;
    let matrix = Matrix::skew((0.5, 0.2));
    let checkerboard = make_checkerboard();
    let src_rect = Rect::from_wh(96.0, 96.0);
    canvas.translate((margin, margin));
    draw(
        canvas,
        src_rect,
        &checkerboard,
        &matrix,
        SamplingOptions::default(),
    );
    canvas.translate((src_rect.width() + margin, 0.0));
    draw(
        canvas,
        src_rect,
        &checkerboard,
        &matrix,
        SamplingOptions::from(FilterMode::Linear),
    );
});
