// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkBmpRLECodec.cpp#L1-L589, src/codec/SkBmpRLECodec.h (chrome/m156)
// Ported from: src/codec/SkBmpRLECodec.cpp, src/codec/SkBmpRLECodec.h
//
// Deviation (documented): with a colour transform, Skia decodes RLE pixels straight into the
// destination for every colour type except RGBA F16, and then transforms them in place. That
// writes BGRA (4 bytes per pixel) into an RGB565 (2 bytes per pixel) buffer, which is a heap
// overflow in the C++. This port always decodes into a BGRA buffer when a transform is set, which
// is the path Skia already takes for F16. For 8888 destinations the results are identical, since
// the in-place and buffered transforms are the same per-pixel conversion.
//
// Not ported: sampling (`setSampleX`, `getSampler`, SkSampledCodec). The sample factor stays 1.

//! The BMP decoder for RLE8 and RLE4 files (and the 24-bit RLE form some JPEG-compressed BMPs use).

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::size::ISize;

use crate::codec::{CodecBase, CodecImpl, Options, Result, ScanlineOrder};
use crate::codec_priv::{
    get_sampled_dimension, get_start_coord, pack_argb_as_bgra, pack_argb_as_rgba, pack_argb32,
    pack888_to_rgb16, pixel32_to_pixel16,
};

use crate::sampler;

use super::{BmpBase, choose_pack_argb, compute_row_bytes, read_exact, rewind};

// Size of the buffer that holds the encoded RLE stream. Port of `kBufferSize`.
const BUFFER_SIZE: usize = 4096;

// The RLE escape codes. Port of the `RLE_*` constants in `decodeRLE`.
const RLE_ESCAPE: u8 = 0;
const RLE_EOL: u8 = 0;
const RLE_EOF: u8 = 1;
const RLE_DELTA: u8 = 2;

// Port of: src/codec/SkCodecPriv.h#L143-L144 and #L196-L198 (IsCoordNecessary, GetDstCoord): the
// coordinate is kept when it is on the sampling grid and inside the scaled width.
#[must_use]
fn is_coord_necessary(src_coord: i32, sample_factor: i32, scaled_dim: i32) -> bool {
    let start_coord = get_start_coord(sample_factor);
    if src_coord < start_coord || src_coord / sample_factor >= scaled_dim {
        return false;
    }
    (src_coord - start_coord) % sample_factor == 0
}

// Writes a native-endian u32 at byte offset `o`.
fn write_u32(dst: &mut [u8], o: usize, value: u32) {
    dst[o..o + 4].copy_from_slice(&value.to_ne_bytes());
}

// Writes a native-endian u16 at byte offset `o`.
fn write_u16(dst: &mut [u8], o: usize, value: u16) {
    dst[o..o + 2].copy_from_slice(&value.to_ne_bytes());
}

/// The RLE BMP decoder. Port of `SkBmpRLECodec` (the `SkBmpCodec` state is in `BmpBase`).
#[derive(Debug)]
#[doc(alias = "SkBmpRLECodec")]
pub(crate) struct BmpRleCodec {
    base: BmpBase,
    // Port of `fColorTable`: `None` for 24-bit RLE, which stores colours inline.
    color_table: Option<Vec<u32>>,
    // Port of `fNumColors`.
    num_colors: u32,
    // Port of `fBytesPerColor`.
    bytes_per_color: u32,
    // Port of `fOffset`: the pixel data offset from the end of the colour table.
    offset: u32,
    // Port of `fStreamBuffer`: the encoded RLE bytes read so far.
    stream_buffer: Vec<u8>,
    // Port of `fBytesBuffered`.
    bytes_buffered: usize,
    // Port of `fCurrRLEByte`: the next unread byte of `stream_buffer`.
    curr_rle_byte: usize,
    // Port of `fSampleX`: always 1 until sampling is ported.
    sample_x: i32,
    // Port of `fLinesToSkip`: rows a DELTA code jumped past, to skip on the next call.
    lines_to_skip: i32,
}

impl BmpRleCodec {
    // Port of: src/codec/SkBmpRLECodec.cpp#L18-L31 (the constructor)
    pub(crate) fn new(
        width: i32,
        bits_per_pixel: u16,
        num_colors: u32,
        bytes_per_color: u32,
        offset: u32,
        row_order: ScanlineOrder,
    ) -> Self {
        Self {
            base: BmpBase::new(width, bits_per_pixel, row_order),
            color_table: None,
            num_colors,
            bytes_per_color,
            offset,
            stream_buffer: vec![0u8; BUFFER_SIZE],
            bytes_buffered: 0,
            curr_rle_byte: 0,
            sample_x: 1,
            lines_to_skip: 0,
        }
    }

    // Port of: src/codec/SkBmpRLECodec.cpp#L85-L144 (createColorTable). Reads the colour table and
    // skips to the pixel data.
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

            // Fill in the colour table. Entries are BGR and opaque.
            let pack = choose_pack_argb(false, dst_color_type);
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
            // Fill the end of the table with black, as the standard decoder does.
            for color in colors.iter_mut().skip(num_colors_to_read as usize) {
                *color = pack_argb32(0xFF, 0, 0, 0);
            }
            self.color_table = Some(colors);
        }

        // Check that we have not read past the pixel array offset (see the standard decoder).
        if self.offset < color_bytes {
            return false;
        }

        // After reading the colour table, skip to the start of the pixel array.
        let to_skip = (self.offset - color_bytes) as usize;
        base.stream()
            .is_some_and(|stream| stream.skip(to_skip) == to_skip)
    }

    // Port of: src/codec/SkBmpRLECodec.cpp#L146-L154 (initializeStreamBuffer)
    fn initialize_stream_buffer(&mut self, base: &mut CodecBase<'_>) -> bool {
        self.bytes_buffered = base
            .stream()
            .map_or(0, |stream| stream.read(&mut self.stream_buffer));
        if self.bytes_buffered == 0 {
            return false;
        }
        self.curr_rle_byte = 0;
        true
    }

    // Port of: src/codec/SkBmpRLECodec.cpp#L156-L175 (checkForMoreData). Returns the number of
    // bytes in the buffer after reading more from the stream.
    fn check_for_more_data(&mut self, base: &mut CodecBase<'_>) -> usize {
        let remaining_bytes = self.bytes_buffered.saturating_sub(self.curr_rle_byte);
        // Move the unread bytes to the start of the buffer, then fill the rest from the stream.
        self.stream_buffer
            .copy_within(self.curr_rle_byte..self.curr_rle_byte + remaining_bytes, 0);
        let additional_bytes = match base.stream() {
            Some(stream) => stream.read(
                &mut self.stream_buffer[remaining_bytes..remaining_bytes + self.curr_rle_byte],
            ),
            None => 0,
        };
        self.curr_rle_byte = 0;
        self.bytes_buffered = remaining_bytes + additional_bytes;
        self.bytes_buffered
    }

    // Port of: src/codec/SkBmpRLECodec.cpp#L177-L199 (setPixel). Writes one colour-table pixel.
    #[allow(clippy::too_many_arguments)] // the C++ signature: destination, row, pixel and index
    #[allow(clippy::cast_sign_loss)] // rows and columns are non-negative: they come from the loops
    fn set_pixel(
        &self,
        dst: Option<&mut [u8]>,
        dst_row_bytes: usize,
        info: &ImageInfo,
        x: i32,
        y: i32,
        index: u8,
    ) {
        let Some(dst) = dst else {
            return;
        };
        if !is_coord_necessary(x, self.sample_x, info.width()) {
            return;
        }
        let row = self.base.get_dst_row(y, info.height()) as usize;
        let dst_x = (x / self.sample_x) as usize;
        let Some(table) = self.color_table.as_deref() else {
            return;
        };
        let color = table[index as usize];
        match info.color_type() {
            ColorType::RGBA8888 | ColorType::BGRA8888 => {
                write_u32(dst, row * dst_row_bytes + dst_x * 4, color);
            }
            ColorType::RGB565 => {
                write_u16(
                    dst,
                    row * dst_row_bytes + dst_x * 2,
                    pixel32_to_pixel16(color),
                );
            }
            // The colour type was checked when the conversion was accepted.
            _ => {}
        }
    }

    // Port of: src/codec/SkBmpRLECodec.cpp#L201-L234 (setRGBPixel). Writes one 24-bit pixel.
    #[allow(clippy::too_many_arguments)] // the C++ signature: destination, row, pixel and colour
    #[allow(clippy::cast_sign_loss)] // rows and columns are non-negative (see `set_pixel`)
    fn set_rgb_pixel(
        &self,
        dst: Option<&mut [u8]>,
        dst_row_bytes: usize,
        info: &ImageInfo,
        x: i32,
        y: i32,
        (red, green, blue): (u8, u8, u8),
    ) {
        let Some(dst) = dst else {
            return;
        };
        if !is_coord_necessary(x, self.sample_x, info.width()) {
            return;
        }
        let row = self.base.get_dst_row(y, info.height()) as usize;
        let dst_x = (x / self.sample_x) as usize;
        let (rv, gv, bv) = (u32::from(red), u32::from(green), u32::from(blue));
        match info.color_type() {
            ColorType::RGBA8888 => {
                write_u32(
                    dst,
                    row * dst_row_bytes + dst_x * 4,
                    pack_argb_as_rgba(0xFF, rv, gv, bv),
                );
            }
            ColorType::BGRA8888 => {
                write_u32(
                    dst,
                    row * dst_row_bytes + dst_x * 4,
                    pack_argb_as_bgra(0xFF, rv, gv, bv),
                );
            }
            ColorType::RGB565 => {
                write_u16(
                    dst,
                    row * dst_row_bytes + dst_x * 2,
                    pack888_to_rgb16(rv, gv, bv),
                );
            }
            _ => {}
        }
    }

    // Port of: src/codec/SkBmpRLECodec.cpp#L236-L254 (onPrepareToDecode)
    fn prepare_to_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        dst_info: &ImageInfo,
        options: &Options,
    ) -> Result {
        // Subsets are not supported.
        if options.subset.is_some() {
            return Result::Unimplemented;
        }

        // Reset the sample factor and the skipped rows. Sampling is not ported, so the factor
        // stays 1.
        self.sample_x = 1;
        self.lines_to_skip = 0;

        // With a colour transform the table is BGRA, which the transform reads.
        let color_table_color_type = if base.color_xform() {
            ColorType::BGRA8888
        } else {
            dst_info.color_type()
        };

        // Create the colour table and prepare the stream for decode.
        if !self.create_color_table(base, color_table_color_type) {
            return Result::InvalidInput;
        }

        // Initialize a buffer for the encoded RLE data.
        if !self.initialize_stream_buffer(base) {
            return Result::InvalidInput;
        }
        Result::Success
    }

    /// Port of `SkBmpRLECodec::decodeRows`. `dst` is `None` to skip rows. Returns the rows decoded.
    // Port of: src/codec/SkBmpRLECodec.cpp#L256-L318 (decodeRows)
    #[allow(clippy::cast_sign_loss)] // row and width counts are non-negative
    fn decode_rows(
        &mut self,
        base: &mut CodecBase<'_>,
        info: &ImageInfo,
        dst: Option<&mut [u8]>,
        dst_row_bytes: usize,
        options: &Options,
    ) -> i32 {
        let mut height = info.height();

        // Account for sampling.
        let fill_width = get_sampled_dimension(base.dimensions().width, self.sample_x);
        let mut dst_info = info.with_wh(fill_width, height);
        let mut dst = dst;

        // Set the background as transparent. Then, if the RLE code skips pixels, the skipped
        // pixels are transparent.
        if let Some(d) = dst.as_deref_mut() {
            sampler::fill(&dst_info, d, dst_row_bytes, options.zero_initialized);
        }

        // Adjust the height and the destination if the previous call left rows to skip.
        if height > self.lines_to_skip {
            height -= self.lines_to_skip;
            if let Some(d) = dst.take() {
                dst = Some(&mut d[self.lines_to_skip as usize * dst_row_bytes..]);
            }
            self.lines_to_skip = 0;
            dst_info = dst_info.with_wh(dst_info.width(), height);
        } else {
            self.lines_to_skip -= height;
            return height;
        }

        let Some(d) = dst else {
            // Skipping rows: decode with no destination.
            return self.decode_rle(base, &dst_info, None, dst_row_bytes);
        };
        if !base.color_xform() {
            return self.decode_rle(base, &dst_info, Some(d), dst_row_bytes);
        }

        // With a colour transform, decode BGRA into a buffer, then transform each row. A count
        // that does not fit an `int` (or is not positive) is rejected, as in Skia's F16 path.
        let width = dst_info.width();
        // The count is an `int` in Skia, so the product wraps the same way here.
        #[allow(clippy::cast_possible_truncation)]
        let count = (i64::from(height) * i64::from(width)) as i32;
        if count <= 0 {
            return 0;
        }
        let row_bytes = width as usize * 4;
        let mut xform_buffer = std::mem::take(&mut self.base.xform_buffer);
        xform_buffer.clear();
        xform_buffer.resize(count as usize * 4, 0);
        let decode_info = dst_info.with_color_type(ColorType::BGRA8888);
        let decoded_height = self.decode_rle(
            base,
            &decode_info,
            Some(xform_buffer.as_mut_slice()),
            row_bytes,
        );
        for y in 0..decoded_height as usize {
            let src_row = &xform_buffer[y * row_bytes..(y + 1) * row_bytes];
            let dst_row = &mut d[y * dst_row_bytes..];
            base.apply_color_xform(dst_row, src_row, width as usize);
        }
        self.base.xform_buffer = xform_buffer;
        decoded_height
    }

    // Port of: src/codec/SkBmpRLECodec.cpp#L320-L480 (decodeRLE). RLE decoding runs over the whole
    // image in one call; `dst` is `None` to skip the pixels.
    #[allow(clippy::too_many_lines)] // the C++ loop, one arm per RLE code
    #[allow(clippy::cast_sign_loss)] // coordinates and counts are non-negative
    fn decode_rle(
        &mut self,
        base: &mut CodecBase<'_>,
        info: &ImageInfo,
        mut dst: Option<&mut [u8]>,
        dst_row_bytes: usize,
    ) -> i32 {
        // Use the original width to count the number of pixels in each row.
        let width = base.dimensions().width;
        // The number of rows to decode.
        let height = info.height();
        let bits_per_pixel = self.base.bits_per_pixel;

        let mut x: i32 = 0;
        let mut y: i32 = 0;
        loop {
            // If we have reached a row beyond the requested height, we have succeeded. It would be
            // better to check for the EOF marker first, but a scanline decode may stop early.
            if y >= height {
                return height;
            }

            // Every entry takes at least two bytes.
            if self.bytes_buffered.saturating_sub(self.curr_rle_byte) < 2
                && self.check_for_more_data(base) < 2
            {
                return y;
            }

            // Read the next two bytes. The first is an escape flag or a pixel count; the second is
            // the task for an escape, or the colour for a repeat.
            let flag = self.stream_buffer[self.curr_rle_byte];
            let task = self.stream_buffer[self.curr_rle_byte + 1];
            self.curr_rle_byte += 2;

            if flag == RLE_ESCAPE {
                match task {
                    RLE_EOL => {
                        x = 0;
                        y += 1;
                    }
                    RLE_EOF => return height,
                    RLE_DELTA => {
                        // Two bytes are needed to specify the delta.
                        if self.bytes_buffered.saturating_sub(self.curr_rle_byte) < 2
                            && self.check_for_more_data(base) < 2
                        {
                            return y;
                        }
                        // Modify x and y.
                        let dx = self.stream_buffer[self.curr_rle_byte];
                        let dy = self.stream_buffer[self.curr_rle_byte + 1];
                        self.curr_rle_byte += 2;
                        x += i32::from(dx);
                        y += i32::from(dy);
                        if x > width {
                            // Invalid input: stop before the rest of the row.
                            return y - i32::from(dy);
                        } else if y > height {
                            self.lines_to_skip = y - height;
                            return height;
                        }
                    }
                    _ => {
                        // Any other task is a run of non-RLE pixels, and the task is their count.
                        let num_pixels = task;
                        let row_bytes =
                            compute_row_bytes(i32::from(num_pixels), u32::from(bits_per_pixel));
                        // Words are aligned to two bytes in the file.
                        let aligned_row_bytes = (row_bytes + 1) & !1;
                        if self.bytes_buffered.saturating_sub(self.curr_rle_byte)
                            < aligned_row_bytes
                            && self.check_for_more_data(base) < aligned_row_bytes
                        {
                            return y;
                        }

                        // Set `num_pixels` pixels.
                        let mut remaining = num_pixels;
                        while remaining > 0 && x < width {
                            match bits_per_pixel {
                                4 => {
                                    let val = self.stream_buffer[self.curr_rle_byte];
                                    self.curr_rle_byte += 1;
                                    self.set_pixel(
                                        dst.as_deref_mut(),
                                        dst_row_bytes,
                                        info,
                                        x,
                                        y,
                                        val >> 4,
                                    );
                                    x += 1;
                                    remaining -= 1;
                                    if remaining != 0 {
                                        self.set_pixel(
                                            dst.as_deref_mut(),
                                            dst_row_bytes,
                                            info,
                                            x,
                                            y,
                                            val & 0xF,
                                        );
                                        x += 1;
                                        remaining -= 1;
                                    }
                                }
                                8 => {
                                    let index = self.stream_buffer[self.curr_rle_byte];
                                    self.curr_rle_byte += 1;
                                    self.set_pixel(
                                        dst.as_deref_mut(),
                                        dst_row_bytes,
                                        info,
                                        x,
                                        y,
                                        index,
                                    );
                                    x += 1;
                                    remaining -= 1;
                                }
                                24 => {
                                    let blue = self.stream_buffer[self.curr_rle_byte];
                                    let green = self.stream_buffer[self.curr_rle_byte + 1];
                                    let red = self.stream_buffer[self.curr_rle_byte + 2];
                                    self.curr_rle_byte += 3;
                                    self.set_rgb_pixel(
                                        dst.as_deref_mut(),
                                        dst_row_bytes,
                                        info,
                                        x,
                                        y,
                                        (red, green, blue),
                                    );
                                    x += 1;
                                    remaining -= 1;
                                }
                                _ => return y,
                            }
                        }

                        // Skip a byte if necessary to keep the run aligned.
                        if !row_bytes.is_multiple_of(2) {
                            self.curr_rle_byte += 1;
                        }
                    }
                }
            } else {
                // A nonzero flag is a repeat count.
                let end_x = (x + i32::from(flag)).min(width);
                if bits_per_pixel == 24 {
                    // In RLE24 the task byte is the blue part of the colour, and two more bytes
                    // finish it.
                    if self.bytes_buffered.saturating_sub(self.curr_rle_byte) < 2
                        && self.check_for_more_data(base) < 2
                    {
                        return y;
                    }
                    let blue = task;
                    let green = self.stream_buffer[self.curr_rle_byte];
                    let red = self.stream_buffer[self.curr_rle_byte + 1];
                    self.curr_rle_byte += 2;
                    while x < end_x {
                        self.set_rgb_pixel(
                            dst.as_deref_mut(),
                            dst_row_bytes,
                            info,
                            x,
                            y,
                            (red, green, blue),
                        );
                        x += 1;
                    }
                } else {
                    // In RLE8 the task is the colour index. In RLE4 it holds two indices, which
                    // alternate.
                    let mut indices = [task, task];
                    if bits_per_pixel == 4 {
                        indices[0] >>= 4;
                        indices[1] &= 0xF;
                    }
                    let mut which = 0usize;
                    while x < end_x {
                        self.set_pixel(
                            dst.as_deref_mut(),
                            dst_row_bytes,
                            info,
                            x,
                            y,
                            indices[which],
                        );
                        which = 1 - which;
                        x += 1;
                    }
                }
            }
        }
    }

    // Port of: src/codec/SkBmpRLECodec.cpp#L570-L576 (SkBmpRLECodec::skipRows: decodes with no
    // destination, so the rows are consumed without being written)
    fn skip_rows(&mut self, base: &mut CodecBase<'_>, count: i32, options: &Options) -> bool {
        let width = base.dimensions().width;
        let row_info = ImageInfo::new(
            ISize::new(width, count),
            ColorType::N32,
            AlphaType::Unpremul,
            None,
        );
        count == self.decode_rows(base, &row_info, None, 0, options)
    }
}

impl CodecImpl for BmpRleCodec {
    // Port of: src/codec/SkBmpCodec.h (onGetEncodedFormat)
    fn on_get_encoded_format(&self) -> EncodedImageFormat {
        EncodedImageFormat::BMP
    }

    // Port of: src/codec/SkBmpRLECodec.cpp#L51-L73 (onGetPixels)
    fn on_get_pixels(
        &mut self,
        base: &mut CodecBase<'_>,
        info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        rows_decoded: &mut i32,
    ) -> Result {
        // Subsets are not supported.
        if options.subset.is_some() {
            return Result::Unimplemented;
        }

        let result = self.prepare_to_decode(base, info, options);
        if result != Result::Success {
            return result;
        }

        // Perform the decode.
        let rows = self.decode_rows(base, info, Some(dst), row_bytes, options);
        if rows != info.height() {
            // The background is already filled, and RLE may skip pixels, so every row counts as
            // decoded.
            *rows_decoded = info.height();
            return Result::IncompleteInput;
        }
        Result::Success
    }

    // Port of: src/codec/SkBmpCodec.cpp#L604-L608 (onRewind)
    fn on_rewind(&mut self, base: &mut CodecBase<'_>) -> bool {
        rewind(base)
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
        let options = base.options().clone();
        self.decode_rows(base, &row_info, Some(dst), row_bytes, &options)
    }

    // Port of: src/codec/SkBmpRLECodec.cpp#L570-L576 (skipRows, through onSkipScanlines)
    fn on_skip_scanlines(&mut self, base: &mut CodecBase<'_>, count: i32) -> bool {
        let options = base.options().clone();
        self.skip_rows(base, count, &options)
    }
}
