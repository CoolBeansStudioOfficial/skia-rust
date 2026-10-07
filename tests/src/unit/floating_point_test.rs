// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/FloatingPointTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::floating_point::{
    double_nearly_zero, doubles_nearly_equal_ulps, float_midpoint, is_finite,
};

use crate::{def_test, reporter_assert};

// Port of: tests/FloatingPointTest.cpp#L20-L45 (chrome/m156)
def_test!(DoubleNearlyZero, |reporter| {
    reporter_assert!(reporter, double_nearly_zero(0.));
    reporter_assert!(reporter, double_nearly_zero(-0.));
    reporter_assert!(reporter, double_nearly_zero(f64::EPSILON));
    reporter_assert!(reporter, double_nearly_zero(-f64::EPSILON));

    let nearly = 1. / 20_000_000_000_f64 /* 20000000000LL */;
    #[allow(clippy::float_cmp)] // exact comparison, as in the C++ test
    {
        reporter_assert!(reporter, nearly != 0.0);
    }
    reporter_assert!(reporter, double_nearly_zero(nearly));
    reporter_assert!(reporter, double_nearly_zero(-nearly));

    reporter_assert!(reporter, !double_nearly_zero(f64::from(f32::EPSILON)));
    reporter_assert!(reporter, !double_nearly_zero(-f64::from(f32::EPSILON)));
    reporter_assert!(reporter, !double_nearly_zero(1.0));
    reporter_assert!(reporter, !double_nearly_zero(-1.0));
    reporter_assert!(reporter, !double_nearly_zero(f64::from(f32::INFINITY))); // INFINITY
    reporter_assert!(reporter, !double_nearly_zero(f64::from(f32::INFINITY))); // HUGE_VALF
    reporter_assert!(reporter, !double_nearly_zero(f64::INFINITY)); // HUGE_VAL
    reporter_assert!(reporter, !double_nearly_zero(f64::INFINITY)); // HUGE_VALL (long double)
    reporter_assert!(reporter, !double_nearly_zero(-f64::from(f32::INFINITY))); // -INFINITY
    reporter_assert!(reporter, !double_nearly_zero(-f64::from(f32::INFINITY))); // -HUGE_VALF
    reporter_assert!(reporter, !double_nearly_zero(-f64::INFINITY)); // -HUGE_VAL
    reporter_assert!(reporter, !double_nearly_zero(-f64::INFINITY)); // -HUGE_VALL (long double)
    reporter_assert!(reporter, !double_nearly_zero(f64::from(f32::NAN))); // NAN
    reporter_assert!(reporter, !double_nearly_zero(-f64::from(f32::NAN))); // -NAN
});

// Port of: tests/FloatingPointTest.cpp#L47-L109 (chrome/m156)
def_test!(DoubleNearlyEqualUlps, |reporter| {
    let flt_epsilon = f64::from(f32::EPSILON);
    let inf = f64::from(f32::INFINITY); // INFINITY
    let nan = f64::from(f32::NAN); // NAN

    // Our tolerance is looser than DBL_EPSILON
    reporter_assert!(reporter, doubles_nearly_equal_ulps(1., 1.));
    reporter_assert!(reporter, doubles_nearly_equal_ulps(1., 1. - f64::EPSILON));
    reporter_assert!(reporter, doubles_nearly_equal_ulps(1., 1. + f64::EPSILON));
    reporter_assert!(reporter, doubles_nearly_equal_ulps(100.5, 100.5));
    reporter_assert!(
        reporter,
        doubles_nearly_equal_ulps(100.5, 100.5 - f64::EPSILON)
    );
    reporter_assert!(
        reporter,
        doubles_nearly_equal_ulps(100.5, 100.5 + f64::EPSILON)
    );

    // Our tolerance is tighter than FLT_EPSILON
    reporter_assert!(reporter, !doubles_nearly_equal_ulps(1., 1. - flt_epsilon));
    reporter_assert!(reporter, !doubles_nearly_equal_ulps(1., 1. + flt_epsilon));
    reporter_assert!(
        reporter,
        !doubles_nearly_equal_ulps(100.5, 100.5 - flt_epsilon)
    );
    reporter_assert!(
        reporter,
        !doubles_nearly_equal_ulps(100.5, 100.5 + flt_epsilon)
    );
    reporter_assert!(reporter, !doubles_nearly_equal_ulps(0., 0.1));
    reporter_assert!(reporter, !doubles_nearly_equal_ulps(flt_epsilon, 0.));

    reporter_assert!(reporter, doubles_nearly_equal_ulps(inf, inf));
    reporter_assert!(reporter, !doubles_nearly_equal_ulps(inf, 10.));
    reporter_assert!(reporter, !doubles_nearly_equal_ulps(10., inf));
    reporter_assert!(reporter, !doubles_nearly_equal_ulps(nan, inf));

    reporter_assert!(reporter, !doubles_nearly_equal_ulps(inf, -inf));
    reporter_assert!(reporter, !doubles_nearly_equal_ulps(-inf, inf));
    reporter_assert!(reporter, doubles_nearly_equal_ulps(-inf, -inf));

    // Test values upto the edge of infinity.
    let biggest = f64::MAX;
    let almost_biggest = |n: i32| {
        let mut almost_biggest = biggest;
        for _ in 0..n {
            almost_biggest = almost_biggest.next_down(); // std::nextafter(x, -INFINITY)
        }
        almost_biggest
    };
    let next_biggest = almost_biggest(1);
    reporter_assert!(reporter, doubles_nearly_equal_ulps(biggest, next_biggest));
    reporter_assert!(
        reporter,
        doubles_nearly_equal_ulps(biggest, almost_biggest(16))
    );
    reporter_assert!(
        reporter,
        !doubles_nearly_equal_ulps(biggest, almost_biggest(17))
    );

    // One ulp less would be infinity.
    #[allow(clippy::unusual_byte_groupings)] // sign | exponent | mantissa, as in the C++ literal
    let smallest_nan_pattern: u64 =
        0b0_11111111111_0000000000000000000000000000000000000000000000000001;
    let smallest_nan = f64::from_bits(smallest_nan_pattern);
    debug_assert!(smallest_nan.is_nan()); // SkASSERT(std::isnan(smallestNAN))
    #[allow(clippy::float_cmp)] // exact comparison, as in the C++ test
    {
        debug_assert_ne!(biggest, next_biggest);
    }

    // Sanity check.
    reporter_assert!(reporter, !doubles_nearly_equal_ulps(smallest_nan, nan));

    // Make sure to return false along the edge of infinity.
    reporter_assert!(reporter, !doubles_nearly_equal_ulps(inf, biggest));
    reporter_assert!(reporter, !doubles_nearly_equal_ulps(smallest_nan, biggest));
    reporter_assert!(reporter, !doubles_nearly_equal_ulps(smallest_nan, inf));

    let smallest = f64::from_bits(1); // std::numeric_limits<double>::denorm_min()
    reporter_assert!(reporter, !doubles_nearly_equal_ulps(nan, nan));
    reporter_assert!(reporter, doubles_nearly_equal_ulps(smallest, 0.));
    reporter_assert!(reporter, doubles_nearly_equal_ulps(smallest, -smallest));
    reporter_assert!(
        reporter,
        doubles_nearly_equal_ulps(8. * smallest, -8. * smallest)
    );
    reporter_assert!(
        reporter,
        !doubles_nearly_equal_ulps(8. * smallest, -9. * smallest)
    );
});

// Port of: tests/FloatingPointTest.cpp#L111-L131 (chrome/m156)
def_test!(BitCastDoubleRoundTrip, |reporter| {
    let test_cases: [f64; 5] = [0.0, 1.0, -13.0, 1.234_567_890_123_456, -543_210.987_654_321];

    for &input in &test_cases {
        let bits = input.to_bits(); // sk_bit_cast<uint64_t>(input)
        let output = f64::from_bits(bits); // sk_bit_cast<double>(bits)
        #[allow(clippy::float_cmp)] // exact round-trip comparison, as in the C++ test
        {
            reporter_assert!(
                reporter,
                input == output,
                "{:.16} is not exactly {:.16}",
                input,
                output
            );
        }
    }

    {
        let bits = f64::NAN.to_bits();
        let output = f64::from_bits(bits);
        reporter_assert!(reporter, output.is_nan(), "{:.16} is not nan", output);
    }
    {
        let bits = f64::from(f32::INFINITY).to_bits();
        let output = f64::from_bits(bits);
        reporter_assert!(
            reporter,
            !is_finite(output),
            "{:.16} is not infinity",
            output
        );
    }
});

// Port of: tests/FloatingPointTest.cpp#L133-L150 (chrome/m156)
def_test!(FMA, |reporter| {
    // 0b0'01111111111'00'0000000000'0000000000'0000000010'0000000000'0000000000
    let over1: f64 = 1. + 4.656_612_873e-10;

    // 0b0'01111111110'11'1111111111'1111111111'1111111100'0000000000'0000000000
    let under1: f64 = 1. - 4.656_612_873e-10;

    // Precision loss
    //                         -------------- becomes 1; extra bits are rounded off.
    let x = 1f64.mul_add(-1., over1 * under1); // std::fma(1, -1, over1 * under1)

    // Precision maintained
    //                  ------------- becomes 1 - 2^-62; extra bits are maintained
    let y = over1.mul_add(under1, -1.); // std::fma(over1, under1, -1)

    #[allow(clippy::float_cmp)] // exact comparison, as in the C++ test
    {
        reporter_assert!(reporter, x == 0.);
        reporter_assert!(reporter, y == -(-62f64).exp2());
    }
});

// Port of: tests/FloatingPointTest.cpp#L152-L161 (chrome/m156)
def_test!(Midpoint, |reporter| {
    let smallest = f32::from_bits(1); // std::numeric_limits<float>::denorm_min()
    #[allow(clippy::float_cmp)] // exact comparison, as in the C++ test
    {
        reporter_assert!(reporter, float_midpoint(smallest, smallest) == smallest);
        reporter_assert!(reporter, float_midpoint(smallest, -smallest) == 0.);

        let biggest = f32::MAX;
        reporter_assert!(reporter, float_midpoint(biggest, biggest) == biggest);
        reporter_assert!(reporter, float_midpoint(biggest, -biggest) == 0.);
    }
});
