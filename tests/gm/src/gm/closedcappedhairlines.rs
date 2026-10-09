// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/closedcappedhairlines.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: literals, short names, local constants, int/float
// conversions, index loops and long bodies are kept as they are there.
#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::trivially_copy_pass_by_ref,
    clippy::write_with_newline,
    clippy::excessive_precision,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::unused_self
)]

use crate::prelude::*;
use skia_rust_core::color::Color;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_raster::surfaces;

// Port of: gm/closedcappedhairlines.cpp#L14 (chrome/m156)
const K_SCALE: f32 = 4.0;
// Port of: gm/closedcappedhairlines.cpp#L15 (chrome/m156)
const K_GRID_WH: i32 = 70;

// Port of: gm/closedcappedhairlines.cpp#L17-L28 (chrome/m156)
fn draw_grid(canvas: &Canvas) {
    let mut grid_paint = Paint::default();
    grid_paint.set_color(Color::DARK_GRAY);
    grid_paint.set_style(Style::Stroke);
    grid_paint.set_stroke_width(0.0);
    for y in 0..=K_GRID_WH {
        canvas.draw_line((0.0, y as f32), (K_GRID_WH as f32, y as f32), &grid_paint);
    }
    for x in 0..=K_GRID_WH {
        canvas.draw_line((x as f32, 0.0), (x as f32, K_GRID_WH as f32), &grid_paint);
    }
}

// Port of: gm/closedcappedhairlines.cpp#L30-L34 (chrome/m156)
fn highlight_box(p: Point) -> Rect {
    let k_offset: f32 = 2.0;
    Rect::from_xywh(
        p.x - k_offset,
        p.y - k_offset,
        k_offset * 2.0,
        k_offset * 2.0,
    )
}

// Port of: gm/closedcappedhairlines.cpp#L36-L46 (chrome/m156)
fn draw_highlights(canvas: &Canvas, paths: &[&Path]) {
    let mut highlight_paint = Paint::default();
    highlight_paint.set_style(Style::Stroke);
    highlight_paint.set_stroke_width(0.0);
    highlight_paint.set_color(Color::RED);
    highlight_paint.set_anti_alias(true);
    for path in paths {
        let points = path.points();
        canvas.draw_rect(highlight_box(points[0]), &highlight_paint);
        canvas.draw_rect(highlight_box(points[points.len() - 1]), &highlight_paint);
    }
}

// Port of: gm/closedcappedhairlines.cpp#L48-L117 (chrome/m156)
fn draw_hairline_contours_with_caps(canvas: &Canvas, cap: Cap) {
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(0.0);
    paint.set_color(Color::BLACK);
    paint.set_anti_alias(true);
    paint.set_stroke_cap(cap);
    let mut path_surface = surfaces::raster(
        &ImageInfo::new_n32_premul((K_GRID_WH, K_GRID_WH), None),
        None,
        None,
    )
    .expect("a surface");
    let path_canvas = path_surface.canvas();

    let line_on_open = PathBuilder::new()
        .line_to((0.0, 5.0))
        .line_to((5.0, 5.0))
        .line_to((5.0, 0.0))
        .detach()
        .make_offset((5.0, 5.0));
    path_canvas.draw_path(&line_on_open, &paint);
    let quad_on_open = PathBuilder::new()
        .quad_to((15.0, 5.0), (0.0, 10.0))
        .detach()
        .make_offset((20.0, 5.0));
    path_canvas.draw_path(&quad_on_open, &paint);
    let cubic_on_open = PathBuilder::new()
        .cubic_to((-5.0, 0.0), (-5.0, 5.0), (0.0, 10.0))
        .detach()
        .make_offset((40.0, 5.0));
    path_canvas.draw_path(&cubic_on_open, &paint);
    let line_off_open = line_on_open
        .make_offset((0.5, 0.5))
        .make_offset((0.0, 15.0));
    path_canvas.draw_path(&line_off_open, &paint);
    let quad_off_open = quad_on_open
        .make_offset((0.5, 0.5))
        .make_offset((0.0, 15.0));
    path_canvas.draw_path(&quad_off_open, &paint);
    let cubic_off_open = cubic_on_open
        .make_offset((0.5, 0.5))
        .make_offset((0.0, 15.0));
    path_canvas.draw_path(&cubic_off_open, &paint);
    let line_on_closed = PathBuilder::new_path(&line_on_open)
        .close()
        .detach()
        .make_offset((0.0, 30.0));
    path_canvas.draw_path(&line_on_closed, &paint);
    let quad_on_closed = PathBuilder::new_path(&quad_on_open)
        .close()
        .detach()
        .make_offset((0.0, 30.0));
    path_canvas.draw_path(&quad_on_closed, &paint);
    let cubic_on_closed = PathBuilder::new_path(&cubic_on_open)
        .close()
        .detach()
        .make_offset((0.0, 30.0));
    path_canvas.draw_path(&cubic_on_closed, &paint);
    let line_off_closed = PathBuilder::new_path(&line_on_open)
        .close()
        .detach()
        .make_offset((0.5, 0.5))
        .make_offset((0.0, 45.0));
    path_canvas.draw_path(&line_off_closed, &paint);
    let quad_off_closed = PathBuilder::new_path(&quad_on_open)
        .close()
        .detach()
        .make_offset((0.5, 0.5))
        .make_offset((0.0, 45.0));
    path_canvas.draw_path(&quad_off_closed, &paint);
    let cubic_off_closed = PathBuilder::new_path(&cubic_on_open)
        .close()
        .detach()
        .make_offset((0.5, 0.5))
        .make_offset((0.0, 45.0));
    path_canvas.draw_path(&cubic_off_closed, &paint);

    let path_img = path_surface.image_snapshot().expect("a snapshot");
    canvas.draw_image(&path_img, (0.0, 0.0), None);
    canvas.scale((K_SCALE, K_SCALE));
    canvas.draw_image(&path_img, (15.0, 0.0), None);
    canvas.translate((15.0, 0.0));
    draw_grid(canvas);
    draw_highlights(
        canvas,
        &[
            &line_on_open,
            &quad_on_open,
            &cubic_on_open,
            &line_off_open,
            &quad_off_open,
            &cubic_off_open,
        ],
    );
}

// Port of: gm/closedcappedhairlines.cpp#L119-L128 (chrome/m156)
const WIDTH: i32 = 250;
const HEIGHT: i32 = 250;

// Port of: gm/closedcappedhairlines.cpp#L119-L122 (chrome/m156)
crate::def_simple_gm!(hairlines_buttcap, canvas, WIDTH, HEIGHT, {
    draw_hairline_contours_with_caps(canvas, Cap::Butt);
});

// Port of: gm/closedcappedhairlines.cpp#L124-L127 (chrome/m156)
crate::def_simple_gm!(hairlines_roundcap, canvas, WIDTH, HEIGHT, {
    draw_hairline_contours_with_caps(canvas, Cap::Round);
});

// Port of: gm/closedcappedhairlines.cpp#L129-L132 (chrome/m156)
crate::def_simple_gm!(hairlines_squarecap, canvas, WIDTH, HEIGHT, {
    draw_hairline_contours_with_caps(canvas, Cap::Square);
});
