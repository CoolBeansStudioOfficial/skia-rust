// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/Point3Test.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::point3::Point3;
use skia_rust_core::random::Random;
use skia_rust_core::scalar::{SCALAR_1, SCALAR_ROOT_2_OVER_2, Scalar, scalar};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/Point3Test.cpp#L19-L26 (chrome/m156)
fn test_eq_ops(reporter: &mut Reporter) {
    let p0 = Point3::new(0.0, 0.0, 0.0);
    let p1 = Point3::new(1.0, 1.0, 1.0);
    let p2 = Point3::new(1.0, 1.0, 1.0);

    reporter_assert!(reporter, p0 != p1);
    reporter_assert!(reporter, p1 == p2);
}

// Port of: tests/Point3Test.cpp#L28-L57 (chrome/m156)
fn test_ops(reporter: &mut Reporter) {
    let mut v = Point3::new(1.0, 1.0, 1.0);
    v.normalize();
    reporter_assert!(reporter, scalar::nearly_equal(v.length(), SCALAR_1, None));

    // scale
    let mut p = v.scaled(3.0);
    reporter_assert!(reporter, scalar::nearly_equal(p.length(), 3.0, None));

    p.scale(1.0 / 3.0);
    reporter_assert!(reporter, scalar::nearly_equal(p.length(), SCALAR_1, None));

    let p1 = Point3::new(20.0, 2.0, 10.0);
    let p2 = -p1;

    // -
    #[allow(clippy::eq_op)] // the C++ test subtracts a point from itself on purpose
    {
        p = p1 - p1;
    }
    reporter_assert!(reporter, scalar::nearly_equal(p.x, 0.0, None));
    reporter_assert!(reporter, scalar::nearly_equal(p.y, 0.0, None));
    reporter_assert!(reporter, scalar::nearly_equal(p.z, 0.0, None));

    // +
    p = p1 + p2;
    reporter_assert!(reporter, scalar::nearly_equal(p.x, 0.0, None));
    reporter_assert!(reporter, scalar::nearly_equal(p.y, 0.0, None));
    reporter_assert!(reporter, scalar::nearly_equal(p.z, 0.0, None));
}

// Port of: tests/Point3Test.cpp#L59-L85 (chrome/m156)
fn test_dot(reporter: &mut Reporter) {
    let x_axis = Point3::new(1.0, 0.0, 0.0);
    let y_axis = Point3::new(0.0, 1.0, 0.0);
    let z_axis = Point3::new(0.0, 0.0, 1.0);

    let mut dot: scalar = x_axis.dot(y_axis);
    reporter_assert!(reporter, scalar::nearly_equal(dot, 0.0, None));

    dot = y_axis.dot(z_axis);
    reporter_assert!(reporter, scalar::nearly_equal(dot, 0.0, None));

    dot = z_axis.dot(x_axis);
    reporter_assert!(reporter, scalar::nearly_equal(dot, 0.0, None));

    let mut v = Point3::new(13.0, 2.0, 7.0);
    v.normalize();
    dot = v.dot(v);
    reporter_assert!(reporter, scalar::nearly_equal(dot, 1.0, None));

    v = Point3::new(SCALAR_ROOT_2_OVER_2, SCALAR_ROOT_2_OVER_2, 0.0);

    dot = x_axis.dot(v);
    reporter_assert!(
        reporter,
        scalar::nearly_equal(dot, SCALAR_ROOT_2_OVER_2, None)
    );

    dot = y_axis.dot(v);
    reporter_assert!(
        reporter,
        scalar::nearly_equal(dot, SCALAR_ROOT_2_OVER_2, None)
    );
}

// Port of: tests/Point3Test.cpp#L87-L98 (chrome/m156)
fn test_length(reporter: &mut Reporter, x: scalar, y: scalar, z: scalar, expected_len: scalar) {
    let point = Point3::new(x, y, z);

    let s1 = point.length();
    let s2 = Point3::length_xyz(x, y, z);
    reporter_assert!(reporter, scalar::nearly_equal(s1, s2, None));
    reporter_assert!(reporter, scalar::nearly_equal(s1, expected_len, None));
}

// Port of: tests/Point3Test.cpp#L100-L124 (chrome/m156)
fn test_normalize(reporter: &mut Reporter, x: scalar, y: scalar, z: scalar, expected_len: scalar) {
    let mut point = Point3::new(x, y, z);
    let result = point.normalize();
    let new_length = point.length();
    #[allow(clippy::float_cmp)] // exact comparison, as in the C++ test
    if 0.0 == expected_len {
        let empty = Point3::new(0.0, 0.0, 0.0);
        reporter_assert!(reporter, scalar::nearly_equal(new_length, 0.0, None));
        reporter_assert!(reporter, !result);
        reporter_assert!(reporter, point == empty);
    } else {
        reporter_assert!(reporter, scalar::nearly_equal(new_length, SCALAR_1, None));
        reporter_assert!(reporter, result);
    }

    let mut random = Random::default();
    random.set_seed(1234);
    let mut pt3 = Point3::default();
    let test_count = 100_000;
    for _index in 0..test_count {
        let mut test_val: scalar;
        loop {
            test_val = random.next_range_f(0.0, 2.0);
            if test_val != 0.0 {
                break;
            }
        }
        pt3.set(test_val, 0.0, 0.0);
        #[allow(clippy::float_cmp)] // exact comparison, as in the C++ test
        {
            reporter_assert!(reporter, !pt3.normalize() || 1.0 == pt3.x);
        }
    }
}

// Port of: tests/Point3Test.cpp#L126-L148 (chrome/m156)
def_test!(Point3, |reporter| {
    #[allow(clippy::struct_field_names)] // mirrors the C++ struct's fX/fY/fZ/fLength
    struct GRec {
        f_x: scalar,
        f_y: scalar,
        f_z: scalar,
        f_length: scalar,
    }

    test_eq_ops(reporter);
    test_ops(reporter);
    test_dot(reporter);

    let g_rec = [
        GRec {
            f_x: 0.0,
            f_y: 0.0,
            f_z: 0.0,
            f_length: 0.0,
        },
        GRec {
            f_x: 0.3,
            f_y: 0.4,
            f_z: 0.5,
            f_length: SCALAR_ROOT_2_OVER_2,
        },
        GRec {
            f_x: 1.0e-37,
            f_y: 1.0e-37,
            f_z: 1.0e-37,
            f_length: 0.0, // underflows
        },
        GRec {
            f_x: 3.4e38,
            f_y: 0.0,
            f_z: 0.0,
            f_length: 3.4e38, // overflows
        },
    ];

    for rec in &g_rec {
        test_length(reporter, rec.f_x, rec.f_y, rec.f_z, rec.f_length);
        test_normalize(reporter, rec.f_x, rec.f_y, rec.f_z, rec.f_length);
    }
});
