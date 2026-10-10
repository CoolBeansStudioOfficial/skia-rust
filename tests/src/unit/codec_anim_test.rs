// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CodecAnimTest.cpp (chrome/m156), the cases that decode GIF and the
// AnimCodecPlayer case. The other cases (Codec_565 and AndroidCodec_animated use WebP, Codec_frames
// also uses WebP, JPEG and DNG) are not ported yet.

#![cfg(test)]
// Sizes and frame indices are small, so the int-to-float and int-to-index casts are exact, as in
// tests/CodecAnimTest.cpp.
#![allow(clippy::cast_precision_loss, clippy::cast_sign_loss)]

use skia_rust_codec::android_codec::AndroidCodec;
use skia_rust_codec::codec::{NO_FRAME, Options, Result};
use skia_rust_codec::{Codec, decoders};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::MemoryStream;
use skia_rust_resources::AnimCodecPlayer;

use crate::resources::get_resource_as_data;
use crate::{Reporter, def_test, errorf, reporter_assert, skip_missing_resource};

// Port of: tests/CodecAnimTest.cpp (test_animated_AndroidCodec): for each sample size, every frame
// decoded with its prior frame matches the same frame decoded without it.
fn test_animated_android_codec(r: &mut Reporter, file: &str) {
    let data = skip_missing_resource!(get_resource_as_data(file), file);
    let Ok(codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        errorf!(r, "Failed to decode {}", file);
        return;
    };
    let Some(mut codec) = AndroidCodec::make_from_codec(codec) else {
        errorf!(r, "Failed to decode {}", file);
        return;
    };

    let mut info = codec.info().with_alpha_type(AlphaType::Premul);
    for sample_size in [8, 32, 100] {
        let dimensions = codec
            .codec()
            .get_scaled_dimensions(1.0 / sample_size as f32);
        info = info.with_dimensions(dimensions);
        let row_bytes = info.min_row_bytes();
        let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
        let frame_count = codec.codec_mut().get_frame_count();
        for i in 0..frame_count {
            let frame_info = codec.codec().get_frame_info(i);
            reporter_assert!(r, frame_info.is_some());
            let frame_info = frame_info.unwrap_or_default();
            if i == 5 {
                reporter_assert!(
                    r,
                    frame_info.disposal_method
                        == skia_rust_codec::codec_animation::DisposalMethod::RestoreBgColor
                );
            }
            let mut options = Options {
                frame_index: i,
                prior_frame: i - 1,
                ..Options::default()
            };
            info = info.with_alpha_type(frame_info.alpha_type);
            let result =
                codec
                    .codec_mut()
                    .get_pixels(&info, &mut pixels, row_bytes, Some(&options));
            reporter_assert!(r, result == Result::Success);

            // Now compare to not using prior frame.
            let mut pixels2 = vec![0u8; info.compute_byte_size(row_bytes)];
            options.prior_frame = skia_rust_codec::codec::NO_FRAME;
            let result =
                codec
                    .codec_mut()
                    .get_pixels(&info, &mut pixels2, row_bytes, Some(&options));
            reporter_assert!(r, result == Result::Success);

            let min_row = info.min_row_bytes();
            for y in 0..info.height() as usize {
                let row = y * row_bytes..y * row_bytes + min_row;
                if pixels[row.clone()] != pixels2[row] {
                    errorf!(
                        r,
                        "pixel mismatch for sample size {}, frame {} resulting in dimensions {} x {} line {}",
                        sample_size,
                        i,
                        info.width(),
                        info.height(),
                        y
                    );
                    break;
                }
            }
        }
    }
}

// Port of: tests/CodecAnimTest.cpp#L36-L45 (Codec_trunc). Reading the frame info of a GIF
// truncated at 23 bytes must not fail; there is nothing to assert beyond that.
def_test!(Codec_trunc, |_reporter| {
    let data = skip_missing_resource!(get_resource_as_data("images/box.gif"), "images/box.gif");
    // See also Codec_GifTruncated2 in GifTest.cpp for this magic 23.
    if let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data[..23]), decoders())
    {
        let _ = codec.frame_infos();
    }
});

// Port of: tests/CodecAnimTest.cpp#L587-L589 (AndroidCodec_animated_gif)
def_test!(AndroidCodec_animated_gif, |r| {
    test_animated_android_codec(r, "images/required.gif");
});

// Port of: tests/CodecAnimTest.cpp#L583-L585 (AndroidCodec_animated)
def_test!(AndroidCodec_animated, |r| {
    test_animated_android_codec(r, "images/required.webp");
});

// Port of: tests/CodecAnimTest.cpp#L50-L69 (Codec_565): a frame decoded to RGB_565 with no prior
// frame, which is blended over the background of the frame before it.
def_test!(Codec_565, |r| {
    let path = "images/blendBG.webp";
    let Some(data) = get_resource_as_data(path) else {
        return;
    };
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec
        .info()
        .with_color_type(skia_rust_core::color_type::ColorType::RGB565);
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];

    let options = Options {
        frame_index: 1,
        prior_frame: NO_FRAME,
        ..Options::default()
    };
    let result = codec.get_pixels(&info, &mut pixels, row_bytes, Some(&options));
    reporter_assert!(r, result == Result::Success);
});

// Port of: tests/CodecAnimTest.cpp#L591-L607 (chrome/m156)
def_test!(EncodedOriginToMatrixTest, |r| {
    use skia_rust_core::encoded_origin::EncodedOrigin;

    // SkAnimCodecPlayer relies on the fact that these matrices are invertible.
    for origin in [
        EncodedOrigin::TopLeft,
        EncodedOrigin::TopRight,
        EncodedOrigin::BottomRight,
        EncodedOrigin::BottomLeft,
        EncodedOrigin::LeftTop,
        EncodedOrigin::RightTop,
        EncodedOrigin::RightBottom,
        EncodedOrigin::LeftBottom,
    ] {
        // Arbitrary output dimensions.
        let matrix = origin.to_matrix(100, 80);
        let inverse = matrix.invert();
        reporter_assert!(r, inverse.is_some());
        reporter_assert!(
            r,
            inverse.is_some_and(|inverse| origin.to_matrix_inverse(100, 80) == inverse)
        );
    }
});

// Port of: tests/CodecAnimTest.cpp#L613-L654 (chrome/m156) (`DEF_TEST(AnimCodecPlayer)`)
def_test!(AnimCodecPlayer, |r| {
    // The file, its duration in milliseconds, and its size.
    let cases: [(&str, u32, ISize); 14] = [
        ("images/alphabetAnim.gif", 1300, ISize::new(100, 100)),
        ("images/randPixels.gif", 0, ISize::new(8, 8)),
        ("images/randPixels.jpg", 0, ISize::new(8, 8)),
        ("images/randPixels.png", 0, ISize::new(8, 8)),
        ("images/stoplight.webp", 2500, ISize::new(11, 29)),
        ("images/stoplight_h.webp", 2500, ISize::new(29, 11)),
        ("images/orientation/1.webp", 0, ISize::new(100, 80)),
        ("images/orientation/2.webp", 0, ISize::new(100, 80)),
        ("images/orientation/3.webp", 0, ISize::new(100, 80)),
        ("images/orientation/4.webp", 0, ISize::new(100, 80)),
        ("images/orientation/5.webp", 0, ISize::new(100, 80)),
        ("images/orientation/6.webp", 0, ISize::new(100, 80)),
        ("images/orientation/7.webp", 0, ISize::new(100, 80)),
        ("images/orientation/8.webp", 0, ISize::new(100, 80)),
    ];

    for (file, duration, size) in cases {
        let data = skip_missing_resource!(get_resource_as_data(file), file);
        let Ok(codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
            errorf!(r, "Failed to decode {}", file);
            continue;
        };

        let mut player = AnimCodecPlayer::new(codec);
        reporter_assert!(r, player.duration() == duration);
        reporter_assert!(r, player.dimensions() == size);

        let f0 = player.get_frame();
        reporter_assert!(r, f0.is_some());
        reporter_assert!(
            r,
            f0.is_some_and(|frame| frame.bounds().size() == size),
            "Mismatched size for initial frame of {}",
            file
        );

        player.seek(500);
        let f1 = player.get_frame();
        reporter_assert!(r, f1.is_some());
        reporter_assert!(
            r,
            f1.is_some_and(|frame| frame.bounds().size() == size),
            "Mismatched size for frame at 500 ms of {}",
            file
        );
    }
});
