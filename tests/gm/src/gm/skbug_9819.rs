// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/skbug_9819.cpp (chrome/m156)

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;

use crate::{mark_gm_bad, mark_gm_good};

// This GM should draw two yellow boxes; the bug drew one in cyan.
// Port of: gm/skbug_9819.cpp#L16-L55 (chrome/m156)
crate::def_simple_gm!(skbug_9819, c, 256, 256, {
    let info = ImageInfo::new((1, 1), ColorType::Unknown, AlphaType::Premul, None);
    let mut rgba = Bitmap::new();
    let mut bgra = Bitmap::new();
    rgba.alloc_pixels_info(&info.with_color_type(ColorType::RGBA8888), None);
    bgra.alloc_pixels_info(&info.with_color_type(ColorType::BGRA8888), None);

    let yellow = Color::new(0xffff_ff00);
    rgba.erase_color(yellow);
    bgra.erase_color(yellow);

    c.save();
    c.scale((128.0, 128.0));
    c.draw_image(rgba.as_image().expect("an image"), (0.0, 0.0), None);
    c.draw_image(bgra.as_image().expect("an image"), (0.0, 1.0), None);
    c.restore();

    #[allow(clippy::cast_precision_loss)] // x+128, y as SkScalar
    let grade = |x: i32, y: i32| {
        let mut bm = Bitmap::new();
        bm.alloc_pixels_info(
            &ImageInfo::new(
                (1, 1),
                ColorType::Gray8,
                AlphaType::Unpremul,
                Some(ColorSpace::new_srgb()),
            ),
            None,
        );
        if !c.read_pixels_to_bitmap(&mut bm, (x, y)) {
            // Picture-backed canvases, that sort of thing.  Just assume they're good.
            mark_gm_good(c, (x + 128) as f32, y as f32);
            return;
        }

        // We test only luma so that grayscale destinations are also correctly graded:
        //    - yellow (good) is around 237
        //    - cyan   (bad)  is around 202
        let gray = i32::from(bm.get_addr8(0, 0));
        if (gray - 237).abs() > 2 {
            mark_gm_bad(c, (x + 128) as f32, y as f32);
        } else {
            mark_gm_good(c, (x + 128) as f32, y as f32);
        }
    };

    grade(64, 64);
    grade(64, 192);
});
