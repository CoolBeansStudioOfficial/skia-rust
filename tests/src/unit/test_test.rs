// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/TestTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_test, reporter_assert};

// This is an example of a normal test. It should not require any GPU backends, that is, it is a
// CPU test.
// Port of: tests/TestTest.cpp#L15-L17 (chrome/m156)
def_test!(TestNormal, |reporter| {
    // C++ asserts that the reporter pointer is set; a `&mut Reporter` always is.
    reporter_assert!(reporter, true);
});

// This is an example of a conditional test with a true condition. The condition `1 == 1` is
// always true, so this is `def_test!`.
// Port of: tests/TestTest.cpp#L20-L22 (chrome/m156), DEF_CONDITIONAL_TEST with `1 == 1`
def_test!(TestTrueCondition, |reporter| {
    reporter_assert!(reporter, true);
});
