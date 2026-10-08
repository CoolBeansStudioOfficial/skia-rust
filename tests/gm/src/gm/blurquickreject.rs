// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/blurquickreject.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;

// This GM tests out the quick reject bounds of the blur mask filter. It draws
// four blurred rects around a central clip. The blurred rect geometry outset
// by the blur radius does not overlap the clip rect so, if the blur clipping
// just uses the radius, they will be clipped out (and the result will differ
// from the result if quick reject were disabled. If the blur clipping uses
// the correct 3 sigma bound then the images with and without quick rejecting
// will be the same.
// Port of: gm/blurquickreject.cpp#L24-L82 (chrome/m156)
struct BlurQuickRejectGm;

const WIDTH: i32 = 300;
const HEIGHT: i32 = 300;

impl GM for BlurQuickRejectGm {
    fn name(&self) -> String {
        "blurquickreject".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        const BLUR_RADIUS: scalar = 20.0;
        const BOX_SIZE: scalar = 100.0;

        let clip_rect = Rect::from_xywh(0.0, 0.0, BOX_SIZE, BOX_SIZE);
        let blur_rects = [
            Rect::new(
                -BOX_SIZE - (BLUR_RADIUS + 1.0),
                0.0,
                -(BLUR_RADIUS + 1.0),
                BOX_SIZE,
            ),
            Rect::new(
                0.0,
                -BOX_SIZE - (BLUR_RADIUS + 1.0),
                BOX_SIZE,
                -(BLUR_RADIUS + 1.0),
            ),
            Rect::new(
                BOX_SIZE + BLUR_RADIUS + 1.0,
                0.0,
                2.0 * BOX_SIZE + BLUR_RADIUS + 1.0,
                BOX_SIZE,
            ),
            Rect::new(
                0.0,
                BOX_SIZE + BLUR_RADIUS + 1.0,
                BOX_SIZE,
                2.0 * BOX_SIZE + BLUR_RADIUS + 1.0,
            ),
        ];
        let colors = [Color::RED, Color::GREEN, Color::BLUE, Color::YELLOW];
        assert_eq!(colors.len(), blur_rects.len());

        let mut hairline_paint = Paint::default();
        hairline_paint.set_style(Style::Stroke);
        hairline_paint.set_color(Color::WHITE);
        hairline_paint.set_stroke_width(0.0);

        let mut blur_paint = Paint::default();
        blur_paint.set_mask_filter(MaskFilter::blur(
            BlurStyle::Normal,
            BlurMask::convert_radius_to_sigma(BLUR_RADIUS),
            None,
        ));

        canvas.clear(Color::BLACK);
        canvas.save();
        canvas.translate((BOX_SIZE, BOX_SIZE));
        canvas.draw_rect(clip_rect, &hairline_paint);
        canvas.clip_rect(clip_rect, None, None);
        for i in 0..blur_rects.len() {
            blur_paint.set_color(colors[i]);
            canvas.draw_rect(blur_rects[i], &blur_paint);
            canvas.draw_rect(blur_rects[i], &hairline_paint);
        }
        canvas.restore();
    }
}

// Port of: gm/blurquickreject.cpp#L87 (chrome/m156)
crate::def_gm!(
    BlurQuickRejectGM_ = "BlurQuickRejectGM()",
    BlurQuickRejectGm
);
