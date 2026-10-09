// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CodecPartialTest.cpp (chrome/m156), the cases that decode PNG only. The other
// cases decode GIF, WebP, JPEG, BMP and WBMP files, and the incremental-stream helpers they use
// (`HaltingStream`, `test_partial`) are ported with them.

use skia_rust_codec::{Codec, Result, decoders};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::stream::MemoryStream;

use crate::resources::get_resource_as_data;
use crate::{def_test, reporter_assert, skip_missing_resource};

// Port of: tests/CodecPartialTest.cpp#L29-L34 (standardize_info): the N32 premultiplied info of
// the image, without a colour space.
fn standardize_info(codec: &Codec<'_>) -> ImageInfo {
    let dims = codec.dimensions();
    ImageInfo::new(
        dims,
        ColorType::N32,
        AlphaType::Premul,
        None::<skia_rust_core::color_space::ColorSpace>,
    )
}

// Port of: tests/CodecPartialTest.cpp#L452-L474 (chrome/m156)
def_test!(Codec_emptyIDAT, |r| {
    let name = "images/baby_tux.png";
    let file = skip_missing_resource!(get_resource_as_data(name), name);
    // Truncate to the beginning of the IDAT, immediately after the IDAT tag.
    let truncated = &file[..80.min(file.len())];
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(truncated), decoders())
    else {
        reporter_assert!(r, false);
        return;
    };
    let info = standardize_info(&codec);
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let result = codec.get_pixels(&info, &mut pixels, row_bytes, None);
    reporter_assert!(r, result == Result::IncompleteInput);
});
