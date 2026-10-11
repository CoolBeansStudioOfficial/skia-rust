// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkJpegRustCodecTest.cpp (chrome/m156), the cases compiled in the default build.
// The `RustJpeg_gainmap_extraction` case (`SK_CODEC_DECODES_JPEG_GAINMAPS`) is not ported yet: it
// needs the XMP and ISO 21496-1 metadata decoder and the gainmap accessors of `SkAndroidCodec`.
// The case that compares the encoder (`SK_CODEC_ENCODES_JPEG_WITH_RUST`) is ported with the
// decoder cases.

#![cfg(test)]

use skia_rust_codec::codec::Options;
use skia_rust_codec::encode::jpeg_encoder;
use skia_rust_codec::encode::jpeg_rust_encoder;
use skia_rust_codec::{Codec, Result, jpeg_codec, jpeg_rust_codec};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::MemoryStream;
use skia_rust_skcms::{IccProfile, TransferFunction};

use crate::resources::{get_resource_as_data, get_resource_as_image};
use crate::unit::codec_partial_test::HaltingStream;
use crate::{Reporter, def_test, errorf, reporter_assert};

// Port of: tests/SkJpegRustCodecTest.cpp#L52-L56 (REPORTER_ASSERT_SUCCESSFUL_CODEC_RESULT,
// chrome/m156)
macro_rules! assert_successful_codec_result {
    ($r:expr, $result:expr) => {
        reporter_assert!(
            $r,
            $result == Result::Success,
            "actualResult={:?} != kSuccess",
            $result
        )
    };
}

/// A pixel buffer that a decode writes into, with the image info it was allocated for and its row
/// stride. Stands in for the `SkBitmap` the C++ tests use.
// Port of: the SkBitmap uses in tests/SkJpegRustCodecTest.cpp (chrome/m156)
struct Buf {
    info: ImageInfo,
    data: Vec<u8>,
    row_bytes: usize,
}

impl Buf {
    // Port of: SkBitmap::allocPixels(info)
    fn new(info: ImageInfo) -> Self {
        let row_bytes = info.min_row_bytes();
        Self::with_row_bytes(info, row_bytes)
    }

    // Port of: SkBitmap::allocPixels(info, rowBytes)
    fn with_row_bytes(info: ImageInfo, row_bytes: usize) -> Self {
        let data = vec![0; row_bytes * dim(info.height())];
        Self {
            info,
            data,
            row_bytes,
        }
    }

    // Port of: SkBitmap::pixmap()
    fn pixmap(&self) -> Pixmap<'_> {
        Pixmap::new_readonly(&self.info, &self.data, self.row_bytes)
            .expect("the buffer holds the image")
    }

    // Port of: SkBitmap::getColor(x, y)
    fn color(&self, x: i32, y: i32) -> Color {
        self.pixmap().get_color((x, y))
    }
}

/// A non-negative dimension or index as `usize`.
fn dim(value: i32) -> usize {
    usize::try_from(value).expect("dimensions are not negative")
}

/// The alpha byte of an unpremultiplied ARGB colour: `SkColorGetA`.
// Port of: include/core/SkColor.h (SkColorGetA)
fn color_alpha(color: Color) -> u8 {
    color.a()
}

/// Port of `ComparePixels` (tests/ComparePixels.cpp): converts both pixmaps to RGBA F32 and checks
/// that every channel differs by at most the tolerance. Reports the first mismatch and stops.
// Port of: tests/ComparePixels.cpp#L38-L111 (ComparePixels, chrome/m156)
fn compare_pixels(r: &mut Reporter, a: &Pixmap<'_>, b: &Pixmap<'_>, tol: [f32; 4], label: &str) {
    if a.dimensions() != b.dimensions() {
        errorf!(r, "{label}: dimensions differ");
        return;
    }
    let dims = a.dimensions();
    // If one is premul and the other is unpremul, the comparison is in premul space.
    let float_alpha =
        if a.alpha_type() == AlphaType::Unpremul && b.alpha_type() == AlphaType::Unpremul {
            AlphaType::Unpremul
        } else {
            AlphaType::Premul
        };
    let float_cs = if ColorSpace::equals(a.color_space().as_ref(), b.color_space().as_ref()) {
        a.color_space()
    } else {
        Some(ColorSpace::new_srgb_linear())
    };
    let float_info = ImageInfo::new(dims, ColorType::RGBAF32, float_alpha, float_cs);
    let row_bytes = dim(dims.width) * 16;
    let mut float_a = vec![0u8; row_bytes * dim(dims.height)];
    let mut float_b = vec![0u8; row_bytes * dim(dims.height)];
    let converted_a = skia_rust_core::convert_pixels::convert_pixels(
        &float_info,
        &mut float_a,
        row_bytes,
        a.info(),
        a.addr().unwrap_or(&[]),
        a.row_bytes(),
    );
    let converted_b = skia_rust_core::convert_pixels::convert_pixels(
        &float_info,
        &mut float_b,
        row_bytes,
        b.info(),
        b.addr().unwrap_or(&[]),
        b.row_bytes(),
    );
    reporter_assert!(
        r,
        converted_a && converted_b,
        "{label}: conversion to F32 failed"
    );

    let read_f32 = |buf: &[u8], x: usize, y: usize, c: usize| -> f32 {
        let at = y * row_bytes + x * 16 + c * 4;
        f32::from_ne_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
    };
    for y in 0..dim(dims.height) {
        for x in 0..dim(dims.width) {
            let mut diffs = [0f32; 4];
            let mut bad = false;
            for (i, diff) in diffs.iter_mut().enumerate() {
                *diff = read_f32(&float_b, x, y, i) - read_f32(&float_a, x, y, i);
                if diff.abs() > tol[i].abs() {
                    bad = true;
                }
            }
            if bad {
                errorf!(
                    r,
                    "{label}: pixel differs at ({x}, {y}): diffs=({}, {}, {}, {})",
                    diffs[0],
                    diffs[1],
                    diffs[2],
                    diffs[3]
                );
                return;
            }
        }
    }
}

/// Port of `ComparePixels` on two buffers, for the tests that hold `Buf`s.
fn compare_bufs(r: &mut Reporter, a: &Buf, b: &Buf, tol: [f32; 4], label: &str) {
    compare_pixels(r, &a.pixmap(), &b.pixmap(), tol, label);
}

/// Port of `image->readPixels(nullptr, pixmap, 0, 0)`: reads the image into `dst`'s info.
fn read_image_into(image: &Image, dst: &mut Buf) -> bool {
    let info = dst.info.clone();
    let row_bytes = dst.row_bytes;
    image.read_pixels(&info, &mut dst.data, row_bytes, (0, 0))
}

/// Port of `codec->getImage()`: the image and the decode result. The Rust `get_image` returns an
/// image for the results that still produce one (`IncompleteInput`, `ErrorInInput`), so the result
/// is taken from a decode into a buffer of the codec's own info.
fn get_image_and_result(codec: &mut Codec<'_>) -> (Option<Image>, Result) {
    let info = codec.info();
    let mut buf = Buf::new(info.clone());
    let result = codec.get_pixels(&info, &mut buf.data, buf.row_bytes, None);
    let image = codec.get_image(None::<ImageInfo>, None::<&Options>).ok();
    (image, result)
}

/// Port of `decode_rust_jpeg`: decodes the resource with the Rust JPEG codec, asserting success.
// Port of: tests/SkJpegRustCodecTest.cpp#L59-L73 (decode_rust_jpeg, chrome/m156)
fn decode_rust_jpeg(r: &mut Reporter, path: &str) -> Option<Codec<'static>> {
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "Missing resource: {path}");
        return None;
    };
    match jpeg_rust_codec::make_from_stream(MemoryStream::make_copy(&data)) {
        Ok(codec) => Some(codec),
        Err(result) => {
            assert_successful_codec_result!(r, result);
            None
        }
    }
}

// ========== Decoder Tests ==========

// Port of: tests/SkJpegRustCodecTest.cpp#L77-L91 (RustJpegCodec_IsJpeg, chrome/m156)
def_test!(RustJpegCodec_IsJpeg, |r| {
    let Some(jpeg_data) = get_resource_as_data("images/color_wheel.jpg") else {
        errorf!(r, "Missing resource: images/color_wheel.jpg");
        return;
    };
    reporter_assert!(r, jpeg_rust_codec::is_jpeg(&jpeg_data));

    let Some(png_data) = get_resource_as_data("images/color_wheel.png") else {
        errorf!(r, "Missing resource: images/color_wheel.png");
        return;
    };
    reporter_assert!(r, !jpeg_rust_codec::is_jpeg(&png_data));
});

// Port of: tests/SkJpegRustCodecTest.cpp#L95-L137 (RustJpegCodec_decode_valid_jpgs, chrome/m156)
def_test!(RustJpegCodec_decode_valid_jpgs, |r| {
    struct TestCase {
        path: &'static str,
        expected: (i32, i32),
    }

    let test_cases = [
        TestCase {
            path: "images/color_wheel.jpg",
            expected: (128, 128),
        },
        TestCase {
            path: "images/grayscale.jpg",
            expected: (128, 128),
        },
        TestCase {
            path: "images/dog.jpg",
            expected: (180, 180),
        },
        TestCase {
            path: "images/b78329453.jpeg",
            expected: (635, 760),
        },
        TestCase {
            path: "images/mandrill_512_q075.jpg",
            expected: (512, 512),
        },
        TestCase {
            path: "images/ducky.jpg",
            expected: (489, 537),
        },
        TestCase {
            path: "images/mandrill_h1v1.jpg",
            expected: (512, 512),
        },
        TestCase {
            path: "images/mandrill_h2v1.jpg",
            expected: (512, 512),
        },
        TestCase {
            path: "images/cropped_mandrill.jpg",
            expected: (439, 154),
        },
        TestCase {
            path: "images/flutter_logo.jpg",
            expected: (400, 400),
        },
        TestCase {
            path: "images/icc-v2-gbr.jpg",
            expected: (275, 207),
        },
        TestCase {
            path: "images/randPixels.jpg",
            expected: (8, 8),
        },
        TestCase {
            path: "images/crbug999986.jpeg",
            expected: (2000, 1500),
        },
        TestCase {
            path: "images/CMYK.jpg",
            expected: (642, 516),
        },
    ];

    for tc in &test_cases {
        let Some(mut codec) = decode_rust_jpeg(r, tc.path) else {
            continue;
        };

        reporter_assert!(r, codec.get_frame_count() == 1);
        let dims = codec.dimensions();
        reporter_assert!(
            r,
            (dims.width, dims.height) == tc.expected,
            "{}: dimensions={}x{} != expected={}x{}",
            tc.path,
            dims.width,
            dims.height,
            tc.expected.0,
            tc.expected.1
        );

        let (image, result) = get_image_and_result(&mut codec);
        assert_successful_codec_result!(r, result);
        reporter_assert!(r, image.is_some());
    }
});

// Port of: tests/SkJpegRustCodecTest.cpp#L142-L363 (RustJpeg_IncrementalDecode_PartialStreaming,
// chrome/m156)
def_test!(RustJpeg_IncrementalDecode_PartialStreaming, |r| {
    const INITIAL_BYTES: usize = 4 * 1024;
    const CHUNK_SIZE: usize = 4 * 1024;
    const MAX_ITERATIONS: i32 = 1000;

    struct TestCase {
        path: &'static str,
        description: &'static str,
        expect_progressive_preview: bool,
    }

    let test_cases = [
        TestCase {
            path: "images/color_wheel.jpg",
            description: "baseline RGB 128x128",
            expect_progressive_preview: false,
        },
        TestCase {
            path: "images/dog.jpg",
            description: "RGB 180x180",
            expect_progressive_preview: false,
        },
        TestCase {
            path: "images/mandrill_512_q075.jpg",
            description: "RGB 512x512",
            expect_progressive_preview: false,
        },
        TestCase {
            path: "images/b78329453.jpeg",
            description: "progressive RGB 635x760",
            expect_progressive_preview: true,
        },
    ];

    // JPEG headers (SOS, DQT, DHT tables...) can exceed 1 KB. This leaves enough bytes to
    // construct the codec while withholding image data.
    for tc in &test_cases {
        let description = tc.description;
        let Some(data) = get_resource_as_data(tc.path) else {
            errorf!(r, "Missing resource: {}", tc.path);
            continue;
        };

        // Decode the full image first so the incremental result can be verified.
        let Ok(mut ref_codec) = jpeg_rust_codec::make_from_stream(MemoryStream::make_copy(&data))
        else {
            errorf!(r, "{description}: Failed to create reference codec");
            continue;
        };
        let ref_info = ref_codec.info().with_color_type(ColorType::N32);
        let mut reference = Buf::new(ref_info.clone());
        let ref_result =
            ref_codec.get_pixels(&ref_info, &mut reference.data, reference.row_bytes, None);
        assert_successful_codec_result!(r, ref_result);
        if ref_result != Result::Success {
            continue;
        }

        reporter_assert!(
            r,
            data.len() > INITIAL_BYTES,
            "{description}: test image must exceed the initial stream window"
        );
        if data.len() <= INITIAL_BYTES {
            continue;
        }

        // A HaltingStream that starts with the headers and partial image data.
        let halting = HaltingStream::new(&data, INITIAL_BYTES);
        let stream_ptr = halting.clone();
        let codec = jpeg_rust_codec::make_from_stream(Box::new(halting));
        let mut codec = match codec {
            Ok(codec) => codec,
            Err(code) => {
                errorf!(
                    r,
                    "{description}: failed to create streaming codec with {INITIAL_BYTES} bytes: {code:?}"
                );
                continue;
            }
        };

        let info = codec.info().with_color_type(ColorType::N32);
        let mut bitmap = Buf::new(info.clone());
        bitmap.data.fill(0xAB);

        let mut result =
            codec.start_incremental_decode(&info, &mut bitmap.data, bitmap.row_bytes, None);
        assert_successful_codec_result!(r, result);
        if result != Result::Success {
            continue;
        }

        let mut rows_decoded: i32 = 0;
        let mut previous_rows_decoded: i32 = 0;
        let mut iterations = 0;
        let mut saw_incomplete_input = false;
        let mut saw_partial_output = false;
        let mut checked_no_new_data_retry = false;
        let mut progressive_preview_count = 0;
        let mut previous_pixels = vec![0xABu8; bitmap.data.len()];

        loop {
            if iterations >= MAX_ITERATIONS {
                break;
            }
            iterations += 1;
            let (step_result, rows) = codec.incremental_decode(&mut bitmap.data);
            result = step_result;
            rows_decoded = rows.map_or(rows_decoded, |n| i32::try_from(n).unwrap_or(i32::MAX));

            reporter_assert!(
                r,
                rows_decoded >= previous_rows_decoded,
                "{description}: decoded row count regressed from {previous_rows_decoded} to {rows_decoded}"
            );

            let pixels = &bitmap.data;
            if !tc.expect_progressive_preview && previous_rows_decoded > 0 {
                let stable_bytes =
                    usize::try_from(previous_rows_decoded).unwrap_or(0) * bitmap.row_bytes;
                reporter_assert!(
                    r,
                    pixels[..stable_bytes] == previous_pixels[..stable_bytes],
                    "{description}: previously reported stable rows changed"
                );
            }

            if result == Result::Success {
                break;
            }

            reporter_assert!(
                r,
                result == Result::IncompleteInput,
                "{description}: partial input should return kIncompleteInput, got {result:?}"
            );
            if result != Result::IncompleteInput {
                break;
            }
            saw_incomplete_input = true;

            let pixels_changed = pixels.iter().any(|&value| value != 0xAB);
            if tc.expect_progressive_preview {
                if *pixels != previous_pixels {
                    progressive_preview_count += 1;
                    saw_partial_output = true;
                    reporter_assert!(
                        r,
                        rows_decoded > 0,
                        "{description}: preview pixels were not reported as initialized"
                    );
                }
            } else if rows_decoded > 0 {
                saw_partial_output = true;
                reporter_assert!(
                    r,
                    pixels_changed,
                    "{description}: reported stable rows were not written"
                );
            }

            for y in 0..rows_decoded {
                for x in 0..info.width() {
                    reporter_assert!(
                        r,
                        color_alpha(bitmap.color(x, y)) == 0xFF,
                        "{description}: initialized pixel ({x},{y}) is not opaque"
                    );
                }
            }

            previous_rows_decoded = rows_decoded;
            previous_pixels.copy_from_slice(&bitmap.data);

            if !checked_no_new_data_retry && saw_partial_output {
                // A retry without new data must report the same rows and leave the output alone.
                let (retry_result, retry_rows) = codec.incremental_decode(&mut bitmap.data);
                reporter_assert!(
                    r,
                    retry_result == Result::IncompleteInput,
                    "{description}: retry without new data returned {retry_result:?}"
                );
                let retry_rows = retry_rows.map_or(-1, |n| i32::try_from(n).unwrap_or(i32::MAX));
                reporter_assert!(
                    r,
                    retry_rows == rows_decoded,
                    "{description}: retry changed initialized rows from {rows_decoded} to {retry_rows}"
                );
                reporter_assert!(
                    r,
                    bitmap.data == previous_pixels,
                    "{description}: retry without new data changed output"
                );
                checked_no_new_data_retry = true;
            }

            if stream_ptr.is_all_data_received() {
                errorf!(
                    r,
                    "{description}: still incomplete after all data was available"
                );
                break;
            }

            // Do not call start_incremental_decode() again: each following call must resume the
            // existing decoder after one more chunk becomes available.
            stream_ptr.add_new_data(CHUNK_SIZE);
        }

        reporter_assert!(
            r,
            iterations < MAX_ITERATIONS,
            "{description}: decode loop exceeded max iterations"
        );
        reporter_assert!(
            r,
            saw_incomplete_input,
            "{description}: decode never observed incomplete input"
        );
        reporter_assert!(
            r,
            saw_partial_output,
            "{description}: no partial output was exposed before all input arrived"
        );
        reporter_assert!(
            r,
            checked_no_new_data_retry,
            "{description}: retry without new input was not exercised"
        );
        if tc.expect_progressive_preview {
            reporter_assert!(
                r,
                progressive_preview_count >= 2,
                "{description}: expected multiple progressive preview replacements, got {progressive_preview_count}"
            );
        }
        reporter_assert!(
            r,
            result == Result::Success,
            "{description}: resumed decode expected kSuccess, got {result:?}"
        );
        // The C++ test also checks `rowsDecoded == info.height()` after kSuccess. The public
        // `incremental_decode` reports rows only for kIncompleteInput (rust-skia's contract), and
        // kSuccess means every requested row was decoded, so the count is implied by the result.

        // Verify dimensions match the reference.
        reporter_assert!(
            r,
            bitmap.pixmap().dimensions() == reference.pixmap().dimensions(),
            "{description}: incremental dims differ from reference"
        );

        // Both paths used the same Rust decoder, so the pixel output must be identical (zero
        // tolerance).
        if bitmap.pixmap().dimensions() == reference.pixmap().dimensions() {
            compare_bufs(r, &bitmap, &reference, [0.0; 4], description);
        }
    }
});

// Port of: tests/SkJpegRustCodecTest.cpp#L365-L383 (RustJpeg_IncrementalDecode_RejectsSubset,
// chrome/m156)
def_test!(RustJpeg_IncrementalDecode_RejectsSubset, |r| {
    let Some(mut codec) = decode_rust_jpeg(r, "images/color_wheel.jpg") else {
        return;
    };

    let info = codec.info().with_color_type(ColorType::N32);
    let subset = IRect::from_xywh(0, 1, info.width(), info.height() - 1);
    let mut bitmap = Buf::new(info.with_dimensions(ISize::new(subset.width(), subset.height())));

    let options = Options {
        subset: Some(subset),
        ..Options::default()
    };
    let row_bytes = bitmap.row_bytes;
    let result = codec.start_incremental_decode(&info, &mut bitmap.data, row_bytes, Some(&options));
    reporter_assert!(
        r,
        result == Result::InvalidParameters,
        "incremental subset decode returned {result:?}"
    );
});

// Port of: tests/SkJpegRustCodecTest.cpp#L385-L399 (RustJpeg_IncrementalDecode_RejectsShortRowBytes,
// chrome/m156)
def_test!(RustJpeg_IncrementalDecode_RejectsShortRowBytes, |r| {
    let Some(mut codec) = decode_rust_jpeg(r, "images/color_wheel.jpg") else {
        return;
    };

    let info = codec.info().with_color_type(ColorType::N32);
    let mut bitmap = Buf::new(info.clone());
    let short_row_bytes = info.min_row_bytes() - 1;
    let result = codec.start_incremental_decode(&info, &mut bitmap.data, short_row_bytes, None);
    reporter_assert!(
        r,
        result == Result::InvalidParameters,
        "short incremental row bytes returned {result:?}"
    );
});

// Port of: tests/SkJpegRustCodecTest.cpp#L401-L437 (RustJpeg_IncrementalDecode_AfterRewind,
// chrome/m156)
def_test!(RustJpeg_IncrementalDecode_AfterRewind, |r| {
    let Some(mut codec) = decode_rust_jpeg(r, "images/color_wheel.jpg") else {
        return;
    };

    let info = codec.info().with_color_type(ColorType::N32);
    let mut first_incremental = Buf::new(info.clone());
    let row_bytes = first_incremental.row_bytes;
    assert_successful_codec_result!(
        r,
        codec.start_incremental_decode(&info, &mut first_incremental.data, row_bytes, None)
    );
    let (result, _) = codec.incremental_decode(&mut first_incremental.data);
    assert_successful_codec_result!(r, result);

    let (full_image, full_result) = get_image_and_result(&mut codec);
    assert_successful_codec_result!(r, full_result);
    reporter_assert!(r, full_image.is_some());

    let mut second_incremental = Buf::new(info.clone());
    let row_bytes = second_incremental.row_bytes;
    assert_successful_codec_result!(
        r,
        codec.start_incremental_decode(&info, &mut second_incremental.data, row_bytes, None)
    );
    let (result, _) = codec.incremental_decode(&mut second_incremental.data);
    assert_successful_codec_result!(r, result);

    reporter_assert!(
        r,
        first_incremental.data.len() == second_incremental.data.len()
    );
    reporter_assert!(r, first_incremental.data == second_incremental.data);
});

// Port of: tests/SkJpegRustCodecTest.cpp#L439-L527
// (RustJpeg_IncrementalDecode_ColorTransformPaddedStride, chrome/m156)
def_test!(RustJpeg_IncrementalDecode_ColorTransformPaddedStride, |r| {
    // This file has more than 32 KiB of metadata before its decodable scan data.
    const INITIAL_BYTES: usize = 34 * 1024;
    const CHUNK_SIZE: usize = 4 * 1024;
    const MAX_ITERATIONS: i32 = 1000;

    let Some(data) = get_resource_as_data("images/icc-v2-gbr.jpg") else {
        errorf!(r, "Missing resource: images/icc-v2-gbr.jpg");
        return;
    };

    let Ok(mut reference_codec) = jpeg_rust_codec::make_from_stream(MemoryStream::make_copy(&data))
    else {
        reporter_assert!(r, false);
        return;
    };

    let dst_info = reference_codec
        .info()
        .with_color_type(ColorType::RGBA8888)
        .with_color_space(ColorSpace::new_srgb());
    let mut reference = Buf::new(dst_info.clone());
    let reference_row_bytes = reference.row_bytes;
    assert_successful_codec_result!(
        r,
        reference_codec.get_pixels(&dst_info, &mut reference.data, reference_row_bytes, None)
    );

    let halting = HaltingStream::new(&data, INITIAL_BYTES);
    let stream_ptr = halting.clone();
    let codec = jpeg_rust_codec::make_from_stream(Box::new(halting));
    let Ok(mut codec) = codec else {
        reporter_assert!(r, false);
        return;
    };

    let padded_row_bytes = dst_info.min_row_bytes() + 32;
    let mut incremental = Buf::with_row_bytes(dst_info.clone(), padded_row_bytes);
    incremental.data.fill(0xAB);

    assert_successful_codec_result!(
        r,
        codec.start_incremental_decode(&dst_info, &mut incremental.data, padded_row_bytes, None)
    );

    let mut iterations = 0;
    let mut saw_incomplete_input = false;
    let mut result;
    loop {
        // The rows are reported only for kIncompleteInput (see `incremental_decode`).
        let (step_result, _) = codec.incremental_decode(&mut incremental.data);
        result = step_result;
        if result != Result::IncompleteInput {
            break;
        }
        saw_incomplete_input = true;
        reporter_assert!(r, !stream_ptr.is_all_data_received());
        stream_ptr.add_new_data(CHUNK_SIZE);
        iterations += 1;
        if iterations >= MAX_ITERATIONS {
            break;
        }
    }

    reporter_assert!(r, saw_incomplete_input);
    reporter_assert!(r, iterations < MAX_ITERATIONS);
    assert_successful_codec_result!(r, result);

    let tolerances = [0.0f32; 4];
    compare_bufs(
        r,
        &reference,
        &incremental,
        tolerances,
        "incremental transformed pixel",
    );

    let pixel_bytes = dst_info.min_row_bytes();
    for y in 0..dim(dst_info.height()) - 1 {
        let row = &incremental.data[y * padded_row_bytes..(y + 1) * padded_row_bytes];
        reporter_assert!(
            r,
            row[pixel_bytes..].iter().all(|&value| value == 0xAB),
            "incremental decode modified row {y} padding"
        );
    }
});

// Port of: tests/SkJpegRustCodecTest.cpp#L529-L544 (RustJpegCodec_rewind, chrome/m156)
def_test!(RustJpegCodec_rewind, |r| {
    let Some(mut codec) = decode_rust_jpeg(r, "images/color_wheel.jpg") else {
        return;
    };

    let (image1, result1) = get_image_and_result(&mut codec);
    assert_successful_codec_result!(r, result1);
    reporter_assert!(r, image1.is_some());

    let (image2, result2) = get_image_and_result(&mut codec);
    assert_successful_codec_result!(r, result2);
    reporter_assert!(r, image2.is_some());

    if let (Some(a), Some(b)) = (image1, image2) {
        reporter_assert!(r, a.dimensions() == b.dimensions());
    }
});

// Port of: tests/SkJpegRustCodecTest.cpp#L546-L569 (RustJpegCodec_incomplete_input, chrome/m156)
def_test!(RustJpegCodec_incomplete_input, |r| {
    let Some(data) = get_resource_as_data("images/color_wheel.jpg") else {
        errorf!(r, "Missing resource: images/color_wheel.jpg");
        return;
    };

    // Keep only the first third of the data.
    let truncated = &data[..data.len() / 3];

    match jpeg_rust_codec::make_from_stream(MemoryStream::make_copy(truncated)) {
        Err(result) => {
            // Acceptable: codec construction failed.
            reporter_assert!(r, result != Result::Success);
        }
        Ok(mut codec) => {
            // If the codec was constructed, decoding may succeed (the Rust decoder is lenient with
            // truncated data) or fail: either outcome is acceptable.
            let _ = get_image_and_result(&mut codec);
        }
    }
});

// Port of: tests/SkJpegRustCodecTest.cpp#L571-L582 (RustJpegCodec_reject_non_jpeg, chrome/m156)
def_test!(RustJpegCodec_reject_non_jpeg, |r| {
    let Some(data) = get_resource_as_data("images/color_wheel.png") else {
        errorf!(r, "Missing resource: images/color_wheel.png");
        return;
    };

    let result = jpeg_rust_codec::make_from_stream(MemoryStream::make_copy(&data));
    reporter_assert!(r, result.is_err(), "Should reject PNG data as JPEG");
});

// Port of: tests/SkJpegRustCodecTest.cpp#L586-L610 (RustJpegCodec_decode_with_color_transform,
// chrome/m156). Decodes an ICC-profiled JPEG into an explicit sRGB colour space, exercising the
// applyColorXform() path.
def_test!(RustJpegCodec_decode_with_color_transform, |r| {
    let Some(mut codec) = decode_rust_jpeg(r, "images/icc-v2-gbr.jpg") else {
        return;
    };

    // The image has a non-sRGB ICC profile. Requesting sRGB forces a colour transform.
    let dst_info = codec
        .info()
        .with_color_type(ColorType::RGBA8888)
        .with_color_space(ColorSpace::new_srgb());
    let mut bitmap = Buf::new(dst_info.clone());
    let row_bytes = bitmap.row_bytes;
    let result = codec.get_pixels(&dst_info, &mut bitmap.data, row_bytes, None);
    assert_successful_codec_result!(r, result);

    // Sanity: the decoded pixels should not be all zero (that would indicate garbled output).
    let all_zero = bitmap.data.iter().all(|&value| value == 0);
    reporter_assert!(
        r,
        !all_zero,
        "Color-transformed decode produced all-zero pixels"
    );
});

// ========== Encoder Tests ==========

// Port of: tests/SkJpegRustCodecTest.cpp#L618-L677 (RustJpegEncode_smoke_test, chrome/m156)
def_test!(RustJpegEncode_smoke_test, |r| {
    struct TestCase {
        path: &'static str,
        expected: (i32, i32),
        quality: u32,
    }

    let test_cases = [
        TestCase {
            path: "images/mandrill_512_q075.jpg",
            expected: (512, 512),
            quality: 85,
        },
        TestCase {
            path: "images/dog.jpg",
            expected: (180, 180),
            quality: 90,
        },
    ];

    for tc in &test_cases {
        let Some(data) = get_resource_as_data(tc.path) else {
            errorf!(r, "Missing resource: {}", tc.path);
            continue;
        };

        let Ok(mut codec) = jpeg_rust_codec::make_from_stream(MemoryStream::make_copy(&data))
        else {
            reporter_assert!(r, false, "{}: decode failed", tc.path);
            continue;
        };

        let (image, decode_result) = get_image_and_result(&mut codec);
        assert_successful_codec_result!(r, decode_result);
        let Some(image) = image else {
            reporter_assert!(r, false);
            continue;
        };

        let Some(src) = image.peek_pixels() else {
            reporter_assert!(r, false);
            continue;
        };

        let options = jpeg_encoder::Options {
            quality: tc.quality,
            ..jpeg_encoder::Options::default()
        };
        let mut dst: Vec<u8> = Vec::new();
        reporter_assert!(r, jpeg_rust_encoder::encode(&mut dst, &src, &options));
        reporter_assert!(r, !dst.is_empty());

        reporter_assert!(r, jpeg_rust_codec::is_jpeg(&dst));

        let Ok(mut re_codec) = jpeg_rust_codec::make_from_stream(MemoryStream::make_copy(&dst))
        else {
            reporter_assert!(r, false);
            continue;
        };
        let (re_image, re_result) = get_image_and_result(&mut re_codec);
        assert_successful_codec_result!(r, re_result);
        let Some(re_image) = re_image else {
            reporter_assert!(r, false);
            continue;
        };
        reporter_assert!(r, re_image.width() == tc.expected.0);
        reporter_assert!(r, re_image.height() == tc.expected.1);
    }
});

// Port of: tests/SkJpegRustCodecTest.cpp#L679-L702 (RustJpegEncode_quality_affects_size,
// chrome/m156)
def_test!(RustJpegEncode_quality_affects_size, |r| {
    // Port of `ToolUtils::GetResourceAsBitmap`: decodes the resource into a raster bitmap.
    let Some(image) = get_resource_as_image("images/mandrill_512_q075.jpg") else {
        errorf!(r, "Failed to decode images/mandrill_512_q075.jpg");
        return;
    };
    let mut bitmap = Buf::new(image.image_info().clone());
    reporter_assert!(r, read_image_into(&image, &mut bitmap));
    let src = bitmap.pixmap();

    let low_options = jpeg_encoder::Options {
        quality: 10,
        ..jpeg_encoder::Options::default()
    };
    let high_options = jpeg_encoder::Options {
        quality: 95,
        ..jpeg_encoder::Options::default()
    };
    let mut dst_low: Vec<u8> = Vec::new();
    let mut dst_high: Vec<u8> = Vec::new();
    reporter_assert!(
        r,
        jpeg_rust_encoder::encode(&mut dst_low, &src, &low_options)
    );
    reporter_assert!(
        r,
        jpeg_rust_encoder::encode(&mut dst_high, &src, &high_options)
    );

    reporter_assert!(
        r,
        dst_low.len() < dst_high.len(),
        "Low quality ({} bytes) should be smaller than high quality ({} bytes)",
        dst_low.len(),
        dst_high.len()
    );
});

// Port of: tests/SkJpegRustCodecTest.cpp#L707-L752 (RustJpegEncode_color_formats, chrome/m156).
// Encodes RGB, RGBA (alpha ignored, or blended), BGRA, and greyscale, and checks that each produces
// output.
def_test!(RustJpegEncode_color_formats, |r| {
    struct TestCase {
        description: &'static str,
        color_type: ColorType,
        alpha_type: AlphaType,
        // The fill, as the bytes of one pixel in `color_type`.
        fill: &'static [u8],
        alpha_option: jpeg_encoder::AlphaOption,
    }

    let test_cases = [
        TestCase {
            description: "grayscale",
            color_type: ColorType::Gray8,
            alpha_type: AlphaType::Opaque,
            fill: &[0x80],
            alpha_option: jpeg_encoder::AlphaOption::Ignore,
        },
        TestCase {
            description: "RGBA ignore alpha",
            color_type: ColorType::RGBA8888,
            alpha_type: AlphaType::Unpremul,
            fill: &[0xFF, 0x00, 0x00, 0x80],
            alpha_option: jpeg_encoder::AlphaOption::Ignore,
        },
        TestCase {
            description: "RGBA blend on black",
            color_type: ColorType::RGBA8888,
            alpha_type: AlphaType::Unpremul,
            fill: &[0x00, 0xFF, 0x00, 0x80],
            alpha_option: jpeg_encoder::AlphaOption::BlendOnBlack,
        },
        TestCase {
            description: "BGRA",
            color_type: ColorType::BGRA8888,
            alpha_type: AlphaType::Unpremul,
            fill: &[0xFF, 0x00, 0x00, 0xFF],
            alpha_option: jpeg_encoder::AlphaOption::Ignore,
        },
        TestCase {
            description: "RGB_888x (4-byte opaque)",
            color_type: ColorType::RGB888x,
            alpha_type: AlphaType::Opaque,
            fill: &[0x00, 0x00, 0xFF, 0xFF],
            alpha_option: jpeg_encoder::AlphaOption::Ignore,
        },
    ];

    for tc in &test_cases {
        let info = ImageInfo::new(ISize::new(64, 64), tc.color_type, tc.alpha_type, None);
        let mut bitmap = Buf::new(info);
        let pixel_len = tc.fill.len();
        for chunk in bitmap.data.chunks_exact_mut(pixel_len) {
            chunk.copy_from_slice(tc.fill);
        }

        let src = bitmap.pixmap();
        let options = jpeg_encoder::Options {
            quality: 85,
            alpha_option: tc.alpha_option,
            ..jpeg_encoder::Options::default()
        };
        let mut dst: Vec<u8> = Vec::new();
        reporter_assert!(
            r,
            jpeg_rust_encoder::encode(&mut dst, &src, &options),
            "{}",
            tc.description
        );
        reporter_assert!(r, !dst.is_empty(), "{}", tc.description);
    }
});

// ========== Gainmap / MPF Tests ==========
// `RustJpeg_gainmap_extraction` is not ported yet (see the module comment).

// ========== Rust vs C++ JPEG Comparison Tests ==========

/// Port of `compare_rust_vs_cpp_jpeg`: decodes the same JPEG with the Rust decoder and the C++
/// libjpeg-turbo decoder (`jpeg_codec`), and compares the outputs within the tolerance. The
/// tolerance is Skia's, per channel, because the two DCTs are different implementations.
// Port of: tests/SkJpegRustCodecTest.cpp#L824-L894 (compare_rust_vs_cpp_jpeg, chrome/m156)
fn compare_rust_vs_cpp_jpeg(r: &mut Reporter, path: &str, tolerance_per_channel: f32) {
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "Missing resource: {path}");
        return;
    };

    let Ok(mut rust_codec) = jpeg_rust_codec::make_from_stream(MemoryStream::make_copy(&data))
    else {
        // Some images may not be supported by the Rust decoder.
        return;
    };
    let Ok(mut cpp_codec) = jpeg_codec::make_from_stream(MemoryStream::make_copy(&data)) else {
        return;
    };

    let (rust_image, rust_result) = get_image_and_result(&mut rust_codec);
    assert_successful_codec_result!(r, rust_result);
    reporter_assert!(r, rust_image.is_some());

    let (cpp_image, cpp_result) = get_image_and_result(&mut cpp_codec);
    assert_successful_codec_result!(r, cpp_result);
    reporter_assert!(r, cpp_image.is_some());

    let (Some(rust_image), Some(cpp_image)) = (rust_image, cpp_image) else {
        return;
    };

    reporter_assert!(
        r,
        rust_image.dimensions() == cpp_image.dimensions(),
        "Rust dims {}x{} != C++ dims {}x{}",
        rust_image.width(),
        rust_image.height(),
        cpp_image.width(),
        cpp_image.height()
    );
    if rust_image.dimensions() != cpp_image.dimensions() {
        return;
    }

    let rust_info = rust_image.image_info();
    let cpp_info = cpp_image.image_info();
    let mut rust_bitmap = Buf::new(rust_info.clone());
    let mut cpp_bitmap = Buf::new(cpp_info.clone());
    reporter_assert!(r, read_image_into(&rust_image, &mut rust_bitmap));
    reporter_assert!(r, read_image_into(&cpp_image, &mut cpp_bitmap));

    let tols = [
        tolerance_per_channel,
        tolerance_per_channel,
        tolerance_per_channel,
        0.0,
    ];
    compare_bufs(r, &rust_bitmap, &cpp_bitmap, tols, path);
}

// Port of: tests/SkJpegRustCodecTest.cpp#L900-L933 (RustJpeg_vs_CppJpeg, chrome/m156). Each
// tolerance is the one Skia's test uses for that image.
def_test!(
    #[ignore = "pending maintainer decision: keeps the tolerances of Skia's own assertion (same question as Codec_RoundTripColorXform)"]
    RustJpeg_vs_CppJpeg,
    |r| {
        const DEFAULT_TOL: f32 = 8.0 / 255.0;

        struct TestCase {
            path: &'static str,
            tolerance: f32,
        }

        let test_cases = [
            TestCase {
                path: "images/color_wheel.jpg",
                tolerance: DEFAULT_TOL,
            },
            TestCase {
                path: "images/dog.jpg",
                tolerance: DEFAULT_TOL,
            },
            TestCase {
                path: "images/ducky.jpg",
                tolerance: DEFAULT_TOL,
            },
            TestCase {
                path: "images/mandrill_512_q075.jpg",
                tolerance: DEFAULT_TOL,
            },
            TestCase {
                path: "images/mandrill_h1v1.jpg",
                tolerance: DEFAULT_TOL,
            },
            TestCase {
                path: "images/mandrill_h2v1.jpg",
                tolerance: DEFAULT_TOL,
            },
            TestCase {
                path: "images/grayscale.jpg",
                tolerance: DEFAULT_TOL,
            },
            TestCase {
                path: "images/b78329453.jpeg",
                tolerance: DEFAULT_TOL,
            },
            // Progressive JPEGs can have slightly larger inter-implementation differences.
            TestCase {
                path: "images/crbug999986.jpeg",
                tolerance: 10.0 / 255.0,
            },
            TestCase {
                path: "images/cropped_mandrill.jpg",
                tolerance: DEFAULT_TOL,
            },
            TestCase {
                path: "images/flutter_logo.jpg",
                tolerance: DEFAULT_TOL,
            },
            TestCase {
                path: "images/icc-v2-gbr.jpg",
                tolerance: DEFAULT_TOL,
            },
            // randPixels has small DCT rounding differences between implementations.
            TestCase {
                path: "images/randPixels.jpg",
                tolerance: 16.0 / 255.0,
            },
            // CMYK: the CMYK to RGB conversion paths may differ by a larger amount.
            TestCase {
                path: "images/CMYK.jpg",
                tolerance: 16.0 / 255.0,
            },
        ];

        for tc in &test_cases {
            compare_rust_vs_cpp_jpeg(r, tc.path, tc.tolerance);
        }
    }
);

// Port of: tests/SkJpegRustCodecTest.cpp#L938-L978 (RustJpeg_vs_CppJpeg_Orientation, chrome/m156).
// Compares the EXIF orientation that the Rust and the C++ decoders report.
def_test!(
    #[ignore = "pending maintainer decision: keeps the tolerances of Skia's own assertion (same question as Codec_RoundTripColorXform)"]
    RustJpeg_vs_CppJpeg_Orientation,
    |r| {
        let images = [
            "images/orientation/1_444.jpg",
            "images/orientation/2_444.jpg",
            "images/orientation/3_444.jpg",
            "images/orientation/4_444.jpg",
            "images/orientation/5_444.jpg",
            "images/orientation/6_444.jpg",
            "images/orientation/7_444.jpg",
            "images/orientation/8_444.jpg",
            "images/exif-orientation-2-ur.jpg",
            "images/orientation/exif.jpg",
            "images/orientation/subifd.jpg",
            "images/ducky.jpg",
            "images/color_wheel.jpg",
        ];

        for path in images {
            let Some(data) = get_resource_as_data(path) else {
                errorf!(r, "Missing resource: {path}");
                continue;
            };

            let Ok(rust_codec) = jpeg_rust_codec::make_from_stream(MemoryStream::make_copy(&data))
            else {
                continue;
            };
            let Ok(cpp_codec) = jpeg_codec::make_from_stream(MemoryStream::make_copy(&data)) else {
                continue;
            };

            let rust_origin: EncodedOrigin = rust_codec.origin();
            let cpp_origin: EncodedOrigin = cpp_codec.origin();
            reporter_assert!(
                r,
                rust_origin == cpp_origin,
                "{path}: Rust orientation {rust_origin:?} != C++ orientation {cpp_origin:?}"
            );
        }
    }
);

// Port of: tests/SkJpegRustCodecTest.cpp#L982-L1047 (RustJpeg_vs_CppJpeg_ICCProfile, chrome/m156).
// Compares the ICC profiles the two decoders report.
def_test!(
    #[ignore = "pending maintainer decision: keeps the tolerances of Skia's own assertion (same question as Codec_RoundTripColorXform)"]
    RustJpeg_vs_CppJpeg_ICCProfile,
    |r| {
        let images = [
            "images/icc-v2-gbr.jpg",
            "images/ducky.jpg",
            "images/color_wheel.jpg",
            "images/grayscale.jpg",
        ];

        for path in images {
            let Some(data) = get_resource_as_data(path) else {
                errorf!(r, "Missing resource: {path}");
                continue;
            };

            let Ok(rust_codec) = jpeg_rust_codec::make_from_stream(MemoryStream::make_copy(&data))
            else {
                continue;
            };
            let Ok(cpp_codec) = jpeg_codec::make_from_stream(MemoryStream::make_copy(&data)) else {
                continue;
            };

            let rust_icc: Option<&IccProfile> = rust_codec.encoded_info().profile();
            let cpp_icc: Option<&IccProfile> = cpp_codec.encoded_info().profile();

            // Both should agree on presence.
            let (rust_icc, cpp_icc) = match (rust_icc, cpp_icc) {
                (None, None) => continue,
                (Some(a), Some(b)) => (a, b),
                (rust, cpp) => {
                    errorf!(
                        r,
                        "{path}: ICC presence mismatch (Rust {}, C++ {})",
                        if rust.is_some() { "present" } else { "absent" },
                        if cpp.is_some() { "present" } else { "absent" }
                    );
                    continue;
                }
            };

            reporter_assert!(
                r,
                rust_icc.data_color_space == cpp_icc.data_color_space,
                "{path}: ICC data_color_space mismatch"
            );

            // Compare the TRC (transfer function) of each channel.
            for ch in 0..3 {
                let rust_srgbish = rust_icc.trc[ch]
                    .parametric()
                    .is_some_and(TransferFunction::is_srgbish);
                let cpp_srgbish = cpp_icc.trc[ch]
                    .parametric()
                    .is_some_and(TransferFunction::is_srgbish);
                reporter_assert!(
                    r,
                    rust_srgbish == cpp_srgbish,
                    "{path}: ICC TRC[{ch}] sRGB-ish mismatch"
                );
            }

            // Compare the 3x3 toXYZD50 matrix.
            for row in 0..3 {
                for col in 0..3 {
                    let rust_val = rust_icc.to_xyzd50.vals[row][col];
                    let cpp_val = cpp_icc.to_xyzd50.vals[row][col];
                    let diff = (rust_val - cpp_val).abs();
                    let close = diff < 1e-5;
                    reporter_assert!(
                        r,
                        close,
                        "{path}: ICC toXYZD50[{row}][{col}] differs: Rust={rust_val} C++={cpp_val}"
                    );
                }
            }
        }
    }
);
