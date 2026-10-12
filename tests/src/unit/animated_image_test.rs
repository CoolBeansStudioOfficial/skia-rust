// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/AnimatedImageTest.cpp (chrome/m156)

#![cfg(test)]
// Sizes and frame indices are small, so the casts between them and `usize` are exact, as in
// tests/AnimatedImageTest.cpp.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use skia_rust_codec::android_codec::AndroidCodec;
use skia_rust_codec::animated_image::{AnimatedImage, FINISHED};
use skia_rust_codec::codec::{Codec, FrameInfo, NO_FRAME, Options, Result as CodecResult};
use skia_rust_codec::decoders;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::size::ISize;
use skia_rust_core::stream::MemoryStream;
use skia_rust_core::un_pre_multiply::pm_color_to_color;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::resources::get_resource_as_data;
use crate::tools::tool_utils::copy_to;
use crate::{Reporter, def_test, errorf, reporter_assert, skip_without_resources};

// Port of `SkCodec::MakeFromData` followed by the `SkAndroidCodec` wrapping the tests do.
fn codec_from_data(data: &[u8]) -> Option<Codec<'static>> {
    Codec::make_from_stream(MemoryStream::make_copy(data), decoders()).ok()
}

// Port of `SkAndroidCodec::MakeFromData`.
fn android_codec_from_data(data: &[u8]) -> Option<AndroidCodec<'static>> {
    AndroidCodec::make_from_codec(codec_from_data(data)?)
}

// Converts a frame index from the C++ `int` the tests use. Frame counts are small.
fn frame_index(i: usize) -> i32 {
    i32::try_from(i).unwrap_or(i32::MAX)
}

// Port of: tests/AnimatedImageTest.cpp#L35-L52 (AnimatedImage_simple)
def_test!(AnimatedImage_simple, |r| {
    skip_without_resources!();

    let file = "images/stoplight_h.webp";
    let Some(data) = get_resource_as_data(file) else {
        errorf!(r, "Could not get {}", file);
        return;
    };

    // An animated image with a non-default exif orientation is no longer "simple"; verify that the
    // assert has been removed.
    let android_codec = android_codec_from_data(&data);
    let animated_image = AnimatedImage::make(android_codec);
    reporter_assert!(r, animated_image.is_some());
});

// Port of: tests/AnimatedImageTest.cpp#L54-L87 (AnimatedImage_rotation)
def_test!(AnimatedImage_rotation, |r| {
    skip_without_resources!();

    // These images use different exif orientations to achieve the same final dimensions
    let expected_bounds = Rect::from_iwh(100, 80);
    for i in 1..=8 {
        for name in [
            format!("images/orientation/{i}.webp"),
            format!("images/orientation/{i}_444.jpg"),
        ] {
            let file = name.as_str();
            let Some(data) = get_resource_as_data(file) else {
                errorf!(r, "Could not get {}", file);
                return;
            };

            let android_codec = android_codec_from_data(&data);
            let Some(animated_image) = AnimatedImage::make(android_codec) else {
                errorf!(r, "Failed to create animated image from {}", file);
                return;
            };

            let bounds = animated_image.bounds();
            if bounds != expected_bounds {
                errorf!(r, "Mismatched bounds for {}", file);
            }
        }
    }
});

// The `Rec` table of tests/AnimatedImageTest.cpp#L89-L147 (AnimatedImage_invalidCrop).
struct Rec {
    valid: bool,
    scaled_size: ISize,
    crop_rect: IRect,
}

// Port of: tests/AnimatedImageTest.cpp#L89-L147 (AnimatedImage_invalidCrop)
def_test!(AnimatedImage_invalidCrop, |r| {
    skip_without_resources!();

    let file = "images/alphabetAnim.gif";
    let Some(data) = get_resource_as_data(file) else {
        errorf!(r, "Could not get {}", file);
        return;
    };

    let recs = [
        // cropRect contained by original dimensions
        Rec {
            valid: true,
            scaled_size: ISize::new(100, 100),
            crop_rect: IRect::new(0, 0, 100, 100),
        },
        Rec {
            valid: true,
            scaled_size: ISize::new(100, 100),
            crop_rect: IRect::new(0, 0, 50, 50),
        },
        Rec {
            valid: true,
            scaled_size: ISize::new(100, 100),
            crop_rect: IRect::new(10, 10, 100, 100),
        },
        Rec {
            valid: true,
            scaled_size: ISize::new(100, 100),
            crop_rect: IRect::new(0, 0, 100, 100),
        },
        // unsorted cropRect
        Rec {
            valid: false,
            scaled_size: ISize::new(100, 100),
            crop_rect: IRect::new(0, 100, 100, 0),
        },
        Rec {
            valid: false,
            scaled_size: ISize::new(100, 100),
            crop_rect: IRect::new(100, 0, 0, 100),
        },
        // cropRect not contained by original dimensions
        Rec {
            valid: false,
            scaled_size: ISize::new(100, 100),
            crop_rect: IRect::new(0, 1, 100, 101),
        },
        Rec {
            valid: false,
            scaled_size: ISize::new(100, 100),
            crop_rect: IRect::new(0, -1, 100, 99),
        },
        Rec {
            valid: false,
            scaled_size: ISize::new(100, 100),
            crop_rect: IRect::new(-1, 0, 99, 100),
        },
        Rec {
            valid: false,
            scaled_size: ISize::new(100, 100),
            crop_rect: IRect::new(100, 100, 200, 200),
        },
        // cropRect contained by scaled dimensions
        Rec {
            valid: true,
            scaled_size: ISize::new(50, 50),
            crop_rect: IRect::new(0, 0, 50, 50),
        },
        Rec {
            valid: true,
            scaled_size: ISize::new(50, 50),
            crop_rect: IRect::new(0, 0, 25, 25),
        },
        Rec {
            valid: true,
            scaled_size: ISize::new(200, 200),
            crop_rect: IRect::new(0, 1, 100, 101),
        },
        // cropRect not contained by scaled dimensions
        Rec {
            valid: false,
            scaled_size: ISize::new(50, 50),
            crop_rect: IRect::new(0, 0, 75, 25),
        },
        Rec {
            valid: false,
            scaled_size: ISize::new(50, 50),
            crop_rect: IRect::new(0, 0, 25, 75),
        },
    ];
    for rec in recs {
        let Some(codec) = codec_from_data(&data) else {
            errorf!(r, "Could not create codec for {}", file);
            return;
        };
        let Some(codec) = AndroidCodec::make_from_codec(codec) else {
            errorf!(r, "Could not create codec for {}", file);
            return;
        };

        let info = codec.info();
        reporter_assert!(r, info.dimensions() == ISize::new(100, 100));

        let image = AnimatedImage::make_with_info(
            Some(codec),
            &info.with_dimensions(rec.scaled_size),
            rec.crop_rect,
            None,
        );

        reporter_assert!(r, rec.valid == image.is_some());
    }
});

// Port of: tests/AnimatedImageTest.cpp#L149-L191 (AnimatedImage_scaled)
def_test!(AnimatedImage_scaled, |r| {
    skip_without_resources!();

    let file = "images/alphabetAnim.gif";
    let Some(data) = get_resource_as_data(file) else {
        errorf!(r, "Could not get {}", file);
        return;
    };

    let Some(codec) = codec_from_data(&data) else {
        errorf!(r, "Could not create codec for {}", file);
        return;
    };
    let Some(codec) = AndroidCodec::make_from_codec(codec) else {
        errorf!(r, "Could not create codec for {}", file);
        return;
    };

    // Force the drawable follow its special case that requires scaling.
    let info = codec.info();
    let info = info.with_wh(info.width() - 5, info.height() - 5);
    let rect = info.bounds();
    let Some(image) = AnimatedImage::make_with_info(Some(codec), &info, rect, None) else {
        errorf!(r, "Failed to create animated image for {}", file);
        return;
    };

    // Clear a bitmap to non-transparent and draw to it. pixels that are transparent in the image
    // should not replace the original non-transparent color.
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(
        &ImageInfo::new_n32_premul((info.width(), info.height()), None),
        None,
    );
    bm.erase_color(Color::BLUE);
    {
        let Some(canvas) = Canvas::from_bitmap(&mut bm, None) else {
            errorf!(r, "Could not create canvas for {}", file);
            return;
        };
        image.draw(&canvas);
    }
    for i in 0..info.width() {
        for j in 0..info.height() {
            // SK_ColorTRANSPARENT is zero in every byte order.
            if bm.get_addr32(i, j) == Color::TRANSPARENT.argb() {
                errorf!(r, "Erased color underneath!");
                return;
            }
        }
    }
});

// Port of: tests/AnimatedImageTest.cpp#L193-L216 (compare_bitmaps). The C++ dumps both frames
// as PNG files on a mismatch (write_bm), a debugging aid that is not ported.
fn compare_bitmaps(
    r: &mut Reporter,
    file: &str,
    expected_frame: usize,
    expected_bm: &Bitmap,
    actual_bm: &Bitmap,
) -> bool {
    reporter_assert!(r, expected_bm.color_type() == actual_bm.color_type());
    reporter_assert!(r, expected_bm.dimensions() == actual_bm.dimensions());
    for i in 0..actual_bm.width() {
        for j in 0..actual_bm.height() {
            let expected = pm_color_to_color(expected_bm.get_addr32(i, j));
            let actual = pm_color_to_color(actual_bm.get_addr32(i, j));
            if expected != actual {
                errorf!(
                    r,
                    "frame {} of {} does not match at pixel {}, {}! expected {:x}\tactual: {:x}",
                    expected_frame,
                    file,
                    i,
                    j,
                    expected.argb(),
                    actual.argb()
                );
                return false;
            }
        }
    }
    true
}

// A transparent N32 premul bitmap of `info`'s size, drawn into by the callers.
fn transparent_bitmap(info: &ImageInfo) -> Bitmap {
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(info, None);
    bm.erase_color(Color::TRANSPARENT);
    bm
}

// Port of: tests/AnimatedImageTest.cpp#L218-L287 (AnimatedImage_copyOnWrite)
def_test!(AnimatedImage_copyOnWrite, |r| {
    skip_without_resources!();

    for file in [
        "images/alphabetAnim.gif",
        "images/colorTables.gif",
        "images/stoplight.webp",
        "images/required.webp",
    ] {
        let Some(data) = get_resource_as_data(file) else {
            errorf!(r, "Could not get {}", file);
            continue;
        };

        let Some(mut codec) = codec_from_data(&data) else {
            errorf!(r, "Could not create codec for {}", file);
            continue;
        };

        let image_info = codec.info().with_alpha_type(AlphaType::Premul);
        let frame_count = usize::try_from(codec.get_frame_count()).unwrap_or(0);
        let Some(android_codec) = AndroidCodec::make_from_codec(codec) else {
            errorf!(r, "Could not create androidCodec for {}", file);
            continue;
        };

        let Some(animated_image) = AnimatedImage::make(Some(android_codec)) else {
            errorf!(r, "Could not create animated image for {}", file);
            continue;
        };
        animated_image.set_repetition_count(0);

        let mut expected: Vec<Bitmap> = Vec::with_capacity(frame_count);
        let mut pictures = Vec::with_capacity(frame_count);
        for i in 0..frame_count {
            expected.push(transparent_bitmap(&image_info));
            let picture = animated_image.make_picture_snapshot();
            {
                let Some(canvas) = Canvas::from_bitmap(&mut expected[i], None) else {
                    errorf!(r, "Could not create canvas for {}", file);
                    continue;
                };
                let Some(snapshot) = picture.as_ref() else {
                    errorf!(r, "Could not snapshot {} frame {}", file, i);
                    continue;
                };
                canvas.draw_picture(snapshot, None, None);
            }
            pictures.push(picture);

            let duration = animated_image.decode_next_frame();
            // We're attempting to decode i + 1, so decodeNextFrame will return kFinished if that is
            // the last frame (or we attempt to decode one more).
            if i + 2 >= frame_count {
                reporter_assert!(r, duration == FINISHED);
            } else {
                reporter_assert!(r, duration != FINISHED);
            }
        }

        for i in 0..frame_count {
            let mut test = transparent_bitmap(&image_info);
            {
                let Some(canvas) = Canvas::from_bitmap(&mut test, None) else {
                    errorf!(r, "Could not create canvas for {}", file);
                    continue;
                };
                let Some(picture) = pictures[i].as_ref() else {
                    errorf!(r, "Could not snapshot {} frame {}", file, i);
                    continue;
                };
                canvas.draw_picture(picture, None, None);
            }

            compare_bitmaps(r, file, i, &expected[i], &test);
        }
    }
});

// Port of: tests/AnimatedImageTest.cpp#L289-L486 (AnimatedImage)
def_test!(AnimatedImage, |r| {
    skip_without_resources!();

    for file in [
        "images/alphabetAnim.gif",
        "images/colorTables.gif",
        "images/stoplight.webp",
        "images/required.webp",
    ] {
        let Some(data) = get_resource_as_data(file) else {
            errorf!(r, "Could not get {}", file);
            continue;
        };

        let Some(mut codec) = codec_from_data(&data) else {
            errorf!(r, "Could not create codec for {}", file);
            continue;
        };

        let frame_infos: Vec<FrameInfo> = codec.frame_infos();
        let mut frames: Vec<Bitmap> = (0..frame_infos.len()).map(|_| Bitmap::new()).collect();
        // Used down below for our test image.
        let image_info = codec.info().with_alpha_type(AlphaType::Premul);

        // Get the repetition count after the codec->getFrameInfo() call above has walked to the
        // end of the encoded image.
        //
        // At the file format level, GIF images can declare their repetition count multiple times
        // and our codec goes with "last one wins". Furthermore, for single-frame (still) GIF images,
        // a zero, positive or infinite repetition count are all equivalent in practice (in all
        // cases, the pixels do not change over time), so the codec has some leeway in what to
        // return for single-frame GIF images, but it cannot distinguish single-frame from
        // multiple-frame GIFs until we count the number of frames (e.g. call getFrameInfo).
        let default_repetition_count = codec.get_repetition_count();

        for i in 0..frame_infos.len() {
            let info = codec.info().with_alpha_type(frame_infos[i].alpha_type);

            let mut options = Options {
                frame_index: frame_index(i),
                prior_frame: frame_infos[i].required_frame,
                ..Options::default()
            };
            if options.prior_frame == NO_FRAME {
                frames[i].alloc_pixels_info(&info, None);
                frames[i].erase_color(Color::TRANSPARENT);
            } else {
                let prior = usize::try_from(options.prior_frame).unwrap_or(0);
                let prior_frame = frames[prior].clone();
                let mut bm = Bitmap::new();
                if !copy_to(&mut bm, prior_frame.color_type(), &prior_frame) {
                    errorf!(r, "Failed to copy {} frame {}", file, options.prior_frame);
                    options.prior_frame = NO_FRAME;
                }
                reporter_assert!(r, bm.set_alpha_type(frame_infos[i].alpha_type));
                frames[i] = bm;
            }

            let row_bytes = frames[i].row_bytes();
            let result = {
                let Some(mut pm) = frames[i].peek_pixels_mut() else {
                    errorf!(r, "error in {} frame {}: no pixels", file, i);
                    continue;
                };
                let Some(pixels) = pm.bytes_mut() else {
                    errorf!(r, "error in {} frame {}: no pixels", file, i);
                    continue;
                };
                codec.get_pixels(&info, pixels, row_bytes, Some(&options))
            };
            if result != CodecResult::Success {
                errorf!(r, "error in {} frame {}: {}", file, i, result.as_str());
            }
        }

        let Some(android_codec) = AndroidCodec::make_from_codec(codec) else {
            errorf!(r, "Could not create androidCodec for {}", file);
            continue;
        };

        let Some(animated_image) = AnimatedImage::make(Some(android_codec)) else {
            errorf!(r, "Could not create animated image for {}", file);
            continue;
        };

        reporter_assert!(
            r,
            default_repetition_count == animated_image.repetition_count()
        );

        reporter_assert!(
            r,
            animated_image.current_frame_duration() == frame_infos[0].duration
        );

        if !test_draw(r, &animated_image, &frames, &image_info, file, 0) {
            errorf!(r, "Did not start with frame 0");
            continue;
        }

        // Start at an arbitrary time.
        let mut failed = false;
        for i in 1..frame_infos.len() {
            let frame_time = animated_image.decode_next_frame();
            reporter_assert!(r, frame_time == animated_image.current_frame_duration());

            if i == frame_infos.len() - 1 && default_repetition_count == 0 {
                reporter_assert!(r, frame_time == FINISHED);
                reporter_assert!(r, animated_image.is_finished());
            } else {
                reporter_assert!(r, frame_time == frame_infos[i].duration);
                reporter_assert!(r, !animated_image.is_finished());
            }

            if !test_draw(r, &animated_image, &frames, &image_info, file, i) {
                errorf!(r, "Did not update to {} properly", i);
                failed = true;
                break;
            }
        }

        if failed {
            continue;
        }

        animated_image.reset();
        reporter_assert!(r, !animated_image.is_finished());
        if !test_draw(r, &animated_image, &frames, &image_info, file, 0) {
            errorf!(r, "reset failed");
            continue;
        }

        // Test reset from all the frames.
        // j is the frame to call reset on.
        for j in 0..frame_infos.len() {
            if failed {
                break;
            }

            // i is the frame to decode.
            for i in 0..=j {
                if i == j {
                    animated_image.reset();
                    if !test_draw(r, &animated_image, &frames, &image_info, file, 0) {
                        errorf!(r, "reset failed for image {} from frame {}", file, i);
                        failed = true;
                        break;
                    }
                } else if i != 0 {
                    animated_image.decode_next_frame();
                    if !test_draw(r, &animated_image, &frames, &image_info, file, i) {
                        errorf!(
                            r,
                            "failed to match frame {} in {} on iteration {}",
                            i,
                            file,
                            j
                        );
                        failed = true;
                        break;
                    }
                }
            }
        }

        if failed {
            continue;
        }

        for loop_count in [0, 1, 2, 5] {
            let Some(codec) = codec_from_data(&data) else {
                errorf!(r, "Could not create codec for {}", file);
                break;
            };
            let Some(android_codec) = AndroidCodec::make_from_codec(codec) else {
                errorf!(r, "Could not create androidCodec for {}", file);
                break;
            };
            let Some(animated_image) = AnimatedImage::make(Some(android_codec)) else {
                errorf!(r, "Could not create animated image for {}", file);
                break;
            };
            animated_image.set_repetition_count(loop_count);
            reporter_assert!(r, animated_image.repetition_count() == loop_count);

            for loops in 0..=loop_count {
                if failed {
                    break;
                }
                reporter_assert!(r, !animated_image.is_finished());
                for i in 1..=frame_infos.len() {
                    let frame_time = animated_image.decode_next_frame();
                    if frame_time == FINISHED {
                        if loops != loop_count {
                            errorf!(
                                r,
                                "{} animation stopped early: loops: {}\tloopCount: {}",
                                file,
                                loops,
                                loop_count
                            );
                            failed = true;
                        }
                        if i != frame_infos.len() - 1 {
                            errorf!(
                                r,
                                "{} animation stopped early: i: {}\tsize: {}",
                                file,
                                i,
                                frame_infos.len()
                            );
                            failed = true;
                        }
                        break;
                    }
                }
            }

            if !animated_image.is_finished() {
                errorf!(
                    r,
                    "{} animation should have finished with specified loop count ({})",
                    file,
                    loop_count
                );
            }
        }
    }
});

// The `[r, &frames, &imageInfo, file]` lambda of the C++ test: draws `animated_image` into a fresh
// bitmap and compares it with `frames[expected_frame]`.
fn test_draw(
    r: &mut Reporter,
    animated_image: &AnimatedImage,
    frames: &[Bitmap],
    image_info: &ImageInfo,
    file: &str,
    expected_frame: usize,
) -> bool {
    let mut test = transparent_bitmap(image_info);
    {
        let Some(canvas) = Canvas::from_bitmap(&mut test, None) else {
            errorf!(r, "Could not create canvas for {}", file);
            return false;
        };
        animated_image.draw(&canvas);
    }

    let frame = &frames[expected_frame];
    compare_bitmaps(r, file, expected_frame, frame, &test)
}
