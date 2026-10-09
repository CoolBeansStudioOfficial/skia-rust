// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/encode/SkPngEncoderImpl.{h,cpp} (chrome/m156): `SkPngEncoderMgr` (the libpng
// write struct and its header, colour space, text and write steps) and `SkPngEncoderImpl` (the
// row and finish steps). libpng is the port in skia-rust-libpng.
//
// The HDR metadata and gainmap chunks (`SkPngEncoder::Options::fHdrMetadata`, `fGainmap`,
// `fGainmapInfo`) are not ported: their chunks are not written.

// Clippy: a line-by-line port of Skia's C++; the casts follow the C++ conversions.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::sync::{Arc, Mutex};

use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_libpng::{
    PNG_HANDLE_CHUNK_ALWAYS, PngColor8, PngInfo, PngResult, PngStruct, TextCompression,
    create_info_struct,
};

use crate::encode::icc::write_icc_profile;
use crate::encode::png_encoder::Options;
use crate::encode::png_encoder_base::{PngEncoderBase, PngRowSink, TargetInfo};
use crate::encoded_info::Color;

/// Port of `kGraySigBit_GrayAlphaIsJustAlpha` (SkPngPriv.h#L17): the gray significant-bit value
/// that marks an alpha-only image stored as gray-alpha.
const K_GRAY_SIG_BIT_GRAY_ALPHA_IS_JUST_ALPHA: u8 = 1;

/// Port of `PNG_sRGB_INTENT_PERCEPTUAL` (png.h).
const PNG_SRGB_INTENT_PERCEPTUAL: u8 = 0;

/// Port of `PNG_COLOR_TYPE_*` (png.h): the colour types of the PNG header.
const PNG_COLOR_TYPE_GRAY: u8 = 0;
const PNG_COLOR_TYPE_RGB: u8 = 2;
const PNG_COLOR_TYPE_RGB_ALPHA: u8 = 6;
const PNG_COLOR_TYPE_GRAY_ALPHA: u8 = 4;

/// Port of `PNG_INTERLACE_NONE` and `PNG_COMPRESSION_TYPE_BASE`, `PNG_FILTER_TYPE_BASE` (png.h).
const PNG_INTERLACE_NONE: u8 = 0;
const PNG_COMPRESSION_TYPE_BASE: u8 = 0;
const PNG_FILTER_TYPE_BASE: u8 = 0;

/// Port of `PNG_KEYWORD_MAX_LENGTH` (png.h): longer keywords are clipped.
const PNG_KEYWORD_MAX_LENGTH: usize = 79;

/// Port of `SkPngEncoderMgr` (SkPngEncoderImpl.cpp#L70-L100): the libpng write struct, its info,
/// and the output the write callback fills.
pub(crate) struct PngEncoderMgr {
    png: PngStruct,
    info: PngInfo,
}

impl PngEncoderMgr {
    /// Port of `SkPngEncoderMgr::Make` (SkPngEncoderImpl.cpp#L115-L130). The output goes to `out`.
    #[must_use]
    pub(crate) fn make(out: Arc<Mutex<Vec<u8>>>) -> Self {
        let png = PngStruct::new_write(Box::new(move |data: &[u8]| match out.lock() {
            Ok(mut buf) => {
                buf.extend_from_slice(data);
                true
            }
            Err(_) => false,
        }));
        PngEncoderMgr {
            png,
            info: create_info_struct(),
        }
    }

    /// Port of `SkPngEncoderMgr::setHeader` (SkPngEncoderImpl.cpp#L132-L240): the header, the
    /// significant bits, the filters, the zlib level and the text comments.
    pub(crate) fn set_header(
        &mut self,
        target_info: &TargetInfo,
        src_info: &ImageInfo,
        options: &Options,
    ) -> PngResult<()> {
        let dst_info = &target_info.dst_info;
        let png_color_type = match dst_info.color() {
            Color::RGB => PNG_COLOR_TYPE_RGB,
            Color::RGBA => {
                let opaque = target_info
                    .dst_row_info
                    .as_ref()
                    .is_some_and(ImageInfo::is_opaque);
                if opaque {
                    PNG_COLOR_TYPE_RGB
                } else {
                    PNG_COLOR_TYPE_RGB_ALPHA
                }
            }
            Color::Gray => PNG_COLOR_TYPE_GRAY,
            Color::GrayAlpha => PNG_COLOR_TYPE_GRAY_ALPHA,
            _ => {
                return Err(
                    self.png_error("`getTargetInfo` returned unexpected `SkEncodedInfo::Color`")
                );
            }
        };

        let (sig_bit, sig_bit_set) = sig_bit_for(src_info.color_type());

        self.png.set_ihdr(
            &mut self.info,
            src_info.width() as u32,
            src_info.height() as u32,
            dst_info.bits_per_component(),
            png_color_type,
            PNG_INTERLACE_NONE,
            PNG_COMPRESSION_TYPE_BASE,
            PNG_FILTER_TYPE_BASE,
        )?;
        if sig_bit_set {
            self.png.set_sbit(&mut self.info, sig_bit);
        }

        let filters = options.filter_flags.bits() & FILTER_ALL;
        self.png.set_filter(0, filters.cast_signed())?;

        let zlib_level = options.z_lib_level.clamp(0, 9);
        self.png.set_compression_level(zlib_level);

        // Comments go to the tEXt chunks, as keyword and text pairs.
        for comment in &options.comments {
            let mut key = comment.keyword.as_bytes();
            if key.len() > PNG_KEYWORD_MAX_LENGTH {
                key = &key[..PNG_KEYWORD_MAX_LENGTH];
            }
            self.png.set_text(
                &mut self.info,
                key,
                comment.text.as_bytes(),
                TextCompression::None,
            );
        }
        Ok(())
    }

    /// Port of `SkPngEncoderMgr::setColorSpace` (SkPngEncoderImpl.cpp#L242-L252) with `set_icc`
    /// (SkPngEncoderImpl.cpp#L254-L266): sRGB is the sRGB chunk, any other space its ICC profile.
    pub(crate) fn set_color_space(&mut self, info: &ImageInfo) {
        let cs = info.color_space();
        match cs {
            Some(cs) if cs.is_srgb() => {
                self.png
                    .set_srgb(&mut self.info, PNG_SRGB_INTENT_PERCEPTUAL);
            }
            cs => {
                if let Some(icc) = write_icc_profile(cs.as_ref()) {
                    self.png.set_iccp(&mut self.info, "Skia", 0, &icc);
                }
            }
        }
    }

    /// Port of `SkPngEncoderMgr::setHdrMetadata` (SkPngEncoderImpl.cpp#L268-L290), for the keep
    /// list that Skia sets unconditionally. The HDR chunks themselves are not ported.
    pub(crate) fn set_hdr_metadata(&mut self) {
        self.png.set_keep_unknown_chunks(
            PNG_HANDLE_CHUNK_ALWAYS,
            &[b"gmAP", b"gdAT", b"mDCV", b"cLLI"],
        );
    }

    /// Port of `SkPngEncoderMgr::writeInfo` (SkPngEncoderImpl.cpp#L292-L309): writes the header,
    /// and for an opaque RGBA source makes libpng drop the filler channel.
    pub(crate) fn write_info(&mut self, target_info: &TargetInfo) -> PngResult<()> {
        self.png.write_info(&self.info)?;
        if target_info.dst_info.color() == Color::RGBA {
            let opaque = target_info
                .dst_row_info
                .as_ref()
                .is_some_and(ImageInfo::is_opaque);
            if opaque {
                self.png.set_filler(0, true);
            }
        }
        Ok(())
    }

    /// Port of the `png_error` text used for the one error the mgr creates itself.
    fn png_error(&mut self, msg: &str) -> skia_rust_libpng::PngError {
        self.png.error(msg)
    }
}

/// The significant bits of `SkPngEncoderMgr::setHeader` (SkPngEncoderImpl.cpp#L152-L205), per
/// source colour type. The flag is `false` for the types with no sBIT.
fn sig_bit_for(ct: ColorType) -> (PngColor8, bool) {
    let mut sb = PngColor8::default();
    let set = match ct {
        ColorType::RGBAF16Norm | ColorType::RGBAF16 | ColorType::RGBAF32 => {
            sb.red = 16;
            sb.green = 16;
            sb.blue = 16;
            sb.alpha = 16;
            true
        }
        ColorType::RGBF16F16F16x => {
            sb.red = 16;
            sb.green = 16;
            sb.blue = 16;
            true
        }
        ColorType::Gray8 => {
            sb.gray = 8;
            true
        }
        ColorType::RGB888x => {
            sb.red = 8;
            sb.green = 8;
            sb.blue = 8;
            true
        }
        ColorType::ARGB4444 => {
            sb.red = 4;
            sb.green = 4;
            sb.blue = 4;
            sb.alpha = 4;
            true
        }
        ColorType::RGB565 => {
            sb.red = 5;
            sb.green = 6;
            sb.blue = 5;
            true
        }
        // We store kAlpha_8 as gray+alpha, but ignore the gray.
        ColorType::Alpha8 => {
            sb.gray = K_GRAY_SIG_BIT_GRAY_ALPHA_IS_JUST_ALPHA;
            sb.alpha = 8;
            true
        }
        ColorType::RGBA1010102 | ColorType::BGRA1010102 => {
            sb.red = 10;
            sb.green = 10;
            sb.blue = 10;
            sb.alpha = 2;
            true
        }
        ColorType::BGR101010xXR | ColorType::RGB101010x | ColorType::BGR101010x => {
            sb.red = 10;
            sb.green = 10;
            sb.blue = 10;
            true
        }
        ColorType::BGRA10101010XR => {
            sb.red = 10;
            sb.green = 10;
            sb.blue = 10;
            sb.alpha = 10;
            true
        }
        ColorType::RGBA8888 | ColorType::BGRA8888 => {
            sb.red = 8;
            sb.green = 8;
            sb.blue = 8;
            sb.alpha = 8;
            true
        }
        _ => false,
    };
    (sb, set)
}

/// The `PNG_FILTER_*` mask of `SkPngEncoder::FilterFlag::kAll` (the bits a filter flag may use).
pub(crate) const FILTER_ALL: u32 = 0xf8;

/// Port of `SkPngEncoderImpl` (SkPngEncoderImpl.h): the base encoder and the libpng manager.
pub(crate) struct PngEncoderImpl<'a> {
    base: PngEncoderBase<'a>,
    mgr: PngEncoderMgr,
}

impl PngEncoderImpl<'_> {
    /// Port of `SkEncoder::encodeRows` on the PNG encoder, with the rows of the source.
    pub(crate) fn encode_rows(&mut self, num_rows: i32) -> bool {
        let mut sink = Sink(&mut self.mgr);
        self.base.encode_rows(num_rows, &mut sink)
    }
}

/// The row sink: `SkPngEncoderImpl::onEncodeRow` and `onFinishEncoding`.
struct Sink<'m>(&'m mut PngEncoderMgr);

impl PngRowSink for Sink<'_> {
    /// Port of `SkPngEncoderImpl::onEncodeRow` (SkPngEncoderImpl.cpp#L438-L452).
    fn on_encode_row(&mut self, row: &[u8]) -> bool {
        // Swap to big endian if we are storing more than a byte per color channel (SkColorTypes
        // are little endian by default). By this point the data is 8888 or 16161616.
        if self.0.info.bit_depth == 16 {
            self.0.png.set_swap();
        }
        self.0.png.write_rows(&[row]).is_ok()
    }

    /// Port of `SkPngEncoderImpl::onFinishEncoding` (SkPngEncoderImpl.cpp#L454-L461).
    fn on_finish_encoding(&mut self) -> bool {
        self.0.png.write_end(&self.0.info).is_ok()
    }
}

/// Port of `SkPixmapIsValid` (SkImageEncoderPriv.h): a usable pixmap has valid info, pixels and
/// row bytes at least one row.
fn pixmap_is_valid(src: &Pixmap<'_>) -> bool {
    if !skia_rust_core::image_info_priv::image_info_is_valid(src.info()) {
        return false;
    }
    if src.addr().is_none() || src.row_bytes() < src.info().min_row_bytes() {
        return false;
    }
    true
}

/// Port of `SkPngEncoder::Make` (SkPngEncoderImpl.cpp#L462-L490), with the output going to `out`.
// Port of: src/encode/SkPngEncoderImpl.cpp#L462-L490 (chrome/m156)
#[must_use]
pub(crate) fn make<'a>(
    out: Arc<Mutex<Vec<u8>>>,
    src: Pixmap<'a>,
    options: &Options,
) -> Option<PngEncoderImpl<'a>> {
    if !pixmap_is_valid(&src) {
        return None;
    }
    let target_info = crate::encode::png_encoder_base::get_target_info(src.info())?;
    let mut mgr = PngEncoderMgr::make(out);
    mgr.set_header(&target_info, src.info(), options).ok()?;
    mgr.set_color_space(src.info());
    mgr.set_hdr_metadata();
    mgr.write_info(&target_info).ok()?;
    Some(PngEncoderImpl {
        base: PngEncoderBase::new(target_info, src),
        mgr,
    })
}
