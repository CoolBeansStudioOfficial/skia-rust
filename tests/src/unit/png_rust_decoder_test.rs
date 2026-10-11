// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkPngRustDecoderTest.cpp (chrome/m156), the cases of the default build (the
// `SK_CODEC_USES_PNG_WITH_RUST_FOR_ANDROID` branches are compiled out, and so are the tests
// that exist only there). The cases that need the decode memory budget or the gainmap accessors
// of `SkAndroidCodec` are not ported yet.

#![cfg(test)]

use std::sync::{Arc, Mutex};

use skia_rust_codec::android_codec::{AndroidCodec, AndroidOptions};
use skia_rust_codec::codec::FrameInfo;
use skia_rust_codec::{Codec, Options, Result, ZeroInitialized, png_codec, png_rust_codec};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::{Color, pre_multiply_color};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::{MemoryStream, Stream};
use skia_rust_codec::codec_animation::{Blend, DisposalMethod};

use crate::unit::codec_partial_test::HaltingStream;
use crate::unit::codec_exact_read_test::SharedStream;
use crate::resources::get_resource_as_data;
use crate::{Reporter, def_test, errorf, reporter_assert};

/// A pixel buffer that a decode writes into, with the image info it was allocated for. Stands in
/// for the `SkBitmap` the C++ tests use.
// Port of: the SkBitmap uses in tests/SkPngRustDecoderTest.cpp (chrome/m156)
struct Buf {
    info: ImageInfo,
    data: Vec<u8>,
    row_bytes: usize,
}

impl Buf {
    // Port of: SkBitmap::allocPixels(info)
    fn new(info: ImageInfo) -> Self {
        let row_bytes = info.min_row_bytes();
        let data = vec![0; info.compute_byte_size(row_bytes)];
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

    // Port of: SkBitmap::eraseColor(color)
    fn erase(&mut self, color: Color) {
        let pm = pre_multiply_color(color);
        let bpp = self.info.bytes_per_pixel();
        let row_len = self.info.width() as usize * bpp;
        let mut row = vec![0u8; row_len];
        for chunk in row.chunks_exact_mut(bpp) {
            chunk.copy_from_slice(&pixel_bytes(&self.info, pm));
        }
        for y in 0..self.info.height() as usize {
            let start = y * self.row_bytes;
            self.data[start..start + row_len].copy_from_slice(&row);
        }
    }
}

/// Port of `SkCodecPriv::GetSampledDimension`: the size of a dimension sampled by `sample_size`.
// Port of: src/codec/SkCodecPriv.h (GetSampledDimension, chrome/m156), as in codec_priv.rs
fn sampled_dimension(src_dimension: i32, sample_size: i32) -> i32 {
    if sample_size > src_dimension {
        return 1;
    }
    if sample_size == 0 {
        return 0;
    }
    src_dimension / sample_size
}

/// The bytes of one pixel of `info` holding the premultiplied colour `pm`. Uses the native N32
/// layout, which is what the tests draw with.
fn pixel_bytes(info: &ImageInfo, pm: skia_rust_core::color::PMColor) -> Vec<u8> {
    let (a, r, g, b) = (
        skia_rust_core::color_priv::get_packed_a32(pm),
        skia_rust_core::color_priv::get_packed_r32(pm),
        skia_rust_core::color_priv::get_packed_g32(pm),
        skia_rust_core::color_priv::get_packed_b32(pm),
    );
    match info.color_type() {
        ColorType::Gray8 => vec![r as u8],
        ColorType::RGBA8888 => vec![r as u8, g as u8, b as u8, a as u8],
        _ => vec![b as u8, g as u8, r as u8, a as u8],
    }
}

// Port of: tests/SkPngRustDecoderTest.cpp#L27-L31 (REPORTER_ASSERT_SUCCESSFUL_CODEC_RESULT)
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

/// Port of `SkPngRustDecoderDecode`: decodes the resource with the Rust PNG codec, asserting
/// success.
// Port of: tests/SkPngRustDecoderTest.cpp#L63-L81 (SkPngRustDecoderDecode, chrome/m156)
fn png_rust_decoder_decode(r: &mut Reporter, path: &str) -> Option<Codec<'static>> {
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "Missing resource: {path}");
        return None;
    };
    match png_rust_codec::make_from_stream(MemoryStream::make_copy(&data)) {
        Ok(codec) => Some(codec),
        Err(result) => {
            assert_successful_codec_result!(r, result);
            None
        }
    }
}

/// Port of `AssertPixelColor`.
// Port of: tests/SkPngRustDecoderTest.cpp#L83-L104 (AssertPixelColor, chrome/m156)
fn assert_pixel_color(
    r: &mut Reporter,
    buf: &Buf,
    x: i32,
    y: i32,
    expected: Color,
    description: &str,
) {
    reporter_assert!(r, x < buf.info.width(), "x={x} >= width={}", buf.info.width());
    reporter_assert!(r, y < buf.info.height(), "y={y} >= height={}", buf.info.height());
    reporter_assert!(
        r,
        buf.info.color_type() == ColorType::N32,
        "kN32_SkColorType != pixmap.ColorType()={:?}",
        buf.info.color_type()
    );
    let actual = buf.color(x, y);
    reporter_assert!(
        r,
        actual == expected,
        "actualColor={actual:?} != expectedColor=={expected:?} at ({x},{y}) ({description})"
    );
}

// Port of: tests/SkPngRustDecoderTest.cpp#L106-L114 (AssertGreenPixel, chrome/m156)
fn assert_green_pixel(r: &mut Reporter, buf: &Buf, x: i32, y: i32, description: &str) {
    assert_pixel_color(r, buf, x, y, Color::GREEN, description);
}

// Port of: tests/SkPngRustDecoderTest.cpp#L116-L124 (AssertRedPixel, chrome/m156)
fn assert_red_pixel(r: &mut Reporter, buf: &Buf, x: i32, y: i32, description: &str) {
    assert_pixel_color(r, buf, x, y, Color::RED, description);
}

// Port of: tests/SkPngRustDecoderTest.cpp#L126-L134 (AssertBluePixel, chrome/m156)
fn assert_blue_pixel(r: &mut Reporter, buf: &Buf, x: i32, y: i32, description: &str) {
    assert_pixel_color(r, buf, x, y, Color::BLUE, description);
}

/// Port of `AssertSingleGreenFrame`.
// Port of: tests/SkPngRustDecoderTest.cpp#L136-L166 (AssertSingleGreenFrame, chrome/m156)
fn assert_single_green_frame(r: &mut Reporter, expected_width: i32, expected_height: i32, path: &str) {
    let Some(mut codec) = png_rust_decoder_decode(r, path) else {
        return;
    };
    reporter_assert!(r, codec.get_frame_count() == 1);
    reporter_assert!(r, codec.get_repetition_count() == 0);
    let info = codec.get_frame_info(0).expect("frame 0 exists");
    reporter_assert!(r, info.blend == Blend::Src);
    reporter_assert!(r, info.disposal_method == DisposalMethod::Keep);
    reporter_assert!(
        r,
        info.frame_rect == IRect::from_wh(expected_width, expected_height)
    );
    reporter_assert!(r, info.required_frame == skia_rust_codec::NO_FRAME);

    let image = match codec.get_image(None::<ImageInfo>, None::<&Options>) {
        Ok(image) => image,
        Err(result) => {
            assert_successful_codec_result!(r, result);
            return;
        }
    };
    reporter_assert!(
        r,
        image.width() == expected_width,
        "actualWidth={} != expectedWidth={expected_width}",
        image.width()
    );
    reporter_assert!(
        r,
        image.height() == expected_height,
        "actualHeight={} != expectedHeight={expected_height}",
        image.height()
    );
    let Some(pixmap) = image.peek_pixels() else {
        reporter_assert!(r, false);
        return;
    };
    reporter_assert!(r, pixmap.get_color((0, 0)) == Color::GREEN);
    reporter_assert!(
        r,
        pixmap.get_color((expected_width / 2, expected_height / 2)) == Color::GREEN
    );
}

/// Port of `StartIncrementalDecodeSubset`: starts an incremental decode of `subset` into `dst`.
// Port of: tests/SkPngRustDecoderTest.cpp#L168-L192 (StartIncrementalDecodeSubset, chrome/m156)
fn start_incremental_decode_subset(
    r: &mut Reporter,
    stream: Box<dyn Stream + Send + 'static>,
    subset: IRect,
    dst: &mut Buf,
) -> Option<Codec<'static>> {
    let Ok(mut codec) = png_rust_codec::make_from_stream(stream) else {
        errorf!(r, "Failed to create Rust codec");
        return None;
    };
    let subset_info = codec
        .info()
        .with_dimensions(subset.size())
        .with_color_type(ColorType::N32);
    *dst = Buf::new(subset_info);
    let full_info = codec.info().with_color_type(ColorType::N32);
    let options = Options {
        subset: Some(subset),
        ..Options::default()
    };
    let row_bytes = dst.row_bytes;
    let result =
        codec.start_incremental_decode(&full_info, &mut dst.data, row_bytes, Some(&options));
    if result != Result::Success {
        errorf!(r, "startIncrementalDecode failed with {result:?}");
        return None;
    }
    Some(codec)
}

/// Port of `DecodeSubsetOneShot`.
// Port of: tests/SkPngRustDecoderTest.cpp#L194-L211 (DecodeSubsetOneShot, chrome/m156)
fn decode_subset_one_shot(r: &mut Reporter, data: &[u8], subset: IRect, dst: &mut Buf) -> bool {
    let Some(mut codec) = start_incremental_decode_subset(
        r,
        MemoryStream::make_copy(data),
        subset,
        dst,
    ) else {
        return false;
    };
    let (result, _) = codec.incremental_decode(&mut dst.data);
    if result != Result::Success {
        errorf!(r, "incrementalDecode failed: {result:?}");
        return false;
    }
    true
}

/// Port of `DecodeSubsetHalting`.
// Port of: tests/SkPngRustDecoderTest.cpp#L213-L238 (DecodeSubsetHalting, chrome/m156)
fn decode_subset_halting(r: &mut Reporter, data: &[u8], subset: IRect, dst: &mut Buf) -> bool {
    let initial_limit = data.len() / 2;
    let halting = HaltingStream::new(data, initial_limit);
    let retained = halting.clone();
    let Some(mut codec) = start_incremental_decode_subset(r, Box::new(halting), subset, dst)
    else {
        return false;
    };
    let (result, _) = codec.incremental_decode(&mut dst.data);
    if result != Result::IncompleteInput {
        errorf!(r, "Expected kIncompleteInput, got {result:?}");
        return false;
    }
    retained.add_new_data(data.len() - initial_limit);
    let (result, _) = codec.incremental_decode(&mut dst.data);
    if result != Result::Success {
        errorf!(r, "incrementalDecode resume failed: {result:?}");
        return false;
    }
    true
}

/// Port of `CompareBitmaps`: the same image info, and the same bytes on every row.
// Port of: tests/SkPngRustDecoderTest.cpp#L240-L254 (CompareBitmaps, chrome/m156)
fn compare_bitmaps(r: &mut Reporter, bm1: &Buf, bm2: &Buf) {
    if bm1.info != bm2.info {
        errorf!(r, "Bitmaps have different image infos!");
        return;
    }
    let row_bytes = bm1.info.min_row_bytes();
    for i in 0..bm1.info.height() as usize {
        let a = &bm1.data[i * bm1.row_bytes..i * bm1.row_bytes + row_bytes];
        let b = &bm2.data[i * bm2.row_bytes..i * bm2.row_bytes + row_bytes];
        if a != b {
            errorf!(r, "Bitmaps have different pixels, starting on line {i}!");
            return;
        }
    }
}

/// A valid `IEND` chunk has an empty payload: 4-byte length (0), 4-byte type ("IEND"), 4-byte CRC.
// Port of: tests/SkPngRustDecoderTest.cpp#L256 (kIendChunkSize, chrome/m156)
const IEND_CHUNK_SIZE: usize = 12;

// Port of: tests/SkPngRustDecoderTest.cpp#L258-L261 (HasTrailingIendChunk, chrome/m156)
fn has_trailing_iend_chunk(data: &[u8]) -> bool {
    data.len() > IEND_CHUNK_SIZE
        && &data[data.len() - IEND_CHUNK_SIZE..data.len() - IEND_CHUNK_SIZE + 8] == b"\0\0\0\0IEND"
}

// Port of: tests/SkPngRustDecoderTest.cpp#L263-L269 (WithCorruptIendCrc, chrome/m156)
fn with_corrupt_iend_crc(data: &[u8]) -> Vec<u8> {
    assert!(has_trailing_iend_chunk(data));
    let mut copy = data.to_vec();
    let last = copy.len() - 1;
    copy[last] ^= 0xFF;
    copy
}

// Port of: tests/SkPngRustDecoderTest.cpp#L271-L275 (WithoutIendCrc, chrome/m156)
fn without_iend_crc(data: &[u8]) -> Vec<u8> {
    assert!(has_trailing_iend_chunk(data));
    data[..data.len() - 4].to_vec()
}

// Port of: tests/SkPngRustDecoderTest.cpp#L277-L281 (WithoutIendChunk, chrome/m156)
fn without_iend_chunk(data: &[u8]) -> Vec<u8> {
    assert!(has_trailing_iend_chunk(data));
    data[..data.len() - IEND_CHUNK_SIZE].to_vec()
}

/// Port of `DecodeAndroidPixels`: decodes through an `SkAndroidCodec` with the given sample size
/// and optional subset.
// Port of: tests/SkPngRustDecoderTest.cpp#L283-L335 (DecodeAndroidPixels, chrome/m156), the
// default branch: `max_decode_memory` is 0 in every caller that is ported.
fn decode_android_pixels(
    r: &mut Reporter,
    codec: Option<Codec<'static>>,
    sample_size: i32,
    get_subset: Option<&dyn Fn(&ImageInfo) -> IRect>,
    expected_result: Result,
) -> Option<Buf> {
    let Some(codec) = codec else {
        reporter_assert!(r, false);
        return None;
    };
    let Some(mut android) = AndroidCodec::make_from_codec(codec) else {
        reporter_assert!(r, false);
        return None;
    };
    let mut opts = AndroidOptions {
        sample_size,
        ..AndroidOptions::default()
    };
    if let Some(get_subset) = get_subset {
        opts.base.subset = Some(get_subset(&android.info()));
    }
    let sampled_dims = android.get_sampled_dimensions(sample_size);
    let mut info = android
        .info()
        .with_dimensions(sampled_dims)
        .with_color_type(ColorType::N32);
    if let Some(subset) = opts.base.subset {
        let subset_width = sampled_dimension(subset.width(), sample_size);
        let subset_height =
            sampled_dimension(subset.height(), sample_size);
        info = info.with_wh(subset_width, subset_height);
    }
    let mut bm = Buf::new(info.clone());
    let row_bytes = bm.row_bytes;
    let result = android.get_android_pixels(&info, &mut bm.data, row_bytes, Some(&opts));
    reporter_assert!(
        r,
        result == expected_result,
        "Expected {expected_result:?}, got {result:?}"
    );
    if result != Result::Success {
        return None;
    }
    Some(bm)
}

/// Port of `DecodePixels`: decodes with `SkCodec::getPixels` into `color_type` and `alpha_type`.
// Port of: tests/SkPngRustDecoderTest.cpp#L338-L366 (DecodePixels, chrome/m156)
fn decode_pixels(
    r: &mut Reporter,
    data: &[u8],
    color_type: ColorType,
    alpha_type: AlphaType,
) -> Option<Buf> {
    let mut codec = match png_rust_codec::make_from_stream(MemoryStream::make_copy(data)) {
        Ok(codec) => codec,
        Err(result) => {
            assert_successful_codec_result!(r, result);
            return None;
        }
    };
    let info = codec
        .info()
        .with_color_type(color_type)
        .with_alpha_type(alpha_type);
    let mut bm = Buf::new(info.clone());
    bm.erase(Color::MAGENTA);
    let options = Options {
        zero_initialized: ZeroInitialized::No,
        ..Options::default()
    };
    let row_bytes = bm.row_bytes;
    let result = codec.get_pixels(&info, &mut bm.data, row_bytes, Some(&options));
    reporter_assert!(
        r,
        result == Result::Success,
        "colorType={color_type:?} alphaType={alpha_type:?}: {result:?}"
    );
    if result != Result::Success {
        return None;
    }
    Some(bm)
}

/// Port of `CompareBitmaps` on the result of `DecodeAndroidPixels` for a rust and a libpng decode.
// Port of: tests/SkPngRustDecoderTest.cpp#L368-L392 (AssertAndroidDecodeSampling, the libpng
// comparison branch, chrome/m156)
fn assert_android_decode_sampling(
    r: &mut Reporter,
    path: &str,
    sample_size: i32,
    get_subset: Option<&dyn Fn(&ImageInfo) -> IRect>,
) {
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "Missing resource: {path}");
        return;
    };
    let rust_codec = png_rust_codec::make_from_stream(MemoryStream::make_copy(&data)).ok();
    let rust_bm = decode_android_pixels(r, rust_codec, sample_size, get_subset, Result::Success);
    let libpng_codec = png_codec::make_from_stream(MemoryStream::make_copy(&data)).ok();
    let libpng_bm = decode_android_pixels(r, libpng_codec, sample_size, get_subset, Result::Success);
    let (Some(rust_bm), Some(libpng_bm)) = (rust_bm, libpng_bm) else {
        return;
    };
    compare_bitmaps(r, &rust_bm, &libpng_bm);
}

/// Port of `AssertBrokenIendTailMatchesIntact`.
// Port of: tests/SkPngRustDecoderTest.cpp#L436-L470 (AssertBrokenIendTailMatchesIntact, chrome/m156)
fn assert_broken_iend_tail_matches_intact(
    r: &mut Reporter,
    path: &str,
    decode: &dyn Fn(&mut Reporter, &[u8]) -> Option<Buf>,
) {
    let Some(full_data) = get_resource_as_data(path) else {
        errorf!(r, "Missing resource: {path}");
        return;
    };
    let Some(ref_bm) = decode(r, &full_data) else {
        reporter_assert!(r, false);
        return;
    };
    let variants = [
        ("corrupt IEND CRC", with_corrupt_iend_crc(&full_data)),
        ("truncated IEND CRC", without_iend_crc(&full_data)),
    ];
    for (label, variant) in variants {
        let bm = decode(r, &variant);
        reporter_assert!(
            r,
            bm.is_some(),
            "{path}: decode failed for variant '{label}'"
        );
        if let Some(bm) = bm {
            compare_bitmaps(r, &ref_bm, &bm);
        }
    }
}

/// Port of `AssertDecodesWithBrokenIendTail`.
// Port of: tests/SkPngRustDecoderTest.cpp#L472-L490 (AssertDecodesWithBrokenIendTail, chrome/m156)
fn assert_decodes_with_broken_iend_tail(r: &mut Reporter, path: &str) {
    let configs = [
        (ColorType::N32, AlphaType::Unpremul),
        (ColorType::N32, AlphaType::Premul),
        (ColorType::RGBA8888, AlphaType::Unpremul),
    ];
    for (color_type, alpha_type) in configs {
        assert_broken_iend_tail_matches_intact(r, path, &|r, data| {
            decode_pixels(r, data, color_type, alpha_type)
        });
    }
}

/// Port of `AssertSampledDecodeWithBrokenIendTail`.
// Port of: tests/SkPngRustDecoderTest.cpp#L492-L512 (AssertSampledDecodeWithBrokenIendTail, chrome/m156)
fn assert_sampled_decode_with_broken_iend_tail(r: &mut Reporter, path: &str, sample_size: i32) {
    let decode = |r: &mut Reporter, data: &[u8]| {
        decode_android_pixels(
            r,
            png_rust_codec::make_from_stream(MemoryStream::make_copy(data)).ok(),
            sample_size,
            None,
            Result::Success,
        )
    };
    assert_broken_iend_tail_matches_intact(r, path, &decode);
    let Some(full_data) = get_resource_as_data(path) else {
        errorf!(r, "Missing resource: {path}");
        return;
    };
    let ref_bm = decode(r, &full_data);
    let without_iend_bm = decode(r, &without_iend_chunk(&full_data));
    reporter_assert!(r, without_iend_bm.is_some());
    if let (Some(ref_bm), Some(without_iend_bm)) = (ref_bm, without_iend_bm) {
        compare_bitmaps(r, &ref_bm, &without_iend_bm);
    }
}

/// Port of `DecodeLastFrame(r, codec)`: decodes every frame, keeping the last one's pixels in the
/// returned image.
// Port of: tests/SkPngRustDecoderTest.cpp#L514-L542 (DecodeLastFrame, chrome/m156)
fn decode_last_frame_of(r: &mut Reporter, codec: &mut Codec<'_>) -> Option<Buf> {
    let frame_count = codec.get_frame_count();
    let mut image: Option<Buf> = None;
    for i in 0..frame_count {
        let info: FrameInfo = match codec.get_frame_info(i) {
            Some(info) => info,
            None => {
                reporter_assert!(r, false);
                return None;
            }
        };
        assert!(info.disposal_method == DisposalMethod::Keep);
        match image.as_mut() {
            None => match codec.get_image(None::<ImageInfo>, None::<&Options>) {
                Ok(img) => {
                    let pm = img.peek_pixels().expect("image has pixels");
                    let buf = Buf {
                        info: pm.info().clone(),
                        data: pm.bytes().unwrap_or_default().to_vec(),
                        row_bytes: pm.row_bytes(),
                    };
                    image = Some(buf);
                }
                Err(result) => {
                    assert_successful_codec_result!(r, result);
                    return None;
                }
            },
            Some(buf) => {
                let options = Options {
                    zero_initialized: ZeroInitialized::No,
                    subset: None,
                    frame_index: i,
                    prior_frame: i - 1,
                };
                let info = buf.info.clone();
                let row_bytes = buf.row_bytes;
                let result = codec.get_pixels(&info, &mut buf.data, row_bytes, Some(&options));
                assert_successful_codec_result!(r, result);
                if result != Result::Success {
                    return None;
                }
            }
        }
    }
    image
}

/// Port of `DecodeLastFrame(r, resourcePath)`.
// Port of: tests/SkPngRustDecoderTest.cpp#L544-L551 (DecodeLastFrame, chrome/m156)
fn decode_last_frame(r: &mut Reporter, path: &str) -> Option<Buf> {
    let mut codec = png_rust_decoder_decode(r, path)?;
    decode_last_frame_of(r, &mut codec)
}

/// Port of `AssertAnimationRepetitionCount`.
// Port of: tests/SkPngRustDecoderTest.cpp#L553-L566 (AssertAnimationRepetitionCount, chrome/m156)
fn assert_animation_repetition_count(r: &mut Reporter, expected: i32, path: &str) {
    let Some(mut codec) = png_rust_decoder_decode(r, path) else {
        return;
    };
    let actual = codec.get_repetition_count();
    reporter_assert!(
        r,
        actual == expected,
        "actualRepetitionCount={actual} != expectedRepetitionCount={expected}"
    );
}

// Port of: tests/SkPngRustDecoderTest.cpp#L568-L570 (RustPngCodec_apng_basic_trivial_static_image)
def_test!(RustPngCodec_apng_basic_trivial_static_image, |r| {
    assert_single_green_frame(
        r,
        128,
        64,
        "images/apng-test-suite--basic--trivial-static-image.png",
    );
});

// Port of: tests/SkPngRustDecoderTest.cpp#L572-L578 (RustPngCodec_apng_basic_using_default_image)
def_test!(RustPngCodec_apng_basic_using_default_image, |r| {
    assert_single_green_frame(
        r,
        128,
        64,
        "images/apng-test-suite--basic--using-default-image.png",
    );
});

// Port of: tests/SkPngRustDecoderTest.cpp#L580-L585 (RustPngCodec_apng_basic_ignoring_default_image)
def_test!(RustPngCodec_apng_basic_ignoring_default_image, |r| {
    assert_single_green_frame(
        r,
        128,
        64,
        "images/apng-test-suite--basic--ignoring-default-image.png",
    );
});

// Port of: tests/SkPngRustDecoderTest.cpp#L587-L592 (RustPngCodec_apng_ignoring_default_image_without_getFrameCount)
def_test!(RustPngCodec_apng_ignoring_default_image_without_getFrameCount, |r| {
    let resource = "images/apng-test-suite--basic--ignoring-default-image.png";
    {
        let Some(mut codec) = png_rust_decoder_decode(r, resource) else {
            return;
        };
        match codec.get_image(None::<ImageInfo>, None::<&Options>) {
            Ok(image) => {
                let pm = image.peek_pixels().expect("image has pixels");
                reporter_assert!(r, pm.get_color((0, 0)) == Color::GREEN);
            }
            Err(result) => assert_successful_codec_result!(r, result),
        }
    }
    {
        let Some(mut codec) = png_rust_decoder_decode(r, resource) else {
            return;
        };
        let mut bitmap = Buf::new(codec.info());
        let info = bitmap.info.clone();
        let row_bytes = bitmap.row_bytes;
        let result = codec.start_incremental_decode(&info, &mut bitmap.data, row_bytes, None);
        assert_successful_codec_result!(r, result);
        if result == Result::Success {
            let (result, _) = codec.incremental_decode(&mut bitmap.data);
            assert_successful_codec_result!(r, result);
            assert_green_pixel(r, &bitmap, 0, 0, "Expecting a green pixel");
        }
    }
    {
        let Some(codec) = png_rust_decoder_decode(r, resource) else {
            return;
        };
        let Some(mut android) = AndroidCodec::make_from_codec(codec) else {
            reporter_assert!(r, false);
            return;
        };
        let sampled_dims = android.get_sampled_dimensions(2);
        let sampled_info = android.info().with_dimensions(sampled_dims);
        let mut bitmap = Buf::new(sampled_info.clone());
        let options = AndroidOptions {
            sample_size: 2,
            ..AndroidOptions::default()
        };
        let row_bytes = bitmap.row_bytes;
        let result =
            android.get_android_pixels(&sampled_info, &mut bitmap.data, row_bytes, Some(&options));
        assert_successful_codec_result!(r, result);
        if result == Result::Success {
            assert_green_pixel(r, &bitmap, 0, 0, "Expecting a green pixel");
        }
    }
});

// Port of: tests/SkPngRustDecoderTest.cpp#L594-L627 (RustPngCodec_apng_dispose_op_none_basic)
def_test!(RustPngCodec_apng_dispose_op_none_basic, |r| {
    let resource = "images/apng-test-suite--dispose-ops--none-basic.png";
    let Some(mut codec) = png_rust_decoder_decode(r, resource) else {
        return;
    };
    reporter_assert!(r, codec.get_frame_count() == 3);
    reporter_assert!(r, codec.get_repetition_count() == 0);
    let info0 = codec.get_frame_info(0).expect("frame 0");
    let info1 = codec.get_frame_info(1).expect("frame 1");
    let info2 = codec.get_frame_info(2).expect("frame 2");
    reporter_assert!(r, info0.fully_received);
    reporter_assert!(r, info1.fully_received);
    reporter_assert!(r, !info2.fully_received);
    reporter_assert!(r, info1.alpha_type == AlphaType::Unpremul);
    reporter_assert!(r, info1.blend == Blend::SrcOver);
    reporter_assert!(r, info1.disposal_method == DisposalMethod::Keep);
    reporter_assert!(r, info1.duration == 100, "dur = {}", info1.duration);
    reporter_assert!(r, info1.frame_rect == IRect::from_wh(128, 64));
    reporter_assert!(r, info1.has_alpha_within_bounds);
    reporter_assert!(r, info1.required_frame == 0);

    let image = match codec.get_image(None::<ImageInfo>, None::<&Options>) {
        Ok(image) => image,
        Err(result) => {
            assert_successful_codec_result!(r, result);
            return;
        }
    };
    reporter_assert!(r, image.width() == 128, "width {} != 128", image.width());
    reporter_assert!(r, image.height() == 64, "height {} != 64", image.height());
    let pixmap = image.peek_pixels().expect("image has pixels");
    reporter_assert!(r, pixmap.get_color((0, 0)) == Color::RED, "Frame #0 should be red");

    let mut buf = Buf {
        info: pixmap.info().clone(),
        data: pixmap.bytes().unwrap_or_default().to_vec(),
        row_bytes: pixmap.row_bytes(),
    };
    let info = buf.info.clone();
    let row_bytes = buf.row_bytes;
    // `buf` holds the first frame before each `getPixels` call.
    let options = Options {
        zero_initialized: ZeroInitialized::No,
        subset: None,
        frame_index: 1,
        prior_frame: 0,
    };
    assert_successful_codec_result!(
        r,
        codec.get_pixels(&info, &mut buf.data, row_bytes, Some(&options))
    );
    assert_green_pixel(r, &buf, 0, 0, "Frame #1 should be green");
    let options = Options {
        zero_initialized: ZeroInitialized::No,
        subset: None,
        frame_index: 2,
        prior_frame: 1,
    };
    assert_successful_codec_result!(
        r,
        codec.get_pixels(&info, &mut buf.data, row_bytes, Some(&options))
    );
    assert_green_pixel(r, &buf, 0, 0, "Frame #2 should be green");
});

// Port of: tests/SkPngRustDecoderTest.cpp#L629-L641 (RustPngCodec_apng_dispose_op_none_basic_incomplete_input1)
def_test!(RustPngCodec_apng_dispose_op_none_basic_incomplete_input1, |r| {
    let path = "images/apng-test-suite--dispose-ops--none-basic.png";
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "Missing resource: {path}");
        return;
    };
    let full_length = data.len();
    const INITIAL_BYTES: usize = 0xAD;
    let halting = HaltingStream::new(&data, INITIAL_BYTES);
    let retained = halting.clone();
    let codec = png_rust_codec::make_from_stream(Box::new(halting));
    let mut codec = match codec {
        Ok(codec) => codec,
        Err(result) => {
            assert_successful_codec_result!(r, result);
            return;
        }
    };
    let dims = codec.dimensions();
    let _bitmap = Buf::new(codec.info().with_dimensions(dims));
    let frame_count = codec.get_frame_count();
    reporter_assert!(r, frame_count == 1);
    retained.add_new_data(full_length);
    match codec.get_image(None::<ImageInfo>, None::<&Options>) {
        Ok(image) => {
            reporter_assert!(r, image.width() == 128, "width {} != 128", image.width());
            reporter_assert!(r, image.height() == 64, "height {} != 64", image.height());
            let pixmap = image.peek_pixels().expect("image has pixels");
            reporter_assert!(r, pixmap.get_color((0, 0)) == Color::RED, "Frame #0 should be red");
        }
        Err(result) => assert_successful_codec_result!(r, result),
    }
});

// Port of: tests/SkPngRustDecoderTest.cpp#L643-L690 (RustPngCodec_apng_dispose_op_none_basic_incomplete_input2)
def_test!(RustPngCodec_apng_dispose_op_none_basic_incomplete_input2, |r| {
    let path = "images/apng-test-suite--dispose-ops--none-basic.png";
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "Missing resource: {path}");
        return;
    };
    let full_length = data.len();
    // Roughly the middle of the IDAT chunk.
    const INITIAL_BYTES: usize = 0x8D;
    let halting = HaltingStream::new(&data, INITIAL_BYTES);
    let retained = halting.clone();
    let mut codec = match png_rust_codec::make_from_stream(Box::new(halting)) {
        Ok(codec) => codec,
        Err(result) => {
            assert_successful_codec_result!(r, result);
            return;
        }
    };
    let dims = codec.dimensions();
    let mut bitmap = Buf::new(codec.info().with_dimensions(dims));
    bitmap.erase(Color::BLUE);
    assert_blue_pixel(r, &bitmap, 0, 0, "");
    assert_blue_pixel(r, &bitmap, 40, 29, "");
    assert_blue_pixel(r, &bitmap, 80, 35, "");
    assert_blue_pixel(r, &bitmap, 127, 63, "");
    let options = Options {
        zero_initialized: ZeroInitialized::No,
        subset: None,
        frame_index: 0,
        prior_frame: skia_rust_codec::NO_FRAME,
    };
    let info = bitmap.info.clone();
    let row_bytes = bitmap.row_bytes;
    let result = codec.start_incremental_decode(&info, &mut bitmap.data, row_bytes, Some(&options));
    reporter_assert!(r, result == Result::Success);
    let (result, rows_decoded) = codec.incremental_decode(&mut bitmap.data);
    reporter_assert!(r, result == Result::IncompleteInput);
    reporter_assert!(r, rows_decoded == Some(10), "actual rowsDecoded = {rows_decoded:?}");
    assert_red_pixel(r, &bitmap, 0, 0, "");
    assert_blue_pixel(r, &bitmap, 40, 29, "");
    assert_blue_pixel(r, &bitmap, 80, 35, "");
    assert_blue_pixel(r, &bitmap, 127, 63, "");
    retained.add_new_data(full_length);
    let frame_count = codec.get_frame_count();
    reporter_assert!(r, frame_count == 1);
    let (result, rows_decoded) = codec.incremental_decode(&mut bitmap.data);
    reporter_assert!(r, result == Result::Success);
    // Not set when `kSuccess`.
    reporter_assert!(r, rows_decoded.is_none());
    assert_red_pixel(r, &bitmap, 0, 0, "");
    assert_red_pixel(r, &bitmap, 40, 29, "");
    assert_red_pixel(r, &bitmap, 80, 35, "");
    assert_red_pixel(r, &bitmap, 127, 63, "");
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1002-L1027 (RustPngCodec_apng_blend_ops_source_on_solid)
def_test!(RustPngCodec_apng_blend_ops_source_on_solid, |r| {
    let resource = "images/apng-test-suite--blend-ops--source-on-solid.png";
    let Some(mut codec) = png_rust_decoder_decode(r, resource) else {
        return;
    };
    let Some(buf) = decode_last_frame_of(r, &mut codec) else {
        return;
    };
    assert_green_pixel(r, &buf, 0, 0, "");
    let info = codec.get_frame_info(1).expect("frame 1");
    reporter_assert!(r, info.blend == Blend::Src);
    reporter_assert!(r, info.required_frame == skia_rust_codec::NO_FRAME);
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1029-L1050 (RustPngCodec_apng_blend_ops_source_on_nearly_transparent)
def_test!(RustPngCodec_apng_blend_ops_source_on_nearly_transparent, |r| {
    let resource = "images/apng-test-suite--blend-ops--source-on-nearly-transparent.png";
    let Some(buf) = decode_last_frame(r, resource) else {
        return;
    };
    assert_pixel_color(
        r,
        &buf,
        0,
        0,
        Color::from(0x0200_FF00),
        "Expecting a nearly transparent pixel",
    );
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1052-L1068 (RustPngCodec_apng_blend_ops_over_on_solid_and_transparent)
def_test!(RustPngCodec_apng_blend_ops_over_on_solid_and_transparent, |r| {
    let resource = "images/apng-test-suite--blend-ops--over-on-solid-and-transparent.png";
    let Some(buf) = decode_last_frame(r, resource) else {
        return;
    };
    assert_green_pixel(r, &buf, 0, 0, "");
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1070-L1086 (RustPngCodec_apng_blend_ops_over_repeatedly)
def_test!(RustPngCodec_apng_blend_ops_over_repeatedly, |r| {
    let resource = "images/apng-test-suite--blend-ops--over-repeatedly.png";
    let Some(buf) = decode_last_frame(r, resource) else {
        return;
    };
    assert_green_pixel(r, &buf, 0, 0, "");
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1088-L1113 (RustPngCodec_apng_regions_dispose_op_none)
def_test!(RustPngCodec_apng_regions_dispose_op_none, |r| {
    let resource = "images/apng-test-suite--regions--dispose-op-none.png";
    let Some(buf) = decode_last_frame(r, resource) else {
        return;
    };
    for y in 0..buf.info.height() {
        for x in 0..buf.info.width() {
            assert_green_pixel(r, &buf, x, y, "");
        }
    }
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1115-L1124 (RustPngCodec_apng_num_plays_0)
def_test!(RustPngCodec_apng_num_plays_0, |r| {
    assert_animation_repetition_count(
        r,
        skia_rust_codec::codec::REPETITION_COUNT_INFINITE,
        "images/apng-test-suite--num-plays--0.png",
    );
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1126-L1135 (RustPngCodec_apng_num_plays_1)
def_test!(RustPngCodec_apng_num_plays_1, |r| {
    assert_animation_repetition_count(r, 0, "images/apng-test-suite--num-plays--1.png");
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1137-L1146 (RustPngCodec_apng_num_plays_2)
def_test!(RustPngCodec_apng_num_plays_2, |r| {
    assert_animation_repetition_count(r, 1, "images/apng-test-suite--num-plays--2.png");
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1154-L1167 (RustPngCodec_apng_invalid_num_frames_outside_valid_range)
def_test!(RustPngCodec_apng_invalid_num_frames_outside_valid_range, |r| {
    let Some(mut codec) = png_rust_decoder_decode(
        r,
        "images/apng-test-suite--invalid--num-frames-outside-valid-range.png",
    ) else {
        return;
    };
    reporter_assert!(r, codec.get_frame_count() == 1);
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1169-L1187 (RustPngCodec_png_swizzling_target_unimplemented)
def_test!(RustPngCodec_png_swizzling_target_unimplemented, |r| {
    let Some(mut codec) = png_rust_decoder_decode(
        r,
        "images/apng-test-suite--basic--ignoring-default-image.png",
    ) else {
        return;
    };
    reporter_assert!(r, codec.get_frame_count() == 1);
    let dst_info = ImageInfo::new(
        codec.dimensions(),
        ColorType::BGRA10101010XR,
        AlphaType::Premul,
        None,
    );
    let result = codec.get_image(dst_info, None::<&Options>);
    match result {
        Err(result) => reporter_assert!(r, result == Result::Unimplemented),
        Ok(_) => reporter_assert!(r, false),
    }
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1189-L1206 (RustPngCodec_png_was_encoded_with_16_bits_or_more_per_component)
def_test!(RustPngCodec_png_was_encoded_with_16_bits_or_more_per_component, |r| {
    let tests = [
        // 4 bit (16 level) grayscale
        ("images/pngsuite/basn0g04.png", false),
        // 3x8 bits rgb color
        ("images/pngsuite/basn2c08.png", false),
        // 3x16 bits rgb color
        ("images/pngsuite/basn2c16.png", true),
        // 1 bit (2 color) paletted
        ("images/pngsuite/basn3p01.png", false),
    ];
    for (filename, encoded_with_16_bits) in tests {
        if let Some(codec) = png_rust_decoder_decode(r, filename) {
            reporter_assert!(
                r,
                codec.has_high_bit_depth_encoded_data() == encoded_with_16_bits
            );
        }
    }
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1208-L1225 (RustPngCodec_png_cicp)
def_test!(RustPngCodec_png_cicp, |r| {
    let Some(codec) = png_rust_decoder_decode(r, "images/cicp_pq.png") else {
        return;
    };
    let Some(profile) = codec.encoded_info().profile() else {
        reporter_assert!(r, false);
        return;
    };
    let cs = skia_rust_core::color_space::ColorSpace::make(profile);
    let tf = cs.map(|cs| cs.transfer_fn());
    reporter_assert!(
        r,
        tf.is_some_and(|tf| tf.is_pqish()
            || tf.is_pq())
    );
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1227-L1245 (RustPngCodec_green15x15)
def_test!(RustPngCodec_green15x15, |r| {
    let Some(mut codec) = png_rust_decoder_decode(r, "images/green15x15.png") else {
        return;
    };
    let dst_info = codec
        .info()
        .with_color_space(Some(skia_rust_core::color_space::ColorSpace::new_srgb()));
    let image = match codec.get_image(dst_info, None::<&Options>) {
        Ok(image) => image,
        Err(result) => {
            assert_successful_codec_result!(r, result);
            return;
        }
    };
    let pixmap = image.peek_pixels().expect("image has pixels");
    let expected_color = Color::from(0xFF00_8000);
    reporter_assert!(
        r,
        pixmap.get_color((0, 0)) == expected_color,
        "Expecting a dark green pixel"
    );
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1247-L1254 (RustPngCodec_exif_orientation)
def_test!(RustPngCodec_exif_orientation, |r| {
    let Some(codec) = png_rust_decoder_decode(r, "images/F-exif-chunk-early.png") else {
        return;
    };
    reporter_assert!(r, codec.origin() == EncodedOrigin::RightTop);
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1256-L1269 (RustPngCodec_f16_trc_tables)
def_test!(RustPngCodec_f16_trc_tables, |r| {
    let Some(mut codec) = png_rust_decoder_decode(r, "images/f16-trc-tables.png") else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec.info();
    reporter_assert!(r, info.color_space().is_some());
    let dst_info = info
        .with_color_type(ColorType::RGBAF16)
        .with_color_space(None);
    let result = codec.get_image(dst_info, None::<&Options>);
    assert_successful_codec_result!(r, result.as_ref().err().copied().unwrap_or(Result::Success));
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1271-L1286 (RustPngCodec_crbug445556737)
def_test!(RustPngCodec_crbug445556737, |r| {
    let Some(buf) = decode_last_frame(r, "images/crbug445556737.png") else {
        return;
    };
    reporter_assert!(r, buf.info.height() == 5);
    reporter_assert!(r, buf.info.width() == 5);
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1288-L1298 (RustPngCodec_invalid_profile)
def_test!(RustPngCodec_invalid_profile, |r| {
    let Some(mut codec) = png_rust_decoder_decode(r, "images/png-zero-gamma-color-profile.png")
    else {
        reporter_assert!(r, false);
        return;
    };
    // There should be no ICC profile.
    reporter_assert!(r, codec.encoded_info().profile().is_none());
    // This should not crash.
    let info = codec.info();
    let result = codec.get_image(info, None::<&Options>);
    assert_successful_codec_result!(r, result.as_ref().err().copied().unwrap_or(Result::Success));
});

/// Port of `test_subset_decode`: decodes the image in tiles, each through a subset, and compares
/// the tiles against the whole image.
// Port of: tests/SkPngRustDecoderTest.cpp#L1300-L1357 (test_subset_decode, chrome/m156)
fn test_subset_decode(r: &mut Reporter, resource: &str) {
    let Some(data) = get_resource_as_data(resource) else {
        reporter_assert!(r, false);
        return;
    };
    let Ok(mut codec) = codecs_make_codec_from_stream(MemoryStream::make_copy(&data)) else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec.info();
    let mut bm = Buf::new(info.clone());
    let row_bytes = bm.row_bytes;
    let _ = codec.get_pixels(&info, &mut bm.data, row_bytes, None);
    let mut tiled = Buf::new(info.clone());

    let height = info.height();
    let width = info.width();
    // If numStripes does not evenly divide height there will be an extra stripe.
    let num_stripes = 4;
    let num_vertical_stripes = 2;
    if num_stripes > height || num_vertical_stripes > width {
        // Image is too small.
        return;
    }
    let stripe_height = height / num_stripes;
    let stripe_width = width / num_vertical_stripes;
    // Iterate through the image twice: once for the odd stripes, and once for the even ones.
    for odd_even in [1, 0] {
        let mut y = odd_even * stripe_height;
        while y < height {
            for x_stripe in 0..num_vertical_stripes {
                let top = y;
                let bottom = (y + stripe_height).min(height);
                let left = x_stripe * stripe_width;
                let right = ((x_stripe + 1) * stripe_width).min(width);
                let subset = IRect::from_ltrb(left, top, right, bottom);
                let options = Options {
                    subset: Some(subset),
                    ..Options::default()
                };
                // Each tile is decoded into a tightly allocated buffer, sized exactly to the subset.
                let mut subset_bm = Buf::new(info.with_dimensions(subset.size()));
                let sub_info = subset_bm.info.clone();
                let sub_row_bytes = subset_bm.row_bytes;
                reporter_assert!(
                    r,
                    Result::Success
                        == codec.start_incremental_decode(
                            &info,
                            &mut subset_bm.data,
                            sub_row_bytes,
                            Some(&options),
                        )
                );
                let _ = sub_info;
                let (result, _) = codec.incremental_decode(&mut subset_bm.data);
                reporter_assert!(r, Result::Success == result);
                // Copy the decoded tile into the full tiled buffer (same image info).
                copy_tile(&subset_bm, &mut tiled, left, top);
            }
            y += 2 * stripe_height;
        }
    }
    compare_bitmaps_by_color(r, &bm, &tiled);
}

/// `SkCodec::MakeFromStream` (the registry), as the tests reach it.
fn codecs_make_codec_from_stream(
    stream: Box<MemoryStream>,
) -> std::result::Result<Codec<'static>, Result> {
    skia_rust_codec::codecs::make_codec_from_stream(stream)
}

/// `SkBitmap::writePixels` for a tile of the same colour type: copies the tile's rows into `dst`
/// at (`x`, `y`).
fn copy_tile(src: &Buf, dst: &mut Buf, x: i32, y: i32) {
    let bpp = src.info.bytes_per_pixel();
    let row_len = src.info.width() as usize * bpp;
    for row in 0..src.info.height() as usize {
        let s = row * src.row_bytes;
        let d = (y as usize + row) * dst.row_bytes + x as usize * bpp;
        dst.data[d..d + row_len].copy_from_slice(&src.data[s..s + row_len]);
    }
}

/// Port of `bitmaps_equal`: the premultiplied colours of two bitmaps agree everywhere.
// Port of: tests/SkPngRustDecoderTest.cpp#L1307-L1320 (bitmaps_equal, chrome/m156)
fn compare_bitmaps_by_color(r: &mut Reporter, actual: &Buf, expected: &Buf) {
    let equal = (0..actual.info.height()).all(|y| {
        (0..actual.info.width()).all(|x| {
            pre_multiply_color(actual.color(x, y)) == pre_multiply_color(expected.color(x, y))
        })
    });
    reporter_assert!(r, equal);
}

// Port of: tests/SkPngRustDecoderTest.cpp#L1322-L1331 (RustPngCodec_subset)
def_test!(RustPngCodec_subset, |r| {
    // Tests subsets by splitting the image into 8 or 10 different tiles and decoding those each
    // separately, then comparing to the full image decoded.
    test_subset_decode(r, "images/baby_tux.png");
    test_subset_decode(r, "images/plane_interlaced.png");
    test_subset_decode(r, "images/basi3p01.png");
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1700-L1743 (RustPngCodec_exactRead_overshoot, the
// default branch: the decoder reads the stream in sequence, one image after another)
def_test!(RustPngCodec_exactRead_overshoot, |r| {
    let path = "images/plane.png";
    let Some(data) = get_resource_as_data(path) else {
        return;
    };
    const NUM_IMAGES: usize = 2;
    let size = data.len();
    let mut multi_data = Vec::with_capacity(size * NUM_IMAGES);
    for _ in 0..NUM_IMAGES {
        multi_data.extend_from_slice(&data);
    }
    drop(data);
    let stream = Arc::new(Mutex::new(*MemoryStream::make_copy(&multi_data)));
    for i in 0..NUM_IMAGES {
        let codec = png_rust_codec::make_from_stream(Box::new(SharedStream(Arc::clone(&stream))));
        if i == 0 {
            let mut codec = match codec {
                Ok(codec) => codec,
                Err(_) => {
                    errorf!(r, "Failed to create a codec from {path}, iteration {i}");
                    return;
                }
            };
            let info = codec.info().with_color_type(ColorType::N32);
            let mut bm = Buf::new(info.clone());
            let row_bytes = bm.row_bytes;
            let result = codec.get_pixels(&info, &mut bm.data, row_bytes, None);
            if result != Result::Success {
                errorf!(r, "Failed to getPixels from {path}, iteration {i} error {result:?}");
                return;
            }
        } else {
            reporter_assert!(r, codec.is_err());
        }
    }
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1811-L1849 (RustPngCodec_apng_offset_frame_does_not_overflow_dst)
def_test!(RustPngCodec_apng_offset_frame_does_not_overflow_dst, |r| {
    let path = "images/apng-single-frame-with-offset.png";
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "Missing resource: {path}");
        return;
    };
    // (sample size, use a subset, custom row bytes)
    let cases = [
        (2, false, 0),
        (64, false, 0),
        (2, false, 256),
        (1, true, 0),
        (2, true, 0),
    ];
    for (sample_size, use_subset, row_bytes) in cases {
        assert_android_decode_refused(r, &data, sample_size, use_subset, row_bytes);
    }
});

/// Port of `AssertAndroidDecodeRefused`: the decode is refused with `kUnimplemented`, and writes
/// nothing outside the destination.
// Port of: tests/SkPngRustDecoderTest.cpp#L394-L434 (AssertAndroidDecodeRefused, chrome/m156)
fn assert_android_decode_refused(
    r: &mut Reporter,
    data: &[u8],
    sample_size: i32,
    use_subset: bool,
    custom_row_bytes: usize,
) {
    const GUARD_BYTES: usize = 64 * 1024;
    const GUARD_VALUE: u8 = 0x5A;
    let Ok(codec) = png_rust_codec::make_from_stream(MemoryStream::make_copy(data)) else {
        reporter_assert!(r, false);
        return;
    };
    let Some(mut android) = AndroidCodec::make_from_codec(codec) else {
        reporter_assert!(r, false);
        return;
    };
    let full_dims = android.info().dimensions();
    let mut subset = IRect::from_wh(full_dims.width, 1.max(full_dims.height / 2));
    if use_subset {
        reporter_assert!(r, android.get_supported_subset(&mut subset));
    }
    let dims = if use_subset {
        ISize::from((
            sampled_dimension(subset.width(), sample_size),
            sampled_dimension(subset.height(), sample_size),
        ))
    } else {
        android.get_sampled_dimensions(sample_size)
    };
    let info = android
        .info()
        .with_dimensions(dims)
        .with_color_type(ColorType::N32)
        .with_alpha_type(AlphaType::Premul);
    let row_bytes = if custom_row_bytes != 0 {
        custom_row_bytes
    } else {
        info.min_row_bytes()
    };
    let pixel_bytes = info.compute_byte_size(row_bytes);
    let mut buffer = vec![GUARD_VALUE; GUARD_BYTES + pixel_bytes + GUARD_BYTES];
    let options = AndroidOptions {
        sample_size,
        base: Options {
            subset: use_subset.then_some(subset),
            ..Options::default()
        },
        ..AndroidOptions::default()
    };
    let result = android.get_android_pixels(
        &info,
        &mut buffer[GUARD_BYTES..GUARD_BYTES + pixel_bytes],
        row_bytes,
        Some(&options),
    );
    reporter_assert!(
        r,
        result == Result::Unimplemented,
        "Expected the decode to be refused with kUnimplemented, got {result:?} \
         (sampleSize={sample_size}, useSubset={use_subset}, rowBytes={row_bytes})"
    );
    let guard_bytes_intact = buffer[..GUARD_BYTES].iter().all(|&b| b == GUARD_VALUE)
        && buffer[GUARD_BYTES + pixel_bytes..].iter().all(|&b| b == GUARD_VALUE);
    reporter_assert!(
        r,
        guard_bytes_intact,
        "The decode wrote outside of the destination buffer \
         (sampleSize={sample_size}, useSubset={use_subset}, rowBytes={row_bytes})"
    );
}

// Port of: tests/SkPngRustDecoderTest.cpp#L1849-L1882 (RustPngCodec_apng_offset_frame_small_row_bytes)
def_test!(RustPngCodec_apng_offset_frame_small_row_bytes, |r| {
    let path = "images/apng-single-frame-with-offset.png";
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "Missing resource: {path}");
        return;
    };
    let Ok(mut codec) = png_rust_codec::make_from_stream(MemoryStream::make_copy(&data)) else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec.info().with_color_type(ColorType::N32);
    let mut buffer = vec![0u8; info.compute_min_byte_size()];
    let result = codec.start_incremental_decode(&info, &mut buffer, info.min_row_bytes() / 2, None);
    assert_successful_codec_result!(r, result);
    let (result, _) = codec.incremental_decode(&mut buffer);
    reporter_assert!(
        r,
        result == Result::InvalidParameters,
        "expected kInvalidParameters, got {result:?}"
    );
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1888-L1921 (RustPngCodec_apng_offset_frame_full_canvas_placement)
def_test!(RustPngCodec_apng_offset_frame_full_canvas_placement, |r| {
    let path = "images/apng-single-frame-with-offset.png";
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "Missing resource: {path}");
        return;
    };
    let Ok(mut codec) = png_rust_codec::make_from_stream(MemoryStream::make_copy(&data)) else {
        errorf!(r, "Failed to create a codec for {path}");
        return;
    };
    for alpha_type in [AlphaType::Unpremul, AlphaType::Premul] {
        let info = codec
            .info()
            .with_color_type(ColorType::N32)
            .with_alpha_type(alpha_type);
        let mut bm = Buf::new(info.clone());
        bm.erase(Color::TRANSPARENT);
        let row_bytes = bm.row_bytes;
        let result = codec.get_pixels(&info, &mut bm.data, row_bytes, None);
        assert_successful_codec_result!(r, result);
        reporter_assert!(r, bm.color(0, 0) == Color::TRANSPARENT);
        reporter_assert!(r, bm.color(63, 31) == Color::TRANSPARENT);
        reporter_assert!(r, bm.color(0, 32) == Color::MAGENTA);
        reporter_assert!(r, bm.color(63, 63) == Color::MAGENTA);
    }
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1970-L1977 (RustPngCodec_subsamplingDoesNotReadToIend ...)
// Not ported: those tests exist only in the `SK_CODEC_USES_PNG_WITH_RUST_FOR_ANDROID` build.

/// Port of `AssertAndroidDecodeSampling` with a sample size list, the default branch of
/// `RustPngCodec_subsampling*`.
fn assert_subsampling(
    r: &mut Reporter,
    path: &str,
    get_subset: Option<&dyn Fn(&ImageInfo) -> IRect>,
) {
    for sample_size in [2, 3, 5, 8, 100, 1000] {
        assert_android_decode_sampling(r, path, sample_size, get_subset);
    }
}

// Port of: tests/SkPngRustDecoderTest.cpp#L2002-L2008 (RustPngCodec_subsampling)
def_test!(RustPngCodec_subsampling, |r| {
    assert_subsampling(r, "images/plane.png", None);
});

// Port of: tests/SkPngRustDecoderTest.cpp#L2010-L2014 (RustPngCodec_subsampling_interlaced)
def_test!(RustPngCodec_subsampling_interlaced, |r| {
    assert_subsampling(r, "images/plane_interlaced.png", None);
});

// Port of: tests/SkPngRustDecoderTest.cpp#L2016-L2024 (RustPngCodec_subsampling_subset)
def_test!(RustPngCodec_subsampling_subset, |r| {
    assert_subsampling(
        r,
        "images/plane.png",
        Some(&|info: &ImageInfo| {
            IRect::from_xywh(info.width() / 2, 0, info.width() / 2, info.height())
        }),
    );
});

// Port of: tests/SkPngRustDecoderTest.cpp#L2026-L2032 (RustPngCodec_subsampling_subset_interlaced)
def_test!(RustPngCodec_subsampling_subset_interlaced, |r| {
    assert_subsampling(
        r,
        "images/plane_interlaced.png",
        Some(&|info: &ImageInfo| IRect::from_xywh(0, 1, info.width(), info.height() - 1)),
    );
});

// Port of: tests/SkPngRustDecoderTest.cpp#L2032-L2034 (RustPngCodec_missingOrCorruptIendSucceeds)
def_test!(RustPngCodec_missingOrCorruptIendSucceeds, |r| {
    assert_decodes_with_broken_iend_tail(r, "images/mandrill_128.png");
});

// Port of: tests/SkPngRustDecoderTest.cpp#L2036-L2038 (RustPngCodec_missingOrCorruptIendSucceeds_interlaced)
def_test!(RustPngCodec_missingOrCorruptIendSucceeds_interlaced, |r| {
    assert_decodes_with_broken_iend_tail(r, "images/plane_interlaced.png");
});

// Port of: tests/SkPngRustDecoderTest.cpp#L2042-L2044 (RustPngCodec_missingOrCorruptIendSucceeds_readRow)
def_test!(RustPngCodec_missingOrCorruptIendSucceeds_readRow, |r| {
    assert_decodes_with_broken_iend_tail(r, "images/color_wheel_with_profile.png");
});

// Port of: tests/SkPngRustDecoderTest.cpp#L2046-L2048 (RustPngCodec_missingOrCorruptIendSucceeds_sampled)
def_test!(RustPngCodec_missingOrCorruptIendSucceeds_sampled, |r| {
    assert_sampled_decode_with_broken_iend_tail(r, "images/mandrill_128.png", 2);
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1405-L1470 (RustPngCodec_interlaced_partial_decode_covers_all_pixels)
def_test!(RustPngCodec_interlaced_partial_decode_covers_all_pixels, |r| {
    let path = "images/plane_interlaced.png";
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "Missing resource: {path}");
        return;
    };
    let full_length = data.len();
    // Enough bytes to cover the initial Adam7 passes, but not the whole image.
    let initial_bytes = full_length / 4;
    let halting = HaltingStream::new(&data, initial_bytes);
    let retained = halting.clone();
    let mut codec = match png_rust_codec::make_from_stream(Box::new(halting)) {
        Ok(codec) => codec,
        Err(result) => {
            assert_successful_codec_result!(r, result);
            return;
        }
    };
    let mut bitmap = Buf::new(codec.info());
    // Fill `bitmap` with a sentinel colour, so that pixels the codec never wrote are detectable.
    let sentinel = Color::from(0xFF12_3456);
    bitmap.erase(sentinel);
    let options = Options {
        zero_initialized: ZeroInitialized::No,
        ..Options::default()
    };
    let info = bitmap.info.clone();
    let row_bytes = bitmap.row_bytes;
    let result = codec.start_incremental_decode(&info, &mut bitmap.data, row_bytes, Some(&options));
    assert_successful_codec_result!(r, result);
    let (result, _) = codec.incremental_decode(&mut bitmap.data);
    reporter_assert!(r, result == Result::IncompleteInput, "result = {result:?}");
    let mut untouched_pixels = 0;
    for y in 0..bitmap.info.height() {
        for x in 0..bitmap.info.width() {
            if bitmap.color(x, y) == sentinel {
                untouched_pixels += 1;
            }
        }
    }
    reporter_assert!(r, untouched_pixels == 0, "untouchedPixels = {untouched_pixels}");

    // Finishing the decode has to produce exactly the same pixels as a decode that had all the
    // input available from the start.
    retained.add_new_data(full_length);
    let (result, _) = codec.incremental_decode(&mut bitmap.data);
    assert_successful_codec_result!(r, result);
    let Some(mut reference_codec) = png_rust_decoder_decode(r, path) else {
        return;
    };
    let mut reference = Buf::new(bitmap.info.clone());
    let reference_row_bytes = reference.row_bytes;
    let result =
        reference_codec.get_pixels(&bitmap.info, &mut reference.data, reference_row_bytes, None);
    assert_successful_codec_result!(r, result);
    compare_bitmaps_by_color(r, &bitmap, &reference);
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1472-L1495 (RustPngCodec_interlaced_animated_blending)
def_test!(RustPngCodec_interlaced_animated_blending, |r| {
    let resource = "images/interlaced-multiframe-with-blending.png";
    let Some(mut codec) = png_rust_decoder_decode(r, resource) else {
        return;
    };
    // Use incrementalDecode for each frame of this image. This should not crash.
    let info = codec.info();
    let mut bm = Buf::new(info.clone());
    reporter_assert!(r, codec.get_frame_count() == 4);
    for i in 0..codec.get_frame_count() {
        let options = Options {
            frame_index: i,
            prior_frame: i - 1,
            ..Options::default()
        };
        let row_bytes = bm.row_bytes;
        let result = codec.start_incremental_decode(&info, &mut bm.data, row_bytes, Some(&options));
        assert_successful_codec_result!(r, result);
        let _ = codec.incremental_decode(&mut bm.data);
    }
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1497-L1510 (RustPngCodec_sbit565_ihdr16bits)
def_test!(RustPngCodec_sbit565_ihdr16bits, |r| {
    let Some(mut codec) = png_rust_decoder_decode(r, "images/basn2c16-sbit565.png") else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec.info();
    let mut bm = Buf::new(info.clone());
    reporter_assert!(r, codec.get_frame_count() == 1);
    let row_bytes = bm.row_bytes;
    let result = codec.start_incremental_decode(&info, &mut bm.data, row_bytes, None);
    assert_successful_codec_result!(r, result);
    let (result, _) = codec.incremental_decode(&mut bm.data);
    assert_successful_codec_result!(r, result);
});

// Port of: tests/SkPngRustDecoderTest.cpp#L1227-L1245 (RustPngCodec_green15x15) is above.

// Port of: tests/SkPngRustDecoderTest.cpp#L2050-L2054 (RustPngCodec_subsetOfTruncatedFile)
def_test!(RustPngCodec_subsetOfTruncatedFile, |r| {
    const PATH: &str = "images/mandrill_128.png";
    let Some(data) = get_resource_as_data(PATH) else {
        errorf!(r, "Missing resource: {PATH}");
        return;
    };
    let top_subset = |info: &ImageInfo| IRect::from_wh(info.width(), 16);
    let ref_bm = decode_android_pixels(
        r,
        png_rust_codec::make_from_stream(MemoryStream::make_copy(&data)).ok(),
        1,
        Some(&top_subset),
        Result::Success,
    );
    reporter_assert!(r, ref_bm.is_some());
    let truncated_half = &data[..data.len() / 2];
    let bm = decode_android_pixels(
        r,
        png_rust_codec::make_from_stream(MemoryStream::make_copy(truncated_half)).ok(),
        1,
        Some(&top_subset),
        Result::Success,
    );
    reporter_assert!(r, bm.is_some());
    if let (Some(ref_bm), Some(bm)) = (ref_bm, bm) {
        compare_bitmaps(r, &ref_bm, &bm);
    }
});

// Port of: tests/SkPngRustDecoderTest.cpp#L2080-L2095 (the `Codec_...` helper checks, the default
// subset-halting case)
def_test!(RustPngCodec_subset_halting, |r| {
    let Some(data) = get_resource_as_data("images/mandrill_128.png") else {
        errorf!(r, "Missing resource: images/mandrill_128.png");
        return;
    };
    let subset = IRect::from_xywh(32, 32, 64, 64);
    let mut bm_one_shot = Buf::new(ImageInfo::new_unknown(None));
    if !decode_subset_one_shot(r, &data, subset, &mut bm_one_shot) {
        return;
    }
    let mut bm_halting = Buf::new(ImageInfo::new_unknown(None));
    if !decode_subset_halting(r, &data, subset, &mut bm_halting) {
        return;
    }
    compare_bitmaps(r, &bm_one_shot, &bm_halting);
});
