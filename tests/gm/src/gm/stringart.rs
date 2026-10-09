// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/stringart.cpp (chrome/m156)

// Reproduces https://code.google.com/p/chromium/issues/detail?id=279014

// The int/float mixing mirrors the C++ arithmetic of the GM.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use crate::tool_utils::color_to_565;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{SCALAR_PI, scalar, scalar_cos, scalar_sin};

const WIDTH: i32 = 440;
const HEIGHT: i32 = 440;
const ANGLE: scalar = 0.305;
// Port of: gm/stringart.cpp (chrome/m156), kMaxNumSteps
const MAX_NUM_STEPS: i32 = 140;

// Renders a string art shape.
// The particular shape rendered can be controlled by adjusting kAngle, from 0 to 1
// Port of: gm/stringart.cpp (chrome/m156), class StringArtGM
#[derive(Debug)]
pub struct StringArtGm {
    num_steps: i32,
}

impl StringArtGm {
    // Port of: gm/stringart.cpp (chrome/m156), StringArtGM()
    #[must_use]
    pub fn new() -> Self {
        Self {
            num_steps: MAX_NUM_STEPS,
        }
    }
}

impl Default for StringArtGm {
    fn default() -> Self {
        Self::new()
    }
}

impl GM for StringArtGm {
    fn name(&self) -> String {
        "stringart".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    // Port of: gm/stringart.cpp (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let angle = ANGLE * SCALAR_PI + (SCALAR_PI / 2.0);
        let size = WIDTH.min(HEIGHT) as scalar;
        let center = Point::new(WIDTH as scalar / 2.0, HEIGHT as scalar / 2.0);
        let mut length: scalar = 5.0;
        let mut step = angle;

        let mut builder = PathBuilder::new();
        builder.move_to(center);

        let mut i = 0;
        while i < self.num_steps && length < ((size / 2.0) - 10.0) {
            let rp = Point::new(
                length * scalar_cos(step) + center.x,
                length * scalar_sin(step) + center.y,
            );
            builder.line_to(rp);
            length += angle / (SCALAR_PI / 2.0);
            step += angle;
            i += 1;
        }

        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_style(Style::Stroke);
        paint.set_color(color_to_565(Color::new(0xFF00_7700)));

        canvas.draw_path(&builder.detach(), &paint);
    }
}

// Port of: gm/stringart.cpp (chrome/m156), DEF_GM( return new StringArtGM; )
crate::def_gm!(StringArtGM, StringArtGm::new());
