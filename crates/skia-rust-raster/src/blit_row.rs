// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBlitRow.h, src/core/SkBlitRow_D32.cpp

//! `SkBlitRow`: blends rows of 32-bit premultiplied pixels onto a 32-bit destination.
//!
//! `blit_row_s32a_opaque`, `blit_row_color32` and `memset32` are the `SkOpts` kernels of
//! `skia-rust-simd` and follow the current CPU tier. The other two procs are compiled into
//! `SkBlitRow_D32.cpp` and picked at *compile time* in Skia (SSE2 on x86-64, NEON on arm64,
//! portable elsewhere):
//!
//! | Proc | x86 (SSE2, 4 pixels at a time, scalar tail) | NEON | Portable |
//! |---|---|---|---|
//! | `blit_row_s32_blend` | `SkPMLerp_SSE2` | 16-bit lanes of the same lerp | `SkPMLerp` |
//! | `blit_row_s32a_blend` | `SkBlendARGB32_SSE2`, tail `SkBlendARGB32` | the same lane math, no tail | `SkBlendARGB32` |
//!
//! The lerp is exact in every form for any input bytes: per channel, adding `((src - dst) *
//! scale) >> 8` to `dst` (16-bit lanes, then a byte add) gives what `SkPMLerp` computes,
//! `(src * scale + (256 - scale) * dst) >> 8`.
//!
//! `SkBlendARGB32`'s two forms agree for every *premultiplied* source (a source channel times
//! `src_scale` plus a destination channel times `dst_scale` then fits 16 bits); for a source
//! channel above its alpha the SIMD forms wrap per 16-bit lane while the portable form carries
//! into the neighbouring channel. `blit_row_s32a_blend` reproduces the SIMD behaviour (what the
//! oracle and every arm64 build run), including the scalar tail. The tests check the lerp and the
//! blend over all premultiplied inputs against an independent model of the lane math.

use skia_rust_core::color::PMColor;
use skia_rust_core::color_data::{blend_argb32, pm_lerp};
use skia_rust_core::color_priv::{alpha_255_to_256, get_packed_a32};
use skia_rust_simd::blit_row::{blit_row_color32, blit_row_s32a_opaque};
use skia_rust_simd::memset::memset32;
use skia_rust_simd::{Tier, selection};

/// `SkBlitRow::kGlobalAlpha_Flag32`: the global `alpha` argument applies.
// Port of: src/core/SkBlitRow.h#L16-L19 (chrome/m156)
pub const GLOBAL_ALPHA_FLAG32: u32 = 1 << 0;
/// `SkBlitRow::kSrcPixelAlpha_Flag32`: the source pixels have alpha.
// Port of: src/core/SkBlitRow.h#L16-L19 (chrome/m156)
pub const SRC_PIXEL_ALPHA_FLAG32: u32 = 1 << 1;

/// `SkBlitRow::Proc32`: blends `src` onto `dst` (`dst.len()` pixels; `src` may be longer) with
/// the global `alpha` (`0..=255`; ignored by the procs that have no global alpha).
// Port of: src/core/SkBlitRow.h#L21-L28 (chrome/m156)
pub type Proc32 = fn(dst: &mut [u32], src: &[u32], alpha: u32);

// Everyone agrees memcpy() is the best way to do this.
// Port of: src/core/SkBlitRow_D32.cpp#L24-L30 (chrome/m156)
fn blit_row_s32_opaque(dst: &mut [u32], src: &[u32], alpha: u32) {
    debug_assert_eq!(255, alpha);
    let n = dst.len();
    dst.copy_from_slice(&src[..n]);
}

// Port of: src/core/SkBlitRow_D32.cpp#L66-L88 (SSE2), #L286-L295 (portable) (chrome/m156)
fn blit_row_s32_blend(dst: &mut [u32], src: &[u32], alpha: u32) {
    debug_assert!(alpha <= 255);
    let scale = alpha_255_to_256(alpha);
    for (d, s) in dst.iter_mut().zip(src) {
        *d = pm_lerp(*s, *d, scale);
    }
}

// `SkBlendARGB32_SSE2` for one pixel, or one pixel of the NEON loop of `blit_row_s32a_blend`:
// the same math in 16-bit lanes (a product that does not fit 16 bits wraps).
// Port of: src/core/SkBlitRow_D32.cpp#L90-L124 (SSE2), #L153-L240 (NEON) (chrome/m156)
pub(crate) fn blend_argb32_lanes(src: PMColor, dst: PMColor, aa: u32) -> PMColor {
    let src_scale = alpha_255_to_256(aa);
    // SkAlphaMulInv256(SkGetPackedA32(src), src_scale), in 32-bit lanes whose high words are 0,
    // so the 16-bit multiply is the whole product.
    let mut dst_scale = (src >> 24).wrapping_mul(src_scale) & 0xFFFF;
    dst_scale = 0xFFFF - dst_scale;
    dst_scale += dst_scale >> 8;
    dst_scale >>= 8;

    // Four 16-bit lanes (one per channel), each scaled, added and shifted down by 8.
    let mut out = 0u32;
    for shift in [0, 8, 16, 24] {
        let s = (src >> shift) & 0xFF;
        let d = (dst >> shift) & 0xFF;
        let lane = ((s * src_scale) & 0xFFFF).wrapping_add((d * dst_scale) & 0xFFFF) & 0xFFFF;
        out |= (lane >> 8) << shift;
    }
    out
}

// Port of: src/core/SkBlitRow_D32.cpp#L126-L148 (SSE2), #L297-L306 (portable) (chrome/m156)
fn blit_row_s32a_blend(dst: &mut [u32], src: &[u32], alpha: u32) {
    debug_assert!(alpha <= 255);
    let n = dst.len();
    let src = &src[..n];
    if selection().tier == Tier::Neon {
        // The NEON code does two pixels at a time with the same lane math, and the odd pixel
        // first; no scalar tail.
        for (d, s) in dst.iter_mut().zip(src) {
            *d = blend_argb32_lanes(*s, *d, alpha);
        }
        return;
    }
    // SSE2 (the x86 baseline): four pixels at a time, then SkBlendARGB32 for the rest.
    let simd = n & !3;
    for (d, s) in dst[..simd].iter_mut().zip(&src[..simd]) {
        *d = blend_argb32_lanes(*s, *d, alpha);
    }
    for (d, s) in dst[simd..].iter_mut().zip(&src[simd..]) {
        *d = blend_argb32(*s, *d, alpha);
    }
}

// `SkOpts::blit_row_s32a_opaque` (the CPU tier's kernel).
fn blit_row_s32a_opaque_proc(dst: &mut [u32], src: &[u32], alpha: u32) {
    blit_row_s32a_opaque(dst, src, alpha);
}

/// `SkBlitRow::Factory32`: the proc that blends source pixels onto 32-bit destinations given
/// `flags32` (a combination of [`GLOBAL_ALPHA_FLAG32`] and [`SRC_PIXEL_ALPHA_FLAG32`]).
///
/// # Panics
/// In debug builds if `flags32 > 3` (`SkASSERT`).
// Port of: src/core/SkBlitRow_D32.cpp#L539-L552 (chrome/m156)
#[doc(alias = "Factory32")]
#[must_use]
pub fn factory32(flags32: u32) -> Proc32 {
    const PROCS: [Proc32; 4] = [
        blit_row_s32_opaque,
        blit_row_s32_blend,
        blit_row_s32a_opaque_proc, // `nullptr` in the table: SkOpts::blit_row_s32a_opaque
        blit_row_s32a_blend,
    ];

    debug_assert!(flags32 < 4);
    let flags32 = (flags32 & 3) as usize; // just to be safe
    PROCS[flags32]
}

/// `SkBlitRow::Color32`: blends the single premultiplied `color` onto `dst` (source-over).
// Port of: src/core/SkBlitRow_D32.cpp#L554-L561 (chrome/m156)
#[doc(alias = "Color32")]
pub fn color32(dst: &mut [u32], color: PMColor) {
    match get_packed_a32(color) {
        0 => {} // Nothing to do
        255 => memset32(dst, color, dst.len()),
        _ => blit_row_color32(dst, color),
    }
}
