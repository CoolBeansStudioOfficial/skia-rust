// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bug12866.cpp (chrome/m156)

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
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::path_utils::fill_path_with_paint;
use skia_rust_core::rect::Rect;

// Port of: gm/bug12866.cpp#L15-L32 (chrome/m156)
fn get_path() -> Path {
    let mut builder = PathBuilder::new_with_fill_type(PathFillType::Winding);
    // 2100.92f, 115.991f
    builder.move_to((f32::from_bits(0x4503_4ec4), f32::from_bits(0x42e7_fb80)));
    // 2063.28f, 179.199f, 2063.28f, 159.058f
    builder.quad_to(
        (f32::from_bits(0x4500_f46c), f32::from_bits(0x4333_3300)),
        (f32::from_bits(0x4500_f46c), f32::from_bits(0x431f_0ec0)),
    );
    // 2063.28f, 138.843f, 2073.27f, 127.417f
    builder.quad_to(
        (f32::from_bits(0x4500_f46c), f32::from_bits(0x430a_d7c0)),
        (f32::from_bits(0x4501_9462), f32::from_bits(0x42fe_d580)),
    );
    // 2083.27f, 115.991f, 2100.92f, 115.991f
    builder.quad_to(
        (f32::from_bits(0x4502_3458), f32::from_bits(0x42e7_fb80)),
        (f32::from_bits(0x4503_4ec4), f32::from_bits(0x42e7_fb80)),
    );
    builder.close();
    builder.detach()
}

// Reproduces the underlying problem from skbug.com/40043963.
// The path (part of a glyph) was being drawn stroked, and with a perspective matrix.
// The perspective matrix forces a very large resScale when stroking the path.
// The resulting filled path is incorrect. Note that stroking with a smaller resScale works fine.
// Port of: gm/bug12866.cpp#L34-L68 (chrome/m156)
crate::def_simple_gm!(bug12866, canvas, 128, 64, {
    let mut stroke_paint = Paint::default();
    stroke_paint.set_anti_alias(true);
    stroke_paint.set_style(Style::Stroke);
    stroke_paint.set_stroke_width(3.0);

    let mut fill_paint = Paint::default();
    fill_paint.set_anti_alias(true);

    let stroke_path = get_path();
    let mut builder = PathBuilder::new();
    fill_path_with_paint(
        &stroke_path,
        &stroke_paint,
        &mut builder,
        None::<&Rect>,
        Matrix::scale((1200.0, 1200.0)),
    );
    let fill_path = builder.detach();

    let stroke_bounds = stroke_path.bounds();
    let fill_bounds = fill_path.bounds();

    // Draw the stroked path. This (internally) uses a resScale of 1.0, and looks good.
    canvas.save();
    canvas.translate((10.0 - stroke_bounds.left, 10.0 - stroke_bounds.top));
    canvas.draw_path(&stroke_path, &stroke_paint);
    canvas.restore();

    // With a perspective CTM, it's possible for resScale to become large. Draw the filled
    // path produced by the stroker in that situation, which ends up being incorrect.
    canvas.save();
    canvas.translate((74.0 - fill_bounds.left, 10.0 - fill_bounds.top));
    canvas.draw_path(&fill_path, &fill_paint);
    canvas.restore();
});

// This is another example of the same underlying bug (recursion limit in the stroker),
// but with cubics, rather than quads.
// Port of: gm/bug12866.cpp#L70-L98 (chrome/m156)
crate::def_simple_gm!(bug40810065, canvas, 256, 512, {
    canvas.scale((2.0, 2.0));

    let path1 = PathBuilder::new()
        .move_to((108.87, 3.78))
        .cubic_to((201.1, -128.61), (34.21, 82.54), (134.14, 126.01))
        .detach();
    let path2 = PathBuilder::new()
        .move_to((108.87, 3.78))
        .cubic_to((201.0, -128.61), (34.21, 82.54), (134.14, 126.0))
        .detach();

    let mut stroke = Paint::default();
    stroke.set_color(Color::BLACK);
    stroke.set_anti_alias(true);
    stroke.set_style(Style::Stroke);
    stroke.set_stroke_width(1.0);
    stroke.set_stroke_cap(Cap::Round);

    canvas.save();
    canvas.translate((-75.0, 50.0));
    canvas.draw_path(&path1, &stroke);
    canvas.restore();

    canvas.save();
    canvas.translate((-20.0, 100.0));
    canvas.draw_path(&path2, &stroke);
    canvas.restore();
});

// Finally: A repro case that involves conics. This should draw NOTHING. When incorrect, it drew
// a large black rectangle over half of the slide.
// Port of: gm/bug12866.cpp#L100-L117 (chrome/m156)
crate::def_simple_gm_bg!(bug41422450, canvas, 863, 473, Color::WHITE, {
    let mat = M44::new(
        1.0,
        -0.000_001_395_662_71,
        0.0,
        -2_321_738.0,
        0.000_113_059_919,
        0.012_344_451_6,
        0.0,
        -353.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    );
    canvas.concat_44(&mat);

    let circle = Rect::from_ltrb(-3_299_135.5, -12_312_541.0, 9_897_407.0, 884_000.812);

    let stroke_path = PathBuilder::new()
        .arc_to(circle, 59.999_996_2, 59.999_996_2, true)
        .detach();

    let mut stroke_paint = Paint::default();
    stroke_paint.set_style(Style::Stroke);
    stroke_paint.set_stroke_width(2.0);
    canvas.draw_path(&stroke_path, &stroke_paint);
});
