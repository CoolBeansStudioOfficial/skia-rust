// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drrect.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Vector;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

// Port of: gm/drrect.cpp#L21-L71 (chrome/m156)
struct DRRectGM;

impl GM for DRRectGM {
    fn name(&self) -> String {
        "drrect".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        let mut outers = [RRect::default(); 4];
        // like squares/circles, to exercise fast-cases in GPU
        let mut r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let radii: [Vector; 4] = [
            Vector::new(0.0, 0.0),
            Vector::new(30.0, 1.0),
            Vector::new(10.0, 40.0),
            Vector::new(40.0, 40.0),
        ];

        let dx = r.width() + 16.0;
        let dy = r.height() + 16.0;

        outers[0].set_rect(r);
        outers[1].set_oval(r);
        outers[2].set_rect_xy(r, 20.0, 20.0);
        outers[3].set_rect_radii(r, &radii);

        let mut inners = [RRect::default(); 5];
        r.inset((25.0, 25.0));

        inners[0].set_empty();
        inners[1].set_rect(r);
        inners[2].set_oval(r);
        inners[3].set_rect_xy(r, 20.0, 20.0);
        inners[4].set_rect_radii(r, &radii);

        canvas.translate((16.0, 16.0));
        for (j, inner) in inners.iter().enumerate() {
            for (i, outer) in outers.iter().enumerate() {
                canvas.save();
                #[allow(clippy::cast_precision_loss)] // size_t -> SkScalar in the C++
                canvas.translate((dx * j as f32, dy * i as f32));
                canvas.draw_drrect(outer, inner, &paint);
                canvas.restore();
            }
        }
    }
}

// Port of: gm/drrect.cpp#L73 (chrome/m156)
crate::def_gm!(DRRectGM_ = "DRRectGM", DRRectGM);
