// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/alpha_image.cpp (chrome/m156)
//
// `alpha_bitmap_is_coverage_ANDROID` is compiled out unless SK_SUPPORT_LEGACY_ALPHA_BITMAP_AS_COVERAGE
// is defined, which the oracle build does not define (excluded in the manifest).

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shaders;

use crate::prelude::*;

// Port of: gm/alpha_image.cpp#L23-L34 (chrome/m156)
fn make_alpha_image(w: i32, h: i32) -> Bitmap {
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(&ImageInfo::new_a8((w, h)), None);
    bm.erase_argb(10, 0, 0, 0);
    for y in 0..bm.height() {
        for x in y..bm.width() {
            bm.set_addr8(x, y, 0xFF);
        }
    }
    bm.set_immutable();
    bm
}

// Port of: gm/alpha_image.cpp#L36-L43 (chrome/m156)
fn make_color_filter() -> ColorFilter {
    // mix G and A.
    #[rustfmt::skip]
    let color_matrix = ColorMatrix::new(
        1.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.5, 0.5, 0.0,
        0.0, 0.0, 0.5, 0.5, 0.0,
    );
    color_filters::matrix(&color_matrix, Clamp::Yes).expect("color filter")
}

// Port of: gm/alpha_image.cpp#L45-L62 (chrome/m156)
crate::def_simple_gm!(alpha_image, canvas, 256, 256, {
    let image = make_alpha_image(96, 96).as_image().expect("image");
    let mut paint = Paint::default();
    paint.set_color_filter(make_color_filter());
    paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 10.0, None));
    canvas.draw_image_with_sampling_options(
        &image,
        (16.0, 16.0),
        SamplingOptions::default(),
        Some(&paint),
    );
    paint.set_color_filter(None);
    paint.set_shader(shaders::color(Color::from_argb(0xFF, 0x00, 0xFF, 0xFF)));
    canvas.draw_image_with_sampling_options(
        &image,
        (144.0, 16.0),
        SamplingOptions::default(),
        Some(&paint),
    );
    paint.set_color_filter(make_color_filter());
    canvas.draw_image_with_sampling_options(
        &image,
        (16.0, 144.0),
        SamplingOptions::default(),
        Some(&paint),
    );
    paint.set_mask_filter(None);
    canvas.draw_image_with_sampling_options(
        &image,
        (144.0, 144.0),
        SamplingOptions::default(),
        Some(&paint),
    );
});

// Port of: gm/alpha_image.cpp#L66-L88 (chrome/m156)
crate::def_simple_gm!(alpha_image_alpha_tint, canvas, 152, 80, {
    canvas.clear(Color::from_argb(0xFF, 0x88, 0x88, 0x88));
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(&ImageInfo::new_a8((64, 64)), None);
    for y in 0..bm.height() {
        for x in 0..bm.width() {
            bm.set_addr8(x, y, u8::try_from(y * 4).expect("y * 4 is at most 252"));
        }
    }
    bm.set_immutable();
    let image = bm.as_image().expect("image");

    let mut paint = Paint::default();
    paint.set_color4f(Color4f::new(0.0, 1.0, 0.0, 0.5), None);
    canvas.translate((8.0, 8.0));
    canvas.draw_image(&image, (0.0, 0.0), Some(&paint));
    canvas.translate((72.0, 0.0));
    paint.set_shader(image.to_shader(None, SamplingOptions::default(), None));
    canvas.draw_rect(Rect::from_ltrb(0.0, 0.0, 64.0, 64.0), &paint);
});
