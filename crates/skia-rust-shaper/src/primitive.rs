// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skshaper/src/SkShaper_primitive.cpp.

//! `SkShapers::Primitive`: the primitive shaper. It maps each code point to one glyph, with no
//! substitution and no positioning beyond the advances, and breaks lines at the spaces of the
//! advance widths.

use skia_rust_core::font::Font;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::point::Vector;
use skia_rust_core::scalar::scalar;
use skia_rust_core::utf::{self, Unichar};

use crate::run_handler::{RunHandler, RunInfo};
use crate::run_iterator::{
    BiDiRunIterator, FontRunIterator, LanguageRunIterator, ScriptRunIterator,
};
use crate::shaper::{Feature, Options, Shaper, ShaperBackend};

// Port of: modules/skshaper/src/SkShaper_primitive.cpp#L63-L84 (chrome/m156),
// `is_breaking_whitespace`: the spaces at which the primitive shaper may break a line.
fn is_breaking_whitespace(c: Unichar) -> bool {
    matches!(
        c,
        0x0020 // SPACE
            // 0x00A0 // NO-BREAK SPACE
            | 0x1680 // OGHAM SPACE MARK
            | 0x180E // MONGOLIAN VOWEL SEPARATOR
            | 0x2000 // EN QUAD
            | 0x2001 // EM QUAD
            | 0x2002 // EN SPACE (nut)
            | 0x2003 // EM SPACE (mutton)
            | 0x2004 // THREE-PER-EM SPACE (thick space)
            | 0x2005 // FOUR-PER-EM SPACE (mid space)
            | 0x2006 // SIX-PER-EM SPACE
            | 0x2007 // FIGURE SPACE
            | 0x2008 // PUNCTUATION SPACE
            | 0x2009 // THIN SPACE
            | 0x200A // HAIR SPACE
            | 0x200B // ZERO WIDTH SPACE
            | 0x202F // NARROW NO-BREAK SPACE
            | 0x205F // MEDIUM MATHEMATICAL SPACE
            | 0x3000 // IDEOGRAPHIC SPACE
                     // 0xFEFF // ZERO WIDTH NO-BREAK SPACE
    )
}

// Port of: modules/skshaper/src/SkShaper_primitive.cpp#L86-L93 (chrome/m156), `SkUTF::NextUTF8`
// on the text at `at`: the code point (or -1 if invalid) and the offset past it.
fn next_utf8_at(text: &[u8], at: usize) -> (Unichar, usize) {
    let mut rest = &text[at..];
    let unichar = utf::next_utf8(&mut rest);
    (unichar, text.len() - rest.len())
}

// Port of: modules/skshaper/src/SkShaper_primitive.cpp#L95-L163 (chrome/m156), `linebreak`.
// Offsets are indices into `text` (Skia's pointers). `advance` holds one advance per code point
// of `text`. Returns the bytes consumed and the bytes of trailing whitespace among them.
fn linebreak(text: &[u8], advance: &[scalar], width: scalar) -> (usize, usize) {
    let stop = text.len();
    let start: usize = 0;
    let mut accumulated_width: scalar = 0.0;
    let mut glyph_index: usize = 0;
    let mut text_at: usize = 0;
    let mut word_start: usize = 0;
    let mut prev_ws = true;
    let mut trailing: usize = 0;
    while text_at < stop {
        let mut prev_text = text_at;
        let (uni, next) = next_utf8_at(text, text_at);
        text_at = next;
        accumulated_width += advance[glyph_index];
        glyph_index += 1;
        let curr_ws = is_breaking_whitespace(uni);
        if !curr_ws && prev_ws {
            word_start = prev_text;
        }
        prev_ws = curr_ws;
        if width < accumulated_width {
            let mut consume_whitespace = false;
            if curr_ws {
                // The previous glyphs fit: this and the following whitespace go in trailing.
                // Don't put this in trailing if it's the first thing.
                if prev_text == start {
                    prev_text = text_at;
                }
                consume_whitespace = true;
            } else if word_start != start {
                // Back up to the last whitespace that fit.
                text_at = word_start;
            } else if prev_text > start {
                // Back up to just before the glyph that didn't fit.
                text_at = prev_text;
            } else {
                // Let it overflow, and put any following whitespace in trailing.
                prev_text = text_at;
                consume_whitespace = true;
            }
            if consume_whitespace {
                let mut next_at = text_at;
                while next_at < stop {
                    let (uni, next) = next_utf8_at(text, next_at);
                    next_at = next;
                    if !is_breaking_whitespace(uni) {
                        break;
                    }
                    text_at = next_at;
                }
                trailing = text_at - prev_text;
            }
            break;
        }
    }
    (text_at - start, trailing)
}

// Port of: modules/skshaper/src/SkShaper_primitive.cpp#L165-L229 (chrome/m156), the `shape`
// overload that takes the run iterators and the features. The features and the bidi, script and
// language iterators are unused, as in Skia. `options.tracking` is in em units.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)] // mirrors SkToU32 and the size_t to SkScalar conversions
fn shape_primitive(
    utf8: &[u8],
    font_runs: &mut dyn FontRunIterator,
    options: Options,
    handler: &mut dyn RunHandler,
) {
    let mut font = Font::default();
    if !font_runs.at_end() {
        font_runs.consume();
        font = font_runs.current_font().clone();
    }
    let glyph_count = font.count_text(utf8, TextEncoding::UTF8);
    let mut glyphs: Vec<GlyphId> = vec![0; glyph_count];
    font.text_to_glyphs(utf8, TextEncoding::UTF8, &mut glyphs);
    let mut advances: Vec<scalar> = vec![0.0; glyph_count];
    font.get_widths(&glyphs, &mut advances);

    // Tracking is specified in em units; turn it into an absolute advance increment.
    let tracking_advance = options.tracking * font.size() * font.scale_x();
    #[allow(clippy::float_cmp)] // mirrors C++ `if (trackingAdvance != 0)`
    if tracking_advance != 0.0 {
        for advance in &mut advances {
            *advance += tracking_advance;
        }
    }

    let mut glyph_offset: usize = 0;
    let mut utf8_offset: usize = 0;
    let mut text = utf8;
    loop {
        let (bytes_consumed, bytes_collapsed) =
            linebreak(text, &advances[glyph_offset..], options.width);
        let bytes_visible = bytes_consumed - bytes_collapsed;
        let num_glyphs = code_points(&text[..bytes_visible]);
        let visible_width = font
            .measure_text(&text[..bytes_visible], TextEncoding::UTF8, None)
            .0;
        let info = RunInfo {
            font: &font,
            bidi_level: 0,
            script: 0,
            language: Some(""),
            advance: Vector::new(visible_width + num_glyphs as scalar * tracking_advance, 0.0),
            glyph_count: num_glyphs,
            utf8_range: utf8_offset..utf8_offset + bytes_visible,
        };

        handler.begin_line();
        if info.glyph_count > 0 {
            handler.run_info(&info);
        }
        handler.commit_run_info();
        if info.glyph_count > 0 {
            let buffer = handler.run_buffer(&info);
            let count = info.glyph_count;
            buffer
                .glyphs
                .copy_from_slice(&glyphs[glyph_offset..glyph_offset + count]);
            let mut position = buffer.point;
            for (i, slot) in buffer.positions.iter_mut().enumerate() {
                *slot = position;
                position.x += advances[i + glyph_offset];
            }
            if let Some(clusters) = buffer.clusters {
                // Each character maps to exactly one glyph.
                let mut txt_ptr: usize = 0;
                for cluster in clusters.iter_mut() {
                    *cluster = (txt_ptr + utf8_offset) as u32; // mirrors SkToU32
                    let (_, next) = next_utf8_at(text, txt_ptr);
                    txt_ptr = next;
                }
            }
            handler.commit_run_buffer(&info);
        }
        handler.commit_line();

        glyph_offset += code_points(&text[..bytes_consumed]);
        utf8_offset += bytes_consumed;
        text = &text[bytes_consumed..];
        if text.is_empty() {
            break;
        }
    }
}

// Port of: `SkUTF::CountUTF8`, as an unsigned count: the code points of `utf8`.
fn code_points(utf8: &[u8]) -> usize {
    usize::try_from(utf::count_utf8(utf8)).unwrap_or(0)
}

/// The primitive shaper: `SkShaperPrimitive`.
#[derive(Debug, Clone, Copy, Default)]
#[doc(alias = "SkShaperPrimitive")]
pub struct PrimitiveShaper;

impl ShaperBackend for PrimitiveShaper {
    fn shape(
        &self,
        utf8: &[u8],
        font_runs: &mut dyn FontRunIterator,
        _bidi_runs: &mut dyn BiDiRunIterator,
        _script_runs: &mut dyn ScriptRunIterator,
        _language_runs: &mut dyn LanguageRunIterator,
        _features: &[Feature],
        options: &Options,
        handler: &mut dyn RunHandler,
    ) {
        shape_primitive(utf8, font_runs, *options, handler);
    }
}

/// `SkShapers::Primitive::PrimitiveText()`: a [`Shaper`] with the primitive backend. The same
/// as [`Shaper::new_primitive`].
#[must_use]
#[doc(alias = "PrimitiveText")]
pub fn primitive_text() -> Shaper {
    Shaper::new_primitive()
}

#[cfg(test)]
mod tests {
    use super::linebreak;

    #[test]
    fn linebreak_moves_the_trailing_space_out_of_the_line() {
        // "ab cd" with 10-unit advances at a width of 25: "ab " does not fit, so the line is
        // "ab" and the space after it is trailing (3 bytes consumed, 1 trailing).
        assert_eq!(linebreak(b"ab cd", &[10.0; 5], 25.0), (3, 1));
        assert_eq!(linebreak(b"cd", &[10.0; 2], 25.0), (2, 0));
    }
}
