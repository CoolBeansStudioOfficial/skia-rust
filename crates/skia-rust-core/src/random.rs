// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRandom.h

//! Marsaglia's multiply-with-carry pseudo random number generator, bit-exact with Skia's.

// Port of: include/private/SkFixed.h#L41 (chrome/m156)
// `SkFixedToFloat(x)` is `((x) * 1.52587890625e-5f)`; the int converts to float first.
#[allow(clippy::cast_precision_loss)] // mirrors the implicit int -> float conversion in C++
fn fixed_to_float(x: i32) -> f32 {
    x as f32 * (1.0_f32 / 65_536.0_f32) // == 1.52587890625e-5f, exactly 2^-16
}

/// Utility type that implements pseudo random 32-bit numbers using Marsaglia's
/// multiply-with-carry "mother of all" algorithm. Unlike `rand()`, this type holds its own
/// state, so that multiple instances can be used with no side-effects.
///
/// Has a large period and all bits are well-randomized.
// Port of: src/core/SkRandom.h#L27-L171 (chrome/m156)
#[doc(alias = "SkRandom")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Random {
    k: u32,
    j: u32,
}

// See "Numerical Recipes in C", 1992 page 284 for these constants.
// For the LCG that sets the initial state from a seed.
const K_MUL: u32 = 1_664_525;
const K_ADD: u32 = 1_013_904_223;
// Constants for the multiply-with-carry steps.
const K_K_MUL: u32 = 30345;
const K_J_MUL: u32 = 18000;

impl Default for Random {
    /// `SkRandom()`: seeded with 0.
    fn default() -> Self {
        Self::new(0)
    }
}

impl Random {
    /// `SkRandom(uint32_t seed)`.
    #[must_use]
    pub fn new(seed: u32) -> Self {
        let mut r = Self { k: 0, j: 0 };
        r.init(seed);
        r
    }

    /// Returns the next pseudo random number as an unsigned 32-bit value.
    #[doc(alias = "nextU")]
    pub fn next_u(&mut self) -> u32 {
        self.k = K_K_MUL
            .wrapping_mul(self.k & 0xffff)
            .wrapping_add(self.k >> 16);
        self.j = K_J_MUL
            .wrapping_mul(self.j & 0xffff)
            .wrapping_add(self.j >> 16);
        self.k.rotate_left(16).wrapping_add(self.j)
    }

    /// Returns the next pseudo random number as a signed 32-bit value.
    #[doc(alias = "nextS")]
    #[allow(clippy::cast_possible_wrap)] // mirrors (int32_t)nextU()
    pub fn next_s(&mut self) -> i32 {
        self.next_u() as i32
    }

    /// Returns a value in `[0...1)` as an IEEE float.
    #[doc(alias = "nextF")]
    pub fn next_f(&mut self) -> f32 {
        let floatint = 0x3f80_0000 | (self.next_u() >> 9);
        f32::from_bits(floatint) - 1.0f32
    }

    /// Returns a value in `[min...max)` as a float.
    #[doc(alias = "nextRangeF")]
    pub fn next_range_f(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f() * (max - min)
    }

    /// Returns the next pseudo random number, as an unsigned value of at most `bit_count` bits.
    #[doc(alias = "nextBits")]
    pub fn next_bits(&mut self, bit_count: u32) -> u32 {
        debug_assert!(bit_count > 0 && bit_count <= 32);
        self.next_u() >> (32 - bit_count)
    }

    /// Returns the next pseudo random unsigned number, mapped to lie within `[min, max]`
    /// inclusive.
    #[doc(alias = "nextRangeU")]
    pub fn next_range_u(&mut self, min: u32, max: u32) -> u32 {
        debug_assert!(min <= max);
        let range = max.wrapping_sub(min).wrapping_add(1);
        if 0 == range {
            self.next_u()
        } else {
            min.wrapping_add(self.next_u() % range)
        }
    }

    /// Returns the next pseudo random unsigned number, mapped to lie within `[0, count)`.
    #[doc(alias = "nextULessThan")]
    pub fn next_u_less_than(&mut self, count: u32) -> u32 {
        debug_assert!(count > 0);
        self.next_range_u(0, count.wrapping_sub(1))
    }

    /// Returns the next pseudo random number expressed as a `f32` in the range `[0..1)`.
    #[doc(alias = "nextUScalar1")]
    pub fn next_u_scalar1(&mut self) -> f32 {
        fixed_to_float(self.next_u_fixed1())
    }

    /// Returns the next pseudo random number expressed as a `f32` in the range `[min..max)`.
    #[doc(alias = "nextRangeScalar")]
    pub fn next_range_scalar(&mut self, min: f32, max: f32) -> f32 {
        self.next_u_scalar1() * (max - min) + min
    }

    /// Returns the next pseudo random number expressed as a `f32` in the range `[-1..1)`.
    #[doc(alias = "nextSScalar1")]
    pub fn next_s_scalar1(&mut self) -> f32 {
        fixed_to_float(self.next_s_fixed1())
    }

    /// Returns the next pseudo random number as a bool.
    #[doc(alias = "nextBool")]
    pub fn next_bool(&mut self) -> bool {
        self.next_u() >= 0x8000_0000
    }

    /// A biased version of [`Random::next_bool`].
    #[doc(alias = "nextBiasedBool")]
    pub fn next_biased_bool(&mut self, fraction_true: f32) -> bool {
        debug_assert!((0.0..=1.0).contains(&fraction_true));
        self.next_u_scalar1() <= fraction_true
    }

    /// Resets the random object.
    #[doc(alias = "setSeed")]
    pub fn set_seed(&mut self, seed: u32) {
        self.init(seed);
    }

    // Initialize state variables with LCG.
    // We must ensure that both J and K are non-zero, otherwise the
    // multiply-with-carry step will forevermore return zero.
    fn init(&mut self, seed: u32) {
        self.k = Self::next_lcg(seed);
        if 0 == self.k {
            self.k = Self::next_lcg(self.k);
        }
        self.j = Self::next_lcg(self.k);
        if 0 == self.j {
            self.j = Self::next_lcg(self.j);
        }
        debug_assert!(0 != self.k && 0 != self.j);
    }

    fn next_lcg(seed: u32) -> u32 {
        K_MUL.wrapping_mul(seed).wrapping_add(K_ADD)
    }

    /// Next pseudo random number as an unsigned `SkFixed` in the range `[0..SK_Fixed1)`.
    #[allow(clippy::cast_possible_wrap)] // value is < 2^16
    fn next_u_fixed1(&mut self) -> i32 {
        (self.next_u() >> 16) as i32
    }

    /// Next pseudo random number as a signed `SkFixed` in the range `[-SK_Fixed1..SK_Fixed1)`.
    fn next_s_fixed1(&mut self) -> i32 {
        self.next_s() >> 15
    }
}
