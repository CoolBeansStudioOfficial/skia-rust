// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/quadpaths.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::color_to_565;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::font::Font;
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/quadpaths.cpp#L36-L52 (chrome/m156), drawPath (the same in QuadClosePathGM)
#[allow(clippy::too_many_arguments)] // mirrors the C++ drawPath signature
fn draw_quad_path(
    path: &mut Path,
    canvas: &Canvas,
    color: Color,
    clip: &Rect,
    cap: Cap,
    join: Join,
    style: Style,
    fill: PathFillType,
    stroke_width: scalar,
) {
    path.set_fill_type(fill);
    let mut paint = Paint::default();
    paint.set_stroke_cap(cap);
    paint.set_stroke_width(stroke_width);
    paint.set_stroke_join(join);
    paint.set_color(color);
    paint.set_style(style);
    canvas.save();
    canvas.clip_rect(clip, ClipOp::Intersect, false);
    canvas.draw_path(path, &paint);
    canvas.restore();
}

// Port of: gm/quadpaths.cpp#L53-L155 (chrome/m156), the onDraw grid shared by QuadPathGM and
// QuadClosePathGM. `title` is the GM's title; `path` its path.
fn draw_quad_grid(canvas: &Canvas, path: &mut Path, title: &str) {
    let fills: [(PathFillType, &str); 4] = [
        (PathFillType::Winding, "Winding"),
        (PathFillType::EvenOdd, "Even / Odd"),
        (PathFillType::InverseWinding, "Inverse Winding"),
        (PathFillType::InverseEvenOdd, "Inverse Even / Odd"),
    ];
    let styles: [(Style, &str); 3] = [
        (Style::Fill, "Fill"),
        (Style::Stroke, "Stroke"),
        (Style::StrokeAndFill, "Stroke And Fill"),
    ];
    let caps: [(Cap, Join, &str); 3] = [
        (Cap::Butt, Join::Bevel, "Butt"),
        (Cap::Round, Join::Round, "Round"),
        (Cap::Square, Join::Bevel, "Square"),
    ];

    // SkPaint titlePaint: the defaults (black fill, no anti-aliasing).
    let title_paint = Paint::default();
    let font = Font::from_size(default_portable_typeface(), 15.0);
    let label_font = Font::from_size(default_portable_typeface(), 10.0);
    canvas.draw_str(title, (20.0, 20.0), &font, &title_paint);

    // `SkRandom rand;` is constructed in C++ but never used.
    let rect = Rect::from_xywh(0.0, 0.0, 100.0, 30.0);
    canvas.save();
    canvas.translate((10.0, 30.0));
    canvas.save();
    for (cap_index, (cap, join, cap_name)) in caps.iter().enumerate() {
        if 0 < cap_index {
            // `std::size(gStyles)`: a count of three, exact in a scalar.
            #[allow(clippy::cast_precision_loss)]
            let style_count = styles.len() as scalar;
            canvas.translate(((rect.width() + 40.0) * style_count, 0.0));
        }
        canvas.save();
        for (fill_index, (fill, fill_name)) in fills.iter().enumerate() {
            if 0 < fill_index {
                canvas.translate((0.0, rect.height() + 40.0));
            }
            canvas.save();
            for (style_index, (style, style_name)) in styles.iter().enumerate() {
                if 0 < style_index {
                    canvas.translate((rect.width() + 40.0, 0.0));
                }
                let color = color_to_565(Color::from(0xff00_7000_u32));
                draw_quad_path(path, canvas, color, &rect, *cap, *join, *style, *fill, 10.0);
                let mut rect_paint = Paint::default();
                rect_paint.set_color(Color::BLACK);
                rect_paint.set_style(Style::Stroke);
                rect_paint.set_stroke_width(-1.0);
                rect_paint.set_anti_alias(true);
                canvas.draw_rect(rect, &rect_paint);

                let mut label_paint = Paint::default();
                label_paint.set_color(color);
                canvas.draw_str(
                    style_name,
                    (0.0, rect.height() + 12.0),
                    &label_font,
                    &label_paint,
                );
                canvas.draw_str(
                    fill_name,
                    (0.0, rect.height() + 24.0),
                    &label_font,
                    &label_paint,
                );
                canvas.draw_str(
                    cap_name,
                    (0.0, rect.height() + 36.0),
                    &label_font,
                    &label_paint,
                );
            }
            canvas.restore();
        }
        canvas.restore();
    }
    canvas.restore();
    canvas.restore();
}

// Port of: gm/quadpaths.cpp#L27-L155 (chrome/m156), QuadPathGM
struct QuadPathGm;

impl GM for QuadPathGm {
    fn name(&self) -> String {
        "quadpath".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1240, 390)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut path = PathBuilder::new()
            .move_to((25.0, 10.0))
            .quad_to((50.0, 20.0), (75.0, 10.0))
            .detach();
        draw_quad_grid(
            canvas,
            &mut path,
            "Quad Drawn Into Rectangle Clips With Indicated Style, Fill and Linecaps, with stroke width 10",
        );
    }
}

// Port of: gm/quadpaths.cpp#L156-L286 (chrome/m156), QuadClosePathGM
struct QuadClosePathGm;

impl GM for QuadClosePathGm {
    fn name(&self) -> String {
        "quadclosepath".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1240, 390)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut path = PathBuilder::new()
            .move_to((25.0, 10.0))
            .quad_to((50.0, 20.0), (75.0, 10.0))
            .close()
            .detach();
        draw_quad_grid(
            canvas,
            &mut path,
            "Quad Closed Drawn Into Rectangle Clips With Indicated Style, Fill and Linecaps, with stroke width 10",
        );
    }
}

// Port of: gm/quadpaths.cpp#L287 (chrome/m156)
crate::def_gm!(QuadPathGM, QuadPathGm);
// Port of: gm/quadpaths.cpp#L289 (chrome/m156)
crate::def_gm!(QuadClosePathGM, QuadClosePathGm);
