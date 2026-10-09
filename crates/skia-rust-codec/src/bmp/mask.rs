// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkBmpMaskCodec.cpp#L1-L113, src/codec/SkBmpMaskCodec.h (chrome/m156)
// Ported from: src/codec/SkBmpMaskCodec.cpp, src/codec/SkBmpMaskCodec.h
//
// `getSampler` returns the mask swizzler, which samples for SkSampledCodec.

//! The BMP decoder for files whose pixels are packed by bit masks (16, 24 or 32 bits per pixel).

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::image_info::ImageInfo;

use crate::codec::{CodecBase, CodecImpl, Options, Result, ScanlineOrder};
use crate::mask_swizzler::MaskSwizzler;
use crate::sampler::Sampler;

use crate::masks::Masks;

use super::{BmpBase, read_exact, rewind};

/// The bit-mask BMP decoder. Port of `SkBmpMaskCodec` (with its `SkBmpBaseCodec` source buffer).
#[derive(Debug)]
#[doc(alias = "SkBmpMaskCodec")]
pub(crate) struct BmpMaskCodec {
    base: BmpBase,
    // Port of `SkBmpBaseCodec::fSrcBuffer`: one padded row of the file.
    src_buffer: Vec<u8>,
    // Port of `fMasks`.
    masks: Masks,
    // Port of `fMaskSwizzler`, made by `onPrepareToDecode`.
    mask_swizzler: Option<MaskSwizzler>,
}

impl BmpMaskCodec {
    // Port of: src/codec/SkBmpMaskCodec.cpp#L14-L22 (the constructor)
    pub(crate) fn new(
        width: i32,
        bits_per_pixel: u16,
        masks: Masks,
        row_order: ScanlineOrder,
    ) -> Self {
        let base = BmpBase::new(width, bits_per_pixel, row_order);
        let src_buffer = vec![0u8; base.src_row_bytes];
        Self {
            base,
            src_buffer,
            masks,
            mask_swizzler: None,
        }
    }

    // Port of: src/codec/SkBmpMaskCodec.cpp#L58-L76 (onPrepareToDecode)
    #[allow(clippy::cast_sign_loss)] // widths are positive (checked by the header reader)
    fn prepare_to_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        dst_info: &ImageInfo,
        options: &Options,
    ) -> Result {
        if base.color_xform() {
            self.base.xform_buffer = vec![0u8; dst_info.width() as usize * 4];
        }

        // A colour transform reads BGRA, so the swizzler writes BGRA for it.
        let mut swizzler_info = dst_info.clone();
        if base.color_xform() {
            swizzler_info = swizzler_info.with_color_type(ColorType::BGRA8888);
            if dst_info.alpha_type() == AlphaType::Premul {
                swizzler_info = swizzler_info.with_alpha_type(AlphaType::Unpremul);
            }
        }

        let src_is_opaque = base.encoded_info().opaque();
        self.mask_swizzler = MaskSwizzler::create(
            &swizzler_info,
            src_is_opaque,
            self.masks,
            u32::from(self.base.bits_per_pixel),
            options,
        );
        if self.mask_swizzler.is_none() {
            return Result::InternalError;
        }
        Result::Success
    }

    // Port of: src/codec/SkBmpMaskCodec.cpp#L93-L113 (decodeRows). Returns the rows decoded.
    #[allow(clippy::cast_sign_loss)] // row indices and widths are non-negative
    fn decode_rows(
        &mut self,
        base: &mut CodecBase<'_>,
        info: &ImageInfo,
        dst: &mut [u8],
        dst_row_bytes: usize,
    ) -> i32 {
        let height = info.height();
        let Some(swizzler) = self.mask_swizzler.as_ref() else {
            return 0;
        };
        let width = swizzler.swizzle_width() as usize;
        // Iterate over the rows of the image.
        for y in 0..height {
            // Read a row of the input.
            if !read_exact(base, &mut self.src_buffer) {
                return y;
            }

            // Decode the row in the destination format.
            let row = self.base.get_dst_row(y, height);
            let dst_row = &mut dst[row as usize * dst_row_bytes..];
            if base.color_xform() {
                swizzler.swizzle(&mut self.base.xform_buffer, &self.src_buffer);
                base.apply_color_xform(dst_row, &self.base.xform_buffer, width);
            } else {
                swizzler.swizzle(dst_row, &self.src_buffer);
            }
        }
        height
    }
}

impl CodecImpl for BmpMaskCodec {
    // Port of: src/codec/SkBmpMaskCodec.h#L59-L62 (getSampler)
    fn on_get_sampler(
        &mut self,
        _base: &CodecBase<'_>,
        _create_if_necessary: bool,
    ) -> Option<&mut dyn Sampler> {
        self.mask_swizzler
            .as_mut()
            .map(|swizzler| swizzler as &mut dyn Sampler)
    }

    // Port of: src/codec/SkBmpCodec.h (onGetEncodedFormat)
    fn on_get_encoded_format(&self) -> EncodedImageFormat {
        EncodedImageFormat::BMP
    }

    // Port of: src/codec/SkBmpMaskCodec.cpp#L30-L56 (onGetPixels)
    fn on_get_pixels(
        &mut self,
        base: &mut CodecBase<'_>,
        info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        rows_decoded: &mut i32,
    ) -> Result {
        if options.subset.is_some() {
            // Subsets are not supported.
            return Result::Unimplemented;
        }
        if info.dimensions() != base.dimensions() {
            return Result::InvalidScale;
        }

        let result = self.prepare_to_decode(base, info, options);
        if result != Result::Success {
            return result;
        }
        let rows = self.decode_rows(base, info, dst, row_bytes);
        if rows != info.height() {
            *rows_decoded = rows;
            return Result::IncompleteInput;
        }
        Result::Success
    }

    // Port of: src/codec/SkBmpCodec.cpp#L604-L608 (onRewind)
    fn on_rewind(&mut self, base: &mut CodecBase<'_>) -> bool {
        rewind(base, false)
    }

    // Port of: src/codec/SkBmpCodec.cpp#L636-L639 (onGetScanlineOrder, via the base's fRowOrder)
    fn on_get_scanline_order(&self) -> ScanlineOrder {
        self.base.row_order
    }

    // Port of: src/codec/SkBmpCodec.cpp#L652-L655 (onStartScanlineDecode: prepareToDecode)
    fn on_start_scanline_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        dst_info: &ImageInfo,
        options: &Options,
    ) -> Result {
        self.prepare_to_decode(base, dst_info, options)
    }

    // Port of: src/codec/SkBmpCodec.cpp#L657-L664 (onGetScanlines)
    fn on_get_scanlines(
        &mut self,
        base: &mut CodecBase<'_>,
        dst: &mut [u8],
        count: i32,
        row_bytes: usize,
    ) -> i32 {
        let row_info = base.dst_info().with_wh(base.dst_info().width(), count);
        self.decode_rows(base, &row_info, dst, row_bytes)
    }

    // Port of: src/codec/SkBmpCodec.cpp#L694-L696 (onSkipScanlines)
    fn on_skip_scanlines(&mut self, base: &mut CodecBase<'_>, count: i32) -> bool {
        self.base.skip_rows(base, count)
    }
}
