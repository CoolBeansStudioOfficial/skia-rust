// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/BadIcoTest.cpp (chrome/m156), the cases that run with the libpng-equivalent PNG
// decoder. The Rust PNG decoder variant needs SkPngRustDecoder, which is not ported.

#![cfg(test)]

use skia_rust_codec::Codec;
use skia_rust_codec::codec::Result;
use skia_rust_codec::codecs::{self, Decoder};
use skia_rust_core::stream::{MemoryStream, Stream};

use crate::codec_priv::{
    ScopedCodecDecoders, make_ico_from_png_resource, make_ico_with_png, png_decoder,
    serial_test_lock,
};
use crate::resources::get_resource_as_data;
use crate::{Reporter, def_test, reporter_assert, skip_missing_resource};

// Port of: tests/BadIcoTest.cpp#L33-L63 (chrome/m156)
def_test!(BadImage, |_r| {
    // These images are corrupt. It is not important whether we succeed or fail in codec creation or
    // decoding. We just want to make sure that we do not crash.
    let bad_images = [
        "sigabort_favicon.ico",
        "sigsegv_favicon.ico",
        "sigsegv_favicon_2.ico",
        "ico_leak01.ico",
        "ico_fuzz0.ico",
        "ico_fuzz1.ico",
        "skbug3442.webp",
        "skbug3429.webp",
        "b38116746.ico",
        "skbug5883.gif",
    ];

    // The registry is read by SkCodec::MakeFromStream, so the serial tests that change it wait.
    let _lock = serial_test_lock();
    let bad_images_folder = "invalid_images";
    for name in bad_images {
        let path = format!("{bad_images_folder}/{name}");
        let data = skip_missing_resource!(get_resource_as_data(&path), path);
        if let Ok(mut codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(&data)) {
            let info = codec.info();
            let row_bytes = info.min_row_bytes();
            let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
            let _ = codec.get_pixels(&info, &mut pixels, row_bytes, None);
        }
    }
});

// A PNG decoder that rejects every stream. Port of the lambda in test_bad_png_in_ico_does_not_fallback
// that sets `kErrorInInput`.
// Port of: tests/BadIcoTest.cpp#L90-L99 (chrome/m156)
fn reject_png<'a>(_stream: Box<dyn Stream + Send + 'a>) -> std::result::Result<Codec<'a>, Result> {
    Err(Result::ErrorInInput)
}

// Port of: tests/BadIcoTest.cpp#L51-L105 (test_bad_png_in_ico_does_not_fallback, chrome/m156)
fn test_bad_png_in_ico_does_not_fallback(reporter: &mut Reporter, png: Decoder) {
    let _scoped = ScopedCodecDecoders::new();

    // 1. When the PNG decoder is registered, decoding an ICO with a malformed PNG fails cleanly
    // without falling back to libpng.
    codecs::register(png);
    let malformed_png: [u8; 12] = [
        0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n', 0x00, 0x00, 0x00, 0x00,
    ];
    let bad_ico = make_ico_with_png(&malformed_png);
    let bad_codec = codecs::make_codec_from_stream(MemoryStream::make_copy(&bad_ico));
    reporter_assert!(reporter, bad_codec.is_err());

    // 2. A registered PNG decoder that rejects a valid PNG libpng could decode: SkIcoCodec must not
    // fall back to libpng (or attempt a redundant second decode).
    codecs::register(Decoder {
        id: "png",
        is_format: png.is_format,
        make_from_stream: reject_png,
    });
    let path = "images/mandrill_128.png";
    let ico_with_valid_png = skip_missing_resource!(make_ico_from_png_resource(path), path);
    let rejected_codec =
        codecs::make_codec_from_stream(MemoryStream::make_copy(&ico_with_valid_png));
    reporter_assert!(reporter, rejected_codec.is_err());
}

// Port of: tests/BadIcoTest.cpp#L121-L123 (chrome/m156), with the libpng decoder
def_test!(Ico_malformedPngWithLibpngNoRedundantFallback, |r| {
    let _lock = serial_test_lock();
    test_bad_png_in_ico_does_not_fallback(r, png_decoder());
});
