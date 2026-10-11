// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: fuzz/FuzzQuadRoots.cpp (chrome/m156)

use skia_rust_core::floating_point::doubles_nearly_equal_ulps;
use skia_rust_core::quads;

use crate::def_fuzz;

// Port of: fuzz/FuzzQuadRoots.cpp#L17-L34 (chrome/m156)
fn fuzz_quad_real_roots(a: f64, b: f64, c: f64) {
    let mut roots = [0.0f64; 2];
    let num_solutions = quads::roots_real(a, b, c, &mut roots);
    assert!(num_solutions <= 2);
    for root in &roots[..num_solutions] {
        assert!(root.is_finite());
        // You may be tempted to add assertions that plug the provided solutions into
        // the quadratic equation and verify that the result is zero. Be advised
        // that the fuzzer is very good at finding float values that result in
        // seemingly arbitrarily large errors, due to the imprecision of floating
        // point math. Unless the input range is sufficiently small, such an
        // effort seems fruitless.
    }
    if num_solutions == 2 {
        // Roots should not be duplicated
        assert!(!doubles_nearly_equal_ulps(roots[0], roots[1]));
    }
}

// Port of: fuzz/FuzzQuadRoots.cpp#L36-L48 (chrome/m156)
def_fuzz!(QuadRoots, fuzz_QuadRoots, |fuzz| {
    let a: f64 = fuzz.next();
    let b: f64 = fuzz.next();
    let c: f64 = fuzz.next();

    fuzz_quad_real_roots(a, b, c);
});
