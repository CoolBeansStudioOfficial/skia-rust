// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: fuzz/FuzzCubicRoots.cpp (chrome/m156)

use skia_rust_core::cubics;
use skia_rust_core::floating_point::doubles_nearly_equal_ulps;

use crate::def_fuzz;

// Port of: fuzz/FuzzCubicRoots.cpp#L18-L33 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn fuzz_cubic_real_roots(a: f64, b: f64, c: f64, d: f64) {
    let mut roots = [0.0f64; 3];
    let num_solutions = cubics::roots_real(a, b, c, d, &mut roots);
    assert!(num_solutions <= 3);
    for root in &roots[..num_solutions] {
        assert!(root.is_finite());
    }
    // Roots should not be duplicated
    if num_solutions >= 2 {
        assert!(!doubles_nearly_equal_ulps(roots[0], roots[1]));
    }
    if num_solutions == 3 {
        assert!(!doubles_nearly_equal_ulps(roots[1], roots[2]));
        assert!(!doubles_nearly_equal_ulps(roots[0], roots[2]));
    }
}

// Port of: fuzz/FuzzCubicRoots.cpp#L35-L52 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn fuzz_cubic_roots_valid_t(a: f64, b: f64, c: f64, d: f64) {
    let mut roots = [0.0f64; 3];
    let num_solutions = cubics::roots_valid_t(a, b, c, d, &mut roots);
    assert!(num_solutions <= 3);
    for &root in &roots[..num_solutions] {
        assert!(root.is_finite());
        assert!(root >= 0.0);
        assert!(root <= 1.0);
    }
    // Roots should not be duplicated
    if num_solutions >= 2 {
        assert!(!doubles_nearly_equal_ulps(roots[0], roots[1]));
    }
    if num_solutions == 3 {
        assert!(!doubles_nearly_equal_ulps(roots[1], roots[2]));
        assert!(!doubles_nearly_equal_ulps(roots[0], roots[2]));
    }
}

// Port of: fuzz/FuzzCubicRoots.cpp#L54-L74 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn fuzz_cubic_roots_binary_search(a: f64, b: f64, c: f64, d: f64) {
    let mut roots = [0.0f64; 3];
    let num_solutions = cubics::binary_search_roots_valid_t(a, b, c, d, &mut roots);
    assert!(num_solutions <= 3);
    for &root in &roots[..num_solutions] {
        assert!(root.is_finite());
        assert!(root >= 0.0);
        assert!(root <= 1.0);
        let actual = cubics::eval_at(a, b, c, d, root);
        // The binary search algorithm *should* be accurate regardless of the inputs.
        assert!(actual.abs() < 0.001);
    }
    // Roots should not be duplicated
    if num_solutions >= 2 {
        assert!(!doubles_nearly_equal_ulps(roots[0], roots[1]));
    }
    if num_solutions == 3 {
        assert!(!doubles_nearly_equal_ulps(roots[1], roots[2]));
        assert!(!doubles_nearly_equal_ulps(roots[0], roots[2]));
    }
}

// Port of: fuzz/FuzzCubicRoots.cpp#L76-L92 (chrome/m156)
def_fuzz!(CubicRoots, fuzz_CubicRoots, |fuzz| {
    let a: f64 = fuzz.next();
    let b: f64 = fuzz.next();
    let c: f64 = fuzz.next();
    let d: f64 = fuzz.next();

    fuzz_cubic_real_roots(a, b, c, d);

    fuzz_cubic_roots_valid_t(a, b, c, d);

    fuzz_cubic_roots_binary_search(a, b, c, d);
});
