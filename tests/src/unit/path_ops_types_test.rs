// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsTypesTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_test, reporter_assert};
use skia_rust_pathops::types::roughly_equal_ulps;

// Port of: tests/PathOpsTypesTest.cpp (chrome/m156)
#[allow(clippy::excessive_precision, clippy::unreadable_literal)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
const ROUGHLY_TESTS: [[f64; 2]; 1] = [[5.0402503619650929e-005, 4.3178054475078825e-005]];

def_test!(PathOpsRoughly, |reporter| {
    for test in &ROUGHLY_TESTS {
        let equal = roughly_equal_ulps(test[0], test[1]);
        reporter_assert!(reporter, equal);
    }
});
