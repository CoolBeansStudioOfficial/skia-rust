// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Array ↔ register conversions for the `Neon` tier (design §3.2): the only `unsafe` in its lane
//! module.
//!
//! Skia's lane types *are* NEON registers (`ext_vector_type`); ours are arrays
//! ([`Vec`](crate::vx::Vec)), so primitives that use an intrinsic convert at the boundary. Every
//! 16-byte register shape goes through `uint32x4_t` (and `vreinterpretq_*`, which is value-only),
//! every 8-byte one through `uint16x4_t`. LLVM removes the round trips once the primitive is
//! inlined into a stage.

use core::arch::aarch64::{
    float32x4_t, uint16x4_t, uint32x4_t, vld1_u16, vld1q_f32, vld1q_u32, vst1_u16, vst1q_f32,
    vst1q_u32,
};

use crate::vx::{Lane, Vec};

/// `float32x4_t` from four floats.
#[target_feature(enable = "neon")]
#[inline]
#[must_use]
pub(crate) fn ps(v: Vec<4, f32>) -> float32x4_t {
    // SAFETY: `v.0` is a live `[f32; 4]`, so its pointer is valid for 16 bytes of reads;
    // `vld1q_f32` needs only `f32` alignment, which the array has.
    unsafe { vld1q_f32(v.0.as_ptr()) }
}

/// Four floats from a `float32x4_t`.
#[target_feature(enable = "neon")]
#[inline]
#[must_use]
pub(crate) fn from_ps(r: float32x4_t) -> Vec<4, f32> {
    let mut out = Vec([0.0f32; 4]);
    // SAFETY: `out.0` is a live `[f32; 4]`, valid for 16 bytes of writes with `f32` alignment.
    unsafe { vst1q_f32(out.0.as_mut_ptr(), r) };
    out
}

/// `uint32x4_t` from four `u32`s.
#[target_feature(enable = "neon")]
#[inline]
#[must_use]
fn q_u32(v: Vec<4, u32>) -> uint32x4_t {
    // SAFETY: `v.0` is a live `[u32; 4]`, valid for 16 bytes of reads with `u32` alignment.
    unsafe { vld1q_u32(v.0.as_ptr()) }
}

/// Four `u32`s from a `uint32x4_t`.
#[target_feature(enable = "neon")]
#[inline]
#[must_use]
fn from_q_u32(r: uint32x4_t) -> Vec<4, u32> {
    let mut out = Vec([0u32; 4]);
    // SAFETY: `out.0` is a live `[u32; 4]`, valid for 16 bytes of writes with `u32` alignment.
    unsafe { vst1q_u32(out.0.as_mut_ptr(), r) };
    out
}

/// `uint16x4_t` from four `u16`s.
#[target_feature(enable = "neon")]
#[inline]
#[must_use]
fn d_u16(v: Vec<4, u16>) -> uint16x4_t {
    // SAFETY: `v.0` is a live `[u16; 4]`, valid for 8 bytes of reads with `u16` alignment.
    unsafe { vld1_u16(v.0.as_ptr()) }
}

/// Four `u16`s from a `uint16x4_t`.
#[target_feature(enable = "neon")]
#[inline]
#[must_use]
fn from_d_u16(r: uint16x4_t) -> Vec<4, u16> {
    let mut out = Vec([0u16; 4]);
    // SAFETY: `out.0` is a live `[u16; 4]`, valid for 8 bytes of writes with `u16` alignment.
    unsafe { vst1_u16(out.0.as_mut_ptr(), r) };
    out
}

/// `uint32x4_t` from any 16-byte integer vector (`I32`, `U32`, lowp `U16`, …): Skia's `(U32)v`.
/// Other 16-byte register types follow with `vreinterpretq_*_u32`.
#[target_feature(enable = "neon")]
#[inline]
#[must_use]
pub(crate) fn q<const N: usize, T: Lane>(v: Vec<N, T>) -> uint32x4_t {
    q_u32(v.bit_cast())
}

/// A 16-byte vector from a `uint32x4_t` (use `vreinterpretq_u32_*` for other register types).
#[target_feature(enable = "neon")]
#[inline]
#[must_use]
pub(crate) fn from_q<const N: usize, T: Lane>(r: uint32x4_t) -> Vec<N, T> {
    from_q_u32(r).bit_cast()
}

/// `uint16x4_t` from any 8-byte vector (highp `U16`).
#[target_feature(enable = "neon")]
#[inline]
#[must_use]
pub(crate) fn d<const N: usize, T: Lane>(v: Vec<N, T>) -> uint16x4_t {
    d_u16(v.bit_cast())
}

/// An 8-byte vector from a `uint16x4_t`.
#[target_feature(enable = "neon")]
#[inline]
#[must_use]
pub(crate) fn from_d<const N: usize, T: Lane>(r: uint16x4_t) -> Vec<N, T> {
    from_d_u16(r).bit_cast()
}
