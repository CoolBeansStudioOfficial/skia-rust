// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/pathcontourstart.cpp (chrome/m156)

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
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::canvas::PointMode;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::point::Vector;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_effects::dash_path_effect;

const K_IMAGE_WIDTH: i32 = 1200;
const K_IMAGE_HEIGHT: i32 = 600;

// Port of: gm/pathcontourstart.cpp#L27-L120 (chrome/m156)
struct ContourStartGm {
    dash_paint: Paint,
    points_paint: Paint,
    rect: Rect,
}

// Port of: gm/pathcontourstart.cpp#L27-L84 (chrome/m156)
impl ContourStartGm {
    fn new() -> Self {
        Self {
            dash_paint: Paint::default(),
            points_paint: Paint::default(),
            rect: Rect::default(),
        }
    }

    // Port of: gm/pathcontourstart.cpp#L86-L112 (chrome/m156)
    fn draw_one_column(
        &self,
        canvas: &Canvas,
        dir: PathDirection,
        make_path: fn(&Rect, PathDirection, usize) -> Path,
    ) {
        let _acr = AutoCanvasRestore::guard(canvas, true);

        for i in 0..8 {
            let path = make_path(&self.rect, dir, i);
            canvas.draw_path(&path, &self.dash_paint);
            canvas.draw_points(PointMode::Points, path.points(), &self.points_paint);

            canvas.translate((0.0, (K_IMAGE_HEIGHT / 8) as f32));
        }
    }

    // Port of: gm/pathcontourstart.cpp#L93-L99 (chrome/m156)
    fn draw_dirs(&self, canvas: &Canvas, make_path: fn(&Rect, PathDirection, usize) -> Path) {
        self.draw_one_column(canvas, PathDirection::CW, make_path);
        canvas.translate(((K_IMAGE_WIDTH / 10) as f32, 0.0));
        self.draw_one_column(canvas, PathDirection::CCW, make_path);
        canvas.translate(((K_IMAGE_WIDTH / 10) as f32, 0.0));
    }
}

fn make_rect_path(rect: &Rect, dir: PathDirection, start_index: usize) -> Path {
    Path::rect_with_start_index(rect, dir, start_index)
}

fn make_oval_path(rect: &Rect, dir: PathDirection, start_index: usize) -> Path {
    Path::oval_with_start_index(rect, dir, start_index)
}

fn make_round_rect_path(rect: &Rect, dir: PathDirection, start_index: usize) -> Path {
    let radii: [Vector; 4] = [
        Vector::new(15.0, 15.0),
        Vector::new(15.0, 15.0),
        Vector::new(15.0, 15.0),
        Vector::new(15.0, 15.0),
    ];
    let rrect = RRect::new_rect_radii(rect, &radii);
    Path::rrect_with_start_index(rrect, dir, start_index)
}

fn make_plain_rrect_path(rect: &Rect, dir: PathDirection, start_index: usize) -> Path {
    let rrect = RRect::new_rect(rect);
    Path::rrect_with_start_index(rrect, dir, start_index)
}

fn make_oval_rrect_path(rect: &Rect, dir: PathDirection, start_index: usize) -> Path {
    let rrect = RRect::new_oval(rect);
    Path::rrect_with_start_index(rrect, dir, start_index)
}

impl GM for ContourStartGm {
    fn name(&self) -> String {
        "contour_start".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(K_IMAGE_WIDTH, K_IMAGE_HEIGHT)
    }

    // Port of: gm/pathcontourstart.cpp#L30-L49 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        const K_MAX_DASH_LEN: f32 = 100.0;
        const K_DASH_GROWTH: f32 = 1.2;

        let mut intervals: Vec<f32> = Vec::new();
        let mut len: f32 = 1.0;
        while len < K_MAX_DASH_LEN {
            intervals.push(len);
            intervals.push(len);
            len *= K_DASH_GROWTH;
        }

        self.dash_paint.set_anti_alias(true);
        self.dash_paint.set_style(Style::Stroke);
        self.dash_paint.set_stroke_width(6.0);
        self.dash_paint.set_color(Color::new(0xff00_8000));
        self.dash_paint
            .set_path_effect(dash_path_effect::new(&intervals, 0.0));

        self.points_paint.set_color(Color::new(0xff80_0000));
        self.points_paint.set_stroke_width(3.0);

        self.rect = Rect::from_ltrb(10.0, 10.0, 100.0, 70.0);
    }

    // Port of: gm/pathcontourstart.cpp#L55-L84 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        self.draw_dirs(canvas, make_rect_path);
        self.draw_dirs(canvas, make_oval_path);
        self.draw_dirs(canvas, make_round_rect_path);
        self.draw_dirs(canvas, make_plain_rrect_path);
        self.draw_dirs(canvas, make_oval_rrect_path);
    }
}

crate::def_gm!(ContourStartGM_ = "ContourStartGM()", ContourStartGm::new());
