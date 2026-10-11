// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: experimental/rust_jpeg/encoder/SkJpegRustEncoder.cpp and SkJpegRustEncoder.h (chrome/m156),
// and of `encode_jpeg` in experimental/rust_jpeg/ffi/FFI.rs, the jpeg-encoder side of the Rust JPEG
// encoder. The cxx bridge is not ported: the pixel conversion and the encoder are called directly.

//! `SkJpegRustEncoder`: the JPEG encoder built on the `jpeg-encoder` crate.

use std::io;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_info_priv::image_info_is_valid;
use skia_rust_core::pixmap::Pixmap;

use crate::encode::jpeg_encoder::{AlphaOption, Options};

/// The pixel layouts that `encode_jpeg` reads. Port of `rust_jpeg::JpegEncodeColor`, without the
/// RGB variant that Skia's bridge never passes from the C++ side.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L58-L66 (JpegEncodeColor, chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JpegEncodeColor {
    Rgba,
    Bgra,
    Grayscale,
}

/// Alpha handling for the JPEG encoding (JPEG is always opaque). Port of
/// `rust_jpeg::JpegEncodeAlpha`.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L68-L74 (JpegEncodeAlpha, chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JpegEncodeAlpha {
    /// Ignore the alpha channel entirely.
    Ignore,
    /// Blend onto a black background before encoding.
    BlendOnBlack,
}

/// Port of `rust_jpeg::EncodingResult`.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L76-L81 (EncodingResult, chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EncodingResult {
    Success,
    ParameterError,
    OtherError,
}

/// Converts the pixels of `pixels` (rows `row_bytes` apart) to 8-bit RGB or greyscale and encodes
/// them as a JPEG into `output`. Port of `rust_jpeg::encode_jpeg`.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L858-L954 (encode_jpeg, chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ bridge's argument list
pub(crate) fn encode_jpeg(
    pixels: &[u8],
    width: u32,
    height: u32,
    row_bytes: u32,
    color_type: JpegEncodeColor,
    alpha_option: JpegEncodeAlpha,
    quality: u32,
    output: &mut Vec<u8>,
) -> EncodingResult {
    let src_bpp: usize = match color_type {
        JpegEncodeColor::Rgba | JpegEncodeColor::Bgra => 4,
        JpegEncodeColor::Grayscale => 1,
    };

    let row_bytes = row_bytes as usize;
    let wide = width as usize;
    let high = height as usize;
    if wide == 0 || high == 0 {
        return EncodingResult::ParameterError;
    }
    let Some(min_row) = wide.checked_mul(src_bpp) else {
        return EncodingResult::ParameterError;
    };
    if row_bytes < min_row {
        return EncodingResult::ParameterError;
    }
    match high.checked_mul(row_bytes) {
        Some(total) if pixels.len() >= total => {}
        _ => return EncodingResult::ParameterError,
    }

    let (Ok(w16), Ok(h16)) = (u16::try_from(width), u16::try_from(height)) else {
        return EncodingResult::ParameterError;
    };

    let (rgb_data, jpeg_color) = match color_type {
        JpegEncodeColor::Grayscale => {
            let mut data = Vec::with_capacity(wide * high);
            for y in 0..high {
                let row_start = y * row_bytes;
                data.extend_from_slice(&pixels[row_start..row_start + wide]);
            }
            (data, jpeg_encoder::ColorType::Luma)
        }
        JpegEncodeColor::Rgba | JpegEncodeColor::Bgra => {
            let swap_rb = color_type == JpegEncodeColor::Bgra;
            let blend_on_black = alpha_option == JpegEncodeAlpha::BlendOnBlack;
            let mut data = Vec::with_capacity(wide * high * 3);
            for y in 0..high {
                let row_start = y * row_bytes;
                for x in 0..wide {
                    let px = row_start + x * 4;
                    let (mut red, green, mut blue, alpha) =
                        (pixels[px], pixels[px + 1], pixels[px + 2], pixels[px + 3]);
                    if swap_rb {
                        std::mem::swap(&mut red, &mut blue);
                    }
                    if blend_on_black && alpha < 255 {
                        let alpha = u32::from(alpha);
                        data.push(blend_channel(red, alpha));
                        data.push(blend_channel(green, alpha));
                        data.push(blend_channel(blue, alpha));
                    } else {
                        data.push(red);
                        data.push(green);
                        data.push(blue);
                    }
                }
            }
            (data, jpeg_encoder::ColorType::Rgb)
        }
    };

    let quality = u8::try_from(quality.clamp(1, 100)).unwrap_or(100);

    let mut buf: Vec<u8> = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut buf, quality);

    match encoder.encode(&rgb_data, w16, h16, jpeg_color) {
        Ok(()) => {
            *output = buf;
            EncodingResult::Success
        }
        Err(_) => EncodingResult::OtherError,
    }
}

/// One channel of an unpremultiplied pixel blended onto black: `(c * a + 127) / 255`.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L940-L942 (the blend-on-black channel arithmetic)
fn blend_channel(channel: u8, alpha: u32) -> u8 {
    u8::try_from((u32::from(channel) * alpha + 127) / 255).unwrap_or(u8::MAX)
}

/// Port of `SkJpegRustEncoder::Encode(SkWStream*, ...)`: writes the JPEG bytes of `src` to `dst`.
/// Returns `false` if the encoding or the write fails.
// Port of: experimental/rust_jpeg/encoder/SkJpegRustEncoder.cpp#L17-L100 (Encode, chrome/m156)
#[doc(alias = "SkJpegRustEncoder::Encode")]
pub fn encode<W: io::Write>(dst: &mut W, src: &Pixmap<'_>, options: &Options) -> bool {
    match encode_to_vec(src, options) {
        Some(bytes) => dst.write_all(&bytes).is_ok(),
        None => false,
    }
}

/// Port of `SkJpegRustEncoder::Encode(const SkPixmap&, ...)`: the JPEG data of `src`.
// Port of: experimental/rust_jpeg/encoder/SkJpegRustEncoder.cpp (Encode, chrome/m156)
#[doc(alias = "SkJpegRustEncoder::Encode")]
#[must_use]
pub fn encode_pixmap(src: &Pixmap<'_>, options: &Options) -> Option<Data> {
    encode_to_vec(src, options).map(|bytes| Data::new_copy(&bytes))
}

/// The JPEG bytes of `src`, with the XMP packet inserted when there is one. Port of the body of
/// `SkJpegRustEncoder::Encode`.
// Port of: experimental/rust_jpeg/encoder/SkJpegRustEncoder.cpp#L23-L100 (Encode body, chrome/m156)
fn encode_to_vec(src: &Pixmap<'_>, options: &Options) -> Option<Vec<u8>> {
    let info: &ImageInfo = src.info();
    let pixels = src.addr()?;
    if !image_info_is_valid(info) || src.row_bytes() < info.min_row_bytes() {
        return None;
    }
    let width = u32::try_from(info.width()).ok()?;
    let height = u32::try_from(info.height()).ok()?;
    let src_row_bytes = u32::try_from(src.row_bytes()).ok()?;
    let total_bytes = usize::try_from(height).ok()?.checked_mul(src.row_bytes())?;

    let color_type = match info.color_type() {
        // kRGB_888x is four bytes per pixel with the fourth ignored, so it is read as RGBA with
        // alpha ignored, and the row stride is the same four-byte stride.
        ColorType::RGB888x | ColorType::RGBA8888 => JpegEncodeColor::Rgba,
        ColorType::BGRA8888 => JpegEncodeColor::Bgra,
        ColorType::Gray8 => JpegEncodeColor::Grayscale,
        _ => return None,
    };

    let has_unpremul_alpha = matches!(info.color_type(), ColorType::RGBA8888 | ColorType::BGRA8888)
        && info.alpha_type() == AlphaType::Unpremul;
    let alpha_option = if options.alpha_option == AlphaOption::BlendOnBlack && has_unpremul_alpha {
        JpegEncodeAlpha::BlendOnBlack
    } else {
        JpegEncodeAlpha::Ignore
    };

    let quality = options.quality.min(100);
    let mut output = Vec::new();
    let result = encode_jpeg(
        &pixels[..total_bytes.min(pixels.len())],
        width,
        height,
        src_row_bytes,
        color_type,
        alpha_option,
        quality,
        &mut output,
    );
    if result != EncodingResult::Success {
        return None;
    }

    // The jpeg-encoder crate writes a complete JPEG. The XMP packet goes into an APP1 segment
    // right after the SOI and the first APP marker (the JFIF APP0 segment).
    match options.xmp_metadata.as_deref() {
        Some(xmp) if !xmp.is_empty() => insert_xmp(&output, xmp.as_bytes()),
        _ => Some(output),
    }
}

/// The XMP namespace, with its NUL terminator, as `SkJpegRustEncoder` writes it.
// Port of: experimental/rust_jpeg/encoder/SkJpegRustEncoder.cpp (kXmpNamespace, chrome/m156)
const XMP_NAMESPACE: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

/// Inserts an XMP APP1 segment into the JPEG `data`, after the first APP marker. Port of the XMP
/// insertion in `SkJpegRustEncoder::Encode`. `None` if the segment does not fit the 16-bit length.
// Port of: experimental/rust_jpeg/encoder/SkJpegRustEncoder.cpp#L47-L97 (XMP insertion, chrome/m156)
fn insert_xmp(data: &[u8], xmp: &[u8]) -> Option<Vec<u8>> {
    let mut insert_pos = 2; // After SOI (FF D8).
    if data.len() > 5 && data[2] == 0xFF && (data[3] & 0xF0) == 0xE0 {
        // The length field at data[4..6] includes its own two bytes.
        let seg_len = (usize::from(data[4]) << 8) | usize::from(data[5]);
        let candidate = 2 + 2 + seg_len;
        if candidate <= data.len() {
            insert_pos = candidate;
        }
    }

    // APP1 XMP segment: marker FF E1, length (2 + namespace + xmp), namespace with its NUL.
    let app1_payload_size = 2 + XMP_NAMESPACE.len() + xmp.len();
    let app1_len = u16::try_from(app1_payload_size).ok()?;

    let mut out = Vec::with_capacity(data.len() + 4 + app1_payload_size);
    out.extend_from_slice(&data[..insert_pos]);
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&app1_len.to_be_bytes());
    out.extend_from_slice(XMP_NAMESPACE);
    out.extend_from_slice(xmp);
    out.extend_from_slice(&data[insert_pos..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_jpeg_rgba_roundtrip() {
        let pixels: Vec<u8> = vec![
            255, 0, 0, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255,
        ];
        let mut output = Vec::new();
        let result = encode_jpeg(
            &pixels,
            2,
            2,
            8,
            JpegEncodeColor::Rgba,
            JpegEncodeAlpha::Ignore,
            90,
            &mut output,
        );
        assert_eq!(result, EncodingResult::Success);
        assert_eq!(&output[..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn test_encode_jpeg_rejects_invalid_row_bytes() {
        let pixels = vec![0u8; 16];
        let mut output = Vec::new();
        let result = encode_jpeg(
            &pixels,
            2,
            2,
            1,
            JpegEncodeColor::Rgba,
            JpegEncodeAlpha::Ignore,
            90,
            &mut output,
        );
        assert_eq!(result, EncodingResult::ParameterError);
    }

    #[test]
    fn test_encode_jpeg_rejects_oversize_dimensions() {
        let pixels = vec![0u8; 4];
        let mut output = Vec::new();
        let result = encode_jpeg(
            &pixels,
            u32::MAX,
            1,
            u32::MAX,
            JpegEncodeColor::Rgba,
            JpegEncodeAlpha::Ignore,
            90,
            &mut output,
        );
        assert_eq!(result, EncodingResult::ParameterError);
    }

    #[test]
    fn test_blend_channel_rounds_to_nearest() {
        assert_eq!(blend_channel(255, 255), 255);
        assert_eq!(blend_channel(255, 0), 0);
        assert_eq!(blend_channel(200, 128), 100);
    }
}
