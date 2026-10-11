// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/dftext.cpp (chrome/m156)
//
// The offscreen `SkSurfaces::RenderTarget` path is GPU-only (it needs `recordingContext()` or the
// Graphite recorder, which a raster canvas never has), so on the raster sink `surface` is null and
// the GM draws straight onto the input canvas. Only that path is ported.

// The GM's small integer loop counters and sizes are converted to float the same way C++ does.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)]

use crate::prelude::*;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::{create_portable_typeface, emoji_sample_default};

// Port of: gm/dftext.cpp#L36-L229 (chrome/m156), DFTextGM
struct DFTextGm {
    emoji_typeface: Option<Typeface>,
    emoji_text: &'static str,
}

impl GM for DFTextGm {
    // Port of: gm/dftext.cpp#L46-L48 (chrome/m156), constructor setBGColor
    fn bg_color(&self) -> Color {
        Color::new(0xFFFF_FFFF)
    }

    // Port of: gm/dftext.cpp#L50 (chrome/m156), getName
    fn name(&self) -> String {
        "dftext".to_owned()
    }

    // Port of: gm/dftext.cpp#L51 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(1024, 768)
    }

    // Port of: gm/dftext.cpp#L49 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let sample = emoji_sample_default();
        self.emoji_typeface = sample.typeface;
        self.emoji_text = sample.sample_text;
    }

    // Port of: gm/dftext.cpp#L52-L229 (chrome/m156), onDraw (raster sink: the surface is null,
    // so `canvas` is the input canvas and the offscreen blit is not run)
    // One function, as in the C++: splitting the sections up would not mirror the port.
    #[allow(clippy::too_many_lines)]
    fn on_draw(&mut self, input_canvas: &Canvas) {
        let text_sizes: [f32; 4] = [9.0, 9.0 * 2.0, 9.0 * 5.0, 9.0 * 2.0 * 5.0];
        let scales: [f32; 4] = [2.0 * 5.0, 5.0, 2.0, 1.0];

        let canvas = input_canvas;
        // init our new canvas with the old canvas's matrix
        canvas.set_matrix(&M44::from(input_canvas.local_to_device_as_3x3()));
        // apply global scale to test glyph positioning
        canvas.scale((1.05, 1.05));
        canvas.clear(0xffff_ffff_u32);

        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        let mut font = Font::from_typeface(Some(create_portable_typeface(
            Some("serif"),
            FontStyle::default(),
        )));
        font.set_subpixel(true);

        let text = "Hamburgefons";
        let text_bytes = text.as_bytes();
        let enc = TextEncoding::UTF8;

        // check scaling up
        let mut x: f32 = 0.0;
        let mut y: f32 = 78.0;
        for i in 0..text_sizes.len() {
            canvas.save();
            canvas.translate((x, y));
            canvas.scale((scales[i], scales[i]));
            font.set_size(text_sizes[i]);
            canvas.draw_simple_text(text_bytes, enc, (0.0, 0.0), &font, &paint);
            y += font.metrics().0 * scales[i];
            canvas.restore();
        }

        // check rotation
        for i in 0..5 {
            let rot_x: f32 = 10.0;
            let mut rot_y: f32 = y;
            canvas.save();
            canvas.translate((10.0 + i as f32 * 200.0, -80.0));
            canvas.rotate(i as f32 * 5.0, Some(Point::new(rot_x, rot_y)));
            for ps in (6..=32).step_by(3) {
                font.set_size(ps as f32);
                canvas.draw_simple_text(text_bytes, enc, (rot_x, rot_y), &font, &paint);
                rot_y += font.metrics().0;
            }
            canvas.restore();
        }

        // check scaling down
        font.set_edging(Edging::SubpixelAntiAlias);
        x = 680.0;
        y = 20.0;
        let array_size = text_sizes.len();
        for i in 0..array_size {
            canvas.save();
            canvas.translate((x, y));
            let scale_factor = 1.0 / scales[array_size - i - 1];
            canvas.scale((scale_factor, scale_factor));
            font.set_size(text_sizes[i]);
            canvas.draw_simple_text(text_bytes, enc, (0.0, 0.0), &font, &paint);
            y += font.metrics().0 * scale_factor;
            canvas.restore();
        }

        // check pos text
        {
            canvas.save();
            canvas.scale((2.0, 2.0));
            let mut glyphs = vec![0; text_bytes.len()];
            let count = font.text_to_glyphs(text_bytes, enc, &mut glyphs);
            let mut pos = vec![Point::new(0.0, 0.0); count];
            font.set_size(text_sizes[0]);
            font.get_pos(&glyphs[..count], &mut pos, Point::new(340.0, 75.0));
            if let Some(blob) = TextBlob::from_pos_glyphs(&glyphs[..count], &pos, &font) {
                canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
            }
            canvas.restore();
        }

        // check gamma-corrected blending
        let fg: [u32; 8] = [
            0xFFFF_FFFF,
            0xFFFF_FF00,
            0xFFFF_00FF,
            0xFF00_FFFF,
            0xFFFF_0000,
            0xFF00_FF00,
            0xFF00_00FF,
            0xFF00_0000,
        ];
        paint.set_color(Color::new(0xFFF7_F3F7));
        let mut r = Rect::from_ltrb(670.0, 215.0, 820.0, 397.0);
        canvas.draw_rect(r, &paint);

        x = 680.0;
        y = 235.0;
        font.set_size(19.0);
        for color in fg {
            paint.set_color(Color::new(color));
            canvas.draw_simple_text(text_bytes, enc, (x, y), &font, &paint);
            y += font.metrics().0;
        }

        paint.set_color(Color::new(0xFF18_1C18));
        r = Rect::from_ltrb(820.0, 215.0, 970.0, 397.0);
        canvas.draw_rect(r, &paint);

        x = 830.0;
        y = 235.0;
        font.set_size(19.0);
        for color in fg {
            paint.set_color(Color::new(color));
            canvas.draw_simple_text(text_bytes, enc, (x, y), &font, &paint);
            y += font.metrics().0;
        }

        // check skew
        {
            font.set_edging(Edging::AntiAlias);
            canvas.save();
            canvas.skew((0.0, 0.151_515));
            font.set_size(32.0);
            canvas.draw_simple_text(text_bytes, enc, (745.0, 70.0), &font, &paint);
            canvas.restore();
        }
        {
            font.set_edging(Edging::SubpixelAntiAlias);
            canvas.save();
            canvas.skew((0.5, 0.0));
            font.set_size(32.0);
            canvas.draw_simple_text(text_bytes, enc, (580.0, 125.0), &font, &paint);
            canvas.restore();
        }

        // check perspective
        {
            font.set_edging(Edging::AntiAlias);
            canvas.save();
            let mut persp = Matrix::default();
            persp.set_all(
                0.9839,
                0.0,
                0.0,
                0.2246,
                0.6829,
                0.0,
                0.000_235_2,
                -0.000_384_4,
                1.0,
            );
            canvas.concat(&persp);
            canvas.translate((1100.0, -295.0));
            font.set_size(37.5);
            canvas.draw_simple_text(text_bytes, enc, (0.0, 0.0), &font, &paint);
            canvas.restore();
        }
        {
            font.set_subpixel(false);
            font.set_edging(Edging::Alias);
            canvas.save();
            let mut persp = Matrix::default();
            persp.set_all(
                0.9839,
                0.0,
                0.0,
                0.2246,
                0.6829,
                0.0,
                0.000_235_2,
                -0.000_384_4,
                1.0,
            );
            canvas.concat(&persp);
            canvas.translate((1075.0, -245.0));
            canvas.scale((375.0, 375.0));
            font.set_size(0.1);
            canvas.draw_simple_text(text_bytes, enc, (0.0, 0.0), &font, &paint);
            canvas.restore();
        }

        // check color emoji
        if let Some(typeface) = &self.emoji_typeface {
            let mut emoji_font = Font::default();
            emoji_font.set_subpixel(true);
            emoji_font.set_typeface(Some(typeface.clone()));
            emoji_font.set_size(19.0);
            canvas.draw_simple_text(
                self.emoji_text.as_bytes(),
                enc,
                (670.0, 90.0),
                &emoji_font,
                &paint,
            );
        }
    }
}

// Port of: gm/dftext.cpp#L268 (chrome/m156), DEF_GM(return new DFTextGM;)
crate::def_gm!(
    DFTextGM,
    DFTextGm {
        emoji_typeface: None,
        emoji_text: "",
    }
);
