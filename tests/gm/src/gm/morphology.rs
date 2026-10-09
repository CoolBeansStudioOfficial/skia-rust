// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/morphology.cpp (chrome/m156)

// Coordinates and radii here are small integers: their f32 conversions are exact.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::font::Font;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_effects::image_filters;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/morphology.cpp#L24-L25 (chrome/m156)
const WIDTH: i32 = 700;
const HEIGHT: i32 = 560;

// Port of: gm/morphology.cpp#L37-L99 (chrome/m156), MorphologyGM
struct MorphologyGm {
    image: Option<Image>,
}

impl MorphologyGm {
    // Port of: gm/morphology.cpp#L40-L41 (chrome/m156), the constructor's background color
    fn new() -> Self {
        Self { image: None }
    }

    // Port of: gm/morphology.cpp#L62-L69 (chrome/m156), drawClippedBitmap
    fn draw_clipped_bitmap(&self, canvas: &Canvas, paint: &Paint, x: i32, y: i32) {
        let image = self
            .image
            .as_ref()
            .expect("onOnceBeforeDraw made the image");
        canvas.save();
        canvas.translate((x as f32, y as f32));
        canvas.clip_irect(image.bounds(), None);
        canvas.draw_image_with_sampling_options(
            image,
            (0.0, 0.0),
            SamplingOptions::default(),
            Some(paint),
        );
        canvas.restore();
    }
}

impl GM for MorphologyGm {
    // Port of: gm/morphology.cpp#L40-L41 (chrome/m156), setBGColor(0xFF000000)
    fn bg_color(&self) -> Color {
        Color::BLACK
    }

    // Port of: gm/morphology.cpp#L43 (chrome/m156), getName
    fn name(&self) -> String {
        "morphology".to_owned()
    }

    // Port of: gm/morphology.cpp#L71 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    // Port of: gm/morphology.cpp#L45-L55 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let mut surf = surfaces::raster(&ImageInfo::new_n32_premul((135, 135), None), None, None)
            .expect("a raster surface");
        let font = Font::from_size(default_portable_typeface(), 64.0);
        let mut paint = Paint::default();
        paint.set_color(Color::WHITE);
        surf.canvas().draw_str("ABC", (10.0, 55.0), &font, &paint);
        surf.canvas().draw_str("XYZ", (10.0, 110.0), &font, &paint);
        self.image = surf.image_snapshot();
    }

    // Port of: gm/morphology.cpp#L73-L99 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        // (fWidth, fHeight, fRadiusX, fRadiusY)
        const SAMPLES: [(i32, i32, i32, i32); 5] = [
            (140, 140, 0, 0),
            (140, 140, 0, 2),
            (140, 140, 2, 0),
            (140, 140, 2, 2),
            (24, 24, 25, 25),
        ];
        let mut paint = Paint::default();
        let crop_rect = IRect::from_xywh(25, 20, 100, 80);
        for j in 0..4_i32 {
            for (i, sample) in (0_i32..).zip(SAMPLES.iter()) {
                let (_, _, radius_x, radius_y) = *sample;
                let cr = if j & 0x02 != 0 {
                    Some(Rect::from_irect(crop_rect))
                } else {
                    None
                };
                if j & 0x01 != 0 {
                    paint.set_image_filter(image_filters::erode(
                        (radius_x as f32, radius_y as f32),
                        None,
                        cr,
                    ));
                } else {
                    paint.set_image_filter(image_filters::dilate(
                        (radius_x as f32, radius_y as f32),
                        None,
                        cr,
                    ));
                }
                self.draw_clipped_bitmap(canvas, &paint, i * 140, j * 140);
            }
        }
    }
}

// Port of: gm/morphology.cpp#L98 (chrome/m156), DEF_GM
crate::def_gm!(MorphologyGM_ = "MorphologyGM", MorphologyGm::new());
