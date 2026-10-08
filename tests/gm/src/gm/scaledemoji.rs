// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/scaledemoji.cpp (chrome/m156)
//
// Skip-matching only (docs/design/text.md §1.2): the non-`Test` emoji formats are resources, which
// the portable configuration cannot read, so the three GMs skip in `onDraw`. Their drawing code is
// not ported, because it can never run here; a typeface panics.

use crate::prelude::*;
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::{EmojiFontFormat, emoji_sample, name_for_font_format};

// Port of: gm/scaledemoji.cpp#L41-L90 (chrome/m156), ScaledEmojiGM
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
}

impl ScaledEmojiGm {
    fn new(kind: ScaledEmojiKind, format: EmojiFontFormat) -> Self {
        Self {
            kind,
            format,
            typeface: None,
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

    // Port of: gm/scaledemoji.cpp#L48 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.typeface = emoji_sample(self.format).typeface;
    }

    // Port of: gm/scaledemoji.cpp#L56-L60 (chrome/m156), onDraw: the skip path
    fn on_draw_with_error(&mut self, _canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        if self.typeface.is_none() {
            *error_msg = format!(
                "Unable to instantiate emoji test font of format {}.",
                name_for_font_format(self.format)
            );
            return DrawResult::Skip;
        }
        panic!("a scaled emoji typeface is never loaded in the portable configuration");
    }
}

// Port of: gm/scaledemoji.cpp#L231-L242 (chrome/m156), the non-Test formats
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
