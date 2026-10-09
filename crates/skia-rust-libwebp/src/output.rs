// Copyright 2011 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the COPYING file. Port by The skia-rust Authors.

//! Port of the output side of libwebp's lossy decoder: `FinishRow` (`src/dec/frame_dec.c`), the
//! fancy upsampler and YUV→RGB conversion (`src/dsp/upsampling.c`, `src/dsp/yuv.h`), and the
//! custom I/O that emits rows and alpha (`src/dec/io_dec.c`: `EmitFancyRGB`, `EmitAlphaRGB`,
//! `CustomPut`).
//!
//! Rows are emitted in the same batches libwebp uses. The fancy upsampler keeps the last luma and
//! chroma row of one batch for the first row pair of the next, so the batch boundaries (the loop
//! filter's extra rows and the crop window) decide the output.

// Module-level clippy allows. Each one mirrors the C source of this module.
// clippy::cast_possible_truncation: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_possible_wrap: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_sign_loss: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::manual_div_ceil: mirrors the C expression (mb_w + 1) / 2 for the chroma width.
// clippy::manual_midpoint: mirrors the C expression (a + b) >> 1 of the upsampler.
// clippy::many_single_char_names: pixel, offset and loop variables keep the single-letter names of the C source.
// clippy::needless_pass_by_value: mirrors the C signature, which passes the struct by value.
// clippy::similar_names: local names mirror the C identifiers, which differ by one letter or a suffix.
// clippy::struct_field_names: field names mirror the C struct members.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::manual_div_ceil,
    clippy::manual_midpoint,
    clippy::many_single_char_names,
    clippy::needless_pass_by_value,
    clippy::similar_names,
    clippy::struct_field_names
)]

use crate::io::{Io, is_alpha_mode, is_premultiplied_mode};
use crate::lossless::{CspMode, apply_alpha_multiply};
use crate::vp8_dec::{Crop, Planes};
use crate::vp8_tables_small::K_FILTER_EXTRA_ROWS;

/// Port of `YUV_FIX2` and `YUV_MASK2` (`yuv.h`).
const YUV_FIX2: i32 = 6;
const YUV_MASK2: i32 = (256 << YUV_FIX2) - 1;

/// Port of `MultHi`: `(v * coeff) >> 8`.
#[inline]
fn mult_hi(v: i32, coeff: i32) -> i32 {
    (v * coeff) >> 8
}

/// Port of `VP8Clip8`.
#[inline]
fn clip8(v: i32) -> i32 {
    if (v & !YUV_MASK2) == 0 {
        v >> YUV_FIX2
    } else if v < 0 {
        0
    } else {
        255
    }
}

/// Port of `VP8YUVToR`.
#[inline]
fn yuv_to_r(y: i32, v: i32) -> i32 {
    clip8(mult_hi(y, 19077) + mult_hi(v, 26149) - 14234)
}

/// Port of `VP8YUVToG`.
#[inline]
fn yuv_to_g(y: i32, u: i32, v: i32) -> i32 {
    clip8(mult_hi(y, 19077) - mult_hi(u, 6419) - mult_hi(v, 13320) + 8708)
}

/// Port of `VP8YUVToB`.
#[inline]
fn yuv_to_b(y: i32, u: i32) -> i32 {
    clip8(mult_hi(y, 19077) + mult_hi(u, 33050) - 17685)
}

/// Bytes per pixel of the colour spaces the fancy upsampler writes, or `None` if unsupported.
fn bytes_per_pixel(cs: CspMode) -> Option<usize> {
    match cs {
        CspMode::Rgba
        | CspMode::Bgra
        | CspMode::RgbA
        | CspMode::BgrA
        | CspMode::Argb
        | CspMode::ArgbPremultiplied => Some(4),
        CspMode::Rgb | CspMode::Bgr => Some(3),
        CspMode::Rgb565 => Some(2),
        _ => None,
    }
}

/// Port of the per-pixel `VP8YuvTo*` functions. `y`, `u`, `v` are `uint8_t` in C.
#[inline]
fn yuv_pixel(cs: CspMode, y: u8, u: u8, v: u8, dst: &mut [u8]) {
    let (y, u, v) = (i32::from(y), i32::from(u), i32::from(v));
    match cs {
        // VP8YuvToRgba / VP8YuvToRgb / VP8YuvToArgb / VP8YuvToBgra / VP8YuvToBgr
        CspMode::Rgba | CspMode::RgbA => {
            dst[0] = yuv_to_r(y, v) as u8;
            dst[1] = yuv_to_g(y, u, v) as u8;
            dst[2] = yuv_to_b(y, u) as u8;
            dst[3] = 0xff;
        }
        CspMode::Bgra | CspMode::BgrA => {
            dst[0] = yuv_to_b(y, u) as u8;
            dst[1] = yuv_to_g(y, u, v) as u8;
            dst[2] = yuv_to_r(y, v) as u8;
            dst[3] = 0xff;
        }
        CspMode::Argb | CspMode::ArgbPremultiplied => {
            dst[0] = 0xff;
            dst[1] = yuv_to_r(y, v) as u8;
            dst[2] = yuv_to_g(y, u, v) as u8;
            dst[3] = yuv_to_b(y, u) as u8;
        }
        CspMode::Rgb => {
            dst[0] = yuv_to_r(y, v) as u8;
            dst[1] = yuv_to_g(y, u, v) as u8;
            dst[2] = yuv_to_b(y, u) as u8;
        }
        CspMode::Bgr => {
            dst[0] = yuv_to_b(y, u) as u8;
            dst[1] = yuv_to_g(y, u, v) as u8;
            dst[2] = yuv_to_r(y, v) as u8;
        }
        CspMode::Rgb565 => {
            // VP8YuvToRgb565 with WEBP_SWAP_16BIT_CSP == 1: gb first, then rg.
            let r = yuv_to_r(y, v);
            let g = yuv_to_g(y, u, v);
            let b = yuv_to_b(y, u);
            let rg = (r & 0xf8) | (g >> 5);
            let gb = ((g << 3) & 0xe0) | (b >> 3);
            dst[0] = gb as u8;
            dst[1] = rg as u8;
        }
        _ => {}
    }
}

/// Where one fancy-upsampled row pair goes: `top` is the row of `top_y`, `bottom` the row of
/// `bottom_y` (if any), as byte offsets in the output.
struct PairOut {
    top: usize,
    bottom: Option<usize>,
}

/// Port of `UPSAMPLE_FUNC`: the fancy upsampler for one pair of luma rows.
///
/// `top_u`/`top_v` and `cur_u`/`cur_v` are the chroma rows above and at the pair. The packed
/// `u | v << 16` arithmetic is the C code's, lane for lane.
#[allow(clippy::too_many_arguments)] // mirrors the C signature of the upsampler
fn upsample_line_pair(
    cs: CspMode,
    top_y: &[u8],
    bottom_y: Option<&[u8]>,
    top_u: &[u8],
    top_v: &[u8],
    cur_u: &[u8],
    cur_v: &[u8],
    out: &mut [u8],
    dst: PairOut,
    len: usize,
) {
    let xstep = bytes_per_pixel(cs).unwrap_or(4);
    let load_uv = |u: u8, v: u8| -> u32 { u32::from(u) | (u32::from(v) << 16) };
    let last_pixel_pair = (len - 1) >> 1;
    let mut tl_uv = load_uv(top_u[0], top_v[0]); // top-left sample
    let mut l_uv = load_uv(cur_u[0], cur_v[0]); // left-sample
    {
        let uv0 = (3 * tl_uv + l_uv + 0x0002_0002) >> 2;
        yuv_pixel(
            cs,
            top_y[0],
            (uv0 & 0xff) as u8,
            (uv0 >> 16) as u8,
            &mut out[dst.top..],
        );
    }
    if let (Some(by), Some(bd)) = (bottom_y, dst.bottom) {
        let uv0 = (3 * l_uv + tl_uv + 0x0002_0002) >> 2;
        yuv_pixel(
            cs,
            by[0],
            (uv0 & 0xff) as u8,
            (uv0 >> 16) as u8,
            &mut out[bd..],
        );
    }
    for x in 1..=last_pixel_pair {
        let t_uv = load_uv(top_u[x], top_v[x]); // top sample
        let uv = load_uv(cur_u[x], cur_v[x]); // sample
        // precompute invariant values associated with first and second diagonals
        let avg = tl_uv + t_uv + l_uv + uv + 0x0008_0008;
        let diag_12 = (avg + 2 * (t_uv + l_uv)) >> 3;
        let diag_03 = (avg + 2 * (tl_uv + uv)) >> 3;
        {
            let uv0 = (diag_12 + tl_uv) >> 1;
            let uv1 = (diag_03 + t_uv) >> 1;
            let t = dst.top;
            yuv_pixel(
                cs,
                top_y[2 * x - 1],
                (uv0 & 0xff) as u8,
                (uv0 >> 16) as u8,
                &mut out[t + (2 * x - 1) * xstep..],
            );
            yuv_pixel(
                cs,
                top_y[2 * x],
                (uv1 & 0xff) as u8,
                (uv1 >> 16) as u8,
                &mut out[t + 2 * x * xstep..],
            );
        }
        if let (Some(by), Some(bd)) = (bottom_y, dst.bottom) {
            let uv0 = (diag_03 + l_uv) >> 1;
            let uv1 = (diag_12 + uv) >> 1;
            yuv_pixel(
                cs,
                by[2 * x - 1],
                (uv0 & 0xff) as u8,
                (uv0 >> 16) as u8,
                &mut out[bd + (2 * x - 1) * xstep..],
            );
            yuv_pixel(
                cs,
                by[2 * x],
                (uv1 & 0xff) as u8,
                (uv1 >> 16) as u8,
                &mut out[bd + 2 * x * xstep..],
            );
        }
        tl_uv = t_uv;
        l_uv = uv;
    }
    if len & 1 == 0 {
        {
            let uv0 = (3 * tl_uv + l_uv + 0x0002_0002) >> 2;
            yuv_pixel(
                cs,
                top_y[len - 1],
                (uv0 & 0xff) as u8,
                (uv0 >> 16) as u8,
                &mut out[dst.top + (len - 1) * xstep..],
            );
        }
        if let (Some(by), Some(bd)) = (bottom_y, dst.bottom) {
            let uv0 = (3 * l_uv + tl_uv + 0x0002_0002) >> 2;
            yuv_pixel(
                cs,
                by[len - 1],
                (uv0 & 0xff) as u8,
                (uv0 >> 16) as u8,
                &mut out[bd + (len - 1) * xstep..],
            );
        }
    }
}

/// The temporary rows `EmitFancyRGB` keeps between batches (`p->tmp_y`, `tmp_u`, `tmp_v`).
#[derive(Debug, Default)]
struct FancyState {
    tmp_y: Vec<u8>,
    tmp_u: Vec<u8>,
    tmp_v: Vec<u8>,
}

/// Port of `EmitFancyRGB` for one batch.
///
/// `y_row` and `uv_row` are the absolute plane rows of the batch's first row (`io->y`, `io->u`
/// after the crop adjustment). The columns start at `io.crop_left` (and half of it for chroma).
/// The output rows start at `io.mb_y` within `io.out`.
fn emit_fancy_rgb(
    st: &mut FancyState,
    planes: &Planes,
    io: &mut Io<'_>,
    y_row: usize,
    uv_row: usize,
) {
    let cs = io.colorspace;
    let mb_w = io.mb_w as usize;
    let mb_h = io.mb_h as usize;
    let uv_w = (mb_w + 1) / 2;
    let ys = planes.y_stride;
    let uvs = planes.uv_stride;
    let out_stride = io.out_stride;
    let crop_left = io.crop_left as usize;
    if st.tmp_y.len() != mb_w {
        st.tmp_y = vec![0; mb_w];
        st.tmp_u = vec![0; uv_w];
        st.tmp_v = vec![0; uv_w];
    }
    // Relative to the window: `y` is the batch start, `y_end` its end (as in C).
    let y_start_rel = io.mb_y as usize;
    let y_end = y_start_rel + mb_h;
    let mut y = y_start_rel;
    let mut cur_y = y_row * ys + crop_left;
    let mut cur_uv = uv_row * uvs + crop_left / 2; // same index in the u and v planes
    let mut dst = y_start_rel * out_stride;

    let (pu, pv, py) = (&planes.u, &planes.v, &planes.y);
    if y == 0 {
        // The first row of the window: top and current chroma are the same row.
        upsample_line_pair(
            cs,
            &py[cur_y..],
            None,
            &pu[cur_uv..],
            &pv[cur_uv..],
            &pu[cur_uv..],
            &pv[cur_uv..],
            io.out,
            PairOut {
                top: dst,
                bottom: None,
            },
            mb_w,
        );
    } else {
        // Previous batch's last row (tmp) on top, this batch's first row below it.
        upsample_line_pair(
            cs,
            &st.tmp_y,
            Some(&py[cur_y..]),
            &st.tmp_u,
            &st.tmp_v,
            &pu[cur_uv..],
            &pv[cur_uv..],
            io.out,
            PairOut {
                top: dst - out_stride,
                bottom: Some(dst),
            },
            mb_w,
        );
    }
    while y + 2 < y_end {
        let top_uv = cur_uv;
        cur_uv += uvs;
        dst += 2 * out_stride;
        cur_y += 2 * ys;
        upsample_line_pair(
            cs,
            &py[cur_y - ys..],
            Some(&py[cur_y..]),
            &pu[top_uv..],
            &pv[top_uv..],
            &pu[cur_uv..],
            &pv[cur_uv..],
            io.out,
            PairOut {
                top: dst - out_stride,
                bottom: Some(dst),
            },
            mb_w,
        );
        y += 2;
    }
    cur_y += ys;
    if io.crop_top as usize + y_end < io.crop_bottom as usize {
        // Keep this batch's last row for the next batch's first pair.
        st.tmp_y.copy_from_slice(&py[cur_y..cur_y + mb_w]);
        st.tmp_u.copy_from_slice(&pu[cur_uv..cur_uv + uv_w]);
        st.tmp_v.copy_from_slice(&pv[cur_uv..cur_uv + uv_w]);
    } else if y_end & 1 == 0 {
        upsample_line_pair(
            cs,
            &py[cur_y..],
            None,
            &pu[cur_uv..],
            &pv[cur_uv..],
            &pu[cur_uv..],
            &pv[cur_uv..],
            io.out,
            PairOut {
                top: dst + out_stride,
                bottom: None,
            },
            mb_w,
        );
    }
}

/// Port of `EmitAlphaRGB` for one batch: copies the alpha rows into the alpha byte of the RGBA
/// rows, and premultiplies them when the output is premultiplied and has transparency.
///
/// `alpha_row` and `width` locate the batch's alpha rows in the whole-frame alpha plane (the
/// `io->a` pointer after the crop adjustment).
fn emit_alpha_rgb(io: &mut Io<'_>, alpha: &[u8], alpha_row: usize, width: usize, fancy: bool) {
    let cs = io.colorspace;
    let mb_w = io.mb_w as usize;
    let alpha_first = matches!(cs, CspMode::Argb | CspMode::ArgbPremultiplied);
    // GetAlphaSourceRow
    let mut start_y = io.mb_y as usize;
    let mut num_rows = io.mb_h as usize;
    let mut a_row = alpha_row;
    if fancy {
        if start_y == 0 {
            num_rows -= 1;
        } else {
            start_y -= 1;
            a_row -= 1;
        }
        if io.crop_top as usize + io.mb_y as usize + io.mb_h as usize == io.crop_bottom as usize {
            num_rows = (io.crop_bottom - io.crop_top) as usize - start_y;
        }
    }
    let stride = io.out_stride;
    let base = start_y * stride;
    let a0 = a_row * width + io.crop_left as usize;
    let dst_off = base + if alpha_first { 0 } else { 3 };
    let mut alpha_mask: u8 = 0xff;
    for j in 0..num_rows {
        for i in 0..mb_w {
            let a = alpha[a0 + j * width + i];
            io.out[dst_off + j * stride + 4 * i] = a;
            alpha_mask &= a;
        }
    }
    if alpha_mask != 0xff && is_premultiplied_mode(cs) {
        apply_alpha_multiply(&mut io.out[base..], alpha_first, mb_w, num_rows, stride);
    }
}

/// Port of `FinishRow` plus `CustomPut`: emits the decoded frame into `io` in libwebp's batches.
///
/// `planes` is the filtered frame, `alpha` its alpha plane (`width * height`, if the image has an
/// `ALPH` chunk), `crop` the window, and `br_mb_y` the number of macroblock rows that were decoded.
/// Returns `false` for colour spaces this port does not produce.
#[doc(alias = "FinishRow")]
pub fn emit_frame(
    planes: &Planes,
    alpha: Option<&[u8]>,
    io: &mut Io<'_>,
    crop: Crop,
    br_mb_y: usize,
) -> bool {
    if bytes_per_pixel(io.colorspace).is_none() {
        return false;
    }
    let _ = crop;
    let extra_y_rows = K_FILTER_EXTRA_ROWS[usize::from(planes.filter_type)] as usize;
    let mut st = FancyState::default();
    for mb_y in 0..br_mb_y {
        let is_first_row = mb_y == 0;
        let is_last_row = mb_y + 1 >= br_mb_y;
        let mut y_start = 16 * mb_y;
        let mut y_end = 16 * (mb_y + 1);
        if !is_first_row {
            y_start -= extra_y_rows;
        }
        if !is_last_row {
            y_end -= extra_y_rows;
        }
        if y_end > io.crop_bottom as usize {
            y_end = io.crop_bottom as usize; // make sure we don't overflow on last row.
        }
        let mut y_row = y_start;
        let mut uv_row = y_start / 2;
        let mut a_row = y_start;
        if y_start < io.crop_top as usize {
            let delta_y = io.crop_top as usize - y_start;
            y_start = io.crop_top as usize;
            y_row += delta_y;
            uv_row += delta_y >> 1;
            a_row += delta_y;
        }
        if y_start < y_end {
            io.mb_y = (y_start - io.crop_top as usize) as i32;
            io.mb_w = io.crop_right - io.crop_left;
            io.mb_h = (y_end - y_start) as i32;
            emit_fancy_rgb(&mut st, planes, io, y_row, uv_row);
            // CustomSetup installs EmitAlphaRGB only for the colour spaces with an alpha channel.
            if let Some(a) = alpha.filter(|_| is_alpha_mode(io.colorspace)) {
                emit_alpha_rgb(io, a, a_row, planes.width as usize, true);
            }
            io.last_y += io.mb_h;
        }
    }
    true
}
