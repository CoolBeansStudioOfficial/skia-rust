// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/utils/SkTextUtils.cpp and include/utils/SkTextUtils.h (chrome/m156).
// `SkTextUtils::GetPath` is not ported yet (it needs `SkPathBuilder::addPath` on glyph paths,
// T15b with `ToolUtils::get_text_path`).

//! `SkTextUtils`: draws a string aligned to its origin, as a text blob.

use crate::canvas::Canvas;
use crate::font::Font;
use crate::font_types::TextEncoding;
use crate::paint::Paint;
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
