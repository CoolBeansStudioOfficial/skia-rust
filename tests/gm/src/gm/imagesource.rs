// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagesource.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::image::Image;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, SamplingOptions};
use skia_rust_effects::image_filters::{image, image_sampled};
use skia_rust_tools::font_tool_utils::create_string_bitmap;

// Port of: gm/imagesource.cpp#L15-L25 (chrome/m156), fill_rect_filtered
fn fill_rect_filtered(
    canvas: &Canvas,
    clip_rect: Rect,
    filter: Option<skia_rust_core::image_filter::ImageFilter>,
) {
    let mut paint = Paint::default();
    paint.set_image_filter(filter);
    canvas.save();
    canvas.clip_rect(clip_rect, None, None);
    canvas.draw_paint(&paint);
    canvas.restore();
}

// Port of: gm/imagesource.cpp#L27-L94 (chrome/m156), ImageSourceGM
struct ImageSourceGm {
    image: Option<Image>,
}

impl ImageSourceGm {
    fn new() -> Self {
        Self { image: None }
    }
}

impl GM for ImageSourceGm {
    fn name(&self) -> String {
        "imagesource".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(500, 150)
    }

    // Port of: gm/imagesource.cpp#L36-L38 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        // ToolUtils::CreateStringImage is CreateStringBitmap(...).asImage().
        self.image =
            create_string_bitmap(100, 100, Color::from(0xFFFF_FFFF), 20, 70, 96, "e").as_image();
    }

    // Port of: gm/imagesource.cpp#L40-L87 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let img = self
            .image
            .clone()
            .expect("the image is made in onOnceBeforeDraw");
        canvas.clear(Color::BLACK);

        let src_rect = Rect::from_xywh(20.0, 20.0, 30.0, 30.0);
        let dst_rect = Rect::from_xywh(0.0, 10.0, 60.0, 60.0);
        let clip_rect = Rect::from_xywh(0.0, 0.0, 100.0, 100.0);
        let bounds = Rect::from_iwh(img.width(), img.height());
        let sampling = SamplingOptions::from(CubicResampler {
            b: 1.0 / 3.0,
            c: 1.0 / 3.0,
        });

        {
            // Draw an unscaled bitmap.
            let image_source = image_sampled(
                Some(img.clone()),
                SamplingOptions::from(FilterMode::Nearest),
            );
            fill_rect_filtered(canvas, clip_rect, image_source);
            canvas.translate((int_to_scalar(100), 0.0));
        }
        {
            // Draw an unscaled subset of the source bitmap (srcRect -> srcRect).
            let image_source_src_rect = image(Some(img.clone()), src_rect, src_rect, sampling);
            fill_rect_filtered(canvas, clip_rect, Some(image_source_src_rect));
            canvas.translate((int_to_scalar(100), 0.0));
        }
        {
            // Draw a subset of the bitmap scaled to a destination rect (srcRect -> dstRect).
            let image_source_src_rect_dst_rect =
                image(Some(img.clone()), src_rect, dst_rect, sampling);
            fill_rect_filtered(canvas, clip_rect, Some(image_source_src_rect_dst_rect));
            canvas.translate((int_to_scalar(100), 0.0));
        }
        {
            // Draw the entire bitmap scaled to a destination rect (bounds -> dstRect).
            let image_source_dst_rect_only = image(Some(img.clone()), bounds, dst_rect, sampling);
            fill_rect_filtered(canvas, clip_rect, Some(image_source_dst_rect_only));
            canvas.translate((int_to_scalar(100), 0.0));
        }
    }
}

// Port of: gm/imagesource.cpp#L96 (chrome/m156)
crate::def_gm!(ImageSourceGM, ImageSourceGm::new());
