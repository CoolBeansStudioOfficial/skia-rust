// Copyright 2026 The skia-rust Authors.
// Copyright 2018 Google LLC (SkWuffsCodec.cpp, which this file ports).
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkWuffsCodec.cpp (chrome/m156), the GIF codec (SkWuffsCodec, SkWuffsFrame,
// SkWuffsFrameHolder and SkGifDecoder). The Wuffs decoder it drives is skia-rust-wuffs, which is
// Apache-2.0 (see that crate's LICENSE).

//! Skia's GIF codec: the `SkWuffsCodec` that drives the Wuffs GIF decoder, with its animation
//! frames, its one-pass and two-pass incremental decoding, and the `SkGifDecoder` entry points.
//!
//! Skia's `SkCodecPrintf` diagnostics are not printed: only the codec results are returned.

// The integer conversions below mirror the C++ (`size_t`, `uint64_t` and `int` in the same
// expressions), so the cast lints are allowed for this module.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
// `cast_precision_loss`: SkRect::Make takes scalars (f32), so the small image sizes are converted
// to f32 as the C++ does.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::Vector;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::{MemoryStream, Stream};
use skia_rust_core::stream_priv::copy_stream_to_data;
use skia_rust_raster::draw::Draw;
use skia_rust_raster::raster_clip::RasterClip;
use skia_rust_skcms::PixelFormat as SkcmsPixelFormat;
use skia_rust_wuffs::base::{
    AnimationDisposal, FLICKS_PER_MILLISECOND, FrameConfig, ImageConfig, IoBuffer, IoMeta,
    PIXEL_FORMAT_BGR_565, PIXEL_FORMAT_BGRA_NONPREMUL, PIXEL_FORMAT_INVALID,
    PIXEL_FORMAT_RGBA_NONPREMUL, PixelBlend, PixelBuffer, PixelConfig, PixelFormat, RectIeU32,
    Status, Table,
};
use skia_rust_wuffs::gif::{GifDecoder, QUIRK_IGNORE_TOO_MUCH_PIXEL_DATA};
use skia_rust_wuffs::strings::{NOTE_END_OF_DATA, SUSPENSION_SHORT_READ};

use crate::codec::{
    Codec, CodecBase, CodecImpl, FrameInfo, IsAnimated, NO_FRAME, Options,
    REPETITION_COUNT_INFINITE, Result as CodecResult, SelectionPolicy,
};
use crate::codec_animation::{Blend, DisposalMethod};
use crate::encoded_info::{Alpha, Color, EncodedInfo};
use crate::frame_holder::{Frame, FrameHolder, set_alpha_and_required_frame};
use crate::sampler;

/// Port of `SK_WUFFS_CODEC_BUFFER_SIZE`: the size of the I/O buffer the codec reads through.
const SK_WUFFS_CODEC_BUFFER_SIZE: usize = 4096;

/// Port of `WUFFS_BASE__PIXEL_SUBSAMPLING__NONE`: GIF has no chroma subsampling.
const PIXEL_SUBSAMPLING_NONE: u32 = 0;

/// Port of `fill_buffer`: compacts the I/O buffer, and reads more of the stream into it. Returns
/// whether any bytes were read.
// Port of: src/codec/SkWuffsCodec.cpp#L60-L84 (chrome/m156)
fn fill_buffer(b: &mut IoBuffer, s: &mut dyn Stream) -> bool {
    b.compact();
    let wi = b.meta.wi;
    let num_read = s.read(&mut b.data[wi..]);
    b.meta.wi += num_read;
    // Wuffs' "closed" flag is hard-coded to false. See the C++ comments for the reasoning: an
    // incomplete input is a short read (a suspension), not an error.
    b.meta.closed = false;
    num_read > 0
}

/// Port of `seek_buffer`: positions the I/O buffer's read index at `pos`, re-using the buffered
/// bytes when `pos` is among them, and otherwise seeking the stream.
// Port of: src/codec/SkWuffsCodec.cpp#L86-L104 (chrome/m156)
fn seek_buffer(b: &mut IoBuffer, s: &mut dyn Stream, pos: u64) -> bool {
    if (pos >= b.meta.pos) && (pos - b.meta.pos <= b.meta.wi as u64) {
        b.meta.ri = (pos - b.meta.pos) as usize;
        return true;
    }
    if (pos > usize::MAX as u64) || !s.seek(pos as usize) {
        return false;
    }
    b.meta.wi = 0;
    b.meta.ri = 0;
    b.meta.pos = pos;
    b.meta.closed = false;
    true
}

/// Port of `wuffs_disposal_to_skia_disposal`.
// Port of: src/codec/SkWuffsCodec.cpp#L106-L118 (chrome/m156)
fn wuffs_disposal_to_skia_disposal(w: AnimationDisposal) -> DisposalMethod {
    match w {
        AnimationDisposal::RestoreBackground => DisposalMethod::RestoreBgColor,
        AnimationDisposal::RestorePrevious => DisposalMethod::RestorePrevious,
        AnimationDisposal::None => DisposalMethod::Keep,
    }
}

/// Port of `to_alpha_type`.
// Port of: src/codec/SkWuffsCodec.cpp#L120-L122 (chrome/m156)
fn to_alpha_type(opaque: bool) -> AlphaType {
    if opaque {
        AlphaType::Opaque
    } else {
        AlphaType::Premul
    }
}

/// Port of `reset_and_decode_image_config`: (re)initializes the decoder and decodes the image
/// config, refilling the I/O buffer from the stream as needed. With `imgcfg`, the pixel format is
/// set to the N32 format Skia decodes to (4 bytes per pixel).
// Port of: src/codec/SkWuffsCodec.cpp#L124-L178 (chrome/m156)
fn reset_and_decode_image_config(
    decoder: &mut GifDecoder,
    imgcfg: Option<&mut ImageConfig>,
    b: &mut IoBuffer,
    s: &mut dyn Stream,
) -> CodecResult {
    // Calling decoder->initialize resets it to a fresh state.
    *decoder = GifDecoder::new();

    // See https://bugs.chromium.org/p/skia/issues/detail?id=12055
    decoder.set_quirk_enabled(QUIRK_IGNORE_TOO_MUCH_PIXEL_DATA, true);

    let mut imgcfg = imgcfg;
    loop {
        let status = decoder.decode_image_config(imgcfg.as_deref_mut(), b);
        match status.repr() {
            None => break,
            Some(SUSPENSION_SHORT_READ) => {
                if !fill_buffer(b, s) {
                    return CodecResult::IncompleteInput;
                }
            }
            Some(_) => return CodecResult::ErrorInInput,
        }
    }

    // A GIF image's natural colour model is indexed colour. Skia overrides that to decode to
    // four bytes per pixel, BGRA or RGBA, matching its N32 colour type.
    let pixfmt = match ColorType::N32 {
        ColorType::BGRA8888 => PIXEL_FORMAT_BGRA_NONPREMUL,
        ColorType::RGBA8888 => PIXEL_FORMAT_RGBA_NONPREMUL,
        _ => return CodecResult::InternalError,
    };
    if let Some(cfg) = imgcfg {
        let (width, height) = (cfg.pixcfg.width, cfg.pixcfg.height);
        cfg.pixcfg
            .set(pixfmt.0, PIXEL_SUBSAMPLING_NONE, width, height);
    }

    CodecResult::Success
}

/// Port of `SkWuffsFrame`'s constructor: the frame described by a Wuffs frame config.
// Port of: src/codec/SkWuffsCodec.cpp#L184-L199 (chrome/m156), SkWuffsFrame::SkWuffsFrame
fn frame_from_config(fc: &FrameConfig) -> Frame {
    let reported_alpha = if fc.opaque_within_bounds {
        Alpha::Opaque
    } else {
        Alpha::Unpremul
    };
    let mut frame = Frame::new(fc.index as i32, reported_alpha);
    let r: RectIeU32 = fc.bounds;
    frame.set_xywh(
        r.min_incl_x as i32,
        r.min_incl_y as i32,
        r.width() as i32,
        r.height() as i32,
    );
    frame.set_disposal_method(wuffs_disposal_to_skia_disposal(fc.disposal));
    frame.set_duration(((fc.duration as u64) / FLICKS_PER_MILLISECOND) as i32);
    frame.set_blend(if fc.overwrite_instead_of_blend {
        Blend::Src
    } else {
        Blend::SrcOver
    });
    frame
}

/// Port of `SkWuffsCodec`: a GIF codec over the Wuffs GIF decoder. The codec owns its stream
/// (`fPrivStream`), so the base `SkCodec` is given no stream.
///
/// Its frames are kept as [`Frame`]s, with the I/O position each frame's data starts at, which
/// `SkWuffsFrame::ioPosition` reports.
// Port of: src/codec/SkWuffsCodec.cpp#L201-L330 (chrome/m156)
#[doc(alias = "SkWuffsCodec")]
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors the separate decode-state flags of SkWuffsCodec"
)]
pub struct WuffsCodec<'a> {
    priv_stream: Box<dyn Stream + Send + 'a>,
    frames: Vec<Frame>,
    frame_io_positions: Vec<u64>,
    screen_width: i32,
    screen_height: i32,
    decoder: Box<GifDecoder>,
    first_frame_io_position: u64,
    frame_config: FrameConfig,
    pixel_config: PixelConfig,
    io_buffer: IoBuffer,

    // Incremental decoding state. `fIncrDecDst` is not kept: the destination is passed to each
    // call, so only whether a decode is in progress is recorded.
    incr_dec_active: bool,
    incr_dec_row_bytes: usize,
    incr_dec_pixel_blend: PixelBlend,
    incr_dec_one_pass: bool,
    // The pixel config of the one-pass destination (its size and format), set at the start.
    incr_dec_one_pass_config: PixelConfig,
    first_call_to_incremental_decode: bool,

    // The lazily allocated intermediate pixel buffer, for two-pass decoding.
    two_pass_pixbuf: Option<Vec<u8>>,

    num_fully_received_frames: u64,
    frames_complete: bool,

    // If a Wuffs call returns an incomplete status, the decoder is suspended in a coroutine. Only
    // resuming it, or resetting it, is safe, so the next seek resets it when this is set.
    decoder_is_suspended: bool,

    can_seek: bool,
}

impl std::fmt::Debug for WuffsCodec<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WuffsCodec")
            .field("frames", &self.frames.len())
            .field("can_seek", &self.can_seek)
            .finish_non_exhaustive()
    }
}

impl<'a> WuffsCodec<'a> {
    /// Port of the `SkWuffsCodec` constructor.
    // Port of: src/codec/SkWuffsCodec.cpp#L339-L368 (chrome/m156)
    fn new(
        stream: Box<dyn Stream + Send + 'a>,
        can_seek: bool,
        decoder: Box<GifDecoder>,
        imgcfg: ImageConfig,
        iobuf: IoBuffer,
    ) -> Self {
        Self {
            priv_stream: stream,
            frames: Vec::new(),
            frame_io_positions: Vec::new(),
            screen_width: imgcfg.pixcfg.width as i32,
            screen_height: imgcfg.pixcfg.height as i32,
            decoder,
            first_frame_io_position: imgcfg.first_frame_io_position,
            frame_config: FrameConfig::default(),
            pixel_config: imgcfg.pixcfg,
            io_buffer: iobuf,
            incr_dec_active: false,
            incr_dec_row_bytes: 0,
            incr_dec_pixel_blend: PixelBlend::Src,
            incr_dec_one_pass: false,
            incr_dec_one_pass_config: PixelConfig::default(),
            first_call_to_incremental_decode: false,
            two_pass_pixbuf: None,
            num_fully_received_frames: 0,
            frames_complete: false,
            decoder_is_suspended: false,
            can_seek,
        }
    }

    /// Port of `SkWuffsCodec::frame`: the frame with index `i`, if there is one.
    // Port of: src/codec/SkWuffsCodec.cpp#L434-L440 (chrome/m156)
    fn frame(&self, i: i32) -> Option<&Frame> {
        if i >= 0 {
            self.frames.get(i as usize)
        } else {
            None
        }
    }

    /// Port of `SkWuffsCodec::seekFrame`: restarts the decoder at the start of frame
    /// `frame_index`.
    // Port of: src/codec/SkWuffsCodec.cpp#L884-L913 (chrome/m156)
    fn seek_frame(&mut self, frame_index: i32) -> CodecResult {
        if self.decoder_is_suspended {
            let res = self.reset_decoder();
            if res != CodecResult::Success {
                return res;
            }
        }

        let pos = match frame_index {
            i if i < 0 => return CodecResult::InternalError,
            0 => self.first_frame_io_position,
            i => match self.frame_io_positions.get(i as usize) {
                Some(pos) => *pos,
                None => return CodecResult::InternalError,
            },
        };

        if !seek_buffer(&mut self.io_buffer, &mut *self.priv_stream, pos) {
            return CodecResult::InternalError;
        }
        let status = self
            .decoder
            .restart_frame(frame_index as u64, self.io_buffer.reader_io_position());
        if status.repr().is_some() {
            return CodecResult::InternalError;
        }
        CodecResult::Success
    }

    /// Port of `SkWuffsCodec::resetDecoder`: rewinds the stream and decodes the image config
    /// again, which leaves the decoder ready to restart at any frame.
    // Port of: src/codec/SkWuffsCodec.cpp#L915-L931 (chrome/m156)
    fn reset_decoder(&mut self) -> CodecResult {
        if !self.priv_stream.rewind() {
            return CodecResult::InternalError;
        }
        self.io_buffer.meta = IoMeta::default();

        let result = reset_and_decode_image_config(
            &mut self.decoder,
            None,
            &mut self.io_buffer,
            &mut *self.priv_stream,
        );
        if result == CodecResult::IncompleteInput {
            return CodecResult::InternalError;
        } else if result != CodecResult::Success {
            return result;
        }

        self.decoder_is_suspended = false;
        CodecResult::Success
    }

    /// Port of `SkWuffsCodec::decodeFrameConfig`: decodes the next frame config into
    /// `frame_config`, refilling the I/O buffer on short reads.
    // Port of: src/codec/SkWuffsCodec.cpp#L933-L946 (chrome/m156)
    fn decode_frame_config(&mut self) -> Status {
        loop {
            let status = self
                .decoder
                .decode_frame_config(Some(&mut self.frame_config), &mut self.io_buffer);
            if status.repr() == Some(SUSPENSION_SHORT_READ)
                && fill_buffer(&mut self.io_buffer, &mut *self.priv_stream)
            {
                continue;
            }
            self.decoder_is_suspended = !status.is_complete();
            self.update_num_fully_received_frames();
            return status;
        }
    }

    /// Port of `SkWuffsCodec::decodeFrame`: decodes the current frame into `pixels`, blending
    /// with the current pixel blend, refilling the I/O buffer on short reads.
    // Port of: src/codec/SkWuffsCodec.cpp#L948-L962 (chrome/m156)
    fn decode_frame(&mut self, pixels: &mut PixelBuffer<'_>) -> Status {
        loop {
            let status =
                self.decoder
                    .decode_frame(pixels, &mut self.io_buffer, self.incr_dec_pixel_blend);
            if status.repr() == Some(SUSPENSION_SHORT_READ)
                && fill_buffer(&mut self.io_buffer, &mut *self.priv_stream)
            {
                continue;
            }
            self.decoder_is_suspended = !status.is_complete();
            self.update_num_fully_received_frames();
            return status;
        }
    }

    /// Port of `SkWuffsCodec::updateNumFullyReceivedFrames`: keeps the largest number of decoded
    /// frames seen, since the count can go down when the stream is seeked back.
    // Port of: src/codec/SkWuffsCodec.cpp#L964-L973 (chrome/m156)
    fn update_num_fully_received_frames(&mut self) {
        let n = self.decoder.num_decoded_frames();
        if self.num_fully_received_frames < n {
            self.num_fully_received_frames = n;
        }
    }

    /// Port of `SkWuffsCodec::onGetFrameCountInternal`: reads the frame configs that are not yet
    /// known, and records each frame.
    // Port of: src/codec/SkWuffsCodec.cpp#L846-L873 (chrome/m156)
    fn on_get_frame_count_internal(&mut self) {
        let n = self.frames.len();
        let start = if n > 0 { n - 1 } else { 0 };
        if self.seek_frame(start as i32) != CodecResult::Success {
            return;
        }

        let mut i = start;
        while i < i32::MAX as usize {
            match self.decode_frame_config().repr() {
                None => {}
                Some(NOTE_END_OF_DATA) => break,
                Some(_) => return,
            }
            if i >= self.frames.len() {
                let mut frame = frame_from_config(&self.frame_config);
                set_alpha_and_required_frame(
                    self.screen_width,
                    self.screen_height,
                    &self.frames,
                    &mut frame,
                );
                self.frames.push(frame);
                self.frame_io_positions.push(self.frame_config.io_position);
            }
            i += 1;
        }

        self.frames_complete = true;
    }

    /// Port of `SkWuffsCodec::onStartIncrementalDecodeOnePass`: validates the destination as a
    /// one-pass target, and chooses the blend for the frame.
    // Port of: src/codec/SkWuffsCodec.cpp#L570-L613 (chrome/m156)
    fn start_one_pass(
        &mut self,
        dst_info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        pixfmt: PixelFormat,
        bytes_per_pixel: usize,
    ) -> CodecResult {
        let mut pixel_config = PixelConfig::default();
        pixel_config.set(
            pixfmt.0,
            PIXEL_SUBSAMPLING_NONE,
            dst_info.width() as u32,
            dst_info.height() as u32,
        );
        {
            let table = Table {
                data: &mut *dst,
                width: dst_info.width() as usize * bytes_per_pixel,
                height: dst_info.height() as usize,
                stride: row_bytes,
            };
            let mut pixel_buffer = PixelBuffer::null();
            if pixel_buffer
                .set_from_table(&pixel_config, table)
                .repr()
                .is_some()
            {
                return CodecResult::InternalError;
            }
        }
        self.incr_dec_one_pass_config = pixel_config;

        // SRC is usually faster than SRC_OVER, but for a dependent frame, dst is assumed to hold
        // the previous frame's pixels (after processing the DisposalMethod). For one-pass
        // decoding, we therefore use SRC_OVER.
        let depends_on_prior_frame = options.frame_index != 0
            && self
                .frame(options.frame_index)
                .is_some_and(|f| f.required_frame() != NO_FRAME);
        if depends_on_prior_frame {
            self.incr_dec_pixel_blend = PixelBlend::SrcOver;
        } else {
            sampler::fill(dst_info, dst, row_bytes, options.zero_initialized);
            self.incr_dec_pixel_blend = PixelBlend::Src;
        }

        CodecResult::Success
    }

    /// Port of `SkWuffsCodec::onStartIncrementalDecodeTwoPass`: (re)allocates the intermediate
    /// pixel buffer, and zeroes the frame's rectangle in it when the buffer is re-used.
    // Port of: src/codec/SkWuffsCodec.cpp#L615-L688 (chrome/m156)
    fn start_two_pass(&mut self) -> CodecResult {
        // Either re-use the previously allocated "two pass" pixel buffer (and zero it), or
        // allocate (and zero initialize) a new one.
        let mut already_zeroed = false;
        if self.two_pass_pixbuf.is_none() {
            let Ok(pixbuf_len) = usize::try_from(self.pixel_config.pixbuf_len()) else {
                return CodecResult::InternalError;
            };
            self.two_pass_pixbuf = Some(vec![0; pixbuf_len]);
            already_zeroed = true;
        }

        let Some(buf) = self.two_pass_pixbuf.as_mut() else {
            return CodecResult::InternalError;
        };
        let mut pixel_buffer = PixelBuffer::null();
        if pixel_buffer
            .set_from_slice(&self.pixel_config, buf)
            .repr()
            .is_some()
        {
            return CodecResult::InternalError;
        }

        if !already_zeroed {
            let src_bits_per_pixel = pixel_buffer.pixel_format().bits_per_pixel();
            if src_bits_per_pixel == 0 || !src_bits_per_pixel.is_multiple_of(8) {
                return CodecResult::InternalError;
            }
            let src_bytes_per_pixel = (src_bits_per_pixel / 8) as usize;

            let frame_rect = self.frame_config.bounds;
            let Some(pixels) = pixel_buffer.plane0_mut() else {
                return CodecResult::InternalError;
            };
            let pixels_w = pixels.width / src_bytes_per_pixel;
            let pixels_h = pixels.height;
            assert!(frame_rect.min_incl_x as usize <= pixels_w);
            assert!(frame_rect.min_incl_y as usize <= pixels_h);
            assert!(frame_rect.max_excl_x as usize <= pixels_w);
            assert!(frame_rect.max_excl_y as usize <= pixels_h);

            // The C++ zeroes the frame's rows with one call when they are contiguous, and row by
            // row otherwise. Zeroing row by row covers the same bytes.
            let len = frame_rect.width() as usize * src_bytes_per_pixel;
            for y in frame_rect.min_incl_y as usize..frame_rect.max_excl_y as usize {
                let start =
                    y * pixels.stride + frame_rect.min_incl_x as usize * src_bytes_per_pixel;
                pixels.data[start..start + len].fill(0);
            }
        }

        self.incr_dec_pixel_blend = PixelBlend::Src;
        CodecResult::Success
    }

    /// Port of `SkWuffsCodec::onIncrementalDecodeOnePass`: decodes into the destination.
    // Port of: src/codec/SkWuffsCodec.cpp#L639-L652 (chrome/m156)
    fn incremental_one_pass(&mut self, dst: &mut [u8]) -> CodecResult {
        let config = self.incr_dec_one_pass_config;
        let bytes_per_pixel = (config.pixfmt.bits_per_pixel() / 8) as usize;
        let table = Table {
            data: dst,
            width: config.width as usize * bytes_per_pixel,
            height: config.height as usize,
            stride: self.incr_dec_row_bytes,
        };
        let mut pixel_buffer = PixelBuffer::null();
        if pixel_buffer.set_from_table(&config, table).repr().is_some() {
            return CodecResult::InternalError;
        }
        decode_status_to_result(self.decode_frame(&mut pixel_buffer))
    }

    /// Port of `SkWuffsCodec::onIncrementalDecodeTwoPass`: decodes into the intermediate buffer,
    /// then composites the frame's dirty rectangle onto the destination.
    // Port of: src/codec/SkWuffsCodec.cpp#L654-L786 (chrome/m156)
    #[allow(
        clippy::too_many_lines,
        reason = "one function, as onIncrementalDecodeTwoPass is in the C++"
    )]
    fn incremental_two_pass(&mut self, base: &CodecBase<'_>, dst: &mut [u8]) -> CodecResult {
        let status = self.decode_two_pass_buffer();
        let index = base.options().frame_index;
        let (independent, alpha_type) = if index == 0 {
            (true, to_alpha_type(base.encoded_info().opaque()))
        } else {
            match self.frame(index) {
                Some(f) => (
                    f.required_frame() == NO_FRAME,
                    to_alpha_type(f.reported_alpha() == Alpha::Opaque),
                ),
                None => return CodecResult::InternalError,
            }
        };

        let mut result = CodecResult::Success;
        if let Some(message) = status.repr() {
            result = if message == SUSPENSION_SHORT_READ {
                CodecResult::IncompleteInput
            } else {
                CodecResult::ErrorInInput
            };
            if !independent {
                // For a dependent frame, the partial result cannot be blended, since that would
                // overwrite the contribution from prior frames.
                return result;
            }
        }

        let src_bits_per_pixel = self.pixel_config.pixfmt.bits_per_pixel();
        if src_bits_per_pixel == 0 || !src_bits_per_pixel.is_multiple_of(8) {
            return CodecResult::InternalError;
        }
        let src_bytes_per_pixel = (src_bits_per_pixel / 8) as usize;

        let frame_rect = self.frame_config.bounds;
        if self.first_call_to_incremental_decode {
            if frame_rect.width() as usize > usize::MAX / src_bytes_per_pixel {
                return CodecResult::InternalError;
            }

            let bounds = IRect::from_ltrb(
                frame_rect.min_incl_x as i32,
                frame_rect.min_incl_y as i32,
                frame_rect.max_excl_x as i32,
                frame_rect.max_excl_y as i32,
            );

            // If the frame rect does not fill the output, ensure that those pixels are not left
            // uninitialized.
            if independent
                && (bounds != IRect::from_wh(base.dimensions().width, base.dimensions().height)
                    || result != CodecResult::Success)
            {
                sampler::fill(
                    base.dst_info(),
                    dst,
                    self.incr_dec_row_bytes,
                    base.options().zero_initialized,
                );
            }
            self.first_call_to_incremental_decode = false;
        } else {
            // Existing clients only show frames beyond the first once they are complete, so a
            // later call for the same frame is only made for the first frame.
            debug_assert_eq!(index, 0);
        }

        // If the frame's dirty rect is empty, there is nothing to swizzle.
        let dirty_rect = self.decoder.frame_dirty_rect();
        if !dirty_rect.is_empty() {
            let (pixels_w_bytes, pixels_h, stride) = self.two_pass_table_dims();
            let pixels_w = pixels_w_bytes / src_bytes_per_pixel;
            assert!(dirty_rect.min_incl_x as usize <= pixels_w);
            assert!(dirty_rect.min_incl_y as usize <= pixels_h);
            assert!(dirty_rect.max_excl_x as usize <= pixels_w);
            assert!(dirty_rect.max_excl_y as usize <= pixels_h);

            // The Wuffs model is that the dst buffer is the image, not the frame. Copy the dirty
            // rectangle out of the image-sized buffer, as a tightly packed bitmap.
            let row_len = dirty_rect.width() as usize * src_bytes_per_pixel;
            let mut packed = Vec::with_capacity(row_len * dirty_rect.height() as usize);
            if let Some(buf) = self.two_pass_pixbuf.as_ref() {
                for y in dirty_rect.min_incl_y as usize..dirty_rect.max_excl_y as usize {
                    let start = y * stride + dirty_rect.min_incl_x as usize * src_bytes_per_pixel;
                    packed.extend_from_slice(&buf[start..start + row_len]);
                }
            }

            // Currently, this is only used for GIF, which never has an ICC profile.
            debug_assert!(base.encoded_info().profile().is_none());
            let src_info = base
                .encoded_info()
                .make_image_info()
                .with_dimensions(ISize::new(
                    dirty_rect.width() as i32,
                    dirty_rect.height() as i32,
                ))
                .with_alpha_type(alpha_type);
            let mut src = Bitmap::new();
            let installed = src.install_pixels(&src_info, packed, row_len);
            debug_assert!(installed);

            let mut paint = Paint::default();
            if independent {
                paint.set_blend_mode(BlendMode::Src);
            }

            let Some(dst_pixmap) = Pixmap::new(base.dst_info(), dst, self.incr_dec_row_bytes)
            else {
                return CodecResult::InternalError;
            };
            let dims = base.dimensions();
            let dst_dims = base.dst_info().dimensions();
            let ctm = Matrix::rect_to_rect_or_identity(
                Rect::from_wh(dims.width as f32, dims.height as f32),
                Rect::from_wh(dst_dims.width as f32, dst_dims.height as f32),
                None,
            );
            let rc = RasterClip::from_rect(&IRect::from_wh(dst_dims.width, dst_dims.height));
            let translate = Matrix::translate(Vector::new(
                dirty_rect.min_incl_x as f32,
                dirty_rect.min_incl_y as f32,
            ));
            let mut draw = Draw::new(dst_pixmap, &ctm, &rc);
            draw.draw_bitmap(
                &src,
                &translate,
                None,
                &SamplingOptions::default(),
                &paint,
                None,
            );
        }

        if result == CodecResult::Success
            && self.frames_complete
            && self.frames.len().checked_sub(1) == Some(index as usize)
        {
            // On success for the last frame, the two-pass buffer is no longer needed: release it.
            // The next decode of a later frame re-allocates it.
            self.two_pass_pixbuf = None;
        }

        result
    }

    /// The two-pass buffer's table, as `(width in bytes, height, stride)`.
    fn two_pass_table_dims(&mut self) -> (usize, usize, usize) {
        let Some(buf) = self.two_pass_pixbuf.as_mut() else {
            return (0, 0, 0);
        };
        let mut pixel_buffer = PixelBuffer::null();
        if pixel_buffer
            .set_from_slice(&self.pixel_config, buf)
            .repr()
            .is_some()
        {
            return (0, 0, 0);
        }
        pixel_buffer.plane0_dims().unwrap_or((0, 0, 0))
    }

    /// Decodes the current frame into the two-pass buffer. The buffer is taken out of `self` for
    /// the call, so the pixel buffer can borrow it while the decoder is borrowed too.
    fn decode_two_pass_buffer(&mut self) -> Status {
        let mut buf = self
            .two_pass_pixbuf
            .take()
            .expect("the two-pass buffer is allocated when two-pass decoding starts");
        let status = {
            let mut pixel_buffer = PixelBuffer::null();
            let set = pixel_buffer.set_from_slice(&self.pixel_config, &mut buf);
            if set.repr().is_some() {
                set
            } else {
                self.decode_frame(&mut pixel_buffer)
            }
        };
        self.two_pass_pixbuf = Some(buf);
        status
    }

    /// Port of `SkWuffsCodec::onStartIncrementalDecode`.
    // Port of: src/codec/SkWuffsCodec.cpp#L489-L545 (chrome/m156)
    fn start_incremental_decode(
        &mut self,
        base: &CodecBase<'_>,
        dst_info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
    ) -> CodecResult {
        if options.subset.is_some() {
            return CodecResult::Unimplemented;
        }
        let result = self.seek_frame(options.frame_index);
        if result != CodecResult::Success {
            return result;
        }

        match self.decode_frame_config().repr() {
            None => {}
            Some(SUSPENSION_SHORT_READ) => return CodecResult::IncompleteInput,
            Some(_) => return CodecResult::ErrorInInput,
        }

        let (pixfmt, bytes_per_pixel) = match dst_info.color_type() {
            ColorType::RGB565 => (PIXEL_FORMAT_BGR_565, 2),
            ColorType::BGRA8888 => (PIXEL_FORMAT_BGRA_NONPREMUL, 4),
            ColorType::RGBA8888 => (PIXEL_FORMAT_RGBA_NONPREMUL, 4),
            _ => (PIXEL_FORMAT_INVALID, 0),
        };

        // One-pass decoding is used if Wuffs can write the Skia pixel format directly, there is
        // no colour profile (Wuffs does not support them), and no scaling is needed (Wuffs does
        // not support scaling).
        let one_pass = pixfmt != PIXEL_FORMAT_INVALID
            && base.encoded_info().profile().is_none()
            && base.dimensions() == dst_info.dimensions();

        let result = if one_pass {
            self.start_one_pass(dst_info, dst, row_bytes, options, pixfmt, bytes_per_pixel)
        } else {
            self.start_two_pass()
        };
        if result != CodecResult::Success {
            return result;
        }

        self.incr_dec_one_pass = one_pass;
        self.incr_dec_row_bytes = row_bytes;
        self.incr_dec_active = true;
        self.first_call_to_incremental_decode = true;
        CodecResult::Success
    }

    /// Port of `SkWuffsCodec::onIncrementalDecode`.
    // Port of: src/codec/SkWuffsCodec.cpp#L627-L637 (chrome/m156)
    fn incremental_decode(
        &mut self,
        base: &CodecBase<'_>,
        dst: &mut [u8],
        rows_decoded: &mut i32,
    ) -> CodecResult {
        if !self.incr_dec_active {
            return CodecResult::InternalError;
        }

        *rows_decoded = base.dst_info().height();

        let result = if self.incr_dec_one_pass {
            self.incremental_one_pass(dst)
        } else {
            self.incremental_two_pass(base, dst)
        };
        if result == CodecResult::Success {
            self.incr_dec_active = false;
            self.incr_dec_row_bytes = 0;
            self.incr_dec_pixel_blend = PixelBlend::Src;
            self.incr_dec_one_pass = false;
        }
        result
    }

    /// Port of `SkWuffsCodec::getEncodedData`: the bytes of the codec's own stream, read again
    /// from a duplicate when the stream has no data of its own.
    // Port of: src/codec/SkWuffsCodec.cpp#L1002-L1016 (chrome/m156)
    fn encoded_data_of_stream(&self) -> Option<Data> {
        let stream: &dyn Stream = &*self.priv_stream;
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

/// Maps a decode's status to the codec result, as `onIncrementalDecodeOnePass` does.
fn decode_status_to_result(status: Status) -> CodecResult {
    match status.repr() {
        None => CodecResult::Success,
        Some(SUSPENSION_SHORT_READ) => CodecResult::IncompleteInput,
        Some(_) => CodecResult::ErrorInInput,
    }
}

impl FrameHolder for WuffsCodec<'_> {
    fn screen_width(&self) -> i32 {
        self.screen_width
    }

    fn screen_height(&self) -> i32 {
        self.screen_height
    }

    // Port of: SkWuffsFrameHolder::onGetFrame (src/codec/SkWuffsCodec.cpp#L375-L378)
    fn get_frame(&self, i: i32) -> Option<&Frame> {
        self.frame(i)
    }
}

impl CodecImpl for WuffsCodec<'_> {
    // Port of: SkWuffsCodec::onGetEncodedFormat (src/codec/SkWuffsCodec.cpp#L441-L443)
    fn on_get_encoded_format(&self) -> EncodedImageFormat {
        EncodedImageFormat::GIF
    }

    // Port of: SkWuffsCodec::onGetPixels (src/codec/SkWuffsCodec.cpp#L445-L455)
    fn on_get_pixels(
        &mut self,
        base: &mut CodecBase<'_>,
        info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        rows_decoded: &mut i32,
    ) -> CodecResult {
        let result = self.start_incremental_decode(base, info, dst, row_bytes, options);
        if result != CodecResult::Success {
            return result;
        }
        self.incremental_decode(base, dst, rows_decoded)
    }

    // Port of: SkWuffsCodec::getEncodedData (src/codec/SkWuffsCodec.cpp#L1002-L1016)
    fn on_get_encoded_data(&mut self, _base: &mut CodecBase<'_>) -> Option<Data> {
        self.encoded_data_of_stream()
    }

    // Port of: SkWuffsCodec::onSupportsIncrementalDecode (src/codec/SkWuffsCodec.cpp#L186-L187)
    fn on_supports_incremental_decode(&self, _dst: &ImageInfo) -> bool {
        true
    }

    // Port of: SkWuffsCodec::onStartIncrementalDecode (src/codec/SkWuffsCodec.cpp#L489-L545)
    fn on_start_incremental_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        dst_info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
    ) -> CodecResult {
        self.start_incremental_decode(base, dst_info, dst, row_bytes, options)
    }

    // Port of: SkWuffsCodec::onIncrementalDecode (src/codec/SkWuffsCodec.cpp#L627-L637)
    fn on_incremental_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        dst: &mut [u8],
        rows_decoded: &mut i32,
    ) -> CodecResult {
        self.incremental_decode(base, dst, rows_decoded)
    }

    // Port of: SkWuffsCodec::onGetFrameCount (src/codec/SkWuffsCodec.cpp#L802-L844)
    fn on_get_frame_count(&mut self) -> i32 {
        if !self.can_seek {
            return 1;
        }

        // Asking for the frame count during an incremental decode must not move the stream, so the
        // count is only read when no decode is in progress.
        let incremental_decode_is_in_progress = self.incr_dec_active;
        if !self.frames_complete && !incremental_decode_is_in_progress {
            self.on_get_frame_count_internal();
            self.update_num_fully_received_frames();
        }
        i32::try_from(self.frames.len()).unwrap_or(i32::MAX)
    }

    // Port of: SkWuffsCodec::onGetFrameInfo (src/codec/SkWuffsCodec.cpp#L875-L890)
    fn on_get_frame_info(&self, index: i32, info: Option<&mut FrameInfo>) -> bool {
        if !self.can_seek {
            // The info is not available without reading the stream forward.
            return false;
        }
        let Some(frame) = self.frame(index) else {
            return false;
        };
        if let Some(info) = info {
            *info = frame.fill_in((index as u64) < self.num_fully_received_frames);
        }
        true
    }

    // Port of: SkWuffsCodec::onGetRepetitionCount (src/codec/SkWuffsCodec.cpp#L892-L906)
    fn on_get_repetition_count(&mut self) -> i32 {
        // Wuffs' loop count is how many times to play the loop. Skia's repetition count is how
        // many times to play it after the first play. Wuffs uses 0 for "forever", as Skia uses
        // kRepetitionCountInfinite.
        let n = self.decoder.num_animation_loops();
        if n == 0 {
            return REPETITION_COUNT_INFINITE;
        }
        i32::try_from(n - 1).unwrap_or(i32::MAX)
    }

    // Port of: SkWuffsCodec::onIsAnimated (src/codec/SkWuffsCodec.cpp#L908-L918)
    fn on_is_animated(&mut self) -> IsAnimated {
        if self.frames.len() > 1 {
            return IsAnimated::Yes;
        }
        // With one frame so far, more may still come, so the answer is only known once the
        // frames are complete.
        if self.frames_complete {
            IsAnimated::No
        } else {
            IsAnimated::Unknown
        }
    }

    // Port of: SkWuffsCodec::getFrameHolder (src/codec/SkWuffsCodec.cpp#L553-L555)
    fn frame_holder(&self) -> Option<&dyn FrameHolder> {
        Some(self)
    }
}

/// Port of `SkGifDecoder::IsGif`.
// Port of: src/codec/SkWuffsCodec.cpp#L1019-L1024 (chrome/m156)
#[must_use]
pub fn is_gif(buf: &[u8]) -> bool {
    buf.len() >= 4 && &buf[..4] == b"GIF8"
}

/// Port of `SkGifDecoder::MakeFromStream` with a selection policy: makes a GIF codec over the
/// stream, which must be rewound. Under [`SelectionPolicy::PreferStillImage`] the stream is read
/// as it is; under any other policy a stream without a position and a length is first copied
/// into memory, so that the frames can be seeked.
///
/// # Errors
/// The [`CodecResult`] the C++ reports in its `result` out-parameter: the Wuffs config failed
/// (`ErrorInInput`, `IncompleteInput`, ...), or the image has no size (`InvalidInput`).
// Port of: src/codec/SkWuffsCodec.cpp#L1026-L1103 (chrome/m156)
#[doc(alias = "SkGifDecoder::MakeFromStream")]
pub fn make_from_stream_with_policy<'a>(
    mut stream: Box<dyn Stream + Send + 'a>,
    policy: SelectionPolicy,
) -> std::result::Result<Codec<'a>, CodecResult> {
    let mut can_seek = stream.has_position() && stream.has_length();

    if policy != SelectionPolicy::PreferStillImage && !can_seek {
        // Some clients need to be able to seek the stream, but may not provide a seekable one.
        let data = copy_stream_to_data(&mut *stream);
        stream = MemoryStream::make(data);
        can_seek = true;
    }

    let mut iobuf = IoBuffer::with_capacity(SK_WUFFS_CODEC_BUFFER_SIZE);
    let mut imgcfg = ImageConfig::default();
    let mut decoder = Box::new(GifDecoder::new());

    let reset_result =
        reset_and_decode_image_config(&mut decoder, Some(&mut imgcfg), &mut iobuf, &mut *stream);
    if reset_result != CodecResult::Success {
        return Err(reset_result);
    }

    let width = imgcfg.pixcfg.width;
    let height = imgcfg.pixcfg.height;
    if width == 0 || width > i32::MAX as u32 || height == 0 || height > i32::MAX as u32 {
        return Err(CodecResult::InvalidInput);
    }

    let color = if imgcfg.pixcfg.pixfmt == PIXEL_FORMAT_BGRA_NONPREMUL {
        Color::BGRA
    } else {
        Color::RGBA
    };

    // In Skia's API, the alpha reported here is only for the first frame.
    let alpha = if imgcfg.first_frame_is_opaque {
        Alpha::Opaque
    } else {
        Alpha::Binary
    };

    let encoded_info = EncodedInfo::make(width as i32, height as i32, color, alpha, 8);
    let imp = WuffsCodec::new(stream, can_seek, decoder, imgcfg, iobuf);

    // The base codec gets no stream: the codec manages its own (see WuffsCodec).
    Ok(Codec::new(
        encoded_info,
        Box::new(imp),
        None,
        EncodedOrigin::TopLeft,
        Some(SkcmsPixelFormat::Rgba8888),
    ))
}

/// Port of `SkGifDecoder::MakeFromStream` with the default selection policy, which is what the
/// decoder registry calls: [`SelectionPolicy::PreferStillImage`].
///
/// # Errors
/// As [`make_from_stream_with_policy`].
// Port of: src/codec/SkWuffsCodec.cpp#L1026-L1103 (chrome/m156), the registry entry point
pub fn make_from_stream<'a>(
    stream: Box<dyn Stream + Send + 'a>,
) -> std::result::Result<Codec<'a>, CodecResult> {
    make_from_stream_with_policy(stream, SelectionPolicy::PreferStillImage)
}
