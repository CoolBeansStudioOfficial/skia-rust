// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the WebP muxer of libwebp 1.4.0 (`src/mux/muxinternal.c`, `muxedit.c` and `muxread.c`
//! at `845d5476`), the parts `SkWebpEncoder` and the animation encoder reach: parsing a WebP
//! file into chunks (`WebPMuxCreate`), setting the image, the chunks (`ICCP`, `EXIF`, `XMP`,
//! `ANIM`, `VP8X`), pushing animation frames, the canvas size, validation, and the assembly of
//! the RIFF file (`WebPMuxAssemble`).
//!
//! The C object is a set of linked lists. Here each list is a `Vec` in the same order, so the
//! emitted bytes are the same: a list whose C `ChunkSetHead` would fail on a non-empty head is
//! checked the same way, and the C `ChunkAppend` is a `push`. Chunk data is always copied; the
//! C `copy_data` flag only decides whether the caller's buffer is referenced, which does not
//! change the output.

// Clippy allows for the C arithmetic: the 24-bit and 32-bit little-endian conversions mirror
// `PutLE24`, `GetLE24` and friends, whose casts are the width changes of the C source.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::too_many_lines,
    clippy::struct_excessive_bools
)]

pub mod anim_encode;

use crate::vp8_dec;
use crate::vp8l;

/// Port of `MKFOURCC`: the tag as the little-endian word of its four bytes.
const fn mkfourcc(a: u8, b: u8, c: u8, d: u8) -> u32 {
    (a as u32) | ((b as u32) << 8) | ((c as u32) << 16) | ((d as u32) << 24)
}

/// Port of `kChunks[].tag` (muxinternal.c#L21-L31).
const TAG_VP8X: u32 = mkfourcc(b'V', b'P', b'8', b'X');
const TAG_ICCP: u32 = mkfourcc(b'I', b'C', b'C', b'P');
const TAG_ANIM: u32 = mkfourcc(b'A', b'N', b'I', b'M');
const TAG_ANMF: u32 = mkfourcc(b'A', b'N', b'M', b'F');
const TAG_ALPH: u32 = mkfourcc(b'A', b'L', b'P', b'H');
const TAG_VP8: u32 = mkfourcc(b'V', b'P', b'8', b' ');
const TAG_VP8L: u32 = mkfourcc(b'V', b'P', b'8', b'L');
const TAG_EXIF: u32 = mkfourcc(b'E', b'X', b'I', b'F');
const TAG_XMP: u32 = mkfourcc(b'X', b'M', b'P', b' ');
/// Port of `NIL_TAG`: matches any tag in `CountChunks`.
const NIL_TAG: u32 = 0;

/// Port of `CHUNK_HEADER_SIZE` (format_constants.h).
const CHUNK_HEADER_SIZE: usize = 8;
/// Port of `RIFF_HEADER_SIZE`.
const RIFF_HEADER_SIZE: usize = 12;
/// Port of `TAG_SIZE`.
const TAG_SIZE: usize = 4;
/// Port of `VP8X_CHUNK_SIZE`, `ANIM_CHUNK_SIZE` and `ANMF_CHUNK_SIZE`.
const VP8X_CHUNK_SIZE: usize = 10;
const ANIM_CHUNK_SIZE: usize = 6;
const ANMF_CHUNK_SIZE: usize = 16;
/// Port of `MAX_CHUNK_PAYLOAD` (`~0U - CHUNK_HEADER_SIZE - 1`).
const MAX_CHUNK_PAYLOAD: u64 = (u32::MAX as u64) - CHUNK_HEADER_SIZE as u64 - 1;
/// Port of `MAX_CANVAS_SIZE`, `MAX_IMAGE_AREA`, `MAX_LOOP_COUNT`, `MAX_DURATION` and
/// `MAX_POSITION_OFFSET` (format_constants.h#L77-L81).
const MAX_CANVAS_SIZE: i64 = 1 << 24;
const MAX_IMAGE_AREA: u64 = 1 << 32;
const MAX_LOOP_COUNT: i32 = 1 << 16;
const MAX_DURATION: i32 = 1 << 24;
const MAX_POSITION_OFFSET: i32 = 1 << 24;

/// Port of the `VP8X` flag bits (`ICCP_FLAG` and friends in muxinternal.h / muxread.c).
const ANIMATION_FLAG: u32 = 0x0000_0002;
const XMP_FLAG: u32 = 0x0000_0004;
const EXIF_FLAG: u32 = 0x0000_0008;
const ALPHA_FLAG: u32 = 0x0000_0010;
const ICCP_FLAG: u32 = 0x0000_0020;

/// Port of `WebPMuxError`, as the error codes the port returns (`WEBP_MUX_OK` is `Ok`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "WebPMuxError")]
pub enum MuxError {
    /// Port of `WEBP_MUX_NOT_FOUND`.
    NotFound,
    /// Port of `WEBP_MUX_INVALID_ARGUMENT`.
    InvalidArgument,
    /// Port of `WEBP_MUX_BAD_DATA`.
    BadData,
    /// Port of `WEBP_MUX_NOT_ENOUGH_DATA`.
    NotEnoughData,
}

/// Port of `WebPChunkId` (mux_types.h).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkId {
    /// Port of `WEBP_CHUNK_VP8X`.
    Vp8x,
    /// Port of `WEBP_CHUNK_ICCP`.
    Iccp,
    /// Port of `WEBP_CHUNK_ANIM`.
    Anim,
    /// Port of `WEBP_CHUNK_ANMF`.
    Anmf,
    /// Port of `WEBP_CHUNK_ALPHA`.
    Alpha,
    /// Port of `WEBP_CHUNK_IMAGE` (`VP8 ` and `VP8L`).
    Image,
    /// Port of `WEBP_CHUNK_EXIF`.
    Exif,
    /// Port of `WEBP_CHUNK_XMP`.
    Xmp,
    /// Port of `WEBP_CHUNK_UNKNOWN`.
    Unknown,
}

/// Port of `ChunkGetIdFromTag` (muxinternal.c#L73-L79).
fn chunk_id_from_tag(tag: u32) -> ChunkId {
    match tag {
        TAG_VP8X => ChunkId::Vp8x,
        TAG_ICCP => ChunkId::Iccp,
        TAG_ANIM => ChunkId::Anim,
        TAG_ANMF => ChunkId::Anmf,
        TAG_ALPH => ChunkId::Alpha,
        TAG_VP8 | TAG_VP8L => ChunkId::Image,
        TAG_EXIF => ChunkId::Exif,
        TAG_XMP => ChunkId::Xmp,
        _ => ChunkId::Unknown,
    }
}

/// Port of `IsWPI`: the ids that belong to an image (`ANMF`, `ALPHA`, `IMAGE`).
fn is_wpi(id: ChunkId) -> bool {
    matches!(id, ChunkId::Anmf | ChunkId::Alpha | ChunkId::Image)
}

/// Port of the `tag` argument of `CountChunks`: the tag of the list of `id`, or `NIL_TAG` for the
/// unknown chunks (any tag).
fn list_tag(id: ChunkId) -> u32 {
    match id {
        ChunkId::Vp8x => TAG_VP8X,
        ChunkId::Iccp => TAG_ICCP,
        ChunkId::Anim => TAG_ANIM,
        ChunkId::Exif => TAG_EXIF,
        ChunkId::Xmp => TAG_XMP,
        _ => NIL_TAG,
    }
}

/// Port of `SizeWithPadding` / `ChunkDiskSize`: a chunk header plus the data, padded to even.
fn chunk_disk_size(data_size: usize) -> usize {
    CHUNK_HEADER_SIZE + ((data_size + 1) & !1)
}

fn get_le16(d: &[u8]) -> u32 {
    u32::from(d[0]) | (u32::from(d[1]) << 8)
}

fn get_le24(d: &[u8]) -> u32 {
    get_le16(d) | (u32::from(d[2]) << 16)
}

fn get_le32(d: &[u8]) -> u32 {
    get_le24(d) | (u32::from(d[3]) << 24)
}

fn put_le16(dst: &mut Vec<u8>, v: u32) {
    dst.push(v as u8);
    dst.push((v >> 8) as u8);
}

fn put_le24(dst: &mut Vec<u8>, v: u32) {
    put_le16(dst, v);
    dst.push((v >> 16) as u8);
}

fn put_le32(dst: &mut Vec<u8>, v: u32) {
    put_le24(dst, v);
    dst.push((v >> 24) as u8);
}

/// Port of `WebPChunk`: a tag and its data (the `next_` link is the position in a `Vec`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Chunk {
    tag: u32,
    data: Vec<u8>,
}

impl Chunk {
    /// Port of `ChunkDiskSize`.
    fn disk_size(&self) -> usize {
        chunk_disk_size(self.data.len())
    }

    /// Port of `ChunkEmit`: the header, the data and a pad byte when the data is odd.
    fn emit(&self, dst: &mut Vec<u8>) {
        put_le32(dst, self.tag);
        put_le32(dst, self.data.len() as u32);
        dst.extend_from_slice(&self.data);
        if self.data.len() & 1 != 0 {
            dst.push(0);
        }
    }
}

/// Port of `ChunkListDiskSize`.
fn list_disk_size(list: &[Chunk]) -> usize {
    list.iter().map(Chunk::disk_size).sum()
}

/// Port of `ChunkListEmit`.
fn list_emit(list: &[Chunk], dst: &mut Vec<u8>) {
    for chunk in list {
        chunk.emit(dst);
    }
}

/// Port of `ChunkSetHead`: the chunk becomes the head of a list, which must be empty.
fn set_head(chunk: Chunk, list: &mut Vec<Chunk>) -> Result<(), MuxError> {
    if !list.is_empty() {
        return Err(MuxError::NotFound);
    }
    list.push(chunk);
    Ok(())
}

/// Port of `WebPMuxImage`: the chunks of one image or animation frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MuxImage {
    /// The `ANMF` chunk of a frame (`header_`).
    header: Option<Chunk>,
    /// The `ALPH` chunk (`alpha_`).
    alpha: Option<Chunk>,
    /// The `VP8 ` or `VP8L` chunk (`img_`).
    img: Option<Chunk>,
    /// Unknown chunks inside an `ANMF` frame (`unknown_`).
    unknown: Vec<Chunk>,
    width: i32,
    height: i32,
    has_alpha: bool,
    is_partial: bool,
}

/// Port of `MuxImageDiskSize`.
fn image_disk_size(wpi: &MuxImage) -> usize {
    let mut size = 0;
    if let Some(h) = &wpi.header {
        size += h.disk_size();
    }
    if let Some(a) = &wpi.alpha {
        size += a.disk_size();
    }
    if let Some(i) = &wpi.img {
        size += i.disk_size();
    }
    size += list_disk_size(&wpi.unknown);
    size
}

/// Port of `MuxImageEmit`: the `ANMF` header (whose size covers the whole frame), then the `ALPH`,
/// the image and the unknown chunks.
fn image_emit(wpi: &MuxImage, dst: &mut Vec<u8>) {
    if let Some(header) = &wpi.header {
        // Port of `ChunkEmitSpecial`: the size field is the offset to the next chunk.
        let total_size = image_disk_size(wpi);
        put_le32(dst, header.tag);
        put_le32(dst, (total_size - CHUNK_HEADER_SIZE) as u32);
        dst.extend_from_slice(&header.data);
        if header.data.len() & 1 != 0 {
            dst.push(0);
        }
    }
    if let Some(alpha) = &wpi.alpha {
        alpha.emit(dst);
    }
    if let Some(img) = &wpi.img {
        img.emit(dst);
    }
    list_emit(&wpi.unknown, dst);
}

/// Port of `ChunkInfo` lookups for the `WebPMuxFrameInfo` id: `ANMF` or `IMAGE` (a still image).
///
/// Port of `WebPMuxFrameInfo` (mux_types.h): the frame's bitstream, offsets, duration, dispose and
/// blend methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameInfo<'a> {
    /// The bitstream of the frame: a WebP file or a raw `VP8`/`VP8L` bitstream.
    pub bitstream: &'a [u8],
    /// Frame offsets; they are snapped to even values as `WebPMuxPushFrame` does.
    pub x_offset: i32,
    pub y_offset: i32,
    /// Frame duration in milliseconds.
    pub duration: i32,
    /// `ChunkId::Anmf` for animation frames.
    pub id: ChunkId,
    pub dispose: Dispose,
    pub blend: Blend,
}

/// Port of `WebPMuxAnimDispose`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dispose {
    /// Port of `WEBP_MUX_DISPOSE_NONE`.
    #[default]
    None,
    /// Port of `WEBP_MUX_DISPOSE_BACKGROUND`.
    Background,
}

/// Port of `WebPMuxAnimBlend`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Blend {
    /// Port of `WEBP_MUX_BLEND`.
    #[default]
    Blend,
    /// Port of `WEBP_MUX_NO_BLEND`.
    NoBlend,
}

/// Port of `WebPMuxAnimParams`: the background colour and the loop count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimParams {
    pub bgcolor: u32,
    pub loop_count: i32,
}

/// Port of `WebPMux`: the file's chunk lists and the images, in the order the C object keeps them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[doc(alias = "WebPMux")]
pub struct Mux {
    images: Vec<MuxImage>,
    iccp: Vec<Chunk>,
    exif: Vec<Chunk>,
    xmp: Vec<Chunk>,
    anim: Vec<Chunk>,
    vp8x: Vec<Chunk>,
    unknown: Vec<Chunk>,
    canvas_width: i32,
    canvas_height: i32,
}

/// Port of `MuxImageFinalize`: reads the size and alpha of the image chunk, and drops the `ALPH`
/// chunk of a lossless image. Returns `false` where the bitstream is not valid.
fn mux_image_finalize(wpi: &mut MuxImage) -> bool {
    let Some(img) = wpi.img.as_ref() else {
        return false;
    };
    let is_lossless = img.tag == TAG_VP8L;
    let info = if is_lossless {
        vp8l::get_info(&img.data)
    } else {
        vp8_dec::get_info(&img.data, img.data.len()).map(|(w, h)| (w, h, false))
    };
    let Some((w, h, vp8l_has_alpha)) = info else {
        return false;
    };
    // Ignore an ALPH chunk accompanying VP8L.
    if is_lossless && wpi.alpha.is_some() {
        wpi.alpha = None;
    }
    wpi.width = w;
    wpi.height = h;
    wpi.has_alpha = vp8l_has_alpha || wpi.alpha.is_some();
    true
}

/// Port of `ChunkVerifyAndAssign`: the chunk at the start of `data` (`data_size` bytes are left,
/// and the RIFF payload holds `riff_size` bytes).
fn chunk_verify_and_assign(
    data: &[u8],
    riff_size: usize,
) -> Result<Chunk, MuxError> {
    if data.len() < CHUNK_HEADER_SIZE {
        return Err(MuxError::NotEnoughData);
    }
    let chunk_size = u64::from(get_le32(&data[TAG_SIZE..]));
    if chunk_size > MAX_CHUNK_PAYLOAD {
        return Err(MuxError::BadData);
    }
    let chunk_disk = chunk_disk_size(chunk_size as usize);
    if chunk_disk > riff_size {
        return Err(MuxError::BadData);
    }
    if chunk_disk > data.len() {
        return Err(MuxError::NotEnoughData);
    }
    Ok(Chunk {
        tag: get_le32(&data[0..]),
        data: data[CHUNK_HEADER_SIZE..CHUNK_HEADER_SIZE + chunk_size as usize].to_vec(),
    })
}

/// Port of `MuxImageParse`: an `ANMF` frame, with its `ALPH`, image and unknown chunks.
fn mux_image_parse(chunk: &Chunk) -> Option<MuxImage> {
    let bytes = &chunk.data;
    let mut wpi = MuxImage::default();
    // The ANMF header is the first ANMF_CHUNK_SIZE bytes of the chunk data.
    if bytes.len() < ANMF_CHUNK_SIZE {
        return None;
    }
    let header = Chunk {
        tag: chunk.tag,
        data: bytes[..ANMF_CHUNK_SIZE].to_vec(),
    };
    set_head_opt(header, &mut wpi.header).ok()?;
    wpi.is_partial = true;

    // Rest of the chunks.
    let mut pos = chunk_disk_size(ANMF_CHUNK_SIZE) - CHUNK_HEADER_SIZE;
    while pos != bytes.len() {
        let remaining = &bytes[pos..];
        let sub = chunk_verify_and_assign(remaining, remaining.len()).ok()?;
        let sub_size = sub.disk_size();
        match chunk_id_from_tag(sub.tag) {
            ChunkId::Alpha => {
                if wpi.alpha.is_some() {
                    return None;
                }
                wpi.alpha = Some(sub);
                wpi.is_partial = true;
            }
            ChunkId::Image => {
                if wpi.img.is_some() {
                    return None;
                }
                wpi.img = Some(sub);
                if !mux_image_finalize(&mut wpi) {
                    return None;
                }
                wpi.is_partial = false;
            }
            ChunkId::Unknown => {
                if wpi.is_partial {
                    return None;
                }
                wpi.unknown.push(sub);
            }
            _ => return None,
        }
        pos += sub_size;
    }
    if wpi.is_partial {
        return None;
    }
    Some(wpi)
}

/// `set_head` for a single optional chunk slot (`ChunkSetHead` on a `WebPChunk*`).
fn set_head_opt(chunk: Chunk, slot: &mut Option<Chunk>) -> Result<(), MuxError> {
    if slot.is_some() {
        return Err(MuxError::NotFound);
    }
    *slot = Some(chunk);
    Ok(())
}

/// Port of `MuxValidate`'s `ValidateChunk` feature test: `IsNotCompatible`.
fn is_not_compatible(feature: u32, num_items: usize) -> bool {
    (feature != 0) != (num_items > 0)
}

impl Mux {
    /// Port of `WebPMuxNew`.
    #[must_use]
    #[doc(alias = "WebPMuxNew")]
    pub fn new() -> Self {
        Self::default()
    }

    /// The chunk list of `id`, as `MuxGetChunkListFromId` picks it.
    fn list_mut(&mut self, id: ChunkId) -> &mut Vec<Chunk> {
        match id {
            ChunkId::Vp8x => &mut self.vp8x,
            ChunkId::Iccp => &mut self.iccp,
            ChunkId::Anim => &mut self.anim,
            ChunkId::Exif => &mut self.exif,
            ChunkId::Xmp => &mut self.xmp,
            _ => &mut self.unknown,
        }
    }

    fn list(&self, id: ChunkId) -> &Vec<Chunk> {
        match id {
            ChunkId::Vp8x => &self.vp8x,
            ChunkId::Iccp => &self.iccp,
            ChunkId::Anim => &self.anim,
            ChunkId::Exif => &self.exif,
            ChunkId::Xmp => &self.xmp,
            _ => &self.unknown,
        }
    }

    /// Port of `MuxImageCount`: the number of images that hold a chunk of `id`.
    fn image_count(&self, id: ChunkId) -> usize {
        self.images
            .iter()
            .filter(|wpi| {
                let chunk = match id {
                    ChunkId::Anmf => wpi.header.as_ref(),
                    ChunkId::Alpha => wpi.alpha.as_ref(),
                    ChunkId::Image => wpi.img.as_ref(),
                    _ => None,
                };
                chunk.is_some_and(|c| chunk_id_from_tag(c.tag) == id)
            })
            .count()
    }

    /// Port of `WebPMuxNumChunks`.
    fn num_chunks(&self, id: ChunkId) -> usize {
        if is_wpi(id) {
            return self.image_count(id);
        }
        let tag = list_tag(id);
        self.list(id)
            .iter()
            .filter(|c| tag == NIL_TAG || c.tag == tag)
            .count()
    }

    /// Port of `MuxDeleteAllNamedData` / `DeleteChunks`: removes every chunk with the tag. Returns
    /// `NotFound` when there was none.
    fn delete_all_named(&mut self, tag: u32) -> Result<(), MuxError> {
        let id = chunk_id_from_tag(tag);
        if is_wpi(id) {
            return Err(MuxError::InvalidArgument);
        }
        let list = self.list_mut(id);
        let before = list.len();
        list.retain(|c| c.tag != tag);
        if list.len() == before {
            Err(MuxError::NotFound)
        } else {
            Ok(())
        }
    }

    /// Port of `WebPMuxCreate`: parses a single-image or animated WebP file (RIFF). Returns `None`
    /// for data that is not a valid WebP file.
    #[must_use]
    #[doc(alias = "WebPMuxCreate")]
    pub fn create(data: &[u8]) -> Option<Self> {
        if data.len() < RIFF_HEADER_SIZE + CHUNK_HEADER_SIZE {
            return None;
        }
        if get_le32(&data[0..]) != mkfourcc(b'R', b'I', b'F', b'F')
            || get_le32(&data[CHUNK_HEADER_SIZE..]) != mkfourcc(b'W', b'E', b'B', b'P')
        {
            return None;
        }
        let mut mux = Self::new();

        let tag = get_le32(&data[RIFF_HEADER_SIZE..]);
        if tag != TAG_VP8 && tag != TAG_VP8L && tag != TAG_VP8X {
            return None; // First chunk should be VP8, VP8L or VP8X.
        }

        let mut riff_size = u64::from(get_le32(&data[TAG_SIZE..]));
        if riff_size > MAX_CHUNK_PAYLOAD {
            return None;
        }
        // Note this padding is historical and differs from demux.c which does not pad the size.
        riff_size = (CHUNK_HEADER_SIZE as u64) + ((riff_size + 1) & !1);
        if riff_size < CHUNK_HEADER_SIZE as u64 {
            return None;
        }
        if riff_size > data.len() as u64 {
            return None;
        }
        // There's no point in reading past the end of the RIFF chunk.
        let mut size = data.len();
        if size as u64 > riff_size + CHUNK_HEADER_SIZE as u64 {
            size = (riff_size + CHUNK_HEADER_SIZE as u64) as usize;
        }
        let riff_size = riff_size as usize;
        let buf = &data[..size];

        let mut wpi = MuxImage::default();
        let mut pos = RIFF_HEADER_SIZE;
        while pos != buf.len() {
            let chunk = chunk_verify_and_assign(&buf[pos..], riff_size).ok()?;
            let data_size = chunk.disk_size();
            match chunk_id_from_tag(chunk.tag) {
                ChunkId::Alpha => {
                    if wpi.alpha.is_some() {
                        return None; // Consecutive ALPH chunks.
                    }
                    wpi.alpha = Some(chunk);
                    wpi.is_partial = true; // Waiting for a VP8 chunk.
                }
                ChunkId::Image => {
                    set_head_opt(chunk, &mut wpi.img).ok()?;
                    if !mux_image_finalize(&mut wpi) {
                        return None;
                    }
                    wpi.is_partial = false; // wpi is completely filled.
                    mux.images.push(std::mem::take(&mut wpi));
                }
                ChunkId::Anmf => {
                    if wpi.is_partial {
                        return None; // Previous wpi is still incomplete.
                    }
                    wpi = mux_image_parse(&chunk)?;
                    mux.images.push(std::mem::take(&mut wpi));
                }
                id => {
                    // A non-image chunk.
                    if wpi.is_partial {
                        return None;
                    }
                    if id == ChunkId::Vp8x {
                        // Grab the global specs.
                        if data_size < CHUNK_HEADER_SIZE + VP8X_CHUNK_SIZE {
                            return None;
                        }
                        mux.canvas_width = get_le24(&buf[pos + 12..]) as i32 + 1;
                        mux.canvas_height = get_le24(&buf[pos + 15..]) as i32 + 1;
                    }
                    mux.list_mut(id).push(chunk);
                }
            }
            pos += data_size;
        }

        // Incomplete image.
        if wpi.is_partial {
            return None;
        }
        // Validate mux if complete.
        if mux.validate().is_err() {
            return None;
        }
        Some(mux)
    }

    /// Port of `MuxGetCanvasInfo` (`WebPMuxGetFeatures` and `WebPMuxGetCanvasSize`): the canvas
    /// size and the `VP8X` flags.
    fn canvas_info(&self) -> Result<(i32, i32, u32), MuxError> {
        let (w, h, f) = if let Some(vp8x) = self.vp8x.first() {
            if vp8x.data.len() < VP8X_CHUNK_SIZE {
                return Err(MuxError::BadData);
            }
            let f = get_le32(&vp8x.data[0..]);
            let w = get_le24(&vp8x.data[4..]) as i32 + 1;
            let h = get_le24(&vp8x.data[7..]) as i32 + 1;
            (w, h, f)
        } else {
            // Grab the user-forced canvas size as the default.
            let mut w = self.canvas_width;
            let mut h = self.canvas_height;
            if w == 0 && h == 0 && self.validate_for_single_image().is_ok() {
                // A single image and no forced canvas size: the dimensions of the first frame.
                w = self.images[0].width;
                h = self.images[0].height;
            }
            let mut f = 0;
            if let Some(wpi) = self.images.first() {
                if wpi.has_alpha {
                    f |= ALPHA_FLAG;
                }
            }
            (w, h, f)
        };
        if (w as u64) * (h as u64) >= MAX_IMAGE_AREA {
            return Err(MuxError::BadData);
        }
        Ok((w, h, f))
    }

    /// Port of `WebPMuxGetCanvasSize`.
    ///
    /// # Errors
    ///
    /// Returns the `MuxError` the C function would return.
    #[doc(alias = "WebPMuxGetCanvasSize")]
    pub fn canvas_size(&self) -> Result<(i32, i32), MuxError> {
        self.canvas_info().map(|(w, h, _)| (w, h))
    }

    /// Port of `WebPMuxGetFeatures`: the `VP8X` flags of the file.
    ///
    /// # Errors
    ///
    /// Returns the `MuxError` the C function would return.
    #[doc(alias = "WebPMuxGetFeatures")]
    pub fn features(&self) -> Result<u32, MuxError> {
        self.canvas_info().map(|(_, _, f)| f)
    }

    /// Port of `ValidateForSingleImage`.
    fn validate_for_single_image(&self) -> Result<(), MuxError> {
        let num_images = self.image_count(ChunkId::Image);
        let num_frames = self.image_count(ChunkId::Anmf);
        if num_images == 0 {
            Err(MuxError::NotFound)
        } else if num_images == 1 && num_frames == 0 {
            Ok(())
        } else {
            Err(MuxError::InvalidArgument)
        }
    }

    /// Port of `ValidateChunk`: at most `max` chunks (`None` for no limit), and the feature bit
    /// agrees with the presence of the chunks.
    fn validate_chunk(
        &self,
        id: ChunkId,
        feature: Option<u32>,
        vp8x_flags: u32,
        max: Option<usize>,
    ) -> Result<usize, MuxError> {
        let num = self.num_chunks(id);
        if max.is_some_and(|m| num > m) {
            return Err(MuxError::InvalidArgument);
        }
        if let Some(feature) = feature
            && is_not_compatible(vp8x_flags & feature, num)
        {
            return Err(MuxError::InvalidArgument);
        }
        Ok(num)
    }

    /// Port of `MuxValidate`.
    fn validate(&self) -> Result<(), MuxError> {
        // Verify the mux has at least one image.
        if self.images.is_empty() {
            return Err(MuxError::InvalidArgument);
        }
        let flags = self.features()?;

        // At most one colour profile, one EXIF and one XMP chunk.
        self.validate_chunk(ChunkId::Iccp, Some(ICCP_FLAG), flags, Some(1))?;
        self.validate_chunk(ChunkId::Exif, Some(EXIF_FLAG), flags, Some(1))?;
        self.validate_chunk(ChunkId::Xmp, Some(XMP_FLAG), flags, Some(1))?;

        // Animation: ANIMATION_FLAG, ANIM chunk and ANMF chunk(s) are consistent.
        let num_anim = self.validate_chunk(ChunkId::Anim, None, flags, Some(1))?;
        let num_frames = self.validate_chunk(ChunkId::Anmf, None, flags, None)?;
        let has_animation = flags & ANIMATION_FLAG != 0;
        if has_animation && (num_anim == 0 || num_frames == 0) {
            return Err(MuxError::InvalidArgument);
        }
        if !has_animation && (num_anim == 1 || num_frames > 0) {
            return Err(MuxError::InvalidArgument);
        }
        if !has_animation {
            // There can be only one image, and its size must match the canvas.
            if self.images.len() != 1 {
                return Err(MuxError::InvalidArgument);
            }
            if self.canvas_width > 0 {
                let wpi = &self.images[0];
                if wpi.width != self.canvas_width || wpi.height != self.canvas_height {
                    return Err(MuxError::InvalidArgument);
                }
            }
        }

        // Either a VP8X chunk is present, or there is only one image.
        let num_vp8x = self.validate_chunk(ChunkId::Vp8x, None, flags, Some(1))?;
        let num_images = self.validate_chunk(ChunkId::Image, None, flags, None)?;
        if num_vp8x == 0 && num_images != 1 {
            return Err(MuxError::InvalidArgument);
        }

        // ALPHA_FLAG and the ALPH chunk(s) are consistent. ALPHA_FLAG can be set without alpha.
        if self.images.iter().any(|w| w.has_alpha) {
            if num_vp8x > 0 {
                if flags & ALPHA_FLAG == 0 {
                    return Err(MuxError::InvalidArgument);
                }
            } else if self.num_chunks(ChunkId::Alpha) > 0 {
                return Err(MuxError::InvalidArgument);
            }
        }
        Ok(())
    }

    /// Port of `WebPMuxSetImage`: replaces the images with the single image of `bitstream` (a WebP
    /// file, or a raw `VP8`/`VP8L` bitstream).
    ///
    /// # Errors
    ///
    /// Returns the `MuxError` the C function would return.
    #[doc(alias = "WebPMuxSetImage")]
    pub fn set_image(&mut self, bitstream: &[u8]) -> Result<(), MuxError> {
        if bitstream.len() as u64 > MAX_CHUNK_PAYLOAD {
            return Err(MuxError::InvalidArgument);
        }
        // Only one 'simple image' can be added in the mux, so the present images are removed.
        self.images.clear();
        let mut wpi = MuxImage::default();
        set_alpha_and_image_chunks(bitstream, &mut wpi)?;
        self.images.push(wpi);
        Ok(())
    }

    /// Port of `WebPMuxSetChunk`: sets the chunk `fourcc` to `data`, replacing any existing ones.
    ///
    /// # Errors
    ///
    /// Returns the `MuxError` the C function would return.
    #[doc(alias = "WebPMuxSetChunk")]
    pub fn set_chunk(&mut self, fourcc: [u8; 4], data: &[u8]) -> Result<(), MuxError> {
        if data.len() as u64 > MAX_CHUNK_PAYLOAD {
            return Err(MuxError::InvalidArgument);
        }
        let tag = mkfourcc(fourcc[0], fourcc[1], fourcc[2], fourcc[3]);
        // Delete the existing chunk(s) with the same fourcc.
        match self.delete_all_named(tag) {
            Ok(()) | Err(MuxError::NotFound) => {}
            Err(e) => return Err(e),
        }
        // MuxSet: the chunk goes to the list of its id (unknown chunks to the unknown list).
        let chunk = Chunk {
            tag,
            data: data.to_vec(),
        };
        let id = chunk_id_from_tag(tag);
        if is_wpi(id) {
            return Err(MuxError::NotFound);
        }
        set_head(chunk, self.list_mut(id))
    }

    /// Port of `WebPMuxDeleteChunk`.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` when there is no such chunk.
    #[doc(alias = "WebPMuxDeleteChunk")]
    pub fn delete_chunk(&mut self, fourcc: [u8; 4]) -> Result<(), MuxError> {
        self.delete_all_named(mkfourcc(fourcc[0], fourcc[1], fourcc[2], fourcc[3]))
    }

    /// Port of `WebPMuxGetChunk`: the data of the first chunk `fourcc`.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` when there is no such chunk.
    #[doc(alias = "WebPMuxGetChunk")]
    pub fn get_chunk(&self, fourcc: [u8; 4]) -> Result<&[u8], MuxError> {
        let tag = mkfourcc(fourcc[0], fourcc[1], fourcc[2], fourcc[3]);
        let id = chunk_id_from_tag(tag);
        if is_wpi(id) {
            return Err(MuxError::NotFound);
        }
        self.list(id)
            .iter()
            .find(|c| c.tag == tag)
            .map(|c| c.data.as_slice())
            .ok_or(MuxError::NotFound)
    }

    /// Port of `WebPMuxSetAnimationParams`.
    ///
    /// # Errors
    ///
    /// Returns the `MuxError` the C function would return.
    #[doc(alias = "WebPMuxSetAnimationParams")]
    pub fn set_animation_params(&mut self, params: AnimParams) -> Result<(), MuxError> {
        if params.loop_count < 0 || params.loop_count >= MAX_LOOP_COUNT {
            return Err(MuxError::InvalidArgument);
        }
        // Delete any existing ANIM chunk(s).
        match self.delete_all_named(TAG_ANIM) {
            Ok(()) | Err(MuxError::NotFound) => {}
            Err(e) => return Err(e),
        }
        let mut data = Vec::with_capacity(ANIM_CHUNK_SIZE);
        put_le32(&mut data, params.bgcolor);
        put_le16(&mut data, params.loop_count as u32);
        set_head(Chunk { tag: TAG_ANIM, data }, &mut self.anim)
    }

    /// Port of `WebPMuxSetCanvasSize`.
    ///
    /// # Errors
    ///
    /// Returns `InvalidArgument` for sizes the format cannot hold.
    #[doc(alias = "WebPMuxSetCanvasSize")]
    pub fn set_canvas_size(&mut self, width: i32, height: i32) -> Result<(), MuxError> {
        if width < 0 || height < 0 || i64::from(width) > MAX_CANVAS_SIZE || i64::from(height) > MAX_CANVAS_SIZE
        {
            return Err(MuxError::InvalidArgument);
        }
        if (width as u64) * (height as u64) >= MAX_IMAGE_AREA {
            return Err(MuxError::InvalidArgument);
        }
        if (width as u64) * (height as u64) == 0 && (width | height) != 0 {
            // One of the width and height is zero, but not both.
            return Err(MuxError::InvalidArgument);
        }
        // An assembled VP8X chunk is invalidated.
        match self.delete_all_named(TAG_VP8X) {
            Ok(()) | Err(MuxError::NotFound) => {}
            Err(e) => return Err(e),
        }
        self.canvas_width = width;
        self.canvas_height = height;
        Ok(())
    }

    /// Port of `WebPMuxPushFrame`: appends an animation frame (`ANMF`) or a still image.
    ///
    /// # Errors
    ///
    /// Returns the `MuxError` the C function would return.
    #[doc(alias = "WebPMuxPushFrame")]
    pub fn push_frame(&mut self, info: &FrameInfo<'_>) -> Result<(), MuxError> {
        if info.id != ChunkId::Anmf {
            return Err(MuxError::InvalidArgument);
        }
        if info.bitstream.len() as u64 > MAX_CHUNK_PAYLOAD {
            return Err(MuxError::InvalidArgument);
        }
        if let Some(image) = self.images.first() {
            let image_id = image
                .header
                .as_ref()
                .map_or(ChunkId::Image, |h| chunk_id_from_tag(h.tag));
            if image_id != info.id {
                return Err(MuxError::InvalidArgument); // Conflicting frame types.
            }
        }

        let mut wpi = MuxImage::default();
        set_alpha_and_image_chunks(info.bitstream, &mut wpi)?;

        // Snap the offsets to even values.
        let x_offset = info.x_offset & !1;
        let y_offset = info.y_offset & !1;
        if !(0..MAX_POSITION_OFFSET).contains(&x_offset)
            || !(0..MAX_POSITION_OFFSET).contains(&y_offset)
            || !(0..MAX_DURATION).contains(&info.duration)
        {
            return Err(MuxError::InvalidArgument);
        }

        // Create the ANMF header: offsets, size, duration, and the flags.
        let mut frame = Vec::with_capacity(ANMF_CHUNK_SIZE);
        put_le24(&mut frame, (x_offset / 2) as u32);
        put_le24(&mut frame, (y_offset / 2) as u32);
        put_le24(&mut frame, (wpi.width - 1) as u32);
        put_le24(&mut frame, (wpi.height - 1) as u32);
        put_le24(&mut frame, info.duration as u32);
        frame.push(
            (if info.blend == Blend::NoBlend { 2 } else { 0 })
                | (if info.dispose == Dispose::Background { 1 } else { 0 }),
        );
        set_head_opt(
            Chunk {
                tag: TAG_ANMF,
                data: frame,
            },
            &mut wpi.header,
        )?;

        self.images.push(wpi);
        Ok(())
    }

    /// Port of `WebPMuxDeleteFrame`: removes the frame `nth` (1-based; 0 is the last one).
    ///
    /// # Errors
    ///
    /// Returns `NotFound` when there is no such frame.
    #[doc(alias = "WebPMuxDeleteFrame")]
    pub fn delete_frame(&mut self, nth: u32) -> Result<(), MuxError> {
        let index = search_image(&self.images, nth).ok_or(MuxError::NotFound)?;
        self.images.remove(index);
        Ok(())
    }

    /// Port of `MuxCleanup`: a single frame that covers the canvas becomes a still image, and a
    /// still image drops the `ANIM` chunk.
    fn cleanup(&mut self) -> Result<(), MuxError> {
        let mut num_frames = self.num_chunks(ChunkId::Anmf);
        if num_frames == 1 {
            // We know that one frame does exist; `MuxImageGetNth(1)` is the first image.
            let frame = &mut self.images[0];
            if frame.header.is_some()
                && ((self.canvas_width == 0 && self.canvas_height == 0)
                    || (frame.width == self.canvas_width && frame.height == self.canvas_height))
            {
                frame.header = None;
                num_frames = 0;
            }
        }
        // Remove the ANIM chunk if this is a non-animated image.
        let num_anim = self.num_chunks(ChunkId::Anim);
        if num_anim >= 1 && num_frames == 0 {
            self.delete_all_named(TAG_ANIM)?;
        }
        Ok(())
    }

    /// Port of `GetAdjustedCanvasSize`: the tightest canvas that holds the images.
    fn adjusted_canvas_size(&self) -> Result<(i32, i32), MuxError> {
        let Some(first) = self.images.first() else {
            return Err(MuxError::InvalidArgument);
        };
        if self.images.len() > 1 {
            // A chain of frames: the bounding box of all of them.
            let mut max_x = 0;
            let mut max_y = 0;
            for wpi in &self.images {
                let header = wpi.header.as_ref().ok_or(MuxError::InvalidArgument)?;
                // Port of `GetFrameInfo`.
                if header.data.len() != ANMF_CHUNK_SIZE {
                    return Err(MuxError::InvalidArgument);
                }
                let x_offset = 2 * get_le24(&header.data[0..]) as i32;
                let y_offset = 2 * get_le24(&header.data[3..]) as i32;
                let max_x_pos = x_offset + wpi.width;
                let max_y_pos = y_offset + wpi.height;
                if max_x_pos > max_x {
                    max_x = max_x_pos;
                }
                if max_y_pos > max_y {
                    max_y = max_y_pos;
                }
            }
            Ok((max_x, max_y))
        } else {
            // A single image: the canvas is the image.
            Ok((first.width, first.height))
        }
    }

    /// Port of `CreateVP8XChunk`: writes the `VP8X` chunk when the file needs one.
    fn create_vp8x(&mut self) -> Result<(), MuxError> {
        let Some(first) = self.images.first() else {
            return Err(MuxError::InvalidArgument);
        };
        if first.img.is_none() {
            return Err(MuxError::InvalidArgument);
        }
        // Remove the VP8X chunk(s), and add a new one with the updated flags.
        match self.delete_all_named(TAG_VP8X) {
            Ok(()) | Err(MuxError::NotFound) => {}
            Err(e) => return Err(e),
        }

        let mut flags = 0u32;
        if !self.iccp.is_empty() {
            flags |= ICCP_FLAG;
        }
        if !self.exif.is_empty() {
            flags |= EXIF_FLAG;
        }
        if !self.xmp.is_empty() {
            flags |= XMP_FLAG;
        }
        let first = &self.images[0];
        if first.header.as_ref().is_some_and(|h| h.tag == TAG_ANMF) {
            // An image with animation.
            flags |= ANIMATION_FLAG;
        }
        if self.image_count(ChunkId::Alpha) > 0 {
            flags |= ALPHA_FLAG; // Some images have an alpha channel.
        }

        let (mut width, mut height) = self.adjusted_canvas_size()?;
        if width <= 0 || height <= 0 {
            return Err(MuxError::InvalidArgument);
        }
        if i64::from(width) > MAX_CANVAS_SIZE || i64::from(height) > MAX_CANVAS_SIZE {
            return Err(MuxError::InvalidArgument);
        }
        if self.canvas_width != 0 || self.canvas_height != 0 {
            if width > self.canvas_width || height > self.canvas_height {
                return Err(MuxError::InvalidArgument);
            }
            width = self.canvas_width;
            height = self.canvas_height;
        }

        // For the simple file format, the VP8X chunk is not added.
        if flags == 0 && self.unknown.is_empty() {
            return Ok(());
        }

        if self.images.iter().any(|w| w.has_alpha) {
            // Some frames explicitly or implicitly contain alpha.
            flags |= ALPHA_FLAG;
        }

        let mut data = Vec::with_capacity(VP8X_CHUNK_SIZE);
        put_le32(&mut data, flags);
        put_le24(&mut data, (width - 1) as u32);
        put_le24(&mut data, (height - 1) as u32);
        set_head(Chunk { tag: TAG_VP8X, data }, &mut self.vp8x)
    }

    /// Port of `WebPMuxAssemble`: the WebP file of the mux, in the order the C code emits it.
    ///
    /// # Errors
    ///
    /// Returns the `MuxError` the C function would return.
    #[doc(alias = "WebPMuxAssemble")]
    pub fn assemble(&mut self) -> Result<Vec<u8>, MuxError> {
        self.cleanup()?;
        self.create_vp8x()?;

        let size = list_disk_size(&self.vp8x)
            + list_disk_size(&self.iccp)
            + list_disk_size(&self.anim)
            + self.images.iter().map(image_disk_size).sum::<usize>()
            + list_disk_size(&self.exif)
            + list_disk_size(&self.xmp)
            + list_disk_size(&self.unknown)
            + RIFF_HEADER_SIZE;

        let mut data = Vec::with_capacity(size);
        // Port of `MuxEmitRiffHeader`.
        put_le32(&mut data, mkfourcc(b'R', b'I', b'F', b'F'));
        put_le32(&mut data, (size - CHUNK_HEADER_SIZE) as u32);
        put_le32(&mut data, mkfourcc(b'W', b'E', b'B', b'P'));
        list_emit(&self.vp8x, &mut data);
        list_emit(&self.iccp, &mut data);
        list_emit(&self.anim, &mut data);
        for wpi in &self.images {
            image_emit(wpi, &mut data);
        }
        list_emit(&self.exif, &mut data);
        list_emit(&self.xmp, &mut data);
        list_emit(&self.unknown, &mut data);
        debug_assert_eq!(data.len(), size);

        self.validate()?;
        Ok(data)
    }
}

/// Port of `SearchImageToGetOrDelete`: the index of the image `nth` (1-based; 0 is the last one).
fn search_image(images: &[MuxImage], nth: u32) -> Option<usize> {
    let nth = if nth == 0 {
        if images.is_empty() {
            return None;
        }
        images.len() as u32
    } else {
        nth
    };
    if (nth as usize) > images.len() {
        return None;
    }
    Some(nth as usize - 1)
}

/// Port of `GetImageData`: the image chunk, the alpha chunk and whether the image is lossless, of
/// a WebP file or of a raw bitstream.
fn get_image_data(bitstream: &[u8]) -> Result<(Vec<u8>, Option<Vec<u8>>, bool), MuxError> {
    if bitstream.len() < TAG_SIZE || &bitstream[..TAG_SIZE] != b"RIFF" {
        // Not a WebP file: the input is the bitstream.
        let lossless = vp8l::check_signature(bitstream);
        return Ok((bitstream.to_vec(), None, lossless));
    }
    // A WebP file: its first image.
    let mux = Mux::create(bitstream).ok_or(MuxError::BadData)?;
    let wpi = mux.images.first().ok_or(MuxError::BadData)?;
    let img = wpi.img.as_ref().ok_or(MuxError::BadData)?;
    let image = img.data.clone();
    let alpha = wpi.alpha.as_ref().map(|a| a.data.clone());
    let lossless = vp8l::check_signature(&image);
    Ok((image, alpha, lossless))
}

/// Port of `AddDataToChunkList`: a chunk with `data` becomes the head of `slot`.
fn add_data_to_slot(data: Vec<u8>, tag: u32, slot: &mut Option<Chunk>) -> Result<(), MuxError> {
    set_head_opt(Chunk { tag, data }, slot)
}

/// Port of `SetAlphaAndImageChunks`: the alpha and the image chunks of `bitstream` in `wpi`.
fn set_alpha_and_image_chunks(bitstream: &[u8], wpi: &mut MuxImage) -> Result<(), MuxError> {
    let (image, alpha, is_lossless) = get_image_data(bitstream)?;
    let image_tag = if is_lossless { TAG_VP8L } else { TAG_VP8 };
    if let Some(alpha) = alpha {
        add_data_to_slot(alpha, TAG_ALPH, &mut wpi.alpha)?;
    }
    add_data_to_slot(image, image_tag, &mut wpi.img)?;
    if mux_image_finalize(wpi) {
        Ok(())
    } else {
        Err(MuxError::InvalidArgument)
    }
}
