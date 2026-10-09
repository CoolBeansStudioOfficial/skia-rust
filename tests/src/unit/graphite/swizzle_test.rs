// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/SwizzleTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_gpu::gpu::swizzle::Swizzle;

use crate::{def_test, reporter_assert};

def_test!(SwizzleInvertTest, |r| {
    // Port of: tests/graphite/SwizzleTest.cpp#L14-L58 (chrome/m156)
    struct Case {
        test: &'static str,
        expected_inverse: &'static str,
        true_inverse: bool,
    }
    let k_swizzle_tests = [
        // These are true inverses since they use each of r,g,b,a exactly once
        Case {
            test: "rgba",
            expected_inverse: "rgba",
            true_inverse: true,
        },
        Case {
            test: "bgra",
            expected_inverse: "bgra",
            true_inverse: true,
        },
        Case {
            test: "garb",
            expected_inverse: "brag",
            true_inverse: true,
        },
        Case {
            test: "argb",
            expected_inverse: "gbar",
            true_inverse: true,
        },
        Case {
            test: "abgr",
            expected_inverse: "abgr",
            true_inverse: true,
        },
        // These are true inverses because the channels with constants are not referenced by the
        // other channels being swizzled
        Case {
            test: "000r",
            expected_inverse: "a001",
            true_inverse: true,
        },
        Case {
            test: "rgb1",
            expected_inverse: "rgb1",
            true_inverse: true,
        },
        Case {
            test: "bgr1",
            expected_inverse: "bgr1",
            true_inverse: true,
        },
        // These are not true inverses because not every channel is used, and the missing
        // channels are not set to a constant 0 or 1
        Case {
            test: "rrr1",
            expected_inverse: "r001",
            true_inverse: false,
        },
        Case {
            test: "rrra",
            expected_inverse: "r00a",
            true_inverse: false,
        },
        // This tests that a non-default constant is preserved if the channel is not in conflict
        // with any other swizzle component (e.g. B=1, but nothing references 'b' in rr11 so
        // preserve B=1 instead of falling back to the default B=0. At the same time, the inverse
        // G = 0 because had set it to 'r' and nothing references 'g' to provide another value).
        Case {
            test: "rr11",
            expected_inverse: "r011",
            true_inverse: false,
        },
        // This one is subtle, because both a000 and a001 map to 000r, but 000r maps back to a001
        Case {
            test: "a000",
            expected_inverse: "000r",
            true_inverse: false,
        },
    ];

    for t in k_swizzle_tests {
        let actual_inverse = Swizzle::new(t.test).invert();
        reporter_assert!(
            r,
            actual_inverse == Swizzle::new(t.expected_inverse),
            "Expected inverse({}) == {}, but was {}",
            t.test,
            t.expected_inverse,
            actual_inverse.as_string()
        );
        if t.true_inverse {
            // For a true inverse, the inverse of the inverse should be the original
            let original = actual_inverse.invert();
            reporter_assert!(
                r,
                Swizzle::new(t.test) == original,
                "Expected inverse(inverse({})) == inverse({}) == {}, but was {}",
                t.test,
                actual_inverse.as_string(),
                t.test,
                original.as_string()
            );
        }
    }
});
