// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/bentleyottmann/tests/Int96Test.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_bentleyottmann::int96::{Int96, multiply};

use crate::{def_test, reporter_assert};

/// `interesting64`: the 64-bit values the tests combine.
const INTERESTING64: [i64; 15] = [
    -i64::MAX,
    -i64::MAX + 1,
    -2_147_483_648,
    -2_147_483_647,
    -2_147_483_646,
    -2,
    -1,
    0,
    1,
    2,
    2_147_483_646,
    2_147_483_647,
    2_147_483_648,
    i64::MAX - 1,
    i64::MAX,
];

def_test!(BO_Int96Basic, |reporter| {
    {
        let t: i32 = 0;
        let z = Int96::from(t);
        reporter_assert!(reporter, z.hi == 0 && z.lo == 0);
    }
    {
        let t: i64 = 0;
        let z = Int96::from(t);
        reporter_assert!(reporter, z.hi == 0 && z.lo == 0);
    }
    {
        let t: i32 = -1;
        let z = Int96::from(t);
        reporter_assert!(reporter, z.hi == -1 && z.lo == 0xFFFF_FFFF);
    }
    {
        let t: i64 = -1;
        let z = Int96::from(t);
        reporter_assert!(reporter, z.hi == -1 && z.lo == 0xFFFF_FFFF);
    }
    {
        let t: i32 = 3;
        let z = Int96::from(t);
        reporter_assert!(reporter, z.hi == 0 && z.lo == 3);
    }
    {
        let t: i64 = 3;
        let z = Int96::from(t);
        reporter_assert!(reporter, z.hi == 0 && z.lo == 3);
    }
    {
        let t: i32 = -3;
        let z = Int96::from(t);
        reporter_assert!(reporter, z.hi == -1 && z.lo == (-3_i32).cast_unsigned());
    }
    {
        let t: i64 = -3;
        let z = Int96::from(t);
        reporter_assert!(reporter, z.hi == -1 && z.lo == (-3_i32).cast_unsigned());
    }
    {
        let t: i64 = 1_i64 << 32;
        let z = Int96::from(t);
        reporter_assert!(reporter, z.hi == 1 && z.lo == 0);
    }
    {
        // -2 << 32 -- without the warnings.
        let t: i64 = -(2_i64 << 32);
        let z = Int96::from(t);
        reporter_assert!(reporter, z.hi == -2 && z.lo == 0);
    }
});

def_test!(BO_Int96Less, |reporter| {
    // The C++ compares against __int128 under `__SIZEOF_INT128__`, which gcc and clang define on
    // every host this port is checked on; `i128` is the same reference.
    for &a in &INTERESTING64 {
        for &b in &INTERESTING64 {
            let a128 = i128::from(a);
            let b128 = i128::from(b);
            let l128 = a128 < b128;
            let g128 = b128 < a128;
            let a96 = Int96::from(a);
            let b96 = Int96::from(b);
            let l96 = a96 < b96;
            let g96 = b96 < a96;
            reporter_assert!(reporter, l128 == l96);
            reporter_assert!(reporter, g128 == g96);
        }
    }
});

def_test!(BO_Int96Add, |reporter| {
    for &a in &INTERESTING64 {
        for &b in &INTERESTING64 {
            let a128 = i128::from(a);
            let b128 = i128::from(b);
            let r128 = a128 + b128;
            let a96 = Int96::from(a);
            let b96 = Int96::from(b);
            let r96 = a96 + b96;
            // Explicitly check the low bits.
            reporter_assert!(reporter, i128::from(r96.lo) == (r128 & 0xFFFF_FFFF));
            // Build a i128 from an Int96.
            let hi128 = i128::from(r96.hi);
            let lo128 = i128::from(r96.lo);
            let all128 = hi128 * 0x1_0000_0000 + lo128;
            reporter_assert!(reporter, r128 == all128);
        }
    }
});

def_test!(BO_Int96Mult, |reporter| {
    let interesting32: [i32; 9] = [
        -i32::MAX,
        -i32::MAX + 1,
        -2,
        -1,
        0,
        1,
        2,
        i32::MAX - 1,
        i32::MAX,
    ];
    for &i64_value in &INTERESTING64 {
        for &i32_value in &interesting32 {
            let a128 = i128::from(i64_value);
            let b128 = i128::from(i32_value);
            let r128 = a128 * b128;
            let r96 = multiply(i64_value, i32_value);
            // Explicitly check the low bits.
            reporter_assert!(reporter, i128::from(r96.lo) == (r128 & 0xFFFF_FFFF));
            // Build a i128 from an Int96.
            let hi128 = i128::from(r96.hi);
            let lo128 = i128::from(r96.lo);
            let all128 = hi128 * 0x1_0000_0000 + lo128;
            reporter_assert!(reporter, r128 == all128);
        }
    }
});
