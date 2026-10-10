// Copyright 2026 The skia-rust Authors.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // test pixel patterns
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Round trips through the PNG encoder and the PNG decoder of this crate, for the 8-bit source
// formats. The 16-bit rows are checked by the libpng differential replay (crates/skia-rust-libpng
// tests/write_diff.rs), since the decoder has no 16-bit unorm target to read them back into.

use skia_rust_codec::codec::Result;
use skia_rust_codec::encode::png_encoder::{self, Options};
use skia_rust_codec::png_codec::make_from_stream;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::stream::MemoryStream;

/// A deterministic pattern for `n` bytes.
fn pattern(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 37 % 251) as u8).collect()
}

/// Encodes `pixels` as `info`, decodes the PNG, and returns the decoded pixels in `info`'s format.
fn round_trip(info: &ImageInfo, pixels: &[u8], row_bytes: usize) -> Vec<u8> {
    let pixmap = Pixmap::new_readonly(info, pixels, row_bytes).expect("a valid pixmap");
    let mut png = Vec::new();
    assert!(png_encoder::encode(&pixmap, &mut png, &Options::default()));
    assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);

    let mut codec = make_from_stream(MemoryStream::make_copy(&png)).expect("a PNG codec");
    let mut out = vec![0u8; info.compute_byte_size(row_bytes)];
    assert_eq!(
        codec.get_pixels(info, &mut out, row_bytes, None),
        Result::Success
    );
    out
}

#[test]
fn rgba8_unpremul_round_trips() {
    let (w, h) = (17, 9);
    let info = ImageInfo::new(
        (w, h),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        Some(ColorSpace::new_srgb()),
    );
    let row_bytes = info.min_row_bytes();
    let pixels = pattern(row_bytes * h as usize);
    assert_eq!(round_trip(&info, &pixels, row_bytes), pixels);
}

#[test]
fn gray8_round_trips() {
    let (w, h) = (13, 5);
    let info = ImageInfo::new(
        (w, h),
        ColorType::Gray8,
        AlphaType::Opaque,
        Some(ColorSpace::new_srgb()),
    );
    let row_bytes = info.min_row_bytes();
    let pixels = pattern(row_bytes * h as usize);
    assert_eq!(round_trip(&info, &pixels, row_bytes), pixels);
}

#[test]
fn display_p3_writes_an_icc_profile_the_decoder_accepts() {
    use skia_rust_core::color_space::{ColorSpace, named_gamut, named_transfer_fn};

    let (w, h) = (4, 3);
    let cs = ColorSpace::new_rgb(&named_transfer_fn::DOT22, &named_gamut::DISPLAY_P3)
        .expect("a 2.2 Display P3 space");
    let info = ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Unpremul, Some(cs));
    let row_bytes = info.min_row_bytes();
    let pixels = pattern(row_bytes * h as usize);
    let pixmap = Pixmap::new_readonly(&info, &pixels, row_bytes).expect("a valid pixmap");
    let mut png = Vec::new();
    assert!(png_encoder::encode(&pixmap, &mut png, &Options::default()));
    assert!(png.windows(4).any(|c| c == b"iCCP"), "an iCCP chunk");
    assert!(!png.windows(4).any(|c| c == b"sRGB"), "no sRGB chunk");

    let mut codec = make_from_stream(MemoryStream::make_copy(&png)).expect("a PNG codec");
    let mut out = vec![0u8; info.compute_byte_size(row_bytes)];
    assert_eq!(
        codec.get_pixels(&info, &mut out, row_bytes, None),
        Result::Success
    );
    assert_eq!(out, pixels);
}
