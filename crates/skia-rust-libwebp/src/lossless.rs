// Copyright 2014 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the COPYING file. Port by The skia-rust Authors.

//! Port of libwebp `src/dsp/lossless.c` (the C kernels), `src/dsp/lossless_common.h`,
//! `src/utils/color_cache_utils.{c,h}` and the decoder's transform descriptor.
//!
//! The predictor, colour and colour-index inverse transforms, the BGRA to output-format
//! conversions and the colour cache are all integer code with a fixed definition, so this port
//! is the C path. libwebp's SSE2/SSE4.1 replacements of these kernels are checked against it by
//! `oracle/codec-diff`.
//!
//! Buffers are slices. Where the C code reads the row above the output (`out - width`) or the
//! pixel before it (`out[-1]`), the caller passes the slice starting there and the index
//! arithmetic is unchanged.

// Module-level clippy allows. Each one mirrors the C source of this module.
// clippy::cast_possible_truncation: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_possible_wrap: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_sign_loss: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

/// Port of `ARGB_BLACK` (`format_constants.h`).
pub const ARGB_BLACK: u32 = 0xff00_0000;
/// Port of `kHashMul` (`color_cache_utils.h`).
const K_HASH_MUL: u32 = 0x1e35_a7bd;

/// Port of `VP8LImageTransformType` (`vp8li_dec.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "VP8LImageTransformType")]
pub enum TransformType {
    PredictorTransform = 0,
    CrossColorTransform = 1,
    SubtractGreenTransform = 2,
    ColorIndexingTransform = 3,
}

/// Port of `VP8LTransform`.
#[derive(Debug, Clone, Default)]
#[doc(alias = "VP8LTransform")]
pub struct Transform {
    pub ttype: Option<TransformType>,
    pub bits: i32,
    pub xsize: i32,
    pub ysize: i32,
    pub data: Vec<u32>,
}

/// Port of `VP8LSubSampleSize`.
#[inline]
#[must_use]
pub fn sub_sample_size(size: i32, sampling_bits: i32) -> i32 {
    (size + (1 << sampling_bits) - 1) >> sampling_bits
}

/// Port of `VP8LAddPixels`: per-channel addition modulo 256.
#[inline]
#[must_use]
pub fn add_pixels(a: u32, b: u32) -> u32 {
    let alpha_and_green = (a & 0xff00_ff00).wrapping_add(b & 0xff00_ff00);
    let red_and_blue = (a & 0x00ff_00ff).wrapping_add(b & 0x00ff_00ff);
    (alpha_and_green & 0xff00_ff00) | (red_and_blue & 0x00ff_00ff)
}

/// Port of `Average2`.
#[inline]
fn average2(a0: u32, a1: u32) -> u32 {
    (((a0 ^ a1) & 0xfefe_fefe) >> 1).wrapping_add(a0 & a1)
}

/// Port of `Average3`.
#[inline]
fn average3(a0: u32, a1: u32, a2: u32) -> u32 {
    average2(average2(a0, a2), a1)
}

/// Port of `Average4`.
#[inline]
fn average4(a0: u32, a1: u32, a2: u32, a3: u32) -> u32 {
    average2(average2(a0, a1), average2(a2, a3))
}

/// Port of `Clip255`.
#[inline]
fn clip255(a: u32) -> u32 {
    if a < 256 {
        return a;
    }
    // return 0, when a is a negative integer.
    // return 255, when a is positive.
    (!a) >> 24
}

/// Port of `AddSubtractComponentFull`.
#[inline]
fn add_subtract_component_full(a: i32, b: i32, c: i32) -> i32 {
    clip255(a.wrapping_add(b).wrapping_sub(c) as u32) as i32
}

/// Port of `ClampedAddSubtractFull`.
#[inline]
fn clamped_add_subtract_full(c0: u32, c1: u32, c2: u32) -> u32 {
    let a = add_subtract_component_full((c0 >> 24) as i32, (c1 >> 24) as i32, (c2 >> 24) as i32);
    let r = add_subtract_component_full(
        ((c0 >> 16) & 0xff) as i32,
        ((c1 >> 16) & 0xff) as i32,
        ((c2 >> 16) & 0xff) as i32,
    );
    let g = add_subtract_component_full(
        ((c0 >> 8) & 0xff) as i32,
        ((c1 >> 8) & 0xff) as i32,
        ((c2 >> 8) & 0xff) as i32,
    );
    let b = add_subtract_component_full((c0 & 0xff) as i32, (c1 & 0xff) as i32, (c2 & 0xff) as i32);
    (u32::try_from(a).unwrap_or(0) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Port of `AddSubtractComponentHalf`. `(a - b) / 2` truncates toward zero, as in C.
#[inline]
fn add_subtract_component_half(a: i32, b: i32) -> i32 {
    clip255(a.wrapping_add((a - b) / 2) as u32) as i32
}

/// Port of `ClampedAddSubtractHalf`.
#[inline]
fn clamped_add_subtract_half(c0: u32, c1: u32, c2: u32) -> u32 {
    let ave = average2(c0, c1);
    let a = add_subtract_component_half((ave >> 24) as i32, (c2 >> 24) as i32);
    let r = add_subtract_component_half(((ave >> 16) & 0xff) as i32, ((c2 >> 16) & 0xff) as i32);
    let g = add_subtract_component_half(((ave >> 8) & 0xff) as i32, ((c2 >> 8) & 0xff) as i32);
    let b = add_subtract_component_half((ave & 0xff) as i32, (c2 & 0xff) as i32);
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Port of `Sub3`.
#[inline]
fn sub3(a: i32, b: i32, c: i32) -> i32 {
    let pb = b - c;
    let pa = a - c;
    pb.abs() - pa.abs()
}

/// Port of `Select`.
#[inline]
fn select(a: u32, b: u32, c: u32) -> u32 {
    let pa_minus_pb = sub3((a >> 24) as i32, (b >> 24) as i32, (c >> 24) as i32)
        + sub3(
            ((a >> 16) & 0xff) as i32,
            ((b >> 16) & 0xff) as i32,
            ((c >> 16) & 0xff) as i32,
        )
        + sub3(
            ((a >> 8) & 0xff) as i32,
            ((b >> 8) & 0xff) as i32,
            ((c >> 8) & 0xff) as i32,
        )
        + sub3((a & 0xff) as i32, (b & 0xff) as i32, (c & 0xff) as i32);
    if pa_minus_pb <= 0 { a } else { b }
}

/// The predictor `VP8LPredictor<mode>_C` for `mode` in `0..=13`, with the neighbours it reads:
/// `left` is `*left`, `t_m1`, `t0` and `t1` are `top[-1]`, `top[0]` and `top[1]`.
///
/// Modes 14 and 15 map to predictor 0 in libwebp (`VP8LDspInit`), and this function does the same.
#[inline]
#[must_use]
pub fn predict(mode: u32, left: u32, t_m1: u32, t0: u32, t1: u32) -> u32 {
    match mode {
        1 => left,
        2 => t0,
        3 => t1,
        4 => t_m1,
        5 => average3(left, t0, t1),
        6 => average2(left, t_m1),
        7 => average2(left, t0),
        8 => average2(t_m1, t0),
        9 => average2(t0, t1),
        10 => average4(left, t_m1, t0, t1),
        11 => select(t0, left, t_m1),
        12 => clamped_add_subtract_full(left, t0, t_m1),
        13 => clamped_add_subtract_half(left, t0, t_m1),
        _ => ARGB_BLACK,
    }
}

/// Port of `PredictorAdd<mode>_C` (the `GENERATE_PREDICTOR_ADD` family) for `num_pixels` pixels.
///
/// `input[in_idx..]` holds the residuals. `out` holds the output row together with the row above
/// it: `upper` is the index in `out` of `top[0]` for the first pixel, and `out_idx` the index of the
/// first output pixel (the left neighbour is `out[out_idx - 1]`, as in C).
fn predictor_add_run(
    mode: u32,
    input: &[u32],
    in_idx: usize,
    upper: usize,
    out: &mut [u32],
    out_idx: usize,
    num_pixels: usize,
) {
    for x in 0..num_pixels {
        let left = out[out_idx + x - 1];
        let t_m1 = if upper + x >= 1 {
            out[upper + x - 1]
        } else {
            0
        };
        let t0 = out[upper + x];
        let t1 = out[upper + x + 1];
        let pred = predict(mode, left, t_m1, t0, t1);
        out[out_idx + x] = add_pixels(input[in_idx + x], pred);
    }
}

/// Port of `PredictorInverseTransform_C`.
///
/// `input` starts at the first row of the range (`in` in C). `out` starts one row above the range
/// (`out - width` in C) so that the top row is available at `out[..width]`.
fn predictor_inverse_transform(
    t: &Transform,
    y_start: i32,
    y_end: i32,
    input: &[u32],
    out: &mut [u32],
) {
    let width = t.xsize as usize;
    let mut in_idx = 0usize;
    // `out_idx` is the index of the current output row inside `out`; row -1 is `out[..width]`.
    let mut out_idx = width;
    let mut y = y_start;
    if y_start == 0 {
        // First Row follows the L (mode=1) mode.
        out[out_idx] = add_pixels(input[in_idx], ARGB_BLACK);
        for x in 1..width {
            out[out_idx + x] = add_pixels(input[in_idx + x], out[out_idx + x - 1]);
        }
        in_idx += width;
        out_idx += width;
        y += 1;
    }
    let tile_width = 1usize << t.bits;
    let mask = tile_width - 1;
    let tiles_per_row = sub_sample_size(t.xsize, t.bits) as usize;
    let mut pred_mode_base = (y as usize >> t.bits) * tiles_per_row;
    while y < y_end {
        let mut pred_mode_src = pred_mode_base;
        // First pixel follows the T (mode=2) mode.
        out[out_idx] = add_pixels(input[in_idx], out[out_idx - width]);
        let mut x = 1usize;
        while x < width {
            let mode = (t.data[pred_mode_src] >> 8) & 0xf;
            pred_mode_src += 1;
            let mut x_end = (x & !mask) + tile_width;
            if x_end > width {
                x_end = width;
            }
            // upper = out + x - width, expressed as an index of `out`.
            predictor_add_run(
                mode,
                input,
                in_idx + x,
                out_idx + x - width,
                out,
                out_idx + x,
                x_end - x,
            );
            x = x_end;
        }
        in_idx += width;
        out_idx += width;
        y += 1;
        if (y as usize & mask) == 0 {
            // Use the same mask, since tiles are squares.
            pred_mode_base += tiles_per_row;
        }
    }
}

/// Port of `VP8LAddGreenToBlueAndRed_C`.
fn add_green_to_blue_and_red(src: &[u32], num_pixels: usize, dst: &mut [u32]) {
    for i in 0..num_pixels {
        let argb = src[i];
        let green = (argb >> 8) & 0xff;
        let mut red_blue = argb & 0x00ff_00ff;
        red_blue = red_blue.wrapping_add((green << 16) | green);
        red_blue &= 0x00ff_00ff;
        dst[i] = (argb & 0xff00_ff00) | red_blue;
    }
}

/// Port of `ColorTransformDelta`.
#[inline]
fn color_transform_delta(color_pred: i8, color: i8) -> i32 {
    (i32::from(color_pred) * i32::from(color)) >> 5
}

/// Port of `VP8LMultipliers` with `ColorCodeToMultipliers`.
#[inline]
fn color_code_to_multipliers(color_code: u32) -> (i8, i8, i8) {
    // (green_to_red, green_to_blue, red_to_blue); the C fields are uint8_t, read as int8_t below.
    let green_to_red = (color_code & 0xff) as u8 as i8;
    let green_to_blue = ((color_code >> 8) & 0xff) as u8 as i8;
    let red_to_blue = ((color_code >> 16) & 0xff) as u8 as i8;
    (green_to_red, green_to_blue, red_to_blue)
}

/// Port of `VP8LTransformColorInverse_C`.
fn transform_color_inverse(m: (i8, i8, i8), src: &[u32], num_pixels: usize, dst: &mut [u32]) {
    let (green_to_red, green_to_blue, red_to_blue) = m;
    for i in 0..num_pixels {
        let argb = src[i];
        let green = (argb >> 8) as u8 as i8;
        let red = argb >> 16;
        let mut new_red = (red & 0xff) as i32;
        let mut new_blue = (argb & 0xff) as i32;
        new_red += color_transform_delta(green_to_red, green);
        new_red &= 0xff;
        new_blue += color_transform_delta(green_to_blue, green);
        new_blue += color_transform_delta(red_to_blue, new_red as u8 as i8);
        new_blue &= 0xff;
        dst[i] = (argb & 0xff00_ff00) | ((new_red as u32) << 16) | (new_blue as u32);
    }
}

/// Port of `ColorSpaceInverseTransform_C`.
fn color_space_inverse_transform(
    t: &Transform,
    y_start: i32,
    y_end: i32,
    src: &[u32],
    dst: &mut [u32],
) {
    let width = t.xsize as usize;
    let tile_width = 1usize << t.bits;
    let mask = tile_width - 1;
    let safe_width = width & !mask;
    let remaining_width = width - safe_width;
    let tiles_per_row = sub_sample_size(t.xsize, t.bits) as usize;
    let mut y = y_start as usize;
    let mut pred_row = (y >> t.bits) * tiles_per_row;
    let mut src_idx = 0usize;
    let mut dst_idx = 0usize;
    while y < y_end as usize {
        let mut pred = pred_row;
        let src_safe_end = src_idx + safe_width;
        let src_end = src_idx + width;
        while src_idx < src_safe_end {
            let m = color_code_to_multipliers(t.data[pred]);
            pred += 1;
            transform_color_inverse(m, &src[src_idx..], tile_width, &mut dst[dst_idx..]);
            src_idx += tile_width;
            dst_idx += tile_width;
        }
        if src_idx < src_end {
            // Left-overs using C-version.
            let m = color_code_to_multipliers(t.data[pred]);
            transform_color_inverse(m, &src[src_idx..], remaining_width, &mut dst[dst_idx..]);
            src_idx += remaining_width;
            dst_idx += remaining_width;
        }
        y += 1;
        if (y & mask) == 0 {
            pred_row += tiles_per_row;
        }
    }
}

/// Port of `ColorIndexInverseTransform_C` (the 32-bit, ARGB-indexed variant).
fn color_index_inverse_transform(
    t: &Transform,
    y_start: i32,
    y_end: i32,
    src: &[u32],
    dst: &mut [u32],
) {
    let bits_per_pixel = 8 >> t.bits;
    let width = t.xsize as usize;
    let color_map = &t.data;
    let mut src_idx = 0usize;
    let mut dst_idx = 0usize;
    if bits_per_pixel < 8 {
        let pixels_per_byte = 1usize << t.bits;
        let count_mask = pixels_per_byte - 1;
        let bit_mask = (1u32 << bits_per_pixel) - 1;
        for _y in y_start..y_end {
            let mut packed_pixels = 0u32;
            for x in 0..width {
                if (x & count_mask) == 0 {
                    packed_pixels = (src[src_idx] >> 8) & 0xff;
                    src_idx += 1;
                }
                dst[dst_idx] = color_map[(packed_pixels & bit_mask) as usize];
                dst_idx += 1;
                packed_pixels >>= bits_per_pixel;
            }
        }
    } else {
        for _y in y_start..y_end {
            for _x in 0..width {
                let idx = (src[src_idx] >> 8) & 0xff;
                src_idx += 1;
                dst[dst_idx] = color_map[idx as usize];
                dst_idx += 1;
            }
        }
    }
}

/// Port of `COLOR_INDEX_INVERSE` for `uint8_t` (`VP8LColorIndexInverseTransformAlpha`): maps the
/// alpha-plane indices to the green channel of the colour map. `src` and `dst` are byte rows.
#[doc(alias = "VP8LColorIndexInverseTransformAlpha")]
pub fn color_index_inverse_transform_alpha(
    t: &Transform,
    y_start: i32,
    y_end: i32,
    src: &[u8],
    dst: &mut [u8],
) {
    let bits_per_pixel = 8 >> t.bits;
    let width = t.xsize as usize;
    let color_map = &t.data;
    let mut src_idx = 0usize;
    let mut dst_idx = 0usize;
    if bits_per_pixel < 8 {
        let pixels_per_byte = 1usize << t.bits;
        let count_mask = pixels_per_byte - 1;
        let bit_mask = (1u32 << bits_per_pixel) - 1;
        for _y in y_start..y_end {
            let mut packed_pixels = 0u32;
            for x in 0..width {
                if (x & count_mask) == 0 {
                    packed_pixels = u32::from(src[src_idx]);
                    src_idx += 1;
                }
                dst[dst_idx] = ((color_map[(packed_pixels & bit_mask) as usize] >> 8) & 0xff) as u8;
                dst_idx += 1;
                packed_pixels >>= bits_per_pixel;
            }
        }
    } else {
        for _y in y_start..y_end {
            for _x in 0..width {
                let idx = src[src_idx];
                src_idx += 1;
                dst[dst_idx] = ((color_map[idx as usize] >> 8) & 0xff) as u8;
                dst_idx += 1;
            }
        }
    }
}

/// Port of `VP8LInverseTransform`.
///
/// `input` holds the rows `row_start..row_end` of the transform's input. `out` starts one row above
/// the output range, so that `out[..width]` is the top row used by the predictor (as in C). When the
/// C code is called with `in == out` the caller passes a copy of the input (see the decoder).
#[doc(alias = "VP8LInverseTransform")]
pub fn inverse_transform(
    t: &Transform,
    row_start: i32,
    row_end: i32,
    input: &[u32],
    out: &mut [u32],
) {
    let width = t.xsize as usize;
    match t.ttype {
        Some(TransformType::SubtractGreenTransform) => {
            let n = (row_end - row_start) as usize * width;
            add_green_to_blue_and_red(input, n, &mut out[width..]);
        }
        Some(TransformType::PredictorTransform) => {
            predictor_inverse_transform(t, row_start, row_end, input, out);
            if row_end != t.ysize {
                // The last predicted row in this iteration will be the top-pred row for the first
                // row in the next iteration.
                let src = width + (row_end - row_start - 1) as usize * width;
                for i in 0..width {
                    out[i] = out[src + i];
                }
            }
        }
        Some(TransformType::CrossColorTransform) => {
            color_space_inverse_transform(t, row_start, row_end, input, &mut out[width..]);
        }
        Some(TransformType::ColorIndexingTransform) => {
            color_index_inverse_transform(t, row_start, row_end, input, &mut out[width..]);
        }
        None => {}
    }
}

/// Output colour spaces of libwebp's `WEBP_CSP_MODE` that the RGB(A) path can produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "WEBP_CSP_MODE")]
pub enum CspMode {
    Rgb = 0,
    Rgba = 1,
    Bgr = 2,
    Bgra = 3,
    Argb = 4,
    Rgba4444 = 5,
    Rgb565 = 6,
    RgbA = 7,
    BgrA = 8,
    /// `MODE_Argb`: premultiplied ARGB.
    ArgbPremultiplied = 9,
    RgbA4444 = 10,
}

/// Port of `VP8LConvertBGRAToRGB_C`.
fn convert_bgra_to_rgb(src: &[u32], dst: &mut [u8]) {
    for (i, &argb) in src.iter().enumerate() {
        dst[3 * i] = ((argb >> 16) & 0xff) as u8;
        dst[3 * i + 1] = ((argb >> 8) & 0xff) as u8;
        dst[3 * i + 2] = (argb & 0xff) as u8;
    }
}

/// Port of `VP8LConvertBGRAToRGBA_C`.
fn convert_bgra_to_rgba(src: &[u32], dst: &mut [u8]) {
    for (i, &argb) in src.iter().enumerate() {
        dst[4 * i] = ((argb >> 16) & 0xff) as u8;
        dst[4 * i + 1] = ((argb >> 8) & 0xff) as u8;
        dst[4 * i + 2] = (argb & 0xff) as u8;
        dst[4 * i + 3] = ((argb >> 24) & 0xff) as u8;
    }
}

/// Port of `VP8LConvertBGRAToRGBA4444_C` with `WEBP_SWAP_16BIT_CSP == 1` (Skia's build).
fn convert_bgra_to_rgba4444(src: &[u32], dst: &mut [u8]) {
    for (i, &argb) in src.iter().enumerate() {
        let rg = (((argb >> 16) & 0xf0) | ((argb >> 12) & 0xf)) as u8;
        let ba = (((argb) & 0xf0) | ((argb >> 28) & 0xf)) as u8;
        dst[2 * i] = ba;
        dst[2 * i + 1] = rg;
    }
}

/// Port of `VP8LConvertBGRAToRGB565_C` with `WEBP_SWAP_16BIT_CSP == 1` (Skia's build).
fn convert_bgra_to_rgb565(src: &[u32], dst: &mut [u8]) {
    for (i, &argb) in src.iter().enumerate() {
        let rg = (((argb >> 16) & 0xf8) | ((argb >> 13) & 0x7)) as u8;
        let gb = (((argb >> 5) & 0xe0) | ((argb >> 3) & 0x1f)) as u8;
        dst[2 * i] = gb;
        dst[2 * i + 1] = rg;
    }
}

/// Port of `VP8LConvertBGRAToBGR_C`.
fn convert_bgra_to_bgr(src: &[u32], dst: &mut [u8]) {
    for (i, &argb) in src.iter().enumerate() {
        dst[3 * i] = (argb & 0xff) as u8;
        dst[3 * i + 1] = ((argb >> 8) & 0xff) as u8;
        dst[3 * i + 2] = ((argb >> 16) & 0xff) as u8;
    }
}

/// Port of `CopyOrSwap` on a little-endian host (`is_big_endian() == 0`).
///
/// With `swap_on_big_endian` true the C code takes the `memcpy` branch, so the bytes are the
/// little-endian bytes of the word (B, G, R, A for BGRA). With it false the C code writes
/// `BSwap32(argb)` through `WebPUint32ToMem`, which stores the big-endian bytes (A, R, G, B).
fn copy_or_swap(src: &[u32], dst: &mut [u8], swap_on_big_endian: bool) {
    for (i, &argb) in src.iter().enumerate() {
        let bytes = if swap_on_big_endian {
            argb.to_le_bytes()
        } else {
            argb.to_be_bytes()
        };
        dst[4 * i..4 * i + 4].copy_from_slice(&bytes);
    }
}

/// Port of `VP8LConvertFromBGRA`. Only the RGB-family modes Skia requests are supported by the
/// caller; the YUV modes are not ported (see the crate documentation).
#[doc(alias = "VP8LConvertFromBGRA")]
pub fn convert_from_bgra(in_data: &[u32], out_colorspace: CspMode, rgba: &mut [u8]) {
    let n = in_data.len();
    match out_colorspace {
        CspMode::Rgb => convert_bgra_to_rgb(in_data, rgba),
        CspMode::Rgba => convert_bgra_to_rgba(in_data, rgba),
        CspMode::RgbA => {
            convert_bgra_to_rgba(in_data, rgba);
            apply_alpha_multiply(&mut rgba[..4 * n], false, n, 1, 0);
        }
        CspMode::Bgr => convert_bgra_to_bgr(in_data, rgba),
        CspMode::Bgra => copy_or_swap(in_data, rgba, true),
        CspMode::BgrA => {
            copy_or_swap(in_data, rgba, true);
            apply_alpha_multiply(&mut rgba[..4 * n], false, n, 1, 0);
        }
        CspMode::Argb => copy_or_swap(in_data, rgba, false),
        CspMode::ArgbPremultiplied => {
            copy_or_swap(in_data, rgba, false);
            apply_alpha_multiply(&mut rgba[..4 * n], true, n, 1, 0);
        }
        CspMode::Rgba4444 => convert_bgra_to_rgba4444(in_data, rgba),
        CspMode::RgbA4444 => {
            convert_bgra_to_rgba4444(in_data, rgba);
            crate::alpha_processing::apply_alpha_multiply_16b(&mut rgba[..2 * n], n, 1, 0);
        }
        CspMode::Rgb565 => convert_bgra_to_rgb565(in_data, rgba),
    }
}

/// Port of `ApplyAlphaMultiply_C` (`alpha_processing.c`) with the `(x * a * 32897) >> 23` rule.
#[doc(alias = "WebPApplyAlphaMultiply")]
pub fn apply_alpha_multiply(rgba: &mut [u8], alpha_first: bool, w: usize, h: usize, stride: usize) {
    let mut base = 0usize;
    for _ in 0..h {
        let rgb_off = base + usize::from(alpha_first);
        let alpha_off = base + if alpha_first { 0 } else { 3 };
        for i in 0..w {
            let a = u32::from(rgba[alpha_off + 4 * i]);
            if a != 0xff {
                let mult = a.wrapping_mul(32897);
                for c in 0..3 {
                    let idx = rgb_off + 4 * i + c;
                    rgba[idx] = (((u32::from(rgba[idx])).wrapping_mul(mult)) >> 23) as u8;
                }
            }
        }
        base += stride;
    }
}

/// Port of `VP8LColorCache`.
#[derive(Debug, Clone, Default)]
#[doc(alias = "VP8LColorCache")]
pub struct ColorCache {
    pub colors: Vec<u32>,
    pub hash_shift: i32,
    pub hash_bits: i32,
}

/// Port of `VP8LHashPix`.
#[inline]
#[must_use]
pub fn hash_pix(argb: u32, shift: i32) -> usize {
    (argb.wrapping_mul(K_HASH_MUL) >> shift) as usize
}

impl ColorCache {
    /// Port of `VP8LColorCacheInit`.
    #[doc(alias = "VP8LColorCacheInit")]
    #[must_use]
    pub fn new(hash_bits: i32) -> Self {
        Self {
            colors: vec![0; 1usize << hash_bits],
            hash_shift: 32 - hash_bits,
            hash_bits,
        }
    }

    /// Port of `VP8LColorCacheLookup`.
    #[inline]
    #[must_use]
    pub fn lookup(&self, key: u32) -> u32 {
        self.colors[key as usize]
    }

    /// Port of `VP8LColorCacheInsert`.
    #[inline]
    pub fn insert(&mut self, argb: u32) {
        let key = hash_pix(argb, self.hash_shift);
        self.colors[key] = argb;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_pixels_wraps_per_channel() {
        assert_eq!(add_pixels(0x80ff_0102, 0x8001_0203), 0x0000_0305);
    }

    #[test]
    fn predictor_mode_zero_is_opaque_black() {
        assert_eq!(predict(0, 1, 2, 3, 4), ARGB_BLACK);
        assert_eq!(predict(15, 1, 2, 3, 4), ARGB_BLACK);
    }
}
