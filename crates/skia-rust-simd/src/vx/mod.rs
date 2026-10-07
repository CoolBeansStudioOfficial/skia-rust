// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkVx.h

//! `skvx`: portable lane-wise SIMD vectors, ported from `src/core/SkVx.h`.
//!
//! [`Vec<N, T>`](Vec) is `N` lanes of `T` stored as a flat array. Everything is lane-wise safe
//! Rust: the SIMD fast paths of the C++ (SSE, AVX, NEON, LSX, wasm) are all bit-identical to the
//! portable code they replace, so only the portable code is ported here. (The one exception,
//! `reduce_add` on floats, uses the summation order of Skia's x86 and `AArch64` paths.)
//!
//! # Mapping from C++
//! | C++ | Rust |
//! |---|---|
//! | `skvx::Vec<N,T>` | [`Vec<N, T>`](Vec) |
//! | `Vec<N,T>(s)` / `Vec<N,T>{a,b}` | [`Vec::splat`], `From<T>` / [`Vec::from_list`] |
//! | `Vec::Load(p)` / `v.store(p)` | [`Vec::load`] / [`Vec::store`] |
//! | `x == y`, `x < y`, ... (return a mask) | [`Vec::eq_mask`], [`Vec::lt_mask`], ... |
//! | `min(x,y)`, `max(x,y)`, `pin(x,lo,hi)` | [`Vec::min`], [`Vec::max`], [`Vec::pin`] |
//! | `min(x)`, `max(x)` (horizontal) | [`reduce_min`], [`reduce_max`] |
//! | `cast<D>(x)` | [`Vec::cast`] |
//! | `sk_bit_cast<Vec<M,U>>(x)` | [`Vec::bit_cast`] |
//! | `shuffle<2,1,0,3>(x)` | [`shuffle`]`(x, [2, 1, 0, 3])` |
//! | `join(a, b)`, `v.lo`, `v.hi` | [`join`], [`Vec::lo`], [`Vec::hi`] (sizes 1 to 32) |
//! | `strided_load2(p, a, b)` | `let (a, b) = strided_load2(p)` |
//! | `~x`, `!x` | `!x`, [`Vec::logical_not`] |
//! | `any`, `all`, `if_then_else`, `floor`, ... | the same-named free functions |
//!
//! Lane masks are all-ones or zero: `i32`/`i64` for `f32`/`f64` lanes, the lane type itself for
//! integers (`skvx::Mask<T>`).

// `#[inline(always)]` on the lane operators is required by the raster pipeline design (§2.4):
// they must inline into `#[target_feature]` stage functions so LLVM vectorizes them there.
#[allow(clippy::inline_always)]
mod funcs;
#[allow(clippy::inline_always)]
mod lane;
#[allow(clippy::inline_always)]
mod vec;

pub use funcs::{
    ScaledDividerU32, abs, all, any, approx_scale, ceil, cross, div255, dot, floor, fma, fract,
    from_half, if_then_else, isfinite, length, lrint, map2, map3, mulhi, mull, naive_if_then_else,
    normalize, reduce_add, reduce_max, reduce_min, round, saturated_add, shuffle, sqrt,
    strided_load2, strided_load4, to_half, trunc,
};
pub use lane::{CastFrom, FloatLane, IntLane, Lane, MulWiden, UnsignedLane};
pub use vec::{Join, Vec, join};

// Port of: src/core/SkVx.h#L1214-L1247 (chrome/m156)
/// `skvx::float2`.
#[doc(alias = "float2")]
pub type Float2 = Vec<2, f32>;
/// `skvx::float4`.
#[doc(alias = "float4")]
pub type Float4 = Vec<4, f32>;
/// `skvx::float8`.
#[doc(alias = "float8")]
pub type Float8 = Vec<8, f32>;

/// `skvx::double2`.
#[doc(alias = "double2")]
pub type Double2 = Vec<2, f64>;
/// `skvx::double4`.
#[doc(alias = "double4")]
pub type Double4 = Vec<4, f64>;
/// `skvx::double8`.
#[doc(alias = "double8")]
pub type Double8 = Vec<8, f64>;

/// `skvx::byte2` (`uint8_t` lanes).
#[doc(alias = "byte2")]
pub type Byte2 = Vec<2, u8>;
/// `skvx::byte4`.
#[doc(alias = "byte4")]
pub type Byte4 = Vec<4, u8>;
/// `skvx::byte8`.
#[doc(alias = "byte8")]
pub type Byte8 = Vec<8, u8>;
/// `skvx::byte16`.
#[doc(alias = "byte16")]
pub type Byte16 = Vec<16, u8>;

/// `skvx::int2`.
#[doc(alias = "int2")]
pub type Int2 = Vec<2, i32>;
/// `skvx::int4`.
#[doc(alias = "int4")]
pub type Int4 = Vec<4, i32>;
/// `skvx::int8`.
#[doc(alias = "int8")]
pub type Int8 = Vec<8, i32>;

/// `skvx::ushort2`.
#[doc(alias = "ushort2")]
pub type UShort2 = Vec<2, u16>;
/// `skvx::ushort4`.
#[doc(alias = "ushort4")]
pub type UShort4 = Vec<4, u16>;
/// `skvx::ushort8`.
#[doc(alias = "ushort8")]
pub type UShort8 = Vec<8, u16>;

/// `skvx::uint2`.
#[doc(alias = "uint2")]
pub type UInt2 = Vec<2, u32>;
/// `skvx::uint4`.
#[doc(alias = "uint4")]
pub type UInt4 = Vec<4, u32>;
/// `skvx::uint8`.
#[doc(alias = "uint8")]
pub type UInt8 = Vec<8, u32>;

/// `skvx::long2`.
#[doc(alias = "long2")]
pub type Long2 = Vec<2, i64>;
/// `skvx::long4`.
#[doc(alias = "long4")]
pub type Long4 = Vec<4, i64>;
/// `skvx::long8`.
#[doc(alias = "long8")]
pub type Long8 = Vec<8, i64>;

/// `skvx::half2`: half-float storage, used with [`from_half`] and [`to_half`].
#[doc(alias = "half2")]
pub type Half2 = Vec<2, u16>;
/// `skvx::half4`.
#[doc(alias = "half4")]
pub type Half4 = Vec<4, u16>;
/// `skvx::half8`.
#[doc(alias = "half8")]
pub type Half8 = Vec<8, u16>;
