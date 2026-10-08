// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/textblob.cpp (chrome/m156)

// The GM mirrors C++ arithmetic: the glyph and position counters are small and are converted to
// scalars exactly as C++ does, and the scalar-to-int floors are SkScalarFloorToInt.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use crate::prelude::*;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::create_portable_typeface;

// Port of: gm/textblob.cpp#L13-L17 (chrome/m156), Pos
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pos {
    Default,
    Scalar,
    Point,
}

// Port of: gm/textblob.cpp#L22-L24 (chrome/m156), BlobCfg
#[derive(Clone, Copy)]
struct BlobCfg {
    count: usize,
    pos: Pos,
    scale: f32,
}

const fn cfg(count: usize, pos: Pos, scale: f32) -> BlobCfg {
    BlobCfg { count, pos, scale }
}

const D: Pos = Pos::Default;
const S: Pos = Pos::Scalar;
const P: Pos = Pos::Point;

// Port of: gm/textblob.cpp#L26-L70 (chrome/m156), blobConfigs
const BLOB_CONFIGS: [[[BlobCfg; 3]; 3]; 6] = [
    [
        [cfg(1024, D, 1.0), cfg(0, D, 0.0), cfg(0, D, 0.0)],
        [cfg(1024, S, 1.0), cfg(0, S, 0.0), cfg(0, S, 0.0)],
        [cfg(1024, P, 1.0), cfg(0, P, 0.0), cfg(0, P, 0.0)],
    ],
    [
        [cfg(4, D, 1.0), cfg(4, D, 1.0), cfg(4, D, 1.0)],
        [cfg(4, S, 1.0), cfg(4, S, 1.0), cfg(4, S, 1.0)],
        [cfg(4, P, 1.0), cfg(4, P, 1.0), cfg(4, P, 1.0)],
    ],
    [
        [cfg(4, D, 1.0), cfg(4, D, 1.0), cfg(4, S, 1.0)],
        [cfg(4, S, 1.0), cfg(4, S, 1.0), cfg(4, P, 1.0)],
        [cfg(4, P, 1.0), cfg(4, P, 1.0), cfg(4, D, 1.0)],
    ],
    [
        [cfg(4, D, 1.0), cfg(4, S, 1.0), cfg(4, P, 1.0)],
        [cfg(4, S, 1.0), cfg(4, P, 1.0), cfg(4, D, 1.0)],
        [cfg(4, P, 1.0), cfg(4, D, 1.0), cfg(4, S, 1.0)],
    ],
    [
        [cfg(4, D, 0.75), cfg(4, D, 1.0), cfg(4, S, 1.25)],
        [cfg(4, S, 0.75), cfg(4, S, 1.0), cfg(4, P, 1.25)],
        [cfg(4, P, 0.75), cfg(4, P, 1.0), cfg(4, D, 1.25)],
    ],
    [
        [cfg(4, D, 1.0), cfg(4, S, 0.75), cfg(4, P, 1.25)],
        [cfg(4, S, 1.0), cfg(4, P, 0.75), cfg(4, D, 1.25)],
        [cfg(4, P, 1.0), cfg(4, D, 0.75), cfg(4, S, 1.25)],
    ],
];

// Port of: gm/textblob.cpp#L72 (chrome/m156), kFontSize
const FONT_SIZE: f32 = 16.0;

// Port of: gm/textblob.cpp#L74-L173 (chrome/m156), TextBlobGM
struct TextBlobGm {
    text: &'static str,
    glyphs: Vec<GlyphId>,
    typeface: Option<Typeface>,
}

impl TextBlobGm {
    // Port of: gm/textblob.cpp#L110-L168 (chrome/m156), makeBlob
    fn make_blob(&self, blob_index: usize) -> Option<TextBlob> {
        let typeface = self.typeface.clone()?;
        let mut builder = TextBlobBuilder::new();
        let mut font = Font::default();
        font.set_subpixel(true);
        font.set_edging(Edging::AntiAlias);
        font.set_typeface(Some(typeface));
        for (l, row) in BLOB_CONFIGS[blob_index].iter().enumerate() {
            let mut current_glyph = 0_usize;
            for (c, cfg) in row.iter().enumerate() {
                let mut count = cfg.count;
                if count > self.glyphs.len() - current_glyph {
                    count = self.glyphs.len() - current_glyph;
                }
                if 0 == count {
                    break;
                }
                font.set_size(FONT_SIZE * cfg.scale);
                let advance_x = font.size() * 0.85;
                let advance_y = font.size() * 1.5;
                let offset = Point::new(
                    current_glyph as f32 * advance_x + c as f32 * advance_x,
                    advance_y * l as f32,
                );
                let src = &self.glyphs[current_glyph..current_glyph + count];
                match cfg.pos {
                    Pos::Default => {
                        let glyphs = builder.alloc_run(&font, count, offset.x, offset.y, None);
                        glyphs.copy_from_slice(src);
                    }
                    Pos::Scalar => {
                        let (glyphs, pos) = builder.alloc_run_pos_h(&font, count, offset.y, None);
                        glyphs.copy_from_slice(src);
                        for (i, x) in pos.iter_mut().enumerate() {
                            *x = offset.x + i as f32 * advance_x;
                        }
                    }
                    Pos::Point => {
                        let (glyphs, points) = builder.alloc_run_pos(&font, count, None);
                        glyphs.copy_from_slice(src);
                        for (i, point) in points.iter_mut().enumerate() {
                            *point = Point::new(
                                offset.x + i as f32 * advance_x,
                                offset.y + i as f32 * (advance_y / count as f32),
                            );
                        }
                    }
                }
                current_glyph += count;
            }
        }
        builder.make()
    }
}

impl GM for TextBlobGm {
    // Port of: gm/textblob.cpp#L94-L101 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let typeface = create_portable_typeface(Some("serif"), FontStyle::default());
        let font = Font::from_typeface(Some(typeface.clone()));
        let bytes = self.text.as_bytes();
        let glyph_count = font.count_text(bytes, TextEncoding::UTF8);
        self.glyphs = vec![0; glyph_count];
        font.text_to_glyphs(bytes, TextEncoding::UTF8, &mut self.glyphs);
        self.typeface = Some(typeface);
    }

    // Port of: gm/textblob.cpp#L103 (chrome/m156), getName
    fn name(&self) -> String {
        "textblob".to_owned()
    }

    // Port of: gm/textblob.cpp#L104 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    // Port of: gm/textblob.cpp#L105-L120 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        for b in 0..BLOB_CONFIGS.len() {
            let Some(blob) = self.make_blob(b) else {
                continue;
            };
            let mut p = Paint::default();
            p.set_anti_alias(true);
            let offset = Point::new((10 + 300 * (b % 2)) as f32, (20 + 150 * (b / 2)) as f32);
            canvas.draw_text_blob(&blob, offset, &p);
            p.set_color(Color::BLUE);
            p.set_style(Style::Stroke);
            let mut bx = *blob.bounds();
            bx.offset(offset);
            p.set_anti_alias(false);
            canvas.draw_rect(bx, &p);
        }
    }
}

// Port of: gm/textblob.cpp#L175 (chrome/m156), DEF_GM(return new TextBlobGM("hamburgefons");)
crate::def_gm!(
    TextBlobGM,
    TextBlobGm {
        text: "hamburgefons",
        glyphs: Vec::new(),
        typeface: None,
    }
);
