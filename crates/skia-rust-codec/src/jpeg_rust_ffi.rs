// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: experimental/rust_jpeg/ffi/FFI.rs (chrome/m156), the `Reader` that `SkJpegRustCodec`
// drives, with the cxx bridge removed. The Rust side of Skia's bridge is the implementation here:
// zune-jpeg decodes, and the `SkStreamAdapter` becomes a read over the codec's stream.
//
// Not ported yet: the Multi-Picture (MPF) and embedded-JPEG scanners, and the segment accessors
// they sit beside. Only the gainmap path of `SkJpegRustCodec` reads them, and that path is blocked
// on the XMP and ISO 21496-1 metadata decoder (`SkJpegMetadataDecoder`).

//! The streaming JPEG reader over zune-jpeg, with Rust-side segment scanning for the ICC and EXIF
//! metadata.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use skia_rust_core::stream::Stream;
use zune_core::bytestream::{ZByteIoError, ZByteReaderTrait, ZCursor, ZSeekFrom};
use zune_core::colorspace::ColorSpace as ZuneColorSpace;

use crate::encoded_info::Color;
use crate::jpeg_metadata::{extract_exif_data, extract_icc_profile};
use crate::jpeg_rust_segment_scan::{SegmentScanner, jpeg_marker};

/// The largest decoded image the reader allocates: 512 MiB. Port of `MAX_DECODE_IMAGE_BYTES`.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L430 (MAX_DECODE_IMAGE_BYTES, chrome/m156)
const MAX_DECODE_IMAGE_BYTES: usize = 512 * 1024 * 1024;

/// Port of `rust_jpeg::DecodingResult`.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L11-L19 (DecodingResult, chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodingResult {
    Success,
    FormatError,
    ParameterError,
    UnsupportedFeature,
    IncompleteInput,
    MemoryError,
}

/// Port of `rust_jpeg::DecodedRowsInfo`: where the rows returned by [`Reader::get_next_rows`] go.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L37-L46 (DecodedRowsInfo, chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DecodedRowsInfo {
    /// The destination row index where the buffer should start being copied.
    pub(crate) dst_row_start: u32,
    pub(crate) row_count: u32,
    /// Whether these rows replace a previous progressive preview rather than extending the
    /// stable final-output prefix.
    pub(crate) is_preview: bool,
}

/// Port of `rust_jpeg::is_jpeg_data`: the two-byte SOI check of `SkJpegRustDecoder::IsJpeg`.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L387-L389 (is_jpeg_data, chrome/m156)
pub(crate) fn is_jpeg_data(data: &[u8]) -> bool {
    data.len() >= 2 && data[0] == 0xFF && data[1] == jpeg_marker::SOI
}

/// Locks a shared buffer or stream, recovering the guard if a previous holder panicked.
fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Read all available bytes from the stream into `sink`, without blocking on more. Port of
/// `read_available_from_stream`: returns the number of bytes appended.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L321-L343 (read_available_from_stream, chrome/m156)
fn read_available_from_stream(
    stream: &Mutex<Box<dyn Stream + Send + '_>>,
    sink: &mut Vec<u8>,
) -> usize {
    let initial_sink_len = sink.len();
    let mut buffer = [0u8; 8192];
    let mut stream = lock(stream);
    loop {
        let bytes_read = stream.read(&mut buffer);
        if bytes_read == 0 {
            break;
        }
        sink.extend_from_slice(&buffer[..bytes_read]);
    }
    sink.len() - initial_sink_len
}

/// Map zune-jpeg errors to our `DecodingResult`. Port of `map_zune_error`.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L345-L353 (map_zune_error, chrome/m156)
fn map_zune_error(err: &zune_jpeg::errors::DecodeErrors) -> DecodingResult {
    use zune_jpeg::errors::DecodeErrors::{LargeDimensions, Unsupported};
    match err {
        Unsupported(_) => DecodingResult::UnsupportedFeature,
        LargeDimensions(_) => DecodingResult::ParameterError,
        _ => DecodingResult::FormatError,
    }
}

/// Map a zune-core colour space to the colour and bytes per pixel of the decoded output. Port of
/// `map_color_space`.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L355-L366 (map_color_space, chrome/m156)
fn map_color_space(cs: ZuneColorSpace) -> (Color, u32) {
    match cs {
        ZuneColorSpace::Luma | ZuneColorSpace::LumaA => (Color::Gray, 1),
        // JPEG CMYK/YCCK is stored as inverted CMYK (255 = no ink). The raw inverted CMYK goes to
        // the swizzler, which does the CMYK to RGB conversion, optionally with a CMYK profile.
        ZuneColorSpace::CMYK | ZuneColorSpace::YCCK => (Color::InvertedCMYK, 4),
        _ => (Color::RGB, 3),
    }
}

/// The colour space zune-jpeg decodes to: greyscale stays grey, CMYK stays raw CMYK, and anything
/// else becomes RGB. Shared by the full and the incremental decode.
// Port of: experimental/rust_jpeg/ffi/FFI.rs (the target_cs match in read_image_data and in
// read_incremental_image_data, chrome/m156)
fn output_color_space(orig: ZuneColorSpace) -> ZuneColorSpace {
    match orig {
        ZuneColorSpace::Luma | ZuneColorSpace::LumaA => ZuneColorSpace::Luma,
        ZuneColorSpace::CMYK | ZuneColorSpace::YCCK => ZuneColorSpace::CMYK,
        _ => ZuneColorSpace::RGB,
    }
}

/// The rows `[row_start, row_start + row_count)` of an image with `row_bytes` per row, or `None`
/// if they are out of bounds. Port of `decoded_row_slice`.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L368-L378 (decoded_row_slice, chrome/m156)
fn decoded_row_slice(
    image_data: &[u8],
    row_start: u32,
    row_count: u32,
    row_bytes: usize,
) -> Option<&[u8]> {
    let start = (row_start as usize).checked_mul(row_bytes)?;
    let size = (row_count as usize).checked_mul(row_bytes)?;
    let end = start.checked_add(size)?;
    image_data.get(start..end)
}

/// A reader over a buffer that grows as the stream delivers bytes. zune-jpeg is resumed on it:
/// when it runs out of bytes it reports a recoverable EOF, and the next attempt sees more data.
/// Port of `GrowingJpegCursor`. The buffer is shared, so that the decoder reads the bytes the
/// reader has appended, and the buffer is `Arc<Mutex<..>>` because the codec must be `Send`.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L86-L108 (GrowingJpegCursor, chrome/m156)
#[derive(Clone)]
struct GrowingJpegCursor {
    data: Arc<Mutex<Vec<u8>>>,
    position: usize,
}

impl GrowingJpegCursor {
    fn new(data: Arc<Mutex<Vec<u8>>>) -> Self {
        Self { data, position: 0 }
    }

    fn available_from_position(&self) -> usize {
        lock(&self.data).len().saturating_sub(self.position)
    }

    fn add_signed_offset(base: usize, offset: i64) -> Option<usize> {
        if offset >= 0 {
            base.checked_add(usize::try_from(offset).ok()?)
        } else {
            base.checked_sub(usize::try_from(offset.checked_neg()?).ok()?)
        }
    }
}

// Port of: experimental/rust_jpeg/ffi/FFI.rs#L110-L221 (ZByteReaderTrait for GrowingJpegCursor)
impl ZByteReaderTrait for GrowingJpegCursor {
    fn read_byte_no_error(&mut self) -> u8 {
        let byte = *lock(&self.data).get(self.position).unwrap_or(&0);
        self.position = self.position.wrapping_add(1);
        byte
    }

    fn read_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        let available = self.available_from_position();
        if available < buf.len() {
            return Err(ZByteIoError::NotEnoughBytes(available, buf.len()));
        }
        let data = lock(&self.data);
        let end = self.position + buf.len();
        buf.copy_from_slice(&data[self.position..end]);
        self.position = end;
        Ok(())
    }

    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        let bytes_to_read = self.available_from_position().min(buf.len());
        let data = lock(&self.data);
        let end = self.position + bytes_to_read;
        buf[..bytes_to_read].copy_from_slice(&data[self.position..end]);
        self.position = end;
        Ok(bytes_to_read)
    }

    fn peek_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        let bytes_to_read = self.available_from_position().min(buf.len());
        let data = lock(&self.data);
        let end = self.position + bytes_to_read;
        buf[..bytes_to_read].copy_from_slice(&data[self.position..end]);
        Ok(bytes_to_read)
    }

    fn peek_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        let available = self.available_from_position();
        if available < buf.len() {
            return Err(ZByteIoError::NotEnoughBytes(available, buf.len()));
        }
        let data = lock(&self.data);
        let end = self.position + buf.len();
        buf.copy_from_slice(&data[self.position..end]);
        Ok(())
    }

    fn z_seek(&mut self, from: ZSeekFrom) -> Result<u64, ZByteIoError> {
        let new_position = match from {
            ZSeekFrom::Start(position) => usize::try_from(position)?,
            ZSeekFrom::End(offset) => {
                let len = lock(&self.data).len();
                Self::add_signed_offset(len, offset)
                    .ok_or(ZByteIoError::SeekError("Negative seek"))?
            }
            ZSeekFrom::Current(offset) => Self::add_signed_offset(self.position, offset)
                .ok_or(ZByteIoError::SeekError("Negative seek"))?,
        };
        self.position = new_position;
        Ok(u64::try_from(self.position)?)
    }

    fn is_eof(&mut self) -> Result<bool, ZByteIoError> {
        Ok(self.position >= lock(&self.data).len())
    }

    fn z_position(&mut self) -> Result<u64, ZByteIoError> {
        Ok(u64::try_from(self.position)?)
    }

    fn read_remaining(&mut self, sink: &mut Vec<u8>) -> Result<usize, ZByteIoError> {
        let data = lock(&self.data);
        let start = self.position.min(data.len());
        sink.extend_from_slice(&data[start..]);
        let bytes_read = data.len() - start;
        self.position = data.len();
        Ok(bytes_read)
    }
}

/// The zune-jpeg decoder that resumes over the growing buffer.
type IncrementalJpegDecoder = zune_jpeg::JpegDecoder<GrowingJpegCursor>;

/// Streaming JPEG reader wrapping zune-jpeg with Rust-side segment scanning. Port of
/// `rust_jpeg::Reader`.
// Port of: experimental/rust_jpeg/ffi/FFI.rs#L223-L266 (Reader, chrome/m156)
pub(crate) struct Reader<'a> {
    stream: Arc<Mutex<Box<dyn Stream + Send + 'a>>>,
    raw_data: Arc<Mutex<Vec<u8>>>,
    stream_exhausted: bool,
    orig_color_space: ZuneColorSpace,
    metadata_loaded: bool,
    width: u32,
    height: u32,
    color: Color,
    bytes_per_pixel: u32,
    image_data: Vec<u8>,
    image_data_loaded: bool,
    /// Append-only stable output prefix. May be less than height for truncated images.
    valid_rows: u32,
    /// Stable rows already returned by `get_next_rows`.
    last_consumed_row_count: u32,
    /// Progressive previews are replaceable full-frame renders. Track the completed scan
    /// represented by the current preview separately from the append-only stable row prefix.
    preview_rows: u32,
    preview_scan_count: usize,
    /// Last preview generation returned by `get_next_rows`.
    last_consumed_preview_scan_count: usize,
    incremental_decoder: Option<IncrementalJpegDecoder>,

    scanner: SegmentScanner,
    scanner_bytes_consumed: usize,
    cached_icc: Option<Vec<u8>>,
    cached_exif: Option<Vec<u8>>,
}

impl<'a> Reader<'a> {
    /// Create a new reader without reading any data from the stream. The stream is shared with
    /// the codec, so that a rewind can reach it, and it is kept for later incremental reads.
    // Port of: experimental/rust_jpeg/ffi/FFI.rs#L268-L290 (Reader::new, chrome/m156)
    pub(crate) fn new(stream: Arc<Mutex<Box<dyn Stream + Send + 'a>>>) -> Self {
        Self {
            stream,
            raw_data: Arc::new(Mutex::new(Vec::new())),
            stream_exhausted: false,
            orig_color_space: ZuneColorSpace::RGB,
            metadata_loaded: false,
            width: 0,
            height: 0,
            color: Color::RGB,
            bytes_per_pixel: 3,
            image_data: Vec::new(),
            image_data_loaded: false,
            valid_rows: 0,
            last_consumed_row_count: 0,
            preview_rows: 0,
            preview_scan_count: 0,
            last_consumed_preview_scan_count: 0,
            incremental_decoder: None,
            // Stop at SOS: only the header segments are needed for metadata.
            scanner: SegmentScanner::new(jpeg_marker::SOS),
            scanner_bytes_consumed: 0,
            cached_icc: None,
            cached_exif: None,
        }
    }

    /// Try to read more data from the stream into `raw_data`, and feed any new bytes to the
    /// segment scanner.
    // Port of: experimental/rust_jpeg/ffi/FFI.rs#L292-L318 (Reader::try_read_more, chrome/m156)
    fn try_read_more(&mut self) {
        if self.stream_exhausted {
            return;
        }
        let bytes_read = read_available_from_stream(&self.stream, &mut lock(&self.raw_data));
        if bytes_read == 0 {
            self.stream_exhausted = true;
        }
        // Feed any new bytes to the segment scanner.
        self.advance_scanner();
    }

    // Port of: experimental/rust_jpeg/ffi/FFI.rs#L320-L352 (Reader::advance_scanner, chrome/m156)
    fn advance_scanner(&mut self) {
        if self.scanner.is_done() || self.scanner.had_error() {
            return;
        }
        let raw_data = lock(&self.raw_data);
        if self.scanner_bytes_consumed < raw_data.len() {
            let new_bytes = &raw_data[self.scanner_bytes_consumed..];
            self.scanner.on_bytes(new_bytes);
            self.scanner_bytes_consumed = raw_data.len();
        }
        // Populate the caches once scanning completes.
        if self.scanner.is_done() && self.cached_icc.is_none() {
            self.cached_icc = Some(extract_icc_profile(&raw_data, &self.scanner.segments));
            if self.cached_exif.is_none() {
                self.cached_exif = Some(extract_exif_data(&raw_data, &self.scanner.segments));
            }
        }
    }

    /// Port of `Reader::width`.
    pub(crate) fn width(&self) -> u32 {
        self.width
    }

    /// Port of `Reader::height`.
    pub(crate) fn height(&self) -> u32 {
        self.height
    }

    /// Port of `Reader::color`, as the `Color` of `SkEncodedInfo`.
    pub(crate) fn color(&self) -> Color {
        self.color
    }

    /// Port of `Reader::metadata_loaded`.
    pub(crate) fn metadata_loaded(&self) -> bool {
        self.metadata_loaded
    }

    /// Port of `Reader::image_data_loaded`.
    pub(crate) fn image_data_loaded(&self) -> bool {
        self.image_data_loaded
    }

    /// Reads the JPEG headers: the dimensions and the colour space. Port of `Reader::read_metadata`.
    // Port of: experimental/rust_jpeg/ffi/FFI.rs#L389-L446 (Reader::read_metadata, chrome/m156)
    pub(crate) fn read_metadata(&mut self) -> DecodingResult {
        if self.metadata_loaded {
            return DecodingResult::Success;
        }

        self.try_read_more();

        let raw_data = lock(&self.raw_data);
        if !is_jpeg_data(&raw_data) {
            if raw_data.len() < 2 && !self.stream_exhausted {
                return DecodingResult::IncompleteInput;
            }
            return DecodingResult::FormatError;
        }

        let options = zune_core::options::DecoderOptions::default().set_strict_mode(false);
        let mut decoder =
            zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(raw_data.as_slice()), options);

        match decoder.decode_headers() {
            Ok(()) => {}
            Err(ref e) => {
                if !self.stream_exhausted {
                    return DecodingResult::IncompleteInput;
                }
                return map_zune_error(e);
            }
        }

        let Some((width, height)) = decoder.dimensions() else {
            return DecodingResult::FormatError;
        };
        let cs = decoder.input_colorspace().unwrap_or(ZuneColorSpace::RGB);
        let (color, bytes_per_pixel) = map_color_space(cs);
        drop(decoder);
        drop(raw_data);

        let (Ok(width), Ok(height)) = (u32::try_from(width), u32::try_from(height)) else {
            return DecodingResult::MemoryError;
        };
        if width.checked_mul(bytes_per_pixel).is_none() {
            return DecodingResult::MemoryError;
        }

        self.width = width;
        self.height = height;
        self.color = color;
        self.bytes_per_pixel = bytes_per_pixel;
        self.orig_color_space = cs;
        self.metadata_loaded = true;

        DecodingResult::Success
    }

    /// Decodes the whole image into the reader's buffer. Port of `Reader::read_image_data`.
    // Port of: experimental/rust_jpeg/ffi/FFI.rs#L448-L517 (Reader::read_image_data, chrome/m156)
    pub(crate) fn read_image_data(&mut self) -> DecodingResult {
        if self.image_data_loaded {
            return DecodingResult::Success;
        }
        if !self.metadata_loaded {
            return DecodingResult::ParameterError;
        }

        self.try_read_more();

        let target_cs = output_color_space(self.orig_color_space);

        // zune-jpeg defaults to a 100-scan limit for progressive JPEGs, matching Blink's
        // ProgressMonitor, so the limit is not set here.
        let options = zune_core::options::DecoderOptions::default()
            .set_strict_mode(false)
            .jpeg_set_out_colorspace(target_cs);

        let raw_data = lock(&self.raw_data);
        let mut decoder =
            zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(raw_data.as_slice()), options);

        let Some(total_bytes) = (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|v| v.checked_mul(self.bytes_per_pixel as usize))
        else {
            return DecodingResult::MemoryError;
        };
        if total_bytes > MAX_DECODE_IMAGE_BYTES {
            return DecodingResult::MemoryError;
        }

        let mut image_data = Vec::new();
        if image_data.try_reserve_exact(total_bytes).is_err() {
            return DecodingResult::MemoryError;
        }
        image_data.resize(total_bytes, 0);
        self.image_data = image_data;

        match decoder.decode_into(&mut self.image_data) {
            Ok(()) => {
                self.valid_rows = self.height;
                self.image_data_loaded = true;
                DecodingResult::Success
            }
            Err(ref e) => {
                if !self.stream_exhausted {
                    self.image_data.clear();
                    return DecodingResult::IncompleteInput;
                }
                let rows = decoder
                    .decoded_scanlines()
                    .and_then(|rows| u32::try_from(rows).ok())
                    .unwrap_or(0)
                    .min(self.height);
                if rows > 0 {
                    self.valid_rows = rows;
                    self.image_data_loaded = true;
                    return DecodingResult::Success;
                }
                self.image_data.clear();
                map_zune_error(e)
            }
        }
    }

    /// Decodes as much of the image as the stream allows, resuming the same zune-jpeg decoder on
    /// each call. Port of `Reader::read_incremental_image_data`.
    // Port of: experimental/rust_jpeg/ffi/FFI.rs#L519-L610 (Reader::read_incremental_image_data,
    // chrome/m156)
    pub(crate) fn read_incremental_image_data(&mut self) -> DecodingResult {
        if self.image_data_loaded {
            return DecodingResult::Success;
        }
        if !self.metadata_loaded {
            return DecodingResult::ParameterError;
        }

        // A zero-byte read during incremental decode only means that no new bytes are visible
        // for this attempt. The codec's caller may make more bytes visible and call again.
        self.stream_exhausted = false;
        self.try_read_more();

        let Some(total_bytes) = (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|v| v.checked_mul(self.bytes_per_pixel as usize))
        else {
            return DecodingResult::MemoryError;
        };
        if total_bytes > MAX_DECODE_IMAGE_BYTES {
            return DecodingResult::MemoryError;
        }
        if self.image_data.len() != total_bytes {
            let mut image_data = Vec::new();
            if image_data.try_reserve_exact(total_bytes).is_err() {
                return DecodingResult::MemoryError;
            }
            image_data.resize(total_bytes, 0);
            self.image_data = image_data;
        }

        let target_cs = output_color_space(self.orig_color_space);
        let raw_data = Arc::clone(&self.raw_data);
        let decoder = self.incremental_decoder.get_or_insert_with(|| {
            let options = zune_core::options::DecoderOptions::default()
                .set_strict_mode(false)
                .jpeg_set_out_colorspace(target_cs);
            let mut decoder =
                zune_jpeg::JpegDecoder::new_with_options(GrowingJpegCursor::new(raw_data), options);
            decoder.set_incremental_mode(true);
            decoder
        });

        match decoder.decode_into(&mut self.image_data) {
            Ok(()) => {
                self.valid_rows = self.height;
                self.image_data_loaded = true;
                DecodingResult::Success
            }
            Err(ref e) if e.is_recoverable_eof() => {
                self.valid_rows = decoder
                    .decoded_scanlines()
                    .and_then(|rows| u32::try_from(rows).ok())
                    .unwrap_or(0)
                    .min(self.height);
                self.preview_rows = decoder
                    .decoded_preview_scanlines()
                    .and_then(|rows| u32::try_from(rows).ok())
                    .unwrap_or(0)
                    .min(self.height);
                self.preview_scan_count = decoder.decoded_scans().unwrap_or(0);
                DecodingResult::IncompleteInput
            }
            Err(ref e) => {
                self.image_data.clear();
                map_zune_error(e)
            }
        }
    }

    /// Returns the rows that are new since the last call, and where they go. A progressive
    /// preview replaces the rows from zero. Port of `Reader::get_next_rows`.
    // Port of: experimental/rust_jpeg/ffi/FFI.rs#L612-L689 (Reader::get_next_rows, chrome/m156)
    pub(crate) fn get_next_rows(&mut self) -> (DecodedRowsInfo, &[u8]) {
        let no_rows = DecodedRowsInfo {
            dst_row_start: 0,
            row_count: 0,
            is_preview: false,
        };
        // Successful output is final even when the last incremental attempt previously exposed
        // a progressive preview.
        if !self.image_data_loaded
            && self.preview_rows > 0
            && self.preview_scan_count > self.last_consumed_preview_scan_count
        {
            let row_bytes = self.row_bytes() as usize;
            let Some(preview) =
                decoded_row_slice(&self.image_data, 0, self.preview_rows, row_bytes)
            else {
                return (no_rows, &[]);
            };
            self.last_consumed_preview_scan_count = self.preview_scan_count;
            let info = DecodedRowsInfo {
                dst_row_start: 0,
                row_count: self.preview_rows,
                is_preview: true,
            };
            return (info, preview);
        }

        let already_consumed = self.last_consumed_row_count;
        let row_bytes = self.row_bytes() as usize;

        // `decoded_scanlines()` is an append-only stable prefix, and is useful before the
        // complete image has loaded (notably for baseline JPEGs).
        let current_rows = self.valid_rows;
        if current_rows <= already_consumed {
            return (no_rows, &[]);
        }

        let new_row_count = current_rows - already_consumed;
        let Some(rows) =
            decoded_row_slice(&self.image_data, already_consumed, new_row_count, row_bytes)
        else {
            return (no_rows, &[]);
        };
        self.last_consumed_row_count = current_rows;
        let info = DecodedRowsInfo {
            dst_row_start: already_consumed,
            row_count: new_row_count,
            is_preview: false,
        };
        (info, rows)
    }

    /// Port of `Reader::row_bytes`: the bytes per decoded row, or 0 if that overflows.
    pub(crate) fn row_bytes(&self) -> u32 {
        self.width.checked_mul(self.bytes_per_pixel).unwrap_or(0)
    }

    /// Port of `Reader::reset_decode_state`: forgets the decoded rows and the incremental decoder.
    pub(crate) fn reset_decode_state(&mut self) {
        self.image_data.clear();
        self.image_data_loaded = false;
        self.valid_rows = 0;
        self.last_consumed_row_count = 0;
        self.preview_rows = 0;
        self.preview_scan_count = 0;
        self.last_consumed_preview_scan_count = 0;
        self.incremental_decoder = None;
        self.stream_exhausted = false;
    }

    /// Port of `Reader::icc_profile`: the reassembled ICC profile, or empty if there is none.
    pub(crate) fn icc_profile(&self) -> Vec<u8> {
        if let Some(ref cached) = self.cached_icc {
            return cached.clone();
        }
        extract_icc_profile(&lock(&self.raw_data), &self.scanner.segments)
    }

    /// Port of `Reader::exif_data`: the TIFF bytes of the EXIF APP1 segment, or empty.
    pub(crate) fn exif_data(&self) -> Vec<u8> {
        if let Some(ref cached) = self.cached_exif {
            return cached.clone();
        }
        extract_exif_data(&lock(&self.raw_data), &self.scanner.segments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::jpeg_rust_encoder::{
        EncodingResult, JpegEncodeAlpha, JpegEncodeColor, encode_jpeg,
    };

    #[test]
    fn test_decoded_row_slice_checks_bounds() {
        let pixels: Vec<u8> = (0..24).collect();
        assert_eq!(decoded_row_slice(&pixels, 1, 2, 6), Some(&pixels[6..18]));
        assert_eq!(decoded_row_slice(&pixels, 3, 2, 6), None);
        assert_eq!(
            decoded_row_slice(&pixels, u32::MAX, u32::MAX, usize::MAX),
            None
        );
    }

    #[test]
    fn test_is_jpeg_data() {
        assert!(is_jpeg_data(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(!is_jpeg_data(&[0x89, b'P', b'N', b'G']));
        assert!(!is_jpeg_data(b"BM"));
        assert!(!is_jpeg_data(&[0xFF]));
        assert!(!is_jpeg_data(&[]));
    }

    // The test pixels wrap on purpose: they are only a varied, deterministic image.
    #[allow(clippy::cast_possible_truncation)]
    #[test]
    fn test_growing_cursor_supports_zune_incremental_retries() {
        let width = 16u32;
        let height = 16u32;
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                pixels.push((x * 13) as u8);
                pixels.push((y * 11) as u8);
                pixels.push(((x + y) * 7) as u8);
                pixels.push(0xFF);
            }
        }

        let mut jpeg = Vec::new();
        assert_eq!(
            encode_jpeg(
                &pixels,
                width,
                height,
                width * 4,
                JpegEncodeColor::Rgba,
                JpegEncodeAlpha::Ignore,
                90,
                &mut jpeg,
            ),
            EncodingResult::Success
        );

        let input = Arc::new(Mutex::new(Vec::new()));
        let options = zune_core::options::DecoderOptions::default()
            .set_strict_mode(false)
            .jpeg_set_out_colorspace(ZuneColorSpace::RGB);
        let mut decoder = zune_jpeg::JpegDecoder::new_with_options(
            GrowingJpegCursor::new(Arc::clone(&input)),
            options,
        );
        decoder.set_incremental_mode(true);

        let mut output = Vec::new();
        let mut headers_done = false;
        let mut done = false;

        for chunk in jpeg.chunks(23) {
            lock(&input).extend_from_slice(chunk);

            if !headers_done {
                match decoder.decode_headers() {
                    Ok(()) => {
                        headers_done = true;
                        output.resize(decoder.output_buffer_size().unwrap(), 0);
                    }
                    Err(ref e) if e.is_recoverable_eof() => continue,
                    Err(e) => panic!("unexpected header error: {e:?}"),
                }
            }

            match decoder.decode_into(&mut output) {
                Ok(()) => {
                    done = true;
                    break;
                }
                Err(ref e) if e.is_recoverable_eof() => {
                    let stable_scanlines = decoder.decoded_scanlines();
                    assert!(stable_scanlines.is_some());

                    match decoder.decode_into(&mut output) {
                        Err(ref e) if e.is_recoverable_eof() => {}
                        other => panic!("expected retryable EOF without new bytes, got {other:?}"),
                    }
                    assert_eq!(decoder.decoded_scanlines(), stable_scanlines);
                }
                Err(e) => panic!("unexpected decode error: {e:?}"),
            }
        }

        if !done {
            decoder.decode_into(&mut output).unwrap();
        }
        assert_eq!(decoder.decoded_scanlines(), Some(height as usize));
    }
}
