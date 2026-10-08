// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/blurredclippedcircle.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color_filters;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

const WIDTH: i32 = 1164;
const HEIGHT: i32 = 802;

// This GM reproduces the precision artifacts seen in crbug.com/560651.
// It draws a largish blurred circle with its center clipped out.
// Port of: gm/blurredclippedcircle.cpp#L22-L88 (chrome/m156)
struct BlurredClippedCircleGm;

impl GM for BlurredClippedCircleGm {
    fn name(&self) -> String {
        "blurredclippedcircle".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFCC_CCCC)
    }

    #[allow(clippy::cast_precision_loss, clippy::items_after_statements)] // SkIntToScalar-like conversions; constants next to their C++ use
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut white_paint = Paint::default();
        white_paint.set_color(Color::WHITE);
        white_paint.set_blend_mode(BlendMode::Src);
        white_paint.set_anti_alias(true);

        // This scale exercises precision limits in the circle blur effect (crbug.com/560651)
        const SCALE: f32 = 2.0;
        canvas.scale((SCALE, SCALE));

        canvas.save();
        let clip_rect1 = Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32);
        canvas.clip_rect(clip_rect1, None, None);

        canvas.save();

        canvas.clip_rect(clip_rect1, None, None);
        canvas.draw_rect(clip_rect1, &white_paint);

        canvas.save();

        let clip_rect2 = Rect::new(8.0, 8.0, 288.0, 288.0);
        let clip_rrect = RRect::new_oval(clip_rect2);
        canvas.clip_rrect(clip_rrect, ClipOp::Difference, true);

        let r = Rect::new(4.0, 4.0, 292.0, 292.0);
        let rr = RRect::new_oval(r);

        let mut paint = Paint::default();

        paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 1.366_025, None));
        paint.set_color_filter(color_filters::blend_color(Color::RED, BlendMode::SrcIn));
        paint.set_anti_alias(true);

        canvas.draw_rrect(rr, &paint);

        canvas.restore();
        canvas.restore();
        canvas.restore();
    }
}

// Port of: gm/blurredclippedcircle.cpp#L90 (chrome/m156)
crate::def_gm!(
    BlurredClippedCircleGM_ = "BlurredClippedCircleGM",
    BlurredClippedCircleGm
);
