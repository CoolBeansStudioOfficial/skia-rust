// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkPngRustEncoderTest.cpp (chrome/m156), the case of the default build. The gainmap
// and `sBIT` cases exist only under `SK_CODEC_USES_PNG_WITH_RUST_FOR_ANDROID`.

#![cfg(test)]

use skia_rust_codec::codecs::make_codec_from_stream;
use skia_rust_codec::encode::png_rust_encoder::{self, Options};
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::stream::MemoryStream;

use crate::resources::get_resource_as_data;
use crate::{def_test, reporter_assert};

/// Port of `ToolUtils::equal_pixels`: the two pixmaps have the same size, and every pixel
/// matches.
// Port of: tools/ToolUtils.cpp (equal_pixels, chrome/m156), as the test calls it
fn equal_pixels(a: &Pixmap<'_>, b: &Pixmap<'_>) -> bool {
    if a.width() != b.width() || a.height() != b.height() {
        return false;
    }
    (0..a.height()).all(|y| (0..a.width()).all(|x| a.get_color((x, y)) == b.get_color((x, y))))
}

// Port of: tests/SkPngRustEncoderTest.cpp#L23-L54 (RustEncodePng_smoke_test, chrome/m156)
def_test!(RustEncodePng_smoke_test, |r| {
    // Port of `ToolUtils::GetResourceAsBitmap`: decodes the resource into a raster bitmap.
    let Some(data) = get_resource_as_data("images/mandrill_128.png") else {
        return;
    };
    let Ok(mut source) = make_codec_from_stream(MemoryStream::make_copy(&data)) else {
        reporter_assert!(r, false);
        return;
    };
    let source_info = source.info();
    let Ok(bitmap) = source.get_image(source_info, None::<&skia_rust_codec::Options>) else {
        reporter_assert!(r, false);
        return;
    };
    let Some(src) = bitmap.peek_pixels() else {
        reporter_assert!(r, false);
        return;
    };

    let options = Options::default();
    let Some(encoded) = png_rust_encoder::encode_pixmap(&src, &options) else {
        reporter_assert!(r, false);
        return;
    };

    // Port of `ToolUtils::DecodeDataToBitmap`: decodes through the codec registry.
    let Ok(mut codec) = make_codec_from_stream(MemoryStream::make_copy(encoded.as_bytes())) else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec.info();
    let Ok(roundtrip) = codec.get_image(info, None::<&skia_rust_codec::Options>) else {
        reporter_assert!(r, false);
        return;
    };
    let Some(roundtrip_pixels) = roundtrip.peek_pixels() else {
        reporter_assert!(r, false);
        return;
    };
    reporter_assert!(r, equal_pixels(&src, &roundtrip_pixels));
});
