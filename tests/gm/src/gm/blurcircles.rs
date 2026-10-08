// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/blurcircles.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;

const NUM_BLURS: usize = 4;

// Port of: gm/blurcircles.cpp#L28-L77 (chrome/m156)
struct BlurCirclesGm {
    blur_filters: [Option<MaskFilter>; NUM_BLURS],
}

impl BlurCirclesGm {
    fn new() -> Self {
        BlurCirclesGm {
            blur_filters: [None, None, None, None],
        }
    }
}

impl GM for BlurCirclesGm {
    fn name(&self) -> String {
        "blurcircles".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(950, 950)
    }

    fn on_once_before_draw(&mut self) {
        const BLUR_RADII: [f32; NUM_BLURS] = [1.0, 5.0, 10.0, 20.0];

        for (filter, radius) in self.blur_filters.iter_mut().zip(BLUR_RADII) {
            *filter = MaskFilter::blur(
                BlurStyle::Normal,
                BlurMask::convert_radius_to_sigma(radius),
                None,
            );
        }
    }

    #[allow(clippy::cast_precision_loss)] // size_t -> float arithmetic as in C++
    fn on_draw(&mut self, canvas: &Canvas) {
        const CIRCLE_RADII: [f32; 4] = [5.0, 10.0, 25.0, 50.0];
        const CENTER: Point = Point { x: 50.0, y: 50.0 };

        canvas.scale((1.5, 1.5));
        canvas.translate((50.0, 50.0));

        for (i, blur_filter) in self.blur_filters.iter().enumerate() {
            canvas.save(); // SkAutoCanvasRestore autoRestore(canvas, true);
            canvas.translate((0.0, 150.0 * i as f32));
            for (j, &circle_radius) in CIRCLE_RADII.iter().enumerate() {
                let mut paint = Paint::default();
                paint.set_color(Color::BLACK);
                paint.set_mask_filter(blur_filter.clone());

                // Throw a rotation in the mix to make sure GPU fast path handles it correctly.
                canvas.save();
                canvas.rotate(j as f32 * 22.0, Some(CENTER));
                canvas.draw_circle(CENTER, circle_radius, &paint);
                canvas.restore();
                canvas.translate((150.0, 0.0));
            }
            canvas.restore();
        }
    }
}

// Port of: gm/blurcircles.cpp#L79 (chrome/m156)
crate::def_gm!(BlurCirclesGM_ = "BlurCirclesGM()", BlurCirclesGm::new());
