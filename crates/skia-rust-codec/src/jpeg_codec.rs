// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkJpegCodec.cpp (chrome/m156), src/codec/SkJpegCodec.h, src/codec/SkJpegConstants.h,
// the ICC part of src/codec/SkJpegMetadataDecoderImpl.cpp (read_metadata), and the memory source
// of src/codec/SkJpegSourceMgr.cpp.
//
// Deviations, recorded in docs/design/codecs.md:
// * `Decompress` holds `Rc` state, so it is not `Send`, and `CodecImpl` requires `Send`. The codec
//   therefore keeps the encoded bytes and builds a decoder for each call. Each call reads the header
//   again, as `SkCodec::rewind` does, and decodes with the same libjpeg calls. The pixels are the
//   same for a whole-image decode. Scanline and incremental decoding (which need state between
//   calls), subsets, YUV planes and gainmaps are not ported yet.
// * EXIF orientation is not read (`SkExif` is not ported), so the origin is top-left.

//! The JPEG decoder (`SkJpegCodec`): whole-image and progressive decodes, native scaling by 1/8
//! steps, colour conversion, RGB565 and grayscale output, and CMYK/YCCK through the swizzler.

use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::{MemoryStream, Stream};
use skia_rust_libjpeg::{
    ColorSpace as JColorSpace, ConsumeResult, Decompress, DitherMode, HeaderResult, JpegSource,
    SavedMarker, SrcBuf,
};
use skia_rust_skcms::{IccProfile, PixelFormat};

use crate::codec::{Codec, CodecBase, CodecImpl, Options, Result};
use crate::encoded_info::{Alpha, Color, EncodedInfo};
use crate::swizzler::Swizzler;

// Port of: src/codec/SkJpegConstants.h#L14-L16 (kJpegSig's first and last bytes)
const JPEG_SIG: [u8; 3] = [0xFF, 0xD8, 0xFF];

// Port of: src/codec/SkJpegConstants.h#L30-L37 (kICCMarker, kICCSig, kICCMarkerHeaderSize)
const ICC_MARKER: i32 = 0xE0 + 2;
const ICC_SIG: &[u8; 12] = b"ICC_PROFILE\0";

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

/// A decoder over `data` after its header has been read (`read_header` in `SkJpegCodec.cpp`).
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
        ColorType::RGBA8888 => JColorSpace::ExtRgba,
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
        ColorType::RGBAF16 | ColorType::BGR101010xXR => JColorSpace::ExtRgba,
        _ => return None,
    };
    // libjpeg-turbo does not convert CMYK to RGBA, so the swizzler (or the colour xform) does.
    if matches!(encoded, JColorSpace::Cmyk | JColorSpace::Ycck) {
        out = JColorSpace::Cmyk;
    }
    Some(out)
}

/// The bytes per output row of libjpeg's output colour space.
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

/// Reads one scanline into `buf`, or nothing when libjpeg has no more rows. A libjpeg error is
/// `None`.
fn read_one_row(decoder: &mut Decompress, buf: &mut [u8]) -> Option<usize> {
    let mut rows: [&mut [u8]; 1] = [buf];
    decoder.read_scanlines(&mut rows).ok()
}

/// Port of `SkJpegCodec::readRows`: decodes `count` rows, passing each through the swizzler and
/// the colour xform as needed. Sets `rows_decoded` to the number of rows written.
// Port of: src/codec/SkJpegCodec.cpp#L400-L478 (readRows)
#[allow(clippy::too_many_arguments)] // mirrors readRows' parameters
fn read_rows(
    decoder: &mut Decompress,
    base: &CodecBase<'_>,
    swizzler: Option<&Swizzler>,
    dst: &mut [u8],
    row_bytes: usize,
    count: usize,
    out_row_len: usize,
    rows_decoded: &mut i32,
) -> Result {
    let xform = base.color_xform();
    // The decoded row before swizzling (when there is a swizzler).
    let mut swizzle_src = vec![0u8; if swizzler.is_some() { out_row_len } else { 0 }];
    // The row fed to the colour xform, in RGBA.
    let mut xform_src = vec![0u8; if xform { out_row_len } else { 0 }];

    for y in 0..count {
        let offset = y * row_bytes;
        let decoded = if swizzler.is_some() {
            read_one_row(decoder, &mut swizzle_src)
        } else if xform {
            read_one_row(decoder, &mut xform_src)
        } else {
            read_one_row(decoder, &mut dst[offset..offset + out_row_len])
        };
        let Some(lines) = decoded else {
            *rows_decoded = 0;
            return Result::InvalidInput;
        };
        if lines == 0 {
            *rows_decoded = y as i32;
            return Result::Success;
        }

        if let Some(swizzler) = swizzler {
            if xform {
                swizzler.swizzle(&mut xform_src, &swizzle_src);
            } else {
                swizzler.swizzle(&mut dst[offset..], &swizzle_src);
            }
        }
        if xform {
            // The RGBA row holds four bytes per pixel.
            base.apply_color_xform(&mut dst[offset..], &xform_src, out_row_len / 4);
        }
    }
    *rows_decoded = count as i32;
    Result::Success
}

/// The decoder for a JPEG (`SkJpegCodec`). It keeps the encoded bytes and the encoded colour space.
// Port of: src/codec/SkJpegCodec.h#L20-L60 (SkJpegCodec)
struct JpegCodec {
    data: Arc<[u8]>,
    jpeg_color_space: JColorSpace,
}

impl JpegCodec {
    // Port of: src/codec/SkJpegCodec.cpp#L531-L620 (onGetPixels, baseline and progressive)
    fn decode(
        &self,
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
        let xform = base.color_xform();
        let Some(out) = out_color_space(info.color_type(), xform, self.jpeg_color_space) else {
            return Result::InvalidConversion;
        };
        let Some(scale) = scale_for_dimensions(&self.data, info.dimensions()) else {
            return Result::InvalidScale;
        };

        let mut decoder = match header_decoder(&self.data) {
            Ok(decoder) => decoder,
            Err(result) => return result,
        };
        decoder.out_color_space = out;
        if info.color_type() == ColorType::RGB565 && !xform {
            decoder.dither_mode = DitherMode::None;
        }
        decoder.scale_num = scale;
        decoder.scale_denom = 8;

        let swizzler = if needs_swizzler_from_cmyk(out, base.encoded_info().profile(), xform) {
            let encoded = EncodedInfo::make(0, 0, Color::InvertedCMYK, Alpha::Opaque, 8);
            let mut swizzler_dst = info.clone();
            if xform {
                // The colour xform expects RGBA 8888 input.
                swizzler_dst = swizzler_dst.with_color_type(ColorType::RGBA8888);
            }
            match Swizzler::make(&encoded, None, &swizzler_dst, options, None) {
                Some(swizzler) => Some(swizzler),
                None => return Result::InternalError,
            }
        } else {
            None
        };

        let height = info.height() as usize;
        let mut rows = 0;
        if decoder.progressive_mode {
            // Port of the progressive branch: consume the input, then decode the last complete scan.
            decoder.buffered_image = true;
            if decoder.start_decompress().is_err() {
                return Result::InvalidInput;
            }
            // The output size is known now that the decode has started.
            let out_len = out_row_bytes(out, decoder.output_width);
            let mut last_scan_completed = 0;
            while !decoder.input_complete() {
                match decoder.consume_input() {
                    Ok(ConsumeResult::Suspended) => break,
                    Ok(ConsumeResult::ScanCompleted) => {
                        last_scan_completed = decoder.input_scan_number;
                    }
                    Ok(_) => {}
                    Err(_) => return Result::InvalidInput,
                }
            }
            if last_scan_completed <= 0 {
                return Result::IncompleteInput;
            }
            if decoder.start_output(last_scan_completed).is_err() {
                return Result::InvalidInput;
            }
            let read_result = read_rows(
                &mut decoder,
                base,
                swizzler.as_ref(),
                dst,
                row_bytes,
                height,
                out_len,
                &mut rows,
            );
            let _ = decoder.finish_output();
            if read_result != Result::Success {
                return Result::InvalidInput;
            }
            if (rows as usize) < height {
                *rows_decoded = rows;
                return Result::IncompleteInput;
            }
            return Result::Success;
        }

        // Baseline image: a failed start is an invalid input.
        match decoder.start_decompress() {
            Ok(true) => {}
            _ => return Result::InvalidInput,
        }
        let out_len = out_row_bytes(out, decoder.output_width);
        // The result of readRows is not checked, as in Skia: rows short of the height are
        // incomplete input.
        let _ = read_rows(
            &mut decoder,
            base,
            swizzler.as_ref(),
            dst,
            row_bytes,
            height,
            out_len,
            &mut rows,
        );
        if (rows as usize) < height {
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

/// The APPn marker payloads that decide the codec's colour, read from the header. Saved by
/// `jpeg_save_markers` before `jpeg_read_header`.
struct Header {
    width: i32,
    height: i32,
    jpeg_color_space: JColorSpace,
    markers: Vec<SavedMarker>,
}

// Port of: src/codec/SkJpegCodec.cpp#L51-L77 (read_header, saving the ICC marker)
fn read_jpeg_header(data: &Arc<[u8]>) -> std::result::Result<Header, Result> {
    let mut decoder = Decompress::new(Box::new(MemSource {
        data: Arc::clone(data),
    }));
    if decoder.save_markers(ICC_MARKER, 0xFFFF).is_err() {
        return Err(Result::InvalidInput);
    }
    match decoder.read_header(true) {
        Ok(HeaderResult::Ok) => {}
        Ok(HeaderResult::Suspended) => return Err(Result::IncompleteInput),
        _ => return Err(Result::InvalidInput),
    }
    Ok(Header {
        width: i32::try_from(decoder.image_width).unwrap_or(i32::MAX),
        height: i32::try_from(decoder.image_height).unwrap_or(i32::MAX),
        jpeg_color_space: decoder.jpeg_color_space,
        markers: decoder.marker_list.clone(),
    })
}

// Port of: src/codec/SkJpegMetadataDecoderImpl.cpp#L292-L390 (read_metadata, for the ICC marker,
// whose index is one byte and whose signature has no padding)
fn read_icc_profile(markers: &[SavedMarker]) -> Option<Vec<u8>> {
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

/// Port of `SkJpegCodec::MakeFromStream`: reads the header and the ICC profile, and builds the
/// codec.
// Port of: src/codec/SkJpegCodec.cpp#L86-L150 (MakeFromStream)
pub fn make_from_stream<'a>(
    mut stream: Box<dyn Stream + Send + 'a>,
) -> std::result::Result<Codec<'a>, Result> {
    let data: Arc<[u8]> = Arc::from(read_all(&mut *stream));
    let header = read_jpeg_header(&data)?;

    let color = match header.jpeg_color_space {
        JColorSpace::Grayscale => Color::Gray,
        JColorSpace::YCbCr => Color::YUV,
        JColorSpace::Rgb => Color::RGB,
        JColorSpace::Ycck => Color::YCCK,
        JColorSpace::Cmyk => Color::InvertedCMYK,
        _ => return Err(Result::InvalidInput),
    };

    // An ICC profile whose data space does not match the JPEG's colour is dropped.
    let mut info = EncodedInfo::make(header.width, header.height, color, Alpha::Opaque, 8);
    if let Some(bytes) = read_icc_profile(&header.markers) {
        if let Some(profile) = skia_rust_skcms::parse(&bytes) {
            let space = data_space(&profile);
            let matches = match header.jpeg_color_space {
                JColorSpace::Cmyk | JColorSpace::Ycck => space == DataSpace::Cmyk,
                JColorSpace::Grayscale => space == DataSpace::Gray || space == DataSpace::Rgb,
                _ => space == DataSpace::Rgb,
            };
            if matches {
                info = info.with_profile(Arc::from(bytes), profile);
            }
        }
    }

    let stream_copy: Box<dyn Stream + Send + 'a> = MemoryStream::make_copy(&data);
    let imp = JpegCodec {
        data,
        jpeg_color_space: header.jpeg_color_space,
    };
    Ok(Codec::new(
        info,
        Box::new(imp),
        Some(stream_copy),
        EncodedOrigin::TopLeft,
        Some(PixelFormat::Rgba8888),
    ))
}

/// Whether `buf` starts with the JPEG signature. Port of `SkJpegCodec::IsJpeg`.
// Port of: src/codec/SkJpegCodec.cpp#L77-L79 (IsJpeg)
#[must_use]
pub fn is_jpeg(buf: &[u8]) -> bool {
    buf.len() >= JPEG_SIG.len() && buf[..JPEG_SIG.len()] == JPEG_SIG
}
