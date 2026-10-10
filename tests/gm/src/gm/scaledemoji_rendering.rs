// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/scaledemoji_rendering.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::scalar::scalar;
use skia_rust_tools::font_tool_utils::{
    EmojiFontFormat, EmojiTestSample, default_typeface, emoji_sample,
};

// Port of: gm/scaledemoji_rendering.cpp#L28-L108 (chrome/m156), ScaledEmojiRenderingGM
#[derive(Default)]
struct ScaledEmojiRenderingGm {
    font_samples: Vec<EmojiTestSample>,
}

// Port of: gm/scaledemoji_rendering.cpp#L33-L39 (chrome/m156)
const FORMATS_TO_TEST: [EmojiFontFormat; 5] = [
    EmojiFontFormat::ColrV0,
    EmojiFontFormat::Sbix,
    EmojiFontFormat::Cbdt,
    EmojiFontFormat::Test,
    EmojiFontFormat::Svg,
];

impl GM for ScaledEmojiRenderingGm {
    // Port of: gm/scaledemoji_rendering.cpp#L42-L49 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.font_samples = FORMATS_TO_TEST
            .iter()
            .map(|&format| {
                let mut sample = emoji_sample(format);
                if sample.typeface.is_none() {
                    sample.typeface = Some(default_typeface());
                }
                sample
            })
            .collect();
    }

    // Port of: gm/scaledemoji_rendering.cpp#L51 (chrome/m156), getName
    fn name(&self) -> String {
        "scaledemoji_rendering".to_string()
    }

    // Port of: gm/scaledemoji_rendering.cpp#L53 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(1200, 1200)
    }

    // Port of: gm/scaledemoji_rendering.cpp#L55-L104 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.draw_color(Color::GRAY, None);
        let mut text_paint = Paint::default();
        text_paint.set_color(Color::CYAN);

        // The bounds and advance paints are only used under `if ((false))`, so they are not made.

        let mut y: scalar = 0.0;
        for sample in &self.font_samples {
            let mut font = Font::from_typeface(sample.typeface.clone());
            font.set_edging(Edging::Alias);

            let text = sample.sample_text;

            for text_size in [70.0, 150.0] {
                font.set_size(text_size);
                let (_, metrics) = font.metrics();
                // All typefaces should support subpixel mode
                font.set_subpixel(true);

                y += -metrics.ascent;

                let mut x: scalar = 0.0;
                for fake_bold in [false, true] {
                    font.set_embolden(fake_bold);
                    let (_, bounds) =
                        font.measure_text(text.as_bytes(), TextEncoding::UTF8, Some(&text_paint));
                    canvas.draw_simple_text(
                        text.as_bytes(),
                        TextEncoding::UTF8,
                        (x, y),
                        &font,
                        &text_paint,
                    );
                    // `x += bounds.width() * 1.2` is evaluated in double and narrowed on store.
                    #[allow(clippy::cast_possible_truncation)] // mirrors the C++ narrowing
                    {
                        x = (f64::from(x) + f64::from(bounds.width()) * 1.2) as scalar;
                    }
                }
                y += metrics.descent + metrics.leading;
            }
        }
    }
}

// Port of: gm/scaledemoji_rendering.cpp#L114 (chrome/m156)
crate::def_gm!(ScaledEmojiRenderingGM, ScaledEmojiRenderingGm::default());
