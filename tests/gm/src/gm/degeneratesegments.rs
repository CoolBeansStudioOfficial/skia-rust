// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/degeneratesegments.cpp (chrome/m156)

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
use crate::tool_utils::color_to_565;
use skia_rust_core::font::Font;
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

/// `AddSegmentFunc`: appends a segment at `start` and returns where the path now ends.
type AddSegmentFunc = fn(&mut PathBuilder, Point) -> Point;

// We need to use explicit commands here, instead of addPath, because we do not want the moveTo
// that is added at the beginning of a path to appear in the appended path.

// Port of: gm/degeneratesegments.cpp#L37-L42 (chrome/m156), AddMove
fn add_move(path: &mut PathBuilder, start: Point) -> Point {
    let move_to = start + Point::new(0.0, 10.0);
    path.move_to(move_to);
    move_to
}

// Port of: gm/degeneratesegments.cpp#L43-L48 (chrome/m156), AddMoveClose
fn add_move_close(path: &mut PathBuilder, start: Point) -> Point {
    let move_to = start + Point::new(0.0, 10.0);
    path.move_to(move_to);
    path.close();
    move_to
}

// Port of: gm/degeneratesegments.cpp#L49-L53 (chrome/m156), AddDegenLine
fn add_degen_line(path: &mut PathBuilder, start: Point) -> Point {
    path.line_to(start);
    start
}

// Port of: gm/degeneratesegments.cpp#L54-L59 (chrome/m156), AddMoveDegenLine
fn add_move_degen_line(path: &mut PathBuilder, start: Point) -> Point {
    let move_to = start + Point::new(0.0, 10.0);
    path.move_to(move_to);
    path.line_to(move_to);
    move_to
}

// Port of: gm/degeneratesegments.cpp#L60-L66 (chrome/m156), AddMoveDegenLineClose
fn add_move_degen_line_close(path: &mut PathBuilder, start: Point) -> Point {
    let move_to = start + Point::new(0.0, 10.0);
    path.move_to(move_to);
    path.line_to(move_to);
    path.close();
    move_to
}

// Port of: gm/degeneratesegments.cpp#L67-L71 (chrome/m156), AddDegenQuad
fn add_degen_quad(path: &mut PathBuilder, start: Point) -> Point {
    path.quad_to(start, start);
    start
}

// Port of: gm/degeneratesegments.cpp#L72-L77 (chrome/m156), AddMoveDegenQuad
fn add_move_degen_quad(path: &mut PathBuilder, start: Point) -> Point {
    let move_to = start + Point::new(0.0, 10.0);
    path.move_to(move_to);
    path.quad_to(move_to, move_to);
    move_to
}

// Port of: gm/degeneratesegments.cpp#L78-L84 (chrome/m156), AddMoveDegenQuadClose
fn add_move_degen_quad_close(path: &mut PathBuilder, start: Point) -> Point {
    let move_to = start + Point::new(0.0, 10.0);
    path.move_to(move_to);
    path.quad_to(move_to, move_to);
    path.close();
    move_to
}

// Port of: gm/degeneratesegments.cpp#L85-L89 (chrome/m156), AddDegenCubic
fn add_degen_cubic(path: &mut PathBuilder, start: Point) -> Point {
    path.cubic_to(start, start, start);
    start
}

// Port of: gm/degeneratesegments.cpp#L90-L95 (chrome/m156), AddMoveDegenCubic
fn add_move_degen_cubic(path: &mut PathBuilder, start: Point) -> Point {
    let move_to = start + Point::new(0.0, 10.0);
    path.move_to(move_to);
    path.cubic_to(move_to, move_to, move_to);
    move_to
}

// Port of: gm/degeneratesegments.cpp#L96-L102 (chrome/m156), AddMoveDegenCubicClose
fn add_move_degen_cubic_close(path: &mut PathBuilder, start: Point) -> Point {
    let move_to = start + Point::new(0.0, 10.0);
    path.move_to(move_to);
    path.cubic_to(move_to, move_to, move_to);
    path.close();
    move_to
}

// Port of: gm/degeneratesegments.cpp#L103-L107 (chrome/m156), AddClose
fn add_close(path: &mut PathBuilder, start: Point) -> Point {
    path.close();
    start
}

// Port of: gm/degeneratesegments.cpp#L108-L113 (chrome/m156), AddLine
fn add_line(path: &mut PathBuilder, start: Point) -> Point {
    let end = start + Point::new(40.0, 0.0);
    path.line_to(end);
    end
}

// Port of: gm/degeneratesegments.cpp#L114-L121 (chrome/m156), AddMoveLine
fn add_move_line(path: &mut PathBuilder, start: Point) -> Point {
    let move_to = start + Point::new(0.0, 10.0);
    let end = move_to + Point::new(40.0, 0.0);
    path.move_to(move_to);
    path.line_to(end);
    end
}

// Port of: gm/degeneratesegments.cpp#L122-L130 (chrome/m156), AddMoveLineClose
fn add_move_line_close(path: &mut PathBuilder, start: Point) -> Point {
    let move_to = start + Point::new(0.0, 10.0);
    let end = move_to + Point::new(40.0, 0.0);
    path.move_to(move_to);
    path.line_to(end);
    path.close();
    end
}

// Port of: gm/degeneratesegments.cpp#L131-L137 (chrome/m156), AddQuad
fn add_quad(path: &mut PathBuilder, start: Point) -> Point {
    let mid = start + Point::new(20.0, 5.0);
    let end = start + Point::new(40.0, 0.0);
    path.quad_to(mid, end);
    end
}

// Port of: gm/degeneratesegments.cpp#L138-L145 (chrome/m156), AddMoveQuad
fn add_move_quad(path: &mut PathBuilder, start: Point) -> Point {
    let move_to = start + Point::new(0.0, 10.0);
    let mid = move_to + Point::new(20.0, 5.0);
    let end = move_to + Point::new(40.0, 0.0);
    path.move_to(move_to);
    path.quad_to(mid, end);
    end
}

// Port of: gm/degeneratesegments.cpp#L146-L154 (chrome/m156), AddMoveQuadClose
fn add_move_quad_close(path: &mut PathBuilder, start: Point) -> Point {
    let move_to = start + Point::new(0.0, 10.0);
    let mid = move_to + Point::new(20.0, 5.0);
    let end = move_to + Point::new(40.0, 0.0);
    path.move_to(move_to);
    path.quad_to(mid, end);
    path.close();
    end
}

// Port of: gm/degeneratesegments.cpp#L155-L162 (chrome/m156), AddCubic
fn add_cubic(path: &mut PathBuilder, start: Point) -> Point {
    let t1 = start + Point::new(15.0, 5.0);
    let t2 = start + Point::new(25.0, 5.0);
    let end = start + Point::new(40.0, 0.0);
    path.cubic_to(t1, t2, end);
    end
}

// Port of: gm/degeneratesegments.cpp#L163-L171 (chrome/m156), AddMoveCubic
fn add_move_cubic(path: &mut PathBuilder, start: Point) -> Point {
    let move_to = start + Point::new(0.0, 10.0);
    let t1 = move_to + Point::new(15.0, 5.0);
    let t2 = move_to + Point::new(25.0, 5.0);
    let end = move_to + Point::new(40.0, 0.0);
    path.move_to(move_to);
    path.cubic_to(t1, t2, end);
    end
}

// Port of: gm/degeneratesegments.cpp#L172-L181 (chrome/m156), AddMoveCubicClose
fn add_move_cubic_close(path: &mut PathBuilder, start: Point) -> Point {
    let move_to = start + Point::new(0.0, 10.0);
    let t1 = move_to + Point::new(15.0, 5.0);
    let t2 = move_to + Point::new(25.0, 5.0);
    let end = move_to + Point::new(40.0, 0.0);
    path.move_to(move_to);
    path.cubic_to(t1, t2, end);
    path.close();
    end
}

// Port of: gm/degeneratesegments.cpp#L183-L188 (chrome/m156), drawPath
#[allow(clippy::too_many_arguments)] // mirrors DegenerateSegmentsGM::drawPath
fn draw_path(
    mut path: Path,
    canvas: &Canvas,
    color: Color,
    clip: &Rect,
    cap: Cap,
    join: Join,
    style: Style,
    fill: PathFillType,
    stroke_width: f32,
) {
    path.set_fill_type(fill);
    let mut paint = Paint::default();
    paint.set_stroke_cap(cap);
    paint.set_stroke_width(stroke_width);
    paint.set_stroke_join(join);
    paint.set_color(color);
    paint.set_style(style);
    canvas.save();
    canvas.clip_rect(clip, None, None);
    canvas.draw_path(&path, &paint);
    canvas.restore();
}

// Port of: gm/degeneratesegments.cpp#L27-L372 (chrome/m156), DegenerateSegmentsGM
struct DegenerateSegmentsGm;

impl GM for DegenerateSegmentsGm {
    // Port of: gm/degeneratesegments.cpp#L35 (chrome/m156), getName
    fn name(&self) -> String {
        "degeneratesegments".to_owned()
    }

    // Port of: gm/degeneratesegments.cpp#L36 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(896, 930)
    }

    // Port of: gm/degeneratesegments.cpp#L216-L370 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        const SEGMENT_FUNCTIONS: [AddSegmentFunc; 21] = [
            add_move,
            add_move_close,
            add_degen_line,
            add_move_degen_line,
            add_move_degen_line_close,
            add_degen_quad,
            add_move_degen_quad,
            add_move_degen_quad_close,
            add_degen_cubic,
            add_move_degen_cubic,
            add_move_degen_cubic_close,
            add_close,
            add_line,
            add_move_line,
            add_move_line_close,
            add_quad,
            add_move_quad,
            add_move_quad_close,
            add_cubic,
            add_move_cubic,
            add_move_cubic_close,
        ];
        const SEGMENT_NAMES: [&str; 21] = [
            "Move",
            "MoveClose",
            "DegenLine",
            "MoveDegenLine",
            "MoveDegenLineClose",
            "DegenQuad",
            "MoveDegenQuad",
            "MoveDegenQuadClose",
            "DegenCubic",
            "MoveDegenCubic",
            "MoveDegenCubicClose",
            "Close",
            "Line",
            "MoveLine",
            "MoveLineClose",
            "Quad",
            "MoveQuad",
            "MoveQuadClose",
            "Cubic",
            "MoveCubic",
            "MoveCubicClose",
        ];
        // (fill, name)
        const FILLS: [(PathFillType, &str); 4] = [
            (PathFillType::Winding, "Winding"),
            (PathFillType::EvenOdd, "Even / Odd"),
            (PathFillType::InverseWinding, "Inverse Winding"),
            (PathFillType::InverseEvenOdd, "Inverse Even / Odd"),
        ];
        // (style, name)
        const STYLES: [(Style, &str); 3] = [
            (Style::Fill, "Fill"),
            (Style::Stroke, "Stroke 10"),
            (Style::StrokeAndFill, "Stroke 10 And Fill"),
        ];
        // (cap, join, name)
        const CAPS: [(Cap, Join, &str); 3] = [
            (Cap::Butt, Join::Bevel, "Butt"),
            (Cap::Round, Join::Round, "Round"),
            (Cap::Square, Join::Bevel, "Square"),
        ];

        let mut title_paint = Paint::default();
        title_paint.set_color(Color::BLACK);
        title_paint.set_anti_alias(true);
        let mut font = Font::from_size(default_portable_typeface(), 15.0);
        let title = "Random Paths Drawn Into Rectangle Clips With \
                     Indicated Style, Fill and Linecaps, \
                     with Stroke width 6";
        canvas.draw_str(title, (20.0, 20.0), &font, &title_paint);

        let mut rand = Random::default();
        let rect = Rect::from_ltrb(0.0, 0.0, 220.0, 50.0);
        canvas.save();
        canvas.translate((2.0, 30.0)); // The title
        canvas.save();
        let num_segments = SEGMENT_FUNCTIONS.len() as u32;
        let num_caps = CAPS.len() as u32;
        let num_styles = STYLES.len() as u32;
        let num_fills = FILLS.len() as u32;
        for row in 0..6 {
            if 0 < row {
                canvas.translate((0.0, rect.height() + 100.0));
            }
            canvas.save();
            for column in 0..4 {
                if 0 < column {
                    canvas.translate((rect.width() + 4.0, 0.0));
                }
                let color = color_to_565(0xff007000);
                let (style_value, style_name) =
                    STYLES[((rand.next_u() >> 16) % num_styles) as usize];
                let (cap, join, cap_name) = CAPS[((rand.next_u() >> 16) % num_caps) as usize];
                let (fill, fill_name) = FILLS[((rand.next_u() >> 16) % num_fills) as usize];
                let s1 = ((rand.next_u() >> 16) % num_segments) as usize;
                let s2 = ((rand.next_u() >> 16) % num_segments) as usize;
                let s3 = ((rand.next_u() >> 16) % num_segments) as usize;
                let s4 = ((rand.next_u() >> 16) % num_segments) as usize;
                let s5 = ((rand.next_u() >> 16) % num_segments) as usize;
                let mut pt = Point::new(10.0, 0.0);
                let mut path = PathBuilder::new();
                pt = SEGMENT_FUNCTIONS[s1](&mut path, pt);
                pt = SEGMENT_FUNCTIONS[s2](&mut path, pt);
                pt = SEGMENT_FUNCTIONS[s3](&mut path, pt);
                pt = SEGMENT_FUNCTIONS[s4](&mut path, pt);
                let _ = SEGMENT_FUNCTIONS[s5](&mut path, pt);
                draw_path(
                    path.detach(),
                    canvas,
                    color,
                    &rect,
                    cap,
                    join,
                    style_value,
                    fill,
                    6.0,
                );

                let mut rect_paint = Paint::default();
                rect_paint.set_color(Color::BLACK);
                rect_paint.set_style(Style::Stroke);
                rect_paint.set_stroke_width(-1.0);
                rect_paint.set_anti_alias(true);
                canvas.draw_rect(rect, &rect_paint);

                let mut label_paint = Paint::default();
                label_paint.set_color(color);
                label_paint.set_anti_alias(true);
                font.set_size(10.0);
                let bottom = rect.height();
                canvas.draw_str(style_name, (0.0, bottom + 12.0), &font, &label_paint);
                canvas.draw_str(fill_name, (0.0, bottom + 24.0), &font, &label_paint);
                canvas.draw_str(cap_name, (0.0, bottom + 36.0), &font, &label_paint);
                canvas.draw_str(SEGMENT_NAMES[s1], (0.0, bottom + 48.0), &font, &label_paint);
                canvas.draw_str(SEGMENT_NAMES[s2], (0.0, bottom + 60.0), &font, &label_paint);
                canvas.draw_str(SEGMENT_NAMES[s3], (0.0, bottom + 72.0), &font, &label_paint);
                canvas.draw_str(SEGMENT_NAMES[s4], (0.0, bottom + 84.0), &font, &label_paint);
                canvas.draw_str(SEGMENT_NAMES[s5], (0.0, bottom + 96.0), &font, &label_paint);
            }
            canvas.restore();
        }
        canvas.restore();
        canvas.restore();
    }
}

// Port of: gm/degeneratesegments.cpp#L372 (chrome/m156), DEF_GM
crate::def_gm!(DegenerateSegmentsGM, DegenerateSegmentsGm);
