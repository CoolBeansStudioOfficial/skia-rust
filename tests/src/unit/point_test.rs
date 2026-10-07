// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PointTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::floating_point::is_finite;
use skia_rust_core::point::Point;
use skia_rust_core::point::point_priv::{as_scalars, can_normalize, set_length_fast};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{
    SCALAR_1, SCALAR_INFINITY, SCALAR_NAN, Scalar, int_to_scalar, scalar, scalar_invert,
};

// Port of: tests/PointTest.cpp#L26-L35 (chrome/m156)
fn test_casts(reporter: &mut Reporter) {
    let p = Point::new(0.0, 0.0);
    let r = Rect::new(0.0, 0.0, 0.0, 0.0);

    // skia-rust: not expressible in Rust: the C++ compares the *addresses* returned by
    // SkPointPriv::AsScalars and SkRect::asScalars with a reinterpret_cast of the object; the
    // safe Rust versions return copies, so the observable property checked here is their
    // contents.
    #[allow(clippy::float_cmp)] // exact comparison, as in the C++ test
    {
        reporter_assert!(reporter, as_scalars(&p) == [p.x, p.y]);
        reporter_assert!(
            reporter,
            r.as_scalars() == [r.left, r.top, r.right, r.bottom]
        );
    }
}

// Tests SkPoint::Normalize() for this (x,y)
// Port of: tests/PointTest.cpp#L37-L48 (chrome/m156)
fn test_normalize(reporter: &mut Reporter, x: scalar, y: scalar) {
    let mut point = Point::default();
    point.set(x, y);
    let old_length = point.length();

    let returned = Point::normalize_vector(&mut point);
    let new_length = point.length();

    reporter_assert!(reporter, scalar::nearly_equal(returned, old_length, None));
    reporter_assert!(reporter, scalar::nearly_equal(new_length, SCALAR_1, None));
}

// Port of: tests/PointTest.cpp#L50-L66 (chrome/m156)
fn test_normalize_cannormalize_consistent(reporter: &mut Reporter) {
    let values: [scalar; 6] = [1.0, 1e18, 1e20, 1e38, SCALAR_INFINITY, SCALAR_NAN];
    for val in values {
        let variants: [scalar; 4] = [val, -val, scalar_invert(val), -scalar_invert(val)];
        for v in variants {
            let pts: [Point; 5] = [
                Point::new(0.0, v),
                Point::new(v, 0.0),
                Point::new(1.0, v),
                Point::new(v, 1.0),
                Point::new(v, v),
            ];
            for mut p in pts {
                let can = can_normalize(p.x, p.y);
                let nor = p.normalize();
                reporter_assert!(reporter, can == nor);
            }
        }
    }
}

// Tests that SkPoint::length() and SkPoint::Length() both return
// approximately expectedLength for this (x,y).
// Port of: tests/PointTest.cpp#L68-L82 (chrome/m156)
fn test_length(reporter: &mut Reporter, x: scalar, y: scalar, expected_length: scalar) {
    let mut point = Point::default();
    point.set(x, y);

    let s1 = point.length();
    let s2 = Point::length_xy(x, y);

    // The following should be exactly the same, but need not be.
    // See http://gcc.gnu.org/bugzilla/show_bug.cgi?id=323
    reporter_assert!(reporter, scalar::nearly_equal(s1, s2, None));

    reporter_assert!(reporter, scalar::nearly_equal(s1, expected_length, None));

    test_normalize(reporter, x, y);
}

// Ugh. Windows compiler can dive into other .cpp files, and sometimes
// notices that I will generate an overflow... which is exactly the point
// of this test!
//
// To avoid this warning, I need to convince the compiler that I might not
// use that big value, hence this hacky helper function: reporter is
// ALWAYS non-null. (shhhhhh, don't tell the compiler that).
// Port of: tests/PointTest.cpp#L84-L96 (chrome/m156)
fn get_value<T>(_reporter: &Reporter, value: T) -> T {
    value
}

// On linux gcc, 32bit, we are seeing the compiler propagate up the value
// of SkPoint::length() as a double (which we use sometimes to avoid overflow
// during the computation), even though the signature says float (SkScalar).
//
// force_as_float is meant to capture our latest technique (horrible as
// it is) to force the value to be a float, so we can test whether it was
// finite or not.
// Port of: tests/PointTest.cpp#L98-L115 (chrome/m156)
fn force_as_float(_reporter: &Reporter, value: f32) -> f32 {
    let storage: u32 = value.to_bits();
    f32::from_bits(storage)
}

// test that we handle very large values correctly. i.e. that we can
// successfully normalize something whose mag overflows a float.
// Port of: tests/PointTest.cpp#L117-L131 (chrome/m156)
fn test_overflow(reporter: &mut Reporter) {
    let big_float: scalar = get_value(reporter, 3.4e38_f32);
    let mut pt = Point::new(big_float, big_float);
    let mut length = pt.length();
    length = force_as_float(reporter, length);

    // expect this to be non-finite, but dump the results if not.
    if is_finite(length) {
        eprintln!("length({}, {}) == {}", pt.x, pt.y, length);
        reporter_assert!(reporter, !is_finite(length));
    }

    // this should succeed, even though we can't represent length
    reporter_assert!(reporter, pt.set_length(SCALAR_1));

    // now that pt is normalized, we check its length
    length = pt.length();
    reporter_assert!(reporter, scalar::nearly_equal(length, SCALAR_1, None));
}

// Port of: tests/PointTest.cpp#L133-L152 (chrome/m156)
def_test!(Point, |reporter| {
    #[allow(clippy::struct_field_names)] // mirrors the C++ struct's fX/fY/fLength
    struct GRec {
        f_x: scalar,
        f_y: scalar,
        f_length: scalar,
    }

    test_casts(reporter);

    let g_rec = [
        GRec {
            f_x: int_to_scalar(3),
            f_y: int_to_scalar(4),
            f_length: int_to_scalar(5),
        },
        GRec {
            f_x: 0.6,
            f_y: 0.8,
            f_length: SCALAR_1,
        },
    ];

    for rec in &g_rec {
        test_length(reporter, rec.f_x, rec.f_y, rec.f_length);
    }

    test_overflow(reporter);
    test_normalize_cannormalize_consistent(reporter);
});

// Port of: tests/PointTest.cpp#L154-L173 (chrome/m156)
def_test!(Point_setLengthFast, |reporter| {
    // Scale a (1,1) point to a bunch of different lengths,
    // making sure the slow and fast paths are within 0.1%.
    let tests: [f32; 6] = [1.0, 0.0, 1.0e-37, 3.4e38, 42.0, 0.00012];
    let k_one = Point::new(1.0, 1.0);
    for test in tests {
        let mut slow = k_one;
        let mut fast = k_one;
        slow.set_length(test);
        set_length_fast(&mut fast, test);
        if slow.length() < f32::MIN_POSITIVE && fast.length() < f32::MIN_POSITIVE {
            continue;
        }
        let ratio: scalar = slow.length() / fast.length();
        #[allow(clippy::neg_cmp_op_on_partial_ord)] // inside reporter_assert!'s `!(cond)`
        {
            reporter_assert!(reporter, ratio > 0.999);
            reporter_assert!(reporter, ratio < 1.001);
        }
    }
});
