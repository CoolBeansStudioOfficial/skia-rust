// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bitmapfilters.cpp (chrome/m156)
//
// Not ported: `FilterGM` (draws its labels with `drawString`; text is not ported).

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: gm/bitmapfilters.cpp#L118-L158 (chrome/m156)
struct TestExtractAlphaGm {
    bitmap: Bitmap,
    alpha: Bitmap,
}

impl GM for TestExtractAlphaGm {
    // Port of: gm/bitmapfilters.cpp#L119-L134 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        // Make a bitmap with per-pixels alpha (stroked circle)
        self.bitmap.alloc_n32_pixels((100, 100), None);
        {
            let canvas = CoreCanvas::from_bitmap(&mut self.bitmap, None).expect("a canvas");
            canvas.clear(Color::new(0));

            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_color(Color::BLUE);
            paint.set_style(Style::Stroke);
            paint.set_stroke_width(20.0);

            canvas.draw_circle((50.0, 50.0), 39.0, &paint);
        }

        let _ = self.bitmap.extract_alpha(&mut self.alpha, None);
    }

    fn name(&self) -> String {
        "extractalpha".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(540, 330)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(Color::RED);

        let sampling = SamplingOptions::from(FilterMode::Linear);

        // should stay blue (ignore paint's color)
        canvas.draw_image_with_sampling_options(
            self.bitmap.as_image().expect("an image"),
            (10.0, 10.0),
            sampling,
            Some(&paint),
        );
        // should draw red
        canvas.draw_image_with_sampling_options(
            self.alpha.as_image().expect("an image"),
            (120.0, 10.0),
            sampling,
            Some(&paint),
        );
    }
}

// Port of: gm/bitmapfilters.cpp#L160 (chrome/m156)
crate::def_gm!(
    TestExtractAlphaGM,
    TestExtractAlphaGm {
        bitmap: Bitmap::new(),
        alpha: Bitmap::new(),
    }
);
