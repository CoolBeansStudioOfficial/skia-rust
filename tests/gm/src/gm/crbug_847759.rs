// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_847759.cpp (chrome/m156)

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

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;

// This path exposed an issue in AAHairlinePathRenderer. When converting from cubics to quads
// we produced quads where the previously vertical tangents at the left and right tips of the
// squashed oval-like path became slightly non-vertical. This caused a missed pixel of AA just
// outside each tip.
// Port of: gm/crbug_847759.cpp#L13-L32 (chrome/m156)
#[allow(clippy::excessive_precision)] // literals as written in the C++ source
fn draw_crbug_847759(canvas: &Canvas) {
    let path = PathBuilder::new()
        .move_to((97.0, 374.5))
        .cubic_to(
            (97.0, 359.864_452_8),
            (155.874_548_8, 348.0),
            (228.5, 348.0),
        )
        .cubic_to(
            (301.125_451_2, 348.0),
            (360.0, 359.864_452_8),
            (360.0, 374.5),
        )
        .cubic_to(
            (360.0, 389.135_547_2),
            (301.125_451_2, 401.0),
            (228.5, 401.0),
        )
        .cubic_to((155.874_548_8, 401.0), (97.0, 389.135_547_2), (97.0, 374.5))
        .close()
        .detach();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_stroke_width(0.0);
    paint.set_stroke_miter(1.5);
    paint.set_style(Style::Stroke);
    canvas.translate((-80.0, -330.0));
    canvas.draw_path(&path, &paint);
}

// Port of: gm/crbug_847759.cpp#L13-L32 (chrome/m156)
crate::def_simple_gm!(crbug_847759, canvas, 500, 500, {
    draw_crbug_847759(canvas);
});
