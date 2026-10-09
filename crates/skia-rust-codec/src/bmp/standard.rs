// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkBmpStandardCodec.cpp#L1-L357, src/codec/SkBmpStandardCodec.h (chrome/m156)
// Ported from: src/codec/SkBmpStandardCodec.cpp, src/codec/SkBmpStandardCodec.h
//
// Not ported: `getSampler` (SkSampledCodec).

//! The BMP decoder for uncompressed files: a colour table for 1 to 8 bits per pixel, or direct
//! 24- and 32-bit pixels.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::stream::{MemoryStream, Stream};

use crate::codec::{CodecBase, CodecImpl, Options, Result, ScanlineOrder, ZeroInitialized};
use crate::codec_priv::{get_sampled_dimension, get_start_coord, pack_argb32};
use crate::encoded_info::{Alpha, Color, EncodedInfo};
use crate::sampler::Sampler;
use crate::swizzler::Swizzler;

use super::{BmpBase, choose_pack_argb, read_exact, rewind};

/// The standard BMP decoder. Port of `SkBmpStandardCodec` (with its `SkBmpBaseCodec` source
/// buffer).
#[derive(Debug)]
#[doc(alias = "SkBmpStandardCodec")]
pub(crate) struct BmpStandardCodec {
    base: BmpBase,
    // Port of `SkBmpBaseCodec::fSrcBuffer`: one padded row of the file.
    src_buffer: Vec<u8>,
    // Port of `fColorTable`: `None` for direct (more than 8 bits per pixel) images.
    color_table: Option<Vec<u32>>,
    // Port of `fNumColors`: the count in the header, or 0 when the header has none.
    num_colors: u32,
    // Port of `fBytesPerColor`.
    bytes_per_color: u32,
    // Port of `fOffset`: the pixel data offset from the end of the colour table.
    offset: u32,
    // Port of `fSwizzler`, made by `initializeSwizzler`.
    swizzler: Option<Swizzler>,
    // Port of `fIsOpaque`: whether the image is opaque (an ICO's 32-bit image is not).
    is_opaque: bool,
    // Port of `fInIco`: the image is a BMP embedded in an ICO, which has an AND mask after it.
    in_ico: bool,
    // Port of `fAndMaskRowBytes`: one row of the AND mask (only used for `in_ico`).
    and_mask_row_bytes: usize,
}

// Port of: src/codec/SkBmpStandardCodec.cpp#L107-L118 (constructor: the swizzler and table are set
// up later, in onPrepareToDecode)
impl BmpStandardCodec {
    #[allow(clippy::too_many_arguments)] // one per constructor argument of SkBmpStandardCodec
    pub(crate) fn new(
        width: i32,
        bits_per_pixel: u16,
        num_colors: u32,
        bytes_per_color: u32,
        offset: u32,
        row_order: ScanlineOrder,
        is_opaque: bool,
        in_ico: bool,
    ) -> Self {
        let base = BmpBase::new(width, bits_per_pixel, row_order);
        let src_buffer = vec![0u8; base.src_row_bytes];
        Self {
            base,
            src_buffer,
            color_table: None,
            num_colors,
            bytes_per_color,
            offset,
            swizzler: None,
            is_opaque,
            in_ico,
            and_mask_row_bytes: if in_ico {
                compute_and_mask_row_bytes(width)
            } else {
                0
            },
        }
    }

    // Port of: src/codec/SkBmpStandardCodec.cpp#L197-L220 (swizzlerInfo). An ICO's palette and
    // 24-bit images are read as they are stored, not as the file's own colour kinds.
    #[allow(clippy::cast_possible_truncation)] // bits per pixel is at most 8 on the palette path
    fn swizzler_info(&self, encoded: &EncodedInfo) -> EncodedInfo {
        if self.in_ico {
            if self.base.bits_per_pixel <= 8 {
                return EncodedInfo::make(
                    0,
                    0,
                    Color::Palette,
                    encoded.alpha(),
                    // Bits per pixel is at most 8 on this path.
                    self.base.bits_per_pixel as u8,
                );
            }
            if self.base.bits_per_pixel == 24 {
                return EncodedInfo::make(0, 0, Color::BGR, Alpha::Opaque, 8);
            }
        }
        EncodedInfo::make(
            0,
            0,
            encoded.color(),
            encoded.alpha(),
            encoded.bits_per_component(),
        )
    }

    // Port of: src/codec/SkBmpStandardCodec.cpp#L124-L195 (createColorTable)
    fn create_color_table(&mut self, base: &mut CodecBase<'_>, dst_color_type: ColorType) -> bool {
        let bits_per_pixel = self.base.bits_per_pixel;
        let mut color_bytes: u32 = 0;
        self.color_table = None;
        if bits_per_pixel <= 8 {
            // Inform the caller of the number of colours.
            let max_colors = 1u32 << bits_per_pixel;
            // Don't bother reading more than max_colors.
            let num_colors_to_read = if self.num_colors == 0 {
                max_colors
            } else {
                self.num_colors.min(max_colors)
            };

            // Read the colour table from the stream.
            color_bytes = num_colors_to_read * self.bytes_per_color;
            let mut c_buffer = vec![0u8; color_bytes as usize];
            if !read_exact(base, &mut c_buffer) {
                return false;
            }

            // A colour transform packs BGRA, which the transform reads. The table is never
            // premultiplied: the codec is opaque, so the C++ `isPremul` is always false.
            let pack_color_type = if base.color_xform() {
                ColorType::BGRA8888
            } else {
                dst_color_type
            };
            let pack = choose_pack_argb(false, pack_color_type);

            // Fill in the colour table. Entries are BGR, and the alpha is always opaque.
            let mut colors = vec![0u32; max_colors as usize];
            for (i, color) in colors
                .iter_mut()
                .take(num_colors_to_read as usize)
                .enumerate()
            {
                let o = i * self.bytes_per_color as usize;
                let blue = c_buffer[o];
                let green = c_buffer[o + 1];
                let red = c_buffer[o + 2];
                *color = pack(0xFF, u32::from(red), u32::from(green), u32::from(blue));
            }
            // To avoid segmentation faults on bad pixel data, fill the end of the colour table with
            // black. This is the same as the Chromium decoder.
            for color in colors.iter_mut().skip(num_colors_to_read as usize) {
                *color = pack_argb32(0xFF, 0, 0, 0);
            }

            if base.color_xform() && !base.xform_on_decode() {
                xform_palette(base, &mut colors);
            }
            self.color_table = Some(colors);
        }

        // A BMP in an ICO has no offset: its pixel data begins right after the colour table.
        if self.in_ico {
            return true;
        }

        // Check that we have not read past the pixel array offset. This may occur on OS/2 1.x and
        // other old versions where the table defaults to the maximum size and the file uses a
        // smaller one. Skia rejects that rather than guessing the size.
        if self.offset < color_bytes {
            return false;
        }

        // After reading the colour table, skip to the start of the pixel array.
        let to_skip = (self.offset - color_bytes) as usize;
        base.stream()
            .is_some_and(|stream| stream.skip(to_skip) == to_skip)
    }

    // Port of: src/codec/SkBmpStandardCodec.cpp#L222-L240 (initializeSwizzler). The swizzler reads
    // the file's own colour type; with a colour transform on decode it writes BGRA instead.
    fn initialize_swizzler(
        &mut self,
        base: &CodecBase<'_>,
        dst_info: &ImageInfo,
        options: &Options,
    ) {
        let mut swizzler_info = dst_info.clone();
        let mut swizzler_options = options.clone();
        if base.xform_on_decode() {
            swizzler_info = swizzler_info.with_color_type(ColorType::BGRA8888);
            if dst_info.alpha_type() == AlphaType::Premul {
                swizzler_info = swizzler_info.with_alpha_type(AlphaType::Unpremul);
            }
            swizzler_options.zero_initialized = ZeroInitialized::No;
        }

        let encoded = self.swizzler_info(base.encoded_info());
        self.swizzler = Swizzler::make(
            &encoded,
            self.color_table.as_deref(),
            &swizzler_info,
            &swizzler_options,
            None,
        );
    }

    // Port of: src/codec/SkBmpStandardCodec.cpp#L242-L263 (onPrepareToDecode)
    #[allow(clippy::cast_sign_loss)] // widths are positive (checked by the header reader)
    fn prepare_to_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        dst_info: &ImageInfo,
        options: &Options,
    ) -> Result {
        if base.xform_on_decode() {
            self.base.xform_buffer = vec![0u8; dst_info.width() as usize * 4];
        }

        // Create the colour table if necessary and prepare the stream for decode.
        if !self.create_color_table(base, dst_info.color_type()) {
            return Result::InvalidInput;
        }

        // Initialize a swizzler.
        self.initialize_swizzler(base, dst_info, options);
        if self.swizzler.is_none() {
            return Result::InternalError;
        }
        Result::Success
    }

    // Port of: src/codec/SkBmpStandardCodec.cpp#L294-L331 (decodeRows). Returns the rows decoded.
    #[allow(clippy::cast_sign_loss)] // row indices and widths are non-negative
    fn decode_rows(
        &mut self,
        base: &mut CodecBase<'_>,
        info: &ImageInfo,
        dst: &mut [u8],
        dst_row_bytes: usize,
    ) -> i32 {
        let height = info.height();
        let width = info.width() as usize;
        let Some(swizzler) = self.swizzler.as_ref() else {
            return 0;
        };
        // Iterate over the rows of the image.
        for y in 0..height {
            // Read a row of the input.
            if !read_exact(base, &mut self.src_buffer) {
                return y;
            }

            // Decode the row in the destination format.
            let row = self.base.get_dst_row(y, height);
            let dst_row = &mut dst[row as usize * dst_row_bytes..];
            if base.xform_on_decode() {
                swizzler.swizzle(&mut self.base.xform_buffer, &self.src_buffer);
                base.apply_color_xform(dst_row, &self.base.xform_buffer, width);
            } else {
                swizzler.swizzle(dst_row, &self.src_buffer);
            }
        }

        // A BMP in an ICO has an AND mask after the colour rows, which makes pixels transparent.
        if self.in_ico && self.is_opaque {
            let width_full = base.dimensions().width;
            let start_scanline = base.curr_scanline();
            if start_scanline < 0 {
                // Not a scanline decode: decode the whole ICO mask.
                if let Some(stream) = base.stream() {
                    self.decode_ico_mask(stream, width_full, info, dst, dst_row_bytes);
                }
                return height;
            }

            // A scanline decode must skip ahead to the AND mask rows it needs. An ICO's stream is
            // always a memory stream, so the mask is read from its memory.
            let full_height = base.dimensions().height;
            let Some(stream) = base.stream() else {
                return height;
            };
            let length = stream.get_length();
            let curr_position = stream.get_position();
            let Some(memory_base) = stream.get_memory_base() else {
                return height;
            };

            // Calculate how many bytes must be skipped to reach the AND mask.
            let remaining_scanlines = full_height - start_scanline - height;
            let bytes_to_skip = remaining_scanlines as usize * self.base.src_row_bytes
                + start_scanline as usize * self.and_mask_row_bytes;
            let sub_start = curr_position + bytes_to_skip;
            if sub_start >= length {
                // Skia cannot report an incomplete decode here either.
                return height;
            }
            let Some(mask_bytes) = memory_base.get(sub_start..length) else {
                return height;
            };
            let mut sub_stream = MemoryStream::make_copy(mask_bytes);
            self.decode_ico_mask(&mut *sub_stream, width_full, info, dst, dst_row_bytes);
        }
        height
    }

    // Port of: src/codec/SkBmpStandardCodec.cpp#L333-L370 (decodeIcoMask). Clears the destination
    // pixels whose AND-mask bit is set, reading the mask one row at a time from `stream`.
    #[allow(clippy::cast_sign_loss)] // widths, rows and sample positions are non-negative
    fn decode_ico_mask(
        &mut self,
        stream: &mut dyn Stream,
        width: i32,
        info: &ImageInfo,
        dst: &mut [u8],
        dst_row_bytes: usize,
    ) {
        // BMP in ICO have transparency, so this cannot be 565. The code below depends on the output
        // being a 32-bit or F16 pixel.
        let bytes_per_pixel = if info.color_type() == ColorType::RGBAF16 {
            8
        } else {
            4
        };

        // If we are sampling, mask only the sampled pixels. The rows are sampled by SkSampledCodec.
        let Some(sample_x) = self.swizzler.as_ref().map(Swizzler::sample_x) else {
            return;
        };
        let sampled_width = get_sampled_dimension(width, sample_x);
        let src_start_x = get_start_coord(sample_x);

        let and_row = self.and_mask_row_bytes;
        for y in 0..info.height() {
            // The source buffer is at least large enough for one AND mask row.
            if stream.read(&mut self.src_buffer[..and_row]) != and_row {
                // Skia prints a warning here and stops masking.
                return;
            }

            let row = self.base.get_dst_row(y, info.height());
            let dst_row_start = row as usize * dst_row_bytes;
            let mut src_x = src_start_x;
            for dst_x in 0..sampled_width {
                // Port of SkTDivMod(srcX, 8, &quotient, &modulus).
                let quotient = (src_x / 8) as usize;
                let modulus = src_x % 8;
                let shift = 7 - modulus as u32;
                let alpha_bit = (self.src_buffer[quotient] >> shift) & 0x1;
                // Skia clears the pixel with `dst &= bit - 1`: zero when the bit is set, unchanged
                // when it is not.
                if alpha_bit == 1 {
                    let start = dst_row_start + dst_x as usize * bytes_per_pixel;
                    dst[start..start + bytes_per_pixel].fill(0);
                }
                src_x += sample_x;
            }
        }
    }
}

// Port of the row size of an ICO's AND mask: one bit per pixel, padded to four bytes
// (`SkAlign4(SkCodecPriv::ComputeRowBytes(width, 1))`).
// Port of: src/codec/SkBmpStandardCodec.cpp#L47-L48 (fAndMaskRowBytes)
#[allow(clippy::cast_sign_loss)] // widths are positive (checked by the header reader)
fn compute_and_mask_row_bytes(width: i32) -> usize {
    (width as usize).div_ceil(8).next_multiple_of(4)
}

// Port of the `applyColorXform(colorTable, colorTable, maxColors)` call in createColorTable. The C++
// transforms the table in place; the copy here is the same per-pixel transform read from the old
// values, so the result is identical.
fn xform_palette(base: &CodecBase<'_>, colors: &mut [u32]) {
    let src: Vec<u8> = colors.iter().flat_map(|c| c.to_ne_bytes()).collect();
    let mut dst = vec![0u8; src.len()];
    base.apply_color_xform(&mut dst, &src, colors.len());
    let (chunks, _) = dst.as_chunks::<4>();
    for (color, bytes) in colors.iter_mut().zip(chunks) {
        *color = u32::from_ne_bytes(*bytes);
    }
}

impl CodecImpl for BmpStandardCodec {
    // Port of: src/codec/SkBmpStandardCodec.h#L72-L75 (getSampler)
    fn on_get_sampler(
        &mut self,
        _base: &CodecBase<'_>,
        _create_if_necessary: bool,
    ) -> Option<&mut dyn Sampler> {
        self.swizzler
            .as_mut()
            .map(|swizzler| swizzler as &mut dyn Sampler)
    }

    // Port of: src/codec/SkBmpCodec.h (onGetEncodedFormat)
    fn on_get_encoded_format(&self) -> EncodedImageFormat {
        EncodedImageFormat::BMP
    }

    // Port of: src/codec/SkBmpStandardCodec.cpp#L77-L101 (onGetPixels)
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
        rewind(base, self.in_ico)
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
        // Decode the requested rows, as an image of the full width and `count` rows.
        let row_info = base.dst_info().with_wh(base.dst_info().width(), count);
        self.decode_rows(base, &row_info, dst, row_bytes)
    }

    // Port of: src/codec/SkBmpCodec.cpp#L694-L696 (onSkipScanlines)
    fn on_skip_scanlines(&mut self, base: &mut CodecBase<'_>, count: i32) -> bool {
        self.base.skip_rows(base, count)
    }
}
