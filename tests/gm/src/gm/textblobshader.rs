// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/textblobshader.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::color::colors;
use skia_rust_core::font::Edging;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::scalar::int_to_scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_tools::font_tool_utils::default_portable_font;

// This GM exercises drawTextBlob offset vs. shader space behavior.
// Port of: gm/textblobshader.cpp#L13-L94 (chrome/m156), TextBlobShaderGM
struct TextBlobShaderGm {
    glyphs: Vec<GlyphId>,
    blob: Option<TextBlob>,
    shader: Option<Shader>,
}

impl TextBlobShaderGm {
    // Port of: gm/textblobshader.cpp#L13-L14 (chrome/m156), the constructor
    fn new() -> Self {
        Self {
            glyphs: Vec::new(),
            blob: None,
            shader: None,
        }
    }
}

impl GM for TextBlobShaderGm {
    // Port of: gm/textblobshader.cpp#L17-L58 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        {
            let font = default_portable_font();
            let txt = "Blobber";
            let count = font.count_text(txt.as_bytes(), TextEncoding::UTF8);
            self.glyphs.resize(count, 0);
            font.text_to_glyphs(txt.as_bytes(), TextEncoding::UTF8, &mut self.glyphs);
        }
        let mut font = default_portable_font();
        font.set_subpixel(true);
        font.set_edging(Edging::AntiAlias);
        font.set_size(30.0);
        let mut builder = TextBlobBuilder::new();
        let glyph_count = self.glyphs.len();
        {
            let run = builder.alloc_run(&font, glyph_count, 10.0, 10.0, None);
            run.copy_from_slice(&self.glyphs);
        }
        {
            let (glyphs, pos) = builder.alloc_run_pos_h(&font, glyph_count, 80.0, None);
            glyphs.copy_from_slice(&self.glyphs);
            for (i, p) in pos.iter_mut().enumerate() {
                *p = font.size() * index_to_scalar(i) * 0.75;
            }
        }
        {
            let (glyphs, pos) = builder.alloc_run_pos(&font, glyph_count, None);
            glyphs.copy_from_slice(&self.glyphs);
            for (i, p) in pos.iter_mut().enumerate() {
                *p = Point::new(
                    font.size() * index_to_scalar(i) * 0.75,
                    150.0 + 5.0 * (index_to_scalar(i) * 8.0 / index_to_scalar(glyph_count)).sin(),
                );
            }
        }
        self.blob = builder.make();

        let colors = [colors::RED, colors::GREEN];
        let mut pos = [0.0_f32; 2];
        let last = pos.len() - 1;
        for (i, p) in pos.iter_mut().enumerate() {
            *p = index_to_scalar(i) / index_to_scalar(last);
        }
        let sz = self.size();
        let center = Point::new(int_to_scalar(sz.width / 2), int_to_scalar(sz.height / 2));
        self.shader = shaders::radial_gradient(
            (center, int_to_scalar(sz.width) * 0.66),
            &Gradient::new(
                Colors::new(&colors, Some(&pos), TileMode::Repeat, None),
                Interpolation::default(),
            ),
            None,
        );
    }

    // Port of: gm/textblobshader.cpp#L60 (chrome/m156), getName
    fn name(&self) -> String {
        "textblobshader".to_owned()
    }

    // Port of: gm/textblobshader.cpp#L61 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    // Port of: gm/textblobshader.cpp#L63-L78 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_style(Style::Fill);
        p.set_shader(self.shader.clone());
        let sz = self.size();
        let x_count: i32 = 4;
        let y_count: i32 = 3;
        let blob = self.blob.as_ref().expect("onOnceBeforeDraw made the blob");
        for i in 0..x_count {
            for j in 0..y_count {
                canvas.draw_text_blob(
                    blob,
                    (
                        int_to_scalar(i * sz.width / x_count),
                        int_to_scalar(j * sz.height / y_count),
                    ),
                    &p,
                );
            }
        }
    }
}

// Port of: gm/textblobshader.cpp#L13-L94 (chrome/m156), the scalar conversion of C++'s implicit
// int-to-float conversions; the glyph and run counts are far below 2^24.
#[allow(clippy::cast_precision_loss)]
fn index_to_scalar(i: usize) -> f32 {
    i as f32
}

// Port of: gm/textblobshader.cpp#L94 (chrome/m156)
crate::def_gm!(TextBlobShaderGM, TextBlobShaderGm::new());
