// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/badpaint.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;

// Port of: gm/badpaint.cpp#L22-L65 (chrome/m156)
struct BadPaintGm {
    paints: Vec<Paint>,
}

impl GM for BadPaintGm {
    fn name(&self) -> String {
        "badpaint".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(100, 100)
    }

    fn on_once_before_draw(&mut self) {
        let empty_bmp = Bitmap::new();

        let mut blue_bmp = Bitmap::new();
        blue_bmp.alloc_n32_pixels((10, 10), None);
        blue_bmp.erase_color(Color::BLUE);

        let mut bad_matrix = Matrix::new_identity();
        bad_matrix.set_all(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);

        // Empty bitmap.
        let mut paint = Paint::default();
        paint.set_color(Color::GREEN);
        paint.set_shader(empty_bmp.to_shader(None, SamplingOptions::default(), None));
        self.paints.push(paint);

        // Non-invertible local matrix.
        let mut paint = Paint::default();
        paint.set_color(Color::GREEN);
        paint.set_shader(blue_bmp.to_shader(None, SamplingOptions::default(), &bad_matrix));
        self.paints.push(paint);
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let rect = Rect::from_xywh(10.0, 10.0, 80.0, 80.0);
        for paint in &self.paints {
            canvas.draw_rect(rect, paint);
        }
    }
}

// Port of: gm/badpaint.cpp#L69 (chrome/m156)
crate::def_gm!(BadPaintGM, BadPaintGm { paints: Vec::new() });
