// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/unpremul.cpp (chrome/m156)

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;

use crate::{mark_gm_bad, mark_gm_good};

// Port of: gm/unpremul.cpp#L20-L77 (chrome/m156)
crate::def_simple_gm!(unpremul, canvas, 200, 200, {
    let color = Color::new(0xbf40_0000);

    let grade = |x: i32, y: i32| {
        let mut bm = Bitmap::new();
        bm.alloc_pixels_info(
            &ImageInfo::new(
                (1, 1),
                ColorType::BGRA8888,
                AlphaType::Unpremul,
                Some(ColorSpace::new_srgb()),
            ),
            None,
        );
        if !canvas.read_pixels_to_bitmap(&mut bm, (x, y)) {
            // Picture-backed canvases, that sort of thing.  Just assume they're good.
            mark_gm_good(canvas, 140.0, 40.0);
            return;
        }

        // `memcpy(&pixel, bm.getAddr(0,0), sizeof(pixel))`: BGRA bytes are an SkColor.
        let pixel = Color::new(bm.get_addr32(0, 0));

        let close = |x: i32, y: i32| x - y < 2 && y - x < 2;

        if close(i32::from(pixel.r()), i32::from(color.r()))
            && close(i32::from(pixel.g()), i32::from(color.g()))
            && close(i32::from(pixel.b()), i32::from(color.b()))
            && close(i32::from(pixel.a()), i32::from(color.a()))
        {
            mark_gm_good(canvas, 140.0, 40.0);
        } else {
            mark_gm_bad(canvas, 140.0, 40.0);
        }
    };

    {
        let mut paint = Paint::default();
        paint.set_blend_mode(BlendMode::Src);
        paint.set_color(color);

        canvas.draw_rect(Rect::new(0.0, 0.0, 100.0, 100.0), &paint);
        grade(50, 50);
    }

    canvas.translate((0.0, 100.0));

    {
        let mut paint = Paint::default();
        paint.set_blend_mode(BlendMode::Src);

        let mut bm = Bitmap::new();
        bm.alloc_pixels_info(
            &ImageInfo::new((100, 100), ColorType::RGBA8888, AlphaType::Unpremul, None),
            None,
        );
        bm.erase_color(color);

        canvas.draw_image_with_sampling_options(
            bm.as_image().expect("an image"),
            (0.0, 0.0),
            SamplingOptions::default(),
            Some(&paint),
        );
        grade(50, 150);
    }
});
