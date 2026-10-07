// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h (SKRP_CPU_SSE41), as a per-lane model

// The body of `model_sse41::<estimates>`; `super::base` is `model_sse2::<estimates>`.

use crate::rp::lanes::x86_model as m;

pub use super::base::{
    F, I32, LOWP_N, N, U8, U16, U32, U64, abs_f, abs_i, all, any, cast_f, cond_to_mask, div_i32,
    div_u32, if_then_else_f, if_then_else_i, iround, mad, max_f, max_i, max_u, min_f, min_i, min_u,
    nmad, pack_u16, rcp_approx, rcp_precise, round, rsqrt_approx, sqrt_, to_i32, trunc_,
};

/// Every function of the model: `#[inline]`, `#[must_use]`, no target features.
macro_rules! si {
    ($($item:item)*) => { $( #[inline] #[must_use] $item )* };
}

si! {
    /// `pack(U32) -> U16`: `packusdw`.
    pub fn pack_u32(v: U32) -> U16 {
        v.map(|v| m::packusdw(v.cast_signed()))
    }

    /// `floor_`: `roundps` (floor).
    pub fn floor_(v: F) -> F {
        v.map(m::roundps_floor)
    }

    /// `ceil_`: `roundps` (ceil).
    pub fn ceil_(v: F) -> F {
        v.map(m::roundps_ceil)
    }

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

/// The lowp primitives, 8 lanes.
pub mod lowp {
    use crate::rp::lanes::x86_model as m;

    pub use super::super::base::lowp::{
        F, I16, I32, I64, N, U8, U16, U32, U64, div255, div255_accurate, if_then_else_f,
        if_then_else_i, if_then_else_u16, if_then_else_u32, mad, max_f, max_i, max_intr_f,
        max_intr_i, max_intr_u16, max_u16, min_f, min_i, min_intr_f, min_intr_i, min_intr_u16,
        min_u16, nmad, rcp_precise, scaled_mult, sqrt_, to_i32, trunc_,
    };

    si! {
        /// `floor_`: `roundps` (floor).
        pub fn floor_(x: F) -> F {
            x.map(m::roundps_floor)
        }
    }

    #[cfg(test)]
    lowp_harness!();
}
