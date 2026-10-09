// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagefiltersclipped.cpp (chrome/m156)
//
// Only `imagefilter_convolve_subset` is ported here. `ImageFiltersClippedGM` needs the point-lit
// diffuse filter (lighting), which is not ported yet, and has its own manifest entry.

// The image height is a small integer: its f32 conversion is exact.
#![allow(clippy::cast_precision_loss)]

use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters;

use crate::tool_utils::get_resource_as_image;

// Port of: gm/imagefiltersclipped.cpp#L167-L186 (chrome/m156), DEF_SIMPLE_GM(imagefilter_convolve_subset)
crate::def_simple_gm!(
    #[ignore = "see notes/gm_imagefiltersclipped_cpp_imagefilter_convolve_subset.md"]
    imagefilter_convolve_subset,
    canvas,
    160,
    180,
    {
        let Some(reference) = get_resource_as_image("images/filter_reference.png") else {
            return;
        };
        let mut crop = Rect::from_irect(IRect::from_size(reference.dimensions()));
        crop.inset((10.0, 10.0));
        let draw_filtered_image = |filter: Option<skia_rust_core::image_filter::ImageFilter>| {
            let mut paint = Paint::default();
            paint.set_image_filter(filter);
            canvas.draw_image(&reference, (0.0, 0.0), Some(&paint));
            canvas.translate((0.0, reference.height() as f32));
        };

        {
            let kernel = [1.0_f32, 1.0, 1.0, 1.0, -7.0, 1.0, 1.0, 1.0, 1.0];
            draw_filtered_image(image_filters::matrix_convolution(
                (3, 3),
                &kernel,
                1.0,
                0.3,
                (1, 1),
                TileMode::Clamp,
                true,
                None,
                Some(crop),
            ));
        }

        draw_filtered_image(image_filters::blur(
            10.0,
            10.0,
            TileMode::Mirror,
            None,
            Some(crop),
        ));
    }
);
