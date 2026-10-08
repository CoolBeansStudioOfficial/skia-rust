// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/simplerect.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::Paint;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;

// Port of: gm/simplerect.cpp#L16-L51 (chrome/m156)
struct SimpleRectGM;

impl GM for SimpleRectGM {
    fn name(&self) -> String {
        "simplerect".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(800, 800)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((1.0, 1.0)); // want to exercise non-identity ctm performance

        let min: f32 = -20.0;
        let max: f32 = 800.0;
        let size: f32 = 20.0;

        let mut rand = Random::default();
        let mut paint = Paint::default();
        for _ in 0..10000 {
            paint.set_color(crate::tool_utils::color_to_565(
                rand.next_u() | (0xFF << 24),
            ));
            let x = rand.next_range_scalar(min, max);
            let y = rand.next_range_scalar(min, max);
            let w = rand.next_range_scalar(0.0, size);
            let h = rand.next_range_scalar(0.0, size);
            canvas.draw_rect(Rect::from_xywh(x, y, w, h), &paint);
        }
    }
}

// Port of: gm/simplerect.cpp#L56 (chrome/m156)
crate::def_gm!(SimpleRectGM_ = "SimpleRectGM", SimpleRectGM);
