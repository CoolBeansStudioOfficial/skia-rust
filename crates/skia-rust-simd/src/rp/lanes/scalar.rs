// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h (SKRP_CPU_SCALAR)

//! The `Scalar` tier (`SKRP_CPU_SCALAR`): one highp lane with C scalar semantics, no lowp.
//!
//! This is what Skia runs on wasm32 (design §1.1), so C's undefined float → int casts follow
//! wasm's `trunc_sat` (Rust's saturating `as`: NaN → 0, out of range → the nearest bound), and
//! `fminf`/`fmaxf` follow musl (Emscripten's libc). The tier uses no estimates and no target
//! features: it is its own model (design §2.8) and runs under Miri.

use super::S;

/// Every function of the tier: `#[inline]` and `#[must_use]` (no target features).
macro_rules! si {
    ($($item:item)*) => { $( #[inline] #[must_use] $item )* };
}

// Port of: src/opts/SkRasterPipeline_opts.h#L124-L131 (chrome/m156)
/// Highp stride.
pub const N: usize = 1;
/// `float`.
pub type F = S<f32>;
/// `int32_t`.
pub type I32 = S<i32>;
/// `uint64_t`.
pub type U64 = S<u64>;
/// `uint32_t`.
pub type U32 = S<u32>;
/// `uint16_t`.
pub type U16 = S<u16>;
/// `uint8_t`.
pub type U8 = S<u8>;

// skia-rust: libm. Port of musl's src/math/fminf.c (Emscripten's libc), which is what `fminf`
// is on the Scalar tier's real target (wasm32). The `x64-scalar` oracle proxy uses the MSVC CRT
// instead, which may order ±0 differently (design §4.5, R5).
/// `fminf`: a NaN operand is ignored; `-0 < +0`.
fn fminf(x: f32, y: f32) -> f32 {
    if x.is_nan() {
        return y;
    }
    if y.is_nan() {
        return x;
    }
    // handle signed zeros, see C99 Annex F.9.9.2
    if x.is_sign_negative() != y.is_sign_negative() {
        return if x.is_sign_negative() { x } else { y };
    }
    if x < y { x } else { y }
}

// skia-rust: libm. Port of musl's src/math/fmaxf.c (see `fminf`).
/// `fmaxf`: a NaN operand is ignored; `-0 < +0`.
fn fmaxf(x: f32, y: f32) -> f32 {
    if x.is_nan() {
        return y;
    }
    if y.is_nan() {
        return x;
    }
    // handle signed zeros, see C99 Annex F.9.9.2
    if x.is_sign_negative() != y.is_sign_negative() {
        return if x.is_sign_negative() { y } else { x };
    }
    if x < y { y } else { x }
}

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L133-L138 (chrome/m156)
    /// `min(F, F)`: `fminf`.
    pub fn min_f(a: F, b: F) -> F {
        S(fminf(a.0, b.0))
    }

    /// `max(F, F)`: `fmaxf`.
    pub fn max_f(a: F, b: F) -> F {
        S(fmaxf(a.0, b.0))
    }

    /// `min(I32, I32)`: `a < b ? a : b`.
    pub fn min_i(a: I32, b: I32) -> I32 {
        if a.0 < b.0 { a } else { b }
    }

    /// `min(U32, U32)`.
    pub fn min_u(a: U32, b: U32) -> U32 {
        if a.0 < b.0 { a } else { b }
    }

    /// `max(I32, I32)`: `a > b ? a : b`.
    pub fn max_i(a: I32, b: I32) -> I32 {
        if a.0 > b.0 { a } else { b }
    }

    /// `max(U32, U32)`.
    pub fn max_u(a: U32, b: U32) -> U32 {
        if a.0 > b.0 { a } else { b }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L140-L151 (chrome/m156)
    /// `mad(f, m, a)`: `a + f*m`, unfused.
    pub fn mad(f: F, m: F, a: F) -> F {
        a + f * m
    }

    /// `nmad(f, m, a)`: `a - f*m`, unfused.
    pub fn nmad(f: F, m: F, a: F) -> F {
        a - f * m
    }

    /// `abs_(F)`: `fabsf` (clears the sign bit, NaNs included).
    pub fn abs_f(v: F) -> F {
        S(v.0.abs())
    }

    /// `abs_(I32)`: `v < 0 ? -v : v` (`INT_MIN` stays `INT_MIN`).
    pub fn abs_i(v: I32) -> I32 {
        if v.0 < 0 { -v } else { v }
    }

    /// `floor_`: `floorf`.
    pub fn floor_(v: F) -> F {
        S(v.0.floor())
    }

    /// `ceil_`: `ceilf`.
    pub fn ceil_(v: F) -> F {
        S(v.0.ceil())
    }

    /// `rcp_approx`: `1.0f / v` (exact). Use `rcp_fast`.
    pub fn rcp_approx(v: F) -> F {
        S(1.0 / v.0)
    }

    /// `rsqrt_approx`: `1.0f / sqrtf(v)`. Use `rsqrt`.
    pub fn rsqrt_approx(v: F) -> F {
        S(1.0 / v.0.sqrt())
    }

    /// `sqrt_`: `sqrtf`.
    pub fn sqrt_(v: F) -> F {
        S(v.0.sqrt())
    }

    /// `rcp_precise`: `1.0f / v`.
    pub fn rcp_precise(v: F) -> F {
        S(1.0 / v.0)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L151-L152 (chrome/m156)
    /// `iround(v)`: `(I32)(v + 0.5f)`, a truncating C cast (wasm `trunc_sat`).
    pub fn iround(v: F) -> I32 {
        #[allow(clippy::cast_possible_truncation)] // mirrors the (I32) cast; saturating as wasm
        let r = (v.0 + 0.5) as i32;
        S(r)
    }

    /// `round(v)`: `(U32)(v + 0.5f)` (negative values give 0, as wasm `trunc_sat`).
    pub fn round(v: F) -> U32 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors (U32)
        let r = (v.0 + 0.5) as u32;
        S(r)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1588-L1600 (chrome/m156) (SKRP_CPU_SCALAR)
    /// `trunc_(v)`: `(U32)v`.
    pub fn trunc_(v: F) -> U32 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors (U32)v
        let r = v.0 as u32;
        S(r)
    }

    /// `(I32)v`: a C cast of a float to `int32_t`.
    pub fn to_i32(v: F) -> I32 {
        #[allow(clippy::cast_possible_truncation)] // mirrors (I32)v
        let r = v.0 as i32;
        S(r)
    }

    /// `cast(U32)`: `(F)v`, an *unsigned* conversion (round to nearest even).
    pub fn cast_f(v: U32) -> F {
        #[allow(clippy::cast_precision_loss)] // mirrors (F)v
        let r = v.0 as f32;
        S(r)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L153-L154 (chrome/m156)
    /// `pack(U32) -> U16`: `(U16)v`, truncation.
    pub fn pack_u32(v: U32) -> U16 {
        #[allow(clippy::cast_possible_truncation)] // mirrors (U16)v
        let r = v.0 as u16;
        S(r)
    }

    /// `pack(U16) -> U8`: `(U8)v`, truncation.
    pub fn pack_u16(v: U16) -> U8 {
        #[allow(clippy::cast_possible_truncation)] // mirrors (U8)v
        let r = v.0 as u8;
        S(r)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L156-L160 (chrome/m156)
    /// `if_then_else(I32 c, F t, F e)`: `c ? t : e` (any nonzero `c` selects `t`).
    pub fn if_then_else_f(c: I32, t: F, e: F) -> F {
        if c.0 != 0 { t } else { e }
    }

    /// `if_then_else(I32 c, I32 t, I32 e)`.
    pub fn if_then_else_i(c: I32, t: I32, e: I32) -> I32 {
        if c.0 != 0 { t } else { e }
    }

    /// `any(c)`: `c != 0`.
    pub fn any(c: I32) -> bool {
        c.0 != 0
    }

    /// `all(c)`: `c != 0`.
    pub fn all(c: I32) -> bool {
        c.0 != 0
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2277-L2286 (chrome/m156)
    /// `cond_to_mask`: conditions are 0/1 here; masks are 0/~0.
    pub fn cond_to_mask(cond: I32) -> I32 {
        if_then_else_i(cond, S(!0), S(0))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4950-L4961 (chrome/m156) (generic div_fn)
    /// `SkSL` `int` division: `x / 0` divides by `-1` instead, and `INT_MIN / -1` by `-2`.
    pub fn div_i32(d: I32, s: I32) -> I32 {
        let mut divisor = s;
        // Integer division crashes when we divide by 0, but we can divide by -1 to not crash
        // (the result will be non-sensical). The mask will be 0xFFFFFFF if true, which happens
        // to be -1.
        divisor |= cond_to_mask(divisor.eq_mask(0));
        // Dividing by -1 works for all numerators *except* INT_MIN, so we can add -1 once more
        // if we are in that case. (`&&` of two C bools: both comparisons are 0/1 here.)
        divisor += cond_to_mask(divisor.eq_mask(-1) & d.eq_mask(i32::MIN));
        d / divisor
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4963-L4970 (chrome/m156) (generic div_fn)
    /// `SkSL` `uint` division: `x / 0` divides by `0xFFFFFFFF` instead.
    pub fn div_u32(d: U32, s: U32) -> U32 {
        let mut divisor = s;
        // Integer division crashes when we divide by 0, but we can divide by something else
        // to not crash (the result will be non-sensical). The mask will be 0xFFFFFFF if true.
        divisor |= cond_to_mask(divisor.eq_mask(0).bit_cast()).bit_cast();
        d / divisor
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1737-L1741 (chrome/m156) (SKRP_CPU_SCALAR)
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
