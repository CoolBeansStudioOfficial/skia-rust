// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/GifTest.cpp (chrome/m156). The cases decode through the codec registry, as
// SkCodec::MakeFromData does, so they exercise the GIF codec (SkWuffsCodec) and the Wuffs decoder
// it drives.

#![cfg(test)]
// The colour literals are written as in tests/GifTest.cpp, and the data is written as the C++ has
// it, so the digit grouping and byte casts follow the source.
#![allow(
    clippy::unreadable_literal,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "literals, byte values and pixel indices mirror tests/GifTest.cpp"
)]

use skia_rust_codec::android_codec::{AndroidCodec, AndroidOptions};
use skia_rust_codec::codec::{NO_FRAME, Options, Result};
use skia_rust_codec::{Codec, decoders};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::IRect;
use skia_rust_core::stream::MemoryStream;

use crate::resources::get_resource_as_data;
use crate::{Reporter, def_test, errorf, reporter_assert, skip_missing_resource};

// Port of: tests/CodecPriv.h#L48-L58 (decode_memory). Decodes `mem` at the codec's natural info,
// into a bitmap that owns the pixels. The flag is whether the decode succeeded or only ran out of
// input, as the C++ reports it.
fn decode_memory(mem: &[u8]) -> (bool, Bitmap) {
    let mut bm = Bitmap::new();
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(mem), decoders()) else {
        return (false, bm);
    };
    let info = codec.info();
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let result = codec.get_pixels(&info, &mut pixels, row_bytes, None);
    let installed = bm.install_pixels(&info, pixels, row_bytes);
    debug_assert!(installed);
    (
        result == Result::Success || result == Result::IncompleteInput,
        bm,
    )
}

// The colour of the pixel at (x, y), as SkBitmap::getColor returns it (ARGB, unpremultiplied).
fn color_at(bm: &Bitmap, x: i32, y: i32) -> u32 {
    u32::from(bm.get_color((x, y)))
}

// Port of: tests/GifTest.cpp (test_gif_data): a 3x3 image with a known pixel at each position.
fn test_gif_data(r: &mut Reporter, data: &[u8]) {
    let (decode_success, bm) = decode_memory(data);
    reporter_assert!(r, decode_success);
    reporter_assert!(r, bm.width() == 3);
    reporter_assert!(r, bm.height() == 3);
    reporter_assert!(r, !bm.is_empty());
    if !bm.is_empty() {
        reporter_assert!(r, color_at(&bm, 0, 0) == 0xffff0000);
        reporter_assert!(r, color_at(&bm, 1, 0) == 0xffffff00);
        reporter_assert!(r, color_at(&bm, 2, 0) == 0xff00ffff);
        reporter_assert!(r, color_at(&bm, 0, 1) == 0xff808080);
        reporter_assert!(r, color_at(&bm, 1, 1) == 0xff000000);
        reporter_assert!(r, color_at(&bm, 2, 1) == 0xff00ff00);
        reporter_assert!(r, color_at(&bm, 0, 2) == 0xffffffff);
        reporter_assert!(r, color_at(&bm, 1, 2) == 0xffff00ff);
        reporter_assert!(r, color_at(&bm, 2, 2) == 0xff0000ff);
    }
}

// Port of: tests/GifTest.cpp (test_gif_data_no_colormap): a 1x1 image with no colour table.
fn test_gif_data_no_colormap(r: &mut Reporter, data: &[u8]) {
    let (decode_success, bm) = decode_memory(data);
    reporter_assert!(r, decode_success);
    reporter_assert!(r, bm.width() == 1);
    reporter_assert!(r, bm.height() == 1);
    reporter_assert!(r, !bm.is_empty());
    if !bm.is_empty() {
        reporter_assert!(r, color_at(&bm, 0, 0) == 0x00000000);
    }
}

// Port of: tests/GifTest.cpp (test_gif_data_dims): only the size of the decoded image is checked.
fn test_gif_data_dims(r: &mut Reporter, data: &[u8], width: i32, height: i32) {
    let (decode_success, bm) = decode_memory(data);
    reporter_assert!(r, decode_success);
    reporter_assert!(r, bm.width() == width);
    reporter_assert!(r, bm.height() == height);
    reporter_assert!(r, !bm.is_empty());
}

// Port of: tests/GifTest.cpp (test_interlaced_gif_data): a 9x9 interlaced image.
fn test_interlaced_gif_data(r: &mut Reporter, data: &[u8]) {
    let (decode_success, bm) = decode_memory(data);
    reporter_assert!(r, decode_success);
    reporter_assert!(r, bm.width() == 9);
    reporter_assert!(r, bm.height() == 9);
    reporter_assert!(r, !bm.is_empty());
    if !bm.is_empty() {
        reporter_assert!(r, color_at(&bm, 0, 0) == 0xffff0000);
        reporter_assert!(r, color_at(&bm, 1, 0) == 0xffffff00);
        reporter_assert!(r, color_at(&bm, 2, 0) == 0xff00ffff);

        reporter_assert!(r, color_at(&bm, 0, 2) == 0xffffffff);
        reporter_assert!(r, color_at(&bm, 1, 2) == 0xffff00ff);
        reporter_assert!(r, color_at(&bm, 2, 2) == 0xff0000ff);

        reporter_assert!(r, color_at(&bm, 0, 4) == 0xff808080);
        reporter_assert!(r, color_at(&bm, 1, 4) == 0xff000000);
        reporter_assert!(r, color_at(&bm, 2, 4) == 0xff00ff00);

        reporter_assert!(r, color_at(&bm, 0, 6) == 0xffff0000);
        reporter_assert!(r, color_at(&bm, 1, 6) == 0xffffff00);
        reporter_assert!(r, color_at(&bm, 2, 6) == 0xff00ffff);

        reporter_assert!(r, color_at(&bm, 0, 8) == 0xffffffff);
        reporter_assert!(r, color_at(&bm, 1, 8) == 0xffff00ff);
        reporter_assert!(r, color_at(&bm, 2, 8) == 0xff0000ff);
    }
}

// Port of: tests/GifTest.cpp (test_gif_data_short): the first six pixels of a truncated image.
fn test_gif_data_short(r: &mut Reporter, data: &[u8]) {
    let (decode_success, bm) = decode_memory(data);
    reporter_assert!(r, decode_success);
    reporter_assert!(r, bm.width() == 3);
    reporter_assert!(r, bm.height() == 3);
    reporter_assert!(r, !bm.is_empty());
    if !bm.is_empty() {
        reporter_assert!(r, color_at(&bm, 0, 0) == 0xffff0000);
        reporter_assert!(r, color_at(&bm, 1, 0) == 0xffffff00);
        reporter_assert!(r, color_at(&bm, 2, 0) == 0xff00ffff);
        reporter_assert!(r, color_at(&bm, 0, 1) == 0xff808080);
        reporter_assert!(r, color_at(&bm, 1, 1) == 0xff000000);
        reporter_assert!(r, color_at(&bm, 2, 1) == 0xff00ff00);
    }
}

// Port of: tests/GifTest.cpp (gGIFData)
const G_GIF_DATA: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x37, 0x61, 0x03, 0x00, 0x03, 0x00, 0xe3, 0x08, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0xff, 0xff, 0x00, 0x00, 0xff, 0x00, 0xff, 0x80, 0x80, 0x80, 0x00, 0xff, 0x00, 0x00,
    0xff, 0xff, 0xff, 0xff, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x2c, 0x00, 0x00,
    0x00, 0x00, 0x03, 0x00, 0x03, 0x00, 0x00, 0x04, 0x07, 0x50, 0x1c, 0x43, 0x40, 0x41, 0x23, 0x44,
    0x00, 0x3b,
];

// Port of: tests/GifTest.cpp (gGIFDataNoColormap)
const G_GIF_DATA_NO_COLORMAP: &[u8] = &[
    // Header
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, // Screen descriptor
    0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, // Graphics control extension
    0x21, 0xf9, 0x04, 0x01, 0x0a, 0x00, 0x01, 0x00, // Image descriptor
    0x2c, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, // Image data
    0x02, 0x02, 0x4c, 0x01, 0x00, // Trailer
    0x3b,
];

// Port of: tests/GifTest.cpp (gInterlacedGIF)
const G_INTERLACED_GIF: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x37, 0x61, 0x09, 0x00, 0x09, 0x00, 0xe3, 0x08, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0xff, 0xff, 0x00, 0x00, 0xff, 0x00, 0xff, 0x80, 0x80, 0x80, 0x00, 0xff, 0x00, 0x00,
    0xff, 0xff, 0xff, 0xff, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x2c, 0x00, 0x00,
    0x00, 0x00, 0x09, 0x00, 0x09, 0x00, 0x40, 0x04, 0x1b, 0x50, 0x1c, 0x23, 0xe9, 0x44, 0x23, 0x60,
    0x9d, 0x09, 0x28, 0x1e, 0xf8, 0x6d, 0x64, 0x56, 0x9d, 0x53, 0xa8, 0x7e, 0xa8, 0x65, 0x94, 0x5c,
    0xb0, 0x8a, 0x45, 0x04, 0x00, 0x3b,
];

// Port of: tests/GifTest.cpp#L129-L168 (Gif). Tests perfectly good images, and mangled ones, which
// should still show as much of the GIF as possible.
def_test!(Gif, |r| {
    // Test perfectly good images.
    test_gif_data(r, G_GIF_DATA);
    test_interlaced_gif_data(r, G_INTERLACED_GIF);

    let mut bad_data = G_GIF_DATA.to_vec();
    bad_data[6] = 0x01; // image too wide
    test_gif_data(r, &bad_data);

    // "libgif warning [image too wide, expanding output to size]"
    let mut bad_data = G_GIF_DATA.to_vec();
    bad_data[8] = 0x01; // image too tall
    test_gif_data(r, &bad_data);
    // "libgif warning [image too tall,  expanding output to size]"

    let mut bad_data = G_GIF_DATA.to_vec();
    bad_data[62] = 0x01; // image shifted right
    test_gif_data_dims(r, &bad_data, 4, 3);

    let mut bad_data = G_GIF_DATA.to_vec();
    bad_data[64] = 0x01; // image shifted down
    test_gif_data_dims(r, &bad_data, 3, 4);

    let mut bad_data = G_GIF_DATA.to_vec();
    bad_data[62] = 0xff; // image shifted right
    bad_data[63] = 0xff;
    test_gif_data_dims(r, &bad_data, 3 + 0xFFFF, 3);

    let mut bad_data = G_GIF_DATA.to_vec();
    bad_data[64] = 0xff; // image shifted down
    bad_data[65] = 0xff;
    test_gif_data_dims(r, &bad_data, 3, 3 + 0xFFFF);

    test_gif_data_no_colormap(r, G_GIF_DATA_NO_COLORMAP);

    // Test short Gif. 80 is missing a few bytes.
    test_gif_data_short(r, &G_GIF_DATA[..80]);

    // "libgif warning [DGifGetLine]"
    test_interlaced_gif_data(r, &G_INTERLACED_GIF[..100]); // 100 is missing a few bytes
    // "libgif warning [interlace DGifGetLine]"
});

// Port of: tests/GifTest.cpp#L213-L257 (Codec_GifInterlacedTruncated)
def_test!(Codec_GifInterlacedTruncated, |r| {
    // Check that gInterlacedGIF is exactly 102 bytes long, and that the final 30 bytes, in the
    // half-open range [72, 102), consists of 0x1b (indicating a block of 27 bytes), then those 27
    // bytes, then 0x00 (end of the blocks) then 0x3b (end of the GIF).
    if G_INTERLACED_GIF.len() != 102
        || G_INTERLACED_GIF[72] != 0x1b
        || G_INTERLACED_GIF[100] != 0x00
        || G_INTERLACED_GIF[101] != 0x3b
    {
        errorf!(r, "Invalid gInterlacedGIF data");
        return;
    }

    // Test the GIF codec's output on some (but not all) of the LZW-compressed data. We also modify
    // the block size down from 0x1b so that the edited version still contains a complete block: a
    // block of 10 bytes.
    let mut data = G_INTERLACED_GIF[..83].to_vec();
    data[72] = (data.len() - 73) as u8;

    // Just like test_interlaced_gif_data, check that we get a 9x9 image.
    let (decode_success, bm) = decode_memory(&data);
    reporter_assert!(r, decode_success);
    reporter_assert!(r, bm.width() == 9);
    reporter_assert!(r, bm.height() == 9);

    // Row 7 is set, by the replication of earlier interlace passes, so it is not transparent
    // black.
    reporter_assert!(r, color_at(&bm, 0, 7) != 0);
});

// Port of: tests/GifTest.cpp#L259-L284 (Gif_Sampled). A regression test for decoding a GIF with a
// sample size of 4, which used to crash.
def_test!(Gif_Sampled, |r| {
    let name = "images/test640x479.gif";
    let data = skip_missing_resource!(get_resource_as_data(name), name);
    let Some(mut codec) = AndroidCodec::make_from_stream(MemoryStream::make_copy(&data)) else {
        reporter_assert!(r, false);
        return;
    };
    let options = AndroidOptions {
        sample_size: 4,
        ..AndroidOptions::default()
    };
    let info = codec.info();
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let result = codec.get_android_pixels(&info, &mut pixels, row_bytes, Some(&options));
    reporter_assert!(r, result == Result::Success);
});

// Port of: tests/GifTest.cpp#L286-L296 (Codec_GifTruncated). If a GIF file is truncated before the
// header for the first image is defined, no codec is created.
def_test!(Codec_GifTruncated, |r| {
    let name = "images/test640x479.gif";
    let data = skip_missing_resource!(get_resource_as_data(name), name);
    // This is right before the header for the first image.
    let truncated = &data[..446];
    let codec = Codec::make_from_stream(MemoryStream::make_copy(truncated), decoders());
    reporter_assert!(r, codec.is_err());
});

// Port of: tests/GifTest.cpp#L323-L373 (Codec_GifTruncated2). Truncates box.gif at 21, 22 and 23
// bytes: 21 is too short for a codec, and 23 gives one with one frame.
def_test!(Codec_GifTruncated2, |r| {
    let name = "images/box.gif";
    let data = skip_missing_resource!(get_resource_as_data(name), name);
    for i in 21..24usize {
        let codec = Codec::make_from_stream(MemoryStream::make_copy(&data[..i]), decoders());
        if i <= 21 {
            if codec.is_ok() {
                errorf!(r, "Invalid data gave non-nullptr codec");
            }
            return;
        }
        let Ok(mut codec) = codec else {
            errorf!(
                r,
                "Failed to create codec with partial data (truncated at {})",
                i
            );
            return;
        };
        reporter_assert!(r, codec.get_frame_count() == 1);
    }
});

// Port of: tests/GifTest.cpp#L379-L416 (Codec_GifTruncated3). The image made from the first 23
// bytes of box.gif is 200x55, and its pixels are all transparent.
def_test!(Codec_GifTruncated3, |r| {
    let name = "images/box.gif";
    let data = skip_missing_resource!(get_resource_as_data(name), name);
    let subset = data[..23].to_vec();
    let Some(image) = skia_rust_codec::images::deferred_from_encoded_data(
        Some(skia_rust_core::data::Data::new_copy(&subset)),
        None,
    ) else {
        errorf!(r, "Missing image");
        return;
    };
    reporter_assert!(r, image.width() == 200);
    reporter_assert!(r, image.height() == 55);

    let info =
        ImageInfo::new_n32_premul((200, 55), None::<skia_rust_core::color_space::ColorSpace>);
    let row_bytes = 200 * 4;
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    reporter_assert!(r, image.read_pixels(&info, &mut pixels, row_bytes, (0, 0)));
    for i in 0..image.width() {
        for j in 0..image.height() {
            let offset = j as usize * row_bytes + i as usize * 4;
            let pm = u32::from_ne_bytes([
                pixels[offset],
                pixels[offset + 1],
                pixels[offset + 2],
                pixels[offset + 3],
            ]);
            let actual = u32::from(skia_rust_core::un_pre_multiply::pm_color_to_color(pm));
            if actual != 0x0000_0000 {
                errorf!(r, "did not initialize pixels! {}, {} is {:x}", i, j, actual);
            }
        }
    }
});

// Port of: tests/GifTest.cpp#L418-L458 (Codec_gif_out_of_palette)
def_test!(Codec_gif_out_of_palette, |r| {
    let path = "images/out-of-palette.gif";
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "failed to find {}", path);
        return;
    };
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        errorf!(r, "Could not create codec from {}", path);
        return;
    };
    let info = codec.info();
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let result = codec.get_pixels(&info, &mut pixels, row_bytes, None);
    reporter_assert!(r, result == Result::Success);
    let bm = {
        let mut bm = Bitmap::new();
        let installed = bm.install_pixels(&info, pixels, row_bytes);
        debug_assert!(installed);
        bm
    };

    // SK_ColorBLACK, SK_ColorWHITE and SK_ColorTRANSPARENT.
    let pixels = [
        (0, 0, 0xff00_0000u32),
        (1, 0, 0xffff_ffff),
        (0, 1, 0x0000_0000),
        (1, 1, 0x0000_0000),
    ];
    for (x, y, expected) in pixels {
        let actual = color_at(&bm, x, y);
        reporter_assert!(r, actual == expected);
    }
});

// Port of: tests/GifTest.cpp#L460-L536 (Codec_AnimatedTransparentGif). Decodes both frames, with
// the second frame blended over the first, in RGBA and in RGB565.
def_test!(Codec_AnimatedTransparentGif, |r| {
    let path = "images/gif-transparent-index.gif";
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "failed to find {}", path);
        return;
    };
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        errorf!(r, "Could not create codec from {}", path);
        return;
    };
    let info = codec.info();
    if info.width() != 4 || info.height() != 2 || codec.frame_infos().len() != 2 {
        errorf!(r, "Unexpected image info");
        return;
    }

    for use565 in [false, true] {
        let frame_info = if use565 {
            info.with_color_type(ColorType::RGB565)
        } else {
            info.clone()
        };
        let row_bytes = frame_info.min_row_bytes();
        // One buffer for both frames: the second frame blends with the pixels of the first.
        let mut pixels = vec![0u8; frame_info.compute_byte_size(row_bytes)];
        for i in 0..2 {
            let options = Options {
                frame_index: i,
                prior_frame: if i > 0 { i - 1 } else { NO_FRAME },
                ..Options::default()
            };
            let result = codec.get_pixels(&frame_info, &mut pixels, row_bytes, Some(&options));
            reporter_assert!(r, result == Result::Success);

            let mut expected_pixels: [[u32; 4]; 2] = [
                [0xFF80_0000, 0xFF90_0000, 0xFFA0_0000, 0xFFB0_0000],
                [0xFFC0_0000, 0xFFD0_0000, 0xFFE0_0000, 0xFFF0_0000],
            ];
            if use565 {
                expected_pixels[0][0] = 0xFF84_0000;
                expected_pixels[0][1] = 0xFF94_0000;
                expected_pixels[0][2] = 0xFFA5_0000;
                expected_pixels[0][3] = 0xFFB5_0000;
                expected_pixels[1][0] = 0xFFC6_0000;
                expected_pixels[1][1] = 0xFFD6_0000;
                expected_pixels[1][2] = 0xFFE7_0000;
                expected_pixels[1][3] = 0xFFF7_0000;
            }
            if i > 0 {
                expected_pixels[1][1] = 0xFF00_00FF;
                expected_pixels[1][3] = if use565 { 0xFF00_0052 } else { 0xFF00_0055 };
            }

            let mut bm = Bitmap::new();
            let installed = bm.install_pixels(&frame_info, pixels.clone(), row_bytes);
            debug_assert!(installed);
            for y in 0..2 {
                for x in 0..4 {
                    let expected = expected_pixels[y as usize][x as usize];
                    let actual = color_at(&bm, x, y);
                    reporter_assert!(r, actual == expected);
                }
            }
        }
    }
});

// Port of: tests/GifTest.cpp#L538-L579 (Codec_xOffsetTooBig). The second frame is placed outside
// the image, and still decodes.
def_test!(Codec_xOffsetTooBig, |r| {
    let path = "images/xOffsetTooBig.gif";
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "failed to find {}", path);
        return;
    };
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        errorf!(r, "Could not create codec from {}", path);
        return;
    };
    reporter_assert!(r, codec.get_frame_count() == 2);
    let info = codec.info();
    reporter_assert!(r, info.width() == 100 && info.height() == 90);

    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    for i in 0..2 {
        let frame_info = codec.get_frame_info(i);
        reporter_assert!(r, frame_info.is_some());
        let frame_info = frame_info.unwrap_or_default();
        let expected_rect = if i == 0 {
            IRect::from_ltrb(0, 0, 100, 90)
        } else {
            IRect::from_ltrb(100, 90, 100, 90)
        };
        reporter_assert!(r, expected_rect == frame_info.frame_rect);

        let options = Options {
            frame_index: i,
            ..Options::default()
        };
        reporter_assert!(
            r,
            Result::Success == codec.get_pixels(&info, &mut pixels, row_bytes, Some(&options))
        );
        let mut bm = Bitmap::new();
        let installed = bm.install_pixels(&info, pixels.clone(), row_bytes);
        debug_assert!(installed);
        // SK_ColorRED.
        reporter_assert!(r, color_at(&bm, 0, 0) == 0xFFFF_0000);
    }
});
