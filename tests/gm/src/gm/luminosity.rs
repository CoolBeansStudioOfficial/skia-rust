// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/luminosity.cpp (chrome/m156)

#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::excessive_precision,
    clippy::float_cmp,
    clippy::inconsistent_digit_grouping,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;

// As reported in b/359049360:
// The luminosity blend mode includes a division that can behave badly (black results) when
// drawing low-alpha content over bright backgrounds. This GM reproduced the effect on several
// GPUs. Proper rendering should be various near-white colors, with no black boxes.
// Port of: gm/luminosity.cpp#L15-L41 (chrome/m156)
crate::def_simple_gm!(luminosity_overflow, canvas, 256, 256, {
    const KRGBS: [u8; 4] = [243, 247, 251, 255];
    canvas.save();
    for r in KRGBS {
        for g in KRGBS {
            for b in KRGBS {
                let mut p = Paint::default();
                p.set_argb(255, r, g, b);
                canvas.draw_rect(Rect::from_wh(4.0, 256.0), &p);
                canvas.translate((4.0, 0.0));
            }
        }
    }
    canvas.restore();

    for a in 1..=16u8 {
        let mut p = Paint::default();
        p.set_argb(a, 255, 255, 255);
        p.set_blend_mode(BlendMode::Luminosity);
        canvas.draw_rect(Rect::from_wh(256.0, 16.0), &p);
        canvas.translate((0.0, 16.0));
    }
});
