// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/tiledscaledbitmap.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::raster_canvas::RasterCanvas;

// This GM reproduces skbug.com/40034014, in which a tiled bitmap shader was failing to draw
// correctly when fractional image scaling was ignored by the high quality bitmap scaler.

// Port of: gm/tiledscaledbitmap.cpp#L25-L69 (chrome/m156)
struct TiledScaledBitmapGm {
    bitmap: Bitmap,
}

impl TiledScaledBitmapGm {
    fn new() -> TiledScaledBitmapGm {
        TiledScaledBitmapGm {
            bitmap: Bitmap::new(),
        }
    }

    // Port of: gm/tiledscaledbitmap.cpp#L38-L47 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar, width/2.f
    fn make_bm(width: i32, height: i32) -> Bitmap {
        let mut bm = Bitmap::new();
        bm.alloc_n32_pixels((width, height), None);
        bm.erase_color(Color::TRANSPARENT);
        {
            let canvas = CoreCanvas::from_bitmap(&mut bm, None).expect("a canvas on the bitmap");
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            canvas.draw_circle(
                (width as f32 / 2.0, height as f32 / 2.0),
                width as f32 / 4.0,
                &paint,
            );
        }
        bm
    }
}

impl GM for TiledScaledBitmapGm {
    fn name(&self) -> String {
        "tiledscaledbitmap".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1016, 616)
    }

    fn on_once_before_draw(&mut self) {
        self.bitmap = Self::make_bm(360, 288);
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();

        paint.set_anti_alias(true);

        let mut mat = Matrix::new_identity();
        mat.set_scale((121.0 / 360.0, 93.0 / 288.0), None);
        mat.post_translate((-72.0, -72.0));

        paint.set_shader(self.bitmap.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            SamplingOptions::from(CubicResampler::mitchell()),
            &mat,
        ));
        canvas.draw_rect(Rect::new(8.0, 8.0, 1008.0, 608.0), &paint);
    }
}

// Port of: gm/tiledscaledbitmap.cpp#L73 (chrome/m156)
crate::def_gm!(TiledScaledBitmapGM, TiledScaledBitmapGm::new());
