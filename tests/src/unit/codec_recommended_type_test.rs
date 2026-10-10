// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CodecRecommendedTypeTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_codec::android_codec::AndroidCodec;
use skia_rust_codec::encode::png_encoder::{self, Options};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::stream::MemoryStream;

use crate::{def_test, errorf, reporter_assert};

// Port of: tests/CodecRecommendedTypeTest.cpp#L23-L43 (chrome/m156)
def_test!(Codec_recommendedF16, |r| {
    // Encode an F16 bitmap. SkPngEncoder will encode this to a true-color PNG with a bit depth of
    // 16. SkAndroidCodec should always recommend F16 for such a PNG.
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(
        &ImageInfo::new(
            (10, 10),
            ColorType::RGBAF16,
            AlphaType::Premul,
            ColorSpace::new_srgb(),
        ),
        None,
    );
    // What is drawn is not important.
    bm.erase_color(Color::BLUE);
    let Some(data) = png_encoder::encode_pixmap(&bm.pixmap(), &Options::default()) else {
        reporter_assert!(r, false);
        return;
    };
    let Some(android_codec) =
        AndroidCodec::make_from_stream(MemoryStream::make_copy(data.as_bytes()))
    else {
        errorf!(r, "Failed to create SkAndroidCodec");
        return;
    };
    reporter_assert!(
        r,
        android_codec.compute_output_color_type(ColorType::N32) == ColorType::RGBAF16
    );
});
