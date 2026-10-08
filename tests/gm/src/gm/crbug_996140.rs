// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_996140.cpp (chrome/m156)

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
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::Rect;

// Canvas example. Expected large blue stroked circle, white middle, small red circle.
// GPU-accelerated canvas produces large blue stroked circle, white middle, NO red circle.
//
// 1:  var c = document.getElementById("myCanvas");
// 2:  var ctx = c.getContext("2d");
// 3:  ctx.beginPath();
// 4:  ctx.scale(203.20, 203.20);
// 5:  ctx.translate(-14.55, -711.51);
// 6:  ctx.fillStyle = "red";
// 7:  ctx.strokeStyle = "blue";
// 8:  //ctx.lineWidth = 1/203.20;
// 9:  ctx.arc(19.221, 720-6.76,0.0295275590551181,0,2*Math.PI);
// 10: ctx.stroke();
// 11: ctx.fill();
// 12: ctx.closePath();
// Port of: gm/crbug_996140.cpp#L31-L80 (chrome/m156)
#[allow(clippy::excessive_precision)] // literals as written in the C++ source
fn draw_crbug_996140(canvas: &Canvas) {
    // Specific parameters taken from the canvas minimum working example
    let cx: f32 = 19.221;
    let cy: f32 = 720.0 - 6.76;
    let radius: f32 = 0.029_527_559_055_118_1;

    let s: f32 = 203.20;
    let tx: f32 = -14.55;
    let ty: f32 = -711.51;

    // 0: The test canvas was 1920x574 and the circle was located in the bottom left, but that's
    // not necessary to reproduce the problem, so translate to make a smaller GM.
    canvas.translate((-800.0, -200.0));

    // 3: ctx.beginPath();

    // 4: ctx.scale(203.20, 203.20);
    canvas.scale((s, s));
    // 5: ctx.translate(-14.55, -711.51);
    canvas.translate((tx, ty));

    // 6: ctx.fillStyle = "red";
    let mut fill = Paint::default();
    fill.set_color(Color::RED);
    fill.set_style(Style::Fill);
    fill.set_anti_alias(true);

    // 7: ctx.strokeStyle = "blue";
    let mut stroke = Paint::default();
    stroke.set_color(Color::BLUE);
    stroke.set_stroke_width(1.0);
    stroke.set_style(Style::Stroke);
    stroke.set_anti_alias(true);

    // 9: ctx.arc(19.221, 720-6.76,0.0295275590551181,0,2*Math.PI);
    // This matches how Canvas prepares an arc(x, y, radius, 0, 2pi) call
    let bounding_box = Rect::from_ltrb(cx - radius, cy - radius, cx + radius, cy + radius);

    let path = PathBuilder::new()
        .arc_to(bounding_box, 0.0, 180.0, false)
        .arc_to(bounding_box, 180.0, 180.0, false)
        .detach();

    // 12: ctx.closePath();
    // path.close();

    // 10: ctx.stroke(); (NOT NEEDED TO REPRODUCE FAILING RED CIRCLE)
    canvas.draw_path(&path, &stroke);
    // 11: ctx.fill()
    canvas.draw_path(&path, &fill);
}

// Port of: gm/crbug_996140.cpp#L31-L80 (chrome/m156)
crate::def_simple_gm_bg!(crbug_996140, canvas, 300, 300, Color::WHITE, {
    draw_crbug_996140(canvas);
});
