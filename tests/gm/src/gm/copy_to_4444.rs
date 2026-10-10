// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/copy_to_4444.cpp (chrome/m156)
#![allow(clippy::cast_precision_loss)] // mirrors the C++ int-to-scalar conversions of small sizes (exact in f32)

use crate::prelude::*;
use crate::tool_utils::{copy_to, get_resource_as_bitmap};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::size::ISize;
use skia_rust_raster::raster_canvas::RasterCanvas;

/// Test copying an image from 8888 to 4444.
// Port of: gm/copy_to_4444.cpp#L17-L40 (chrome/m156)
struct CopyTo4444Gm;

impl GM for CopyTo4444Gm {
    fn name(&self) -> String {
        "copyTo4444".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(360, 180)
    }

    // Port of: gm/copy_to_4444.cpp#L24-L38 (chrome/m156)
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let Some(bm) = get_resource_as_bitmap("images/dog.jpg") else {
            *error_msg =
                "Could not decode the file. Did you forget to set the resourcePath?".to_string();
            return DrawResult::Fail;
        };
        canvas.draw_image(bm.as_image().expect("an image"), (0.0, 0.0), None);

        // This should dither or we will see artifacts in the background of the image.
        let mut bm4444 = Bitmap::new();
        assert!(copy_to(&mut bm4444, ColorType::ARGB4444, &bm));
        canvas.draw_image(
            bm4444.as_image().expect("an image"),
            (bm.width() as f32, 0.0),
            None,
        );
        DrawResult::Ok
    }
}

crate::def_gm!(CopyTo4444GM = "CopyTo4444GM", CopyTo4444Gm);

/// `pack4444(a, r, g, b)` from `format4444`.
// Port of: gm/copy_to_4444.cpp#L44-L46 (chrome/m156)
fn pack4444(a: u16, r: u16, g: u16, b: u16) -> u16 {
    a | (b << 4) | (g << 8) | (r << 12)
}

// Port of: gm/copy_to_4444.cpp#L42-L66 (chrome/m156)
crate::def_simple_gm!(format4444, canvas, 64, 64, {
    canvas.scale((16.0, 16.0));
    let image_info = ImageInfo::new(
        (1, 1),
        ColorType::ARGB4444,
        AlphaType::Premul,
        None::<skia_rust_core::color_space::ColorSpace>,
    );
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&image_info, None);
    {
        let offscreen = CoreCanvas::from_bitmap(&mut bitmap, None).expect("a canvas");
        offscreen.clear(Color::RED);
    }
    canvas.draw_image(bitmap.as_image().expect("an image"), (0.0, 0.0), None);
    {
        let offscreen = CoreCanvas::from_bitmap(&mut bitmap, None).expect("a canvas");
        offscreen.clear(Color::BLUE);
    }
    canvas.draw_image(bitmap.as_image().expect("an image"), (1.0, 1.0), None);

    let red4444 = pack4444(0xF, 0xF, 0x0, 0x0);
    let blue4444 = pack4444(0xF, 0x0, 0x0, 0x0F);
    let red_bytes = red4444.to_ne_bytes();
    let red_pixmap = Pixmap::new_readonly(&image_info, &red_bytes, 2).expect("a pixmap");
    if bitmap.write_pixels(&red_pixmap, 0, 0) {
        canvas.draw_image(bitmap.as_image().expect("an image"), (2.0, 2.0), None);
    }
    let blue_bytes = blue4444.to_ne_bytes();
    let blue_pixmap = Pixmap::new_readonly(&image_info, &blue_bytes, 2).expect("a pixmap");
    if bitmap.write_pixels(&blue_pixmap, 0, 0) {
        canvas.draw_image(bitmap.as_image().expect("an image"), (3.0, 3.0), None);
    }
});
