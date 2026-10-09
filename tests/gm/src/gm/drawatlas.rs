// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drawatlas.cpp (chrome/m156)
//
// Only `drawTextRSXform` is ported here: it is the text-module entry of this file. The other GMs
// in gm/drawatlas.cpp have their own manifest entries.

use crate::prelude::*;
use skia_rust_core::color::colors;
use skia_rust_core::font::Font;
use skia_rust_core::font_priv::get_font_bounds;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_measure::PathMeasure;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::shader::Shader;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/drawatlas.cpp#L134-L136 (chrome/m156), the `std::max` of two scalars
fn sk_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

// Port of: gm/drawatlas.cpp#L133-L183 (chrome/m156), draw_text_on_path
fn draw_text_on_path(
    canvas: &Canvas,
    text: &[u8],
    xy: &[Point],
    path: &Path,
    font: &Font,
    paint: &Paint,
    baseline_offset: f32,
) {
    let meas = PathMeasure::new(path, false, None);

    let count = font.count_text(text, TextEncoding::UTF8);
    let mut xform = vec![RSXform::default(); count];
    let mut widths = vec![0.0_f32; count];

    // Compute a conservative bounds so we can cull the draw
    let fontb = get_font_bounds(font);
    let max = sk_max(
        sk_max(fontb.left.abs(), fontb.right.abs()),
        sk_max(fontb.top.abs(), fontb.bottom.abs()),
    );
    let mut bounds = *path.bounds();
    bounds.outset((max, max));

    let mut glyphs = vec![GlyphId::default(); count];
    font.text_to_glyphs(text, TextEncoding::UTF8, &mut glyphs);
    font.get_widths(&glyphs, &mut widths);

    for i in 0..count {
        // we want to position each character on the center of its advance
        let offset = widths[i] / 2.0;
        let mut pos = Point::new(0.0, 0.0);
        let mut tan = Vector::new(0.0, 0.0);
        if !meas.get_pos_tan(xy[i].x + offset, Some(&mut pos), Some(&mut tan)) {
            pos = xy[i];
            tan.set(1.0, 0.0);
        }
        pos += Vector::new(-tan.y, tan.x) * baseline_offset;

        xform[i].scos = tan.x;
        xform[i].ssin = tan.y;
        xform[i].tx = pos.x - tan.y * xy[i].y - tan.x * offset;
        xform[i].ty = pos.y + tan.x * xy[i].y - tan.y * offset;
    }

    if let Some(blob) = TextBlob::from_rsxform_glyphs(&glyphs, &xform, font) {
        canvas.draw_text_blob(&blob, (0.0, 0.0), paint);
    }

    {
        let mut p = Paint::default();
        p.set_style(Style::Stroke);
        canvas.draw_rect(bounds, &p);
    }
}

// Port of: gm/drawatlas.cpp#L185-L189 (chrome/m156), make_shader
fn make_shader() -> Option<Shader> {
    let pts = [Point::new(0.0, 0.0), Point::new(220.0, 0.0)];
    let grad_colors = [colors::RED, colors::BLUE];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&grad_colors, None, TileMode::Mirror, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/drawatlas.cpp#L191-L225 (chrome/m156), drawTextPath
fn draw_text_path(canvas: &Canvas, do_stroke: bool) {
    let text0 = b"ABCDFGHJKLMNOPQRSTUVWXYZ";
    let n = text0.len();
    let mut pos = vec![Point::new(0.0, 0.0); n];

    let mut font = default_portable_font();
    font.set_size(100.0);

    let mut paint = Paint::default();
    paint.set_shader(make_shader());
    paint.set_anti_alias(true);
    if do_stroke {
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(2.25);
        paint.set_stroke_join(skia_rust_core::paint::Join::Round);
    }

    let mut x: f32 = 0.0;
    for i in 0..n {
        pos[i].set(x, 0.0);
        x += font
            .measure_text(&text0[i..=i], TextEncoding::UTF8, Some(&paint))
            .0;
    }

    let mut path = Path::default();
    let baseline_offset: f32 = -5.0;

    let dirs = [PathDirection::CW, PathDirection::CCW];
    for d in dirs {
        path = Path::oval(Rect::from_xywh(160.0, 160.0, 540.0, 540.0), d);
        draw_text_on_path(canvas, text0, &pos, &path, &font, &paint, baseline_offset);
    }

    paint.reset();
    paint.set_style(Style::Stroke);
    canvas.draw_path(&path, &paint);
}

// Port of: gm/drawatlas.cpp#L227-L235 (chrome/m156), drawTextRSXform
crate::def_simple_gm!(drawTextRSXform, canvas, 430, 860, {
    canvas.scale((0.5, 0.5));
    let do_stroke = [false, true];
    for st in do_stroke {
        draw_text_path(canvas, st);
        canvas.translate((0.0, 860.0));
    }
});
