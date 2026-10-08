// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkChecksum.h and src/core/SkMathPriv.h (the two helpers below).

//! The few helpers this crate needs from `skia-rust-core`, copied until `skia-rust-base` exists.
//!
//! // TODO(S1): moves to skia-rust-base. The math modules move out of core into `skia-rust-base`
//! // (`docs/design/sksl.md` §2.2). When that lands, delete this module and import the originals.
//! The dependency direction is kept: `skia-rust-sksl` does not depend on `skia-rust-core`.

/// `SkChecksum::Mix`: the Murmur3 finalizer, a `uint32_t` → `uint32_t` mix used by `SkGoodHash`.
// Port of: src/core/SkChecksum.h#L27-L36 (chrome/m156)
#[doc(alias = "SkChecksum::Mix")]
#[must_use]
pub fn checksum_mix(mut hash: u32) -> u32 {
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85eb_ca6b);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xc2b2_ae35);
    hash ^= hash >> 16;
    hash
}

/// `SkNextLog2`: the smallest `n` with `2^n >= value`. `value` must not be zero.
// Port of: src/core/SkMathPriv.h#L157-L160 (chrome/m156)
#[must_use]
pub fn next_log2(value: u32) -> u32 {
    debug_assert_ne!(value, 0, "SkNextLog2 is undefined for 0");
    32 - (value - 1).leading_zeros()
}

/// `SkNextPow2`: the smallest power of two that is `>= value`. `value` must not be zero.
// Port of: src/core/SkMathPriv.h#L181-L184 (chrome/m156)
#[doc(alias = "SkNextPow2")]
#[must_use]
pub fn next_pow2(value: u32) -> u32 {
    1 << next_log2(value)
}

#[cfg(test)]
mod tests {
    use super::{checksum_mix, next_pow2};

    #[test]
    fn mix_is_the_murmur3_finalizer() {
        // Zero is a fixed point; any other input moves.
        assert_eq!(checksum_mix(0), 0);
        assert_eq!(checksum_mix(1), 0x514e_28b7);
    }

    #[test]
    fn next_pow2_rounds_up() {
        assert_eq!(next_pow2(1), 1);
        assert_eq!(next_pow2(2), 2);
        assert_eq!(next_pow2(3), 4);
        assert_eq!(next_pow2(5), 8);
        assert_eq!(next_pow2(16), 16);
    }
}

/// `SkSafeMath`'s integer operations. Each records overflow in `ok` and returns the C++ result.
// Port of: src/core/SkSafeMath.h#L24-L60 (chrome/m156) (shim, owner skia-rust-base)
#[derive(Clone, Copy, Debug)]
pub struct SafeMath {
    ok: bool,
}

impl Default for SafeMath {
    fn default() -> Self {
        Self::new()
    }
}

impl SafeMath {
    /// A checker with no overflow yet.
    #[must_use]
    pub fn new() -> Self {
        Self { ok: true }
    }

    /// `ok()`: no operation overflowed.
    #[must_use]
    pub fn ok(self) -> bool {
        self.ok
    }

    /// `addInt`.
    pub fn add_int(&mut self, a: i32, b: i32) -> i32 {
        let result = i64::from(a) + i64::from(b);
        if i32::try_from(result).is_err() {
            self.ok = false;
        }
        result as i32
    }

    /// `subInt`.
    pub fn sub_int(&mut self, a: i32, b: i32) -> i32 {
        let result = i64::from(a) - i64::from(b);
        if i32::try_from(result).is_err() {
            self.ok = false;
        }
        result as i32
    }

    /// `mulInt`.
    pub fn mul_int(&mut self, a: i32, b: i32) -> i32 {
        let result = i64::from(a) * i64::from(b);
        if i32::try_from(result).is_err() {
            self.ok = false;
        }
        result as i32
    }

    /// `divInt`: flags a zero divisor and `INT_MIN / -1`, and returns `a` for them.
    pub fn div_int(&mut self, a: i32, b: i32) -> i32 {
        if b == 0 || (a == i32::MIN && b == -1) {
            self.ok = false;
            return a;
        }
        a / b
    }

    /// `modInt`: the remainder, with the checks of [`SafeMath::div_int`].
    pub fn mod_int(&mut self, a: i32, b: i32) -> i32 {
        if b == 0 || (a == i32::MIN && b == -1) {
            self.ok = false;
            return a;
        }
        a % b
    }
}

/// `SkSafeMath::Add(size_t, size_t)`: the sum, saturated to the maximum on overflow.
// Port of: src/core/SkSafeMath.cpp#L10-L14 (chrome/m156) (shim, owner skia-rust-base)
#[must_use]
pub fn saturating_add_size(x: usize, y: usize) -> usize {
    x.saturating_add(y)
}

/// `sk_double_saturate2int`: clamps to the `int` range. NaN gives `INT_MAX`, as the C++
/// comparisons do.
// Port of: include/private/SkFloatingPoint.h#L99-L104 (chrome/m156) (shim, owner skia-rust-base)
#[must_use]
pub fn double_saturate2int(x: f64) -> i32 {
    let x = if x < f64::from(i32::MAX) {
        x
    } else {
        f64::from(i32::MAX)
    };
    let x = if x > f64::from(i32::MIN) {
        x
    } else {
        f64::from(i32::MIN)
    };
    x as i32
}
