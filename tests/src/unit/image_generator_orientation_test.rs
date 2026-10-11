// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ImageGeneratorOrientationTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_codec::codec::{Codec, Result as CodecResult};
use skia_rust_codec::codec_image_generator::CodecImageGenerator;
use skia_rust_codec::codecs;
use skia_rust_codec::pixmap_utils::{orient, swap_width_height};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::data::Data;
use skia_rust_core::m44::V3;
use skia_rust_core::stream::{MemoryStream, Stream};

use crate::resources::get_resource_as_data;
use crate::{def_test, errorf, reporter_assert};

// Port of: tests/ImageGeneratorOrientationTest.cpp#L26-L30 (chrome/m156), nearly_equal
fn nearly_equal(a: Color4f, b: Color4f) -> bool {
    let d = V3 {
        x: a.r - b.r,
        y: a.g - b.g,
        z: a.b - b.b,
    }
    .length();
    // Tolerance from the C++ test itself; kept as is.
    const K_TOL: f32 = 0.02;
    d <= K_TOL
}

// The packed value of `SkColor` (`toSkColor`), for the failure message.
fn argb(c: Color) -> u32 {
    u32::from_be_bytes([c.a(), c.r(), c.g(), c.b()])
}

// Port of: tests/ImageGeneratorOrientationTest.cpp#L32-L124 (chrome/m156), ImageGeneratorOrientationTest
def_test!(ImageGeneratorOrientationTest, |reporter| {
    for i in '1'..='8' {
        let path = format!("images/orientation/{i}_444.jpg");
        // Get the image as data
        let data = get_resource_as_data(&path);
        reporter_assert!(reporter, data.is_some(), "missing resource {path}");
        let Some(data) = data else {
            continue;
        };

        // SkCodecImageGenerator::MakeFromEncodedCodec
        let Some(mut generator) = CodecImageGenerator::make_from_encoded_codec(
            Data::new_copy(&data),
            None,
        ) else {
            reporter_assert!(reporter, false, "generator for {path}");
            continue;
        };

        // Get bitmap from image generator.
        let generator_info = generator.info().clone();
        let mut generator_bitmap = Bitmap::new();
        let success = generator_bitmap.try_alloc_pixels_info(&generator_info, None);
        reporter_assert!(reporter, success);
        let generator_pm_has_addr = generator_bitmap.pixmap().addr().is_some();
        reporter_assert!(reporter, generator_pm_has_addr);
        let success2 = match generator_bitmap.peek_pixels_mut() {
            Some(mut generator_pm) => generator.get_pixels_into(&mut generator_pm),
            None => false,
        };
        reporter_assert!(reporter, success2);

        // Get bitmap from codec. SkCodec::getPixels does not automatically apply
        // the origin so we rotate it manually.
        let mut codec_bitmap = Bitmap::new();
        let stream: Box<dyn Stream + Send> = MemoryStream::make_copy(&data);
        let codec = Codec::make_from_stream(stream, codecs::decoders());
        reporter_assert!(reporter, codec.is_ok(), "codec for {path}");
        let Ok(mut codec) = codec else {
            continue;
        };

        let mut codec_info = codec.info();
        let origin = codec.origin();

        let success = codec_bitmap.try_alloc_pixels_info(&codec_info, None);
        reporter_assert!(reporter, success);
        let row_bytes = codec_bitmap.row_bytes();
        let result = match codec_bitmap.peek_pixels_mut() {
            Some(mut pm) => match pm.writable_addr() {
                Some(pixels) => codec.get_pixels(&codec_info, pixels, row_bytes, None),
                None => CodecResult::InvalidParameters,
            },
            None => CodecResult::InvalidParameters,
        };
        reporter_assert!(reporter, result == CodecResult::Success);

        let mut tmp_bitmap = Bitmap::new();
        if origin.swaps_width_height() {
            codec_info = swap_width_height(&codec_info);
        }
        let success = tmp_bitmap.try_alloc_pixels_info(&codec_info, None);
        reporter_assert!(reporter, success);

        let success = match tmp_bitmap.peek_pixels_mut() {
            Some(mut tmp_pm) => orient(&mut tmp_pm, &codec_bitmap.pixmap(), origin),
            None => false,
        };
        reporter_assert!(reporter, success);

        codec_bitmap.swap(&mut tmp_bitmap);

        // Compare the decoded SkCodec and the bits from the SkImageGenerator.
        // They should be the same because both should respect orientation.
        if codec_bitmap.dimensions() != generator_info.dimensions() {
            let c = codec_bitmap.dimensions();
            let g = generator_bitmap.dimensions();
            errorf!(
                reporter,
                "Bitmaps do not have the same dimentions.\n\tgenBitmap: ({}, {})\tcodecBitmap: ({}, {})",
                c.width, c.height, g.width, g.height
            );
        }
        for k in 0..codec_bitmap.width() {
            for j in 0..codec_bitmap.height() {
                let c1 = generator_bitmap.get_color_4f((k, j));
                let c2 = codec_bitmap.get_color_4f((k, j));
                if !nearly_equal(c1, c2) {
                    errorf!(
                        reporter,
                        "Bitmaps for {path} do not match starting at position {i}, {j}\n\tgenBitmap: {:x}\tcodecBitmap: {:x}",
                        argb(c1.to_color()),
                        argb(c2.to_color())
                    );
                    return;
                }
            }
        }
    }
});
