// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/overdrawcolorfilter.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_effects::overdraw_color_filter;

// Port of: gm/overdrawcolorfilter.cpp#L15-L54 (chrome/m156)
struct OverdrawColorFilterGm;

impl GM for OverdrawColorFilterGm {
    fn name(&self) -> String {
        "overdrawcolorfilter".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(200, 400)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        const COLORS: [Color; overdraw_color_filter::NUM_COLORS] = [
            Color::new(0x80FF_0000),
            Color::new(0x8000_FF00),
            Color::new(0x8000_00FF),
            Color::new(0x80FF_FF00),
            Color::new(0x8000_FFFF),
            Color::new(0x80FF_00FF),
        ];

        let mut paint = Paint::default();
        paint.set_color_filter(overdraw_color_filter::make_with_sk_colors(&COLORS));

        let sampling = SamplingOptions::default();
        let info = ImageInfo::new_a8((100, 100));
        let mut bitmap = Bitmap::new();
        bitmap.alloc_pixels_info(&info, None);
        let mut draw = |alpha: u8, x: f32, y: f32| {
            bitmap.erase_argb(alpha, 0, 0, 0);
            if let Some(image) = bitmap.as_image() {
                canvas.draw_image_with_sampling_options(&image, (x, y), sampling, Some(&paint));
            }
        };
        draw(0, 0.0, 0.0);
        draw(1, 0.0, 100.0);
        draw(2, 0.0, 200.0);
        draw(3, 0.0, 300.0);
        draw(4, 100.0, 0.0);
        draw(5, 100.0, 100.0);
        draw(6, 100.0, 200.0);
    }
}

crate::def_gm!(OverdrawColorFilter, OverdrawColorFilterGm);
