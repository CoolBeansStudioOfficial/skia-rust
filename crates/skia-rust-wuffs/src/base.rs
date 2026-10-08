// Copyright 2017 The Wuffs Authors (the Wuffs sources this file ports).
// Modifications (the Rust port) Copyright (C) 2025 The skia-rust Authors.
// Licensed under the Apache License, Version 2.0; see the LICENSE file of this crate. This file
// is modified from the Wuffs sources.
// Port of: wuffs-v0.3.c, the "base" module (status codes, I/O buffers, pixel formats, pixel
// configs and buffers, the image and frame configs, and the pixel swizzler), as used by Skia's
// SkWuffsCodec (third_party/skia/src/codec/SkWuffsCodec.cpp) and by the GIF decoder.
//
// Wuffs' generated C (release/c/wuffs-v0.3.c at google/wuffs-mirror-release-c@e3f919cc) is the
// reference. Only the paths Skia reaches are ported; the omitted paths are listed on each item
// that is narrower than the C code.

// The ported arithmetic is Wuffs' C integer arithmetic (uint8, uint16, uint32, uint64 and size_t
// conversions, with the truncating and sign-changing casts the C code relies on). Each cast mirrors
// one C cast, so the cast lints are allowed for the module.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless
)]

use crate::strings;

/// Port of `wuffs_base__status`: `None` is success, otherwise the first byte of the message says
/// what kind of status it is (`'$'` suspension, `'#'` error, `'@'` note).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status(Option<&'static str>);

impl Status {
    /// Port of `wuffs_base__make_status(NULL)`.
    pub const OK: Status = Status(None);

    /// Port of `wuffs_base__make_status(repr)`.
    #[must_use]
    pub const fn new(repr: &'static str) -> Status {
        Status(Some(repr))
    }

    /// Returns the raw message, including its kind prefix, or `None` for success. The C field
    /// `status.repr`.
    #[must_use]
    pub const fn repr(self) -> Option<&'static str> {
        self.0
    }

    /// Port of `wuffs_base__status__is_ok`.
    #[must_use]
    pub const fn is_ok(self) -> bool {
        self.0.is_none()
    }

    /// Port of `wuffs_base__status__is_error`.
    #[must_use]
    pub fn is_error(self) -> bool {
        self.0.is_some_and(|s| s.starts_with('#'))
    }

    /// Port of `wuffs_base__status__is_suspension`.
    #[must_use]
    pub fn is_suspension(self) -> bool {
        self.0.is_some_and(|s| s.starts_with('$'))
    }

    /// Port of `wuffs_base__status__is_note`.
    #[must_use]
    pub fn is_note(self) -> bool {
        self.0
            .is_some_and(|s| !s.starts_with('$') && !s.starts_with('#'))
    }

    /// Port of `wuffs_base__status__is_complete`: success, a note, or an error, but not a
    /// suspension.
    #[must_use]
    pub fn is_complete(self) -> bool {
        self.0
            .is_none_or(|s| !s.starts_with('$') && !s.starts_with('#'))
    }

    /// Port of `wuffs_base__status__message`: the text without its kind prefix.
    #[must_use]
    pub fn message(self) -> Option<&'static str> {
        match self.0 {
            Some(s) if s.starts_with(['$', '#', '@']) => Some(&s[1..]),
            other => other,
        }
    }
}

/// Port of `wuffs_base__io_buffer_meta`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IoMeta {
    /// Write index. Invariant: `wi <= data.len()`.
    pub wi: usize,
    /// Read index. Invariant: `ri <= wi`.
    pub ri: usize,
    /// Buffer position, relative to the start of the stream.
    pub pos: u64,
    /// No further writes are expected.
    pub closed: bool,
}

/// Port of `wuffs_base__io_buffer`: a byte buffer plus its read and write indexes. The C
/// pointer `data.ptr` is the start of `data`; `data.len()` is the capacity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IoBuffer {
    /// The backing storage. An empty `Vec` is Wuffs' empty io buffer (`data.ptr == NULL`).
    pub data: Vec<u8>,
    /// The indexes and metadata.
    pub meta: IoMeta,
}

impl IoBuffer {
    /// An empty io buffer with `len` bytes of storage and the empty metadata.
    #[must_use]
    pub fn with_capacity(len: usize) -> IoBuffer {
        IoBuffer {
            data: vec![0; len],
            meta: IoMeta::default(),
        }
    }

    /// Port of `wuffs_base__io_buffer__compact`.
    pub fn compact(&mut self) {
        if self.meta.ri == 0 {
            return;
        }
        self.meta.pos = self.meta.pos.saturating_add(self.meta.ri as u64);
        let n = self.meta.wi - self.meta.ri;
        if n != 0 {
            self.data.copy_within(self.meta.ri..self.meta.ri + n, 0);
        }
        self.meta.wi = n;
        self.meta.ri = 0;
    }

    /// Port of `wuffs_base__io_buffer__reader_io_position`.
    #[must_use]
    pub fn reader_io_position(&self) -> u64 {
        self.meta.pos.saturating_add(self.meta.ri as u64)
    }
}

/// Port of `wuffs_base__io_buffer_meta` reset to empty (`wuffs_base__empty_io_buffer_meta`).
#[must_use]
pub fn empty_io_buffer_meta() -> IoMeta {
    IoMeta::default()
}

/// Port of `wuffs_base__pixel_blend`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelBlend {
    /// `WUFFS_BASE__PIXEL_BLEND__SRC`
    Src,
    /// `WUFFS_BASE__PIXEL_BLEND__SRC_OVER`
    SrcOver,
}

/// Port of `wuffs_base__animation_disposal`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnimationDisposal {
    /// `WUFFS_BASE__ANIMATION_DISPOSAL__NONE`
    #[default]
    None,
    /// `WUFFS_BASE__ANIMATION_DISPOSAL__RESTORE_BACKGROUND`
    RestoreBackground,
    /// `WUFFS_BASE__ANIMATION_DISPOSAL__RESTORE_PREVIOUS`
    RestorePrevious,
}

impl AnimationDisposal {
    /// Port of the `wuffs_base__animation_disposal` integer values.
    #[must_use]
    pub const fn repr(self) -> u8 {
        match self {
            AnimationDisposal::None => 0,
            AnimationDisposal::RestoreBackground => 1,
            AnimationDisposal::RestorePrevious => 2,
        }
    }
}

/// Port of `wuffs_base__rect_ie_u32`: a half-open rectangle `[min_incl, max_excl)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RectIeU32 {
    /// `min_incl_x`
    pub min_incl_x: u32,
    /// `min_incl_y`
    pub min_incl_y: u32,
    /// `max_excl_x`
    pub max_excl_x: u32,
    /// `max_excl_y`
    pub max_excl_y: u32,
}

impl RectIeU32 {
    /// Port of `wuffs_base__utility__make_rect_ie_u32`.
    #[must_use]
    pub const fn new(min_incl_x: u32, min_incl_y: u32, max_excl_x: u32, max_excl_y: u32) -> Self {
        RectIeU32 {
            min_incl_x,
            min_incl_y,
            max_excl_x,
            max_excl_y,
        }
    }

    /// Port of `wuffs_base__rect_ie_u32__width` (saturating).
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.max_excl_x.saturating_sub(self.min_incl_x)
    }

    /// Port of `wuffs_base__rect_ie_u32__height` (saturating).
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.max_excl_y.saturating_sub(self.min_incl_y)
    }

    /// Port of `wuffs_base__rect_ie_u32__is_empty`.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.min_incl_x >= self.max_excl_x || self.min_incl_y >= self.max_excl_y
    }
}

/// Port of `wuffs_base__pixel_format`: a packed `repr`, with the layout of Wuffs' pixel format
/// encoding (bits 0..15 channel widths, 16..17 planes, 18 indexed, 24..25 transparency).
/// The default is `PIXEL_FORMAT_INVALID`, as `repr == 0`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PixelFormat(pub u32);

/// `WUFFS_BASE__PIXEL_FORMAT__INVALID`
pub const PIXEL_FORMAT_INVALID: PixelFormat = PixelFormat(0x0000_0000);
/// `WUFFS_BASE__PIXEL_FORMAT__INDEXED__BGRA_NONPREMUL`
pub const PIXEL_FORMAT_INDEXED_BGRA_NONPREMUL: PixelFormat = PixelFormat(0x8104_0008);
/// `WUFFS_BASE__PIXEL_FORMAT__INDEXED__BGRA_BINARY`: the GIF source format.
pub const PIXEL_FORMAT_INDEXED_BGRA_BINARY: PixelFormat = PixelFormat(0x8304_0008);
/// `WUFFS_BASE__PIXEL_FORMAT__BGR_565`
pub const PIXEL_FORMAT_BGR_565: PixelFormat = PixelFormat(0x8000_0565);
/// `WUFFS_BASE__PIXEL_FORMAT__BGRA_NONPREMUL`
pub const PIXEL_FORMAT_BGRA_NONPREMUL: PixelFormat = PixelFormat(0x8100_8888);
/// `WUFFS_BASE__PIXEL_FORMAT__RGBA_NONPREMUL`
pub const PIXEL_FORMAT_RGBA_NONPREMUL: PixelFormat = PixelFormat(0xA100_8888);

/// Port of `wuffs_base__pixel_palette_byte_length` (`WUFFS_BASE__PIXEL_FORMAT__INDEXED__PALETTE_BYTE_LENGTH`).
pub const PIXEL_FORMAT_INDEXED_PALETTE_BYTE_LENGTH: usize = 1024;

/// Port of `wuffs_base__pixel_format__bits_per_channel`.
const BITS_PER_CHANNEL: [u32; 16] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x0A, 0x0C, 0x10, 0x18, 0x20, 0x30, 0x40,
];

impl PixelFormat {
    /// Port of `wuffs_base__pixel_format__bits_per_pixel`.
    #[must_use]
    pub fn bits_per_pixel(self) -> u32 {
        let f = self.0;
        if ((f >> 16) & 0x03) != 0 {
            return 0;
        }
        BITS_PER_CHANNEL[(0x0F & f) as usize]
            + BITS_PER_CHANNEL[(0x0F & (f >> 4)) as usize]
            + BITS_PER_CHANNEL[(0x0F & (f >> 8)) as usize]
            + BITS_PER_CHANNEL[(0x0F & (f >> 12)) as usize]
    }

    /// Port of `wuffs_base__pixel_format__is_indexed`.
    #[must_use]
    pub fn is_indexed(self) -> bool {
        ((self.0 >> 18) & 0x01) != 0
    }

    /// Port of `wuffs_base__pixel_format__is_planar`.
    #[must_use]
    pub fn is_planar(self) -> bool {
        ((self.0 >> 16) & 0x03) != 0
    }
}

/// Port of `wuffs_base__pixel_config`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PixelConfig {
    /// The pixel format.
    pub pixfmt: PixelFormat,
    /// The subsampling (always `WUFFS_BASE__PIXEL_SUBSAMPLING__NONE` for GIF).
    pub pixsub: u32,
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
}

impl PixelConfig {
    /// Port of `wuffs_base__pixel_config__set`. The C check that `width * height` fits in
    /// `SIZE_MAX` always holds for 32-bit dimensions on a 64-bit target, so it is not repeated.
    pub fn set(&mut self, pixfmt: u32, pixsub: u32, width: u32, height: u32) {
        if pixfmt != 0 {
            *self = PixelConfig {
                pixfmt: PixelFormat(pixfmt),
                pixsub,
                width,
                height,
            };
        } else {
            *self = PixelConfig::default();
        }
    }

    /// Port of `wuffs_base__pixel_config__pixbuf_len`. Returns 0 for planar formats and for
    /// formats whose bit width is not a whole number of bytes.
    #[must_use]
    pub fn pixbuf_len(&self) -> u64 {
        if self.pixfmt.is_planar() {
            return 0;
        }
        let bits_per_pixel = self.pixfmt.bits_per_pixel();
        if bits_per_pixel == 0 || !bits_per_pixel.is_multiple_of(8) {
            return 0;
        }
        let bytes_per_pixel = u64::from(bits_per_pixel / 8);
        let mut n = u64::from(self.width) * u64::from(self.height);
        if n > u64::MAX / bytes_per_pixel {
            return 0;
        }
        n *= bytes_per_pixel;
        if self.pixfmt.is_indexed() {
            if n > u64::MAX - PIXEL_FORMAT_INDEXED_PALETTE_BYTE_LENGTH as u64 {
                return 0;
            }
            n += PIXEL_FORMAT_INDEXED_PALETTE_BYTE_LENGTH as u64;
        }
        n
    }
}

/// Port of `wuffs_base__image_config`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImageConfig {
    /// The pixel configuration of the first frame's canvas.
    pub pixcfg: PixelConfig,
    /// Port of `private_impl.first_frame_io_position`.
    pub first_frame_io_position: u64,
    /// Port of `private_impl.first_frame_is_opaque`.
    pub first_frame_is_opaque: bool,
}

impl ImageConfig {
    /// Port of `wuffs_base__image_config__set`.
    pub fn set(
        &mut self,
        pixfmt: u32,
        pixsub: u32,
        width: u32,
        height: u32,
        first_frame_io_position: u64,
        first_frame_is_opaque: bool,
    ) {
        if pixfmt != 0 {
            self.pixcfg.pixfmt = PixelFormat(pixfmt);
            self.pixcfg.pixsub = pixsub;
            self.pixcfg.width = width;
            self.pixcfg.height = height;
            self.first_frame_io_position = first_frame_io_position;
            self.first_frame_is_opaque = first_frame_is_opaque;
        }
    }
}

/// Port of `wuffs_base__frame_config`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameConfig {
    /// The frame rectangle, on the image canvas.
    pub bounds: RectIeU32,
    /// The duration, in flicks (`WUFFS_BASE__FLICKS_PER_MILLISECOND` is 705600).
    pub duration: i64,
    /// The frame index.
    pub index: u64,
    /// The I/O position of the frame's data.
    pub io_position: u64,
    /// How the frame is disposed of before the next frame.
    pub disposal: AnimationDisposal,
    /// Whether the frame's bounds are fully opaque.
    pub opaque_within_bounds: bool,
    /// Whether the frame overwrites instead of blending.
    pub overwrite_instead_of_blend: bool,
    /// The background colour, as premultiplied ARGB.
    pub background_color: u32,
}

impl FrameConfig {
    /// Port of `wuffs_base__frame_config__set`.
    #[allow(clippy::too_many_arguments)] // mirrors the C signature, which takes every field
    pub fn set(
        &mut self,
        bounds: RectIeU32,
        duration: i64,
        index: u64,
        io_position: u64,
        disposal: AnimationDisposal,
        opaque_within_bounds: bool,
        overwrite_instead_of_blend: bool,
        background_color: u32,
    ) {
        self.bounds = bounds;
        self.duration = duration;
        self.index = index;
        self.io_position = io_position;
        self.disposal = disposal;
        self.opaque_within_bounds = opaque_within_bounds;
        self.overwrite_instead_of_blend = overwrite_instead_of_blend;
        self.background_color = background_color;
    }
}

/// `WUFFS_BASE__FLICKS_PER_MILLISECOND`
pub const FLICKS_PER_MILLISECOND: u64 = 705_600;

/// Port of `wuffs_base__table_u8` plus the memory it addresses. `data` starts at the table's
/// first byte; row `y` is `data[stride*y .. stride*y + width]`.
#[derive(Debug)]
pub struct Table<'a> {
    /// The memory.
    pub data: &'a mut [u8],
    /// The row width, in bytes.
    pub width: usize,
    /// The number of rows.
    pub height: usize,
    /// The distance between row starts, in bytes.
    pub stride: usize,
}

/// Port of `wuffs_base__pixel_buffer`, with at most one plane. Indexed formats are rejected by
/// the setters, so the C palette plane is never present (and `palette_or_else` always returns
/// its fallback). Planar formats are rejected too, as in `set_from_table` and `set_from_slice`.
#[derive(Debug, Default)]
pub struct PixelBuffer<'a> {
    /// The pixel configuration.
    pub pixcfg: PixelConfig,
    plane0: Option<Table<'a>>,
}

impl<'a> PixelBuffer<'a> {
    /// Port of `wuffs_base__null_pixel_buffer`.
    #[must_use]
    pub fn null() -> Self {
        PixelBuffer::default()
    }

    /// Port of `wuffs_base__pixel_buffer__set_from_slice` for interleaved formats. The indexed
    /// branch (palette plane) is not ported; indexed formats return `unsupported option`.
    pub fn set_from_slice(&mut self, pixcfg: &PixelConfig, pixbuf_memory: &'a mut [u8]) -> Status {
        *self = PixelBuffer::default();
        if pixcfg.pixfmt.is_planar() || pixcfg.pixfmt.is_indexed() {
            return Status::new(strings::ERROR_UNSUPPORTED_OPTION);
        }
        let bits_per_pixel = pixcfg.pixfmt.bits_per_pixel();
        if bits_per_pixel == 0 || !bits_per_pixel.is_multiple_of(8) {
            return Status::new(strings::ERROR_UNSUPPORTED_OPTION);
        }
        let bytes_per_pixel = u64::from(bits_per_pixel / 8);
        let wh = u64::from(pixcfg.width) * u64::from(pixcfg.height);
        let mut width = pixcfg.width as usize;
        if wh > u64::MAX / bytes_per_pixel || width as u64 > u64::MAX / bytes_per_pixel {
            return Status::new(strings::ERROR_BAD_ARGUMENT);
        }
        let len = wh * bytes_per_pixel;
        width = (width as u64 * bytes_per_pixel) as usize;
        if len > pixbuf_memory.len() as u64 {
            return Status::new(strings::ERROR_BAD_ARGUMENT_LENGTH_TOO_SHORT);
        }
        self.pixcfg = *pixcfg;
        self.plane0 = Some(Table {
            data: pixbuf_memory,
            width,
            height: pixcfg.height as usize,
            stride: width,
        });
        Status::OK
    }

    /// Port of `wuffs_base__pixel_buffer__set_from_table` for interleaved formats. Also checks
    /// that every row lies inside `primary.data`, which the C code leaves to the caller.
    pub fn set_from_table(&mut self, pixcfg: &PixelConfig, primary: Table<'a>) -> Status {
        *self = PixelBuffer::default();
        if pixcfg.pixfmt.is_indexed() || pixcfg.pixfmt.is_planar() {
            return Status::new(strings::ERROR_BAD_ARGUMENT);
        }
        let bits_per_pixel = pixcfg.pixfmt.bits_per_pixel();
        if bits_per_pixel == 0 || !bits_per_pixel.is_multiple_of(8) {
            return Status::new(strings::ERROR_UNSUPPORTED_OPTION);
        }
        let bytes_per_pixel = (bits_per_pixel / 8) as usize;
        let width_in_bytes = pixcfg.width as usize * bytes_per_pixel;
        if width_in_bytes > primary.width || pixcfg.height as usize > primary.height {
            return Status::new(strings::ERROR_BAD_ARGUMENT);
        }
        if pixcfg.height != 0
            && (pixcfg.height as usize - 1) * primary.stride + primary.width > primary.data.len()
        {
            return Status::new(strings::ERROR_BAD_ARGUMENT);
        }
        self.pixcfg = *pixcfg;
        self.plane0 = Some(primary);
        Status::OK
    }

    /// Port of `wuffs_base__pixel_buffer__pixel_format`.
    #[must_use]
    pub fn pixel_format(&self) -> PixelFormat {
        self.pixcfg.pixfmt
    }

    /// The primary plane (`wuffs_base__pixel_buffer__plane(pb, 0)`), if one was set.
    #[must_use]
    pub fn plane0_mut(&mut self) -> Option<&mut Table<'a>> {
        self.plane0.as_mut()
    }

    /// The primary plane's table dimensions, `(width, height, stride)`, if one was set.
    #[must_use]
    pub fn plane0_dims(&self) -> Option<(usize, usize, usize)> {
        self.plane0.as_ref().map(|t| (t.width, t.height, t.stride))
    }
}

/// Port of `wuffs_base__pixel_swizzler__func`: converts `src` pixels into `dst` (with the palette
/// for indexed sources) and returns the number of source pixels consumed. The palette slice is
/// empty at prepare time, as the C code passes `NULL, 0`.
pub type SwizzleFn = fn(dst: &mut [u8], dst_palette: &[u8], src: &[u8]) -> u64;

/// Port of `wuffs_base__pixel_swizzler`.
#[derive(Clone, Copy, Debug, Default)]
pub struct PixelSwizzler {
    func: Option<SwizzleFn>,
    dst_pixfmt_bytes_per_pixel: u32,
    src_pixfmt_bytes_per_pixel: u32,
}

/// Port of `wuffs_base__pixel_swizzler__prepare`. Only the source format `INDEXED__BGRA_BINARY`
/// (the GIF source format) and the destinations `BGRA_NONPREMUL`, `RGBA_NONPREMUL` and `BGR_565`
/// are ported, which is every combination `SkWuffsCodec` requests. Other combinations return
/// `unsupported pixel swizzler option`, as the C code does for unknown combinations.
#[must_use]
pub fn swizzler_prepare(
    p: &mut PixelSwizzler,
    dst_pixfmt: PixelFormat,
    dst_palette: &mut [u8],
    src_pixfmt: PixelFormat,
    src_palette: &[u8],
    blend: PixelBlend,
) -> Status {
    p.func = None;
    p.dst_pixfmt_bytes_per_pixel = 0;
    p.src_pixfmt_bytes_per_pixel = 0;
    let dst_bits = dst_pixfmt.bits_per_pixel();
    if dst_bits == 0 || dst_bits & 7 != 0 {
        return Status::new(strings::ERROR_UNSUPPORTED_PIXEL_SWIZZLER_OPTION);
    }
    let src_bits = src_pixfmt.bits_per_pixel();
    if src_bits == 0 || src_bits & 7 != 0 {
        return Status::new(strings::ERROR_UNSUPPORTED_PIXEL_SWIZZLER_OPTION);
    }
    let func = if src_pixfmt == PIXEL_FORMAT_INDEXED_BGRA_BINARY {
        prepare_indexed_bgra_binary(dst_pixfmt, dst_palette, src_palette, blend)
    } else {
        None
    };
    p.func = func;
    p.dst_pixfmt_bytes_per_pixel = dst_bits / 8;
    p.src_pixfmt_bytes_per_pixel = src_bits / 8;
    if func.is_some() {
        Status::OK
    } else {
        Status::new(strings::ERROR_UNSUPPORTED_PIXEL_SWIZZLER_OPTION)
    }
}

/// Port of `wuffs_base__pixel_swizzler__swizzle_interleaved_from_slice`.
#[must_use]
pub fn swizzle_interleaved_from_slice(
    p: &PixelSwizzler,
    dst: &mut [u8],
    dst_palette: &[u8],
    src: &[u8],
) -> u64 {
    match p.func {
        Some(func) => func(dst, dst_palette, src),
        None => 0,
    }
}

/// Port of `wuffs_base__pixel_swizzler__prepare__indexed__bgra_binary`, restricted to the
/// destinations in [`swizzler_prepare`].
fn prepare_indexed_bgra_binary(
    dst_pixfmt: PixelFormat,
    dst_palette: &mut [u8],
    src_palette: &[u8],
    blend: PixelBlend,
) -> Option<SwizzleFn> {
    const PALETTE_LEN: usize = PIXEL_FORMAT_INDEXED_PALETTE_BYTE_LENGTH;
    match dst_pixfmt {
        PIXEL_FORMAT_BGRA_NONPREMUL => {
            if copy_from_slice(dst_palette, src_palette) != PALETTE_LEN {
                return None;
            }
            match blend {
                PixelBlend::Src => Some(xxxx_index_src),
                PixelBlend::SrcOver => Some(xxxx_index_binary_alpha_src_over),
            }
        }
        PIXEL_FORMAT_RGBA_NONPREMUL => {
            if swap_rgbx_bgrx(dst_palette, &[], src_palette) != PALETTE_LEN / 4 {
                return None;
            }
            match blend {
                PixelBlend::Src => Some(xxxx_index_src),
                PixelBlend::SrcOver => Some(xxxx_index_binary_alpha_src_over),
            }
        }
        PIXEL_FORMAT_BGR_565 => {
            if squash_align4_bgr_565_8888(dst_palette, src_palette, false) != PALETTE_LEN / 4 {
                return None;
            }
            match blend {
                PixelBlend::Src => Some(bgr_565_index_src),
                PixelBlend::SrcOver => Some(bgr_565_index_binary_alpha_src_over),
            }
        }
        _ => None,
    }
}

/// Port of `wuffs_base__slice_u8__copy_from_slice`: copies the shorter of the two and returns
/// the number of bytes copied.
pub fn copy_from_slice(dst: &mut [u8], src: &[u8]) -> usize {
    let len = dst.len().min(src.len());
    dst[..len].copy_from_slice(&src[..len]);
    len
}

/// Port of `wuffs_base__color_u32_argb_nonpremul__as__color_u32_argb_premul`.
#[must_use]
pub fn color_u32_argb_nonpremul_as_premul(argb_nonpremul: u32) -> u32 {
    let a = 0xFF & (argb_nonpremul >> 24);
    let a16 = a * (0x101 * 0x101);
    let mut r = 0xFF & (argb_nonpremul >> 16);
    r = ((r * a16) / 0xFFFF) >> 8;
    let mut g = 0xFF & (argb_nonpremul >> 8);
    g = ((g * a16) / 0xFFFF) >> 8;
    let mut b = 0xFF & argb_nonpremul;
    b = ((b * a16) / 0xFFFF) >> 8;
    (a << 24) | (r << 16) | (g << 8) | b
}

/// Port of `wuffs_base__pixel_swizzler__squash_align4_bgr_565_8888`: packs each 4-byte palette
/// entry into a 565 value in the low two bytes, with the alpha in the high two.
fn squash_align4_bgr_565_8888(dst: &mut [u8], src: &[u8], nonpremul: bool) -> usize {
    let len = dst.len().min(src.len()) / 4;
    for n in 0..len {
        let s = &src[4 * n..4 * n + 4];
        let mut argb = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
        if nonpremul {
            argb = color_u32_argb_nonpremul_as_premul(argb);
        }
        let b5 = 0x1F & (argb >> (8 - 5));
        let g6 = 0x3F & (argb >> (16 - 6));
        let r5 = 0x1F & (argb >> (24 - 5));
        let alpha = argb & 0xFF00_0000;
        let out = alpha | (r5 << 11) | (g6 << 5) | b5;
        dst[4 * n..4 * n + 4].copy_from_slice(&out.to_le_bytes());
    }
    len
}

/// Port of `wuffs_base__pixel_swizzler__swap_rgbx_bgrx`: swaps bytes 0 and 2 of each 4-byte
/// pixel. The palette argument is unused, as in the C code.
fn swap_rgbx_bgrx(dst: &mut [u8], _dst_palette: &[u8], src: &[u8]) -> usize {
    let len = dst.len().min(src.len()) / 4;
    for n in 0..len {
        let s0 = src[4 * n];
        let s1 = src[4 * n + 1];
        let s2 = src[4 * n + 2];
        let s3 = src[4 * n + 3];
        dst[4 * n] = s2;
        dst[4 * n + 1] = s1;
        dst[4 * n + 2] = s0;
        dst[4 * n + 3] = s3;
    }
    len
}

/// Port of `wuffs_base__pixel_swizzler__xxxx__index__src` (4-byte destinations, SRC blend).
fn xxxx_index_src(dst: &mut [u8], dst_palette: &[u8], src: &[u8]) -> u64 {
    if dst_palette.len() != PIXEL_FORMAT_INDEXED_PALETTE_BYTE_LENGTH {
        return 0;
    }
    let dst_len4 = dst.len() / 4;
    let len = dst_len4.min(src.len());
    for i in 0..len {
        let p = 4 * (src[i] as usize);
        dst[4 * i..4 * i + 4].copy_from_slice(&dst_palette[p..p + 4]);
    }
    len as u64
}

/// Port of `wuffs_base__pixel_swizzler__xxxx__index_binary_alpha__src_over`: a palette entry
/// with a zero word leaves the destination pixel alone.
fn xxxx_index_binary_alpha_src_over(dst: &mut [u8], dst_palette: &[u8], src: &[u8]) -> u64 {
    if dst_palette.len() != PIXEL_FORMAT_INDEXED_PALETTE_BYTE_LENGTH {
        return 0;
    }
    let dst_len4 = dst.len() / 4;
    let len = dst_len4.min(src.len());
    for i in 0..len {
        let p = 4 * (src[i] as usize);
        let s0 = u32::from_le_bytes([
            dst_palette[p],
            dst_palette[p + 1],
            dst_palette[p + 2],
            dst_palette[p + 3],
        ]);
        if s0 != 0 {
            dst[4 * i..4 * i + 4].copy_from_slice(&s0.to_le_bytes());
        }
    }
    len as u64
}

/// Port of `wuffs_base__pixel_swizzler__bgr_565__index__src`.
fn bgr_565_index_src(dst: &mut [u8], dst_palette: &[u8], src: &[u8]) -> u64 {
    if dst_palette.len() != PIXEL_FORMAT_INDEXED_PALETTE_BYTE_LENGTH {
        return 0;
    }
    let dst_len2 = dst.len() / 2;
    let len = dst_len2.min(src.len());
    for i in 0..len {
        let p = 4 * (src[i] as usize);
        dst[2 * i..2 * i + 2].copy_from_slice(&dst_palette[p..p + 2]);
    }
    len as u64
}

/// Port of `wuffs_base__pixel_swizzler__bgr_565__index_binary_alpha__src_over`. The C code
/// stores `(uint16_t)s0`, the low two bytes of the palette entry, without converting it to 565;
/// the port keeps that.
fn bgr_565_index_binary_alpha_src_over(dst: &mut [u8], dst_palette: &[u8], src: &[u8]) -> u64 {
    if dst_palette.len() != PIXEL_FORMAT_INDEXED_PALETTE_BYTE_LENGTH {
        return 0;
    }
    let dst_len2 = dst.len() / 2;
    let len = dst_len2.min(src.len());
    for i in 0..len {
        let p = 4 * (src[i] as usize);
        let s0 = u32::from_le_bytes([
            dst_palette[p],
            dst_palette[p + 1],
            dst_palette[p + 2],
            dst_palette[p + 3],
        ]);
        if s0 != 0 {
            dst[2 * i..2 * i + 2].copy_from_slice(&(s0 as u16).to_le_bytes());
        }
    }
    len as u64
}
