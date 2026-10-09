// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Crate tests for the SkSwizzler port (not 1:1 ports of a Skia test: Skia has no unit test that
// drives the swizzler directly). The expected bytes are worked out from the C++ formulas in
// src/codec/SkSwizzler.cpp, and channel order is written out explicitly so the results do not
// depend on the host's N32 order.

use skia_rust_codec::codec::{Options, ZeroInitialized};
use skia_rust_codec::encoded_info::{Alpha, Color, EncodedInfo};
use skia_rust_codec::swizzler::Swizzler;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;

fn info(ct: ColorType, at: AlphaType, w: i32) -> ImageInfo {
    ImageInfo::new(ISize::new(w, 1), ct, at, None)
}

fn encoded(w: i32, color: Color, alpha: Alpha, bpc: u8) -> EncodedInfo {
    EncodedInfo::make(w, 1, color, alpha, bpc)
}

// RGB rows expand to RGBA with an opaque alpha, red in the first byte.
#[test]
fn rgb_to_rgba_writes_red_first() {
    let src = [1u8, 2, 3, 4, 5, 6];
    let dst_info = info(ColorType::RGBA8888, AlphaType::Unpremul, 2);
    let enc = encoded(2, Color::RGB, Alpha::Opaque, 8);
    let sw = Swizzler::make(&enc, None, &dst_info, &Options::default(), None).expect("swizzler");
    let mut dst = [0u8; 8];
    sw.swizzle(&mut dst, &src);
    assert_eq!(dst, [1, 2, 3, 0xFF, 4, 5, 6, 0xFF]);
}

// RGBA with premultiply: 255 * 128 / 255 rounds to 128 for each colour channel.
#[test]
fn rgba_premultiplies_colour_channels() {
    let src = [255u8, 255, 255, 128];
    let dst_info = info(ColorType::RGBA8888, AlphaType::Premul, 1);
    let enc = encoded(1, Color::RGBA, Alpha::Unpremul, 8);
    let sw = Swizzler::make(&enc, None, &dst_info, &Options::default(), None).expect("swizzler");
    let mut dst = [0u8; 4];
    sw.swizzle(&mut dst, &src);
    assert_eq!(dst, [128, 128, 128, 128]);
}

// A 2-bit palette reads four indices per byte, most significant bits first: 01 10 11 00.
#[test]
fn two_bit_palette_reads_msb_first() {
    let ctable = [0x1111_1111u32, 0x2222_2222, 0x3333_3333, 0x4444_4444];
    let src = [0b0110_1100u8];
    let dst_info = info(ColorType::RGBA8888, AlphaType::Unpremul, 4);
    let enc = encoded(4, Color::Palette, Alpha::Opaque, 2);
    let sw = Swizzler::make(&enc, Some(&ctable), &dst_info, &Options::default(), None)
        .expect("swizzler");
    let mut dst = [0u8; 16];
    sw.swizzle(&mut dst, &src);
    let expected: Vec<u8> = [1usize, 2, 3, 0]
        .iter()
        .flat_map(|&i| ctable[i].to_ne_bytes())
        .collect();
    assert_eq!(dst.as_slice(), expected.as_slice());
}

// A palette without a colour table is rejected, as SkSwizzler::Make does.
#[test]
fn palette_without_color_table_is_rejected() {
    let dst_info = info(ColorType::RGBA8888, AlphaType::Unpremul, 4);
    let enc = encoded(4, Color::Palette, Alpha::Opaque, 8);
    assert!(Swizzler::make(&enc, None, &dst_info, &Options::default(), None).is_none());
}

// Sampling by 2 keeps the pixels at x = 1 and x = 3: the first kept pixel is
// SkCodecPriv::GetStartCoord(2) = 1, and then every second pixel after it.
#[test]
fn set_sample_x_keeps_every_second_pixel() {
    let src: Vec<u8> = (0u8..12).collect(); // four RGB pixels: 0..3, 3..6, 6..9, 9..12
    let dst_info = info(ColorType::RGBA8888, AlphaType::Unpremul, 4);
    let enc = encoded(4, Color::RGB, Alpha::Opaque, 8);
    let mut sw =
        Swizzler::make(&enc, None, &dst_info, &Options::default(), None).expect("swizzler");
    let width = sw.set_sample_x(2);
    assert_eq!(width, 2);
    let mut dst = [0u8; 8];
    sw.swizzle(&mut dst, &src);
    assert_eq!(dst, [3, 4, 5, 0xFF, 9, 10, 11, 0xFF]);
}

// Gray alpha with a zero-initialised destination skips leading transparent pixels, leaving the
// (already zero) destination untouched for them.
#[test]
fn zero_initialized_gray_alpha_skips_leading_zero_pixels() {
    let src = [0u8, 0, 7, 255];
    let dst_info = info(ColorType::RGBA8888, AlphaType::Premul, 2);
    let enc = encoded(2, Color::GrayAlpha, Alpha::Unpremul, 8);
    let options = Options {
        zero_initialized: ZeroInitialized::Yes,
        ..Options::default()
    };
    let sw = Swizzler::make(&enc, None, &dst_info, &options, None).expect("swizzler");
    let mut dst = [0xEEu8; 8];
    sw.swizzle(&mut dst, &src);
    assert_eq!(&dst[..4], &[0xEE, 0xEE, 0xEE, 0xEE]);
    // Pixel 1 is gray 7 at full alpha, which is unchanged by premultiplication.
    assert_eq!(&dst[4..], &[7, 7, 7, 255]);
}

// A frame offset moves the destination start, as SkSwizzler's frame argument does.
#[test]
fn frame_offset_moves_destination() {
    let src = [9u8, 8, 7];
    let dst_info = info(ColorType::RGBA8888, AlphaType::Unpremul, 3);
    let enc = encoded(1, Color::Gray, Alpha::Opaque, 8);
    let frame = IRect::new(1, 0, 2, 1);
    let sw =
        Swizzler::make(&enc, None, &dst_info, &Options::default(), Some(frame)).expect("swizzler");
    let mut dst = [0u8; 12];
    sw.swizzle(&mut dst, &src);
    // The first pixel is written at destination pixel 1, one pixel in.
    assert_eq!(&dst[..4], &[0, 0, 0, 0]);
    assert_eq!(&dst[4..8], &[9, 9, 9, 0xFF]);
}
