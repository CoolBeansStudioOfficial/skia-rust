// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h (SKRP_CPU_ML4), as a per-lane model

// The body of `model_ml4::<estimates>`; `super::EST` is the instantiation's estimate source.

use super::EST;
use crate::rp::lanes::x86_model as m;
use crate::vx::{Vec, map2, map3};

pub use crate::rp::lanes::portable::{abs_i, cond_to_mask, max_i, max_u, min_i, min_u};

/// Every function of the model: `#[inline]`, `#[must_use]`, no target features.
macro_rules! si {
    ($($item:item)*) => { $( #[inline] #[must_use] $item )* };
}

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
    /// `mad(f, m, a)`: `vfmadd` (one rounding).
    pub fn mad(f: F, mm: F, a: F) -> F {
        map3(m::fmadd, f, mm, a)
    }

    /// `nmad(f, m, a)`: `vfnmadd`, `-(f*m) + a` (one rounding).
    pub fn nmad(f: F, mm: F, a: F) -> F {
        map3(m::fnmadd, f, mm, a)
    }

    /// `min(F, F)`: `vminps`.
    pub fn min_f(a: F, b: F) -> F {
        map2(m::minps, a, b)
    }

    /// `max(F, F)`: `vmaxps`.
    pub fn max_f(a: F, b: F) -> F {
        map2(m::maxps, a, b)
    }

    /// `abs_(F)`: `vandps(v, vsubps(0, v))`.
    pub fn abs_f(v: F) -> F {
        v.map(|v| m::andps(v, m::sub(0.0, v)))
    }

    /// `floor_`: `vrndscaleps` (floor).
    pub fn floor_(v: F) -> F {
        v.map(m::roundps_floor)
    }

    /// `ceil_`: `vrndscaleps` (ceil).
    pub fn ceil_(v: F) -> F {
        v.map(m::roundps_ceil)
    }

    /// `rcp_approx`: `vrcp14ps` from the instantiation's estimates.
    pub fn rcp_approx(v: F) -> F {
        Vec(m::rcp14ps(EST, v.0))
    }

    /// `rsqrt_approx`: `vrsqrt14ps` from the instantiation's estimates.
    pub fn rsqrt_approx(v: F) -> F {
        Vec(m::rsqrt14ps(EST, v.0))
    }

    /// `sqrt_`: `vsqrtps`.
    pub fn sqrt_(v: F) -> F {
        v.map(m::sqrtps)
    }

    /// `rcp_precise`: `vmulps(vfnmadd(v, e, 2), e)`, `e = vrcp14ps(v)`.
    pub fn rcp_precise(v: F) -> F {
        let e = rcp_approx(v);
        map2(|v, e| m::mul(m::fnmadd(v, e, 2.0), e), v, e)
    }

    /// `iround`: `vcvtps2dq`.
    pub fn iround(v: F) -> I32 {
        v.map(m::cvtps2dq)
    }

    /// `round`: `vcvtps2dq`, as `U32`.
    pub fn round(v: F) -> U32 {
        v.map(|v| m::cvtps2dq(v).cast_unsigned())
    }

    /// `pack(U32) -> U16`: `vpackusdw` (lane order restored by `vpermq`).
    pub fn pack_u32(v: U32) -> U16 {
        v.map(|v| m::packusdw(v.cast_signed()))
    }

    /// `pack(U16) -> U8`: `vpackuswb` (lane order restored by `vpermq`).
    pub fn pack_u16(v: U16) -> U8 {
        v.map(|v| m::packuswb(v.cast_signed()))
    }

    /// `if_then_else(I32 c, F t, F e)`: the sign bit of `c` selects (`vptestmd` + blend).
    pub fn if_then_else_f(c: I32, t: F, e: F) -> F {
        map3(|c: i32, t: f32, e: f32| if c < 0 { t } else { e }, c, t, e)
    }

    /// `if_then_else(I32 c, I32 t, I32 e)`: the sign bit of `c` selects.
    pub fn if_then_else_i(c: I32, t: I32, e: I32) -> I32 {
        map3(|c: i32, t: i32, e: i32| if c < 0 { t } else { e }, c, t, e)
    }

    /// `any(c)`: `vptestmd(c, c) != 0`: some lane nonzero.
    pub fn any(c: I32) -> bool {
        c.0.iter().any(|&x| x != 0)
    }

    /// `all(c)`: `vptestmd(c, c) == 0xffff`: every lane nonzero.
    pub fn all(c: I32) -> bool {
        c.0.iter().all(|&x| x != 0)
    }

    /// `div_fn(I32*, I32*)`: `vcvttpd2dq(d / s)` in `f64`.
    pub fn div_i32(d: I32, s: I32) -> I32 {
        map2(|d, s| m::cvttpd2dq(f64::from(d) / f64::from(s)), d, s)
    }

    /// `div_fn(U32*, U32*)`: `vcvttpd2udq(d / s)` in `f64` with unsigned conversions (exact;
    /// `x / 0` gives `0xFFFFFFFF`).
    pub fn div_u32(d: U32, s: U32) -> U32 {
        map2(|d, s| m::cvttpd2udq(f64::from(d) / f64::from(s)), d, s)
    }

    /// `trunc_`: `vcvttps2dq`, as `U32`.
    pub fn trunc_(v: F) -> U32 {
        v.map(|v| m::cvttps2dq(v).cast_unsigned())
    }

    /// A float → `I32` vector cast: `vcvttps2dq`.
    pub fn to_i32(v: F) -> I32 {
        v.map(m::cvttps2dq)
    }

    /// `cast(U32) -> F`: `vcvtdq2ps` of the bits as `I32`.
    pub fn cast_f(v: U32) -> F {
        v.map(|v| m::cvtdq2ps(v.cast_signed()))
    }

    /// `from_half`: `vcvtph2ps`.
    pub fn from_half(h: U16) -> F {
        h.map(m::cvtph2ps)
    }

    /// `to_half`: `vcvtps2ph` (round to nearest even).
    pub fn to_half(f: F) -> U16 {
        f.map(m::cvtps2ph)
    }

    /// `rcp_fast`: the raw `vrcp14ps` estimate.
    pub fn rcp_fast(v: F) -> F {
        rcp_approx(v)
    }

    /// `rsqrt`: the raw `vrsqrt14ps` estimate.
    pub fn rsqrt(v: F) -> F {
        rsqrt_approx(v)
    }
}

#[cfg(test)]
lane_harness!();

/// The lowp primitives, 16 lanes.
pub mod lowp {
    use crate::rp::lanes::x86_model as m;
    use crate::vx::{Vec, map2, map3};

    pub use crate::rp::lanes::portable::lowp::{
        div255, div255_accurate, if_then_else_f, if_then_else_i, if_then_else_u16,
        if_then_else_u32, max_f, max_i, max_u16, min_f, min_i, min_u16,
    };

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
        /// `max_intr(F, F)`: `vmaxps`.
        pub fn max_intr_f(x: F, y: F) -> F {
            map2(m::maxps, x, y)
        }

        /// `min_intr(F, F)`: `vminps`.
        pub fn min_intr_f(x: F, y: F) -> F {
            map2(m::minps, x, y)
        }

        /// `max_intr(I32, I32)`: `vpmaxsd`.
        pub fn max_intr_i(x: I32, y: I32) -> I32 {
            map2(i32::max, x, y)
        }

        /// `min_intr(I32, I32)`: `vpminsd`.
        pub fn min_intr_i(x: I32, y: I32) -> I32 {
            map2(i32::min, x, y)
        }

        /// `max_intr(U16, U16)`: `vpmaxuw`.
        pub fn max_intr_u16(x: U16, y: U16) -> U16 {
            map2(u16::max, x, y)
        }

        /// `min_intr(U16, U16)`: `vpminuw`.
        pub fn min_intr_u16(x: U16, y: U16) -> U16 {
            map2(u16::min, x, y)
        }

        /// `mad(f, m, a)`: `vaddps(a, vmulps(f, m))` (unfused).
        pub fn mad(f: F, mm: F, a: F) -> F {
            map3(|f, mm, a| m::add(a, m::mul(f, mm)), f, mm, a)
        }

        /// `nmad(f, m, a)`: `vsubps(a, vmulps(f, m))` (unfused).
        pub fn nmad(f: F, mm: F, a: F) -> F {
            map3(|f, mm, a| m::sub(a, m::mul(f, mm)), f, mm, a)
        }

        /// `trunc_`: `vcvttps2dq`, as `U32`.
        pub fn trunc_(x: F) -> U32 {
            x.map(|v| m::cvttps2dq(v).cast_unsigned())
        }

        /// `cast<I32>(F)`: `vcvttps2dq`.
        pub fn to_i32(x: F) -> I32 {
            x.map(m::cvttps2dq)
        }

        /// `rcp_precise`: `vrcp14ps` and one fused Newton–Raphson step (the highp formula).
        pub fn rcp_precise(x: F) -> F {
            super::rcp_precise(x)
        }

        /// `sqrt_`: `vsqrtps`.
        pub fn sqrt_(x: F) -> F {
            x.map(m::sqrtps)
        }

        /// `floor_`: `vrndscaleps` (floor).
        pub fn floor_(x: F) -> F {
            x.map(m::roundps_floor)
        }

        /// `scaled_mult`: `vpmulhrsw`.
        pub fn scaled_mult(a: I16, b: I16) -> I16 {
            map2(m::pmulhrsw, a, b)
        }
    }

    #[cfg(test)]
    lowp_harness!();
}
