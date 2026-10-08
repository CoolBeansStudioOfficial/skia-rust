// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/mac_aa_explorer.cpp (chrome/m156)
//
// `MacAAFontsGM` is compiled only under `SK_BUILD_FOR_MAC` (CoreGraphics), which the oracle build
// is not, so it has no golden and is not ported.

// The GM mirrors C++ arithmetic: scalar and integer conversions of small loop counts, the C++
// variable names (doAAA, doAAB) and one C++ function per GM body, so these lints do not apply.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::{FontHinting, TextEncoding};
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_tools::font_tool_utils::create_test_typeface;

// Port of: gm/mac_aa_explorer.cpp#L141-L189 (chrome/m156), macaa_colors
crate::def_simple_gm!(macaa_colors, canvas, 800, 500, {
    const GRAY: Color = Color::from_argb(0xFF, 0x80, 0x80, 0x80);
    let colors = [
        Color::BLACK,
        Color::WHITE,
        Color::BLACK,
        GRAY,
        Color::WHITE,
        Color::BLACK,
        Color::WHITE,
        GRAY,
    ];
    let sizes = [10.0_f32, 12.0, 15.0, 18.0, 24.0];
    let width = 200.0_f32;
    let height = 500.0_f32;
    let text = "Hamburgefons";

    // `CreateTestTypeface` falls back to the portable face, so this is never null.
    let face = create_test_typeface(Some("Times"), FontStyle::default());
    let mut font = Font::from_size(face, 12.0);

    for pair in colors.as_chunks::<2>().0 {
        canvas.save();
        let mut paint = Paint::default();
        paint.set_color(pair[1]);
        canvas.draw_rect(Rect::from_ltrb(0.0, 0.0, width, height), &paint);
        paint.set_color(pair[0]);
        let mut y = 10.0_f32;
        let x = 10.0_f32;
        for ps in sizes {
            font.set_size(ps);
            for lcd in [false, true] {
                font.set_edging(if lcd {
                    Edging::SubpixelAntiAlias
                } else {
                    Edging::AntiAlias
                });
                for hinting in [FontHinting::None, FontHinting::Normal] {
                    font.set_hinting(hinting);
                    y += font.metrics().0 + 2.0;
                    canvas.draw_simple_text(text, TextEncoding::UTF8, (x, y), &font, &paint);
                }
            }
            y += 8.0;
        }
        canvas.restore();
        canvas.translate((width, 0.0));
    }
});
