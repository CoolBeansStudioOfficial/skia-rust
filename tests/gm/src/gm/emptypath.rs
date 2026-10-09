// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/emptypath.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::PointMode;
use skia_rust_core::color::Color;
use skia_rust_core::font::Font;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::utils::text_utils::{Align, draw_string};
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// `ToolUtils::color_to_565`.
// Port of: tools/ToolUtils.cpp#L142-L151 (chrome/m156)
fn color_to_565(color: u32) -> Color {
    use skia_rust_core::color::pre_multiply_color;
    use skia_rust_core::color_data::{pixel16_to_color, pixel32_to_pixel16};
    let pm_color = pre_multiply_color(Color::new(color));
    let color16 = pixel32_to_pixel16(pm_color);
    pixel16_to_color(color16)
}

// Port of: gm/emptypath.cpp#L11-L22 (chrome/m156)
fn draw_empty(canvas: &Canvas, color: Color, clip: Rect, style: Style, fill: PathFillType) {
    let mut path = Path::new();
    path.set_fill_type(fill);
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.set_style(style);
    canvas.save();
    canvas.clip_rect(clip, None, None);
    canvas.draw_path(&path, &paint);
    canvas.restore();
}

// Port of: gm/emptypath.cpp#L9-L110 (chrome/m156)
struct EmptyPathGm;

impl GM for EmptyPathGm {
    fn name(&self) -> String {
        "emptypath".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(600, 280)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        // Port of: gm/emptypath.cpp#L24-L36 (chrome/m156)
        let fills: [(PathFillType, &str); 4] = [
            (PathFillType::Winding, "Winding"),
            (PathFillType::EvenOdd, "Even / Odd"),
            (PathFillType::InverseWinding, "Inverse Winding"),
            (PathFillType::InverseEvenOdd, "Inverse Even / Odd"),
        ];
        // Port of: gm/emptypath.cpp#L37-L44 (chrome/m156)
        let styles: [(Style, &str); 3] = [
            (Style::Fill, "Fill"),
            (Style::Stroke, "Stroke"),
            (Style::StrokeAndFill, "Stroke And Fill"),
        ];

        let font = Font::from_size(default_portable_typeface(), 15.0);
        let title = "Empty Paths Drawn Into Rectangle Clips With Indicated Style and Fill";
        draw_string(
            canvas,
            title,
            20.0,
            20.0,
            &font,
            &Paint::default(),
            Align::Left,
        );

        let mut rand = Random::default();
        let rect = Rect::from_wh(100.0, 30.0);
        let mut i = 0;
        canvas.save();
        canvas.translate((10.0, 0.0));
        canvas.save();
        for (style, style_name) in styles {
            for (fill, fill_name) in fills {
                if i % 4 == 0 {
                    canvas.restore();
                    canvas.translate((0.0, rect.height() + 40.0));
                    canvas.save();
                } else {
                    canvas.translate((rect.width() + 40.0, 0.0));
                }
                i += 1;
                let mut color = rand.next_u();
                color |= 0xff00_0000; // force solid
                let color = color_to_565(color);
                draw_empty(canvas, color, rect, style, fill);
                let mut rect_paint = Paint::default();
                rect_paint.set_color(Color::BLACK);
                rect_paint.set_style(Style::Stroke);
                rect_paint.set_stroke_width(-1.0);
                rect_paint.set_anti_alias(true);
                canvas.draw_rect(rect, &rect_paint);
                let mut label_paint = Paint::default();
                label_paint.set_color(color);
                let label_font = Font::from_size(default_portable_typeface(), 12.0);
                draw_string(
                    canvas,
                    style_name,
                    0.0,
                    rect.height() + 15.0,
                    &label_font,
                    &label_paint,
                    Align::Left,
                );
                draw_string(
                    canvas,
                    fill_name,
                    0.0,
                    rect.height() + 28.0,
                    &label_font,
                    &label_paint,
                    Align::Left,
                );
            }
        }
        canvas.restore();
        canvas.restore();
    }
}

// Port of: gm/emptypath.cpp#L112 (chrome/m156)
crate::def_gm!(EmptyPathGM, EmptyPathGm);

// Port of: gm/emptypath.cpp#L124-L128 (chrome/m156)
const PTS: [Point; 3] = [
    Point::new(40.0, 40.0),
    Point::new(80.0, 40.0),
    Point::new(120.0, 40.0),
];

// Port of: gm/emptypath.cpp#L130-L136 (chrome/m156)
fn make_path_move() -> Path {
    let mut builder = PathBuilder::new();
    for p in PTS {
        builder.move_to(p);
    }
    builder.detach()
}

// Port of: gm/emptypath.cpp#L138-L144 (chrome/m156)
fn make_path_move_close() -> Path {
    let mut builder = PathBuilder::new();
    for p in PTS {
        builder.move_to(p).close();
    }
    builder.detach()
}

// Port of: gm/emptypath.cpp#L146-L152 (chrome/m156)
fn make_path_move_line() -> Path {
    let mut builder = PathBuilder::new();
    for p in PTS {
        builder.move_to(p).line_to(p);
    }
    builder.detach()
}

// Port of: gm/emptypath.cpp#L154-L159 (chrome/m156)
fn make_path_move_mix() -> Path {
    PathBuilder::new()
        .move_to(PTS[0])
        .move_to(PTS[1])
        .close()
        .move_to(PTS[2])
        .line_to(PTS[2])
        .detach()
}

// Port of: gm/emptypath.cpp#L161-L190 (chrome/m156)
struct EmptyStrokeGM;

impl GM for EmptyStrokeGM {
    fn name(&self) -> String {
        "emptystroke".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(200, 240)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let procs: [fn() -> Path; 4] = [
            make_path_move,       // expect red red red
            make_path_move_close, // expect black black black
            make_path_move_line,  // expect black black black
            make_path_move_mix,   // expect red black black,
        ];

        let mut stroke_paint = Paint::default();
        stroke_paint.set_style(Style::Stroke);
        stroke_paint.set_stroke_width(21.0);
        stroke_paint.set_stroke_cap(Cap::Square);

        let mut dot_paint = Paint::default();
        dot_paint.set_color(Color::RED);
        stroke_paint.set_style(Style::Stroke);
        dot_paint.set_stroke_width(7.0);

        for proc in procs {
            canvas.draw_points(PointMode::Points, &PTS, &dot_paint);
            canvas.draw_path(&proc(), &stroke_paint);
            canvas.translate((0.0, 40.0));
        }
    }
}

// Port of: gm/emptypath.cpp#L191 (chrome/m156)
crate::def_gm!(EmptyStrokeGM_ = "EmptyStrokeGM", EmptyStrokeGM);
