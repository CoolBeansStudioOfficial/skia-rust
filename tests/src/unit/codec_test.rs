// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CodecTest.cpp (chrome/m156), the cases that only need the codec base and the PNG,
// BMP and WBMP decoders. The rest of the file needs the Android codec, the image generator and the
// other decoders, and is ported with them.

use skia_rust_codec::android_codec::{AndroidCodec, AndroidOptions};
use skia_rust_codec::codecs::{self, Decoder};
use skia_rust_codec::{Codec, Options, Result, ZeroInitialized, decoders, png_codec};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::{MemoryStream, Stream};

use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::codec_priv::{
    ScopedCodecDecoders, ico_decoder, make_ico_from_png_resource, serial_test_lock,
};
use crate::resources::{get_resource_as_data, get_resource_as_image, resource_dir};
use crate::{Reporter, def_test, errorf, reporter_assert, skip_missing_resource};

// Port of: tests/CodecTest.cpp#L122-L135 (test_info, without the digest comparison that these
// cases do not use)
fn test_info(reporter: &mut Reporter, codec: &mut Codec<'_>, info: &ImageInfo, expected: Result) {
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let result = codec.get_pixels(info, &mut pixels, row_bytes, None);
    reporter_assert!(reporter, result == expected);
}

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
    test_info(r, &mut codec, &info, Result::Success);
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
    test_info(r, &mut codec, &info, Result::Success);
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
