// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/Float16Test.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::half::{HALF_1, HALF_MAX, HALF_MIN};
use skia_rust_core::random::Random;
use skia_rust_core::scalar::{SCALAR_INFINITY, SCALAR_NEGATIVE_INFINITY};
use skia_rust_simd::vx::{self, Float2, Float4, Half2, Half4};

use crate::{def_test, reporter_assert};

// float = s[31] e[30:23] m[22:0]
// Port of: tests/Float16Test.cpp#L18-L21 (chrome/m156)
const K_F32_SIGN: u32 = 1 << 31;
const K_F32_EXP: u32 = 255 << 23;
const K_F32_MANT: u32 = !(K_F32_SIGN | K_F32_EXP);
const K_F32_BIAS: i32 = 127;

// half  = s[15] e[14:10] m[9:0]
// Port of: tests/Float16Test.cpp#L24-L27 (chrome/m156)
const K_F16_SIGN: u32 = 1 << 15;
const K_F16_EXP: u32 = 31 << 10;
const K_F16_MANT: u32 = !(K_F16_SIGN | K_F16_EXP);
const K_F16_BIAS: i32 = 15;

// Port of: tests/Float16Test.cpp#L29-L117 (chrome/m156)
def_test!(
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss
    )] // mirrors the integer casts of the C++
    FloatToHalf,
    |r| {
        // Check all 8-bit exponents and all 10-bit upper mantissas, with a combination of all 0s,
        // all 1s, and random bits in the remaining 13 fractional mantissa bits.
        const K_TEST_COUNT: i32 = /*sign*/ 2 * /*exp*/ 255 * /*man*/ 1024 * /*frac*/ 8;
        let mut rand = Random::default();
        for i in 0..K_TEST_COUNT {
            let sign: u32 = ((i & 1) as u32) << 31;
            let exp: u32 = (((i >> 1) & 255) as u32) << 23;
            let man: u32 = (((i >> 9) & 1023) as u32) << 13;
            let frac: u32 = ((i >> 19) & 7) as u32; // 0 and 1 are special, 6 other values are random bits
            let bits: u32 = sign
                | exp
                | man
                | (if frac == 0 {
                    0 // all 0s in lost fraction
                } else if frac == 1 {
                    (1 << 13) - 1 // all 1s in lost fraction
                } else {
                    rand.next_bits(13) // random lost bits
                });

            let f = f32::from_bits(bits);
            if f.is_nan() {
                // We want float->half and half->float to play well with infinities and max
                // representable values in the 16-bit precision, but NaNs should have been caught
                // ahead of time, so the conversion logic is allowed to convert them to infinities
                // in release builds. We skip calling `to_half` in debug since it asserts that NaN
                // isn't passed in.
                #[cfg(not(debug_assertions))]
                {
                    let actual2 = u32::from(vx::to_half(Float2::from_list(&[f]))[0]);
                    let actual4 = u32::from(vx::to_half(Float4::from_list(&[f]))[0]);
                    reporter_assert!(r, (actual2 & K_F16_EXP) == K_F16_EXP);
                    reporter_assert!(r, (actual4 & K_F16_EXP) == K_F16_EXP);
                }
                continue;
            }

            let s32: u32 = bits & K_F32_SIGN;
            let e32: u32 = bits & K_F32_EXP;
            let mut m32: u32 = bits & K_F32_MANT;

            // Half floats can represent a real exponent from -14 to 15. Anything less than that
            // would need to be a denorm, which is flushed to zero, or overflows and becomes
            // infinity.
            let e: i32 = (e32 >> 23) as i32 - K_F32_BIAS; // the true signed exponent

            let s16: u32 = s32 >> 16;
            let mut e16: u32;
            let m16: u32;
            if e < -K_F16_BIAS - 10 || (e == -K_F16_BIAS - 10 && m32 == 0) {
                // Rounds to zero
                e16 = 0;
                m16 = 0;
            } else if (e32 | m32) < 0x38fe_0000 {
                // A subnormal non-zero f16 value
                e16 = 0;
                m16 = 0xffff & (0.5_f32 + f32::from_bits(e32 | m32)).to_bits();
            } else if (e32 | m32) < 0x3880_0000 {
                // Rounds up to smallest normal f16 (2^-14)
                e16 = 1;
                m16 = 0;
            } else if e > K_F16_BIAS {
                // Either f32 infinity or a value larger than what rounds down to the max normal
                // half.
                e16 = K_F16_EXP;
                m16 = 0;
            } else {
                // A normal half value, which is rounded towards nearest even.
                e16 = ((e + K_F16_BIAS) as u32) << 10;
                debug_assert_eq!(e16 & !K_F16_EXP, 0);

                // round to nearest even
                m32 += 0xfff + ((m32 >> 13) & 1);

                if m32 > K_F32_MANT {
                    // overflow
                    e16 += 1 << 10;
                    m16 = 0;
                } else {
                    m16 = m32 >> 13;
                }
            }

            // Expected conversion from f32 to f16
            let expected: u16 = (s16 | e16 | m16) as u16;
            let actual2: u16 = vx::to_half(Float2::from_list(&[f]))[0];
            let actual4: u16 = vx::to_half(Float4::from_list(&[f]))[0];
            reporter_assert!(r, expected == actual2);
            reporter_assert!(r, expected == actual4);
        }
    }
);

// Port of: tests/Float16Test.cpp#L119-L127 (chrome/m156)
def_test!(FloatToHalf_Constants, |r| {
    let to_half = |f: f32| vx::to_half(Float4::from_list(&[f]))[0];
    reporter_assert!(r, 0 == to_half(0.0));
    reporter_assert!(r, K_F16_SIGN == u32::from(to_half(-0.0)));
    reporter_assert!(r, HALF_1 == to_half(1.0));
    reporter_assert!(
        r,
        (K_F16_SIGN | u32::from(HALF_1)) == u32::from(to_half(-1.0))
    );
    reporter_assert!(r, HALF_MAX == to_half(65504.0));
    reporter_assert!(r, HALF_MIN == to_half(1.0 / f32::from(1_u16 << 14)));
});

// Port of: tests/Float16Test.cpp#L129-L172 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)] // the C++ compares floats with ==
    HalfToFloat,
    |r| {
        for bits in 0..=0xffff_u32 {
            let s16: u32 = bits & K_F16_SIGN;
            let e16: u32 = bits & K_F16_EXP;
            let m16: u32 = bits & K_F16_MANT;

            #[allow(clippy::cast_possible_truncation)] // mirrors (uint16_t) bits
            let actual2: f32 = vx::from_half(Half2::from_list(&[bits as u16]))[0];
            #[allow(clippy::cast_possible_truncation)] // mirrors (uint16_t) bits
            let actual4: f32 = vx::from_half(Half4::from_list(&[bits as u16]))[0];

            if e16 == 0 {
                // De-normal f16 or a zero = 2^-14 * 0.[m16] = 2^-14 * 2^-10 * [m16].0
                #[allow(clippy::cast_precision_loss)] // mirrors the implicit uint32_t -> float
                let mut expected: f32 =
                    (1.0 / f32::from(1_u16 << 14)) * (1.0 / f32::from(1_u16 << 10)) * m16 as f32;
                if s16 != 0 {
                    expected *= -1.0;
                }
                reporter_assert!(r, actual2 == expected);
                reporter_assert!(r, actual4 == expected);
            } else if e16 == K_F16_EXP {
                if m16 != 0 {
                    // A NaN stays NaN
                    reporter_assert!(r, actual2.is_nan());
                    reporter_assert!(r, actual4.is_nan());
                } else {
                    // +/- infinity stays infinite
                    if s16 != 0 {
                        reporter_assert!(r, actual2 == SCALAR_NEGATIVE_INFINITY);
                        reporter_assert!(r, actual4 == SCALAR_NEGATIVE_INFINITY);
                    } else {
                        reporter_assert!(r, actual2 == SCALAR_INFINITY);
                        reporter_assert!(r, actual4 == SCALAR_INFINITY);
                    }
                }
            } else {
                // A normal f16 is exactly representable in f32
                let s32: u32 = s16 << 16;
                #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
                // mirrors the int/uint32_t mix in the C++
                let e32: u32 = (((e16 >> 10) as i32 + K_F32_BIAS - K_F16_BIAS) as u32) << 23;
                let m32: u32 = m16 << 13;

                let expected: f32 = f32::from_bits(s32 | e32 | m32);
                reporter_assert!(r, actual2 == expected);
                reporter_assert!(r, actual4 == expected);
            }
        }
    }
);
