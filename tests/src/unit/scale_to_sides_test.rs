// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ScaleToSidesTest.cpp (chrome/m156)

#![cfg(test)]
// The float literals are copied verbatim from the C++ test, digits and all.
#![allow(clippy::unreadable_literal, clippy::excessive_precision)]

use skia_rust_core::floating_point::ieee_double_divide;
use skia_rust_core::rrect::adjust_radii;

use crate::def_test;

// Port of: tests/ScaleToSidesTest.cpp#L16-L77 (chrome/m156)
// The test asserts nothing: it passes if the adjustment never crashes or hits a debug assertion.
def_test!(ScaleToSides, |_reporter| {
    let interesting_values: [f64; 27] = [
        // From sample app - PathFuzzer
        260.01662826538085938,
        63.61007690429687500,
        795.98901367187500000,
        217.71697616577148438,
        686.15960693359375000,
        556.57641601562500000,
        // From skp bitbucket
        111.60000228881836,
        55.800003051757813,
        0.99999996581812677920,
        0.0,
        0.5,
        1.0,
        2.0,
        3.0,
        33.0,
        33554430.0,
        33554431.0,
        33554464.0,
        333333332.0,
        333333333.0,
        333333334.0,
        f64::from(f32::MAX),
        f64::from(f32::EPSILON),
        f64::from(f32::MIN_POSITIVE),
        340282569745034499980078846904281071616.0,
        170141284872517249990039423452140535808.0,
        170141244307698042686698575557637963776.0,
    ];
    let num_interesting_values = interesting_values.len();
    for s in 0..=num_interesting_values {
        for &value_i in &interesting_values {
            for &value_j in &interesting_values {
                for &width in &interesting_values {
                    // We're about to cast values i and j to float, don't bother if they won't fit.
                    // (Is there a more robust way to test this, like SkTFitsIn but double->float?)
                    if value_i > f64::from(f32::MAX) || value_j > f64::from(f32::MAX) {
                        continue;
                    }
                    #[allow(clippy::cast_possible_truncation)]
                    // mirrors the (float) casts in the C++ test
                    let mut radius1 = value_i as f32;
                    #[allow(clippy::cast_possible_truncation)]
                    // mirrors the (float) casts in the C++ test
                    let mut radius2 = value_j as f32;
                    let mut scale =
                        ieee_double_divide(width, f64::from(radius1) + f64::from(radius2));
                    if width > 0.0 {
                        if s != 0 {
                            // std::min(scale, interestingValues[s-1]): the second argument when
                            // it is smaller, else the first.
                            let previous = interesting_values[s - 1];
                            scale = if previous < scale { previous } else { scale };
                        }
                        #[allow(clippy::manual_range_contains)] // the C++ strict bounds, as written
                        if scale < 1.0 && scale > 0.0 {
                            adjust_radii(width, scale, &mut radius1, &mut radius2);
                        }
                    }
                }
            }
        }
    }
});
