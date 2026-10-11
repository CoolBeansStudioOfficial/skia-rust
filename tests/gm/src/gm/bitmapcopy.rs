// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bitmapcopy.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::{copy_to, int_to_scalar};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/bitmapcopy.cpp#L58-L70 (chrome/m156), gColorTypes
const G_COLOR_TYPES: [ColorType; 3] = [ColorType::RGB565, ColorType::ARGB4444, ColorType::N32];

// Port of: gm/bitmapcopy.cpp#L14-L56 (chrome/m156), color_type_name
// skia-rust: only the names of the colour types this GM uses are ported (the three in
// `G_COLOR_TYPES`, and the source's N32 type); any other type panics.
fn color_type_name(color_type: ColorType) -> &'static str {
    match color_type {
        ColorType::RGB565 => "565",
        ColorType::ARGB4444 => "4444",
        t if t == ColorType::N32 => "8888",
        _ => panic!("color_type_name: this GM only names 565, 4444 and N32"),
    }
}

// Port of: gm/bitmapcopy.cpp#L74-L88 (chrome/m156), draw_checks
fn draw_checks(canvas: &skia_rust_core::canvas::Canvas, width: i32, height: i32) {
    let mut paint = Paint::default();
    paint.set_color(Color::RED);
    canvas.draw_rect(Rect::from_iwh(width / 2, height / 2), &paint);
    paint.set_color(Color::GREEN);
    canvas.draw_rect(
        Rect::from_ltrb(
            int_to_scalar(width / 2),
            0.0,
            int_to_scalar(width),
            int_to_scalar(height / 2),
        ),
        &paint,
    );
    paint.set_color(Color::BLUE);
    canvas.draw_rect(
        Rect::from_ltrb(
            0.0,
            int_to_scalar(height / 2),
            int_to_scalar(width / 2),
            int_to_scalar(height),
        ),
        &paint,
    );
    paint.set_color(Color::YELLOW);
    canvas.draw_rect(
        Rect::from_ltrb(
            int_to_scalar(width / 2),
            int_to_scalar(height / 2),
            int_to_scalar(width),
            int_to_scalar(height),
        ),
        &paint,
    );
}

// Port of: gm/bitmapcopy.cpp#L90-L157 (chrome/m156), BitmapCopyGM
struct BitmapCopyGm {
    dst: [Bitmap; 3],
}

impl BitmapCopyGm {
    fn new() -> Self {
        Self {
            dst: [Bitmap::new(), Bitmap::new(), Bitmap::new()],
        }
    }
}

impl GM for BitmapCopyGm {
    fn name(&self) -> String {
        "bitmapcopy".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(540, 330)
    }

    // Port of: gm/bitmapcopy.cpp#L93 (chrome/m156), setBGColor(0xFFDDDDDD)
    fn bg_color(&self) -> Color {
        Color::from(0xFFDD_DDDD)
    }

    // Port of: gm/bitmapcopy.cpp#L95-L113 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        let horiz_margin = 10.0_f32;
        let vert_margin = 10.0_f32;

        let mut src = Bitmap::new();
        src.alloc_n32_pixels((40, 40), true);
        {
            let canvas_tmp = Canvas::from_bitmap(&mut src, None).expect("a canvas");
            draw_checks(&canvas_tmp, 40, 40);
        }
        for (i, color_type) in G_COLOR_TYPES.iter().enumerate() {
            copy_to(&mut self.dst[i], *color_type, &src);
        }

        canvas.clear(Color::from(0xFFDD_DDDD));
        paint.set_anti_alias(true);

        let font = default_portable_font();
        let mut width = 40.0_f32;
        let mut height = 40.0_f32;
        let spacing = font.metrics().0;
        if spacing > height {
            height = spacing;
        }
        for _ in 0..G_COLOR_TYPES.len() {
            let name = color_type_name(src.color_type());
            let (text_width, _) = font.measure_text(name.as_bytes(), TextEncoding::UTF8, None);
            if text_width > width {
                width = text_width;
            }
        }

        let horiz_offset = width + horiz_margin;
        let vert_offset = height + vert_margin;

        canvas.translate((20.0, 20.0));
        for i in 0..G_COLOR_TYPES.len() {
            canvas.save();

            // Draw destination config name
            let name = color_type_name(self.dst[i].color_type());
            let (text_width, _) = font.measure_text(name.as_bytes(), TextEncoding::UTF8, None);
            let x = (width - text_width) / 2.0;
            let y = font.metrics().0 / 2.0;
            canvas.draw_simple_text(name.as_bytes(), TextEncoding::UTF8, (x, y), &font, &paint);

            // Draw destination bitmap
            canvas.translate((0.0, vert_offset));
            let x = (width - 40.0) / 2.0;
            if let Some(image) = self.dst[i].as_image() {
                canvas.draw_image_with_sampling_options(
                    &image,
                    (x, 0.0),
                    SamplingOptions::default(),
                    Some(&paint),
                );
            }
            canvas.restore();
            canvas.translate((horiz_offset, 0.0));
        }
    }
}

// Port of: gm/bitmapcopy.cpp#L157 (chrome/m156)
crate::def_gm!(BitmapCopyGM, BitmapCopyGm::new());
