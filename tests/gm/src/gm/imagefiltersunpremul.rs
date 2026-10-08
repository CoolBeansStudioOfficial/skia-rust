// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagefiltersunpremul.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::sampling_options::{CubicResampler, SamplingOptions};
use skia_rust_core::size::ISize;
use skia_rust_effects::image_filters::image_sampled;

// Port of: gm/imagefiltersunpremul.cpp#L14-L25 (chrome/m156)
crate::def_simple_gm_bg!(imagefiltersunpremul, canvas, 64, 64, Color::BLACK, {
    // Draw an kUnpremul_SkAlphaType image using SkImageFilters::Image() and
    // verify alpha channel was blended correctly.
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(
        &ImageInfo::new(
            ISize::new(64, 64),
            ColorType::RGBA8888,
            AlphaType::Unpremul,
            None,
        ),
        None,
    );
    bitmap.erase_color(Color::from_argb(50, 255, 0, 0));
    let mut paint = Paint::default();
    paint.set_image_filter(image_sampled(
        images::raster_from_bitmap(&bitmap),
        SamplingOptions::from(CubicResampler::mitchell()),
    ));
    canvas.draw_paint(&paint);
});
