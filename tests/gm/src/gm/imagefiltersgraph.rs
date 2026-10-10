// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagefiltersgraph.cpp (chrome/m156)

// GM ports mirror the C++ GM body, which is long by nature.
#![allow(clippy::too_many_lines)]
use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::image::Image;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{
    arithmetic, blend, blur, color_filter, dilate, erode, image_sampled, matrix_convolution, merge,
    offset,
};
use skia_rust_tools::font_tool_utils::create_string_bitmap;

// Port of: gm/imagefiltersgraph.cpp#L38-L45 (chrome/m156), DrawClippedImage
fn draw_clipped_image(canvas: &Canvas, image: &Image, paint: &Paint) {
    canvas.save();
    canvas.clip_irect(image.bounds(), None);
    canvas.draw_image_with_sampling_options(
        image,
        (0.0, 0.0),
        SamplingOptions::default(),
        Some(paint),
    );
    canvas.restore();
}

// Port of: gm/imagefiltersgraph.cpp#L19-L40 (chrome/m156), ImageFiltersGraphGM
struct ImageFiltersGraphGm {
    image: Option<Image>,
}

impl GM for ImageFiltersGraphGm {
    fn name(&self) -> String {
        "imagefiltersgraph".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(600, 150)
    }

    // Port of: gm/imagefiltersgraph.cpp#L28-L30 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.image = create_string_bitmap(100, 100, Color::WHITE, 20, 70, 96, "e").as_image();
    }

    // Port of: gm/imagefiltersgraph.cpp#L32-L122 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.clear(Color::BLACK);
        let Some(image) = self.image.clone() else {
            return;
        };
        {
            let bitmap_source = image_sampled(
                Some(image.clone()),
                SamplingOptions::from(FilterMode::Linear),
            );
            let cf = color_filters::blend(Color4f::from(Color::RED), None, BlendMode::SrcIn);
            let blur_filter = blur(4.0, 4.0, TileMode::Decal, bitmap_source, None);
            let erode_filter = erode((4.0, 4.0), blur_filter.clone(), None);
            let color = color_filter(cf, erode_filter, None);
            let merged = merge(&[blur_filter, color], None);
            let mut paint = Paint::default();
            paint.set_image_filter(merged);
            canvas.draw_paint(&paint);
            canvas.translate((100.0, 0.0));
        }
        {
            let morph = dilate((5.0, 5.0), None, None);
            let matrix: [f32; 20] = [
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.5, 0.0,
            ];
            let matrix_filter = color_filters::matrix_row_major(&matrix, Clamp::Yes);
            let color_morph = color_filter(matrix_filter, morph, None);
            let mut paint = Paint::default();
            paint.set_image_filter(blend(BlendMode::SrcOver, color_morph, None, None));
            draw_clipped_image(canvas, &image, &paint);
            canvas.translate((100.0, 0.0));
        }
        {
            let matrix: [f32; 20] = [
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.5, 0.0,
            ];
            let matrix_cf = color_filters::matrix_row_major(&matrix, Clamp::Yes);
            let matrix_filter = color_filter(matrix_cf, None, None);
            let offset_filter = offset((10.0, 10.0), matrix_filter.clone(), None);
            let mut paint = Paint::default();
            paint.set_image_filter(arithmetic(
                0.0,
                1.0,
                1.0,
                0.0,
                true,
                matrix_filter,
                offset_filter,
                None,
            ));
            draw_clipped_image(canvas, &image, &paint);
            canvas.translate((100.0, 0.0));
        }
        {
            let blur_filter = blur(10.0, 10.0, TileMode::Decal, None, None);
            let crop_rect = Rect::from(IRect::from_xywh(0, 0, 95, 100));
            let mut paint = Paint::default();
            paint.set_image_filter(blend(BlendMode::SrcIn, blur_filter, None, Some(crop_rect)));
            draw_clipped_image(canvas, &image, &paint);
            canvas.translate((100.0, 0.0));
        }
        {
            // Dilate -> matrix convolution.
            // This tests that a filter using asFragmentProcessor (matrix
            // convolution) correctly handles a non-zero source offset
            // (supplied by the dilate).
            let dilate_filter = dilate((5.0, 5.0), None, None);
            #[rustfmt::skip]
            let kernel: [f32; 9] = [
                -1.0, -1.0, -1.0,
                -1.0,  7.0, -1.0,
                -1.0, -1.0, -1.0,
            ];
            let convolve = matrix_convolution(
                (3, 3),
                &kernel,
                1.0,
                0.0,
                (1, 1),
                TileMode::Clamp,
                false,
                dilate_filter,
                None,
            );
            let mut paint = Paint::default();
            paint.set_image_filter(convolve);
            draw_clipped_image(canvas, &image, &paint);
            canvas.translate((100.0, 0.0));
        }
        {
            // Test that crop offsets are absolute, not relative to the parent's crop rect.
            let cf1 = color_filters::blend(Color4f::from(Color::BLUE), None, BlendMode::SrcIn);
            let cf2 = color_filters::blend(Color4f::from(Color::GREEN), None, BlendMode::SrcIn);
            let outer_rect = Rect::from(IRect::from_xywh(10, 10, 80, 80));
            let inner_rect = Rect::from(IRect::from_xywh(20, 20, 60, 60));
            let color1 = color_filter(cf1, None, Some(outer_rect));
            let color2 = color_filter(cf2, color1, Some(inner_rect));
            let mut paint = Paint::default();
            paint.set_image_filter(color2);
            paint.set_color(Color::RED);
            canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 100.0, 100.0), &paint);
            canvas.translate((100.0, 0.0));
        }
    }
}

// Port of: gm/imagefiltersgraph.cpp#L161 (chrome/m156), DEF_GM(return new ImageFiltersGraphGM;)
crate::def_gm!(ImageFiltersGraphGM, ImageFiltersGraphGm { image: None });
