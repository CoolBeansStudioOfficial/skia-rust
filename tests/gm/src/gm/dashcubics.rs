// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/dashcubics.cpp (chrome/m156)

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
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::paint::{Cap, Join, Paint};
use skia_rust_core::path::Path;
use skia_rust_core::scalar::{scalar, scalar_floor_to_scalar};
use skia_rust_core::utils::parse_path;
use skia_rust_core::utils::parse_path::from_svg;
use skia_rust_effects::dash_path_effect;
use skia_rust_effects::trim_path_effect::{self, Mode};

// Inspired by http://code.google.com/p/chromium/issues/detail?id=112145
// Port of: gm/dashcubics.cpp#L28-L45 (chrome/m156)
fn flower(canvas: &Canvas, path: &Path, intervals: &[f32; 2], join: Join) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_stroke(true);
    paint.set_stroke_join(join);
    paint.set_stroke_width(42.0);
    canvas.draw_path(path, &paint);

    paint.set_color(Color::RED);
    paint.set_stroke_width(21.0);
    paint.set_path_effect(dash_path_effect::new(intervals, 0.0));
    canvas.draw_path(path, &paint);

    paint.set_color(Color::GREEN);
    paint.set_path_effect(None);
    paint.set_stroke_width(0.0);
    canvas.draw_path(path, &paint);
}

// Port of: gm/dashcubics.cpp#L47-L74 (chrome/m156)
crate::def_simple_gm!(dashcubics, canvas, 865, 750, {
    let d = "M 337,98 C 250,141 250,212 250,212 C 250,212 250,212 250,212\
             C 250,212 250,212 250,212 C 250,212 250,141 163,98 C 156,195 217,231 217,231\
             C 217,231 217,231 217,231 C 217,231 217,231 217,231 C 217,231 156,195 75,250\
             C 156,305 217,269 217,269 C 217,269 217,269 217,269 C 217,269 217,269 217,269\
             C 217,269 156,305 163,402 C 250,359 250,288 250,288 C 250,288 250,288 250,288\
             C 250,288 250,288 250,288 C 250,288 250,359 338,402 C 345,305 283,269 283,269\
             C 283,269 283,269 283,269 C 283,269 283,269 283,269 C 283,269 345,305 425,250\
             C 344,195 283,231 283,231 C 283,231 283,231 283,231 C 283,231 283,231 283,231\
             C 283,231 344,195 338,98";

    let path = parse_path::from_svg(d).unwrap_or_default();
    canvas.translate((-35.0, -55.0));
    for x in 0..2 {
        for y in 0..2 {
            canvas.save();
            #[allow(clippy::cast_precision_loss)] // x * 430.f
            canvas.translate((x as f32 * 430.0, y as f32 * 355.0));
            let intervals = [5.0 + (if x != 0 { 0.0 } else { 0.0001 + 0.0001 }), 10.0];
            flower(
                canvas,
                &path,
                &intervals,
                if y != 0 { Join::Miter } else { Join::Round },
            );
            canvas.restore();
        }
    }
});

// skia-rust (TrimGM): `onAnimate` is not ported, because the `GM` trait has no animate hook. `fOffset` is
// set only by `onAnimate`, and at animation time zero it is `0`, so the `fOffset` branch of
// `onDraw` is never taken here and `offset` stays `0`.
// Port of: gm/dashcubics.cpp#L76-L176 (chrome/m156), TrimGM
struct TrimGm {
    paths: Vec<Path>,
    offset: scalar,
}

impl TrimGm {
    fn new() -> Self {
        Self {
            paths: Vec::new(),
            offset: 0.0,
        }
    }
}

impl GM for TrimGm {
    // Port of: gm/dashcubics.cpp#L80-L103 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        // The C++ string literals are adjacent, so they concatenate without separators.
        self.paths.push(
            from_svg(concat!(
                "M   0,100 C  10, 50 190, 50 200,100",
                "M 200,100 C 210,150 390,150 400,100",
                "M 400,100 C 390, 50 210, 50 200,100",
                "M 200,100 C 190,150  10,150   0,100",
            ))
            .expect("SkAssertResult"),
        );
        self.paths.push(
            from_svg(concat!(
                "M   0, 75 L 200, 75",
                "M 200, 91 L 200, 91",
                "M 200,108 L 200,108",
                "M 200,125 L 400,125",
            ))
            .expect("SkAssertResult"),
        );
        self.paths.push(
            from_svg(concat!(
                "M   0,100 L  50, 50",
                "M  50, 50 L 150,150",
                "M 150,150 L 250, 50",
                "M 250, 50 L 350,150",
                "M 350,150 L 400,100",
            ))
            .expect("SkAssertResult"),
        );
    }

    // Port of: gm/dashcubics.cpp#L106-L106 (chrome/m156), getName
    fn name(&self) -> String {
        "trimpatheffect".to_owned()
    }

    // Port of: gm/dashcubics.cpp#L108-L108 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(1400, 1000)
    }

    // Port of: gm/dashcubics.cpp#L110-L163 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        const K_CELL_SIZE: (scalar, scalar) = (440.0, 150.0);
        const K_OFFSETS: [[scalar; 2]; 6] = [
            [-0.33, -0.66],
            [0.0, 1.0],
            [0.0, 0.25],
            [0.25, 0.75],
            [0.75, 1.0],
            [1.0, 0.75],
        ];

        let mut hairline_paint = Paint::default();
        hairline_paint.set_anti_alias(true);
        hairline_paint.set_stroke(true);
        hairline_paint.set_stroke_cap(Cap::Round);
        hairline_paint.set_stroke_width(2.0);

        let mut normal_paint = hairline_paint.clone();
        normal_paint.set_stroke_width(10.0);
        normal_paint.set_color(Color::new(0x8000_ff00));

        let mut inverted_paint = normal_paint.clone();
        inverted_paint.set_color(Color::new(0x80ff_0000));

        for offset in K_OFFSETS {
            let mut start = offset[0] + self.offset;
            let mut stop = offset[1] + self.offset;
            let mut normal_mode = Mode::Normal;
            let mut inverted_mode = Mode::Inverted;
            if self.offset != 0.0 {
                start -= scalar_floor_to_scalar(start);
                stop -= scalar_floor_to_scalar(stop);
                if start > stop {
                    std::mem::swap(&mut start, &mut stop);
                    std::mem::swap(&mut normal_mode, &mut inverted_mode);
                }
            }
            normal_paint.set_path_effect(trim_path_effect::new(start, stop, normal_mode));
            inverted_paint.set_path_effect(trim_path_effect::new(start, stop, inverted_mode));
            {
                canvas.save();
                for path in &self.paths {
                    canvas.draw_path(path, &normal_paint);
                    canvas.draw_path(path, &inverted_paint);
                    canvas.draw_path(path, &hairline_paint);
                    canvas.translate((K_CELL_SIZE.0, 0.0));
                }
                canvas.restore();
            }
            canvas.translate((0.0, K_CELL_SIZE.1));
        }
    }
}

// Port of: gm/dashcubics.cpp#L177-L177 (chrome/m156)
crate::def_gm!(TrimGM_ = "TrimGM", TrimGm::new());
