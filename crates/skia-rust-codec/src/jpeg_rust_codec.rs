// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.cpp and SkJpegRustCodec.h, and the
// entry points of experimental/rust_jpeg/decoder/SkJpegRustDecoder.cpp (chrome/m156), the JPEG
// decoder built on zune-jpeg. The zune-jpeg reader is `jpeg_rust_ffi::Reader`, the port of
// `FFI.rs`; the metadata comes from `jpeg_metadata` and `exif`.
//
// Not ported yet: the gainmap hooks (`onGetGainmapInfo`, `onGetGainmapCodec`), which need the XMP
// and ISO 21496-1 decoder, and `onGetScaledDimensions` and the YUVA paths, which Skia's own
// comments list as TODO.

//! `SkJpegRustCodec`: the JPEG decoder on zune-jpeg, with the swizzler and colour transform of the
//! shared codec layer.

use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::IRect;
use skia_rust_core::stream::Stream;
use skia_rust_skcms::PixelFormat;

use crate::codec::{
    Codec, CodecBase, CodecImpl, FrameInfo, NO_FRAME, Options, Result, ZeroInitialized,
};
use crate::codec_animation::{Blend, DisposalMethod};
use crate::encoded_info::{Alpha, Color, EncodedInfo};
use crate::exif;
use crate::jpeg_codec::{DataSpace, data_space};
use crate::jpeg_rust_ffi::{DecodingResult, Reader, is_jpeg_data};
use crate::swizzler::Swizzler;

/// The colour type the colour transform decodes into: each pixel is four bytes, as in Skia's
/// `kXformSrcColorType`.
// Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.cpp#L35 (kXformSrcColorType,
// chrome/m156)
const XFORM_SRC_COLOR_TYPE: ColorType = ColorType::RGBA8888;

/// Port of `MapDecodingResult`: the codec result for a reader result.
// Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.cpp#L49-L67 (MapDecodingResult,
// chrome/m156)
fn map_decoding_result(result: DecodingResult) -> Result {
    match result {
        DecodingResult::Success => Result::Success,
        DecodingResult::FormatError => Result::ErrorInInput,
        DecodingResult::ParameterError => Result::InvalidParameters,
        DecodingResult::UnsupportedFeature => Result::Unimplemented,
        DecodingResult::IncompleteInput => Result::IncompleteInput,
        DecodingResult::MemoryError => Result::InternalError,
    }
}

/// Port of `SkJpegRustCodec::swizzleRow`: converts one decoded row into `dst_row`, through the
/// colour transform (and the RGBA transform buffer) when there is one. A free function, so that the
/// rows borrowed from the reader can be used while the swizzler and its buffer are borrowed.
// Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.cpp#L359-L368 (swizzleRow,
// chrome/m156)
fn swizzle_row(
    swizzler: Option<&Swizzler>,
    xform_buffer: &mut [u8],
    base: &CodecBase<'_>,
    src_row: &[u8],
    dst_row: &mut [u8],
) {
    let Some(swizzler) = swizzler else {
        return;
    };
    if base.xform_on_decode() {
        let width = usize::try_from(swizzler.swizzle_width()).unwrap_or(0);
        swizzler.swizzle(xform_buffer, src_row);
        base.apply_color_xform(dst_row, xform_buffer, width);
    } else {
        swizzler.swizzle(dst_row, src_row);
    }
}

/// The state of an incremental decode between calls: the row stride of the destination, and how
/// many rows have been written so far. The destination itself is passed to each call, so it is
/// not stored. Port of `SkJpegRustCodec::DecodingState`.
// Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.h#L84-L88 (DecodingState,
// chrome/m156)
#[derive(Debug, Clone, Copy)]
struct DecodingState {
    dst_row_stride: usize,
    total_rows_initialized: i32,
}

/// The JPEG decoder on zune-jpeg. Port of `SkJpegRustCodec`.
// Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.h#L23-L111 (SkJpegRustCodec,
// chrome/m156)
pub(crate) struct JpegRustCodec<'a> {
    /// The stream the reader reads, kept here so a rewind and the encoded data can reach it.
    /// Port of `fPrivStream`.
    priv_stream: Arc<Mutex<Box<dyn Stream + Send + 'a>>>,
    /// Port of `fReader`.
    reader: Reader<'a>,
    /// Port of `fIncrementalDecodingState`.
    incremental: Option<DecodingState>,
    /// Port of `fSwizzler`.
    swizzler: Option<Swizzler>,
    /// Port of `fXformBuffer`: one RGBA pixel per destination pixel, when the colour transform
    /// runs on each decoded row.
    xform_buffer: Vec<u8>,
}

impl JpegRustCodec<'_> {
    /// Port of `SkJpegRustCodec::initializeSwizzler`.
    // Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.cpp#L339-L357
    // (initializeSwizzler, chrome/m156)
    fn initialize_swizzler(
        &mut self,
        base: &CodecBase<'_>,
        dst_info: &ImageInfo,
        opts: &Options,
    ) -> Result {
        let mut swizzler_info = dst_info.clone();
        let mut swizzler_options = opts.clone();
        if base.xform_on_decode() {
            let width = usize::try_from(dst_info.width()).unwrap_or(0);
            self.xform_buffer = vec![0; width * 4];
            swizzler_info = swizzler_info.with_color_type(XFORM_SRC_COLOR_TYPE);
            if dst_info.alpha_type() == AlphaType::Premul {
                swizzler_info = swizzler_info.with_alpha_type(AlphaType::Unpremul);
            }
            swizzler_options.zero_initialized = ZeroInitialized::No;
        }
        self.swizzler = Swizzler::make(
            base.encoded_info(),
            None,
            &swizzler_info,
            &swizzler_options,
            None,
        );
        if self.swizzler.is_none() {
            return Result::InvalidConversion;
        }
        Result::Success
    }

    /// Port of `SkJpegRustCodec::performFullDecode`: decodes every row into `dst`.
    // Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.cpp#L423-L457
    // (performFullDecode, chrome/m156)
    fn perform_full_decode(
        &mut self,
        base: &CodecBase<'_>,
        dst_info: &ImageInfo,
        dst: &mut [u8],
        dst_row_stride: usize,
        rows_decoded: &mut i32,
    ) -> Result {
        let read_result = self.reader.read_image_data();
        if read_result != DecodingResult::Success {
            return map_decoding_result(read_result);
        }

        let src_row_bytes = self.reader.row_bytes() as usize;
        // The reader and the swizzler are separate fields, so the rows borrowed from the reader
        // are used while the swizzler and its buffer are borrowed too.
        let (rows_info, image_data) = self.reader.get_next_rows();
        if image_data.is_empty() {
            return Result::ErrorInInput;
        }

        let Ok(height) = usize::try_from(dst_info.height()) else {
            return Result::InternalError;
        };
        let Some(total_dst_size) = height.checked_mul(dst_row_stride) else {
            return Result::InternalError;
        };
        if dst.len() < total_dst_size {
            return Result::InvalidParameters;
        }

        let row_count = rows_info.row_count;
        for y in 0..row_count as usize {
            let Some(src_row) = image_data.get(y * src_row_bytes..(y + 1) * src_row_bytes) else {
                return Result::ErrorInInput;
            };
            let Some(dst_row) = dst.get_mut(y * dst_row_stride..(y + 1) * dst_row_stride) else {
                return Result::InvalidParameters;
            };
            swizzle_row(
                self.swizzler.as_ref(),
                &mut self.xform_buffer,
                base,
                src_row,
                dst_row,
            );
        }

        *rows_decoded = i32::try_from(row_count).unwrap_or(i32::MAX);
        if row_count == u32::try_from(base.dimensions().height).unwrap_or(0) {
            Result::Success
        } else {
            Result::IncompleteInput
        }
    }

    /// Port of `SkJpegRustCodec::incrementalDecode`: decodes the rows that the stream now allows,
    /// and reports how many rows of `dst` are initialized.
    // Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.cpp#L629-L697
    // (incrementalDecode, chrome/m156)
    fn incremental_decode(
        &mut self,
        base: &CodecBase<'_>,
        state: &mut DecodingState,
        dst: &mut [u8],
        rows_decoded: &mut i32,
    ) -> Result {
        let src_row_bytes = self.reader.row_bytes() as usize;
        let read_result = self.reader.read_incremental_image_data();
        if read_result != DecodingResult::Success && read_result != DecodingResult::IncompleteInput
        {
            return map_decoding_result(read_result);
        }

        let (rows_info, image_data) = self.reader.get_next_rows();
        let dst_row_start = rows_info.dst_row_start;
        let row_count = rows_info.row_count;

        // Progressive previews replace the initialized frame from row zero. SkCodec permits
        // rowsDecoded to include initialized but unfinished rows.
        if rows_info.is_preview && dst_row_start != 0 {
            return Result::ErrorInInput;
        }

        let height = u32::try_from(base.dimensions().height).unwrap_or(0);
        let mut copied_rows: u32 = 0;
        if row_count > 0 {
            if src_row_bytes == 0
                || image_data.is_empty()
                || dst_row_start >= height
                || row_count > height - dst_row_start
            {
                return Result::ErrorInInput;
            }
            let Some(required_src_bytes) = (row_count as usize).checked_mul(src_row_bytes) else {
                return Result::ErrorInInput;
            };
            if image_data.len() < required_src_bytes {
                return Result::ErrorInInput;
            }

            for i in 0..row_count as usize {
                let dst_y = dst_row_start as usize + i;
                let src_offset = i * src_row_bytes;
                let Some(dst_offset) = dst_y.checked_mul(state.dst_row_stride) else {
                    return Result::InternalError;
                };
                let Some(src_row) = image_data.get(src_offset..src_offset + src_row_bytes) else {
                    return Result::ErrorInInput;
                };
                let Some(dst_row) = dst.get_mut(dst_offset..dst_offset + state.dst_row_stride)
                else {
                    return Result::InvalidParameters;
                };
                swizzle_row(
                    self.swizzler.as_ref(),
                    &mut self.xform_buffer,
                    base,
                    src_row,
                    dst_row,
                );
                copied_rows += 1;
            }
            let rows_now = i32::try_from(dst_row_start + copied_rows).unwrap_or(i32::MAX);
            state.total_rows_initialized = state.total_rows_initialized.max(rows_now);
        }

        *rows_decoded = state.total_rows_initialized;
        let height = i32::try_from(height).unwrap_or(i32::MAX);
        if read_result == DecodingResult::Success && state.total_rows_initialized >= height {
            Result::Success
        } else {
            Result::IncompleteInput
        }
    }
}

// Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.h#L30-L111 (SkJpegRustCodec
// overrides, chrome/m156)
impl CodecImpl for JpegRustCodec<'_> {
    // Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.h#L37-L39 (onGetEncodedFormat)
    fn on_get_encoded_format(&self) -> EncodedImageFormat {
        EncodedImageFormat::JPEG
    }

    // Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.cpp#L459-L466 (onGetPixels)
    fn on_get_pixels(
        &mut self,
        base: &mut CodecBase<'_>,
        info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        rows_decoded: &mut i32,
    ) -> Result {
        self.reader.reset_decode_state();
        let result = self.initialize_swizzler(base, info, options);
        if result != Result::Success {
            return result;
        }
        let mut decoded_rows = 0;
        let result = self.perform_full_decode(base, info, dst, row_bytes, &mut decoded_rows);
        *rows_decoded = decoded_rows;
        result
    }

    // Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.h#L44 (onSupportsIncrementalDecode)
    fn on_supports_incremental_decode(&self, _dst: &ImageInfo) -> bool {
        true
    }

    // Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.cpp#L583-L604
    // (onStartIncrementalDecode, chrome/m156)
    fn on_start_incremental_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        dst_info: &ImageInfo,
        _dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
    ) -> Result {
        if row_bytes < dst_info.min_row_bytes() {
            return Result::InvalidParameters;
        }
        if options.subset.is_some() {
            return Result::InvalidParameters;
        }
        let result = self.initialize_swizzler(base, dst_info, options);
        if result != Result::Success {
            return result;
        }
        self.reader.reset_decode_state();

        let Ok(height) = usize::try_from(dst_info.height()) else {
            return Result::InternalError;
        };
        if height.checked_mul(row_bytes).is_none() {
            return Result::InternalError;
        }
        self.incremental = Some(DecodingState {
            dst_row_stride: row_bytes,
            total_rows_initialized: 0,
        });
        Result::Success
    }

    // Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.cpp#L606-L619
    // (onIncrementalDecode, chrome/m156)
    fn on_incremental_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        dst: &mut [u8],
        rows_decoded: &mut i32,
    ) -> Result {
        let Some(mut state) = self.incremental.take() else {
            return Result::InvalidParameters;
        };
        let result = self.incremental_decode(base, &mut state, dst, rows_decoded);
        // Only `IncompleteInput` leaves the state usable for another call.
        if result == Result::IncompleteInput {
            self.incremental = Some(state);
        }
        result
    }

    // Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.cpp#L503-L515 (onRewind)
    fn on_rewind(&mut self, _base: &mut CodecBase<'_>) -> bool {
        self.incremental = None;
        let rewound = self
            .priv_stream
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .rewind();
        if !rewound {
            return false;
        }
        let mut reader = Reader::new(Arc::clone(&self.priv_stream));
        if !reader.metadata_loaded() && reader.read_metadata() != DecodingResult::Success {
            return false;
        }
        self.reader = reader;
        true
    }

    // Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.cpp#L537-L552
    // (onGetFrameInfo, chrome/m156)
    fn on_get_frame_info(&self, index: i32, info: Option<&mut FrameInfo>) -> bool {
        if index != 0 {
            return false;
        }
        if let Some(info) = info {
            info.required_frame = NO_FRAME;
            info.duration = 0;
            info.fully_received = self.reader.image_data_loaded();
            // A JPEG is opaque, so its info has an opaque alpha type.
            info.alpha_type = AlphaType::Opaque;
            // JPEG never has alpha.
            info.has_alpha_within_bounds = false;
            info.disposal_method = DisposalMethod::Keep;
            info.blend = Blend::Src;
            info.frame_rect = IRect::from_wh(
                i32::try_from(self.reader.width()).unwrap_or(i32::MAX),
                i32::try_from(self.reader.height()).unwrap_or(i32::MAX),
            );
        }
        true
    }

    // Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.cpp#L570-L581 (getEncodedData)
    fn on_get_encoded_data(&mut self, _base: &mut CodecBase<'_>) -> Option<Data> {
        let stream = self
            .priv_stream
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(data) = stream.get_data() {
            return Some(data);
        }
        let mut duplicate = stream.duplicate()?;
        if !duplicate.has_length() {
            return None;
        }
        let size = duplicate.get_length();
        Data::from_stream(&mut *duplicate, size)
    }
}

/// Port of `SkJpegRustCodec::MakeFromStream`: reads the JPEG headers, and makes the codec.
///
/// # Errors
/// The [`Result`] of the failed step: a reader error, or an overflow of the pixel buffer size.
// Port of: experimental/rust_jpeg/decoder/impl/SkJpegRustCodec.cpp#L101-L201
// (MakeFromStream, chrome/m156)
#[doc(alias = "SkJpegRustCodec::MakeFromStream")]
pub fn make_from_stream<'a>(
    stream: Box<dyn Stream + Send + 'a>,
) -> std::result::Result<Codec<'a>, Result> {
    let priv_stream = Arc::new(Mutex::new(stream));
    let mut reader = Reader::new(Arc::clone(&priv_stream));
    if !reader.metadata_loaded() {
        let metadata_result = reader.read_metadata();
        if metadata_result != DecodingResult::Success {
            return Err(map_decoding_result(metadata_result));
        }
    }

    let (Ok(width), Ok(height)) = (
        i32::try_from(reader.width()),
        i32::try_from(reader.height()),
    ) else {
        return Err(Result::InternalError);
    };
    let color = reader.color();

    // An ICC profile whose data space does not match the image's colour is dropped, as the C++
    // codec does.
    let icc_bytes = reader.icc_profile();
    let icc = if icc_bytes.is_empty() {
        None
    } else {
        skia_rust_skcms::parse(&icc_bytes).map(|profile| (icc_bytes, profile))
    };
    let mut info = EncodedInfo::make(width, height, color, Alpha::Opaque, 8);
    if let Some((bytes, profile)) = icc {
        let space = data_space(&profile);
        let matches = match color {
            Color::InvertedCMYK => space == DataSpace::Cmyk,
            Color::Gray => space == DataSpace::Gray || space == DataSpace::Rgb,
            _ => space == DataSpace::Rgb,
        };
        if matches {
            info = info.with_profile(Arc::from(bytes), profile);
        }
    }

    // The orientation comes from the EXIF data, through `SkExif::Parse`, when there is any.
    let mut origin = EncodedOrigin::TopLeft;
    let exif_bytes = reader.exif_data();
    if !exif_bytes.is_empty() {
        let mut metadata = exif::Metadata::default();
        exif::parse(&mut metadata, Some(&exif_bytes));
        if let Some(found) = metadata.origin {
            origin = found;
        }
    }

    let bytes_per_pixel: usize = match color {
        Color::RGB => 3,
        Color::Gray => 1,
        Color::InvertedCMYK => 4,
        _ => return Err(Result::InvalidInput),
    };
    // Reject images whose raw pixel buffer would exceed the `usize` range.
    let (Ok(width_u), Ok(height_u)) = (usize::try_from(width), usize::try_from(height)) else {
        return Err(Result::InternalError);
    };
    let Some(src_row_bytes) = width_u.checked_mul(bytes_per_pixel) else {
        return Err(Result::InternalError);
    };
    if height_u.checked_mul(src_row_bytes).is_none() {
        return Err(Result::InternalError);
    }

    let imp = JpegRustCodec {
        priv_stream,
        reader,
        incremental: None,
        swizzler: None,
        xform_buffer: Vec::new(),
    };
    // The base codec holds no stream: this codec reads its own, as Skia's does.
    Ok(Codec::new(
        info,
        Box::new(imp),
        None,
        origin,
        Some(PixelFormat::Rgba8888),
    ))
}

/// Port of `SkJpegRustDecoder::IsJpeg`: whether `buf` starts with the JPEG SOI marker.
// Port of: experimental/rust_jpeg/decoder/SkJpegRustDecoder.cpp#L10-L17 (IsJpeg, chrome/m156)
#[doc(alias = "SkJpegRustDecoder::IsJpeg")]
#[must_use]
pub fn is_jpeg(buf: &[u8]) -> bool {
    is_jpeg_data(buf)
}
