// Copyright 2010 The Android Open Source Project
// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/encode/SkWebpEncoder.h and src/encode/SkWebpEncoderImpl.cpp (chrome/m156), with
// the skia-safe API shape (`webp_encoder::{encode, encode_pixmap, encode_image, Options,
// Compression}`).
//
// Ported: the lossless encoder (`Compression::Lossless`, libwebp's VP8L at `method 0`, as
// `SkWebpEncoderImpl` sets it), and the lossy encoder (`Compression::Lossy`, libwebp's VP8 at
// `method 3`, `WebPConfigPreset(DEFAULT, quality)`), including the ALPH chunk of pictures with
// transparency, through `skia_rust_libwebp::enc`.
//
// The ICC profile of the colour space is embedded as an `ICCP` chunk by `skia_rust_libwebp::mux`
// (libwebp's WebPMux), as `SkWebpEncoder::Encode` does.
//
// The animated encoder (`encode_animated`, WebPAnimEncoder) is ported for lossless frames. Lossy
// animation is not ported and reports `false`. The GPU-backed `EncodeImage` is not ported (no
// `DirectContext` type in this port; `encode_image` reads the raster pixels of the image).

use std::io;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::convert_pixels::convert_pixels;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_info_priv::color_type_is_alpha_only;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_libwebp::enc::vp8_lossy::encode_lossy;
use skia_rust_libwebp::enc::{encode_lossless, encode_lossless_method};
use skia_rust_libwebp::mux::Mux;
use skia_rust_libwebp::mux::anim_encode::{ArgbPicture, FrameConfig, anim_encoder_new};

use crate::encode::icc::write_icc_profile;

/// Port of `SkWebpEncoder::Compression`: lossy or lossless WebP.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub enum Compression {
    /// The VP8 lossy encoder (libwebp `method 3`), for pictures without transparency.
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

/// `SkPixmap` pixels as unpremultiplied `RGBA_8888` bytes, the way `preprocess_webp_picture`
/// imports them. Returns `None` for alpha-only or unreadable pixmaps.
fn rgba_pixels(src: &Pixmap<'_>) -> Option<Vec<u8>> {
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
    Some(rgba)
}

/// `SkPixmap` pixels as ARGB words (`0xAARRGGBB`), read as unpremultiplied `RGBA_8888`.
/// Returns `None` for alpha-only or unreadable pixmaps.
fn argb_pixels(src: &Pixmap<'_>) -> Option<Vec<u32>> {
    let rgba = rgba_pixels(src)?;
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
    // SkWebpEncoder::Encode: the ICC profile of the colour space, if it has one.
    let icc = write_icc_profile(src.color_space().as_ref());
    let encoded = encode_picture(src, options)?;
    let Some(icc) = icc else {
        return Some(encoded);
    };
    // libwebp needs an encoded image before a profile can be added: WebPMuxSetImage, then
    // WebPMuxSetChunk("ICCP") and WebPMuxAssemble.
    let mut mux = Mux::new();
    mux.set_image(&encoded).ok()?;
    mux.set_chunk(*b"ICCP", &icc).ok()?;
    mux.assemble().ok()
}

/// The WebP bitstream of the pixels of `src`, without the ICC profile (`WebPEncode`).
fn encode_picture(src: &Pixmap<'_>, options: &Options) -> Option<Vec<u8>> {
    let info = src.info();
    let width = usize::try_from(info.width()).ok()?;
    let height = usize::try_from(info.height()).ok()?;
    if options.compression == Compression::Lossy {
        // SkWebpEncoderImpl: the RGBA import (use_argb = 0), with the quality as the preset's.
        let rgba = rgba_pixels(src)?;
        return encode_lossy(&rgba, width, height, options.quality, false);
    }
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

/// Port of `SkEncoder::Frame`: the pixels of one frame of an animation and its duration (ms).
#[derive(Debug)]
pub struct Frame<'a> {
    /// The frame's pixels.
    pub pixmap: Pixmap<'a>,
    /// The frame's duration in milliseconds.
    pub duration: i32,
}

/// Encodes `frames` as an animated WebP file into `writer`. Returns `true` on success.
///
/// Port of `SkWebpEncoder::EncodeAnimated`: each frame goes through the same preprocessing and
/// `WebPEncode` as a still image (its transparent pixels replaced), then `WebPAnimEncoderAdd`
/// with its timestamp, and `WebPAnimEncoderAssemble` at the end. Lossy frames are not ported
/// (the lossy candidates of the animation encoder), so a lossy `options` returns `false`.
#[must_use]
pub fn encode_animated<W: io::Write>(
    writer: &mut W,
    frames: &[Frame<'_>],
    options: &Options,
) -> bool {
    let Some(first) = frames.first() else {
        return false;
    };
    let first_info = first.pixmap.info();
    let canvas_width = first_info.width();
    let canvas_height = first_info.height();
    let Some(mut enc) = anim_encoder_new(canvas_width, canvas_height, None) else {
        return false;
    };
    if options.compression != Compression::Lossless || !(0.0..=100.0).contains(&options.quality) {
        return false;
    }
    let mut timestamp: i32 = 0;
    for frame in frames {
        let info = frame.pixmap.info();
        if info.width() != canvas_width || info.height() != canvas_height {
            return false;
        }
        let Some(mut argb) = argb_pixels(&frame.pixmap) else {
            return false;
        };
        // preprocess_webp_picture: VP8L at method 0, use_argb, and the WebPEncode of the picture,
        // whose WebPReplaceTransparentPixels changes the pixels the animation encoder then reads.
        let config = FrameConfig {
            lossless: true,
            method: 0,
            quality: options.quality,
            exact: false,
        };
        let (Ok(w), Ok(h)) = (
            usize::try_from(canvas_width),
            usize::try_from(canvas_height),
        ) else {
            return false;
        };
        // WebPEncode's output is not used here; a failure of the encode fails the animation.
        // The quality is in `0.0..=100.0` (checked above): the truncation is libwebp's `(int)`.
        #[allow(clippy::cast_possible_truncation)]
        let quality = options.quality as i32;
        if !argb_replace_transparent_then_encode(&mut argb, w, h, quality) {
            return false;
        }
        let picture = ArgbPicture {
            width: canvas_width,
            height: canvas_height,
            argb,
        };
        if enc.add(Some(&picture), timestamp, Some(config)).is_err() {
            return false;
        }
        timestamp = timestamp.wrapping_add(frame.duration);
    }
    // Add a last fake frame to signal the last duration.
    if enc.add(None, timestamp, None).is_err() {
        return false;
    }
    match enc.assemble() {
        Ok(data) => writer.write_all(&data).is_ok(),
        Err(_) => false,
    }
}

/// `WebPReplaceTransparentPixels(pic, 0x000000)` on `argb` (`WebPEncode` with `exact = 0`), then
/// the lossless encode of the picture (`VP8LEncodeImage`). Returns `false` where it fails.
fn argb_replace_transparent_then_encode(
    argb: &mut [u32],
    width: usize,
    height: usize,
    quality: i32,
) -> bool {
    for px in argb.iter_mut() {
        if (*px >> 24) == 0 {
            *px = 0;
        }
    }
    encode_lossless_method(width, height, argb, 0, quality, true).is_some()
}
