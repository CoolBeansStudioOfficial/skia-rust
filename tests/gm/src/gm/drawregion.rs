// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drawregion.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::{Op, Region};

// Port of: gm/drawregion.cpp#L17-L55 (chrome/m156)
struct DrawRegionGm {
    region: Region,
}

impl DrawRegionGm {
    fn new() -> Self {
        DrawRegionGm {
            region: Region::new(),
        }
    }
}

impl GM for DrawRegionGm {
    fn name(&self) -> String {
        "drawregion".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(500, 500)
    }

    fn on_once_before_draw(&mut self) {
        for x in (50..250).step_by(2) {
            for y in (50..250).step_by(2) {
                self.region
                    .op_rect(IRect::from_ltrb(x, y, x + 1, y + 1), Op::Union);
            }
        }
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((10.0, 10.0));

        let mut paint = Paint::default();
        paint.set_style(Style::Fill);
        paint.set_color(Color::from(0xFFFF_00FF_u32));
        canvas.draw_rect(Rect::from_ltrb(50.0, 50.0, 250.0, 250.0), &paint);

        paint.set_color(Color::from(0xFF00_FFFF_u32));
        canvas.draw_region(&self.region, &paint);
    }
}

// Port of: gm/drawregion.cpp#L57 (chrome/m156)
crate::def_gm!(DrawRegionGM, DrawRegionGm::new());
