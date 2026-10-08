// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CodecTest.cpp (chrome/m156). The cases that use the PNG, BMP, ICO and WBMP
// decoders, with their `check()` helper (codec, scanline, subset, Android and image-generator
// branches), are ported here. The cases that need the JPEG, GIF, WebP or RAW decoders, or the
// PNG encoder, are ported with those decoders.

use skia_rust_codec::android_codec::{AndroidCodec, AndroidOptions};
use skia_rust_codec::codecs::{self, Decoder};
use skia_rust_codec::image_generator_from_encoded;
use skia_rust_codec::{
    Codec, Options, Result, ScanlineOrder, ZeroInitialized, decoders, png_codec,
};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::md5::{Digest, Md5};
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::random::Random;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::{MemoryStream, Stream};

use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::codec_priv::{
    ScopedCodecDecoders, ico_decoder, make_ico_from_png_resource, serial_test_lock,
};
use crate::resources::{get_resource_as_data, get_resource_as_image, resource_dir};
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
