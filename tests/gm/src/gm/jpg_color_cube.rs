// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/jpg_color_cube.cpp (chrome/m156)

use skia_rust_codec::encode::jpeg_encoder::{self, Options};
use skia_rust_codec::images::deferred_from_encoded_data;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_priv::pack_argb32;
use skia_rust_core::image::Image;

use crate::prelude::*;

/// Port of `class ColorCubeGM` (jpg_color_cube.cpp#L17-L69): a 512x512 colour cube, JPEG-encoded
/// once and drawn from the decoded image.
struct ColorCubeGm {
    image: Option<Image>,
}

impl GM for ColorCubeGm {
    fn name(&self) -> String {
        "jpg-color-cube".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(512, 512)
    }

    fn on_once_before_draw(&mut self) {
        let mut bmp = Bitmap::new();
        bmp.alloc_n32_pixels((512, 512), true);
        let row_bytes = bmp.row_bytes();
        {
            let Some(mut pm) = bmp.peek_pixels_mut() else {
                return;
            };
            let Some(pixels) = pm.writable_addr() else {
                return;
            };
            let mut bx = 0usize;
            let mut by = 0usize;
            for b in 0u32..64 {
                for r in 0u32..64 {
                    for g in 0u32..64 {
                        // SkPackARGB32(255, pin(r * 4), pin(g * 4), pin(b * 4)), in N32 order.
                        let pixel =
                            pack_argb32(255, (r * 4).min(255), (g * 4).min(255), (b * 4).min(255));
                        let x = bx + r as usize;
                        let y = by + g as usize;
                        let at = y * row_bytes + x * 4;
                        pixels[at..at + 4].copy_from_slice(&pixel.to_ne_bytes());
                    }
                }
                bx += 64;
                if bx >= 512 {
                    bx = 0;
                    by += 64;
                }
            }
        }
        if let Some(pixmap) = bmp.peek_pixels()
            && let Some(data) = jpeg_encoder::encode_pixmap(&pixmap, &Options::default())
        {
            self.image = deferred_from_encoded_data(Some(data), None);
        }
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        if let Some(image) = &self.image {
            canvas.draw_image(image, (0.0, 0.0), None);
        }
    }
}

// Port of: gm/jpg_color_cube.cpp#L70 (chrome/m156)
crate::def_gm!(ColorCubeGM, ColorCubeGm { image: None });
