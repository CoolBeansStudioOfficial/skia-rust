// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/inversepaths.cpp (chrome/m156)

// The file's other GMs (inverse_fill_filters, inverse_windingmode_filters) are not ported here.

// The size and index arithmetic mirrors the C++ GM.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::PathEffect;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::dash_path_effect;
use skia_rust_effects::image_filters::blur;

// Port of: gm/inversepaths.cpp#L12-L14 (chrome/m156)
fn generate_square(cx: scalar, cy: scalar, w: scalar) -> Path {
    Path::rect(Rect::from_xywh(cx - w / 2.0, cy - w / 2.0, w, w), None)
}

// Port of: gm/inversepaths.cpp#L16-L18 (chrome/m156)
fn generate_rect_line(cx: scalar, cy: scalar, l: scalar) -> Path {
    Path::rect(Rect::from_xywh(cx - l / 2.0, cy, l, 0.0), None)
}

// Port of: gm/inversepaths.cpp#L20-L22 (chrome/m156)
fn generate_circle(cx: scalar, cy: scalar, d: scalar) -> Path {
    Path::circle((cx, cy), d / 2.0, PathDirection::CW)
}

// Port of: gm/inversepaths.cpp#L24-L26 (chrome/m156)
fn generate_line(cx: scalar, cy: scalar, l: scalar) -> Path {
    Path::line((cx - l / 2.0, cy), (cx + l / 2.0, cy))
}

// Port of: gm/inversepaths.cpp#L33-L36 (chrome/m156), make_dash
fn make_dash() -> Option<PathEffect> {
    const INTERVALS: [scalar; 2] = [4.0, 3.0];
    dash_path_effect::new(&INTERVALS, 0.0)
}

// Port of: gm/inversepaths.cpp#L38-L53 (chrome/m156), the `styles` table
struct StyleEntry {
    paint_style: Style,
    path_effect: Option<PathEffect>,
}

// Port of: gm/inversepaths.cpp#L55-L80 (chrome/m156), the `styles`, `pathSizes`, `strokeWidths`
// and `paths` tables
fn styles() -> Vec<StyleEntry> {
    vec![
        StyleEntry {
            paint_style: Style::Stroke,
            path_effect: None,
        },
        StyleEntry {
            paint_style: Style::StrokeAndFill,
            path_effect: None,
        },
        StyleEntry {
            paint_style: Style::Fill,
            path_effect: None,
        },
        StyleEntry {
            paint_style: Style::Stroke,
            path_effect: make_dash(),
        },
    ]
}

const PATH_SIZES: [scalar; 3] = [40.0, 10.0, 0.0];
const STROKE_WIDTHS: [scalar; 2] = [10.0, 0.0];

const SLIDE_WIDTH: scalar = 90.0;
const SLIDE_HEIGHT: scalar = 90.0;
const SLIDE_BOUNDARY: scalar = 5.0;

fn paths() -> [fn(scalar, scalar, scalar) -> Path; 4] {
    [
        generate_square,
        generate_rect_line,
        generate_circle,
        generate_line,
    ]
}

// Port of: gm/inversepaths.cpp#L120-L179 (chrome/m156), DEF_SIMPLE_GM(inverse_paths)
crate::def_simple_gm!(inverse_paths, canvas, 800, 1200, {
    let cx = SLIDE_WIDTH / 2.0 + SLIDE_BOUNDARY;
    let cy = SLIDE_HEIGHT / 2.0 + SLIDE_BOUNDARY;
    let dx = SLIDE_WIDTH + 2.0 * SLIDE_BOUNDARY;
    let dy = SLIDE_HEIGHT + 2.0 * SLIDE_BOUNDARY;

    let clip_rect = Rect::from_ltrb(
        SLIDE_BOUNDARY,
        SLIDE_BOUNDARY,
        SLIDE_BOUNDARY + SLIDE_WIDTH,
        SLIDE_BOUNDARY + SLIDE_HEIGHT,
    );
    let mut clip_paint = Paint::default();
    clip_paint.set_style(Style::Stroke);
    clip_paint.set_stroke_width(2.0);

    let mut outline_paint = Paint::default();
    outline_paint.set_color(Color::new(0x4000_0000));
    outline_paint.set_style(Style::Stroke);
    outline_paint.set_stroke_width(0.0);

    let styles = styles();
    let paths = paths();
    for style in &styles {
        for &size in &PATH_SIZES {
            canvas.save();

            for &stroke_width in &STROKE_WIDTHS {
                let mut paint = Paint::default();
                paint.set_color(Color::new(0xff00_7000));
                paint.set_stroke_width(stroke_width);
                paint.set_style(style.paint_style);
                paint.set_path_effect(style.path_effect.clone());

                for make_path in paths {
                    canvas.draw_rect(clip_rect, &clip_paint);

                    canvas.save();
                    canvas.clip_rect(clip_rect, None, None);

                    let mut path = make_path(cx, cy, size);
                    path.set_fill_type(PathFillType::InverseWinding);
                    canvas.draw_path(&path, &paint);

                    path.set_fill_type(PathFillType::Winding);
                    canvas.draw_path(&path, &outline_paint);

                    canvas.restore();
                    canvas.translate((dx, 0.0));
                }
            }
            canvas.restore();
            canvas.translate((0.0, dy));
        }
    }
});

// Port of: gm/inversepaths.cpp#L181-L197 (chrome/m156), inverse_fill_filters
crate::def_simple_gm!(inverse_fill_filters, canvas, 384, 128, {
    let draw = |paint: &Paint| {
        let mut path = Path::circle((65.0, 65.0), 30.0, None);
        path.set_fill_type(PathFillType::InverseWinding);
        canvas.save();
        canvas.clip_rect(Rect::from_xywh(0.0, 0.0, 128.0, 128.0), None, None);
        canvas.draw_path(&path, paint);
        canvas.restore();

        let mut stroke = Paint::default();
        stroke.set_style(Style::Stroke);
        stroke.set_color(Color::WHITE);
        canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 128.0, 128.0), &stroke);
    };

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    draw(&paint);
    canvas.translate((128.0, 0.0));
    paint.set_image_filter(blur(5.0, 5.0, TileMode::Decal, None, None));
    draw(&paint);
    canvas.translate((128.0, 0.0));
    paint.set_image_filter(None);
    paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 5.0, None));
    draw(&paint);
});

// Port of: gm/inversepaths.cpp#L199-L231 (chrome/m156), inverse_windingmode_filters
crate::def_simple_gm!(inverse_windingmode_filters, canvas, 256, 100, {
    let mut builder = PathBuilder::new();
    builder.add_rect(
        Rect::from_ltrb(10.0, 10.0, 30.0, 30.0),
        PathDirection::CW,
        None,
    );
    builder.add_rect(
        Rect::from_ltrb(20.0, 20.0, 40.0, 40.0),
        PathDirection::CW,
        None,
    );
    builder.add_rect(
        Rect::from_ltrb(10.0, 60.0, 30.0, 80.0),
        PathDirection::CW,
        None,
    );
    builder.add_rect(
        Rect::from_ltrb(20.0, 70.0, 40.0, 90.0),
        PathDirection::CCW,
        None,
    );
    let mut path = builder.detach();

    let mut stroke_paint = Paint::default();
    stroke_paint.set_style(Style::Stroke);
    let clip_rect = Rect::from_ltrb(0.0, 0.0, 51.0, 99.0);
    canvas.draw_path(&path, &stroke_paint);

    let mut fill_paint = Paint::default();
    fill_paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 1.0, None));
    for fill_type in [
        PathFillType::Winding,
        PathFillType::EvenOdd,
        PathFillType::InverseWinding,
        PathFillType::InverseEvenOdd,
    ] {
        canvas.translate((51.0, 0.0));
        canvas.save();
        canvas.clip_rect(clip_rect, None, None);
        path.set_fill_type(fill_type);
        canvas.draw_path(&path, &fill_paint);
        canvas.restore();

        let mut clip_paint = Paint::default();
        clip_paint.set_color(Color::RED);
        clip_paint.set_style(Style::Stroke);
        clip_paint.set_stroke_width(1.0);
        canvas.draw_rect(clip_rect, &clip_paint);
    }
});
