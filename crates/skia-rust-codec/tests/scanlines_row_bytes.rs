// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `get_scanlines` must not panic on a row stride or destination that is too small for the rows
//! it is asked to decode; it reports zero lines decoded instead.

use skia_rust_codec::{Codec, Result, decoders};
use skia_rust_core::stream::MemoryStream;

// An 8x2 WBMP: a type-0 header, width 8, height 2, then one byte per row.
const WBMP_8X2: [u8; 6] = [0x00, 0x00, 0x08, 0x02, 0xFF, 0x00];

#[test]
fn get_scanlines_rejects_short_row_bytes_and_dst() {
    let mut codec = Codec::make_from_stream(MemoryStream::make_copy(&WBMP_8X2), decoders())
        .expect("8x2 WBMP decodes");
    let info = codec.info();
    let row_bytes = info.min_row_bytes();
    assert_eq!(codec.start_scanline_decode(&info, None), Result::Success);

    let mut dst = vec![0u8; 2 * row_bytes];
    // A stride shorter than one row must not reach the decoder.
    assert_eq!(codec.get_scanlines(&mut dst, 1, row_bytes - 1), 0);
    // A destination too small for `count` rows must not reach the decoder.
    assert_eq!(
        codec.get_scanlines(&mut dst[..row_bytes - 1], 1, row_bytes),
        0
    );
    // A negative count is also a zero-line result, as in Skia.
    assert_eq!(codec.get_scanlines(&mut dst, -1, row_bytes), 0);

    // With valid arguments the decode proceeds, one row at a time.
    assert_eq!(codec.get_scanlines(&mut dst, 1, row_bytes), 1);
    assert_eq!(codec.get_scanlines(&mut dst, 1, row_bytes), 1);
}
