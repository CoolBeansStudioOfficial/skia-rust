// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkWebpCodec.cpp (chrome/m156) and src/codec/SkWebpCodec.h, the WebP codec on
// top of skia-rust-libwebp (demux for the container and frames, the incremental decoder for the
// pixels). The orientation is read from the EXIF chunk through `exif::parse_encoded_origin`.

//! Skia's WebP codec: `SkWebpCodec`, with its animation frames, scaling and subset decodes, and
//! the blend of an animated frame with the frame before it.

// The integer and float conversions below mirror the C++ (`int` sizes, `uint32_t` payload sizes and
// `float` scales in SkWebpCodec.cpp), so the cast lints are allowed for this module.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::raster_pipeline::{
    MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline, Stage,
};
use skia_rust_core::rect::{Contains, IRect};
use skia_rust_core::scalar::scalar_round_to_int;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::Stream;
use skia_rust_libwebp::demux::{
    Blend as WebpBlend, DemuxState, Demuxer, Dispose, FormatFeature, demux_internal,
};
use skia_rust_libwebp::{CspMode, DecodeOptions, IDecoder, Status, get_features};
use skia_rust_skcms::PixelFormat as SkcmsPixelFormat;

use crate::codec::{
    Codec, CodecBase, CodecImpl, FrameInfo, IsAnimated, NO_FRAME, Options,
    REPETITION_COUNT_INFINITE, Result as CodecResult,
};
use crate::codec_animation::{Blend, DisposalMethod};
use crate::encoded_info::{Alpha, Color, EncodedInfo};
use crate::exif;
use crate::frame_holder::{Frame, FrameHolder, set_alpha_and_required_frame};
use crate::sampler;

/// `ANIMATION_FLAG` of `webp/format_constants.h`.
const ANIMATION_FLAG: u8 = 0x02;
/// `EXIF_FLAG` of `webp/format_constants.h`.
const EXIF_FLAG: u8 = 0x08;
/// `kBufferSize` of `get_header_from_stream`.
const HEADER_BUFFER_SIZE: usize = 4096;

/// Port of `SkWebpCodec::IsWebp`: `RIFF` at the start, `WEBPVP` at byte 8 (the chunk of a VP8, VP8L
/// or VP8X header).
// Port of: src/codec/SkWebpCodec.cpp#L35-L41 (chrome/m156)
#[must_use]
pub fn is_webp(buf: &[u8]) -> bool {
    buf.len() >= 14 && &buf[..4] == b"RIFF" && &buf[8..14] == b"WEBPVP"
}

/// Reads the rest of `stream` into `out`. Port of the `SkStreamPriv::Copy` the codec uses for
/// `ensureAllData`.
fn read_rest(stream: &mut dyn Stream, out: &mut Vec<u8>) {
    let mut buf = [0u8; HEADER_BUFFER_SIZE];
    loop {
        let n = stream.read(&mut buf);
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
    }
}

/// Port of `get_header_from_stream`: reads as much of the stream as the codec needs to parse the
/// header and find the first frame. Returns whether only the header was read, and the bytes read,
/// or `None` for incomplete or invalid input.
// Port of: src/codec/SkWebpCodec.cpp#L44-L134 (chrome/m156)
fn get_header_from_stream(stream: &mut dyn Stream) -> (bool, Option<Vec<u8>>) {
    // "RIFF" (4) + file size (4) + "WEBP" (4) = 12
    const INITIAL_HEADER_SIZE: usize = 12;
    const CHUNK_SIZE_HEADER: usize = 8;
    const VP8_UNCOMPRESSED_CHUNK_MIN_SIZE: usize = 3;
    const VP8_UNCOMPRESSED_CHUNK_MAX_SIZE: usize = 10;
    const IMAGE_HEADER_SIZE: usize = 5;
    // VP8X payload should always be 10
    const VP8X_PAYLOAD_SIZE: usize = 10;
    let mut temp: Vec<u8> = Vec::new();
    let mut buffer = [0u8; HEADER_BUFFER_SIZE];
    let bytes_read = stream.read(&mut buffer[..INITIAL_HEADER_SIZE]);
    if bytes_read < INITIAL_HEADER_SIZE || &buffer[..4] != b"RIFF" || &buffer[8..12] != b"WEBP" {
        // Fail if we don't have valid webp data
        return (false, None);
    }
    temp.extend_from_slice(&buffer[..bytes_read]);
    let mut is_animated = false;
    let mut first_chunk_read = false;

    while !stream.is_at_end() {
        // Read the chunk's FourCC and payload size
        let bytes_read = stream.read(&mut buffer[..CHUNK_SIZE_HEADER]);
        if bytes_read < CHUNK_SIZE_HEADER {
            return (false, None);
        }
        temp.extend_from_slice(&buffer[..bytes_read]);

        // Return upon VP8 or VP8L chunk, keep reading and writing otherwise
        // Handle ANMF subchunks, if animated image read the first frame and return
        if &buffer[..4] == b"VP8 " {
            let bytes_read = stream.read(&mut buffer[..VP8_UNCOMPRESSED_CHUNK_MAX_SIZE]);
            if bytes_read < VP8_UNCOMPRESSED_CHUNK_MIN_SIZE {
                // Invalid webp
                return (false, None);
            }
            temp.extend_from_slice(&buffer[..bytes_read]);
            return (true, Some(temp));
        } else if &buffer[..4] == b"VP8L" {
            let bytes_read = stream.read(&mut buffer[..IMAGE_HEADER_SIZE]);
            if bytes_read < IMAGE_HEADER_SIZE {
                // Invalid webp
                return (false, None);
            }
            temp.extend_from_slice(&buffer[..bytes_read]);
            return (true, Some(temp));
        } else if &buffer[..4] == b"VP8X" {
            if first_chunk_read {
                return (false, None);
            }
            first_chunk_read = true;
            let payload_size = u32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);
            if payload_size as usize != VP8X_PAYLOAD_SIZE {
                return (false, None);
            }
            let bytes_read = stream.read(&mut buffer[..VP8X_PAYLOAD_SIZE]);
            if bytes_read < VP8X_PAYLOAD_SIZE {
                // Invalid webp
                return (false, None);
            }
            temp.extend_from_slice(&buffer[..bytes_read]);

            let vp8x_feature_flags = buffer[0];
            let has_exif = vp8x_feature_flags & EXIF_FLAG != 0;
            is_animated = vp8x_feature_flags & ANIMATION_FLAG != 0;
            if has_exif {
                // If exif data, we have to read the whole stream because exif data is at the end
                // of the file and we want that in order to create the codec.
                loop {
                    let bytes_read = stream.read(&mut buffer);
                    temp.extend_from_slice(&buffer[..bytes_read]);
                    if stream.is_at_end() {
                        break;
                    }
                }
                return (false, Some(temp));
            }
        } else {
            if !first_chunk_read {
                // We only expect VP8, VP8L, or VP8X chunks to be the first.
                return (false, None);
            }
            let animated_frame = &buffer[..4] == b"ANMF";

            // According to the RIFF document format, if 'payloadSize' is odd a single padding byte
            // -- which must be 0 to conform with RIFF -- is added.
            let mut payload_size = u32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);
            if payload_size % 2 != 0 {
                payload_size += 1;
            }

            // Read 'payload size' bytes ahead to get to the next chunk
            while payload_size != 0 {
                let want = (payload_size as usize).min(HEADER_BUFFER_SIZE);
                let bytes_read = stream.read(&mut buffer[..want]);
                if bytes_read == 0 {
                    break;
                }
                temp.extend_from_slice(&buffer[..bytes_read]);
                payload_size -= bytes_read as u32;
            }
            if payload_size != 0 {
                // Invalid webp
                return (false, None);
            }

            if animated_frame && is_animated {
                // If this is an animated frame and we are expecting it, return the whole frame
                // (this is the first frame which we want in order to create the codec).
                return (true, Some(temp));
            }
        }
    }
    // We never reached image data but read the whole stream
    (true, Some(temp))
}

/// Port of `SkWebpCodec::webp_decode_mode`: the libwebp output mode of a destination colour type.
// Port of: src/codec/SkWebpCodec.cpp#L300-L311 (chrome/m156)
fn webp_decode_mode(ct: ColorType, premultiply: bool) -> Option<CspMode> {
    match ct {
        ColorType::BGRA8888 => Some(if premultiply {
            CspMode::BgrA
        } else {
            CspMode::Bgra
        }),
        ColorType::RGBA8888 => Some(if premultiply {
            CspMode::RgbA
        } else {
            CspMode::Rgba
        }),
        ColorType::RGB565 => Some(CspMode::Rgb565),
        _ => None,
    }
}

/// Port of `is_8888`.
// Port of: src/codec/SkWebpCodec.cpp#L545-L553 (chrome/m156)
fn is_8888(ct: ColorType) -> bool {
    matches!(ct, ColorType::RGBA8888 | ColorType::BGRA8888)
}

/// Port of `SkWebpCodec::FrameHolder`: the frames of the image, and the size of the screen they
/// are drawn on. The holder is the codec's `WebpCodec` itself, so the frames are its own fields.
impl FrameHolder for WebpCodec<'_> {
    fn screen_width(&self) -> i32 {
        self.screen_width
    }

    fn screen_height(&self) -> i32 {
        self.screen_height
    }

    // Port of: SkWebpCodec::FrameHolder::onGetFrame (src/codec/SkWebpCodec.cpp#L503-L506)
    fn get_frame(&self, i: i32) -> Option<&Frame> {
        usize::try_from(i).ok().and_then(|i| self.frames.get(i))
    }
}

/// The decoder half of `SkWebpCodec`. The demuxer borrows `data`, so it is parsed again from the
/// bytes when it is needed: the parse is deterministic, so the answers are the same.
// Port of: src/codec/SkWebpCodec.h#L23-L95 (chrome/m156)
pub struct WebpCodec<'a> {
    /// `fData`: the bytes the demuxer parses.
    data: Arc<[u8]>,
    /// `fOnlyHeaderParsed`: `data` holds the header and the first frame, not the whole stream.
    only_header_parsed: bool,
    /// `fFailed`: reading the frames failed, so no more are parsed.
    failed: bool,
    /// The stream `data` came from, which is read to the end by `ensureAllData` when only the
    /// header was read.
    stream: Option<Box<dyn Stream + Send + 'a>>,
    screen_width: i32,
    screen_height: i32,
    /// `fFrameHolder`'s frames, indexed by frame id.
    frames: Vec<Frame>,
}

impl std::fmt::Debug for WebpCodec<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebpCodec")
            .field("only_header_parsed", &self.only_header_parsed)
            .field("failed", &self.failed)
            .field("frames", &self.frames.len())
            .finish_non_exhaustive()
    }
}

impl WebpCodec<'_> {
    /// The demuxer over the bytes read so far. Port of `fDemux`.
    fn demuxer(&self) -> Option<Demuxer<'_>> {
        demux_internal(&self.data, true).0
    }

    /// Port of `SkWebpCodec::ensureAllData`: reads the rest of the stream if only the header was
    /// read, and parses the whole data again. Returns `false` if the data does not parse.
    // Port of: src/codec/SkWebpCodec.cpp#L212-L228 (chrome/m156)
    fn ensure_all_data(&mut self) -> bool {
        if self.only_header_parsed {
            let mut new_data: Vec<u8> = self.data.to_vec();
            if let Some(stream) = self.stream.as_deref_mut() {
                read_rest(stream, &mut new_data);
            }
            self.data = Arc::from(new_data);
            self.only_header_parsed = false;
            let (_, state) = demux_internal(&self.data, true);
            // We know the demuxer state is at least ParsedHeader already, which we allow.
            if state == DemuxState::ParseError {
                return false;
            }
        }
        true
    }

    /// Port of `SkWebpCodec::onGetPixels`'s frame placement and decode: decodes frame `index` into
    /// `dst`, blending it with the frame before it where the frame needs that.
    // Port of: src/codec/SkWebpCodec.cpp#L555-L800 (chrome/m156)
    // One function, as SkWebpCodec::onGetPixels is, so the steps stay comparable with the C++.
    #[allow(clippy::too_many_lines)]
    fn decode_frame(
        &mut self,
        base: &mut CodecBase<'_>,
        dst_info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        rows_decoded: &mut i32,
    ) -> CodecResult {
        let index = options.frame_index;
        if !self.ensure_all_data() {
            return CodecResult::InvalidInput;
        }
        let screen_width = self.screen_width;
        let screen_height = self.screen_height;
        let independent = if index == 0 {
            true
        } else {
            let Some(frame) = usize::try_from(index).ok().and_then(|i| self.frames.get(i)) else {
                return CodecResult::InvalidInput;
            };
            frame.required_frame() == NO_FRAME
        };
        let Some(demuxer) = self.demuxer() else {
            return CodecResult::InvalidInput;
        };
        // If this succeeded in onGetFrameCount(), it should succeed again here.
        let Some(frame) = demuxer.get_frame(index + 1) else {
            return CodecResult::InvalidInput;
        };
        let fragment: &[u8] = frame.fragment;
        let frame_has_alpha = frame.has_alpha;
        let frame_blend = frame.blend_method;

        // Get the frameRect. libwebp will have already signaled an error if this is not fully
        // contained by the canvas.
        let bounds = IRect::from_wh(screen_width, screen_height);
        let mut frame_rect =
            IRect::from_xywh(frame.x_offset, frame.y_offset, frame.width, frame.height);
        let frame_is_subset = frame_rect != bounds;
        if independent && frame_is_subset {
            sampler::fill(dst_info, dst, row_bytes, options.zero_initialized);
        }

        let mut dst_x = frame_rect.left();
        let mut dst_y = frame_rect.top();
        let mut subset_width = frame_rect.width();
        let mut subset_height = frame_rect.height();
        let mut crop: Option<(i32, i32, i32, i32)> = None;
        if let Some(subset_in) = options.subset {
            let mut subset = subset_in;
            if !IRect::intersects(&subset, &frame_rect) {
                return CodecResult::Success;
            }

            let min_x_offset = dst_x.min(subset.left());
            let min_y_offset = dst_y.min(subset.top());
            dst_x -= min_x_offset;
            dst_y -= min_y_offset;
            frame_rect.offset((-min_x_offset, -min_y_offset));
            subset.offset((-min_x_offset, -min_y_offset));

            let intersection =
                IRect::intersect(&frame_rect, &subset).unwrap_or_else(IRect::new_empty);
            subset_width = intersection.width();
            subset_height = intersection.height();

            crop = Some((subset.left(), subset.top(), subset_width, subset_height));
        }

        // Ignore the frame size and offset when determining if scaling is necessary.
        let mut scaled_width = subset_width;
        let mut scaled_height = subset_height;
        let src_size = match options.subset {
            Some(subset) => ISize::new(subset.width(), subset.height()),
            None => base.dimensions(),
        };
        let mut scale: Option<(i32, i32)> = None;
        if src_size != dst_info.dimensions() {
            if frame_is_subset {
                let scale_x = dst_info.width() as f32 / src_size.width as f32;
                let scale_y = dst_info.height() as f32 / src_size.height as f32;
                // We need to be conservative here and floor rather than round. Otherwise, we may
                // find ourselves decoding off the end of memory.
                dst_x = (scale_x * dst_x as f32) as i32;
                scaled_width = (scale_x * scaled_width as f32) as i32;
                dst_y = (scale_y * dst_y as f32) as i32;
                scaled_height = (scale_y * scaled_height as f32) as i32;
                if scaled_width == 0 || scaled_height == 0 {
                    return CodecResult::Success;
                }
            } else {
                scaled_width = dst_info.width();
                scaled_height = dst_info.height();
            }
            scale = Some((scaled_width, scaled_height));
        }

        let blend_with_prev_frame =
            !independent && frame_blend == WebpBlend::Blend && frame_has_alpha;

        let mut webp_info = dst_info.clone();
        if !frame_has_alpha {
            webp_info = webp_info.with_alpha_type(AlphaType::Opaque);
        } else if base.color_xform() || blend_with_prev_frame {
            // the colorXform and blend_line expect unpremul.
            webp_info = webp_info.with_alpha_type(AlphaType::Unpremul);
        }
        if base.color_xform() {
            // Swizzling between RGBA and BGRA is zero cost in a color transform. So when we have a
            // color transform, we should decode to whatever is easiest for libwebp, and then let
            // the color transform swizzle if necessary.
            webp_info = webp_info.with_color_type(ColorType::BGRA8888);
        }

        // The pixels of the frame are decoded into `temp` when a colour transform or a blend needs
        // them first, and straight into `dst` otherwise (`webpDst.installPixels`). The rows are
        // `webp_stride` bytes apart, and the frame's first pixel is at `webp_origin`.
        let webp_bpp = webp_info.bytes_per_pixel();
        let dst_bpp = dst_info.bytes_per_pixel();
        let use_temp =
            (base.color_xform() && !is_8888(dst_info.color_type())) || blend_with_prev_frame;
        let (webp_stride, webp_origin) = if use_temp {
            let stride = dst_info.width() as usize * webp_bpp;
            (stride, dst_x as usize * webp_bpp + dst_y as usize * stride)
        } else {
            (
                row_bytes,
                dst_x as usize * dst_bpp + dst_y as usize * row_bytes,
            )
        };
        let mut temp: Vec<u8> = if use_temp {
            vec![0u8; webp_stride * dst_info.height() as usize]
        } else {
            Vec::new()
        };

        let Some(mode) = webp_decode_mode(
            webp_info.color_type(),
            webp_info.alpha_type() == AlphaType::Premul,
        ) else {
            return CodecResult::InvalidInput;
        };
        let decode_options = DecodeOptions {
            crop,
            scale,
            ..DecodeOptions::default()
        };
        let Some(mut idec) = IDecoder::new(mode, decode_options) else {
            return CodecResult::InvalidInput;
        };

        let status = idec.update(fragment);
        let (decoded_rows, result) = match status {
            Status::Ok => (scaled_height, CodecResult::Success),
            Status::Suspended => {
                // WebPIDecGetRGB's last_y: the rows the decoder wrote so far.
                let Some(rgb) = idec.rgb().filter(|r| !r.pixels.is_empty()) else {
                    return CodecResult::InvalidInput;
                };
                if rgb.last_y <= 0 {
                    return CodecResult::InvalidInput;
                }
                *rows_decoded = rgb.last_y + dst_y;
                (rgb.last_y, CodecResult::IncompleteInput)
            }
            _ => return CodecResult::InvalidInput,
        };

        // The rows libwebp wrote go where its external memory would have put them.
        let row_len = scaled_width as usize * webp_bpp;
        if let Some(rgb) = idec.rgb().filter(|r| !r.pixels.is_empty()) {
            for y in 0..decoded_rows as usize {
                let src_row = &rgb.pixels[y * rgb.stride..y * rgb.stride + row_len];
                let o = webp_origin + y * webp_stride;
                if use_temp {
                    temp[o..o + row_len].copy_from_slice(src_row);
                } else {
                    dst[o..o + row_len].copy_from_slice(src_row);
                }
            }
        }

        // The destination row of the frame's first pixel, as `dst` is shifted in the C++.
        let dst_origin = dst_x as usize * dst_bpp + dst_y as usize * row_bytes;
        let width = scaled_width as usize;
        if base.color_xform() {
            // Each row is transformed into the destination format. The source row is copied out
            // first, since it may be the destination's own memory.
            let mut xform_row = vec![0u8; width * dst_bpp];
            let mut tmp_row = vec![0u8; width * dst_bpp];
            let pipeline = blend_pipeline(
                dst_info.color_type(),
                dst_info.color_type(),
                dst_info.alpha_type(),
                frame_has_alpha,
            );
            for y in 0..decoded_rows as usize {
                let src_at = webp_origin + y * webp_stride;
                let src_row: Vec<u8> = if use_temp {
                    temp[src_at..src_at + row_len].to_vec()
                } else {
                    dst[src_at..src_at + row_len].to_vec()
                };
                let dst_at = dst_origin + y * row_bytes;
                if blend_with_prev_frame {
                    base.apply_color_xform(&mut tmp_row, &src_row, width);
                    blend_line(&pipeline, &mut dst[dst_at..], &tmp_row, width);
                } else {
                    base.apply_color_xform(&mut xform_row, &src_row, width);
                    dst[dst_at..dst_at + width * dst_bpp].copy_from_slice(&xform_row);
                }
            }
        } else if blend_with_prev_frame {
            // Blending always decodes into `temp`, so the source is the decoded row.
            let pipeline = blend_pipeline(
                dst_info.color_type(),
                webp_info.color_type(),
                dst_info.alpha_type(),
                frame_has_alpha,
            );
            for y in 0..decoded_rows as usize {
                let src_at = webp_origin + y * webp_stride;
                let dst_at = dst_origin + y * row_bytes;
                blend_line(
                    &pipeline,
                    &mut dst[dst_at..],
                    &temp[src_at..src_at + row_len],
                    width,
                );
            }
        }

        result
    }
}

/// Port of `RPBlender`'s pipeline: loads the destination, premultiplies it if the destination is
/// unpremultiplied, loads the source, premultiplies it if it has alpha, composites with
/// source-over, and stores the destination again. The source is unpremultiplied.
// Port of: src/codec/SkWebpCodec.cpp#L467-L493 (RPBlender::RPBlender, chrome/m156)
fn blend_pipeline(
    dst_ct: ColorType,
    src_ct: ColorType,
    dst_alpha: AlphaType,
    src_has_alpha: bool,
) -> RasterPipeline<'static> {
    let dst_ctx = MemoryCtx::new(MemSlot(0));
    let src_ctx = MemoryCtx::new(MemSlot(1));
    let mut p = RasterPipeline::new();
    p.append_load_dst(dst_ct, dst_ctx);
    if dst_alpha == AlphaType::Unpremul {
        p.append(Stage::PremulDst);
    }
    p.append_load(src_ct, src_ctx);
    if src_has_alpha {
        p.append(Stage::Premul);
    }
    p.append(Stage::Srcover);
    if dst_alpha == AlphaType::Unpremul {
        p.append(Stage::Unpremul);
    }
    p.append_store(dst_ct, dst_ctx);
    p
}

/// Port of `RPBlender::blendLine`: blends `width` pixels of `src` over `dst`.
// Port of: src/codec/SkWebpCodec.cpp#L495-L500 (chrome/m156)
fn blend_line(pipeline: &RasterPipeline<'static>, dst: &mut [u8], src: &[u8], width: usize) {
    let mut mem = MemoryBindings::new();
    mem.bind(MemSlot(0), MemView::write(dst));
    mem.bind(MemSlot(1), MemView::read(src));
    pipeline.run(0, 0, width, 1, &mut mem);
}

impl CodecImpl for WebpCodec<'_> {
    // Port of: SkWebpCodec::onGetEncodedFormat (src/codec/SkWebpCodec.h#L37)
    fn on_get_encoded_format(&self) -> EncodedImageFormat {
        EncodedImageFormat::WEBP
    }

    // Port of: SkWebpCodec::onGetPixels (src/codec/SkWebpCodec.cpp#L555-L800)
    fn on_get_pixels(
        &mut self,
        base: &mut CodecBase<'_>,
        info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        rows_decoded: &mut i32,
    ) -> CodecResult {
        self.decode_frame(base, info, dst, row_bytes, options, rows_decoded)
    }

    // Port of: SkWebpCodec::onGetValidSubset (src/codec/SkWebpCodec.cpp#L340-L356)
    fn on_get_valid_subset(&self, base: &CodecBase<'_>, subset: &mut IRect) -> bool {
        let bounds = IRect::from_size(base.dimensions());
        if !bounds.contains(*subset) {
            return false;
        }
        // As stated below, libwebp snaps to even left and top. Make sure top and left are even, so
        // we decode this exact subset. Leave right and bottom unmodified, so we suggest a slightly
        // larger subset than requested.
        *subset = IRect::from_ltrb(
            (subset.left() >> 1) << 1,
            (subset.top() >> 1) << 1,
            subset.right(),
            subset.bottom(),
        );
        true
    }

    // Port of: SkWebpCodec::onRewind (src/codec/SkWebpCodec.cpp#L414-L418). The codec holds its
    // own copy of the data, so it never rewinds the stream.
    fn on_rewind(&mut self, _base: &mut CodecBase<'_>) -> bool {
        true
    }

    // Port of: SkCodec::getEncodedData on the stream the codec keeps (src/codec/SkCodec.cpp#L1087-L1100)
    fn on_get_encoded_data(&mut self, _base: &mut CodecBase<'_>) -> Option<Data> {
        let stream = self.stream.as_deref_mut()?;
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

    // Port of: SkWebpCodec::onGetFrameCount (src/codec/SkWebpCodec.cpp#L452-L498)
    fn on_get_frame_count(&mut self) -> i32 {
        let flags = self
            .demuxer()
            .map_or(0, |d| d.get_i(FormatFeature::FormatFlags));
        if flags & u32::from(ANIMATION_FLAG) == 0 {
            return 1;
        }

        let old_frame_count = self.frames.len();
        if self.failed {
            return i32::try_from(old_frame_count).unwrap_or(i32::MAX);
        }

        if !self.ensure_all_data() {
            return 0;
        }

        let Some(demuxer) = demux_internal(&self.data, true).0 else {
            return 0;
        };
        let frame_count = demuxer.get_i(FormatFeature::FrameCount) as usize;
        if old_frame_count == frame_count {
            // We have already parsed this.
            return frame_count as i32;
        }

        self.frames
            .reserve(frame_count.saturating_sub(old_frame_count));
        for i in old_frame_count..frame_count {
            let Some(iter) = demuxer.get_frame(i as i32 + 1) else {
                self.failed = true;
                break;
            };

            // libwebp only reports complete frames of an animated image.
            let mut frame = Frame::new(
                i as i32,
                if iter.has_alpha {
                    Alpha::Unpremul
                } else {
                    Alpha::Opaque
                },
            );
            frame.set_xywh(iter.x_offset, iter.y_offset, iter.width, iter.height);
            frame.set_disposal_method(if iter.dispose_method == Dispose::Background {
                DisposalMethod::RestoreBgColor
            } else {
                DisposalMethod::Keep
            });
            frame.set_duration(iter.duration);
            if iter.blend_method != WebpBlend::Blend {
                frame.set_blend(Blend::Src);
            }
            set_alpha_and_required_frame(
                self.screen_width,
                self.screen_height,
                &self.frames,
                &mut frame,
            );
            self.frames.push(frame);
        }

        i32::try_from(self.frames.len()).unwrap_or(i32::MAX)
    }

    // Port of: SkWebpCodec::onGetFrameInfo (src/codec/SkWebpCodec.cpp#L427-L443)
    fn on_get_frame_info(&self, index: i32, info: Option<&mut FrameInfo>) -> bool {
        let Some(frame) = usize::try_from(index).ok().and_then(|i| self.frames.get(i)) else {
            return false;
        };
        if let Some(info) = info {
            // libwebp only reports fully received frames for an animated image.
            *info = frame.fill_in(true);
        }
        true
    }

    // Port of: SkWebpCodec::onGetRepetitionCount (src/codec/SkWebpCodec.cpp#L402-L416)
    fn on_get_repetition_count(&mut self) -> i32 {
        let Some(demuxer) = self.demuxer() else {
            return 0;
        };
        if demuxer.get_i(FormatFeature::FormatFlags) & u32::from(ANIMATION_FLAG) == 0 {
            return 0;
        }
        let loop_count = demuxer.get_i(FormatFeature::LoopCount) as i32;
        if loop_count == 0 {
            return REPETITION_COUNT_INFINITE;
        }
        loop_count - 1
    }

    // Port of: SkWebpCodec::onIsAnimated (src/codec/SkWebpCodec.cpp#L398-L401)
    fn on_is_animated(&mut self) -> IsAnimated {
        let flags = self
            .demuxer()
            .map_or(0, |d| d.get_i(FormatFeature::FormatFlags));
        if flags & u32::from(ANIMATION_FLAG) != 0 {
            IsAnimated::Yes
        } else {
            IsAnimated::No
        }
    }

    // Port of: SkWebpCodec::getFrameHolder (src/codec/SkWebpCodec.h#L46-L48)
    fn frame_holder(&self) -> Option<&dyn FrameHolder> {
        Some(self)
    }

    // Port of: SkScalingCodec::onGetScaledDimensions (src/codec/SkScalingCodec.h#L28-L35)
    fn on_get_scaled_dimensions(&self, base: &CodecBase<'_>, desired_scale: f32) -> ISize {
        let dim = base.dimensions();
        // SkCodec treats zero dimensional images as errors, so the minimum size that we will
        // recommend is 1x1.
        ISize::new(
            1.max(scalar_round_to_int(desired_scale * dim.width as f32)),
            1.max(scalar_round_to_int(desired_scale * dim.height as f32)),
        )
    }

    // Port of: SkScalingCodec::onDimensionsSupported (src/codec/SkScalingCodec.h#L37-L43)
    fn on_dimensions_supported(&self, base: &CodecBase<'_>, dim: ISize) -> bool {
        let size = base.dimensions();
        1 <= dim.width && dim.width <= size.width && 1 <= dim.height && dim.height <= size.height
    }
}

/// Port of `SkWebpCodec::MakeFromStream`: parses the RIFF container, and makes a codec from the
/// size, the ICC profile and the first frame's features.
///
/// # Errors
/// The [`CodecResult`] the C++ reports in its `result` out-parameter: `IncompleteInput` for data
/// that is cut short, `InvalidInput` for data that does not parse.
// Port of: src/codec/SkWebpCodec.cpp#L136-L250 (chrome/m156), the registry entry point
#[doc(alias = "SkWebpDecoder::Decode")]
pub fn make_from_stream<'a>(
    mut stream: Box<dyn Stream + Send + 'a>,
) -> std::result::Result<Codec<'a>, CodecResult> {
    // Webp demux needs a contiguous data buffer. If there is no memory base for the stream, only
    // the necessary portions are read; the rest is read later.
    // The stream is kept, as the C++ base codec keeps it, for getEncodedData and for the rest of
    // the data when only the header was read. Without a memory base it is dropped once the whole
    // data has been read.
    let (only_header_parsed, data, keep_stream): (bool, Vec<u8>, bool) =
        if let Some(bytes) = stream.get_memory_base() {
            // It is safe to make without copy because we'll hold onto the stream.
            (false, bytes.to_vec(), true)
        } else {
            let (only_header, data) = get_header_from_stream(&mut *stream);
            let Some(data) = data else {
                return Err(CodecResult::IncompleteInput);
            };
            (only_header, data, only_header)
        };

    let (demuxer, state) = demux_internal(&data, true);
    let demuxer = match state {
        DemuxState::ParseError => return Err(CodecResult::InvalidInput),
        DemuxState::ParsingHeader => return Err(CodecResult::IncompleteInput),
        DemuxState::ParsedHeader | DemuxState::Done => demuxer,
    };
    let Some(demuxer) = demuxer else {
        return Err(CodecResult::InvalidInput);
    };

    let width = i64::from(demuxer.get_i(FormatFeature::CanvasWidth));
    let height = i64::from(demuxer.get_i(FormatFeature::CanvasHeight));

    // Validate the image size that's about to be decoded: the pixel count must fit, and so must
    // the 4 bytes of each pixel.
    let size = width * height;
    if i32::try_from(size).is_err() || size > (0x7FFF_FFFF >> 2) {
        return Err(CodecResult::InvalidInput);
    }
    let width = width as i32;
    let height = height as i32;

    // The ICC profile, if it is an RGB one (SkCodecs::ColorProfile::MakeICCProfile).
    let mut profile = None;
    if let Some(iter) = demuxer.get_chunk(*b"ICCP", 1) {
        let chunk: Arc<[u8]> = Arc::from(iter.chunk);
        if let Some(parsed) = skia_rust_skcms::parse(&chunk)
            && parsed.data_color_space == skia_rust_skcms::signature::RGB
        {
            profile = Some((chunk, parsed));
        }
    }

    // The orientation from the EXIF chunk (`SkParseEncodedOrigin`), top-left when there is none.
    let mut origin = EncodedOrigin::TopLeft;
    if let Some(iter) = demuxer.get_chunk(*b"EXIF", 1)
        && let Some(parsed) = exif::parse_encoded_origin(iter.chunk)
    {
        origin = parsed;
    }

    // Get the first frame and its "features" to determine the color and alpha types.
    let Some(frame) = demuxer.get_frame(1) else {
        return Err(CodecResult::IncompleteInput);
    };
    let features = match get_features(frame.fragment) {
        Ok(features) => features,
        Err(Status::Suspended | Status::NotEnoughData) => {
            return Err(CodecResult::IncompleteInput);
        }
        Err(_) => return Err(CodecResult::InvalidInput),
    };

    let has_alpha = frame.has_alpha || frame.width != width || frame.height != height;
    let (color, alpha) = match (features.is_lossless, has_alpha) {
        // The lossless format (BGRA).
        (true, true) => (Color::BGRA, Alpha::Unpremul),
        (true, false) => (Color::BGRX, Alpha::Opaque),
        // The lossy format (YUV).
        (false, true) => (Color::YUVA, Alpha::Unpremul),
        (false, false) => (Color::YUV, Alpha::Opaque),
    };

    let mut encoded_info = EncodedInfo::make(width, height, color, alpha, 8);
    if let Some((chunk, parsed)) = profile {
        encoded_info = encoded_info.with_profile(chunk, parsed);
    }

    let imp = WebpCodec {
        data: Arc::from(data),
        only_header_parsed,
        failed: false,
        stream: if keep_stream { Some(stream) } else { None },
        screen_width: width,
        screen_height: height,
        frames: Vec::new(),
    };

    Ok(Codec::new(
        encoded_info,
        Box::new(imp),
        None,
        origin,
        Some(SkcmsPixelFormat::Bgra8888),
    ))
}
