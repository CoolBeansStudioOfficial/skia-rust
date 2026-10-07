// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SizeTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::scalar::{int_to_scalar, scalar};
use skia_rust_core::size::{ISize, Size};

use crate::{def_test, reporter_assert};

// Port of: tests/SizeTest.cpp#L12-L32 (chrome/m156)
def_test!(ISize, |reporter| {
    let mut a = ISize::default();
    let mut b = ISize::default();

    a.set(0, 0);
    reporter_assert!(reporter, a.is_empty());

    a.set(5, -5);
    reporter_assert!(reporter, a.is_empty());

    a = ISize {
        width: 5,
        height: 0,
    };
    reporter_assert!(reporter, a.is_empty());

    b.set(5, 0);
    reporter_assert!(reporter, a == b);

    a.set(3, 5);
    reporter_assert!(reporter, !a.is_empty());

    b = a;
    reporter_assert!(reporter, !b.is_empty());
    reporter_assert!(reporter, a == b);
    reporter_assert!(reporter, !(a != b));
    reporter_assert!(reporter, a.width == b.width && a.height == b.height);
});

// Port of: tests/SizeTest.cpp#L34-L63 (chrome/m156)
def_test!(Size, |reporter| {
    let mut a = Size::default();
    let mut b = Size::default();
    let ix: i32 = 5;
    let iy: i32 = 3;
    let x: scalar = int_to_scalar(ix);
    let y: scalar = int_to_scalar(iy);

    a.set(0.0, 0.0);
    reporter_assert!(reporter, a.is_empty());

    a.set(x, -x);
    reporter_assert!(reporter, a.is_empty());

    a = Size {
        width: x,
        height: 0.0,
    };
    reporter_assert!(reporter, a.is_empty());

    b.set(x, 0.0);
    reporter_assert!(reporter, a == b);

    a.set(y, x);
    reporter_assert!(reporter, !a.is_empty());

    b = a;
    reporter_assert!(reporter, !b.is_empty());
    reporter_assert!(reporter, a == b);
    reporter_assert!(reporter, !(a != b));
    #[allow(clippy::float_cmp)] // exact comparison, as in the C++ test
    {
        reporter_assert!(reporter, a.width == b.width && a.height == b.height);
    }

    let mut ia = ISize::default();
    ia.set(ix, iy);
    a.set(x, y);
    reporter_assert!(reporter, a.to_round() == ia);
});
