// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h (SKRP_CPU_SSE41 / SKRP_CPU_AVX)

//! The `Sse41` tier (`SKRP_CPU_SSE41`, also what an AVX baseline compiles): Skia shares the
//! `Sse2` block and switches a few primitives to SSE4.1 instructions, and `rcp_fast`/`rsqrt` to
//! the raw estimates.
//!
//! Every function is a safe `#[target_feature(enable = "sse2,ssse3,sse4.1")]` function
//! (`Sse41Token::FEATURES`); the re-exported `Sse2` functions need a subset of those features,
//! so stage code stamped into this tier calls them safely.

use core::arch::x86_64::{
    _mm_ceil_ps, _mm_floor_ps, _mm_min_epu32, _mm_packus_epi32, _mm_set1_epi32,
};

use super::x86::{from_ps, from_si, ps, si};
use crate::vx::Vec;

pub use super::portable::{abs_i, cond_to_mask, max_i, max_u, min_i, min_u};
pub use super::sse2::{
    F, I32, LOWP_N, N, U8, U16, U32, U64, abs_f, all, any, cast_f, if_then_else_f, if_then_else_i,
    iround, mad, max_f, min_f, nmad, pack_u16, rcp_approx, rcp_precise, round, rsqrt_approx, sqrt_,
    to_i32, trunc_,
};

/// Every function of the tier: Skia's `SI` with the tier's target features.
macro_rules! si {
    ($($(#[$m:meta])* $v:vis fn $name:ident($($args:tt)*) -> $ret:ty $body:block)*) => {$(
        $(#[$m])*
        ///
        /// # Safety
        /// The CPU must support SSE2, SSSE3 and SSE4.1 (`Sse41Token`). Code compiled with the
        /// `Sse41` tier's features, such as the tier's stage functions, calls this safely.
        #[target_feature(enable = "sse2,ssse3,sse4.1")]
        #[inline]
        #[must_use]
        $v fn $name($($args)*) -> $ret $body
    )*};
}

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L1006-L1040 (chrome/m156) (SSE4.1 clamp)
    /// `SkSL` `uint` division: operands clamped to `INT_MAX` with `pminud`, then the `f64`
    /// division (same results as `Sse2`).
    pub fn div_u32(d: U32, s: U32) -> U32 {
        let max_safe = _mm_set1_epi32(0x7FFF_FFFF);
        let d = _mm_min_epu32(si(d), max_safe);
        let s = _mm_min_epu32(si(s), max_safe);
        from_si(super::sse2::div_f64(d, s))
    }

    /// `SkSL` `int` division (the `Sse2` code).
    pub fn div_i32(d: I32, s: I32) -> I32 {
        super::sse2::div_i32(d, s)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1045-L1053 (chrome/m156) (SSE4.1 branch)
    /// `pack(U32) -> U16`: `packusdw`, i.e. *saturation* of the lanes as signed 32-bit values
    /// to `[0, 65535]`.
    pub fn pack_u32(v: U32) -> U16 {
        let r = si(v);
        let p: Vec<8, u16> = from_si(_mm_packus_epi32(r, r));
        p.lo() // We have two copies.  Return (the lower) one.
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1065-L1081 (chrome/m156) (SSE4.1 branch)
    /// `floor_`: `roundps` (exact `floorf`).
    pub fn floor_(v: F) -> F {
        from_ps(_mm_floor_ps(ps(v)))
    }

    /// `ceil_`: `roundps` (exact `ceilf`).
    pub fn ceil_(v: F) -> F {
        from_ps(_mm_ceil_ps(ps(v)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1742-L1745 (chrome/m156)
    /// `rcp_fast`: the raw `rcpps` estimate.
    pub fn rcp_fast(v: F) -> F {
        rcp_approx(v)
    }

    /// `rsqrt`: the raw `rsqrtps` estimate.
    pub fn rsqrt(v: F) -> F {
        rsqrt_approx(v)
    }
}

soft_half!();

#[cfg(test)]
lane_harness!();

/// The lowp primitives (`namespace lowp`), 8 lanes.
pub mod lowp {
    use core::arch::x86_64::{
        _mm_max_epi32, _mm_max_epu16, _mm_min_epi32, _mm_min_epu16, _mm_mulhrs_epi16,
    };

    use super::super::x86::{from_si, si};
    use crate::vx::join;

    pub use super::super::portable::lowp::{
        div255, div255_accurate, if_then_else_f, if_then_else_i, if_then_else_u16,
        if_then_else_u32, max_f, max_i, max_u16, min_f, min_i, min_u16,
    };
    pub use super::super::sse2::lowp::{
        F, I16, I32, I64, N, U8, U16, U32, U64, mad, max_intr_f, min_intr_f, nmad, rcp_precise,
        sqrt_, to_i32, trunc_,
    };

    si! {
        // Port of: src/opts/SkRasterPipeline_opts.h#L5860-L5880 (chrome/m156) (SSE4.1 branch)
        /// `max_intr(I32, I32)`: `pmaxsd` on each half.
        pub fn max_intr_i(x: I32, y: I32) -> I32 {
            let (lo, hi) = (_mm_max_epi32(si(x.lo()), si(y.lo())), _mm_max_epi32(si(x.hi()), si(y.hi())));
            join(from_si::<4, i32>(lo), from_si(hi))
        }

        /// `min_intr(I32, I32)`: `pminsd` on each half.
        pub fn min_intr_i(x: I32, y: I32) -> I32 {
            let (lo, hi) = (_mm_min_epi32(si(x.lo()), si(y.lo())), _mm_min_epi32(si(x.hi()), si(y.hi())));
            join(from_si::<4, i32>(lo), from_si(hi))
        }

        /// `max_intr(U16, U16)`: `pmaxuw`.
        pub fn max_intr_u16(x: U16, y: U16) -> U16 {
            from_si(_mm_max_epu16(si(x), si(y)))
        }

        /// `min_intr(U16, U16)`: `pminuw`.
        pub fn min_intr_u16(x: U16, y: U16) -> U16 {
            from_si(_mm_min_epu16(si(x), si(y)))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L6008-L6035 (chrome/m156) (SSE4.1 branch)
        /// `floor_`: `roundps` on each half.
        pub fn floor_(x: F) -> F {
            join(super::floor_(x.lo()), super::floor_(x.hi()))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L6042-L6063 (chrome/m156) (SSE4.1 branch)
        /// `scaled_mult(a, b)`: `pmulhrsw` (`(a*b + 2^14) >> 15`, wrapping).
        pub fn scaled_mult(a: I16, b: I16) -> I16 {
            from_si(_mm_mulhrs_epi16(si(a), si(b)))
        }
    }

    #[cfg(test)]
    lowp_harness!();
}
