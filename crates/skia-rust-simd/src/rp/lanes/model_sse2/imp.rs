// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h (SKRP_CPU_SSE2), as a per-lane model

// The body of `model_sse2::<estimates>`; `super::EST` is the instantiation's estimate source.

use super::EST;
use crate::rp::lanes::x86_model as m;
use crate::vx::{Vec, map2, map3};

pub use crate::rp::lanes::portable::{abs_i, cond_to_mask, max_i, max_u, min_i, min_u};

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

/// The lane bits as an `f32` (for bitwise selects).
fn bits_f(x: u32) -> f32 {
    f32::from_bits(x)
}

si! {
    /// `if_then_else(I32 c, F t, F e)`: bitwise (`andps`/`andnps`/`orps`).
    pub fn if_then_else_f(c: I32, t: F, e: F) -> F {
        map3(
            |c: i32, t: f32, e: f32| {
                let c = c.cast_unsigned();
                bits_f((c & t.to_bits()) | (!c & e.to_bits()))
            },
            c,
            t,
            e,
        )
    }

    /// `if_then_else(I32 c, I32 t, I32 e)`: bitwise.
    pub fn if_then_else_i(c: I32, t: I32, e: I32) -> I32 {
        map3(|c: i32, t: i32, e: i32| (c & t) | (!c & e), c, t, e)
    }

    /// `min(F, F)`: `minps`.
    pub fn min_f(a: F, b: F) -> F {
        map2(m::minps, a, b)
    }

    /// `max(F, F)`: `maxps`.
    pub fn max_f(a: F, b: F) -> F {
        map2(m::maxps, a, b)
    }

    /// `mad(f, m, a)`: `addps(a, mulps(f, m))`.
    pub fn mad(f: F, mm: F, a: F) -> F {
        map3(|f, mm, a| m::add(a, m::mul(f, mm)), f, mm, a)
    }

    /// `nmad(f, m, a)`: `subps(a, mulps(f, m))`.
    pub fn nmad(f: F, mm: F, a: F) -> F {
        map3(|f, mm, a| m::sub(a, m::mul(f, mm)), f, mm, a)
    }

    /// `abs_(F)`: `andps(v, subps(0, v))`.
    pub fn abs_f(v: F) -> F {
        v.map(|v| m::andps(v, m::sub(0.0, v)))
    }

    /// `rcp_approx`: `rcpps` from the instantiation's estimates.
    pub fn rcp_approx(v: F) -> F {
        Vec(m::rcpps(EST, v.0))
    }

    /// `rcp_precise`: `mulps(e, subps(2, mulps(v, e)))`, `e = rcpps(v)`.
    pub fn rcp_precise(v: F) -> F {
        let e = rcp_approx(v);
        map2(|v, e| m::mul(e, m::sub(2.0, m::mul(v, e))), v, e)
    }

    /// `rsqrt_approx`: `rsqrtps` from the instantiation's estimates.
    pub fn rsqrt_approx(v: F) -> F {
        Vec(m::rsqrtps(EST, v.0))
    }

    /// `sqrt_`: `sqrtps`.
    pub fn sqrt_(v: F) -> F {
        v.map(m::sqrtps)
    }

    /// `div_fn(I32*, I32*)`: `cvttpd2dq(d / s)` in `f64`.
    pub fn div_i32(d: I32, s: I32) -> I32 {
        map2(|d, s| m::cvttpd2dq(f64::from(d) / f64::from(s)), d, s)
    }

    /// `div_fn(U32*, U32*)`: both operands clamped to `INT_MAX`, then as `div_i32`.
    pub fn div_u32(d: U32, s: U32) -> U32 {
        let clamp = |x: u32| f64::from(x.min(0x7FFF_FFFF));
        map2(|d, s| m::cvttpd2dq(clamp(d) / clamp(s)).cast_unsigned(), d, s)
    }

    /// `iround`: `cvtps2dq`.
    pub fn iround(v: F) -> I32 {
        v.map(m::cvtps2dq)
    }

    /// `round`: `cvtps2dq`, as `U32`.
    pub fn round(v: F) -> U32 {
        v.map(|v| m::cvtps2dq(v).cast_unsigned())
    }

    /// `pack(U32) -> U16`: sign-extend the low 16 bits, `packssdw`.
    pub fn pack_u32(v: U32) -> U16 {
        v.map(|v| m::packssdw((v.cast_signed() << 16) >> 16).cast_unsigned())
    }

    /// `pack(U16) -> U8`: `packuswb`.
    pub fn pack_u16(v: U16) -> U8 {
        v.map(|v| m::packuswb(v.cast_signed()))
    }

    /// `any(c)`: `movmskps != 0`.
    pub fn any(c: I32) -> bool {
        m::movmskps(c.0) != 0b0000
    }

    /// `all(c)`: `movmskps == 0b1111`.
    pub fn all(c: I32) -> bool {
        m::movmskps(c.0) == 0b1111
    }

    /// `floor_`: `roundtrip - (roundtrip > v ? 1 : 0)`, `roundtrip = cvtdq2ps(cvttps2dq(v))`.
    pub fn floor_(v: F) -> F {
        v.map(|v| {
            let roundtrip = m::cvtdq2ps(m::cvttps2dq(v));
            m::sub(roundtrip, if roundtrip > v { 1.0 } else { 0.0 })
        })
    }

    /// `ceil_`: `roundtrip + (roundtrip < v ? 1 : 0)`.
    pub fn ceil_(v: F) -> F {
        v.map(|v| {
            let roundtrip = m::cvtdq2ps(m::cvttps2dq(v));
            m::add(roundtrip, if roundtrip < v { 1.0 } else { 0.0 })
        })
    }

    /// `trunc_`: `cvttps2dq`, as `U32`.
    pub fn trunc_(v: F) -> U32 {
        v.map(|v| m::cvttps2dq(v).cast_unsigned())
    }

    /// A float → `I32` vector cast: `cvttps2dq`.
    pub fn to_i32(v: F) -> I32 {
        v.map(m::cvttps2dq)
    }

    /// `cast(U32) -> F`: `cvtdq2ps` of the bits as `I32`.
    pub fn cast_f(v: U32) -> F {
        v.map(|v| m::cvtdq2ps(v.cast_signed()))
    }

    /// `rcp_fast`: `rcp_precise`.
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

/// The lowp primitives, 8 lanes.
pub mod lowp {
    use crate::rp::lanes::x86_model as m;
    use crate::vx::{Vec, join, map2, map3};

    pub use crate::rp::lanes::portable::lowp::{
        div255, div255_accurate, if_then_else_f, if_then_else_i, if_then_else_u16,
        if_then_else_u32, max_f, max_i, max_u16, min_f, min_i, min_u16,
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
        /// `max_intr(F, F)`: `maxps`.
        pub fn max_intr_f(x: F, y: F) -> F {
            map2(m::maxps, x, y)
        }

        /// `min_intr(F, F)`: `minps`.
        pub fn min_intr_f(x: F, y: F) -> F {
            map2(m::minps, x, y)
        }

        /// `max_intr(I32, I32)`.
        pub fn max_intr_i(x: I32, y: I32) -> I32 {
            map2(i32::max, x, y)
        }

        /// `min_intr(I32, I32)`.
        pub fn min_intr_i(x: I32, y: I32) -> I32 {
            map2(i32::min, x, y)
        }

        /// `max_intr(U16, U16)`.
        pub fn max_intr_u16(x: U16, y: U16) -> U16 {
            map2(u16::max, x, y)
        }

        /// `min_intr(U16, U16)`.
        pub fn min_intr_u16(x: U16, y: U16) -> U16 {
            map2(u16::min, x, y)
        }

        /// `mad(f, m, a)`: `addps(a, mulps(f, m))`.
        pub fn mad(f: F, mm: F, a: F) -> F {
            map3(|f, mm, a| m::add(a, m::mul(f, mm)), f, mm, a)
        }

        /// `nmad(f, m, a)`: `subps(a, mulps(f, m))`.
        pub fn nmad(f: F, mm: F, a: F) -> F {
            map3(|f, mm, a| m::sub(a, m::mul(f, mm)), f, mm, a)
        }

        /// `trunc_`: `cvttps2dq`, as `U32`.
        pub fn trunc_(x: F) -> U32 {
            x.map(|v| m::cvttps2dq(v).cast_unsigned())
        }

        /// `cast<I32>(F)`: `cvttps2dq`.
        pub fn to_i32(x: F) -> I32 {
            x.map(m::cvttps2dq)
        }

        /// `rcp_precise`: the highp `rcp_precise` on each half.
        pub fn rcp_precise(x: F) -> F {
            join(super::rcp_precise(x.lo()), super::rcp_precise(x.hi()))
        }

        /// `sqrt_`: `sqrtps`.
        pub fn sqrt_(x: F) -> F {
            x.map(m::sqrtps)
        }

        /// `floor_`: the `cvttps2dq` round trip corrected by one.
        pub fn floor_(x: F) -> F {
            x.map(|v| {
                let roundtrip = m::cvtdq2ps(m::cvttps2dq(v));
                m::sub(roundtrip, if roundtrip > v { 1.0 } else { 0.0 })
            })
        }

        /// `scaled_mult`: `pmulhrsw` (the generic formula on Sse2 has the same results).
        pub fn scaled_mult(a: I16, b: I16) -> I16 {
            map2(m::pmulhrsw, a, b)
        }
    }

    #[cfg(test)]
    lowp_harness!();
}
