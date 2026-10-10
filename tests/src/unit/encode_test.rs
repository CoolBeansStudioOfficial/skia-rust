// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/EncodeTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_codec::codec::Result as CodecResult;
use skia_rust_codec::codecs::make_codec_from_stream;
use skia_rust_codec::encode::png_encoder::{self, Comment, FilterFlag, Options};
use skia_rust_codec::images::deferred_from_encoded_data;
use skia_rust_codec::png_codec::make_from_stream;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::{
    Color, pm_color_get_a, pm_color_get_b, pm_color_get_g, pm_color_get_r, pre_multiply_color,
};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::convert_pixels::convert_pixels;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::stream::MemoryStream;

use skia_rust_codec::encode::jpeg_encoder::{self, AlphaOption, Downsample};
use skia_rust_raster::surfaces;
use std::sync::{Arc, Mutex};

use crate::resources::{get_resource_as_data, get_resource_as_image};
use crate::{def_test, reporter_assert};
use skia_rust_codec::encode::webp_encoder::{self as webp, Compression};
use skia_rust_libwebp::get_features;

/// Port of `PNG_KEYWORD_MAX_LENGTH` (png.h).
const PNG_KEYWORD_MAX_LENGTH: usize = 79;

/// Port of `almost_equals(const SkBitmap&, const SkBitmap&, int)` (EncodeTest.cpp#L151-L166):
/// the bitmaps must have the same info, and every 32-bit pixel must match within `tolerance`.
fn almost_equals_bitmap(a: &Bitmap, b: &Bitmap, tolerance: i32) -> bool {
    if a.info() != b.info() {
        return false;
    }
    for y in 0..a.height() {
        for x in 0..a.width() {
            let ca = a.get_color((x, y));
            let cb = b.get_color((x, y));
            let channels = [
                (i32::from(ca.r()), i32::from(cb.r())),
                (i32::from(ca.g()), i32::from(cb.g())),
                (i32::from(ca.b()), i32::from(cb.b())),
                (i32::from(ca.a()), i32::from(cb.a())),
            ];
            if channels.iter().any(|&(p, q)| (p - q).abs() > tolerance) {
                return false;
            }
        }
    }
    true
}

/// Port of `testPngComments` (EncodeTest.cpp#L403-L460): the comments are written as `tEXt`
/// chunks, with their CRCs, and a keyword longer than 79 bytes is clipped.
fn test_png_comments(
    reporter: &mut crate::Reporter,
    src: &skia_rust_core::pixmap::Pixmap<'_>,
    options: &mut Options,
) {
    let long_key = "x".repeat(PNG_KEYWORD_MAX_LENGTH);
    // The C++ test pushes the long key itself in debug builds, where the longer key would trip
    // SkDEBUGFAILF; this port's tests are debug builds too.
    options.comments = vec![
        Comment {
            keyword: "key".to_owned(),
            text: "text".to_owned(),
        },
        Comment {
            keyword: "test".to_owned(),
            text: "something".to_owned(),
        },
        Comment {
            keyword: "have some".to_owned(),
            text: "spaces in both".to_owned(),
        },
        Comment {
            keyword: long_key.clone(),
            text: String::new(),
        },
    ];

    let mut dst = Vec::new();
    let success = png_encoder::encode(src, &mut dst, options);
    reporter_assert!(reporter, success);
    let output = dst.as_slice();

    // Each chunk is of the form length (4 bytes), chunk type (tEXt), data, checksum (4 bytes).
    // Make sure we find all of them in the encoded results. The expected records include the
    // zero byte that C's `sizeof` of a string literal counts.
    let expected1: &[u8] = b"\x00\x00\x00\x08tEXtkey\x00text\x9e\xe7\x66\x51\x00";
    let expected2: &[u8] = b"\x00\x00\x00\x0etEXttest\x00something\x29\xba\xef\xac\x00";
    let expected3: &[u8] = b"\x00\x00\x00\x18tEXthave some\x00spaces in both\x8d\x69\x34\x2d\x00";
    let long_key_record = format!("tEXt{long_key}");
    let too_long_record = format!("tExt{long_key}x");
    let contains = |needle: &[u8]| output.windows(needle.len()).any(|w| w == needle);

    reporter_assert!(reporter, contains(expected1));
    reporter_assert!(reporter, contains(expected2));
    reporter_assert!(reporter, contains(expected3));
    reporter_assert!(reporter, contains(long_key_record.as_bytes()));
    reporter_assert!(reporter, !contains(too_long_record.as_bytes()));
}

// Port of: tests/EncodeTest.cpp#L465-L506 (chrome/m156)
def_test!(Encode_PngOptions, |reporter| {
    let Some(image) = get_resource_as_image("images/mandrill_128.png") else {
        return;
    };
    let Some(bitmap) = image.as_legacy_bitmap() else {
        return;
    };
    let Some(src) = bitmap.peek_pixels() else {
        reporter_assert!(reporter, false);
        return;
    };

    let mut options = Options::default();
    let mut dst0 = Vec::new();
    let success = png_encoder::encode(&src, &mut dst0, &options);
    reporter_assert!(reporter, success);

    options.filter_flags = FilterFlag::UP;
    let mut dst1 = Vec::new();
    let success = png_encoder::encode(&src, &mut dst1, &options);
    reporter_assert!(reporter, success);

    options.z_lib_level = 3;
    let mut dst2 = Vec::new();
    let success = png_encoder::encode(&src, &mut dst2, &options);
    reporter_assert!(reporter, success);

    test_png_comments(reporter, &src, &mut options);

    reporter_assert!(reporter, dst0.len() < dst1.len());
    reporter_assert!(reporter, dst1.len() < dst2.len());

    let decode = |bytes: &[u8]| -> Bitmap {
        deferred_from_encoded_data(Some(Data::new_copy(bytes)), None)
            .and_then(|img: Image| img.as_legacy_bitmap())
            .unwrap_or_default()
    };
    let bm0 = decode(&dst0);
    let bm1 = decode(&dst1);
    let bm2 = decode(&dst2);
    reporter_assert!(reporter, almost_equals_bitmap(&bm0, &bm1, 0));
    reporter_assert!(reporter, almost_equals_bitmap(&bm0, &bm2, 0));
});

/// The premultiplied channels of `c`, in R, G, B, A order (`SkPreMultiplyColor` and the
/// `SkGetPacked*32` accessors of `almost_equals`).
fn premul_channels(c: Color) -> [i32; 4] {
    let pm = pre_multiply_color(c);
    [
        i32::from(pm_color_get_r(pm)),
        i32::from(pm_color_get_g(pm)),
        i32::from(pm_color_get_b(pm)),
        i32::from(pm_color_get_a(pm)),
    ]
}

/// Port of `test_png_encoding_roundtrip_from_specific_source_format` (EncodeTest.cpp#L172-L318):
/// the colour wheel, converted to `color_type` and `alpha_type`, must survive a PNG round trip
/// within `tolerance` (on premultiplied channels).
fn test_png_encoding_roundtrip_from_specific_source_format(
    reporter: &mut crate::Reporter,
    color_type: ColorType,
    alpha_type: AlphaType,
    tolerance: i32,
) {
    // Decode the test image into `original_rgba8` (RGBA8, as the name implies).
    let resource = if alpha_type == AlphaType::Opaque {
        "images/color_wheel.jpg"
    } else {
        "images/color_wheel.png"
    };
    let Some(data) = get_resource_as_data(resource) else {
        return;
    };
    let Ok(mut codec) = make_codec_from_stream(MemoryStream::make_copy(&data)) else {
        reporter_assert!(reporter, false);
        return;
    };
    let dims = codec.dimensions();
    let rgba_alpha = if alpha_type == AlphaType::Opaque {
        AlphaType::Opaque
    } else {
        AlphaType::Unpremul
    };
    let rgba_info = ImageInfo::new(
        dims,
        ColorType::RGBA8888,
        rgba_alpha,
        Some(ColorSpace::new_srgb()),
    );
    let rgba_rb = rgba_info.min_row_bytes();
    let mut original_rgba8 = vec![0u8; rgba_info.compute_byte_size(rgba_rb)];
    let result = codec.get_pixels(&rgba_info, &mut original_rgba8, rgba_rb, None);
    reporter_assert!(reporter, result == CodecResult::Success);
    if result != CodecResult::Success {
        return;
    }

    // Transform `original_rgba8` into `original` (into the colour and alpha type under test).
    let original_info = rgba_info
        .with_color_type(color_type)
        .with_alpha_type(alpha_type);
    let original_rb = original_info.min_row_bytes();
    let mut original = vec![0u8; original_info.compute_byte_size(original_rb)];
    let success = convert_pixels(
        &original_info,
        &mut original,
        original_rb,
        &rgba_info,
        &original_rgba8,
        rgba_rb,
    );
    reporter_assert!(reporter, success);
    if !success {
        return;
    }

    // Encode `original` into `encoded_png`.
    let Some(original_pixmap) = Pixmap::new_readonly(&original_info, &original, original_rb) else {
        reporter_assert!(reporter, false);
        return;
    };
    let mut encoded_png = Vec::new();
    let success = png_encoder::encode(&original_pixmap, &mut encoded_png, &Options::default());
    reporter_assert!(reporter, success);
    if !success {
        return;
    }

    // Decode `encoded_png` into `roundtrip` (RGBA8).
    let Ok(mut codec) = make_from_stream(MemoryStream::make_copy(&encoded_png)) else {
        reporter_assert!(reporter, false);
        return;
    };
    let roundtrip_info = codec.info().with_color_type(ColorType::RGBA8888);
    let roundtrip_rb = roundtrip_info.min_row_bytes();
    let mut roundtrip = vec![0u8; roundtrip_info.compute_byte_size(roundtrip_rb)];
    let result = codec.get_pixels(&roundtrip_info, &mut roundtrip, roundtrip_rb, None);
    reporter_assert!(reporter, result == CodecResult::Success);
    if result != CodecResult::Success {
        return;
    }

    // Ensure that `original` and `roundtrip` are (almost) equal, on premultiplied channels.
    let Some(roundtrip_pixmap) = Pixmap::new_readonly(&roundtrip_info, &roundtrip, roundtrip_rb)
    else {
        reporter_assert!(reporter, false);
        return;
    };
    reporter_assert!(
        reporter,
        original_pixmap.dimensions() == roundtrip_pixmap.dimensions()
    );
    for y in 0..original_pixmap.height() {
        for x in 0..original_pixmap.width() {
            let original_premul = premul_channels(original_pixmap.get_color((x, y)));
            let roundtrip_premul = premul_channels(roundtrip_pixmap.get_color((x, y)));
            let almost_same = original_premul
                .iter()
                .zip(roundtrip_premul.iter())
                .all(|(p, q)| (p - q).abs() <= tolerance);
            reporter_assert!(reporter, almost_same);
            if !almost_same {
                return;
            }
        }
    }
}

// Port of: tests/EncodeTest.cpp#L300-L321 (chrome/m156)
def_test!(
    Encode_png_roundtrip_for_different_source_formats,
    |reporter| {
        test_png_encoding_roundtrip_from_specific_source_format(
            reporter,
            ColorType::N32,
            AlphaType::Opaque,
            0,
        );
        test_png_encoding_roundtrip_from_specific_source_format(
            reporter,
            ColorType::N32,
            AlphaType::Unpremul,
            0,
        );
        test_png_encoding_roundtrip_from_specific_source_format(
            reporter,
            ColorType::N32,
            AlphaType::Premul,
            0,
        );
        test_png_encoding_roundtrip_from_specific_source_format(
            reporter,
            ColorType::RGB565,
            AlphaType::Opaque,
            1,
        );
        // PNG encoder used to narrow down `kRGBA_F16_SkColorType` from RGBA to RGB (BE16) by skipping
        // the alpha channel via `png_set_filler`. But this wasn't done quite right for
        // `kRGBA_F32_SkColorType`, which motivated this test.
        test_png_encoding_roundtrip_from_specific_source_format(
            reporter,
            ColorType::RGBAF16,
            AlphaType::Opaque,
            0,
        );
        test_png_encoding_roundtrip_from_specific_source_format(
            reporter,
            ColorType::RGBAF16,
            AlphaType::Premul,
            1,
        );
        test_png_encoding_roundtrip_from_specific_source_format(
            reporter,
            ColorType::RGBAF32,
            AlphaType::Opaque,
            0,
        );
    }
);

/// The formats of EncodeTest.cpp's `encode` and `make` helpers (`SkEncodedImageFormat::kJPEG` and
/// `kPNG`; the WebP case is not ported).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestFormat {
    Jpeg,
    Png,
}

/// Port of `encode(SkEncodedImageFormat, const SkPixmap&)` (EncodeTest.cpp#L26-L34), with the
/// default options.
fn encode_format(format: TestFormat, src: &skia_rust_core::pixmap::Pixmap<'_>) -> Option<Data> {
    match format {
        TestFormat::Jpeg => jpeg_encoder::encode_pixmap(src, &jpeg_encoder::Options::default()),
        TestFormat::Png => png_encoder::encode_pixmap(src, &Options::default()),
    }
}

/// The encoder `make(SkEncodedImageFormat, SkWStream*, const SkPixmap&)` returns.
enum AnyEncoder<'a> {
    Jpeg(Box<jpeg_encoder::JpegEncoder<'a>>),
    Png(Box<png_encoder::PngEncoder<'a>>),
}

impl AnyEncoder<'_> {
    fn encode_rows(&mut self, num_rows: i32) -> bool {
        match self {
            AnyEncoder::Jpeg(e) => e.encode_rows(num_rows),
            AnyEncoder::Png(e) => e.encode_rows(num_rows),
        }
    }
}

/// Port of `make(SkEncodedImageFormat, SkWStream*, const SkPixmap&)` (EncodeTest.cpp#L36-L44),
/// with the output going to `out`.
fn make_encoder(
    format: TestFormat,
    out: Arc<Mutex<Vec<u8>>>,
    src: skia_rust_core::pixmap::Pixmap<'_>,
) -> Option<AnyEncoder<'_>> {
    match format {
        TestFormat::Jpeg => jpeg_encoder::make(out, src, &jpeg_encoder::Options::default())
            .map(|e| AnyEncoder::Jpeg(Box::new(e))),
        TestFormat::Png => {
            png_encoder::make(out, src, &Options::default()).map(|e| AnyEncoder::Png(Box::new(e)))
        }
    }
}

/// A read-only view of the same pixels, for an encoder that takes its source by value.
fn pixmap_view<'a>(
    src: &'a skia_rust_core::pixmap::Pixmap<'_>,
) -> Option<skia_rust_core::pixmap::Pixmap<'a>> {
    skia_rust_core::pixmap::Pixmap::new_readonly(src.info(), src.addr()?, src.row_bytes())
}

/// Port of `test_encode(skiatest::Reporter*, SkEncodedImageFormat)` (EncodeTest.cpp#L46-L84): one
/// shot, row by row, three rows at a time, and all rows at once must give the same bytes.
fn test_encode(reporter: &mut crate::Reporter, format: TestFormat) {
    let Some(image) = get_resource_as_image("images/mandrill_128.png") else {
        return;
    };
    let Some(bitmap) = image.as_legacy_bitmap() else {
        return;
    };
    let Some(src) = bitmap.peek_pixels() else {
        reporter_assert!(reporter, false);
        return;
    };

    let Some(data0) = encode_format(format, &src) else {
        reporter_assert!(reporter, false);
        return;
    };
    let height = src.info().height();

    let out1 = Arc::new(Mutex::new(Vec::new()));
    if let Some(mut encoder1) =
        pixmap_view(&src).and_then(|v| make_encoder(format, Arc::clone(&out1), v))
    {
        for _ in 0..height {
            reporter_assert!(reporter, encoder1.encode_rows(1));
        }
    } else {
        reporter_assert!(reporter, false);
    }

    let out2 = Arc::new(Mutex::new(Vec::new()));
    if let Some(mut encoder2) =
        pixmap_view(&src).and_then(|v| make_encoder(format, Arc::clone(&out2), v))
    {
        let mut i = 0;
        while i < height {
            reporter_assert!(reporter, encoder2.encode_rows(3));
            i += 3;
        }
    } else {
        reporter_assert!(reporter, false);
    }

    let out3 = Arc::new(Mutex::new(Vec::new()));
    if let Some(mut encoder3) =
        pixmap_view(&src).and_then(|v| make_encoder(format, Arc::clone(&out3), v))
    {
        reporter_assert!(reporter, encoder3.encode_rows(200));
    } else {
        reporter_assert!(reporter, false);
    }

    let want = data0.as_bytes();
    reporter_assert!(reporter, out1.lock().is_ok_and(|v| v.as_slice() == want));
    reporter_assert!(reporter, out2.lock().is_ok_and(|v| v.as_slice() == want));
    reporter_assert!(reporter, out3.lock().is_ok_and(|v| v.as_slice() == want));
}

// Port of: tests/EncodeTest.cpp#L126-L129 (chrome/m156)
def_test!(Encode, |reporter| {
    test_encode(reporter, TestFormat::Jpeg);
    test_encode(reporter, TestFormat::Png);
});

// Port of: tests/EncodeTest.cpp#L323-L358 (chrome/m156)
def_test!(Encode_JPG, |reporter| {
    let Some(image) = get_resource_as_image("images/mandrill_128.png") else {
        return;
    };
    let formats = [
        ColorType::RGBA8888,
        ColorType::BGRA8888,
        ColorType::RGB565,
        ColorType::ARGB4444,
        ColorType::Gray8,
        ColorType::RGBAF16,
    ];
    for ct in formats {
        for at in [AlphaType::Premul, AlphaType::Unpremul, AlphaType::Opaque] {
            let info = ImageInfo::new((image.width(), image.height()), ct, at, None);
            let Some(mut surface) = surfaces::raster(&info, None, None) else {
                reporter_assert!(reporter, false);
                continue;
            };
            surface.canvas().draw_image(&image, (0, 0), None);

            let mut bm = Bitmap::new();
            bm.alloc_pixels_info(&info, None);
            let Some(snapshot) = surface.image_snapshot() else {
                reporter_assert!(reporter, false);
                continue;
            };
            let Some(mut pm) = bm.peek_pixels_mut() else {
                reporter_assert!(reporter, false);
                continue;
            };
            if !snapshot.read_pixels_to_pixmap(&mut pm, (0, 0)) {
                // "failed to readPixels!"
                reporter_assert!(reporter, false);
                continue;
            }
            let Some(src) = bm.peek_pixels() else {
                reporter_assert!(reporter, false);
                continue;
            };
            for alpha_option in [AlphaOption::Ignore, AlphaOption::BlendOnBlack] {
                let opts = jpeg_encoder::Options {
                    alpha_option,
                    ..jpeg_encoder::Options::default()
                };
                if jpeg_encoder::encode_pixmap(&src, &opts).is_none() {
                    reporter_assert!(
                        reporter,
                        ct == ColorType::ARGB4444 && alpha_option == AlphaOption::BlendOnBlack
                    );
                }
            }
        }
    }
});

// Port of: tests/EncodeTest.cpp#L360-L395 (chrome/m156)
def_test!(Encode_JpegDownsample, |reporter| {
    let Some(image) = get_resource_as_image("images/mandrill_128.png") else {
        return;
    };
    let Some(bitmap) = image.as_legacy_bitmap() else {
        return;
    };
    let Some(src) = bitmap.peek_pixels() else {
        reporter_assert!(reporter, false);
        return;
    };

    let mut options = jpeg_encoder::Options::default();
    let data0 = jpeg_encoder::encode_pixmap(&src, &options);
    reporter_assert!(reporter, data0.is_some());

    options.downsample = Downsample::Horizontal;
    let data1 = jpeg_encoder::encode_pixmap(&src, &options);
    reporter_assert!(reporter, data1.is_some());

    options.downsample = Downsample::No;
    let data2 = jpeg_encoder::encode_pixmap(&src, &options);
    reporter_assert!(reporter, data2.is_some());

    let (Some(data0), Some(data1), Some(data2)) = (data0, data1, data2) else {
        return;
    };
    reporter_assert!(reporter, data0.as_bytes().len() < data1.as_bytes().len());
    reporter_assert!(reporter, data1.as_bytes().len() < data2.as_bytes().len());

    let decode = |data: Data| {
        deferred_from_encoded_data(Some(data), None).and_then(|img| img.as_legacy_bitmap())
    };
    let (Some(bm0), Some(bm1), Some(bm2)) = (decode(data0), decode(data1), decode(data2)) else {
        reporter_assert!(reporter, false);
        return;
    };
    reporter_assert!(reporter, almost_equals_bitmap(&bm0, &bm1, 60));
    reporter_assert!(reporter, almost_equals_bitmap(&bm1, &bm2, 60));
});

// Port of: tests/EncodeTest.cpp#L682-L713 (chrome/m156)
def_test!(Encode_Alpha, |reporter| {
    // These formats have no sensible way to encode alpha images.
    for format in ["jpeg", "png", "webp"] {
        for ct_index in (ColorType::Unknown as i32 + 1)..=(ColorType::LAST_ENUM as i32) {
            let Some(ct) = ColorType::from_i32(ct_index) else {
                continue;
            };
            // Non-alpha-only colortypes are tested elsewhere.
            if !skia_rust_core::image_info_priv::color_type_is_alpha_only(ct) {
                continue;
            }
            let info = ImageInfo::new((10, 10), ct, AlphaType::Premul, None);
            let mut bm = Bitmap::new();
            bm.alloc_pixels_info(&info, None);
            bm.erase_color(Color::TRANSPARENT);
            let pixmap = bm.pixmap();
            let encoded = match format {
                "jpeg" => jpeg_encoder::encode_pixmap(&pixmap, &jpeg_encoder::Options::default()),
                "png" => png_encoder::encode_pixmap(&pixmap, &Options::default()),
                _ => skia_rust_codec::encode::webp_encoder::encode_pixmap(
                    &pixmap,
                    &skia_rust_codec::encode::webp_encoder::Options::default(),
                ),
            };
            if matches!(format, "jpeg" | "png") && ct == ColorType::Alpha8 {
                // We support encoding alpha8 to png and jpeg with our own private meaning.
                reporter_assert!(reporter, encoded.is_some());
                reporter_assert!(reporter, encoded.as_ref().is_some_and(|d| d.size() > 0));
            } else {
                reporter_assert!(reporter, encoded.is_none());
            }
        }
    }
});

// Port of: tests/EncodeTest.cpp#L508-L559 (chrome/m156)
def_test!(Encode_WebpQuality, |reporter| {
    let mut bm = Bitmap::new();
    let info = ImageInfo::new((100, 100), ColorType::N32, AlphaType::Premul, None);
    bm.alloc_pixels_info(&info, None);
    bm.erase_color(Color::BLUE);
    let pixmap = bm.pixmap();

    let mut opts = webp::Options {
        compression: Compression::Lossless,
        ..webp::Options::default()
    };
    let data_lossless = webp::encode_pixmap(&pixmap, &opts);
    reporter_assert!(reporter, data_lossless.is_some());

    opts.compression = Compression::Lossy;
    opts.quality = 99.0;
    let data_lossy = webp::encode_pixmap(&pixmap, &opts);
    reporter_assert!(reporter, data_lossy.is_some());

    // `expected` is true for a lossless encode; the format is what WebPGetFeatures reports.
    let mut test = |data: &Option<Data>, expected_lossless: bool| {
        let Some(data) = data else {
            reporter_assert!(reporter, false);
            return;
        };
        let Ok(features) = get_features(data.as_bytes()) else {
            reporter_assert!(reporter, false);
            return;
        };
        reporter_assert!(reporter, features.is_lossless == expected_lossless);
    };

    test(&data_lossy, false);
    test(&data_lossless, true);
});

// Port of: tests/EncodeTest.cpp#L561-L607 (chrome/m156)
def_test!(
    #[ignore = "needs the lossy WebP encoder for pictures with transparency (ALPH: VP8L at method 3, not ported)"]
    Encode_WebpOptions,
    |reporter| {
        // ToolUtils::GetResourceAsBitmap: the test returns when the resource is not available.
        let Some(image) = get_resource_as_image("images/google_chrome.ico") else {
            return;
        };
        let Some(bitmap) = image.as_legacy_bitmap() else {
            reporter_assert!(reporter, false);
            return;
        };
        let Some(src) = bitmap.peek_pixels() else {
            reporter_assert!(reporter, false);
            return;
        };

        let mut options = webp::Options {
            compression: Compression::Lossless,
            quality: 0.0,
        };
        let data0 = webp::encode_pixmap(&src, &options);
        reporter_assert!(reporter, data0.is_some());

        options.quality = 100.0;
        let data1 = webp::encode_pixmap(&src, &options);
        reporter_assert!(reporter, data1.is_some());

        options.compression = Compression::Lossy;
        options.quality = 100.0;
        let data2 = webp::encode_pixmap(&src, &options);
        reporter_assert!(reporter, data2.is_some());

        options.compression = Compression::Lossy;
        options.quality = 50.0;
        let data3 = webp::encode_pixmap(&src, &options);
        reporter_assert!(reporter, data3.is_some());

        let (Some(data0), Some(data1), Some(data2), Some(data3)) = (data0, data1, data2, data3)
        else {
            return;
        };
        reporter_assert!(reporter, data0.size() > data1.size());
        reporter_assert!(reporter, data1.size() > data2.size());
        reporter_assert!(reporter, data2.size() > data3.size());

        let decode = |data: Data| {
            deferred_from_encoded_data(Some(data), None).and_then(|img| img.as_legacy_bitmap())
        };
        let (Some(bm0), Some(bm1), Some(bm2), Some(bm3)) =
            (decode(data0), decode(data1), decode(data2), decode(data3))
        else {
            reporter_assert!(reporter, false);
            return;
        };
        reporter_assert!(reporter, almost_equals_bitmap(&bm0, &bm1, 0));
        reporter_assert!(reporter, almost_equals_bitmap(&bm0, &bm2, 90));
        reporter_assert!(reporter, almost_equals_bitmap(&bm2, &bm3, 50));
    }
);
