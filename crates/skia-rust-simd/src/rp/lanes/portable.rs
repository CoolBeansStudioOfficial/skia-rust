// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Lane functions whose results are identical on every SIMD tier (and in every model), written
//! once over [`Vec`] and re-exported by the tier modules.
//!
//! They are `#[inline(always)]` without target features, so they inline into any tier's
//! `#[target_feature]` stage functions. None of them does float arithmetic (whose NaN payloads
//! a model must control), only compares, selects and integer arithmetic.

use crate::vx::{IntLane, Lane, Vec, if_then_else};

// Port of: src/opts/SkRasterPipeline_opts.h#L957-L964 (chrome/m156)
// (SSE2's compare-select; SSE4.1 uses pminsd/pmaxsd/pminud/pmaxud with the same results.)
/// `min(I32, I32)`.
#[inline(always)]
#[must_use]
pub fn min_i<const N: usize>(a: Vec<N, i32>, b: Vec<N, i32>) -> Vec<N, i32> {
    if_then_else(a.lt_mask(b), a, b)
}

/// `max(I32, I32)`.
#[inline(always)]
#[must_use]
pub fn max_i<const N: usize>(a: Vec<N, i32>, b: Vec<N, i32>) -> Vec<N, i32> {
    if_then_else(a.gt_mask(b), a, b)
}

/// `min(U32, U32)` (unsigned compare).
#[inline(always)]
#[must_use]
pub fn min_u<const N: usize>(a: Vec<N, u32>, b: Vec<N, u32>) -> Vec<N, u32> {
    if_then_else(a.lt_mask(b), a, b)
}

/// `max(U32, U32)` (unsigned compare).
#[inline(always)]
#[must_use]
pub fn max_u<const N: usize>(a: Vec<N, u32>, b: Vec<N, u32>) -> Vec<N, u32> {
    if_then_else(a.gt_mask(b), a, b)
}

// Port of: src/opts/SkRasterPipeline_opts.h#L971-L975 (chrome/m156)
/// `abs_(I32)`: `max(v, -v)` (`INT_MIN` stays `INT_MIN`, like `pabsd`).
#[inline(always)]
#[must_use]
pub fn abs_i<const N: usize>(v: Vec<N, i32>) -> Vec<N, i32> {
    max_i(v, -v)
}

// Port of: src/opts/SkRasterPipeline_opts.h#L2277-L2286 (chrome/m156)
/// `cond_to_mask`: SIMD comparisons already produce masks.
#[inline(always)]
#[must_use]
pub fn cond_to_mask<const N: usize>(cond: Vec<N, i32>) -> Vec<N, i32> {
    cond
}

// Port of: src/opts/SkRasterPipeline_opts.h#L4950-L4961 (chrome/m156) (generic div_fn)
/// `SkSL` `int` division, the generic `div_fn(I32*, I32*)` (tiers without an optimized one:
/// Neon): `x / 0` divides by `-1`, `INT_MIN / -1` by `-2`.
#[inline(always)]
#[must_use]
pub fn div_i32<const N: usize>(d: Vec<N, i32>, s: Vec<N, i32>) -> Vec<N, i32> {
    let mut divisor = s;
    // Integer division crashes when we divide by 0, but we can divide by -1 to not crash (the
    // result will be non-sensical). The mask will be 0xFFFFFFF if true, which happens to be -1.
    divisor |= cond_to_mask(divisor.eq_mask(0));
    // Dividing by -1 works for all numerators *except* INT_MIN, so we can add -1 once more if
    // we are in that case.
    divisor += cond_to_mask(divisor.eq_mask(-1) & d.eq_mask(i32::MIN));
    d / divisor
}

// Port of: src/opts/SkRasterPipeline_opts.h#L4963-L4970 (chrome/m156) (generic div_fn)
/// `SkSL` `uint` division, the generic `div_fn(U32*, U32*)`: `x / 0` divides by `0xFFFFFFFF`.
#[inline(always)]
#[must_use]
pub fn div_u32<const N: usize>(d: Vec<N, u32>, s: Vec<N, u32>) -> Vec<N, u32> {
    let mut divisor = s;
    // Integer division crashes when we divide by 0, but we can divide by something else to not
    // crash (the result will be non-sensical). The mask will be 0xFFFFFFF if true.
    let is_zero: Vec<N, i32> = divisor.eq_mask(0).bit_cast();
    divisor |= cond_to_mask(is_zero).bit_cast::<N, u32>();
    d / divisor
}

/// The lowp functions shared by every x86 tier (and, except `div255*`, by Neon).
pub mod lowp {
    use super::{IntLane, Lane, Vec, if_then_else};

    // Port of: src/opts/SkRasterPipeline_opts.h#L5701-L5714 (chrome/m156) (non-NEON branch)
    /// `div255(v)` on x86: `(v + 255) / 256` (wrapping 16-bit), never wrong by more than 1 for
    /// products of two bytes.
    #[inline(always)]
    #[must_use]
    pub fn div255<const N: usize>(v: Vec<N, u16>) -> Vec<N, u16> {
        (v + 255) / 256
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5716-L5728 (chrome/m156) (non-NEON branch)
    /// `div255_accurate(v)` on x86: `v += 128; (v + v/256) / 256` (wrapping 16-bit).
    #[inline(always)]
    #[must_use]
    pub fn div255_accurate<const N: usize>(v: Vec<N, u16>) -> Vec<N, u16> {
        let v = v + 128;
        (v + v / 256) / 256
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5756-L5769 (chrome/m156)
    /// `if_then_else(I32 c, F t, F e)`: `(t & c) | (e & ~c)` on the bits.
    #[inline(always)]
    #[must_use]
    pub fn if_then_else_f<const N: usize>(
        c: Vec<N, i32>,
        t: Vec<N, f32>,
        e: Vec<N, f32>,
    ) -> Vec<N, f32> {
        let (t, e): (Vec<N, i32>, Vec<N, i32>) = (t.bit_cast(), e.bit_cast());
        ((t & c) | (e & !c)).bit_cast()
    }

    /// `if_then_else(I32 c, I32 t, I32 e)`.
    #[inline(always)]
    #[must_use]
    pub fn if_then_else_i<const N: usize>(
        c: Vec<N, i32>,
        t: Vec<N, i32>,
        e: Vec<N, i32>,
    ) -> Vec<N, i32> {
        (t & c) | (e & !c)
    }

    /// `if_then_else(I16 c, U16 t, U16 e)`.
    #[inline(always)]
    #[must_use]
    pub fn if_then_else_u16<const N: usize>(
        c: Vec<N, i16>,
        t: Vec<N, u16>,
        e: Vec<N, u16>,
    ) -> Vec<N, u16> {
        let c: Vec<N, u16> = c.bit_cast();
        (t & c) | (e & !c)
    }

    /// `if_then_else(I32 c, U32 t, U32 e)`.
    #[inline(always)]
    #[must_use]
    pub fn if_then_else_u32<const N: usize>(
        c: Vec<N, i32>,
        t: Vec<N, u32>,
        e: Vec<N, u32>,
    ) -> Vec<N, u32> {
        let c: Vec<N, u32> = c.bit_cast();
        (t & c) | (e & !c)
    }

    /// Compare-select `max`: `if_then_else(x < y, y, x)`.
    #[inline(always)]
    fn select_max<const N: usize, T: Lane>(x: Vec<N, T>, y: Vec<N, T>) -> Vec<N, T>
    where
        T::Mask: IntLane,
    {
        if_then_else(x.lt_mask(y), y, x)
    }

    /// Compare-select `min`: `if_then_else(x < y, x, y)`.
    #[inline(always)]
    fn select_min<const N: usize, T: Lane>(x: Vec<N, T>, y: Vec<N, T>) -> Vec<N, T>
    where
        T::Mask: IntLane,
    {
        if_then_else(x.lt_mask(y), x, y)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5771-L5778 (chrome/m156)
    /// lowp `max(F, F)`: `if_then_else(x < y, y, x)` (NaN `x` stays, NaN `y` gives `x`; `-0`
    /// vs `+0` gives `x`). Not `maxps`: see `max_intr_f`.
    #[inline(always)]
    #[must_use]
    pub fn max_f<const N: usize>(x: Vec<N, f32>, y: Vec<N, f32>) -> Vec<N, f32> {
        select_max(x, y)
    }

    /// lowp `min(F, F)`: `if_then_else(x < y, x, y)`.
    #[inline(always)]
    #[must_use]
    pub fn min_f<const N: usize>(x: Vec<N, f32>, y: Vec<N, f32>) -> Vec<N, f32> {
        select_min(x, y)
    }

    /// lowp `max(I32, I32)`.
    #[inline(always)]
    #[must_use]
    pub fn max_i<const N: usize>(x: Vec<N, i32>, y: Vec<N, i32>) -> Vec<N, i32> {
        select_max(x, y)
    }

    /// lowp `min(I32, I32)`.
    #[inline(always)]
    #[must_use]
    pub fn min_i<const N: usize>(x: Vec<N, i32>, y: Vec<N, i32>) -> Vec<N, i32> {
        select_min(x, y)
    }

    /// lowp `max(U16, U16)`.
    #[inline(always)]
    #[must_use]
    pub fn max_u16<const N: usize>(x: Vec<N, u16>, y: Vec<N, u16>) -> Vec<N, u16> {
        select_max(x, y)
    }

    /// lowp `min(U16, U16)`.
    #[inline(always)]
    #[must_use]
    pub fn min_u16<const N: usize>(x: Vec<N, u16>, y: Vec<N, u16>) -> Vec<N, u16> {
        select_min(x, y)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6042-L6063 (chrome/m156) (generic branch)
    /// `scaled_mult(a, b)`: the Q15 product `(a*b + 2^14) >> 15`, wrapped to 16 bits (so
    /// `-32768 * -32768` is `-32768`, exactly like `pmulhrsw`).
    #[inline(always)]
    #[must_use]
    pub fn scaled_mult<const N: usize>(a: Vec<N, i16>, b: Vec<N, i16>) -> Vec<N, i16> {
        let rounding_term = Vec::<N, i32>::splat(1 << 14);
        ((a.cast::<i32>() * b.cast::<i32>() + rounding_term) >> 15).cast::<i16>()
    }
}
