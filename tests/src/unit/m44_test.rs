// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/M44Test.cpp (chrome/m156)
//
// Not ported yet (manifest stays `todo`): `M44_mapRect`, which compares against
// `SkPathBuilder::addRect(..).transform(..).detach().getBounds()` (SkPath, SkPathBuilder).

#![cfg(test)]

use skia_rust_core::m44::{M44, V2, V3, V4};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::matrix_priv;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{Scalar, scalar};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/M44Test.cpp#L16-L25 (chrome/m156)
fn eq(a: &M44, b: &M44, tol: f32) -> bool {
    let mut fa = [0.0; 16];
    let mut fb = [0.0; 16];
    a.get_col_major(&mut fa);
    b.get_col_major(&mut fb);
    for i in 0..16 {
        if !scalar::nearly_equal(fa[i], fb[i], tol) {
            return false;
        }
    }
    true
}

// `SkM44::invert(SkM44* inverse)`: the C++ out-parameter form, which writes `inverse` only when
// it succeeds.
fn invert_into(m: &M44, inverse: &mut M44) -> bool {
    if let Some(inv) = m.invert() {
        *inverse = inv;
        return true;
    }
    false
}

// Port of: tests/M44Test.cpp#L33-L77 (chrome/m156)
// Allowed lints: the C++ compares floats with ==
def_test!(
    #[allow(clippy::float_cmp)]
    M44,
    |reporter| {
        let mut m = M44::default();
        let mut im = M44::default();

        reporter_assert!(
            reporter,
            M44::new(
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0
            ) == m
        );
        reporter_assert!(reporter, M44::default() == m);
        reporter_assert!(reporter, invert_into(&m, &mut im));
        reporter_assert!(reporter, M44::default() == im);

        m.set_translate(3.0, 4.0, 2.0);
        reporter_assert!(
            reporter,
            M44::new(
                1.0, 0.0, 0.0, 3.0, 0.0, 1.0, 0.0, 4.0, 0.0, 0.0, 1.0, 2.0, 0.0, 0.0, 0.0, 1.0
            ) == m
        );

        let f: [f32; 16] = [
            1.0, 0.0, 0.0, 2.0, 3.0, 1.0, 2.0, 5.0, 0.0, 5.0, 3.0, 0.0, 0.0, 1.0, 0.0, 2.0,
        ];
        m = M44::col_major(&f);
        reporter_assert!(
            reporter,
            M44::new(
                f[0], f[4], f[8], f[12], f[1], f[5], f[9], f[13], f[2], f[6], f[10], f[14], f[3],
                f[7], f[11], f[15]
            ) == m
        );

        {
            let t = m.transpose();
            reporter_assert!(reporter, t != m);
            reporter_assert!(reporter, t.rc(1, 0) == m.rc(0, 1));
            let tt = t.transpose();
            reporter_assert!(reporter, tt == m);
        }

        m = M44::row_major(&f);
        reporter_assert!(
            reporter,
            M44::new(
                f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8], f[9], f[10], f[14], f[12],
                f[13], f[14], f[15]
            ) == m
        );

        reporter_assert!(reporter, invert_into(&m, &mut im));

        m = &m * &im;
        // m should be identity now, but our calc is not perfect...
        reporter_assert!(reporter, eq(&M44::default(), &m, 0.000_000_5));
        reporter_assert!(reporter, M44::default() != m);
    }
);

// Port of: tests/M44Test.cpp#L79-L101 (chrome/m156)
// Allowed lints: names follow the C++; the C++ compares floats with ==
def_test!(
    #[allow(clippy::float_cmp, clippy::many_single_char_names)]
    M44_v3,
    |reporter| {
        let a = V3::new(1.0, 2.0, 3.0);
        let b = V3::new(1.0, 2.0, 2.0);

        reporter_assert!(reporter, a.length_squared() == 1.0 + 4.0 + 9.0);
        reporter_assert!(reporter, b.length() == 3.0);
        reporter_assert!(reporter, a.dot(&b) == 1.0 + 4.0 + 6.0);
        reporter_assert!(reporter, b.dot(&a) == 1.0 + 4.0 + 6.0);
        reporter_assert!(reporter, a.cross(&b) == V3::new(-2.0, 1.0, 0.0));
        reporter_assert!(reporter, b.cross(&a) == V3::new(2.0, -1.0, 0.0));

        let m = M44::new(
            2.0, 0.0, 0.0, 3.0, 0.0, 1.0, 0.0, 5.0, 0.0, 0.0, 3.0, 1.0, 0.0, 0.0, 0.0, 1.0,
        );

        let c = &m * a;
        reporter_assert!(reporter, c == V3::new(2.0, 2.0, 9.0));
        let d = m.map(4.0, 3.0, 2.0, 1.0);
        reporter_assert!(reporter, d == V4::new(11.0, 8.0, 7.0, 1.0));
    }
);

// Port of: tests/M44Test.cpp#L103-L142 (chrome/m156)
def_test!(M44_v4, |reporter| {
    let m = M44::new(
        1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
    );

    let r0 = m.row(0);
    let r1 = m.row(1);
    let r2 = m.row(2);
    let r3 = m.row(3);

    reporter_assert!(reporter, r0 == V4::new(1.0, 2.0, 3.0, 4.0));
    reporter_assert!(reporter, r1 == V4::new(5.0, 6.0, 7.0, 8.0));
    reporter_assert!(reporter, r2 == V4::new(9.0, 10.0, 11.0, 12.0));
    reporter_assert!(reporter, r3 == V4::new(13.0, 14.0, 15.0, 16.0));

    reporter_assert!(reporter, M44::rows(&r0, &r1, &r2, &r3) == m);

    let c0 = m.col(0);
    let c1 = m.col(1);
    let c2 = m.col(2);
    let c3 = m.col(3);

    reporter_assert!(reporter, c0 == V4::new(1.0, 5.0, 9.0, 13.0));
    reporter_assert!(reporter, c1 == V4::new(2.0, 6.0, 10.0, 14.0));
    reporter_assert!(reporter, c2 == V4::new(3.0, 7.0, 11.0, 15.0));
    reporter_assert!(reporter, c3 == V4::new(4.0, 8.0, 12.0, 16.0));

    reporter_assert!(reporter, M44::cols(&c0, &c1, &c2, &c3) == m);

    // implement matrix * vector using column vectors
    let v = V4::new(1.0, 2.0, 3.0, 4.0);
    let v1 = &m * v;
    let v2 = c0 * v.x + c1 * v.y + c2 * v.z + c3 * v.w;
    reporter_assert!(reporter, v1 == v2);

    reporter_assert!(
        reporter,
        c0 + r0 == V4::new(c0.x + r0.x, c0.y + r0.y, c0.z + r0.z, c0.w + r0.w)
    );
    reporter_assert!(
        reporter,
        c0 - r0 == V4::new(c0.x - r0.x, c0.y - r0.y, c0.z - r0.z, c0.w - r0.w)
    );
    reporter_assert!(
        reporter,
        c0 * r0 == V4::new(c0.x * r0.x, c0.y * r0.y, c0.z * r0.z, c0.w * r0.w)
    );
});

// Port of: tests/M44Test.cpp#L144-L196 (chrome/m156)
// Allowed lints: mirrors the local constants of the C++
def_test!(
    #[allow(clippy::items_after_statements)]
    M44_rotate,
    |reporter| {
        let x = V3::new(1.0, 0.0, 0.0);
        let y = V3::new(0.0, 1.0, 0.0);
        let z = V3::new(0.0, 0.0, 1.0);

        // We have radians version of setRotateAbout methods, but even with our best approx
        // for PI, sin(SK_ScalarPI) != 0, so to make the comparisons in the unittest clear,
        // I'm using the variants that explicitly take the sin,cos values.

        struct Rec {
            sin_angle: scalar,
            cos_angle: scalar,
            about_axis: V3,
            expected_x: V3,
            expected_y: V3,
            expected_z: V3,
        }
        let rec = |sin_angle, cos_angle, about_axis, expected_x, expected_y, expected_z| Rec {
            sin_angle,
            cos_angle,
            about_axis,
            expected_x,
            expected_y,
            expected_z,
        };

        let recs = [
            rec(0.0, 1.0, x, x, y, z),    // angle = 0
            rec(0.0, 1.0, y, x, y, z),    // angle = 0
            rec(0.0, 1.0, z, x, y, z),    // angle = 0
            rec(0.0, -1.0, x, x, -y, -z), // angle = 180
            rec(0.0, -1.0, y, -x, y, -z), // angle = 180
            rec(0.0, -1.0, z, -x, -y, z), // angle = 180
            // Skia coordinate system is right-handed
            rec(1.0, 0.0, x, x, z, -y),  // angle = 90
            rec(1.0, 0.0, y, -z, y, x),  // angle = 90
            rec(1.0, 0.0, z, y, -x, z),  // angle = 90
            rec(-1.0, 0.0, x, x, -z, y), // angle = -90
            rec(-1.0, 0.0, y, z, y, -x), // angle = -90
            rec(-1.0, 0.0, z, -y, x, z), // angle = -90
        ];

        for r in &recs {
            let mut m = M44::nan();
            m.set_rotate_unit_sin_cos(r.about_axis, r.sin_angle, r.cos_angle);

            let mut mx = &m * x;
            let mut my = &m * y;
            let mut mz = &m * z;
            reporter_assert!(reporter, mx == r.expected_x);
            reporter_assert!(reporter, my == r.expected_y);
            reporter_assert!(reporter, mz == r.expected_z);

            // flipping the axis-of-rotation should flip the results
            mx = &m * -x;
            my = &m * -y;
            mz = &m * -z;
            reporter_assert!(reporter, mx == -r.expected_x);
            reporter_assert!(reporter, my == -r.expected_y);
            reporter_assert!(reporter, mz == -r.expected_z);
        }
    }
);

// The `map2d` lambda of `M44_rectToRect`.
// Port of: tests/M44Test.cpp#L211-L216 (chrome/m156)
#[allow(clippy::float_cmp)] // the C++ compares floats with ==
fn map2d(reporter: &mut Reporter, m: &M44, p: V2) -> V2 {
    let mapped = m.map(p.x, p.y, 0.0, 1.0);
    reporter_assert!(reporter, mapped.z == 0.0);
    reporter_assert!(reporter, mapped.w == 1.0);
    V2::new(mapped.x, mapped.y)
}

// The `assertNearlyEqual` lambda of `M44_rectToRect`.
// Port of: tests/M44Test.cpp#L217-L220 (chrome/m156)
fn assert_nearly_equal(reporter: &mut Reporter, actual: f32, expected: f32) {
    reporter_assert!(
        reporter,
        scalar::nearly_equal(actual, expected, None),
        "Expected {} == {}",
        actual,
        expected
    );
}

// The `assertEdges` lambda of `M44_rectToRect`.
// Port of: tests/M44Test.cpp#L221-L229 (chrome/m156)
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the C++ asserts float comparisons directly
fn assert_edges(
    reporter: &mut Reporter,
    actual_low: f32,
    actual_high: f32,
    expected_low: f32,
    expected_high: f32,
) {
    debug_assert!(expected_low < expected_high);
    reporter_assert!(
        reporter,
        actual_low < actual_high,
        "Expected {} < {}",
        actual_low,
        actual_high
    );

    assert_nearly_equal(reporter, actual_low, expected_low);
    assert_nearly_equal(reporter, actual_high, expected_high);
}

// Port of: tests/M44Test.cpp#L198-L260 (chrome/m156)
def_test!(M44_rectToRect, |reporter| {
    let dst_scales = [
        V2::new(1.0, 1.0),  // no aspect ratio change, nor up/down scaling
        V2::new(0.25, 0.5), // aspect ratio narrows, downscale x and y
        V2::new(0.5, 0.25), // aspect ratio widens, downscale x and y
        V2::new(0.5, 0.5),  // no aspect ratio change, downscale x and y
        V2::new(2.0, 3.0),  // aspect ratio narrows, upscale x and y
        V2::new(3.0, 2.0),  // aspect ratio widens, upscale x and y
        V2::new(2.0, 2.0),  // no aspect ratio change, upscale x and y
        V2::new(0.5, 2.0),  // aspect ratio narrows, downscale x and upscale y
        V2::new(2.0, 0.5),  // aspect ratio widens, upscale x and downscale y
    ];

    let mut rand = Random::default();
    for r in &dst_scales {
        let src = Rect::from_xywh(
            rand.next_range_f(-10.0, 10.0),
            rand.next_range_f(-10.0, 10.0),
            rand.next_range_f(1.0, 10.0),
            rand.next_range_f(1.0, 10.0),
        );
        let dst = Rect::from_xywh(
            rand.next_range_f(-10.0, 10.0),
            rand.next_range_f(-10.0, 10.0),
            r.x * src.width(),
            r.y * src.height(),
        );

        let m = M44::rect_to_rect(src, dst);

        // Regardless of the factory, center of src maps to center of dst
        let center = map2d(reporter, &m, V2::new(src.center_x(), src.center_y()));
        assert_nearly_equal(reporter, center.x, dst.center_x());
        assert_nearly_equal(reporter, center.y, dst.center_y());

        // Map the four corners of src and validate against expected edge mapping
        let tl = map2d(reporter, &m, V2::new(src.left, src.top));
        let tr = map2d(reporter, &m, V2::new(src.right, src.top));
        let br = map2d(reporter, &m, V2::new(src.right, src.bottom));
        let bl = map2d(reporter, &m, V2::new(src.left, src.bottom));

        assert_edges(reporter, tl.x, tr.x, dst.left, dst.right);
        assert_edges(reporter, bl.x, br.x, dst.left, dst.right);
        assert_edges(reporter, tl.y, bl.y, dst.top, dst.bottom);
        assert_edges(reporter, tr.y, br.y, dst.top, dst.bottom);
    }
});

// Port of: tests/M44Test.cpp#L374-L386 (chrome/m156)
// Allowed lints: float literals are copied verbatim from the C++
def_test!(
    #[allow(clippy::excessive_precision)]
    M44_mapRect_skbug12335,
    |r| {
        // Stripped down test case from skbug.com/40043416. Essentially, the corners of this rect would
        // map to homogoneous coords with very small w's (below the old value of kW0PlaneDistance) and
        // so they would be clipped "behind" the plane, resulting in an empty mapped rect. Coordinates
        // with positive that wouldn't overflow when divided by w should still be included in the
        // mapped rectangle.
        let rect = Rect::new(0.0, 0.0, 319.0, 620.0);
        let m = M44::from(&Matrix::new_all(
            0.000_152_695_269,
            0.000_000_00,
            -6.538_484_01e-05,
            -1.756_975_33e-05,
            0.000_157_153_074,
            -1.108_479_75e-06,
            -6.004_153_62e-08,
            0.000_000_00,
            0.000_169_880_834,
        ));
        let out = matrix_priv::map_rect(&m, &rect);
        reporter_assert!(r, !out.is_empty());
    }
);
