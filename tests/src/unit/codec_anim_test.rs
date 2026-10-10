// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CodecAnimTest.cpp (chrome/m156). The DNG row of Codec_frames is not ported: it is
// RAW, which is not built.

#![cfg(test)]
// Sizes and frame indices are small, so the int-to-float and int-to-index casts are exact, as in
// tests/CodecAnimTest.cpp.
// Frame counts and sizes in the Codec_frames table are small, so the casts between them and
// `usize` are exact, as in tests/CodecAnimTest.cpp.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]

use skia_rust_codec::android_codec::AndroidCodec;
use skia_rust_codec::codec::{
    FrameInfo, IsAnimated, NO_FRAME, Options, REPETITION_COUNT_INFINITE, Result,
};
use skia_rust_codec::codec_animation::{Blend, DisposalMethod};
use skia_rust_codec::{Codec, decoders};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::IRect;
use skia_rust_core::stream::MemoryStream;

use crate::resources::get_resource_as_data;
use crate::tools::tool_utils::copy_to;
use crate::{Reporter, def_test, errorf, reporter_assert, skip_missing_resource};
use skia_rust_core::size::ISize;
use skia_rust_resources::AnimCodecPlayer;

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

// One row of the table in Codec_frames. Port of the anonymous struct in tests/CodecAnimTest.cpp
// (chrome/m156): the expected frame metadata of one file.
struct FrameRecord<'a> {
    name: &'static str,
    // The number of frames, including the first, which is always independent.
    frame_count: i32,
    // One less than `frame_count`, since the first frame is always independent.
    required_frames: &'a [i32],
    // Same, since the first frame should match `info`.
    alphas: &'a [AlphaType],
    // The size of this one should match `frame_count` for animated, empty otherwise.
    durations: &'a [i32],
    repetition_count: i32,
    disposal_methods: &'a [DisposalMethod],
    alpha_within_bounds: &'a [bool],
    blends: &'a [Blend],
    frame_rects: &'a [IRect],
}

// Port of: tests/CodecAnimTest.cpp#L76-L78 (restore_previous)
fn restore_previous(info: &FrameInfo) -> bool {
    info.disposal_method == DisposalMethod::RestorePrevious
}

// Port of: tests/CodecAnimTest.cpp#L110-L131 (the decode lambda in Codec_frames): decodes frame
// `index` into `bm`, after copying the cached frame `cached_index` into it when there is one.
// Returns whether the decode succeeded; a decode with a kRestorePrevious prior frame must fail.
#[allow(clippy::too_many_arguments)] // mirrors the C++ lambda's captures, which are the test's state
fn decode_frame(
    r: &mut Reporter,
    codec: &mut Codec<'_>,
    rec_name: &str,
    info: &ImageInfo,
    frame_infos: &[FrameInfo],
    bm: &mut Bitmap,
    index: i32,
    cached_index: i32,
    cached_frames: &[Bitmap],
) -> bool {
    let mut decode_info = info.clone();
    if index > 0 {
        decode_info = info.with_alpha_type(frame_infos[index as usize].alpha_type);
    }
    bm.alloc_pixels_info(&decode_info, None);
    if cached_index != NO_FRAME {
        // First copy the pixels from the cached frame
        let copied = copy_to(bm, ColorType::N32, &cached_frames[cached_index as usize]);
        reporter_assert!(r, copied);
    }
    let options = Options {
        frame_index: index,
        prior_frame: cached_index,
        ..Options::default()
    };
    let row_bytes = bm.row_bytes();
    let result = match bm.peek_pixels_mut() {
        Some(mut pixmap) => match pixmap.bytes_mut() {
            Some(pixels) => codec.get_pixels(&decode_info, pixels, row_bytes, Some(&options)),
            None => Result::InvalidParameters,
        },
        None => Result::InvalidParameters,
    };
    if cached_index != NO_FRAME && restore_previous(&frame_infos[cached_index as usize]) {
        if result == Result::InvalidParameters {
            return true;
        }
        errorf!(
            r,
            "Using a kRestorePrevious frame as fPriorFrame should fail"
        );
        return false;
    }
    if result != Result::Success {
        errorf!(
            r,
            "Failed to decode frame {} from {} when providing prior frame {}, error {}",
            index,
            rec_name,
            cached_index,
            result.as_str()
        );
    }
    result == Result::Success
}

// Port of: tests/CodecAnimTest.cpp#L97-L515 (chrome/m156), Codec_frames. Each frame's metadata
// is checked both through getFrameInfo() and through getFrameInfo(i), and every frame that has a
// required frame is decoded again with each earlier frame it may blend with as the prior frame.
// The C++ dumps the mismatching frames as PNG files for debugging; that is not ported, and the
// mismatch is reported.
def_test!(Codec_frames, |r| {
    let recs = [
        FrameRecord {
            name: "images/required.gif",
            frame_count: 7,
            required_frames: &[0, 1, 2, 3, 4, 5],
            alphas: &[
                AlphaType::Opaque,
                AlphaType::Unpremul,
                AlphaType::Unpremul,
                AlphaType::Unpremul,
                AlphaType::Unpremul,
                AlphaType::Unpremul,
            ],
            durations: &[100, 100, 100, 100, 100, 100, 100],
            repetition_count: 0,
            disposal_methods: &[
                DisposalMethod::Keep,
                DisposalMethod::RestoreBgColor,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::RestoreBgColor,
                DisposalMethod::Keep,
            ],
            alpha_within_bounds: &[false, true, true, true, true, true, true],
            blends: &[Blend::SrcOver; 7],
            frame_rects: &[
                IRect::from_ltrb(0, 0, 100, 100),
                IRect::from_ltrb(0, 0, 75, 75),
                IRect::from_ltrb(0, 0, 50, 50),
                IRect::from_ltrb(0, 0, 60, 60),
                IRect::from_ltrb(0, 0, 100, 100),
                IRect::from_ltrb(0, 0, 50, 50),
                IRect::from_ltrb(0, 0, 75, 75),
            ],
        },
        FrameRecord {
            name: "images/alphabetAnim.gif",
            frame_count: 13,
            required_frames: &[NO_FRAME, 0, 0, 0, 0, 5, 6, NO_FRAME, NO_FRAME, 9, 10, 11],
            alphas: &[AlphaType::Unpremul; 12],
            durations: &[100; 13],
            repetition_count: 0,
            disposal_methods: &[
                DisposalMethod::Keep,
                DisposalMethod::RestorePrevious,
                DisposalMethod::RestorePrevious,
                DisposalMethod::RestorePrevious,
                DisposalMethod::RestorePrevious,
                DisposalMethod::RestoreBgColor,
                DisposalMethod::Keep,
                DisposalMethod::RestoreBgColor,
                DisposalMethod::RestoreBgColor,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::RestoreBgColor,
                DisposalMethod::Keep,
            ],
            alpha_within_bounds: &[
                true, false, true, false, true, true, true, true, true, true, true, true, true,
            ],
            blends: &[Blend::SrcOver; 13],
            frame_rects: &[
                IRect::from_ltrb(25, 25, 75, 75),
                IRect::from_ltrb(25, 25, 75, 75),
                IRect::from_ltrb(25, 25, 75, 75),
                IRect::from_ltrb(37, 37, 62, 62),
                IRect::from_ltrb(37, 37, 62, 62),
                IRect::from_ltrb(25, 25, 75, 75),
                IRect::from_ltrb(0, 0, 50, 50),
                IRect::from_ltrb(0, 0, 100, 100),
                IRect::from_ltrb(25, 25, 75, 75),
                IRect::from_ltrb(25, 25, 75, 75),
                IRect::from_ltrb(0, 0, 100, 100),
                IRect::from_ltrb(25, 25, 75, 75),
                IRect::from_ltrb(37, 37, 62, 62),
            ],
        },
        FrameRecord {
            name: "images/randPixelsAnim2.gif",
            frame_count: 4,
            // required frames
            required_frames: &[0, 0, 1],
            // alphas
            alphas: &[AlphaType::Opaque, AlphaType::Opaque, AlphaType::Opaque],
            // durations
            durations: &[0, 1000, 170, 40],
            // repetition count
            repetition_count: 0,
            disposal_methods: &[
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::RestorePrevious,
                DisposalMethod::Keep,
            ],
            alpha_within_bounds: &[false, true, false, false],
            blends: &[Blend::SrcOver; 4],
            frame_rects: &[
                IRect::from_ltrb(0, 0, 8, 8),
                IRect::from_ltrb(6, 6, 8, 8),
                IRect::from_ltrb(4, 4, 8, 8),
                IRect::from_ltrb(7, 0, 8, 8),
            ],
        },
        FrameRecord {
            name: "images/randPixelsAnim.gif",
            frame_count: 13,
            // required frames
            required_frames: &[0, 1, 2, 3, 4, 3, 6, 7, 7, 7, 9, 9],
            alphas: &[AlphaType::Unpremul; 12],
            // durations
            durations: &[0, 1000, 170, 40, 220, 7770, 90, 90, 90, 90, 90, 90, 90],
            // repetition count
            repetition_count: 0,
            disposal_methods: &[
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::RestoreBgColor,
                DisposalMethod::RestoreBgColor,
                DisposalMethod::RestoreBgColor,
                DisposalMethod::RestoreBgColor,
                DisposalMethod::RestorePrevious,
                DisposalMethod::RestoreBgColor,
                DisposalMethod::RestorePrevious,
                DisposalMethod::RestorePrevious,
                DisposalMethod::RestorePrevious,
            ],
            alpha_within_bounds: &[
                false, true, true, false, true, true, false, false, true, true, false, false, true,
            ],
            blends: &[Blend::SrcOver; 13],
            frame_rects: &[
                IRect::from_ltrb(4, 4, 12, 12),
                IRect::from_ltrb(4, 4, 12, 12),
                IRect::from_ltrb(4, 4, 12, 12),
                IRect::from_ltrb(0, 0, 8, 8),
                IRect::from_ltrb(8, 8, 16, 16),
                IRect::from_ltrb(8, 8, 16, 16),
                IRect::from_ltrb(8, 8, 16, 16),
                IRect::from_ltrb(2, 2, 10, 10),
                IRect::from_ltrb(7, 7, 15, 15),
                IRect::from_ltrb(7, 7, 15, 15),
                IRect::from_ltrb(7, 7, 15, 15),
                IRect::from_ltrb(0, 0, 8, 8),
                IRect::from_ltrb(14, 14, 16, 16),
            ],
        },
        FrameRecord {
            name: "images/box.gif",
            frame_count: 1,
            required_frames: &[],
            alphas: &[],
            durations: &[],
            repetition_count: REPETITION_COUNT_INFINITE,
            disposal_methods: &[DisposalMethod::Keep],
            alpha_within_bounds: &[],
            blends: &[],
            frame_rects: &[],
        },
        FrameRecord {
            name: "images/color_wheel.gif",
            frame_count: 1,
            required_frames: &[],
            alphas: &[],
            durations: &[],
            repetition_count: REPETITION_COUNT_INFINITE,
            disposal_methods: &[DisposalMethod::Keep],
            alpha_within_bounds: &[],
            blends: &[],
            frame_rects: &[],
        },
        FrameRecord {
            name: "images/test640x479.gif",
            frame_count: 4,
            required_frames: &[0, 1, 2],
            alphas: &[AlphaType::Opaque, AlphaType::Opaque, AlphaType::Opaque],
            durations: &[200, 200, 200, 200],
            repetition_count: REPETITION_COUNT_INFINITE,
            disposal_methods: &[
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
            ],
            alpha_within_bounds: &[false, true, true, true],
            blends: &[Blend::SrcOver; 4],
            frame_rects: &[
                IRect::from_ltrb(0, 0, 640, 479),
                IRect::from_ltrb(0, 0, 640, 479),
                IRect::from_ltrb(0, 0, 640, 479),
                IRect::from_ltrb(0, 0, 640, 479),
            ],
        },
        FrameRecord {
            name: "images/colorTables.gif",
            frame_count: 2,
            required_frames: &[0],
            alphas: &[AlphaType::Opaque],
            durations: &[1000, 1000],
            repetition_count: 5,
            disposal_methods: &[DisposalMethod::Keep, DisposalMethod::Keep],
            alpha_within_bounds: &[false, true],
            blends: &[Blend::SrcOver; 2],
            frame_rects: &[
                IRect::from_ltrb(0, 0, 640, 400),
                IRect::from_ltrb(0, 0, 640, 200),
            ],
        },
        FrameRecord {
            name: "images/arrow.png",
            frame_count: 1,
            required_frames: &[],
            alphas: &[],
            durations: &[],
            repetition_count: 0,
            disposal_methods: &[],
            alpha_within_bounds: &[],
            blends: &[],
            frame_rects: &[],
        },
        FrameRecord {
            name: "images/google_chrome.ico",
            frame_count: 1,
            required_frames: &[],
            alphas: &[],
            durations: &[],
            repetition_count: 0,
            disposal_methods: &[],
            alpha_within_bounds: &[],
            blends: &[],
            frame_rects: &[],
        },
        FrameRecord {
            name: "images/brickwork-texture.jpg",
            frame_count: 1,
            required_frames: &[],
            alphas: &[],
            durations: &[],
            repetition_count: 0,
            disposal_methods: &[],
            alpha_within_bounds: &[],
            blends: &[],
            frame_rects: &[],
        },
        FrameRecord {
            name: "images/mandrill.wbmp",
            frame_count: 1,
            required_frames: &[],
            alphas: &[],
            durations: &[],
            repetition_count: 0,
            disposal_methods: &[],
            alpha_within_bounds: &[],
            blends: &[],
            frame_rects: &[],
        },
        FrameRecord {
            name: "images/randPixels.bmp",
            frame_count: 1,
            required_frames: &[],
            alphas: &[],
            durations: &[],
            repetition_count: 0,
            disposal_methods: &[],
            alpha_within_bounds: &[],
            blends: &[],
            frame_rects: &[],
        },
        FrameRecord {
            name: "images/yellow_rose.webp",
            frame_count: 1,
            required_frames: &[],
            alphas: &[],
            durations: &[],
            repetition_count: 0,
            disposal_methods: &[],
            alpha_within_bounds: &[],
            blends: &[],
            frame_rects: &[],
        },
        FrameRecord {
            name: "images/stoplight.webp",
            frame_count: 3,
            required_frames: &[0, 1],
            alphas: &[AlphaType::Opaque, AlphaType::Opaque],
            durations: &[1000, 500, 1000],
            repetition_count: REPETITION_COUNT_INFINITE,
            disposal_methods: &[
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
            ],
            alpha_within_bounds: &[false, false, false],
            blends: &[Blend::SrcOver; 3],
            frame_rects: &[
                IRect::from_ltrb(0, 0, 11, 29),
                IRect::from_ltrb(2, 10, 9, 27),
                IRect::from_ltrb(2, 2, 9, 18),
            ],
        },
        FrameRecord {
            name: "images/blendBG.webp",
            frame_count: 7,
            required_frames: &[0, NO_FRAME, NO_FRAME, NO_FRAME, 4, 4],
            alphas: &[
                AlphaType::Opaque,
                AlphaType::Opaque,
                AlphaType::Unpremul,
                AlphaType::Opaque,
                AlphaType::Unpremul,
                AlphaType::Unpremul,
            ],
            durations: &[525, 500, 525, 437, 609, 729, 444],
            repetition_count: 6,
            disposal_methods: &[
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
            ],
            alpha_within_bounds: &[false, true, false, true, false, true, true],
            blends: &[
                Blend::Src,
                Blend::SrcOver,
                Blend::Src,
                Blend::Src,
                Blend::Src,
                Blend::Src,
                Blend::Src,
            ],
            frame_rects: &[
                IRect::from_ltrb(0, 0, 200, 200),
                IRect::from_ltrb(0, 0, 200, 200),
                IRect::from_ltrb(0, 0, 200, 200),
                IRect::from_ltrb(0, 0, 200, 200),
                IRect::from_ltrb(0, 0, 200, 200),
                IRect::from_ltrb(100, 100, 200, 200),
                IRect::from_ltrb(100, 100, 200, 200),
            ],
        },
        FrameRecord {
            name: "images/required.webp",
            frame_count: 7,
            required_frames: &[0, 1, 1, NO_FRAME, 4, 4],
            alphas: &[
                AlphaType::Opaque,
                AlphaType::Unpremul,
                AlphaType::Unpremul,
                AlphaType::Opaque,
                AlphaType::Opaque,
                AlphaType::Opaque,
            ],
            durations: &[100, 100, 100, 100, 100, 100, 100],
            repetition_count: 0,
            disposal_methods: &[
                DisposalMethod::Keep,
                DisposalMethod::RestoreBgColor,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::Keep,
                DisposalMethod::RestoreBgColor,
                DisposalMethod::Keep,
            ],
            alpha_within_bounds: &[false, false, false, false, false, false, false],
            blends: &[
                Blend::Src,
                Blend::SrcOver,
                Blend::SrcOver,
                Blend::SrcOver,
                Blend::Src,
                Blend::SrcOver,
                Blend::SrcOver,
            ],
            frame_rects: &[
                IRect::from_ltrb(0, 0, 100, 100),
                IRect::from_ltrb(0, 0, 75, 75),
                IRect::from_ltrb(0, 0, 50, 50),
                IRect::from_ltrb(0, 0, 60, 60),
                IRect::from_ltrb(0, 0, 100, 100),
                IRect::from_ltrb(0, 0, 50, 50),
                IRect::from_ltrb(0, 0, 75, 75),
            ],
        },
    ];

    for rec in &recs {
        let Some(data) = get_resource_as_data(rec.name) else {
            // Useful error statement, but sometimes people run tests without resources, and they
            // do not want to see these messages.
            continue;
        };

        let Ok(codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
            errorf!(r, "Failed to create an SkCodec from '{}'", rec.name);
            continue;
        };

        // Codecs that parse frames lazily (e.g. GIF, WebP) will not have frame 0 available before
        // getFrameCount() is called. If available, verify that frame 0 is an independent frame.
        if let Some(frame0) = codec.get_frame_info(0) {
            reporter_assert!(r, frame0.required_frame == NO_FRAME);
            reporter_assert!(r, frame0.alpha_type == codec.info().alpha_type());
        }

        let expected = rec.frame_count;
        if rec.required_frames.len() + 1 != expected as usize {
            errorf!(
                r,
                "'{}' has wrong number entries in fRequiredFrames; expected: {}\tactual: {}",
                rec.name,
                expected - 1,
                rec.required_frames.len()
            );
            continue;
        }

        if expected > 1 {
            if rec.durations.len() != expected as usize {
                errorf!(
                    r,
                    "'{}' has wrong number entries in fDurations; expected: {}\tactual: {}",
                    rec.name,
                    expected,
                    rec.durations.len()
                );
                continue;
            }

            if rec.alphas.len() + 1 != expected as usize {
                errorf!(
                    r,
                    "'{}' has wrong number entries in fAlphas; expected: {}\tactual: {}",
                    rec.name,
                    expected - 1,
                    rec.alphas.len()
                );
                continue;
            }

            if rec.disposal_methods.len() != expected as usize {
                errorf!(
                    r,
                    "'{}' has wrong number entries in fDisposalMethods; expected {}\tactual: {}",
                    rec.name,
                    expected,
                    rec.disposal_methods.len()
                );
                continue;
            }
        }

        for test_vector in [true, false] {
            // Re-create the codec to reset state and test parsing.
            let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders())
            else {
                errorf!(r, "Failed to create an SkCodec from '{}'", rec.name);
                break;
            };

            // Port of `TestMode::kVector` (getFrameInfo()) and `TestMode::kIndividual`
            // (getFrameCount()).
            let (frame_count, frame_infos) = if test_vector {
                let infos = codec.frame_infos();
                // getFrameInfo returns empty set for non-animated.
                let count = if infos.is_empty() {
                    1
                } else {
                    infos.len() as i32
                };
                (count, infos)
            } else {
                (codec.get_frame_count(), Vec::new())
            };

            if frame_count != expected {
                errorf!(
                    r,
                    "'{}' expected frame count: {}\tactual: {}",
                    rec.name,
                    expected,
                    frame_count
                );
                continue;
            }

            // Get the repetition count after the codec->getFrameInfo() or codec->getFrameCount()
            // call above has walked to the end of the encoded image.
            let repetition_count = codec.get_repetition_count();
            if repetition_count != rec.repetition_count {
                errorf!(
                    r,
                    "{} repetition count does not match! expected: {}\tactual: {}",
                    rec.name,
                    rec.repetition_count,
                    repetition_count
                );
            }

            // When decoding the full, non-partial input, `isAnimated()` will just be a proxy for
            // "is there just a single frame?".
            let expected_is_animated = if rec.frame_count == 1 {
                IsAnimated::No
            } else {
                IsAnimated::Yes
            };
            let actual_is_animated = codec.is_animated();
            if expected_is_animated != actual_is_animated {
                errorf!(
                    r,
                    "{} isAnimated does not match! expected: {:?}\tactual: {:?}",
                    rec.name,
                    expected_is_animated,
                    actual_is_animated
                );
            }

            // From here on, we are only concerned with animated images.
            if frame_count == 1 {
                continue;
            }

            for i in 0..frame_count {
                let frame_info = if test_vector {
                    frame_infos[i as usize].clone()
                } else {
                    let found = codec.get_frame_info(i);
                    reporter_assert!(r, found.is_some());
                    found.unwrap_or_default()
                };
                let idx = i as usize;

                if rec.durations[idx] != frame_info.duration {
                    errorf!(
                        r,
                        "{} frame {}'s durations do not match! expected: {}\tactual: {}",
                        rec.name,
                        i,
                        rec.durations[idx],
                        frame_info.duration
                    );
                }

                let expected_alpha = if i == 0 {
                    codec.info().alpha_type()
                } else {
                    rec.alphas[idx - 1]
                };
                let alpha = frame_info.alpha_type;
                if expected_alpha != alpha {
                    errorf!(
                        r,
                        "{}'s frame {} has wrong alpha type! expected: {:?}\tactual: {:?}",
                        rec.name,
                        i,
                        expected_alpha,
                        alpha
                    );
                }

                if i == 0 {
                    reporter_assert!(r, frame_info.required_frame == NO_FRAME);
                } else if rec.required_frames[idx - 1] != frame_info.required_frame {
                    errorf!(
                        r,
                        "{}'s frame {} has wrong dependency! expected: {}\tactual: {}",
                        rec.name,
                        i,
                        rec.required_frames[idx - 1],
                        frame_info.required_frame
                    );
                }

                reporter_assert!(r, frame_info.disposal_method == rec.disposal_methods[idx]);

                if rec.alpha_within_bounds[idx] != frame_info.has_alpha_within_bounds {
                    errorf!(
                        r,
                        "{}'s frame {} has wrong alpha within bounds! expected: {:?}\tactual: {:?}",
                        rec.name,
                        i,
                        rec.alpha_within_bounds[idx],
                        frame_info.has_alpha_within_bounds
                    );
                }

                if rec.blends[idx] != frame_info.blend {
                    errorf!(
                        r,
                        "{}'s frame {} has wrong blend mode! expected: {:?}\tactual: {:?}",
                        rec.name,
                        i,
                        rec.blends[idx],
                        frame_info.blend
                    );
                }

                if rec.frame_rects[idx] != frame_info.frame_rect {
                    errorf!(
                        r,
                        "{}'s frame {} has wrong frame rect! expected: {:?}\tactual: {:?}",
                        rec.name,
                        i,
                        rec.frame_rects[idx],
                        frame_info.frame_rect
                    );
                }
            }

            if !test_vector {
                // No need to test decoding twice.
                continue;
            }

            // Compare decoding in multiple ways:
            // - Start from scratch for each frame. |codec| will have to decode the required frame
            //   (and any it depends on) to decode. This is stored in |cachedFrames|.
            // - Provide the frame that a frame depends on, so |codec| just has to blend.
            // - Provide a frame after the required frame, which will be covered up by the newest
            //   frame.
            // All should look the same.
            let mut cached_frames: Vec<Bitmap> = (0..frame_count).map(|_| Bitmap::new()).collect();
            let info = codec.info().with_color_type(ColorType::N32);

            for i in 0..frame_count {
                let mut decoded = Bitmap::new();
                let ok = decode_frame(
                    r,
                    &mut codec,
                    rec.name,
                    &info,
                    &frame_infos,
                    &mut decoded,
                    i,
                    NO_FRAME,
                    &cached_frames,
                );
                cached_frames[i as usize] = decoded;
                if !ok {
                    continue;
                }
                let req_frame = frame_infos[i as usize].required_frame;
                if req_frame == NO_FRAME {
                    // Nothing to compare against.
                    continue;
                }
                for j in req_frame..i {
                    let mut frame = Bitmap::new();
                    if restore_previous(&frame_infos[j as usize]) {
                        decode_frame(
                            r,
                            &mut codec,
                            rec.name,
                            &info,
                            &frame_infos,
                            &mut frame,
                            i,
                            j,
                            &cached_frames,
                        );
                        continue;
                    }
                    if !decode_frame(
                        r,
                        &mut codec,
                        rec.name,
                        &info,
                        &frame_infos,
                        &mut frame,
                        i,
                        j,
                        &cached_frames,
                    ) {
                        continue;
                    }

                    // Now verify they're equal.
                    let cached = &cached_frames[i as usize];
                    let cached_pixmap = cached.pixmap();
                    let frame_pixmap = frame.pixmap();
                    let (Some(cached_bytes), Some(frame_bytes)) =
                        (cached_pixmap.bytes(), frame_pixmap.bytes())
                    else {
                        errorf!(r, "{}'s frame {} has no pixels", rec.name, i);
                        continue;
                    };
                    let row_len = info.bytes_per_pixel() * info.width() as usize;
                    let cached_row_bytes = cached.row_bytes();
                    let frame_row_bytes = frame.row_bytes();
                    for y in 0..info.height() as usize {
                        let cached_row = &cached_bytes[y * cached_row_bytes..][..row_len];
                        let frame_row = &frame_bytes[y * frame_row_bytes..][..row_len];
                        if cached_row != frame_row {
                            // C++ dumps both frames as PNG files here (write_bm) for debugging.
                            errorf!(
                                r,
                                "{}'s frame {} is different (starting from line {}) when providing prior frame {}!",
                                rec.name,
                                i,
                                y,
                                j
                            );
                            break;
                        }
                    }
                }
            }
        }
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
