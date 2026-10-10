// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/mixedtextblobs.cpp (chrome/m156)

// The size_t-to-scalar cast of the clip index mirrors the C++ arithmetic of the GM.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{scalar, scalar_floor_to_scalar};
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::{
    add_to_text_blob, create_typeface_from_resource, default_portable_typeface, planet_typeface,
};
use skia_rust_tools::resources::get_resource_as_stream;

// Port of: gm/mixedtextblobs.cpp#L29-L43 (chrome/m156)
fn draw_blob(canvas: &Canvas, blob: &TextBlob, sk_paint: &Paint, clip_rect: &Rect) {
    let mut clip_hairline = Paint::default();
    clip_hairline.set_color(Color::WHITE);
    clip_hairline.set_style(Style::Stroke);

    let mut paint = sk_paint.clone();
    canvas.save();
    canvas.draw_rect(clip_rect, &clip_hairline);
    paint.set_alpha_f(0.125);
    canvas.draw_text_blob(blob, (0.0, 0.0), &paint);
    canvas.clip_rect(clip_rect, None, None);
    paint.set_alpha_f(1.0);
    canvas.draw_text_blob(blob, (0.0, 0.0), &paint);
    canvas.restore();
}

// Port of: gm/mixedtextblobs.cpp#L45-L162 (chrome/m156), MixedTextBlobsGM
#[derive(Default)]
struct MixedTextBlobsGm {
    emoji_typeface: Option<Typeface>,
    really_big_a_typeface: Option<Typeface>,
    emoji_text: &'static str,
    blob: Option<TextBlob>,
}

impl MixedTextBlobsGm {
    const WIDTH: i32 = 1250;
    const HEIGHT: i32 = 700;
}

impl GM for MixedTextBlobsGm {
    // Port of: gm/mixedtextblobs.cpp#L51-L107 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.emoji_typeface = Some(planet_typeface());
        self.emoji_text = "\u{2641}\u{2643}";
        self.really_big_a_typeface =
            create_typeface_from_resource(get_resource_as_stream("fonts/ReallyBigA.ttf"), 0);
        if self.really_big_a_typeface.is_none() {
            self.really_big_a_typeface = Some(default_portable_typeface());
        }

        let mut builder = TextBlobBuilder::new();

        // make textblob
        // Text so large we draw as paths
        let mut font = Font::from_size(default_portable_typeface(), 385.0);
        font.set_edging(Edging::Alias);
        let mut text = "O";

        let (_, mut bounds) = font.measure_text(text.as_bytes(), TextEncoding::UTF8, None);

        let mut y_offset: scalar = bounds.height();
        add_to_text_blob(&mut builder, text, &font, 10.0, y_offset);
        let corrupted_ax: scalar = bounds.width();
        let corrupted_ay: scalar = y_offset;

        let bounds_half_width: scalar = bounds.width() * 0.5;
        let bounds_half_height: scalar = bounds.height() * 0.5;

        let x_offset: scalar = bounds_half_width;
        y_offset = bounds_half_height;

        // LCD
        font.set_size(32.0);
        font.set_edging(Edging::SubpixelAntiAlias);
        font.set_subpixel(true);
        text = "LCD!!!!!";
        (_, bounds) = font.measure_text(text.as_bytes(), TextEncoding::UTF8, None);
        add_to_text_blob(
            &mut builder,
            text,
            &font,
            x_offset - bounds.width() * 0.25,
            y_offset - bounds.height() * 0.5,
        );

        // color emoji font with large glyph
        if let Some(emoji_typeface) = &self.emoji_typeface {
            font.set_edging(Edging::Alias);
            font.set_subpixel(false);
            font.set_typeface(Some(emoji_typeface.clone()));
            // The measured bounds are unused.
            let _ = font.measure_text(self.emoji_text.as_bytes(), TextEncoding::UTF8, None);
            add_to_text_blob(&mut builder, self.emoji_text, &font, x_offset, y_offset);
        }

        // outline font with large glyph
        font.set_size(12.0);
        text = "aA";
        font.set_typeface(self.really_big_a_typeface.clone());
        add_to_text_blob(&mut builder, text, &font, corrupted_ax, corrupted_ay);
        self.blob = builder.make();
    }

    // Port of: gm/mixedtextblobs.cpp#L109 (chrome/m156), getName
    fn name(&self) -> String {
        "mixedtextblobs".to_string()
    }

    // Port of: gm/mixedtextblobs.cpp#L111 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(Self::WIDTH, Self::HEIGHT)
    }

    // Port of: gm/mixedtextblobs.cpp#L113-L148 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.draw_color(Color::GRAY, None);

        let mut paint = Paint::default();

        // setup work needed to draw text with different clips
        paint.set_color(Color::BLACK);
        canvas.translate((10.0, 40.0));

        // compute the bounds of the text and setup some clips
        let Some(blob) = &self.blob else {
            return;
        };
        let bounds = *blob.bounds();

        let bounds_half_width: scalar = bounds.width() * 0.5;
        let bounds_half_height: scalar = bounds.height() * 0.5;
        let bounds_quarter_width: scalar = bounds_half_width * 0.5;
        let bounds_quarter_height: scalar = bounds_half_height * 0.5;

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

        let count = clip_rects.len();
        for (x, clip_rect) in clip_rects.iter().enumerate() {
            draw_blob(canvas, blob, &paint, clip_rect);
            if x == (count >> 1) - 1 {
                canvas.translate((
                    scalar_floor_to_scalar(bounds.width() + 25.0),
                    -(x as scalar * scalar_floor_to_scalar(bounds.height() + 25.0)),
                ));
            } else {
                canvas.translate((0.0, scalar_floor_to_scalar(bounds.height() + 25.0)));
            }
        }
    }
}

// Port of: gm/mixedtextblobs.cpp#L168 (chrome/m156)
crate::def_gm!(MixedTextBlobsGM, MixedTextBlobsGm::default());
