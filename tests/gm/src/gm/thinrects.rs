// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/thinrects.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Vector;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

// Draw various width thin rects at 1/8 horizontal pixel increments
// Port of: gm/thinrects.cpp#L22-L141 (chrome/m156)
struct ThinRectsGM {
    round: bool,
}

impl ThinRectsGM {
    fn new(round: bool) -> Self {
        Self { round }
    }

    // Port of: gm/thinrects.cpp#L67-L94 (chrome/m156)
    fn draw_vert_rects(&self, canvas: &Canvas, p: &Paint) {
        let vert_rects = [
            Rect::new(1.0, 1.0, 5.0, 21.0),      // 4 pix wide
            Rect::new(8.0, 1.0, 10.0, 21.0),     // 2 pix wide
            Rect::new(13.0, 1.0, 14.0, 21.0),    // 1 pix wide
            Rect::new(17.0, 1.0, 17.5, 21.0),    // 1/2 pix wide
            Rect::new(21.0, 1.0, 21.25, 21.0),   // 1/4 pix wide
            Rect::new(25.0, 1.0, 25.125, 21.0),  // 1/8 pix wide
            Rect::new(29.0, 1.0, 29.0, 21.0),    // 0 pix wide
        ];

        let radii: [Vector; 4] = [
            Vector::new(1.0 / 32.0, 2.0 / 32.0),
            Vector::new(3.0 / 32.0, 1.0 / 32.0),
            Vector::new(2.0 / 32.0, 3.0 / 32.0),
            Vector::new(1.0 / 32.0, 3.0 / 32.0),
        ];
        let mut rrect = RRect::default();
        for rect in &vert_rects {
            if self.round {
                rrect.set_rect_radii(rect, &radii);
                canvas.draw_rrect(rrect, p);
            } else {
                canvas.draw_rect(rect, p);
            }
        }
    }

    // Port of: gm/thinrects.cpp#L96-L117 (chrome/m156)
    fn draw_horiz_rects(&self, canvas: &Canvas, p: &Paint) {
        let horiz_rects = [
            Rect::new(1.0, 1.0, 21.0, 5.0),      // 4 pix high
            Rect::new(1.0, 8.0, 21.0, 10.0),     // 2 pix high
            Rect::new(1.0, 13.0, 21.0, 14.0),    // 1 pix high
            Rect::new(1.0, 17.0, 21.0, 17.5),    // 1/2 pix high
            Rect::new(1.0, 21.0, 21.0, 21.25),   // 1/4 pix high
            Rect::new(1.0, 25.0, 21.0, 25.125),  // 1/8 pix high
            Rect::new(1.0, 29.0, 21.0, 29.0),    // 0 pix high
        ];

        let mut rrect = RRect::default();
        for rect in &horiz_rects {
            if self.round {
                rrect.set_nine_patch(rect, 1.0 / 32.0, 2.0 / 32.0, 3.0 / 32.0, 4.0 / 32.0);
                canvas.draw_rrect(rrect, p);
            } else {
                canvas.draw_rect(rect, p);
            }
        }
    }

    // Port of: gm/thinrects.cpp#L119-L139 (chrome/m156)
    fn draw_squares(&self, canvas: &Canvas, p: &Paint) {
        let squares = [
            Rect::new(1.0, 1.0, 5.0, 5.0),          // 4 pix
            Rect::new(8.0, 8.0, 10.0, 10.0),        // 2 pix
            Rect::new(13.0, 13.0, 14.0, 14.0),      // 1 pix
            Rect::new(17.0, 17.0, 17.5, 17.5),      // 1/2 pix
            Rect::new(21.0, 21.0, 21.25, 21.25),    // 1/4 pix
            Rect::new(25.0, 25.0, 25.125, 25.125),  // 1/8 pix
            Rect::new(29.0, 29.0, 29.0, 29.0),      // 0 pix
        ];

        let mut rrect = RRect::default();
        for rect in &squares {
            if self.round {
                rrect.set_rect_xy(rect, 1.0 / 32.0, 2.0 / 32.0);
                canvas.draw_rrect(rrect, p);
            } else {
                canvas.draw_rect(rect, p);
            }
        }
    }
}

impl GM for ThinRectsGM {
    fn name(&self) -> String {
        if self.round { "thinroundrects" } else { "thinrects" }.to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(240, 320)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFF00_0000)
    }

    // Port of: gm/thinrects.cpp#L39-L65 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // int * float in C++
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut white = Paint::default();
        white.set_color(Color::WHITE);
        white.set_anti_alias(true);

        let mut green = Paint::default();
        green.set_color(Color::GREEN);
        green.set_anti_alias(true);

        for i in 0..8 {
            let fi = i as f32;
            canvas.save();
            canvas.translate((fi * 0.125, fi * 40.0));
            self.draw_vert_rects(canvas, &white);

            canvas.translate((40.0, 0.0));
            self.draw_vert_rects(canvas, &green);
            canvas.restore();

            canvas.save();
            canvas.translate((80.0, fi * 40.0 + fi * 0.125));
            self.draw_horiz_rects(canvas, &white);

            canvas.translate((40.0, 0.0));
            self.draw_horiz_rects(canvas, &green);
            canvas.restore();

            canvas.save();
            canvas.translate((160.0 + fi * 0.125, fi * 40.0 + fi * 0.125));
            self.draw_squares(canvas, &white);

            canvas.translate((40.0, 0.0));
            self.draw_squares(canvas, &green);
            canvas.restore();
        }
    }
}

// Port of: gm/thinrects.cpp#L143-L144 (chrome/m156)
crate::def_gm!(ThinRectsGM_false = "ThinRectsGM(false)", ThinRectsGM::new(false));
crate::def_gm!(ThinRectsGM_true = "ThinRectsGM(true)", ThinRectsGM::new(true));
