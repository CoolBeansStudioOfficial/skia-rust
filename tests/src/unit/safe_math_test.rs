// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SafeMathTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::safe_math::SafeMath;

use crate::{def_test, reporter_assert};

// Port of: tests/SafeMathTest.cpp#L14-L33 (chrome/m156)
def_test!(SafeMath_SizeT_Success, |r| {
    let max: usize = usize::MAX;
    let half_max: usize = max >> 1;
    let half_max_plus_1: usize = half_max + 1;
    let mut safe = SafeMath::default();

    reporter_assert!(r, safe.add(half_max, half_max) == 2 * half_max);
    reporter_assert!(r, safe.add(half_max, half_max_plus_1) == max);
    reporter_assert!(r, safe.ok());

    let bits: usize = usize::BITS as usize; // sizeof(size_t) * 8
    let half_bits: usize = bits / 2;
    let sqrt_max: usize = max >> half_bits;
    let sqrt_max_plus_1: usize = sqrt_max + 1;
    safe = SafeMath::default();
    reporter_assert!(r, safe.mul(sqrt_max, sqrt_max) == sqrt_max * sqrt_max);
    reporter_assert!(
        r,
        safe.mul(sqrt_max, sqrt_max_plus_1) == sqrt_max << half_bits
    );
    reporter_assert!(r, safe.ok());
});

// Port of: tests/SafeMathTest.cpp#L35-L59 (chrome/m156)
def_test!(SafeMath_SizeT_Failure, |r| {
    let max: usize = usize::MAX;
    let mut safe = SafeMath::default();

    let _ = safe.add(max, 1);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.add(max, max);
    reporter_assert!(r, !safe.ok());

    let bits: usize = usize::BITS as usize; // sizeof(size_t) * 8
    let half_bits: usize = bits / 2;
    let sqrt_max: usize = max >> half_bits;
    let sqrt_max_plus_1: usize = sqrt_max + 1;
    safe = SafeMath::default();
    let _ = safe.mul(sqrt_max_plus_1, sqrt_max_plus_1);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.mul(max, max);
    reporter_assert!(r, !safe.ok());
});

// Port of: tests/SafeMathTest.cpp#L61-L78 (chrome/m156)
def_test!(SafeMath_Int_Success, |r| {
    const MAX_INT: i32 = i32::MAX;
    const MIN_INT: i32 = i32::MIN;
    let mut safe = SafeMath::default();

    reporter_assert!(r, safe.add_int(MAX_INT, 0) == MAX_INT);
    reporter_assert!(r, safe.add_int(MIN_INT, 1) == MIN_INT + 1);
    reporter_assert!(r, safe.sub_int(MIN_INT, 0) == MIN_INT);
    reporter_assert!(r, safe.sub_int(-1, MIN_INT) == MAX_INT);
    reporter_assert!(r, safe.mul_int(MAX_INT, 1) == MAX_INT);
    reporter_assert!(r, safe.mul_int(MAX_INT, 0) == 0);
    reporter_assert!(r, safe.div_int(6, 3) == 2);
    reporter_assert!(r, safe.mod_int(6, 3) == 0);
    reporter_assert!(r, safe.mod_int(9, 4) == 1);
    reporter_assert!(r, safe.ok());
});

// Port of: tests/SafeMathTest.cpp#L80-L135 (chrome/m156)
def_test!(SafeMath_Int_Failure, |r| {
    const MAX_INT: i32 = i32::MAX;
    const MIN_INT: i32 = i32::MIN;
    const SQRT_MAX_INT: i32 = 46340; // floor(sqrt(std::numeric_limits<int>::max()))
    let mut safe = SafeMath::default();

    let _ = safe.add_int(MAX_INT, 1);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.add_int(MIN_INT, -1);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.sub_int(MIN_INT, 1);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.sub_int(MAX_INT, -1);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.mul_int(MAX_INT, 2);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.mul_int(SQRT_MAX_INT + 1, SQRT_MAX_INT + 1);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.mul_int(MAX_INT, -2);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.mul_int(MIN_INT, 2);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.mul_int(MIN_INT, MIN_INT);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.mul_int(MIN_INT, -1);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.div_int(6, 0);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.div_int(MIN_INT, -1);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.mod_int(6, 0);
    reporter_assert!(r, !safe.ok());

    safe = SafeMath::default();
    let _ = safe.mod_int(MIN_INT, -1);
    reporter_assert!(r, !safe.ok());
});
