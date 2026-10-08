// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/beziers.cpp (chrome/m156)

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
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::random::Random;

const W: i32 = 400;
const H: i32 = 400;
const N: i32 = 10;

// Port of: gm/beziers.cpp#L23-L42 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // int to scalar, as in C++
fn rnd_quad(paint: &mut Paint, rand: &mut Random) -> Path {
    let a = rand.next_range_scalar(0.0, W as f32);
    let b = rand.next_range_scalar(0.0, H as f32);

    let mut builder = PathBuilder::new();
    builder.move_to((a, b));
    for _x in 0..2 {
        let c = rand.next_range_scalar((W / 4) as f32, W as f32);
        let d = rand.next_range_scalar(0.0, H as f32);
        let e = rand.next_range_scalar(0.0, W as f32);
        let f = rand.next_range_scalar((H / 4) as f32, H as f32);
        builder.quad_to((c, d), (e, f));
    }
    paint.set_color(Color::new(rand.next_u()));
    let mut width = rand.next_range_scalar(1.0, 5.0);
    width *= width;
    paint.set_stroke_width(width);
    paint.set_alpha_f(1.0);
    builder.detach()
}

// Port of: gm/beziers.cpp#L44-L65 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // int to scalar, as in C++
fn rnd_cubic(paint: &mut Paint, rand: &mut Random) -> Path {
    let a = rand.next_range_scalar(0.0, W as f32);
    let b = rand.next_range_scalar(0.0, H as f32);

    let mut builder = PathBuilder::new();
    builder.move_to((a, b));
    for _x in 0..2 {
        let c = rand.next_range_scalar((W / 4) as f32, W as f32);
        let d = rand.next_range_scalar(0.0, H as f32);
        let e = rand.next_range_scalar(0.0, W as f32);
        let f = rand.next_range_scalar((H / 4) as f32, H as f32);
        let g = rand.next_range_scalar((W / 4) as f32, W as f32);
        let h = rand.next_range_scalar((H / 4) as f32, H as f32);
        builder.cubic_to((c, d), (e, f), (g, h));
    }
    paint.set_color(Color::new(rand.next_u()));
    let mut width = rand.next_range_scalar(1.0, 5.0);
    width *= width;
    paint.set_stroke_width(width);
    paint.set_alpha_f(1.0);
    builder.detach()
}

// Port of: gm/beziers.cpp#L67-L94 (chrome/m156)
struct BeziersGm;

impl GM for BeziersGm {
    fn name(&self) -> String {
        "beziers".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(W, H * 2)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(9.0 / 2.0);
        paint.set_anti_alias(true);

        let mut rand = Random::default();
        for _ in 0..N {
            let path = rnd_quad(&mut paint, &mut rand);
            canvas.draw_path(&path, &paint);
        }
        canvas.translate((0.0, H as f32));
        for _ in 0..N {
            let path = rnd_cubic(&mut paint, &mut rand);
            canvas.draw_path(&path, &paint);
        }
    }
}

crate::def_gm!(BeziersGM, BeziersGm);
