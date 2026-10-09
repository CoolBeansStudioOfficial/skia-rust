// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/resizeimagefilter.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::Color;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::size::Size;
use skia_rust_effects::image_filters::{image, matrix_transform};
use skia_rust_raster::surfaces;

// Port of: gm/resizeimagefilter.cpp#L14-L53 (chrome/m156)
fn draw(
    canvas: &Canvas,
    rect: Rect,
    device_size: Size,
    sampling: SamplingOptions,
    input: Option<ImageFilter>,
) {
    let (dst_rect, _) = canvas.local_to_device_as_3x3().map_rect(rect);
    canvas.save();
    let device_scale_x = device_size.width / dst_rect.width();
    let device_scale_y = device_size.height / dst_rect.height();
    canvas.translate((rect.left, rect.top));
    canvas.scale((device_scale_x, device_scale_y));
    canvas.translate((-rect.left, -rect.top));
    let matrix = Matrix::scale((1.0 / device_scale_x, 1.0 / device_scale_y));
    let filter = matrix_transform(&matrix, sampling, input);
    let mut filtered_paint = Paint::default();
    filtered_paint.set_image_filter(filter);
    canvas.save_layer(&SaveLayerRec::default().bounds(&rect).paint(&filtered_paint));
    let mut paint = Paint::default();
    paint.set_color(Color::new(0xFF00_FF00));
    let mut oval_rect = rect;
    oval_rect.inset((4.0, 4.0));
    canvas.draw_oval(oval_rect, &paint);
    canvas.restore(); // for saveLayer
    canvas.restore();
}

// Port of: gm/resizeimagefilter.cpp#L14-L53 (chrome/m156), onDraw
crate::def_simple_gm_bg_name!(
    ResizeGM,
    canvas,
    630,
    100,
    Color::TRANSPARENT,
    "resizeimagefilter",
    {
        canvas.clear(Color::BLACK);
        let samplings = [
            SamplingOptions::default(),
            SamplingOptions::from(FilterMode::Linear),
            SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
            SamplingOptions::from(CubicResampler::mitchell()),
            SamplingOptions::from_aniso(16),
        ];
        let src_rect = Rect::from_wh(96.0, 96.0);
        let device_size = Size::new(16.0, 16.0);
        for sampling in samplings {
            draw(canvas, src_rect, device_size, sampling, None);
            canvas.translate((src_rect.width() + 10.0, 0.0));
        }
        {
            let mut surface = surfaces::raster_n32_premul((16, 16)).expect("a surface");
            {
                let surface_canvas = surface.canvas();
                surface_canvas.clear(Color::new(0x0000_0000));
                let mut paint = Paint::default();
                paint.set_color(Color::new(0xFF00_FF00));
                let mut oval_rect = Rect::from_wh(16.0, 16.0);
                let inset = 2.0f32 / 3.0;
                oval_rect.inset((inset, inset));
                surface_canvas.draw_oval(oval_rect, &paint);
            }
            let snapshot = surface.image_snapshot().expect("an image");
            let in_rect = Rect::from_xywh(-4.0, -4.0, 20.0, 20.0);
            let out_rect = Rect::from_xywh(-24.0, -24.0, 120.0, 120.0);
            let source = image(
                Some(snapshot),
                in_rect,
                out_rect,
                SamplingOptions::from(CubicResampler {
                    b: 1.0 / 3.0,
                    c: 1.0 / 3.0,
                }),
            );
            draw(canvas, src_rect, device_size, samplings[3], Some(source));
        }
    }
);
