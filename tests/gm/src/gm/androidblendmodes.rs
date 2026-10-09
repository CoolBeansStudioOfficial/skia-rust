// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/androidblendmodes.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use crate::tool_utils::{color_to_565, draw_checkerboard};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{Canvas as CoreCanvas, SaveLayerRec};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::utils::text_utils::{self, Align};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_tools::font_tool_utils::default_portable_font;

const BITMAP_SIZE: i32 = 256;
const NUM_ROWS: i32 = 5;
const NUM_COLS: i32 = 4;

// SkColorSetARGB(255, 22, 150, 243) and friends
const K_BLUE: Color = Color::from_argb(255, 22, 150, 243);
const K_RED: Color = Color::from_argb(255, 233, 30, 99);
const K_WHITE: Color = Color::from_argb(255, 243, 243, 243);
const K_GREY: Color = Color::from_argb(255, 222, 222, 222);

// Port of: gm/androidblendmodes.cpp#L27-L99 (chrome/m156), AndroidBlendModesGM
struct AndroidBlendModesGm {
    composite_src: Bitmap,
    composite_dst: Bitmap,
}

impl AndroidBlendModesGm {
    fn new() -> Self {
        Self {
            composite_src: Bitmap::new(),
            composite_dst: Bitmap::new(),
        }
    }

    // Port of: gm/androidblendmodes.cpp#L69-L80 (chrome/m156), drawTile
    fn draw_tile(&self, canvas: &Canvas, x_offset: i32, y_offset: i32, mode: BlendMode) {
        canvas.translate((int_to_scalar(x_offset), int_to_scalar(y_offset)));
        canvas.clip_rect(Rect::from_xywh(0.0, 0.0, 256.0, 256.0), None, None);
        canvas.save_layer(&SaveLayerRec::default());

        let mut p = Paint::default();
        if let Some(image) = self.composite_dst.as_image() {
            canvas.draw_image_with_sampling_options(
                &image,
                (0.0, 0.0),
                SamplingOptions::default(),
                Some(&p),
            );
        }
        p.set_blend_mode(mode);
        if let Some(image) = self.composite_src.as_image() {
            canvas.draw_image_with_sampling_options(
                &image,
                (0.0, 0.0),
                SamplingOptions::default(),
                Some(&p),
            );
        }
    }
}

impl GM for AndroidBlendModesGm {
    fn name(&self) -> String {
        "androidblendmodes".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(NUM_COLS * BITMAP_SIZE, NUM_ROWS * BITMAP_SIZE)
    }

    fn bg_color(&self) -> Color {
        Color::BLACK
    }

    // Port of: gm/androidblendmodes.cpp#L37-L60 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let ii = ImageInfo::new_n32_premul((BITMAP_SIZE, BITMAP_SIZE), None);
        {
            self.composite_src.alloc_pixels_info(&ii, None);
            let tmp = CoreCanvas::from_bitmap(&mut self.composite_src, None)
                .expect("a canvas on the bitmap");
            tmp.clear(Color::TRANSPARENT);
            let mut p = Paint::default();
            p.set_anti_alias(true);
            p.set_color(color_to_565(K_BLUE));
            tmp.draw_rect(Rect::new(16.0, 96.0, 160.0, 240.0), &p);
        }
        {
            self.composite_dst.alloc_pixels_info(&ii, None);
            let tmp = CoreCanvas::from_bitmap(&mut self.composite_dst, None)
                .expect("a canvas on the bitmap");
            tmp.clear(Color::TRANSPARENT);
            let mut p = Paint::default();
            p.set_anti_alias(true);
            p.set_color(color_to_565(K_RED));
            tmp.draw_circle((160.0, 95.0), 80.0, &p);
        }
    }

    // Port of: gm/androidblendmodes.cpp#L82-L118 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let font = default_portable_font();

        draw_checkerboard(canvas, K_WHITE, K_GREY, 32);

        let mut x_offset = 0;
        let mut y_offset = 0;

        // Android doesn't expose all the blend modes.
        // Note that the Android documentation calls:
        //    Skia's kPlus,     add
        //    Skia's kModulate, multiply
        let modes = [
            BlendMode::Plus,
            BlendMode::Clear,
            BlendMode::Darken,
            BlendMode::Dst,
            BlendMode::DstATop,
            BlendMode::DstIn,
            BlendMode::DstOut,
            BlendMode::DstOver,
            BlendMode::Lighten,
            BlendMode::Modulate,
            BlendMode::Overlay,
            BlendMode::Screen,
            BlendMode::Src,
            BlendMode::SrcATop,
            BlendMode::SrcIn,
            BlendMode::SrcOut,
            BlendMode::SrcOver,
            BlendMode::Xor,
        ];
        for mode in modes {
            let save_count = canvas.save();
            self.draw_tile(canvas, x_offset, y_offset, mode);
            canvas.restore_to_count(save_count);

            text_utils::draw_string(
                canvas,
                mode.name(),
                int_to_scalar(x_offset) + int_to_scalar(BITMAP_SIZE) / 2.0,
                int_to_scalar(y_offset) + int_to_scalar(BITMAP_SIZE),
                &font,
                &Paint::default(),
                Align::Center,
            );

            x_offset += 256;
            if x_offset >= 1024 {
                x_offset = 0;
                y_offset += 256;
            }
        }
    }
}

// Port of: gm/androidblendmodes.cpp#L121 (chrome/m156)
crate::def_gm!(AndroidBlendModesGM, AndroidBlendModesGm::new());
