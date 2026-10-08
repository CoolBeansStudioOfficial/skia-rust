// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/tallstretchedbitmaps.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{Canvas as CoreCanvas, SrcRectConstraint};
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::random::Random;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tiled_image_utils;
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: gm/tallstretchedbitmaps.cpp#L21-L59 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // kMargin + kRadius as SkScalar
fn make_bm(bm: &mut Bitmap, height: i32) -> i32 {
    const RADIUS: i32 = 22;
    const MARGIN: i32 = 8;
    const START_ANGLE: f32 = 0.0;
    const D_ANGLE: f32 = 25.0;
    const SWEEP: f32 = 320.0;
    const THICKNESS: f32 = 8.0;

    let count = height / (2 * RADIUS + MARGIN);
    let height = count * (2 * RADIUS + MARGIN);

    bm.alloc_n32_pixels((2 * (RADIUS + MARGIN), height), None);
    let mut random = Random::default();

    {
        let whole_canvas = CoreCanvas::from_bitmap(bm, None).expect("a canvas");
        whole_canvas.clear(Color::new(0x0000_0000));
    }

    let mut angle = START_ANGLE;
    for i in 0..count {
        let mut paint = Paint::default();
        // The sw rasterizer disables AA for large canvii. So we make a small canvas for each draw.
        let sub_rect = IRect::from_xywh(
            0,
            i * (MARGIN + 2 * RADIUS),
            2 * RADIUS + MARGIN,
            2 * RADIUS + MARGIN,
        );
        // (`bm->extractSubset(&smallBM, subRect); SkCanvas canvas(smallBM);` draws into the
        // shared pixels; here the canvas draws into the subset of `bm`'s pixels directly.)
        let mut whole = bm.peek_pixels_mut().expect("pixels");
        let mut small = whole.extract_subset_mut(sub_rect).expect("a subset");
        let small_info = small.info().clone();
        let row_bytes = small.row_bytes();
        let canvas = CoreCanvas::from_raster_direct(
            &small_info,
            small.bytes_mut().expect("writable pixels"),
            row_bytes,
            None,
        )
        .expect("a canvas");
        canvas.translate(((MARGIN + RADIUS) as f32, (MARGIN + RADIUS) as f32));

        paint.set_anti_alias(true);
        paint.set_color(Color::new(random.next_u() | 0xFF00_0000));
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(THICKNESS);
        paint.set_stroke_cap(Cap::Round);
        let radius = RADIUS as f32 - THICKNESS / 2.0;
        let bounds = Rect::new(-radius, -radius, radius, radius);

        canvas.draw_arc(bounds, angle, SWEEP, false, &paint);
        angle += D_ANGLE;
    }
    bm.set_immutable();
    count
}

/// `FTallBmps`.
#[derive(Default)]
struct TallBmp {
    bmp: Bitmap,
    item_cnt: i32,
}

// Port of: gm/tallstretchedbitmaps.cpp#L61-L111 (chrome/m156)
struct TallStretchedBitmapsGm {
    tall_bmps: Vec<TallBmp>,
}

impl GM for TallStretchedBitmapsGm {
    fn name(&self) -> String {
        "tall_stretched_bitmaps".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(730, 690)
    }

    fn on_once_before_draw(&mut self) {
        self.tall_bmps = (0..8).map(|_| TallBmp::default()).collect();
        for (i, tall) in self.tall_bmps.iter_mut().enumerate() {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // SkToInt
            let h = ((4 + i) * 1024) as i32;

            tall.item_cnt = make_bm(&mut tall.bmp, h);
        }
    }

    #[allow(clippy::cast_precision_loss)] // SkIntToScalar, 10.f * itemHeight
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.scale((1.3, 1.3));
        for tall in &self.tall_bmps {
            debug_assert!(tall.item_cnt > 10);
            let bmp = &tall.bmp;
            // Draw the last 10 elements of the bitmap.
            let start_item = tall.item_cnt - 10;
            let item_height = bmp.height() / tall.item_cnt;
            let sub_rect = IRect::new(0, start_item * item_height, bmp.width(), bmp.height());
            let dst_rect = Rect::from_wh(bmp.width() as f32, 10.0 * item_height as f32);
            tiled_image_utils::draw_image_rect(
                canvas,
                &bmp.as_image().expect("an image"),
                &Rect::from_irect(sub_rect),
                &dst_rect,
                &SamplingOptions::from(FilterMode::Linear),
                None,
                SrcRectConstraint::Strict,
            );
            canvas.translate(((bmp.width() + 10) as f32, 0.0));
        }
    }
}

// Port of: gm/tallstretchedbitmaps.cpp#L114 (chrome/m156)
crate::def_gm!(
    TallStretchedBitmapsGM,
    TallStretchedBitmapsGm {
        tall_bmps: Vec::new()
    }
);
