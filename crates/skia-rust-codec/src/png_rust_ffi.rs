// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: rust/png/FFI.rs (chrome/m156), the non-`for_android` build, with the cxx bridge removed.
// The Rust side of Skia's bridge is the implementation here, so the wrapper calls the `png` crate
// directly and the `SkStreamAdapter` becomes a `Read` over the codec's stream.

//! A thin wrapper around `png::Reader` that exposes what `SkPngRustCodec` reads from it: the
//! header facts, the chunks it takes, the frame control, and the row decoding.

use std::io::{self, BufReader, ErrorKind, Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::stream::Stream;

/// Port of `rust_png::DecodingResult`: a simplification of `Option<png::DecodingError>`.
// Port of: rust/png/FFI.rs#L36-L70 (chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodingResult {
    Success,
    FormatError,
    ParameterError,
    LimitsExceededError,
    /// Equivalent to `png::DecodingError::IoError(UnexpectedEof)`.
    IncompleteInput,
    OtherIoError,
    /// No more frames or rows (`read_row` returned `None`).
    EndOfFrame,
}

// Port of: rust/png/FFI.rs#L295-L312 (`From<Option<&png::DecodingError>>`, chrome/m156)
impl From<Option<&png::DecodingError>> for DecodingResult {
    fn from(option: Option<&png::DecodingError>) -> Self {
        match option {
            None => Self::Success,
            Some(png::DecodingError::IoError(e)) => {
                if e.kind() == ErrorKind::UnexpectedEof {
                    Self::IncompleteInput
                } else {
                    Self::OtherIoError
                }
            }
            Some(png::DecodingError::Format(_)) => Self::FormatError,
            Some(png::DecodingError::Parameter(_)) => Self::ParameterError,
            Some(png::DecodingError::LimitsExceeded) => Self::LimitsExceededError,
        }
    }
}

/// The primaries of a `cHRM` chunk, in the order `SkColorSpacePrimaries` takes them.
// Port of: rust/png/FFI.rs ColorSpacePrimaries (chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ColorSpacePrimaries {
    pub(crate) rx: f32,
    pub(crate) ry: f32,
    pub(crate) gx: f32,
    pub(crate) gy: f32,
    pub(crate) bx: f32,
    pub(crate) by: f32,
    pub(crate) wx: f32,
    pub(crate) wy: f32,
}

/// The `Read` side of `SkStreamAdapter`: reads from the codec's stream. The stream is shared with
/// the codec, which rewinds it and asks for its length.
// Port of: rust/common/SkStreamAdapter.h (chrome/m156)
pub(crate) struct StreamAdapter<'a> {
    pub(crate) stream: Arc<Mutex<Box<dyn Stream + Send + 'a>>>,
}

impl Read for StreamAdapter<'_> {
    // Port of: rust/common/SkStreamAdapter.cpp (SkStreamAdapter::read)
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut stream = self.stream.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(stream.read(buf))
    }
}

impl StreamAdapter<'_> {
    // Port of: rust/common/SkStreamAdapter.cpp (SkStreamAdapter::seek_from_start)
    fn stream_seek_from_start(&self, requested_pos: u64) -> Option<u64> {
        let mut stream = self.stream.lock().unwrap_or_else(PoisonError::into_inner);
        let pos = usize::try_from(requested_pos).ok()?;
        if !stream.seek(pos) {
            return None;
        }
        Some(requested_pos)
    }

    // Port of: rust/common/SkStreamAdapter.cpp (SkStreamAdapter::seek_from_end)
    fn stream_seek_from_end(&self, requested_offset: i64) -> Option<u64> {
        let length = {
            let stream = self.stream.lock().unwrap_or_else(PoisonError::into_inner);
            if !stream.has_length() {
                return None;
            }
            stream.get_length()
        };
        // `SkStream` does not read beyond its length.
        if requested_offset > 0 {
            return None;
        }
        let offset = usize::try_from(requested_offset.unsigned_abs()).ok()?;
        let end_pos = u64::try_from(length).ok()?;
        self.stream_seek_from_start(end_pos.checked_sub(offset as u64)?)
    }

    // Port of: rust/common/SkStreamAdapter.cpp (SkStreamAdapter::seek_relative)
    fn stream_seek_relative(&self, requested_offset: i64) -> Option<u64> {
        let mut stream = self.stream.lock().unwrap_or_else(PoisonError::into_inner);
        if !stream.has_position() {
            return None;
        }
        // Only move the stream when the offset is non-zero, as a zero offset is a no-op.
        if requested_offset != 0 && !stream.move_by(requested_offset) {
            return None;
        }
        u64::try_from(stream.get_position()).ok()
    }
}

impl Seek for StreamAdapter<'_> {
    // Port of: rust/common/io_traits_ffi.rs (Seek for SkStreamAdapter)
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let found = match pos {
            SeekFrom::Start(requested) => self.stream_seek_from_start(requested),
            SeekFrom::End(requested) => self.stream_seek_from_end(requested),
            SeekFrom::Current(requested) => self.stream_seek_relative(requested),
        };
        found.ok_or_else(|| io::Error::other("seek failed"))
    }
}

/// The capacity of the buffered input. Port of `BUF_CAPACITY` in `Reader::new`.
const BUF_CAPACITY: usize = 32 * 1024;

/// Port of `rust_png::Reader`.
// Port of: rust/png/FFI.rs#L571-L585 (chrome/m156)
pub(crate) struct Reader<'a> {
    reader: png::Reader<BufReader<StreamAdapter<'a>>>,
    last_interlace_info: Option<png::InterlaceInfo>,
}

/// Port of `compute_transformations`: the transforms the decoder asks `png` for.
// Port of: rust/png/FFI.rs#L379-L422 (chrome/m156), without the
// `skia_png_new_gray16_behavior_for_crbug359245096` feature (off in every Skia build)
fn compute_transformations(info: &png::Info<'_>) -> png::Transformations {
    let mut result = match info.color_type {
        png::ColorType::Rgba | png::ColorType::GrayscaleAlpha if (info.bit_depth as u8) < 8 => {
            png::Transformations::EXPAND
        }
        png::ColorType::Rgb | png::ColorType::Grayscale => png::Transformations::EXPAND,
        png::ColorType::Indexed | png::ColorType::Rgba | png::ColorType::GrayscaleAlpha => {
            png::Transformations::IDENTITY
        }
    };
    if info.bit_depth == png::BitDepth::Sixteen
        && matches!(
            info.color_type,
            png::ColorType::Grayscale | png::ColorType::GrayscaleAlpha
        )
    {
        result |= png::Transformations::STRIP_16;
    }
    result
}

/// Port of `png_u32_into_f32`.
// Port of: rust/png/FFI.rs#L1009-L1015 (chrome/m156)
fn png_u32_into_f32(v: png::ScaledFloat) -> f32 {
    // This uses `0.00001_f32 * (v.into_scaled() as f32)` instead of just `v.into_value()` for
    // compatibility with the legacy `ReadColorProfile` in Blink's PNG decoder.
    #[allow(clippy::cast_precision_loss)] // mirrors `(float)` in the C++ cast
    let scaled = v.into_scaled() as f32;
    0.00001_f32 * scaled
}

impl<'a> Reader<'a> {
    /// Port of `Reader::new`: reads the header and sets up the transforms.
    // Port of: rust/png/FFI.rs#L586-L615 (chrome/m156), non-`for_android` build
    pub(crate) fn new(stream: Arc<Mutex<Box<dyn Stream + Send + 'a>>>) -> Result<Self, png::DecodingError> {
        let input = BufReader::with_capacity(BUF_CAPACITY, StreamAdapter { stream });
        let mut decoder = {
            let mut options = png::DecodeOptions::default();
            options.set_ignore_text_chunk(true);
            png::Decoder::new_with_options(input, options)
        };
        let info = decoder.read_header_info()?;
        let transformations = compute_transformations(info);
        decoder.set_transformations(transformations);
        Ok(Self {
            reader: decoder.read_info()?,
            last_interlace_info: None,
        })
    }

    /// Port of `Reader::height`.
    pub(crate) fn height(&self) -> u32 {
        self.reader.info().height
    }

    /// Port of `Reader::width`.
    pub(crate) fn width(&self) -> u32 {
        self.reader.info().width
    }

    /// Port of `Reader::interlaced`.
    pub(crate) fn interlaced(&self) -> bool {
        self.reader.info().interlaced
    }

    /// Port of `Reader::is_srgb`.
    pub(crate) fn is_srgb(&self) -> bool {
        self.reader.info().srgb.is_some()
    }

    /// Port of `Reader::try_get_chrm`.
    pub(crate) fn try_get_chrm(&self) -> Option<ColorSpacePrimaries> {
        let chrm = self.reader.info().chrm_chunk.as_ref()?;
        Some(ColorSpacePrimaries {
            wx: png_u32_into_f32(chrm.white.0),
            wy: png_u32_into_f32(chrm.white.1),
            rx: png_u32_into_f32(chrm.red.0),
            ry: png_u32_into_f32(chrm.red.1),
            gx: png_u32_into_f32(chrm.green.0),
            gy: png_u32_into_f32(chrm.green.1),
            bx: png_u32_into_f32(chrm.blue.0),
            by: png_u32_into_f32(chrm.blue.1),
        })
    }

    /// Port of `Reader::try_get_cicp_chunk`: the (primaries, transfer, matrix, full range) values.
    pub(crate) fn try_get_cicp_chunk(&self) -> Option<(u8, u8, u8, bool)> {
        let cicp = self.reader.info().coding_independent_code_points.as_ref()?;
        Some((
            cicp.color_primaries,
            cicp.transfer_function,
            cicp.matrix_coefficients,
            cicp.is_video_full_range_image,
        ))
    }

    /// Port of `Reader::try_get_gama`.
    pub(crate) fn try_get_gama(&self) -> Option<f32> {
        self.reader.info().gama_chunk.map(png_u32_into_f32)
    }

    /// Port of `Reader::has_exif_chunk` and `get_exif_chunk`.
    pub(crate) fn get_exif_chunk(&self) -> Option<&[u8]> {
        self.reader.info().exif_metadata.as_deref()
    }

    /// Port of `Reader::has_iccp_chunk` and `get_iccp_chunk`.
    pub(crate) fn get_iccp_chunk(&self) -> Option<&[u8]> {
        self.reader.info().icc_profile.as_deref()
    }

    /// Port of `Reader::has_trns_chunk` and `get_trns_chunk`.
    pub(crate) fn get_trns_chunk(&self) -> Option<&[u8]> {
        self.reader.info().trns.as_deref()
    }

    /// Port of `Reader::has_plte_chunk` and `get_plte_chunk`.
    pub(crate) fn get_plte_chunk(&self) -> Option<&[u8]> {
        self.reader.info().palette.as_deref()
    }

    /// Port of `Reader::has_sbit_chunk` and `get_sbit_chunk`.
    pub(crate) fn get_sbit_chunk(&self) -> Option<&[u8]> {
        self.reader.info().sbit.as_deref()
    }

    /// Port of `Reader::has_plte_chunk`.
    pub(crate) fn has_plte_chunk(&self) -> bool {
        self.reader.info().palette.is_some()
    }

    /// Port of `Reader::has_actl_chunk`: the `acTL` chunk (the non-`for_android` build).
    pub(crate) fn has_actl_chunk(&self) -> bool {
        self.reader.info().animation_control.is_some()
    }

    /// Port of `Reader::get_actl_num_frames`. Only called when `has_actl_chunk` is true.
    pub(crate) fn get_actl_num_frames(&self) -> u32 {
        self.reader
            .info()
            .animation_control
            .as_ref()
            .map_or(0, |actl| actl.num_frames)
    }

    /// Port of `Reader::get_actl_num_plays`. Only called when `has_actl_chunk` is true.
    pub(crate) fn get_actl_num_plays(&self) -> u32 {
        self.reader
            .info()
            .animation_control
            .as_ref()
            .map_or(0, |actl| actl.num_plays)
    }

    /// Port of `Reader::has_fctl_chunk`: the `fcTL` chunk (the non-`for_android` build).
    pub(crate) fn has_fctl_chunk(&self) -> bool {
        self.reader.info().frame_control.is_some()
    }

    /// Port of `Reader::get_fctl_info`: (width, height, x, y, dispose, blend, duration in ms).
    /// Only called when `has_fctl_chunk` is true.
    // Port of: rust/png/FFI.rs#L873-L894 (chrome/m156)
    pub(crate) fn get_fctl_info(
        &self,
    ) -> (u32, u32, u32, u32, png::DisposeOp, png::BlendOp, u32) {
        let fctl = self
            .reader
            .info()
            .frame_control
            .as_ref()
            .expect("get_fctl_info called without an fcTL chunk");
        let duration_ms = if fctl.delay_den == 0 {
            10 * u32::from(fctl.delay_num)
        } else {
            1000 * u32::from(fctl.delay_num) / u32::from(fctl.delay_den)
        };
        (
            fctl.width,
            fctl.height,
            fctl.x_offset,
            fctl.y_offset,
            fctl.dispose_op,
            fctl.blend_op,
            duration_ms,
        )
    }

    /// Port of `Reader::get_unknown_chunks_count` in the non-`for_android` build, which captures
    /// nothing.
    #[allow(clippy::unused_self)] // mirrors the C++ method, which reads the reader's chunks
    pub(crate) fn get_unknown_chunks_count(&self) -> usize {
        0
    }

    /// Port of `Reader::output_color_type`.
    pub(crate) fn output_color_type(&self) -> png::ColorType {
        self.reader.output_color_type().0
    }

    /// Port of `Reader::output_bits_per_component`.
    pub(crate) fn output_bits_per_component(&self) -> u8 {
        self.reader.output_color_type().1 as u8
    }

    /// Port of `Reader::next_frame_info`.
    pub(crate) fn next_frame_info(&mut self) -> DecodingResult {
        self.reader.next_frame_info().err().as_ref().into()
    }

    /// Port of `Reader::next_interlaced_row`: copies the next row into `row`, which is empty when
    /// there are no more rows.
    // Port of: rust/png/FFI.rs#L862-L871 (chrome/m156)
    pub(crate) fn next_interlaced_row(&mut self, row: &mut Vec<u8>) -> DecodingResult {
        row.clear();
        match self.reader.next_interlaced_row() {
            Ok(Some(decoded)) => {
                self.last_interlace_info = Some(*decoded.interlace());
                row.extend_from_slice(decoded.data());
                DecodingResult::Success
            }
            Ok(None) => {
                self.last_interlace_info = None;
                DecodingResult::Success
            }
            Err(e) => DecodingResult::from(Some(&e)),
        }
    }

    /// Port of `Reader::expand_last_interlaced_row`.
    // Port of: rust/png/FFI.rs#L909-L923 (chrome/m156)
    pub(crate) fn expand_last_interlaced_row(
        &self,
        img: &mut [u8],
        img_row_stride: usize,
        row: &[u8],
        bits_per_pixel: u8,
    ) {
        let Some(png::InterlaceInfo::Adam7(adam7info)) = self.last_interlace_info.as_ref() else {
            panic!("This function should only be called after decoding an interlaced row");
        };
        png::expand_interlaced_row(img, img_row_stride, row, adam7info, bits_per_pixel);
    }

    /// Port of `Reader::splat_last_interlaced_row`.
    // Port of: rust/png/FFI.rs#L925-L938 (chrome/m156)
    pub(crate) fn splat_last_interlaced_row(
        &self,
        img: &mut [u8],
        img_row_stride: usize,
        row: &[u8],
        bits_per_pixel: u8,
    ) {
        let Some(png::InterlaceInfo::Adam7(adam7info)) = self.last_interlace_info.as_ref() else {
            panic!("This function should only be called after decoding an interlaced row");
        };
        png::splat_interlaced_row(img, img_row_stride, row, adam7info, bits_per_pixel);
    }

    /// Port of `Reader::read_row`: decodes one row into `output_buffer`.
    // Port of: rust/png/FFI.rs#L940-L950 (chrome/m156)
    pub(crate) fn read_row(&mut self, output_buffer: &mut [u8]) -> DecodingResult {
        match self.reader.read_row(output_buffer) {
            Ok(Some(info)) => {
                self.last_interlace_info = Some(info);
                DecodingResult::Success
            }
            Ok(None) => DecodingResult::EndOfFrame,
            Err(e) => DecodingResult::from(Some(&e)),
        }
    }

    /// Port of `Reader::finish_decoding`. Nothing to finish in the non-`for_android` build.
    #[allow(clippy::unused_self)] // mirrors the C++ method, which finishes the reader
    pub(crate) fn finish_decoding(&mut self) -> DecodingResult {
        DecodingResult::Success
    }
}
