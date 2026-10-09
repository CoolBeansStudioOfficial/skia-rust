// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/manypaths.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::color::HSV;
use skia_rust_core::paint::Paint;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

const K_X_LIMIT: i32 = 700;
const K_Y_INCREMENT: i32 = 5;
const K_X_INCREMENT: i32 = 5;

// This GM attempts to flood Ganesh with more rrects than will fit in a single index buffer
// Stresses crbug.com/684112
// Port of: gm/manypaths.cpp#L69-L112 (chrome/m156)
struct ManyRRectsGM;

impl GM for ManyRRectsGM {
    fn name(&self) -> String {
        "manyrrects".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(800, 300)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFFF_FFFF)
    }

    #[allow(clippy::cast_precision_loss, clippy::similar_names)] // SkIntToScalar; C++ names rect/rrect
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(Color::BLUE);
        let mut remaining = 7000;

        // Rectangle positioning variables
        let mut x: i32 = 0;
        let mut y: i32 = 0;

        let rect = Rect::new(0.0, 0.0, 4.0, 4.0);
        let rrect = RRect::new_rect_xy(rect, 1.0, 1.0);
        while remaining != 0 {
            remaining -= 1;
            canvas.save();
            canvas.translate((x as f32, y as f32));
            canvas.draw_rrect(rrect, &paint);
            x += K_X_INCREMENT;
            if x > K_X_LIMIT {
                x = 0;
                y += K_Y_INCREMENT;
            }
            canvas.restore();
        }
    }
}

// Port of: gm/manypaths.cpp#L116-L117 (chrome/m156)
crate::def_gm!(ManyRRectsGM_ = "ManyRRectsGM", ManyRRectsGM);

// Port of: gm/manypaths.cpp#L14-L21 (chrome/m156), gen_color
fn gen_color(rand: &mut Random) -> Color {
    let h = rand.next_range_f(0.0, 360.0);
    let s = rand.next_range_f(0.5, 1.0);
    let v = rand.next_range_f(0.5, 1.0);
    crate::tool_utils::color_to_565(HSV { h, s, v }.to_color(0xFF))
}

// This GM attempts to flood Ganesh with more circles than will fit in a single index buffer
// Stresses crbug.com/688582.
// Port of: gm/manypaths.cpp#L23-L53 (chrome/m156), class ManyCirclesGM
struct ManyCirclesGm;

impl GM for ManyCirclesGm {
    fn name(&self) -> String {
        "manycircles".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(800, 600)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFFF_FFFF)
    }

    // Port of: gm/manypaths.cpp#L36-L49 (chrome/m156), onDraw
    #[allow(clippy::cast_precision_loss)] // SkScalar arithmetic on the int size constants
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut rand = Random::new(1);
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        let mut total = 10000;
        while total != 0 {
            total -= 1;
            let x = rand.next_f() * 800.0 - 100.0;
            let y = rand.next_f() * 600.0 - 100.0;
            let w = rand.next_f() * 200.0;
            let circle = Rect::from_xywh(x, y, w, w);
            paint.set_color(gen_color(&mut rand));
            canvas.draw_oval(circle, &paint);
        }
    }
}

// Port of: gm/manypaths.cpp#L116-L116 (chrome/m156)
crate::def_gm!(ManyCirclesGM, ManyCirclesGm);
