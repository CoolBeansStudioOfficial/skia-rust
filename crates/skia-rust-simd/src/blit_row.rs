// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkBlitRow_opts.h

//! `SkOpts::blit_row_s32a_opaque` and `SkOpts::blit_row_color32` (design §1.8, task B7).
//!
//! # `blit_row_s32a_opaque`
//! Source-over of premultiplied pixels, per tier like the `SkOpts` kernels it replaces:
//!
//! | Tier | Skia | Formula |
//! |---|---|---|
//! | `Sse2`, `Sse41` | `SkPMSrcOver_SSE2`, 4 pixels at a time, scalar tail | `s + ((d * (256 - sa)) >> 8)`, saturating byte add |
//! | `Ml3`, `Ml4` | `SkPMSrcOver_AVX2` 8 wide, then SSE2 4 wide, scalar tail | the same |
//! | `Neon` | `SkPMSrcOver_neon8`/`_neon2` | `s + SkMulDiv255Round(255 - sa, d)`, saturating byte add (**different results**) |
//! | `Scalar` (wasm) | `SkPMSrcOver` | the x86 formula |
//!
//! The scalar twin ([`blit_row_s32a_opaque_scalar`]) is `SkPMSrcOver` per pixel. The x86 model
//! ([`Backend::Model`]) is the x86 formula written per byte; both equal the x86 SIMD code for
//! every input (a test checks them on random non-premultiplied data), so the x86 tiers differ
//! from `Neon` only through the formula above.
//!
//! # `blit_row_color32`
//! Written with `skvx` in Skia and exact on every tier, so there is one implementation
//! ([`blit_row_color32`], ported with [`Vec`]) and a per-byte scalar twin.

use crate::color_util::{A32_SHIFT, alpha_255_to_256, mul_div_255_round, pm_src_over};
use crate::tier::{Backend, Selection, Tier, selection};
use crate::vx::{self, Vec as VxVec};

/// `SkOpts::blit_row_s32a_opaque(dst, src, len, alpha)` for `len == dst.len()`, on the current
/// [`selection`].
///
/// # Panics
/// If `src` is shorter than `dst`, or `alpha != 0xFF` in debug builds (`SkASSERT`).
// Port of: src/opts/SkBlitRow_opts.h#L128-L220 (chrome/m156)
pub fn blit_row_s32a_opaque(dst: &mut [u32], src: &[u32], alpha: u32) {
    blit_row_s32a_opaque_with(selection(), dst, src, alpha);
}

/// [`blit_row_s32a_opaque`] on an explicit [`Selection`].
///
/// # Panics
/// As [`blit_row_s32a_opaque`]; also if `sel` is a native tier this host cannot run.
pub fn blit_row_s32a_opaque_with(sel: Selection, dst: &mut [u32], src: &[u32], alpha: u32) {
    debug_assert_eq!(alpha, 0xFF);
    assert!(src.len() >= dst.len(), "src is shorter than dst");
    let src = &src[..dst.len()];
    match (sel.tier, sel.backend) {
        (Tier::Scalar, _) => blit_row_s32a_opaque_scalar(dst, src, alpha),
        (Tier::Sse2 | Tier::Sse41 | Tier::Ml3 | Tier::Ml4, Backend::Model(_)) => {
            for (d, s) in dst.iter_mut().zip(src) {
                *d = src_over_x86(*s, *d);
            }
        }
        (Tier::Neon, Backend::Model(_)) => {
            for (d, s) in dst.iter_mut().zip(src) {
                *d = src_over_neon(*s, *d);
            }
        }
        #[cfg(target_arch = "x86_64")]
        (Tier::Sse2 | Tier::Sse41, Backend::Native) => {
            let token = crate::cpu::Sse2Token::get()
                .unwrap_or_else(|| panic!("{sel}: the host cannot run this tier"));
            x86::sse2(token, dst, src);
        }
        #[cfg(target_arch = "x86_64")]
        (Tier::Ml3, Backend::Native) => {
            let token = crate::cpu::Ml3Token::get()
                .unwrap_or_else(|| panic!("{sel}: the host cannot run this tier"));
            x86::avx2_ml3(token, dst, src);
        }
        #[cfg(target_arch = "x86_64")]
        (Tier::Ml4, Backend::Native) => {
            let token = crate::cpu::Ml4Token::get()
                .unwrap_or_else(|| panic!("{sel}: the host cannot run this tier"));
            x86::avx2_ml4(token, dst, src);
        }
        #[cfg(target_arch = "aarch64")]
        (Tier::Neon, Backend::Native) => {
            let token = crate::cpu::NeonToken::get()
                .unwrap_or_else(|| panic!("{sel}: the host cannot run this tier"));
            neon::blit(token, dst, src);
        }
        (_, Backend::Native) => panic!("{sel}: the host cannot run this tier"),
    }
}

/// The scalar twin of [`blit_row_s32a_opaque`]: `SkPMSrcOver` on every pixel (the kernel's
/// tail loop, and all of it on `Scalar`).
///
/// # Panics
/// If `src` is shorter than `dst`.
// Port of: src/opts/SkBlitRow_opts.h#L211-L217 (chrome/m156)
pub fn blit_row_s32a_opaque_scalar(dst: &mut [u32], src: &[u32], alpha: u32) {
    debug_assert_eq!(alpha, 0xFF);
    assert!(src.len() >= dst.len(), "src is shorter than dst");
    for (d, s) in dst.iter_mut().zip(src) {
        *d = pm_src_over(*s, *d);
    }
}

/// The x86 SIMD formula for one pixel, per byte: `s + ((d * (256 - sa)) >> 8)` with a saturating
/// add (`SkPMSrcOver_SSE2`/`_AVX2`).
// Port of: src/opts/SkBlitRow_opts.h#L24-L66 (chrome/m156)
fn src_over_x86(src: u32, dst: u32) -> u32 {
    let scale = 256 - (src >> A32_SHIFT);
    let mut out = [0u8; 4];
    for (i, o) in out.iter_mut().enumerate() {
        let s = (src >> (8 * i)) & 0xFF;
        let d = (dst >> (8 * i)) & 0xFF;
        // `d * scale` fits 16 bits (255 * 256), as in `_mm_mullo_epi16`.
        let scaled = (d * scale) >> 8;
        // The saturating byte add (`_mm_adds_epu8`); `s + scaled` is at most 510.
        *o = u8::try_from((s + scaled).min(0xFF)).unwrap_or(0xFF);
    }
    u32::from_le_bytes(out)
}

/// The `Neon` formula for one pixel, per byte: `s + SkMulDiv255Round(255 - sa, d)` with a
/// saturating add (`SkPMSrcOver_neon8`/`_neon2`).
// Port of: src/opts/SkBlitRow_opts.h#L69-L86 (chrome/m156)
fn src_over_neon(src: u32, dst: u32) -> u32 {
    let nalpha = 255 - (src >> A32_SHIFT); // vmvn_u8(alpha)
    let mut out = [0u8; 4];
    for (i, o) in out.iter_mut().enumerate() {
        let s = (src >> (8 * i)) & 0xFF;
        let d = (dst >> (8 * i)) & 0xFF;
        *o = u8::try_from((s + mul_div_255_round(nalpha, d)).min(0xFF)).unwrap_or(0xFF);
    }
    u32::from_le_bytes(out)
}

/// `SkOpts::blit_row_color32(dst, count, color)` for `count == dst.len()`: blends the
/// premultiplied constant `color` over every pixel. `color`'s alpha must be in `1..=254`
/// (callers handle 0 and 255 specially, as `SkBlitRow::Color32` does).
///
/// All tiers run the same `skvx` code in Skia, so this has no tier dispatch.
// Port of: src/opts/SkBlitRow_opts.h#L222-L256 (chrome/m156)
pub fn blit_row_color32(dst: &mut [u32], color: u32) {
    const N: usize = 4; // 8, 16 also reasonable choices
    type U32 = VxVec<N, u32>;
    type U16 = VxVec<{ 4 * N }, u16>;
    type U8 = VxVec<{ 4 * N }, u8>;

    // Note when the kernel is used below, the "src" is the existing pixel color.
    let kernel = |src: U32| -> U32 {
        let inv_a = alpha_255_to_256(255 - (color >> A32_SHIFT));
        debug_assert!(0 < inv_a && inv_a < 256); // We handle alpha == 0 or alpha == 255 specially.

        // color is premul, so the channels have already been
        // scaled by alpha. We just need to scale src by (255 - a)
        // using the trick of adding 1 and dividing by 256 which is
        // much faster than dividing by 255. Then we can add that
        // to color to get the result.
        let s: U8 = src.bit_cast();
        #[allow(clippy::cast_possible_truncation)] // U8(invA): in range per the assert above
        let a = U8::splat(inv_a as u8);
        let c: U16 = U32::splat(color).bit_cast::<{ 4 * N }, u8>().cast();
        let r = (vx::mull(s, a) >> 8) + c;
        r.cast::<u8>().bit_cast()
    };

    let (chunks, tail) = dst.as_chunks_mut::<N>();
    for chunk in chunks {
        kernel(U32::load(chunk)).store(chunk);
    }
    for px in tail {
        *px = kernel(U32::splat(*px)).0[0];
    }
}

/// The scalar twin of [`blit_row_color32`]: the same arithmetic one byte at a time.
// Port of: src/opts/SkBlitRow_opts.h#L222-L256 (chrome/m156)
pub fn blit_row_color32_scalar(dst: &mut [u32], color: u32) {
    let inv_a = alpha_255_to_256(255 - (color >> A32_SHIFT));
    debug_assert!(0 < inv_a && inv_a < 256);
    for px in dst {
        let mut out = [0u8; 4];
        for (i, o) in out.iter_mut().enumerate() {
            let s = (*px >> (8 * i)) & 0xFF;
            let c = (color >> (8 * i)) & 0xFF;
            // `cast<uint8_t>(r)` wraps; inv_a == 256 wraps to 0 like `U8(invA)`.
            let a = inv_a & 0xFF;
            *o = u8::try_from(((s * a) >> 8).wrapping_add(c) & 0xFF).unwrap_or(0);
        }
        *px = u32::from_le_bytes(out);
    }
}

#[cfg(target_arch = "x86_64")]
mod x86 {
    //! `SkPMSrcOver_SSE2` and `SkPMSrcOver_AVX2` with their loops (`SK_CPU_X64_LEVEL`).

    use core::arch::x86_64::{
        __m128i, __m256i, _mm_adds_epu8, _mm_and_si128, _mm_andnot_si128, _mm_loadu_si128,
        _mm_mullo_epi16, _mm_or_si128, _mm_set1_epi32, _mm_slli_epi32, _mm_srli_epi16,
        _mm_srli_epi32, _mm_storeu_si128, _mm_sub_epi32, _mm256_adds_epu8, _mm256_and_si256,
        _mm256_andnot_si256, _mm256_loadu_si256, _mm256_mullo_epi16, _mm256_or_si256,
        _mm256_set1_epi16, _mm256_set1_epi32, _mm256_setr_epi8, _mm256_shuffle_epi8,
        _mm256_srli_epi16, _mm256_storeu_si256, _mm256_sub_epi16,
    };

    use crate::color_util::pm_src_over;
    use crate::cpu::{Ml3Token, Ml4Token, Sse2Token};

    /// Four pixels from an array.
    #[target_feature(enable = "sse2")]
    #[inline]
    fn load4(v: &[u32; 4]) -> __m128i {
        // SAFETY: `v` is a live `[u32; 4]`, valid for 16 bytes of reads; `_mm_loadu_si128` has
        // no alignment requirement (the pointer cast only changes the pointee type).
        unsafe { _mm_loadu_si128(v.as_ptr().cast()) }
    }

    /// Four pixels into an array.
    #[target_feature(enable = "sse2")]
    #[inline]
    fn store4(v: &mut [u32; 4], r: __m128i) {
        // SAFETY: `v` is a live `[u32; 4]`, valid for 16 bytes of writes; `_mm_storeu_si128` has
        // no alignment requirement (the pointer cast only changes the pointee type).
        unsafe { _mm_storeu_si128(v.as_mut_ptr().cast(), r) };
    }

    /// Eight pixels from an array.
    #[target_feature(enable = "avx2")]
    #[inline]
    fn load8(v: &[u32; 8]) -> __m256i {
        // SAFETY: `v` is a live `[u32; 8]`, valid for 32 bytes of reads; `_mm256_loadu_si256`
        // has no alignment requirement (the pointer cast only changes the pointee type).
        unsafe { _mm256_loadu_si256(v.as_ptr().cast()) }
    }

    /// Eight pixels into an array.
    #[target_feature(enable = "avx2")]
    #[inline]
    fn store8(v: &mut [u32; 8], r: __m256i) {
        // SAFETY: `v` is a live `[u32; 8]`, valid for 32 bytes of writes; `_mm256_storeu_si256`
        // has no alignment requirement (the pointer cast only changes the pointee type).
        unsafe { _mm256_storeu_si256(v.as_mut_ptr().cast(), r) };
    }

    // Port of: src/opts/SkBlitRow_opts.h#L58-L74 (chrome/m156)
    #[target_feature(enable = "sse2")]
    #[inline]
    fn pm_src_over_sse2(src: __m128i, dst: __m128i) -> __m128i {
        let scale = _mm_sub_epi32(_mm_set1_epi32(256), _mm_srli_epi32::<24>(src));
        let scale_x2 = _mm_or_si128(_mm_slli_epi32::<16>(scale), scale);

        let mut rb = _mm_and_si128(_mm_set1_epi32(0x00ff_00ff), dst);
        rb = _mm_mullo_epi16(rb, scale_x2);
        rb = _mm_srli_epi16::<8>(rb);

        let mut ga = _mm_srli_epi16::<8>(dst);
        ga = _mm_mullo_epi16(ga, scale_x2);
        ga = _mm_andnot_si128(_mm_set1_epi32(0x00ff_00ff), ga);

        _mm_adds_epu8(src, _mm_or_si128(rb, ga))
    }

    // Port of: src/opts/SkBlitRow_opts.h#L19-L56 (chrome/m156)
    #[target_feature(enable = "avx2")]
    #[inline]
    fn pm_src_over_avx2(src: __m256i, dst: __m256i) -> __m256i {
        // Shuffle each pixel's srcA to the low byte of each 16-bit half of the pixel.
        let src_a_x2 = _mm256_shuffle_epi8(
            src,
            _mm256_setr_epi8(
                3, -1, 3, -1, 7, -1, 7, -1, 11, -1, 11, -1, 15, -1, 15, -1, 3, -1, 3, -1, 7, -1, 7,
                -1, 11, -1, 11, -1, 15, -1, 15, -1,
            ),
        );
        let scale_x2 = _mm256_sub_epi16(_mm256_set1_epi16(256), src_a_x2);

        // Scale red and blue, leaving results in the low byte of each 16-bit lane.
        let mut rb = _mm256_and_si256(_mm256_set1_epi32(0x00ff_00ff), dst);
        rb = _mm256_mullo_epi16(rb, scale_x2);
        rb = _mm256_srli_epi16::<8>(rb);

        // Scale green and alpha, leaving results in the high byte, masking off the low bits.
        let mut ga = _mm256_srli_epi16::<8>(dst);
        ga = _mm256_mullo_epi16(ga, scale_x2);
        ga = _mm256_andnot_si256(_mm256_set1_epi32(0x00ff_00ff), ga);

        _mm256_adds_epu8(src, _mm256_or_si256(rb, ga))
    }

    /// The `SK_CPU_X64_LEVEL_SSE2` loop and the scalar tail.
    #[target_feature(enable = "sse2")]
    fn blit_sse2(dst: &mut [u32], src: &[u32]) {
        let (d4, d_tail) = dst.as_chunks_mut::<4>();
        let (s4, s_tail) = src.as_chunks::<4>();
        for (d, s) in d4.iter_mut().zip(s4) {
            store4(d, pm_src_over_sse2(load4(s), load4(d)));
        }
        for (d, s) in d_tail.iter_mut().zip(s_tail) {
            *d = pm_src_over(*s, *d);
        }
    }

    /// The `SK_CPU_X64_LEVEL_AVX2` loop, then the SSE2 loop and scalar tail.
    #[target_feature(enable = "avx2")]
    fn blit_avx2(dst: &mut [u32], src: &[u32]) {
        let (d8, d_tail) = dst.as_chunks_mut::<8>();
        let (s8, s_tail) = src.as_chunks::<8>();
        for (d, s) in d8.iter_mut().zip(s8) {
            store8(d, pm_src_over_avx2(load8(s), load8(d)));
        }
        blit_sse2(d_tail, s_tail);
    }

    /// `Sse2`/`Sse41` tiers.
    pub(super) fn sse2(_token: Sse2Token, dst: &mut [u32], src: &[u32]) {
        // SAFETY: `blit_sse2` enables `sse2`, which `_token` proves the host supports (SSE2 is
        // also the x86-64 baseline).
        unsafe { blit_sse2(dst, src) };
    }

    /// `Ml3` tier.
    pub(super) fn avx2_ml3(_token: Ml3Token, dst: &mut [u32], src: &[u32]) {
        // SAFETY: `blit_avx2` enables `avx2` (and its implied features), all of which
        // `Ml3Token::FEATURES` lists, and `_token` proves them detected.
        unsafe { blit_avx2(dst, src) };
    }

    /// `Ml4` tier (Skia compiles the same AVX2 code at the ML4 level).
    pub(super) fn avx2_ml4(_token: Ml4Token, dst: &mut [u32], src: &[u32]) {
        // SAFETY: as `avx2_ml3`; `Ml4Token::FEATURES` is a superset of the `Ml3` features.
        unsafe { blit_avx2(dst, src) };
    }
}

#[cfg(target_arch = "aarch64")]
mod neon {
    //! `SkPMSrcOver_neon8` / `SkPMSrcOver_neon2` with their loops.

    use core::arch::aarch64::{
        uint8x8_t, uint8x8x4_t, vcreate_u8, vget_lane_u64, vld1_u8, vld4_u8, vmull_u8, vmvn_u8,
        vqadd_u8, vraddhn_u16, vreinterpret_u64_u8, vrshrq_n_u16, vst1_u8, vst4_u8, vtbl1_u8,
    };

    use crate::cpu::NeonToken;

    /// Eight pixels, deinterleaved into planes.
    #[target_feature(enable = "neon")]
    #[inline]
    fn load8(v: &[u32; 8]) -> uint8x8x4_t {
        // SAFETY: `v` is a live `[u32; 8]`, valid for 32 bytes of reads; `vld4_u8` needs only
        // byte alignment.
        unsafe { vld4_u8(v.as_ptr().cast()) }
    }

    /// Eight pixels, interleaved from planes.
    #[target_feature(enable = "neon")]
    #[inline]
    fn store8(v: &mut [u32; 8], r: uint8x8x4_t) {
        // SAFETY: `v` is a live `[u32; 8]`, valid for 32 bytes of writes; `vst4_u8` needs only
        // byte alignment.
        unsafe { vst4_u8(v.as_mut_ptr().cast(), r) };
    }

    /// Two pixels.
    #[target_feature(enable = "neon")]
    #[inline]
    fn load2(v: &[u32; 2]) -> uint8x8_t {
        // SAFETY: `v` is a live `[u32; 2]`, valid for 8 bytes of reads; `vld1_u8` needs only
        // byte alignment.
        unsafe { vld1_u8(v.as_ptr().cast()) }
    }

    /// Two pixels.
    #[target_feature(enable = "neon")]
    #[inline]
    fn store2(v: &mut [u32; 2], r: uint8x8_t) {
        // SAFETY: `v` is a live `[u32; 2]`, valid for 8 bytes of writes; `vst1_u8` needs only
        // byte alignment.
        unsafe { vst1_u8(v.as_mut_ptr().cast(), r) };
    }

    // SkMulDiv255Round() applied to each lane.
    // Port of: src/opts/SkBlitRow_opts.h#L69-L73 (chrome/m156)
    #[target_feature(enable = "neon")]
    #[inline]
    fn mul_div_255_round_neon8(x: uint8x8_t, y: uint8x8_t) -> uint8x8_t {
        let prod = vmull_u8(x, y);
        vraddhn_u16(prod, vrshrq_n_u16::<8>(prod))
    }

    // Port of: src/opts/SkBlitRow_opts.h#L75-L83 (chrome/m156)
    #[target_feature(enable = "neon")]
    #[inline]
    fn pm_src_over_neon8(dst: uint8x8x4_t, src: uint8x8x4_t) -> uint8x8x4_t {
        let nalphas = vmvn_u8(src.3); // 255 - alpha
        uint8x8x4_t(
            vqadd_u8(src.0, mul_div_255_round_neon8(nalphas, dst.0)),
            vqadd_u8(src.1, mul_div_255_round_neon8(nalphas, dst.1)),
            vqadd_u8(src.2, mul_div_255_round_neon8(nalphas, dst.2)),
            vqadd_u8(src.3, mul_div_255_round_neon8(nalphas, dst.3)),
        )
    }

    // Variant assuming dst and src contain the color components of two consecutive pixels.
    // Port of: src/opts/SkBlitRow_opts.h#L85-L90 (chrome/m156)
    #[target_feature(enable = "neon")]
    #[inline]
    fn pm_src_over_neon2(dst: uint8x8_t, src: uint8x8_t) -> uint8x8_t {
        let alpha_indices = vcreate_u8(0x0707_0707_0303_0303);
        let nalphas = vmvn_u8(vtbl1_u8(src, alpha_indices));
        vqadd_u8(src, mul_div_255_round_neon8(nalphas, dst))
    }

    #[target_feature(enable = "neon")]
    fn blit_neon(dst: &mut [u32], src: &[u32]) {
        let (d8, d_rest) = dst.as_chunks_mut::<8>();
        let (s8, s_rest) = src.as_chunks::<8>();
        for (d, s) in d8.iter_mut().zip(s8) {
            store8(d, pm_src_over_neon8(load8(d), load8(s)));
        }

        let (d2, d_rest) = d_rest.as_chunks_mut::<2>();
        let (s2, s_rest) = s_rest.as_chunks::<2>();
        for (d, s) in d2.iter_mut().zip(s2) {
            store2(d, pm_src_over_neon2(load2(d), load2(s)));
        }

        if let (Some(d), Some(s)) = (d_rest.first_mut(), s_rest.first()) {
            let result = pm_src_over_neon2(vcreate_u8(u64::from(*d)), vcreate_u8(u64::from(*s)));
            // `vst1_lane_u32(dst, vreinterpret_u32_u8(result), 0)`: the low pixel.
            // The truncation keeps exactly the low 32 bits (lane 0).
            #[allow(clippy::cast_possible_truncation)]
            {
                *d = vget_lane_u64::<0>(vreinterpret_u64_u8(result)) as u32;
            }
        }
    }

    pub(super) fn blit(_token: NeonToken, dst: &mut [u32], src: &[u32]) {
        // SAFETY: `blit_neon` enables `neon`, which `_token` proves the host supports (NEON is
        // also the aarch64 baseline).
        unsafe { blit_neon(dst, src) };
    }
}

#[cfg(test)]
mod tests;
