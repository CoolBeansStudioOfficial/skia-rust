// Copyright 2012 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the COPYING file. Port by The skia-rust Authors.

//! Port of libwebp's demuxer, `src/demux/demux.c` (`WebPDemuxInternal` with and without partial
//! data, `WebPDemuxGetI`, the frame iterators `WebPDemuxGetFrame`/`WebPDemuxNextFrame`/
//! `WebPDemuxPrevFrame`, and the chunk iterators `WebPDemuxGetChunk`/`WebPDemuxNextChunk`/
//! `WebPDemuxPrevChunk`), for the `libwebp@845d5476` pin.
//!
//! The C structures are kept as they are, with the linked lists of frames and chunks stored in
//! `Vec`s in the same order. Iterators hold slices of the input instead of the C pointers.
//! Frame and chunk payloads are the input bytes, so the demuxer borrows its input.

// Module-level clippy allows. Each one mirrors the C source of this module.
// clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::cast_sign_loss: the
// chunk sizes are `uint32_t`, the offsets `size_t`, and the dimensions `int`, converted as in C.
// clippy::many_single_char_names: none, but the C names are kept.
// clippy::struct_excessive_bools: `Frame` holds the C struct's flags.
// clippy::struct_field_names: `Frame::frame_num` is the C member name `frame_num_`.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::struct_excessive_bools,
    clippy::struct_field_names
)]

use crate::io::Status;
use crate::webp_dec;

/// Port of `CHUNK_HEADER_SIZE`.
const CHUNK_HEADER_SIZE: usize = 8;
/// Port of `RIFF_HEADER_SIZE`.
const RIFF_HEADER_SIZE: usize = 12;
/// Port of `TAG_SIZE`.
const TAG_SIZE: usize = 4;
/// Port of `CHUNK_SIZE_BYTES` (the RIFF and WEBP tags).
const CHUNK_SIZE_BYTES: usize = 4;
/// Port of `VP8X_CHUNK_SIZE`.
const VP8X_CHUNK_SIZE: u32 = 10;
/// Port of `ANIM_CHUNK_SIZE`.
const ANIM_CHUNK_SIZE: u32 = 6;
/// Port of `ANMF_CHUNK_SIZE`.
const ANMF_CHUNK_SIZE: u32 = 16;
/// Port of `MAX_CHUNK_PAYLOAD` (`~0U - CHUNK_HEADER_SIZE - 1`).
const MAX_CHUNK_PAYLOAD: u32 = !0u32 - CHUNK_HEADER_SIZE as u32 - 1;
/// Port of `MAX_IMAGE_AREA`.
const MAX_IMAGE_AREA: u64 = 1u64 << 32;
/// Port of the `WebPFeatureFlags` of `webp/mux_types.h`.
const ANIMATION_FLAG: u32 = 0x0000_0002;
const EXIF_FLAG: u32 = 0x0000_0008;
const XMP_FLAG: u32 = 0x0000_0004;
const ALPHA_FLAG: u32 = 0x0000_0010;
const ICCP_FLAG: u32 = 0x0000_0020;
/// Port of `ALL_VALID_FLAGS`.
const ALL_VALID_FLAGS: u32 = 0x0000_003e;

/// Port of `WebPDemuxState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "WebPDemuxState")]
pub enum DemuxState {
    ParseError,
    ParsingHeader,
    ParsedHeader,
    Done,
}

/// Port of `WebPMuxAnimDispose`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "WebPMuxAnimDispose")]
pub enum Dispose {
    None,
    Background,
}

/// Port of `WebPMuxAnimBlend`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "WebPMuxAnimBlend")]
pub enum Blend {
    Blend,
    NoBlend,
}

/// Port of `WebPFormatFeature` (the values `WebPDemuxGetI` takes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "WebPFormatFeature")]
pub enum FormatFeature {
    FormatFlags,
    CanvasWidth,
    CanvasHeight,
    LoopCount,
    BackgroundColor,
    FrameCount,
}

/// Port of `ChunkData`: an offset into the input and a size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ChunkData {
    offset: usize,
    size: usize,
}

/// Port of `Frame`.
#[derive(Debug, Clone, Copy)]
struct Frame {
    x_offset: i32,
    y_offset: i32,
    width: i32,
    height: i32,
    has_alpha: bool,
    duration: i32,
    dispose_method: Dispose,
    blend_method: Blend,
    frame_num: i32,
    complete: bool,
    /// `img_components_`: 0 = VP8{,L}, 1 = ALPH.
    img_components: [ChunkData; 2],
}

impl Frame {
    /// The frame as `calloc` leaves it.
    fn zeroed() -> Self {
        Self {
            x_offset: 0,
            y_offset: 0,
            width: 0,
            height: 0,
            has_alpha: false,
            duration: 0,
            dispose_method: Dispose::None,
            blend_method: Blend::Blend,
            frame_num: 0,
            complete: false,
            img_components: [ChunkData::default(); 2],
        }
    }
}

/// Port of `ParseStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseStatus {
    Ok,
    NeedMoreData,
    Error,
}

/// Port of `MemBuffer`: the input and the window of it that is parsed.
#[derive(Debug, Clone, Copy)]
struct MemBuffer<'a> {
    start: usize,
    end: usize,
    riff_end: usize,
    buf_size: usize,
    buf: &'a [u8],
}

impl<'a> MemBuffer<'a> {
    /// Port of `InitMemBuffer`.
    fn new(buf: &'a [u8]) -> Self {
        Self {
            start: 0,
            end: buf.len(),
            riff_end: 0,
            buf_size: buf.len(),
            buf,
        }
    }

    /// Port of `MemDataSize`.
    fn data_size(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Port of `SizeIsInvalid`.
    fn size_is_invalid(&self, size: usize) -> bool {
        size > self.riff_end.wrapping_sub(self.start)
    }

    fn skip(&mut self, size: usize) {
        self.start = self.start.wrapping_add(size);
    }

    fn rewind(&mut self, size: usize) {
        self.start = self.start.wrapping_sub(size);
    }

    /// Port of `GetBuffer`: the bytes from the current position.
    fn buffer(&self) -> &'a [u8] {
        self.buf.get(self.start..).unwrap_or(&[])
    }

    /// Reads `n` bytes at `off` without a bounds failure (missing bytes read as zero).
    fn bytes_at(&self, off: usize, n: usize) -> [u8; 4] {
        let mut out = [0u8; 4];
        for (i, o) in out.iter_mut().enumerate().take(n) {
            *o = self.buf.get(off + i).copied().unwrap_or(0);
        }
        out
    }

    /// Port of `ReadByte`.
    fn read_byte(&mut self) -> u8 {
        let byte = self.bytes_at(self.start, 1)[0];
        self.skip(1);
        byte
    }

    /// Port of `ReadLE16s`.
    fn read_le16s(&mut self) -> i32 {
        let b = self.bytes_at(self.start, 2);
        let val = i32::from(u16::from_le_bytes([b[0], b[1]]));
        self.skip(2);
        val
    }

    /// Port of `ReadLE24s`.
    fn read_le24s(&mut self) -> i32 {
        let b = self.bytes_at(self.start, 3);
        let val = (i32::from(b[2]) << 16) | (i32::from(b[1]) << 8) | i32::from(b[0]);
        self.skip(3);
        val
    }

    /// Port of `ReadLE32`.
    fn read_le32(&mut self) -> u32 {
        let val = u32::from_le_bytes(self.bytes_at(self.start, 4));
        self.skip(4);
        val
    }
}

/// Port of `GetLE32` over a 4-byte prefix of `d`.
fn get_le32(d: &[u8]) -> u32 {
    u32::from_le_bytes([d[0], d[1], d[2], d[3]])
}

/// The outcome of one chunk of `StoreFrame`'s switch: `Done` is its `goto Done` label, which
/// the default branch shares.
enum Next {
    Continue,
    Done,
}

/// Port of `StoreFrame`: fills `frame` with the image and alpha chunks that follow.
fn store_frame(
    frame_num: i32,
    min_size: u32,
    mem: &mut MemBuffer<'_>,
    frame: &mut Frame,
) -> ParseStatus {
    let mut alpha_chunks = 0;
    let mut image_chunks = 0;
    let mut done = mem.data_size() < CHUNK_HEADER_SIZE || (mem.data_size() < min_size as usize);
    let mut status = ParseStatus::Ok;
    if done {
        return ParseStatus::NeedMoreData;
    }
    loop {
        let chunk_start_offset = mem.start;
        let fourcc = mem.read_le32().to_le_bytes();
        let payload_size = mem.read_le32();
        if payload_size > MAX_CHUNK_PAYLOAD {
            return ParseStatus::Error;
        }
        let payload_size_padded = payload_size + (payload_size & 1);
        let payload_available = if payload_size_padded as usize > mem.data_size() {
            mem.data_size()
        } else {
            payload_size_padded as usize
        };
        let chunk_size = CHUNK_HEADER_SIZE + payload_available;
        if mem.size_is_invalid(payload_size_padded as usize) {
            return ParseStatus::Error;
        }
        if payload_size_padded as usize > mem.data_size() {
            status = ParseStatus::NeedMoreData;
        }
        // The fourcc switch of StoreFrame.
        let next = match &fourcc {
            b"ALPH" => {
                if alpha_chunks == 0 {
                    alpha_chunks += 1;
                    frame.img_components[1] = ChunkData {
                        offset: chunk_start_offset,
                        size: chunk_size,
                    };
                    frame.has_alpha = true;
                    frame.frame_num = frame_num;
                    mem.skip(payload_available);
                    Next::Continue
                } else {
                    Next::Done
                }
            }
            b"VP8L" | b"VP8 " => {
                if fourcc == *b"VP8L" && alpha_chunks > 0 {
                    return ParseStatus::Error; // VP8L has its own alpha
                }
                if image_chunks == 0 {
                    let chunk = mem.buf.get(chunk_start_offset..).unwrap_or(&[]);
                    let chunk = &chunk[..chunk_size.min(chunk.len())];
                    let features = match webp_dec::get_features(chunk) {
                        Ok(f) => f,
                        Err(Status::NotEnoughData) if status == ParseStatus::NeedMoreData => {
                            return ParseStatus::NeedMoreData;
                        }
                        Err(_) => return ParseStatus::Error,
                    };
                    image_chunks += 1;
                    set_frame_info(
                        chunk_start_offset,
                        chunk_size,
                        frame_num,
                        status == ParseStatus::Ok,
                        &features,
                        frame,
                    );
                    mem.skip(payload_available);
                    Next::Continue
                } else {
                    Next::Done
                }
            }
            _ => Next::Done,
        };
        if let Next::Done = next {
            mem.rewind(CHUNK_HEADER_SIZE);
            done = true;
        }
        if mem.start == mem.riff_end {
            done = true;
        } else if mem.data_size() < CHUNK_HEADER_SIZE {
            status = ParseStatus::NeedMoreData;
        }
        if done || status != ParseStatus::Ok {
            break;
        }
    }
    status
}

/// Port of `SetFrameInfo`.
fn set_frame_info(
    start_offset: usize,
    size: usize,
    frame_num: i32,
    complete: bool,
    features: &webp_dec::Features,
    frame: &mut Frame,
) {
    frame.img_components[0] = ChunkData {
        offset: start_offset,
        size,
    };
    frame.width = features.width;
    frame.height = features.height;
    frame.has_alpha |= features.has_alpha;
    frame.frame_num = frame_num;
    frame.complete = complete;
}

/// The frames and non-image chunks of a parsed file, and the header fields.
#[derive(Debug, Clone)]
#[doc(alias = "WebPDemuxer")]
pub struct Demuxer<'a> {
    mem: MemBuffer<'a>,
    state: DemuxState,
    is_ext_format: bool,
    feature_flags: u32,
    canvas_width: i32,
    canvas_height: i32,
    loop_count: i32,
    bgcolor: u32,
    num_frames: i32,
    frames: Vec<Frame>,
    chunks: Vec<ChunkData>,
}

/// Port of `AddFrame`: appends `frame` unless the last frame is incomplete.
fn add_frame(dmux: &mut Demuxer<'_>, frame: Frame) -> bool {
    if dmux.frames.last().is_some_and(|last| !last.complete) {
        return false;
    }
    dmux.frames.push(frame);
    true
}

/// Port of `StoreChunk`.
fn store_chunk(dmux: &mut Demuxer<'_>, start_offset: usize, size: usize) {
    dmux.chunks.push(ChunkData {
        offset: start_offset,
        size,
    });
}

/// Port of `NewFrame`: checks the sizes of an `ANMF` chunk.
fn new_frame(mem: &MemBuffer<'_>, min_size: u32, actual_size: u32) -> ParseStatus {
    if mem.size_is_invalid(min_size as usize) {
        return ParseStatus::Error;
    }
    if actual_size < min_size {
        return ParseStatus::Error;
    }
    if mem.data_size() < min_size as usize {
        return ParseStatus::NeedMoreData;
    }
    ParseStatus::Ok
}

/// Port of `ParseAnimationFrame`.
fn parse_animation_frame(dmux: &mut Demuxer<'_>, frame_chunk_size: u32) -> ParseStatus {
    let is_animation = dmux.feature_flags & ANIMATION_FLAG != 0;
    let anmf_payload_size = frame_chunk_size - ANMF_CHUNK_SIZE;
    let mut added_frame = false;
    let mut mem = dmux.mem;
    let status = new_frame(&mem, ANMF_CHUNK_SIZE, frame_chunk_size);
    if status != ParseStatus::Ok {
        return status;
    }
    let mut frame = Frame::zeroed();
    frame.x_offset = 2 * mem.read_le24s();
    frame.y_offset = 2 * mem.read_le24s();
    frame.width = 1 + mem.read_le24s();
    frame.height = 1 + mem.read_le24s();
    frame.duration = mem.read_le24s();
    let bits = mem.read_byte();
    frame.dispose_method = if bits & 1 != 0 {
        Dispose::Background
    } else {
        Dispose::None
    };
    frame.blend_method = if bits & 2 != 0 {
        Blend::NoBlend
    } else {
        Blend::Blend
    };
    if frame.width as u64 * frame.height as u64 >= MAX_IMAGE_AREA {
        dmux.mem = mem;
        return ParseStatus::Error;
    }
    let start_offset = mem.start;
    let mut status = store_frame(dmux.num_frames + 1, anmf_payload_size, &mut mem, &mut frame);
    if status != ParseStatus::Error && mem.start - start_offset > anmf_payload_size as usize {
        status = ParseStatus::Error;
    }
    if status != ParseStatus::Error && is_animation && frame.frame_num > 0 {
        added_frame = add_frame(dmux, frame);
        if added_frame {
            dmux.num_frames += 1;
        } else {
            status = ParseStatus::Error;
        }
    }
    dmux.mem = mem;
    let _ = added_frame; // a frame that is not added is dropped, as WebPSafeFree does
    status
}

/// Port of `ReadHeader`: checks the RIFF header and narrows the buffer to the RIFF size.
fn read_header(mem: &mut MemBuffer<'_>) -> ParseStatus {
    let min_size = RIFF_HEADER_SIZE + CHUNK_HEADER_SIZE;
    if mem.data_size() < min_size {
        return ParseStatus::NeedMoreData;
    }
    let b = mem.buffer();
    if &b[..CHUNK_SIZE_BYTES] != b"RIFF"
        || &b[CHUNK_HEADER_SIZE..CHUNK_HEADER_SIZE + CHUNK_SIZE_BYTES] != b"WEBP"
    {
        return ParseStatus::Error;
    }
    let riff_size = get_le32(&b[TAG_SIZE..]);
    if riff_size < CHUNK_HEADER_SIZE as u32 {
        return ParseStatus::Error;
    }
    if riff_size > MAX_CHUNK_PAYLOAD {
        return ParseStatus::Error;
    }
    mem.riff_end = riff_size as usize + CHUNK_HEADER_SIZE;
    if mem.buf_size > mem.riff_end {
        mem.buf_size = mem.riff_end;
        mem.end = mem.riff_end;
    }
    mem.skip(RIFF_HEADER_SIZE);
    ParseStatus::Ok
}

/// Port of `ParseSingleImage`.
fn parse_single_image(dmux: &mut Demuxer<'_>) -> ParseStatus {
    let min_size = CHUNK_HEADER_SIZE;
    let mut image_added = false;
    if !dmux.frames.is_empty() {
        return ParseStatus::Error;
    }
    if dmux.mem.size_is_invalid(min_size) {
        return ParseStatus::Error;
    }
    if dmux.mem.data_size() < min_size {
        return ParseStatus::NeedMoreData;
    }
    let mut frame = Frame::zeroed();
    let mut mem = dmux.mem;
    let mut status = store_frame(1, 0, &mut mem, &mut frame);
    dmux.mem = mem;
    if status != ParseStatus::Error {
        let has_alpha = dmux.feature_flags & ALPHA_FLAG != 0;
        if !has_alpha && frame.img_components[1].size > 0 {
            frame.img_components[1] = ChunkData::default();
            frame.has_alpha = false;
        }
        if !dmux.is_ext_format && frame.width > 0 && frame.height > 0 {
            dmux.state = DemuxState::ParsedHeader;
            dmux.canvas_width = frame.width;
            dmux.canvas_height = frame.height;
            if frame.has_alpha {
                dmux.feature_flags |= ALPHA_FLAG;
            }
        }
        if add_frame(dmux, frame) {
            image_added = true;
            dmux.num_frames = 1;
        } else {
            status = ParseStatus::Error; // last frame was left incomplete
        }
    }
    let _ = image_added;
    status
}

/// Port of `ParseVP8XChunks`.
fn parse_vp8x_chunks(dmux: &mut Demuxer<'_>) -> ParseStatus {
    let is_animation = dmux.feature_flags & ANIMATION_FLAG != 0;
    let mut anim_chunks = 0;
    let mut status = ParseStatus::Ok;
    loop {
        let mut store = true;
        let mut mem = dmux.mem;
        let chunk_start_offset = mem.start;
        let fourcc = mem.read_le32().to_le_bytes();
        let chunk_size = mem.read_le32();
        if chunk_size > MAX_CHUNK_PAYLOAD {
            dmux.mem = mem;
            return ParseStatus::Error;
        }
        let chunk_size_padded = chunk_size + (chunk_size & 1);
        if mem.size_is_invalid(chunk_size_padded as usize) {
            dmux.mem = mem;
            return ParseStatus::Error;
        }
        // The switch of ParseVP8XChunks. `skip` is the `Skip:` label, shared with the default.
        let mut skip = false;
        match &fourcc {
            b"VP8X" => {
                dmux.mem = mem;
                return ParseStatus::Error;
            }
            b"ALPH" | b"VP8 " | b"VP8L" => {
                if anim_chunks > 0 || is_animation {
                    dmux.mem = mem;
                    return ParseStatus::Error;
                }
                mem.rewind(CHUNK_HEADER_SIZE);
                dmux.mem = mem;
                status = parse_single_image(dmux);
                mem = dmux.mem;
            }
            b"ANIM" => {
                if chunk_size_padded < ANIM_CHUNK_SIZE {
                    dmux.mem = mem;
                    return ParseStatus::Error;
                }
                if mem.data_size() < chunk_size_padded as usize {
                    status = ParseStatus::NeedMoreData;
                } else if anim_chunks == 0 {
                    anim_chunks += 1;
                    dmux.bgcolor = mem.read_le32();
                    dmux.loop_count = mem.read_le16s();
                    mem.skip(chunk_size_padded as usize - ANIM_CHUNK_SIZE as usize);
                } else {
                    store = false;
                    skip = true;
                }
            }
            b"ANMF" => {
                if anim_chunks == 0 {
                    dmux.mem = mem;
                    return ParseStatus::Error; // 'ANIM' precedes frames.
                }
                dmux.mem = mem;
                status = parse_animation_frame(dmux, chunk_size_padded);
                mem = dmux.mem;
            }
            b"ICCP" => {
                store = dmux.feature_flags & ICCP_FLAG != 0;
                skip = true;
            }
            b"EXIF" => {
                store = dmux.feature_flags & EXIF_FLAG != 0;
                skip = true;
            }
            b"XMP " => {
                store = dmux.feature_flags & XMP_FLAG != 0;
                skip = true;
            }
            _ => skip = true,
        }
        if skip {
            // The `Skip:` label: store the unknown chunk and step over it.
            if chunk_size_padded as usize <= mem.data_size() {
                if store {
                    store_chunk(
                        dmux,
                        chunk_start_offset,
                        CHUNK_HEADER_SIZE + chunk_size as usize,
                    );
                }
                mem.skip(chunk_size_padded as usize);
            } else {
                status = ParseStatus::NeedMoreData;
            }
        }
        dmux.mem = mem;
        if dmux.mem.start == dmux.mem.riff_end {
            break;
        } else if dmux.mem.data_size() < CHUNK_HEADER_SIZE {
            status = ParseStatus::NeedMoreData;
        }
        if status != ParseStatus::Ok {
            break;
        }
    }
    status
}

/// Port of `ParseVP8X`.
fn parse_vp8x(dmux: &mut Demuxer<'_>) -> ParseStatus {
    let mut mem = dmux.mem;
    if mem.data_size() < CHUNK_HEADER_SIZE {
        return ParseStatus::NeedMoreData;
    }
    dmux.is_ext_format = true;
    mem.skip(TAG_SIZE); // VP8X
    let mut vp8x_size = mem.read_le32();
    if vp8x_size > MAX_CHUNK_PAYLOAD {
        return ParseStatus::Error;
    }
    if vp8x_size < VP8X_CHUNK_SIZE {
        return ParseStatus::Error;
    }
    vp8x_size += vp8x_size & 1;
    if mem.size_is_invalid(vp8x_size as usize) {
        return ParseStatus::Error;
    }
    if mem.data_size() < vp8x_size as usize {
        return ParseStatus::NeedMoreData;
    }
    dmux.feature_flags = u32::from(mem.read_byte());
    mem.skip(3); // Reserved.
    dmux.canvas_width = 1 + mem.read_le24s();
    dmux.canvas_height = 1 + mem.read_le24s();
    if dmux.canvas_width as u64 * dmux.canvas_height as u64 >= MAX_IMAGE_AREA {
        return ParseStatus::Error; // image final dimension is too large
    }
    mem.skip((vp8x_size - VP8X_CHUNK_SIZE) as usize); // skip any trailing data.
    dmux.state = DemuxState::ParsedHeader;
    dmux.mem = mem;
    if dmux.mem.size_is_invalid(CHUNK_HEADER_SIZE) {
        return ParseStatus::Error;
    }
    if dmux.mem.data_size() < CHUNK_HEADER_SIZE {
        return ParseStatus::NeedMoreData;
    }
    parse_vp8x_chunks(dmux)
}

/// Port of `IsValidSimpleFormat`.
fn is_valid_simple_format(dmux: &Demuxer<'_>) -> bool {
    if dmux.state == DemuxState::ParsingHeader {
        return true;
    }
    if dmux.canvas_width <= 0 || dmux.canvas_height <= 0 {
        return false;
    }
    let Some(frame) = dmux.frames.first() else {
        return dmux.state != DemuxState::Done;
    };
    if frame.width <= 0 || frame.height <= 0 {
        return false;
    }
    true
}

/// Port of `CheckFrameBounds`.
fn check_frame_bounds(frame: &Frame, exact: bool, canvas_width: i32, canvas_height: i32) -> bool {
    if exact {
        if frame.x_offset != 0 || frame.y_offset != 0 {
            return false;
        }
        if frame.width != canvas_width || frame.height != canvas_height {
            return false;
        }
    } else {
        if frame.x_offset < 0 || frame.y_offset < 0 {
            return false;
        }
        if frame.width + frame.x_offset > canvas_width {
            return false;
        }
        if frame.height + frame.y_offset > canvas_height {
            return false;
        }
    }
    true
}

/// Port of `IsValidExtendedFormat`.
fn is_valid_extended_format(dmux: &Demuxer<'_>) -> bool {
    let is_animation = dmux.feature_flags & ANIMATION_FLAG != 0;
    if dmux.state == DemuxState::ParsingHeader {
        return true;
    }
    if dmux.canvas_width <= 0 || dmux.canvas_height <= 0 {
        return false;
    }
    if dmux.loop_count < 0 {
        return false;
    }
    if dmux.state == DemuxState::Done && dmux.frames.is_empty() {
        return false;
    }
    if dmux.feature_flags & !ALL_VALID_FLAGS != 0 {
        return false; // invalid bitstream
    }
    let mut i = 0;
    while i < dmux.frames.len() {
        let cur_frame_set = dmux.frames[i].frame_num;
        while i < dmux.frames.len() && dmux.frames[i].frame_num == cur_frame_set {
            let f = &dmux.frames[i];
            let image = f.img_components[0];
            let alpha = f.img_components[1];
            if !is_animation && f.frame_num > 1 {
                return false;
            }
            if f.complete {
                if alpha.size == 0 && image.size == 0 {
                    return false;
                }
                if alpha.size > 0 && alpha.offset > image.offset {
                    return false;
                }
                if f.width <= 0 || f.height <= 0 {
                    return false;
                }
            } else {
                if dmux.state == DemuxState::Done {
                    return false;
                }
                if alpha.size > 0 && image.size > 0 && alpha.offset > image.offset {
                    return false;
                }
                // `f->next_ != NULL`: an incomplete frame must be the last one.
                if i + 1 < dmux.frames.len() {
                    return false;
                }
            }
            if f.width > 0
                && f.height > 0
                && !check_frame_bounds(f, !is_animation, dmux.canvas_width, dmux.canvas_height)
            {
                return false;
            }
            i += 1;
        }
    }
    true
}

/// Port of `CreateRawImageDemuxer`: a bare VP8 or VP8L bitstream, with no RIFF header.
fn create_raw_image_demuxer<'a>(mem: &MemBuffer<'a>) -> Result<Demuxer<'a>, ParseStatus> {
    let features = match webp_dec::get_features(mem.buf) {
        Ok(f) => f,
        Err(Status::NotEnoughData) => return Err(ParseStatus::NeedMoreData),
        Err(_) => return Err(ParseStatus::Error),
    };
    let mut dmux = Demuxer::init(*mem);
    let mut frame = Frame::zeroed();
    set_frame_info(0, mem.buf_size, 1, true, &features, &mut frame);
    if !add_frame(&mut dmux, frame) {
        return Err(ParseStatus::Error);
    }
    dmux.state = DemuxState::Done;
    dmux.canvas_width = frame.width;
    dmux.canvas_height = frame.height;
    if frame.has_alpha {
        dmux.feature_flags |= ALPHA_FLAG;
    }
    dmux.num_frames = 1;
    Ok(dmux)
}

impl<'a> Demuxer<'a> {
    /// Port of `InitDemux`.
    fn init(mem: MemBuffer<'a>) -> Self {
        Self {
            mem,
            state: DemuxState::ParsingHeader,
            is_ext_format: false,
            feature_flags: 0,
            canvas_width: -1,
            canvas_height: -1,
            loop_count: 1,
            bgcolor: 0xffff_ffff, // White background by default.
            num_frames: 0,
            frames: Vec::new(),
            chunks: Vec::new(),
        }
    }

    /// Port of `WebPDemuxGetI`.
    #[doc(alias = "WebPDemuxGetI")]
    #[must_use]
    pub fn get_i(&self, feature: FormatFeature) -> u32 {
        match feature {
            FormatFeature::FormatFlags => self.feature_flags,
            FormatFeature::CanvasWidth => self.canvas_width as u32,
            FormatFeature::CanvasHeight => self.canvas_height as u32,
            FormatFeature::LoopCount => self.loop_count as u32,
            FormatFeature::BackgroundColor => self.bgcolor,
            FormatFeature::FrameCount => self.num_frames as u32,
        }
    }

    /// The parse state (`WebPDemuxState`) of this demuxer.
    #[must_use]
    pub fn state(&self) -> DemuxState {
        self.state
    }

    /// Port of `GetFrame`: the frame numbered `frame_num`, if it was parsed.
    fn find_frame(&self, frame_num: i32) -> Option<&Frame> {
        self.frames.iter().find(|f| f.frame_num == frame_num)
    }

    /// Port of `GetFramePayload`: the bytes of the frame (its alpha chunk, its image chunk, and
    /// the bytes between them).
    fn frame_payload(&self, frame: &Frame) -> &'a [u8] {
        let image = frame.img_components[0];
        let alpha = frame.img_components[1];
        let mut start_offset = image.offset;
        let mut data_size = image.size;
        if alpha.size > 0 {
            let inter_size = if image.offset > 0 {
                image.offset.saturating_sub(alpha.offset + alpha.size)
            } else {
                0
            };
            start_offset = alpha.offset;
            data_size += alpha.size + inter_size;
        }
        let buf = self.mem.buf;
        start_offset
            .checked_add(data_size)
            .and_then(|end| buf.get(start_offset..end))
            .unwrap_or(&[])
    }

    /// Port of `WebPDemuxGetFrame`: the frame `frame_num` (1-based; 0 is the last one).
    #[doc(alias = "WebPDemuxGetFrame")]
    #[must_use]
    pub fn get_frame(&self, frame_num: i32) -> Option<FrameIter<'_, 'a>> {
        self.set_frame(frame_num)
    }

    /// Port of `SetFrame`.
    fn set_frame(&self, mut frame_num: i32) -> Option<FrameIter<'_, 'a>> {
        if frame_num < 0 {
            return None;
        }
        if frame_num > self.num_frames {
            return None;
        }
        if frame_num == 0 {
            frame_num = self.num_frames;
        }
        let frame = self.find_frame(frame_num)?;
        Some(FrameIter {
            dmux: self,
            frame_num: frame.frame_num,
            num_frames: self.num_frames,
            x_offset: frame.x_offset,
            y_offset: frame.y_offset,
            width: frame.width,
            height: frame.height,
            duration: frame.duration,
            dispose_method: frame.dispose_method,
            blend_method: frame.blend_method,
            complete: frame.complete,
            has_alpha: frame.has_alpha,
            fragment: self.frame_payload(frame),
        })
    }

    /// Port of `WebPDemuxGetChunk`: the `chunk_num`-th chunk with the `FourCC` `fourcc` (1-based;
    /// 0 is the last one).
    #[doc(alias = "WebPDemuxGetChunk")]
    #[must_use]
    pub fn get_chunk(&self, fourcc: [u8; 4], chunk_num: i32) -> Option<ChunkIterator<'_, 'a>> {
        self.set_chunk(fourcc, chunk_num)
    }

    /// Port of `ChunkCount`.
    fn chunk_count(&self, fourcc: [u8; 4]) -> i32 {
        self.chunks
            .iter()
            .filter(|c| self.mem.buf.get(c.offset..c.offset + TAG_SIZE) == Some(&fourcc[..]))
            .count() as i32
    }

    /// Port of `SetChunk`.
    fn set_chunk(&self, fourcc: [u8; 4], mut chunk_num: i32) -> Option<ChunkIterator<'_, 'a>> {
        if chunk_num < 0 {
            return None;
        }
        let count = self.chunk_count(fourcc);
        if count == 0 {
            return None;
        }
        if chunk_num == 0 {
            chunk_num = count;
        }
        if chunk_num > count {
            return None;
        }
        // Port of GetChunk: the chunk_num-th match.
        let mut seen = 0;
        let mut found = None;
        for c in &self.chunks {
            if self.mem.buf.get(c.offset..c.offset + TAG_SIZE) == Some(&fourcc[..]) {
                seen += 1;
            }
            if seen == chunk_num {
                found = Some(*c);
                break;
            }
        }
        let c = found?;
        let start = c.offset + CHUNK_HEADER_SIZE;
        let end = c.offset + c.size;
        Some(ChunkIterator {
            dmux: self,
            fourcc,
            chunk: self.mem.buf.get(start..end).unwrap_or(&[]),
            num_chunks: count,
            chunk_num,
        })
    }
}

/// Port of `WebPIterator`: one frame of a demuxed file.
#[derive(Debug, Clone, Copy)]
#[doc(alias = "WebPIterator")]
pub struct FrameIter<'d, 'a> {
    dmux: &'d Demuxer<'a>,
    /// 1-based frame number.
    pub frame_num: i32,
    /// `WEBP_FF_FRAME_COUNT`.
    pub num_frames: i32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub width: i32,
    pub height: i32,
    /// Display duration in milliseconds.
    pub duration: i32,
    pub dispose_method: Dispose,
    /// True if `fragment` holds a full frame; partial images may still be decoded incrementally.
    pub complete: bool,
    /// The frame's bytes (`fragment` in C): the image and alpha chunks of the frame.
    pub fragment: &'a [u8],
    pub has_alpha: bool,
    pub blend_method: Blend,
}

impl<'d, 'a> FrameIter<'d, 'a> {
    /// Port of `WebPDemuxNextFrame`.
    #[doc(alias = "WebPDemuxNextFrame")]
    #[must_use]
    pub fn next_frame(&self) -> Option<FrameIter<'d, 'a>> {
        self.dmux.set_frame(self.frame_num + 1)
    }

    /// Port of `WebPDemuxPrevFrame`.
    #[doc(alias = "WebPDemuxPrevFrame")]
    #[must_use]
    pub fn prev_frame(&self) -> Option<FrameIter<'d, 'a>> {
        if self.frame_num <= 1 {
            return None;
        }
        self.dmux.set_frame(self.frame_num - 1)
    }
}

/// Port of `WebPChunkIterator`: one chunk (other than image chunks) of a demuxed file.
#[derive(Debug, Clone, Copy)]
#[doc(alias = "WebPChunkIterator")]
pub struct ChunkIterator<'d, 'a> {
    dmux: &'d Demuxer<'a>,
    fourcc: [u8; 4],
    /// The chunk's payload (`chunk` in C, without its header).
    pub chunk: &'a [u8],
    pub num_chunks: i32,
    /// 1-based chunk number.
    pub chunk_num: i32,
}

impl<'d, 'a> ChunkIterator<'d, 'a> {
    /// Port of `WebPDemuxNextChunk`.
    #[doc(alias = "WebPDemuxNextChunk")]
    #[must_use]
    pub fn next_chunk(&self) -> Option<ChunkIterator<'d, 'a>> {
        self.dmux.set_chunk(self.fourcc, self.chunk_num + 1)
    }

    /// Port of `WebPDemuxPrevChunk`.
    #[doc(alias = "WebPDemuxPrevChunk")]
    #[must_use]
    pub fn prev_chunk(&self) -> Option<ChunkIterator<'d, 'a>> {
        if self.chunk_num <= 1 {
            return None;
        }
        self.dmux.set_chunk(self.fourcc, self.chunk_num - 1)
    }
}

/// Port of `WebPDemuxInternal`: parses `data`. With `allow_partial`, truncated input gives a
/// demuxer in an incomplete state (`ParsingHeader` or `ParsedHeader`). Returns the demuxer, if
/// any, and the state the C function reports.
#[doc(alias = "WebPDemuxInternal")]
#[must_use]
pub fn demux_internal(data: &[u8], allow_partial: bool) -> (Option<Demuxer<'_>>, DemuxState) {
    if data.is_empty() {
        return (None, DemuxState::ParseError);
    }
    let mut mem = MemBuffer::new(data);
    let mut status = read_header(&mut mem);
    if status != ParseStatus::Ok {
        if status == ParseStatus::Error {
            match create_raw_image_demuxer(&mem) {
                Ok(dmux) => return (Some(dmux), DemuxState::Done),
                Err(s) => status = s,
            }
        }
        let state = if status == ParseStatus::NeedMoreData {
            DemuxState::ParsingHeader
        } else {
            DemuxState::ParseError
        };
        return (None, state);
    }
    let partial = mem.buf_size < mem.riff_end;
    if !allow_partial && partial {
        return (None, DemuxState::ParseError);
    }
    let mut dmux = Demuxer::init(mem);
    status = ParseStatus::Error;
    let tag = dmux.mem.buffer().get(..TAG_SIZE).unwrap_or(&[]).to_vec();
    let parse: Option<fn(&mut Demuxer<'_>) -> ParseStatus> = match tag.as_slice() {
        b"VP8 " | b"VP8L" => Some(parse_single_image),
        b"VP8X" => Some(parse_vp8x),
        _ => None,
    };
    if let Some(parse) = parse {
        status = parse(&mut dmux);
        if status == ParseStatus::Ok {
            dmux.state = DemuxState::Done;
        }
        if status == ParseStatus::NeedMoreData && !partial {
            status = ParseStatus::Error;
        }
        let valid = if tag.as_slice() == b"VP8X" {
            is_valid_extended_format(&dmux)
        } else {
            is_valid_simple_format(&dmux)
        };
        if status != ParseStatus::Error && !valid {
            status = ParseStatus::Error;
        }
        if status == ParseStatus::Error {
            dmux.state = DemuxState::ParseError;
        }
    }
    let state = dmux.state;
    if status == ParseStatus::Error {
        return (None, state);
    }
    (Some(dmux), state)
}

/// Port of `WebPDemux`: parses a complete file.
#[doc(alias = "WebPDemux")]
#[must_use]
pub fn demux(data: &[u8]) -> Option<Demuxer<'_>> {
    demux_internal(data, false).0
}
