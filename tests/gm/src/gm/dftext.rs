// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/dftext.cpp (chrome/m156)
//
// The raster sink has no recording context or Graphite recorder, so the offscreen surface of
// Skia's `surface ? surface->getCanvas() : inputCanvas` is never made and the GM draws straight
// on the input canvas. Its `kUseDeviceIndependentFonts` props only matter to that surface.

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_tools::font_tool_utils::{
    EmojiTestSample, create_portable_typeface, emoji_sample_default,
};

struct DfTextGm {
    // `fEmojiSample`, made in onOnceBeforeDraw.
    emoji_sample: Option<EmojiTestSample>,
}

impl DfTextGm {
    fn new() -> Self {
        DfTextGm { emoji_sample: None }
    }
}

// The perspective matrix of the GM's two perspective blocks (`persp.setAll(...)`).
// Port of: gm/dftext.cpp#L205-L208 (chrome/m156)
fn perspective_matrix() -> Matrix {
    let mut persp = Matrix::new_identity();
    persp.set_all(
        0.9839,
        0.0,
        0.0, //
        0.2246,
        0.6829,
        0.0, //
        0.000_235_2,
        -0.000_384_4,
        1.0,
    );
    persp
}

impl GM for DfTextGm {
    fn name(&self) -> String {
        "dftext".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1024, 768)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFFF_FFFF)
    }

    // Port of: gm/dftext.cpp#L30 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.emoji_sample = Some(emoji_sample_default());
    }

    // Port of: gm/dftext.cpp#L36-L179 (chrome/m156), onDraw (raster path)
    // One function, as in the C++, so the sections keep their order.
    #[allow(clippy::too_many_lines)]
    fn on_draw(&mut self, input_canvas: &Canvas) {
        let text = "Hamburgefons";
        let text_bytes = text.as_bytes();
        let text_sizes: [f32; 4] = [9.0, 9.0 * 2.0, 9.0 * 5.0, 9.0 * 2.0 * 5.0];
        let scales: [f32; 4] = [2.0 * 5.0, 5.0, 2.0, 1.0];

        // init our new canvas with the old canvas's matrix
        let canvas = input_canvas;
        canvas.set_matrix(&M44::from(input_canvas.local_to_device_as_3x3()));
        // apply global scale to test glyph positioning
        canvas.scale((1.05, 1.05));
        canvas.clear(Color::from(0xFFFF_FFFF));

        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        let mut font = Font::from_size(
            create_portable_typeface(Some("serif"), FontStyle::default()),
            12.0,
        );
        font.set_subpixel(true);

        // check scaling up
        let mut x = 0.0_f32;
        let mut y = 78.0_f32;
        for i in 0..text_sizes.len() {
            let acr = AutoCanvasRestore::guard(canvas, true);
            acr.translate((x, y));
            acr.scale((scales[i], scales[i]));
            font.set_size(text_sizes[i]);
            acr.draw_simple_text(text_bytes, TextEncoding::UTF8, (0.0, 0.0), &font, &paint);
            y += font.metrics().0 * scales[i];
        }

        // check rotation
        for i in 0..5_i32 {
            let rot_x = 10.0_f32;
            let mut rot_y = y;
            let acr = AutoCanvasRestore::guard(canvas, true);
            acr.translate((int_to_scalar(10 + i * 200), -80.0));
            acr.rotate(int_to_scalar(i * 5), Some(Point::new(rot_x, rot_y)));
            for ps in (6..=32).step_by(3) {
                font.set_size(int_to_scalar(ps));
                acr.draw_simple_text(
                    text_bytes,
                    TextEncoding::UTF8,
                    (rot_x, rot_y),
                    &font,
                    &paint,
                );
                rot_y += font.metrics().0;
            }
        }

        // check scaling down
        font.set_edging(Edging::SubpixelAntiAlias);
        x = 680.0;
        y = 20.0;
        let array_size = text_sizes.len();
        for i in 0..array_size {
            let acr = AutoCanvasRestore::guard(canvas, true);
            acr.translate((x, y));
            let scale_factor = 1.0 / scales[array_size - i - 1];
            acr.scale((scale_factor, scale_factor));
            font.set_size(text_sizes[i]);
            acr.draw_simple_text(text_bytes, TextEncoding::UTF8, (0.0, 0.0), &font, &paint);
            y += font.metrics().0 * scale_factor;
        }

        // check pos text
        {
            let acr = AutoCanvasRestore::guard(canvas, true);
            acr.scale((2.0, 2.0));
            let mut glyphs: Vec<GlyphId> = vec![0; text_bytes.len()];
            let count = font.text_to_glyphs(text_bytes, TextEncoding::UTF8, &mut glyphs);
            let glyphs = &glyphs[..count];
            let mut pos = vec![Point::new(0.0, 0.0); count];
            font.set_size(text_sizes[0]);
            font.get_pos(glyphs, &mut pos, Point::new(340.0, 75.0));
            if let Some(blob) = TextBlob::from_pos_glyphs(glyphs, &pos, &font) {
                acr.draw_text_blob(&blob, (0.0, 0.0), &paint);
            }
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
        paint.set_color(Color::from(0xFFF7_F3F7));
        let r = Rect::new(670.0, 215.0, 820.0, 397.0);
        canvas.draw_rect(r, &paint);

        x = 680.0;
        y = 235.0;
        font.set_size(19.0);
        for c in fg {
            paint.set_color(Color::from(c));
            canvas.draw_simple_text(text_bytes, TextEncoding::UTF8, (x, y), &font, &paint);
            y += font.metrics().0;
        }

        paint.set_color(Color::from(0xFF18_1C18));
        let r = Rect::new(820.0, 215.0, 970.0, 397.0);
        canvas.draw_rect(r, &paint);

        x = 830.0;
        y = 235.0;
        font.set_size(19.0);
        for c in fg {
            paint.set_color(Color::from(c));
            canvas.draw_simple_text(text_bytes, TextEncoding::UTF8, (x, y), &font, &paint);
            y += font.metrics().0;
        }

        // check skew
        {
            font.set_edging(Edging::AntiAlias);
            let acr = AutoCanvasRestore::guard(canvas, true);
            acr.skew((0.0, 0.151_515));
            font.set_size(32.0);
            acr.draw_simple_text(text_bytes, TextEncoding::UTF8, (745.0, 70.0), &font, &paint);
        }
        {
            font.set_edging(Edging::SubpixelAntiAlias);
            let acr = AutoCanvasRestore::guard(canvas, true);
            acr.skew((0.5, 0.0));
            font.set_size(32.0);
            acr.draw_simple_text(
                text_bytes,
                TextEncoding::UTF8,
                (580.0, 125.0),
                &font,
                &paint,
            );
        }

        // check perspective
        {
            font.set_edging(Edging::AntiAlias);
            let acr = AutoCanvasRestore::guard(canvas, true);
            acr.concat(&perspective_matrix());
            acr.translate((1100.0, -295.0));
            font.set_size(37.5);
            acr.draw_simple_text(text_bytes, TextEncoding::UTF8, (0.0, 0.0), &font, &paint);
        }
        {
            font.set_subpixel(false);
            font.set_edging(Edging::Alias);
            let acr = AutoCanvasRestore::guard(canvas, true);
            acr.concat(&perspective_matrix());
            acr.translate((1075.0, -245.0));
            acr.scale((375.0, 375.0));
            font.set_size(0.1);
            acr.draw_simple_text(text_bytes, TextEncoding::UTF8, (0.0, 0.0), &font, &paint);
        }

        // check color emoji
        if let Some(sample) = &self.emoji_sample
            && let Some(typeface) = sample.typeface.clone()
        {
            let mut emoji_font = Font::default();
            emoji_font.set_subpixel(true);
            emoji_font.set_typeface(Some(typeface));
            emoji_font.set_size(19.0);
            canvas.draw_simple_text(
                sample.sample_text.as_bytes(),
                TextEncoding::UTF8,
                (670.0, 90.0),
                &emoji_font,
                &paint,
            );
        }
    }
}

crate::def_gm!(DFTextGM, DfTextGm::new());
