// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CodecTest.cpp (chrome/m156). The cases that use the PNG, BMP, ICO and WBMP
// decoders, with their `check()` helper (codec, scanline, subset, Android and image-generator
// branches), are ported here. The cases that need the JPEG, GIF, WebP or RAW decoders, or the
// PNG encoder, are ported with those decoders.

use skia_rust_codec::android_codec::{AndroidCodec, AndroidOptions};
use skia_rust_codec::codecs::{self, Decoder};
use skia_rust_codec::encode::{jpeg_encoder, png_encoder, webp_encoder};
use skia_rust_codec::image_generator_from_encoded;
use skia_rust_codec::{
    Codec, Options, Result, ScanlineOrder, ZeroInitialized, decoders, png_codec,
};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::{ColorSpace, named_gamut, named_transfer_fn};
use skia_rust_core::color_space_priv::color_space_almost_equal;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::md5::{Digest, Md5};
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::random::Random;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::{MemoryStream, Stream};
use std::sync::{Arc, Mutex};

use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::codec_priv::{
    ScopedCodecDecoders, ico_decoder, make_ico_from_png_resource, serial_test_lock,
};
use crate::resources::{get_resource_as_data, get_resource_as_image, resource_dir};
use crate::unit::codec_exact_read_test::SharedStream;
use crate::{Reporter, def_test, errorf, reporter_assert, skip_missing_resource};

/// The `getPixels(info, pixels, rowBytes)` that the C++ test template calls on either an
/// `SkCodec` or an `SkAndroidCodec`. The Android codec's `getPixels` forwards to
/// `getAndroidPixels` with the default options, which is what the impl below does.
// Port of: include/codec/SkAndroidCodec.h#L261-L263 (getPixels), chrome/m156
trait GetPixels {
    fn info(&self) -> ImageInfo;
    fn get_pixels(&mut self, info: &ImageInfo, dst: &mut [u8], row_bytes: usize) -> Result;
}

impl GetPixels for Codec<'_> {
    fn info(&self) -> ImageInfo {
        Codec::info(self)
    }

    fn get_pixels(&mut self, info: &ImageInfo, dst: &mut [u8], row_bytes: usize) -> Result {
        Codec::get_pixels(self, info, dst, row_bytes, None)
    }
}

impl GetPixels for AndroidCodec<'_> {
    fn info(&self) -> ImageInfo {
        AndroidCodec::info(self)
    }

    fn get_pixels(&mut self, info: &ImageInfo, dst: &mut [u8], row_bytes: usize) -> Result {
        AndroidCodec::get_android_pixels(self, info, dst, row_bytes, None)
    }
}

/// An `SkBitmap` made by `allocPixels(info)`: the pixels of `info` in one buffer, with the
/// minimum row bytes, so `bm.getAddr(0, y)` is `data[y * row_bytes..]`.
struct Pixels {
    info: ImageInfo,
    row_bytes: usize,
    data: Vec<u8>,
}

impl Pixels {
    // Port of: SkBitmap::allocPixels(info)
    fn alloc(info: &ImageInfo) -> Self {
        let row_bytes = info.min_row_bytes();
        Self {
            info: info.clone(),
            row_bytes,
            data: vec![0; info.compute_byte_size(row_bytes)],
        }
    }

    /// Port of `SkBitmap::getAddr(0, y)` as an offset into `data`.
    fn row_offset(&self, y: i32) -> usize {
        usize::try_from(y).unwrap_or(0) * self.row_bytes
    }

    /// Port of `SkBitmap::eraseColor`.
    fn erase(&mut self, color: Color) {
        if let Some(mut pixmap) = Pixmap::new(&self.info, &mut self.data, self.row_bytes) {
            pixmap.erase(color, None);
        }
    }

    // Port of: md5(const SkBitmap& bm) in tests/CodecTest.cpp#L93-L103
    fn md5(&self) -> Digest {
        let mut md5 = Md5::new();
        let row_len = self.info.bytes_per_pixel() * usize::try_from(self.info.width()).unwrap_or(0);
        for y in 0..self.info.height() {
            let start = self.row_offset(y);
            md5.write_bytes(&self.data[start..start + row_len]);
        }
        md5.finish()
    }
}

// Port of: tests/CodecTest.cpp#L109-L115 (compare_to_good_digest)
fn compare_to_good_digest(reporter: &mut Reporter, good_digest: &Digest, bm: &Pixels) {
    reporter_assert!(reporter, bm.md5() == *good_digest);
}

// Port of: tests/CodecTest.cpp#L122-L135 (test_info)
fn test_info<C: GetPixels>(
    reporter: &mut Reporter,
    codec: &mut C,
    info: &ImageInfo,
    expected: Result,
    good_digest: Option<&Digest>,
) {
    let mut bm = Pixels::alloc(info);
    let result = codec.get_pixels(info, &mut bm.data, bm.row_bytes);
    reporter_assert!(reporter, result == expected);
    if let Some(good_digest) = good_digest {
        compare_to_good_digest(reporter, good_digest, &bm);
    }
}

// Port of: tests/CodecTest.cpp#L137-L145 (generate_random_subset)
fn generate_random_subset(rand: &mut Random, w: i32, h: i32) -> IRect {
    let w = u32::try_from(w).unwrap_or(0);
    let h = u32::try_from(h).unwrap_or(0);
    loop {
        let left = rand.next_range_u(0, w);
        let top = rand.next_range_u(0, h);
        let right = rand.next_range_u(0, w);
        let bottom = rand.next_range_u(0, h);
        let mut rect = IRect::new(
            i32::try_from(left).unwrap_or(0),
            i32::try_from(top).unwrap_or(0),
            i32::try_from(right).unwrap_or(0),
            i32::try_from(bottom).unwrap_or(0),
        );
        rect.sort();
        if !rect.is_empty() {
            return rect;
        }
    }
}

// Port of: tests/CodecTest.cpp#L147-L159 (test_incremental_decode)
fn test_incremental_decode(
    reporter: &mut Reporter,
    codec: &mut Codec<'_>,
    info: &ImageInfo,
    good_digest: &Digest,
) {
    let mut bm = Pixels::alloc(info);
    let row_bytes = bm.row_bytes;
    match codec.start_incremental_decode(info, &mut bm.data, row_bytes, None) {
        Ok(mut decode) => {
            let (result, _) = decode.incremental_decode();
            reporter_assert!(reporter, result == Result::Success);
        }
        Err(result) => reporter_assert!(reporter, result == Result::Success),
    }
    compare_to_good_digest(reporter, good_digest, &bm);
}

// Test in stripes, similar to DM's kStripe_Mode
// Port of: tests/CodecTest.cpp#L161-L201 (test_in_stripes)
fn test_in_stripes(
    reporter: &mut Reporter,
    codec: &mut Codec<'_>,
    info: &ImageInfo,
    good_digest: &Digest,
) {
    let mut bm = Pixels::alloc(info);
    bm.erase(Color::YELLOW);

    let height = info.height();
    // Note that if numStripes does not evenly divide height there will be an extra stripe.
    let num_stripes = 4;
    if num_stripes > height {
        // Image is too small.
        return;
    }

    let stripe_height = height / num_stripes;

    // Iterate through the image twice. Once to decode odd stripes, and once for even.
    for odd_even in [1, 0] {
        let mut y = odd_even * stripe_height;
        while y < height {
            let subset = IRect::new(0, y, info.width(), (y + stripe_height).min(height));
            let options = Options {
                subset: Some(subset),
                ..Options::default()
            };
            let offset = bm.row_offset(y);
            let row_bytes = bm.row_bytes;
            match codec.start_incremental_decode(
                info,
                &mut bm.data[offset..],
                row_bytes,
                Some(&options),
            ) {
                Err(_) => {
                    errorf!(
                        reporter,
                        "failed to start incremental decode!\ttop: {}\tbottom{}",
                        subset.top(),
                        subset.bottom()
                    );
                    return;
                }
                Ok(mut decode) => {
                    if decode.incremental_decode().0 != Result::Success {
                        errorf!(
                            reporter,
                            "failed incremental decode starting from line {}",
                            y
                        );
                        return;
                    }
                }
            }
            y += 2 * stripe_height;
        }
    }

    compare_to_good_digest(reporter, good_digest, &bm);
}

// Port of: tests/CodecTest.cpp#L203-L294 (test_codec). The bitmap it decodes into is returned,
// since the scanline checks read it afterwards, and so is the digest of the full decode.
fn test_codec<C: GetPixels>(
    reporter: &mut Reporter,
    path: &str,
    codec: &mut C,
    info: &ImageInfo,
    size: ISize,
    expected: Result,
    good_digest: Option<&Digest>,
) -> (Pixels, Digest) {
    reporter_assert!(reporter, info.dimensions() == size);
    let mut bm = Pixels::alloc(info);

    let result = codec.get_pixels(info, &mut bm.data, bm.row_bytes);
    reporter_assert!(reporter, result == expected);

    let digest = bm.md5();
    if let Some(good_digest) = good_digest {
        reporter_assert!(reporter, digest == *good_digest);
    }

    {
        // Test decoding to 565
        let info565 = info.with_color_type(ColorType::RGB565);
        if info.alpha_type() == AlphaType::Opaque {
            // Decoding to 565 should succeed.
            let mut bm565 = Pixels::alloc(&info565);

            // This will allow comparison even if the image is incomplete.
            bm565.erase(Color::BLACK);

            let actual_result = codec.get_pixels(&info565, &mut bm565.data, bm565.row_bytes);
            if actual_result == expected {
                let digest565 = bm565.md5();

                // A request for non-opaque should also succeed.
                for alpha in [AlphaType::Premul, AlphaType::Unpremul] {
                    test_info(
                        reporter,
                        codec,
                        &info565.with_alpha_type(alpha),
                        expected,
                        Some(&digest565),
                    );
                }
            } else {
                errorf!(
                    reporter,
                    "Decoding {} to 565 failed with result \"{}\"\n\t\t\t\texpected:\"{}\"",
                    path,
                    actual_result.as_str(),
                    expected.as_str()
                );
            }
        } else {
            test_info(reporter, codec, &info565, Result::InvalidConversion, None);
        }
    }

    let codec_info = codec.info();
    if codec_info.color_type() == ColorType::Gray8 {
        let mut gray_bm = Pixels::alloc(&codec_info);
        gray_bm.erase(Color::BLACK);

        reporter_assert!(
            reporter,
            expected == codec.get_pixels(&codec_info, &mut gray_bm.data, gray_bm.row_bytes)
        );

        let gray_digest = gray_bm.md5();

        for alpha in [AlphaType::Premul, AlphaType::Unpremul] {
            test_info(
                reporter,
                codec,
                &codec_info.with_alpha_type(alpha),
                expected,
                Some(&gray_digest),
            );
        }
    }

    // Verify that re-decoding gives the same result.  It is interesting to check this after
    // a decode to 565, since choosing to decode to 565 may result in some of the decode
    // options being modified.  These options should return to their defaults on another
    // decode to kN32, so the new digest should match the old digest.
    test_info(reporter, codec, info, expected, Some(&digest));

    {
        // Check alpha type conversions
        if info.alpha_type() == AlphaType::Opaque {
            test_info(
                reporter,
                codec,
                &info.with_alpha_type(AlphaType::Unpremul),
                expected,
                Some(&digest),
            );
            test_info(
                reporter,
                codec,
                &info.with_alpha_type(AlphaType::Premul),
                expected,
                Some(&digest),
            );
        } else {
            // Decoding to opaque should fail
            test_info(
                reporter,
                codec,
                &info.with_alpha_type(AlphaType::Opaque),
                Result::InvalidConversion,
                None,
            );
            let other_at = if info.alpha_type() == AlphaType::Premul {
                AlphaType::Unpremul
            } else {
                AlphaType::Premul
            };
            // The other non-opaque alpha type should always succeed, but not match.
            test_info(
                reporter,
                codec,
                &info.with_alpha_type(other_at),
                expected,
                None,
            );
        }
    }

    (bm, digest)
}

// Port of: tests/CodecTest.cpp#L296-L308 (supports_partial_scanlines)
fn supports_partial_scanlines(path: &str) -> bool {
    ["jpg", "jpeg", "png", "webp", "JPG", "JPEG", "PNG", "WEBP"]
        .iter()
        .any(|ext| path.ends_with(ext))
}

// Port of: tests/CodecTest.cpp#L310-L398 (check_scanline_decode)
fn check_scanline_decode(
    reporter: &mut Reporter,
    codec: &mut Codec<'_>,
    info: &ImageInfo,
    path: &str,
    size: ISize,
    support: Support,
) -> Digest {
    // Test full image decodes with SkCodec
    let expected_result = if support.incomplete {
        Result::IncompleteInput
    } else {
        Result::Success
    };
    let (mut bm, codec_digest) =
        test_codec(reporter, path, codec, info, size, expected_result, None);

    // Scanline decoding follows.
    if support.new_scanline && !support.incomplete {
        test_incremental_decode(reporter, codec, info, &codec_digest);
        // This is only supported by codecs that use incremental decoding to
        // support subset decodes - png and jpeg (once SkJpegCodec is
        // converted).
        if path.ends_with("png") || path.ends_with("PNG") {
            test_in_stripes(reporter, codec, info, &codec_digest);
        }
    }

    // Need to call startScanlineDecode() first.
    reporter_assert!(reporter, codec.get_scanlines(&mut bm.data, 1, 0) == 0);
    reporter_assert!(reporter, !codec.skip_scanlines(1));
    let start_result = codec.start_scanline_decode(info, None);
    if support.scanline {
        bm.erase(Color::YELLOW);

        reporter_assert!(reporter, start_result == Result::Success);

        for y in 0..info.height() {
            let offset = bm.row_offset(y);
            let lines = codec.get_scanlines(&mut bm.data[offset..], 1, 0);
            if !support.incomplete {
                reporter_assert!(reporter, lines == 1);
            }
        }
        // verify that scanline decoding gives the same result.
        if codec.scanline_order() == ScanlineOrder::TopDown {
            compare_to_good_digest(reporter, &codec_digest, &bm);
        }

        // Cannot continue to decode scanlines beyond the end
        reporter_assert!(reporter, codec.get_scanlines(&mut bm.data, 1, 0) == 0);

        // Interrupting a scanline decode with a full decode starts from
        // scratch
        {
            reporter_assert!(
                reporter,
                codec.start_scanline_decode(info, None) == Result::Success
            );
            let lines = codec.get_scanlines(&mut bm.data, 1, 0);
            if !support.incomplete {
                reporter_assert!(reporter, lines == 1);
            }
            reporter_assert!(
                reporter,
                codec.get_pixels(info, &mut bm.data, bm.row_bytes, None) == expected_result
            );
            reporter_assert!(reporter, codec.get_scanlines(&mut bm.data, 1, 0) == 0);
            reporter_assert!(reporter, !codec.skip_scanlines(1));
        }

        // Test partial scanline decodes
        if supports_partial_scanlines(path) && info.width() >= 3 {
            let width = info.width();
            let height = info.height();
            let x = 2 * (width / 3);
            let subset = IRect::new(x, 0, x + width / 3, height);
            let options = Options {
                subset: Some(subset),
                ..Options::default()
            };

            let partial_start_result = codec.start_scanline_decode(info, Some(&options));
            reporter_assert!(reporter, partial_start_result == Result::Success);

            for y in 0..height {
                let offset = bm.row_offset(y);
                let lines = codec.get_scanlines(&mut bm.data[offset..], 1, 0);
                if !support.incomplete {
                    reporter_assert!(reporter, lines == 1);
                }
            }
        }
    } else {
        reporter_assert!(reporter, start_result == Result::Unimplemented);
    }

    codec_digest
}

// Port of: tests/CodecTest.cpp#L400-L440 (check_subset_decode)
fn check_subset_decode(
    reporter: &mut Reporter,
    codec: &mut Codec<'_>,
    info: &ImageInfo,
    size: ISize,
    support: Support,
) {
    // This function tests decoding subsets, and will decode a handful of randomly-sized subsets.
    // Do not attempt to decode subsets of an image of only one pixel, since there is no
    // meaningful subset.
    if size.width * size.height == 1 {
        return;
    }

    let mut rand = Random::default();
    for _ in 0..5 {
        let mut subset = generate_random_subset(&mut rand, size.width, size.height);
        let supported = codec.get_valid_subset(&mut subset);
        reporter_assert!(reporter, supported == support.subset);

        let subset_info = info.with_dimensions(ISize::new(subset.width(), subset.height()));
        let mut bm = Pixels::alloc(&subset_info);
        let opts = Options {
            subset: Some(subset),
            ..Options::default()
        };
        let result = codec.get_pixels(&subset_info, &mut bm.data, bm.row_bytes, Some(&opts));

        if support.subset {
            if !support.incomplete {
                reporter_assert!(reporter, result == Result::Success);
            }
            // Webp is the only codec that supports subsets, and it will have modified the subset
            // to have even left/top.
            reporter_assert!(reporter, subset.left() % 2 == 0 && subset.top() % 2 == 0);
        } else {
            // No subsets will work.
            reporter_assert!(reporter, result == Result::Unimplemented);
        }
    }
}

// Port of: tests/CodecTest.cpp#L442-L466 (check_android_codec)
fn check_android_codec(
    reporter: &mut Reporter,
    codec: Codec<'static>,
    codec_digest: &Digest,
    info: &ImageInfo,
    path: &str,
    size: ISize,
    support: Support,
) {
    if support.scanline || support.subset || support.new_scanline {
        let Some(mut android_codec) = AndroidCodec::make_from_codec(codec) else {
            errorf!(reporter, "Unable to decode '{}'", path);
            return;
        };

        let expected_result = if support.incomplete {
            Result::IncompleteInput
        } else {
            Result::Success
        };
        test_codec(
            reporter,
            path,
            &mut android_codec,
            info,
            size,
            expected_result,
            Some(codec_digest),
        );
    }
}

// Port of: tests/CodecTest.cpp#L468-L496 (check_codec_image_generator)
fn check_codec_image_generator(
    reporter: &mut Reporter,
    codec_digest: &Digest,
    info: &ImageInfo,
    path: &str,
    supports_incomplete: bool,
) {
    // Test SkCodecImageGenerator
    if supports_incomplete {
        return;
    }
    let Some(full_data) = get_resource_as_data(path) else {
        return;
    };
    // `SkCodecImageGenerator::MakeFromEncodedCodec(fullData)` is `MakeFromEncoded` with no alpha
    // type override, which is the path taken here.
    let Some(mut generator) =
        image_generator_from_encoded::make_from_encoded(Some(Data::new_from_vec(full_data)), None)
    else {
        reporter_assert!(reporter, false, "no codec recognises {}", path);
        return;
    };
    let mut bm = Pixels::alloc(info);
    reporter_assert!(
        reporter,
        generator.get_pixels(info, &mut bm.data, bm.row_bytes)
    );
    compare_to_good_digest(reporter, codec_digest, &bm);
    // skia-rust: the FrontBufferedStream branch of Skia's test is compiled only with
    // SK_ENABLE_ANDROID_UTILS (is_skia_dev_build), and tests Android's stream utility, which is
    // not ported.
}

/// The bool parameters of Skia's `check()`: which decoding paths a test case expects a codec to
/// support.
// The four flags mirror the bool parameters of the C++ `check()`, so they stay separate bools.
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors the bool parameters of the C++ check()"
)]
#[derive(Clone, Copy, Default)]
struct Support {
    scanline: bool,
    subset: bool,
    incomplete: bool,
    new_scanline: bool,
}

// Port of: tests/CodecTest.cpp#L498-L544 (check)
fn check(reporter: &mut Reporter, path: &str, size: ISize, support: Support) {
    // ReporterContext(r, path): the path names the failures that follow.
    reporter.set_context(Some(path.to_owned()));
    // If we're testing incomplete decodes, let's run the same test on full decodes.
    if support.incomplete {
        check_one(
            reporter,
            path,
            size,
            Support {
                incomplete: false,
                ..support
            },
        );
    }
    check_one(reporter, path, size, support);
    reporter.set_context(None);
}

// The body of `check()`, after its recursion on full decodes.
fn check_one(reporter: &mut Reporter, path: &str, size: ISize, support: Support) {
    // Initialize a codec with a data stream.
    let Some(data) = get_resource_as_data(path) else {
        return;
    };

    let codec = if support.incomplete {
        // SkCodec::MakeFromData(SkData::MakeFromStream(stream, 2 * length / 3))
        let truncated = &data[..2 * data.len() / 3];
        codecs::make_codec_from_stream(MemoryStream::make_copy(truncated))
    } else {
        codecs::make_codec_from_stream(MemoryStream::make_copy(&data))
    };
    let Ok(mut codec) = codec else {
        errorf!(reporter, "Unable to decode '{}'", path);
        return;
    };

    let info = codec.info().with_color_type(ColorType::N32);

    // Run tests with this codec.
    let codec_digest = check_scanline_decode(reporter, &mut codec, &info, path, size, support);

    check_subset_decode(reporter, &mut codec, &info, size, support);

    check_android_codec(reporter, codec, &codec_digest, &info, path, size, support);

    check_codec_image_generator(reporter, &codec_digest, &info, path, support.incomplete);
}

// Port of: tests/CodecTest.cpp#L595-L597 (chrome/m156)
def_test!(Codec_wbmp, |r| {
    check(
        r,
        "images/mandrill.wbmp",
        ISize::new(512, 512),
        Support {
            scanline: true,
            incomplete: true,
            ..Support::default()
        },
    );
});

// Port of: tests/CodecTest.cpp#L605-L609 (chrome/m156)
def_test!(Codec_bmp, |r| {
    check(
        r,
        "images/randPixels.bmp",
        ISize::new(8, 8),
        Support {
            scanline: true,
            incomplete: true,
            ..Support::default()
        },
    );
    check(
        r,
        "images/rle.bmp",
        ISize::new(320, 240),
        Support {
            scanline: true,
            incomplete: true,
            ..Support::default()
        },
    );
});

// Port of: tests/CodecTest.cpp#L611-L619 (chrome/m156)
def_test!(Codec_ico, |r| {
    // Decodes an embedded BMP image
    check(
        r,
        "images/color_wheel.ico",
        ISize::new(128, 128),
        Support {
            scanline: true,
            ..Support::default()
        },
    );
    // Decodes an embedded PNG image
    check(
        r,
        "images/google_chrome.ico",
        ISize::new(256, 256),
        Support {
            new_scanline: true,
            ..Support::default()
        },
    );
});

// Port of: tests/CodecTest.cpp#L621-L626 (chrome/m156)
def_test!(Codec_gif, |r| {
    check(
        r,
        "images/box.gif",
        ISize::new(200, 55),
        Support {
            incomplete: true,
            new_scanline: true,
            ..Support::default()
        },
    );
    check(
        r,
        "images/color_wheel.gif",
        ISize::new(128, 128),
        Support {
            incomplete: true,
            new_scanline: true,
            ..Support::default()
        },
    );
    // randPixels.gif is too small to test incomplete
    check(
        r,
        "images/randPixels.gif",
        ISize::new(8, 8),
        Support {
            new_scanline: true,
            ..Support::default()
        },
    );
});

// Port of: tests/CodecTest.cpp#L638-L655 (chrome/m156)
def_test!(Codec_png, |r| {
    let incomplete_and_new = Support {
        incomplete: true,
        new_scanline: true,
        ..Support::default()
    };
    let full_new = Support {
        new_scanline: true,
        ..Support::default()
    };
    check(
        r,
        "images/arrow.png",
        ISize::new(187, 312),
        incomplete_and_new,
    );
    check(
        r,
        "images/baby_tux.png",
        ISize::new(240, 246),
        incomplete_and_new,
    );
    check(
        r,
        "images/color_wheel.png",
        ISize::new(128, 128),
        incomplete_and_new,
    );
    // half-transparent-white-pixel.png is too small to test incomplete
    check(
        r,
        "images/half-transparent-white-pixel.png",
        ISize::new(1, 1),
        full_new,
    );
    check(
        r,
        "images/mandrill_128.png",
        ISize::new(128, 128),
        incomplete_and_new,
    );
    // mandrill_16.png is too small (relative to embedded sRGB profile) to test incomplete
    check(r, "images/mandrill_16.png", ISize::new(16, 16), full_new);
    check(
        r,
        "images/mandrill_256.png",
        ISize::new(256, 256),
        incomplete_and_new,
    );
    check(
        r,
        "images/mandrill_32.png",
        ISize::new(32, 32),
        incomplete_and_new,
    );
    check(
        r,
        "images/mandrill_512.png",
        ISize::new(512, 512),
        incomplete_and_new,
    );
    check(
        r,
        "images/mandrill_64.png",
        ISize::new(64, 64),
        incomplete_and_new,
    );
    check(
        r,
        "images/plane.png",
        ISize::new(250, 126),
        incomplete_and_new,
    );
    check(
        r,
        "images/plane_interlaced.png",
        ISize::new(250, 126),
        incomplete_and_new,
    );
    check(
        r,
        "images/randPixels.png",
        ISize::new(8, 8),
        incomplete_and_new,
    );
    check(
        r,
        "images/yellow_rose.png",
        ISize::new(400, 301),
        incomplete_and_new,
    );
});

// Port of: tests/CodecTest.cpp#L1122-L1146 (chrome/m156)
def_test!(Codec_wbmp_restrictive, |r| {
    let path = "images/mandrill.wbmp";
    let mut data = skip_missing_resource!(get_resource_as_data(path), path);

    // Modify the stream to contain a second byte with some bits set.
    data[1] = !0x9F_u8;

    // SkCodec should support this.
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec.info();
    test_info(r, &mut codec, &info, Result::Success, None);
});

// wbmp images have a header that can be arbitrarily large, depending on the size of the image. We
// cap the size at 65535, meaning we only need to look at 8 bytes to determine whether we can read
// the image. This is important because SkCodec only passes a limited number of bytes to
// SkWbmpCodec to determine whether the image is a wbmp.
// Port of: tests/CodecTest.cpp#L1148-L1172 (chrome/m156)
def_test!(Codec_wbmp_max_size, |r| {
    let max_size_wbmp: [u8; 8] = [
        0x00, 0x00, // Header
        0x83, 0xFF, 0x7F, // W: 65535
        0x83, 0xFF, 0x7F, // H: 65535
    ];
    let Ok(codec) = Codec::make_from_stream(MemoryStream::make_copy(&max_size_wbmp), decoders())
    else {
        reporter_assert!(r, false);
        return;
    };
    reporter_assert!(r, codec.info().width() == 65535);
    reporter_assert!(r, codec.info().height() == 65535);

    // Now test an image which is too big. Any image with a larger header (i.e. has bigger
    // width/height) is also too big.
    let too_big_wbmp: [u8; 8] = [
        0x00, 0x00, // Header
        0x84, 0x80, 0x00, // W: 65536
        0x84, 0x80, 0x00, // H: 65536
    ];
    let codec = Codec::make_from_stream(MemoryStream::make_copy(&too_big_wbmp), decoders());
    reporter_assert!(r, codec.is_err());
});

// Port of: tests/CodecTest.cpp#L1600-L1626 (chrome/m156)
def_test!(Codec_rowsDecoded, |r| {
    let file = "images/plane_interlaced.png";
    let data = skip_missing_resource!(get_resource_as_data(file), file);
    // This is enough to read the header etc, but no rows.
    let header_len = data.len().min(99);
    let Ok(mut codec) =
        Codec::make_from_stream(MemoryStream::make_copy(&data[..header_len]), decoders())
    else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec.info().with_color_type(ColorType::N32);
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let Ok(mut incremental) = codec.start_incremental_decode(&info, &mut pixels, row_bytes, None)
    else {
        reporter_assert!(r, false);
        return;
    };
    // The rows decoded are reported from zero, which is the value the C++ test checks for after
    // an arbitrary starting value.
    let (result, rows_decoded) = incremental.incremental_decode();
    reporter_assert!(r, result == Result::IncompleteInput);
    reporter_assert!(r, rows_decoded == 0);
});

// Port of: tests/CodecTest.cpp#L1988-L1999 (chrome/m156)
def_test!(Codec_F16_noColorSpace, |r| {
    let path = "images/color_wheel.png";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec
        .info()
        .with_color_type(ColorType::RGBAF16)
        .with_color_space(None::<ColorSpace>);
    test_info(r, &mut codec, &info, Result::Success, None);
});

// Port of: tests/CodecTest.cpp#L546-L566 (decodeToSkImage)
fn decode_to_sk_image(
    reporter: &mut Reporter,
    path: &str,
    dst_color_type: ColorType,
    dst_alpha_type: AlphaType,
) -> Option<Image> {
    let data = get_resource_as_data(path);
    reporter_assert!(reporter, data.is_some());
    let data = data?;

    let codec = codecs::make_codec_from_stream(MemoryStream::make_copy(&data));
    reporter_assert!(reporter, codec.is_ok());
    let mut codec = codec.ok()?;

    let dst_info = codec
        .info()
        .with_color_type(dst_color_type)
        .with_alpha_type(dst_alpha_type)
        .with_color_space(Some(ColorSpace::new_srgb()));
    // C++: `REPORTER_ASSERT(r, !!result == SkCodec::kSuccess)` and `REPORTER_ASSERT(r, !!image)`.
    let image = codec.get_image(dst_info, None);
    reporter_assert!(reporter, image.is_ok());
    image.ok()
}

// Port of: tests/CodecTest.cpp#L674-L703 (verifyFirstFourDecodedBytes). The expected bytes are
// memory bytes of the destination colour type, so they hold on every byte order.
fn verify_first_four_decoded_bytes(
    reporter: &mut Reporter,
    file_name: &str,
    dst_color_type: ColorType,
    dst_alpha_type: AlphaType,
    expected: [u8; 4],
) {
    let resource_path = format!("images/{file_name}");
    let Some(image) = decode_to_sk_image(reporter, &resource_path, dst_color_type, dst_alpha_type)
    else {
        // `REPORTER_ASSERT` should already fire in `decode_to_sk_image`.
        return;
    };
    let Some(pixmap) = image.peek_pixels() else {
        reporter_assert!(reporter, false);
        return;
    };
    let Some(addr) = pixmap.addr() else {
        reporter_assert!(reporter, false);
        return;
    };
    let pixel = &addr[..4];
    for i in 0..4 {
        reporter_assert!(reporter, pixel[i] == expected[i]);
    }
}

// Port of: tests/CodecTest.cpp#L706-L717 (Codec_png_plte_trns)
def_test!(Codec_png_plte_trns, |r| {
    // RGB in `PLTE` chunk is: 100 (0x64), 150 (0x96), 200 (0xC8)
    // Alpha in `tRNS` chunk is: 64 (i.e. 25% or 0x40)
    //
    // After alpha premultiplication by 25% we should get: R=25, G=38, B=50.
    verify_first_four_decoded_bytes(
        r,
        "plte_trns.png",
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        [100, 150, 200, 64],
    );
    verify_first_four_decoded_bytes(
        r,
        "plte_trns.png",
        ColorType::BGRA8888,
        AlphaType::Unpremul,
        [200, 150, 100, 64],
    );
    verify_first_four_decoded_bytes(
        r,
        "plte_trns.png",
        ColorType::RGBA8888,
        AlphaType::Premul,
        [25, 38, 50, 64],
    );
    verify_first_four_decoded_bytes(
        r,
        "plte_trns.png",
        ColorType::BGRA8888,
        AlphaType::Premul,
        [50, 38, 25, 64],
    );
});

// Port of: tests/CodecTest.cpp#L719-L732 (Codec_png_plte_trns_gama)
def_test!(Codec_png_plte_trns_gama, |r| {
    // RGB in `PLTE` chunk is: 100 (0x64), 150 (0x96), 200 (0xC8)
    // Alpha in `tRNS` chunk is: 64 (i.e. 25% or 0x40)
    //
    // After `gAMA` transformation we should get: R=161, G=197, B=227.
    //
    // After alpha premultiplication by 25% we should get: R=40, G=49, B=57.
    verify_first_four_decoded_bytes(
        r,
        "plte_trns_gama.png",
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        [161, 197, 227, 64],
    );
    verify_first_four_decoded_bytes(
        r,
        "plte_trns_gama.png",
        ColorType::BGRA8888,
        AlphaType::Unpremul,
        [227, 197, 161, 64],
    );
    verify_first_four_decoded_bytes(
        r,
        "plte_trns_gama.png",
        ColorType::RGBA8888,
        AlphaType::Premul,
        [40, 49, 57, 64],
    );
    verify_first_four_decoded_bytes(
        r,
        "plte_trns_gama.png",
        ColorType::BGRA8888,
        AlphaType::Premul,
        [57, 49, 40, 64],
    );
});

// Port of: tests/CodecTest.cpp#L2262-L2283 (chrome/m156)
def_test!(Codec_bmp_indexed_colorxform, |r| {
    let path = "images/bmp-size-32x32-8bpp.bmp";
    let data = skip_missing_resource!(get_resource_as_data(path), path);

    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        reporter_assert!(r, false);
        return;
    };

    // decode to a < 32bpp buffer with a color transform
    let decode_info = codec
        .info()
        .with_color_type(ColorType::RGB565)
        .with_color_space(ColorSpace::new_srgb_linear());
    let row_bytes = decode_info.min_row_bytes();
    let mut pixels = vec![0u8; decode_info.compute_byte_size(row_bytes)];

    // should not crash
    let res = codec.get_pixels(&decode_info, &mut pixels, row_bytes, None);
    reporter_assert!(r, res == Result::Success);
});

// Port of: tests/CodecTest.cpp#L2646-L2687 (chrome/m156)
def_test!(Codec_Bmp_b511820841, |r| {
    let path = "images/rle.bmp";
    let mut buffer = skip_missing_resource!(get_resource_as_data(path), path);
    if buffer.len() < 26 {
        return;
    }

    // Set width and height to 46341 (0x0000B505 in little endian).
    // 46341 < 65536 (kMaxDim), but 46341 * 46341 = 2,147,488,281 > INT32_MAX.
    let dim: u32 = 46341;
    buffer[18..22].copy_from_slice(&dim.to_le_bytes());
    buffer[22..26].copy_from_slice(&dim.to_le_bytes());

    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&buffer), decoders())
    else {
        reporter_assert!(r, false);
        return;
    };

    // Request kRGBA_F16_SkColorType to trigger colorXform() == true on RGBA_F16 decode path.
    let info = codec.info().with_color_type(ColorType::RGBAF16);

    // kYes_ZeroInitialized so SkSampler::Fill doesn't attempt to memset unallocated memory. The
    // destination is only a placeholder: the decode must fail before it touches the pixels.
    let opts = Options {
        zero_initialized: ZeroInitialized::Yes,
        ..Options::default()
    };
    let mut unused_pixels = [0u8; 4];
    let row_bytes = info.min_row_bytes();
    let result = codec.get_pixels(&info, &mut unused_pixels, row_bytes, Some(&opts));
    reporter_assert!(r, result != Result::Success);
});

// Port of: tests/CodecTest.cpp#L1663-L1674 (test_invalid_header)
fn test_invalid_header(reporter: &mut Reporter, path: &str) {
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let codec = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders());
    reporter_assert!(reporter, codec.is_err());
}

// Port of: tests/CodecTest.cpp#L1675-L1687 (chrome/m156)
def_test!(Codec_InvalidHeader, |r| {
    // The ICO case needs SkIcoCodec, which is ported with the ICO decoder.
    test_invalid_header(r, "invalid_images/int_overflow.ico");

    // These files report values that have caused problems with SkFILEStreams.
    // They are invalid, and should not create SkCodecs.
    test_invalid_header(r, "invalid_images/b33651913.bmp");
    test_invalid_header(r, "invalid_images/b34778578.bmp");
});

// The PNG decoder that Ico_usesRegisteredPngDecoder registers. It ignores the stream it is given and
// decodes images/plane.png instead.
// Port of: tests/CodecTest.cpp#L2695-L2701 (the lambda of Ico_usesRegisteredPngDecoder)
fn plane_png_decoder<'a>(
    _stream: Box<dyn Stream + Send + 'a>,
) -> std::result::Result<Codec<'a>, Result> {
    let data = get_resource_as_data("images/plane.png").ok_or(Result::InvalidInput)?;
    png_codec::make_from_stream(MemoryStream::make_copy(&data))
}

// Port of: tests/CodecTest.cpp#L2690-L2713 (chrome/m156), a serial test
def_test!(Ico_usesRegisteredPngDecoder, |r| {
    let _lock = serial_test_lock();
    let path = "images/mandrill_128.png";
    let ico = skip_missing_resource!(make_ico_from_png_resource(path), path);

    let _scoped = ScopedCodecDecoders::new();
    // Register a custom PNG decoder that returns a different image ("images/plane.png", 250x126)
    // than the embedded PNG ("images/mandrill_128.png", 128x128). This verifies that SkIcoCodec
    // delegates to the registered PNG decoder rather than calling libpng directly.
    codecs::register(Decoder {
        id: "png",
        is_format: png_codec::is_png_format,
        make_from_stream: plane_png_decoder,
    });

    let codec = codecs::make_codec_from_stream(MemoryStream::make_copy(&ico));
    reporter_assert!(r, codec.is_ok());
    if let Ok(codec) = codec {
        reporter_assert!(
            r,
            codec.dimensions() == ISize::new(250, 126),
            "Expected SkIcoCodec to use the registered PNG decoder"
        );
    }
});

// Port of: tests/CodecTest.cpp#L2715-L2725 (chrome/m156), a serial test
def_test!(Ico_fallbackToLibpngWhenPngNotRegistered, |r| {
    let _lock = serial_test_lock();
    let scoped = ScopedCodecDecoders::new();
    scoped.clear();
    codecs::register(ico_decoder());

    let path = "images/mandrill_128.png";
    let ico = skip_missing_resource!(make_ico_from_png_resource(path), path);

    // Even though no "png" decoder is registered, SkIcoCodec falls back to libpng.
    let codec = codecs::make_codec_from_stream(MemoryStream::make_copy(&ico));
    reporter_assert!(
        r,
        codec.is_ok(),
        "Expected fallback to libpng when PNG is not registered"
    );
});

// Port of: tests/CodecTest.cpp#L1572-L1598 (chrome/m156)
def_test!(Codec_reusePng, |r| {
    let path = "images/plane.png";
    let data = skip_missing_resource!(get_resource_as_data(path), path);

    let codec = codecs::make_codec_from_stream(MemoryStream::make_copy(&data))
        .ok()
        .and_then(AndroidCodec::make_from_codec);
    let Some(mut codec) = codec else {
        reporter_assert!(r, false, "Failed to create codec");
        return;
    };

    let mut opts = AndroidOptions {
        sample_size: 5,
        ..AndroidOptions::default()
    };
    let size = codec.get_sampled_dimensions(opts.sample_size);
    let info = codec
        .info()
        .with_dimensions(size)
        .with_color_type(ColorType::N32);
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let result = codec.get_android_pixels(&info, &mut pixels, row_bytes, Some(&opts));
    reporter_assert!(r, result == Result::Success);

    let info = codec.info().with_color_type(ColorType::N32);
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    opts.sample_size = 1;
    let result = codec.get_android_pixels(&info, &mut pixels, row_bytes, Some(&opts));
    reporter_assert!(r, result == Result::Success);
});

// Port of: tests/CodecTest.cpp#L628-L636 (chrome/m156)
def_test!(Codec_jpg, |r| {
    check(
        r,
        "images/CMYK.jpg",
        ISize::new(642, 516),
        Support {
            scanline: true,
            incomplete: true,
            ..Support::default()
        },
    );
    check(
        r,
        "images/color_wheel.jpg",
        ISize::new(128, 128),
        Support {
            scanline: true,
            incomplete: true,
            ..Support::default()
        },
    );
    // grayscale.jpg is too small to test incomplete
    check(
        r,
        "images/grayscale.jpg",
        ISize::new(128, 128),
        Support {
            scanline: true,
            ..Support::default()
        },
    );
    check(
        r,
        "images/mandrill_512_q075.jpg",
        ISize::new(512, 512),
        Support {
            scanline: true,
            incomplete: true,
            ..Support::default()
        },
    );
    // randPixels.jpg is too small to test incomplete
    check(
        r,
        "images/randPixels.jpg",
        ISize::new(8, 8),
        Support {
            scanline: true,
            ..Support::default()
        },
    );
});

// Port of: tests/CodecTest.cpp#L1172-L1220 (chrome/m156)
def_test!(Codec_jpeg_rewind, |r| {
    let path = "images/mandrill_512_q075.jpg";
    let data = skip_missing_resource!(get_resource_as_data(path), path);

    let half = &data[..data.len() / 2];
    let Some(mut codec) = AndroidCodec::make_from_stream(MemoryStream::make_copy(half)) else {
        errorf!(r, "Unable to create codec '{}'.", path);
        return;
    };

    let info = codec.info();
    let width = info.width();
    let height = info.height();
    let row_bytes = 4 * usize::try_from(width).unwrap_or(0);
    let mut pixels = vec![0u8; row_bytes * usize::try_from(height).unwrap_or(0)];

    // Perform a sampled decode.
    let mut opts = AndroidOptions {
        sample_size: 12,
        ..AndroidOptions::default()
    };
    let sampled_info = info.with_wh(width / 12, height / 12);
    let result = codec.get_android_pixels(&sampled_info, &mut pixels, row_bytes, Some(&opts));
    reporter_assert!(r, result == Result::IncompleteInput);

    // Rewind the codec and perform a full image decode.
    let result = codec.get_android_pixels(&info, &mut pixels, row_bytes, None);
    reporter_assert!(r, result == Result::IncompleteInput);

    // Now perform a subset decode.
    {
        opts.sample_size = 1;
        let subset = IRect::from_wh(100, 100);
        opts.base.subset = Some(subset);
        let result =
            codec.get_android_pixels(&info.with_wh(100, 100), &mut pixels, row_bytes, Some(&opts));
        // Though we only have half the data, it is enough to decode this subset.
        reporter_assert!(r, result == Result::Success);
    }

    // Perform another full image decode. This would read the old subset if the codec depended on
    // its old state (both SkJpegCodec::readRows and SkCodec::fillIncompleteImage used to).
    opts.base.subset = None;
    let result = codec.get_android_pixels(&info, &mut pixels, row_bytes, Some(&opts));
    reporter_assert!(r, result == Result::IncompleteInput);
});

// Port of: tests/CodecTest.cpp#L2194-L2225 (chrome/m156)
def_test!(Codec_jpeg_can_return_data_from_original_stream, |r| {
    let path = "images/dog.jpg";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let expected_bytes = data.len();

    let Ok(codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(&data)) else {
        reporter_assert!(r, false);
        return;
    };
    let Some(image) = codecs::deferred_image(Some(codec), Some(AlphaType::Unpremul)) else {
        reporter_assert!(r, false);
        return;
    };
    reporter_assert!(r, image.width() == 180, "width {} != 180", image.width());
    reporter_assert!(r, image.height() == 180, "height {} != 180", image.height());
    reporter_assert!(
        r,
        image.alpha_type() == AlphaType::Unpremul,
        "AlphaType is wrong {:?}",
        image.alpha_type()
    );

    // The whole point of DeferredFromCodec is that it allows the client to hold onto the original
    // image data for later.
    let Some(encoded) = image.ref_encoded_data() else {
        reporter_assert!(r, false);
        return;
    };
    // The returned data should be the same as what went in.
    reporter_assert!(r, encoded.size() == expected_bytes);
    reporter_assert!(r, skia_rust_codec::jpeg_codec::is_jpeg(encoded.as_bytes()));
});

// Port of: tests/CodecTest.cpp#L2227-L2242 (chrome/m156)
def_test!(Codec_jpeg_decode_progressive_truncated_stream, |r| {
    let path = "images/progressive_kitten_missing_eof.jpg";
    let data = skip_missing_resource!(get_resource_as_data(path), path);

    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec.info();
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let result = codec.get_pixels(&info, &mut pixels, row_bytes, None);
    reporter_assert!(r, result == Result::Success);
});

// Port of: tests/CodecTest.cpp#L2244-L2260 (chrome/m156)
def_test!(Codec_jpeg_decode_progressive_stream_incomplete, |r| {
    let path = "images/progressive_kitten_missing_eof.jpg";
    let data = skip_missing_resource!(get_resource_as_data(path), path);

    // SkCodec::MakeFromData(SkData::MakeFromStream(stream, 1 * length / 10))
    let truncated = &data[..data.len() / 10];
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(truncated), decoders())
    else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec.info();
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let result = codec.get_pixels(&info, &mut pixels, row_bytes, None);
    reporter_assert!(r, result == Result::IncompleteInput);
});

// Port of: tests/CodecTest.cpp#L2547-L2560 (chrome/m156)
def_test!(LibpngCodec_f16_trc_tables, |r| {
    let path = "images/f16-trc-tables.png";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec.info();
    reporter_assert!(r, info.color_space().is_some());

    // Decoding to F16 without color space conversion.
    let dst_info = info
        .with_color_type(ColorType::RGBAF16)
        .with_color_space(None::<ColorSpace>);
    // This should not crash.
    let image = codec.get_image(dst_info.clone(), None);
    reporter_assert!(r, image.is_ok());
    // `getImage` returns the decode result only in C++ (`kSuccess` is asserted there). Its
    // pixels come from `getPixels` with the same info, so that result is what is asserted here.
    let row_bytes = dst_info.min_row_bytes();
    let mut pixels = vec![0u8; dst_info.compute_byte_size(row_bytes)];
    let result = codec.get_pixels(&dst_info, &mut pixels, row_bytes, None);
    reporter_assert!(r, result == Result::Success);
});

// Port of: tests/CodecTest.cpp#L1953-L1986 (chrome/m156)
def_test!(Codec_crbug807324, |r| {
    // Port of `GetResourcePath().isEmpty()`: without Skia's resources there is nothing to test.
    if resource_dir().is_none() {
        return;
    }

    let file = "images/crbug807324.png";
    let Some(image) = get_resource_as_image(file) else {
        errorf!(r, "Missing {}", file);
        return;
    };

    let width = image.width();
    let height = image.height();

    let mut bm = Bitmap::new();
    if !bm.try_alloc_pixels_info(&ImageInfo::new_n32_premul((width, height), None), None) {
        errorf!(r, "Could not allocate pixels ({} x {})", width, height);
        return;
    }

    bm.erase_color(Color::TRANSPARENT);

    let canvas = Canvas::from_bitmap(&mut bm, None).expect("canvas");
    canvas.draw_image(&image, (0, 0), None);
    // skia-rust: the canvas borrows the bitmap until it drops (the C++ reads the bitmap through
    // its own pointer meanwhile), so it is dropped before the pixels are read.
    drop(canvas);

    for i in 0..width {
        for j in 0..height {
            if bm.get_addr32(i, j) == u32::from(Color::TRANSPARENT) {
                errorf!(r, "image should not be transparent! {}, {} is 0", i, j);
                return;
            }
        }
    }
});

// A stream that is neither seekable nor has a length, over a copy of the data. Port of
// tests/FakeStreams.h#L43-L57 (NonseekableStream): it reads, but cannot rewind or seek.
struct NonseekableStream {
    inner: Box<MemoryStream>,
}

impl NonseekableStream {
    fn new(data: &[u8]) -> Self {
        Self {
            inner: MemoryStream::make_copy(data),
        }
    }
}

impl Stream for NonseekableStream {
    fn read(&mut self, buffer: &mut [u8]) -> usize {
        self.inner.read(buffer)
    }

    fn peek(&mut self, buffer: &mut [u8]) -> usize {
        self.inner.peek(buffer)
    }

    fn is_at_end(&self) -> bool {
        self.inner.is_at_end()
    }

    fn rewind(&mut self) -> bool {
        false
    }

    fn seek(&mut self, _position: usize) -> bool {
        false
    }
}

// Port of: tests/CodecTest.cpp#L1443-L1487 (LimitedRewindingStream): a stream that can only rewind
// while it has read no more than `limit` bytes. It does not report a position or a length.
struct LimitedRewindingStream {
    stream: Box<dyn Stream + Send>,
    limit: usize,
    position: usize,
}

impl Stream for LimitedRewindingStream {
    fn read(&mut self, buffer: &mut [u8]) -> usize {
        let bytes = self.stream.read(buffer);
        self.position += bytes;
        bytes
    }

    fn is_at_end(&self) -> bool {
        self.stream.is_at_end()
    }

    fn rewind(&mut self) -> bool {
        if self.position <= self.limit && self.stream.rewind() {
            self.position = 0;
            true
        } else {
            false
        }
    }
}

// Port of: tests/CodecTest.cpp#L1531-L1555 (seek_and_decode): the first frame is decoded after the
// frame count is read, so that the decode needs a rewind.
fn seek_and_decode(reporter: &mut Reporter, file: &str, stream: Box<dyn Stream + Send>) {
    let Ok(mut codec) = Codec::make_from_stream(stream, decoders()) else {
        errorf!(reporter, "Failed to create codec for {},", file);
        return;
    };
    // Trigger reading through the stream, so that decoding the first frame will require a rewind.
    let _ = codec.get_frame_count();
    let info = codec.info().with_color_type(ColorType::N32);
    let mut bm = Pixels::alloc(&info);
    let result = codec.get_pixels(&info, &mut bm.data, bm.row_bytes, None);
    if result != Result::Success {
        errorf!(
            reporter,
            "Failed to decode {} with error {}",
            file,
            result.as_str()
        );
    }
}

// Port of: tests/CodecTest.cpp#L1557-L1567 (Wuffs_seek_and_decode). Only the first stream is
// tried: the FrontBufferedStream variant needs SK_ENABLE_ANDROID_UTILS, which is not built.
def_test!(Wuffs_seek_and_decode, |r| {
    let file = "images/flightAnim.gif";
    let data = skip_missing_resource!(get_resource_as_data(file), file);
    let stream = LimitedRewindingStream {
        stream: MemoryStream::make_copy(&data),
        limit: skia_rust_codec::codec::MIN_BUFFERED_BYTES_NEEDED,
        position: 0,
    };
    seek_and_decode(r, file, Box::new(stream));
});

// Port of: tests/CodecTest.cpp#L2063-L2094 (Codec_gif_notseekable). A non-seekable stream decodes
// the first frame the same as a seekable one.
def_test!(Codec_gif_notseekable, |r| {
    let path = "images/flightAnim.gif";
    let data = skip_missing_resource!(get_resource_as_data(path), path);

    // Verify that using a non-seekable stream works the same as a seekable one for decoding the
    // first frame.
    let good_digest = {
        let Ok(mut codec) = skia_rust_codec::wuffs_codec::make_from_stream_with_policy(
            MemoryStream::make_copy(&data),
            skia_rust_codec::codec::SelectionPolicy::PreferAnimation,
        ) else {
            reporter_assert!(r, false);
            return;
        };
        reporter_assert!(r, codec.get_frame_count() == 60);
        let info = codec.info();
        let mut bm = Pixels::alloc(&info);
        let result = codec.get_pixels(&info, &mut bm.data, bm.row_bytes, None);
        reporter_assert!(r, result == Result::Success);
        bm.md5()
    };

    let Ok(mut codec) = skia_rust_codec::wuffs_codec::make_from_stream_with_policy(
        Box::new(NonseekableStream::new(&data)),
        skia_rust_codec::codec::SelectionPolicy::PreferStillImage,
    ) else {
        reporter_assert!(r, false);
        return;
    };
    reporter_assert!(r, codec.get_frame_count() == 1);
    let info = codec.info();
    test_info(r, &mut codec, &info, Result::Success, Some(&good_digest));
});

// Port of: tests/CodecTest.cpp#L2096-L2137 (Codec_gif_notseekable2). A non-seekable stream decodes
// a later frame the same as a seekable one, by copying the stream.
def_test!(Codec_gif_notseekable2, |r| {
    let path = "images/flightAnim.gif";
    let data = skip_missing_resource!(get_resource_as_data(path), path);

    // Verify that using a non-seekable stream works the same as a seekable one for decoding a
    // later frame.
    let options = Options {
        frame_index: 5,
        ..Options::default()
    };
    let good_digest = {
        let Ok(mut codec) = skia_rust_codec::wuffs_codec::make_from_stream_with_policy(
            MemoryStream::make_copy(&data),
            skia_rust_codec::codec::SelectionPolicy::PreferAnimation,
        ) else {
            reporter_assert!(r, false);
            return;
        };
        reporter_assert!(r, codec.get_frame_count() == 60);
        let info = codec.info();
        let mut bm = Pixels::alloc(&info);
        let result = codec.get_pixels(&info, &mut bm.data, bm.row_bytes, Some(&options));
        reporter_assert!(r, result == Result::Success);
        bm.md5()
    };

    // This should copy the non seekable stream.
    let Ok(mut codec) = skia_rust_codec::wuffs_codec::make_from_stream_with_policy(
        Box::new(NonseekableStream::new(&data)),
        skia_rust_codec::codec::SelectionPolicy::PreferAnimation,
    ) else {
        reporter_assert!(r, false);
        return;
    };
    reporter_assert!(r, codec.get_frame_count() == 60);
    let info = codec.info();
    let mut bm = Pixels::alloc(&info);
    let result = codec.get_pixels(&info, &mut bm.data, bm.row_bytes, Some(&options));
    reporter_assert!(r, result == Result::Success);
    compare_to_good_digest(r, &good_digest, &bm);
});

// Port of: tests/CodecTest.cpp#L2163-L2185 (Codec_gif_can_preserve_original_data). A deferred image
// made from a GIF keeps the encoded data it was made from.
def_test!(Codec_gif_can_preserve_original_data, |r| {
    let path = "images/flightAnim.gif";
    let data = skip_missing_resource!(get_resource_as_data(path), path);

    let encoded = Data::new_copy(&data);
    let Ok(codec) =
        skia_rust_codec::wuffs_codec::make_from_stream(MemoryStream::make(Some(encoded)))
    else {
        reporter_assert!(r, false);
        return;
    };
    let Some(image) = codecs::deferred_image(Some(codec), Some(AlphaType::Premul)) else {
        reporter_assert!(r, false);
        return;
    };
    reporter_assert!(r, image.width() == 320);
    reporter_assert!(r, image.height() == 240);
    reporter_assert!(r, image.alpha_type() == AlphaType::Premul);

    // The whole point of DeferredFromCodec is that it allows the client to hold onto the original
    // image data for later. The returned data should be the same as what went in.
    let Some(encoded_data) = image.ref_encoded_data() else {
        reporter_assert!(r, false);
        return;
    };
    reporter_assert!(r, encoded_data.size() == data.len());
    reporter_assert!(r, encoded_data.as_bytes() == data.as_slice());
});

/// `LimitedPeekingMemStream`: a stream whose `peek` returns at most `limit` bytes, and whose `read`
/// and `rewind` are the memory stream's.
// Port of: tests/CodecTest.cpp#L1051-L1075 (LimitedPeekingMemStream)
struct LimitedPeekingStream {
    inner: Box<MemoryStream>,
    limit: usize,
}

impl LimitedPeekingStream {
    fn new(data: &[u8], limit: usize) -> Self {
        Self {
            inner: MemoryStream::make_copy(data),
            limit,
        }
    }
}

impl Stream for LimitedPeekingStream {
    fn read(&mut self, buffer: &mut [u8]) -> usize {
        self.inner.read(buffer)
    }

    fn peek(&mut self, buffer: &mut [u8]) -> usize {
        let n = buffer.len().min(self.limit);
        self.inner.peek(&mut buffer[..n])
    }

    fn is_at_end(&self) -> bool {
        self.inner.is_at_end()
    }

    fn rewind(&mut self) -> bool {
        self.inner.rewind()
    }
}

// Port of: tests/CodecTest.cpp#L599-L603 (chrome/m156)
def_test!(Codec_webp, |r| {
    check(
        r,
        "images/baby_tux.webp",
        ISize::new(386, 395),
        Support {
            subset: true,
            incomplete: true,
            ..Support::default()
        },
    );
    check(
        r,
        "images/color_wheel.webp",
        ISize::new(128, 128),
        Support {
            subset: true,
            incomplete: true,
            ..Support::default()
        },
    );
    check(
        r,
        "images/yellow_rose.webp",
        ISize::new(400, 301),
        Support {
            subset: true,
            incomplete: true,
            ..Support::default()
        },
    );
});

// Port of: tests/CodecTest.cpp#L1096-L1116 (chrome/m156)
def_test!(Codec_webp_peek, |r| {
    let path = "images/baby_tux.webp";
    let data = skip_missing_resource!(get_resource_as_data(path), path);

    // The limit is less than webp needs to peek or read.
    let Ok(mut codec) =
        codecs::make_codec_from_stream(Box::new(LimitedPeekingStream::new(&data, 25)))
    else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec.info();
    test_info(r, &mut codec, &info, Result::Success, None);

    // Similarly, a stream which does not peek should still succeed.
    let Ok(mut codec) =
        codecs::make_codec_from_stream(Box::new(LimitedPeekingStream::new(&data, 0)))
    else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec.info();
    test_info(r, &mut codec, &info, Result::Success, None);
});

// Port of: tests/CodecTest.cpp#L1840-L1859 (chrome/m156)
def_test!(Codec_webp_rowsDecoded, |r| {
    let path = "images/baby_tux.webp";
    let data = skip_missing_resource!(get_resource_as_data(path), path);

    // Truncate this file so that the header is available but no rows can be decoded. This should
    // create a codec but fail to decode.
    let truncated_size = 5000;
    let Ok(mut codec) =
        codecs::make_codec_from_stream(MemoryStream::make_copy(&data[..truncated_size]))
    else {
        errorf!(
            r,
            "Failed to create a codec for {} truncated to only {} bytes",
            path,
            truncated_size
        );
        return;
    };
    let info = codec.info();
    test_info(r, &mut codec, &info, Result::InvalidInput, None);
});

// Port of: tests/CodecTest.cpp#L2521-L2545 (chrome/m156)
def_test!(Codec_webp_animated_image_rewind, |r| {
    // stoplight.webp is an animated image.
    let path = "images/stoplight.webp";
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "Could not create data for: {}", path);
        return;
    };
    let Ok(mut codec) = codecs::make_codec_from_stream(Box::new(NonseekableStream::new(&data)))
    else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec.info();
    let mut bm = Pixels::alloc(&info);
    let options = Options {
        frame_index: 0,
        ..Options::default()
    };
    let res = codec.get_pixels(&info, &mut bm.data, bm.row_bytes, Some(&options));
    reporter_assert!(r, res == Result::Success);

    // For a non-rewindable stream, reading the next frame from an animated image should still
    // succeed.
    let options = Options {
        prior_frame: 0,
        frame_index: 1,
        ..Options::default()
    };
    let res = codec.get_pixels(&info, &mut bm.data, bm.row_bytes, Some(&options));
    reporter_assert!(r, res == Result::Success);
});

// Port of: tests/CodecTest.cpp#L1934-L1951 (chrome/m156)
def_test!(Codec_A8, |r| {
    let path = "images/mandrill_cmyk.jpg";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        reporter_assert!(r, false, "failed to create codec from {path}");
        return;
    };
    let info = codec.info().with_color_type(ColorType::Alpha8);
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    reporter_assert!(
        r,
        codec.get_pixels(&info, &mut pixels, row_bytes, None) == Result::InvalidConversion
    );
});

// Port of: tests/CodecTest.cpp#L859-L886 (chrome/m156)
def_test!(Codec_Empty, |r| {
    // Test images that should not be able to create a codec.
    let invalid = [
        "empty_images/zero-dims.gif",
        "empty_images/zero-embedded.ico",
        "empty_images/zero-width.bmp",
        "empty_images/zero-height.bmp",
        "empty_images/zero-width.jpg",
        "empty_images/zero-height.jpg",
        "empty_images/zero-width.png",
        "empty_images/zero-height.png",
        "empty_images/zero-width.wbmp",
        "empty_images/zero-height.wbmp",
        // This image is an ico with an embedded mask-bmp. This is illegal.
        "invalid_images/mask-bmp-ico.ico",
        // It is illegal for a webp frame to not be fully contained by the canvas.
        "invalid_images/invalid-offset.webp",
        "invalid_images/b37623797.ico",
        "invalid_images/osfuzz6295.webp",
        "invalid_images/osfuzz6288.bmp",
        "invalid_images/ossfuzz6347",
    ];
    for path in invalid {
        let data = skip_missing_resource!(get_resource_as_data(path), path);
        reporter_assert!(
            r,
            Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()).is_err(),
            "{path} should not create a codec"
        );
    }
});

// Port of: tests/CodecTest.cpp#L1898-L1932 (chrome/m156)
def_test!(Codec_78329453, |r| {
    // A bug in jpeg_skip_scanlines resulted in an infinite loop for this specific
    // sample size on this image. Other sample sizes could have had the same result,
    // but the ones tested by DM happen to not.
    const SAMPLE_SIZE: i32 = 19;
    let file = "images/b78329453.jpeg";
    let data = skip_missing_resource!(get_resource_as_data(file), file);
    let Ok(codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        reporter_assert!(r, false, "failed to create codec from {file}");
        return;
    };
    let Some(mut codec) = AndroidCodec::make_from_codec(codec) else {
        reporter_assert!(r, false, "failed to create codec from {file}");
        return;
    };

    let size = codec.get_sampled_dimensions(SAMPLE_SIZE);
    let info = codec.info().with_dimensions(size);
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let options = AndroidOptions {
        sample_size: SAMPLE_SIZE,
        ..AndroidOptions::default()
    };
    let result = codec.get_android_pixels(&info, &mut pixels, row_bytes, Some(&options));
    reporter_assert!(
        r,
        result == Result::Success,
        "failed to decode with error {result:?}"
    );
});

// Port of: tests/CodecTest.cpp#L1222-L1248 (check_color_xform, chrome/m156)
fn check_color_xform(reporter: &mut Reporter, path: &str) {
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Some(mut codec) = AndroidCodec::make_from_stream(MemoryStream::make_copy(&data)) else {
        reporter_assert!(reporter, false, "Unable to create codec '{path}'");
        return;
    };

    let sample_size = 3;
    let subset_width = codec.info().width() / 2;
    let subset_height = codec.info().height() / 2;
    let subset = IRect::from_wh(subset_width, subset_height);
    let opts = AndroidOptions {
        sample_size,
        base: Options {
            subset: Some(subset),
            ..Options::default()
        },
    };

    let dst_width = subset_width / sample_size;
    let dst_height = subset_height / sample_size;
    let color_space = ColorSpace::new_rgb(&named_transfer_fn::DOT22, &named_gamut::ADOBE_RGB);
    let dst_info = codec
        .info()
        .with_dimensions(ISize::new(dst_width, dst_height))
        .with_color_type(ColorType::N32)
        .with_color_space(color_space);

    let row_bytes = dst_info.min_row_bytes();
    let mut pixels = vec![0u8; dst_info.compute_byte_size(row_bytes)];
    let result = codec.get_android_pixels(&dst_info, &mut pixels, row_bytes, Some(&opts));
    reporter_assert!(reporter, result == Result::Success);
}

// Port of: tests/CodecTest.cpp#L1245-L1248 (Codec_ColorXform)
def_test!(Codec_ColorXform, |r| {
    check_color_xform(r, "images/mandrill_512_q075.jpg");
    check_color_xform(r, "images/mandrill_512.png");
});

// Port of: tests/CodecTest.cpp#L1335-L1388 (test_conversion_possible, chrome/m156)
fn test_conversion_possible(
    reporter: &mut Reporter,
    path: &str,
    supports_scanline_decoder: bool,
    supports_incremental_decoder: bool,
) {
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        reporter_assert!(reporter, false, "failed to create a codec for {path}");
        return;
    };

    let mut info_f16 = codec.info().with_color_type(ColorType::RGBAF16);
    for pass in 0..2 {
        if pass == 1 {
            // Port of `infoF16.makeColorSpace(infoF16.colorSpace()->makeLinearGamma())`.
            let Some(cs) = info_f16.color_space() else {
                reporter_assert!(reporter, false, "{path} has no colour space");
                return;
            };
            info_f16 = info_f16.with_color_space(Some(cs.with_linear_gamma()));
        }

        let row_bytes = info_f16.min_row_bytes();
        let mut bm = vec![0u8; info_f16.compute_byte_size(row_bytes)];
        let result = codec.get_pixels(&info_f16, &mut bm, row_bytes, None);
        reporter_assert!(reporter, result == Result::Success);

        // The first pass accepts kSuccess for an unsupported decoder too; the second pass
        // requires kUnimplemented, as the C++ test does.
        let result = codec.start_scanline_decode(&info_f16, None);
        if supports_scanline_decoder {
            reporter_assert!(reporter, result == Result::Success);
        } else if pass == 0 {
            reporter_assert!(
                reporter,
                result == Result::Unimplemented || result == Result::Success
            );
        } else {
            reporter_assert!(reporter, result == Result::Unimplemented);
        }

        let result = match codec.start_incremental_decode(&info_f16, &mut bm, row_bytes, None) {
            Ok(_) => Result::Success,
            Err(result) => result,
        };
        if supports_incremental_decoder {
            reporter_assert!(reporter, result == Result::Success);
        } else if pass == 0 {
            reporter_assert!(
                reporter,
                result == Result::Unimplemented || result == Result::Success
            );
        } else {
            reporter_assert!(reporter, result == Result::Unimplemented);
        }
    }
}

// Port of: tests/CodecTest.cpp#L1390-L1394 (Codec_F16ConversionPossible)
def_test!(Codec_F16ConversionPossible, |r| {
    test_conversion_possible(r, "images/color_wheel.webp", false, false);
    test_conversion_possible(r, "images/mandrill_512_q075.jpg", true, false);
    test_conversion_possible(r, "images/yellow_rose.png", false, true);
});

/// Port of `color_type_match` (tests/CodecTest.cpp#L1250-L1259): RGBA and BGRA decodes are
/// interchangeable for the purpose of this check.
// Port of: tests/CodecTest.cpp#L1250-L1259 (chrome/m156)
fn color_type_match(orig_color_type: ColorType, codec_color_type: ColorType) -> bool {
    match orig_color_type {
        ColorType::RGBA8888 | ColorType::BGRA8888 => {
            codec_color_type == ColorType::RGBA8888 || codec_color_type == ColorType::BGRA8888
        }
        _ => orig_color_type == codec_color_type,
    }
}

/// Port of `alpha_type_match` (tests/CodecTest.cpp#L1261-L1270): premul and unpremul decodes are
/// interchangeable for the purpose of this check.
// Port of: tests/CodecTest.cpp#L1261-L1270 (chrome/m156)
fn alpha_type_match(orig_alpha_type: AlphaType, codec_alpha_type: AlphaType) -> bool {
    match orig_alpha_type {
        AlphaType::Unpremul | AlphaType::Premul => {
            codec_alpha_type == AlphaType::Unpremul || codec_alpha_type == AlphaType::Premul
        }
        _ => orig_alpha_type == codec_alpha_type,
    }
}

/// Port of `check_round_trip` (tests/CodecTest.cpp#L1272-L1292): decodes `orig_codec` to `info`,
/// encodes those pixels as PNG, decodes the PNG to `info`, and compares the two decodes by MD5.
// Port of: tests/CodecTest.cpp#L1272-L1292 (chrome/m156)
fn check_round_trip(reporter: &mut Reporter, orig_codec: &mut Codec<'_>, info: &ImageInfo) {
    let mut bm1 = Pixels::alloc(info);
    let result = orig_codec.get_pixels(info, &mut bm1.data, bm1.row_bytes, None);
    reporter_assert!(reporter, result == Result::Success);

    // Encode the image to png.
    let Some(pixmap) = Pixmap::new_readonly(&bm1.info, &bm1.data, bm1.row_bytes) else {
        reporter_assert!(reporter, false);
        return;
    };
    let Some(data) = png_encoder::encode_pixmap(&pixmap, &png_encoder::Options::default()) else {
        reporter_assert!(reporter, false);
        return;
    };

    let Ok(mut codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(data.as_bytes()))
    else {
        reporter_assert!(reporter, false);
        return;
    };
    reporter_assert!(
        reporter,
        color_type_match(info.color_type(), codec.info().color_type())
    );
    reporter_assert!(
        reporter,
        alpha_type_match(info.alpha_type(), codec.info().alpha_type())
    );

    let mut bm2 = Pixels::alloc(info);
    let result = codec.get_pixels(info, &mut bm2.data, bm2.row_bytes, None);
    reporter_assert!(reporter, result == Result::Success);

    reporter_assert!(reporter, bm1.md5() == bm2.md5());
}

/// `GetResourceAsStream` followed by `SkCodec::MakeFromStream`. Returns `None` when the resource
/// is missing (the caller then has nothing to test) and reports a failed decode.
fn make_codec_from_resource(reporter: &mut Reporter, path: &str) -> Option<Codec<'static>> {
    let data = get_resource_as_data(path)?;
    let codec = codecs::make_codec_from_stream(MemoryStream::make_copy(&data));
    if codec.is_err() {
        errorf!(reporter, "Unable to decode '{}'", path);
    }
    codec.ok()
}

// Port of: tests/CodecTest.cpp#L1294-L1333 (chrome/m156)
def_test!(Codec_pngRoundTrip, |r| {
    let Some(mut codec) = make_codec_from_resource(r, "images/mandrill_512_q075.jpg") else {
        return;
    };

    let color_types_opaque = [ColorType::RGB565, ColorType::RGBA8888, ColorType::BGRA8888];
    for color_type in color_types_opaque {
        let new_info = codec.info().with_color_type(color_type);
        check_round_trip(r, &mut codec, &new_info);
    }

    let Some(mut codec) = make_codec_from_resource(r, "images/grayscale.jpg") else {
        return;
    };
    let info = codec.info();
    check_round_trip(r, &mut codec, &info);

    let Some(mut codec) = make_codec_from_resource(r, "images/yellow_rose.png") else {
        return;
    };

    let color_types_with_alpha = [ColorType::RGBA8888, ColorType::BGRA8888];
    let alpha_types = [AlphaType::Unpremul, AlphaType::Premul];
    for color_type in color_types_with_alpha {
        for alpha_type in alpha_types {
            // Set color space to nullptr because color correct premultiplies do not round trip.
            let new_info = codec
                .info()
                .with_color_type(color_type)
                .with_alpha_type(alpha_type)
                .with_color_space(None::<ColorSpace>);
            check_round_trip(r, &mut codec, &new_info);
        }
    }

    let Some(mut codec) = make_codec_from_resource(r, "images/index8.png") else {
        return;
    };

    for alpha_type in alpha_types {
        let new_info = codec
            .info()
            .with_alpha_type(alpha_type)
            .with_color_space(None::<ColorSpace>);
        check_round_trip(r, &mut codec, &new_info);
    }
});

// Port of: tests/CodecTest.cpp#L1396-L1405 (chrome/m156)
fn decode_frame(reporter: &mut Reporter, codec: &mut Codec<'_>, frame: i32) {
    let info = codec.info().with_color_type(ColorType::N32);
    let mut bm = Pixels::alloc(&info);
    let opts = Options {
        frame_index: frame,
        ..Options::default()
    };
    let result = codec.get_pixels(&info, &mut bm.data, bm.row_bytes, Some(&opts));
    reporter_assert!(reporter, result == Result::Success);
}

// For an animated GIF, we should only read enough to decode frame 0 if the
// client never calls getFrameInfo and only decodes frame 0.
// Port of: tests/CodecTest.cpp#L1407-L1440 (chrome/m156)
def_test!(Codec_skipFullParse, |r| {
    let path = "images/test640x479.gif";
    let data = skip_missing_resource!(get_resource_as_data(path), path);

    // Note that we cheat and hold on to the stream pointer, but SkCodec will take ownership. We
    // will not refer to the stream after the SkCodec deletes it.
    let stream = Arc::new(Mutex::new(*MemoryStream::make_copy(&data)));
    let Ok(mut codec) = codecs::make_codec_from_stream(Box::new(SharedStream(Arc::clone(&stream))))
    else {
        errorf!(r, "Failed to create codec for {}", path);
        return;
    };
    let position = || stream.lock().expect("stream lock").get_position();
    let length = || stream.lock().expect("stream lock").get_length();

    reporter_assert!(r, stream.lock().expect("stream lock").has_position());
    let size_position = position();
    reporter_assert!(
        r,
        stream.lock().expect("stream lock").has_length() && size_position < length()
    );

    // This should read more of the stream, but not the whole stream.
    decode_frame(r, &mut codec, 0);
    let position_after_first_frame = position();
    reporter_assert!(
        r,
        position_after_first_frame > size_position && position_after_first_frame < length()
    );

    // There is more data in the stream.
    let frame_info = codec.frame_infos();
    reporter_assert!(r, frame_info.len() == 4);
    reporter_assert!(r, position() > position_after_first_frame);
});

// Port of: tests/CodecTest.cpp#L2045-L2061 (chrome/m156)
def_test!(Codec_kBGR_101010x_XR_SkColorType_supported, |r| {
    let src_info = ImageInfo::new(
        (100, 100),
        ColorType::BGRA8888,
        AlphaType::Opaque,
        ColorSpace::new_srgb(),
    );
    let dst_info = src_info.with_color_type(ColorType::BGR101010xXR);
    let src_bm = Pixels::alloc(&src_info);
    let Some(src_pixmap) = Pixmap::new_readonly(&src_bm.info, &src_bm.data, src_bm.row_bytes)
    else {
        reporter_assert!(r, false);
        return;
    };
    let Some(data) = png_encoder::encode_pixmap(&src_pixmap, &png_encoder::Options::default())
    else {
        reporter_assert!(r, false);
        return;
    };
    let Ok(mut codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(data.as_bytes()))
    else {
        reporter_assert!(r, false);
        return;
    };
    let mut dst_bm = Pixels::alloc(&dst_info);
    let success = codec.get_pixels(&dst_info, &mut dst_bm.data, dst_bm.row_bytes, None);
    reporter_assert!(r, success == Result::Success);
});

// Port of: tests/CodecTest.cpp#L1486-L1529 (chrome/m156)
def_test!(Codec_fallBack, |r| {
    // SkAndroidCodec needs to be able to fall back to scanline decoding if incremental decoding
    // does not work. Make sure this does not require a rewind.

    // Formats that currently do not support incremental decoding.
    let files = [
        "images/CMYK.jpg",
        "images/mandrill.wbmp",
        "images/randPixels.bmp",
        "images/color_wheel.ico",
    ];
    for file in files {
        let data = skip_missing_resource!(get_resource_as_data(file), file);
        let stream = LimitedRewindingStream {
            stream: MemoryStream::make_copy(&data),
            limit: skia_rust_codec::codec::MIN_BUFFERED_BYTES_NEEDED,
            position: 0,
        };

        let Ok(mut codec) = codecs::make_codec_from_stream(Box::new(stream)) else {
            errorf!(r, "Failed to create codec for {},", file);
            continue;
        };

        let info = codec.info().with_color_type(ColorType::N32);
        let mut bm = Pixels::alloc(&info);

        let result = match codec.start_incremental_decode(&info, &mut bm.data, bm.row_bytes, None) {
            Ok(_) => Result::Success,
            Err(result) => result,
        };
        if result != Result::Unimplemented {
            errorf!(r, "Is scanline decoding now implemented for {}?", file);
            continue;
        }

        // Scanline decoding should not require a rewind.
        let result = codec.start_scanline_decode(&info, None);
        if result != Result::Success {
            errorf!(
                r,
                "Scanline decoding failed for {} with {}",
                file,
                result.as_str()
            );
        }
    }
});

// Port of: tests/CodecTest.cpp#L1885-L1896 (chrome/m156)
def_test!(Codec_ossfuzz6274, |r| {
    if resource_dir().is_none() {
        return;
    }

    let file = "invalid_images/ossfuzz6274.gif";
    if get_resource_as_image(file).is_some() {
        errorf!(r, "Invalid data gave non-nullptr image");
    }
});

// Port of: tests/CodecTest.cpp#L2139-L2161 (chrome/m156)
def_test!(Codec_gif_null_param, |r| {
    let path = "images/flightAnim.gif";
    let data = skip_missing_resource!(get_resource_as_data(path), path);

    // SkGifDecoder::Decode(stream, &result, nullptr) uses the still-image policy, which is what
    // wuffs_codec::make_from_stream selects.
    let Ok(mut codec) =
        skia_rust_codec::wuffs_codec::make_from_stream(MemoryStream::make_copy(&data))
    else {
        reporter_assert!(r, false);
        return;
    };

    match codec.get_image(None::<ImageInfo>, None::<&Options>) {
        Ok(image) => {
            reporter_assert!(r, image.width() == 320, "width {} != 320", image.width());
            reporter_assert!(r, image.height() == 240, "height {} != 240", image.height());

            // Decoding the image this way loses the original data.
            reporter_assert!(r, image.ref_encoded_data().is_none());
        }
        Err(_) => reporter_assert!(r, false),
    }
});

// Port of: tests/CodecTest.cpp#L1628-L1640 (chrome/m156)
fn test_invalid_images(reporter: &mut Reporter, path: &str, expected_result: Result) {
    let Some(data) = get_resource_as_data(path) else {
        return;
    };

    let Ok(mut codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(&data)) else {
        reporter_assert!(reporter, false);
        return;
    };

    let info = codec.info().with_color_type(ColorType::N32);
    test_info(reporter, &mut codec, &info, expected_result, None);
}

// Port of: tests/CodecTest.cpp#L1642-L1661 (chrome/m156)
def_test!(Codec_InvalidImages, |r| {
    test_invalid_images(r, "invalid_images/b33251605.bmp", Result::IncompleteInput);
    test_invalid_images(r, "invalid_images/bad_palette.png", Result::InvalidInput);
    test_invalid_images(
        r,
        "invalid_images/many-progressive-scans.jpg",
        Result::InvalidInput,
    );

    // An earlier revision of this test case passed kErrorInInput (instead of kSuccess) as the
    // third argument (expectedResult). However, after
    // https://skia-review.googlesource.com/c/skia/+/414417 `SkWuffsCodec: ignore too much pixel
    // data` combined with https://github.com/google/wuffs/commit/e44920d3 `Let gif "ignore too
    // much" quirk skip lzw errors`, the codec silently accepts skbug5887.gif (without the ASAN
    // buffer-overflow violation that lead to that test case in the first place), even though it's
    // technically an invalid GIF.
    //
    // Note that, in practice, real world GIF decoders already diverge (in different ways) from the
    // GIF specification. For compatibility, (ad hoc) implementation often trumps specification.
    // https://github.com/google/wuffs/blob/e44920d3/test/data/artificial-gif/frame-out-of-bounds.gif.make-artificial.txt#L30-L31
    test_invalid_images(r, "invalid_images/skbug5887.gif", Result::Success);
});

// Port of: tests/CodecTest.cpp#L1753-L1787 (chrome/m156)
def_test!(Codec_InvalidAnimated, |r| {
    // ASAN will complain if there is an issue.
    let path = "invalid_images/skbug6046.gif";
    let Some(data) = get_resource_as_data(path) else {
        return;
    };

    let Ok(mut codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(&data)) else {
        reporter_assert!(r, false);
        return;
    };

    let info = codec.info().with_color_type(ColorType::N32);
    let mut bm = Pixels::alloc(&info);

    let frame_infos = codec.frame_infos();
    for i in 0..frame_infos.len() {
        let index = i32::try_from(i).unwrap_or(i32::MAX);
        let req_frame = frame_infos[i].required_frame;
        let opts = Options {
            frame_index: index,
            prior_frame: if req_frame == index - 1 {
                req_frame
            } else {
                skia_rust_codec::codec::NO_FRAME
            },
            ..Options::default()
        };
        match codec.start_incremental_decode(&info, &mut bm.data, bm.row_bytes, Some(&opts)) {
            Err(result) => {
                errorf!(
                    r,
                    "Failed to start decoding frame {} (out of {}) with error {}",
                    index,
                    frame_infos.len(),
                    result.as_str()
                );
            }
            Ok(mut incremental) => {
                incremental.incremental_decode();
            }
        }
    }
});

// Neither of these calls should return a codec. Bots should catch us if we leaked anything.
// Port of: tests/CodecTest.cpp#L743-L749 (chrome/m156)
fn test_invalid_stream(reporter: &mut Reporter, stream: &[u8]) {
    reporter_assert!(
        reporter,
        codecs::make_codec_from_stream(MemoryStream::make_copy(stream)).is_err()
    );
    reporter_assert!(
        reporter,
        AndroidCodec::make_from_stream(MemoryStream::make_copy(stream)).is_none()
    );
}

// Ensure that SkCodec::NewFromStream handles freeing the passed in SkStream, even on failure. Test
// some bad streams.
// Port of: tests/CodecTest.cpp#L751-L774 (chrome/m156)
def_test!(Codec_leaks, |r| {
    // No codec should claim this as their format, so this tests SkCodec::NewFromStream. The C++
    // strings include their terminating NUL, which the sizeof() calls count.
    let non_supported_stream: &[u8] = b"hello world\0";
    // The other strings should look like the beginning of a file type, so we'll call some
    // internal version of NewFromStream, which must also delete the stream on failure.
    let empty_png: &[u8] = &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    let empty_jpeg: &[u8] = &[0xFF, 0xD8, 0xFF];
    let empty_webp: &[u8] = b"RIFF1234WEBPVP\0";
    let empty_bmp: &[u8] = b"BM";
    let empty_ico: &[u8] = &[0x00, 0x00, 0x01, 0x00];
    let empty_gif: &[u8] = b"GIFVER\0";

    test_invalid_stream(r, non_supported_stream);
    test_invalid_stream(r, empty_png);
    test_invalid_stream(r, empty_jpeg);
    test_invalid_stream(r, empty_webp);
    test_invalid_stream(r, empty_bmp);
    test_invalid_stream(r, empty_ico);
    test_invalid_stream(r, empty_gif);
});

// Port of: tests/CodecTest.cpp#L781-L801 and #L802-L817 (chrome/m156), test_dimensions
fn test_dimensions(reporter: &mut Reporter, path: &str) {
    // Create the codec from the resource file.
    let Some(data) = get_resource_as_data(path) else {
        return;
    };
    let Some(mut codec) = AndroidCodec::make_from_stream(MemoryStream::make_copy(&data)) else {
        errorf!(reporter, "Unable to create codec '{}'", path);
        return;
    };

    // Check that the decode is successful for a variety of scales.
    for sample_size in 1..32 {
        // Scale the output dimensions.
        let scaled_dims = codec.get_sampled_dimensions(sample_size);
        let scaled_info = codec
            .info()
            .with_dimensions(scaled_dims)
            .with_color_type(ColorType::N32);

        // Set up for the decode. C++: rowBytes = width * sizeof(SkPMColor), with 4-byte pixels.
        let row_bytes = usize::try_from(scaled_dims.width).unwrap_or(0) * 4;
        let mut pixels = vec![0u8; scaled_info.compute_byte_size(row_bytes)];

        let options = AndroidOptions {
            sample_size,
            ..AndroidOptions::default()
        };
        let result = codec.get_android_pixels(&scaled_info, &mut pixels, row_bytes, Some(&options));
        if result != Result::Success {
            errorf!(
                reporter,
                "Failed to decode {} with sample size {}; error: {}",
                path,
                sample_size,
                result.as_str()
            );
        }
    }
}

// Port of: tests/CodecTest.cpp#L818-L847 (chrome/m156)
def_test!(Codec_Dimensions, |r| {
    // JPG
    test_dimensions(r, "images/CMYK.jpg");
    test_dimensions(r, "images/color_wheel.jpg");
    test_dimensions(r, "images/grayscale.jpg");
    test_dimensions(r, "images/mandrill_512_q075.jpg");
    test_dimensions(r, "images/randPixels.jpg");

    // Decoding small images with very large scaling factors is a potential source of bugs and
    // crashes. We disable these tests in Gold because tiny images are not very useful to look at.
    // Here we make sure that we do not crash or access illegal memory when performing scaled
    // decodes on small images.
    test_dimensions(r, "images/1x1.png");
    test_dimensions(r, "images/2x2.png");
    test_dimensions(r, "images/3x3.png");
    test_dimensions(r, "images/3x1.png");
    test_dimensions(r, "images/1x1.png");
    test_dimensions(r, "images/16x1.png");
    test_dimensions(r, "images/1x16.png");
    test_dimensions(r, "images/mandrill_16.png");

    // RAW
    // skia-rust: not expressible in Rust: the RAW block is compiled only when SK_CODEC_DECODES_RAW
    // is set and not on Win32, and there is no RAW decoder in this port (design: codecs.md 11).
});

// Port of: tests/CodecTest.cpp#L2004-L2043 (chrome/m156)
def_test!(Codec_noConversion, |r| {
    let recs: [(&str, u32); 2] = [
        ("images/cmyk_yellow_224_224_32.jpg", 0xFFD8_FC04),
        ("images/wide_gamut_yellow_224_224_64.jpeg", 0xFFE0_E040),
    ];

    for (name, color) in recs {
        let Some(data) = get_resource_as_data(name) else {
            continue;
        };

        let Ok(mut codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(&data)) else {
            errorf!(r, "Failed to create a codec from {}", name);
            continue;
        };

        // codec->getICCProfile()
        let Some(profile) = codec.encoded_info().profile() else {
            errorf!(r, "Expected {} to have a profile", name);
            continue;
        };
        let cs = ColorSpace::make(profile);
        reporter_assert!(r, cs.is_none());

        let info = codec.info().with_color_space(None::<ColorSpace>);
        let mut bm = Pixels::alloc(&info);
        if codec.get_pixels(&info, &mut bm.data, bm.row_bytes, None) != Result::Success {
            errorf!(r, "Failed to decode {}", name);
            continue;
        }
        let Some(pixmap) = Pixmap::new_readonly(&info, &bm.data, bm.row_bytes) else {
            reporter_assert!(r, false);
            continue;
        };
        reporter_assert!(r, pixmap.get_color((0, 0)) == Color::from(color));
    }
});

// Port of: tests/CodecTest.cpp#L1789-L1801 (chrome/m156), encode_format
fn encode_format(pixmap: &Pixmap<'_>, format: EncodedImageFormat) -> Option<Data> {
    match format {
        EncodedImageFormat::PNG => {
            png_encoder::encode_pixmap(pixmap, &png_encoder::Options::default())
        }
        EncodedImageFormat::JPEG => {
            jpeg_encoder::encode_pixmap(pixmap, &jpeg_encoder::Options::default())
        }
        EncodedImageFormat::WEBP => {
            webp_encoder::encode_pixmap(pixmap, &webp_encoder::Options::default())
        }
        // SkASSERT(false) in the C++.
        _ => None,
    }
}

// Port of: tests/CodecTest.cpp#L1803-L1832 (chrome/m156), test_encode_icc
fn test_encode_icc(reporter: &mut Reporter, format: EncodedImageFormat) {
    // Test with sRGB color space.
    let srgb_info = ImageInfo::new(
        (1, 1),
        ColorType::N32,
        AlphaType::Opaque,
        None::<ColorSpace>,
    );
    let srgb_bm = Pixels::alloc(&srgb_info);
    let Some(pixmap) = Pixmap::new_readonly(&srgb_info, &srgb_bm.data, srgb_bm.row_bytes) else {
        reporter_assert!(reporter, false);
        return;
    };
    let Some(srgb_data) = encode_format(&pixmap, format) else {
        reporter_assert!(reporter, false);
        return;
    };
    let Ok(srgb_codec) =
        codecs::make_codec_from_stream(MemoryStream::make_copy(srgb_data.as_bytes()))
    else {
        reporter_assert!(reporter, false);
        return;
    };
    reporter_assert!(
        reporter,
        srgb_codec
            .info()
            .color_space()
            .is_some_and(|cs| cs.ptr_eq(&ColorSpace::new_srgb()))
    );

    // Test with P3 color space.
    let Some(p3) = ColorSpace::new_rgb(&named_transfer_fn::SRGB, &named_gamut::DISPLAY_P3) else {
        reporter_assert!(reporter, false);
        return;
    };
    let p3_info = srgb_info.with_color_space(p3.clone());
    let p3_bm = Pixels::alloc(&p3_info);
    let Some(p3_pixmap) = Pixmap::new_readonly(&p3_info, &p3_bm.data, p3_bm.row_bytes) else {
        reporter_assert!(reporter, false);
        return;
    };
    let Some(p3_data) = encode_format(&p3_pixmap, format) else {
        reporter_assert!(reporter, false);
        return;
    };
    let Ok(p3_codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(p3_data.as_bytes()))
    else {
        reporter_assert!(reporter, false);
        return;
    };
    let Some(p3_codec_space) = p3_codec.info().color_space() else {
        reporter_assert!(reporter, false);
        return;
    };
    reporter_assert!(reporter, p3_codec_space.gamma_close_to_srgb());
    // SkColorSpace::toXYZD50 always returns true (SkColorSpace.cpp#L248-L251), so the
    // `success` checks of the C++ cannot fail and are not repeated here.
    let mat0 = p3.to_xyzd50();
    let mat1 = p3_codec_space.to_xyzd50();

    for i in 0..3 {
        for j in 0..3 {
            reporter_assert!(
                reporter,
                color_space_almost_equal(mat0.vals[i][j], mat1.vals[i][j])
            );
        }
    }
}

// Port of: tests/CodecTest.cpp#L1834-L1838 (chrome/m156)
def_test!(Codec_EncodeICC, |r| {
    test_encode_icc(r, EncodedImageFormat::PNG);
    test_encode_icc(r, EncodedImageFormat::JPEG);
    test_encode_icc(r, EncodedImageFormat::WEBP);
});
