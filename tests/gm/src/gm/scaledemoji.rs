// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/scaledemoji.cpp (chrome/m156)
//
// The non-`Test` emoji formats are resources, which the portable configuration cannot read
// (docs/design/text.md §1.2), so those GMs skip in `onDraw`. The `Test` format is the portable
// `Emoji` family (`TestSVGTypeface`).

use crate::prelude::*;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utf::next_utf8;
use skia_rust_tools::font_tool_utils::{EmojiFontFormat, emoji_sample, name_for_font_format};

// Port of: gm/scaledemoji.cpp#L24-L34 (chrome/m156)
fn make_hpos_test_blob_utf8(text: &str, font: &Font) -> Option<TextBlob> {
    let enc = TextEncoding::UTF8;
    let mut builder = TextBlobBuilder::new();
    let len = text.len();
    let glyph_count = font.count_text(&text.as_bytes()[..len], enc);
    let (glyphs, pos) = builder.alloc_run_pos_h(font, glyph_count, 0.0, None);
    let _ = font.text_to_glyphs(text.as_bytes(), enc, glyphs);
    font.get_x_pos(glyphs, pos, 0.0);
    builder.make()
}

// Port of: gm/scaledemoji.cpp#L36-L90 (chrome/m156), ScaledEmojiGM
// Port of: gm/scaledemoji.cpp#L92-L149 (chrome/m156), ScaledEmojiPosGM
// Port of: gm/scaledemoji.cpp#L151-L229 (chrome/m156), ScaledEmojiPerspectiveGM
#[derive(Clone, Copy)]
enum ScaledEmojiKind {
    Plain,
    Pos,
    Perspective,
}

struct ScaledEmojiGm {
    kind: ScaledEmojiKind,
    format: EmojiFontFormat,
    typeface: Option<Typeface>,
    sample_text: &'static str,
    /// `fStripSpacesSampleText` (the perspective GM).
    strip_spaces_sample_text: String,
}

impl ScaledEmojiGm {
    fn new(kind: ScaledEmojiKind, format: EmojiFontFormat) -> Self {
        Self {
            kind,
            format,
            typeface: None,
            sample_text: "",
            strip_spaces_sample_text: String::new(),
        }
    }
}

impl GM for ScaledEmojiGm {
    // Port of: gm/scaledemoji.cpp#L50-L52, #L101-L103, #L174-L176 (chrome/m156), getName
    fn name(&self) -> String {
        let prefix = match self.kind {
            ScaledEmojiKind::Plain => "scaledemoji_",
            ScaledEmojiKind::Pos => "scaledemojipos_",
            ScaledEmojiKind::Perspective => "scaledemojiperspective_",
        };
        format!("{prefix}{}", name_for_font_format(self.format))
    }

    // Port of: gm/scaledemoji.cpp#L54 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(1200, 1200)
    }

    // Port of: gm/scaledemoji.cpp#L48, #L99, #L161-L172 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let sample = emoji_sample(self.format);
        self.typeface = sample.typeface;
        self.sample_text = sample.sample_text;

        if let ScaledEmojiKind::Perspective = self.kind {
            let mut count = 0;
            let mut ch_ptr = self.sample_text.as_bytes();
            while !ch_ptr.is_empty() && count < 2 {
                let ch = next_utf8(&mut ch_ptr);
                if ch != i32::from(b' ') {
                    // appendUnichar
                    self.strip_spaces_sample_text
                        .push(char::from_u32(ch as u32).unwrap_or('\u{FFFD}'));
                    count += 1;
                }
            }
        }
    }

    // Port of: gm/scaledemoji.cpp#L56-L87, #L105-L144, #L178-L225 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let Some(typeface) = self.typeface.clone() else {
            *error_msg = format!(
                "Unable to instantiate emoji test font of format {}.",
                name_for_font_format(self.format)
            );
            return DrawResult::Skip;
        };

        canvas.draw_color(Color::GRAY, None);

        let paint = Paint::default();
        match self.kind {
            ScaledEmojiKind::Plain => {
                let mut font = Font::from_typeface(Some(typeface));
                font.set_edging(Edging::Alias);

                let text = self.sample_text;

                // draw text at different point sizes
                // Testing GPU bitmap path, SDF path with no scaling,
                // SDF path with scaling, path rendering with scaling
                let mut y: scalar = 0.0;
                for text_size in [70.0, 180.0, 270.0, 340.0] {
                    font.set_size(text_size);
                    let (_, metrics) = font.metrics();
                    y += -metrics.ascent;
                    canvas.draw_simple_text(
                        text.as_bytes(),
                        TextEncoding::UTF8,
                        (10.0, y),
                        &font,
                        &paint,
                    );
                    y += metrics.descent + metrics.leading;
                }
            }
            ScaledEmojiKind::Pos => {
                let mut font = Font::from_size(typeface, 12.0);
                let text = self.sample_text;

                // draw text at different point sizes
                // Testing GPU bitmap path, SDF path with no scaling,
                // SDF path with scaling, path rendering with scaling
                let mut y: scalar = 0.0;
                for text_size in [70.0, 180.0, 270.0, 340.0] {
                    font.set_size(text_size);
                    let (_, metrics) = font.metrics();
                    y += -metrics.ascent;

                    let blob = make_hpos_test_blob_utf8(text, &font);
                    if let Some(blob) = &blob {
                        // Draw with an origin.
                        canvas.draw_text_blob(blob, (10.0, y), &paint);

                        // Draw with shifted canvas.
                        canvas.save();
                        canvas.translate((750.0, 0.0));
                        canvas.draw_text_blob(blob, (10.0, y), &paint);
                        canvas.restore();
                    }

                    y += metrics.descent + metrics.leading;
                }
            }
            ScaledEmojiKind::Perspective => {
                let mut taper = Matrix::default();
                taper.set_persp_y(-0.0025);

                let font = Font::from_size(typeface, 40.0);
                let blob = make_hpos_test_blob_utf8(&self.strip_spaces_sample_text, &font);

                // draw text at different point sizes
                // Testing GPU bitmap path, SDF path with no scaling,
                // SDF path with scaling, path rendering with scaling
                let (_, metrics) = font.metrics();
                for rotate in [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0] {
                    canvas.save();
                    let mut perspective = Matrix::default();
                    perspective.post_translate((-600.0, -600.0));
                    perspective.post_concat(&taper);
                    perspective.post_rotate(rotate, None);
                    perspective.post_translate((600.0, 600.0));
                    canvas.concat(&perspective);
                    let mut y: scalar = 670.0;
                    for _ in 0..5 {
                        y += -metrics.ascent;

                        // Draw with an origin.
                        if let Some(blob) = &blob {
                            canvas.draw_text_blob(blob, (565.0, y), &paint);
                        }

                        y += metrics.descent + metrics.leading;
                    }
                    canvas.restore();
                }
            }
        }

        DrawResult::Ok
    }
}

// Port of: gm/scaledemoji.cpp#L231-L245 (chrome/m156)
crate::def_gm!(
    ScaledEmojiGM_Cbdt = "ScaledEmojiGM(ToolUtils::EmojiFontFormat::Cbdt)",
    ScaledEmojiGm::new(ScaledEmojiKind::Plain, EmojiFontFormat::Cbdt)
);
crate::def_gm!(
    ScaledEmojiPosGM_Cbdt = "ScaledEmojiPosGM(ToolUtils::EmojiFontFormat::Cbdt)",
    ScaledEmojiGm::new(ScaledEmojiKind::Pos, EmojiFontFormat::Cbdt)
);
crate::def_gm!(
    ScaledEmojiPerspectiveGM_Cbdt = "ScaledEmojiPerspectiveGM(ToolUtils::EmojiFontFormat::Cbdt)",
    ScaledEmojiGm::new(ScaledEmojiKind::Perspective, EmojiFontFormat::Cbdt)
);
crate::def_gm!(
    ScaledEmojiGM_Sbix = "ScaledEmojiGM(ToolUtils::EmojiFontFormat::Sbix)",
    ScaledEmojiGm::new(ScaledEmojiKind::Plain, EmojiFontFormat::Sbix)
);
crate::def_gm!(
    ScaledEmojiPosGM_Sbix = "ScaledEmojiPosGM(ToolUtils::EmojiFontFormat::Sbix)",
    ScaledEmojiGm::new(ScaledEmojiKind::Pos, EmojiFontFormat::Sbix)
);
crate::def_gm!(
    ScaledEmojiPerspectiveGM_Sbix = "ScaledEmojiPerspectiveGM(ToolUtils::EmojiFontFormat::Sbix)",
    ScaledEmojiGm::new(ScaledEmojiKind::Perspective, EmojiFontFormat::Sbix)
);
crate::def_gm!(
    ScaledEmojiGM_ColrV0 = "ScaledEmojiGM(ToolUtils::EmojiFontFormat::ColrV0)",
    ScaledEmojiGm::new(ScaledEmojiKind::Plain, EmojiFontFormat::ColrV0)
);
crate::def_gm!(
    ScaledEmojiPosGM_ColrV0 = "ScaledEmojiPosGM(ToolUtils::EmojiFontFormat::ColrV0)",
    ScaledEmojiGm::new(ScaledEmojiKind::Pos, EmojiFontFormat::ColrV0)
);
crate::def_gm!(
    ScaledEmojiPerspectiveGM_ColrV0 =
        "ScaledEmojiPerspectiveGM(ToolUtils::EmojiFontFormat::ColrV0)",
    ScaledEmojiGm::new(ScaledEmojiKind::Perspective, EmojiFontFormat::ColrV0)
);
crate::def_gm!(
    ScaledEmojiGM_Svg = "ScaledEmojiGM(ToolUtils::EmojiFontFormat::Svg)",
    ScaledEmojiGm::new(ScaledEmojiKind::Plain, EmojiFontFormat::Svg)
);
crate::def_gm!(
    ScaledEmojiPosGM_Svg = "ScaledEmojiPosGM(ToolUtils::EmojiFontFormat::Svg)",
    ScaledEmojiGm::new(ScaledEmojiKind::Pos, EmojiFontFormat::Svg)
);
crate::def_gm!(
    ScaledEmojiPerspectiveGM_Svg = "ScaledEmojiPerspectiveGM(ToolUtils::EmojiFontFormat::Svg)",
    ScaledEmojiGm::new(ScaledEmojiKind::Perspective, EmojiFontFormat::Svg)
);
crate::def_gm!(
    ScaledEmojiGM_Test = "ScaledEmojiGM(ToolUtils::EmojiFontFormat::Test)",
    ScaledEmojiGm::new(ScaledEmojiKind::Plain, EmojiFontFormat::Test)
);
crate::def_gm!(
    ScaledEmojiPosGM_Test = "ScaledEmojiPosGM(ToolUtils::EmojiFontFormat::Test)",
    ScaledEmojiGm::new(ScaledEmojiKind::Pos, EmojiFontFormat::Test)
);
crate::def_gm!(
    ScaledEmojiPerspectiveGM_Test = "ScaledEmojiPerspectiveGM(ToolUtils::EmojiFontFormat::Test)",
    ScaledEmojiGm::new(ScaledEmojiKind::Perspective, EmojiFontFormat::Test)
);
