// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/coloremoji_blendmodes.cpp (chrome/m156)
//
// The non-`Test` emoji formats are resources, which the portable configuration cannot read
// (docs/design/text.md §1.2), so those GMs skip in `onDraw`. The `Test` format is the portable
// `Emoji` family (`TestSVGTypeface`).

// The int-to-scalar casts of small constants mirror the C++ arithmetic of the GM.
#![allow(clippy::cast_precision_loss)]
// Single-letter names mirror the C++ GM (w, h, x, y).
#![allow(clippy::many_single_char_names)]

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::font::Font;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utf::next_utf8;
use skia_rust_core::utils::text_utils::{self, Align};
use skia_rust_tools::font_tool_utils::{
    EmojiFontFormat, EmojiTestSample, create_portable_typeface, default_portable_typeface,
    emoji_sample, name_for_font_format,
};

// Port of: gm/coloremoji_blendmodes.cpp#L36 (chrome/m156)
const G_DATA: [u32; 4] = [0xFFFF_FFFF, 0xFFCC_CCCC, 0xFFCC_CCCC, 0xFFFF_FFFF];

// Port of: gm/coloremoji_blendmodes.cpp#L38-L168 (chrome/m156), ColorEmojiBlendModesGM
struct ColorEmojiBlendModesGm {
    bg: Bitmap,
    format: EmojiFontFormat,
    color_sample: Option<EmojiTestSample>,
}

impl ColorEmojiBlendModesGm {
    const W: i32 = 64;
    const H: i32 = 64;

    // Port of: gm/coloremoji_blendmodes.cpp#L44 (chrome/m156), the constructor
    fn new(format: EmojiFontFormat) -> Self {
        Self {
            bg: Bitmap::new(),
            format,
            color_sample: None,
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

    // Port of: gm/coloremoji_blendmodes.cpp#L47-L66 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        // The sweep gradient paint of the C++ is never used, so it is not made.
        let orig = create_portable_typeface(Some("serif"), FontStyle::bold());
        let _ = orig;
        self.color_sample = Some(emoji_sample(self.format));

        let info = ImageInfo::new(
            (2, 2),
            skia_rust_core::color_type::ColorType::RGBA8888,
            skia_rust_core::alpha_type::AlphaType::Opaque,
            None,
        );
        assert!(self.bg.install_pixels(
            &info,
            G_DATA.iter().flat_map(|v| v.to_ne_bytes()).collect::<Vec<u8>>(),
            8,
        ));
    }

    // Port of: gm/coloremoji_blendmodes.cpp#L73-L160 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let Some(color_sample) = self.color_sample.clone() else {
            return DrawResult::Fail;
        };
        let Some(typeface) = color_sample.typeface.clone() else {
            *error_msg = format!(
                "Unable to instantiate emoji test font of format {}.",
                name_for_font_format(self.format)
            );
            return DrawResult::Skip;
        };

        canvas.translate((10.0, 20.0));

        const MODES: [BlendMode; 29] = [
            BlendMode::Clear,
            BlendMode::Src,
            BlendMode::Dst,
            BlendMode::SrcOver,
            BlendMode::DstOver,
            BlendMode::SrcIn,
            BlendMode::DstIn,
            BlendMode::SrcOut,
            BlendMode::DstOut,
            BlendMode::SrcATop,
            BlendMode::DstATop,
            BlendMode::Xor,
            BlendMode::Plus,
            BlendMode::Modulate,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::Darken,
            BlendMode::Lighten,
            BlendMode::ColorDodge,
            BlendMode::ColorBurn,
            BlendMode::HardLight,
            BlendMode::SoftLight,
            BlendMode::Difference,
            BlendMode::Exclusion,
            BlendMode::Multiply,
            BlendMode::Hue,
            BlendMode::Saturation,
            BlendMode::Color,
            BlendMode::Luminosity,
        ];

        let w = Self::W as scalar;
        let h = Self::H as scalar;
        let m = Matrix::scale((6.0, 6.0));
        let s = self.bg.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            SamplingOptions::default(),
            &m,
        );

        let label_font = Font::from_typeface(Some(default_portable_typeface()));

        let mut text_p = Paint::default();
        text_p.set_anti_alias(true);
        let text_font = Font::from_size(typeface, 70.0);

        let k_wrap = 5;

        let x0: scalar = 0.0;
        let y0: scalar = 0.0;
        let mut x = x0;
        let mut y = y0;
        for (i, &mode) in MODES.iter().enumerate() {
            let mut r = Rect::from_ltrb(x, y, x + w, y + h);

            let mut p = Paint::default();
            p.set_style(Style::Fill);
            p.set_shader(s.clone());
            canvas.draw_rect(r, &p);

            r.inset((-0.5, -0.5));
            p.set_style(Style::Stroke);
            p.set_shader(None);
            canvas.draw_rect(r, &p);

            {
                let save_count = canvas.save_count();
                canvas.save();
                canvas.clip_rect(r, None, None);
                text_p.set_blend_mode(mode);
                let mut text = color_sample.sample_text.as_bytes();
                let unichar = next_utf8(&mut text);
                debug_assert!(unichar >= 0);
                canvas.draw_simple_text(
                    unichar.to_ne_bytes(),
                    TextEncoding::UTF32,
                    (x + w / 10.0, y + 7.0 * h / 8.0),
                    &text_font,
                    &text_p,
                );
                canvas.restore_to_count(save_count);
            }
            let label = mode.name();
            text_utils::draw_string(
                canvas,
                label,
                x + w / 2.0,
                y - label_font.size() / 2.0,
                &label_font,
                &Paint::default(),
                Align::Center,
            );
            x += w + 10.0;
            if i % k_wrap == k_wrap - 1 {
                x = x0;
                y += h + 30.0;
            }
        }

        DrawResult::Ok
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
// Port of: gm/coloremoji_blendmodes.cpp#L184 (chrome/m156)
crate::def_gm!(
    ColorEmojiBlendModesGM_Test = "ColorEmojiBlendModesGM(ToolUtils::EmojiFontFormat::Test)",
    ColorEmojiBlendModesGm::new(EmojiFontFormat::Test)
);
// Port of: gm/coloremoji_blendmodes.cpp#L185 (chrome/m156)
crate::def_gm!(
    ColorEmojiBlendModesGM_Svg = "ColorEmojiBlendModesGM(ToolUtils::EmojiFontFormat::Svg)",
    ColorEmojiBlendModesGm::new(EmojiFontFormat::Svg)
);
