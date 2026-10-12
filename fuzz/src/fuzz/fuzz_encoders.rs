// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: fuzz/FuzzEncoders.cpp (chrome/m156)
//
// skia-rust encodes PNG with libpng (`SK_CODEC_ENCODES_PNG_WITH_LIBPNG`), so `PNGEncoder` is
// ported with libpng, and `PNGRustEncoder` (`SK_CODEC_ENCODES_PNG_WITH_RUST`) with the Rust
// encoder of `skia_rust_codec::encode::png_rust_encoder`.
// `_MakeEncoderCorpus` is not a fuzzer but a corpus writer; `crate::seeds` builds the encoder
// seeds in its format.

use skia_rust_codec::encode::png_rust_encoder::{self, CompressionLevel};
use skia_rust_codec::encode::{jpeg_encoder, png_encoder, webp_encoder};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::image_info::ImageInfo;

use crate::{Fuzz, def_fuzz};

// These values were picked arbitrarily to hopefully limit the size of the
// serialized SkPixmaps.
// Port of: fuzz/FuzzEncoders.cpp#L27-L28 (chrome/m156)
pub const MAX_WIDTH: i32 = 512;
pub const MAX_HEIGHT: i32 = 512;

// Port of: fuzz/FuzzEncoders.cpp#L30-L42 (chrome/m156)
fn make_fuzzed_bitmap(fuzz: &mut Fuzz<'_>) -> Bitmap {
    let mut bm = Bitmap::new();
    #[allow(clippy::cast_sign_loss)] // the positive MAX_WIDTH / MAX_HEIGHT
    let w: u32 = fuzz.next_range(1, MAX_WIDTH as u32);
    #[allow(clippy::cast_sign_loss)] // the positive MAX_WIDTH / MAX_HEIGHT
    let h: u32 = fuzz.next_range(1, MAX_HEIGHT as u32);
    #[allow(clippy::cast_possible_wrap)] // `w` and `h` are at most 512
    let info = ImageInfo::new_n32_premul((w as i32, h as i32), None);
    if !bm.try_alloc_pixels_info(&info, None) {
        return bm;
    }
    let n = w * h;
    // `nextN((SkPMColor*)bm.getPixels(), n)`: the rows are tight (minimum row bytes), so the
    // pixels are `n` consecutive 32-bit words.
    let colors: Vec<u32> = fuzz.next_n(n as usize);
    let mut pixmap = bm
        .peek_pixels_mut()
        .expect("allocated pixels can be peeked");
    let bytes = pixmap.bytes_mut().expect("allocated pixels are writable");
    for (dst, c) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(colors) {
        dst.copy_from_slice(&c.to_ne_bytes());
    }
    bm
}

// Port of: fuzz/FuzzEncoders.cpp#L45-L53 (chrome/m156)
def_fuzz!(PNGEncoder, fuzz_PNGEncoder, |fuzz| {
    let bm = make_fuzzed_bitmap(fuzz);

    let mut opts = png_encoder::Options::default();
    opts.z_lib_level = fuzz.next_range(0, 9);

    let _ = png_encoder::encode_pixmap(&bm.pixmap(), &opts);
});

// Port of: fuzz/FuzzEncoders.cpp#L57-L76 (chrome/m156)
def_fuzz!(PNGRustEncoder, fuzz_PNGRustEncoder, |fuzz| {
    let bm = make_fuzzed_bitmap(fuzz);
    let mut opts = png_rust_encoder::Options::default();
    // `int compression`.
    let compression: i32 = fuzz.next_range(0, 3);
    // Compression level 3 keeps the default (`kMedium`), as the C++ switch has no case for it.
    match compression {
        0 => opts.compression_level = CompressionLevel::Low,
        1 => opts.compression_level = CompressionLevel::Medium,
        2 => opts.compression_level = CompressionLevel::High,
        _ => {}
    }
    let _ = png_rust_encoder::encode_pixmap(&bm.pixmap(), &opts);
});

// Port of: fuzz/FuzzEncoders.cpp#L79-L86 (chrome/m156)
def_fuzz!(JPEGEncoder, fuzz_JPEGEncoder, |fuzz| {
    let bm = make_fuzzed_bitmap(fuzz);

    // `fQuality` is an `int`.
    let quality: i32 = fuzz.next_range(0, 100);
    let opts = jpeg_encoder::Options {
        quality: quality.cast_unsigned(), // in [0, 100]
        ..Default::default()
    };

    let _ = jpeg_encoder::encode_pixmap(&bm.pixmap(), &opts);
});

// Port of: fuzz/FuzzEncoders.cpp#L88-L102 (chrome/m156)
def_fuzz!(WEBPEncoder, fuzz_WEBPEncoder, |fuzz| {
    let bm = make_fuzzed_bitmap(fuzz);

    let quality = fuzz.next_range_f32(0.0, 100.0);
    let lossy = fuzz.next_bool();
    let opts = webp_encoder::Options {
        quality,
        compression: if lossy {
            webp_encoder::Compression::Lossy
        } else {
            webp_encoder::Compression::Lossless
        },
    };

    let _ = webp_encoder::encode_pixmap(&bm.pixmap(), &opts);
});
