// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkFontPriv.h, src/core/SkFont.cpp (the `SkFontPriv` functions)

//! `SkFontPriv`: helpers for [`Font`] that are not part of its public API.
//!
//! Not ported: `GetFontBounds`, which needs `SkTypeface::getBounds` (`onComputeBounds`, which
//! builds a scaler context from a font with no paint).

use crate::font::Font;
use crate::font_metrics::FontMetrics;
use crate::font_types::{GlyphId, TextEncoding};
use crate::matrix::Matrix;
use crate::matrix_priv::differential_area_scale;
use crate::point::Point;
use crate::rect::Rect;
use crate::scalar::{Scalars, scalar};
use crate::utf::{Unichar, count_utf8, count_utf16};

/// `SkFontPriv::ScaleFontMetrics`: multiplies every scalar metric by `scale`.
// Port of: src/core/SkFont.cpp#L342-L358 (chrome/m156)
#[doc(alias = "ScaleFontMetrics")]
pub fn scale_font_metrics(metrics: &mut FontMetrics, scale: scalar) {
    metrics.top *= scale;
    metrics.ascent *= scale;
    metrics.descent *= scale;
    metrics.bottom *= scale;
    metrics.leading *= scale;
    metrics.avg_char_width *= scale;
    metrics.max_char_width *= scale;
    metrics.x_min *= scale;
    metrics.x_max *= scale;
    metrics.x_height *= scale;
    metrics.cap_height *= scale;
    metrics.underline_thickness *= scale;
    metrics.underline_position *= scale;
    metrics.strikeout_thickness *= scale;
    metrics.strikeout_position *= scale;
}

/// `SkScalarNearlyZero` with the default tolerance `SK_ScalarNearlyZero` (1/4096).
// Port of: include/private/base/SkFloatingPoint.h (SkScalarNearlyZero, chrome/m156)
fn scalar_nearly_zero(x: scalar) -> bool {
    const SK_SCALAR_NEARLY_ZERO: scalar = 1.0 / 4096.0;
    x.abs() <= SK_SCALAR_NEARLY_ZERO
}

/// `SkFontPriv::ApproximateTransformedTextSize`: the size of text drawn through `matrix` at
/// `text_location`. Returns a negative size when the perspective scale is unusable.
// Port of: src/core/SkFont.cpp#L372-L385 (chrome/m156)
#[doc(alias = "ApproximateTransformedTextSize")]
#[must_use]
pub fn approximate_transformed_text_size(
    font: &Font,
    matrix: &Matrix,
    text_location: Point,
) -> scalar {
    if matrix.has_perspective() {
        // Approximate the scale, since it can't be read directly from a perspective matrix.
        let max_scale_sq = differential_area_scale(matrix, text_location);
        if max_scale_sq.is_finite() && !scalar_nearly_zero(max_scale_sq) {
            font.size() * max_scale_sq.sqrt()
        } else {
            -font.size()
        }
    } else {
        font.size() * matrix.max_scale()
    }
}

/// `SkFontPriv::GetFontBounds`: the typeface's bounds mapped by the font's size, scale and skew.
// Port of: src/core/SkFont.cpp#L360-L370 (chrome/m156)
#[doc(alias = "GetFontBounds")]
#[must_use]
pub fn get_font_bounds(font: &Font) -> Rect {
    let mut m = Matrix::default();
    m.set_scale((font.size() * font.scale_x(), font.size()), None);
    m.post_skew((font.skew_x(), 0.0), None);

    let (bounds, _) = m.map_rect(font.typeface().get_bounds());
    bounds
}

/// `SkFontPriv::IsFinite`: whether the font's size, scale and skew are all finite.
// Port of: src/core/SkFontPriv.h#L78-L80 (chrome/m156)
#[doc(alias = "IsFinite")]
#[must_use]
pub fn is_finite(font: &Font) -> bool {
    [font.size(), font.scale_x(), font.skew_x()].are_finite()
}

/// `SkFontPriv::CountTextElements`: the number of characters (or glyphs) in `text`. Lengths are
/// in bytes, as in C++.
// Port of: src/core/SkFont.cpp#L387-L400 (chrome/m156)
#[doc(alias = "CountTextElements")]
#[must_use]
pub fn count_text_elements(text: &[u8], encoding: TextEncoding) -> usize {
    match encoding {
        TextEncoding::UTF8 => usize::try_from(count_utf8(text)).unwrap_or(0),
        TextEncoding::UTF16 => {
            let units: Vec<u16> = text
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&[a, b]| u16::from_ne_bytes([a, b]))
                .collect();
            usize::try_from(count_utf16(&units)).unwrap_or(0)
        }
        TextEncoding::UTF32 => text.len() >> 2,
        TextEncoding::GlyphId => text.len() >> 1,
    }
}

/// `SkFontPriv::GlyphsToUnichars`: the unichar of each glyph, with 0xFFFD for glyphs the typeface
/// does not have.
// Port of: src/core/SkFont.cpp#L402-L416 (chrome/m156)
#[doc(alias = "GlyphsToUnichars")]
pub fn glyphs_to_unichars(font: &Font, glyphs: &[GlyphId], text: &mut [Unichar]) {
    if glyphs.is_empty() {
        return;
    }
    let typeface = font.typeface();
    let num_glyphs_in_typeface = u32::try_from(typeface.count_glyphs()).unwrap_or(0);
    let mut unichars = vec![0; num_glyphs_in_typeface as usize];
    typeface.glyph_to_unicode_map(&mut unichars);
    for (out, &glyph) in text.iter_mut().zip(glyphs) {
        let id = u32::from(glyph);
        *out = if id < num_glyphs_in_typeface {
            unichars[id as usize]
        } else {
            0xFFFD
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeface::Typeface;

    #[test]
    fn text_elements_are_counted_per_encoding() {
        // "aé" in UTF-8 is three bytes and two characters.
        assert_eq!(count_text_elements("aé".as_bytes(), TextEncoding::UTF8), 2);
        let utf16: Vec<u8> = [0x61u16, 0x00E9]
            .iter()
            .flat_map(|u| u.to_ne_bytes())
            .collect();
        assert_eq!(count_text_elements(&utf16, TextEncoding::UTF16), 2);
        assert_eq!(count_text_elements(&[0; 12], TextEncoding::UTF32), 3);
        assert_eq!(count_text_elements(&[0; 5], TextEncoding::GlyphId), 2);
    }

    #[test]
    fn metrics_scale_and_unknown_glyphs_map_to_replacement() {
        let mut metrics = FontMetrics {
            ascent: -10.0,
            descent: 2.0,
            ..FontMetrics::default()
        };
        scale_font_metrics(&mut metrics, 0.5);
        assert_eq!((metrics.ascent, metrics.descent), (-5.0, 1.0));

        // The empty typeface has no glyphs, so every glyph is unknown.
        let font = Font::from_typeface(Some(Typeface::empty()));
        let mut text = [0; 2];
        glyphs_to_unichars(&font, &[1, 7], &mut text);
        assert_eq!(text, [0xFFFD, 0xFFFD]);
    }

    #[test]
    fn text_size_under_a_scale_matrix() {
        let font = Font::from_size(Typeface::empty(), 12.0);
        let matrix = Matrix::scale((2.0, 2.0));
        assert_eq!(
            approximate_transformed_text_size(&font, &matrix, Point::new(0.0, 0.0)),
            24.0
        );
    }
}
