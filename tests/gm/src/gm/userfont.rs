// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/userfont.cpp (chrome/m156)

// The GM mirrors C++ arithmetic: the size and spacing loops are scalar, and the rounding is
// SkScalarRoundToInt.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use skia_rust_core::color::Color;
use skia_rust_core::drawable::Drawable;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_mgr::{FontMgr, TypefaceDecoder};
use skia_rust_core::font_types::FontHinting;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{scalar, scalar_round_to_int};
use skia_rust_core::stream::MemoryStream;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_core::typeface::{SerializeBehavior, Typeface};
use skia_rust_text::utils::custom_typeface::{CustomTypefaceBuilder, FACTORY_ID, make_from_stream};
use skia_rust_tools::font_tool_utils::{default_font, default_typeface, with_typeface_decoders};

use crate::prelude::*;

/// `make_drawable(path)`: the path, drawn in green, as a drawable within its tight bounds.
// Port of: gm/userfont.cpp#L10-L20 (chrome/m156), make_drawable
fn make_drawable(path: &Path) -> Option<Drawable> {
    let bounds = path.compute_tight_bounds();
    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(bounds, false);
    let mut paint = Paint::default();
    paint.set_color(Color::from_argb(0xFF, 0x00, 0x80, 0x00));
    paint.set_anti_alias(true);
    canvas.draw_path(path, &paint);
    recorder.finish_recording_as_drawable()
}

/// `make_tf()`: a custom typeface with the first 128 glyphs of the default font, at one point.
/// Odd glyphs are drawables, even ones paths.
// Port of: gm/userfont.cpp#L22-L51 (chrome/m156), make_tf
fn make_tf() -> Option<Typeface> {
    let mut builder = CustomTypefaceBuilder::new();
    let mut font = default_font();
    let default_face = font.typeface().clone();
    let upem = default_face.units_per_em().unwrap_or(0) as scalar;
    // request a big size, to improve precision at the fontscaler level
    font.set_size(upem);
    font.set_hinting(FontHinting::None);
    // so we can scale our paths back down to 1-point
    let scale = Matrix::scale((1.0 / upem, 1.0 / upem));
    {
        let (_, metrics) = font.metrics();
        builder.set_metrics(&metrics, 1.0 / upem);
    }
    builder.set_font_style(default_face.font_style());
    // Steal the first 128 chars from the default font
    for index in 0..=127u16 {
        let glyph = font.unichar_to_glyph(i32::from(index));
        let mut widths = [0.0 as scalar];
        font.get_widths(&[glyph], &mut widths);
        let width = widths[0];
        let path = font
            .get_path(glyph)
            .unwrap_or_default()
            .make_transform(&scale);
        // we use the charcode to be our glyph index, since we have no cmap table
        if index % 2 == 1 {
            if let Some(drawable) = make_drawable(&path) {
                builder.set_glyph_drawable(
                    index,
                    width / upem,
                    drawable,
                    path.compute_tight_bounds(),
                );
            }
        } else {
            builder.set_glyph(index, width / upem, &path);
        }
    }
    builder.detach()
}

/// `round_trip(tf)`: serialized and deserialized. The decoder list is the one C++ has globally:
/// the custom typeface's decoder, with no family lookups (`MakeDeserialize(&stream, nullptr)`).
// Port of: gm/userfont.cpp#L53-L58 (chrome/m156), round_trip
fn round_trip(tf: &Typeface) -> Option<Typeface> {
    let data = tf.serialize(SerializeBehavior::IncludeDataIfLocal)?;
    let mut stream = MemoryStream::make_copy(data.as_bytes());
    let decoders = with_typeface_decoders(
        FontMgr::empty(),
        vec![TypefaceDecoder {
            factory_id: FACTORY_ID,
            make_from_stream,
        }],
    );
    Typeface::make_deserialize(&mut *stream, Some(&decoders), None)
}

/// `make_blob(tf, size, &spacing)`: the text at `size`, and the font's line spacing.
// Port of: gm/userfont.cpp#L63-L70 (chrome/m156), make_blob
fn make_blob(tf: &Typeface, size: scalar) -> (Option<TextBlob>, scalar) {
    let mut font = Font::from_size(tf.clone(), size);
    font.set_edging(Edging::AntiAlias);
    let spacing = font.metrics().0;
    (TextBlob::from_str("Typeface", &font), spacing)
}

struct UserFontGm {
    tf: Option<Typeface>,
}

impl GM for UserFontGm {
    // Port of: gm/userfont.cpp#L60-L62 (chrome/m156), onOnceBeforeDraw: the typeface makes a
    // serialization round trip
    fn on_once_before_draw(&mut self) {
        self.tf = make_tf().and_then(|tf| round_trip(&tf));
    }

    // Port of: gm/userfont.cpp#L76 (chrome/m156), getName
    fn name(&self) -> String {
        "user_typeface".to_owned()
    }

    // Port of: gm/userfont.cpp#L77 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(810, 452)
    }

    // Port of: gm/userfont.cpp#L78-L104 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let waterfall = |canvas: &Canvas, tf: &Typeface, default_face: bool| {
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            let x: scalar = 20.0;
            let mut y: scalar = 16.0;
            let mut size: scalar = 9.0;
            while size <= 100.0 {
                let (blob, spacing) = make_blob(tf, size);
                if let Some(blob) = &blob {
                    // shared baseline
                    if default_face {
                        paint.set_color(Color::from_argb(0xFF, 0xDD, 0xDD, 0xDD));
                        canvas.draw_rect(Rect::from_ltrb(0.0, y, 810.0, y + 1.0), &paint);
                    }
                    paint.set_color(Color::from_argb(0xFF, 0xCC, 0xCC, 0xCC));
                    paint.set_style(Style::Stroke);
                    canvas.draw_rect(blob.bounds().with_offset((x, y)), &paint);
                    paint.set_style(Style::Fill);
                    paint.set_color(Color::BLACK);
                    canvas.draw_text_blob(blob, (x, y), &paint);
                }
                y += scalar_round_to_int(spacing * 1.25 + 2.0) as scalar;
                size *= 1.25;
            }
        };
        waterfall(canvas, &default_typeface(), true);
        canvas.translate((400.0, 0.0));
        if let Some(tf) = &self.tf {
            waterfall(canvas, tf, false);
        }
    }
}

// Port of: gm/userfont.cpp#L107 (chrome/m156), DEF_GM
crate::def_gm!(UserFont, UserFontGm { tf: None });
