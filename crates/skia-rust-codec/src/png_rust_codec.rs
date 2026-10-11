// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkPngRustCodec.cpp and src/codec/SkPngRustCodec.h (chrome/m156), and the
// `SkPngRustDecoder` entry points in src/codec/SkPngRustDecoder.cpp. The `png` crate is the
// decoder (through `png_rust_ffi`, the port of `rust/png/FFI.rs`), as in Skia's build with
// `skia_use_rust_png_decode`. This is the default, non-`for_android` configuration: APNG
// frames are read, unknown chunks are not captured, and the gainmap hooks are not ported.

//! `SkPngRustCodec`: the PNG decoder built on the `png` crate, with APNG frames, interlacing,
//! sampling and subsetting, and the colour handling shared with `SkPngCodecBase`.

use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_space::{ColorSpace, named_gamut};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::raster_pipeline::{
    MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline, Stage,
};
use skia_rust_core::rect::IRect;
use skia_rust_core::stream::Stream;
use skia_rust_libpng::PngColor;
use skia_rust_skcms::{IccProfile, TransferFunction};

use crate::codec::{
    Codec, CodecBase, CodecImpl, FrameInfo, IsAnimated, Options, REPETITION_COUNT_INFINITE, Result,
};
use crate::codec_animation::{Blend, DisposalMethod};
use crate::codec_priv::get_sampled_dimension;
use crate::encoded_info::{Alpha, Color, EncodedInfo};
use crate::exif::parse_encoded_origin;
use crate::frame_holder::{Frame, FrameHolder, set_alpha_and_required_frame};
use crate::png_codec_base::{
    PaletteSource, PngCodecBase, is_compatible_color_profile_and_type, to_pixel_format,
};
use crate::png_composite_chunk_reader::{PngChunkReader, PngCompositeChunkReader};
use crate::png_rust_ffi::{DecodingResult, Reader};
use crate::sampler::Sampler;
use crate::swizzler::Swizzler;

/// Port of `kGraySigBit_GrayAlphaIsJustAlpha` (SkPngPriv.h): the `sBIT` value that marks a gray
/// plus alpha image whose gray channel is just alpha.
const GRAY_SIG_BIT_GRAY_ALPHA_IS_JUST_ALPHA: u8 = 1;

/// Port of `kMaxPNGSize` in `SkPngRustCodec::CreateEncodedInfo` (<http://bugzil.la/251381>).
const MAX_PNG_SIZE: u32 = 1_000_000;

/// The colour profile of a PNG, as `SkCodecs::ColorProfile` holds it: the ICC data it was parsed
/// from (kept, so `getEncodedData` and the encoded info can share it), or a profile made from
/// `cICP`, `gAMA`/`cHRM` or a colour space.
// Port of: src/codec/SkCodecColorProfile.h (SkCodecs::ColorProfile), chrome/m156
enum ColorProfile {
    /// An ICC profile that was read from the `iCCP` chunk.
    Icc {
        data: Arc<[u8]>,
        profile: IccProfile,
    },
    /// A profile generated from the colour information (no ICC bytes).
    Generated(IccProfile),
}

impl ColorProfile {
    fn profile(&self) -> &IccProfile {
        match self {
            Self::Icc { profile, .. } | Self::Generated(profile) => profile,
        }
    }
}

// Port of: src/codec/SkPngRustCodec.cpp#L43-L67 (ToColor, chrome/m156)
fn to_color(color_type: png::ColorType, reader: &Reader<'_>) -> Color {
    match color_type {
        png::ColorType::Grayscale => Color::Gray,
        png::ColorType::Rgb => {
            if let Some(sbit) = reader.get_sbit_chunk() {
                // Restricting to 8-bit images, because of `SkEncodedInfo` asserts.
                if sbit.len() == 3
                    && reader.output_bits_per_component() == 8
                    && sbit[0] == 5
                    && sbit[1] == 6
                    && sbit[2] == 5
                {
                    return Color::Color565;
                }
            }
            Color::RGB
        }
        png::ColorType::GrayscaleAlpha => {
            if reader.get_sbit_chunk().is_some_and(|sbit| {
                sbit.len() == 2
                    && reader.output_bits_per_component() == 8
                    && sbit[0] == GRAY_SIG_BIT_GRAY_ALPHA_IS_JUST_ALPHA
                    && sbit[1] == 8
            }) {
                return Color::XAlpha;
            }
            Color::GrayAlpha
        }
        png::ColorType::Rgba => Color::RGBA,
        png::ColorType::Indexed => Color::Palette,
    }
}

// Port of: src/codec/SkPngRustCodec.cpp#L69-L87 (ToAlpha, chrome/m156)
fn to_alpha(color_type: png::ColorType, reader: &Reader<'_>) -> Alpha {
    match color_type {
        png::ColorType::Grayscale | png::ColorType::Rgb => Alpha::Opaque,
        png::ColorType::GrayscaleAlpha | png::ColorType::Rgba => Alpha::Unpremul,
        png::ColorType::Indexed => {
            if reader.get_trns_chunk().is_some() {
                Alpha::Unpremul
            } else {
                Alpha::Opaque
            }
        }
    }
}

// Port of: src/codec/SkPngRustCodec.cpp#L89-L101 (ToDisposalMethod, chrome/m156)
fn to_disposal_method(op: png::DisposeOp) -> DisposalMethod {
    match op {
        png::DisposeOp::None => DisposalMethod::Keep,
        png::DisposeOp::Background => DisposalMethod::RestoreBgColor,
        png::DisposeOp::Previous => DisposalMethod::RestorePrevious,
    }
}

// Port of: src/codec/SkPngRustCodec.cpp#L103-L112 (ToBlend, chrome/m156)
fn to_blend(op: png::BlendOp) -> Blend {
    match op {
        png::BlendOp::Source => Blend::Src,
        png::BlendOp::Over => Blend::SrcOver,
    }
}

// Port of: src/codec/SkPngRustCodec.cpp#L278-L289 (ToSkCodecResult, chrome/m156)
fn to_codec_result(result: DecodingResult) -> Result {
    match result {
        DecodingResult::Success | DecodingResult::EndOfFrame => Result::Success,
        DecodingResult::FormatError => Result::ErrorInInput,
        DecodingResult::ParameterError => Result::InvalidParameters,
        DecodingResult::OtherIoError | DecodingResult::LimitsExceededError => Result::InternalError,
        DecodingResult::IncompleteInput => Result::IncompleteInput,
    }
}

/// Port of `SkPngRustCodec::CreateColorProfile`: the `cICP`, `iCCP`, `gAMA`/`cHRM` colour
/// information, in the precedence the spec gives it.
// Port of: src/codec/SkPngRustCodec.cpp#L114-L186 (CreateColorProfile, chrome/m156)
fn create_color_profile(reader: &Reader<'_>) -> Option<ColorProfile> {
    // Considering the `cICP` chunk first, because the spec at
    // https://www.w3.org/TR/png-3/#cICP-chunk says: "This chunk, if understood by the decoder, is
    // the highest-precedence color chunk."
    if let Some((primaries, transfer, matrix, full_range)) = reader.try_get_cicp_chunk() {
        // RGB is the only colour model in PNG, so the matrix coefficients are 0. Narrow range
        // images are rare and not supported (see `SkColorSpace::MakeCICP`).
        let primaries = skia_rust_core::color_space::named_primaries::CicpId::from_u8(primaries);
        let transfer = skia_rust_core::color_space::named_transfer_fn::CicpId::from_u8(transfer);
        if let (true, Some(primaries), Some(transfer)) =
            (matrix == 0 && full_range, primaries, transfer)
            && let Some(color_space) = ColorSpace::new_cicp(primaries, transfer)
        {
            return Some(ColorProfile::Generated(color_space.to_profile()));
        }
    }

    if let Some(bytes) = reader.get_iccp_chunk() {
        let data: Arc<[u8]> = Arc::from(bytes);
        if let Some(profile) = skia_rust_skcms::parse(&data) {
            return Some(ColorProfile::Icc { data, profile });
        }
    }

    if reader.is_srgb() {
        // TODO(https://crbug.com/362304558): Consider the intent field from the `sRGB` chunk.
        return None;
    }

    // Next, check for presence of `gAMA` and `cHRM` chunks. Unlike libpng, the `png` crate does not
    // validate the gamma value, so zero (which the wild does contain) is rejected here.
    let gamma = match reader.try_get_gama() {
        Some(gamma) if gamma > 0.0 => gamma,
        _ => return None,
    };

    // The `cHRM` chunk is ignored without a `gAMA` chunk (Chromium's 83587041dc5f1428c09).
    let chrm = reader.try_get_chrm();

    if chrm.is_none() {
        // If there is no `cHRM` chunk then check if `gamma` is neutral (in PNG / `k2Dot2` sense).
        // `kPngGammaThreshold` mimics `PNG_GAMMA_THRESHOLD_FIXED` from libpng.
        const K_PNG_GAMMA_THRESHOLD: f32 = 0.05;
        const K_MIN_NEUTRAL_VALUE: f32 = 1.0 - K_PNG_GAMMA_THRESHOLD;
        const K_MAX_NEUTRAL_VALUE: f32 = 1.0 + K_PNG_GAMMA_THRESHOLD;
        let tmp = gamma * 2.2_f32;
        let is_neutral = K_MIN_NEUTRAL_VALUE < tmp && tmp < K_MAX_NEUTRAL_VALUE;
        if is_neutral {
            return None;
        }
    }

    // Construct a colour profile based on `cHRM` and `gAMA` chunks.
    let to_xyzd50 = if let Some(chrm) = chrm {
        skia_rust_skcms::primaries_to_xyzd50(
            chrm.rx, chrm.ry, chrm.gx, chrm.gy, chrm.bx, chrm.by, chrm.wx, chrm.wy,
        )?
    } else {
        // `blink::PNGImageDecoder` returns a null colour profile when `gAMA` is present without
        // `cHRM`. The sRGB gamut is used instead, because the gamma is applied by `skcms`.
        named_gamut::SRGB
    };

    let transfer = TransferFunction {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
        e: 0.0,
        f: 0.0,
        g: 1.0 / gamma,
    };
    let mut profile = IccProfile::new();
    profile.set_transfer_function(&transfer);
    profile.set_xyzd50(&to_xyzd50);
    Some(ColorProfile::Generated(profile))
}

/// Port of `SkPngRustCodec::CreateEncodedInfo`: the encoded information of the image. `None` when
/// the image is too large.
// Port of: src/codec/SkPngRustCodec.cpp#L188-L226 (CreateEncodedInfo, chrome/m156)
fn create_encoded_info(reader: &Reader<'_>) -> Option<EncodedInfo> {
    let rust_color = reader.output_color_type();
    let sk_color = to_color(rust_color, reader);
    let mut profile = create_color_profile(reader);
    if !is_compatible_color_profile_and_type(profile.as_ref().map(ColorProfile::profile), sk_color)
    {
        profile = None;
    }

    // Protect against large PNGs. See http://bugzil.la/251381 for more details.
    if reader.width() > MAX_PNG_SIZE || reader.height() > MAX_PNG_SIZE {
        return None;
    }
    let width = i32::try_from(reader.width()).ok()?;
    let height = i32::try_from(reader.height()).ok()?;
    let bits = reader.output_bits_per_component();
    let encoded = EncodedInfo::make_with_depth(
        width,
        height,
        sk_color,
        to_alpha(rust_color, reader),
        bits,
        bits,
    );
    Some(match profile {
        Some(ColorProfile::Icc { data, profile }) => encoded.with_profile(data, profile),
        Some(ColorProfile::Generated(profile)) => encoded.with_generated_profile(profile),
        None => encoded,
    })
}

/// Port of `SkPngRustCodec::GetEncodedOrigin`.
// Port of: src/codec/SkPngRustCodec.cpp#L228-L238 (GetEncodedOrigin, chrome/m156)
fn get_encoded_origin(reader: &Reader<'_>) -> EncodedOrigin {
    reader
        .get_exif_chunk()
        .and_then(parse_encoded_origin)
        .unwrap_or(EncodedOrigin::TopLeft)
}

/// Port of `IsValidFctlIfAny`: an `fcTL` before the first `IDAT` must have the image's size and
/// no offset.
// Port of: src/codec/SkPngRustCodec.cpp#L240-L281 (IsValidFctlIfAny, chrome/m156)
fn is_valid_fctl_if_any(image_info: &EncodedInfo, reader: &Reader<'_>) -> bool {
    if reader.has_fctl_chunk() {
        let (width, height, x_offset, y_offset, _, _, _) = reader.get_fctl_info();
        let (Ok(frame_width), Ok(frame_height)) = (i32::try_from(width), i32::try_from(height))
        else {
            return false;
        };
        if x_offset != 0
            || frame_width != image_info.width()
            || y_offset != 0
            || frame_height != image_info.height()
        {
            return false;
        }
    }
    true
}

/// Port of `ReadUnknownChunks`. The non-`for_android` build of the `png` crate captures no
/// chunks, so there is nothing to hand to the chunk reader.
// Port of: src/codec/SkPngRustCodec.cpp#L283-L293 (ReadUnknownChunks, chrome/m156)
fn read_unknown_chunks(reader: &Reader<'_>, _chunk_reader: &Mutex<PngCompositeChunkReader>) {
    debug_assert_eq!(reader.get_unknown_chunks_count(), 0);
}

/// Port of `blendRow`: source-over blends `src` onto `dst`, pixel by pixel, in `color`. The
/// unpremultiplied case premultiplies both sides around the blend.
// Port of: src/codec/SkPngRustCodec.cpp#L296-L318 (blendRow, chrome/m156)
fn blend_row(dst: &mut [u8], src: &[u8], color: ColorType, alpha: AlphaType) {
    let dst_ctx = MemoryCtx::new(MemSlot(0));
    let src_ctx = MemoryCtx::new(MemSlot(1));
    let mut p = RasterPipeline::new();
    p.append_load_dst(color, dst_ctx);
    if alpha == AlphaType::Unpremul {
        p.append(Stage::PremulDst);
    }
    p.append_load(color, src_ctx);
    if alpha == AlphaType::Unpremul {
        p.append(Stage::Premul);
    }
    p.append(Stage::Srcover);
    if alpha == AlphaType::Unpremul {
        p.append(Stage::Unpremul);
    }
    p.append_store(color, dst_ctx);

    let bpp = color.bytes_per_pixel();
    let width = src.len() / bpp;
    let mut mem = MemoryBindings::new();
    mem.bind(MemSlot(0), MemView::write(dst));
    mem.bind(MemSlot(1), MemView::read(src));
    p.run(0, 0, width, 1, &mut mem);
}

/// Port of `blendAllRows`: blends each full row of `src_frame` onto the rows of `dst_frame`.
// Port of: src/codec/SkPngRustCodec.cpp#L320-L332 (blendAllRows, chrome/m156)
fn blend_all_rows(
    dst_frame: &mut [u8],
    src_frame: &[u8],
    row_size: usize,
    row_stride: usize,
    color: ColorType,
    alpha: AlphaType,
) {
    let mut src_pos = 0;
    let mut dst_pos = 0;
    while src_frame.len() - src_pos >= row_size {
        blend_row(
            &mut dst_frame[dst_pos..],
            &src_frame[src_pos..src_pos + row_size],
            color,
            alpha,
        );
        src_pos += row_stride.min(src_frame.len() - src_pos);
        dst_pos += row_stride.min(dst_frame.len() - dst_pos);
    }
}

/// The destination of a decode, once its rows are known: a window of the caller's buffer.
/// `start..end` is `fDst`, the span of the caller's buffer that still has to be written.
// Port of: src/codec/SkPngRustCodec.h#L62-L87 (DecodingDstInfo, chrome/m156)
#[derive(Clone, Copy, Debug)]
struct DecodingDst {
    row_stride: usize,
    start: usize,
    end: usize,
    row_size: usize,
    row_count: usize,
    bytes_per_pixel: u8,
}

/// Port of `DecodingState::fDst`: the destination pointer, before the sampling and subsetting
/// parameters are known (`UninitializedDstInfo`), or the window once they are.
// Port of: src/codec/SkPngRustCodec.h#L89-L118 (UninitializedDstInfo / DecodingState::fDst)
#[derive(Clone, Copy, Debug)]
enum DstState {
    Uninitialized { row_stride: usize },
    Decoding(DecodingDst),
}

/// The state of one decode, kept between incremental calls. Port of `SkPngRustCodec::DecodingState`.
// Port of: src/codec/SkPngRustCodec.h#L89-L170 (DecodingState, chrome/m156)
#[derive(Debug)]
struct DecodingState {
    dst: DstState,
    /// Port of `fPreblendBuffer`: the frame's previous contents, blended with, or the whole
    /// encoded image for interlaced subsets. Empty when there is nothing to blend.
    preblend: Vec<u8>,
    first_row: i32,
    last_row: i32,
    /// Port of `fCurrentSourceRow`.
    current_source_row: i32,
    /// Port of `fRowsWrittenToOutput`.
    rows_written_to_output: i32,
    /// Port of `fYByteOffset`: the offset of the subset's first row in the encoded image.
    y_byte_offset: usize,
}

impl DecodingState {
    fn dst(&self) -> DecodingDst {
        match self.dst {
            DstState::Decoding(dst) => dst,
            DstState::Uninitialized { .. } => panic!("destination is not initialized"),
        }
    }

    fn set_dst(&mut self, dst: DecodingDst) {
        self.dst = DstState::Decoding(dst);
    }
}

/// The window `start..end` of `buf`, as a slice.
fn window<'b>(buf: &'b mut [u8], dst: &DecodingDst) -> &'b mut [u8] {
    &mut buf[dst.start..dst.end]
}

/// Port of `SkSpan::subspan` on a window: moves `start` forward by `n`, never past `end`.
fn advance(dst: &mut DecodingDst, n: usize) {
    dst.start = (dst.start + n).min(dst.end);
}

/// A dimension (or index) as `usize`. Dimensions are positive once the header is read, and frame
/// indices are checked against the frame count before use, so the conversion is exact.
fn dim(value: i32) -> usize {
    usize::try_from(value).unwrap_or(0)
}

/// The `tRNS` chunk as palette alpha, when the image is indexed. Port of `onTryGetTrnsChunk`.
// Port of: src/codec/SkPngRustCodec.cpp#L1278-L1286 (onTryGetTrnsChunk, chrome/m156)
fn try_get_trns_chunk<'r>(reader: &'r Reader<'_>) -> Option<&'r [u8]> {
    if reader.output_color_type() != png::ColorType::Indexed {
        return None;
    }
    reader.get_trns_chunk()
}

/// The palette of an indexed image, as `onTryGetPlteChunk` gives it: whole three-byte entries.
// Port of: src/codec/SkPngRustCodec.cpp#L1262-L1276 (onTryGetPlteChunk, chrome/m156)
fn try_get_plte_chunk(reader: &Reader<'_>) -> Option<Vec<PngColor>> {
    if reader.output_color_type() != png::ColorType::Indexed {
        return None;
    }
    let bytes = reader.get_plte_chunk()?;
    Some(
        bytes
            .as_chunks::<3>()
            .0
            .iter()
            .map(|entry| PngColor {
                red: entry[0],
                green: entry[1],
                blue: entry[2],
            })
            .collect(),
    )
}

/// Port of `SkPngRustCodec::FrameHolder`: the frames of the image, with their "fully received"
/// flags. Frame `i` of `frames` is frame id `i`.
// Port of: src/codec/SkPngRustCodec.h#L227-L252 (FrameHolder, chrome/m156)
#[derive(Debug)]
pub(crate) struct PngFrames {
    screen_width: i32,
    screen_height: i32,
    frames: Vec<Frame>,
    fully_received: Vec<bool>,
}

impl PngFrames {
    // Port of: src/codec/SkPngRustCodec.cpp#L1019-L1024 (FrameHolder ctor, chrome/m156)
    fn new(width: i32, height: i32) -> Self {
        Self {
            screen_width: width,
            screen_height: height,
            frames: Vec::new(),
            fully_received: Vec::new(),
        }
    }

    /// Port of `FrameHolder::size`.
    fn size(&self) -> i32 {
        i32::try_from(self.frames.len()).unwrap_or(i32::MAX)
    }

    /// Port of `FrameHolder::markFrameAsFullyReceived`.
    // Port of: src/codec/SkPngRustCodec.cpp#L1152-L1156 (markFrameAsFullyReceived, chrome/m156)
    fn mark_frame_as_fully_received(&mut self, index: usize) {
        self.fully_received[index] = true;
    }

    /// Port of `FrameHolder::getFrameInfo`.
    // Port of: src/codec/SkPngRustCodec.cpp#L1158-L1166 (getFrameInfo, chrome/m156)
    fn get_frame_info(&self, index: i32, info: Option<&mut FrameInfo>) -> bool {
        let Some(index) = usize::try_from(index)
            .ok()
            .filter(|&i| i < self.frames.len())
        else {
            return false;
        };
        if let Some(info) = info {
            *info = self.frames[index].fill_in(self.fully_received[index]);
        }
        true
    }

    /// Port of `FrameHolder::appendNewFrame`: adds the frame described by the reader's current
    /// `fcTL` chunk, or the image itself when the file has no animation.
    // Port of: src/codec/SkPngRustCodec.cpp#L1168-L1200 (appendNewFrame, chrome/m156)
    fn append_new_frame(&mut self, reader: &Reader<'_>, info: &EncodedInfo) -> Result {
        let Ok(id) = i32::try_from(self.frames.len()) else {
            return Result::ErrorInInput;
        };
        if reader.has_actl_chunk() && reader.has_fctl_chunk() {
            if let Some(last) = self.fully_received.last_mut() {
                // The previous frame's `fdAT` / `IDAT` chunks all came before this `fcTL`.
                *last = true;
            }
            let mut frame = Frame::new(id, info.alpha());
            let result = self.set_frame_info_from_current_fctl_chunk(reader, &mut frame);
            if result == Result::Success {
                self.frames.push(frame);
                self.fully_received.push(false);
            }
            return result;
        }

        debug_assert!(!reader.has_actl_chunk() || !reader.has_fctl_chunk());
        debug_assert_eq!(id, 0);
        let mut frame = Frame::new(id, info.alpha());
        frame.set_xywh(0, 0, info.width(), info.height());
        frame.set_blend(Blend::Src);
        set_alpha_and_required_frame(self.screen_width, self.screen_height, &[], &mut frame);
        self.frames.push(frame);
        self.fully_received.push(false);
        Result::Success
    }

    /// Port of `FrameHolder::setFrameInfoFromCurrentFctlChunk`.
    // Port of: src/codec/SkPngRustCodec.cpp#L1202-L1232 (setFrameInfoFromCurrentFctlChunk, chrome/m156)
    fn set_frame_info_from_current_fctl_chunk(
        &self,
        reader: &Reader<'_>,
        frame: &mut Frame,
    ) -> Result {
        debug_assert!(reader.has_fctl_chunk());
        let (width, height, x_offset, y_offset, dispose_op, mut blend_op, duration_ms) =
            reader.get_fctl_info();
        let (Ok(x), Ok(y), Ok(w), Ok(h), Ok(duration)) = (
            i32::try_from(x_offset),
            i32::try_from(y_offset),
            i32::try_from(width),
            i32::try_from(height),
            i32::try_from(duration_ms),
        ) else {
            return Result::ErrorInInput;
        };
        frame.set_xywh(x, y, w, h);
        frame.set_duration(duration);
        frame.set_disposal_method(to_disposal_method(dispose_op));
        // https://wiki.mozilla.org/APNG_Specification#.60fcTL.60:_The_Frame_Control_Chunk says that
        // "for the first frame the two blend modes are functionally equivalent", so `Source` is
        // used because it is cheaper.
        if frame.frame_id() == 0 {
            blend_op = png::BlendOp::Source;
        }
        frame.set_blend(to_blend(blend_op));
        // `set_alpha_and_required_frame` goes last: it depends on the properties set above.
        set_alpha_and_required_frame(self.screen_width, self.screen_height, &self.frames, frame);
        Result::Success
    }
}

impl FrameHolder for PngFrames {
    fn screen_width(&self) -> i32 {
        self.screen_width
    }

    fn screen_height(&self) -> i32 {
        self.screen_height
    }

    // Port of: SkPngRustCodec::FrameHolder::onGetFrame (src/codec/SkPngRustCodec.cpp#L1139-L1147)
    fn get_frame(&self, i: i32) -> Option<&Frame> {
        usize::try_from(i).ok().and_then(|i| self.frames.get(i))
    }
}

/// `SkPngRustCodec`: the PNG decoder over the `png` crate. Port of the class, with the stream the
/// codec reads through and its reader.
// Port of: src/codec/SkPngRustCodec.h#L38-L110 (chrome/m156)
#[doc(alias = "SkPngRustCodec")]
pub struct PngRustCodec<'a> {
    base_png: PngCodecBase,
    /// Port of `fReader`. Replaced when the stream is rewound.
    reader: Reader<'a>,
    /// Port of `fPrivStream`: the stream the codec reads through. The base codec does not hold it.
    priv_stream: Arc<Mutex<Box<dyn Stream + Send + 'a>>>,
    /// Port of `fEncodedInfo`: a copy of the base's encoded information, for the frame parsing
    /// that runs without a base.
    encoded: EncodedInfo,
    /// Port of `fFrameAtCurrentStreamPosition`: `-1` when `IDAT` is not part of an animation.
    frame_at_current_stream_position: i32,
    /// Port of `fStreamIsPositionedAtStartOfFrameData`.
    stream_is_positioned_at_start_of_frame_data: bool,
    /// Port of `fMaxStreamLengthSeenWhenParsingAdditionalFrameInfos`.
    max_stream_length_seen: Option<usize>,
    /// Port of `fIncrementalDecodingState`.
    incremental: Option<DecodingState>,
    /// Port of `fFrameHolder`.
    frame_holder: PngFrames,
    /// Port of `fCanParseAdditionalFrameInfos`.
    can_parse_additional_frame_infos: bool,
}

impl std::fmt::Debug for PngRustCodec<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PngRustCodec")
            .field("encoded", &self.encoded)
            .field(
                "frame_at_current_stream_position",
                &self.frame_at_current_stream_position,
            )
            .field("frames", &self.frame_holder)
            .finish_non_exhaustive()
    }
}

impl PngRustCodec<'_> {
    /// Port of `getRawFrameCount`: the frames the `acTL` chunk declares, or 1 without one.
    // Port of: src/codec/SkPngRustCodec.cpp#L622-L630 (getRawFrameCount, chrome/m156)
    fn raw_frame_count(&self) -> i32 {
        if !self.reader.has_actl_chunk() {
            return 1;
        }
        // `Sk64_pin_to_s32`.
        i32::try_from(self.reader.get_actl_num_frames()).unwrap_or(i32::MAX)
    }

    /// Port of `isAnimated() == kYes` as the codec computes it.
    fn is_animated_value(&self) -> IsAnimated {
        if self.reader.has_actl_chunk() && self.reader.get_actl_num_frames() > 1 {
            IsAnimated::Yes
        } else {
            IsAnimated::No
        }
    }

    /// Port of `SkPngRustCodec::isLastFrame`.
    // Port of: src/codec/SkPngRustCodec.cpp#L521-L524 (isLastFrame, chrome/m156)
    fn is_last_frame(&self, base: &CodecBase<'_>) -> bool {
        self.is_animated_value() == IsAnimated::No
            || base.options().frame_index + 1 == self.raw_frame_count()
    }

    /// Port of `SkPngRustCodec::isSampling`.
    // Port of: src/codec/SkPngRustCodec.cpp#L526-L528 (isSampling, chrome/m156)
    fn is_sampling(&self) -> bool {
        self.base_png
            .swizzler
            .as_ref()
            .is_some_and(|swizzler| swizzler.sample_x() > 1 || swizzler.sample_y() > 1)
    }

    /// Port of `SkPngRustCodec::supportsSamplingOrSubsetting`: only static, full-canvas images.
    // Port of: src/codec/SkPngRustCodec.cpp#L529-L536 (supportsSamplingOrSubsetting, chrome/m156)
    fn supports_sampling_or_subsetting(&self, base: &CodecBase<'_>) -> bool {
        if self.is_animated_value() != IsAnimated::No || self.frame_holder.size() == 0 {
            return false;
        }
        self.frame_holder
            .get_frame(0)
            .is_some_and(|frame| frame.frame_rect() == IRect::from_size(base.dimensions()))
    }

    /// Port of `SkPngRustCodec::finishStreamAfterLastFrame`. Nothing to finish without
    /// `for_android`.
    // Port of: src/codec/SkPngRustCodec.cpp#L538 (finishStreamAfterLastFrame, chrome/m156)
    fn finish_stream_after_last_frame(&mut self) {
        let _ = self.reader.finish_decoding();
    }

    /// Port of `SkPngRustCodec::canReadRow`: whether rows can be decoded straight into `dst`.
    // Port of: src/codec/SkPngRustCodec.cpp#L545-L593 (canReadRow, chrome/m156)
    fn can_read_row(&self, base: &CodecBase<'_>) -> bool {
        let dst_info = base.dst_info();
        if dst_info.alpha_type() != AlphaType::Unpremul {
            return false;
        }
        // Subsetting and sub-sampling need the whole row decoded first.
        if base.options().subset.is_some() || self.is_sampling() {
            return false;
        }
        let color_type = self.reader.output_color_type();
        let bits_per_component = self.reader.output_bits_per_component();
        match dst_info.color_type() {
            ColorType::RGBA8888 => {
                if color_type != png::ColorType::Rgba || bits_per_component != 8 {
                    return false;
                }
            }
            ColorType::Gray8 => {
                if color_type != png::ColorType::Grayscale || bits_per_component != 8 {
                    return false;
                }
            }
            _ => return false,
        }
        // Check profiles.
        let encoded_profile = base.encoded_info().profile();
        let dst_color_space = dst_info.color_space();
        if encoded_profile.is_some() != dst_color_space.is_some() {
            return false;
        }
        if let (Some(encoded_profile), Some(dst_color_space)) = (encoded_profile, dst_color_space) {
            let dst_profile = dst_color_space.to_profile();
            if !skia_rust_skcms::approximately_equal_profiles(encoded_profile, &dst_profile) {
                return false;
            }
        }
        true
    }

    /// Port of `SkPngRustCodec::readToStartOfNextFrame`.
    // Port of: src/codec/SkPngRustCodec.cpp#L384-L397 (readToStartOfNextFrame, chrome/m156)
    fn read_to_start_of_next_frame(&mut self) -> Result {
        debug_assert!(self.frame_at_current_stream_position < self.raw_frame_count());
        let result = to_codec_result(self.reader.next_frame_info());
        if result != Result::Success {
            self.stream_is_positioned_at_start_of_frame_data = false;
            return result;
        }
        self.stream_is_positioned_at_start_of_frame_data = true;
        self.frame_at_current_stream_position += 1;
        if self.frame_at_current_stream_position == self.frame_holder.size() {
            return self
                .frame_holder
                .append_new_frame(&self.reader, &self.encoded);
        }
        Result::Success
    }

    /// Port of `SkPngRustCodec::seekToStartOfFrame`: rewinds the stream if the frame is behind the
    /// current position, then reads frame by frame up to `index`.
    // Port of: src/codec/SkPngRustCodec.cpp#L399-L436 (seekToStartOfFrame, chrome/m156)
    fn seek_to_start_of_frame(&mut self, index: i32) -> Result {
        debug_assert!((0..=self.frame_holder.size()).contains(&index));
        if index < self.frame_at_current_stream_position
            || (index == self.frame_at_current_stream_position
                && !self.stream_is_positioned_at_start_of_frame_data)
        {
            let rewound = self
                .priv_stream
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .rewind();
            if !rewound {
                return Result::CouldNotRewind;
            }
            // The constructor read this header, so reading it again succeeds.
            let reader =
                Reader::new(Arc::clone(&self.priv_stream)).expect("the PNG header was read before");
            self.reader = reader;
            read_unknown_chunks(&self.reader, &self.base_png.chunk_reader);
            let idat_is_not_part_of_animation =
                self.reader.has_actl_chunk() && !self.reader.has_fctl_chunk();
            self.frame_at_current_stream_position =
                if idat_is_not_part_of_animation { -1 } else { 0 };
            self.stream_is_positioned_at_start_of_frame_data = true;
        }
        while self.frame_at_current_stream_position < index {
            let result = self.read_to_start_of_next_frame();
            if result != Result::Success {
                return result;
            }
        }
        Result::Success
    }

    /// Port of `SkPngRustCodec::parseAdditionalFrameInfos`.
    // Port of: src/codec/SkPngRustCodec.cpp#L438-L450 (parseAdditionalFrameInfos, chrome/m156)
    fn parse_additional_frame_infos(&mut self) -> Result {
        while self.frame_holder.size() < self.raw_frame_count() {
            let old_frame_count = self.frame_holder.size();
            let result = self.seek_to_start_of_frame(self.frame_holder.size());
            if result != Result::Success {
                return result;
            }
            debug_assert_eq!(self.frame_holder.size(), old_frame_count + 1);
        }
        Result::Success
    }

    /// Port of `SkPngRustCodec::getSubsetOrSampleFromFullImage`: copies the rows of an interlaced
    /// image that a subset or sample needs from the whole encoded image.
    // Port of: src/codec/SkPngRustCodec.cpp#L452-L494 (getSubsetOrSampleFromFullImage, chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    fn get_subset_or_sample_from_full_image(
        &mut self,
        base: &mut CodecBase<'_>,
        full_image: &[u8],
        dst: &mut [u8],
        dst_row_stride: usize,
        offset: usize,
        src_height: i32,
        dst_height: i32,
        dst_row_size: usize,
    ) {
        debug_assert!(self.reader.interlaced());
        let encoded_row_bytes = self.base_png.encoded_row_bytes;
        let full = &full_image[offset..];
        let mut full_pos = 0;
        let mut dst_pos = 0;
        let mut rows_written = 0;
        for y in 0..src_height {
            if rows_written >= dst_height {
                break;
            }
            let src_row = &full[full_pos..full_pos + encoded_row_bytes];
            full_pos += encoded_row_bytes;
            // Skip rows that the vertical sampler does not need.
            if self
                .base_png
                .swizzler
                .as_ref()
                .is_some_and(|swizzler| !swizzler.row_needed(y))
            {
                continue;
            }
            let row_len = dst_row_size.min(dst.len() - dst_pos);
            // Advancing by a whole stride is robust if the last row lacks stride padding.
            let stride = dst_row_stride.min(dst.len() - dst_pos);
            self.base_png
                .apply_xform_row(base, &mut dst[dst_pos..dst_pos + row_len], src_row);
            dst_pos += stride;
            rows_written += 1;
        }
    }

    /// Port of `SkPngRustCodec::startDecoding`: validates the request and makes the decoding state.
    // Port of: src/codec/SkPngRustCodec.cpp#L637-L705 (startDecoding, chrome/m156)
    fn start_decoding(
        &mut self,
        base: &mut CodecBase<'_>,
        row_bytes: usize,
        options: &Options,
        decoding_state: &mut Option<DecodingState>,
    ) -> Result {
        if options.frame_index == 0 && self.frame_holder.size() == 0 {
            let result = self.seek_to_start_of_frame(0);
            if result != Result::Success {
                return result;
            }
        }
        if options.frame_index < 0 || options.frame_index >= self.frame_holder.size() {
            return Result::InvalidParameters;
        }
        let Some(frame) = self.frame_holder.get_frame(options.frame_index) else {
            panic!("the frame index was validated");
        };
        let rect = frame.frame_rect();
        let frame_width = rect.width();
        let frame_height = rect.height();

        // TODO(https://crbug.com/362830091): Consider handling `fSubset` for APNG.
        if options.subset.is_some() && !self.supports_sampling_or_subsetting(base) {
            return Result::Unimplemented;
        }

        let result = self.seek_to_start_of_frame(options.frame_index);
        if result != Result::Success {
            return result;
        }

        // https://www.w3.org/TR/png-3/#11PLTE says that for colour type 3 (indexed-color), the
        // PLTE chunk is required. `Codec_InvalidImages` expects that we will succeed in this case
        // and produce *some* output. This check follows `seekToStartOfFrame`, which may reset
        // whether the reader has seen a PLTE chunk.
        if base.encoded_info().color() == Color::Palette && !self.reader.has_plte_chunk() {
            return Result::InvalidInput;
        }

        let plte = try_get_plte_chunk(&self.reader);
        let palette = PaletteSource {
            plte: plte.as_deref(),
            trns: try_get_trns_chunk(&self.reader),
        };
        let result = self
            .base_png
            .initialize_xforms(base, options, frame_width, &palette);
        if result != Result::Success {
            return result;
        }

        let (y_byte_offset, first_row, last_row) = if let Some(subset) = options.subset {
            let Some(y_byte_offset) = self
                .encoded_row_bytes_usize()
                .checked_mul(usize::try_from(subset.top()).unwrap_or(0))
            else {
                return Result::ErrorInInput;
            };
            (y_byte_offset, subset.top(), subset.bottom() - 1)
        } else {
            (0, 0, frame_height - 1)
        };

        *decoding_state = Some(DecodingState {
            dst: DstState::Uninitialized {
                row_stride: row_bytes,
            },
            preblend: Vec::new(),
            first_row,
            last_row,
            current_source_row: 0,
            rows_written_to_output: 0,
            y_byte_offset,
        });
        Result::Success
    }

    /// The encoded row size as `usize`.
    fn encoded_row_bytes_usize(&self) -> usize {
        self.base_png.encoded_row_bytes
    }

    /// Port of `SkPngRustCodec::initializeDecodingDst`: computes the window of `dst` the decode
    /// writes, and the preblend buffer. `dst_len` is the length of the caller's buffer.
    // Port of: src/codec/SkPngRustCodec.cpp#L707-L815 (initializeDecodingDst, chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn initialize_decoding_dst(
        &mut self,
        base: &mut CodecBase<'_>,
        state: &mut DecodingState,
        dst_len: usize,
    ) -> Result {
        let DstState::Uninitialized {
            row_stride: uninit_stride,
        } = state.dst
        else {
            return Result::Success;
        };
        let options = base.options().clone();
        let Some(frame) = self.frame_holder.get_frame(options.frame_index) else {
            panic!("the frame index was validated");
        };
        let rect = frame.frame_rect();
        let frame_width = rect.width();
        let frame_height = rect.height();
        let frame_x = rect.left();
        let frame_y = rect.top();
        let frame_blend = frame.blend();

        let Ok(dst_bytes_per_pixel) = u8::try_from(base.dst_info().bytes_per_pixel()) else {
            return Result::InvalidParameters;
        };
        if dst_bytes_per_pixel >= 32 {
            return Result::InvalidParameters;
        }
        let dst_row_count = dim(frame_height);

        let dst_row_size = if self.can_read_row(base) {
            dst_bytes_per_pixel as usize * dim(frame_width)
        } else {
            self.base_png.initialize_xform_params(base);
            self.base_png.dst_row_size
        };

        let mut span = DecodingDst {
            row_stride: uninit_stride,
            start: 0,
            end: 0,
            row_size: dst_row_size,
            row_count: dst_row_count,
            bytes_per_pixel: dst_bytes_per_pixel,
        };
        if self.is_sampling() || options.subset.is_some() {
            if !self.supports_sampling_or_subsetting(base) {
                return Result::InvalidParameters;
            }
            if uninit_stride < dst_row_size {
                return Result::InvalidParameters;
            }
            let src_height = state.last_row - state.first_row + 1;
            let sample_y = self
                .base_png
                .swizzler
                .as_ref()
                .map_or(1, Swizzler::sample_y);
            let dst_height = get_sampled_dimension(src_height, sample_y);
            let Some(dst_buffer_size) = usize::try_from(dst_height)
                .ok()
                .and_then(|h| h.checked_mul(uninit_stride))
            else {
                return Result::ErrorInInput;
            };
            span.end = dst_buffer_size.min(dst_len);
        } else {
            if uninit_stride < base.dst_info().min_row_bytes() {
                return Result::InvalidParameters;
            }
            let image_height = dim(base.dst_info().height());
            let Some(image_size) = uninit_stride.checked_mul(image_height) else {
                return Result::ErrorInInput;
            };
            let x_byte_offset_frame = dst_bytes_per_pixel as usize * dim(frame_x);
            let y_byte_offset_frame = uninit_stride * dim(frame_y);
            let Some(frame_height_times_row_stride) = dst_row_count.checked_mul(uninit_stride)
            else {
                return Result::ErrorInInput;
            };
            // `SkSpan(ptr, imageSize).subspan(x).subspan(y)`, then `first(...)` when shorter.
            span.end = image_size.min(dst_len);
            span.start = x_byte_offset_frame.min(span.end);
            span.start = (span.start + y_byte_offset_frame).min(span.end);
            if frame_height_times_row_stride < span.end - span.start {
                span.end = span.start + frame_height_times_row_stride;
            }
        }

        let mut preblend_buffer_size = 0;
        if self.reader.interlaced() {
            // The whole image is decoded untransformed into the preblend buffer, then truncated and
            // transformed later.
            if options.subset.is_some() || self.is_sampling() {
                let Some(size) = self
                    .base_png
                    .encoded_row_bytes
                    .checked_mul(dim(base.encoded_info().height()))
                else {
                    return Result::ErrorInInput;
                };
                preblend_buffer_size = size;
            } else if frame_blend == Blend::SrcOver {
                preblend_buffer_size = span.end - span.start;
            }
        } else if frame_blend == Blend::SrcOver {
            preblend_buffer_size = dst_row_size;
        }
        if preblend_buffer_size > 0 {
            // Skia charges this allocation to the decode budget, which is not ported.
            state.preblend = vec![0; preblend_buffer_size];
        }
        state.set_dst(span);
        Result::Success
    }

    /// Port of `SkPngRustCodec::expandDecodedInterlacedRow`: places an interlaced row at its
    /// Adam7 positions, replicating it over the pixels later passes will fill.
    // Port of: src/codec/SkPngRustCodec.cpp#L817-L866 (expandDecodedInterlacedRow, chrome/m156)
    fn expand_decoded_interlaced_row(
        &mut self,
        base: &CodecBase<'_>,
        dst_frame: &mut [u8],
        src_row: &[u8],
        decoding_dst: DecodingDst,
        xform_needed: bool,
    ) {
        debug_assert!(self.reader.interlaced());
        let mut xformed_interlaced_row = Vec::new();
        let dst_row_stride = decoding_dst.row_stride;
        if xform_needed {
            // Copy the (possibly shorter, for the initial Adam7 passes) `src_row` into a full-width
            // row, because `apply_xform_row` needs full-width rows.
            let mut decoded_full_width_row = vec![0; self.base_png.encoded_row_bytes];
            debug_assert!(decoded_full_width_row.len() >= src_row.len());
            decoded_full_width_row[..src_row.len()].copy_from_slice(src_row);
            xformed_interlaced_row = vec![0; decoding_dst.row_size];
            self.base_png.apply_xform_row(
                base,
                &mut xformed_interlaced_row,
                &decoded_full_width_row,
            );
        }

        let dst_bytes_per_pixel = decoding_dst.bytes_per_pixel;
        debug_assert!(dst_bytes_per_pixel < 32);
        let expanded_row: &[u8] = if xform_needed {
            &xformed_interlaced_row
        } else {
            src_row
        };
        let bits_per_pixel = if xform_needed {
            dst_bytes_per_pixel * 8
        } else {
            base.encoded_info().bits_per_pixel()
        };

        // `splat_last_interlaced_row` infers the rows from the size, so it needs whole rows for
        // the full height. An offset APNG frame can end short of a whole row.
        let whole_frame_size = dst_row_stride * decoding_dst.row_count;
        if dst_frame.len() >= whole_frame_size {
            self.reader.splat_last_interlaced_row(
                &mut dst_frame[..whole_frame_size],
                dst_row_stride,
                expanded_row,
                bits_per_pixel,
            );
        } else {
            self.reader.expand_last_interlaced_row(
                dst_frame,
                dst_row_stride,
                expanded_row,
                bits_per_pixel,
            );
        }
    }

    /// Port of `SkPngRustCodec::incrementalDecodeXForm`: decodes rows that need a transform or a
    /// swizzle, one at a time.
    // Port of: src/codec/SkPngRustCodec.cpp#L868-L1009 (incrementalDecodeXForm, chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn incremental_decode_xform(
        &mut self,
        base: &mut CodecBase<'_>,
        state: &mut DecodingState,
        dst: &mut [u8],
        rows_decoded: &mut i32,
    ) -> Result {
        debug_assert!(!self.can_read_row(base));
        let is_sampling = self.is_sampling();
        let interlaced = self.reader.interlaced();
        let subset = base.options().subset.is_some();
        let mut decoding_dst = state.dst();
        let src_height = state.last_row - state.first_row + 1;
        let sample_y = self
            .base_png
            .swizzler
            .as_ref()
            .map_or(1, Swizzler::sample_y);
        let dst_height = get_sampled_dimension(src_height, sample_y);

        // A non-interlaced partial decode stops once it has written the rows it needs. Interlaced
        // decodes always walk the whole frame, because every Adam7 pass is read before a row is
        // final.
        let partial_decode = subset || is_sampling;
        let stops_before_end_of_frame = partial_decode && !interlaced;
        let frame_index = base.options().frame_index;
        let mut decoded_row = Vec::new();
        loop {
            self.stream_is_positioned_at_start_of_frame_data = false;
            decoded_row.clear();
            if !stops_before_end_of_frame || state.rows_written_to_output < dst_height {
                let result = to_codec_result(self.reader.next_interlaced_row(&mut decoded_row));
                if result != Result::Success {
                    if result == Result::IncompleteInput {
                        *rows_decoded = state.rows_written_to_output;
                    }
                    return result;
                }
            }

            // No more rows, or every row the subset or sample needs has been decoded.
            if decoded_row.is_empty() {
                if interlaced && !state.preblend.is_empty() {
                    if partial_decode {
                        let window = window(dst, &decoding_dst);
                        self.get_subset_or_sample_from_full_image(
                            base,
                            &state.preblend,
                            window,
                            decoding_dst.row_stride,
                            state.y_byte_offset,
                            src_height,
                            dst_height,
                            decoding_dst.row_size,
                        );
                    } else {
                        blend_all_rows(
                            window(dst, &decoding_dst),
                            &state.preblend,
                            decoding_dst.row_size,
                            decoding_dst.row_stride,
                            base.dst_info().color_type(),
                            base.dst_info().alpha_type(),
                        );
                    }
                }
                if !interlaced && !partial_decode {
                    // All of the original destination should be filled out at this point.
                    debug_assert_eq!(decoding_dst.start, decoding_dst.end);
                }
                if !stops_before_end_of_frame {
                    self.frame_holder
                        .mark_frame_as_fully_received(dim(frame_index));
                }
                if !partial_decode && self.is_last_frame(base) {
                    self.finish_stream_after_last_frame();
                }
                return Result::Success;
            }

            if interlaced {
                if state.preblend.is_empty() {
                    self.expand_decoded_interlaced_row(
                        base,
                        window(dst, &decoding_dst),
                        &decoded_row,
                        decoding_dst,
                        true,
                    );
                } else if partial_decode {
                    let encoded_bytes_per_pixel =
                        u8::try_from(base.encoded_info().make_image_info().bytes_per_pixel())
                            .unwrap_or(u8::MAX);
                    let full_image_decoding_dst = DecodingDst {
                        row_stride: self.base_png.encoded_row_bytes,
                        start: 0,
                        end: state.preblend.len(),
                        row_size: self.base_png.encoded_row_bytes,
                        row_count: dim(base.encoded_info().height()),
                        bytes_per_pixel: encoded_bytes_per_pixel,
                    };
                    let mut preblend = std::mem::take(&mut state.preblend);
                    self.expand_decoded_interlaced_row(
                        base,
                        &mut preblend,
                        &decoded_row,
                        full_image_decoding_dst,
                        false,
                    );
                    state.preblend = preblend;
                } else {
                    let mut preblend = std::mem::take(&mut state.preblend);
                    self.expand_decoded_interlaced_row(
                        base,
                        &mut preblend,
                        &decoded_row,
                        decoding_dst,
                        true,
                    );
                    state.preblend = preblend;
                }
                // `rows_written_to_output` is not incremented: full, contiguous rows are not final
                // until the last Adam7 pass.
            } else {
                let y = state.current_source_row - state.first_row;
                state.current_source_row += 1;
                if y < 0 {
                    continue;
                }
                if self
                    .base_png
                    .swizzler
                    .as_ref()
                    .is_some_and(|swizzler| !swizzler.row_needed(y))
                {
                    continue;
                }
                if state.preblend.is_empty() {
                    let target = window(dst, &decoding_dst);
                    self.base_png.apply_xform_row(base, target, &decoded_row);
                } else {
                    // A non-empty preblend buffer means a second or later APNG frame. APNGs have no
                    // sampler (`getSampler` returns none), so sampling does not reach here.
                    debug_assert!(!is_sampling);
                    let preblend = &mut state.preblend;
                    self.base_png.apply_xform_row(base, preblend, &decoded_row);
                    blend_row(
                        window(dst, &decoding_dst),
                        &state.preblend,
                        base.dst_info().color_type(),
                        base.dst_info().alpha_type(),
                    );
                }
                let stride = decoding_dst.row_stride;
                advance(&mut decoding_dst, stride);
                state.set_dst(decoding_dst);
                state.rows_written_to_output += 1;
            }
        }
    }

    /// Port of `SkPngRustCodec::incrementalDecode`: decodes rows straight into `dst`, when
    /// `canReadRow` allows it.
    // Port of: src/codec/SkPngRustCodec.cpp#L1011-L1098 (incrementalDecode, chrome/m156)
    fn incremental_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        state: &mut DecodingState,
        dst: &mut [u8],
        rows_decoded: &mut i32,
    ) -> Result {
        debug_assert!(self.can_read_row(base));
        let interlaced = self.reader.interlaced();
        let frame_index = base.options().frame_index;
        // Interlaced rows are decoded whole, then copied to their Adam7 positions.
        let mut full_width_row = if interlaced {
            vec![0; self.base_png.encoded_row_bytes]
        } else {
            Vec::new()
        };
        let mut decoding_dst = state.dst();
        loop {
            self.stream_is_positioned_at_start_of_frame_data = false;
            let rust_result = if interlaced {
                self.reader.read_row(&mut full_width_row)
            } else if state.preblend.is_empty() {
                self.reader.read_row(window(dst, &decoding_dst))
            } else {
                self.reader.read_row(&mut state.preblend)
            };
            let result = to_codec_result(rust_result);
            if result != Result::Success {
                if result == Result::IncompleteInput {
                    *rows_decoded = state.rows_written_to_output;
                }
                return result;
            }

            // No more rows.
            if rust_result == DecodingResult::EndOfFrame {
                if interlaced && !state.preblend.is_empty() {
                    blend_all_rows(
                        window(dst, &decoding_dst),
                        &state.preblend,
                        decoding_dst.row_size,
                        decoding_dst.row_stride,
                        base.dst_info().color_type(),
                        base.dst_info().alpha_type(),
                    );
                }
                if !interlaced {
                    // All of the original destination should be filled out at this point.
                    debug_assert_eq!(decoding_dst.start, decoding_dst.end);
                }
                self.frame_holder
                    .mark_frame_as_fully_received(dim(frame_index));
                if self.is_last_frame(base) {
                    // No partial-decode check here: `canReadRow` rules out subsets and sampling.
                    self.finish_stream_after_last_frame();
                }
                return Result::Success;
            }

            // Expand interlaced rows, or blend into the previous frame if needed.
            if interlaced {
                if state.preblend.is_empty() {
                    self.expand_decoded_interlaced_row(
                        base,
                        window(dst, &decoding_dst),
                        &full_width_row,
                        decoding_dst,
                        false,
                    );
                } else {
                    let mut preblend = std::mem::take(&mut state.preblend);
                    self.expand_decoded_interlaced_row(
                        base,
                        &mut preblend,
                        &full_width_row,
                        decoding_dst,
                        false,
                    );
                    state.preblend = preblend;
                }
            } else {
                if !state.preblend.is_empty() {
                    blend_row(
                        window(dst, &decoding_dst),
                        &state.preblend,
                        base.dst_info().color_type(),
                        base.dst_info().alpha_type(),
                    );
                }
                // Move on to the next row of the destination.
                let stride = decoding_dst.row_stride;
                advance(&mut decoding_dst, stride);
                state.set_dst(decoding_dst);
                state.rows_written_to_output += 1;
            }
        }
    }

    /// Port of `SkPngRustCodec::onGetPixels`'s body: validates, initializes, and decodes.
    // Port of: src/codec/SkPngRustCodec.cpp#L1100-L1118 (onGetPixels, chrome/m156)
    fn decode_pixels(
        &mut self,
        base: &mut CodecBase<'_>,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        rows_decoded: &mut i32,
    ) -> Result {
        let mut decoding_state = None;
        let result = self.start_decoding(base, row_bytes, options, &mut decoding_state);
        if result != Result::Success {
            return result;
        }
        let Some(mut state) = decoding_state else {
            return Result::InternalError;
        };
        let result = self.initialize_decoding_dst(base, &mut state, dst.len());
        if result != Result::Success {
            return result;
        }
        if self.can_read_row(base) {
            self.incremental_decode(base, &mut state, dst, rows_decoded)
        } else {
            self.incremental_decode_xform(base, &mut state, dst, rows_decoded)
        }
    }

    /// Port of `SkPngRustCodec::getSampler`: sampling is only offered for static full-canvas
    /// images, and a sampler made for a request that is then not satisfied is dropped.
    // Port of: src/codec/SkPngRustCodec.cpp#L1505-L1514 (getSampler, chrome/m156)
    fn get_sampler_inner(
        &mut self,
        base: &CodecBase<'_>,
        create_if_necessary: bool,
    ) -> Option<&mut dyn Sampler> {
        if !self.supports_sampling_or_subsetting(base) {
            if create_if_necessary {
                // Clean up tentative decoding state set up by startIncrementalDecode.
                self.incremental = None;
            }
            return None;
        }
        // Port of `SkPngCodecBase::getSampler`.
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
}

impl CodecImpl for PngRustCodec<'_> {
    // Port of: src/codec/SkPngRustCodec.cpp (SkPngCodecBase::onGetEncodedFormat), chrome/m156
    fn on_get_encoded_format(&self) -> EncodedImageFormat {
        EncodedImageFormat::PNG
    }

    // Port of: src/codec/SkPngRustCodec.cpp#L1100-L1118 (onGetPixels, chrome/m156)
    fn on_get_pixels(
        &mut self,
        base: &mut CodecBase<'_>,
        _info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        rows_decoded: &mut i32,
    ) -> Result {
        self.decode_pixels(base, dst, row_bytes, options, rows_decoded)
    }

    // Port of: src/codec/SkPngRustCodec.cpp#L1120-L1124 (onSupportsIncrementalDecode, chrome/m156)
    fn on_supports_incremental_decode(&self, _dst: &ImageInfo) -> bool {
        true
    }

    // Port of: src/codec/SkPngRustCodec.cpp#L1126-L1134 (onStartIncrementalDecode, chrome/m156)
    fn on_start_incremental_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        _dst_info: &ImageInfo,
        _dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
    ) -> Result {
        // Any state of another, partially decoded frame is clobbered.
        self.incremental = None;
        let mut state = None;
        let result = self.start_decoding(base, row_bytes, options, &mut state);
        self.incremental = state;
        result
    }

    // Port of: src/codec/SkPngRustCodec.cpp#L1136-L1152 (onIncrementalDecode, chrome/m156)
    fn on_incremental_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        dst: &mut [u8],
        rows_decoded: &mut i32,
    ) -> Result {
        let Some(mut state) = self.incremental.take() else {
            return Result::InvalidParameters;
        };
        let result = self.initialize_decoding_dst(base, &mut state, dst.len());
        if result != Result::Success {
            return result;
        }
        let result = if self.can_read_row(base) {
            self.incremental_decode(base, &mut state, dst, rows_decoded)
        } else {
            self.incremental_decode_xform(base, &mut state, dst, rows_decoded)
        };
        // Only `IncompleteInput` leaves the state usable: after success or a fatal error the
        // client does not call again.
        if result == Result::IncompleteInput {
            self.incremental = Some(state);
        }
        result
    }

    // Port of: src/codec/SkPngRustCodec.cpp#L1154-L1192 (onGetFrameCount, chrome/m156)
    fn on_get_frame_count(&mut self) -> i32 {
        'parse: {
            if !self.can_parse_additional_frame_infos || self.incremental.is_some() {
                break 'parse;
            }
            if self.priv_stream_has_length() {
                let current_length = self.priv_stream_length();
                if let Some(old_length) = self.max_stream_length_seen {
                    // `>=` rather than `==`, because the underlying stream can be cleared.
                    if old_length >= current_length {
                        // Don't retry if the input did not change (or is smaller than last time).
                        break 'parse;
                    }
                }
                self.max_stream_length_seen = Some(current_length);
            }
            match self.parse_additional_frame_infos() {
                Result::IncompleteInput => self.can_parse_additional_frame_infos = true,
                Result::Success => {
                    debug_assert_eq!(self.frame_holder.size(), self.raw_frame_count());
                    self.can_parse_additional_frame_infos = false;
                }
                _ => self.can_parse_additional_frame_infos = false,
            }
        }
        self.frame_holder.size()
    }

    // Port of: src/codec/SkPngRustCodec.cpp#L1194-L1196 (onGetFrameInfo, chrome/m156)
    fn on_get_frame_info(&self, index: i32, info: Option<&mut FrameInfo>) -> bool {
        self.frame_holder.get_frame_info(index, info)
    }

    // Port of: src/codec/SkPngRustCodec.cpp#L1198-L1214 (onGetRepetitionCount, chrome/m156)
    fn on_get_repetition_count(&mut self) -> i32 {
        if !self.reader.has_actl_chunk() {
            return 0;
        }
        let num_frames = self.reader.get_actl_num_frames();
        if num_frames <= 1 {
            return 0;
        }
        // APNG: "`num_plays` indicates the number of times that this animation should play; if it
        // is 0, the animation should play indefinitely."
        let Ok(num_plays) = i32::try_from(self.reader.get_actl_num_plays()) else {
            return i32::MAX;
        };
        if num_plays == 0 {
            return REPETITION_COUNT_INFINITE;
        }
        // `SkCodec::onGetRepetitionCount` does not count the first play through of each frame.
        num_plays - 1
    }

    // Port of: src/codec/SkPngRustCodec.cpp#L1216-L1222 (onIsAnimated, chrome/m156)
    fn on_is_animated(&mut self) -> IsAnimated {
        self.is_animated_value()
    }

    // Port of: src/codec/SkPngRustCodec.cpp (getFrameHolder), chrome/m156
    fn frame_holder(&self) -> Option<&dyn FrameHolder> {
        Some(&self.frame_holder)
    }

    // Port of: src/codec/SkPngRustCodec.cpp#L1483-L1494 (getEncodedData, chrome/m156)
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
        let mut bytes = vec![0; duplicate.get_length()];
        let read = duplicate.read(&mut bytes);
        bytes.truncate(read);
        Some(Data::new_copy(&bytes))
    }

    // Port of: src/codec/SkPngRustCodec.cpp#L1505-L1514 (getSampler, chrome/m156)
    fn on_get_sampler(
        &mut self,
        base: &CodecBase<'_>,
        create_if_necessary: bool,
    ) -> Option<&mut dyn Sampler> {
        self.get_sampler_inner(base, create_if_necessary)
    }
}

impl PngRustCodec<'_> {
    fn priv_stream_has_length(&self) -> bool {
        self.priv_stream
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .has_length()
    }

    fn priv_stream_length(&self) -> usize {
        self.priv_stream
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_length()
    }
}

/// Port of `SkPngRustCodec::MakeFromStream`: reads the header through the `png` crate and makes a
/// codec over the stream. `chunk_reader`, when given, receives the chunks the file has beyond the
/// image.
///
/// # Errors
/// The `Result` a failed header read or an unsupported image gives.
// Port of: src/codec/SkPngRustCodec.cpp#L1227-L1256 (MakeFromStream, chrome/m156)
#[doc(alias = "SkPngRustCodec::MakeFromStream")]
pub fn make_from_stream_with_chunk_reader<'a>(
    stream: Box<dyn Stream + Send + 'a>,
    chunk_reader: Option<Box<dyn PngChunkReader>>,
) -> std::result::Result<Codec<'a>, Result> {
    let chunk_reader = Arc::new(Mutex::new(PngCompositeChunkReader::new(chunk_reader)));
    let priv_stream = Arc::new(Mutex::new(stream));
    let reader = Reader::new(Arc::clone(&priv_stream))
        .map_err(|e| to_codec_result(DecodingResult::from(Some(&e))))?;
    let Some(encoded) = create_encoded_info(&reader) else {
        return Err(Result::ErrorInInput);
    };
    if !is_valid_fctl_if_any(&encoded, &reader) {
        return Err(Result::ErrorInInput);
    }
    read_unknown_chunks(&reader, &chunk_reader);

    let origin = get_encoded_origin(&reader);
    let src_format = to_pixel_format(&encoded);
    let idat_is_not_part_of_animation = reader.has_actl_chunk() && !reader.has_fctl_chunk();
    let mut frame_holder = PngFrames::new(encoded.width(), encoded.height());
    if !idat_is_not_part_of_animation {
        // Width and height are capped above, and without `fcTL` there is no frame metadata to
        // check, so this append succeeds.
        let result = frame_holder.append_new_frame(&reader, &encoded);
        debug_assert_eq!(result, Result::Success);
    }
    let codec = PngRustCodec {
        base_png: PngCodecBase::new(chunk_reader),
        reader,
        priv_stream,
        encoded: encoded.clone(),
        frame_at_current_stream_position: if idat_is_not_part_of_animation { -1 } else { 0 },
        stream_is_positioned_at_start_of_frame_data: true,
        max_stream_length_seen: None,
        incremental: None,
        frame_holder,
        can_parse_additional_frame_infos: true,
    };
    // The base codec holds no stream: this codec reads its own, as Skia's does.
    Ok(Codec::new(
        encoded,
        Box::new(codec),
        None,
        origin,
        Some(src_format),
    ))
}

/// Port of `SkPngRustDecoder::Decode` without a chunk reader: the registry's entry point.
///
/// # Errors
/// As [`make_from_stream_with_chunk_reader`].
// Port of: src/codec/SkPngRustDecoder.cpp#L16-L28 (Decode, chrome/m156)
#[doc(alias = "SkPngRustDecoder::Decode")]
pub fn make_from_stream<'a>(
    stream: Box<dyn Stream + Send + 'a>,
) -> std::result::Result<Codec<'a>, Result> {
    make_from_stream_with_chunk_reader(stream, None)
}

/// Port of `SkPngRustDecoder::IsPng`.
// Port of: src/codec/SkPngRustDecoder.cpp#L10 (IsPng, chrome/m156)
#[doc(alias = "SkPngRustDecoder::IsPng")]
#[must_use]
pub fn is_png_format(buf: &[u8]) -> bool {
    crate::png_codec_base::is_png(buf)
}
