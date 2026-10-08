// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/stlouisarch.cpp (chrome/m156)

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

const K_WIDTH: f32 = 256.0;
const K_HEIGHT: f32 = 256.0;

// this GM tests hairlines which fill nearly the entire render target
// Port of: gm/stlouisarch.cpp#L22-L110 (chrome/m156)
struct StLouisArchGm {
    paths: Vec<Path>,
}

impl StLouisArchGm {
    // Port of: gm/stlouisarch.cpp#L29-L80 (chrome/m156)
    fn new() -> Self {
        let mut paths = Vec::new();
        paths.push(
            PathBuilder::new()
                .move_to((0.0, 0.0))
                .quad_to((K_WIDTH / 2.0, K_HEIGHT), (K_WIDTH, 0.0))
                .detach(),
        );

        {
            let y_pos = K_HEIGHT / 2.0 + 10.0;
            paths.push(
                PathBuilder::new()
                    .move_to((0.0, y_pos))
                    .quad_to((0.0, y_pos), (K_WIDTH, y_pos))
                    .detach(),
            );
        }

        paths.push(
            PathBuilder::new()
                .move_to((0.0, 0.0))
                .cubic_to((0.0, K_HEIGHT), (K_WIDTH, K_HEIGHT), (K_WIDTH, 0.0))
                .detach(),
        );

        {
            let y_pos = K_HEIGHT / 2.0;
            paths.push(
                PathBuilder::new()
                    .move_to((0.0, y_pos))
                    .cubic_to((0.0, y_pos), (0.0, y_pos), (K_WIDTH, y_pos))
                    .detach(),
            );
        }

        paths.push(
            PathBuilder::new()
                .move_to((0.0, 0.0))
                .conic_to((K_WIDTH / 2.0, K_HEIGHT), (K_WIDTH, 0.0), 0.5)
                .detach(),
        );

        {
            let y_pos = K_HEIGHT / 2.0 - 10.0;
            paths.push(
                PathBuilder::new()
                    .move_to((0.0, y_pos))
                    .conic_to((0.0, y_pos), (K_WIDTH, y_pos), 0.5)
                    .detach(),
            );
        }

        Self { paths }
    }
}

impl GM for StLouisArchGm {
    fn name(&self) -> String {
        "stlouisarch".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(K_WIDTH as i32, K_HEIGHT as i32)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.save();
        canvas.scale((1.0, -1.0));
        canvas.translate((0.0, -K_HEIGHT));
        for path in &self.paths {
            let mut paint = Paint::default();
            paint.set_argb(0xff, 0, 0, 0);
            paint.set_anti_alias(true);
            paint.set_style(Style::Stroke);
            paint.set_stroke_width(0.0);
            canvas.draw_path(path, &paint);
        }
        canvas.restore();
    }
}

crate::def_gm!(StLouisArchGM, StLouisArchGm::new());
