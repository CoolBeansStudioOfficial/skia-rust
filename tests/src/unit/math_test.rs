// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/MathTest.cpp (chrome/m156)
//
// Not ported yet (manifest stays `todo`): `NeonU16Div255` (ARM NEON intrinsics only).

#![cfg(test)]

use skia_rust_core::endian::{
    TEndianSwap16, TEndianSwap32, TEndianSwap64, endian_swap16, endian_swap32, endian_swap64,
};
use skia_rust_core::fixed::{
    FIXED_1, Fixed, fixed_ceil_to_fixed, fixed_div, fixed_floor_to_fixed, fixed_round_to_fixed,
};
use skia_rust_core::floating_point::{
    FLOAT_INFINITY, FLOAT_NAN, FLOAT_NEGATIVE_INFINITY, FloatingPoint, MAX_S32_FITS_IN_FLOAT,
    MAX_S64_FITS_IN_FLOAT, MIN_S32_FITS_IN_FLOAT, MIN_S64_FITS_IN_FLOAT, double_saturate2int,
    float_rsqrt, float_saturate2int, float_saturate2int64, is_finite, is_finite_all, is_nan,
};
use skia_rust_core::half::{float_to_half, half_to_float};
use skia_rust_core::math::{
    MAX_S32, MAX_S64, MIN_S32, MIN_S64, NAN32, left_shift_64, mul_div_255_round,
};
use skia_rust_core::math_priv::{
    clz, copy_sign32, mul_div_255_ceiling, mul_div_255_trunc, next_pow2, next_size_pow2, pop_count,
    t_div_mod,
};
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::random::Random;
use skia_rust_core::scalar::{
    SCALAR_1, SCALAR_INFINITY, SCALAR_NAN, Scalar, scalar, scalar_copy_sign, scalar_round_to_int,
};
use skia_rust_core::t_pin::t_pin;

use crate::{Reporter, def_test, errorf, reporter_assert};

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

///////////////////////////////////////////////////////////////////////////////

// Port of: tests/MathTest.cpp#L28-L30 (chrome/m156)
fn sk_fsel(pred: f32, result_ge: f32, result_lt: f32) -> f32 {
    if pred >= 0.0 { result_ge } else { result_lt }
}

// Port of: tests/MathTest.cpp#L32-L36 (chrome/m156)
fn fast_floor(x: f32) -> f32 {
    //    float big = sk_fsel(x, 0x1.0p+23, -0x1.0p+23);
    #[allow(clippy::cast_precision_loss)] // 1 << 23 is exact
    let big = sk_fsel(x, (1 << 23) as f32, -((1 << 23) as f32));
    (x + big) - big
}

// Port of: tests/MathTest.cpp#L38-L40 (chrome/m156)
fn std_floor(x: f32) -> f32 {
    x.floor()
}

// Port of: tests/MathTest.cpp#L42-L48 (chrome/m156)
#[allow(clippy::float_cmp)] // exact float comparisons, as in the C++ test
fn test_floor_value(reporter: &mut Reporter, value: f32) {
    let fast = fast_floor(value);
    let std = std_floor(value);
    if std != fast {
        errorf!(
            reporter,
            "fast_floor({value:.9}) == {fast:.9} != {std:.9} == std_floor({value:.9})"
        );
    }
}

// Port of: tests/MathTest.cpp#L50-L69 (chrome/m156)
fn test_floor(reporter: &mut Reporter) {
    static G_VALS: [f32; 9] = [
        0.0,
        1.0,
        1.1,
        1.01,
        1.001,
        1.0001,
        1.00001,
        1.000_001,
        1.000_000_1,
    ];

    for &value in &G_VALS {
        test_floor_value(reporter, value);
        //        test_floor_value(reporter, -gVals[i]);
    }
}

///////////////////////////////////////////////////////////////////////////////

// Port of: tests/MathTest.cpp#L73-L75 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors the C++ int to float arithmetic
fn float_blend(src: i32, dst: i32, unit: f32) -> f32 {
    dst as f32 + (src - dst) as f32 * unit
}

// Port of: tests/MathTest.cpp#L77-L80 (chrome/m156)
fn blend31(src: i32, dst: i32, a31: i32) -> i32 {
    dst + (((src - dst) * a31 * 2114) >> 16)
    //    return dst + ((src - dst) * a31 * 33 >> 10);
}

// Port of: tests/MathTest.cpp#L82-L86 (chrome/m156)
fn blend31_slow(src: i32, dst: i32, a31: i32) -> i32 {
    let mut prod = src * a31 + (31 - a31) * dst + 16;
    prod = (prod + (prod >> 5)) >> 5;
    prod
}

// Port of: tests/MathTest.cpp#L88-L92 (chrome/m156)
fn blend31_round(src: i32, dst: i32, a31: i32) -> i32 {
    let mut prod = (src - dst) * a31 + 16;
    prod = (prod + (prod >> 5)) >> 5;
    dst + prod
}

// Port of: tests/MathTest.cpp#L94-L97 (chrome/m156)
fn blend31_old(src: i32, dst: i32, mut a31: i32) -> i32 {
    a31 += a31 >> 4;
    dst + (((src - dst) * a31) >> 5)
}

// suppress unused code warning
// Port of: tests/MathTest.cpp#L99-L104 (chrome/m156)
static BLEND_FUNCTIONS: [fn(i32, i32, i32) -> i32; 4] =
    [blend31, blend31_slow, blend31_round, blend31_old];

// Port of: tests/MathTest.cpp#L106-L142 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors the (int) cast
fn test_blend31() {
    let mut failed = 0;
    let mut death = 0;
    if false {
        // avoid bit rot, suppress warning
        failed = (BLEND_FUNCTIONS[0])(0, 0, 0);
    }
    for src in 0..=255 {
        for dst in 0..=255 {
            for a in 0..=31 {
                //                int r0 = blend31(src, dst, a);
                //                int r0 = blend31_round(src, dst, a);
                //                int r0 = blend31_old(src, dst, a);
                let r0 = blend31_slow(src, dst, a);

                #[allow(clippy::cast_precision_loss)] // a / 31.f
                let f = float_blend(src, dst, a as f32 / 31.0);
                let r1 = f as i32;
                let r2 = scalar_round_to_int(f);

                if r0 != r1 && r0 != r2 {
                    eprintln!("src:{src} dst:{dst} a:{a} result:{r0} float:{f}");
                    failed += 1;
                }
                if r0 > 255 {
                    death += 1;
                    eprintln!("death src:{src} dst:{dst} a:{a} result:{r0} float:{f}");
                }
            }
        }
    }
    eprintln!("---- failed {failed} death {death}");
}

// Port of: tests/MathTest.cpp#L144-L151 (chrome/m156)
fn check_length(reporter: &mut Reporter, p: Point, target_len: scalar) {
    let x = p.x;
    let y = p.y;
    let mut len = (x * x + y * y).sqrt();

    len /= target_len;

    reporter_assert!(reporter, len > 0.999 && len < 1.001);
}

// Port of: tests/MathTest.cpp#L153-L186 (chrome/m156)
fn unittest_isfinite<T: FloatingPoint>(reporter: &mut Reporter) {
    let zero = T::ZERO;
    let plain = T::from_u8(123);
    let inf = T::INFINITY;
    let big = T::MAX;
    let nan = inf * zero;

    reporter_assert!(reporter, !is_nan(inf));
    reporter_assert!(reporter, !is_nan(-inf));
    reporter_assert!(reporter, !is_finite(inf));
    reporter_assert!(reporter, !is_finite(-inf));

    reporter_assert!(reporter, is_nan(nan));
    reporter_assert!(reporter, !is_nan(big));
    reporter_assert!(reporter, !is_nan(-big));
    reporter_assert!(reporter, !is_nan(zero));

    reporter_assert!(reporter, !is_finite(nan));
    reporter_assert!(reporter, is_finite(big));
    reporter_assert!(reporter, is_finite(-big));
    reporter_assert!(reporter, is_finite(zero));

    // SkIsFinite supports testing multiple values at once.
    reporter_assert!(reporter, !is_finite_all(inf, &[plain]));
    reporter_assert!(reporter, !is_finite_all(plain, &[-inf]));
    reporter_assert!(reporter, !is_finite_all(nan, &[plain]));
    reporter_assert!(reporter, is_finite_all(plain, &[big]));
    reporter_assert!(reporter, is_finite_all(-big, &[plain]));
    reporter_assert!(reporter, is_finite_all(plain, &[zero]));

    reporter_assert!(reporter, !is_finite_all(inf, &[plain, plain]));
    reporter_assert!(reporter, !is_finite_all(plain, &[-inf, plain]));
    reporter_assert!(reporter, !is_finite_all(plain, &[plain, nan]));
    reporter_assert!(reporter, is_finite_all(big, &[plain, plain]));
    reporter_assert!(reporter, is_finite_all(plain, &[-big, plain]));
    reporter_assert!(reporter, is_finite_all(plain, &[plain, zero]));
}

// Port of: tests/MathTest.cpp#L188-L242 (chrome/m156)
fn unittest_half(reporter: &mut Reporter) {
    static G_FLOATS: [f32; 12] = [
        0.0,
        1.0,
        0.5,
        0.499_999,
        0.500_000_1,
        1.0 / 3.0,
        -0.0,
        -1.0,
        -0.5,
        -0.499_999,
        -0.500_000_1,
        -1.0 / 3.0,
    ];

    for &g_float in &G_FLOATS {
        let h = float_to_half(g_float);
        let f = half_to_float(h);
        reporter_assert!(reporter, scalar::nearly_equal(f, g_float, None));
    }

    // check some special values
    let largest_positive_half = f32::from_bits((142 << 23) | (1023 << 13));
    let mut h = float_to_half(largest_positive_half);
    let mut f = half_to_float(h);
    reporter_assert!(
        reporter,
        scalar::nearly_equal(f, largest_positive_half, None)
    );

    let largest_negative_half = f32::from_bits((1u32 << 31) | (142u32 << 23) | (1023u32 << 13));
    h = float_to_half(largest_negative_half);
    f = half_to_float(h);
    reporter_assert!(
        reporter,
        scalar::nearly_equal(f, largest_negative_half, None)
    );

    let smallest_positive_half = f32::from_bits(102 << 23);
    h = float_to_half(smallest_positive_half);
    f = half_to_float(h);
    reporter_assert!(
        reporter,
        scalar::nearly_equal(f, smallest_positive_half, None)
    );

    let overflow_half = f32::from_bits((143 << 23) | (1023 << 13));
    h = float_to_half(overflow_half);
    f = half_to_float(h);
    reporter_assert!(reporter, !is_finite(f));

    let underflow_half = f32::from_bits(101 << 23);
    h = float_to_half(underflow_half);
    f = half_to_float(h);
    reporter_assert!(reporter, f == 0.0);

    let inf32 = f32::from_bits(255 << 23);
    h = float_to_half(inf32);
    f = half_to_float(h);
    reporter_assert!(reporter, !is_finite(f));

    let nan32 = f32::from_bits((255 << 23) | 1);
    h = float_to_half(nan32);
    f = half_to_float(h);
    reporter_assert!(reporter, is_nan(f));
}

// Port of: tests/MathTest.cpp#L244-L279 (chrome/m156)
#[allow(clippy::neg_cmp_op_on_partial_ord)] // inside reporter_assert!'s `!(cond)`
fn test_rsqrt(reporter: &mut Reporter, rsqrt: impl Fn(f32) -> f32) {
    let max_relative_error = 6.501_967e-4_f32;

    // test close to 0 up to 1
    let mut input = 0.000_001_f32;
    for _ in 0..1000 {
        let exact = 1.0f32 / input.sqrt();
        let estimate = rsqrt(input);
        let relative_error = (exact - estimate).abs() / exact;
        reporter_assert!(reporter, relative_error <= max_relative_error);
        input += 0.001;
    }

    // test 1 to ~100
    input = 1.0;
    for _ in 0..1000 {
        let exact = 1.0f32 / input.sqrt();
        let estimate = rsqrt(input);
        let relative_error = (exact - estimate).abs() / exact;
        reporter_assert!(reporter, relative_error <= max_relative_error);
        input += 0.01;
    }

    // test some big numbers
    input = 1_000_000.0;
    for _ in 0..100 {
        let exact = 1.0f32 / input.sqrt();
        let estimate = rsqrt(input);
        let relative_error = (exact - estimate).abs() / exact;
        reporter_assert!(reporter, relative_error <= max_relative_error);
        input += 754_326.0;
    }
}

// Port of: tests/MathTest.cpp#L281-L301 (chrome/m156)
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)] // mirrors the C++ int/float conversions
fn test_muldiv255(reporter: &mut Reporter) {
    for a in 0u32..=255 {
        for b in 0u32..=255 {
            let ab = a * b;
            let s = ab as f32 / 255.0f32;
            let round = (s + 0.5f32).floor() as u32;
            let trunc = s.floor() as u32;

            let iround = mul_div_255_round(a, b);
            let itrunc = mul_div_255_trunc(a, b);

            reporter_assert!(reporter, iround == round);
            reporter_assert!(reporter, itrunc == trunc);

            reporter_assert!(reporter, itrunc <= iround);
            reporter_assert!(reporter, iround <= a);
            reporter_assert!(reporter, iround <= b);
        }
    }
}

// Port of: tests/MathTest.cpp#L303-L314 (chrome/m156)
#[allow(clippy::manual_div_ceil)] // mirrors the C++ expression
fn test_muldiv255ceiling(reporter: &mut Reporter) {
    for c in 0u32..=255 {
        for a in 0u32..=255 {
            let product = c * a + 255;
            let expected_ceiling = (product + (product >> 8)) >> 8;
            let webkit_ceiling = (c * a + 254) / 255;
            reporter_assert!(reporter, expected_ceiling == webkit_ceiling);
            let skia_ceiling = mul_div_255_ceiling(c, a);
            reporter_assert!(reporter, skia_ceiling == webkit_ceiling);
        }
    }
}

// Port of: tests/MathTest.cpp#L316-L352 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // small integers
#[allow(clippy::float_cmp)] // exact float comparisons, as in the C++ test
fn test_copysign(reporter: &mut Reporter) {
    static G_TRIPLES: [i32; 27] = [
        // x, y, expected result
        0, 0, 0, //
        0, 1, 0, //
        0, -1, 0, //
        1, 0, 1, //
        1, 1, 1, //
        1, -1, -1, //
        -1, 0, 1, //
        -1, 1, 1, //
        -1, -1, -1, //
    ];
    for triple in G_TRIPLES.chunks(3) {
        reporter_assert!(reporter, copy_sign32(triple[0], triple[1]) == triple[2]);
        let x = triple[0] as f32;
        let y = triple[1] as f32;
        let expected = triple[2] as f32;
        reporter_assert!(reporter, x.copysign(y) == expected);
    }

    let mut rand = Random::default();
    for _j in 0..1000 {
        let ix = rand.next_s();
        // wrapping_neg: C++ negation of INT_MIN wraps in practice
        reporter_assert!(reporter, copy_sign32(ix, ix) == ix);
        reporter_assert!(
            reporter,
            copy_sign32(ix, ix.wrapping_neg()) == ix.wrapping_neg()
        );
        reporter_assert!(reporter, copy_sign32(ix.wrapping_neg(), ix) == ix);
        reporter_assert!(
            reporter,
            copy_sign32(ix.wrapping_neg(), ix.wrapping_neg()) == ix.wrapping_neg()
        );

        let sx = rand.next_s_scalar1();
        reporter_assert!(reporter, scalar_copy_sign(sx, sx) == sx);
        reporter_assert!(reporter, scalar_copy_sign(sx, -sx) == -sx);
        reporter_assert!(reporter, scalar_copy_sign(-sx, sx) == sx);
        reporter_assert!(reporter, scalar_copy_sign(-sx, -sx) == -sx);
    }
}

// Port of: tests/MathTest.cpp#L354-L367 (chrome/m156)
fn huge_vector_normalize(reporter: &mut Reporter) {
    // these values should fail (overflow/underflow) trying to normalize
    let fail = [
        Vector::new(0.0, 0.0),
        Vector::new(SCALAR_INFINITY, 0.0),
        Vector::new(0.0, SCALAR_INFINITY),
        Vector::new(0.0, SCALAR_NAN),
        Vector::new(SCALAR_NAN, 0.0),
    ];
    for mut v in fail {
        let mut v2 = v;
        if v2.set_length(1.0) {
            reporter_assert!(reporter, !v.set_length(1.0));
        }
    }
}

// Port of: tests/MathTest.cpp#L409-L419 (chrome/m156)
fn test_clz(reporter: &mut Reporter) {
    let expect = |reporter: &mut Reporter, value: u32, count: i32| {
        reporter_assert!(reporter, clz(value) == count);
    };

    expect(reporter, 0, 32);
    for i in 0..32 {
        let value: u32 = 0xFFFF_FFFF;
        expect(reporter, value >> i, i);
    }
}

// Port of: tests/MathTest.cpp#L421-L503 (chrome/m156)
def_test!(
    #[allow(clippy::cast_sign_loss)] // mirrors the C++ casts
    Math,
    |reporter| {
        let mut rand = Random::default();

        // these should assert
        // (disabled in the C++ with `#if 0`: SkToS8(128), SkToS8(-129), SkToU8(256), SkToU8(-5), ...)

        test_muldiv255(reporter);
        test_muldiv255ceiling(reporter);
        test_copysign(reporter);

        {
            let x: scalar = SCALAR_NAN;
            reporter_assert!(reporter, is_nan(x));
        }

        for _ in 0..10000 {
            let mut p = Point::default();

            // These random values are being treated as 32-bit-patterns, not as
            // ints; calling SkIntToScalar() here produces crashes.
            #[allow(clippy::cast_precision_loss)] // mirrors (SkScalar)rand.nextS()
            {
                let x = rand.next_s() as scalar;
                let y = rand.next_s() as scalar;
                p.set_length_xy(x, y, SCALAR_1);
            }
            check_length(reporter, p, SCALAR_1);
            #[allow(clippy::cast_precision_loss)] // mirrors (SkScalar)(rand.nextS() >> 13)
            {
                let x = (rand.next_s() >> 13) as scalar;
                let y = (rand.next_s() >> 13) as scalar;
                p.set_length_xy(x, y, SCALAR_1);
            }
            check_length(reporter, p, SCALAR_1);
        }

        {
            let mut result = fixed_div(100, 100);
            reporter_assert!(reporter, result == FIXED_1);
            result = fixed_div(1, FIXED_1);
            reporter_assert!(reporter, result == 1);
            result = fixed_div(10 - 1, FIXED_1 * 3);
            reporter_assert!(reporter, result == 3);
        }

        {
            reporter_assert!(
                reporter,
                (fixed_round_to_fixed(-FIXED_1 * 10) >> 1) == -FIXED_1 * 5
            );
            reporter_assert!(
                reporter,
                (fixed_floor_to_fixed(-FIXED_1 * 10) >> 1) == -FIXED_1 * 5
            );
            reporter_assert!(
                reporter,
                (fixed_ceil_to_fixed(-FIXED_1 * 10) >> 1) == -FIXED_1 * 5
            );
        }

        huge_vector_normalize(reporter);
        unittest_isfinite::<f32>(reporter);
        unittest_isfinite::<f64>(reporter);
        unittest_half(reporter);
        test_rsqrt(reporter, float_rsqrt);

        for _ in 0..10000 {
            let numer: Fixed = rand.next_s();
            let denom: Fixed = rand.next_s();
            let result = fixed_div(numer, denom);
            let mut check: i64 = left_shift_64(i64::from(numer), 16) / i64::from(denom);

            let _ = clz(numer as u32);
            let _ = clz(denom as u32);

            reporter_assert!(reporter, result != NAN32);
            if check > i64::from(MAX_S32) {
                check = i64::from(MAX_S32);
            } else if check < -i64::from(MAX_S32) {
                check = i64::from(MIN_S32);
            }
            #[allow(clippy::cast_possible_truncation)] // mirrors (int32_t)check
            if result != check as i32 {
                errorf!(
                    reporter,
                    "\nFixed Divide: {:8x} / {:8x} -> {:8x} {:8x}\n",
                    numer as u32,
                    denom as u32,
                    result as u32,
                    check as u64
                );
            }
            #[allow(clippy::cast_possible_truncation)] // mirrors (int32_t)check
            {
                reporter_assert!(reporter, result == check as i32);
            }
        }

        if false {
            test_floor(reporter);
        }

        // disable for now
        if false {
            test_blend31(); // avoid bit rot, suppress warning
        }

        test_clz(reporter);
    }
);
