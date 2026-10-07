// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/complexclip2.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::scalar::scalar_round_to_int;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Clip {
    Rect,
    RRect,
    Path,
}

const ROWS: usize = 5;
const COLS: usize = 5;
const PAD_X: i32 = 20;
const PAD_Y: i32 = 20;

// Port of: gm/complexclip2.cpp#L28-L219 (chrome/m156)
struct ComplexClip2GM {
    clip: Clip,
    anti_alias: bool,
    rects: [Rect; 5],
    rrects: [RRect; 5],
    paths: [Path; 5],
    rect_colors: [Color; 5],
    ops: [[ClipOp; 5]; ROWS * COLS],
    width: f32,
    height: f32,
    total_width: f32,
    total_height: f32,
    bg: Color,
}

impl ComplexClip2GM {
    // Port of: gm/complexclip2.cpp#L36-L50 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // int -> SkScalar arithmetic as in C++
    fn new(clip: Clip, anti_alias: bool) -> Self {
        let x_a: f32 = 0.65;
        let x_f: f32 = 50.65;

        let y_a: f32 = 0.65;
        let y_f: f32 = 50.65;

        let width = x_f - x_a;
        let height = y_f - y_a;

        let total_width = COLS as f32 * width + 1.0 * (COLS + 1) as f32 * PAD_X as f32;
        let total_height = ROWS as f32 * height + 1.0 * (ROWS + 1) as f32 * PAD_Y as f32;

        Self {
            clip,
            anti_alias,
            rects: [Rect::default(); 5],
            rrects: [RRect::default(); 5],
            paths: [
                Path::default(),
                Path::default(),
                Path::default(),
                Path::default(),
                Path::default(),
            ],
            rect_colors: [Color::BLACK; 5],
            ops: [[ClipOp::Intersect; 5]; ROWS * COLS],
            width,
            height,
            total_width,
            total_height,
            bg: Color::WHITE,
        }
    }

    // Port of: gm/complexclip2.cpp#L100-L113 (chrome/m156)
    fn clip_str(clip: Clip) -> &'static str {
        match clip {
            Clip::Rect => "rect",
            Clip::RRect => "rrect",
            Clip::Path => "path",
        }
    }
}

impl GM for ComplexClip2GM {
    // Port of: gm/complexclip2.cpp#L115-L126 (chrome/m156)
    fn name(&self) -> String {
        if Clip::Rect == self.clip && !self.anti_alias {
            return "complexclip2".to_owned();
        }

        format!(
            "complexclip2_{}_{}",
            Self::clip_str(self.clip),
            if self.anti_alias { "aa" } else { "bw" }
        )
    }

    // Port of: gm/complexclip2.cpp#L128-L131 (chrome/m156)
    fn size(&mut self) -> ISize {
        ISize::new(
            scalar_round_to_int(self.total_width),
            scalar_round_to_int(self.total_height),
        )
    }

    fn bg_color(&self) -> Color {
        self.bg
    }

    // Port of: gm/complexclip2.cpp#L53-L98 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        self.bg = Color::from_rgb(0xDD, 0xA0, 0xDD);

        // offset the rects a bit so we get antialiasing even in the rect case
        let x_a: f32 = 0.65;
        let x_b: f32 = 10.65;
        let x_c: f32 = 20.65;
        let x_d: f32 = 30.65;
        let x_e: f32 = 40.65;
        let x_f: f32 = 50.65;

        let y_a: f32 = 0.65;
        let y_b: f32 = 10.65;
        let y_c: f32 = 20.65;
        let y_d: f32 = 30.65;
        let y_e: f32 = 40.65;
        let y_f: f32 = 50.65;

        self.rects[0] = Rect::from_ltrb(x_b, y_b, x_e, y_e);
        self.rrects[0] = RRect::new_rect_xy(self.rects[0], 7.0, 7.0);
        self.paths[0] = Path::rrect_xy(self.rects[0], 5.0, 5.0, None);
        self.rect_colors[0] = Color::RED;

        self.rects[1] = Rect::from_ltrb(x_a, y_a, x_d, y_d);
        self.rrects[1] = RRect::new_rect_xy(self.rects[1], 7.0, 7.0);
        self.paths[1] = Path::rrect_xy(self.rects[1], 5.0, 5.0, None);
        self.rect_colors[1] = Color::GREEN;

        self.rects[2] = Rect::from_ltrb(x_c, y_a, x_f, y_d);
        self.rrects[2] = RRect::new_rect_xy(self.rects[2], 7.0, 7.0);
        self.paths[2] = Path::rrect_xy(self.rects[2], 5.0, 5.0, None);
        self.rect_colors[2] = Color::BLUE;

        self.rects[3] = Rect::from_ltrb(x_a, y_c, x_d, y_f);
        self.rrects[3] = RRect::new_rect_xy(self.rects[3], 7.0, 7.0);
        self.paths[3] = Path::rrect_xy(self.rects[3], 5.0, 5.0, None);
        self.rect_colors[3] = Color::YELLOW;

        self.rects[4] = Rect::from_ltrb(x_c, y_c, x_f, y_f);
        self.rrects[4] = RRect::new_rect_xy(self.rects[4], 7.0, 7.0);
        self.paths[4] = Path::rrect_xy(self.rects[4], 5.0, 5.0, None);
        self.rect_colors[4] = Color::CYAN;

        let ops = [ClipOp::Difference, ClipOp::Intersect];

        let mut r = Random::default();
        for i in 0..ROWS {
            for j in 0..COLS {
                for k in 0..5 {
                    self.ops[j * ROWS + i][k] = ops[(r.next_u() % 2) as usize];
                }
            }
        }
    }

    // Port of: gm/complexclip2.cpp#L133-L193 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // int -> SkScalar arithmetic as in C++
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut rect_paint = Paint::default();
        rect_paint.set_style(Style::Stroke);
        rect_paint.set_stroke_width(-1.0);

        let mut fill_paint = Paint::default();
        fill_paint.set_color(Color::from_rgb(0xA0, 0xDD, 0xA0));

        for i in 0..ROWS {
            for j in 0..COLS {
                canvas.save();

                canvas.translate((
                    PAD_X as f32 * 1.0 + (self.width + PAD_X as f32 * 1.0) * j as f32,
                    PAD_Y as f32 * 1.0 + (self.height + PAD_Y as f32 * 1.0) * i as f32,
                ));

                // draw the original shapes first so we can see the
                // antialiasing on the clipped draw
                for k in 0..5 {
                    rect_paint.set_color(self.rect_colors[k]);
                    match self.clip {
                        Clip::Rect => {
                            canvas.draw_rect(self.rects[k], &rect_paint);
                        }
                        Clip::RRect => {
                            canvas.draw_rrect(self.rrects[k], &rect_paint);
                        }
                        Clip::Path => {
                            canvas.draw_path(&self.paths[k], &rect_paint);
                        }
                    }
                }

                for k in 0..5 {
                    let op = self.ops[j * ROWS + i][k];
                    match self.clip {
                        Clip::Rect => {
                            canvas.clip_rect(self.rects[k], op, self.anti_alias);
                        }
                        Clip::RRect => {
                            canvas.clip_rrect(self.rrects[k], op, self.anti_alias);
                        }
                        Clip::Path => {
                            canvas.clip_path(&self.paths[k], op, self.anti_alias);
                        }
                    }
                }
                canvas.draw_rect(Rect::from_wh(self.width, self.height), &fill_paint);
                canvas.restore();
            }
        }
    }
}

// bw
// Port of: gm/complexclip2.cpp#L226-L228 (chrome/m156)
crate::def_gm!(
    ComplexClip2GM_rect_false = "ComplexClip2GM(ComplexClip2GM::kRect_Clip, false)",
    ComplexClip2GM::new(Clip::Rect, false)
);
crate::def_gm!(
    ComplexClip2GM_rrect_false = "ComplexClip2GM(ComplexClip2GM::kRRect_Clip, false)",
    ComplexClip2GM::new(Clip::RRect, false)
);
crate::def_gm!(
    ComplexClip2GM_path_false = "ComplexClip2GM(ComplexClip2GM::kPath_Clip, false)",
    ComplexClip2GM::new(Clip::Path, false)
);

// aa
// Port of: gm/complexclip2.cpp#L231-L233 (chrome/m156)
crate::def_gm!(
    ComplexClip2GM_rect_true = "ComplexClip2GM(ComplexClip2GM::kRect_Clip, true)",
    ComplexClip2GM::new(Clip::Rect, true)
);
crate::def_gm!(
    ComplexClip2GM_rrect_true = "ComplexClip2GM(ComplexClip2GM::kRRect_Clip, true)",
    ComplexClip2GM::new(Clip::RRect, true)
);
crate::def_gm!(
    ComplexClip2GM_path_true = "ComplexClip2GM(ComplexClip2GM::kPath_Clip, true)",
    ComplexClip2GM::new(Clip::Path, true)
);
