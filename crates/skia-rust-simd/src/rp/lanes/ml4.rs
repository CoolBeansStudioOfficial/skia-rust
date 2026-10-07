// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h (SKRP_CPU_ML4)

//! The `Ml4` tier (`SKRP_CPU_ML4`, what `SkOpts` installs at run time on `ML4 = AVX512F|DQ|CD|
//! BW|VL` CPUs): 16 highp lanes, 16 lowp lanes, fused `mad`, F16C half conversions, the 14-bit
//! `vrcp14ps`/`vrsqrt14ps` estimates, sign-bit selects, per-lane `vptestmd` for `any`/`all`, and
//! an exact unsigned `div_fn`.
//!
//! Every function is a safe `#[target_feature]` function with exactly `Ml4Token::FEATURES`
//! (`-march=x86-64-v4`); the `Ml3` functions it reuses need a subset of those features. See
//! [the module overview](super) for what each primitive does on each tier.

use core::arch::x86_64::{
    __m128i, _MM_FROUND_CEIL, _MM_FROUND_CUR_DIRECTION, _MM_FROUND_FLOOR, _mm256_castsi128_si256,
    _mm256_castsi256_si128, _mm256_cvtepi32_pd, _mm256_cvtepu32_pd, _mm256_cvttpd_epi32,
    _mm256_cvttpd_epu32, _mm256_div_pd, _mm256_insertf128_si256, _mm256_packus_epi16,
    _mm256_packus_epi32, _mm256_permute4x64_epi64, _mm256_permutex_epi64, _mm512_and_ps,
    _mm512_and_si512, _mm512_castsi512_si256, _mm512_cvtepi32_ps, _mm512_cvtph_ps,
    _mm512_cvtps_epi32, _mm512_cvtps_ph, _mm512_cvttps_epi32, _mm512_extracti64x4_epi64,
    _mm512_fmadd_ps, _mm512_fnmadd_ps, _mm512_mask_blend_epi32, _mm512_mask_blend_ps,
    _mm512_max_ps, _mm512_min_ps, _mm512_rcp14_ps, _mm512_roundscale_ps, _mm512_rsqrt14_ps,
    _mm512_set1_epi32, _mm512_set1_ps, _mm512_setzero_ps, _mm512_sqrt_ps, _mm512_sub_ps,
    _mm512_test_epi32_mask,
};

use super::x86::{from_ps512, from_si, from_si256, from_si512, ps512, si, si256, si512};
use crate::vx::{Vec, join};

pub use super::portable::{abs_i, cond_to_mask, max_i, max_u, min_i, min_u};

/// Every function of the tier: Skia's `SI` with the tier's target features.
macro_rules! si {
    ($($(#[$m:meta])* $v:vis fn $name:ident($($args:tt)*) -> $ret:ty $body:block)*) => {$(
        $(#[$m])*
        ///
        /// # Safety
        /// The CPU must support the `Ml4` features (`Ml4Token`: the `Ml3` features plus
        /// AVX-512 F, DQ, CD, BW and VL). Code compiled with the `Ml4` tier's features, such as
        /// the tier's stage functions, calls this safely.
        #[target_feature(
            enable = "sse2,ssse3,sse4.1,sse4.2,avx,avx2,bmi1,bmi2,f16c,fma,avx512f,avx512dq,avx512cd,avx512bw,avx512vl"
        )]
        #[inline]
        #[must_use]
        $v fn $name($($args)*) -> $ret $body
    )*};
}

// Port of: src/opts/SkRasterPipeline_opts.h#L334-L341 (chrome/m156)
/// Highp stride.
pub const N: usize = 16;
/// Lowp stride.
pub const LOWP_N: usize = 16;
/// `float` lanes.
pub type F = Vec<16, f32>;
/// `int32_t` lanes.
pub type I32 = Vec<16, i32>;
/// `uint64_t` lanes.
pub type U64 = Vec<16, u64>;
/// `uint32_t` lanes.
pub type U32 = Vec<16, u32>;
/// `uint16_t` lanes.
pub type U16 = Vec<16, u16>;
/// `uint8_t` lanes.
pub type U8 = Vec<16, u8>;

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L343-L344 (chrome/m156)
    /// `mad(f, m, a)`: `_mm512_fmadd_ps`, `f*m + a` with one rounding.
    pub fn mad(f: F, m: F, a: F) -> F {
        from_ps512(_mm512_fmadd_ps(ps512(f), ps512(m), ps512(a)))
    }

    /// `nmad(f, m, a)`: `_mm512_fnmadd_ps`, `-(f*m) + a` with one rounding.
    pub fn nmad(f: F, m: F, a: F) -> F {
        from_ps512(_mm512_fnmadd_ps(ps512(f), ps512(m), ps512(a)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L345-L350 (chrome/m156)
    /// `min(F, F)`: `vminps zmm`, `a < b ? a : b` (NaN or ±0 tie → `b`).
    pub fn min_f(a: F, b: F) -> F {
        from_ps512(_mm512_min_ps(ps512(a), ps512(b)))
    }

    /// `max(F, F)`: `vmaxps zmm`, `a > b ? a : b` (NaN or ±0 tie → `b`).
    pub fn max_f(a: F, b: F) -> F {
        from_ps512(_mm512_max_ps(ps512(a), ps512(b)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L351-L361 (chrome/m156)
    /// `abs_(F)`: `v & (0 - v)` (a NaN keeps its sign bit).
    pub fn abs_f(v: F) -> F {
        let v = ps512(v);
        from_ps512(_mm512_and_ps(v, _mm512_sub_ps(_mm512_setzero_ps(), v)))
    }

    /// `floor_`: `vrndscaleps` (exact `floorf`).
    pub fn floor_(v: F) -> F {
        from_ps512(_mm512_roundscale_ps::<_MM_FROUND_FLOOR>(ps512(v)))
    }

    /// `ceil_`: `vrndscaleps` (exact `ceilf`).
    pub fn ceil_(v: F) -> F {
        from_ps512(_mm512_roundscale_ps::<_MM_FROUND_CEIL>(ps512(v)))
    }

    /// `rcp_approx`: `vrcp14ps` (a 14-bit estimate; Intel's reference algorithm). Use
    /// `rcp_fast`.
    pub fn rcp_approx(v: F) -> F {
        from_ps512(_mm512_rcp14_ps(ps512(v)))
    }

    /// `rsqrt_approx`: `vrsqrt14ps`. Use `rsqrt`.
    pub fn rsqrt_approx(v: F) -> F {
        from_ps512(_mm512_rsqrt14_ps(ps512(v)))
    }

    /// `sqrt_`: `vsqrtps zmm` (exact).
    pub fn sqrt_(v: F) -> F {
        from_ps512(_mm512_sqrt_ps(ps512(v)))
    }

    /// `rcp_precise`: one fused Newton–Raphson step, `fnmadd(v, e, 2) * e` with
    /// `e = vrcp14ps(v)`.
    pub fn rcp_precise(v: F) -> F {
        let e = rcp_approx(v);
        from_ps512(_mm512_fnmadd_ps(ps512(v), ps512(e), _mm512_set1_ps(2.0))) * e
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L362-L363 (chrome/m156)
    /// `iround`: `vcvtps2dq` (ties to even; NaN and overflow give `0x80000000`).
    pub fn iround(v: F) -> I32 {
        from_si512(_mm512_cvtps_epi32(ps512(v)))
    }

    /// `round`: `vcvtps2dq`, as `U32`.
    pub fn round(v: F) -> U32 {
        from_si512(_mm512_cvtps_epi32(ps512(v)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L364-L372 (chrome/m156)
    /// `pack(U32) -> U16`: `vpackusdw` of the two 256-bit halves, lanes put back in order
    /// (unsigned saturation of the lanes as signed 32-bit values).
    pub fn pack_u32(v: U32) -> U16 {
        let v = si512(v);
        let rst = _mm256_packus_epi32(_mm512_castsi512_si256(v), _mm512_extracti64x4_epi64::<1>(v));
        from_si256(_mm256_permutex_epi64::<216>(rst))
    }

    /// `pack(U16) -> U8`: `vpackuswb` (the `U16` lanes saturate as *signed* 16-bit values).
    pub fn pack_u16(v: U16) -> U8 {
        let rst = _mm256_packus_epi16(si256(v), si256(v));
        from_si(_mm256_castsi256_si128(_mm256_permute4x64_epi64::<8>(rst)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L373-L382 (chrome/m156)
    /// `if_then_else(I32 c, F t, F e)`: `vptestmd` of `c & 0x80000000`, then a masked blend:
    /// **only the sign bit** of each `c` lane selects.
    pub fn if_then_else_f(c: I32, t: F, e: F) -> F {
        let mask = _mm512_set1_epi32(i32::MIN); // 0x80000000
        let aa = _mm512_and_si512(si512(c), mask);
        from_ps512(_mm512_mask_blend_ps(_mm512_test_epi32_mask(aa, aa), ps512(e), ps512(t)))
    }

    /// `if_then_else(I32 c, I32 t, I32 e)`: sign bit only.
    pub fn if_then_else_i(c: I32, t: I32, e: I32) -> I32 {
        let mask = _mm512_set1_epi32(i32::MIN); // 0x80000000
        let aa = _mm512_and_si512(si512(c), mask);
        from_si512(_mm512_mask_blend_epi32(_mm512_test_epi32_mask(aa, aa), si512(e), si512(t)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L383-L390 (chrome/m156)
    /// `any(c)`: `vptestmd(c, c) != 0`: some lane has **any bit** set.
    pub fn any(c: I32) -> bool {
        let c = si512(c);
        _mm512_test_epi32_mask(c, c) != 0
    }

    /// `all(c)`: `vptestmd(c, c) == 0xffff`: every lane has some bit set.
    pub fn all(c: I32) -> bool {
        let c = si512(c);
        _mm512_test_epi32_mask(c, c) == 0xffff
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L439-L488 (chrome/m156)
    /// `SkSL` `int` division `div_fn(I32*, I32*)`: four 4-lane groups through `f64` and
    /// `vcvttpd2dq` (`x / 0` and `INT_MIN / -1` give `INT_MIN`).
    pub fn div_i32(d: I32, s: I32) -> I32 {
        let (d_lo, d_hi, s_lo, s_hi) = (d.lo(), d.hi(), s.lo(), s.hi());
        let r0 = div_i32_4(si(d_lo.lo()), si(s_lo.lo()));
        let r1 = div_i32_4(si(d_lo.hi()), si(s_lo.hi()));
        let r2 = div_i32_4(si(d_hi.lo()), si(s_hi.lo()));
        let r3 = div_i32_4(si(d_hi.hi()), si(s_hi.hi()));
        // Recombine the four 128-bit blocks into two 256-bit registers.
        let r_lo = _mm256_insertf128_si256::<1>(_mm256_castsi128_si256(r0), r1);
        let r_hi = _mm256_insertf128_si256::<1>(_mm256_castsi128_si256(r2), r3);
        join(from_si256::<8, i32>(r_lo), from_si256(r_hi))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L491-L540 (chrome/m156)
    /// `SkSL` `uint` division `div_fn(U32*, U32*)`: four 4-lane groups through `f64` with the
    /// unsigned conversions `vcvtudq2pd`/`vcvttpd2udq`, so the quotient is **exact**; `x / 0`
    /// gives `0xFFFFFFFF` (the unsigned integer indefinite).
    pub fn div_u32(d: U32, s: U32) -> U32 {
        let (d_lo, d_hi, s_lo, s_hi) = (d.lo(), d.hi(), s.lo(), s.hi());
        let r0 = div_u32_4(si(d_lo.lo()), si(s_lo.lo()));
        let r1 = div_u32_4(si(d_lo.hi()), si(s_lo.hi()));
        let r2 = div_u32_4(si(d_hi.lo()), si(s_hi.lo()));
        let r3 = div_u32_4(si(d_hi.hi()), si(s_hi.hi()));
        // Recombine the four 128-bit blocks into two 256-bit registers.
        let r_lo = _mm256_insertf128_si256::<1>(_mm256_castsi128_si256(r0), r1);
        let r_hi = _mm256_insertf128_si256::<1>(_mm256_castsi128_si256(r2), r3);
        join(from_si256::<8, u32>(r_lo), from_si256(r_hi))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1588-L1600 (chrome/m156) (SIMD branch)
    /// `trunc_(v)`: `(U32)__builtin_convertvector(v, I32)`, i.e. `vcvttps2dq`.
    pub fn trunc_(v: F) -> U32 {
        from_si512(_mm512_cvttps_epi32(ps512(v)))
    }

    /// A float → `I32` vector cast (`__builtin_convertvector(v, I32)`): `vcvttps2dq`.
    pub fn to_i32(v: F) -> I32 {
        from_si512(_mm512_cvttps_epi32(ps512(v)))
    }

    /// `cast(U32) -> F`: `__builtin_convertvector((I32)v, F)`, a *signed* conversion
    /// (`vcvtdq2ps`).
    pub fn cast_f(v: U32) -> F {
        from_ps512(_mm512_cvtepi32_ps(si512(v)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1651-L1695 (chrome/m156) (SKRP_CPU_ML4)
    /// `from_half`: `vcvtph2ps zmm` (exact; half denormals, infinities and NaNs kept).
    pub fn from_half(h: U16) -> F {
        from_ps512(_mm512_cvtph_ps(si256(h)))
    }

    /// `to_half`: `vcvtps2ph zmm` with `_MM_FROUND_CUR_DIRECTION` (round to nearest even under
    /// the default MXCSR; half denormals produced, overflow to infinity).
    pub fn to_half(f: F) -> U16 {
        from_si256(_mm512_cvtps_ph::<_MM_FROUND_CUR_DIRECTION>(ps512(f)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1742-L1745 (chrome/m156)
    /// `rcp_fast`: the raw `vrcp14ps` estimate.
    pub fn rcp_fast(v: F) -> F {
        rcp_approx(v)
    }

    /// `rsqrt`: the raw `vrsqrt14ps` estimate.
    pub fn rsqrt(v: F) -> F {
        rsqrt_approx(v)
    }
}

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L439-L488 (chrome/m156) (one 4-lane group)
    /// `vcvtdq2pd`, `vdivpd`, `vcvttpd2dq` (NaN, ±inf and overflow give `0x80000000`).
    #[allow(clippy::similar_names)] // the C++ locals' names (fd0, fs0, fr0)
    fn div_i32_4(d0: __m128i, s0: __m128i) -> __m128i {
        let fd0 = _mm256_cvtepi32_pd(d0); // convert to double
        let fs0 = _mm256_cvtepi32_pd(s0);
        let fr0 = _mm256_div_pd(fd0, fs0); // divide
        _mm256_cvttpd_epi32(fr0) // convert to int (truncates toward zero)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L491-L540 (chrome/m156) (one 4-lane group)
    /// `vcvtudq2pd`, `vdivpd`, `vcvttpd2udq` (NaN and ±inf give `0xFFFFFFFF`).
    #[allow(clippy::similar_names)] // the C++ locals' names (fd0, fs0, fr0)
    fn div_u32_4(d0: __m128i, s0: __m128i) -> __m128i {
        let fd0 = _mm256_cvtepu32_pd(d0); // convert to double
        let fs0 = _mm256_cvtepu32_pd(s0);
        let fr0 = _mm256_div_pd(fd0, fs0); // divide
        _mm256_cvttpd_epu32(fr0) // convert to uint (truncates toward zero)
    }
}

#[cfg(test)]
lane_harness!();

/// The lowp primitives (`namespace lowp`), 16 lanes (the same width as highp on this tier).
pub mod lowp {
    use core::arch::x86_64::{
        _mm256_max_epu16, _mm256_min_epu16, _mm256_mulhrs_epi16, _mm512_fnmadd_ps,
        _mm512_max_epi32, _mm512_min_epi32, _mm512_rcp14_ps, _mm512_set1_ps,
    };

    use super::super::x86::{from_ps512, from_si256, from_si512, ps512, si256, si512};
    use crate::vx::Vec;

    pub use super::super::portable::lowp::{
        div255, div255_accurate, if_then_else_f, if_then_else_i, if_then_else_u16,
        if_then_else_u32, max_f, max_i, max_u16, min_f, min_i, min_u16,
    };

    // Port of: src/opts/SkRasterPipeline_opts.h#L5484-L5499 (chrome/m156)
    /// Lowp stride.
    pub const N: usize = 16;
    /// `uint8_t` lanes.
    pub type U8 = Vec<16, u8>;
    /// `uint16_t` lanes.
    pub type U16 = Vec<16, u16>;
    /// `int16_t` lanes.
    pub type I16 = Vec<16, i16>;
    /// `int32_t` lanes.
    pub type I32 = Vec<16, i32>;
    /// `uint32_t` lanes.
    pub type U32 = Vec<16, u32>;
    /// `int64_t` lanes.
    pub type I64 = Vec<16, i64>;
    /// `uint64_t` lanes.
    pub type U64 = Vec<16, u64>;
    /// `float` lanes.
    pub type F = Vec<16, f32>;

    si! {
        // Port of: src/opts/SkRasterPipeline_opts.h#L5813-L5819 (chrome/m156) (SKRP_CPU_ML4)
        /// `max_intr(F, F)`: `vmaxps zmm`.
        pub fn max_intr_f(x: F, y: F) -> F {
            super::max_f(x, y)
        }

        /// `min_intr(F, F)`: `vminps zmm`.
        pub fn min_intr_f(x: F, y: F) -> F {
            super::min_f(x, y)
        }

        /// `max_intr(I32, I32)`: `vpmaxsd zmm`.
        pub fn max_intr_i(x: I32, y: I32) -> I32 {
            from_si512(_mm512_max_epi32(si512(x), si512(y)))
        }

        /// `min_intr(I32, I32)`: `vpminsd zmm`.
        pub fn min_intr_i(x: I32, y: I32) -> I32 {
            from_si512(_mm512_min_epi32(si512(x), si512(y)))
        }

        /// `max_intr(U16, U16)`: `vpmaxuw ymm`.
        pub fn max_intr_u16(x: U16, y: U16) -> U16 {
            from_si256(_mm256_max_epu16(si256(x), si256(y)))
        }

        /// `min_intr(U16, U16)`: `vpminuw ymm`.
        pub fn min_intr_u16(x: U16, y: U16) -> U16 {
            from_si256(_mm256_min_epu16(si256(x), si256(y)))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L5920-L5937 (chrome/m156)
        /// `mad(f, m, a)`: `a + f*m`, **unfused** (lowp's `mad` is plain arithmetic on every
        /// tier).
        pub fn mad(f: F, m: F, a: F) -> F {
            a + f * m
        }

        /// `nmad(f, m, a)`: `a - f*m`, unfused.
        pub fn nmad(f: F, m: F, a: F) -> F {
            a - f * m
        }

        /// `trunc_(F) -> U32`: `(U32)cast<I32>(x)`, `vcvttps2dq`.
        pub fn trunc_(x: F) -> U32 {
            super::trunc_(x)
        }

        /// `cast<I32>(F)`: `vcvttps2dq`.
        pub fn to_i32(x: F) -> I32 {
            super::to_i32(x)
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L5939-L5966 (chrome/m156) (SKRP_CPU_ML4)
        /// `rcp_precise`: `vrcp14ps` and one fused Newton–Raphson step (the highp formula).
        pub fn rcp_precise(x: F) -> F {
            let e = from_ps512(_mm512_rcp14_ps(ps512(x)));
            from_ps512(_mm512_fnmadd_ps(ps512(x), ps512(e), _mm512_set1_ps(2.0))) * e
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L5967-L6006 (chrome/m156) (SKRP_CPU_ML4)
        /// `sqrt_`: `vsqrtps zmm`.
        pub fn sqrt_(x: F) -> F {
            super::sqrt_(x)
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L6008-L6035 (chrome/m156) (SKRP_CPU_ML4)
        /// `floor_`: `vrndscaleps`.
        pub fn floor_(x: F) -> F {
            super::floor_(x)
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L6042-L6063 (chrome/m156) (SKRP_CPU_ML4)
        /// `scaled_mult(a, b)`: `vpmulhrsw ymm` (`(a*b + 2^14) >> 15`, wrapping).
        pub fn scaled_mult(a: I16, b: I16) -> I16 {
            from_si256(_mm256_mulhrs_epi16(si256(a), si256(b)))
        }
    }

    #[cfg(test)]
    lowp_harness!();
}
