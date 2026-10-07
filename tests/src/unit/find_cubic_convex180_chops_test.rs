// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/FindCubicConvex180ChopsTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::geometry::{
    chop_cubic_at_ts, find_cubic_inflections, measure_non_inflect_cubic_rotation,
};
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{SCALAR_NEARLY_ZERO, SCALAR_PI, Scalar, scalar};
use skia_rust_core::tessellation::find_cubic_convex_180_chops;

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/FindCubicConvex180ChopsTest.cpp#L22-L24 (chrome/m156)
fn is_linear3(p0: Point, p1: Point, p2: Point) -> bool {
    (p0 - p1).cross(p2 - p1).nearly_zero(None)
}

// Port of: tests/FindCubicConvex180ChopsTest.cpp#L26-L28 (chrome/m156)
fn is_linear(p: &[Point]) -> bool {
    is_linear3(p[0], p[1], p[2]) && is_linear3(p[0], p[2], p[3]) && is_linear3(p[1], p[2], p[3])
}

// Port of: tests/FindCubicConvex180ChopsTest.cpp#L30-L69 (chrome/m156)
fn check_cubic_convex_180(r: &mut Reporter, p: &[Point]) {
    let mut are_cusps = false;
    let mut inflect_t = [0.0f32; 2];
    let mut convex180_t = [0.0f32; 2];
    let inflect_n = find_cubic_inflections(p, &mut inflect_t);
    if inflect_n != 0 {
        // The curve has inflections. FindCubicConvex180Chops should return the inflection
        // points.
        let convex180_n = find_cubic_convex_180_chops(p, &mut convex180_t, &mut are_cusps);
        reporter_assert!(r, inflect_n == convex180_n);
        if !are_cusps {
            reporter_assert!(
                r,
                inflect_n == 1 || (inflect_t[0] - inflect_t[1]).abs() >= SCALAR_NEARLY_ZERO
            );
        }
        for i in 0..convex180_n {
            reporter_assert!(r, scalar::nearly_equal(inflect_t[i], convex180_t[i], None));
        }
    } else {
        let total_rotation = measure_non_inflect_cubic_rotation(p);
        let convex180_n = find_cubic_convex_180_chops(p, &mut convex180_t, &mut are_cusps);
        let mut chops = [Point::default(); 10];
        chop_cubic_at_ts(p, Some(&mut chops), &convex180_t[..convex180_n]);
        let mut rads_sum = 0.0f32;
        for i in 0..=convex180_n {
            let rads = measure_non_inflect_cubic_rotation(&chops[i * 3..]);
            debug_assert!(rads < SCALAR_PI + SCALAR_NEARLY_ZERO);
            rads_sum += rads;
        }
        if total_rotation < SCALAR_PI - SCALAR_NEARLY_ZERO {
            // The curve should never chop if rotation is <180 degrees.
            reporter_assert!(r, convex180_n == 0);
        } else if !is_linear(p) {
            reporter_assert!(r, scalar::nearly_equal(rads_sum, total_rotation, None));
            if total_rotation > SCALAR_PI + SCALAR_NEARLY_ZERO {
                reporter_assert!(r, convex180_n == 1);
                // This works because cusps take the "inflection" path above, so we don't get
                // non-lilnear curves that lose rotation when chopped.
                reporter_assert!(
                    r,
                    scalar::nearly_equal(
                        measure_non_inflect_cubic_rotation(&chops),
                        SCALAR_PI,
                        None
                    )
                );
                reporter_assert!(
                    r,
                    scalar::nearly_equal(
                        measure_non_inflect_cubic_rotation(&chops[3..]),
                        total_rotation - SCALAR_PI,
                        None
                    )
                );
            }
            reporter_assert!(r, !are_cusps);
        } else {
            reporter_assert!(r, are_cusps);
        }
    }
}

// Port of: tests/FindCubicConvex180ChopsTest.cpp#L71-L129 (chrome/m156)
def_test!(
    #[allow(clippy::items_after_statements, clippy::unreadable_literal)]
    // constants stay next to the C++ code they mirror; literals copied verbatim from the C++ test
    FindCubicConvex180Chops,
    |r| {
        // Test all combinations of corners from the square [0,0,1,1]. This covers every cubic type as
        // well as a wide variety of special cases for cusps, lines, loops, and inflections.
        for i in 0..(1 << 8) {
            let bit = |n: i32| -> f32 { f32::from(u8::from((i >> n) & 1 != 0)) };
            let p = [
                Point::new(bit(0), bit(1)),
                Point::new(bit(2), bit(3)),
                Point::new(bit(4), bit(5)),
                Point::new(bit(6), bit(7)),
            ];
            check_cubic_convex_180(r, &p);
        }

        {
            // This cubic has a convex-180 chop at T=1-"epsilon"
            static HEX_PTS: [u32; 8] = [
                0x3ee0ac74, 0x3f1e061a, 0x3e0fc408, 0x3f457230, 0x3f42ac7c, 0x3f70d76c, 0x3f4e6520,
                0x3f6acafa,
            ];
            let mut p = [Point::default(); 4];
            for (k, pt) in p.iter_mut().enumerate() {
                *pt = Point::new(
                    f32::from_bits(HEX_PTS[k * 2]),
                    f32::from_bits(HEX_PTS[k * 2 + 1]),
                );
            }
            check_cubic_convex_180(r, &p);
        }

        // Now test an exact quadratic.
        let quad = [
            Point::new(0.0, 0.0),
            Point::new(2.0, 2.0),
            Point::new(4.0, 2.0),
            Point::new(6.0, 0.0),
        ];
        let mut t = [0.0f32; 2];
        let mut are_cusps = false;
        reporter_assert!(
            r,
            find_cubic_convex_180_chops(&quad, &mut t, &mut are_cusps) == 0
        );

        // Now test that cusps and near-cusps get flagged as cusps.
        let mut cusp = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(1.0, 0.0),
            Point::new(0.0, 1.0),
        ];
        reporter_assert!(
            r,
            find_cubic_convex_180_chops(&cusp, &mut t, &mut are_cusps) == 1
        );
        reporter_assert!(r, are_cusps);

        // Find the height of the right side of "cusp" at which the distance between its inflection
        // points is kEpsilon (in parametric space).
        const K_EPSILON: f64 = 1.0 / (1 << 11) as f64;
        const K_EPSILON_SQUARED: f64 = K_EPSILON * K_EPSILON;
        let h = (1.0 - K_EPSILON_SQUARED) / (3.0 * K_EPSILON_SQUARED + 1.0);
        let dy = (1.0 - h) / 2.0;
        #[allow(clippy::cast_possible_truncation)] // (float)(1 - dy)
        {
            cusp[1].y = (1.0 - dy) as f32;
            cusp[2].y = (0.0 + dy) as f32;
        }
        reporter_assert!(r, find_cubic_inflections(&cusp, &mut t) == 2);
        #[allow(clippy::cast_possible_truncation)] // (float)kEpsilon
        {
            reporter_assert!(
                r,
                scalar::nearly_equal(t[1] - t[0], K_EPSILON as f32, K_EPSILON_SQUARED as f32)
            );
        }

        // Ensure two inflection points barely more than kEpsilon apart do not get flagged as cusps.
        #[allow(clippy::cast_possible_truncation)] // (float)(1 - 1.1 * dy)
        {
            cusp[1].y = (1.0 - 1.1 * dy) as f32;
            cusp[2].y = (0.0 + 1.1 * dy) as f32;
        }
        reporter_assert!(
            r,
            find_cubic_convex_180_chops(&cusp, &mut t, &mut are_cusps) == 2
        );
        reporter_assert!(r, !are_cusps);

        // Ensure two inflection points barely less than kEpsilon apart do get flagged as cusps.
        #[allow(clippy::cast_possible_truncation)] // (float)(1 - .9 * dy)
        {
            cusp[1].y = (1.0 - 0.9 * dy) as f32;
            cusp[2].y = (0.0 + 0.9 * dy) as f32;
        }
        reporter_assert!(
            r,
            find_cubic_convex_180_chops(&cusp, &mut t, &mut are_cusps) == 1
        );
        reporter_assert!(r, are_cusps);
    }
);
