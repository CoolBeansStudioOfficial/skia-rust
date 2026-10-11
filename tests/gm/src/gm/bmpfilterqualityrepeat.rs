// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bmpfilterqualityrepeat.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_tools::font_tool_utils::default_portable_font;

use crate::tool_utils::color_to_565;

// Port of: gm/bmpfilterqualityrepeat.cpp#L16-L104 (chrome/m156)
struct BmpFilterQualityRepeatGm {
    bmp: Bitmap,
}

impl BmpFilterQualityRepeatGm {
    fn new() -> Self {
        Self { bmp: Bitmap::new() }
    }

    // Port of: gm/bmpfilterqualityrepeat.cpp#L49-L79 (chrome/m156), drawAll
    fn draw_all(&self, canvas: &Canvas, scale_x: f32) {
        let rect = Rect::from_ltrb(20.0, 60.0, 220.0, 210.0);
        let mut lm = Matrix::new_identity();
        lm.set_scale_x(scale_x);
        lm.set_translate_x(423.0);
        lm.set_translate_y(330.0);

        let mut text_paint = Paint::default();
        text_paint.set_anti_alias(true);
        let mut bmp_paint = text_paint.clone();

        let font = default_portable_font();

        canvas.save();

        let recs: [(&str, SamplingOptions); 4] = [
            ("none", SamplingOptions::from(FilterMode::Nearest)),
            ("low", SamplingOptions::from(FilterMode::Linear)),
            (
                "medium",
                SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
            ),
            ("high", SamplingOptions::from(CubicResampler::mitchell())),
        ];
        for (name, sampling) in recs {
            let tm = TileMode::Repeat;
            bmp_paint.set_shader(self.bmp.to_shader((tm, tm), sampling, &lm));
            canvas.draw_rect(rect, &bmp_paint);
            canvas.draw_simple_text(
                name.as_bytes(),
                TextEncoding::UTF8,
                (20.0, 40.0),
                &font,
                &text_paint,
            );
            canvas.translate((250.0, 0.0));
        }

        canvas.restore();
    }
}

impl GM for BmpFilterQualityRepeatGm {
    fn name(&self) -> String {
        "bmp_filter_quality_repeat".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1000, 400)
    }

    // Port of: gm/bmpfilterqualityrepeat.cpp#L23 (chrome/m156), setBGColor
    fn bg_color(&self) -> Color {
        color_to_565(Color::from(0xFFCC_BBAA))
    }

    // Port of: gm/bmpfilterqualityrepeat.cpp#L26-L43 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.bmp.alloc_n32_pixels((40, 40), true);
        let mut color_bmp = Bitmap::new();
        {
            let canvas = Canvas::from_bitmap(&mut self.bmp, None).expect("a canvas");

            color_bmp.alloc_n32_pixels((20, 20), true);
            color_bmp.erase_color(Color::from(0xFFFF_0000));
            canvas.draw_image(&color_bmp.as_image().expect("an image"), (0.0, 0.0), None);
            color_bmp.erase_color(color_to_565(Color::from(0xFF00_8200)));
            canvas.draw_image(&color_bmp.as_image().expect("an image"), (20.0, 0.0), None);
            color_bmp.erase_color(color_to_565(Color::from(0xFFFF_9000)));
            canvas.draw_image(&color_bmp.as_image().expect("an image"), (0.0, 20.0), None);
            color_bmp.erase_color(color_to_565(Color::from(0xFF20_00FF)));
            canvas.draw_image(&color_bmp.as_image().expect("an image"), (20.0, 20.0), None);
        }
    }

    // Port of: gm/bmpfilterqualityrepeat.cpp#L84-L90 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        self.draw_all(canvas, 2.5);
        canvas.translate((0.0, 250.0));
        canvas.scale((0.5, 0.5));
        self.draw_all(canvas, 1.0);
    }
}

// Port of: gm/bmpfilterqualityrepeat.cpp#L104 (chrome/m156)
crate::def_gm!(BmpFilterQualityRepeat, BmpFilterQualityRepeatGm::new());
