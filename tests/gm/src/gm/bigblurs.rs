// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bigblurs.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{int_to_scalar, scalar};

const CLOSE_UP_SIZE: i32 = 64;
const WIDTH: i32 = 5 * CLOSE_UP_SIZE;
const HEIGHT: i32 = 2 * (BlurStyle::LAST_ENUM as i32 + 1) * CLOSE_UP_SIZE;

// This GM exercises the blurred rect nine-patching special cases when the
// blurred rect is very large and/or very far from the origin.
// It creates a large blurred rect/rectori then renders the 4 corners and the
// middle.
// Port of: gm/bigblurs.cpp#L26-L116 (chrome/m156)
struct BigBlursGm;

impl GM for BigBlursGm {
    fn name(&self) -> String {
        "bigblurs".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFDD_DDDD)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        const BIG: i32 = 65536;
        let sigma: scalar = BlurMask::convert_radius_to_sigma(int_to_scalar(4));

        let big_rect = Rect::from_iwh(BIG, BIG);
        let mut inset_rect = big_rect;
        inset_rect.inset((20.0, 20.0));

        let rectori = PathBuilder::new()
            .add_rect(big_rect, None, None)
            .add_rect(inset_rect, PathDirection::CCW, None)
            .detach();

        // The blur extends 3*kSigma out from the big rect.
        // Offset the close-up windows so we get the entire blur
        let left_top_pad: scalar = 3.0 * sigma; // use on left & up of big rect
        let right_bot_pad: scalar = int_to_scalar(CLOSE_UP_SIZE) - 3.0 * sigma; // use on right and bot sides

        let big = int_to_scalar(BIG);
        let close_up = int_to_scalar(CLOSE_UP_SIZE);
        // UL hand corners of the rendered closeups
        let origins = [
            Point::new(-left_top_pad, -left_top_pad),             // UL
            Point::new(big - right_bot_pad, -left_top_pad),       // UR
            Point::new(big - right_bot_pad, big - right_bot_pad), // LR
            Point::new(-left_top_pad, big - right_bot_pad),       // LL
            // center: kBig/2-kCloseUpSize/2 is integer arithmetic
            Point::new(
                int_to_scalar(BIG / 2 - CLOSE_UP_SIZE / 2),
                int_to_scalar(BIG / 2 - CLOSE_UP_SIZE / 2),
            ),
        ];
        let _ = close_up;

        let mut outline_paint = Paint::default();
        outline_paint.set_color(Color::RED);
        outline_paint.set_style(Style::Stroke);

        let mut blur_paint = Paint::default();
        blur_paint.set_anti_alias(true);
        blur_paint.set_color(Color::BLACK);

        let mut desired_x: i32 = 0;
        let mut desired_y: i32 = 0;

        for i in 0..2 {
            for j in 0..=(BlurStyle::LAST_ENUM as i32) {
                blur_paint.set_mask_filter(MaskFilter::blur(
                    BlurStyle::from_i32(j).unwrap(),
                    sigma,
                    None,
                ));

                for origin in &origins {
                    canvas.save();

                    let clip_rect = Rect::from_xywh(
                        int_to_scalar(desired_x),
                        int_to_scalar(desired_y),
                        int_to_scalar(CLOSE_UP_SIZE),
                        int_to_scalar(CLOSE_UP_SIZE),
                    );

                    canvas.clip_rect(clip_rect, None, None);

                    canvas.translate((
                        int_to_scalar(desired_x) - origin.x,
                        int_to_scalar(desired_y) - origin.y,
                    ));

                    if 0 == i {
                        canvas.draw_rect(big_rect, &blur_paint);
                    } else {
                        canvas.draw_path(&rectori, &blur_paint);
                    }
                    canvas.restore();
                    canvas.draw_rect(clip_rect, &outline_paint);

                    desired_x += CLOSE_UP_SIZE;
                }

                desired_x = 0;
                desired_y += CLOSE_UP_SIZE;
            }
        }
    }
}

// Port of: gm/bigblurs.cpp#L118 (chrome/m156)
crate::def_gm!(BigBlursGM_ = "BigBlursGM", BigBlursGm);
