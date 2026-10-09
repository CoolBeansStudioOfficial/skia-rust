// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/fontcache.cpp (chrome/m156)
//
// GM to stress the glyph cache. The GPU-only knobs (`modifyGrContextOptions`) do not apply to a
// raster sink; the drawing is the same on both.

use crate::prelude::*;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::scalar::{int_to_scalar, scalar_ceil_to_scalar};
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::{create_portable_typeface, default_portable_font};

// Port of: gm/fontcache.cpp#L37-L41 (chrome/m156), draw_string
fn draw_string(canvas: &Canvas, text: &str, x: f32, y: f32, font: &Font) -> f32 {
    canvas.draw_str(text, (x, y), font, &Paint::default());
    x + font
        .measure_text(text.as_bytes(), TextEncoding::UTF8, None)
        .0
}

// Port of: gm/fontcache.cpp#L43-L100 (chrome/m156), FontCacheGM
struct FontCacheGm {
    allow_multiple_textures: bool,
    typefaces: Vec<Typeface>,
}

impl FontCacheGm {
    // Port of: gm/fontcache.cpp#L47-L50 (chrome/m156), the constructor
    fn new(allow_multiple_textures: bool) -> Self {
        Self {
            allow_multiple_textures,
            typefaces: Vec::new(),
        }
    }

    // Port of: gm/fontcache.cpp#L103-L146 (chrome/m156), drawText
    fn draw_text(&self, canvas: &Canvas) {
        const K_SIZES: [i32; 9] = [8, 9, 10, 11, 12, 13, 18, 20, 25];
        const K_TEXTS: [&str; 4] = [
            "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            "abcdefghijklmnopqrstuvwxyz",
            "0123456789",
            "!@#$%^&*()<>[]{}",
        ];
        let mut font = default_portable_font();
        font.set_edging(Edging::AntiAlias);
        font.set_subpixel(true);
        let mut x: f32 = 0.0;
        let mut y: f32 = 10.0;
        let mut subpixel_x: f32 = 0.0;
        let mut subpixel_y: f32 = 0.0;
        let mut offset_x = true;
        if self.allow_multiple_textures {
            canvas.scale((10.0, 10.0));
        }
        loop {
            for s in K_SIZES {
                let size = 2 * s;
                font.set_size(int_to_scalar(size));
                for typeface in &self.typefaces {
                    font.set_typeface(Some(typeface.clone()));
                    for text in K_TEXTS {
                        x = int_to_scalar(size)
                            + draw_string(canvas, text, x + subpixel_x, y + subpixel_y, &font);
                        x = scalar_ceil_to_scalar(x);
                        if x + 100.0 > K_SIZE {
                            x = 0.0;
                            y += scalar_ceil_to_scalar(int_to_scalar(size) + 3.0);
                            if y > K_SIZE {
                                return;
                            }
                        }
                    }
                }
                if offset_x {
                    subpixel_x += K_SUBPIXEL_INC;
                } else {
                    subpixel_y += K_SUBPIXEL_INC;
                }
                offset_x = !offset_x;
            }
        }
    }
}

// Port of: gm/fontcache.cpp#L14 (chrome/m156), kSize
const K_SIZE: f32 = 1280.0;

// Port of: gm/fontcache.cpp#L72-L73 (chrome/m156), kSubPixelInc
const K_SUBPIXEL_INC: f32 = 1.0 / 2.0;

impl GM for FontCacheGm {
    // Port of: gm/fontcache.cpp#L45-L46 (chrome/m156), the background colour (SK_ColorLTGRAY)
    fn bg_color(&self) -> Color {
        Color::new(0xFFCC_CCCC)
    }

    // Port of: gm/fontcache.cpp#L58-L63 (chrome/m156), getName
    fn name(&self) -> String {
        if self.allow_multiple_textures {
            "fontcache-mt".to_owned()
        } else {
            "fontcache".to_owned()
        }
    }

    // Port of: gm/fontcache.cpp#L65 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(1280, 1280)
    }

    // Port of: gm/fontcache.cpp#L66-L75 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.typefaces = vec![
            create_portable_typeface(Some("serif"), FontStyle::italic()),
            create_portable_typeface(Some("sans-serif"), FontStyle::italic()),
            create_portable_typeface(Some("serif"), FontStyle::normal()),
            create_portable_typeface(Some("sans-serif"), FontStyle::normal()),
            create_portable_typeface(Some("serif"), FontStyle::bold()),
            create_portable_typeface(Some("sans-serif"), FontStyle::bold()),
        ];
    }

    // Port of: gm/fontcache.cpp#L76-L88 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        self.draw_text(canvas);
    }
}

// Port of: gm/fontcache.cpp#L146-L149 (chrome/m156), FontCacheGM(true)
crate::def_gm!(
    FontCacheGM_true = "FontCacheGM(/*allowMultipleTextures=*/true)",
    FontCacheGm::new(true)
);
// Port of: gm/fontcache.cpp#L146-L149 (chrome/m156), FontCacheGM(false)
crate::def_gm!(
    FontCacheGM_false = "FontCacheGM(/*allowMultipleTextures=*/false)",
    FontCacheGm::new(false)
);
