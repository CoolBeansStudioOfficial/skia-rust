// Copyright 2017 Google LLC
// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkSafeMath.h, src/core/SkSafeMath.cpp

//! Overflow-checked arithmetic (`SkSafeMath`).
//!
//! `SkSafeMath` always checks that a series of operations do not overflow.
//! This must be correct for all platforms, because this is a check for safety at runtime.

use crate::t_fits_in::t_fits_in;
use crate::to::WrappingCast;

/// Tracks whether a series of arithmetic operations overflowed.
// Port of: src/core/SkSafeMath.h#L23-L160 (chrome/m156)
#[doc(alias = "SkSafeMath")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafeMath {
    ok: bool,
}

impl Default for SafeMath {
    fn default() -> Self {
        Self { ok: true }
    }
}

impl SafeMath {
    /// A fresh `SafeMath` with no overflow recorded.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// True if no operation so far has overflowed.
    // Port of: src/core/SkSafeMath.h#L27-L28 (chrome/m156)
    #[must_use]
    pub fn ok(&self) -> bool {
        self.ok
    }

    /// Checked `x * y`.
    // Port of: src/core/SkSafeMath.h#L30-L32 (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // the 32-bit branch only runs when usize is 32 bits
    pub fn mul(&mut self, x: usize, y: usize) -> usize {
        if size_of::<usize>() == size_of::<u64>() {
            self.mul64(x as u64, y as u64) as usize
        } else {
            self.mul32(x as u32, y as u32) as usize
        }
    }

    /// Checked `x + y`.
    // Port of: src/core/SkSafeMath.h#L34-L38 (chrome/m156)
    pub fn add(&mut self, x: usize, y: usize) -> usize {
        let result = x.wrapping_add(y);
        self.ok &= result >= x;
        result
    }

    /// Checked `x - y`.
    // Port of: src/core/SkSafeMath.h#L40-L43 (chrome/m156)
    pub fn sub(&mut self, x: usize, y: usize) -> usize {
        self.ok &= x >= y;
        x.wrapping_sub(y)
    }

    /// Checked `a + b` on `i32`.
    // Port of: src/core/SkSafeMath.h#L45-L51 (chrome/m156)
    #[doc(alias = "addInt")]
    #[allow(clippy::cast_possible_truncation)] // mirrors static_cast<int>; overflow is recorded in ok
    pub fn add_int(&mut self, a: i32, b: i32) -> i32 {
        let result = i64::from(a) + i64::from(b);
        if !t_fits_in::<i32, i64>(result) {
            self.ok = false;
        }
        result as i32
    }

    /// Checked `a - b` on `i32`.
    // Port of: src/core/SkSafeMath.h#L53-L59 (chrome/m156)
    #[doc(alias = "subInt")]
    #[allow(clippy::cast_possible_truncation)] // mirrors static_cast<int>; overflow is recorded in ok
    pub fn sub_int(&mut self, a: i32, b: i32) -> i32 {
        let result = i64::from(a) - i64::from(b);
        if !t_fits_in::<i32, i64>(result) {
            self.ok = false;
        }
        result as i32
    }

    /// Checked `x * y` on `i32`.
    // Port of: src/core/SkSafeMath.h#L61-L67 (chrome/m156)
    #[doc(alias = "mulInt")]
    #[allow(clippy::cast_possible_truncation)] // mirrors static_cast<int>; overflow is recorded in ok
    pub fn mul_int(&mut self, x: i32, y: i32) -> i32 {
        let result = i64::from(x) * i64::from(y);
        if !t_fits_in::<i32, i64>(result) {
            self.ok = false;
        }
        result as i32
    }

    /// Checked `a / b` on `i32`.
    // Port of: src/core/SkSafeMath.h#L69-L75 (chrome/m156)
    #[doc(alias = "divInt")]
    pub fn div_int(&mut self, a: i32, b: i32) -> i32 {
        if b == 0 || (a == i32::MIN && b == -1) {
            self.ok = false;
            return a;
        }
        a / b
    }

    /// Checked `a % b` on `i32`.
    // Port of: src/core/SkSafeMath.h#L77-L83 (chrome/m156)
    #[doc(alias = "modInt")]
    pub fn mod_int(&mut self, a: i32, b: i32) -> i32 {
        if b == 0 || (a == i32::MIN && b == -1) {
            self.ok = false;
            return a;
        }
        a % b
    }

    /// Checked align-up to a power-of-2 `alignment`.
    // Port of: src/core/SkSafeMath.h#L85-L88 (chrome/m156)
    #[doc(alias = "alignUp")]
    pub fn align_up(&mut self, x: usize, alignment: usize) -> usize {
        debug_assert!(alignment.is_power_of_two());
        self.add(x, alignment - 1) & !(alignment - 1)
    }

    /// Checked align-up to any non-zero `alignment`.
    // Port of: src/core/SkSafeMath.h#L90-L97 (chrome/m156)
    #[doc(alias = "alignUpNonPow2")]
    pub fn align_up_non_pow2(&mut self, x: usize, alignment: usize) -> usize {
        if alignment == 0 {
            self.ok = false;
            return 0;
        }
        let aligned = self.add(x, alignment - 1);
        (aligned / alignment) * alignment
    }

    /// Checked least common multiple.
    // Port of: src/core/SkSafeMath.h#L99-L106 (chrome/m156)
    pub fn lcm(&mut self, a: usize, b: usize) -> usize {
        if a == 0 || b == 0 {
            self.ok = false;
            return 0;
        }
        let gcd = gcd(a, b);
        self.mul(a / gcd, b)
    }

    /// Checked integer cast: records an overflow if `value` does not fit in `TDst`, and returns
    /// the `static_cast` (wrapped) result either way.
    // Port of: src/core/SkSafeMath.h#L108-L113 (chrome/m156)
    #[doc(alias = "castTo")]
    pub fn cast_to<TDst, TSrc>(&mut self, value: TSrc) -> TDst
    where
        TDst: TryFrom<TSrc>,
        TSrc: WrappingCast<TDst>,
    {
        if !t_fits_in::<TDst, TSrc>(value) {
            self.ok = false;
        }
        value.wrapping_cast()
    }

    /// Saturating `x + y`: `usize::MAX` on overflow.
    // Port of: src/core/SkSafeMath.cpp#L10-L14 (chrome/m156)
    #[doc(alias = "Add")]
    #[must_use]
    pub fn saturating_add(x: usize, y: usize) -> usize {
        let mut tmp = SafeMath::new();
        let sum = tmp.add(x, y);
        if tmp.ok() { sum } else { usize::MAX }
    }

    /// Saturating `x * y`: `usize::MAX` on overflow.
    // Port of: src/core/SkSafeMath.cpp#L16-L20 (chrome/m156)
    #[doc(alias = "Mul")]
    #[must_use]
    pub fn saturating_mul(x: usize, y: usize) -> usize {
        let mut tmp = SafeMath::new();
        let prod = tmp.mul(x, y);
        if tmp.ok() { prod } else { usize::MAX }
    }

    /// Saturating align-up to a multiple of 4.
    // Port of: src/core/SkSafeMath.h#L118-L121 (chrome/m156)
    #[doc(alias = "Align4")]
    #[must_use]
    pub fn align4(x: usize) -> usize {
        let mut safe = SafeMath::new();
        safe.align_up(x, 4)
    }

    // `add` for the 64-bit path of `mul64`, where size_t is uint64_t in Skia.
    // Port of: src/core/SkSafeMath.h#L34-L38 (chrome/m156)
    fn add_u64(&mut self, x: u64, y: u64) -> u64 {
        let result = x.wrapping_add(y);
        self.ok &= result >= x;
        result
    }

    // Port of: src/core/SkSafeMath.h#L124-L131 (chrome/m156)
    fn mul32(&mut self, x: u32, y: u32) -> u32 {
        let bx = u64::from(x);
        let by = u64::from(y);
        let result = bx * by;
        self.ok &= result >> 32 == 0;
        // Overflow information is capture in ok. Return the result modulo 2^32.
        #[allow(clippy::cast_possible_truncation)] // mirrors (uint32_t)result
        let truncated = result as u32;
        truncated
    }

    // Port of: src/core/SkSafeMath.h#L133-L157 (chrome/m156)
    #[allow(clippy::similar_names)] // names mirror Skia's lx_ly/hx_ly/lx_hy/hx_hy
    fn mul64(&mut self, x: u64, y: u64) -> u64 {
        if x <= u64::MAX >> 32 && y <= u64::MAX >> 32 {
            x * y
        } else {
            let hi = |x: u64| x >> 32;
            let lo = |x: u64| x & 0xFFFF_FFFF;

            let lx_ly = lo(x) * lo(y);
            let hx_ly = hi(x) * lo(y);
            let lx_hy = lo(x) * hi(y);
            let hx_hy = hi(x) * hi(y);
            let mut result = self.add_u64(lx_ly, hx_ly << 32);
            result = self.add_u64(result, lx_hy << 32);
            self.ok &= (hx_hy + (hx_ly >> 32) + (lx_hy >> 32)) == 0;

            if cfg!(debug_assertions) {
                let double_check = u128::from(x) * u128::from(y);
                #[allow(clippy::cast_possible_truncation)] // low 64 bits, as & 0xFFFFFFFFFFFFFFFF
                let low = double_check as u64;
                debug_assert_eq!(result, low);
                debug_assert!(!self.ok || (double_check >> 64 == 0));
            }

            result
        }
    }
}

/// `std::gcd` for `usize`.
fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}
