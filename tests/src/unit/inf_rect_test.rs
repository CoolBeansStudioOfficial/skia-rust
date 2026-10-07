// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/InfRectTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::floating_point::{FLOAT_INFINITY, FLOAT_NAN};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{int_to_scalar, scalar};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/InfRectTest.cpp#L18-L24 (chrome/m156)
fn check_invalid(reporter: &mut Reporter, l: scalar, t: scalar, r: scalar, b: scalar) {
    let mut rect = Rect::default();
    rect.set_ltrb(l, t, r, b);
    reporter_assert!(reporter, !rect.is_finite());
}

// Tests that isFinite() will reject any rect with +/-inf values
// as one of its coordinates.
// Port of: tests/InfRectTest.cpp#L26-L45 (chrome/m156)
def_test!(InfRect, |reporter| {
    let inf: f32 = FLOAT_INFINITY;
    let nan: f32 = FLOAT_NAN;
    #[allow(clippy::eq_op, clippy::float_cmp)]
    // NaN is not equal to itself: the point of the assert
    {
        debug_assert!(!(nan == nan));
    }

    let small: scalar = int_to_scalar(10);
    let big: scalar = int_to_scalar(100);

    reporter_assert!(reporter, Rect::new_empty().is_finite());
    let rect = Rect::from_xywh(small, small, big, big);
    reporter_assert!(reporter, rect.is_finite());

    let invalid: [scalar; 3] = [nan, inf, -inf];
    for value in invalid {
        check_invalid(reporter, small, small, big, value);
        check_invalid(reporter, small, small, value, big);
        check_invalid(reporter, small, value, big, big);
        check_invalid(reporter, value, small, big, big);
    }
});

// need tests for SkStrSearch
