// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Array ↔ register conversions for the 128-bit x86 tiers (design §3.2): the only `unsafe` in
//! the lane modules.
//!
//! Skia's lane types *are* registers (`ext_vector_type`); ours are arrays
//! ([`Vec`](crate::vx::Vec)), so primitives that use an intrinsic convert at the boundary. LLVM
//! removes the round trip once the primitive is inlined into a stage.

use core::arch::x86_64::{
    __m128, __m128i, _mm_loadu_ps, _mm_loadu_si128, _mm_storeu_ps, _mm_storeu_si128,
};

use crate::vx::{Lane, Vec};

/// `__m128` from four floats.
#[target_feature(enable = "sse2")]
#[inline]
#[must_use]
pub(crate) fn ps(v: Vec<4, f32>) -> __m128 {
    // SAFETY: `v.0` is a live `[f32; 4]`, so its pointer is valid for 16 bytes of reads;
    // `_mm_loadu_ps` has no alignment requirement.
    unsafe { _mm_loadu_ps(v.0.as_ptr()) }
}

/// Four floats from an `__m128`.
#[target_feature(enable = "sse2")]
#[inline]
#[must_use]
pub(crate) fn from_ps(r: __m128) -> Vec<4, f32> {
    let mut out = Vec([0.0f32; 4]);
    // SAFETY: `out.0` is a live `[f32; 4]`, valid for 16 bytes of writes; `_mm_storeu_ps` has
    // no alignment requirement.
    unsafe { _mm_storeu_ps(out.0.as_mut_ptr(), r) };
    out
}

/// `__m128i` from four `i32`s.
#[target_feature(enable = "sse2")]
#[inline]
#[must_use]
fn si_i32(v: Vec<4, i32>) -> __m128i {
    // SAFETY: `v.0` is a live `[i32; 4]`, valid for 16 bytes of reads; `_mm_loadu_si128` has no
    // alignment requirement (the pointer cast only changes the pointee type).
    unsafe { _mm_loadu_si128(v.0.as_ptr().cast()) }
}

/// Four `i32`s from an `__m128i`.
#[target_feature(enable = "sse2")]
#[inline]
#[must_use]
fn from_si_i32(r: __m128i) -> Vec<4, i32> {
    let mut out = Vec([0i32; 4]);
    // SAFETY: `out.0` is a live `[i32; 4]`, valid for 16 bytes of writes; `_mm_storeu_si128`
    // has no alignment requirement (the pointer cast only changes the pointee type).
    unsafe { _mm_storeu_si128(out.0.as_mut_ptr().cast(), r) };
    out
}

/// `__m128i` from any 16-byte integer vector (`I32`, `U32`, lowp `U16`, …): Skia's `(__m128i)v`.
#[target_feature(enable = "sse2")]
#[inline]
#[must_use]
pub(crate) fn si<const N: usize, T: Lane>(v: Vec<N, T>) -> __m128i {
    si_i32(v.bit_cast())
}

/// A 16-byte vector from an `__m128i`: Skia's `(U32)r`, `(U16)r`, ….
#[target_feature(enable = "sse2")]
#[inline]
#[must_use]
pub(crate) fn from_si<const N: usize, T: Lane>(r: __m128i) -> Vec<N, T> {
    from_si_i32(r).bit_cast()
}
