// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h (SKRP_CPU_NEON, SK_CPU_ARM64)

//! The `Neon` tier (`SKRP_CPU_NEON` on arm64): 4 highp lanes, 8 lowp lanes, Skia's A64 NEON
//! instructions (fused `mad`, `FMIN`/`FMAX`, `FRECPE`/`FRSQRTE` plus Newton–Raphson steps,
//! `FCVTNS`, hardware half conversions, exact `div255`).
//!
//! Every function is a safe `#[target_feature(enable = "neon")]` function
//! (`NeonToken::FEATURES`). See [the module overview](super) for what each primitive does on each
//! tier. Only the arm64 branches of Skia's NEON block are ported (design §1.3: arm32 is out of
//! scope).

use core::arch::aarch64::{
    vabsq_f32, vbslq_f32, vbslq_s32, vcvt_f16_f32, vcvt_f32_f16, vcvtnq_s32_f32, vcvtnq_u32_f32,
    vcvtq_f32_s32, vcvtq_s32_f32, vfmaq_f32, vfmsq_f32, vmaxq_f32, vmaxvq_u32, vminq_f32,
    vminvq_u32, vmulq_f32, vrecpeq_f32, vrecpsq_f32, vreinterpret_f16_u16, vreinterpret_u16_f16,
    vreinterpretq_s32_u32, vreinterpretq_u32_s32, vrndmq_f32, vrndpq_f32, vrsqrteq_f32,
    vrsqrtsq_f32, vsqrtq_f32,
};

use super::aarch64::{d, from_d, from_ps, from_q, ps, q};
use crate::vx::Vec;

pub use super::portable::{abs_i, cond_to_mask, div_i32, div_u32, max_i, max_u, min_i, min_u};

/// Every function of the tier: Skia's `SI` with the tier's target features.
macro_rules! si {
    ($($(#[$m:meta])* $v:vis fn $name:ident($($args:tt)*) -> $ret:ty $body:block)*) => {$(
        $(#[$m])*
        ///
        /// # Safety
        /// The CPU must support NEON (`NeonToken`). Code compiled with the `Neon` tier's
        /// features, such as the tier's stage functions, calls this safely.
        #[target_feature(enable = "neon")]
        #[inline]
        #[must_use]
        $v fn $name($($args)*) -> $ret $body
    )*};
}

// Port of: src/opts/SkRasterPipeline_opts.h#L205-L212 (chrome/m156)
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
    // Port of: src/opts/SkRasterPipeline_opts.h#L214-L220 (chrome/m156)
    // (The I32/U32 overloads, vminq_s32 etc., are the portable `min_i`/`min_u`: same results.)
    /// `min(F, F)`: `vminq_f32` (`FMIN`): a NaN operand propagates (quieted; a signalling NaN
    /// wins), `-0 < +0`.
    pub fn min_f(a: F, b: F) -> F {
        from_ps(vminq_f32(ps(a), ps(b)))
    }

    /// `max(F, F)`: `vmaxq_f32` (`FMAX`): a NaN operand propagates, `-0 < +0`.
    pub fn max_f(a: F, b: F) -> F {
        from_ps(vmaxq_f32(ps(a), ps(b)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L222-L226 (chrome/m156)
    /// `abs_(F)`: `vabsq_f32` (clears the sign bit, also of NaNs).
    pub fn abs_f(v: F) -> F {
        from_ps(vabsq_f32(ps(v)))
    }

    /// `rcp_approx`: `FRECPE` and one fused Newton–Raphson step, `vrecpsq_f32(v,e) * e`.
    pub fn rcp_approx(v: F) -> F {
        let v = ps(v);
        let e = vrecpeq_f32(v);
        from_ps(vmulq_f32(vrecpsq_f32(v, e), e))
    }

    /// `rcp_precise`: `rcp_approx` and one more step, `vrecpsq_f32(v,e) * e`.
    pub fn rcp_precise(v: F) -> F {
        let e = ps(rcp_approx(v));
        from_ps(vmulq_f32(vrecpsq_f32(ps(v), e), e))
    }

    /// `rsqrt_approx`: `FRSQRTE` and one step, `vrsqrtsq_f32(v, e*e) * e`.
    pub fn rsqrt_approx(v: F) -> F {
        let v = ps(v);
        let e = vrsqrteq_f32(v);
        from_ps(vmulq_f32(vrsqrtsq_f32(v, vmulq_f32(e, e)), e))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L228-L229 (chrome/m156)
    /// `pack(U32) -> U16`: `__builtin_convertvector`, i.e. truncation.
    pub fn pack_u32(v: U32) -> U16 {
        v.cast()
    }

    /// `pack(U16) -> U8`: truncation.
    pub fn pack_u16(v: U16) -> U8 {
        v.cast()
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L231-L232 (chrome/m156)
    /// `if_then_else(I32 c, F t, F e)`: `vbslq_f32`, bitwise.
    pub fn if_then_else_f(c: I32, t: F, e: F) -> F {
        from_ps(vbslq_f32(q(c), ps(t), ps(e)))
    }

    /// `if_then_else(I32 c, I32 t, I32 e)`: `vbslq_s32`, bitwise.
    pub fn if_then_else_i(c: I32, t: I32, e: I32) -> I32 {
        let (t, e) = (vreinterpretq_s32_u32(q(t)), vreinterpretq_s32_u32(q(e)));
        from_q(vreinterpretq_u32_s32(vbslq_s32(q(c), t, e)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L234-L236 (chrome/m156) (SK_CPU_ARM64)
    /// `any(c)`: `vmaxvq_u32(c) != 0`: some lane is nonzero (any bit).
    pub fn any(c: I32) -> bool {
        vmaxvq_u32(q(c)) != 0
    }

    /// `all(c)`: `vminvq_u32(c) != 0`: every lane is nonzero (any bit).
    pub fn all(c: I32) -> bool {
        vminvq_u32(q(c)) != 0
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L238-L244 (chrome/m156) (SK_CPU_ARM64)
    /// `mad(f, m, a)`: `vfmaq_f32(a, f, m)`, fused (`FMLA`).
    pub fn mad(f: F, m: F, a: F) -> F {
        from_ps(vfmaq_f32(ps(a), ps(f), ps(m)))
    }

    /// `nmad(f, m, a)`: `vfmsq_f32(a, f, m)`, fused `a - f*m` (`FMLS`).
    pub fn nmad(f: F, m: F, a: F) -> F {
        from_ps(vfmsq_f32(ps(a), ps(f), ps(m)))
    }

    /// `floor_`: `vrndmq_f32` (`FRINTM`, exact).
    pub fn floor_(v: F) -> F {
        from_ps(vrndmq_f32(ps(v)))
    }

    /// `ceil_`: `vrndpq_f32` (`FRINTP`, exact).
    pub fn ceil_(v: F) -> F {
        from_ps(vrndpq_f32(ps(v)))
    }

    /// `sqrt_`: `vsqrtq_f32` (exact).
    pub fn sqrt_(v: F) -> F {
        from_ps(vsqrtq_f32(ps(v)))
    }

    /// `iround`: `vcvtnq_s32_f32` (`FCVTNS`: ties to even, saturating, NaN → 0).
    pub fn iround(v: F) -> I32 {
        from_q(vreinterpretq_u32_s32(vcvtnq_s32_f32(ps(v))))
    }

    /// `round`: `vcvtnq_u32_f32` (`FCVTNU`: ties to even, saturating to `[0, u32::MAX]`,
    /// NaN → 0).
    pub fn round(v: F) -> U32 {
        from_q(vcvtnq_u32_f32(ps(v)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1588-L1600 (chrome/m156) (SIMD branch)
    /// `trunc_(v)`: `(U32)__builtin_convertvector(v, I32)`, i.e. `FCVTZS` (saturating,
    /// NaN → 0).
    pub fn trunc_(v: F) -> U32 {
        from_q(vreinterpretq_u32_s32(vcvtq_s32_f32(ps(v))))
    }

    /// A float → `I32` vector cast (`__builtin_convertvector(v, I32)`): `FCVTZS`.
    pub fn to_i32(v: F) -> I32 {
        from_q(vreinterpretq_u32_s32(vcvtq_s32_f32(ps(v))))
    }

    /// `cast(U32) -> F`: `__builtin_convertvector((I32)v, F)`, a *signed* conversion (`SCVTF`).
    pub fn cast_f(v: U32) -> F {
        from_ps(vcvtq_f32_s32(vreinterpretq_s32_u32(q(v))))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1651-L1654 (chrome/m156) (NEON && ARM64)
    /// `from_half`: `vcvt_f32_f16` (IEEE: half denormals kept, NaNs quieted).
    pub fn from_half(h: U16) -> F {
        from_ps(vcvt_f32_f16(vreinterpret_f16_u16(d(h))))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1674-L1677 (chrome/m156) (NEON && ARM64)
    /// `to_half`: `vcvt_f16_f32` (IEEE: round to nearest even, half denormals produced,
    /// overflow → infinity).
    pub fn to_half(f: F) -> U16 {
        from_d(vreinterpret_u16_f16(vcvt_f16_f32(ps(f))))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1742-L1745 (chrome/m156) (not SCALAR/SSE2)
    /// `rcp_fast`: `rcp_approx` (`FRECPE` + one step).
    pub fn rcp_fast(v: F) -> F {
        rcp_approx(v)
    }

    /// `rsqrt`: `rsqrt_approx` (`FRSQRTE` + one step).
    pub fn rsqrt(v: F) -> F {
        rsqrt_approx(v)
    }
}

#[cfg(test)]
lane_harness!();

/// The lowp primitives (`namespace lowp`), 8 lanes.
pub mod lowp {
    use core::arch::aarch64::{
        vqrdmulhq_s16, vreinterpretq_s16_u32, vreinterpretq_u16_u32, vreinterpretq_u32_s16,
        vreinterpretq_u32_u16, vrshrq_n_u16, vrsraq_n_u16,
    };

    use super::super::aarch64::{from_q, q};
    use crate::vx::{Vec, join};

    pub use super::super::portable::lowp::{
        if_then_else_f, if_then_else_i, if_then_else_u16, if_then_else_u32, max_f, max_i, max_u16,
        min_f, min_i, min_u16,
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
        // Port of: src/opts/SkRasterPipeline_opts.h#L5701-L5714 (chrome/m156) (NEON branch)
        /// `div255(v)`: `vrshrq_n_u16(vrsraq_n_u16(v, v, 8), 8)`, i.e.
        /// `(v + ((v+128)>>8) + 128) >> 8` with the accumulate wrapping at 16 bits: exact for
        /// every product of two bytes.
        pub fn div255(v: U16) -> U16 {
            let v = vreinterpretq_u16_u32(q(v));
            from_q(vreinterpretq_u32_u16(vrshrq_n_u16::<8>(vrsraq_n_u16::<8>(v, v))))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L5716-L5728 (chrome/m156) (NEON branch)
        /// `div255_accurate(v)`: `div255` ("already correct for all inputs").
        pub fn div255_accurate(v: U16) -> U16 {
            div255(v)
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L5881-L5889 (chrome/m156) (generic branch)
        /// `max_intr(F, F)`: the compare-select `max` on Neon.
        pub fn max_intr_f(x: F, y: F) -> F {
            max_f(x, y)
        }

        /// `min_intr(F, F)`: the compare-select `min`.
        pub fn min_intr_f(x: F, y: F) -> F {
            min_f(x, y)
        }

        /// `max_intr(I32, I32)`.
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

        /// `trunc_(F) -> U32`: `(U32)cast<I32>(x)`, `FCVTZS` per half.
        pub fn trunc_(x: F) -> U32 {
            join(super::trunc_(x.lo()), super::trunc_(x.hi()))
        }

        /// `cast<I32>(F)`: `FCVTZS` per half.
        pub fn to_i32(x: F) -> I32 {
            join(super::to_i32(x.lo()), super::to_i32(x.hi()))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L5939-L5966 (chrome/m156) (NEON branch)
        /// `rcp_precise`: the highp `rcp_precise` on each half.
        pub fn rcp_precise(x: F) -> F {
            join(super::rcp_precise(x.lo()), super::rcp_precise(x.hi()))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L5967-L6006 (chrome/m156) (SK_CPU_ARM64)
        /// `sqrt_`: `vsqrtq_f32` on each half.
        pub fn sqrt_(x: F) -> F {
            join(super::sqrt_(x.lo()), super::sqrt_(x.hi()))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L6008-L6035 (chrome/m156) (SK_CPU_ARM64)
        /// `floor_`: `vrndmq_f32` on each half.
        pub fn floor_(x: F) -> F {
            join(super::floor_(x.lo()), super::floor_(x.hi()))
        }

        // Port of: src/opts/SkRasterPipeline_opts.h#L6042-L6063 (chrome/m156) (SK_CPU_ARM64)
        /// `scaled_mult(a, b)`: `vqrdmulhq_s16`, `(2*a*b + 2^15) >> 16` **saturated** (so
        /// `-32768 * -32768` is `32767`, unlike x86).
        pub fn scaled_mult(a: I16, b: I16) -> I16 {
            let (a, b) = (vreinterpretq_s16_u32(q(a)), vreinterpretq_s16_u32(q(b)));
            from_q(vreinterpretq_u32_s16(vqrdmulhq_s16(a, b)))
        }
    }

    #[cfg(test)]
    lowp_harness!();
}
