// Copyright 2010 The Android Open Source Project
// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/encode/SkWebpEncoder.h and src/encode/SkWebpEncoderImpl.cpp (chrome/m156), with
// the skia-safe API shape (`webp_encoder::{encode, encode_pixmap, encode_image, Options,
// Compression}`).
//
// Ported: the lossless encoder (`Compression::Lossless`, libwebp's VP8L at `method 0`, as
// `SkWebpEncoderImpl` sets it), through `skia_rust_libwebp::enc`.
//
// Not ported, and reported as `false` / `None` rather than encoded differently:
// - `Compression::Lossy`: the VP8 lossy encoder (libwebp `enc/`), which is not in
//   `skia-rust-libwebp` yet.
// - an ICC profile on the pixmap's colour space: the ICCP chunk is written by libwebp's WebPMux,
//   which is not ported. Without a colour space the file is the plain VP8L bitstream, as in Skia.
// - `EncodeAnimated` (WebPAnimEncoder) and the GPU-backed `EncodeImage` (no `DirectContext`
//   type in this port; `encode_image` reads the raster pixels of the image).

use std::io;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::convert_pixels::convert_pixels;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_info_priv::color_type_is_alpha_only;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_libwebp::enc::encode_lossless;

use crate::encode::icc::write_icc_profile;

/// Port of `SkWebpEncoder::Compression`: lossy or lossless WebP.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub enum Compression {
    /// The VP8 lossy encoder (not ported; see the module comment).
    #[default]
    Lossy,
    /// The VP8L lossless encoder.
    Lossless,
}

/// Port of `SkWebpEncoder::Options`: the compression, and the quality in `0.0..=100.0`. For lossy
/// the quality is the visual quality; for lossless it is the effort (the libwebp `quality`).
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Lossy or lossless compression. Default lossy.
    pub compression: Compression,
    /// The quality, `0.0..=100.0`. Default 100.
    pub quality: f32,
}

impl Default for Options {
    /// The defaults of `SkWebpEncoder::Options`.
    fn default() -> Self {
        Self {
            compression: Compression::Lossy,
            quality: 100.0,
        }
    }
}

/// `SkPixmap` pixels as ARGB words (`0xAARRGGBB`), read as unpremultiplied `RGBA_8888` the way
/// `preprocess_webp_picture` imports them. Returns `None` for alpha-only or unreadable pixmaps.
fn argb_pixels(src: &Pixmap<'_>) -> Option<Vec<u32>> {
    let info = src.info();
    if color_type_is_alpha_only(info.color_type()) {
        return None;
    }
    let width = usize::try_from(info.width()).ok()?;
    let height = usize::try_from(info.height()).ok()?;
    let pixels = src.addr()?;
    let dst_info = ImageInfo::new(
        (info.width(), info.height()),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let dst_rb = width * 4;
    let mut rgba = vec![0u8; dst_rb * height];
    if !convert_pixels(&dst_info, &mut rgba, dst_rb, info, pixels, src.row_bytes()) {
        return None;
    }
    Some(
        rgba.as_chunks::<4>()
            .0
            .iter()
            .map(|p| {
                (u32::from(p[3]) << 24)
                    | (u32::from(p[0]) << 16)
                    | (u32::from(p[1]) << 8)
                    | u32::from(p[2])
            })
            .collect(),
    )
}

/// Port of `SkWebpEncoder::Encode(SkPixmap)` into bytes: `None` where the encoder reports failure
/// or the configuration is one this port does not cover (see the module comment).
fn encode_to_vec(src: &Pixmap<'_>, options: &Options) -> Option<Vec<u8>> {
    if !(0.0..=100.0).contains(&options.quality) {
        return None;
    }
    match options.compression {
        Compression::Lossy => return None,
        Compression::Lossless => {}
    }
    if write_icc_profile(src.color_space().as_ref()).is_some() {
        return None;
    }
    let info = src.info();
    let width = usize::try_from(info.width()).ok()?;
    let height = usize::try_from(info.height()).ok()?;
    let argb = argb_pixels(src)?;
    // libwebp takes the quality as `(int)config->quality` once `WebPConfigPreset` has checked it.
    // The quality is in `0.0..=100.0` (checked above), so the truncation is libwebp's `(int)`.
    #[allow(clippy::cast_possible_truncation)]
    let quality = options.quality as i32;
    encode_lossless(width, height, &argb, quality, false)
}

/// Encodes the pixels of `src` as WebP into `writer`. Returns `true` on success, and `false` for
/// an invalid or unsupported pixmap or options, or a failed write.
///
/// Port of `SkWebpEncoder::Encode(SkWStream*, const SkPixmap&, const Options&)`.
#[must_use]
pub fn encode<W: io::Write>(pixmap: &Pixmap<'_>, writer: &mut W, options: &Options) -> bool {
    match encode_to_vec(pixmap, options) {
        Some(bytes) => writer.write_all(&bytes).is_ok(),
        None => false,
    }
}

/// Encodes the pixels of `src` as WebP and returns the bytes, or `None`.
///
/// Port of `SkWebpEncoder::Encode(const SkPixmap&, const Options&)`.
#[must_use]
pub fn encode_pixmap(src: &Pixmap<'_>, options: &Options) -> Option<Data> {
    encode_to_vec(src, options).map(|bytes| Data::new_copy(&bytes))
}

/// Encodes the pixels of `img` as WebP and returns the bytes, or `None`. The image must be
/// raster-backed: its pixels are read as a pixmap.
///
/// Port of `SkWebpEncoder::EncodeImage` for raster images.
#[must_use]
pub fn encode_image(img: &Image, options: &Options) -> Option<Data> {
    let bitmap = img.as_legacy_bitmap()?;
    let pixmap = bitmap.peek_pixels()?;
    encode_pixmap(&pixmap, options)
}
