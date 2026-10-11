// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/tileimagefilter.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::{create_checkerboard_image, int_to_scalar};
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::image::Image;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_effects::image_filters::{color_filter, image_sampled, tile};
use skia_rust_tools::font_tool_utils::create_string_bitmap;

const WIDTH: i32 = 400;
const HEIGHT: i32 = 200;
const MARGIN: i32 = 12;

// Port of: gm/tileimagefilter.cpp#L17-L125 (chrome/m156), TileImageFilterGM
struct TileImageFilterGm {
    bitmap: Option<Image>,
    checkerboard: Option<Image>,
}

impl TileImageFilterGm {
    fn new() -> Self {
        Self {
            bitmap: None,
            checkerboard: None,
        }
    }
}

impl GM for TileImageFilterGm {
    fn name(&self) -> String {
        "tileimagefilter".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    // Port of: gm/tileimagefilter.cpp#L19-L21 (chrome/m156), setBGColor(0xFF000000)
    fn bg_color(&self) -> Color {
        Color::BLACK
    }

    // Port of: gm/tileimagefilter.cpp#L26-L29 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        // ToolUtils::CreateStringImage is CreateStringBitmap(...).asImage().
        self.bitmap =
            create_string_bitmap(50, 50, Color::from(0xD000_D000), 10, 45, 50, "e").as_image();
        self.checkerboard = Some(create_checkerboard_image(
            80,
            80,
            Color::from(0xFFA0_A0A0),
            Color::from(0xFF40_4040),
            8,
        ));
    }

    // Port of: gm/tileimagefilter.cpp#L35-L121 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let bitmap = self
            .bitmap
            .clone()
            .expect("the bitmap is made in onOnceBeforeDraw");
        let checkerboard = self
            .checkerboard
            .clone()
            .expect("the checkerboard is made in onOnceBeforeDraw");

        canvas.clear(Color::BLACK);

        let mut red = Paint::default();
        red.set_color(Color::RED);
        red.set_style(Style::Stroke);

        let mut blue = Paint::default();
        blue.set_color(Color::BLUE);
        blue.set_style(Style::Stroke);

        let mut x = 0;
        let mut y = 0;
        for i in 0..4_i32 {
            let image = if (i & 0x01) != 0 {
                &checkerboard
            } else {
                &bitmap
            };
            let (iw, ih) = (image.width(), image.height());
            let src_rect = Rect::from_xywh(
                int_to_scalar(iw / 4),
                int_to_scalar(ih / 4),
                int_to_scalar(iw / (i + 1)),
                int_to_scalar(ih / (i + 1)),
            );
            let dst_rect = Rect::from_xywh(
                int_to_scalar(i * 8),
                int_to_scalar(i * 4),
                int_to_scalar(iw - i * 12),
                int_to_scalar(ih) - int_to_scalar(i * 12),
            );

            let tile_input = image_sampled(
                Some(image.clone()),
                SamplingOptions::from(FilterMode::Linear),
            );
            let filter = tile(&src_rect, &dst_rect, tile_input);

            canvas.save();
            canvas.translate((int_to_scalar(x), int_to_scalar(y)));
            let mut paint = Paint::default();
            paint.set_image_filter(filter);
            canvas.draw_image_with_sampling_options(
                &bitmap,
                (0.0, 0.0),
                SamplingOptions::default(),
                Some(&paint),
            );
            canvas.draw_rect(src_rect, &red);
            canvas.draw_rect(dst_rect, &blue);
            canvas.restore();

            x += iw + MARGIN;
            if x + iw > WIDTH {
                x = 0;
                y += ih + MARGIN;
            }
        }

        {
            // float matrix[20] = identity
            let matrix = [
                1.0, 0.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 0.0, 1.0, 0.0,
            ];
            let src_rect = Rect::from_wh(
                int_to_scalar(bitmap.width()),
                int_to_scalar(bitmap.height()),
            );
            let dst_rect = Rect::from_wh(
                int_to_scalar(bitmap.width() * 2),
                int_to_scalar(bitmap.height() * 2),
            );
            let tile_filter = tile(&src_rect, &dst_rect, None);
            let cf = color_filters::matrix_row_major(&matrix, Clamp::Yes);
            let mut paint = Paint::default();
            paint.set_image_filter(color_filter(cf, tile_filter, None));

            canvas.save();
            canvas.translate((int_to_scalar(x), int_to_scalar(y)));
            canvas.clip_rect(dst_rect, None, None);
            canvas.save_layer(&SaveLayerRec::default().bounds(&dst_rect).paint(&paint));
            canvas.draw_image(&bitmap, (0.0, 0.0), None);
            canvas.restore();
            canvas.draw_rect(src_rect, &red);
            canvas.draw_rect(dst_rect, &blue);
            canvas.restore();
        }

        // test that the crop rect properly applies to the tile image filter
        {
            canvas.translate((0.0, 100.0));
            let src_rect = Rect::from_xywh(0.0, 0.0, 50.0, 50.0);
            let dst_rect = Rect::from_xywh(0.0, 0.0, 100.0, 100.0);
            // SkIRect::MakeXYWH(5, 5, 40, 40) as a float rect
            let crop_rect = Rect::from_ltrb(5.0, 5.0, 45.0, 45.0);
            let green_cf = color_filters::blend_color(Color::GREEN, BlendMode::Src);
            let green = color_filter(green_cf, None, Some(crop_rect));
            let mut paint = Paint::default();
            paint.set_color(Color::RED);
            paint.set_image_filter(tile(&src_rect, &dst_rect, green));
            canvas.draw_rect(dst_rect, &paint);
        }
    }
}

// Port of: gm/tileimagefilter.cpp#L137 (chrome/m156)
crate::def_gm!(TileImageFilterGM, TileImageFilterGm::new());
