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

// WIP: the encoder is being ported in pieces; this allow is removed when `WebPAnimEncoderAdd` and
// `WebPAnimEncoderAssemble` use every helper.
#![allow(dead_code, unused_imports)]
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

use super::{AnimParams, Blend, ChunkId, Dispose, FrameInfo, Mux, MuxError};

/// Port of `MAX_IMAGE_AREA` (format_constants.h).
const MAX_IMAGE_AREA: u64 = 1 << 32;
/// Port of `DELTA_INFINITY`.
const DELTA_INFINITY: i64 = 1 << 32;
/// Port of `KEYFRAME_NONE`.
const KEYFRAME_NONE: i32 = -1;
/// Port of `MAX_CACHED_FRAMES` (anim_encode.c#L124).
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
            eprintln!("WARNING: Setting kmin = {}, so that kmin < kmax.", options.kmin);
        }
    } else {
        let kmin_limit = options.kmax / 2 + 1;
        if options.kmin < kmin_limit && kmin_limit < options.kmax {
            // This ensures that enc.keyframe + kmin >= kmax is always true.
            options.kmin = kmin_limit;
            if print_warning {
                eprintln!("WARNING: Setting kmin = {}, so that kmin >= kmax / 2 + 1.", options.kmin);
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
    match rect {
        Some(r) => clear_rectangle(picture, r.x_offset, r.y_offset, r.width, r.height),
        None => {
            let (w, h) = (picture.width, picture.height);
            clear_rectangle(picture, 0, 0, w, h);
        }
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
    let max_allowed_diff = if is_lossless { 0 } else { max_allowed_diff_lossy };
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
