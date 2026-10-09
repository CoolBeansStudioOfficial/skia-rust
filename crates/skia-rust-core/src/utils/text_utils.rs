// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/utils/SkTextUtils.cpp and include/utils/SkTextUtils.h (chrome/m156).
// `SkTextUtils::GetPath` is ported (`get_path`) on the glyph outlines of `SkFont::getPaths`.

//! `SkTextUtils`: draws a string aligned to its origin, as a text blob.

use crate::canvas::Canvas;
use crate::font::Font;
use crate::font_types::TextEncoding;
use crate::paint::Paint;
use crate::path::Path;
use crate::path_builder::PathBuilder;
use crate::point::Point;
use crate::scalar::scalar;
use crate::text_blob::TextBlob;

/// Where the text sits relative to its origin (`SkTextUtils::Align`).
#[doc(alias = "SkTextUtils::Align")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// The origin is the start of the text (`kLeft_Align`).
    #[default]
    Left,
    /// The origin is the middle of the text (`kCenter_Align`).
    Center,
    /// The origin is the end of the text (`kRight_Align`).
    Right,
}

/// `SkTextUtils::Draw(canvas, text, size, encoding, x, y, font, paint, align)`: draws `text` as
/// a text blob, moved left by its width for the centered and right alignments.
// Port of: src/utils/SkTextUtils.cpp#L14-L29 (chrome/m156)
#[doc(alias = "SkTextUtils::Draw")]
#[allow(clippy::too_many_arguments)] // mirrors SkTextUtils::Draw, which takes as many
pub fn draw(
    canvas: &Canvas,
    text: &[u8],
    encoding: TextEncoding,
    x: scalar,
    y: scalar,
    font: &Font,
    paint: &Paint,
    align: Align,
) {
    let mut x = x;
    if align != Align::Left {
        let (mut width, _) = font.measure_text(text, encoding, None);
        if align == Align::Center {
            width *= 0.5;
        }
        x -= width;
    }

    if let Some(blob) = TextBlob::from_text(text, encoding, font) {
        canvas.draw_text_blob(&blob, (x, y), paint);
    }
}

/// `SkTextUtils::DrawString(canvas, text, x, y, font, paint, align)`: draws the UTF-8 `text`.
// Port of: include/utils/SkTextUtils.h#L24-L27 (chrome/m156)
#[doc(alias = "SkTextUtils::DrawString")]
pub fn draw_string(
    canvas: &Canvas,
    text: impl AsRef<str>,
    x: scalar,
    y: scalar,
    font: &Font,
    paint: &Paint,
    align: Align,
) {
    draw(
        canvas,
        text.as_ref().as_bytes(),
        TextEncoding::UTF8,
        x,
        y,
        font,
        paint,
        align,
    );
}

/// `SkTextUtils::GetPath`: the outline of `text` drawn at `(x, y)` with `font`. The glyph outlines
/// are appended in order, each moved to its glyph position; glyphs without an outline (spaces)
/// add nothing.
// Port of: src/utils/SkTextUtils.cpp#L39-L60 (chrome/m156)
#[doc(alias = "GetPath")]
#[must_use]
pub fn get_path(text: &[u8], encoding: TextEncoding, x: scalar, y: scalar, font: &Font) -> Path {
    // SkAutoToGlyphs: the glyphs of the text.
    let count = font.count_text(text, encoding);
    let mut glyphs = vec![0; count];
    let count = font.text_to_glyphs(text, encoding, &mut glyphs);
    glyphs.truncate(count);

    // font.getPos(ag.glyphs(), pos, {x, y})
    let mut pos = vec![Point::default(); glyphs.len()];
    font.get_pos(&glyphs, &mut pos, Point::new(x, y));

    let mut dst = PathBuilder::new();
    let mut index = 0;
    font.get_paths(&glyphs, |src, mx| {
        if let Some(src) = src {
            let mut m = mx.clone();
            m.post_translate((pos[index].x, pos[index].y));
            dst.add_path_with_transform(src, &m, None);
        }
        index += 1;
    });
    dst.detach()
}
