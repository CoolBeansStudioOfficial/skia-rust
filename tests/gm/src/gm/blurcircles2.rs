// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/blurcircles2.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;

// Port of: gm/blurcircles2.cpp#L128-L135 (chrome/m156)
const MIN_RADIUS: scalar = 15.0;
const MAX_RADIUS: scalar = 45.0;

const MIN_BLUR_RADIUS: scalar = 5.0;
const MAX_BLUR_RADIUS: scalar = 45.0;

/**
 * In GM mode this draws an array of circles with different radii and different blur radii. Below
 * each circle an almost-circle path is drawn with the same blur filter for comparison.
 *
 * skia-rust: only the GM mode is ported (the Sample and Bench modes of Skia's viewer and bench
 * tool are not GM modes), so the animation members (`fAnimRadius`, `fAnimBlurRadius`, the
 * `TimeUtils::PingPong` constants, `onAnimate`) and `fRandom` are not ported.
 */
// Port of: gm/blurcircles2.cpp#L27-L142 (chrome/m156)
struct BlurCircles2Gm;

impl BlurCircles2Gm {
    fn new() -> Self {
        BlurCircles2Gm
    }
}

impl GM for BlurCircles2Gm {
    fn name(&self) -> String {
        "blurcircles2".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(730, 1350)
    }

    #[allow(clippy::cast_precision_loss, clippy::items_after_statements)] // int -> float arithmetic and constants next to their C++ use
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn on_draw(&mut self, canvas: &Canvas) {
        let almost_circle_maker = |radius: scalar| -> Path {
            PathBuilder::new()
                .add_arc(
                    Rect::from_xywh(-radius, -radius, 2.0 * radius, 2.0 * radius),
                    0.0,
                    355.0,
                )
                .set_is_volatile(true)
                .close()
                .detach()
        };

        let blur_maker = |radius: scalar| -> Option<MaskFilter> {
            MaskFilter::blur(
                BlurStyle::Normal,
                BlurMask::convert_radius_to_sigma(radius),
                None,
            )
        };

        let mut paint = Paint::default();
        paint.set_color(Color::BLACK);

        canvas.save();
        const PAD: scalar = 5.0;
        const RADIUS_STEPS: i32 = 5;
        const BLUR_RADIUS_STEPS: i32 = 5;
        canvas.translate((
            PAD + MIN_RADIUS + MAX_BLUR_RADIUS,
            PAD + MIN_RADIUS + MAX_BLUR_RADIUS,
        ));
        const DELTA_RADIUS: scalar = (MAX_RADIUS - MIN_RADIUS) / RADIUS_STEPS as scalar;
        const DELTA_BLUR_RADIUS: scalar =
            (MAX_BLUR_RADIUS - MIN_BLUR_RADIUS) / BLUR_RADIUS_STEPS as scalar;
        let mut line_width: scalar = 0.0;
        for r in 0..RADIUS_STEPS - 1 {
            let radius = r as scalar * DELTA_RADIUS + MIN_RADIUS;
            line_width += 2.0 * (radius + MAX_BLUR_RADIUS) + PAD;
        }
        for br in 0..BLUR_RADIUS_STEPS {
            let blur_radius = br as scalar * DELTA_BLUR_RADIUS + MIN_BLUR_RADIUS;
            let max_row_r = blur_radius + MAX_RADIUS;
            paint.set_mask_filter(blur_maker(blur_radius));
            canvas.save();
            for r in 0..RADIUS_STEPS {
                let radius = r as scalar * DELTA_RADIUS + MIN_RADIUS;
                let almost_circle = almost_circle_maker(radius);
                canvas.save();
                canvas.draw_circle((0.0, 0.0), radius, &paint);
                canvas.translate((0.0, 2.0 * max_row_r + PAD));
                canvas.draw_path(&almost_circle, &paint);
                canvas.restore();
                let max_col_r = radius + MAX_BLUR_RADIUS;
                canvas.translate((max_col_r * 2.0 + PAD, 0.0));
            }
            canvas.restore();
            let mut black_paint = Paint::default();
            black_paint.set_color(Color::BLACK);
            let line_y = 3.0 * max_row_r + 1.5 * PAD;
            if br != BLUR_RADIUS_STEPS - 1 {
                canvas.draw_line((0.0, line_y), (line_width, line_y), &black_paint);
            }
            canvas.translate((0.0, max_row_r * 4.0 + 2.0 * PAD));
        }
        canvas.restore();
    }
}

// Port of: gm/blurcircles2.cpp#L144 (chrome/m156)
crate::def_gm!(BlurCircles2GM_ = "BlurCircles2GM()", BlurCircles2Gm::new());
