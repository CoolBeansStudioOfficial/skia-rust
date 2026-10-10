// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/coloremoji.cpp (chrome/m156)
//
// The non-`Test` emoji formats are resources, which the portable configuration cannot read
// (docs/design/text.md §1.2), so those GMs skip after drawing gray. The `Test` format is the
// portable `Emoji` family (`TestSVGTypeface`).

// The int-to-scalar casts of small constants mirror the C++ arithmetic of the GM.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_priv::ColorConverter;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, shaders};
use skia_rust_effects::image_filters::{blur, color_filter};
use skia_rust_tools::font_tool_utils::{
    EmojiFontFormat, EmojiTestSample, emoji_sample, name_for_font_format,
};

/// Spits out an arbitrary gradient to test blur with shader on paint.
// Port of: gm/coloremoji.cpp#L46-L53 (chrome/m156)
fn make_linear() -> Option<Shader> {
    let k_pts = [Point::new(0.0, 0.0), Point::new(32.0, 32.0)];
    let k_pos: [scalar; 3] = [0.0, 1.0 / 2.0, 1.0];
    let k_colors: [Color; 3] = [
        Color::new(0x80F0_0080),
        Color::new(0xF0F0_8000),
        Color::new(0x8000_80F0),
    ];
    let conv = ColorConverter::new(&k_colors);
    shaders::linear_gradient(
        (k_pts[0], k_pts[1]),
        &Gradient::new(
            Colors::new(conv.colors4f(), Some(&k_pos), TileMode::Clamp, None),
            skia_rust_effects::gradient::Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/coloremoji.cpp#L55-L64 (chrome/m156)
fn make_grayscale(input: Option<ImageFilter>) -> Option<ImageFilter> {
    let mut matrix = [0.0_f32; 20];
    matrix[0] = 0.2126;
    matrix[5] = 0.2126;
    matrix[10] = 0.2126;
    matrix[1] = 0.7152;
    matrix[6] = 0.7152;
    matrix[11] = 0.7152;
    matrix[2] = 0.0722;
    matrix[7] = 0.0722;
    matrix[12] = 0.0722;
    matrix[18] = 1.0;
    let filter = color_filters::matrix_row_major(&matrix, Clamp::Yes);
    color_filter(filter, input, None)
}

// Port of: gm/coloremoji.cpp#L66-L68 (chrome/m156)
fn make_blur(amount: f32, input: Option<ImageFilter>) -> Option<ImageFilter> {
    blur(amount, amount, TileMode::Decal, input, None)
}

// Port of: gm/coloremoji.cpp#L70-L73 (chrome/m156)
fn make_color_filter() -> Option<ColorFilter> {
    color_filters::lighting(
        Color::from_rgb(0x00, 0x80, 0xFF),
        Color::from_rgb(0xFF, 0x20, 0x00),
    )
}

// Port of: gm/coloremoji.cpp#L80-L233 (chrome/m156), ColorEmojiGM
struct ColorEmojiGm {
    format: EmojiFontFormat,
    emoji_font: Option<EmojiTestSample>,
}

impl ColorEmojiGm {
    // Port of: gm/coloremoji.cpp#L82 (chrome/m156), the constructor
    fn new(format: EmojiFontFormat) -> Self {
        Self {
            format,
            emoji_font: None,
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
        self.emoji_font = Some(emoji_sample(self.format));
    }

    // Port of: gm/coloremoji.cpp#L112-L233 (chrome/m156), onDraw
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        canvas.draw_color(Color::GRAY, None);

        let Some(emoji_font) = self.emoji_font.clone() else {
            return DrawResult::Fail;
        };
        let Some(typeface) = emoji_font.typeface.clone() else {
            *error_msg = format!(
                "Unable to instantiate emoji test font of format {}.",
                name_for_font_format(self.format)
            );
            return DrawResult::Skip;
        };

        let mut font = Font::from_typeface(Some(typeface));
        let text = emoji_font.sample_text;
        let text_bytes = text.as_bytes();

        // draw text at different point sizes
        let text_sizes: [scalar; 3] = [10.0, 30.0, 50.0];
        let mut y: scalar = 0.0;
        for fake_bold in [false, true] {
            font.set_embolden(fake_bold);
            for &text_size in &text_sizes {
                font.set_size(text_size);
                let (_, metrics) = font.metrics();
                y += -metrics.ascent;
                canvas.draw_simple_text(
                    text_bytes,
                    TextEncoding::UTF8,
                    (10.0, y),
                    &font,
                    &Paint::default(),
                );
                y += metrics.descent + metrics.leading;
            }
        }

        // draw one more big one to max out one Plot
        font.set_size(256.0);
        let (_, metrics) = font.metrics();
        canvas.draw_simple_text(
            text_bytes,
            TextEncoding::UTF8,
            (190.0, -metrics.ascent),
            &font,
            &Paint::default(),
        );

        y += 20.0;
        let saved_y = y;
        // draw with shaders and image filters
        for make_linear_i in 0..2 {
            for make_blur_i in 0..2 {
                for make_gray_i in 0..2 {
                    for make_mode in 0..2 {
                        for alpha in 0..2 {
                            let mut shader_font =
                                Font::from_typeface(Some(font.typeface().clone()));
                            let mut shader_paint = Paint::default();
                            if make_linear_i != 0 {
                                shader_paint.set_shader(make_linear());
                            }

                            if make_blur_i != 0 && make_gray_i != 0 {
                                let gray_scale = make_grayscale(None);
                                let blur = make_blur(3.0, gray_scale);
                                shader_paint.set_image_filter(blur);
                            } else if make_blur_i != 0 {
                                shader_paint.set_image_filter(make_blur(3.0, None));
                            } else if make_gray_i != 0 {
                                shader_paint.set_image_filter(make_grayscale(None));
                            }
                            if make_mode != 0 {
                                shader_paint.set_color_filter(make_color_filter());
                            }
                            if alpha != 0 {
                                shader_paint.set_alpha_f(0.5);
                            }
                            shader_font.set_size(30.0);
                            let (_, metrics) = shader_font.metrics();
                            y += -metrics.ascent;
                            canvas.draw_simple_text(
                                text_bytes,
                                TextEncoding::UTF8,
                                (380.0, y),
                                &shader_font,
                                &shader_paint,
                            );
                            y += metrics.descent + metrics.leading;
                        }
                    }
                }
            }
        }
        // setup work needed to draw text with different clips
        canvas.translate((10.0, saved_y));
        font.set_size(40.0);

        // compute the bounds of the text
        let (_, bounds) = font.measure_text(text_bytes, TextEncoding::UTF8, None);

        let bounds_half_width = bounds.width() * 0.5;
        let bounds_half_height = bounds.height() * 0.5;
        let bounds_quarter_width = bounds_half_width * 0.5;
        let bounds_quarter_height = bounds_half_height * 0.5;

        let upper_left_clip = Rect::from_xywh(
            bounds.left(),
            bounds.top(),
            bounds_half_width,
            bounds_half_height,
        );
        let lower_right_clip = Rect::from_xywh(
            bounds.center_x(),
            bounds.center_y(),
            bounds_half_width,
            bounds_half_height,
        );
        let mut interior_clip = bounds;
        interior_clip.inset((bounds_quarter_width, bounds_quarter_height));

        let clip_rects = [bounds, upper_left_clip, lower_right_clip, interior_clip];

        let mut clip_hairline = Paint::default();
        clip_hairline.set_color(Color::WHITE);
        clip_hairline.set_style(Style::Stroke);

        let mut paint = Paint::default();
        for clip_rect in &clip_rects {
            canvas.translate((0.0, bounds.height()));
            canvas.save();
            canvas.draw_rect(clip_rect, &clip_hairline);
            paint.set_alpha(0x20);
            canvas.draw_simple_text(text_bytes, TextEncoding::UTF8, (0.0, 0.0), &font, &paint);
            canvas.clip_rect(clip_rect, None, None);
            paint.set_alpha_f(1.0);
            canvas.draw_simple_text(text_bytes, TextEncoding::UTF8, (0.0, 0.0), &font, &paint);
            canvas.restore();
            canvas.translate((0.0, 25.0));
        }

        DrawResult::Ok
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
// Port of: gm/coloremoji.cpp#L239 (chrome/m156)
crate::def_gm!(
    ColorEmojiGM_Test = "ColorEmojiGM(ToolUtils::EmojiFontFormat::Test)",
    ColorEmojiGm::new(EmojiFontFormat::Test)
);
// Port of: gm/coloremoji.cpp#L240 (chrome/m156)
crate::def_gm!(
    ColorEmojiGM_Svg = "ColorEmojiGM(ToolUtils::EmojiFontFormat::Svg)",
    ColorEmojiGm::new(EmojiFontFormat::Svg)
);
