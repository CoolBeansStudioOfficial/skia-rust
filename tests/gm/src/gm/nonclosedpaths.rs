// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/nonclosedpaths.cpp (chrome/m156)

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
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;

// This GM tests a grab-bag of non-closed paths. All these paths look like
// closed rects, but they don't call path.close(). Depending on the stroke
// settings these slightly different paths give widely different results.
//
// Port of: gm/nonclosedpaths.cpp#L19-L146 (chrome/m156)
#[derive(Clone, Copy, PartialEq, Eq)]
enum ClosureType {
    // The last point doesn't coincide with the first one in the contour.
    // The path looks not closed at all.
    TotallyNonClosed,
    // The last point coincides with the first one at a corner.
    // The path looks closed, but final rendering has 2 ends with cap.
    FakeCloseCorner,
    // The last point coincides with the first one in the middle of a line.
    // The path looks closed, and the final rendering looks closed too.
    FakeCloseMiddle,
}

// Port of: gm/nonclosedpaths.cpp#L47-L63 (chrome/m156)
// Use rect-like geometry for non-closed path, for right angles make it
// easier to show the visual difference of lineCap and lineJoin.
fn make_path(kind: ClosureType) -> Path {
    let mut path = PathBuilder::new();
    if ClosureType::FakeCloseMiddle == kind {
        path.move_to((30.0, 50.0));
        path.line_to((30.0, 30.0));
    } else {
        path.move_to((30.0, 30.0));
    }
    path.line_to((70.0, 30.0));
    path.line_to((70.0, 70.0));
    path.line_to((30.0, 70.0));
    path.line_to((30.0, 50.0));
    if ClosureType::FakeCloseCorner == kind {
        path.line_to((30.0, 30.0));
    }
    path.detach()
}

// Port of: gm/nonclosedpaths.cpp#L65-L70 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // int cell coordinates, as in C++
fn set_location(canvas: &Canvas, counter: i32, line_num: i32) {
    let x = 100.0 * ((counter % line_num) as f32) + 10.0 + 0.25;
    let y = 100.0 * ((counter / line_num) as f32) + 10.0 + 0.75;
    canvas.translate((x, y));
}

// Port of: gm/nonclosedpaths.cpp#L72-L136 (chrome/m156)
#[allow(clippy::too_many_lines)]
fn draw_nonclosed_paths(canvas: &Canvas) {
    // Stroke widths are:
    // 0(may use hairline rendering), 10(common case for stroke-style)
    // 40 and 50(>= geometry width/height, make the contour filled in fact)
    const STROKE_WIDTH: [i32; 4] = [0, 10, 40, 50];
    let num_widths = STROKE_WIDTH.len() as i32;

    const STYLE: [Style; 2] = [Style::Stroke, Style::StrokeAndFill];

    const CAP: [Cap; 3] = [Cap::Butt, Cap::Round, Cap::Square];

    const JOIN: [Join; 3] = [Join::Miter, Join::Round, Join::Bevel];

    const TYPE: [ClosureType; 3] = [
        ClosureType::TotallyNonClosed,
        ClosureType::FakeCloseCorner,
        ClosureType::FakeCloseMiddle,
    ];

    let mut counter = 0;
    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    // For stroke style painter and fill-and-stroke style painter
    for kind in TYPE {
        for style in STYLE {
            for cap in CAP {
                for join in JOIN {
                    for width in STROKE_WIDTH {
                        canvas.save();
                        // kJoinCount == 3
                        set_location(canvas, counter, 3 * num_widths);

                        let path = make_path(kind);

                        paint.set_style(style);
                        paint.set_stroke_cap(cap);
                        paint.set_stroke_join(join);
                        paint.set_stroke_width(width as f32);

                        canvas.draw_path(&path, &paint);
                        canvas.restore();
                        counter += 1;
                    }
                }
            }
        }
    }

    // For fill style painter
    paint.set_style(Style::Fill);
    for kind in TYPE {
        canvas.save();
        set_location(canvas, counter, 3 * num_widths);

        let path = make_path(kind);

        canvas.draw_path(&path, &paint);
        canvas.restore();
        counter += 1;
    }
}

// Port of: gm/nonclosedpaths.cpp#L17-L144 (chrome/m156)
struct NonClosedPathsGm;

impl GM for NonClosedPathsGm {
    fn name(&self) -> String {
        "nonclosedpaths".to_string()
    }

    // 12 * 18 + 3 cases, every case is 100 * 100 pixels.
    fn size(&mut self) -> ISize {
        ISize::new(1220, 1920)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        draw_nonclosed_paths(canvas);
    }
}

crate::def_gm!(NonClosedPathsGM, NonClosedPathsGm);
