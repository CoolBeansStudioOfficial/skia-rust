// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/colorfilteralpha8.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;

// Port of: gm/colorfilteralpha8.cpp#L14-L35 (chrome/m156), ColorFilterAlpha8
struct ColorFilterAlpha8Gm;

impl GM for ColorFilterAlpha8Gm {
    fn name(&self) -> String {
        "colorfilteralpha8".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(400, 400)
    }

    // Port of: gm/colorfilteralpha8.cpp#L19-L34 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.clear(Color::RED);
        let mut bitmap = Bitmap::new();
        let info = ImageInfo::new_a8((200, 200));
        bitmap.alloc_pixels_info(&info, None);
        bitmap.erase_color(Color::from(0x88FF_FFFF));

        let mut paint = Paint::default();
        #[rustfmt::skip]
        let opaque_gray_matrix = ColorMatrix::new(
            0.0, 0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 1.0,
        );
        paint.set_color_filter(color_filters::matrix(&opaque_gray_matrix, Clamp::Yes));
        if let Some(image) = bitmap.as_image() {
            canvas.draw_image(&image, (100.0, 100.0), Some(&paint));
        }
    }
}

// Port of: gm/colorfilteralpha8.cpp#L37 (chrome/m156)
crate::def_gm!(ColorFilterAlpha8, ColorFilterAlpha8Gm);
