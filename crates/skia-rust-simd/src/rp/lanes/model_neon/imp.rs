// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h (SKRP_CPU_NEON, SK_CPU_ARM64), as a per-lane
// model

// The body of `model_neon::<estimates>`; `super::EST` is the instantiation's estimate source.

use super::EST;
use crate::rp::lanes::neon_model as m;
use crate::vx::{Vec, map2, map3};

use crate::rp::lanes::portable::lowp::{if_then_else_f as select_f, if_then_else_i as select_i};
pub use crate::rp::lanes::portable::{
    abs_i, cond_to_mask, div_i32, div_u32, max_i, max_u, min_i, min_u,
};

/// Every function of the model: `#[inline]`, `#[must_use]`, no target features.
macro_rules! si {
    ($($item:item)*) => { $( #[inline] #[must_use] $item )* };
}

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
    /// `min(F, F)`: `FMIN`.
    pub fn min_f(a: F, b: F) -> F {
        map2(m::fmin, a, b)
    }

    /// `max(F, F)`: `FMAX`.
    pub fn max_f(a: F, b: F) -> F {
        map2(m::fmax, a, b)
    }

    /// `abs_(F)`: `FABS`.
    pub fn abs_f(v: F) -> F {
        v.map(m::fabs)
    }

    /// `rcp_approx`: `FMUL(FRECPS(v, e), e)`, `e = FRECPE(v)` from the instantiation's
    /// estimates.
    pub fn rcp_approx(v: F) -> F {
        let e = Vec(m::frecpe(EST, v.0));
        map2(|v, e| m::fmul(m::frecps(v, e), e), v, e)
    }

    /// `rcp_precise`: `FMUL(FRECPS(v, e), e)`, `e = rcp_approx(v)`.
    pub fn rcp_precise(v: F) -> F {
        let e = rcp_approx(v);
        map2(|v, e| m::fmul(m::frecps(v, e), e), v, e)
    }

    /// `rsqrt_approx`: `FMUL(FRSQRTS(v, FMUL(e, e)), e)`, `e = FRSQRTE(v)`.
    pub fn rsqrt_approx(v: F) -> F {
        let e = Vec(m::frsqrte(EST, v.0));
        map2(|v, e| m::fmul(m::frsqrts(v, m::fmul(e, e)), e), v, e)
    }

    /// `pack(U32) -> U16`: truncation.
    pub fn pack_u32(v: U32) -> U16 {
        v.cast()
    }

    /// `pack(U16) -> U8`: truncation.
    pub fn pack_u16(v: U16) -> U8 {
        v.cast()
    }

    /// `if_then_else(I32 c, F t, F e)`: `BSL`, bitwise.
    pub fn if_then_else_f(c: I32, t: F, e: F) -> F {
        select_f(c, t, e)
    }

    /// `if_then_else(I32 c, I32 t, I32 e)`: `BSL`, bitwise.
    pub fn if_then_else_i(c: I32, t: I32, e: I32) -> I32 {
        select_i(c, t, e)
    }

    /// `any(c)`: `UMAXV != 0`.
    pub fn any(c: I32) -> bool {
        c.0.iter().any(|&x| x != 0)
    }

    /// `all(c)`: `UMINV != 0`.
    pub fn all(c: I32) -> bool {
        c.0.iter().all(|&x| x != 0)
    }

    /// `mad(f, m, a)`: `FMLA` (`FPMulAdd(a, f, m)`).
    pub fn mad(f: F, mm: F, a: F) -> F {
        map3(|f, mm, a| m::fmla(a, f, mm), f, mm, a)
    }

    /// `nmad(f, m, a)`: `FMLS` (`FPMulAdd(a, -f, m)`).
    pub fn nmad(f: F, mm: F, a: F) -> F {
        map3(|f, mm, a| m::fmls(a, f, mm), f, mm, a)
    }

    /// `floor_`: `FRINTM`.
    pub fn floor_(v: F) -> F {
        v.map(m::frintm)
    }

    /// `ceil_`: `FRINTP`.
    pub fn ceil_(v: F) -> F {
        v.map(m::frintp)
    }

    /// `sqrt_`: `FSQRT`.
    pub fn sqrt_(v: F) -> F {
        v.map(m::fsqrt)
    }

    /// `iround`: `FCVTNS`.
    pub fn iround(v: F) -> I32 {
        v.map(m::fcvtns)
    }

    /// `round`: `FCVTNU`.
    pub fn round(v: F) -> U32 {
        v.map(m::fcvtnu)
    }

    /// `trunc_`: `FCVTZS`, as `U32`.
    pub fn trunc_(v: F) -> U32 {
        v.map(|v| m::fcvtzs(v).cast_unsigned())
    }

    /// A float → `I32` vector cast: `FCVTZS`.
    pub fn to_i32(v: F) -> I32 {
        v.map(m::fcvtzs)
    }

    /// `cast(U32) -> F`: `SCVTF` of the bits as `I32`.
    pub fn cast_f(v: U32) -> F {
        v.map(|v| m::scvtf(v.cast_signed()))
    }

    /// `from_half`: `FCVTL` (half → single, IEEE).
    pub fn from_half(h: U16) -> F {
        h.map(m::fcvt_f32_f16)
    }

    /// `to_half`: `FCVTN` (single → half, IEEE, round to nearest even).
    pub fn to_half(f: F) -> U16 {
        f.map(m::fcvt_f16_f32)
    }

    /// `rcp_fast`: `rcp_approx`.
    pub fn rcp_fast(v: F) -> F {
        rcp_approx(v)
    }

    /// `rsqrt`: `rsqrt_approx`.
    pub fn rsqrt(v: F) -> F {
        rsqrt_approx(v)
    }
}

#[cfg(test)]
lane_harness!();

/// The lowp primitives, 8 lanes.
pub mod lowp {
    use crate::rp::lanes::neon_model as m;
    use crate::vx::{Vec, join, map2, map3};

    pub use crate::rp::lanes::portable::lowp::{
        if_then_else_f, if_then_else_i, if_then_else_u16, if_then_else_u32, max_f, max_i, max_u16,
        min_f, min_i, min_u16,
    };

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
        /// `div255`: `URSRA` + `URSHR` (exact for products of two bytes).
        pub fn div255(v: U16) -> U16 {
            v.map(m::div255)
        }

        /// `div255_accurate`: `div255`.
        pub fn div255_accurate(v: U16) -> U16 {
            div255(v)
        }

        /// `max_intr(F, F)`: compare-select.
        pub fn max_intr_f(x: F, y: F) -> F {
            max_f(x, y)
        }

        /// `min_intr(F, F)`: compare-select.
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

        /// `mad(f, m, a)`: `FADD(a, FMUL(f, m))`, unfused.
        pub fn mad(f: F, mm: F, a: F) -> F {
            map3(|f, mm, a| m::fadd(a, m::fmul(f, mm)), f, mm, a)
        }

        /// `nmad(f, m, a)`: `FSUB(a, FMUL(f, m))`, unfused.
        pub fn nmad(f: F, mm: F, a: F) -> F {
            map3(|f, mm, a| m::fsub(a, m::fmul(f, mm)), f, mm, a)
        }

        /// `trunc_`: `FCVTZS`, as `U32`.
        pub fn trunc_(x: F) -> U32 {
            x.map(|v| m::fcvtzs(v).cast_unsigned())
        }

        /// `cast<I32>(F)`: `FCVTZS`.
        pub fn to_i32(x: F) -> I32 {
            x.map(m::fcvtzs)
        }

        /// `rcp_precise`: the highp `rcp_precise` on each half.
        pub fn rcp_precise(x: F) -> F {
            join(super::rcp_precise(x.lo()), super::rcp_precise(x.hi()))
        }

        /// `sqrt_`: `FSQRT`.
        pub fn sqrt_(x: F) -> F {
            x.map(m::fsqrt)
        }

        /// `floor_`: `FRINTM`.
        pub fn floor_(x: F) -> F {
            x.map(m::frintm)
        }

        /// `scaled_mult`: `SQRDMULH` (saturating).
        pub fn scaled_mult(a: I16, b: I16) -> I16 {
            map2(m::sqrdmulh, a, b)
        }
    }

    #[cfg(test)]
    lowp_harness!();
}
