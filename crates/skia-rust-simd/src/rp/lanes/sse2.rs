// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h (SKRP_CPU_SSE2)

//! The `Sse2` tier (`SKRP_CPU_SSE2`): 4 highp lanes, 8 lowp lanes, Skia's SSE2 instructions.
//!
//! Every function is a safe `#[target_feature(enable = "sse2")]` function (`Sse2Token::FEATURES`).
//! See [the module overview](super) for what each primitive does on each tier.

use core::arch::x86_64::{
    __m128i, _mm_and_ps, _mm_and_si128, _mm_andnot_ps, _mm_andnot_si128, _mm_castps_si128,
    _mm_castsi128_ps, _mm_cvtepi32_pd, _mm_cvtepi32_ps, _mm_cvtps_epi32, _mm_cvttpd_epi32,
    _mm_cvttps_epi32, _mm_div_pd, _mm_max_ps, _mm_min_ps, _mm_movemask_ps, _mm_or_ps, _mm_or_si128,
    _mm_packs_epi32, _mm_packus_epi16, _mm_rcp_ps, _mm_rsqrt_ps, _mm_set1_epi32, _mm_setzero_ps,
    _mm_slli_epi32, _mm_sqrt_ps, _mm_srai_epi32, _mm_srli_si128, _mm_sub_ps, _mm_unpacklo_epi64,
};

use super::x86::{from_ps, from_si, ps, si};
use crate::vx::{Vec, join};

pub use super::portable::{abs_i, cond_to_mask, max_i, max_u, min_i, min_u};

/// Every function of the tier: Skia's `SI` with the tier's target features.
macro_rules! si {
    ($($(#[$m:meta])* $v:vis fn $name:ident($($args:tt)*) -> $ret:ty $body:block)*) => {$(
        $(#[$m])*
        ///
        /// # Safety
        /// The CPU must support SSE2 (`Sse2Token`). Code compiled with the `Sse2` tier's
        /// features, such as the tier's stage functions, calls this safely.
        #[target_feature(enable = "sse2")]
        #[inline]
        #[must_use]
        $v fn $name($($args)*) -> $ret $body
    )*};
}

// Port of: src/opts/SkRasterPipeline_opts.h#L933-L940 (chrome/m156)
/// Highp stride.
pub const N: usize = 4;
/// Lowp stride.
pub const LOWP_N: usize = 8;
/// `float` lanes.
pub type F = Vec<4, f32>;
/// `int32_t` lanes.
pub type I32 = Vec<4, i32>;
/// `uint64_t` lanes.
pub type U64 = Vec<4, u64>;
/// `uint32_t` lanes.
pub type U32 = Vec<4, u32>;
/// `uint16_t` lanes.
pub type U16 = Vec<4, u16>;
/// `uint8_t` lanes.
pub type U8 = Vec<4, u8>;

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L942-L948 (chrome/m156)
    /// `if_then_else(I32 c, F t, F e)`: bitwise `(c & t) | (~c & e)`.
    pub fn if_then_else_f(c: I32, t: F, e: F) -> F {
        let c = _mm_castsi128_ps(si(c));
        from_ps(_mm_or_ps(_mm_and_ps(c, ps(t)), _mm_andnot_ps(c, ps(e))))
    }

    /// `if_then_else(I32 c, I32 t, I32 e)`: bitwise.
    pub fn if_then_else_i(c: I32, t: I32, e: I32) -> I32 {
        let c = _mm_castsi128_ps(si(c));
        let (t, e) = (_mm_castsi128_ps(si(t)), _mm_castsi128_ps(si(e)));
        from_si(_mm_castps_si128(_mm_or_ps(_mm_and_ps(c, t), _mm_andnot_ps(c, e))))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L950-L951 (chrome/m156)
    /// `min(F, F)`: `minps`, `a < b ? a : b` (NaN or ±0 tie → `b`).
    pub fn min_f(a: F, b: F) -> F {
        from_ps(_mm_min_ps(ps(a), ps(b)))
    }

    /// `max(F, F)`: `maxps`, `a > b ? a : b` (NaN or ±0 tie → `b`).
    pub fn max_f(a: F, b: F) -> F {
        from_ps(_mm_max_ps(ps(a), ps(b)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L968-L980 (chrome/m156)
    /// `mad(f, m, a)`: `a + f*m`, unfused.
    pub fn mad(f: F, m: F, a: F) -> F {
        a + f * m
    }

    /// `nmad(f, m, a)`: `a - f*m`, unfused.
    pub fn nmad(f: F, m: F, a: F) -> F {
        a - f * m
    }

    /// `abs_(F)`: `v & (0 - v)` (a NaN keeps its sign bit).
    pub fn abs_f(v: F) -> F {
        let v = ps(v);
        from_ps(_mm_and_ps(v, _mm_sub_ps(_mm_setzero_ps(), v)))
    }

    /// `rcp_approx`: `rcpps` (a 12-bit, vendor-specific estimate). Use `rcp_fast`.
    pub fn rcp_approx(v: F) -> F {
        from_ps(_mm_rcp_ps(ps(v)))
    }

    /// `rcp_precise`: one Newton–Raphson step, `e * (2 - v*e)` with `e = rcpps(v)`.
    pub fn rcp_precise(v: F) -> F {
        let e = rcp_approx(v);
        e * (2.0 - v * e)
    }

    /// `rsqrt_approx`: `rsqrtps`. Use `rsqrt`.
    pub fn rsqrt_approx(v: F) -> F {
        from_ps(_mm_rsqrt_ps(ps(v)))
    }

    /// `sqrt_`: `sqrtps` (exact).
    pub fn sqrt_(v: F) -> F {
        from_ps(_mm_sqrt_ps(ps(v)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L981-L1003 (chrome/m156)
    /// The `f64` division of `div_fn`: both halves converted to `f64`, divided, truncated back
    /// (`cvttpd2dq`: NaN, ±inf and overflow give `0x80000000`).
    #[allow(clippy::similar_names)] // the C++ locals' names (fd_lo, fs_lo, fr_lo, …)
    pub(crate) fn div_f64(d: __m128i, s: __m128i) -> __m128i {
        // Convert top 2 lanes to double, divide, and convert back to integer.
        // Double-precision division natively handles dividing by zero (producing +/-Inf/NaN)
        // and INT_MIN / -1 overflow (producing +2147483648.0) without any hardware crash.
        // Upon truncation back to I32, both saturate to INT_MIN (0x80000000).
        let fd_lo = _mm_cvtepi32_pd(d);
        let fs_lo = _mm_cvtepi32_pd(s);
        let fr_lo = _mm_div_pd(fd_lo, fs_lo);
        let r_lo = _mm_cvttpd_epi32(fr_lo);
        // Same for upper 2 lanes.
        let fd_hi = _mm_cvtepi32_pd(_mm_srli_si128::<8>(d));
        let fs_hi = _mm_cvtepi32_pd(_mm_srli_si128::<8>(s));
        let fr_hi = _mm_div_pd(fd_hi, fs_hi);
        let r_hi = _mm_cvttpd_epi32(fr_hi);
        // Recombine lanes
        _mm_unpacklo_epi64(r_lo, r_hi)
    }

    /// `SkSL` `int` division `div_fn(I32*, I32*)`: `x / 0` and `INT_MIN / -1` give `INT_MIN`.
    pub fn div_i32(d: I32, s: I32) -> I32 {
        from_si(div_f64(si(d), si(s)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1006-L1040 (chrome/m156) (SSE2 clamp)
    /// `SkSL` `uint` division `div_fn(U32*, U32*)`: both operands clamped to `INT_MAX`, then the
    /// `f64` division (`x / 0` gives `0x80000000`).
    pub fn div_u32(d: U32, s: U32) -> U32 {
        let d_raw = si(d);
        let s_raw = si(s);
        // AVX2 (and below) lack a way to turn unsigned integers to doubles directly so we
        // clamp to INT_MAX before converting to doubles.
        let max_safe = _mm_set1_epi32(0x7FFF_FFFF);
        let d_mask = _mm_srai_epi32::<31>(d_raw);
        let d = _mm_or_si128(_mm_andnot_si128(d_mask, d_raw), _mm_and_si128(d_mask, max_safe));
        let s_mask = _mm_srai_epi32::<31>(s_raw);
        let s = _mm_or_si128(_mm_andnot_si128(s_mask, s_raw), _mm_and_si128(s_mask, max_safe));
        from_si(div_f64(d, s))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1042-L1043 (chrome/m156)
    /// `iround`: `cvtps2dq` (ties to even; NaN and overflow give `0x80000000`).
    pub fn iround(v: F) -> I32 {
        from_si(_mm_cvtps_epi32(ps(v)))
    }

    /// `round`: `cvtps2dq`, as `U32`.
    pub fn round(v: F) -> U32 {
        from_si(_mm_cvtps_epi32(ps(v)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1045-L1053 (chrome/m156) (SSE2 branch)
    /// `pack(U32) -> U16`: sign-extend the low 16 bits, then `packssdw`: truncation.
    pub fn pack_u32(v: U32) -> U16 {
        // Sign extend so that _mm_packs_epi32() does the pack we want.
        let p = _mm_srai_epi32::<16>(_mm_slli_epi32::<16>(si(v)));
        let p = _mm_packs_epi32(p, p);
        let p: Vec<8, u16> = from_si(p);
        p.lo() // We have two copies.  Return (the lower) one.
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1055-L1059 (chrome/m156)
    /// `pack(U16) -> U8`: `packuswb` (the `U16` lanes saturate as *signed* 16-bit values).
    pub fn pack_u16(v: U16) -> U8 {
        // widen_cast<__m128i>(v): the upper half is unspecified in Skia; it never reaches the
        // returned (lower) lanes.
        let r = si(join(v, Vec::<4, u16>::splat(0)));
        let r: Vec<16, u8> = from_si(_mm_packus_epi16(r, r));
        r.lo().lo()
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1061-L1063 (chrome/m156)
    /// `any(c)`: `movmskps`, so only each lane's sign bit counts.
    pub fn any(c: I32) -> bool {
        _mm_movemask_ps(_mm_castsi128_ps(si(c))) != 0b0000
    }

    /// `all(c)`: every lane's sign bit set.
    pub fn all(c: I32) -> bool {
        _mm_movemask_ps(_mm_castsi128_ps(si(c))) == 0b1111
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1065-L1081 (chrome/m156) (SSE2 branch)
    /// `floor_`: `cvt(cvtt(v))` corrected by one. `floor_(-0.0) = +0.0`; NaN and
    /// `|v| >= 2^31` give `-2147483648.0` (or one less, rounded).
    pub fn floor_(v: F) -> F {
        let roundtrip = from_ps(_mm_cvtepi32_ps(_mm_cvttps_epi32(ps(v))));
        roundtrip - if_then_else_f(roundtrip.gt_mask(v), F::splat(1.0), F::splat(0.0))
    }

    /// `ceil_`: `cvt(cvtt(v))` corrected by one (`ceil_(-0.5) = +0.0`).
    pub fn ceil_(v: F) -> F {
        let roundtrip = from_ps(_mm_cvtepi32_ps(_mm_cvttps_epi32(ps(v))));
        roundtrip + if_then_else_f(roundtrip.lt_mask(v), F::splat(1.0), F::splat(0.0))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1588-L1600 (chrome/m156) (SIMD branch)
    /// `trunc_(v)`: `(U32)__builtin_convertvector(v, I32)`, i.e. `cvttps2dq`.
    pub fn trunc_(v: F) -> U32 {
        from_si(_mm_cvttps_epi32(ps(v)))
    }

    /// A float → `I32` vector cast (`__builtin_convertvector(v, I32)`): `cvttps2dq`.
    pub fn to_i32(v: F) -> I32 {
        from_si(_mm_cvttps_epi32(ps(v)))
    }

    /// `cast(U32) -> F`: `__builtin_convertvector((I32)v, F)`, a *signed* conversion
    /// (`cvtdq2ps`).
    pub fn cast_f(v: U32) -> F {
        from_ps(_mm_cvtepi32_ps(si(v)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1737-L1741 (chrome/m156) (SKRP_CPU_SSE2)
    /// `rcp_fast`: `rcp_precise` ("we always use precise math" on Scalar and Sse2).
    pub fn rcp_fast(v: F) -> F {
        rcp_precise(v)
    }

    /// `rsqrt`: `rcp_precise(sqrt_(v))`.
    pub fn rsqrt(v: F) -> F {
        rcp_precise(sqrt_(v))
    }
}

soft_half!();

#[cfg(test)]
lane_harness!();

/// The lowp primitives (`namespace lowp`), 8 lanes.
pub mod lowp {
    use crate::vx::{Vec, join};

    pub use super::super::portable::lowp::{
        div255, div255_accurate, if_then_else_f, if_then_else_i, if_then_else_u16,
        if_then_else_u32, max_f, max_i, max_u16, min_f, min_i, min_u16, scaled_mult,
    };

    // Port of: src/opts/SkRasterPipeline_opts.h#L5484-L5499 (chrome/m156)
    /// Lowp stride.
    pub const N: usize = 8;
    /// `uint8_t` lanes.
    pub type U8 = Vec<8, u8>;
    /// `uint16_t` lanes.
    pub type U16 = Vec<8, u16>;
    /// `int16_t` lanes.
    pub type I16 = Vec<8, i16>;
    /// `int32_t` lanes.
    pub type I32 = Vec<8, i32>;
    /// `uint32_t` lanes.
    pub type U32 = Vec<8, u32>;
    /// `int64_t` lanes.
    pub type I64 = Vec<8, i64>;
    /// `uint64_t` lanes.
    pub type U64 = Vec<8, u64>;
    /// `float` lanes.
    pub type F = Vec<8, f32>;

    si! {
        // Port of: src/opts/SkRasterPipeline_opts.h#L5847-L5859 (chrome/m156) (SSE2 branch)
        /// `max_intr(F, F)`: `maxps` on each 128-bit half.
        pub fn max_intr_f(x: F, y: F) -> F {
            join(super::max_f(x.lo(), y.lo()), super::max_f(x.hi(), y.hi()))
        }

        /// `min_intr(F, F)`: `minps` on each 128-bit half.
        pub fn min_intr_f(x: F, y: F) -> F {
            join(super::min_f(x.lo(), y.lo()), super::min_f(x.hi(), y.hi()))
        }

        /// `max_intr(I32, I32)`: the compare-select `max` on SSE2.
        pub fn max_intr_i(x: I32, y: I32) -> I32 {
            max_i(x, y)
        }

        /// `min_intr(I32, I32)`.
        pub fn min_intr_i(x: I32, y: I32) -> I32 {
            min_i(x, y)
        }

        /// `max_intr(U16, U16)`.
        pub fn max_intr_u16(x: U16, y: U16) -> U16 {
            max_u16(x, y)
        }

        /// `min_intr(U16, U16)`.
        pub fn min_intr_u16(x: U16, y: U16) -> U16 {
            min_u16(x, y)
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L5920-L5937 (chrome/m156)
        /// `mad(f, m, a)`: `a + f*m`, unfused.
        pub fn mad(f: F, m: F, a: F) -> F {
            a + f * m
        }

        /// `nmad(f, m, a)`: `a - f*m`, unfused.
        pub fn nmad(f: F, m: F, a: F) -> F {
            a - f * m
        }

        /// `trunc_(F) -> U32`: `(U32)cast<I32>(x)`, `cvttps2dq` per half.
        pub fn trunc_(x: F) -> U32 {
            join(super::trunc_(x.lo()), super::trunc_(x.hi()))
        }

        /// `cast<I32>(F)`: `cvttps2dq` per half.
        pub fn to_i32(x: F) -> I32 {
            join(super::to_i32(x.lo()), super::to_i32(x.hi()))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L5939-L5966 (chrome/m156)
        /// `rcp_precise`: the highp `rcp_precise` on each half.
        pub fn rcp_precise(x: F) -> F {
            join(super::rcp_precise(x.lo()), super::rcp_precise(x.hi()))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L5967-L6006 (chrome/m156)
        /// `sqrt_`: `sqrtps` on each half.
        pub fn sqrt_(x: F) -> F {
            join(super::sqrt_(x.lo()), super::sqrt_(x.hi()))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L6008-L6035 (chrome/m156) (generic branch)
        /// `floor_`: `cast<F>(cast<I32>(x))` corrected by one, as highp `floor_` on Sse2.
        pub fn floor_(x: F) -> F {
            let roundtrip: F = to_i32(x).cast();
            roundtrip - if_then_else_f(roundtrip.gt_mask(x), F::splat(1.0), F::splat(0.0))
        }
    }

    #[cfg(test)]
    lowp_harness!();
}
