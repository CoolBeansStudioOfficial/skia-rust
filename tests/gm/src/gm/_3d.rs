// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/3d.cpp (chrome/m156)

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
use skia_rust_core::m44::{M44, V3};
use skia_rust_core::paint::Paint;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::SCALAR_PI;

// Port of: gm/3d.cpp#L16-L24 (chrome/m156)
struct Info {
    near: f32,
    far: f32,
    angle: f32,

    eye: V3,
    coa: V3,
    up: V3,
}

impl Info {
    fn new() -> Self {
        let angle = SCALAR_PI / 4.0;
        Self {
            near: 0.05,
            far: 4.0,
            angle,
            eye: V3::new(0.0, 0.0, 1.0 / (angle / 2.0).tan() - 1.0),
            coa: V3::new(0.0, 0.0, 0.0),
            up: V3::new(0.0, 1.0, 0.0),
        }
    }
}

// Port of: gm/3d.cpp#L26-L32 (chrome/m156)
fn inv(m: &M44) -> M44 {
    match m.invert() {
        Some(inverse) => inverse,
        None => M44::new_identity(),
    }
}

// Port of: gm/3d.cpp#L34-L45 (chrome/m156)
fn make_ctm(info: &Info, model: &M44, width: f32, height: f32) -> M44 {
    let perspective = M44::perspective(info.near, info.far, info.angle);
    let camera = M44::look_at(&info.eye, &info.coa, &info.up);
    let mut viewport = M44::new_identity();
    viewport.set_scale(width * 0.5, height * 0.5, 1.0);

    let a = M44::concat(&viewport, &perspective);
    let a = M44::concat(&a, &camera);
    let a = M44::concat(&a, model);
    M44::concat(&a, &inv(&viewport))
}

// Port of: gm/3d.cpp#L47-L60 (chrome/m156)
fn do_draw(canvas: &Canvas, color: Color) {
    let _acr = AutoCanvasRestore::guard(canvas, true);

    let info = Info::new();

    let m = M44::rotate(V3::new(0.0, 1.0, 0.0), SCALAR_PI / 6.0);

    canvas.concat_44(&make_ctm(&info, &m, 300.0, 300.0));

    canvas.translate((150.0, 150.0));
    let mut paint = Paint::default();
    paint.set_color(color);
    canvas.draw_rect(Rect::from_ltrb(-100.0, -100.0, 100.0, 100.0), &paint);
}

// Test calling drawables w/ translate and matrices
// Port of: gm/3d.cpp#L62-L75 (chrome/m156)
crate::def_simple_gm!(sk3d_simple, real_canvas, 300, 300, {
    do_draw(real_canvas, Color::new(0xFFFF_0000));

    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(Rect::from_wh(300.0, 300.0), false);

    do_draw(canvas, Color::new(0x880000FF));

    let pic = recorder
        .finish_recording_as_picture(None)
        .expect("recording was started");
    real_canvas.draw_picture(&pic, None, None);
});
