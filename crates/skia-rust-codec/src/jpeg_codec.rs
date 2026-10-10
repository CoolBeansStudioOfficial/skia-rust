// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkJpegCodec.cpp (chrome/m156), src/codec/SkJpegCodec.h, src/codec/SkJpegConstants.h,
// the ICC part of src/codec/SkJpegMetadataDecoderImpl.cpp (read_metadata), and the memory source
// of src/codec/SkJpegSourceMgr.cpp.
//
// Deviations, recorded in docs/design/codecs.md:
// * The decoder (`fDecoderMgr->dinfo()`) persists across calls, as in Skia. It is `Send` now, so
//   `CodecImpl` can own it. It is replaced on a rewind, which reads the header again.
// * The encoded bytes are kept in memory (`MemSource`), so a rewind never reads the stream.
// * The orientation comes from the EXIF data through `exif` (`SkExif::Parse`).
// * YUV planes (`onQueryYUVAInfo`, `onGetYUVAPlanes`) are not ported yet.

//! The JPEG decoder (`SkJpegCodec`): whole-image and progressive decodes, native scaling by 1/8
//! steps, colour conversion, RGB565 and grayscale output, CMYK/YCCK through the swizzler, and
//! scanline decoding with subsets (`jpeg_crop_scanline` and `jpeg_skip_scanlines`).

use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::{MemoryStream, Stream};
use skia_rust_libjpeg::{
    ColorSpace as JColorSpace, ConsumeResult, Decompress, DitherMode, HeaderResult, JpegSource,
    SrcBuf,
};
use skia_rust_skcms::{IccProfile, PixelFormat};

use crate::codec::{Codec, CodecBase, CodecImpl, Options, Result};
use crate::encoded_info::{Alpha, Color, EncodedInfo};
use crate::exif;
use crate::sampler::Sampler;
use crate::swizzler::Swizzler;

// Port of: src/codec/SkJpegConstants.h#L14-L16 (kJpegSig's first and last bytes)
const JPEG_SIG: [u8; 3] = [0xFF, 0xD8, 0xFF];

// Port of: src/codec/SkJpegConstants.h#L30-L37 (kICCMarker, kICCSig, kICCMarkerHeaderSize)
const ICC_MARKER: i32 = 0xE0 + 2;
const ICC_SIG: &[u8; 12] = b"ICC_PROFILE\0";

// Port of: src/codec/SkJpegConstants.h#L55-L56 (kExifMarker, kExifSig)
const EXIF_MARKER: i32 = 0xE0 + 1;
const EXIF_SIG: &[u8; 5] = b"Exif\0";

// Port of: include/core/SkColorSpace.h `SkCodecs::ColorProfile::DataSpace` signatures, as read by
// `SkCodecs::ColorProfile::dataSpace()`: CMYK and GRAY colour data, RGB otherwise.
const SIGNATURE_CMYK: u32 = 0x434D_594B;
const SIGNATURE_GRAY: u32 = 0x4752_4159;

/// Port of `SkCodecs::ColorProfile::DataSpace`, the three values the JPEG decoder compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataSpace {
    Rgb,
    Gray,
    Cmyk,
}

// Port of: src/codec/SkCodecColorProfile.cpp#L128-L140 (ColorProfile::dataSpace)
fn data_space(profile: &IccProfile) -> DataSpace {
    match profile.data_color_space {
        SIGNATURE_CMYK => DataSpace::Cmyk,
        SIGNATURE_GRAY => DataSpace::Gray,
        _ => DataSpace::Rgb,
    }
}

/// The memory source (`skjpeg_source_mgr` with a memory stream): the whole file is in the buffer,
/// and asking for more data suspends the decoder.
// Port of: src/codec/SkJpegSourceMgr.cpp#L186-L260 (sk_init_mem_source, sk_skip_mem_input_data,
// sk_fill_mem_input_buffer)
struct MemSource {
    data: Arc<[u8]>,
}

impl JpegSource for MemSource {
    fn init_source(&mut self, buf: &mut SrcBuf) {
        buf.data = self.data.to_vec();
        buf.next = 0;
        buf.bytes_in_buffer = buf.data.len();
    }

    fn fill_input_buffer(&mut self, _buf: &mut SrcBuf) -> bool {
        // The whole JPEG data is in memory, so asking for more is a suspension.
        false
    }

    fn skip_input_bytes(&mut self, bytes_to_skip: usize, buf: &mut SrcBuf) -> bool {
        if bytes_to_skip > buf.bytes_in_buffer {
            buf.next = buf.data.len();
            buf.bytes_in_buffer = 0;
        } else {
            buf.next += bytes_to_skip;
            buf.bytes_in_buffer -= bytes_to_skip;
        }
        true
    }
}

/// A decoder over `data` after its header has been read (`read_header(kNo)` in `SkJpegCodec.cpp`).
/// Errors map to the codec results that `SkJpegCodec` returns for them.
// Port of: src/codec/SkJpegCodec.cpp#L51-L77 (read_header, without saving markers)
fn header_decoder(data: &Arc<[u8]>) -> std::result::Result<Decompress, Result> {
    let mut decoder = Decompress::new(Box::new(MemSource {
        data: Arc::clone(data),
    }));
    match decoder.read_header(true) {
        Ok(HeaderResult::Ok) => Ok(decoder),
        Ok(HeaderResult::Suspended) => Err(Result::IncompleteInput),
        _ => Err(Result::InvalidInput),
    }
}

/// The header's decoder with the EXIF and ICC markers saved (`read_header(kYes)`).
// Port of: src/codec/SkJpegCodec.cpp#L51-L77 (read_header, saving the EXIF, ICC and MPF markers;
// MPF shares its marker code with ICC and is not ported)
fn header_decoder_saving_markers(data: &Arc<[u8]>) -> std::result::Result<Decompress, Result> {
    let mut decoder = Decompress::new(Box::new(MemSource {
        data: Arc::clone(data),
    }));
    if decoder.save_markers(EXIF_MARKER, 0xFFFF).is_err()
        || decoder.save_markers(ICC_MARKER, 0xFFFF).is_err()
    {
        return Err(Result::InvalidInput);
    }
    match decoder.read_header(true) {
        Ok(HeaderResult::Ok) => Ok(decoder),
        Ok(HeaderResult::Suspended) => Err(Result::IncompleteInput),
        _ => Err(Result::InvalidInput),
    }
}

/// The output dimensions for a scale of `num / 8` (`calc_output_dimensions` on a decoder whose
/// header was read).
// Port of: src/codec/SkJpegCodec.cpp#L244-L258 (calc_output_dimensions, onGetScaledDimensions)
fn output_dimensions(data: &Arc<[u8]>, num: u32) -> Option<(u32, u32)> {
    let mut decoder = header_decoder(data).ok()?;
    decoder.scale_num = num;
    decoder.scale_denom = 8;
    decoder.calc_output_dimensions().ok()?;
    Some((decoder.output_width, decoder.output_height))
}

/// The native scale (in eighths) that gives `dim`, or `None`. Port of the search in
/// `SkJpegCodec::onDimensionsSupported`, which tries 8/8 and steps down.
// Port of: src/codec/SkJpegCodec.cpp#L340-L375 (onDimensionsSupported)
fn scale_for_dimensions(data: &Arc<[u8]>, dim: ISize) -> Option<u32> {
    let dst_w = i64::from(dim.width);
    let dst_h = i64::from(dim.height);
    let mut num: u32 = 8;
    let (mut out_w, mut out_h) = output_dimensions(data, num)?;
    while i64::from(out_w) != dst_w || i64::from(out_h) != dst_h {
        // Return a failure if we have tried all of the possible scales.
        if num == 1 || dst_w > i64::from(out_w) || dst_h > i64::from(out_h) {
            return None;
        }
        num -= 1;
        (out_w, out_h) = output_dimensions(data, num)?;
    }
    Some(num)
}

/// The scale (in eighths) for a desired scale factor. Port of `SkJpegCodec::onGetScaledDimensions`'s
/// table, which rounds to the nearest size libjpeg can produce.
// Port of: src/codec/SkJpegCodec.cpp#L224-L242 (onGetScaledDimensions, the scale choice)
fn scale_num_for(desired_scale: f32) -> u32 {
    if desired_scale >= 0.9375 {
        8
    } else if desired_scale >= 0.8125 {
        7
    } else if desired_scale >= 0.6875 {
        6
    } else if desired_scale >= 0.5625 {
        5
    } else if desired_scale >= 0.4375 {
        4
    } else if desired_scale >= 0.3125 {
        3
    } else if desired_scale >= 0.1875 {
        2
    } else {
        1
    }
}

/// The colour space libjpeg outputs for a destination colour type, or `None` when the conversion is
/// not supported. Port of the switch in `SkJpegCodec::conversionSupported`.
// Port of: src/codec/SkJpegCodec.cpp#L291-L355 (conversionSupported)
fn out_color_space(
    color_type: ColorType,
    needs_color_xform: bool,
    encoded: JColorSpace,
) -> Option<JColorSpace> {
    let mut out = match color_type {
        ColorType::RGBA8888 | ColorType::RGBAF16 | ColorType::BGR101010xXR => JColorSpace::ExtRgba,
        ColorType::BGRA8888 => {
            if needs_color_xform {
                // Colour xforms take RGBA input.
                JColorSpace::ExtRgba
            } else {
                JColorSpace::ExtBgra
            }
        }
        ColorType::RGB565 => {
            if needs_color_xform {
                JColorSpace::ExtRgba
            } else {
                JColorSpace::Rgb565
            }
        }
        ColorType::Gray8 => {
            if encoded != JColorSpace::Grayscale {
                return None;
            }
            if needs_color_xform {
                JColorSpace::ExtRgba
            } else {
                JColorSpace::Grayscale
            }
        }
        _ => return None,
    };
    // libjpeg-turbo does not convert CMYK to RGBA, so the swizzler (or the colour xform) does.
    if matches!(encoded, JColorSpace::Cmyk | JColorSpace::Ycck) {
        out = JColorSpace::Cmyk;
    }
    Some(out)
}

/// The bytes per output row of libjpeg's output colour space (`get_row_bytes`).
// Port of: src/codec/SkJpegCodec.cpp#L124-L131 (get_row_bytes)
fn out_row_bytes(out: JColorSpace, width: u32) -> usize {
    let bytes_per_pixel = match out {
        JColorSpace::Rgb565 => 2,
        JColorSpace::Grayscale => 1,
        _ => 4,
    };
    width as usize * bytes_per_pixel
}

/// Whether the swizzler is needed to convert CMYK output. Port of
/// `needs_swizzler_to_convert_from_cmyk`.
// Port of: src/codec/SkJpegCodec.cpp#L467-L479 (needs_swizzler_to_convert_from_cmyk)
fn needs_swizzler_from_cmyk(out: JColorSpace, profile: Option<&IccProfile>, xform: bool) -> bool {
    if out != JColorSpace::Cmyk {
        return false;
    }
    let has_cmyk_color_space = profile.is_some_and(|p| data_space(p) == DataSpace::Cmyk);
    !has_cmyk_color_space || !xform
}

/// A dimension as a count: a negative value is zero.
fn non_negative(value: i32) -> usize {
    usize::try_from(value).unwrap_or(0)
}

/// Whether `rows` decoded rows fall short of `height`.
fn rows_short(rows: i32, height: usize) -> bool {
    usize::try_from(rows).unwrap_or(0) < height
}

/// Reads one scanline into `buf`, or nothing when libjpeg has no more rows. A libjpeg error is
/// `None`.
fn read_one_row(decoder: &mut Decompress, buf: &mut [u8]) -> Option<usize> {
    let mut rows: [&mut [u8]; 1] = [buf];
    decoder.read_scanlines(&mut rows).ok()
}

/// The decoder for a JPEG (`SkJpegCodec`). It owns the decoder that persists between calls, the
/// swizzler and the row storage that `allocateStorage` sizes.
// Port of: src/codec/SkJpegCodec.h#L20-L120 (SkJpegCodec's members)
struct JpegCodec {
    data: Arc<[u8]>,
    jpeg_color_space: JColorSpace,
    /// `fDecoderMgr->dinfo()`: the decoder whose header has been read, kept until the next rewind.
    decoder: Decompress,
    /// `fSwizzler`.
    swizzler: Option<Swizzler>,
    /// `fSwizzlerSubset`: the subset, relative to the columns libjpeg crops to.
    swizzler_subset: IRect,
    /// `fSwizzleSrcRow`: one decoded row, present when the swizzler converts it.
    swizzle_src_row: Vec<u8>,
    /// `fColorXformSrcRow`: the swizzled row, present when a colour xform follows and the
    /// destination is not four bytes per pixel.
    color_xform_src_row: Vec<u8>,
}

impl JpegCodec {
    /// Port of `SkJpegCodec::prepare`'s part of `conversionSupported` and `onDimensionsSupported`:
    /// the output colour space, the RGB565 dither and the native scale, set on the decoder.
    // Port of: src/codec/SkJpegCodec.cpp#L291-L375 (conversionSupported, onDimensionsSupported)
    fn prepare(
        &mut self,
        base: &CodecBase<'_>,
        info: &ImageInfo,
    ) -> std::result::Result<JColorSpace, Result> {
        let xform = base.color_xform();
        let Some(out) = out_color_space(info.color_type(), xform, self.jpeg_color_space) else {
            return Err(Result::InvalidConversion);
        };
        let Some(scale) = scale_for_dimensions(&self.data, info.dimensions()) else {
            return Err(Result::InvalidScale);
        };
        self.decoder.out_color_space = out;
        if info.color_type() == ColorType::RGB565 && !xform {
            self.decoder.dither_mode = DitherMode::None;
        }
        self.decoder.scale_num = scale;
        self.decoder.scale_denom = 8;
        Ok(out)
    }

    /// Port of `SkJpegCodec::initializeSwizzler`.
    // Port of: src/codec/SkJpegCodec.cpp#L481-L530 (initializeSwizzler)
    fn initialize_swizzler(
        &mut self,
        base: &CodecBase<'_>,
        dst_info: &ImageInfo,
        options: &Options,
        needs_cmyk_to_rgb: bool,
    ) {
        let mut swizzler_options = options.clone();
        if options.subset.is_some() {
            // Use the subset relative to libjpeg's cropped output.
            swizzler_options.subset = Some(self.swizzler_subset);
        }
        let mut swizzler_dst = dst_info.clone();
        if base.color_xform() {
            // The colour xform expects RGBA 8888 input.
            swizzler_dst = swizzler_dst.with_color_type(ColorType::RGBA8888);
        }
        self.swizzler = if needs_cmyk_to_rgb {
            // The swizzler does not use the width or height on the encoded info.
            let encoded = EncodedInfo::make(0, 0, Color::InvertedCMYK, Alpha::Opaque, 8);
            Swizzler::make(&encoded, None, &swizzler_dst, &swizzler_options, None)
        } else {
            let src_bpp = match self.decoder.out_color_space {
                JColorSpace::ExtRgba | JColorSpace::ExtBgra | JColorSpace::Cmyk => 4,
                JColorSpace::Rgb565 => 2,
                JColorSpace::Grayscale => 1,
                _ => 0,
            };
            Swizzler::make_simple(src_bpp, &swizzler_dst, &swizzler_options, None)
        };
    }

    /// Port of `SkJpegCodec::allocateStorage`: sizes the decoded-row and colour xform rows.
    // Port of: src/codec/SkJpegCodec.cpp#L532-L560 (allocateStorage)
    fn allocate_storage(&mut self, base: &CodecBase<'_>, dst_info: &ImageInfo) {
        let mut dst_width = non_negative(dst_info.width());
        let mut swizzle_bytes = 0;
        if let Some(swizzler) = &self.swizzler {
            swizzle_bytes = out_row_bytes(self.decoder.out_color_space, self.decoder.output_width);
            dst_width = non_negative(swizzler.swizzle_width());
        }
        let mut xform_bytes = 0;
        if base.color_xform() && dst_info.bytes_per_pixel() != 4 {
            xform_bytes = dst_width * 4;
        }
        self.swizzle_src_row = vec![0; swizzle_bytes];
        self.color_xform_src_row = vec![0; xform_bytes];
    }

    /// Port of `SkJpegCodec::readRows`: decodes `count` rows, passing each through the swizzler and
    /// the colour xform as needed. `subset_width` is the subset's width, or the destination's, and
    /// is the width the colour xform converts when no swizzler narrows it. Sets `rows_decoded`.
    // Port of: src/codec/SkJpegCodec.cpp#L400-L478 (readRows)
    fn read_rows(
        &mut self,
        base: &CodecBase<'_>,
        dst: &mut [u8],
        row_bytes: usize,
        count: usize,
        subset_width: usize,
        rows_decoded: &mut i32,
    ) -> Result {
        let out_row_len = out_row_bytes(self.decoder.out_color_space, self.decoder.output_width);
        let xform = base.color_xform();
        let has_swizzle = self.swizzler.is_some();
        let has_xform_row = !self.color_xform_src_row.is_empty();
        let dst_width = match &self.swizzler {
            Some(swizzler) => non_negative(swizzler.swizzle_width()),
            None => subset_width,
        };
        // Without a xform row, an in-place xform needs its source copied out first.
        let mut xform_copy = vec![
            0u8;
            if xform && !has_xform_row {
                dst_width * 4
            } else {
                0
            }
        ];

        for y in 0..count {
            let offset = y * row_bytes;
            let read = if has_swizzle {
                read_one_row(&mut self.decoder, &mut self.swizzle_src_row)
            } else if has_xform_row {
                read_one_row(&mut self.decoder, &mut self.color_xform_src_row)
            } else {
                read_one_row(&mut self.decoder, &mut dst[offset..offset + out_row_len])
            };
            let Some(lines) = read else {
                *rows_decoded = 0;
                return Result::InvalidInput;
            };
            if lines == 0 {
                *rows_decoded = i32::try_from(y).unwrap_or(i32::MAX);
                return Result::Success;
            }

            if let Some(swizzler) = &self.swizzler {
                if has_xform_row {
                    swizzler.swizzle(&mut self.color_xform_src_row, &self.swizzle_src_row);
                } else {
                    swizzler.swizzle(&mut dst[offset..], &self.swizzle_src_row);
                }
            }
            if xform {
                if has_xform_row {
                    base.apply_color_xform(
                        &mut dst[offset..],
                        &self.color_xform_src_row,
                        dst_width,
                    );
                } else {
                    let n = dst_width * 4;
                    xform_copy[..n].copy_from_slice(&dst[offset..offset + n]);
                    base.apply_color_xform(&mut dst[offset..], &xform_copy[..n], dst_width);
                }
            }
        }
        *rows_decoded = i32::try_from(count).unwrap_or(i32::MAX);
        Result::Success
    }

    /// Port of `SkJpegCodec::onGetPixels`, baseline and progressive.
    // Port of: src/codec/SkJpegCodec.cpp#L531-L620 (onGetPixels)
    fn decode(
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
        let out = match self.prepare(base, info) {
            Ok(out) => out,
            Err(result) => return result,
        };
        let height = usize::try_from(info.height()).unwrap_or(0);
        let width = usize::try_from(info.width()).unwrap_or(0);
        let needs_cmyk =
            needs_swizzler_from_cmyk(out, base.encoded_info().profile(), base.color_xform());

        if self.decoder.progressive_mode {
            // Port of the progressive branch: consume the input, then decode the last complete scan.
            self.decoder.buffered_image = true;
            if self.decoder.start_decompress().is_err() {
                return Result::InvalidInput;
            }
            if needs_cmyk {
                self.initialize_swizzler(base, info, options, true);
            }
            self.allocate_storage(base, info);
            let mut last_scan_completed = 0;
            while !self.decoder.input_complete() {
                // Port of: src/codec/SkJpegDecoderMgr.cpp#L42-L51 (progress_monitor). libjpeg calls
                // it through `dinfo->progress` before each consume; the error it raises at 100
                // scans is a longjmp to the decoder manager, which returns kInvalidInput.
                if self.decoder.input_scan_number >= 100 {
                    return Result::InvalidInput;
                }
                match self.decoder.consume_input() {
                    Ok(ConsumeResult::Suspended) => break,
                    Ok(ConsumeResult::ScanCompleted) => {
                        last_scan_completed = self.decoder.input_scan_number;
                    }
                    Ok(_) => {}
                    Err(_) => return Result::InvalidInput,
                }
            }
            if last_scan_completed <= 0 {
                return Result::IncompleteInput;
            }
            if self.decoder.start_output(last_scan_completed).is_err() {
                return Result::InvalidInput;
            }
            let mut rows = 0;
            let read_result = self.read_rows(base, dst, row_bytes, height, width, &mut rows);
            let _ = self.decoder.finish_output();
            if read_result != Result::Success {
                return Result::InvalidInput;
            }
            if rows_short(rows, height) {
                *rows_decoded = rows;
                return Result::IncompleteInput;
            }
            return Result::Success;
        }

        // Baseline image: a failed start is an invalid input.
        match self.decoder.start_decompress() {
            Ok(true) => {}
            _ => return Result::InvalidInput,
        }
        if needs_cmyk {
            self.initialize_swizzler(base, info, options, true);
        }
        self.allocate_storage(base, info);
        let mut rows = 0;
        // The result of readRows is not checked, as in Skia: rows short of the height are
        // incomplete input.
        let _ = self.read_rows(base, dst, row_bytes, height, width, &mut rows);
        if rows_short(rows, height) {
            *rows_decoded = rows;
            return Result::IncompleteInput;
        }
        Result::Success
    }
}

impl CodecImpl for JpegCodec {
    fn on_get_encoded_format(&self) -> EncodedImageFormat {
        EncodedImageFormat::JPEG
    }

    // Port of: src/codec/SkJpegCodec.cpp#L531-L620 (onGetPixels)
    fn on_get_pixels(
        &mut self,
        base: &mut CodecBase<'_>,
        info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        rows_decoded: &mut i32,
    ) -> Result {
        self.decode(base, info, dst, row_bytes, options, rows_decoded)
    }

    // Port of: src/codec/SkJpegCodec.cpp#L268-L285 (onRewind)
    fn on_rewind(&mut self, base: &mut CodecBase<'_>) -> bool {
        if !base.rewind_stream() {
            return false;
        }
        let Ok(decoder) = header_decoder(&self.data) else {
            return false;
        };
        let same_size = i32::try_from(decoder.image_width).ok()
            == Some(base.encoded_info().width())
            && i32::try_from(decoder.image_height).ok() == Some(base.encoded_info().height());
        if !same_size {
            return false;
        }
        self.decoder = decoder;
        self.swizzler = None;
        self.swizzle_src_row = Vec::new();
        self.color_xform_src_row = Vec::new();
        true
    }

    // Port of: src/codec/SkJpegCodec.cpp#L340-L375 (onDimensionsSupported)
    fn on_dimensions_supported(&self, _base: &CodecBase<'_>, dim: ISize) -> bool {
        scale_for_dimensions(&self.data, dim).is_some()
    }

    // Port of: src/codec/SkJpegCodec.cpp#L224-L255 (onGetScaledDimensions)
    fn on_get_scaled_dimensions(&self, base: &CodecBase<'_>, desired_scale: f32) -> ISize {
        let num = scale_num_for(desired_scale);
        match output_dimensions(&self.data, num) {
            Some((w, h)) => ISize::new(
                i32::try_from(w).unwrap_or(i32::MAX),
                i32::try_from(h).unwrap_or(i32::MAX),
            ),
            None => base.dimensions(),
        }
    }

    // Port of: src/codec/SkJpegCodec.cpp#L291-L355 (conversionSupported)
    fn conversion_supported(
        &self,
        _base: &CodecBase<'_>,
        dst: &ImageInfo,
        _src_is_opaque: bool,
        needs_color_xform: bool,
    ) -> bool {
        if dst.alpha_type() == AlphaType::Unknown {
            return false;
        }
        out_color_space(dst.color_type(), needs_color_xform, self.jpeg_color_space).is_some()
    }

    // Port of: src/codec/SkJpegCodec.cpp#L467-L500 (onStartScanlineDecode)
    fn on_start_scanline_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        dst_info: &ImageInfo,
        options: &Options,
    ) -> Result {
        let out = match self.prepare(base, dst_info) {
            Ok(out) => out,
            Err(result) => return result,
        };
        if !matches!(self.decoder.start_decompress(), Ok(true)) {
            return Result::InvalidInput;
        }
        let needs_cmyk =
            needs_swizzler_from_cmyk(out, base.encoded_info().profile(), base.color_xform());
        if let Some(subset) = options.subset {
            let mut start_x = u32::try_from(subset.x()).unwrap_or(0);
            let mut width = u32::try_from(subset.width()).unwrap_or(0);
            // libjpeg-turbo may need to align startX to a multiple of the IDCT block size. If so,
            // it decreases startX and increases width, so the right edge stays the same.
            if self
                .decoder
                .crop_scanline(&mut start_x, &mut width)
                .is_err()
            {
                return Result::InvalidInput;
            }
            // The swizzler (if any) subsets libjpeg's output further. Only the x dimension matters,
            // since the scanline decoder handles one row at a time.
            self.swizzler_subset = IRect::from_xywh(
                subset.x() - i32::try_from(start_x).unwrap_or(i32::MAX),
                0,
                subset.width(),
                subset.height(),
            );
            // A swizzler is needed if libjpeg-turbo cannot provide the exact subset requested.
            if start_x != u32::try_from(subset.x()).unwrap_or(0)
                || width != u32::try_from(subset.width()).unwrap_or(0)
            {
                self.initialize_swizzler(base, dst_info, options, needs_cmyk);
            }
        }
        // Make sure we have a swizzler if we are converting from CMYK.
        if self.swizzler.is_none() && needs_cmyk {
            self.initialize_swizzler(base, dst_info, options, true);
        }
        self.allocate_storage(base, dst_info);
        Result::Success
    }

    // Port of: src/codec/SkJpegCodec.cpp#L480-L490 (onGetScanlines)
    fn on_get_scanlines(
        &mut self,
        base: &mut CodecBase<'_>,
        dst: &mut [u8],
        count: i32,
        row_bytes: usize,
    ) -> i32 {
        let info = base.dst_info().clone();
        let subset_width = non_negative(
            base.options()
                .subset
                .map_or(info.width(), |subset| subset.width()),
        );
        let Ok(count) = usize::try_from(count) else {
            return 0;
        };
        let mut rows = 0;
        // The result of readRows is ignored, as in Skia. A short read is seen by the caller as
        // fewer rows than requested.
        let _ = self.read_rows(base, dst, row_bytes, count, subset_width, &mut rows);
        rows
    }

    // Port of: src/codec/SkJpegCodec.cpp#L763-L772 (onSkipScanlines)
    fn on_skip_scanlines(&mut self, _base: &mut CodecBase<'_>, count: i32) -> bool {
        let Ok(count) = u32::try_from(count) else {
            return false;
        };
        matches!(self.decoder.skip_scanlines(count), Ok(skipped) if skipped == count)
    }

    // Port of: src/codec/SkJpegCodec.cpp#L380-L399 (getSampler)
    fn on_get_sampler(
        &mut self,
        base: &CodecBase<'_>,
        create_if_necessary: bool,
    ) -> Option<&mut dyn Sampler> {
        if !create_if_necessary || self.swizzler.is_some() {
            return self
                .swizzler
                .as_mut()
                .map(|swizzler| swizzler as &mut dyn Sampler);
        }
        let needs_cmyk = needs_swizzler_from_cmyk(
            self.decoder.out_color_space,
            base.encoded_info().profile(),
            base.color_xform(),
        );
        let dst_info = base.dst_info().clone();
        let options = base.options().clone();
        self.initialize_swizzler(base, &dst_info, &options, needs_cmyk);
        self.allocate_storage(base, &dst_info);
        self.swizzler
            .as_mut()
            .map(|swizzler| swizzler as &mut dyn Sampler)
    }
}

/// Reads all of `stream` into memory.
fn read_all(stream: &mut dyn Stream) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk);
        if n == 0 {
            break;
        }
        out.extend_from_slice(&chunk[..n]);
    }
    out
}

// Port of: src/codec/SkJpegMetadataDecoderImpl.cpp#L292-L390 (read_metadata, for the ICC marker,
// whose index is one byte and whose signature has no padding)
fn read_icc_profile(markers: &[skia_rust_libjpeg::SavedMarker]) -> Option<Vec<u8>> {
    const BYTES_IN_INDEX: usize = 1;
    let header_size = ICC_SIG.len() + 2 * BYTES_IN_INDEX;
    let mut parts: Vec<Option<&[u8]>> = Vec::new();
    let mut parts_total_size = 0usize;
    let mut found_part_count: u32 = 0;
    let mut expected_part_count: u32 = 0;

    for marker in markers {
        // marker_has_signature: the right marker, more than the signature, and the signature.
        if i32::from(marker.marker) != ICC_MARKER
            || marker.data.len() <= ICC_SIG.len()
            || !marker.data.starts_with(ICC_SIG)
        {
            continue;
        }
        let data = &marker.data;
        if data.len() <= header_size {
            continue;
        }
        let part_index = u32::from(data[ICC_SIG.len()]);
        let part_count = u32::from(data[ICC_SIG.len() + 1]);
        // A part count of 0 is invalid, and so is an index outside 1..=count.
        if part_count == 0 || part_index == 0 || part_index > part_count {
            return None;
        }
        if expected_part_count == 0 {
            expected_part_count = part_count;
            parts = vec![None; part_count as usize];
        }
        if part_count != expected_part_count {
            return None;
        }
        let part = &data[header_size..];
        let slot = &mut parts[part_index as usize - 1];
        if slot.is_some() {
            // Duplicate parts.
            return None;
        }
        parts_total_size += part.len();
        *slot = Some(part);
        found_part_count += 1;
        if found_part_count == expected_part_count {
            break;
        }
    }

    // No ICC data is not an error. Missing parts are.
    if expected_part_count == 0 || found_part_count != expected_part_count {
        return None;
    }
    let mut out = Vec::with_capacity(parts_total_size);
    for part in parts.into_iter().flatten() {
        out.extend_from_slice(part);
    }
    Some(out)
}

// Port of: src/codec/SkJpegMetadataDecoderImpl.cpp#L441-L449 (getExifMetadata), through
// read_metadata (src/codec/SkJpegMetadataDecoderImpl.cpp#L292-L390) with one part and no index:
// the first APP1 marker with the Exif signature, minus the signature and its padding byte.
fn read_exif_data(markers: &[skia_rust_libjpeg::SavedMarker]) -> Option<&[u8]> {
    // The signature, then one byte of padding.
    let header_size = EXIF_SIG.len() + 1;
    for marker in markers {
        // marker_has_signature: the right marker, more than the signature, and the signature.
        if i32::from(marker.marker) != EXIF_MARKER
            || marker.data.len() <= EXIF_SIG.len()
            || !marker.data.starts_with(EXIF_SIG)
        {
            continue;
        }
        if marker.data.len() <= header_size {
            continue;
        }
        return Some(&marker.data[header_size..]);
    }
    None
}

/// Port of `SkJpegCodec::MakeFromStream`: reads the header and the ICC profile, and builds the
/// codec. The decoder whose header was read here is the one the first decode uses.
///
/// # Errors
/// The header's result: `IncompleteInput` for a truncated header, `InvalidInput` for a file that
/// is not a JPEG or has no image.
// Port of: src/codec/SkJpegCodec.cpp#L86-L150 (MakeFromStream)
pub fn make_from_stream<'a>(
    mut stream: Box<dyn Stream + Send + 'a>,
) -> std::result::Result<Codec<'a>, Result> {
    let data: Arc<[u8]> = Arc::from(read_all(&mut *stream));
    let decoder = header_decoder_saving_markers(&data)?;

    let jpeg_color_space = decoder.jpeg_color_space;
    let color = match jpeg_color_space {
        JColorSpace::Grayscale => Color::Gray,
        JColorSpace::YCbCr => Color::YUV,
        JColorSpace::Rgb => Color::RGB,
        JColorSpace::Ycck => Color::YCCK,
        JColorSpace::Cmyk => Color::InvertedCMYK,
        _ => return Err(Result::InvalidInput),
    };
    let width = i32::try_from(decoder.image_width).unwrap_or(i32::MAX);
    let height = i32::try_from(decoder.image_height).unwrap_or(i32::MAX);

    // An ICC profile whose data space does not match the JPEG's colour is dropped.
    let mut info = EncodedInfo::make(width, height, color, Alpha::Opaque, 8);
    let icc = read_icc_profile(&decoder.marker_list)
        .and_then(|bytes| skia_rust_skcms::parse(&bytes).map(|profile| (bytes, profile)));
    if let Some((bytes, profile)) = icc {
        let space = data_space(&profile);
        let matches = match jpeg_color_space {
            JColorSpace::Cmyk | JColorSpace::Ycck => space == DataSpace::Cmyk,
            JColorSpace::Grayscale => space == DataSpace::Gray || space == DataSpace::Rgb,
            _ => space == DataSpace::Rgb,
        };
        if matches {
            info = info.with_profile(Arc::from(bytes), profile);
        }
    }

    // The orientation is read from the EXIF data, when there is any (`get_exif_orientation`).
    let mut metadata = exif::Metadata::default();
    exif::parse(&mut metadata, read_exif_data(&decoder.marker_list));
    let origin = metadata.origin.unwrap_or(EncodedOrigin::TopLeft);

    let stream_copy: Box<dyn Stream + Send + 'a> = MemoryStream::make_copy(&data);
    let imp = JpegCodec {
        data,
        jpeg_color_space,
        decoder,
        swizzler: None,
        swizzler_subset: IRect::from_xywh(0, 0, 0, 0),
        swizzle_src_row: Vec::new(),
        color_xform_src_row: Vec::new(),
    };
    Ok(Codec::new(
        info,
        Box::new(imp),
        Some(stream_copy),
        origin,
        Some(PixelFormat::Rgba8888),
    ))
}

/// Whether `buf` starts with the JPEG signature. Port of `SkJpegCodec::IsJpeg`.
// Port of: src/codec/SkJpegCodec.cpp#L77-L79 (IsJpeg)
#[must_use]
pub fn is_jpeg(buf: &[u8]) -> bool {
    buf.len() >= JPEG_SIG.len() && buf[..JPEG_SIG.len()] == JPEG_SIG
}
