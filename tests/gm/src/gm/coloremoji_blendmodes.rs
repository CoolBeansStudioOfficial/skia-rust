// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/coloremoji_blendmodes.cpp (chrome/m156)
//
// Skip-matching only (docs/design/text.md §1.2): the non-`Test` emoji formats are resources, which
// the portable configuration cannot read, so `ColorEmojiBlendModesGM::onDraw` skips before it draws
// anything. The `Test` format needs `TestSVGTypeface` (T20) and is not registered here.

use crate::prelude::*;
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::{EmojiFontFormat, emoji_sample, name_for_font_format};

// Port of: gm/coloremoji_blendmodes.cpp#L40-L46 (chrome/m156), ColorEmojiBlendModesGM
struct ColorEmojiBlendModesGm {
    format: EmojiFontFormat,
    typeface: Option<Typeface>,
}

impl ColorEmojiBlendModesGm {
    // Port of: gm/coloremoji_blendmodes.cpp#L44 (chrome/m156), the constructor
    fn new(format: EmojiFontFormat) -> Self {
        Self {
            format,
            typeface: None,
        }
    }
}

impl GM for ColorEmojiBlendModesGm {
    // Port of: gm/coloremoji_blendmodes.cpp#L67-L69 (chrome/m156), getName
    fn name(&self) -> String {
        format!(
            "coloremoji_blendmodes_{}",
            name_for_font_format(self.format)
        )
    }

    // Port of: gm/coloremoji_blendmodes.cpp#L71 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(400, 640)
    }

    // Port of: gm/coloremoji_blendmodes.cpp#L47-L66 (chrome/m156), onOnceBeforeDraw (the sample)
    fn on_once_before_draw(&mut self) {
        self.typeface = emoji_sample(self.format).typeface;
    }

    // Port of: gm/coloremoji_blendmodes.cpp#L73-L80 (chrome/m156), onDraw: the skip path
    fn on_draw_with_error(&mut self, _canvas: &Canvas, error_msg: &mut String) -> DrawResult {
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

// Port of: gm/coloremoji_blendmodes.cpp#L181 (chrome/m156)
crate::def_gm!(
    ColorEmojiBlendModesGM_ColrV0 = "ColorEmojiBlendModesGM(ToolUtils::EmojiFontFormat::ColrV0)",
    ColorEmojiBlendModesGm::new(EmojiFontFormat::ColrV0)
);
// Port of: gm/coloremoji_blendmodes.cpp#L183 (chrome/m156)
crate::def_gm!(
    ColorEmojiBlendModesGM_Cbdt = "ColorEmojiBlendModesGM(ToolUtils::EmojiFontFormat::Cbdt)",
    ColorEmojiBlendModesGm::new(EmojiFontFormat::Cbdt)
);
// Port of: gm/coloremoji_blendmodes.cpp#L182 (chrome/m156)
crate::def_gm!(
    ColorEmojiBlendModesGM_Sbix = "ColorEmojiBlendModesGM(ToolUtils::EmojiFontFormat::Sbix)",
    ColorEmojiBlendModesGm::new(EmojiFontFormat::Sbix)
);
// Port of: gm/coloremoji_blendmodes.cpp#L185 (chrome/m156)
crate::def_gm!(
    ColorEmojiBlendModesGM_Svg = "ColorEmojiBlendModesGM(ToolUtils::EmojiFontFormat::Svg)",
    ColorEmojiBlendModesGm::new(EmojiFontFormat::Svg)
);
