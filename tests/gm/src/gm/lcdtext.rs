// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/lcdtext.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::{SaveLayerFlags, SaveLayerRec};
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/lcdtext.cpp#L24 (chrome/m156), LcdTextGM::kTextHeight
const TEXT_HEIGHT: scalar = 36.0;

// Port of: gm/lcdtext.cpp#L23-L58 (chrome/m156), LcdTextGM
struct LcdTextGm {
    y: scalar,
}

impl LcdTextGm {
    // Port of: gm/lcdtext.cpp#L25 (chrome/m156), fY = kTextHeight
    fn new() -> Self {
        Self { y: TEXT_HEIGHT }
    }

    // Port of: gm/lcdtext.cpp#L43-L57 (chrome/m156), drawText
    fn draw_text(
        &mut self,
        canvas: &Canvas,
        string: &str,
        subpixel_text_enabled: bool,
        lcd_render_text_enabled: bool,
    ) {
        let mut paint = Paint::default();
        paint.set_color(Color::BLACK);
        paint.set_dither(true);

        let mut font = Font::from_size(default_portable_typeface(), TEXT_HEIGHT);
        if subpixel_text_enabled {
            font.set_subpixel(true);
        }
        if lcd_render_text_enabled {
            font.set_edging(Edging::SubpixelAntiAlias);
        }
        canvas.draw_str(string, (0.0, self.y), &font, &paint);
        self.y += TEXT_HEIGHT;
    }
}

impl GM for LcdTextGm {
    fn name(&self) -> String {
        "lcdtext".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    // Port of: gm/lcdtext.cpp#L31-L41 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        self.y = TEXT_HEIGHT;
        self.draw_text(canvas, "TEXT: SubpixelTrue LCDRenderTrue", true, true);
        self.draw_text(canvas, "TEXT: SubpixelTrue LCDRenderFalse", true, false);
        self.draw_text(canvas, "TEXT: SubpixelFalse LCDRenderTrue", false, true);
        self.draw_text(canvas, "TEXT: SubpixelFalse LCDRenderFalse", false, false);
    }
}

// Port of: gm/lcdtext.cpp#L50 (chrome/m156), LcdTextSizeGM::onDraw's limit
const LCD_TEXT_SIZE_LIMIT: scalar = 48.0;

// Port of: gm/lcdtext.cpp#L66-L106 (chrome/m156), LcdTextSizeGM
struct LcdTextSizeGm;

impl GM for LcdTextSizeGm {
    fn name(&self) -> String {
        "lcdtextsize".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(320, 120)
    }

    // Port of: gm/lcdtext.cpp#L77-L105 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let lcd_text = "LCD";
        let gray_text = "GRAY";
        // (location, text size, scale, text)
        let rec: [(Point, scalar, scalar, &str); 4] = [
            (
                Point::new(10.0, 50.0),
                LCD_TEXT_SIZE_LIMIT - 1.0,
                1.0,
                lcd_text,
            ),
            (
                Point::new(160.0, 50.0),
                LCD_TEXT_SIZE_LIMIT + 1.0,
                1.0,
                gray_text,
            ),
            (
                Point::new(10.0, 100.0),
                LCD_TEXT_SIZE_LIMIT / 2.0,
                1.99,
                lcd_text,
            ),
            (
                Point::new(160.0, 100.0),
                LCD_TEXT_SIZE_LIMIT / 2.0,
                2.01,
                gray_text,
            ),
        ];
        for (loc, text_size, scale, text) in rec {
            // SkAutoCanvasRestore acr(canvas, true)
            canvas.save();
            let mut font = Font::from_size(default_portable_typeface(), text_size);
            font.set_edging(Edging::SubpixelAntiAlias);
            // ScaleAbout(canvas, scale, scale, loc.x(), loc.y())
            let mut m = Matrix::default();
            m.set_scale((scale, scale), loc);
            canvas.concat(&m);
            canvas.draw_str(text, loc, &font, &Paint::default());
            canvas.restore();
        }
    }
}

// Port of: gm/lcdtext.cpp#L108-L136 (chrome/m156), SaveLayerPreserveLCDTextGM
struct SaveLayerPreserveLcdTextGm;

impl SaveLayerPreserveLcdTextGm {
    // Port of: gm/lcdtext.cpp#L121-L135 (chrome/m156), drawText
    fn draw_text(canvas: &Canvas, string: &str, y: scalar, save_layer_flags: SaveLayerFlags) {
        let rec = SaveLayerRec::default().flags(save_layer_flags);
        canvas.save_layer(&rec);

        let mut paint = Paint::default();
        paint.set_color(Color::WHITE);
        canvas.draw_rect(
            Rect::from_xywh(0.0, y - 10.0, 640.0, TEXT_HEIGHT + 20.0),
            &paint,
        );
        paint.set_color(Color::BLACK);
        let mut font = Font::from_size(default_portable_typeface(), TEXT_HEIGHT);
        font.set_edging(Edging::SubpixelAntiAlias);
        canvas.draw_str(string, (10.0, y), &font, &paint);
        canvas.restore();
    }
}

impl GM for SaveLayerPreserveLcdTextGm {
    fn name(&self) -> String {
        "savelayerpreservelcdtext".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(620, 300)
    }

    // Port of: gm/lcdtext.cpp#L115-L120 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        Self::draw_text(
            canvas,
            "SaveLayer PreserveLCDText",
            50.0,
            SaveLayerFlags::PRESERVE_LCD_TEXT,
        );
        Self::draw_text(
            canvas,
            "SaveLayer Default (LCDText not preserved)",
            150.0,
            SaveLayerFlags::empty(),
        );
    }
}

// Port of: gm/lcdtext.cpp#L138 (chrome/m156)
crate::def_gm!(LcdTextGm_ = "LcdTextGM", LcdTextGm::new());
// Port of: gm/lcdtext.cpp#L139 (chrome/m156)
crate::def_gm!(LcdTextSizeGm_ = "LcdTextSizeGM", LcdTextSizeGm);
// Port of: gm/lcdtext.cpp#L140 (chrome/m156)
crate::def_gm!(
    SaveLayerPreserveLcdTextGm_ = "SaveLayerPreserveLCDTextGM",
    SaveLayerPreserveLcdTextGm
);
