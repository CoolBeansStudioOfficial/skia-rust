// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/emptypath.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::PointMode;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;

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
