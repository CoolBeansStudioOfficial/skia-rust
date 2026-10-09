// Copyright 2010 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the COPYING file. Port by The skia-rust Authors.

//! Port of the still-image path of libwebp's `src/dec/webp_dec.c`: `ParseHeadersInternal` and its
//! helpers (`ParseRIFF`, `ParseVP8X`, `ParseOptionalChunks`, `ParseVP8Header`), `GetFeatures`,
//! `WebPParseHeaders`, and the parts of `WebPDecode` and `DecodeInto` that read the headers.
//!
//! Handled: the RIFF container with an optional `VP8X` header, `ALPH` for lossy frames, and the
//! `VP8 ` (lossy) and `VP8L` (lossless) image chunks, as well as bare VP8 and VP8L bitstreams.
//! Animation (`ANIM`/`ANMF`) is decoded frame by frame through the demuxer ([`crate::demux`]); a
//! file whose `VP8X` header sets the animation flag gets [`Status::UnsupportedFeature`] from this
//! still-image decode path, as in libwebp.

// Module-level clippy allows. Each one mirrors the C source of this module.
// clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::cast_sign_loss: the header
// fields are little-endian integers of 8, 24 and 32 bits, read with the widths of the C `GetLE*`
// helpers, and the sizes are converted between `uint32_t` and `size_t` as in the C code.
// clippy::struct_excessive_bools: `ParseState` holds the flag locals of `ParseHeadersInternal`.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::struct_excessive_bools
)]

use crate::alpha;
use crate::io::{Io, Status};
use crate::lossless::CspMode;
use crate::output;
use crate::rescaler;
use crate::vp8_dec::{self, Crop};
use crate::vp8_tables_small::K_FILTER_EXTRA_ROWS;
use crate::vp8l;

/// Port of `RIFF_HEADER_SIZE`.
const RIFF_HEADER_SIZE: usize = 12;
/// Port of `CHUNK_HEADER_SIZE`.
const CHUNK_HEADER_SIZE: usize = 8;
/// Port of `TAG_SIZE`.
const TAG_SIZE: usize = 4;
/// Port of `VP8X_CHUNK_SIZE`.
const VP8X_CHUNK_SIZE: usize = 10;
/// Port of `MAX_CHUNK_PAYLOAD` (`~0U - CHUNK_HEADER_SIZE - 1`, an unsigned 32-bit value).
const MAX_CHUNK_PAYLOAD: u32 = !0u32 - CHUNK_HEADER_SIZE as u32 - 1;
/// Port of `MAX_IMAGE_AREA`.
const MAX_IMAGE_AREA: u64 = 1u64 << 32;
/// Port of `ANIMATION_FLAG` (`webp/mux_types.h`).
const ANIMATION_FLAG: u32 = 0x0000_0002;
/// Port of `ALPHA_FLAG` (`webp/mux_types.h`).
const ALPHA_FLAG: u32 = 0x0000_0010;
/// Port of `VP8_FRAME_HEADER_SIZE`.
const VP8_FRAME_HEADER_SIZE: usize = vp8_dec::VP8_FRAME_HEADER_SIZE;
/// Port of `VP8L_FRAME_HEADER_SIZE`.
const VP8L_FRAME_HEADER_SIZE: usize = vp8l::VP8L_FRAME_HEADER_SIZE;

/// Port of `WebPBitstreamFeatures`: the size and flags of one image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "WebPBitstreamFeatures")]
pub struct Features {
    pub width: i32,
    pub height: i32,
    pub has_alpha: bool,
    pub has_animation: bool,
    pub is_lossless: bool,
}

/// The fields of `WebPHeaderStructure` that the still-image decoder reads.
#[derive(Debug)]
pub(crate) struct Headers<'a> {
    /// Offset of the bitstream (after its chunk header) from the start of the input.
    pub(crate) offset: usize,
    /// The `ALPH` payload, if any.
    pub(crate) alpha_data: Option<&'a [u8]>,
    /// The bitstream chunk size (`compressed_size`).
    pub(crate) compressed_size: usize,
    pub(crate) is_lossless: bool,
}

/// The working state of `ParseHeadersInternal`, gathered as the C function's locals.
#[derive(Default)]
struct ParseState<'a> {
    found_riff: bool,
    riff_size: usize,
    found_vp8x: bool,
    has_alpha: bool,
    has_animation: bool,
    image_width: i32,
    image_height: i32,
    pub(crate) alpha_data: Option<&'a [u8]>,
    pub(crate) compressed_size: usize,
    pub(crate) is_lossless: bool,
    pub(crate) offset: usize,
}

/// Port of `GetLE16`-style little-endian reads of the header fields.
fn get_le24(d: &[u8]) -> u32 {
    u32::from(d[0]) | (u32::from(d[1]) << 8) | (u32::from(d[2]) << 16)
}

/// Port of `GetLE32`.
fn get_le32(d: &[u8]) -> u32 {
    u32::from_le_bytes([d[0], d[1], d[2], d[3]])
}

/// Port of `ParseRIFF`. Advances `data` past the RIFF header and returns the RIFF size (zero when
/// there is no RIFF container).
fn parse_riff(data: &mut &[u8], have_all_data: bool) -> Result<usize, Status> {
    if data.len() >= RIFF_HEADER_SIZE && &data[..TAG_SIZE] == b"RIFF" {
        if &data[8..8 + TAG_SIZE] != b"WEBP" {
            return Err(Status::BitstreamError); // Wrong image file signature.
        }
        let size = get_le32(&data[TAG_SIZE..]);
        // Check that we have at least one chunk (i.e "WEBP" + "VP8?nnnn").
        if size < (TAG_SIZE + CHUNK_HEADER_SIZE) as u32 {
            return Err(Status::BitstreamError);
        }
        if size > MAX_CHUNK_PAYLOAD {
            return Err(Status::BitstreamError);
        }
        if have_all_data && (size as usize > data.len() - CHUNK_HEADER_SIZE) {
            return Err(Status::NotEnoughData); // Truncated bitstream.
        }
        // We have a RIFF container. Skip it.
        *data = &data[RIFF_HEADER_SIZE..];
        return Ok(size as usize);
    }
    Ok(0)
}

/// Port of `ParseVP8X`. Returns `(flags, canvas_width, canvas_height)` when a `VP8X` chunk is
/// present, and advances `data` past it.
fn parse_vp8x(data: &mut &[u8]) -> Result<Option<(u32, i32, i32)>, Status> {
    let vp8x_size = CHUNK_HEADER_SIZE + VP8X_CHUNK_SIZE;
    if data.len() < CHUNK_HEADER_SIZE {
        return Err(Status::NotEnoughData); // Insufficient data.
    }
    if &data[..TAG_SIZE] != b"VP8X" {
        return Ok(None);
    }
    let chunk_size = get_le32(&data[TAG_SIZE..]);
    if chunk_size != VP8X_CHUNK_SIZE as u32 {
        return Err(Status::BitstreamError); // Wrong chunk size.
    }
    // Verify if enough data is available to validate the VP8X chunk.
    if data.len() < vp8x_size {
        return Err(Status::NotEnoughData);
    }
    let flags = get_le32(&data[CHUNK_HEADER_SIZE..]);
    let width = 1 + get_le24(&data[12..]) as i32;
    let height = 1 + get_le24(&data[15..]) as i32;
    if u64::from(width as u32) * u64::from(height as u32) >= MAX_IMAGE_AREA {
        return Err(Status::BitstreamError); // image is too large
    }
    // Skip over VP8X header bytes.
    *data = &data[vp8x_size..];
    Ok(Some((flags, width, height)))
}

/// Port of `ParseOptionalChunks`. Skips the chunks before the image bitstream and returns the
/// `ALPH` payload if one was seen. On success `data` starts at the `VP8 `/`VP8L` chunk.
fn parse_optional_chunks<'a>(
    data: &mut &'a [u8],
    riff_size: usize,
) -> Result<Option<&'a [u8]>, Status> {
    // "WEBP" + "VP8Xnnnn" + data.
    let mut total_size: u32 = (TAG_SIZE + CHUNK_HEADER_SIZE + VP8X_CHUNK_SIZE) as u32;
    let mut buf: &'a [u8] = data;
    let mut alpha_data = None;
    loop {
        *data = buf;
        if buf.len() < CHUNK_HEADER_SIZE {
            return Err(Status::NotEnoughData); // Insufficient data.
        }
        let chunk_size = get_le32(&buf[TAG_SIZE..]);
        if chunk_size > MAX_CHUNK_PAYLOAD {
            return Err(Status::BitstreamError); // Not a valid chunk size.
        }
        // For odd-sized chunk-payload, there's one byte padding at the end.
        let disk_chunk_size = (CHUNK_HEADER_SIZE as u32)
            .wrapping_add(chunk_size)
            .wrapping_add(1)
            & !1u32;
        total_size = total_size.wrapping_add(disk_chunk_size);
        // Check that total bytes skipped so far does not exceed riff_size.
        if riff_size > 0 && (total_size as usize > riff_size) {
            return Err(Status::BitstreamError); // Not a valid chunk size.
        }
        // Start of a (possibly incomplete) VP8/VP8L chunk implies that we have parsed all the
        // optional chunks. This check must occur before the insufficient-data check below, to
        // allow incomplete VP8/VP8L chunks.
        if &buf[..TAG_SIZE] == b"VP8 " || &buf[..TAG_SIZE] == b"VP8L" {
            return Ok(alpha_data);
        }
        if (buf.len() as u64) < u64::from(disk_chunk_size) {
            return Err(Status::NotEnoughData); // Insufficient data.
        }
        if &buf[..TAG_SIZE] == b"ALPH" {
            // A valid ALPH header.
            alpha_data = Some(&buf[CHUNK_HEADER_SIZE..CHUNK_HEADER_SIZE + chunk_size as usize]);
        }
        // We have a full and valid chunk; skip it.
        buf = &buf[disk_chunk_size as usize..];
    }
}

/// Port of `ParseVP8Header`. Returns `(chunk_size, is_lossless)` and advances `data` past the
/// `VP8 `/`VP8L` chunk header.
fn parse_vp8_header(
    data: &mut &[u8],
    have_all_data: bool,
    riff_size: usize,
) -> Result<(usize, bool), Status> {
    let is_vp8 = data.len() >= TAG_SIZE && &data[..TAG_SIZE] == b"VP8 ";
    let is_vp8l = data.len() >= TAG_SIZE && &data[..TAG_SIZE] == b"VP8L";
    let minimal_size = TAG_SIZE + CHUNK_HEADER_SIZE; // "WEBP" + "VP8 nnnn" OR "WEBP" + "VP8Lnnnn"
    if data.len() < CHUNK_HEADER_SIZE {
        return Err(Status::NotEnoughData); // Insufficient data.
    }
    if is_vp8 || is_vp8l {
        // Bitstream contains VP8/VP8L header.
        let size = get_le32(&data[TAG_SIZE..]) as usize;
        if riff_size >= minimal_size && size > riff_size - minimal_size {
            return Err(Status::BitstreamError); // Inconsistent size information.
        }
        if have_all_data && (size > data.len() - CHUNK_HEADER_SIZE) {
            return Err(Status::NotEnoughData); // Truncated bitstream.
        }
        // Skip over CHUNK_HEADER_SIZE bytes from VP8/VP8L Header.
        *data = &data[CHUNK_HEADER_SIZE..];
        Ok((size, is_vp8l))
    } else {
        // Raw VP8/VP8L bitstream (no header).
        Ok((data.len(), vp8l::check_signature(data)))
    }
}

/// Port of `ParseHeadersInternal`. `want_headers` is `headers != NULL` in the C code: it selects
/// the features-only path, which returns the canvas size for a `VP8X` file whose image data is
/// not complete yet.
fn parse_headers_steps<'a>(
    data: &'a [u8],
    have_all_data: bool,
    want_headers: bool,
    st: &mut ParseState<'a>,
) -> Result<(), Status> {
    if data.len() < RIFF_HEADER_SIZE {
        return Err(Status::NotEnoughData);
    }
    let mut rest = data;
    // Skip over RIFF header.
    st.riff_size = parse_riff(&mut rest, have_all_data)?;
    st.found_riff = st.riff_size > 0;

    // Skip over VP8X.
    let vp8x = parse_vp8x(&mut rest)?;
    let (flags, canvas_width, canvas_height) = vp8x.unwrap_or((0, 0, 0));
    st.found_vp8x = vp8x.is_some();
    if !st.found_riff && st.found_vp8x {
        // Note: This restriction may be removed in the future, if it becomes necessary to send
        // VP8X chunk to the decoder.
        return Err(Status::BitstreamError);
    }
    let animation_present = flags & ANIMATION_FLAG != 0;
    st.has_alpha = flags & ALPHA_FLAG != 0;
    st.has_animation = animation_present;
    st.image_width = canvas_width;
    st.image_height = canvas_height;
    if st.found_vp8x && animation_present && !want_headers {
        // Just return features from VP8X header.
        return Ok(());
    }

    if rest.len() < TAG_SIZE {
        return Err(Status::NotEnoughData);
    }

    // Skip over optional chunks if data started with "RIFF + VP8X" or "ALPH".
    if (st.found_riff && st.found_vp8x)
        || (!st.found_riff && !st.found_vp8x && &rest[..TAG_SIZE] == b"ALPH")
    {
        st.alpha_data = parse_optional_chunks(&mut rest, st.riff_size)?;
    }

    // Skip over VP8/VP8L header.
    let (compressed_size, is_lossless) = parse_vp8_header(&mut rest, have_all_data, st.riff_size)?;
    st.compressed_size = compressed_size;
    st.is_lossless = is_lossless;
    if compressed_size > MAX_CHUNK_PAYLOAD as usize {
        return Err(Status::BitstreamError);
    }

    if is_lossless {
        if rest.len() < VP8L_FRAME_HEADER_SIZE {
            return Err(Status::NotEnoughData);
        }
        // Validates raw VP8L data.
        let (w, h, alpha) = vp8l::get_info(rest).ok_or(Status::BitstreamError)?;
        st.image_width = w;
        st.image_height = h;
        st.has_alpha = alpha;
    } else {
        if rest.len() < VP8_FRAME_HEADER_SIZE {
            return Err(Status::NotEnoughData);
        }
        // Validates raw VP8 data.
        let (w, h) = vp8_dec::get_info(rest, compressed_size).ok_or(Status::BitstreamError)?;
        st.image_width = w;
        st.image_height = h;
    }
    // Validates image size coherency.
    if st.found_vp8x && (canvas_width != st.image_width || canvas_height != st.image_height) {
        return Err(Status::BitstreamError);
    }
    st.offset = data.len() - rest.len();
    Ok(())
}

/// Port of `ParseHeadersInternal` including its `ReturnWidthHeight` exit: a `VP8X` file whose
/// data is incomplete still reports its canvas size to the features call.
fn parse_headers_internal<'a>(
    data: &'a [u8],
    have_all_data: bool,
    want_headers: bool,
    st: &mut ParseState<'a>,
) -> Result<(), Status> {
    let status = parse_headers_steps(data, have_all_data, want_headers, st);
    let status = match status {
        Err(Status::NotEnoughData) if st.found_vp8x && !want_headers => Ok(()),
        other => other,
    };
    if status.is_ok() {
        // If the data did not contain a VP8X/VP8L chunk the only definitive way to set this is
        // by looking for alpha data (from an ALPH chunk).
        st.has_alpha |= st.alpha_data.is_some();
    }
    status
}

/// Port of `GetFeatures`.
fn features_of(data: &[u8]) -> Result<Features, Status> {
    let mut st = ParseState::default();
    parse_headers_internal(data, false, false, &mut st)?;
    Ok(Features {
        width: st.image_width,
        height: st.image_height,
        has_alpha: st.has_alpha,
        has_animation: st.has_animation,
        is_lossless: st.is_lossless,
    })
}

/// Port of `WebPParseHeaders`: fills in the bitstream headers with `have_all_data` set, and
/// reports animation as unsupported.
fn parse_headers(data: &[u8]) -> Result<Headers<'_>, Status> {
    parse_headers_with(data, true)
}

/// `WebPParseHeaders` with `have_all_data` cleared, as `DecodeWebPHeaders` (`idec_dec.c`) calls
/// it on the data available so far. `NotEnoughData` means the VP8/VP8L chunk header is missing.
pub(crate) fn parse_headers_partial(data: &[u8]) -> Result<Headers<'_>, Status> {
    parse_headers_with(data, false)
}

fn parse_headers_with(data: &[u8], have_all_data: bool) -> Result<Headers<'_>, Status> {
    let mut st = ParseState::default();
    let status = parse_headers_internal(data, have_all_data, true, &mut st);
    match status {
        Ok(()) | Err(Status::NotEnoughData) if st.has_animation => Err(Status::UnsupportedFeature),
        Ok(()) => Ok(Headers {
            offset: st.offset,
            alpha_data: st.alpha_data,
            compressed_size: st.compressed_size,
            is_lossless: st.is_lossless,
        }),
        Err(s) => Err(s),
    }
}

/// Port of `WebPGetFeatures` for one image: the size, the flags, and whether it is lossless.
///
/// # Errors
///
/// Returns `NotEnoughData` for a truncated file that the features call cannot size, and
/// `BitstreamError` when the container or the image header does not parse.
#[doc(alias = "WebPGetFeatures")]
pub fn get_features(data: &[u8]) -> Result<Features, Status> {
    features_of(data)
}

/// The `WebPDecoderOptions` fields Skia sets, with `WebPInitDecoderConfig`'s defaults (no crop,
/// no scale, fancy upsampling on, filtering on).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[doc(alias = "WebPDecoderOptions")]
pub struct DecodeOptions {
    /// `use_cropping` with `(crop_left, crop_top, crop_width, crop_height)`.
    pub crop: Option<(i32, i32, i32, i32)>,
    /// `use_scaling` with `(scaled_width, scaled_height)`; a zero side is derived from the other.
    pub scale: Option<(i32, i32)>,
    /// `bypass_filtering`: skip the in-loop filter.
    pub bypass_filtering: bool,
    /// `no_fancy_upsampling`.
    pub no_fancy_upsampling: bool,
}

/// Port of `WebPCheckCropDimensions`.
#[must_use]
fn check_crop_dimensions(
    image_width: i32,
    image_height: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> bool {
    !(x < 0
        || y < 0
        || w <= 0
        || h <= 0
        || x >= image_width
        || w > image_width
        || w > image_width - x
        || y >= image_height
        || h > image_height
        || h > image_height - y)
}

/// The window and output parameters that `WebPIoInitFromOptions` computes from the options.
#[derive(Debug, Clone, Copy)]
pub(crate) struct IoParams {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) use_scaling: bool,
    pub(crate) scaled_width: i32,
    pub(crate) scaled_height: i32,
    pub(crate) bypass_filtering: bool,
    pub(crate) fancy_upsampling: bool,
}

impl IoParams {
    /// The output size: the crop window, or its scaled size (`WebPAllocateDecBuffer`).
    pub(crate) fn output_size(&self) -> (i32, i32) {
        if self.use_scaling {
            (self.scaled_width, self.scaled_height)
        } else {
            (self.w, self.h)
        }
    }
}

/// The checks of `CheckDecBuffer`/`AllocateBuffer` that apply to an external RGB buffer of
/// `out_w` x `out_h` pixels with `out_stride` bytes per row.
pub(crate) fn check_output(
    mode: CspMode,
    out: &[u8],
    out_stride: usize,
    out_w: i32,
    out_h: i32,
) -> Result<(), Status> {
    let bpp = output::bytes_per_pixel(mode).ok_or(Status::InvalidParam)?;
    if out_w <= 0
        || out_h <= 0
        || out_stride < out_w as usize * bpp
        || out.len() < out_stride * (out_h as usize - 1) + out_w as usize * bpp
    {
        return Err(Status::InvalidParam);
    }
    Ok(())
}

/// Port of `WebPIoInitFromOptions` (window, scaling, filter and upsampler parts) for a
/// `width` x `height` image. `snap_crop` is true for YUV sources (lossy), whose crop origin is
/// rounded down to even coordinates; RGB sources (lossless) keep it.
pub(crate) fn io_params(
    width: i32,
    height: i32,
    options: &DecodeOptions,
    snap_crop: bool,
) -> Result<IoParams, Status> {
    let (mut x, mut y, mut w, mut h) = (0, 0, width, height);
    if let Some((cl, ct, cw, ch)) = options.crop {
        w = cw;
        h = ch;
        x = cl;
        y = ct;
        if snap_crop {
            x &= !1;
            y &= !1;
        }
        if !check_crop_dimensions(width, height, x, y, w, h) {
            return Err(Status::InvalidParam); // out of frame boundary error
        }
    }
    let (mut scaled_width, mut scaled_height) = (w, h);
    let use_scaling = options.scale.is_some();
    if let Some((sw, sh)) = options.scale {
        scaled_width = sw;
        scaled_height = sh;
        if !rescaler::get_scaled_dimensions(w, h, &mut scaled_width, &mut scaled_height) {
            return Err(Status::InvalidParam);
        }
    }
    // Disable the filter for large downscaling ratios; the fancy upsampler is off when scaling.
    let bypass_filtering = options.bypass_filtering
        || (use_scaling && scaled_width < width * 3 / 4 && scaled_height < height * 3 / 4);
    Ok(IoParams {
        x,
        y,
        w,
        h,
        use_scaling,
        scaled_width,
        scaled_height,
        bypass_filtering,
        fancy_upsampling: !options.no_fancy_upsampling && !use_scaling,
    })
}

/// Decodes a still WebP image into `out` in colour space `mode` (`WebPDecode` with default
/// options: full frame, no scaling, fancy upsampling). Returns the image size.
///
/// # Errors
///
/// Returns the `Status` of the first parse or decode failure. As in `WebPDecode`, a truncated
/// file reports `BitstreamError` rather than `NotEnoughData` when the features step fails.
#[doc(alias = "WebPDecode")]
pub fn decode(
    data: &[u8],
    mode: CspMode,
    out: &mut [u8],
    out_stride: usize,
) -> Result<(i32, i32), Status> {
    decode_with_options(data, mode, out, out_stride, &DecodeOptions::default())
}

/// Decodes a still WebP image with `WebPDecode` and the given options. Returns the size of the
/// output: the crop window, or its scaled size.
///
/// # Errors
///
/// Returns the `Status` of the first parse, option or decode failure, as `WebPDecode` does.
#[doc(alias = "WebPDecode")]
pub fn decode_with_options(
    data: &[u8],
    mode: CspMode,
    out: &mut [u8],
    out_stride: usize,
    options: &DecodeOptions,
) -> Result<(i32, i32), Status> {
    // Port of the GetFeatures step of WebPDecode: NOT_ENOUGH_DATA is not valid here.
    match features_of(data) {
        Ok(_) => {}
        Err(Status::NotEnoughData) => return Err(Status::BitstreamError),
        Err(s) => return Err(s),
    }
    // Port of the WebPParseHeaders step of DecodeInto (have_all_data = 1).
    let headers = parse_headers(data)?;
    let rest = &data[headers.offset..];
    if headers.is_lossless {
        // RGB output: the crop origin is not snapped (WebPIoInitFromOptions with MODE_BGRA).
        let (width, height, _) = vp8l::get_info(rest).ok_or(Status::BitstreamError)?;
        let p = io_params(width, height, options, false)?;
        let (out_w, out_h) = p.output_size();
        check_output(mode, out, out_stride, out_w, out_h)?;
        let scale = p.use_scaling.then_some((p.scaled_width, p.scaled_height));
        return crate::decode_vp8l_window(rest, mode, out, out_stride, (p.x, p.y, p.w, p.h), scale);
    }
    let (width, height) =
        vp8_dec::get_info(rest, headers.compressed_size).ok_or(Status::BitstreamError)?;
    let p = io_params(width, height, options, true)?;
    let crop = Crop {
        left: p.x,
        top: p.y,
        right: p.x + p.w,
        bottom: p.y + p.h,
    };
    let planes = vp8_dec::decode_with_bypass(rest, crop, p.bypass_filtering)?;
    let alpha_plane = match headers.alpha_data {
        None => None,
        Some(alph) => {
            let window = (crop.left, crop.right, crop.top, crop.bottom);
            let mut dec =
                alpha::alpha_init(alph, width, height, window).ok_or(Status::BitstreamError)?;
            // FinishRow reports any alpha failure as BITSTREAM_ERROR, whatever the alpha decoder
            // recorded (VP8DecompressAlphaRows returns NULL, and VP8SetError overrides the status).
            if !alpha::alpha_decode(&mut dec, 0, height) {
                return Err(Status::BitstreamError);
            }
            Some(dec.plane().to_vec())
        }
    };
    let (out_w, out_h) = p.output_size();
    check_output(mode, out, out_stride, out_w, out_h)?;
    let mut io = Io::new(out, out_stride, mode, width, height);
    io.use_cropping = options.crop.is_some();
    io.crop_left = p.x;
    io.crop_top = p.y;
    io.crop_right = p.x + p.w;
    io.crop_bottom = p.y + p.h;
    io.mb_w = p.w;
    io.mb_h = p.h;
    io.use_scaling = p.use_scaling;
    io.scaled_width = p.scaled_width;
    io.scaled_height = p.scaled_height;
    io.fancy_upsampling = p.fancy_upsampling;
    io.bypass_filtering = p.bypass_filtering;
    // The batch count of FinishRow is dec->br_mb_y_: the macroblock rows decoded up to the crop
    // bottom, with the loop filter's extra rows.
    let extra = K_FILTER_EXTRA_ROWS[usize::from(planes.filter_type)];
    let br_mb_y = (((crop.bottom + 15 + extra) >> 4) as usize).min(planes.mb_h);
    if !output::emit_frame(&planes, alpha_plane.as_deref(), &mut io, crop, br_mb_y) {
        return Err(Status::UnsupportedFeature);
    }
    Ok((out_w, out_h))
}
