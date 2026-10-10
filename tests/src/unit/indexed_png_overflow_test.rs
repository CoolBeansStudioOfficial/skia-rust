// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/IndexedPngOverflowTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_codec::{Codec, Result, decoders};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::stream::MemoryStream;
use skia_rust_raster::surfaces;

use crate::{def_test, reporter_assert};

// Port of: tests/CodecPriv.h#L48-L58 (chrome/m156), decode_memory: decodes `mem` into `bm` at the
// codec's natural info, and reports whether the decode succeeded or only ran out of input.
fn decode_memory(mem: &[u8], bm: &mut Bitmap) -> bool {
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(mem), decoders()) else {
        return false;
    };
    let info = codec.info();
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let result = codec.get_pixels(&info, &mut pixels, row_bytes, None);
    let _ = bm.install_pixels(&info, pixels, row_bytes);
    result == Result::Success || result == Result::IncompleteInput
}

// Port of: tests/IndexedPngOverflowTest.cpp#L18-L31 (chrome/m156), gPng
const G_PNG: [u8; 128] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00, 0x01, 0x08, 0x03, 0x00, 0x00, 0x00, 0xe9, 0x4c, 0x7e,
    0x17, 0x00, 0x00, 0x00, 0x09, 0x70, 0x48, 0x59, 0x73, 0x00, 0x00, 0x00, 0x1c, 0x00, 0x00, 0x00,
    0x1c, 0x00, 0x0f, 0x01, 0xb9, 0x8f, 0x00, 0x00, 0x00, 0x06, 0x50, 0x4c, 0x54, 0x45, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0xa5, 0x67, 0xb9, 0xcf, 0x00, 0x00, 0x00, 0x20, 0x49, 0x44, 0x41, 0x54,
    0x78, 0xda, 0xed, 0xfd, 0x07, 0x01, 0x00, 0x20, 0x08, 0x00, 0x41, 0xbc, 0x5b, 0xe8, 0xdf, 0x97,
    0x99, 0xe3, 0x92, 0xa0, 0xf2, 0xdf, 0x3d, 0x7b, 0x0d, 0xda, 0x04, 0x1c, 0x03, 0xad, 0x00, 0x38,
    0x5c, 0x2e, 0xad, 0x12, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

// Port of: tests/IndexedPngOverflowTest.cpp#L33-L40 (chrome/m156)
def_test!(IndexedPngOverflow, |reporter| {
    let mut bm = Bitmap::new();
    let success = decode_memory(&G_PNG, &mut bm);
    reporter_assert!(reporter, success);

    // SkSurfaces::Raster(SkImageInfo::MakeN32Premul(20, 1))->getCanvas()->drawImage(bm.asImage(), 0, 0)
    let info = ImageInfo::new(
        (20, 1),
        ColorType::N32,
        AlphaType::Premul,
        None::<skia_rust_core::color_space::ColorSpace>,
    );
    let Some(mut surface) = surfaces::raster(&info, None, None) else {
        return;
    };
    if let Some(image) = bm.as_image() {
        surface.canvas().draw_image(&image, (0, 0), None);
    }
});
