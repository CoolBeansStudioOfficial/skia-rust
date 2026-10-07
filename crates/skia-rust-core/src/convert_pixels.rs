// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkConvertPixels.cpp, src/core/SkRasterPipeline.cpp (appendStore,
// appendTransferFunction), src/opts/SkRasterPipeline_opts.h (the highp stages used here)

//! A restricted stand-in for `SkConvertPixels`, for exactly what `Pixmap::erase` needs.
//!
//! `SkPixmap::erase` converts one premultiplied `RGBA_F32` pixel to the pixmap's color type, alpha
//! type and color space with `SkConvertPixels`, which in turn runs `SkRasterPipeline`. Neither is
//! ported yet, so [`convert_rgba_f32_premul_pixel`] reproduces that one conversion: the
//! `rect_memcpy` / `convert_to_alpha8` fast paths, and otherwise the highp pipeline
//! `load_f32, <SkColorSpaceXformSteps stages>, <appendStore stages>` evaluated for a single pixel.
//!
//! The highp pipeline is compiled per CPU tier in Skia; this follows the portable shape:
//! `mad(f, m, a)` is `f * m + a` (the AVX2 tiers use an FMA there), and `round` rounds half to
//! even as `_mm_cvtps_epi32` does. The two only differ in the last ulp / on exact ties.
//!
//! Replace this with the real `SkConvertPixels` once `SkRasterPipeline` is ported.

use crate::alpha_type::AlphaType;
use crate::color::PMColor4f;
use crate::color_space_xform_steps::ColorSpaceXformSteps;
use crate::color_type::ColorType;
use crate::half::float_to_half;
use crate::image_info::ImageInfo;
use skia_rust_skcms::{TfType, TransferFunction, srgb_inverse_transfer_function};

// `_mm_max_ps(a, b)` is `a > b ? a : b` and `_mm_min_ps(a, b)` is `a < b ? a : b`: with a NaN
// operand they return the second operand.
fn sse_max(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

fn sse_min(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

// `SI F mad(F f, F m, F a)`
fn mad(f: f32, m: f32, a: f32) -> f32 {
    f * m + a
}

// `SI F nmad(F f, F m, F a)`
fn nmad(f: f32, m: f32, a: f32) -> f32 {
    a - f * m
}

// `SI U32 round(F v)`: round to nearest, ties to even (`_mm_cvtps_epi32`).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // v is clamped by the callers
fn round(v: f32) -> u32 {
    (v.round_ties_even() as i32).cast_unsigned()
}

// Port of: src/opts/SkRasterPipeline_opts.h#L2269-L2275 (chrome/m156)
fn to_unorm_full(v: f32, scale: f32, bias: f32, max_i: i32) -> u32 {
    // Any time we use round() we probably want to use to_unorm().
    #[allow(clippy::cast_precision_loss)] // maxI is at most 1023
    let max_i = max_i as f32;
    round(sse_min(sse_max(0.0f32, mad(v, scale, bias)), max_i))
}

#[allow(clippy::cast_precision_loss)] // scale is at most 65535
fn to_unorm(v: f32, scale: i32) -> u32 {
    to_unorm_full(v, scale as f32, 0.0f32, scale)
}

// Port of: src/opts/SkRasterPipeline_opts.h#L1590-L1650 (chrome/m156)
fn fract(v: f32) -> f32 {
    v - v.floor()
}

// See http://www.machinedlearnings.com/2011/06/fast-approximate-logarithm-exponential.html
// Port of: src/opts/SkRasterPipeline_opts.h#L1604-L1613 (chrome/m156)
#[allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant
)] // Skia's float literals kept verbatim
#[allow(clippy::cast_precision_loss)] // mirrors the C++ int -> float conversions
fn approx_log2(x: f32) -> f32 {
    // e - 127 is a fair approximation of log2(x) in its own right...
    // cast(U32) converts the bits as a signed integer.
    let e = (x.to_bits().cast_signed() as f32) * (1.0f32 / (1 << 23) as f32);

    // ... but using the mantissa to refine its error is _much_ better.
    let m = f32::from_bits((x.to_bits() & 0x007f_ffff) | 0x3f00_0000);

    nmad(m, 1.498030302f32, e - 124.225514990f32) - 1.725879990f32 / (0.3520887068f32 + m)
}

// Port of: src/opts/SkRasterPipeline_opts.h#L1615-L1618 (chrome/m156)
#[allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant
)] // Skia's float literals kept verbatim
fn approx_log(x: f32) -> f32 {
    let ln2 = 0.69314718f32;
    ln2 * approx_log2(x)
}

// Port of: src/opts/SkRasterPipeline_opts.h#L1620-L1631 (chrome/m156)
#[allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant
)] // Skia's float literals kept verbatim
#[allow(clippy::cast_precision_loss)] // mirrors the C++ int -> float conversions
fn approx_pow2(x: f32) -> f32 {
    // constexpr float kInfinityBits = 0x7f800000; (the numeric value, as a float)
    const INFINITY_BITS: f32 = 0x7f80_0000 as f32;

    let f = fract(x);
    let mut approx = nmad(f, 1.490129070f32, x + 121.274057500f32);
    approx += 27.728023300f32 / (4.84252568f32 - f);
    approx *= 1.0f32 * (1 << 23) as f32;
    approx = sse_min(sse_max(approx, 0.0f32), INFINITY_BITS); // guard against underflow/overflow

    f32::from_bits(round(approx))
}

// Port of: src/opts/SkRasterPipeline_opts.h#L1633-L1636 (chrome/m156)
#[allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant
)] // Skia's float literals kept verbatim
fn approx_exp(x: f32) -> f32 {
    let log2_e = 1.4426950408889634074f32;
    approx_pow2(log2_e * x)
}

// Port of: src/opts/SkRasterPipeline_opts.h#L1638-L1641 (chrome/m156)
#[allow(clippy::float_cmp)] // mirrors (x == 0) | (x == 1)
fn approx_powf(x: f32, y: f32) -> f32 {
    if x == 0.0 || x == 1.0 {
        x
    } else {
        approx_pow2(approx_log2(x) * y)
    }
}

// Port of: src/opts/SkRasterPipeline_opts.h#L2990-L2998 (chrome/m156)
fn strip_sign(x: f32) -> (f32, u32) {
    let bits = x.to_bits();
    let sign = bits & 0x8000_0000;
    (f32::from_bits(bits ^ sign), sign)
}

fn apply_sign(x: f32, sign: u32) -> f32 {
    f32::from_bits(sign | x.to_bits())
}

// Port of: src/opts/SkRasterPipeline_opts.h#L3000-L3012 (chrome/m156)
fn parametric(tf: &TransferFunction, v: f32) -> f32 {
    let (v, sign) = strip_sign(v);
    let r = if v <= tf.d {
        mad(tf.c, v, tf.f)
    } else {
        approx_powf(mad(tf.a, v, tf.b), tf.g) + tf.e
    };
    apply_sign(r, sign)
}

// Port of: src/opts/SkRasterPipeline_opts.h#L3014-L3023 (chrome/m156)
fn gamma_(g: f32, v: f32) -> f32 {
    let (v, sign) = strip_sign(v);
    apply_sign(approx_powf(v, g), sign)
}

// Port of: src/opts/SkRasterPipeline_opts.h#L3025-L3039 (chrome/m156)
fn pq_ish(tf: &TransferFunction, v: f32) -> f32 {
    let (v, sign) = strip_sign(v);
    let r = approx_powf(
        sse_max(mad(tf.b, approx_powf(v, tf.c), tf.a), 0.0f32)
            / (mad(tf.e, approx_powf(v, tf.c), tf.d)),
        tf.f,
    );
    apply_sign(r, sign)
}

// Port of: src/opts/SkRasterPipeline_opts.h#L3041-L3059 (chrome/m156)
#[allow(clippy::many_single_char_names)] // the names of the C++ locals
fn hlg_ish(tf: &TransferFunction, v: f32) -> f32 {
    let (v, sign) = strip_sign(v);
    let (big_r, big_g) = (tf.a, tf.b);
    let (a, b, c) = (tf.c, tf.d, tf.e);
    let k = tf.f + 1.0f32;

    let r = if v * big_r <= 1.0 {
        approx_powf(v * big_r, big_g)
    } else {
        approx_exp((v - c) * a) + b
    };
    k * apply_sign(r, sign)
}

// Port of: src/opts/SkRasterPipeline_opts.h#L3061-L3078 (chrome/m156)
#[allow(clippy::many_single_char_names)] // the names of the C++ locals
fn hlg_inv_ish(tf: &TransferFunction, v: f32) -> f32 {
    let (mut v, sign) = strip_sign(v);
    let (big_r, big_g) = (tf.a, tf.b);
    let (a, b, c) = (tf.c, tf.d, tf.e);
    let k = tf.f + 1.0f32;

    v /= k;
    let r = if v <= 1.0 {
        big_r * approx_powf(v, big_g)
    } else {
        a * approx_log(v - b) + c
    };
    apply_sign(r, sign)
}

// Port of: src/core/SkRasterPipeline.cpp#L549-L566 (chrome/m156)
#[allow(clippy::float_cmp)] // mirrors the exact comparisons of the C++
fn append_transfer_function(tf: &TransferFunction, rgba: &mut [f32; 4]) {
    let rgb = &mut rgba[..3];
    match tf.tf_type() {
        TfType::SRGBish => {
            if tf.a == 1.0
                && tf.b == 0.0
                && tf.c == 0.0
                && tf.d == 0.0
                && tf.e == 0.0
                && tf.f == 0.0
            {
                for v in rgb {
                    *v = gamma_(tf.g, *v);
                }
            } else {
                for v in rgb {
                    *v = parametric(tf, *v);
                }
            }
        }
        TfType::PQish => {
            for v in rgb {
                *v = pq_ish(tf, *v);
            }
        }
        TfType::HLGish => {
            for v in rgb {
                *v = hlg_ish(tf, *v);
            }
        }
        TfType::HLGinvish => {
            for v in rgb {
                *v = hlg_inv_ish(tf, *v);
            }
        }
        _ => debug_assert!(false),
    }
}

// Port of: src/core/SkColorSpaceXformSteps.cpp#L268-L277 (chrome/m156), with the highp stages
// `unpremul`, `ootf`, `matrix_3x3` and `premul`.
fn apply_steps_pipeline(steps: &ColorSpaceXformSteps, rgba: &mut [f32; 4]) {
    if steps.flags.unpremul {
        // Port of: src/opts/SkRasterPipeline_opts.h#L2710-L2716 (chrome/m156)
        let inf = f32::from_bits(0x7f80_0000);
        let scale = if 1.0f32 / rgba[3] < inf {
            1.0f32 / rgba[3]
        } else {
            0.0f32
        };
        rgba[0] *= scale;
        rgba[1] *= scale;
        rgba[2] *= scale;
    }
    if steps.flags.linearize {
        append_transfer_function(&steps.src_tf, rgba);
    }
    if steps.flags.src_ootf {
        ootf(&steps.src_ootf, rgba);
    }
    if steps.flags.gamut_transform {
        // Port of: src/opts/SkRasterPipeline_opts.h#L3686-L3695 (chrome/m156)
        let m = &steps.src_to_dst_matrix;
        let [r, g, b, _] = *rgba;
        rgba[0] = mad(r, m[0], mad(g, m[3], b * m[6]));
        rgba[1] = mad(r, m[1], mad(g, m[4], b * m[7]));
        rgba[2] = mad(r, m[2], mad(g, m[5], b * m[8]));
    }
    if steps.flags.dst_ootf {
        ootf(&steps.dst_ootf, rgba);
    }
    if steps.flags.encode {
        append_transfer_function(&steps.dst_tf_inv, rgba);
    }
    if steps.flags.premul {
        // Port of: src/opts/SkRasterPipeline_opts.h#L2700-L2704 (chrome/m156)
        rgba[0] *= rgba[3];
        rgba[1] *= rgba[3];
        rgba[2] *= rgba[3];
    }
}

// Port of: src/opts/SkRasterPipeline_opts.h#L3080-L3089 (chrome/m156)
fn ootf(ctx: &[f32; 4], rgba: &mut [f32; 4]) {
    let y = ctx[0] * rgba[0] + ctx[1] * rgba[1] + ctx[2] * rgba[2];

    let (y, sign) = strip_sign(y);
    let y_to_gamma_minus_one = apply_sign(approx_powf(y, ctx[3]), sign);
    rgba[0] *= y_to_gamma_minus_one;
    rgba[1] *= y_to_gamma_minus_one;
    rgba[2] *= y_to_gamma_minus_one;
}

fn put16(out: &mut [u8], index: usize, v: u32) {
    // pack(U32): the values stored here never exceed 16 bits.
    #[allow(clippy::cast_possible_truncation)] // mirrors the pack() to 16 bits
    let v = v as u16;
    out[2 * index..2 * index + 2].copy_from_slice(&v.to_ne_bytes());
}

fn put32(out: &mut [u8], v: u32) {
    out[..4].copy_from_slice(&v.to_ne_bytes());
}

// store_8888, shared by several color types.
fn store_8888(out: &mut [u8], [r, g, b, a]: [f32; 4]) {
    let px = to_unorm(r, 255)
        | (to_unorm(g, 255) << 8)
        | (to_unorm(b, 255) << 16)
        | (to_unorm(a, 255) << 24);
    put32(out, px);
}

// store_1010102, shared by several color types.
fn store_1010102(out: &mut [u8], [r, g, b, a]: [f32; 4]) {
    let px = to_unorm(r, 1023)
        | (to_unorm(g, 1023) << 10)
        | (to_unorm(b, 1023) << 20)
        | (to_unorm(a, 3) << 30);
    put32(out, px);
}

fn swap_rb([r, g, b, a]: [f32; 4]) -> [f32; 4] {
    [b, g, r, a]
}

fn force_opaque([r, g, b, _]: [f32; 4]) -> [f32; 4] {
    [r, g, b, 1.0f32]
}

// store_f16
fn store_f16(out: &mut [u8], rgba: [f32; 4]) {
    for (i, v) in rgba.into_iter().enumerate() {
        put16(out, i, u32::from(float_to_half(v)));
    }
}

// Port of: src/core/SkRasterPipeline.cpp#L482-L547 (chrome/m156) and the store stages at
// src/opts/SkRasterPipeline_opts.h#L3109-L3562 (chrome/m156). Returns false for
// `ColorType::Unknown`.
#[allow(clippy::too_many_lines)] // one arm per color type, as appendStore
fn append_store(ct: ColorType, rgba: [f32; 4], out: &mut [u8; 16]) -> bool {
    let [r, g, b, a] = rgba;
    match ct {
        ColorType::Unknown => return false,

        ColorType::Alpha8 => out[0] = to_unorm(a, 255).to_ne_bytes()[0],
        ColorType::R8UNorm => out[0] = to_unorm(r, 255).to_ne_bytes()[0],
        ColorType::A16UNorm => put16(out, 0, to_unorm(a, 65535)),
        ColorType::A16Float => put16(out, 0, u32::from(float_to_half(a))),
        ColorType::RGB565 => {
            let px = (to_unorm(r, 31) << 11) | (to_unorm(g, 63) << 5) | to_unorm(b, 31);
            put16(out, 0, px);
        }
        ColorType::ARGB4444 => {
            let px = (to_unorm(r, 15) << 12)
                | (to_unorm(g, 15) << 8)
                | (to_unorm(b, 15) << 4)
                | to_unorm(a, 15);
            put16(out, 0, px);
        }
        ColorType::R8G8UNorm => {
            let px = to_unorm(r, 255) | (to_unorm(g, 255) << 8);
            put16(out, 0, px);
        }
        ColorType::R16UNorm => put16(out, 0, to_unorm(r, 65535)),
        ColorType::R16Float => put16(out, 0, u32::from(float_to_half(r))),
        ColorType::R16G16UNorm => {
            let px = to_unorm(r, 65535) | (to_unorm(g, 65535) << 16);
            put32(out, px);
        }
        ColorType::R16G16Float => {
            put16(out, 0, u32::from(float_to_half(r)));
            put16(out, 1, u32::from(float_to_half(g)));
        }
        ColorType::RGBA8888 => store_8888(out, rgba),
        ColorType::RGBA1010102 => store_1010102(out, rgba),
        ColorType::R16G16B16A16UNorm => {
            for (i, v) in rgba.into_iter().enumerate() {
                put16(out, i, to_unorm(v, 65535));
            }
        }
        ColorType::RGBAF16Norm | ColorType::RGBAF16 => store_f16(out, rgba),
        ColorType::RGBAF32 => {
            for (i, v) in rgba.into_iter().enumerate() {
                out[4 * i..4 * i + 4].copy_from_slice(&v.to_ne_bytes());
            }
        }
        ColorType::RGBA10x6 => {
            for (i, v) in rgba.into_iter().enumerate() {
                put16(out, i, to_unorm(v, 1023) << 6);
            }
        }

        ColorType::RGB888x => store_8888(out, force_opaque(rgba)),
        ColorType::BGRA1010102 => store_1010102(out, swap_rb(rgba)),
        ColorType::RGB101010x => store_1010102(out, force_opaque(rgba)),
        ColorType::BGR101010x => store_1010102(out, swap_rb(force_opaque(rgba))),
        ColorType::BGR101010xXR => {
            let [r, g, b, a] = swap_rb(force_opaque(rgba));
            // This is the inverse of from_1010102_xr, e.g. (v * 510 + 384)
            let px = to_unorm_full(r, 510.0, 384.0, 1023)
                | (to_unorm_full(g, 510.0, 384.0, 1023) << 10)
                | (to_unorm_full(b, 510.0, 384.0, 1023) << 20)
                | (to_unorm(a, 3) << 30);
            put32(out, px);
        }
        ColorType::RGBF16F16F16x => store_f16(out, force_opaque(rgba)),
        ColorType::BGRA10101010XR => {
            // This is the inverse of from_10101010_xr, e.g. (v * 510 + 384)
            for (i, v) in swap_rb(rgba).into_iter().enumerate() {
                put16(out, i, to_unorm_full(v, 510.0, 384.0, 1023) << 6);
            }
        }
        ColorType::Gray8 => {
            // bt709_luminance_or_luma_to_alpha, then store_a8
            let a = r * 0.2126f32 + g * 0.7152f32 + b * 0.0722f32;
            out[0] = to_unorm(a, 255).to_ne_bytes()[0];
        }
        ColorType::BGRA8888 => store_8888(out, swap_rb(rgba)),
        ColorType::SRGBA8888 => {
            let mut rgba = rgba;
            append_transfer_function(srgb_inverse_transfer_function(), &mut rgba);
            store_8888(out, rgba);
        }
    }
    true
}

/// Converts one premultiplied `RGBA_F32` pixel (with no color space) to the color type, alpha type
/// and color space of the 1x1 image info `dst`, as
/// `SkConvertPixels(dst, dstPixel, 16, 1x1 RGBA_F32 premul, &c, 16)` does. The converted pixel is
/// returned in the first `dst.bytes_per_pixel()` bytes of the result, in native byte order.
///
/// Returns `None` where `SkConvertPixels` returns false (an unknown color type).
// Port of: src/core/SkConvertPixels.cpp#L26-L45 and #L205-L291 (chrome/m156)
#[doc(alias = "SkConvertPixels")]
#[must_use]
pub fn convert_rgba_f32_premul_pixel(dst: &ImageInfo, c: &PMColor4f) -> Option<[u8; 16]> {
    let dst_ct = dst.color_type();
    if dst_ct == ColorType::Unknown {
        return None;
    }

    let steps = ColorSpaceXformSteps::new(
        None,
        AlphaType::Premul,
        dst.color_space().as_ref(),
        dst.alpha_type(),
    );

    let mut out = [0u8; 16];

    // rect_memcpy: no color type, alpha type, or color space changes.
    if dst_ct == ColorType::RGBAF32 && steps.flags.mask() == 0 {
        for (i, v) in [c.r, c.g, c.b, c.a].into_iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&v.to_ne_bytes());
        }
        return Some(out);
    }

    // swizzle_or_premul needs an 8888 source.

    // convert_to_alpha8
    if dst_ct == ColorType::Alpha8 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // mirrors (uint8_t)(255.0f * rgba[3])
        {
            out[0] = (255.0f32 * c.a) as u8;
        }
        return Some(out);
    }

    // convert_with_pipeline: load_f32, the xform steps, then the store stages.
    let mut rgba = [c.r, c.g, c.b, c.a];
    apply_steps_pipeline(&steps, &mut rgba);
    if append_store(dst_ct, rgba, &mut out) {
        Some(out)
    } else {
        None
    }
}
