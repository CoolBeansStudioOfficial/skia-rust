// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the YUV import of libwebp 1.4.0 (`src/enc/picture_csp_enc.c`, the RGBA/RGBX path of
//! `Import` into `ImportYUVAFromRGBA`), `WebPPictureAllocYUVA` (`picture_enc.c`), and the YUV
//! branch of `WebPCleanupTransparentArea` (`picture_tools_enc.c`), with the RGB-to-YUV helpers of
//! `src/dsp/yuv.h` and `ConvertRGBA32ToUV_C` (`src/dsp/yuv.c`), and `ExtractAlpha_C`
//! (`src/dsp/alpha_processing.c`).
//!
//! `SkWebpEncoder` (lossy) imports with `WebPPictureImportRGBA` or `WebPPictureImportRGBX` and
//! `use_argb = 0`, with `config->use_sharp_yuv = 0`, `preprocessing = 0` and `exact = 0`. That
//! reaches only the non-iterative, undithered conversion of `ImportYUVAFromRGBA` with
//! `step = 4`, so the iterative (sharp) conversion, the dithered conversion, the `step == 3`
//! DSP shortcut and the ARGB branch of `WebPCleanupTransparentArea` are not ported; they are not
//! reached (see `docs/design/codecs.md` §7).
//!
//! The gamma tables are computed with `pow` (the host libm, marked `skia-rust: libm`), as the C
//! `InitGammaTables` does; see the note on [`gamma_tables`].

// The C arithmetic mixes int, uint32_t and uint8_t, and the casts below are the width and sign
// conversions of the C source. The index loops keep the C control flow for readability.
// Clippy allows for the C arithmetic and control flow: the C code mixes int, uint32_t
// and uint8_t, spells table offsets as `0 + 0 * BPS`, nests the mode trees as `if` chains,
// and indexes by position. The port keeps those shapes so that each line can be checked
// against the C source; the casts are the width and sign conversions of the C source.
#![allow(
    clippy::identity_op,
    clippy::erasing_op,
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::too_many_arguments,
    clippy::bool_to_int_with_if,
    clippy::unreadable_literal,
    clippy::if_not_else,
    clippy::needless_pass_by_value,
    clippy::items_after_statements,
    clippy::float_cmp,
    clippy::int_plus_one,
    clippy::precedence,
    clippy::unusual_byte_groupings
)]

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::manual_range_contains,
    clippy::fn_params_excessive_bools,
    clippy::struct_excessive_bools
)]

use std::sync::OnceLock;

/// Port of `GAMMA_FIX`: fixed-point precision for linear values.
const GAMMA_FIX: i32 = 12;
/// Port of `GAMMA_TAB_FIX`: fixed-point fractional bits of the gamma table.
const GAMMA_TAB_FIX: i32 = 7;
/// Port of `GAMMA_TAB_SIZE`.
const GAMMA_TAB_SIZE: usize = 1 << (GAMMA_FIX - GAMMA_TAB_FIX);
/// Port of `kGamma`.
const K_GAMMA: f64 = 0.80;
/// Port of `kGammaScale`.
const K_GAMMA_SCALE: i32 = (1 << GAMMA_FIX) - 1;
/// Port of `kGammaTabScale`.
const K_GAMMA_TAB_SCALE: i32 = 1 << GAMMA_TAB_FIX;
/// Port of `kGammaTabRounder`.
const K_GAMMA_TAB_ROUNDER: i32 = (1 << GAMMA_TAB_FIX) >> 1;

/// Port of `kAlphaFix`.
const K_ALPHA_FIX: i32 = 19;

/// Port of `YUV_FIX` (`src/dsp/yuv.h`): fixed-point precision for RGB->YUV.
const YUV_FIX: i32 = 16;
/// Port of `YUV_HALF`.
const YUV_HALF: i32 = 1 << (YUV_FIX - 1);

/// The gamma tables of `picture_csp_enc.c` (`kGammaToLinearTab`, `kLinearToGammaTab`).
struct GammaTables {
    gamma_to_linear: [u16; 256],
    linear_to_gamma: [i32; GAMMA_TAB_SIZE + 1],
}

/// Port of `InitGammaTables`. The tables are rounded from `pow` results, so they depend on the
/// host libm only where a `pow` result lies within one ulp of a rounding boundary.
fn gamma_tables() -> &'static GammaTables {
    static TABLES: OnceLock<GammaTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut gamma_to_linear = [0u16; 256];
        let mut linear_to_gamma = [0i32; GAMMA_TAB_SIZE + 1];
        let scale = f64::from(1 << GAMMA_TAB_FIX) / f64::from(K_GAMMA_SCALE);
        let norm = 1. / 255.;
        for v in 0..=255 {
            // skia-rust: libm (pow)
            gamma_to_linear[v] =
                ((norm * v as f64).powf(K_GAMMA) * f64::from(K_GAMMA_SCALE) + 0.5) as u16;
        }
        for v in 0..=GAMMA_TAB_SIZE {
            // skia-rust: libm (pow)
            linear_to_gamma[v] = (255. * (scale * v as f64).powf(1. / K_GAMMA) + 0.5) as i32;
        }
        GammaTables {
            gamma_to_linear,
            linear_to_gamma,
        }
    })
}

/// Port of `GammaToLinear`.
#[inline]
fn gamma_to_linear(v: u8) -> u32 {
    u32::from(gamma_tables().gamma_to_linear[usize::from(v)])
}

/// Port of `Interpolate`.
#[inline]
fn interpolate(v: i32) -> i32 {
    let tables = gamma_tables();
    let tab_pos = (v >> (GAMMA_TAB_FIX + 2)) as usize; // integer part
    let x = v & ((K_GAMMA_TAB_SCALE << 2) - 1); // fractional part
    let v0 = tables.linear_to_gamma[tab_pos];
    let v1 = tables.linear_to_gamma[tab_pos + 1];
    v1 * x + v0 * ((K_GAMMA_TAB_SCALE << 2) - x) // interpolate
}

/// Port of `LinearToGamma`.
#[inline]
fn linear_to_gamma(base_value: u32, shift: i32) -> i32 {
    let y = interpolate((base_value << shift) as i32); // final uplifted value
    (y + K_GAMMA_TAB_ROUNDER) >> GAMMA_TAB_FIX // descale
}

/// Port of `kInvAlpha[a]`: `(1 << kAlphaFix) / a`, with 0 for `a == 0`. The C table has 1021
/// entries and was checked against this formula for every entry.
#[inline]
fn inv_alpha(a: u32) -> u32 {
    (1u32 << K_ALPHA_FIX).checked_div(a).unwrap_or(0)
}

/// Port of `DIVIDE_BY_ALPHA` for the `USE_INVERSE_ALPHA_TABLE` branch (which `picture_csp_enc.c`
/// defines): `((sum) * kInvAlpha[a]) >> (kAlphaFix - 2)`, in `uint32_t` arithmetic.
#[inline]
fn divide_by_alpha(sum: u32, a: u32) -> u32 {
    sum.wrapping_mul(inv_alpha(a)) >> (K_ALPHA_FIX - 2)
}

/// Port of `LinearToGammaWeighted`: the alpha-weighted gamma average of four pixels.
#[inline]
fn linear_to_gamma_weighted(
    src: &[u8],
    src_off: usize,
    a: &[u8],
    a_off: usize,
    total_a: u32,
    step: usize,
    rgb_stride: usize,
) -> i32 {
    let sum = u32::from(a[a_off])
        .wrapping_mul(gamma_to_linear(src[src_off]))
        .wrapping_add(u32::from(a[a_off + step]).wrapping_mul(gamma_to_linear(src[src_off + step])))
        .wrapping_add(
            u32::from(a[a_off + rgb_stride])
                .wrapping_mul(gamma_to_linear(src[src_off + rgb_stride])),
        )
        .wrapping_add(
            u32::from(a[a_off + rgb_stride + step])
                .wrapping_mul(gamma_to_linear(src[src_off + rgb_stride + step])),
        );
    linear_to_gamma(divide_by_alpha(sum, total_a), 0)
}

/// Port of `SUM4(ptr, step)`: the gamma-domain sum of a 2x2 block of one channel.
#[inline]
fn sum4(src: &[u8], off: usize, step: usize, rgb_stride: usize) -> i32 {
    linear_to_gamma(
        gamma_to_linear(src[off])
            + gamma_to_linear(src[off + step])
            + gamma_to_linear(src[off + rgb_stride])
            + gamma_to_linear(src[off + rgb_stride + step]),
        0,
    )
}

/// Port of `SUM2(ptr)`: the gamma-domain sum of a 2x1 block of one channel.
#[inline]
fn sum2(src: &[u8], off: usize, rgb_stride: usize) -> i32 {
    linear_to_gamma(
        gamma_to_linear(src[off]) + gamma_to_linear(src[off + rgb_stride]),
        1,
    )
}

/// Port of `SUM2ALPHA(ptr)`.
#[inline]
fn sum2_alpha(a: &[u8], off: usize, rgb_stride: usize) -> u32 {
    u32::from(a[off]) + u32::from(a[off + rgb_stride])
}

/// Port of `SUM4ALPHA(ptr)`.
#[inline]
fn sum4_alpha(a: &[u8], off: usize, rgb_stride: usize) -> u32 {
    sum2_alpha(a, off, rgb_stride) + sum2_alpha(a, off + 4, rgb_stride)
}

/// Port of `VP8ClipUV` (`src/dsp/yuv.h`).
#[inline]
fn vp8_clip_uv(uv: i32, rounding: i32) -> i32 {
    let uv = (uv + rounding + (128 << (YUV_FIX + 2))) >> (YUV_FIX + 2);
    if (uv & !0xff) == 0 {
        uv
    } else if uv < 0 {
        0
    } else {
        255
    }
}

/// Port of `VP8RGBToY` (`src/dsp/yuv.h`).
#[inline]
#[must_use]
pub fn vp8_rgb_to_y(r: i32, g: i32, b: i32, rounding: i32) -> i32 {
    let luma = 16839 * r + 33059 * g + 6420 * b;
    (luma + rounding + (16 << YUV_FIX)) >> YUV_FIX // no need to clip
}

/// Port of `VP8RGBToU` (`src/dsp/yuv.h`).
#[inline]
#[must_use]
pub fn vp8_rgb_to_u(r: i32, g: i32, b: i32, rounding: i32) -> i32 {
    let u = -9719 * r - 19081 * g + 28800 * b;
    vp8_clip_uv(u, rounding)
}

/// Port of `VP8RGBToV` (`src/dsp/yuv.h`).
#[inline]
#[must_use]
pub fn vp8_rgb_to_v(r: i32, g: i32, b: i32, rounding: i32) -> i32 {
    let v = 28800 * r - 24116 * g - 4684 * b;
    vp8_clip_uv(v, rounding)
}

/// Port of `WebPConvertRGBA32ToUV_C` (`src/dsp/yuv.c`): chroma from the 16-bit 2x2 sums.
fn convert_rgba32_to_uv(rgb: &[u16], u: &mut [u8], v: &mut [u8], width: usize) {
    for i in 0..width {
        let r = i32::from(rgb[4 * i]);
        let g = i32::from(rgb[4 * i + 1]);
        let b = i32::from(rgb[4 * i + 2]);
        u[i] = vp8_rgb_to_u(r, g, b, YUV_HALF << 2) as u8;
        v[i] = vp8_rgb_to_v(r, g, b, YUV_HALF << 2) as u8;
    }
}

/// Port of `ExtractAlpha_C` (`src/dsp/alpha_processing.c`). Copies the alpha bytes (`argb[4 * i]`
/// for the `i`-th pixel of each row) and returns `true` when every copied alpha is 0xff.
fn extract_alpha(
    argb: &[u8],
    argb_off: usize,
    argb_stride: usize,
    width: usize,
    height: usize,
    alpha: &mut [u8],
    alpha_off: usize,
    alpha_stride: usize,
) -> bool {
    let mut alpha_mask = 0xffu8;
    let mut src = argb_off;
    let mut dst = alpha_off;
    for _ in 0..height {
        for i in 0..width {
            let alpha_value = argb[src + 4 * i];
            alpha[dst + i] = alpha_value;
            alpha_mask &= alpha_value;
        }
        src += argb_stride;
        dst += alpha_stride;
    }
    alpha_mask == 0xff
}

/// Port of `WebPHasAlpha32b` (`src/dsp/alpha_processing.c`) over the alpha bytes of `width`
/// pixels starting at `off` with 4-byte pixels: `true` if any alpha is not 0xff.
fn has_alpha_32b(src: &[u8], off: usize, length: usize) -> bool {
    (0..length).any(|x| src[off + 4 * x] != 0xff)
}

/// A YUV 4:2:0 picture (`WebPPicture` with `colorspace` YUV420 or YUV420A), the output of
/// [`import_rgba`]. `a` is present only for a picture that has transparency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YuvPicture {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Luma, `y_stride * height` bytes, with `y_stride = width`.
    pub y: Vec<u8>,
    /// Chroma U, `uv_stride * uv_height` bytes, with `uv_stride = (width + 1) / 2`.
    pub u: Vec<u8>,
    /// Chroma V, same layout as `u`.
    pub v: Vec<u8>,
    /// Alpha, `a_stride * height` bytes with `a_stride = width`, or `None` for an opaque picture.
    pub a: Option<Vec<u8>>,
}

impl YuvPicture {
    /// Port of `WebPPictureAllocYUVA` for `width x height` with or without the alpha plane.
    /// The strides are the C ones (`y_stride = width`, `uv_stride = uv_width`, `a_stride =
    /// width`).
    #[must_use]
    pub fn alloc(width: usize, height: usize, has_alpha: bool) -> Self {
        let uv_width = (width + 1) >> 1;
        let uv_height = (height + 1) >> 1;
        Self {
            width,
            height,
            y: vec![0; width * height],
            u: vec![0; uv_width * uv_height],
            v: vec![0; uv_width * uv_height],
            a: has_alpha.then(|| vec![0; width * height]),
        }
    }

    /// `y_stride` of the C picture.
    #[must_use]
    pub fn y_stride(&self) -> usize {
        self.width
    }

    /// `uv_stride` of the C picture.
    #[must_use]
    pub fn uv_stride(&self) -> usize {
        (self.width + 1) >> 1
    }
}

/// Port of `ImportYUVAFromRGBA` (non-iterative, undithered, `step = 4`) reached through
/// `Import(picture, rgb, rgb_stride, 4, swap_rb, import_alpha)` with `use_argb = 0`.
///
/// `rgb` holds the pixels; `rgb_stride` is the row stride in bytes (`>= 4 * width`, as
/// `Import` checks). With `import_alpha`, the alpha channel is read from byte 3 of each pixel and
/// the picture gets an alpha plane when some alpha is not 0xff. `swap_rb` reads R and B from
/// bytes 2 and 0 (the BGRA import).
///
/// Returns `None` where `Import` fails (`abs(rgb_stride) < 4 * width`, or too few bytes).
#[must_use]
pub fn import_rgba(
    rgb: &[u8],
    rgb_stride: usize,
    width: usize,
    height: usize,
    swap_rb: bool,
    import_alpha: bool,
) -> Option<YuvPicture> {
    if rgb_stride < (if import_alpha { 4 } else { 3 }) * width {
        return None;
    }
    if width == 0 || height == 0 {
        return None;
    }
    if rgb.len() < rgb_stride * (height - 1) + 4 * width {
        return None;
    }
    let r_ch = if swap_rb { 2 } else { 0 };
    let b_ch = if swap_rb { 0 } else { 2 };
    // CheckNonOpaque(a_ptr, width, height, step, rgb_stride), with a_ptr = NULL when !import_alpha.
    let has_alpha = import_alpha
        && (0..height).any(|row| has_alpha_32b(rgb, row * rgb_stride + 3, width));
    let mut pic = YuvPicture::alloc(width, height, has_alpha);
    let uv_width = (width + 1) >> 1;
    let y_stride = pic.y_stride();
    let uv_stride = pic.uv_stride();
    let mut tmp_rgb = vec![0u16; 4 * uv_width];

    // Row offsets of the channel pointers: `r_ptr`, `g_ptr`, `b_ptr`, `a_ptr` of the C code.
    let convert_row_to_y = |row_off: usize, dst: &mut [u8], dst_off: usize| {
        for i in 0..width {
            let j = row_off + 4 * i;
            dst[dst_off + i] = vp8_rgb_to_y(
                i32::from(rgb[j + r_ch]),
                i32::from(rgb[j + 1]),
                i32::from(rgb[j + b_ch]),
                YUV_HALF,
            ) as u8;
        }
    };

    let mut row = 0usize;
    for y in 0..(height >> 1) {
        let row0 = row;
        let row1 = row + rgb_stride;
        convert_row_to_y(row0, &mut pic.y, 2 * y * y_stride);
        convert_row_to_y(row1, &mut pic.y, (2 * y + 1) * y_stride);
        let mut rows_have_alpha = pic.a.is_some();
        if let Some(a) = pic.a.as_mut() {
            // rows_have_alpha &= !WebPExtractAlpha(a_ptr, rgb_stride, width, 2, dst_a, a_stride)
            let all_opaque = extract_alpha(
                rgb,
                row0 + 3,
                rgb_stride,
                width,
                2,
                a,
                2 * y * width,
                width,
            );
            rows_have_alpha &= !all_opaque;
        }
        // The alpha offsets are relative to the pixel of this row, as `a_ptr` in the C code.
        accumulate_pair(
            rgb,
            row0,
            rgb_stride,
            r_ch,
            b_ch,
            width,
            rows_have_alpha,
            &mut tmp_rgb,
        );
        let (u_plane, v_plane) = (&mut pic.u, &mut pic.v);
        convert_rgba32_to_uv(
            &tmp_rgb,
            &mut u_plane[y * uv_stride..],
            &mut v_plane[y * uv_stride..],
            uv_width,
        );
        row += 2 * rgb_stride;
    }
    if height & 1 == 1 {
        // extra last row
        let row0 = row;
        convert_row_to_y(row0, &mut pic.y, (height - 1) * y_stride);
        let mut row_has_alpha = pic.a.is_some();
        if let Some(a) = pic.a.as_mut() {
            if row_has_alpha {
                let all_opaque = extract_alpha(
                    rgb,
                    row0 + 3,
                    0,
                    width,
                    1,
                    a,
                    (height - 1) * width,
                    0,
                );
                row_has_alpha &= !all_opaque;
            }
        }
        accumulate_pair(rgb, row0, 0, r_ch, b_ch, width, row_has_alpha, &mut tmp_rgb);
        let y = height >> 1;
        let (u_plane, v_plane) = (&mut pic.u, &mut pic.v);
        convert_rgba32_to_uv(
            &tmp_rgb,
            &mut u_plane[y * uv_stride..],
            &mut v_plane[y * uv_stride..],
            uv_width,
        );
    }
    Some(pic)
}

/// `LinearToGammaWeighted` (`picture_csp_enc.c`): the alpha-weighted gamma average of the four
/// pixels (`step = 4`) or the two pixels (`step = 0`, the odd trailing column) of one channel.
#[inline]
fn linear_to_gamma_weighted_channel(
    rgb: &[u8],
    src_off: usize,
    a_off: usize,
    total_a: u32,
    step: usize,
    rgb_stride: usize,
) -> i32 {
    linear_to_gamma_weighted(rgb, src_off, rgb, a_off, total_a, step, rgb_stride)
}

/// Port of the `AccumulateRGB` / `AccumulateRGBA` pair of `ImportYUVAFromRGBA`: the 2x2 gamma
/// averages of one row pair (`rgb_stride` is the distance to the second row, `0` for the single
/// last row), as 16-bit values in `dst` (four per output chroma sample).
fn accumulate_pair(
    rgb: &[u8],
    row0: usize,
    rgb_stride: usize,
    r_ch: usize,
    b_ch: usize,
    width: usize,
    with_alpha: bool,
    dst: &mut [u16],
) {
    const G_CH: usize = 1;
    const A_CH: usize = 3;
    let mut j = 0usize;
    let mut k = 0usize;
    for _ in 0..(width >> 1) {
        let base = row0 + j;
        if with_alpha {
            let a = sum4_alpha(rgb, base + A_CH, rgb_stride);
            let (r, g, b) = if a == 4 * 0xff || a == 0 {
                (
                    sum4(rgb, base + r_ch, 4, rgb_stride),
                    sum4(rgb, base + G_CH, 4, rgb_stride),
                    sum4(rgb, base + b_ch, 4, rgb_stride),
                )
            } else {
                (
                    linear_to_gamma_weighted_channel(rgb, base + r_ch, base + A_CH, a, 4, rgb_stride),
                    linear_to_gamma_weighted_channel(rgb, base + G_CH, base + A_CH, a, 4, rgb_stride),
                    linear_to_gamma_weighted_channel(rgb, base + b_ch, base + A_CH, a, 4, rgb_stride),
                )
            };
            dst[k] = r as u16;
            dst[k + 1] = g as u16;
            dst[k + 2] = b as u16;
            dst[k + 3] = a as u16;
        } else {
            dst[k] = sum4(rgb, base + r_ch, 4, rgb_stride) as u16;
            dst[k + 1] = sum4(rgb, base + G_CH, 4, rgb_stride) as u16;
            dst[k + 2] = sum4(rgb, base + b_ch, 4, rgb_stride) as u16;
        }
        j += 2 * 4;
        k += 4;
    }
    if width & 1 == 1 {
        let base = row0 + j;
        if with_alpha {
            let a = 2u32 * sum2_alpha(rgb, base + A_CH, rgb_stride);
            let (r, g, b) = if a == 4 * 0xff || a == 0 {
                (
                    sum2(rgb, base + r_ch, rgb_stride),
                    sum2(rgb, base + G_CH, rgb_stride),
                    sum2(rgb, base + b_ch, rgb_stride),
                )
            } else {
                (
                    linear_to_gamma_weighted_channel(rgb, base + r_ch, base + A_CH, a, 0, rgb_stride),
                    linear_to_gamma_weighted_channel(rgb, base + G_CH, base + A_CH, a, 0, rgb_stride),
                    linear_to_gamma_weighted_channel(rgb, base + b_ch, base + A_CH, a, 0, rgb_stride),
                )
            };
            dst[k] = r as u16;
            dst[k + 1] = g as u16;
            dst[k + 2] = b as u16;
            dst[k + 3] = a as u16;
        } else {
            dst[k] = sum2(rgb, base + r_ch, rgb_stride) as u16;
            dst[k + 1] = sum2(rgb, base + G_CH, rgb_stride) as u16;
            dst[k + 2] = sum2(rgb, base + b_ch, rgb_stride) as u16;
        }
    }
}
