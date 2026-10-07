// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/arcofzorro.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;

// This GM draws a lot of arcs in a 'Z' shape. It particularly exercises
// the 'drawArc' code near a singularly of its processing (i.e., near the
// edge of one of its underlying quads).
// Port of: gm/arcofzorro.cpp#L21-L81 (chrome/m156)
struct ArcOfZorroGm;

impl GM for ArcOfZorroGm {
    fn name(&self) -> String {
        "arcofzorro".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1000, 1000)
    }

    fn bg_color(&self) -> Color {
        Color::new(0xFFCCCCCC)
    }

    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut rand = Random::default();

        let rect = Rect::from_xywh(10.0, 10.0, 200.0, 200.0);

        let mut p = Paint::default();

        p.set_style(Style::Stroke);
        p.set_stroke_width(35.0);
        let mut x_offset: i32 = 0;
        let mut y_offset: i32 = 0;
        let mut direction = 0;

        let mut arc: f32 = 134.0;
        while arc < 136.0 {
            let mut color = rand.next_u();
            color |= 0xff00_0000;
            p.set_color(Color::new(color));

            canvas.save();
            canvas.translate((x_offset as f32, y_offset as f32));
            canvas.draw_arc(rect, 0.0, arc, false, &p);
            canvas.restore();

            match direction {
                0 => {
                    x_offset += 10;
                    if x_offset >= 700 {
                        direction = 1;
                    }
                }
                1 => {
                    x_offset -= 10;
                    y_offset += 10;
                    if x_offset < 50 {
                        direction = 2;
                    }
                }
                2 => {
                    x_offset += 10;
                }
                _ => {}
            }
            arc += 0.01;
        }
    }
}

// Port of: gm/arcofzorro.cpp#L85 (chrome/m156)
crate::def_gm!(ArcOfZorroGM, ArcOfZorroGm);
