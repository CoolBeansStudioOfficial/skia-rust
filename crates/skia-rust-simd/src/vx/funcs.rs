// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkVx.h

//! The free functions of `skvx`, as portable lane-wise safe Rust.

use core::array;

use super::lane::{FloatLane, IntLane, Lane, MulWiden, UnsignedLane};
use super::vec::Vec;

// Port of: src/core/SkVx.h#L477-L480 (chrome/m156)
/// `naive_if_then_else(cond, t, e)`: `(cond & t) | (~cond & e)` on the lane bits.
#[must_use]
pub fn naive_if_then_else<const N: usize, T: Lane>(
    cond: Vec<N, T::Mask>,
    t: Vec<N, T>,
    e: Vec<N, T>,
) -> Vec<N, T> {
    Vec(array::from_fn(|i| {
        let c = cond.0[i];
        let bits = c
            .lane_and(t.0[i].to_mask_bits())
            .lane_or(c.lane_not().lane_and(e.0[i].to_mask_bits()));
        T::from_mask_bits(bits)
    }))
}

// Port of: src/core/SkVx.h#L482-L531 (chrome/m156)
/// `if_then_else(cond, t, e)`: lanes of `t` where `cond` has bits set, else lanes of `e`.
///
/// Bitwise select, so any lane of `cond` that is neither all-ones nor zero mixes the bits.
#[must_use]
pub fn if_then_else<const N: usize, T: Lane>(
    cond: Vec<N, T::Mask>,
    t: Vec<N, T>,
    e: Vec<N, T>,
) -> Vec<N, T> {
    // The SIMD blends (`blendv`, `vbsl`, ...) are bitwise selects equal to the portable default.
    naive_if_then_else(cond, t, e)
}

// Port of: src/core/SkVx.h#L533-L583 (chrome/m156)
/// `any(x)`: true if any lane is non-zero.
#[must_use]
pub fn any<const N: usize, T: Lane>(x: Vec<N, T>) -> bool {
    x.0.iter().any(|v| v.is_nonzero())
}

// Port of: src/core/SkVx.h#L585-L626 (chrome/m156)
/// `all(x)`: true if every lane is non-zero.
#[must_use]
pub fn all<const N: usize, T: Lane>(x: Vec<N, T>) -> bool {
    x.0.iter().all(|v| v.is_nonzero())
}

/// Reduces `lanes` as a balanced binary tree over the `lo`/`hi` halves, as Skia's recursion does.
fn tree<T: Copy>(lanes: &[T], f: &impl Fn(T, T) -> T) -> T {
    if lanes.len() == 1 {
        return lanes[0];
    }
    let (lo, hi) = lanes.split_at(lanes.len() / 2);
    f(tree(lo, f), tree(hi, f))
}

// Port of: src/core/SkVx.h#L642-L646 (chrome/m156)
/// `min(x)`: the smallest lane, combining halves with `std::min` (`b < a ? b : a`).
#[must_use]
#[doc(alias = "min")]
pub fn reduce_min<const N: usize, T: Lane>(x: Vec<N, T>) -> T {
    tree(&x.0, &|a, b| if b < a { b } else { a })
}

// Port of: src/core/SkVx.h#L642-L646 (chrome/m156)
/// `max(x)`: the largest lane, combining halves with `std::max` (`a < b ? b : a`).
#[must_use]
#[doc(alias = "max")]
pub fn reduce_max<const N: usize, T: Lane>(x: Vec<N, T>) -> T {
    tree(&x.0, &|a, b| if a < b { b } else { a })
}

// Port of: src/core/SkVx.h#L662-L677 (chrome/m156)
/// `shuffle<Ix...>(x)`: `{ x[ix[0]], x[ix[1]], ... }`, with an output of any size `M`.
///
/// ```text
/// shuffle(rgba, [2, 1, 0, 3])  ~> {B,G,R,A}
/// shuffle(rgba, [2, 1])        ~> {B,G}
/// shuffle(rgba, [3, 3, 3, 3])  ~> {A,A,A,A}
/// ```
///
/// # Panics
/// If an index is `>= N`.
#[must_use]
pub fn shuffle<const M: usize, const N: usize, T: Copy>(x: Vec<N, T>, ix: [usize; M]) -> Vec<M, T> {
    Vec(ix.map(|i| x.0[i]))
}

// Port of: src/core/SkVx.h#L679-L701 (chrome/m156)
/// `map(fn, x, y)`: `{ fn(x[0], y[0]), fn(x[1], y[1]), ... }`. See also [`Vec::map`].
#[must_use]
pub fn map2<const N: usize, T: Lane, U: Lane, R: Lane>(
    mut f: impl FnMut(T, U) -> R,
    x: Vec<N, T>,
    y: Vec<N, U>,
) -> Vec<N, R> {
    Vec(array::from_fn(|i| f(x.0[i], y.0[i])))
}

// Port of: src/core/SkVx.h#L679-L701 (chrome/m156)
/// `map(fn, x, y, z)`. See also [`Vec::map`].
#[must_use]
pub fn map3<const N: usize, T: Lane, U: Lane, V: Lane, R: Lane>(
    mut f: impl FnMut(T, U, V) -> R,
    x: Vec<N, T>,
    y: Vec<N, U>,
    z: Vec<N, V>,
) -> Vec<N, R> {
    Vec(array::from_fn(|i| f(x.0[i], y.0[i], z.0[i])))
}

// Port of: src/core/SkVx.h#L703 (chrome/m156)
/// `ceil(x)`.
#[must_use]
pub fn ceil<const N: usize, T: FloatLane>(x: Vec<N, T>) -> Vec<N, T> {
    x.map(T::lane_ceil)
}

// Port of: src/core/SkVx.h#L704 (chrome/m156)
/// `floor(x)`.
#[must_use]
pub fn floor<const N: usize, T: FloatLane>(x: Vec<N, T>) -> Vec<N, T> {
    x.map(T::lane_floor)
}

// Port of: src/core/SkVx.h#L705 (chrome/m156)
/// `trunc(x)`.
#[must_use]
pub fn trunc<const N: usize, T: FloatLane>(x: Vec<N, T>) -> Vec<N, T> {
    x.map(T::lane_trunc)
}

// Port of: src/core/SkVx.h#L706 (chrome/m156)
/// `round(x)`: half away from zero (`roundf`).
#[must_use]
pub fn round<const N: usize, T: FloatLane>(x: Vec<N, T>) -> Vec<N, T> {
    x.map(T::lane_round)
}

// Port of: src/core/SkVx.h#L707 (chrome/m156)
/// `sqrt(x)`.
#[must_use]
pub fn sqrt<const N: usize, T: FloatLane>(x: Vec<N, T>) -> Vec<N, T> {
    x.map(T::lane_sqrt)
}

// Port of: src/core/SkVx.h#L708 (chrome/m156)
/// `abs(x)`.
#[must_use]
pub fn abs<const N: usize, T: FloatLane>(x: Vec<N, T>) -> Vec<N, T> {
    x.map(T::lane_abs)
}

// Port of: src/core/SkVx.h#L709-L715 (chrome/m156)
/// `fma(x, y, z)`: `x*y + z` with a single rounding.
#[must_use]
pub fn fma<const N: usize, T: FloatLane>(x: Vec<N, T>, y: Vec<N, T>, z: Vec<N, T>) -> Vec<N, T> {
    map3(T::lane_fma, x, y, z)
}

// Port of: src/core/SkVx.h#L717-L743 (chrome/m156)
/// `lrint(x)`: round to nearest even and convert to `i32`.
///
/// Like Skia on x86, out-of-range and NaN lanes are `i32::MIN` when `N >= 4` (the `cvtps2dq`
/// "integer indefinite", which Skia reaches through its SSE/AVX paths); `N` of 1 and 2 follow
/// Skia's scalar `(int)lrintf(x)` path, which converts through a 64-bit `long` first.
#[must_use]
#[allow(clippy::cast_possible_truncation)] // mirrors the (int) cast of lrintf's long result
pub fn lrint<const N: usize>(x: Vec<N, f32>) -> Vec<N, i32> {
    x.map(|v| {
        let r = v.round_ties_even();
        if N < 4 {
            // (int)lrintf(x): lrintf is 64-bit on LP64 and returns LONG_MIN when out of range.
            let l = if r.is_nan()
                || r >= 9_223_372_036_854_775_808.0_f32
                || r < -9_223_372_036_854_775_808.0_f32
            {
                i64::MIN
            } else {
                // in range by the check above
                r as i64
            };
            l as i32
        } else if r.is_nan() || r >= 2_147_483_648.0_f32 || r < -2_147_483_648.0_f32 {
            i32::MIN
        } else {
            // in range by the check above
            r as i32
        }
    })
}

// Port of: src/core/SkVx.h#L745 (chrome/m156)
/// `fract(x)`: `x - floor(x)`.
#[must_use]
pub fn fract<const N: usize>(x: Vec<N, f32>) -> Vec<N, f32> {
    x - floor(x)
}

// Port of: src/core/SkVx.h#L747-L786 (chrome/m156)
/// `to_half(x)`: converts float to half (IEEE binary16 bits), rounding to nearest even,
/// supporting subnormal f16 results and overflow to f16 infinity. Must not be called with NaNs.
///
/// KEEP IN SYNC with skcms' `Half_from_F` (as the C++ says).
///
/// # Panics
/// In debug builds, if a lane is NaN.
#[must_use]
#[allow(clippy::many_single_char_names)] // the names of the C++ locals (s, em, ...)
pub fn to_half<const N: usize>(x: Vec<N, f32>) -> Vec<N, u16> {
    debug_assert!(all(x.eq_mask(x))); // No NaNs should reach this function

    // The vector algorithm is entirely lane-wise, so it is applied to one lane at a time.
    x.map(|x| {
        let i = |f: f32| f.to_bits().cast_signed(); // I(x)
        let f = |v: i32| f32::from_bits(v.cast_unsigned()); // F(x)
        let sem = i(x);
        let s = sem & i32::MIN; // 0x8000'0000
        // |x| clamped to f16 infinity: min(sem ^ s, 0x4780'0000)
        let em = {
            let (a, b) = (sem ^ s, 0x4780_0000_i32);
            if b < a { b } else { a }
        };
        // F(em)*8192 increases the exponent by 13, which when added back to em will shift the
        // mantissa bits 13 to the right. We clamp to 1/2 for subnormal values, which
        // automatically shifts the mantissa to match 2^-14 expected for a subnorm f16.
        let magic = {
            let (a, b) = (f(em) * 8192.0_f32, 0.5_f32); // max(F(em)*8192.f, 0.5f)
            i(if a < b { b } else { a }) & (255 << 23)
        };
        // shift mantissa with automatic round-to-even
        let rounded = i(f(em) + f(magic));
        // Subtract 127 for f32 bias, subtract 13 to undo the *8192, subtract 1 to remove the
        // implicit leading 1., and add 15 to get the f16 biased exponent.
        let exp = (magic >> 13).wrapping_sub((127 - 15 + 13 + 1) << 10); // shift and re-bias exponent
        let f16 = rounded.wrapping_add(exp); // use + if 'rounded' rolled over into first exponent bit
        // cast<uint16_t>((s>>16) | f16)
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // C cast to uint16_t
        let half = ((s >> 16) | f16) as u16;
        half
    })
}

// Port of: src/core/SkVx.h#L788-L816 (chrome/m156)
/// `from_half(x)`: converts half (IEEE binary16 bits) to float, preserving NaN and +/- infinity.
///
/// KEEP IN SYNC with skcms' `F_from_Half` (as the C++ says).
#[must_use]
pub fn from_half<const N: usize>(x: Vec<N, u16>) -> Vec<N, f32> {
    x.map(|x| {
        let wide = i32::from(x);
        let s = wide & 0x8000;
        let em = wide ^ s;
        // Expands exponent to fill 8 bits
        let inf_or_nan = if em >= (31 << 10) { 255 << 23 } else { 0 };
        let is_norm: i32 = if em > 0x3ff { !0 } else { 0 };
        // subnormal f16's are 2^-14*0.[m0:9] == 2^-24*[m0:9].0
        #[allow(clippy::cast_precision_loss)] // cast<float>(em), em < 2^15 is exact
        let sub = ((em as f32) * (1.0_f32 / (1 << 24) as f32))
            .to_bits()
            .cast_signed();
        // Shifts mantissa, shifts + re-biases exp
        let norm = (em << 13).wrapping_add((127 - 15) << 23);
        let finite = (is_norm & norm) | (!is_norm & sub);
        // If 'x' is f16 +/- infinity, inf_or_nan will be the filled 8-bit exponent but 'norm'
        // will be all 0s since 'x's mantissa is 0. Thus norm | inf_or_nan becomes f32 infinity.
        // However, if 'x' is an f16 NaN, some bits of 'norm' will be non-zero, so it stays an
        // f32 NaN after the OR.
        f32::from_bits(((s << 16) | finite | inf_or_nan).cast_unsigned())
    })
}

// Port of: src/core/SkVx.h#L818-L821 (chrome/m156)
/// `div255(x) = (x + 127) / 255`: a bit-exact rounding divide-by-255, packing down to 8-bit.
#[must_use]
pub fn div255<const N: usize>(x: Vec<N, u16>) -> Vec<N, u8> {
    ((x + 127) / 255).cast::<u8>()
}

// Port of: src/core/SkVx.h#L823-L831 (chrome/m156)
/// `approx_scale(x, y)` approximates `div255(cast<u16>(x) * cast<u16>(y))` within a bit, and is
/// always perfect when `x` or `y` is 0 or 255.
#[must_use]
pub fn approx_scale<const N: usize>(x: Vec<N, u8>, y: Vec<N, u8>) -> Vec<N, u8> {
    // All of (x*y+x)/256, (x*y+y)/256, and (x*y+255)/256 meet the criteria above.
    // We happen to have historically picked (x*y+x)/256.
    let xx = x.cast::<u16>();
    let yy = y.cast::<u16>();
    ((xx * yy + xx) / 256).cast::<u8>()
}

// Port of: src/core/SkVx.h#L833-L860 (chrome/m156)
/// `saturated_add(x, y)`: sums values and clamps to the maximum value instead of overflowing.
#[must_use]
pub fn saturated_add<const N: usize, T: UnsignedLane>(x: Vec<N, T>, y: Vec<N, T>) -> Vec<N, T> {
    // The saturating SIMD instructions agree with this portable form.
    let sum = x + y;
    if_then_else(sum.lt_mask(x), Vec::splat(T::MAX), sum)
}

// Port of: src/core/SkVx.h#L908-L938 (chrome/m156)
/// `mull(x, y)`: widening multiply, `u8 * u8 -> u16` or `u16 * u16 -> u32`.
#[must_use]
pub fn mull<const N: usize, T: MulWiden>(x: Vec<N, T>, y: Vec<N, T>) -> Vec<N, T::Wide> {
    x.cast::<T::Wide>() * y.cast::<T::Wide>()
}

// Port of: src/core/SkVx.h#L940-L964 (chrome/m156)
/// `mulhi(x, y)`: the high 16 bits of the 32-bit product.
#[must_use]
pub fn mulhi<const N: usize>(x: Vec<N, u16>, y: Vec<N, u16>) -> Vec<N, u16> {
    (mull(x, y) >> 16).cast::<u16>()
}

// Port of: src/core/SkVx.h#L966-L982 (chrome/m156)
/// `dot(a, b)`: `a[0]*b[0] + a[1]*b[1] + ...`, summed left to right.
#[must_use]
pub fn dot<const N: usize, T: Lane>(a: Vec<N, T>, b: Vec<N, T>) -> T {
    let ab = a * b;
    // N == 2: ab[0] + ab[1]; N == 4: ab[0] + ab[1] + ab[2] + ab[3]; else a for loop: all of
    // them are the same left fold.
    ab.0[1..].iter().fold(ab.0[0], |sum, &v| sum.lane_add(v))
}

// Port of: src/core/SkVx.h#L984-L1046 (chrome/m156)
/// `reduce_add(x)`: the sum of the lanes, as a reduction tree.
///
/// Wider vectors fold `lo + hi` down to 4 lanes, which are summed `(x0 + x1) + (x2 + x3)`: the
/// order of Skia's x86 (SSE shuffle-reduce) and `AArch64` (`vaddvq_f32`) implementations.
#[must_use]
#[allow(clippy::many_single_char_names)] // the lane names of the 1/2/4-lane sums
pub fn reduce_add<const N: usize, T: Lane>(x: Vec<N, T>) -> T {
    let mut lanes = x.0;
    let mut len = N;
    while len > 4 {
        let half = len / 2;
        for i in 0..half {
            lanes[i] = lanes[i].lane_add(lanes[i + half]);
        }
        len = half;
    }
    match lanes[..len] {
        [a] => a,
        [a, b] => a.lane_add(b),
        [a, b, c, d] => a.lane_add(b).lane_add(c.lane_add(d)),
        _ => unreachable!("N must be a power of two"),
    }
}

// Port of: src/core/SkVx.h#L1048-L1051 (chrome/m156)
/// `cross(a, b)`: the 2D cross product `a.x*b.y - a.y*b.x`.
#[must_use]
pub fn cross<T: Lane>(a: Vec<2, T>, b: Vec<2, T>) -> T {
    let x = a * shuffle(b, [1, 0]);
    x.0[0].lane_sub(x.0[1])
}

// Port of: src/core/SkVx.h#L1053-L1059 (chrome/m156)
/// `length(v)`: `sqrt(dot(v, v))`.
#[must_use]
pub fn length<const N: usize, T: FloatLane>(v: Vec<N, T>) -> T {
    dot(v, v).lane_sqrt()
}

// Port of: src/core/SkVx.h#L1061-L1067 (chrome/m156)
/// `normalize(v)`: `v / length(v)`.
#[must_use]
pub fn normalize<const N: usize, T: FloatLane>(v: Vec<N, T>) -> Vec<N, T> {
    v / length(v)
}

// Port of: src/core/SkVx.h#L1069-L1077 (chrome/m156)
/// `isfinite(v)`: true if no float lane is infinite or NaN (always true for integer lanes).
#[must_use]
pub fn isfinite<const N: usize, T: Lane>(v: Vec<N, T>) -> bool {
    v.0.iter().all(|x| x.is_finite_lane())
}

// Port of: src/core/SkVx.h#L1079-L1176 (chrome/m156)
/// `strided_load4(v, a, b, c, d)`: de-interleaving load of 4 vectors from `v[0..4*N]`.
///
/// # Panics
/// If `v` has fewer than `4*N` elements.
#[must_use]
#[allow(clippy::type_complexity)] // the four output vectors of Skia's out-parameters
pub fn strided_load4<const N: usize, T: Lane>(
    v: &[T],
) -> (Vec<N, T>, Vec<N, T>, Vec<N, T>, Vec<N, T>) {
    (
        Vec(array::from_fn(|i| v[4 * i])),
        Vec(array::from_fn(|i| v[4 * i + 1])),
        Vec(array::from_fn(|i| v[4 * i + 2])),
        Vec(array::from_fn(|i| v[4 * i + 3])),
    )
}

// Port of: src/core/SkVx.h#L1178-L1212 (chrome/m156)
/// `strided_load2(v, a, b)`: de-interleaving load of 2 vectors from `v[0..2*N]`.
///
/// # Panics
/// If `v` has fewer than `2*N` elements.
#[must_use]
pub fn strided_load2<const N: usize, T: Lane>(v: &[T]) -> (Vec<N, T>, Vec<N, T>) {
    (
        Vec(array::from_fn(|i| v[2 * i])),
        Vec(array::from_fn(|i| v[2 * i + 1])),
    )
}

// Port of: src/core/SkVx.h#L862-L905 (chrome/m156)
/// Creates a function `divide(numerator)` that calculates `numerator / denominator` for a
/// divisor > 1. For this to be rounded properly, `numerator` should have `half()` added in:
/// `divide(numerator + half) == floor(numerator/denominator + 1/2)`, within +/- 1 of the true
/// value. The maximum that can be divided and rounded is `u32::MAX - half`.
///
/// `divisor_factor = (1 / divisor) * 2^32` and `half = (divisor + 1) / 2`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScaledDividerU32 {
    divisor_factor: u32,
    half: u32,
}

impl ScaledDividerU32 {
    /// Creates a divider for `divisor` (which must be > 1).
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // (uint32_t)(std::round(..)): < 2^32
    pub fn new(divisor: u32) -> Self {
        debug_assert!(divisor > 1);
        Self {
            divisor_factor: ((1.0 / f64::from(divisor)) * 4_294_967_296.0).round() as u32,
            half: divisor.wrapping_add(1) >> 1,
        }
    }

    /// `divide(numerator)`: `(numerator * divisorFactor) >> 32` per lane.
    #[must_use]
    pub fn divide(&self, numerator: Vec<4, u32>) -> Vec<4, u32> {
        ((numerator.cast::<u64>() * u64::from(self.divisor_factor)) >> 32).cast::<u32>()
    }

    /// `half()`.
    #[must_use]
    pub fn half(&self) -> u32 {
        self.half
    }

    /// `divisorFactor()`.
    #[must_use]
    pub fn divisor_factor(&self) -> u32 {
        self.divisor_factor
    }
}
