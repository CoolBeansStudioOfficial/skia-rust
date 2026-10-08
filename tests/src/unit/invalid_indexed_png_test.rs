// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/InvalidIndexedPngTest.cpp (chrome/m156)

use skia_rust_codec::{Codec, Result, decoders};
use skia_rust_core::stream::MemoryStream;

use crate::def_test;

// Port of: tests/CodecPriv.h#L48-L58 (decode_memory): decodes `mem` into the codec's natural info,
// and reports whether the decode succeeded or only ran out of input.
fn decode_memory(mem: &[u8]) -> bool {
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(mem), decoders()) else {
        return false;
    };
    let info = codec.info();
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let result = codec.get_pixels(&info, &mut pixels, row_bytes, None);
    result == Result::Success || result == Result::IncompleteInput
}

// A valid 1x1 indexed PNG.
// Port of: tests/InvalidIndexedPngTest.cpp#L8-L17 (gPngData)
#[rustfmt::skip]
const G_PNG_DATA: [u8; 115] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d,
    0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
    0x08, 0x03, 0x00, 0x00, 0x00, 0x28, 0xcb, 0x34, 0xbb, 0x00, 0x00, 0x00,
    0x09, 0x70, 0x48, 0x59, 0x73, 0x00, 0x00, 0x00, 0x1c, 0x00, 0x00, 0x00,
    0x1c, 0x00, 0x0f, 0x01, 0xb9, 0x8f, 0x00, 0x00, 0x00, 0x06, 0x50, 0x4c,
    0x54, 0x45, 0xff, 0x00, 0x00, 0x00, 0xff, 0x00, 0xd2, 0x87, 0xef, 0x71,
    0x00, 0x00, 0x00, 0x13, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0xed, 0xfd,
    0x81, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0xf8, 0xaf, 0x16, 0x46, 0x00,
    0x02, 0x00, 0x01, 0x32, 0x60, 0xf7, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x49,
    0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

// Attempt to decode an invalid PNG that has a palette. Mostly we're looking to make sure we don't
// leak memory since libpng uses setjmp for error handling so it's very easy to accidentally skip
// destructors when a failure happens. As a result, we do not have any reporter_assert statements.
// Port of: tests/InvalidIndexedPngTest.cpp#L30-L36 (chrome/m156)
def_test!(InvalidIndexedPng, |_reporter| {
    // Make our PNG invalid by changing a byte.
    let mut png = G_PNG_DATA;
    png[G_PNG_DATA.len() - 1] = 1;
    decode_memory(&png);
});
