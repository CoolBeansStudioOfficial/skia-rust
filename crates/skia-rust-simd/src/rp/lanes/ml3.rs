// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h (SKRP_CPU_AVX2, built as `ml3`)

//! The `Ml3` tier (`SKRP_CPU_AVX2`, what `SkOpts` installs at run time on `ML3 = AVX2|BMI1|BMI2|
//! F16C|FMA` CPUs): 8 highp lanes, 16 lowp lanes, fused `mad`, F16C half conversions, `blendvps`
//! selects (sign bit only) and whole-register `vptest` for `any`/`all`.
//!
//! Every function is a safe `#[target_feature]` function with exactly `Ml3Token::FEATURES`
//! (`-march=x86-64-v3`). See [the module overview](super) for what each primitive does on each
//! tier.

use core::arch::x86_64::{
    __m128i, _MM_FROUND_CUR_DIRECTION, _mm_min_epu32, _mm_packus_epi16, _mm_packus_epi32,
    _mm_set1_epi32, _mm256_and_ps, _mm256_blendv_ps, _mm256_castps_si256, _mm256_castsi256_ps,
    _mm256_ceil_ps, _mm256_cvtepi32_pd, _mm256_cvtepi32_ps, _mm256_cvtph_ps, _mm256_cvtps_epi32,
    _mm256_cvtps_ph, _mm256_cvttpd_epi32, _mm256_cvttps_epi32, _mm256_div_pd,
    _mm256_extractf128_si256, _mm256_floor_ps, _mm256_fmadd_ps, _mm256_fnmadd_ps, _mm256_max_ps,
    _mm256_min_ps, _mm256_rcp_ps, _mm256_rsqrt_ps, _mm256_set1_epi32, _mm256_set1_ps,
    _mm256_setzero_ps, _mm256_sqrt_ps, _mm256_sub_ps, _mm256_testc_si256, _mm256_testz_si256,
};

use super::x86::{from_ps256, from_si, from_si256, ps256, si, si256};
use crate::vx::{Vec, join};

pub use super::portable::{abs_i, cond_to_mask, max_i, max_u, min_i, min_u};

/// Every function of the tier: Skia's `SI` with the tier's target features.
macro_rules! si {
    ($($(#[$m:meta])* $v:vis fn $name:ident($($args:tt)*) -> $ret:ty $body:block)*) => {$(
        $(#[$m])*
        ///
        /// # Safety
        /// The CPU must support the `Ml3` features (`Ml3Token`: AVX2, BMI1, BMI2, F16C, FMA and
        /// their predecessors). Code compiled with the `Ml3` tier's features, such as the tier's
        /// stage functions, calls this safely.
        #[target_feature(enable = "sse2,ssse3,sse4.1,sse4.2,avx,avx2,bmi1,bmi2,f16c,fma")]
        #[inline]
        #[must_use]
        $v fn $name($($args)*) -> $ret $body
    )*};
}

// Port of: src/opts/SkRasterPipeline_opts.h#L688-L696 (chrome/m156)
/// Highp stride.
pub const N: usize = 8;
/// Lowp stride.
pub const LOWP_N: usize = 16;
/// `float` lanes.
pub type F = Vec<8, f32>;
/// `int32_t` lanes.
pub type I32 = Vec<8, i32>;
/// `uint64_t` lanes.
pub type U64 = Vec<8, u64>;
/// `uint32_t` lanes.
pub type U32 = Vec<8, u32>;
/// `uint16_t` lanes.
pub type U16 = Vec<8, u16>;
/// `uint8_t` lanes.
pub type U8 = Vec<8, u8>;

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L698-L699 (chrome/m156)
    /// `mad(f, m, a)`: `_mm256_fmadd_ps`, `f*m + a` with one rounding.
    pub fn mad(f: F, m: F, a: F) -> F {
        from_ps256(_mm256_fmadd_ps(ps256(f), ps256(m), ps256(a)))
    }

    /// `nmad(f, m, a)`: `_mm256_fnmadd_ps`, `-(f*m) + a` with one rounding.
    pub fn nmad(f: F, m: F, a: F) -> F {
        from_ps256(_mm256_fnmadd_ps(ps256(f), ps256(m), ps256(a)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L701-L706 (chrome/m156)
    /// `min(F, F)`: `vminps`, `a < b ? a : b` (NaN or ±0 tie → `b`).
    pub fn min_f(a: F, b: F) -> F {
        from_ps256(_mm256_min_ps(ps256(a), ps256(b)))
    }

    /// `max(F, F)`: `vmaxps`, `a > b ? a : b` (NaN or ±0 tie → `b`).
    pub fn max_f(a: F, b: F) -> F {
        from_ps256(_mm256_max_ps(ps256(a), ps256(b)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L708-L719 (chrome/m156)
    /// `abs_(F)`: `v & (0 - v)` (a NaN keeps its sign bit).
    pub fn abs_f(v: F) -> F {
        let v = ps256(v);
        from_ps256(_mm256_and_ps(v, _mm256_sub_ps(_mm256_setzero_ps(), v)))
    }

    /// `floor_`: `vroundps` (exact `floorf`).
    pub fn floor_(v: F) -> F {
        from_ps256(_mm256_floor_ps(ps256(v)))
    }

    /// `ceil_`: `vroundps` (exact `ceilf`).
    pub fn ceil_(v: F) -> F {
        from_ps256(_mm256_ceil_ps(ps256(v)))
    }

    /// `rcp_approx`: `vrcpps` (a 12-bit, vendor-specific estimate). Use `rcp_fast`.
    pub fn rcp_approx(v: F) -> F {
        from_ps256(_mm256_rcp_ps(ps256(v)))
    }

    /// `rsqrt_approx`: `vrsqrtps`. Use `rsqrt`.
    pub fn rsqrt_approx(v: F) -> F {
        from_ps256(_mm256_rsqrt_ps(ps256(v)))
    }

    /// `sqrt_`: `vsqrtps` (exact).
    pub fn sqrt_(v: F) -> F {
        from_ps256(_mm256_sqrt_ps(ps256(v)))
    }

    /// `rcp_precise`: one fused Newton–Raphson step, `fnmadd(v, e, 2) * e` with `e = vrcpps(v)`.
    pub fn rcp_precise(v: F) -> F {
        let e = rcp_approx(v);
        from_ps256(_mm256_fnmadd_ps(ps256(v), ps256(e), _mm256_set1_ps(2.0))) * e
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L720-L721 (chrome/m156)
    /// `iround`: `vcvtps2dq` (ties to even; NaN and overflow give `0x80000000`).
    pub fn iround(v: F) -> I32 {
        from_si256(_mm256_cvtps_epi32(ps256(v)))
    }

    /// `round`: `vcvtps2dq`, as `U32`.
    pub fn round(v: F) -> U32 {
        from_si256(_mm256_cvtps_epi32(ps256(v)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L722-L729 (chrome/m156)
    /// `pack(U32) -> U16`: `packusdw` of the two 128-bit halves (unsigned saturation of the
    /// lanes as signed 32-bit values).
    pub fn pack_u32(v: U32) -> U16 {
        let v = si256(v);
        from_si(_mm_packus_epi32(
            _mm256_extractf128_si256::<0>(v),
            _mm256_extractf128_si256::<1>(v),
        ))
    }

    /// `pack(U16) -> U8`: `packuswb` (the `U16` lanes saturate as *signed* 16-bit values).
    pub fn pack_u16(v: U16) -> U8 {
        let r = si(v);
        let r: Vec<16, u8> = from_si(_mm_packus_epi16(r, r));
        r.lo() // sk_unaligned_load<U8>(&r)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L731-L734 (chrome/m156)
    /// `if_then_else(I32 c, F t, F e)`: `vblendvps`, so **only the sign bit** of each `c` lane
    /// selects.
    pub fn if_then_else_f(c: I32, t: F, e: F) -> F {
        from_ps256(_mm256_blendv_ps(ps256(e), ps256(t), _mm256_castsi256_ps(si256(c))))
    }

    /// `if_then_else(I32 c, I32 t, I32 e)`: `vblendvps` (sign bit only).
    pub fn if_then_else_i(c: I32, t: I32, e: I32) -> I32 {
        let (t, e) = (_mm256_castsi256_ps(si256(t)), _mm256_castsi256_ps(si256(e)));
        let r = _mm256_blendv_ps(e, t, _mm256_castsi256_ps(si256(c)));
        from_si256(_mm256_castps_si256(r))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L736-L738 (chrome/m156)
    /// `any(c)`: `!vptest(c, ~0).ZF`: **any bit** set anywhere in the register.
    pub fn any(c: I32) -> bool {
        _mm256_testz_si256(si256(c), _mm256_set1_epi32(-1)) == 0
    }

    /// `all(c)`: `vptest(c, ~0).CF`: **every bit** of the register set ("only works with mask
    /// values").
    pub fn all(c: I32) -> bool {
        _mm256_testc_si256(si256(c), _mm256_set1_epi32(-1)) != 0
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L780-L803 (chrome/m156)
    /// `SkSL` `int` division `div_fn(I32*, I32*)`: each half through `f64` and `vcvttpd2dq`
    /// (`x / 0` and `INT_MIN / -1` give `INT_MIN`).
    pub fn div_i32(d: I32, s: I32) -> I32 {
        join(div_f64_4(si(d.lo()), si(s.lo())), div_f64_4(si(d.hi()), si(s.hi())))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L806-L836 (chrome/m156)
    /// `SkSL` `uint` division `div_fn(U32*, U32*)`: operands clamped to `INT_MAX` with
    /// `pminud`, then the `f64` division (`x / 0` gives `0x80000000`).
    pub fn div_u32(d: U32, s: U32) -> U32 {
        // AVX2 (and below) lack a way to turn unsigned integers to doubles directly so we
        // clamp to INT_MAX before converting to doubles.
        let max_safe = _mm_set1_epi32(0x7FFF_FFFF);
        let d0_safe = _mm_min_epu32(si(d.lo()), max_safe);
        let d1_safe = _mm_min_epu32(si(d.hi()), max_safe);
        let s0_safe = _mm_min_epu32(si(s.lo()), max_safe);
        let s1_safe = _mm_min_epu32(si(s.hi()), max_safe);
        let r0: Vec<4, i32> = div_f64_4(d0_safe, s0_safe);
        let r1: Vec<4, i32> = div_f64_4(d1_safe, s1_safe);
        join(r0, r1).bit_cast()
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1588-L1600 (chrome/m156) (SIMD branch)
    /// `trunc_(v)`: `(U32)__builtin_convertvector(v, I32)`, i.e. `vcvttps2dq`.
    pub fn trunc_(v: F) -> U32 {
        from_si256(_mm256_cvttps_epi32(ps256(v)))
    }

    /// A float → `I32` vector cast (`__builtin_convertvector(v, I32)`): `vcvttps2dq`.
    pub fn to_i32(v: F) -> I32 {
        from_si256(_mm256_cvttps_epi32(ps256(v)))
    }

    /// `cast(U32) -> F`: `__builtin_convertvector((I32)v, F)`, a *signed* conversion
    /// (`vcvtdq2ps`).
    pub fn cast_f(v: U32) -> F {
        from_ps256(_mm256_cvtepi32_ps(si256(v)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1651-L1695 (chrome/m156) (SKRP_CPU_AVX2)
    /// `from_half`: F16C `vcvtph2ps` (exact; half denormals, infinities and NaNs kept).
    pub fn from_half(h: U16) -> F {
        from_ps256(_mm256_cvtph_ps(si(h)))
    }

    /// `to_half`: F16C `vcvtps2ph` with `_MM_FROUND_CUR_DIRECTION` (round to nearest even
    /// under the default MXCSR; half denormals produced, overflow to infinity).
    pub fn to_half(f: F) -> U16 {
        from_si(_mm256_cvtps_ph::<_MM_FROUND_CUR_DIRECTION>(ps256(f)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1742-L1745 (chrome/m156)
    /// `rcp_fast`: the raw `vrcpps` estimate.
    pub fn rcp_fast(v: F) -> F {
        rcp_approx(v)
    }

    /// `rsqrt`: the raw `vrsqrtps` estimate.
    pub fn rsqrt(v: F) -> F {
        rsqrt_approx(v)
    }
}

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L780-L803 (chrome/m156) (one 4-lane half)
    /// Four lanes of `div_fn`'s `f64` division: `vcvtdq2pd`, `vdivpd`, `vcvttpd2dq` (NaN, ±inf
    /// and overflow give `0x80000000`).
    #[allow(clippy::similar_names)] // the C++ locals' names (fd0, fs0, fr0)
    fn div_f64_4(d: __m128i, s: __m128i) -> Vec<4, i32> {
        let fd0 = _mm256_cvtepi32_pd(d);
        let fs0 = _mm256_cvtepi32_pd(s);
        let fr0 = _mm256_div_pd(fd0, fs0);
        from_si(_mm256_cvttpd_epi32(fr0)) // (truncates toward zero)
    }
}

#[cfg(test)]
lane_harness!();

/// The lowp primitives (`namespace lowp`), 16 lanes.
pub mod lowp {
    use core::arch::x86_64::{
        _mm256_max_epi32, _mm256_max_epu16, _mm256_min_epi32, _mm256_min_epu16, _mm256_mulhrs_epi16,
    };

    use super::super::x86::{from_si256, si256};
    use crate::vx::{Vec, join};

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
        // Port of: src/opts/SkRasterPipeline_opts.h#L5820-L5846 (chrome/m156) (SKRP_CPU_AVX2)
        /// `max_intr(F, F)`: `vmaxps` on each 256-bit half.
        pub fn max_intr_f(x: F, y: F) -> F {
            join(super::max_f(x.lo(), y.lo()), super::max_f(x.hi(), y.hi()))
        }

        /// `min_intr(F, F)`: `vminps` on each 256-bit half.
        pub fn min_intr_f(x: F, y: F) -> F {
            join(super::min_f(x.lo(), y.lo()), super::min_f(x.hi(), y.hi()))
        }

        /// `max_intr(I32, I32)`: `vpmaxsd` on each half.
        pub fn max_intr_i(x: I32, y: I32) -> I32 {
            let lo = _mm256_max_epi32(si256(x.lo()), si256(y.lo()));
            let hi = _mm256_max_epi32(si256(x.hi()), si256(y.hi()));
            join(from_si256::<8, i32>(lo), from_si256(hi))
        }

        /// `min_intr(I32, I32)`: `vpminsd` on each half.
        pub fn min_intr_i(x: I32, y: I32) -> I32 {
            let lo = _mm256_min_epi32(si256(x.lo()), si256(y.lo()));
            let hi = _mm256_min_epi32(si256(x.hi()), si256(y.hi()));
            join(from_si256::<8, i32>(lo), from_si256(hi))
        }

        /// `max_intr(U16, U16)`: `vpmaxuw`.
        pub fn max_intr_u16(x: U16, y: U16) -> U16 {
            from_si256(_mm256_max_epu16(si256(x), si256(y)))
        }

        /// `min_intr(U16, U16)`: `vpminuw`.
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

        /// `trunc_(F) -> U32`: `(U32)cast<I32>(x)`, `vcvttps2dq` per half.
        pub fn trunc_(x: F) -> U32 {
            join(super::trunc_(x.lo()), super::trunc_(x.hi()))
        }

        /// `cast<I32>(F)`: `vcvttps2dq` per half.
        pub fn to_i32(x: F) -> I32 {
            join(super::to_i32(x.lo()), super::to_i32(x.hi()))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L5939-L5966 (chrome/m156) (SKRP_CPU_AVX2)
        /// `rcp_precise`: the highp (fused) `rcp_precise` on each half.
        pub fn rcp_precise(x: F) -> F {
            join(super::rcp_precise(x.lo()), super::rcp_precise(x.hi()))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L5967-L6006 (chrome/m156) (SKRP_CPU_AVX2)
        /// `sqrt_`: `vsqrtps` on each half.
        pub fn sqrt_(x: F) -> F {
            join(super::sqrt_(x.lo()), super::sqrt_(x.hi()))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L6008-L6035 (chrome/m156) (SKRP_CPU_AVX2)
        /// `floor_`: `vroundps` on each half.
        pub fn floor_(x: F) -> F {
            join(super::floor_(x.lo()), super::floor_(x.hi()))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L6042-L6063 (chrome/m156) (SKRP_CPU_AVX2)
        /// `scaled_mult(a, b)`: `vpmulhrsw` (`(a*b + 2^14) >> 15`, wrapping).
        pub fn scaled_mult(a: I16, b: I16) -> I16 {
            from_si256(_mm256_mulhrs_epi16(si256(a), si256(b)))
        }
    }

    #[cfg(test)]
    lowp_harness!();
}
