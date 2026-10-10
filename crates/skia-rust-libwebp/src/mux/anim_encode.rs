// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the animated WebP encoder of libwebp 1.4.0 (`src/mux/anim_encode.c` at `845d5476`),
//! as `SkWebpEncoder::EncodeAnimated` uses it: `WebPAnimEncoderNew` with the default options,
//! `WebPAnimEncoderAdd` for each frame, and `WebPAnimEncoderAssemble`.
//!
//! Canvases are owned ARGB pictures (`ArgbPicture`); the C `WebPPictureView`s of the sub-frames
//! are the rectangles they were cut from. Only the lossless path is ported: the frames are
//! encoded with `WebPEncode` at `lossless = 1`. The lossy candidates (YUV pictures, the
//! `WebPPictureYUVAToARGB` of `WebPAnimEncoderAdd`) are not, and `WebPAnimEncoderAdd` reports the
//! error for a lossy configuration.

// Clippy allows for the C arithmetic: the rectangle and pixel loops keep the C index forms, and
// the casts are the int/uint32 conversions of the C source.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::struct_excessive_bools,
    clippy::needless_range_loop
)]

use super::{AnimParams, Blend, ChunkId, Dispose, FrameInfo, Mux, MuxError, OwnedFrame};
use crate::enc::encode_lossless_method;

/// Port of `MAX_IMAGE_AREA` (`format_constants.h`).
const MAX_IMAGE_AREA: u64 = 1 << 32;
/// Port of `DELTA_INFINITY`.
const DELTA_INFINITY: i64 = 1 << 32;
/// Port of `KEYFRAME_NONE`.
const KEYFRAME_NONE: i32 = -1;
/// Port of `MAX_CACHED_FRAMES` (`anim_encode.c#L124`).
const MAX_CACHED_FRAMES: i32 = 30;
/// Port of `TRANSPARENT_COLOR`: the starting value that `WebPCleanupTransparentAreaLossless`
/// is tuned for.
const TRANSPARENT_COLOR: u32 = 0x0000_0000;

/// Port of `WebPAnimEncoderOptions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "WebPAnimEncoderOptions")]
pub struct AnimEncoderOptions {
    /// The loop count and the background colour of the animation.
    pub anim_params: AnimParams,
    /// Whether to minimize the size (disables key-frames).
    pub minimize_size: bool,
    /// Minimum and maximum distance between key-frames.
    pub kmin: i32,
    pub kmax: i32,
    /// Whether lossy and lossless frames may be mixed.
    pub allow_mixed: bool,
    /// Whether to print warnings about option changes.
    pub verbose: bool,
}

impl Default for AnimEncoderOptions {
    /// Port of `DefaultEncoderOptions` (anim_encode.c#L175-L182).
    fn default() -> Self {
        let mut options = Self {
            anim_params: AnimParams {
                bgcolor: 0xffff_ffff, // White.
                loop_count: 0,
            },
            minimize_size: false,
            kmin: 0,
            kmax: 0,
            allow_mixed: false,
            verbose: false,
        };
        disable_keyframes(&mut options);
        options
    }
}

/// Port of `DisableKeyframes`.
fn disable_keyframes(options: &mut AnimEncoderOptions) {
    options.kmax = i32::MAX;
    options.kmin = options.kmax - 1;
}

/// Port of `SanitizeEncoderOptions` (anim_encode.c#L126-L171). The warnings go to stderr as the C
/// code's `fprintf` does when `verbose` is set.
fn sanitize_options(options: &mut AnimEncoderOptions) {
    let mut print_warning = options.verbose;

    if options.minimize_size {
        disable_keyframes(options);
    }

    if options.kmax == 1 {
        // All frames will be key-frames.
        options.kmin = 0;
        options.kmax = 0;
        return;
    } else if options.kmax <= 0 {
        disable_keyframes(options);
        print_warning = false;
    }

    if options.kmin >= options.kmax {
        options.kmin = options.kmax - 1;
        if print_warning {
            eprintln!(
                "WARNING: Setting kmin = {}, so that kmin < kmax.",
                options.kmin
            );
        }
    } else {
        let kmin_limit = options.kmax / 2 + 1;
        if options.kmin < kmin_limit && kmin_limit < options.kmax {
            // This ensures that enc.keyframe + kmin >= kmax is always true.
            options.kmin = kmin_limit;
            if print_warning {
                eprintln!(
                    "WARNING: Setting kmin = {}, so that kmin >= kmax / 2 + 1.",
                    options.kmin
                );
            }
        }
    }
    // Limit the max number of frames that are allocated.
    if options.kmax - options.kmin > MAX_CACHED_FRAMES {
        options.kmin = options.kmax - MAX_CACHED_FRAMES;
        if print_warning {
            eprintln!(
                "WARNING: Setting kmin = {}, so that kmax - kmin <= {}.",
                options.kmin, MAX_CACHED_FRAMES
            );
        }
    }
}

/// Port of `WebPAnimEncoderOptionsInit`: the default options.
#[must_use]
#[doc(alias = "WebPAnimEncoderOptionsInit")]
pub fn anim_encoder_options_init() -> AnimEncoderOptions {
    AnimEncoderOptions::default()
}

/// An owned ARGB picture (`WebPPicture` with `use_argb = 1`, the stride equal to the width).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgbPicture {
    /// Port of `WebPPicture::width`.
    pub width: i32,
    /// Port of `WebPPicture::height`.
    pub height: i32,
    /// Port of `WebPPicture::argb`, `0xAARRGGBB` words, row by row.
    pub argb: Vec<u32>,
}

impl ArgbPicture {
    /// Port of `WebPPictureAlloc` for an ARGB picture: a zeroed canvas of the size.
    #[must_use]
    pub fn new(width: i32, height: i32) -> Self {
        let len = (width as usize) * (height as usize);
        Self {
            width,
            height,
            argb: vec![0; len],
        }
    }

    /// The stride of the pixels, `WebPPicture::argb_stride` (the width).
    fn stride(&self) -> usize {
        self.width as usize
    }
}

/// Port of `FrameRectangle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameRectangle {
    pub x_offset: i32,
    pub y_offset: i32,
    pub width: i32,
    pub height: i32,
}

/// Port of `ClearRectangle` (anim_encode.c#L197-L207).
fn clear_rectangle(picture: &mut ArgbPicture, left: i32, top: i32, width: i32, height: i32) {
    let stride = picture.stride();
    for j in top..top + height {
        for i in left..left + width {
            picture.argb[j as usize * stride + i as usize] = TRANSPARENT_COLOR;
        }
    }
}

/// Port of `WebPUtilClearPic` (anim_encode.c#L209-L217): clears `rect`, or the whole picture.
fn util_clear_pic(picture: &mut ArgbPicture, rect: Option<&FrameRectangle>) {
    if let Some(r) = rect {
        clear_rectangle(picture, r.x_offset, r.y_offset, r.width, r.height);
    } else {
        let (w, h) = (picture.width, picture.height);
        clear_rectangle(picture, 0, 0, w, h);
    }
}

/// Port of `ComparePixelsLossless`: whether `length` pixels of `src` and `dst` are equal.
fn compare_pixels_lossless(
    src: &ArgbPicture,
    src_at: usize,
    src_step: usize,
    dst: &ArgbPicture,
    dst_at: usize,
    dst_step: usize,
    length: usize,
) -> bool {
    let (mut s, mut d) = (src_at, dst_at);
    for _ in 0..length {
        if src.argb[s] != dst.argb[d] {
            return false;
        }
        s += src_step;
        d += dst_step;
    }
    true
}

/// Port of `PixelsAreSimilar` (anim_encode.c#L368-L383).
fn pixels_are_similar(src: u32, dst: u32, max_allowed_diff: i32) -> bool {
    let src_a = ((src >> 24) & 0xff) as i32;
    let src_r = ((src >> 16) & 0xff) as i32;
    let src_g = ((src >> 8) & 0xff) as i32;
    let src_b = (src & 0xff) as i32;
    let dst_a = ((dst >> 24) & 0xff) as i32;
    let dst_r = ((dst >> 16) & 0xff) as i32;
    let dst_g = ((dst >> 8) & 0xff) as i32;
    let dst_b = (dst & 0xff) as i32;

    src_a == dst_a
        && (src_r - dst_r).abs() * dst_a <= max_allowed_diff * 255
        && (src_g - dst_g).abs() * dst_a <= max_allowed_diff * 255
        && (src_b - dst_b).abs() * dst_a <= max_allowed_diff * 255
}

/// Port of `ComparePixelsLossy` (anim_encode.c#L387-L399).
#[allow(clippy::too_many_arguments)] // The C signature: two pictures, their offsets and steps.
fn compare_pixels_lossy(
    src: &ArgbPicture,
    src_at: usize,
    src_step: usize,
    dst: &ArgbPicture,
    dst_at: usize,
    dst_step: usize,
    length: usize,
    max_allowed_diff: i32,
) -> bool {
    let (mut s, mut d) = (src_at, dst_at);
    for _ in 0..length {
        if !pixels_are_similar(src.argb[s], dst.argb[d], max_allowed_diff) {
            return false;
        }
        s += src_step;
        d += dst_step;
    }
    true
}

/// Port of `IsEmptyRect`.
fn is_empty_rect(rect: &FrameRectangle) -> bool {
    rect.width == 0 || rect.height == 0
}

/// Port of `QualityToMaxDiff` (anim_encode.c#L405-L409).
fn quality_to_max_diff(quality: f32) -> i32 {
    let val = (f64::from(quality) / 100.0).powf(0.5);
    let max_diff = 31.0 * (1.0 - val) + 1.0 * val;
    (max_diff + 0.5) as i32
}

/// Port of `MinimizeChangeRectangle` (anim_encode.c#L412-L496). Assumes the initial guess `rect`
/// is valid; shrinks it to the rows and columns that differ between `src` and `dst`.
fn minimize_change_rectangle(
    src: &ArgbPicture,
    dst: &ArgbPicture,
    rect: &mut FrameRectangle,
    is_lossless: bool,
    quality: f32,
) {
    let max_allowed_diff_lossy = quality_to_max_diff(quality);
    let max_allowed_diff = if is_lossless {
        0
    } else {
        max_allowed_diff_lossy
    };
    let src_stride = src.stride();
    let dst_stride = dst.stride();

    let compare = |s_at: usize, s_step: usize, d_at: usize, d_step: usize, len: usize| -> bool {
        if is_lossless {
            compare_pixels_lossless(src, s_at, s_step, dst, d_at, d_step, len)
        } else {
            compare_pixels_lossy(src, s_at, s_step, dst, d_at, d_step, len, max_allowed_diff)
        }
    };

    // Left boundary.
    for i in rect.x_offset..rect.x_offset + rect.width {
        let s_at = rect.y_offset as usize * src_stride + i as usize;
        let d_at = rect.y_offset as usize * dst_stride + i as usize;
        if compare(s_at, src_stride, d_at, dst_stride, rect.height as usize) {
            rect.width -= 1; // Redundant column.
            rect.x_offset += 1;
        } else {
            break;
        }
    }
    if rect.width == 0 {
        no_change(rect);
        return;
    }

    // Right boundary.
    let mut i = rect.x_offset + rect.width - 1;
    while i >= rect.x_offset {
        let s_at = rect.y_offset as usize * src_stride + i as usize;
        let d_at = rect.y_offset as usize * dst_stride + i as usize;
        if compare(s_at, src_stride, d_at, dst_stride, rect.height as usize) {
            rect.width -= 1; // Redundant column.
        } else {
            break;
        }
        i -= 1;
    }
    if rect.width == 0 {
        no_change(rect);
        return;
    }

    // Top boundary.
    for j in rect.y_offset..rect.y_offset + rect.height {
        let s_at = j as usize * src_stride + rect.x_offset as usize;
        let d_at = j as usize * dst_stride + rect.x_offset as usize;
        if compare(s_at, 1, d_at, 1, rect.width as usize) {
            rect.height -= 1; // Redundant row.
            rect.y_offset += 1;
        } else {
            break;
        }
    }
    if rect.height == 0 {
        no_change(rect);
        return;
    }

    // Bottom boundary.
    let mut j = rect.y_offset + rect.height - 1;
    while j >= rect.y_offset {
        let s_at = j as usize * src_stride + rect.x_offset as usize;
        let d_at = j as usize * dst_stride + rect.x_offset as usize;
        if compare(s_at, 1, d_at, 1, rect.width as usize) {
            rect.height -= 1; // Redundant row.
        } else {
            break;
        }
        j -= 1;
    }
    if rect.height == 0 {
        no_change(rect);
        return;
    }

    if is_empty_rect(rect) {
        no_change(rect);
    }
}

/// The `NoChange` label of `MinimizeChangeRectangle`: an empty rectangle at the origin.
fn no_change(rect: &mut FrameRectangle) {
    rect.x_offset = 0;
    rect.y_offset = 0;
    rect.width = 0;
    rect.height = 0;
}

/// Port of `SnapToEvenOffsets` (anim_encode.c#L499-L504).
fn snap_to_even_offsets(rect: &mut FrameRectangle) {
    rect.width += rect.x_offset & 1;
    rect.height += rect.y_offset & 1;
    rect.x_offset &= !1;
    rect.y_offset &= !1;
}

/// Port of `GetSubRect` (anim_encode.c#L536-L563) without the `WebPPictureView`: the rectangle
/// of the sub-frame. Returns `false` when the rectangle is empty and not allowed.
// The C signature: the key-frame, first-frame, empty-rectangle and lossless flags are separate.
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn get_sub_rect(
    prev_canvas: &ArgbPicture,
    curr_canvas: &ArgbPicture,
    is_key_frame: bool,
    is_first_frame: bool,
    empty_rect_allowed: bool,
    is_lossless: bool,
    quality: f32,
    rect: &mut FrameRectangle,
) -> bool {
    if !is_key_frame || is_first_frame {
        // Optimize the frame rectangle. For the first frame, 'prev_canvas' is a fully
        // transparent canvas.
        minimize_change_rectangle(prev_canvas, curr_canvas, rect, is_lossless, quality);
    }

    if is_empty_rect(rect) {
        if empty_rect_allowed {
            // No need to get the sub-frame.
            return true;
        }
        // Force a 1x1 rectangle.
        rect.width = 1;
        rect.height = 1;
    }

    snap_to_even_offsets(rect);
    true
}

/// Port of `SubFrameParams` (anim_encode.c#L506-L515): the lossless rectangle (the lossy one is
/// not ported).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SubFrameParams {
    should_try: bool,
    empty_rect_allowed: bool,
    rect_ll: FrameRectangle,
}

/// Port of `GetSubRects` (anim_encode.c#L567-L586), the lossless half: the lossless rectangle
/// seeded with the full canvas.
fn get_sub_rects(
    prev_canvas: &ArgbPicture,
    curr_canvas: &ArgbPicture,
    is_key_frame: bool,
    is_first_frame: bool,
    quality: f32,
    params: &mut SubFrameParams,
) -> bool {
    params.rect_ll = FrameRectangle {
        x_offset: 0,
        y_offset: 0,
        width: curr_canvas.width,
        height: curr_canvas.height,
    };
    get_sub_rect(
        prev_canvas,
        curr_canvas,
        is_key_frame,
        is_first_frame,
        params.empty_rect_allowed,
        true,
        quality,
        &mut params.rect_ll,
    )
}

/// Port of `clip` (anim_encode.c#L588-L590).
fn clip(v: i32, min_v: i32, max_v: i32) -> i32 {
    if v < min_v {
        min_v
    } else if v > max_v {
        max_v
    } else {
        v
    }
}

/// Port of `WebPAnimEncoderRefineRect` (anim_encode.c#L592-L620): shrinks the rectangle
/// `(x, y, w, h)` to the pixels that differ between the canvases. Returns `None` for canvases
/// that do not match.
#[must_use]
#[doc(alias = "WebPAnimEncoderRefineRect")]
// The C signature: the canvases, the lossless flag, the quality and the rectangle.
#[allow(clippy::too_many_arguments)]
pub fn refine_rect(
    prev_canvas: &ArgbPicture,
    curr_canvas: &ArgbPicture,
    is_lossless: bool,
    quality: f32,
    x_offset: i32,
    y_offset: i32,
    width: i32,
    height: i32,
) -> Option<(i32, i32, i32, i32)> {
    if prev_canvas.width != curr_canvas.width || prev_canvas.height != curr_canvas.height {
        return None;
    }
    let right = clip(x_offset + width, 0, curr_canvas.width);
    let left = clip(x_offset, 0, curr_canvas.width - 1);
    let bottom = clip(y_offset + height, 0, curr_canvas.height);
    let top = clip(y_offset, 0, curr_canvas.height - 1);
    let mut rect = FrameRectangle {
        x_offset: left,
        y_offset: top,
        width: 0,
        height: 0,
    };
    rect.width = clip(right - left, 0, curr_canvas.width - rect.x_offset);
    rect.height = clip(bottom - top, 0, curr_canvas.height - rect.y_offset);
    minimize_change_rectangle(prev_canvas, curr_canvas, &mut rect, is_lossless, quality);
    snap_to_even_offsets(&mut rect);
    Some((rect.x_offset, rect.y_offset, rect.width, rect.height))
}

/// Port of `DisposeFrameRectangle` (anim_encode.c#L622-L629).
fn dispose_frame_rectangle(dispose: Dispose, rect: &FrameRectangle, curr_canvas: &mut ArgbPicture) {
    if dispose == Dispose::Background {
        util_clear_pic(curr_canvas, Some(rect));
    }
}

/// Port of `RectArea` (anim_encode.c#L631-L633).
fn rect_area(rect: &FrameRectangle) -> u32 {
    (rect.width as u32).wrapping_mul(rect.height as u32)
}

/// Port of `IsLosslessBlendingPossible` (anim_encode.c#L635-L655).
fn is_lossless_blending_possible(
    src: &ArgbPicture,
    dst: &ArgbPicture,
    rect: &FrameRectangle,
) -> bool {
    let stride = src.stride();
    for j in rect.y_offset..rect.y_offset + rect.height {
        for i in rect.x_offset..rect.x_offset + rect.width {
            let src_pixel = src.argb[j as usize * stride + i as usize];
            let dst_pixel = dst.argb[j as usize * stride + i as usize];
            let dst_alpha = dst_pixel >> 24;
            if dst_alpha != 0xff && src_pixel != dst_pixel {
                // Blending cannot attain the desired 'dst_pixel' value for this pixel.
                return false;
            }
        }
    }
    true
}

/// Port of `WebPConfig`, as the fields the lossless animation encoder reads.
#[derive(Debug, Clone, Copy, PartialEq)]
#[doc(alias = "WebPConfig")]
pub struct FrameConfig {
    /// `config->lossless`. Only lossless configs are ported (see the module comment).
    pub lossless: bool,
    /// `config->method`: the effort, 0 to 6.
    pub method: u32,
    /// `config->quality`.
    pub quality: f32,
    /// `config->exact`: keep the colour of fully transparent pixels.
    pub exact: bool,
}

impl Default for FrameConfig {
    /// Port of `WebPConfigInitInternal` (config_enc.c#L25-L50): `quality` 75, `method` 4, lossy.
    fn default() -> Self {
        Self {
            lossless: false,
            method: 4,
            quality: 75.0,
            exact: false,
        }
    }
}

/// The errors of the animation encoder: the C `WebPEncodingError`, `WebPMuxError` and the
/// `MarkError` cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimError {
    /// Port of `ERROR adding frame: Invalid frame dimensions` and the other invalid arguments.
    InvalidArgument,
    /// Port of `ERROR adding frame: timestamps must be non-decreasing`.
    TimestampsNotIncreasing,
    /// A lossy `WebPConfig`: the lossy candidates are not ported.
    LossyNotPorted,
    /// `WebPEncode` failed.
    Encoding,
    /// `WebPDecode` of a frame failed.
    Decoding,
    /// A `WebPMux` call failed.
    Mux(MuxError),
    /// `WebPAnimEncoderAssemble` with no frames.
    NoFrames,
}

impl From<MuxError> for AnimError {
    fn from(e: MuxError) -> Self {
        Self::Mux(e)
    }
}

/// Port of `EncodedFrame` (anim_encode.c#L43-L47): the candidate sub-frame and key-frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
// The field names are the C names (`sub_frame_`, `key_frame_`, `is_key_frame_`).
#[allow(clippy::struct_field_names)]
struct EncodedFrame {
    sub_frame: OwnedMuxFrame,
    key_frame: OwnedMuxFrame,
    is_key_frame: bool,
}

/// Port of `WebPMuxFrameInfo` as the encoder stores it (an owned bitstream).
#[derive(Debug, Clone, PartialEq, Eq)]
struct OwnedMuxFrame {
    bitstream: Vec<u8>,
    x_offset: i32,
    y_offset: i32,
    duration: i32,
    id: ChunkId,
    dispose: Dispose,
    blend: Blend,
}

impl Default for OwnedMuxFrame {
    /// The zeroed `WebPMuxFrameInfo` of `memset`; the id is set before the frame is used.
    fn default() -> Self {
        Self {
            bitstream: Vec::new(),
            x_offset: 0,
            y_offset: 0,
            duration: 0,
            id: ChunkId::Anmf,
            dispose: Dispose::None,
            blend: Blend::Blend,
        }
    }
}

/// Port of `Candidate` (anim_encode.c#L778-L783): an encoded candidate with its metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    bitstream: Vec<u8>,
    info: OwnedMuxFrame,
    rect: FrameRectangle,
}

/// Port of the `LL_DISP_NONE` and `LL_DISP_BG` indices of `Candidate candidates[]`.
const LL_DISP_NONE: usize = 0;
const LL_DISP_BG: usize = 1;

/// Port of `WebPAnimEncoder` (anim_encode.c#L49-L102), for the lossless candidates.
#[derive(Debug, Clone)]
#[doc(alias = "WebPAnimEncoder")]
pub struct AnimEncoder {
    canvas_width: i32,
    canvas_height: i32,
    options: AnimEncoderOptions,

    prev_rect: FrameRectangle,
    /// `last_config_`: the config of the last `SetFrame`; `None` until one is set.
    last_config: Option<FrameConfig>,

    /// `curr_canvas_`: the frame being added (only set during `add`).
    curr_canvas: Option<ArgbPicture>,
    /// `curr_canvas_copy_`: the possibly modified current canvas.
    curr_canvas_copy: ArgbPicture,
    /// `curr_canvas_copy_modified_`.
    curr_canvas_copy_modified: bool,

    prev_canvas: ArgbPicture,
    prev_canvas_disposed: ArgbPicture,

    encoded_frames: Vec<EncodedFrame>,
    /// `size_`: the number of allocated frames.
    size: usize,
    /// `start_`: the index of the first frame of the cache.
    start: usize,
    /// `count_`: the number of valid frames.
    count: usize,
    /// `flush_count_`: frames from `start` that are ready for the mux.
    flush_count: usize,

    best_delta: i64,
    keyframe: i32,
    count_since_key_frame: i32,

    first_timestamp: i32,
    prev_timestamp: i32,
    prev_candidate_undecided: bool,

    is_first_frame: bool,
    got_null_frame: bool,

    in_frame_count: usize,
    out_frame_count: usize,

    mux: Mux,
}

/// Port of `WebPAnimEncoderNewInternal` (anim_encode.c#L237-L307), with the lossless candidates.
/// `options` of `None` are the defaults (`DefaultEncoderOptions`).
#[must_use]
#[doc(alias = "WebPAnimEncoderNew")]
pub fn anim_encoder_new(
    width: i32,
    height: i32,
    options: Option<&AnimEncoderOptions>,
) -> Option<AnimEncoder> {
    if width <= 0 || height <= 0 || (width as u64) * (height as u64) >= MAX_IMAGE_AREA {
        return None;
    }
    // The options given are sanitized; NULL is the defaults (`DefaultEncoderOptions`).
    let opts = match options {
        Some(o) => {
            let mut o = *o;
            sanitize_options(&mut o);
            o
        }
        None => AnimEncoderOptions::default(),
    };
    let size = i64::from(opts.kmax) - i64::from(opts.kmin) + 1;
    let size = size.max(2) as usize;
    let mut enc = AnimEncoder {
        canvas_width: width,
        canvas_height: height,
        options: opts,
        prev_rect: FrameRectangle::default(),
        last_config: None,
        curr_canvas: None,
        curr_canvas_copy: ArgbPicture::new(width, height),
        curr_canvas_copy_modified: true,
        prev_canvas: ArgbPicture::new(width, height),
        prev_canvas_disposed: ArgbPicture::new(width, height),
        encoded_frames: vec![EncodedFrame::default(); size],
        size,
        start: 0,
        count: 0,
        flush_count: 0,
        best_delta: DELTA_INFINITY,
        keyframe: KEYFRAME_NONE,
        count_since_key_frame: 0,
        first_timestamp: 0,
        prev_timestamp: 0,
        prev_candidate_undecided: false,
        is_first_frame: true,
        got_null_frame: false,
        in_frame_count: 0,
        out_frame_count: 0,
        mux: Mux::new(),
    };
    // The previous canvas starts fully transparent (`WebPUtilClearPic`).
    util_clear_pic(&mut enc.prev_canvas, None);
    Some(enc)
}

/// The pixels of `pic` in `rect`, row by row (`WebPPictureView` read by the encoder).
fn rect_pixels(pic: &ArgbPicture, rect: &FrameRectangle) -> Vec<u32> {
    let stride = pic.stride();
    let mut out = Vec::with_capacity((rect.width * rect.height) as usize);
    for j in 0..rect.height as usize {
        let row = (rect.y_offset as usize + j) * stride + rect.x_offset as usize;
        out.extend_from_slice(&pic.argb[row..row + rect.width as usize]);
    }
    out
}

/// Port of `WebPReplaceTransparentPixels(pic, 0x000000)` on the pixels of `rect` in `pic`, in
/// place (`WebPAlphaReplace`).
fn replace_transparent_in_rect(pic: &mut ArgbPicture, rect: &FrameRectangle) {
    let stride = pic.stride();
    for j in 0..rect.height as usize {
        let row = (rect.y_offset as usize + j) * stride + rect.x_offset as usize;
        for px in &mut pic.argb[row..row + rect.width as usize] {
            if (*px >> 24) == 0 {
                *px = 0;
            }
        }
    }
}

/// Port of `WebPEncode` on a lossless picture (`EncodeFrame`): `WebPReplaceTransparentPixels`
/// when `!exact`, which changes `argb` in place, then `VP8LEncodeImage`.
fn encode_lossless_in_place(
    config: &FrameConfig,
    width: usize,
    height: usize,
    argb: &mut [u32],
) -> Option<Vec<u8>> {
    if !config.exact {
        for px in argb.iter_mut() {
            if (*px >> 24) == 0 {
                *px = 0;
            }
        }
    }
    // The replacement is done; the encoder would not change the pixels again.
    encode_lossless_method(
        width,
        height,
        argb,
        config.method,
        config.quality as i32,
        true,
    )
}

/// `IncreaseTransparency` (anim_encode.c#L685-L703): pixels of `dst` that equal those of `src`
/// become transparent. Returns whether a pixel changed.
fn increase_transparency(src: &ArgbPicture, rect: &FrameRectangle, dst: &mut ArgbPicture) -> bool {
    let stride = src.stride();
    let mut modified = false;
    for j in rect.y_offset..rect.y_offset + rect.height {
        for i in rect.x_offset..rect.x_offset + rect.width {
            let at = j as usize * stride + i as usize;
            if src.argb[at] == dst.argb[at] && dst.argb[at] != TRANSPARENT_COLOR {
                dst.argb[at] = TRANSPARENT_COLOR;
                modified = true;
            }
        }
    }
    modified
}

/// `EncodeCandidate` (anim_encode.c#L786-L826), lossless: the sub-frame of `curr_canvas` in
/// `rect`, encoded. The transparent pixels are replaced in `curr_canvas` itself, as
/// `WebPEncode` does on the `WebPPictureView` of it.
fn encode_candidate(
    curr_canvas: &mut ArgbPicture,
    rect: &FrameRectangle,
    config: &FrameConfig,
    use_blending: bool,
) -> Result<Candidate, AnimError> {
    let mut sub = rect_pixels(curr_canvas, rect);
    let width = rect.width as usize;
    let height = rect.height as usize;
    let bitstream =
        encode_lossless_in_place(config, width, height, &mut sub).ok_or(AnimError::Encoding)?;
    if !config.exact {
        // The view's pixels were replaced in the canvas, as the C code leaves them.
        replace_transparent_in_rect(curr_canvas, rect);
    }
    Ok(Candidate {
        bitstream,
        info: OwnedMuxFrame {
            bitstream: Vec::new(),
            x_offset: rect.x_offset,
            y_offset: rect.y_offset,
            duration: 0,
            id: ChunkId::Anmf,
            dispose: Dispose::None,
            blend: if use_blending {
                Blend::Blend
            } else {
                Blend::NoBlend
            },
        },
        rect: *rect,
    })
}

/// Port of `MAX_DURATION` (`format_constants.h#L80`).
const MAX_DURATION: i32 = 1 << 24;

/// Port of `LOSSLESS_1X1` bytes (anim_encode.c#L963-L967): a 1x1 transparent VP8L frame.
const LOSSLESS_1X1_BYTES: [u8; 28] = [
    0x52, 0x49, 0x46, 0x46, 0x14, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x4c,
    0x08, 0x00, 0x00, 0x00, 0x2f, 0x00, 0x00, 0x00, 0x10, 0x88, 0x88, 0x08,
];

/// Port of `LOSSY_1X1` bytes (anim_encode.c#L971-L978): a 1x1 transparent VP8 frame with ALPH.
const LOSSY_1X1_BYTES: [u8; 72] = [
    0x52, 0x49, 0x46, 0x46, 0x40, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x58,
    0x0a, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x41, 0x4c,
    0x50, 0x48, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x56, 0x50, 0x38, 0x20, 0x18, 0x00, 0x00, 0x00,
    0x30, 0x01, 0x00, 0x9d, 0x01, 0x2a, 0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x34, 0x25, 0xa4, 0x00,
    0x03, 0x70, 0x00, 0xfe, 0xfb, 0xfd, 0x50, 0x00,
];

/// Port of `key_frame_penalty` (`KeyFramePenalty`, anim_encode.c#L1191-L1194).
fn key_frame_penalty(encoded_frame: &EncodedFrame) -> i64 {
    encoded_frame.key_frame.bitstream.len() as i64 - encoded_frame.sub_frame.bitstream.len() as i64
}

impl AnimEncoder {
    /// Port of `CopyCurrentCanvas` (anim_encode.c#L828-L835): the canvas copy is re-copied from the
    /// frame when it was modified.
    fn copy_current_canvas(&mut self) {
        if self.curr_canvas_copy_modified {
            if let Some(curr) = &self.curr_canvas {
                self.curr_canvas_copy.argb.copy_from_slice(&curr.argb);
            }
            self.curr_canvas_copy_modified = false;
        }
    }

    /// Port of `IncreasePreviousDuration` (anim_encode.c#L945-L1005).
    fn increase_previous_duration(&mut self, duration: i32) {
        let prev_idx = self.start + self.count - 1;
        let new_duration = self.encoded_frames[prev_idx].sub_frame.duration + duration;
        if new_duration >= MAX_DURATION {
            // Separate the previous frame from the earlier merged frames to avoid overflow: add a
            // 1x1 transparent frame for the previous frame, with blending on.
            let rect = FrameRectangle {
                x_offset: 0,
                y_offset: 0,
                width: 1,
                height: 1,
            };
            let can_use_lossless =
                self.last_config.is_some_and(|c| c.lossless) || self.options.allow_mixed;
            let curr_idx = self.start + self.count;
            let curr = &mut self.encoded_frames[curr_idx];
            curr.is_key_frame = false;
            curr.sub_frame.id = ChunkId::Anmf;
            curr.sub_frame.x_offset = 0;
            curr.sub_frame.y_offset = 0;
            curr.sub_frame.dispose = Dispose::None;
            curr.sub_frame.blend = Blend::Blend;
            curr.sub_frame.duration = duration;
            curr.sub_frame.bitstream = if can_use_lossless {
                LOSSLESS_1X1_BYTES.to_vec()
            } else {
                LOSSY_1X1_BYTES.to_vec()
            };
            self.count += 1;
            self.count_since_key_frame += 1;
            self.flush_count = self.count - 1;
            self.prev_candidate_undecided = false;
            self.prev_rect = rect;
        } else {
            // Regular case: increase the duration of the previous frame.
            let prev = &mut self.encoded_frames[prev_idx];
            prev.sub_frame.duration = new_duration;
            prev.key_frame.duration = new_duration;
        }
    }

    /// Port of `SetPreviousDisposeMethod` (anim_encode.c#L927-L943).
    fn set_previous_dispose_method(&mut self, dispose: Dispose) {
        let idx = self.start + self.count - 2;
        let undecided = self.prev_candidate_undecided;
        let prev = &mut self.encoded_frames[idx];
        if undecided {
            // The undecided frame is a sub-frame with DISPOSE_NONE; both variants are updated.
            prev.sub_frame.dispose = dispose;
            prev.key_frame.dispose = dispose;
        } else {
            let info = if prev.is_key_frame {
                &mut prev.key_frame
            } else {
                &mut prev.sub_frame
            };
            info.dispose = dispose;
        }
    }

    /// Port of `PickBestCandidate` (anim_encode.c#L1011-L1052): the smallest candidate is kept, the
    /// others are released.
    fn pick_best_candidate(
        &mut self,
        candidates: &mut [Option<Candidate>; 2],
        is_key_frame: bool,
        encoded_idx: usize,
    ) -> Result<(), AnimError> {
        let mut best_idx: Option<usize> = None;
        let mut best_size = usize::MAX;
        for (i, candidate) in candidates.iter().enumerate() {
            if let Some(c) = candidate
                && c.bitstream.len() < best_size
            {
                best_idx = Some(i);
                best_size = c.bitstream.len();
            }
        }
        let best = best_idx.ok_or(AnimError::Encoding)?;
        for (i, slot) in candidates.iter_mut().enumerate() {
            let Some(c) = slot.take() else {
                continue;
            };
            if i != best {
                continue;
            }
            let mut info = c.info;
            info.bitstream = c.bitstream;
            {
                let dst = if is_key_frame {
                    &mut self.encoded_frames[encoded_idx].key_frame
                } else {
                    &mut self.encoded_frames[encoded_idx].sub_frame
                };
                *dst = info;
            }
            if !is_key_frame {
                // The previous dispose method only matters for non-key-frames.
                let prev_dispose = if best == LL_DISP_NONE {
                    Dispose::None
                } else {
                    Dispose::Background
                };
                self.set_previous_dispose_method(prev_dispose);
            }
            self.prev_rect = c.rect;
        }
        Ok(())
    }

    /// Port of `GenerateCandidates` (anim_encode.c#L850-L915), lossless: the candidate for one
    /// dispose method, with blending where it is possible.
    fn generate_candidates(
        &mut self,
        dispose: Dispose,
        is_key_frame: bool,
        params: &SubFrameParams,
        config: &FrameConfig,
        candidates: &mut [Option<Candidate>; 2],
    ) -> Result<(), AnimError> {
        let is_dispose_none = dispose == Dispose::None;
        let idx = if is_dispose_none {
            LL_DISP_NONE
        } else {
            LL_DISP_BG
        };

        self.copy_current_canvas();
        let use_blending = !is_key_frame && {
            let prev = if is_dispose_none {
                &self.prev_canvas
            } else {
                &self.prev_canvas_disposed
            };
            is_lossless_blending_possible(prev, &self.curr_canvas_copy, &params.rect_ll)
        };

        // Lossless candidate: with `allow_mixed` off, only the lossless candidates are tried.
        self.copy_current_canvas();
        if use_blending {
            let prev = if is_dispose_none {
                &self.prev_canvas
            } else {
                &self.prev_canvas_disposed
            };
            self.curr_canvas_copy_modified =
                increase_transparency(prev, &params.rect_ll, &mut self.curr_canvas_copy);
        }
        let candidate = encode_candidate(
            &mut self.curr_canvas_copy,
            &params.rect_ll,
            config,
            use_blending,
        )?;
        candidates[idx] = Some(candidate);
        Ok(())
    }

    /// Port of `SetFrame` (anim_encode.c#L1058-L1187), lossless. Returns `true` when the frame is
    /// skipped (its rectangle is empty and merging with the previous frame is possible).
    fn set_frame(
        &mut self,
        config: &FrameConfig,
        is_key_frame: bool,
        encoded_idx: usize,
    ) -> Result<bool, AnimError> {
        let is_first_frame = self.is_first_frame;
        // The first frame cannot be skipped: there is no previous frame to merge it into.
        let empty_rect_allowed_none = !is_first_frame;
        // Even with an exact match against the disposed canvas, the frame cannot be skipped.
        let empty_rect_allowed_bg = false;
        // With key-frames, or with a previous frame that may still become a key-frame, the
        // dispose-to-background candidate is not tried.
        let dispose_bg_possible = !is_key_frame && !self.prev_candidate_undecided;

        self.last_config = Some(*config);
        let mut dispose_none_params = SubFrameParams {
            should_try: true,
            empty_rect_allowed: empty_rect_allowed_none,
            rect_ll: FrameRectangle::default(),
        };
        let mut dispose_bg_params = SubFrameParams {
            should_try: false,
            empty_rect_allowed: empty_rect_allowed_bg,
            rect_ll: FrameRectangle::default(),
        };
        let mut candidates: [Option<Candidate>; 2] = [None, None];

        // Change-rectangle assuming the previous frame was DISPOSE_NONE.
        get_sub_rects(
            &self.prev_canvas,
            &self.curr_canvas_copy,
            is_key_frame,
            is_first_frame,
            config.quality,
            &mut dispose_none_params,
        );
        if is_empty_rect(&dispose_none_params.rect_ll) {
            // The frame is not encoded: the duration of the previous frame is increased later.
            return Ok(true);
        }

        if dispose_bg_possible {
            // Change-rectangle assuming the previous frame was DISPOSE_BACKGROUND.
            self.prev_canvas_disposed
                .argb
                .copy_from_slice(&self.prev_canvas.argb);
            dispose_frame_rectangle(
                Dispose::Background,
                &self.prev_rect,
                &mut self.prev_canvas_disposed,
            );
            get_sub_rects(
                &self.prev_canvas_disposed,
                &self.curr_canvas_copy,
                is_key_frame,
                is_first_frame,
                config.quality,
                &mut dispose_bg_params,
            );

            if self.options.minimize_size {
                // Try both dispose methods.
                dispose_bg_params.should_try = true;
                dispose_none_params.should_try = true;
            } else if rect_area(&dispose_bg_params.rect_ll)
                < rect_area(&dispose_none_params.rect_ll)
            {
                dispose_bg_params.should_try = true; // Pick DISPOSE_BACKGROUND.
                dispose_none_params.should_try = false;
            }
        }

        if dispose_none_params.should_try {
            self.generate_candidates(
                Dispose::None,
                is_key_frame,
                &dispose_none_params,
                config,
                &mut candidates,
            )?;
        }
        if dispose_bg_params.should_try {
            self.generate_candidates(
                Dispose::Background,
                is_key_frame,
                &dispose_bg_params,
                config,
                &mut candidates,
            )?;
        }

        self.pick_best_candidate(&mut candidates, is_key_frame, encoded_idx)?;
        Ok(false)
    }

    /// Port of `CacheFrame` (anim_encode.c#L1196-L1295), lossless.
    fn cache_frame(&mut self, config: &FrameConfig) -> Result<(), AnimError> {
        let position = self.count;
        let encoded_idx = self.start + position;
        self.count += 1;

        let frame_skipped: bool;
        if self.is_first_frame {
            // Add this as a key-frame.
            frame_skipped = self.set_frame(config, true, encoded_idx)?;
            self.encoded_frames[encoded_idx].is_key_frame = true;
            self.flush_count = 0;
            self.count_since_key_frame = 0;
            self.prev_candidate_undecided = false;
        } else {
            self.count_since_key_frame += 1;
            if self.count_since_key_frame <= self.options.kmin {
                // Add this as a frame rectangle.
                frame_skipped = self.set_frame(config, false, encoded_idx)?;
                if !frame_skipped {
                    self.encoded_frames[encoded_idx].is_key_frame = false;
                    self.flush_count = self.count - 1;
                    self.prev_candidate_undecided = false;
                }
            } else {
                // Add this as a frame rectangle, and as a key-frame, and keep the smaller.
                frame_skipped = self.set_frame(config, false, encoded_idx)?;
                if !frame_skipped {
                    let prev_rect_sub = self.prev_rect;
                    self.set_frame(config, true, encoded_idx)?;
                    let prev_rect_key = self.prev_rect;

                    let curr_delta = key_frame_penalty(&self.encoded_frames[encoded_idx]);
                    if curr_delta <= self.best_delta {
                        // Pick this as the key-frame.
                        if self.keyframe != KEYFRAME_NONE {
                            let old = self.start + self.keyframe as usize;
                            self.encoded_frames[old].is_key_frame = false;
                        }
                        self.encoded_frames[encoded_idx].is_key_frame = true;
                        self.prev_candidate_undecided = true;
                        self.keyframe = position as i32;
                        self.best_delta = curr_delta;
                        self.flush_count = self.count - 1; // We can flush previous frames.
                    } else {
                        self.encoded_frames[encoded_idx].is_key_frame = false;
                        self.prev_candidate_undecided = false;
                    }
                    // Note: '>=' because when kmin and kmax are both zero, count_since_key_frame
                    // is always greater than kmax.
                    if self.count_since_key_frame >= self.options.kmax {
                        self.flush_count = self.count - 1;
                        self.count_since_key_frame = 0;
                        self.keyframe = KEYFRAME_NONE;
                        self.best_delta = DELTA_INFINITY;
                    }
                    if !self.prev_candidate_undecided {
                        self.prev_rect = if self.encoded_frames[encoded_idx].is_key_frame {
                            prev_rect_key
                        } else {
                            prev_rect_sub
                        };
                    }
                }
            }
        }

        if !frame_skipped {
            // Update the previous canvas for the next call, from the frame and not the copy.
            if let Some(curr) = &self.curr_canvas {
                self.prev_canvas.argb.copy_from_slice(&curr.argb);
            }
            self.is_first_frame = false;
        }

        self.in_frame_count += 1;
        if frame_skipped {
            self.encoded_frames[encoded_idx] = EncodedFrame::default();
            // Reset the counters, as the frame was skipped.
            self.count -= 1;
            if !self.is_first_frame {
                self.count_since_key_frame -= 1;
            }
        }
        Ok(())
    }

    /// Port of `FlushFrames` (anim_encode.c#L1297-L1332): the frames that are ready go to the mux.
    fn flush_frames(&mut self) -> Result<(), AnimError> {
        while self.flush_count > 0 {
            let curr_idx = self.start;
            {
                let curr = &self.encoded_frames[curr_idx];
                let info = if curr.is_key_frame {
                    &curr.key_frame
                } else {
                    &curr.sub_frame
                };
                let frame = FrameInfo {
                    bitstream: &info.bitstream,
                    x_offset: info.x_offset,
                    y_offset: info.y_offset,
                    duration: info.duration,
                    id: info.id,
                    dispose: info.dispose,
                    blend: info.blend,
                };
                self.mux.push_frame(&frame)?;
            }
            self.out_frame_count += 1;
            self.encoded_frames[curr_idx] = EncodedFrame::default();
            self.start += 1;
            self.flush_count -= 1;
            self.count -= 1;
            if self.keyframe != KEYFRAME_NONE {
                self.keyframe -= 1;
            }
        }

        if self.count == 1 && self.start != 0 {
            // Move `start` to index 0.
            let start = self.start;
            self.encoded_frames.swap(0, start);
            self.encoded_frames[start] = EncodedFrame::default();
            self.start = 0;
        }
        Ok(())
    }

    /// Port of `WebPAnimEncoderAdd` (anim_encode.c#L1337-L1421). `frame` of `None` is the last
    /// call, which only signals the end of the timestamps. `config` of `None` is the lossless
    /// default.
    ///
    /// # Errors
    ///
    /// Returns the `AnimError` the C function reports as a failure. A lossy `config` is
    /// `LossyNotPorted`.
    #[doc(alias = "WebPAnimEncoderAdd")]
    pub fn add(
        &mut self,
        frame: Option<&ArgbPicture>,
        timestamp: i32,
        config: Option<FrameConfig>,
    ) -> Result<(), AnimError> {
        if self.is_first_frame {
            self.first_timestamp = timestamp;
        } else {
            // Make sure timestamps are non-decreasing (integer wrap-around is OK).
            let prev_frame_duration = (timestamp as u32).wrapping_sub(self.prev_timestamp as u32);
            if prev_frame_duration >= MAX_DURATION as u32 {
                return Err(AnimError::TimestampsNotIncreasing);
            }
            self.increase_previous_duration(prev_frame_duration as i32);
            // IncreasePreviousDuration() may add a frame: flush before the cache overflows.
            if self.count == self.size {
                self.flush_frames()?;
            }
        }

        let Some(frame) = frame else {
            // Special: last call.
            self.got_null_frame = true;
            self.prev_timestamp = timestamp;
            return Ok(());
        };

        if frame.width != self.canvas_width || frame.height != self.canvas_height {
            return Err(AnimError::InvalidArgument);
        }

        let config = config.unwrap_or(FrameConfig {
            lossless: true,
            ..FrameConfig::default()
        });
        if !config.lossless {
            return Err(AnimError::LossyNotPorted);
        }

        self.curr_canvas = Some(frame.clone());
        self.copy_current_canvas();

        let ok = self.cache_frame(&config).and_then(|()| self.flush_frames());

        self.curr_canvas = None;
        self.curr_canvas_copy_modified = true;
        ok?;
        self.prev_timestamp = timestamp;
        Ok(())
    }

    /// Port of `DecodeFrameOntoCanvas` (anim_encode.c#L1426-L1453): the frame's pixels are drawn
    /// onto the cleared canvas at the frame's offset.
    fn decode_frame_onto_canvas(
        frame: &OwnedFrame,
        canvas: &mut ArgbPicture,
    ) -> Result<(), AnimError> {
        util_clear_pic(canvas, None);
        let features =
            crate::webp_dec::get_features(&frame.bitstream).map_err(|_| AnimError::Decoding)?;
        let (w, h) = (features.width, features.height);
        let (x, y) = (frame.x_offset, frame.y_offset);
        if x < 0 || y < 0 || w <= 0 || h <= 0 || x + w > canvas.width || y + h > canvas.height {
            return Err(AnimError::Decoding);
        }
        let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
        crate::webp_dec::decode(
            &frame.bitstream,
            crate::lossless::CspMode::Bgra,
            &mut buf,
            (w as usize) * 4,
        )
        .map_err(|_| AnimError::Decoding)?;
        let stride = canvas.stride();
        for j in 0..h as usize {
            for i in 0..w as usize {
                let at = (j * w as usize + i) * 4;
                // BGRA bytes, read little-endian, are the 0xAARRGGBB word.
                let px = u32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]);
                canvas.argb[(y as usize + j) * stride + x as usize + i] = px;
            }
        }
        Ok(())
    }

    /// Port of `FrameToFullCanvas` (anim_encode.c#L1455-L1482), lossless: the frame drawn on the
    /// canvas copy, encoded as a full image.
    fn frame_to_full_canvas(&mut self, frame: &OwnedFrame) -> Result<Vec<u8>, AnimError> {
        Self::decode_frame_onto_canvas(frame, &mut self.curr_canvas_copy)?;
        let config = self.last_config.unwrap_or_default();
        if !config.lossless {
            return Err(AnimError::LossyNotPorted);
        }
        let (w, h) = (
            self.curr_canvas_copy.width as usize,
            self.curr_canvas_copy.height as usize,
        );
        encode_lossless_in_place(&config, w, h, &mut self.curr_canvas_copy.argb)
            .ok_or(AnimError::Encoding)
    }

    /// Port of `OptimizeSingleFrame` (anim_encode.c#L1487-L1527): a single-frame animation becomes
    /// a still image when that is smaller.
    fn optimize_single_frame(&mut self, webp_data: Vec<u8>) -> Result<Vec<u8>, AnimError> {
        let mut mux = Mux::create(&webp_data).ok_or(MuxError::BadData)?;
        let frame = mux.get_frame(1)?;
        if frame.id != ChunkId::Anmf {
            return Ok(webp_data); // Non-animation: nothing to do.
        }
        mux.canvas_size()?;
        let full_image = self.frame_to_full_canvas(&frame)?;
        mux.set_image(&full_image)?;
        let webp_data2 = mux.assemble()?;
        if webp_data2.len() < webp_data.len() {
            Ok(webp_data2)
        } else {
            Ok(webp_data)
        }
    }

    /// Port of `WebPAnimEncoderAssemble` (anim_encode.c#L1529-L1585): the animated WebP file.
    ///
    /// # Errors
    ///
    /// Returns the `AnimError` the C function reports as a failure.
    #[doc(alias = "WebPAnimEncoderAssemble")]
    pub fn assemble(&mut self) -> Result<Vec<u8>, AnimError> {
        if self.in_frame_count == 0 {
            return Err(AnimError::NoFrames);
        }
        if !self.got_null_frame && self.in_frame_count > 1 && self.count > 0 {
            // Set the duration of the last frame to the average of the previous durations.
            let delta_time =
                f64::from((self.prev_timestamp as u32).wrapping_sub(self.first_timestamp as u32));
            // The C code converts the frame count to double as well.
            #[allow(clippy::cast_precision_loss)]
            let average_duration = (delta_time / (self.in_frame_count - 1) as f64) as i32;
            self.increase_previous_duration(average_duration);
        }

        // Flush any remaining frames.
        self.flush_count = self.count;
        self.flush_frames()?;

        // The definitive canvas size and the animation parameters.
        self.mux
            .set_canvas_size(self.canvas_width, self.canvas_height)?;
        self.mux.set_animation_params(self.options.anim_params)?;

        let data = self.mux.assemble()?;
        if self.out_frame_count == 1 {
            return self.optimize_single_frame(data);
        }
        Ok(data)
    }

    /// Port of `WebPAnimEncoderSetChunk`.
    ///
    /// # Errors
    ///
    /// Returns the `MuxError` the C function would return.
    pub fn set_chunk(&mut self, fourcc: [u8; 4], data: &[u8]) -> Result<(), MuxError> {
        self.mux.set_chunk(fourcc, data)
    }

    /// Port of `WebPAnimEncoderGetChunk`.
    ///
    /// # Errors
    ///
    /// Returns the `MuxError` the C function would return.
    pub fn get_chunk(&self, fourcc: [u8; 4]) -> Result<&[u8], MuxError> {
        self.mux.get_chunk(fourcc)
    }

    /// Port of `WebPAnimEncoderDeleteChunk`.
    ///
    /// # Errors
    ///
    /// Returns the `MuxError` the C function would return.
    pub fn delete_chunk(&mut self, fourcc: [u8; 4]) -> Result<(), MuxError> {
        self.mux.delete_chunk(fourcc)
    }
}
