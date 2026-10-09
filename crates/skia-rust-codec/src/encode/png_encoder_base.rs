// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/encode/SkPngEncoderBase.{h,cpp} and include/encode/SkEncoder.h, src/encode/SkEncoder.cpp
// (chrome/m156). The pixel conversion to the rows libpng is given, and the row loop that calls the
// PNG writer's `onEncodeRow` and `onFinishEncoding`.

// Clippy: a line-by-line port of Skia's C++; the casts follow the C++ conversions.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::convert_pixels::convert_pixels;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_info_priv::{color_type_max_bits_per_channel, color_type_num_channels};
use skia_rust_core::pixmap::Pixmap;

use crate::encode::image_encoder_fns::transform_scanline_a8_to_gray_alpha;
use crate::encoded_info::{Alpha, Color, EncodedInfo};

/// Port of `SkPngEncoderBase::TargetInfo` (SkPngEncoderBase.h#L27-L34): what the source rows are
/// converted to, and the description libpng needs.
#[derive(Debug, Clone)]
pub(crate) struct TargetInfo {
    /// Port of `fSrcRowInfo`: one row of the source, `None` for `kAlpha_8` (handled separately).
    pub(crate) src_row_info: Option<ImageInfo>,
    /// Port of `fDstRowInfo`: one row of the converted pixels.
    pub(crate) dst_row_info: Option<ImageInfo>,
    /// Port of `fDstInfo`: the colour and depth libpng writes.
    pub(crate) dst_info: EncodedInfo,
    /// Port of `fDstRowSize`: bytes in one converted row.
    pub(crate) dst_row_size: usize,
}

/// Port of `makeInfo` (SkPngEncoderBase.cpp#L26-L33): the alpha is opaque for gray and RGB.
fn make_info(src: &ImageInfo, color: Color, bits_per_component: u8) -> EncodedInfo {
    let alpha = if color == Color::Gray || color == Color::RGB {
        Alpha::Opaque
    } else {
        Alpha::Unpremul
    };
    EncodedInfo::make(src.width(), src.height(), color, alpha, bits_per_component)
}

/// Port of `makeTargetInfo` (SkPngEncoderBase.cpp#L49-L58).
fn make_target_info(
    dst_info: EncodedInfo,
    src_image_info: &ImageInfo,
    dst_ct: ColorType,
    dst_alpha: AlphaType,
) -> TargetInfo {
    let dst_row_info = ImageInfo::new((src_image_info.width(), 1), dst_ct, dst_alpha, None);
    let src_row_info = src_image_info.with_wh(src_image_info.width(), 1);
    let dst_row_size = dst_row_info.min_row_bytes();
    TargetInfo {
        src_row_info: Some(src_row_info),
        dst_row_info: Some(dst_row_info),
        dst_info,
        dst_row_size,
    }
}

/// Port of `makeAlpha8TargetInfo` (SkPngEncoderBase.cpp#L60-L77): an alpha-only source is written
/// as gray-alpha, with the gray channel zero, so there is no row conversion.
fn make_alpha8_target_info(dst_info: EncodedInfo) -> Option<TargetInfo> {
    let bits_per_pixel = usize::from(dst_info.bits_per_pixel());
    let bytes_per_pixel = bits_per_pixel / 8;
    let dst_row_size = (dst_info.width() as usize).checked_mul(bytes_per_pixel)?;
    Some(TargetInfo {
        src_row_info: None,
        dst_row_info: None,
        dst_info,
        dst_row_size,
    })
}

/// Port of `SkPngEncoderBase::getTargetInfo` (SkPngEncoderBase.cpp#L79-L157). Returns `None` for
/// the source types the PNG encoder does not support.
// Port of: src/encode/SkPngEncoderBase.cpp#L79-L157 (chrome/m156)
#[doc(alias = "getTargetInfo")]
#[must_use]
pub(crate) fn get_target_info(src_info: &ImageInfo) -> Option<TargetInfo> {
    let src_ct = src_info.color_type();
    let src_alpha = src_info.alpha_type();
    let num_channels = color_type_num_channels(src_ct);
    match num_channels {
        1 => {
            // kGray_SkColorChannelFlag: the only one-channel type with a gray channel is kGray_8.
            if src_ct == ColorType::Gray8 {
                return Some(make_target_info(
                    make_info(src_info, Color::Gray, 8),
                    src_info,
                    src_ct,
                    src_alpha,
                ));
            }
            // We support encoding kAlpha_8_SkColorType to GrayAlpha images and just ignore gray.
            // Otherwise, there is no sensible way to encode alpha only images.
            if src_ct == ColorType::Alpha8 {
                return make_alpha8_target_info(make_info(src_info, Color::GrayAlpha, 8));
            }
            None
        }
        3 => {
            if src_alpha == AlphaType::Unknown {
                return None;
            }
            let max_bits = color_type_max_bits_per_channel(src_ct);
            if max_bits <= 8 {
                Some(make_target_info(
                    make_info(src_info, Color::RGBA, 8),
                    src_info,
                    ColorType::RGB888x,
                    AlphaType::Opaque,
                ))
            } else if max_bits <= 32 {
                Some(make_target_info(
                    make_info(src_info, Color::RGBA, 16),
                    src_info,
                    ColorType::R16G16B16A16UNorm,
                    AlphaType::Opaque,
                ))
            } else {
                None
            }
        }
        4 => {
            if src_alpha == AlphaType::Unknown {
                return None;
            }
            let max_bits = color_type_max_bits_per_channel(src_ct);
            if max_bits <= 8 {
                if src_alpha == AlphaType::Opaque {
                    Some(make_target_info(
                        make_info(src_info, Color::RGBA, 8),
                        src_info,
                        ColorType::RGB888x,
                        AlphaType::Opaque,
                    ))
                } else {
                    Some(make_target_info(
                        make_info(src_info, Color::RGBA, 8),
                        src_info,
                        ColorType::RGBA8888,
                        AlphaType::Unpremul,
                    ))
                }
            } else if max_bits <= 32 {
                if src_alpha == AlphaType::Opaque {
                    Some(make_target_info(
                        make_info(src_info, Color::RGBA, 16),
                        src_info,
                        ColorType::R16G16B16A16UNorm,
                        AlphaType::Opaque,
                    ))
                } else {
                    Some(make_target_info(
                        make_info(src_info, Color::RGBA, 16),
                        src_info,
                        ColorType::R16G16B16A16UNorm,
                        AlphaType::Unpremul,
                    ))
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The per-row work the PNG writer does: `SkPngEncoderBase`'s virtual `onEncodeRow` and
/// `onFinishEncoding`.
pub(crate) trait PngRowSink {
    /// Port of `onEncodeRow`: writes one converted row.
    fn on_encode_row(&mut self, row: &[u8]) -> bool;
    /// Port of `onFinishEncoding`: writes the end of the PNG.
    fn on_finish_encoding(&mut self) -> bool;
}

/// Port of the `SkEncoder` base (SkEncoder.h, SkEncoder.cpp) and `SkPngEncoderBase`: the source
/// pixmap, the current row, and the row buffer that the conversion writes into.
pub(crate) struct PngEncoderBase<'a> {
    /// Port of `fSrc`.
    pub(crate) src: Pixmap<'a>,
    /// Port of `fCurrRow`.
    pub(crate) curr_row: i32,
    /// Port of `fStorage`: one converted row.
    storage: Vec<u8>,
    /// Port of `fTargetInfo`.
    pub(crate) target_info: TargetInfo,
    /// Port of `fFinishedEncoding`.
    finished_encoding: bool,
}

impl<'a> PngEncoderBase<'a> {
    /// Port of the `SkEncoder` constructor (SkEncoder.h) with `SkPngEncoderBase`'s constructor.
    pub(crate) fn new(target_info: TargetInfo, src: Pixmap<'a>) -> Self {
        let storage = vec![0u8; target_info.dst_row_size];
        PngEncoderBase {
            src,
            curr_row: 0,
            storage,
            target_info,
            finished_encoding: false,
        }
    }

    /// Port of `SkEncoder::encodeRows` (SkEncoder.cpp#L9-L28): encodes up to `num_rows` rows,
    /// and any further calls after a failure encode nothing.
    #[doc(alias = "encodeRows")]
    pub(crate) fn encode_rows(&mut self, num_rows: i32, sink: &mut impl PngRowSink) -> bool {
        let mut num_rows = num_rows;
        if num_rows <= 0 || self.curr_row >= self.src.height() {
            return false;
        }
        if self.curr_row + num_rows > self.src.height() {
            num_rows = self.src.height() - self.curr_row;
        }
        if !self.on_encode_rows(num_rows, sink) {
            // If we fail, short circuit any future calls.
            self.curr_row = self.src.height();
            return false;
        }
        true
    }

    /// Port of `SkPngEncoderBase::onEncodeRows` (SkPngEncoderBase.cpp#L162-L222).
    // Port of: src/encode/SkPngEncoderBase.cpp#L162-L222 (chrome/m156)
    fn on_encode_rows(&mut self, num_rows: i32, sink: &mut impl PngRowSink) -> bool {
        // https://www.w3.org/TR/png-3/#11IHDR says that "zero is an invalid value" for width and
        // height.
        if self.src.width() == 0 || self.src.height() == 0 {
            return false;
        }
        if num_rows < 0 {
            return false;
        }
        let mut num_rows = num_rows;
        while num_rows > 0 {
            if self.curr_row == self.src.height() {
                return false;
            }
            let width = self.src.width() as usize;
            let Some(src_row) = self.src.addr_at((0, self.curr_row)) else {
                return false;
            };
            if self.src.color_type() == ColorType::Alpha8 {
                // A special case: kAlpha_8 images are stored as GrayAlpha in png.
                transform_scanline_a8_to_gray_alpha(&mut self.storage, src_row, width);
            } else {
                let (Some(dst_row_info), Some(src_row_info)) = (
                    self.target_info.dst_row_info.as_ref(),
                    self.target_info.src_row_info.as_ref(),
                ) else {
                    return false;
                };
                let src_rb = src_row_info.min_row_bytes();
                if !convert_pixels(
                    dst_row_info,
                    &mut self.storage,
                    self.target_info.dst_row_size,
                    src_row_info,
                    src_row,
                    src_rb,
                ) {
                    return false;
                }
            }
            let row_len = self.target_info.dst_row_size;
            if !sink.on_encode_row(&self.storage[..row_len]) {
                return false;
            }
            self.curr_row += 1;
            num_rows -= 1;
        }
        if self.curr_row == self.src.height() && !self.finished_encoding {
            self.finished_encoding = true;
            return sink.on_finish_encoding();
        }
        true
    }
}
