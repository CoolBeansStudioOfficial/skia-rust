// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drawregion.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::{Op, Region};

//  This is very similar to the RectGrid macrobench in Android.
// Port of: gm/drawregion.cpp#L16-L50 (chrome/m156)
struct DrawRegionGM {
    region: Region,
}

impl GM for DrawRegionGM {
    fn name(&self) -> String {
        "drawregion".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(500, 500)
    }

    fn on_once_before_draw(&mut self) {
        let mut x = 50;
        while x < 250 {
            let mut y = 50;
            while y < 250 {
                self.region
                    .op_rect(IRect::new(x, y, x + 1, y + 1), Op::Union);
                y += 2;
            }
            x += 2;
        }
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((10.0, 10.0));

        let mut paint = Paint::default();
        paint.set_style(Style::Fill);
        paint.set_color(Color::from(0xFFFF_00FF));
        canvas.draw_rect(Rect::new(50.0, 50.0, 250.0, 250.0), &paint);

        paint.set_color(Color::from(0xFF00_FFFF));
        canvas.draw_region(&self.region, &paint);
    }
}

// Port of: gm/drawregion.cpp#L52 (chrome/m156)
crate::def_gm!(
    DrawRegionGM_ = "DrawRegionGM",
    DrawRegionGM {
        region: Region::new()
    }
);
