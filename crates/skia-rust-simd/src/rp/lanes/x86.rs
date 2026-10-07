// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Array ↔ register conversions for the x86 tiers (design §3.2): the only `unsafe` in the lane
//! modules. 128-bit registers for `Sse2`/`Sse41` (and the 128-bit halves `Ml3` uses), 256-bit
//! for `Ml3` (and `Ml4`'s 256-bit `U16`/`U8` registers), 512-bit for `Ml4`.
//!
//! Skia's lane types *are* registers (`ext_vector_type`); ours are arrays
//! ([`Vec`](crate::vx::Vec)), so primitives that use an intrinsic convert at the boundary. LLVM
//! removes the round trip once the primitive is inlined into a stage.

use core::arch::x86_64::{
    __m128, __m128i, __m256, __m256i, __m512, __m512i, _mm_loadu_ps, _mm_loadu_si128,
    _mm_storeu_ps, _mm_storeu_si128, _mm256_loadu_ps, _mm256_loadu_si256, _mm256_storeu_ps,
    _mm256_storeu_si256, _mm512_loadu_ps, _mm512_loadu_si512, _mm512_storeu_ps,
    _mm512_storeu_si512,
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

/// `__m256` from eight floats.
#[target_feature(enable = "avx")]
#[inline]
#[must_use]
pub(crate) fn ps256(v: Vec<8, f32>) -> __m256 {
    // SAFETY: `v.0` is a live `[f32; 8]`, so its pointer is valid for 32 bytes of reads;
    // `_mm256_loadu_ps` has no alignment requirement.
    unsafe { _mm256_loadu_ps(v.0.as_ptr()) }
}

/// Eight floats from an `__m256`.
#[target_feature(enable = "avx")]
#[inline]
#[must_use]
pub(crate) fn from_ps256(r: __m256) -> Vec<8, f32> {
    let mut out = Vec([0.0f32; 8]);
    // SAFETY: `out.0` is a live `[f32; 8]`, valid for 32 bytes of writes; `_mm256_storeu_ps`
    // has no alignment requirement.
    unsafe { _mm256_storeu_ps(out.0.as_mut_ptr(), r) };
    out
}

/// `__m256i` from eight `i32`s.
#[target_feature(enable = "avx")]
#[inline]
#[must_use]
fn si256_i32(v: Vec<8, i32>) -> __m256i {
    // SAFETY: `v.0` is a live `[i32; 8]`, valid for 32 bytes of reads; `_mm256_loadu_si256` has
    // no alignment requirement (the pointer cast only changes the pointee type).
    unsafe { _mm256_loadu_si256(v.0.as_ptr().cast()) }
}

/// Eight `i32`s from an `__m256i`.
#[target_feature(enable = "avx")]
#[inline]
#[must_use]
fn from_si256_i32(r: __m256i) -> Vec<8, i32> {
    let mut out = Vec([0i32; 8]);
    // SAFETY: `out.0` is a live `[i32; 8]`, valid for 32 bytes of writes; `_mm256_storeu_si256`
    // has no alignment requirement (the pointer cast only changes the pointee type).
    unsafe { _mm256_storeu_si256(out.0.as_mut_ptr().cast(), r) };
    out
}

/// `__m256i` from any 32-byte integer vector: Skia's `(__m256i)v`.
#[target_feature(enable = "avx")]
#[inline]
#[must_use]
pub(crate) fn si256<const N: usize, T: Lane>(v: Vec<N, T>) -> __m256i {
    si256_i32(v.bit_cast())
}

/// A 32-byte vector from an `__m256i`: Skia's `(U32)r`, `(U16)r`, ….
#[target_feature(enable = "avx")]
#[inline]
#[must_use]
pub(crate) fn from_si256<const N: usize, T: Lane>(r: __m256i) -> Vec<N, T> {
    from_si256_i32(r).bit_cast()
}

/// `__m512` from sixteen floats.
#[target_feature(enable = "avx512f")]
#[inline]
#[must_use]
pub(crate) fn ps512(v: Vec<16, f32>) -> __m512 {
    // SAFETY: `v.0` is a live `[f32; 16]`, so its pointer is valid for 64 bytes of reads;
    // `_mm512_loadu_ps` has no alignment requirement.
    unsafe { _mm512_loadu_ps(v.0.as_ptr()) }
}

/// Sixteen floats from an `__m512`.
#[target_feature(enable = "avx512f")]
#[inline]
#[must_use]
pub(crate) fn from_ps512(r: __m512) -> Vec<16, f32> {
    let mut out = Vec([0.0f32; 16]);
    // SAFETY: `out.0` is a live `[f32; 16]`, valid for 64 bytes of writes; `_mm512_storeu_ps`
    // has no alignment requirement.
    unsafe { _mm512_storeu_ps(out.0.as_mut_ptr(), r) };
    out
}

/// `__m512i` from sixteen `i32`s.
#[target_feature(enable = "avx512f")]
#[inline]
#[must_use]
fn si512_i32(v: Vec<16, i32>) -> __m512i {
    // SAFETY: `v.0` is a live `[i32; 16]`, valid for 64 bytes of reads; `_mm512_loadu_si512` has
    // no alignment requirement (the pointer cast only changes the pointee type).
    unsafe { _mm512_loadu_si512(v.0.as_ptr().cast()) }
}

/// Sixteen `i32`s from an `__m512i`.
#[target_feature(enable = "avx512f")]
#[inline]
#[must_use]
fn from_si512_i32(r: __m512i) -> Vec<16, i32> {
    let mut out = Vec([0i32; 16]);
    // SAFETY: `out.0` is a live `[i32; 16]`, valid for 64 bytes of writes;
    // `_mm512_storeu_si512` has no alignment requirement (the pointer cast only changes the
    // pointee type).
    unsafe { _mm512_storeu_si512(out.0.as_mut_ptr().cast(), r) };
    out
}

/// `__m512i` from any 64-byte integer vector: Skia's `(__m512i)v`.
#[target_feature(enable = "avx512f")]
#[inline]
#[must_use]
pub(crate) fn si512<const N: usize, T: Lane>(v: Vec<N, T>) -> __m512i {
    si512_i32(v.bit_cast())
}

/// A 64-byte vector from an `__m512i`: Skia's `(U32)r`, `(I32)r`, ….
#[target_feature(enable = "avx512f")]
#[inline]
#[must_use]
pub(crate) fn from_si512<const N: usize, T: Lane>(r: __m512i) -> Vec<N, T> {
    from_si512_i32(r).bit_cast()
}
