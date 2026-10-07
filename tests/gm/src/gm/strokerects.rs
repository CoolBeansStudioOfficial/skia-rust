// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/strokerects.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;

// Port of: gm/strokerects.cpp#L22-L27 (chrome/m156)
const W: i32 = 400;
const H: i32 = 400;
const N: i32 = 100;

#[allow(clippy::cast_precision_loss)] // SkIntToScalar
const SW: f32 = W as f32;
#[allow(clippy::cast_precision_loss)] // SkIntToScalar
const SH: f32 = H as f32;

// Port of: gm/strokerects.cpp#L29-L92 (chrome/m156)
struct StrokeRectsGm {
    rotated: bool,
}

impl StrokeRectsGm {
    fn new(rotated: bool) -> Self {
        Self { rotated }
    }

    // Port of: gm/strokerects.cpp#L48-L58 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // int to float conversions as in C++
    fn rnd_rect(r: &mut Rect, rand: &mut Random) {
        let x = rand.next_u_scalar1() * W as f32;
        let y = rand.next_u_scalar1() * H as f32;
        let w = rand.next_u_scalar1() * (W >> 2) as f32;
        let h = rand.next_u_scalar1() * (H >> 2) as f32;
        let hoffset = rand.next_s_scalar1();
        let woffset = rand.next_s_scalar1();

        r.set_xywh(x, y, w, h);
        r.offset((-w / 2.0 + woffset, -h / 2.0 + hoffset));
    }
}

impl GM for StrokeRectsGm {
    fn name(&self) -> String {
        if self.rotated {
            "strokerects_rotated".to_string()
        } else {
            "strokerects".to_string()
        }
    }

    fn size(&mut self) -> ISize {
        ISize::new(W * 2, H * 2)
    }

    #[allow(clippy::cast_precision_loss)] // x * SkIntToScalar(3), SW * x
    fn on_draw(&mut self, canvas: &Canvas) {
        if self.rotated {
            canvas.rotate(45.0, Some(Point::new(SW, SH)));
        }

        let mut paint = Paint::default();
        paint.set_style(Style::Stroke);

        for y in 0..2 {
            paint.set_anti_alias(y != 0);
            for x in 0..2 {
                paint.set_stroke_width(x as f32 * 3.0);

                let _acr = AutoCanvasRestore::guard(canvas, true);
                canvas.translate((SW * x as f32, SH * y as f32));
                canvas.clip_rect(Rect::from_ltrb(2.0, 2.0, SW - 2.0, SH - 2.0), None, None);

                let mut rand = Random::default();
                for _ in 0..N {
                    let mut r = Rect::new_empty();
                    Self::rnd_rect(&mut r, &mut rand);
                    canvas.draw_rect(r, &paint);
                }
            }
        }
    }
}

// Port of: gm/strokerects.cpp#L88-L89 (chrome/m156)
crate::def_gm!(StrokeRectsGM_false = "StrokeRectsGM(false)", StrokeRectsGm::new(false));
crate::def_gm!(StrokeRectsGM_true = "StrokeRectsGM(true)", StrokeRectsGm::new(true));
