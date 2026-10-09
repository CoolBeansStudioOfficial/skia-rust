// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/coloremoji.cpp (chrome/m156)
//
// Skip-matching only (docs/design/text.md §1.2): the non-`Test` emoji formats are resources, which
// the portable configuration cannot read, so `ColorEmojiGM::onDraw` skips after drawing gray. The
// drawing code after the skip is not ported, because it can never run here; a typeface panics.

use crate::prelude::*;
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::{EmojiFontFormat, emoji_sample, name_for_font_format};

// Port of: gm/coloremoji.cpp#L80-L118 (chrome/m156), ColorEmojiGM
struct ColorEmojiGm {
    format: EmojiFontFormat,
    typeface: Option<Typeface>,
}

impl ColorEmojiGm {
    // Port of: gm/coloremoji.cpp#L82 (chrome/m156), the constructor
    fn new(format: EmojiFontFormat) -> Self {
        Self {
            format,
            typeface: None,
        }
    }
}

impl GM for ColorEmojiGm {
    // Port of: gm/coloremoji.cpp#L90-L92 (chrome/m156), getName
    fn name(&self) -> String {
        format!("coloremoji_{}", name_for_font_format(self.format))
    }

    // Port of: gm/coloremoji.cpp#L94 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(650, 1200)
    }

    // Port of: gm/coloremoji.cpp#L86-L88 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.typeface = emoji_sample(self.format).typeface;
    }

    // Port of: gm/coloremoji.cpp#L112-L119 (chrome/m156), onDraw: the skip path
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        canvas.draw_color(Color::GRAY, None);
        if self.typeface.is_none() {
            *error_msg = format!(
                "Unable to instantiate emoji test font of format {}.",
                name_for_font_format(self.format)
            );
            return DrawResult::Skip;
        }
        panic!("a color emoji typeface is never loaded in the portable configuration");
    }
}

// Port of: gm/coloremoji.cpp#L236 (chrome/m156)
crate::def_gm!(
    ColorEmojiGM_ColrV0 = "ColorEmojiGM(ToolUtils::EmojiFontFormat::ColrV0)",
    ColorEmojiGm::new(EmojiFontFormat::ColrV0)
);
// Port of: gm/coloremoji.cpp#L237 (chrome/m156)
crate::def_gm!(
    ColorEmojiGM_Cbdt = "ColorEmojiGM(ToolUtils::EmojiFontFormat::Cbdt)",
    ColorEmojiGm::new(EmojiFontFormat::Cbdt)
);
// Port of: gm/coloremoji.cpp#L238 (chrome/m156)
crate::def_gm!(
    ColorEmojiGM_Sbix = "ColorEmojiGM(ToolUtils::EmojiFontFormat::Sbix)",
    ColorEmojiGm::new(EmojiFontFormat::Sbix)
);
// Port of: gm/coloremoji.cpp#L240 (chrome/m156)
crate::def_gm!(
    ColorEmojiGM_Svg = "ColorEmojiGM(ToolUtils::EmojiFontFormat::Svg)",
    ColorEmojiGm::new(EmojiFontFormat::Svg)
);
