// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/MathTest.cpp (chrome/m156)
//
// Not ported yet (manifest stays `todo`): `Math` (needs SkPoint) and `NeonU16Div255` (ARM NEON
// intrinsics only).

#![cfg(test)]

use skia_rust_core::endian::{
    TEndianSwap16, TEndianSwap32, TEndianSwap64, endian_swap16, endian_swap32, endian_swap64,
};
use skia_rust_core::floating_point::{
    FLOAT_INFINITY, FLOAT_NAN, FLOAT_NEGATIVE_INFINITY, MAX_S32_FITS_IN_FLOAT,
    MAX_S64_FITS_IN_FLOAT, MIN_S32_FITS_IN_FLOAT, MIN_S64_FITS_IN_FLOAT, double_saturate2int,
    float_saturate2int, float_saturate2int64,
};
use skia_rust_core::math::{MAX_S32, MAX_S64, MIN_S32, MIN_S64};
use skia_rust_core::math_priv::{next_pow2, next_size_pow2, pop_count, t_div_mod};
use skia_rust_core::random::Random;
use skia_rust_core::scalar::scalar;
use skia_rust_core::t_pin::t_pin;

use crate::{def_test, reporter_assert};

// Port of: tests/MathTest.cpp#L505-L507 (chrome/m156)
struct PairRec<T> {
    f_yin: T,
    f_yang: T,
}

// Port of: tests/MathTest.cpp#L509-L541 (chrome/m156)
def_test!(TestEndian, |reporter| {
    static G16: [PairRec<u16>; 3] = [
        PairRec {
            f_yin: 0x0,
            f_yang: 0x0,
        },
        PairRec {
            f_yin: 0xFFFF,
            f_yang: 0xFFFF,
        },
        PairRec {
            f_yin: 0x1122,
            f_yang: 0x2211,
        },
    ];
    static G32: [PairRec<u32>; 3] = [
        PairRec {
            f_yin: 0x0,
            f_yang: 0x0,
        },
        PairRec {
            f_yin: 0xFFFF_FFFF,
            f_yang: 0xFFFF_FFFF,
        },
        PairRec {
            f_yin: 0x1122_3344,
            f_yang: 0x4433_2211,
        },
    ];
    static G64: [PairRec<u64>; 3] = [
        PairRec {
            f_yin: 0x0,
            f_yang: 0x0,
        },
        PairRec {
            f_yin: 0xFFFF_FFFF_FFFF_FFFF,
            f_yang: 0xFFFF_FFFF_FFFF_FFFF,
        },
        PairRec {
            f_yin: 0x1122_3344_5566_7788,
            f_yang: 0x8877_6655_4433_2211,
        },
    ];

    reporter_assert!(reporter, 0x1122 == TEndianSwap16::<0x2211>::VALUE);
    reporter_assert!(reporter, 0x1122_3344 == TEndianSwap32::<0x4433_2211>::VALUE);
    reporter_assert!(
        reporter,
        0x1122_3344_5566_7788_u64 == TEndianSwap64::<0x8877_6655_4433_2211>::VALUE
    );

    for rec in &G16 {
        reporter_assert!(reporter, rec.f_yang == endian_swap16(rec.f_yin));
    }
    for rec in &G32 {
        reporter_assert!(reporter, rec.f_yang == endian_swap32(rec.f_yin));
    }
    for rec in &G64 {
        reporter_assert!(reporter, rec.f_yang == endian_swap64(rec.f_yin));
    }
});

// Port of: tests/MathTest.cpp#L621-L630 (chrome/m156)
fn test_nextsizepow2(r: &mut crate::Reporter, test: usize, expected_ans: usize) {
    let ans = next_size_pow2(test);

    reporter_assert!(r, ans == expected_ans);
    //SkDebugf("0x%zx -> 0x%zx (0x%zx)\n", test, ans, expectedAns);
}

// Port of: tests/MathTest.cpp#L632-L666 (chrome/m156)
def_test!(SkNextSizePow2, |reporter| {
    const NUM_SIZE_T_BITS: usize = usize::BITS as usize;

    let mut test: usize = 0;
    let mut expected_ans: usize = 1;

    test_nextsizepow2(reporter, test, expected_ans);

    test = 1;
    expected_ans = 1;

    for _ in 1..NUM_SIZE_T_BITS {
        test_nextsizepow2(reporter, test, expected_ans);

        test += 1;
        expected_ans <<= 1;

        test_nextsizepow2(reporter, test, expected_ans);

        test = expected_ans;
    }

    // For the remaining three tests there is no higher power (of 2)
    test = 0x1;
    test <<= NUM_SIZE_T_BITS - 1;
    test_nextsizepow2(reporter, test, test);

    test += 1;
    test_nextsizepow2(reporter, test, test);

    test_nextsizepow2(reporter, usize::MAX, usize::MAX);
});

// Port of: tests/MathTest.cpp#L668-L692 (chrome/m156)
def_test!(FloatSaturate32, |reporter| {
    struct Rec {
        f_float: f32,
        f_expected_int: i32,
    }
    #[allow(clippy::cast_precision_loss)] // mirrors (float)SK_MaxS32 and SK_MaxS32 * 100.0f
    let recs = [
        Rec {
            f_float: 0.0,
            f_expected_int: 0,
        },
        Rec {
            f_float: 100.5,
            f_expected_int: 100,
        },
        Rec {
            f_float: MAX_S32 as f32,
            f_expected_int: MAX_S32_FITS_IN_FLOAT,
        },
        Rec {
            f_float: MIN_S32 as f32,
            f_expected_int: MIN_S32_FITS_IN_FLOAT,
        },
        Rec {
            f_float: MAX_S32 as f32 * 100.0,
            f_expected_int: MAX_S32_FITS_IN_FLOAT,
        },
        Rec {
            f_float: MIN_S32 as f32 * 100.0,
            f_expected_int: MIN_S32_FITS_IN_FLOAT,
        },
        Rec {
            f_float: FLOAT_INFINITY,
            f_expected_int: MAX_S32_FITS_IN_FLOAT,
        },
        Rec {
            f_float: FLOAT_NEGATIVE_INFINITY,
            f_expected_int: MIN_S32_FITS_IN_FLOAT,
        },
        Rec {
            f_float: FLOAT_NAN,
            f_expected_int: MAX_S32_FITS_IN_FLOAT,
        },
    ];

    for r in &recs {
        let i = float_saturate2int(r.f_float);
        reporter_assert!(reporter, r.f_expected_int == i);

        // Ensure that SkTPin bounds even non-finite values (including NaN)
        let p = t_pin::<scalar>(r.f_float, 0.0, 100.0);
        reporter_assert!(reporter, (0.0..=100.0).contains(&p));
    }
});

// Port of: tests/MathTest.cpp#L694-L714 (chrome/m156)
def_test!(FloatSaturate64, |reporter| {
    struct Rec {
        f_float: f32,
        f_expected64: i64,
    }
    #[allow(clippy::cast_precision_loss)] // mirrors (float)SK_MaxS64 and SK_MaxS64 * 100.0f
    let recs = [
        Rec {
            f_float: 0.0,
            f_expected64: 0,
        },
        Rec {
            f_float: 100.5,
            f_expected64: 100,
        },
        Rec {
            f_float: MAX_S64 as f32,
            f_expected64: MAX_S64_FITS_IN_FLOAT,
        },
        Rec {
            f_float: MIN_S64 as f32,
            f_expected64: MIN_S64_FITS_IN_FLOAT,
        },
        Rec {
            f_float: MAX_S64 as f32 * 100.0,
            f_expected64: MAX_S64_FITS_IN_FLOAT,
        },
        Rec {
            f_float: MIN_S64 as f32 * 100.0,
            f_expected64: MIN_S64_FITS_IN_FLOAT,
        },
        Rec {
            f_float: FLOAT_INFINITY,
            f_expected64: MAX_S64_FITS_IN_FLOAT,
        },
        Rec {
            f_float: FLOAT_NEGATIVE_INFINITY,
            f_expected64: MIN_S64_FITS_IN_FLOAT,
        },
        Rec {
            f_float: FLOAT_NAN,
            f_expected64: MAX_S64_FITS_IN_FLOAT,
        },
    ];

    for r in &recs {
        let i = float_saturate2int64(r.f_float);
        reporter_assert!(reporter, r.f_expected64 == i);
    }
});

// Port of: tests/MathTest.cpp#L716-L748 (chrome/m156)
def_test!(SkNextPow2, |reporter| {
    struct Case {
        f_input: i32,
        f_expected: i32,
    }
    // start off with some easy-to verify cases and some edge cases.
    let cases = [
        // 0 is undefined for the current implementation.
        Case {
            f_input: 1,
            f_expected: 1,
        },
        Case {
            f_input: 2,
            f_expected: 2,
        },
        Case {
            f_input: 3,
            f_expected: 4,
        },
        Case {
            f_input: 4,
            f_expected: 4,
        },
        Case {
            f_input: 5,
            f_expected: 8,
        },
        Case {
            f_input: 1_073_741_822,
            f_expected: 1_073_741_824,
        },
        Case {
            f_input: 1_073_741_823,
            f_expected: 1_073_741_824,
        },
        Case {
            f_input: 1_073_741_824,
            f_expected: 1_073_741_824,
        }, // Anything larger than this will overflow
    ];

    for c in &cases {
        let actual = next_pow2(c.f_input);
        reporter_assert!(
            reporter,
            c.f_expected == actual,
            "SkNextPow2({}) == {} not {}",
            c.f_input,
            actual,
            c.f_expected
        );
        reporter_assert!(reporter, actual == next_pow2(c.f_input));
    }

    // exhaustive search for all the between numbers
    for i in 6..63356 {
        let actual = next_pow2(i);
        // skia-rust: libm (ln, powf)
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        // mirrors the implicit float -> int conversion of the C++ initializer
        let expected = 2f32.powf(((i as f32).ln() / 2f32.ln()).ceil()) as i32;
        reporter_assert!(
            reporter,
            expected == actual,
            "SkNextPow2({}) == {} not {}",
            i,
            actual,
            expected
        );
        reporter_assert!(reporter, actual == next_pow2(i));
    }
});

// Port of: tests/MathTest.cpp#L750-L772 (chrome/m156)
def_test!(DoubleSaturate32, |reporter| {
    struct Rec {
        f_double: f64,
        f_expected_int: i32,
    }
    let recs = [
        Rec {
            f_double: 0.0,
            f_expected_int: 0,
        },
        Rec {
            f_double: 100.5,
            f_expected_int: 100,
        },
        Rec {
            f_double: f64::from(MAX_S32),
            f_expected_int: MAX_S32,
        },
        Rec {
            f_double: f64::from(MIN_S32),
            f_expected_int: MIN_S32,
        },
        Rec {
            f_double: f64::from(MAX_S32 - 1),
            f_expected_int: MAX_S32 - 1,
        },
        Rec {
            f_double: f64::from(MIN_S32 + 1),
            f_expected_int: MIN_S32 + 1,
        },
        Rec {
            f_double: f64::from(MAX_S32) * 100.0,
            f_expected_int: MAX_S32,
        },
        Rec {
            f_double: f64::from(MIN_S32) * 100.0,
            f_expected_int: MIN_S32,
        },
        Rec {
            f_double: f64::from(FLOAT_INFINITY),
            f_expected_int: MAX_S32,
        },
        Rec {
            f_double: f64::from(FLOAT_NEGATIVE_INFINITY),
            f_expected_int: MIN_S32,
        },
        Rec {
            f_double: f64::from(FLOAT_NAN),
            f_expected_int: MAX_S32,
        },
    ];

    for r in &recs {
        let i = double_saturate2int(r.f_double);
        reporter_assert!(reporter, r.f_expected_int == i);
    }
});

// Port of: tests/MathTest.cpp#L369-L407 (chrome/m156)
def_test!(PopCount, |reporter| {
    {
        let test_val: u32 = 0;
        reporter_assert!(reporter, pop_count(test_val) == 0);
    }

    for i in 0..32 {
        let mut test_val: u32 = 0x1 << i;
        reporter_assert!(reporter, pop_count(test_val) == 1);

        test_val ^= 0xFFFF_FFFF;
        reporter_assert!(reporter, pop_count(test_val) == 31);
    }

    {
        let test_val: u32 = 0xFFFF_FFFF;
        reporter_assert!(reporter, pop_count(test_val) == 32);
    }

    let mut rand = Random::default();
    for _ in 0..100 {
        let mut expected_num_set_bits: i32 = 0;
        let mut test_val: u32 = 0;

        let num_tries = rand.next_u_less_than(33);
        for _ in 0..num_tries {
            let bit = rand.next_range_u(0, 31);

            if test_val & (0x1 << bit) != 0 {
                continue;
            }

            expected_num_set_bits += 1;
            test_val |= 0x1 << bit;
        }

        reporter_assert!(reporter, pop_count(test_val) == expected_num_set_bits);
    }
});

// Port of: tests/MathTest.cpp#L556-L596 (chrome/m156)
// The C++ template `test_divmod<T>` is instantiated per integer type with a macro.
macro_rules! test_divmod {
    ($name:ident, $t:ty) => {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // mirrors the (T) casts of the C++
        fn $name(r: &mut crate::Reporter) {
            struct EdgeCases {
                numer: $t,
                denom: $t,
            }
            let k_edge_cases = [
                EdgeCases {
                    numer: 17_i32 as $t,
                    denom: 17_i32 as $t,
                },
                EdgeCases {
                    numer: 17_i32 as $t,
                    denom: 4_i32 as $t,
                },
                EdgeCases {
                    numer: 0_i32 as $t,
                    denom: 17_i32 as $t,
                },
                // For unsigned T these negatives are just some large numbers.
                // Doesn't hurt to test them.
                EdgeCases {
                    numer: -17_i32 as $t,
                    denom: -17_i32 as $t,
                },
                EdgeCases {
                    numer: -17_i32 as $t,
                    denom: 4_i32 as $t,
                },
                EdgeCases {
                    numer: 17_i32 as $t,
                    denom: -4_i32 as $t,
                },
                EdgeCases {
                    numer: -17_i32 as $t,
                    denom: -4_i32 as $t,
                },
            ];

            for edge in &k_edge_cases {
                let numer: $t = edge.numer;
                let denom: $t = edge.denom;
                let (div, mod_) = t_div_mod(numer, denom);
                reporter_assert!(r, numer / denom == div);
                reporter_assert!(r, numer % denom == mod_);
            }

            let mut rand = Random::default();
            for _ in 0..10000_usize {
                let numer: $t = rand.next_s() as $t;
                let mut denom: $t = 0;
                while 0 == denom {
                    denom = rand.next_s() as $t;
                }
                let (div, mod_) = t_div_mod(numer, denom);
                reporter_assert!(r, numer / denom == div);
                reporter_assert!(r, numer % denom == mod_);
            }
        }
    };
}

test_divmod!(test_divmod_u8, u8);
test_divmod!(test_divmod_u16, u16);
test_divmod!(test_divmod_u32, u32);
test_divmod!(test_divmod_u64, u64);
test_divmod!(test_divmod_s8, i8);
test_divmod!(test_divmod_s16, i16);
test_divmod!(test_divmod_s32, i32);
test_divmod!(test_divmod_s64, i64);

// Port of: tests/MathTest.cpp#L598-L600 (chrome/m156)
def_test!(divmod_u8, |r| {
    test_divmod_u8(r);
});

// Port of: tests/MathTest.cpp#L602-L604 (chrome/m156)
def_test!(divmod_u16, |r| {
    test_divmod_u16(r);
});

// Port of: tests/MathTest.cpp#L606-L608 (chrome/m156)
def_test!(divmod_u32, |r| {
    test_divmod_u32(r);
});

// Port of: tests/MathTest.cpp#L610-L612 (chrome/m156)
def_test!(divmod_u64, |r| {
    test_divmod_u64(r);
});

// Port of: tests/MathTest.cpp#L614-L616 (chrome/m156)
def_test!(divmod_s8, |r| {
    test_divmod_s8(r);
});

// Port of: tests/MathTest.cpp#L618-L620 (chrome/m156)
def_test!(divmod_s16, |r| {
    test_divmod_s16(r);
});

// Port of: tests/MathTest.cpp#L622-L624 (chrome/m156)
def_test!(divmod_s32, |r| {
    test_divmod_s32(r);
});

// Port of: tests/MathTest.cpp#L626-L628 (chrome/m156)
def_test!(divmod_s64, |r| {
    test_divmod_s64(r);
});
