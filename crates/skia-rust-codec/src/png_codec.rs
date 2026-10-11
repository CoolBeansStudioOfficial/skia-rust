// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkPngCodec.cpp (chrome/m156), src/codec/SkPngCodec.h (chrome/m156),
// include/codec/SkPngDecoder.h (chrome/m156)
// Ported from: src/codec/SkPngCodec.{h,cpp}, include/codec/SkPngDecoder.h
//
// Gainmaps: the `gmAP` and `gdAT` chunks are read when the codec is made, and the gainmap codec is
// decoded on request (see `png_codec_base`). Not ported yet: the HDR metadata (`cLLI`, `mDCV`), the
// Android framework logging, and the sampled-codec entry point. Skia's
// `SkPngCodec` has no `cICP` handling in m156, so none is ported here either.
//
// The control flow of Skia's decoder is libpng's longjmp: a row callback that has all the rows it
// needs jumps out of `png_process_data` (`kStopDecoding`), and a libpng error jumps out as well
// (`kPngError`). The port returns the same two outcomes as `PngError::Stop` and `PngError::Error`
// from libpng, and `process_data` maps them back to Skia's `processData` results.
//
// Pixels reach the destination through `RowShared`, not by a callback that holds the codec: the
// libpng row handler is a `'static` box, so it queues copies of the decoded rows, and the codec
// transforms the queued rows into the destination after each `png_process_data` call. The rows
// come out in the same order with the same bytes, and the stop decision is made at the same row.

//! The PNG decoder (`SkPngCodec`) on top of the libpng port.

use std::sync::{Arc, Mutex};

use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::gainmap_info::GainmapInfo;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::stream::Stream;
use skia_rust_libpng::error::{PngError, PngResult};
use skia_rust_libpng::{
    PNG_HANDLE_CHUNK_ALWAYS, PNG_MAXIMUM_INFLATE_WINDOW, PngInfo, PngStruct, ProgressiveHandler,
    create_info_struct, get_chrm_fixed, get_gama_fixed, get_iccp, get_ihdr, get_plte, get_rowbytes,
    get_sbit, get_srgb, get_trns, get_valid,
};
use skia_rust_skcms::{IccProfile, TransferFunction};

use crate::codec::{Codec, CodecBase, CodecImpl, Options, Result};
use crate::codec_priv::{get_sampled_dimension, get_start_coord};
use crate::encoded_info::{Alpha, Color, EncodedInfo};
use crate::png_codec_base::{
    PaletteSource, PngCodecBase, is_compatible_color_profile_and_type, is_png, to_pixel_format,
};
use crate::png_composite_chunk_reader::{
    PngChunkReader, PngCompositeChunkReader, UserChunkAdapter,
};
use crate::sampler::Sampler;
use crate::swizzler::Swizzler;

/// Port of `kGraySigBit_GrayAlphaIsJustAlpha` (src/codec/SkPngPriv.h): the significant bits Skia
/// writes for an alpha-only image stored as gray-alpha.
const GRAY_SIG_BIT_GRAY_ALPHA_IS_JUST_ALPHA: u8 = 1;

/// The size of the buffer the stream is read through. Port of the `kBufferSize` constant of
/// `SkPngCodec::processData`.
const PROCESS_BUFFER_SIZE: usize = 4096;

// PNG colour types (png.h). RGBA (6) is the default arm of the colour switch, as in Skia.
const PNG_COLOR_TYPE_GRAY: u8 = 0;
const PNG_COLOR_TYPE_RGB: u8 = 2;
const PNG_COLOR_TYPE_PALETTE: u8 = 3;
const PNG_COLOR_TYPE_GRAY_ALPHA: u8 = 4;

/// Port of `log_and_return_error`: a failed decode that read all its input is incomplete.
// Port of: src/codec/SkPngCodec.cpp#L406-L413 (chrome/m156)
fn log_and_return_error(success: bool) -> Result {
    if success {
        Result::IncompleteInput
    } else {
        Result::ErrorInInput
    }
}

/// The libpng reader state that Skia keeps as `fPng_ptr` and `fInfo_ptr`.
struct PngReadState {
    png: PngStruct,
    info: PngInfo,
}

/// The rows the libpng callbacks produce, shared with the codec. The callbacks run inside
/// `png_process_data`, and the codec consumes the rows after it returns.
// Port of: the output the row callbacks write in src/codec/SkPngCodec.cpp (the `fDst` and
// `fInterlaceBuffer` members), with the destination kept by the codec
#[derive(Default)]
struct RowShared {
    /// Rows waiting to be transformed into the destination, in order (non-interlaced decodes).
    rows: Vec<Vec<u8>>,
    /// Port of `fInterlaceBuffer`: the combined image of an interlaced decode.
    interlace: Vec<u8>,
    /// Port of `fLinesDecoded`.
    lines_decoded: i32,
    /// Port of `fInterlacedComplete`.
    interlaced_complete: bool,
}

/// What a row callback does with each row. Each variant is one of the callbacks in
/// `SkPngNormalDecoder` and `SkPngInterlacedDecoder`.
enum RowMode {
    /// Port of `SkPngNormalDecoder::allRowsCallback`: every row, in order, no stop.
    AllRows,
    /// Port of `SkPngNormalDecoder::rowCallback`: the rows of a range the swizzler needs, and a
    /// stop once `rows_needed` rows are written.
    Range {
        first_row: i32,
        rows_needed: i32,
        /// Port of `swizzler->rowNeeded(row - firstRow)` for each row of the range, or `None` when
        /// there is no swizzler and every row is needed.
        needed: Option<Vec<bool>>,
        /// Port of `fRowsWrittenToOutput`, which carries over from earlier incremental calls.
        written: i32,
    },
    /// Port of `SkPngInterlacedDecoder::interlacedRowCallback`.
    Interlaced {
        first_row: i32,
        last_row: i32,
        number_passes: i32,
        height: i32,
        sample_y: i32,
        png_rowbytes: usize,
    },
}

/// The libpng row handler. Port of the callbacks that `SkPngNormalDecoder` and
/// `SkPngInterlacedDecoder` install with `png_set_progressive_read_fn`.
struct RowHandler {
    shared: Arc<Mutex<RowShared>>,
    mode: RowMode,
}

fn lock(shared: &Mutex<RowShared>) -> std::sync::MutexGuard<'_, RowShared> {
    shared
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl ProgressiveHandler for RowHandler {
    fn info(&mut self, _png: &mut PngStruct, _info: &mut PngInfo) -> PngResult<()> {
        // Skia's decoders install no info callback.
        Ok(())
    }

    fn row(
        &mut self,
        png: &mut PngStruct,
        row: Option<&[u8]>,
        row_num: u32,
        pass: i32,
    ) -> PngResult<()> {
        let shared = Arc::clone(&self.shared);
        let mut shared = lock(&shared);
        // A row number larger than i32 cannot occur: libpng limits the height to 1,000,000.
        let row_num = i32::try_from(row_num).unwrap_or(i32::MAX);
        match &mut self.mode {
            // Port of SkPngNormalDecoder::allRowsCallback.
            RowMode::AllRows => {
                shared
                    .rows
                    .push(row.map(<[u8]>::to_vec).unwrap_or_default());
                Ok(())
            }
            // Port of SkPngNormalDecoder::rowCallback.
            RowMode::Range {
                first_row,
                rows_needed,
                needed,
                written,
            } => {
                if row_num < *first_row {
                    // Ignore this row.
                    return Ok(());
                }
                // If there is no swizzler, all rows are needed.
                let index = usize::try_from(row_num - *first_row).unwrap_or(0);
                if needed.as_ref().is_none_or(|needed| needed[index]) {
                    shared
                        .rows
                        .push(row.map(<[u8]>::to_vec).unwrap_or_default());
                    *written += 1;
                }
                if *written == *rows_needed {
                    // Fake error to stop decoding scanlines.
                    return Err(PngError::Stop);
                }
                Ok(())
            }
            // Port of SkPngInterlacedDecoder::interlacedRowCallback.
            RowMode::Interlaced {
                first_row,
                last_row,
                number_passes,
                height,
                sample_y,
                png_rowbytes,
            } => {
                if row_num < *first_row || row_num > *last_row || shared.interlaced_complete {
                    // Ignore this row.
                    return Ok(());
                }
                let offset = usize::try_from(row_num - *first_row).unwrap_or(0) * *png_rowbytes;
                let old_row = &mut shared.interlace[offset..offset + *png_rowbytes];
                png.progressive_combine_row(old_row, row)?;

                if pass == 0 {
                    // The first pass initializes all rows.
                    shared.lines_decoded += 1;
                } else if *number_passes - 1 == pass && row_num == *last_row {
                    // Last pass, and every row the caller needs has been read.
                    shared.interlaced_complete = true;
                    // Stop only when the whole image is not decoded, since reading the rest can be
                    // expensive. A full decode reads through IEND, as Skia does for Android.
                    if *last_row != *height - 1 || *sample_y != 1 {
                        return Err(PngError::Stop);
                    }
                }
                Ok(())
            }
        }
    }

    fn end(&mut self, _png: &mut PngStruct, _info: &mut PngInfo) -> PngResult<()> {
        Ok(())
    }
}

/// The row range of the current decode. Port of `fFirstRow`, `fLastRow`, `fRowBytes`,
/// `fRowsWrittenToOutput` and `fRowsNeeded`.
#[derive(Debug, Clone, Copy, Default)]
struct DecodeRange {
    first_row: i32,
    last_row: i32,
    row_bytes: usize,
    rows_written: i32,
    rows_needed: i32,
}

/// The header of a PNG file, read by `read_header`. Port of the output fields of
/// `SkPngCodec::read_header` and `AutoCleanPng::infoCallback`.
struct HeaderRead {
    state: PngReadState,
    idat_length: u32,
    encoded: EncodedInfo,
    number_passes: u32,
}

/// A colour profile read from the PNG chunks. Port of the `SkCodecs::ColorProfile` that
/// `read_color_profile` returns: an ICC profile with its bytes, or one built from a transfer
/// function and primaries.
struct PngColorProfile {
    data: Option<Arc<[u8]>>,
    profile: IccProfile,
}

// Port of: src/codec/SkPngCodec.cpp#L271-L278 (png_fixed_point_to_float) and #L280-L283
// (png_inverted_fixed_point_to_float)
#[must_use]
fn png_fixed_point_to_float(x: i32) -> f32 {
    // libpng converts fixed point to double; Skia converts to float itself.
    #[allow(clippy::cast_precision_loss)] // mirrors (float) x in the C
    let x = x as f32;
    x * 0.00001_f32
}

#[must_use]
fn png_inverted_fixed_point_to_float(x: i32) -> f32 {
    // The gAMA chunk stores 1/gamma.
    1.0_f32 / png_fixed_point_to_float(x)
}

// Port of: src/codec/SkPngCodec.cpp#L285-L345 (read_color_profile). With no profile information
// the decoder uses sRGB, so the caller gets `None`.
fn read_color_profile(info: &PngInfo) -> Option<PngColorProfile> {
    // First check for an ICC profile. libpng has already inflated it.
    if let Some((_name, _compression, profile_bytes)) = get_iccp(info) {
        let data: Arc<[u8]> = Arc::from(profile_bytes);
        let profile = skia_rust_skcms::parse(&data)?;
        return Some(PngColorProfile {
            data: Some(data),
            profile,
        });
    }

    // Second, check for sRGB. The ICC chunk above wins when both are present.
    if get_srgb(info).is_some() {
        return None;
    }

    // Default to the sRGB gamut.
    let mut to_xyzd50 = skia_rust_core::color_space::named_gamut::SRGB;

    // Next, check for chromaticities.
    if let Some(chrm) = get_chrm_fixed(info) {
        let rx = png_fixed_point_to_float(chrm[2]);
        let ry = png_fixed_point_to_float(chrm[3]);
        let gx = png_fixed_point_to_float(chrm[4]);
        let gy = png_fixed_point_to_float(chrm[5]);
        let bx = png_fixed_point_to_float(chrm[6]);
        let by = png_fixed_point_to_float(chrm[7]);
        let wx = png_fixed_point_to_float(chrm[0]);
        let wy = png_fixed_point_to_float(chrm[1]);
        // If the primaries are degenerate, Skia falls back to sRGB.
        if let Some(matrix) = skia_rust_skcms::primaries_to_xyzd50(rx, ry, gx, gy, bx, by, wx, wy) {
            to_xyzd50 = matrix;
        }
    }

    let transfer = match get_gama_fixed(info) {
        Some(gamma) => TransferFunction {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
            e: 0.0,
            f: 0.0,
            g: png_inverted_fixed_point_to_float(gamma),
        },
        // Default to the sRGB gamma when the image has colour space information but no gamma.
        None => *skia_rust_skcms::srgb_transfer_function(),
    };

    let mut profile = IccProfile::new();
    profile.set_transfer_function(&transfer);
    profile.set_xyzd50(&to_xyzd50);
    Some(PngColorProfile {
        data: None,
        profile,
    })
}

/// Port of `AutoCleanPng::infoCallback`'s transform choices and encoded description. Sets the
/// libpng transforms the decode needs, and returns the encoded colour, alpha and bit depth, the
/// profile, and the number of interlace passes.
// Port of: src/codec/SkPngCodec.cpp#L752-L898 (chrome/m156)
fn info_callback(png: &mut PngStruct, info: &mut PngInfo) -> (EncodedInfo, u32) {
    let (orig_width, orig_height, mut bit_depth, encoded_color_type, _, _, _) = get_ihdr(info);
    let width = i32::try_from(orig_width).unwrap_or(i32::MAX);
    let height = i32::try_from(orig_height).unwrap_or(i32::MAX);

    // 16-bit gray is decoded at 8 bits.
    if bit_depth == 16
        && (encoded_color_type == PNG_COLOR_TYPE_GRAY
            || encoded_color_type == PNG_COLOR_TYPE_GRAY_ALPHA)
    {
        bit_depth = 8;
        png.set_strip_16();
    }

    // Choose the default colour type and alpha type, and set the transforms that are needed.
    // Often the swizzler does the transform; libpng does the rare, PNG-specific cases.
    let has_trns = get_valid(info, skia_rust_libpng::info::TRNS);
    let (color, alpha) = match encoded_color_type {
        PNG_COLOR_TYPE_PALETTE => {
            // Extract the 1, 2 and 4 bit indices into bytes.
            if bit_depth < 8 {
                bit_depth = 8;
                png.set_packing();
            }
            let alpha = if has_trns {
                Alpha::Unpremul
            } else {
                Alpha::Opaque
            };
            (Color::Palette, alpha)
        }
        PNG_COLOR_TYPE_RGB => {
            if has_trns {
                // Convert to RGBA when there is a transparency chunk.
                png.set_trns_to_alpha();
                (Color::RGBA, Alpha::Binary)
            } else {
                (Color::RGB, Alpha::Opaque)
            }
        }
        PNG_COLOR_TYPE_GRAY => {
            // Expand the 1, 2 and 4 bit gray images to 8 bits.
            if bit_depth < 8 {
                bit_depth = 8;
                png.set_expand_gray_1_2_4_to_8();
            }
            if has_trns {
                png.set_trns_to_alpha();
                (Color::GrayAlpha, Alpha::Binary)
            } else {
                (Color::Gray, Alpha::Opaque)
            }
        }
        PNG_COLOR_TYPE_GRAY_ALPHA => (Color::GrayAlpha, Alpha::Unpremul),
        // PNG_COLOR_TYPE_RGBA, and the defaults that Skia asserts against.
        _ => (Color::RGBA, Alpha::Unpremul),
    };

    let number_passes = png.set_interlace_handling();

    let profile = read_color_profile(info);
    let profile = profile.filter(|p| is_compatible_color_profile_and_type(Some(&p.profile), color));

    // The significant bits can recommend a narrower colour type.
    let mut color = color;
    match encoded_color_type {
        PNG_COLOR_TYPE_GRAY_ALPHA => {
            if get_sbit(info).is_some_and(|sig_bits| {
                sig_bits.alpha == 8 && sig_bits.gray == GRAY_SIG_BIT_GRAY_ALPHA_IS_JUST_ALPHA
            }) {
                color = Color::XAlpha;
            }
        }
        // Recommend a decode to 565 when the significant bits say so.
        PNG_COLOR_TYPE_RGB
            if get_sbit(info).is_some_and(|sig_bits| {
                sig_bits.red == 5 && sig_bits.green == 6 && sig_bits.blue == 5
            }) =>
        {
            color = Color::Color565;
        }
        _ => {}
    }

    let encoded = EncodedInfo::make_with_depth(width, height, color, alpha, bit_depth, bit_depth);
    let encoded = match profile {
        Some(PngColorProfile {
            data: Some(data),
            profile,
        }) => encoded.with_profile(data, profile),
        Some(PngColorProfile {
            data: None,
            profile,
        }) => encoded.with_generated_profile(profile),
        None => encoded,
    };
    (encoded, number_passes)
}

/// Port of `read_header`'s stream walk (`AutoCleanPng::decodeBounds`): feeds the signature and the
/// chunks before IDAT to libpng, and returns the IDAT chunk's length when it is reached. Returns
/// `None` if the stream ends first, or libpng rejects a chunk.
// Port of: src/codec/SkPngCodec.cpp#L95-L156 (chrome/m156) (AutoCleanPng::decodeBounds)
fn decode_bounds(stream: &mut dyn Stream, png: &mut PngStruct, info: &mut PngInfo) -> Option<u32> {
    let mut buffer = vec![0u8; PROCESS_BUFFER_SIZE];

    // Parse the signature.
    let mut signature = [0u8; 8];
    if stream.read(&mut signature) < 8 {
        return None;
    }
    png.process_data(info, &signature).ok()?;

    loop {
        // Parse the chunk length and type.
        let mut chunk = [0u8; 8];
        if stream.read(&mut chunk) < 8 {
            // The input ended without the bounds.
            return None;
        }
        let length = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        if &chunk[4..8] == b"IDAT" {
            return Some(length);
        }
        png.process_data(info, &chunk).ok()?;
        // Process the full chunk plus its CRC.
        if !feed_stream(stream, png, info, &mut buffer, length as usize + 4).ok()? {
            return None;
        }
    }
}

/// Port of the `process_data` helper of `SkPngCodec.cpp` (chrome/m156), used by `decode_bounds`:
/// reads `length` bytes and passes them to libpng in pieces. Returns `Ok(false)` if the stream
/// ended first.
// Port of: src/codec/SkPngCodec.cpp#L140-L152 (chrome/m156)
fn feed_stream(
    stream: &mut dyn Stream,
    png: &mut PngStruct,
    info: &mut PngInfo,
    buffer: &mut [u8],
    length: usize,
) -> PngResult<bool> {
    let mut remaining = length;
    while remaining > 0 {
        let to_process = remaining.min(buffer.len());
        let read = stream.read(&mut buffer[..to_process]);
        png.process_data(info, &buffer[..read])?;
        if read < to_process {
            return Ok(false);
        }
        remaining -= to_process;
    }
    Ok(true)
}

/// Port of `read_header` (chrome/m156): creates the libpng reader, installs the chunk reader,
/// reads the bounds and the encoded information. The stream is left after the IDAT header.
// Port of: src/codec/SkPngCodec.cpp#L658-L721 (chrome/m156)
fn read_header(
    stream: &mut dyn Stream,
    chunk_reader: &Arc<Mutex<PngCompositeChunkReader>>,
) -> std::result::Result<HeaderRead, Result> {
    let mut png = PngStruct::new_read();
    // This setting ensures that images with incorrect CMF bytes are displayed (crbug.com/807324).
    png.set_option(PNG_MAXIMUM_INFLATE_WINDOW, true);
    let mut info = create_info_struct();

    // Hook up the chunk reader, so the unknown chunks reach it. This is installed before the header
    // is read.
    png.set_keep_unknown_chunks(PNG_HANDLE_CHUNK_ALWAYS, &[]);
    png.set_read_user_chunk_fn(Some(Box::new(UserChunkAdapter(Arc::clone(chunk_reader)))));

    let Some(idat_length) = decode_bounds(stream, &mut png, &mut info) else {
        return Err(Result::IncompleteInput);
    };
    // Port of AutoCleanPng::infoCallback, which decodeBounds runs when it reaches IDAT.
    let (encoded, number_passes) = info_callback(&mut png, &mut info);
    Ok(HeaderRead {
        state: PngReadState { png, info },
        idat_length,
        encoded,
        number_passes,
    })
}

/// The PNG decoder. Port of `SkPngCodec` and its two decoders, `SkPngNormalDecoder` and
/// `SkPngInterlacedDecoder`, which differ only in how they decode rows.
#[doc(alias = "SkPngCodec")]
pub struct PngCodec {
    base_png: PngCodecBase,
    /// Port of `fPng_ptr` and `fInfo_ptr`. `None` after a failed rewind.
    read: Option<Box<PngReadState>>,
    /// Port of `fIdatLength`.
    idat_length: u32,
    /// Port of `fDecodedIdat`.
    decoded_idat: bool,
    /// Port of the decoder choice: 1 for a non-interlaced image, 7 for Adam7.
    number_passes: u32,
    /// Port of the row output, shared with the libpng handler.
    rows: Arc<Mutex<RowShared>>,
    /// Port of `fRowBytes` and the row counters.
    range: DecodeRange,
    /// Port of `fPng_rowbytes`: the row size of the interlace buffer.
    png_rowbytes: usize,
}

impl std::fmt::Debug for PngCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PngCodec")
            .field("idat_length", &self.idat_length)
            .field("number_passes", &self.number_passes)
            .finish_non_exhaustive()
    }
}

impl PngCodec {
    /// Whether the image is interlaced (Adam7). Port of `SkPngInterlacedDecoder` being chosen.
    fn is_interlaced(&self) -> bool {
        self.number_passes != 1
    }

    /// Port of `SkPngCodec::destroyReadStruct`.
    fn destroy_read_struct(&mut self) {
        self.read = None;
    }

    /// Hands bytes to libpng, then transforms any rows it produced into `dst`. Port of
    /// `png_process_data` followed by the callbacks that Skia's decoders run inside it.
    fn feed(&mut self, base: &CodecBase<'_>, dst: &mut [u8], bytes: &[u8]) -> PngResult<()> {
        let result = match self.read.as_deref_mut() {
            Some(state) => state.png.process_data(&mut state.info, bytes),
            None => Err(PngError::Error("the PNG reader was destroyed".to_owned())),
        };
        self.drain_rows(base, dst);
        result
    }

    /// Transforms the rows that libpng produced since the last call into `dst`, in order. Port of
    /// the `applyXformRow` calls in `allRowsCallback` and `rowCallback`.
    fn drain_rows(&mut self, base: &CodecBase<'_>, dst: &mut [u8]) {
        let queued = std::mem::take(&mut lock(&self.rows).rows);
        for row in queued {
            let offset =
                usize::try_from(self.range.rows_written).unwrap_or(0) * self.range.row_bytes;
            if offset >= dst.len() {
                break;
            }
            self.base_png
                .apply_xform_row(base, &mut dst[offset..], &row);
            self.range.rows_written += 1;
        }
    }

    /// Port of `SkPngCodec::processData`: feeds the IDAT chunk and the chunks after it to libpng,
    /// until the end of the file or a stop. Returns false on a libpng error.
    // Port of: src/codec/SkPngCodec.cpp#L168-L210 (chrome/m156)
    fn process_data(&mut self, base: &mut CodecBase<'_>, dst: &mut [u8]) -> bool {
        let mut buffer = vec![0u8; PROCESS_BUFFER_SIZE];
        let mut iend = false;
        loop {
            let length: usize;
            if self.decoded_idat {
                // Parse the chunk length and type.
                let mut chunk = [0u8; 8];
                if read_stream(base, &mut chunk) < 8 {
                    break;
                }
                if let Err(error) = self.feed(base, dst, &chunk) {
                    return error_is_stop(&error);
                }
                if &chunk[4..8] == b"IEND" {
                    iend = true;
                }
                length = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as usize;
            } else {
                // Synthesize the IDAT header whose length was read from the stream by decodeBounds.
                length = self.idat_length as usize;
                let mut idat = [0u8; 8];
                idat[..4].copy_from_slice(&self.idat_length.to_be_bytes());
                idat[4..].copy_from_slice(b"IDAT");
                if let Err(error) = self.feed(base, dst, &idat) {
                    return error_is_stop(&error);
                }
                self.decoded_idat = true;
            }

            // Process the full chunk plus its CRC.
            match self.feed_length(base, dst, &mut buffer, length + 4) {
                Ok(complete) => {
                    if !complete || iend {
                        break;
                    }
                }
                Err(error) => return error_is_stop(&error),
            }
        }
        true
    }

    /// Port of the `process_data` helper that `processData` uses (see [`feed_stream`]). Returns
    /// `Ok(false)` if the stream ended first.
    fn feed_length(
        &mut self,
        base: &mut CodecBase<'_>,
        dst: &mut [u8],
        buffer: &mut [u8],
        length: usize,
    ) -> PngResult<bool> {
        let mut remaining = length;
        while remaining > 0 {
            let to_process = remaining.min(buffer.len());
            let read = read_stream(base, &mut buffer[..to_process]);
            self.feed(base, dst, &buffer[..read])?;
            if read < to_process {
                return Ok(false);
            }
            remaining -= to_process;
        }
        Ok(true)
    }

    /// Port of `SkPngCodec::initializeXforms`: updates libpng, then sets up the transforms of the
    /// base codec for the destination.
    // Port of: src/codec/SkPngCodec.cpp#L579-L590 (chrome/m156)
    fn initialize_xforms(&mut self, base: &mut CodecBase<'_>, options: &Options) -> Result {
        let Some(state) = self.read.as_deref_mut() else {
            return Result::InvalidInput;
        };
        // Port of png_read_update_info, under setjmp.
        if state.png.read_update_info(&mut state.info).is_err() {
            return Result::InvalidInput;
        }
        // An APNG frame would be narrower than the image. This decoder has only the full image.
        let frame_width = base.dst_info().width();
        let palette = PaletteSource {
            plte: get_plte(&state.info),
            trns: get_trns(&state.info).map(|(alphas, _)| alphas),
        };
        self.base_png
            .initialize_xforms(base, options, frame_width, &palette)
    }

    /// Port of `SkPngNormalDecoder::decodeAllRows` and `SkPngInterlacedDecoder::decodeAllRows`.
    // Port of: src/codec/SkPngCodec.cpp#L430-L450 (normal) and #L494-L530 (interlaced)
    fn decode_all_rows(
        &mut self,
        base: &mut CodecBase<'_>,
        dst: &mut [u8],
        row_bytes: usize,
        rows_decoded: &mut i32,
    ) -> Result {
        let height = base.dimensions().height;
        if self.is_interlaced() {
            let result = self.set_up_interlace_buffer(height);
            if result != Result::Success {
                return result;
            }
            self.range = DecodeRange {
                first_row: 0,
                last_row: height - 1,
                row_bytes,
                rows_written: 0,
                rows_needed: 0,
            };
            lock(&self.rows).lines_decoded = 0;
            let sample_y = self.sample_y();
            self.install_handler(RowMode::Interlaced {
                first_row: 0,
                last_row: height - 1,
                number_passes: self.number_passes_i32(),
                height,
                sample_y,
                png_rowbytes: self.png_rowbytes,
            });

            let success = self.process_data(base, dst);
            let shared = Arc::clone(&self.rows);
            let shared = lock(&shared);
            // FIXME (Skia): when resuming, this may rewrite rows that did not change.
            let lines = shared.lines_decoded;
            for row_num in 0..lines {
                let start = usize::try_from(row_num).unwrap_or(0) * self.png_rowbytes;
                let src_row = &shared.interlace[start..start + self.png_rowbytes];
                let dst_start = usize::try_from(row_num).unwrap_or(0) * row_bytes;
                self.base_png
                    .apply_xform_row(base, &mut dst[dst_start..], src_row);
            }
            if success && shared.interlaced_complete {
                return Result::Success;
            }
            *rows_decoded = lines;
            return log_and_return_error(success);
        }

        self.range = DecodeRange {
            first_row: 0,
            last_row: height - 1,
            row_bytes,
            rows_written: 0,
            rows_needed: 0,
        };
        self.install_handler(RowMode::AllRows);
        let success = self.process_data(base, dst);
        if success && self.range.rows_written == height {
            return Result::Success;
        }
        *rows_decoded = self.range.rows_written;
        log_and_return_error(success)
    }

    /// Port of `SkPngNormalDecoder::setRange` and `SkPngInterlacedDecoder::setRange`: records the
    /// rows an incremental decode will produce.
    // Port of: src/codec/SkPngCodec.cpp#L357-L366 (normal) and #L467-L477 (interlaced)
    fn set_range(&mut self, first_row: i32, last_row: i32, row_bytes: usize) -> Result {
        if self.is_interlaced() {
            let result = self.set_up_interlace_buffer(last_row - first_row + 1);
            if result != Result::Success {
                return result;
            }
            lock(&self.rows).lines_decoded = 0;
        }
        self.range = DecodeRange {
            first_row,
            last_row,
            row_bytes,
            rows_written: 0,
            rows_needed: last_row - first_row + 1,
        };
        Result::Success
    }

    /// Port of `SkPngNormalDecoder::decode` and `SkPngInterlacedDecoder::decode`: decodes the
    /// rows of the current range that the swizzler needs.
    // Port of: src/codec/SkPngCodec.cpp#L368-L392 (normal) and #L532-L580 (interlaced)
    fn decode(
        &mut self,
        base: &mut CodecBase<'_>,
        dst: &mut [u8],
        rows_decoded: &mut i32,
    ) -> Result {
        let range_height = self.range.last_row - self.range.first_row + 1;
        if self.is_interlaced() {
            let sample_y = self.sample_y();
            self.install_handler(RowMode::Interlaced {
                first_row: self.range.first_row,
                last_row: self.range.last_row,
                number_passes: self.number_passes_i32(),
                height: base.dimensions().height,
                sample_y,
                png_rowbytes: self.png_rowbytes,
            });
            let success = self.process_data(base, dst);

            // Now apply the transforms to all the rows that were decoded.
            let shared = Arc::clone(&self.rows);
            let shared = lock(&shared);
            let lines = shared.lines_decoded;
            if lines == 0 {
                *rows_decoded = 0;
                return log_and_return_error(success);
            }
            let rows_needed = get_sampled_dimension(range_height, sample_y);

            // The first row of the interlace buffer is the first row of the range. Start at the
            // first sampled row.
            let mut src_row = get_start_coord(sample_y);
            let mut dst_row = 0usize;
            let mut rows_written = 0;
            while rows_written < rows_needed && src_row < lines {
                let start = usize::try_from(src_row).unwrap_or(0) * self.png_rowbytes;
                let src = &shared.interlace[start..start + self.png_rowbytes];
                self.base_png.apply_xform_row(
                    base,
                    &mut dst[dst_row * self.range.row_bytes..],
                    src,
                );
                dst_row += 1;
                rows_written += 1;
                src_row += sample_y;
            }

            if success && shared.interlaced_complete {
                return Result::Success;
            }
            *rows_decoded = rows_written;
            return log_and_return_error(success);
        }

        let sample_y = self.swizzler_sample_y();
        let rows_needed = match sample_y {
            Some(sample_y) => get_sampled_dimension(range_height, sample_y),
            None => range_height,
        };
        self.range.rows_needed = rows_needed;
        // Port of the swizzler's rowNeeded for every row of the range.
        let needed = self.base_png.swizzler.as_ref().map(|swizzler| {
            (0..range_height)
                .map(|row| swizzler.row_needed(row))
                .collect()
        });
        self.install_handler(RowMode::Range {
            first_row: self.range.first_row,
            rows_needed,
            needed,
            written: self.range.rows_written,
        });

        let success = self.process_data(base, dst);
        if success && self.range.rows_written == rows_needed {
            return Result::Success;
        }
        *rows_decoded = self.range.rows_written;
        log_and_return_error(success)
    }

    /// Port of `SkPngInterlacedDecoder::setUpInterlaceBuffer`: allocates the combined image for
    /// `height` rows at the libpng row size.
    // Port of: src/codec/SkPngCodec.cpp#L622-L645 (chrome/m156)
    fn set_up_interlace_buffer(&mut self, height: i32) -> Result {
        let Some(state) = self.read.as_deref() else {
            return Result::InternalError;
        };
        self.png_rowbytes = get_rowbytes(&state.info);
        let Ok(height) = usize::try_from(height) else {
            return Result::InternalError;
        };
        let Some(size) = self.png_rowbytes.checked_mul(height) else {
            return Result::InternalError;
        };
        let mut shared = lock(&self.rows);
        shared.interlace = vec![0u8; size];
        shared.interlaced_complete = false;
        Result::Success
    }

    /// Installs the row handler for the next `png_process_data` calls.
    fn install_handler(&mut self, mode: RowMode) {
        let handler = RowHandler {
            shared: Arc::clone(&self.rows),
            mode,
        };
        if let Some(state) = self.read.as_deref_mut() {
            state.png.set_progressive_read_fn(Some(Box::new(handler)));
        }
    }

    /// The swizzler's sample factor, or `None` when there is no swizzler.
    fn swizzler_sample_y(&self) -> Option<i32> {
        self.base_png.swizzler.as_ref().map(Swizzler::sample_y)
    }

    /// The swizzler's sample factor, or 1 when there is no swizzler (Skia's `sampleY` default).
    fn sample_y(&self) -> i32 {
        self.swizzler_sample_y().unwrap_or(1)
    }

    fn number_passes_i32(&self) -> i32 {
        i32::try_from(self.number_passes).unwrap_or(1)
    }

    /// Port of `SkPngCodec::onGetPixels`'s decode body, for the whole image.
    // Port of: src/codec/SkPngCodec.cpp#L592-L608 (chrome/m156)
    fn get_pixels_inner(
        &mut self,
        base: &mut CodecBase<'_>,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        rows_decoded: &mut i32,
    ) -> Result {
        let result = self.initialize_xforms(base, options);
        if result != Result::Success {
            return result;
        }
        if options.subset.is_some() {
            return Result::Unimplemented;
        }
        self.base_png.initialize_xform_params(base);
        self.decode_all_rows(base, dst, row_bytes, rows_decoded)
    }
}

/// Port of the `kPngError` and `kStopDecoding` cases of `processData`: a stop is a success, and a
/// libpng error is a failure.
// Port of: src/codec/SkPngCodec.cpp#L168-L178 (chrome/m156)
#[must_use]
fn error_is_stop(error: &PngError) -> bool {
    matches!(error, PngError::Stop)
}

/// Reads from the codec's stream, or returns 0 when there is none. Port of `this->stream()->read`.
fn read_stream(base: &mut CodecBase<'_>, buf: &mut [u8]) -> usize {
    base.stream().map_or(0, |stream| stream.read(buf))
}

/// Port of `SkPngCodec::MakeFromStream`'s codec construction: the header's encoded information and
/// the libpng state become a codec over the stream.
impl CodecImpl for PngCodec {
    // Port of: src/codec/SkPngCodec.cpp#L1061-L1064 (chrome/m156), the gainmap hooks, which
    // SkPngCodecBase implements (`onGetGainmapInfo`, `onGetGainmapCodec`).
    fn on_get_gainmap_info(&self, info: Option<&mut GainmapInfo>) -> bool {
        self.base_png.get_gainmap_info(info)
    }

    fn on_get_gainmap_codec(
        &mut self,
        info: Option<&mut GainmapInfo>,
        want_codec: bool,
    ) -> (bool, Option<Codec<'static>>) {
        self.base_png.get_gainmap_codec(info, want_codec)
    }

    // Port of: src/codec/SkPngCodecBase.cpp#L278-L290 (SkPngCodecBase::getSampler). The swizzler is
    // made on demand for a sampled decode, with the destination's width as the frame width.
    fn on_get_sampler(
        &mut self,
        base: &CodecBase<'_>,
        create_if_necessary: bool,
    ) -> Option<&mut dyn Sampler> {
        if self.base_png.swizzler.is_none() && create_if_necessary {
            let options = base.options().clone();
            let frame_width = base.dst_info().width();
            // Ignoring the result matches Skia: on failure the swizzler stays None.
            let _ = self
                .base_png
                .initialize_swizzler(base, &options, true, frame_width);
        }
        self.base_png
            .swizzler
            .as_mut()
            .map(|swizzler| swizzler as &mut dyn Sampler)
    }

    // Port of: src/codec/SkPngCodecBase.cpp#L157-L160 (onGetEncodedFormat)
    fn on_get_encoded_format(&self) -> EncodedImageFormat {
        EncodedImageFormat::PNG
    }

    // Port of: src/codec/SkPngCodec.cpp#L592-L608 (onGetPixels)
    fn on_get_pixels(
        &mut self,
        base: &mut CodecBase<'_>,
        _info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        rows_decoded: &mut i32,
    ) -> Result {
        self.get_pixels_inner(base, dst, row_bytes, options, rows_decoded)
    }

    // Port of: src/codec/SkPngCodec.cpp#L650-L665 (onRewind)
    fn on_rewind(&mut self, base: &mut CodecBase<'_>) -> bool {
        if !base.rewind_stream() {
            return false;
        }
        // Any reader state is replaced. If the header read fails, the reader stays destroyed, and
        // later calls rewind and retry.
        self.destroy_read_struct();

        let chunk_reader = Arc::clone(&self.base_png.chunk_reader);
        let Some(stream) = base.stream() else {
            return false;
        };
        match read_header(stream, &chunk_reader) {
            Ok(header) => {
                // The IDAT length and the encoded information stay as they were at creation.
                self.read = Some(Box::new(header.state));
                self.decoded_idat = false;
                true
            }
            Err(_) => false,
        }
    }

    // Port of: src/codec/SkPngCodec.cpp#L81-L84 (onSupportsIncrementalDecode, in the header)
    fn on_supports_incremental_decode(&self, _dst: &ImageInfo) -> bool {
        true
    }

    // Port of: src/codec/SkPngCodec.cpp#L667-L684 (onStartIncrementalDecode)
    fn on_start_incremental_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        dst_info: &ImageInfo,
        _dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
    ) -> Result {
        let result = self.initialize_xforms(base, options);
        if result != Result::Success {
            return result;
        }
        let (first_row, last_row) = match options.subset {
            Some(subset) => (subset.top(), subset.bottom() - 1),
            None => (0, dst_info.height() - 1),
        };
        self.set_range(first_row, last_row, row_bytes)
    }

    // Port of: src/codec/SkPngCodec.cpp#L686-L692 (onIncrementalDecode)
    fn on_incremental_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        dst: &mut [u8],
        rows_decoded: &mut i32,
    ) -> Result {
        // Only the first call strictly needs this, but the transform parameters can change between
        // calls, and the cost is small.
        self.base_png.initialize_xform_params(base);
        self.decode(base, dst, rows_decoded)
    }
}

/// Port of `SkPngCodec::MakeFromStream`: reads the header of a PNG stream and makes a codec for it.
/// `chunk_reader`, when given, receives the unknown chunks of the file.
///
/// # Errors
/// `IncompleteInput` if the stream ends or a chunk is rejected before the image data.
// Port of: src/codec/SkPngCodec.cpp#L723-L738 (chrome/m156)
#[doc(alias = "SkPngCodec::MakeFromStream")]
pub fn make_from_stream_with_chunk_reader<'a>(
    mut stream: Box<dyn Stream + Send + 'a>,
    chunk_reader: Option<Box<dyn PngChunkReader>>,
) -> std::result::Result<Codec<'a>, Result> {
    let chunk_reader = Arc::new(Mutex::new(PngCompositeChunkReader::new(chunk_reader)));
    let header = read_header(&mut *stream, &chunk_reader)?;
    let src_format = to_pixel_format(&header.encoded);
    // The gainmap chunks are taken when the codec is made, as Skia's readHeader does
    // (`takeGainmapStream`, `getGainmapInfo`).
    let (gainmap_stream, gainmap_info) = {
        let mut reader = chunk_reader
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (reader.take_gainmap_stream(), reader.gainmap_info())
    };
    let mut base_png = PngCodecBase::new(chunk_reader);
    base_png.gainmap_stream = gainmap_stream;
    base_png.gainmap_info = gainmap_info;
    let imp = PngCodec {
        base_png,
        read: Some(Box::new(header.state)),
        idat_length: header.idat_length,
        decoded_idat: false,
        number_passes: header.number_passes,
        rows: Arc::new(Mutex::new(RowShared::default())),
        range: DecodeRange::default(),
        png_rowbytes: 0,
    };
    Ok(Codec::new(
        header.encoded,
        Box::new(imp),
        Some(stream),
        EncodedOrigin::TopLeft,
        Some(src_format),
    ))
}

/// Port of `SkPngDecoder::Decode` for a stream with no chunk reader: the registry's entry point.
///
/// # Errors
/// As [`make_from_stream_with_chunk_reader`].
// Port of: src/codec/SkPngCodec.cpp#L755-L760 (chrome/m156), the registry entry
#[doc(alias = "SkPngDecoder::Decode")]
pub fn make_from_stream<'a>(
    stream: Box<dyn Stream + Send + 'a>,
) -> std::result::Result<Codec<'a>, Result> {
    make_from_stream_with_chunk_reader(stream, None)
}

/// Port of `SkPngDecoder::IsPng`.
#[doc(alias = "SkPngDecoder::IsPng")]
#[must_use]
pub fn is_png_format(buf: &[u8]) -> bool {
    is_png(buf)
}
