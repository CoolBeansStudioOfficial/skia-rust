// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/texteffects.cpp (chrome/m156)

// The int-to-scalar and size casts mirror the C++ int arithmetic of the GM.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use crate::prelude::*;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_tools::font_tool_utils::{create_portable_typeface, default_portable_font};

// Port of: gm/texteffects.cpp#L19-L43 (chrome/m156), create_underline
fn create_underline(
    intersections: &[scalar],
    last: scalar,
    final_pos: scalar,
    u_pos: scalar,
    u_width: scalar,
    text_size: scalar,
) -> Path {
    let mut underline = PathBuilder::new();
    let mut end = last;
    let mut last = last;
    for pair in intersections.as_chunks::<2>().0 {
        let start = pair[0] - u_width;
        end = pair[1] + u_width;
        if start > last && last + text_size / 12.0 < start {
            underline.move_to((last, u_pos));
            underline.line_to((start, u_pos));
        }
        last = end;
    }
    if end < final_pos {
        underline.move_to((end, u_pos));
        underline.line_to((final_pos, u_pos));
    }
    underline.detach()
}

// Port of: gm/texteffects.cpp#L46-L87 (chrome/m156), MakeFancyBlob
fn make_fancy_blob(font: &Font, text: &[u8]) -> Option<TextBlob> {
    let glyph_count = font.count_text(text, TextEncoding::UTF8);
    let mut glyphs = vec![0; glyph_count];
    font.text_to_glyphs(text, TextEncoding::UTF8, &mut glyphs);
    let mut widths = vec![0.0; glyph_count];
    font.get_widths(&glyphs, &mut widths);

    let mut blob_builder = TextBlobBuilder::new();
    let mut glyph_index = 0;
    let mut advance: scalar = 0.0;

    // Default-positioned run.
    {
        let default_run_len = glyph_count / 3;
        let buf = blob_builder.alloc_run(font, default_run_len, advance, 0.0, None);
        buf.copy_from_slice(&glyphs[..default_run_len]);
        for _ in 0..default_run_len {
            advance += widths[glyph_index];
            glyph_index += 1;
        }
    }

    // Horizontal-positioned run.
    {
        let horizontal_run_len = glyph_count / 3;
        let (buf_glyphs, buf_pos) =
            blob_builder.alloc_run_pos_h(font, horizontal_run_len, 0.0, None);
        buf_glyphs.copy_from_slice(&glyphs[glyph_index..glyph_index + horizontal_run_len]);
        for pos in buf_pos.iter_mut() {
            *pos = advance;
            advance += widths[glyph_index];
            glyph_index += 1;
        }
    }

    // Full-positioned run.
    {
        let full_run_len = glyph_count - glyph_index;
        let (buf_glyphs, buf_pos) = blob_builder.alloc_run_pos(font, full_run_len, None);
        buf_glyphs.copy_from_slice(&glyphs[glyph_index..glyph_index + full_run_len]);
        for pos in buf_pos.iter_mut() {
            *pos = Point::new(advance, 0.0); // x offset, y offset
            advance += widths[glyph_index];
            glyph_index += 1;
        }
    }

    blob_builder.make()
}

// Port of: gm/texteffects.cpp#L89-L123 (chrome/m156), the GM body of fancyblobunderline
crate::def_simple_gm!(
    #[ignore = "see notes/gm-texteffects-fancyblobunderline.md"]
    fancyblobunderline,
    canvas,
    1480,
    1380,
    {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        let fam = ["sans-serif", "serif", "monospace"];
        let test = b"aAjJgGyY_|{-(~[,]qQ}pP}zZ";
        let blob_offset = Point::new(10.0, 80.0);

        for family in fam {
            let mut text_size: scalar = 100.0;
            while text_size > 10.0 {
                let font = Font::from_size(
                    create_portable_typeface(Some(family), FontStyle::default()),
                    text_size,
                );
                let u_width = text_size / 15.0;
                paint.set_stroke_width(u_width);
                paint.set_style(Style::Fill);

                if let Some(blob) = make_fancy_blob(&font, test) {
                    canvas.draw_text_blob(&blob, blob_offset, &paint);
                    let u_pos = u_width;
                    let bounds = [u_pos - u_width / 2.0, u_pos + u_width / 2.0];
                    let intercepts = blob.get_intercepts(bounds, Some(&paint));
                    let blob_bounds = *blob.bounds();
                    let start = blob_bounds.left;
                    let end = blob_bounds.right;
                    let underline =
                        create_underline(&intercepts, start, end, u_pos, u_width, text_size)
                            .make_offset((blob_offset.x, blob_offset.y));
                    paint.set_style(Style::Stroke);
                    canvas.draw_path(&underline, &paint);
                }
                canvas.translate((0.0, text_size * 1.3));
                text_size -= 20.0;
            }
            canvas.translate((0.0, 60.0));
        }
    }
);

// Port of: gm/texteffects.cpp#L126-L131 (chrome/m156), make_text
fn glyph_bytes(glyphs: &[GlyphId]) -> Vec<u8> {
    glyphs
        .iter()
        .flat_map(|glyph| glyph.to_ne_bytes())
        .collect()
}

// Port of: gm/texteffects.cpp#L126-L131 (chrome/m156), make_text
fn make_text(font: &Font, glyphs: &[GlyphId]) -> Option<TextBlob> {
    TextBlob::from_text(&glyph_bytes(glyphs), TextEncoding::GlyphId, font)
}

// Port of: gm/texteffects.cpp#L133-L143 (chrome/m156), make_posh
fn make_posh(font: &Font, glyphs: &[GlyphId], spacing: scalar) -> Option<TextBlob> {
    let count = glyphs.len();
    let mut xpos = vec![0.0; count];
    font.get_x_pos(glyphs, &mut xpos, 0.0);
    for (i, x) in xpos.iter_mut().enumerate().skip(1) {
        *x += spacing * i as scalar;
    }
    TextBlob::from_pos_h_glyphs(glyphs, &xpos, 0.0, font)
}

// Port of: gm/texteffects.cpp#L145-L155 (chrome/m156), make_pos
fn make_pos(font: &Font, glyphs: &[GlyphId], spacing: scalar) -> Option<TextBlob> {
    let count = glyphs.len();
    let mut pos = vec![Point::default(); count];
    font.get_pos(glyphs, &mut pos, Point::default());
    for (i, p) in pos.iter_mut().enumerate().skip(1) {
        p.x += spacing * i as scalar;
    }
    TextBlob::from_pos_glyphs(glyphs, &pos, font)
}

// Port of: gm/texteffects.cpp#L157-L168 (chrome/m156), trim_with_halo
// Widens the gaps by `margin` on each side, and drops the intervals that go away. Returns the
// new count of values.
fn trim_with_halo(intervals: &mut [scalar], count: usize, margin: scalar) -> usize {
    let mut n = count;
    let mut stop = count;
    let mut i = 0;
    intervals[i] -= margin;
    i += 1;
    while i + 1 < stop {
        intervals[i] += margin;
        intervals[i + 1] -= margin;
        if intervals[i] >= intervals[i + 1] {
            // went away
            let remaining = stop as isize - i as isize - 2;
            if remaining > 0 {
                let remaining = remaining as usize;
                intervals.copy_within(i + 2..i + 2 + remaining, i);
            }
            stop -= 2;
            n -= 2;
        } else {
            i += 2;
        }
    }
    intervals[i] += margin;
    n
}

// Port of: gm/texteffects.cpp#L170-L199 (chrome/m156), draw_blob_adorned
fn draw_blob_adorned(canvas: &Canvas, blob: &TextBlob) {
    let mut paint = Paint::default();
    canvas.draw_text_blob(blob, (0.0, 0.0), &paint);

    let yminmax = [8.0, 16.0];
    let mut intervals = blob.get_intercepts(yminmax, None);
    if intervals.is_empty() {
        return;
    }
    let count = intervals.len();
    let count = trim_with_halo(
        &mut intervals,
        count,
        ((yminmax[1] - yminmax[0]) / 2.0) * 1.5,
    );
    let y = skia_rust_core::floating_point::float_midpoint(yminmax[0], yminmax[1]);
    let end: scalar = 900.0;
    let mut builder = PathBuilder::new();
    builder.move_to((0.0, y));
    for i in (0..count).step_by(2) {
        builder.line_to((intervals[i], y));
        builder.move_to((intervals[i + 1], y));
    }
    if intervals[count - 1] < end {
        builder.line_to((end, y));
    }
    paint.set_anti_alias(true);
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(yminmax[1] - yminmax[0]);
    canvas.draw_path(&builder.detach(), &paint);
}

// Port of: gm/texteffects.cpp#L201-L225 (chrome/m156), DEF_SIMPLE_GM(textblob_intercepts)
crate::def_simple_gm!(textblob_intercepts, canvas, 940, 800, {
    let text = b"Hyjay {worlp}.";
    let mut font = default_portable_font();
    font.set_size(100.0);
    font.set_edging(Edging::AntiAlias);

    let count = font.count_text(text, TextEncoding::UTF8);
    let mut glyphs = vec![0; count];
    font.text_to_glyphs(text, TextEncoding::UTF8, &mut glyphs);

    if let Some(b0) = make_text(&font, &glyphs) {
        canvas.translate((20.0, 120.0));
        draw_blob_adorned(canvas, &b0);
    }
    let mut spacing: scalar = 0.0;
    while spacing < 30.0 {
        let b1 = make_posh(&font, &glyphs, spacing);
        let b2 = make_pos(&font, &glyphs, spacing);
        canvas.translate((0.0, 150.0));
        if let Some(b1) = b1 {
            draw_blob_adorned(canvas, &b1);
        }
        canvas.translate((0.0, 150.0));
        if let Some(b2) = b2 {
            draw_blob_adorned(canvas, &b2);
        }
        spacing += 20.0;
    }
});
