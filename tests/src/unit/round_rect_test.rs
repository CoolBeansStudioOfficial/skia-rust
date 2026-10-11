// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RoundRectTest.cpp (chrome/m156)

#![cfg(test)]
// The assertions compare floats exactly, as the C++ test does: a tolerance here would weaken
// them, and the port has no tolerances.
#![allow(clippy::float_cmp)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::float_bits::bits_to_float;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::point::{Point, Vector, point_priv};
use skia_rust_core::random::Random;
use skia_rust_core::rect::{Contains, Rect};
use skia_rust_core::rrect::rrect_priv::{self, are_rect_and_radii_valid};
use skia_rust_core::rrect::{Corner, RRect, Type};
use skia_rust_core::scalar::{SCALAR_SQRT2, Scalar, scalar};
use skia_rust_pathops::op;
use skia_rust_pathops::path_op::PathOp;

// Port of: tests/RoundRectTest.cpp#L1652-L1682 (chrome/m156)
def_test!(RRect_fuzzer_regressions, |r| {
    {
        let buf: [u8; 48] = [
            0x0a, 0x00, 0x00, 0xff, 0x00, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7f, 0x7f, 0x7f,
            0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f,
            0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x02, 0x00, 0x00,
            0x00, 0x00, 0x20, 0x00, 0x02, 0x00,
        ];
        reporter_assert!(r, buf.len() == RRect::new().read_from_memory(&buf));
    }

    {
        let buf: [u8; 48] = [
            0x5d, 0xff, 0xff, 0x5d, 0x0a, 0x60, 0x0a, 0x0a, 0x0a, 0x7e, 0x0a, 0x5a, 0x0a, 0x12,
            0x3a, 0x3a, 0x3a, 0x3a, 0x3a, 0x3a, 0x3a, 0x3a, 0x3a, 0x3a, 0x3a, 0x3a, 0x3a, 0x3a,
            0x3a, 0x3a, 0x3a, 0x3a, 0x00, 0x00, 0x00, 0x0a, 0x0a, 0x0a, 0x0a, 0x26, 0x0a, 0x0a,
            0x0a, 0x0a, 0xff, 0xff, 0x0a, 0x0a,
        ];
        reporter_assert!(r, buf.len() == RRect::new().read_from_memory(&buf));
    }

    {
        let buf: [u8; 48] = [
            0xfe, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x04, 0xdd, 0xdd, 0x15, 0xfe, 0x00,
            0x00, 0x04, 0x05, 0x7e, 0x00, 0x00, 0x00, 0xff, 0x08, 0x04, 0xff, 0xff, 0xfe, 0xfe,
            0xff, 0x32, 0x32, 0x32, 0x32, 0x00, 0x32, 0x32, 0x04, 0xdd, 0x3d, 0x1c, 0xfe, 0x89,
            0x04, 0x0a, 0x0e, 0x05, 0x7e, 0x0a,
        ];
        reporter_assert!(r, buf.len() == RRect::new().read_from_memory(&buf));
    }
});

// Port of: tests/RoundRectTest.cpp#L1684-L1712 (chrome/m156)
def_test!(RRect_b511391129, |r| {
    let k_val_a = bits_to_float(0x4b4b_4b4a); // 13323074.0f
    let k_val_b = bits_to_float(0x4b4b_4b4b); // 13323083.0f

    {
        let rect = Rect::new(0.0, 0.0, k_val_b, k_val_b);
        let mut rr = RRect::new();
        rr.set_rect_xy(rect, k_val_a, k_val_a);
        reporter_assert!(r, rr.is_valid());
    }

    {
        let rect = Rect::new(0.0, 0.0, k_val_b, k_val_b);
        let mut rr = RRect::new();
        rr.set_nine_patch(rect, k_val_a, k_val_a, k_val_a, k_val_a);
        reporter_assert!(r, rr.is_valid());
    }

    {
        let rect = Rect::new(0.0, 0.0, k_val_b, k_val_b);
        let radii: [Vector; 4] = [Point::new(k_val_a, k_val_a); 4];
        let mut rr = RRect::new();
        rr.set_rect_radii(rect, &radii);
        reporter_assert!(r, rr.is_valid());
    }
});

// Port of: tests/RoundRectTest.cpp#L1714-L1729 (chrome/m156)
def_test!(RRect_b527765132, |r| {
    // These values were found via fuzzing to trigger a validation due to ULPs only not being a
    // good way to compare floats
    let width = 37.40884_f32;
    let height = 73.09393_f32;
    let dx = -5_430.867_f32;
    let dy = -26_967.047_f32;

    let rect = Rect::from_wh(width, height);
    let rr = RRect::new_oval(rect);

    let offset_rrect = rr.with_offset((dx, dy));

    reporter_assert!(r, offset_rrect.is_valid());
    reporter_assert!(r, offset_rrect.is_oval());
});

// Port of: tests/RoundRectTest.cpp#L1731-L1784 (chrome/m156)
def_test!(RRect_b547198215, |r| {
    // Overlapping corner radii on any side must be rejected by AreRectAndRadiiValid.
    let rect = Rect::from_wh(100.0, 100.0);

    // Overlapping right side (UR.y + LR.y > height)
    {
        let radii: [Vector; 4] = [
            Point::new(0.0, 0.0),
            Point::new(0.0, 60.0),
            Point::new(0.0, 60.0),
            Point::new(0.0, 0.0),
        ];
        reporter_assert!(r, !are_rect_and_radii_valid(&rect, &radii));

        // Constructing via setRectRadii scales radii to fit, making it valid.
        let mut rr = RRect::new();
        rr.set_rect_radii(rect, &radii);
        reporter_assert!(r, rr.is_valid());
    }

    // Overlapping top side (UL.x + UR.x > width)
    {
        let radii: [Vector; 4] = [
            Point::new(70.0, 0.0),
            Point::new(70.0, 0.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
        ];
        reporter_assert!(r, !are_rect_and_radii_valid(&rect, &radii));
    }

    // Overlapping bottom side (LL.x + LR.x > width)
    {
        let radii: [Vector; 4] = [
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
            Point::new(60.0, 0.0),
            Point::new(60.0, 0.0),
        ];
        reporter_assert!(r, !are_rect_and_radii_valid(&rect, &radii));
    }

    // Overlapping left side (UL.y + LL.y > height)
    {
        let radii: [Vector; 4] = [
            Point::new(0.0, 55.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, 55.0),
        ];
        reporter_assert!(r, !are_rect_and_radii_valid(&rect, &radii));
    }

    // Negative radii must be rejected by AreRectAndRadiiValid.
    {
        let radii: [Vector; 4] = [
            Point::new(-5.0, 0.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
        ];
        reporter_assert!(r, !are_rect_and_radii_valid(&rect, &radii));
    }
    {
        let radii: [Vector; 4] = [
            Point::new(0.0, -5.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
        ];
        reporter_assert!(r, !are_rect_and_radii_valid(&rect, &radii));
    }

    // Test the specific testcase geometry from b/547198215
    {
        let fuzzed_rect = Rect::new(
            1.356_315_6e-19_f32,
            1.356_315_6e-19_f32,
            163_968.5_f32,
            800.501_95_f32,
        );
        let fuzzed_radii: [Vector; 4] = [
            Point::new(1.356_315_6e-19_f32, 1.356_315_6e-19_f32),
            Point::new(1.356_315_6e-19_f32, 640.501_95_f32),
            Point::new(1.356_315_6e-19_f32, 640.501_95_f32),
            Point::new(1.356_315_6e-19_f32, 1.356_315_6e-19_f32),
        ];
        reporter_assert!(r, !are_rect_and_radii_valid(&fuzzed_rect, &fuzzed_radii));
    }
});

// Port of: tests/RoundRectTest.cpp#L1786-L1803 (chrome/m156)
def_test!(
    #[allow(clippy::similar_names)] // names follow the C++
    RRect_b561770646_part1,
    |r| {
        // Fuzzed testcase from b/561770646 where fBottom - fTop == fBottom in float due to tiny
        // fTop, causing a scaled radius of height (fBottom) to produce fBottom - rad = 0 < fTop.
        let rect = Rect::from_ltrb(0.0, 1e-30, 100.0, 100.0);
        let radii = [
            Vector::new(0.0, 0.0),    // Upper-Left
            Vector::new(0.0, 0.0),    // Upper-Right
            Vector::new(0.0, 0.0),    // Lower-Right
            Vector::new(10.0, 100.0), // Lower-Left (fY == height == fBottom)
        ];

        let mut rrect = RRect::default();
        rrect.set_rect_radii(rect, &radii);
        reporter_assert!(r, rrect.is_valid());
        let path = Path::rrect_with_start_index(rrect, PathDirection::CW, 0);
        let _out = path.is_rrect();
    }
);

// Port of: tests/RoundRectTest.cpp#L1805-L1818 (chrome/m156)
def_test!(RRect_b561770646_part2, |r| {
    // Axis-aligned transform that collapses an SkRRect / SkPath::RRect to empty bounds due to
    // floating-point precision loss must fail transform and not crash DeduceRRectFromContour.
    let rr = RRect::new_rect_xy(Rect::from_wh(100.0, 100.0), 10.0, 10.0);
    let collapse_matrix = Matrix::translate((1e20, 0.0));

    let transformed_rr = rr.transform(&collapse_matrix);
    reporter_assert!(r, transformed_rr.is_none());

    let path = Path::rrect(rr, None);
    let transformed_path = path.make_transform(&collapse_matrix);
    reporter_assert!(r, transformed_path.is_rrect().is_none());
});

// Port of: tests/RoundRectTest.cpp#L28-L51 (chrome/m156), test_tricky_radii
fn test_tricky_radii(reporter: &mut Reporter) {
    {
        // crbug.com/458522
        let mut rr = RRect::new();
        let bounds = Rect::new(3709.0, 3709.0, 3709.0 + 7402.0, 3709.0 + 29825.0);
        let rad: scalar = 12814.0;
        let vec = [
            Vector::new(rad, rad),
            Vector::new(0.0, rad),
            Vector::new(rad, rad),
            Vector::new(0.0, rad),
        ];
        rr.set_rect_radii(bounds, &vec);
    }

    {
        // crbug.com//463920
        let r = Rect::new(0.0, 0.0, 1009.0, 33_554_432.0);
        let radii = [
            Vector::new(13.0, 8.0),
            Vector::new(170.0, 2.0),
            Vector::new(256.0, 33_554_432.0),
            Vector::new(110.0, 5.0),
        ];
        let mut rr = RRect::new();
        rr.set_rect_radii(r, &radii);

        let radii_sum =
            f64::from(rr.radii(Corner::UpperRight).y) + f64::from(rr.radii(Corner::LowerRight).y);
        let fits = radii_sum <= f64::from(rr.height());
        reporter_assert!(reporter, fits);
    }
}

// Port of: tests/RoundRectTest.cpp#L53-L65 (chrome/m156), test_empty_crbug_458524
fn test_empty_crbug_458524(reporter: &mut Reporter) {
    let mut rr = RRect::new();
    let bounds = Rect::new(3709.0, 3709.0, 3709.0 + 7402.0, 3709.0 + 29825.0);
    let rad = 40.0;
    rr.set_rect_xy(bounds, rad, rad);

    let mut matrix = Matrix::default();
    matrix.set_scale((0.0, 1.0), None);
    let other = rr.transform(&matrix);
    reporter_assert!(reporter, other.is_none());
}

// Test that all the SkRRect entry points correctly handle un-sorted and
// zero-sized input rects
// Port of: tests/RoundRectTest.cpp#L67-L133 (chrome/m156), test_empty
fn test_empty(reporter: &mut Reporter) {
    // out of order
    let ooo_rects = [
        Rect::new(100.0, 0.0, 0.0, 100.0), // ooo horizontal
        Rect::new(0.0, 100.0, 100.0, 0.0), // ooo vertical
        Rect::new(100.0, 100.0, 0.0, 0.0), // ooo both
    ];

    let empty_rects = [
        Rect::new(100.0, 100.0, 100.0, 200.0), // empty horizontal
        Rect::new(100.0, 100.0, 200.0, 100.0), // empty vertical
        Rect::new(100.0, 100.0, 100.0, 100.0), // empty both
        Rect::new(0.0, 0.0, 0.0, 0.0),         // setEmpty-empty
    ];

    let radii = [
        Vector::new(0.0, 1.0),
        Vector::new(2.0, 3.0),
        Vector::new(4.0, 5.0),
        Vector::new(6.0, 7.0),
    ];

    let mut r = RRect::new();

    for rect in ooo_rects {
        r.set_rect(rect);
        reporter_assert!(reporter, !r.is_empty());
        reporter_assert!(reporter, *r.rect() == rect.sorted());

        r.set_oval(rect);
        reporter_assert!(reporter, !r.is_empty());
        reporter_assert!(reporter, *r.rect() == rect.sorted());

        r.set_rect_xy(rect, 1.0, 2.0);
        reporter_assert!(reporter, !r.is_empty());
        reporter_assert!(reporter, *r.rect() == rect.sorted());

        r.set_nine_patch(rect, 0.0, 1.0, 2.0, 3.0);
        reporter_assert!(reporter, !r.is_empty());
        reporter_assert!(reporter, *r.rect() == rect.sorted());

        r.set_rect_radii(rect, &radii);
        reporter_assert!(reporter, !r.is_empty());
        reporter_assert!(reporter, *r.rect() == rect.sorted());
    }

    for rect in empty_rects {
        r.set_rect(rect);
        reporter_assert!(reporter, r.is_empty());
        reporter_assert!(reporter, *r.rect() == rect);

        r.set_oval(rect);
        reporter_assert!(reporter, r.is_empty());
        reporter_assert!(reporter, *r.rect() == rect);

        r.set_rect_xy(rect, 1.0, 2.0);
        reporter_assert!(reporter, r.is_empty());
        reporter_assert!(reporter, *r.rect() == rect);

        r.set_nine_patch(rect, 0.0, 1.0, 2.0, 3.0);
        reporter_assert!(reporter, r.is_empty());
        reporter_assert!(reporter, *r.rect() == rect);

        r.set_rect_radii(rect, &radii);
        reporter_assert!(reporter, r.is_empty());
        reporter_assert!(reporter, *r.rect() == rect);
    }

    r.set_rect(Rect::new(f32::NAN, 10.0, 10.0, 20.0));
    reporter_assert!(reporter, r == RRect::new_empty());
    r.set_rect(Rect::new(0.0, 10.0, 10.0, f32::INFINITY));
    reporter_assert!(reporter, r == RRect::new_empty());
}

// Port of: tests/RoundRectTest.cpp#L135-L136 (chrome/m156), kWidth and kHeight
const K_WIDTH: scalar = 100.0;
const K_HEIGHT: scalar = 100.0;

// The corners in the order of `SkRRect::Corner` (`(SkRRect::Corner) i`).
const CORNERS: [Corner; 4] = [
    Corner::UpperLeft,
    Corner::UpperRight,
    Corner::LowerRight,
    Corner::LowerLeft,
];

// Port of: tests/RoundRectTest.cpp#L138-L158 (chrome/m156), test_inset
fn test_inset(reporter: &mut Reporter) {
    let r = Rect::new(0.0, 0.0, 100.0, 100.0);

    let mut rr = RRect::new();
    rr.set_rect(r);
    let rr2 = rr.with_inset((-20.0, -20.0));
    reporter_assert!(reporter, rr2.is_rect());

    let rr2 = rr.with_inset((20.0, 20.0));
    reporter_assert!(reporter, rr2.is_rect());

    let rr2 = rr.with_inset((r.width() / 2.0, r.height() / 2.0));
    reporter_assert!(reporter, rr2.is_empty());

    rr.set_rect_xy(r, 20.0, 20.0);
    let rr2 = rr.with_inset((19.0, 19.0));
    reporter_assert!(reporter, rr2.is_simple());
    let rr2 = rr.with_inset((20.0, 20.0));
    reporter_assert!(reporter, rr2.is_rect());
}

// Port of: tests/RoundRectTest.cpp#L160-L183 (chrome/m156), test_9patch_rrect
fn test_9patch_rrect(
    reporter: &mut Reporter,
    rect: Rect,
    l: scalar,
    t: scalar,
    r: scalar,
    b: scalar,
    check_radii: bool,
) {
    let mut rr = RRect::new();
    rr.set_nine_patch(rect, l, t, r, b);

    reporter_assert!(reporter, rr.get_type() == Type::NinePatch);
    reporter_assert!(reporter, *rr.rect() == rect);

    if check_radii {
        // This test doesn't hold if the radii will be rescaled by SkRRect
        let nine_patch_radii = Rect::new(l, t, r, b);
        let rquad = nine_patch_radii.to_quad(PathDirection::CW);
        for (i, corner) in CORNERS.into_iter().enumerate() {
            reporter_assert!(reporter, rquad[i] == rr.radii(corner));
        }
    }
    let mut rr2 = RRect::new(); // construct the same RR using the most general set function
    let radii = [
        Vector::new(l, t),
        Vector::new(r, t),
        Vector::new(r, b),
        Vector::new(l, b),
    ];
    rr2.set_rect_radii(rect, &radii);
    reporter_assert!(reporter, rr2 == rr && rr2.get_type() == rr.get_type());
}

// Test out the basic API entry points
// Port of: tests/RoundRectTest.cpp#L185-L290 (chrome/m156), test_round_rect_basic
fn test_round_rect_basic(reporter: &mut Reporter) {
    // Test out initialization methods
    let zero_pt = Point::new(0.0, 0.0);
    let mut empty = RRect::new();

    empty.set_empty();

    reporter_assert!(reporter, empty.get_type() == Type::Empty);
    reporter_assert!(reporter, empty.rect().is_empty());

    for corner in CORNERS {
        reporter_assert!(reporter, zero_pt == empty.radii(corner));
    }

    //----
    let rect = Rect::new(0.0, 0.0, K_WIDTH, K_HEIGHT);

    let mut rr1 = RRect::new();
    rr1.set_rect(rect);

    reporter_assert!(reporter, rr1.get_type() == Type::Rect);
    reporter_assert!(reporter, *rr1.rect() == rect);

    for corner in CORNERS {
        reporter_assert!(reporter, zero_pt == rr1.radii(corner));
    }
    let mut rr1_2 = RRect::new(); // construct the same RR using the most general set function
    let rr1_2_radii = [Vector::new(0.0, 0.0); 4];
    rr1_2.set_rect_radii(rect, &rr1_2_radii);
    reporter_assert!(reporter, rr1_2 == rr1 && rr1_2.get_type() == rr1.get_type());
    let mut rr1_3 = RRect::new(); // construct the same RR using the nine patch set function
    rr1_3.set_nine_patch(rect, 0.0, 0.0, 0.0, 0.0);
    reporter_assert!(reporter, rr1_3 == rr1 && rr1_3.get_type() == rr1.get_type());

    //----
    let half_point = Point::new(K_WIDTH / 2.0, K_HEIGHT / 2.0);
    let mut rr2 = RRect::new();
    rr2.set_oval(rect);

    reporter_assert!(reporter, rr2.get_type() == Type::Oval);
    reporter_assert!(reporter, *rr2.rect() == rect);

    for corner in CORNERS {
        reporter_assert!(
            reporter,
            point_priv::equals_within_tolerance(rr2.radii(corner), half_point)
        );
    }
    let mut rr2_2 = RRect::new(); // construct the same RR using the most general set function
    let rr2_2_radii = [Vector::new(half_point.x, half_point.y); 4];
    rr2_2.set_rect_radii(rect, &rr2_2_radii);
    reporter_assert!(reporter, rr2_2 == rr2 && rr2_2.get_type() == rr2.get_type());
    let mut rr2_3 = RRect::new(); // construct the same RR using the nine patch set function
    rr2_3.set_nine_patch(rect, half_point.x, half_point.y, half_point.x, half_point.y);
    reporter_assert!(reporter, rr2_3 == rr2 && rr2_3.get_type() == rr2.get_type());

    //----
    let p = Point::new(5.0, 5.0);
    let mut rr3 = RRect::new();
    rr3.set_rect_xy(rect, p.x, p.y);

    reporter_assert!(reporter, rr3.get_type() == Type::Simple);
    reporter_assert!(reporter, *rr3.rect() == rect);

    for corner in CORNERS {
        reporter_assert!(reporter, p == rr3.radii(corner));
    }
    let mut rr3_2 = RRect::new(); // construct the same RR using the most general set function
    let rr3_2_radii = [Vector::new(5.0, 5.0); 4];
    rr3_2.set_rect_radii(rect, &rr3_2_radii);
    reporter_assert!(reporter, rr3_2 == rr3 && rr3_2.get_type() == rr3.get_type());
    let mut rr3_3 = RRect::new(); // construct the same RR using the nine patch set function
    rr3_3.set_nine_patch(rect, 5.0, 5.0, 5.0, 5.0);
    reporter_assert!(reporter, rr3_3 == rr3 && rr3_3.get_type() == rr3.get_type());

    //----
    test_9patch_rrect(reporter, rect, 10.0, 9.0, 8.0, 7.0, true);

    {
        // Test out the rrect from skbug.com/40034587
        let rect2 = Rect::new(0.358_212, 0.755_430_2, 0.872_866_15, 0.806_214_33);

        test_9patch_rrect(
            reporter,
            rect2,
            0.926_942_35,
            0.642_850_3,
            0.529_063_46,
            0.587_844_4,
            false,
        );
    }

    //----
    let radii2 = [
        Point::new(0.0, 0.0),
        Point::new(0.0, 0.0),
        Point::new(50.0, 50.0),
        Point::new(20.0, 50.0),
    ];

    let mut rr5 = RRect::new();
    rr5.set_rect_radii(rect, &radii2);

    reporter_assert!(reporter, rr5.get_type() == Type::Complex);
    reporter_assert!(reporter, *rr5.rect() == rect);

    for (i, corner) in CORNERS.into_iter().enumerate() {
        reporter_assert!(reporter, radii2[i] == rr5.radii(corner));
    }

    // Test out == & !=
    reporter_assert!(reporter, empty != rr3);
    reporter_assert!(reporter, rr3 != rr5);
}

// Port of: tests/RoundRectTest.cpp#L292-L330 (chrome/m156), test_round_rect_rects
fn test_round_rect_rects(reporter: &mut Reporter) {
    //----
    let mut empty = RRect::new();

    empty.set_empty();

    reporter_assert!(reporter, empty.get_type() == Type::Empty);
    let r = *empty.rect();
    reporter_assert!(
        reporter,
        0.0 == r.left && 0.0 == r.top && 0.0 == r.right && 0.0 == r.bottom
    );

    //----
    let rect = Rect::new(0.0, 0.0, K_WIDTH, K_HEIGHT);
    let mut rr1 = RRect::new();
    rr1.set_rect_xy(rect, 0.0, 0.0);

    reporter_assert!(reporter, rr1.get_type() == Type::Rect);
    let r = *rr1.rect();
    reporter_assert!(reporter, rect == r);

    //----
    let radii = [Vector::new(0.0, 0.0); 4];

    let mut rr2 = RRect::new();
    rr2.set_rect_radii(rect, &radii);

    reporter_assert!(reporter, rr2.get_type() == Type::Rect);
    let r = *rr2.rect();
    reporter_assert!(reporter, rect == r);

    //----
    let radii2 = [
        Vector::new(0.0, 0.0),
        Vector::new(20.0, 20.0),
        Vector::new(50.0, 50.0),
        Vector::new(20.0, 50.0),
    ];

    let mut rr3 = RRect::new();
    rr3.set_rect_radii(rect, &radii2);
    reporter_assert!(reporter, rr3.get_type() == Type::Complex);
}

// Test out the cases when the RR degenerates to an oval
// Port of: tests/RoundRectTest.cpp#L332-L343 (chrome/m156), test_round_rect_ovals
fn test_round_rect_ovals(reporter: &mut Reporter) {
    //----
    let rect = Rect::new(0.0, 0.0, K_WIDTH, K_HEIGHT);
    let mut rr1 = RRect::new();
    rr1.set_rect_xy(rect, K_WIDTH / 2.0, K_HEIGHT / 2.0);

    reporter_assert!(reporter, rr1.get_type() == Type::Oval);
    let oval = *rr1.rect();
    reporter_assert!(reporter, oval == rect);
}

// Test out the non-degenerate RR cases
// Port of: tests/RoundRectTest.cpp#L345-L361 (chrome/m156), test_round_rect_general
fn test_round_rect_general(reporter: &mut Reporter) {
    //----
    let rect = Rect::new(0.0, 0.0, K_WIDTH, K_HEIGHT);
    let mut rr1 = RRect::new();
    rr1.set_rect_xy(rect, 20.0, 20.0);

    reporter_assert!(reporter, rr1.get_type() == Type::Simple);

    //----
    let radii = [
        Vector::new(0.0, 0.0),
        Vector::new(20.0, 20.0),
        Vector::new(50.0, 50.0),
        Vector::new(20.0, 50.0),
    ];

    let mut rr2 = RRect::new();
    rr2.set_rect_radii(rect, &radii);

    reporter_assert!(reporter, rr2.get_type() == Type::Complex);
}

// Test out questionable-parameter handling
// Port of: tests/RoundRectTest.cpp#L363-L392 (chrome/m156), test_round_rect_iffy_parameters
fn test_round_rect_iffy_parameters(reporter: &mut Reporter) {
    // When the radii exceed the base rect they are proportionally scaled down
    // to fit
    let rect = Rect::new(0.0, 0.0, K_WIDTH, K_HEIGHT);
    let radii = [
        Vector::new(50.0, 100.0),
        Vector::new(100.0, 50.0),
        Vector::new(50.0, 100.0),
        Vector::new(100.0, 50.0),
    ];

    let mut rr1 = RRect::new();
    rr1.set_rect_radii(rect, &radii);

    reporter_assert!(reporter, rr1.get_type() == Type::Complex);

    let p = rr1.radii(Corner::UpperLeft);

    reporter_assert!(reporter, f32::nearly_equal(p.x, 33.33333, None));
    reporter_assert!(reporter, f32::nearly_equal(p.y, 66.66666, None));

    // Negative radii should be capped at zero
    let mut rr2 = RRect::new();
    rr2.set_rect_xy(rect, -10.0, -20.0);

    reporter_assert!(reporter, rr2.get_type() == Type::Rect);

    let p2 = rr2.radii(Corner::UpperLeft);

    reporter_assert!(reporter, 0.0 == p2.x);
    reporter_assert!(reporter, 0.0 == p2.y);
}

// Move a small box from the start position by (stepX, stepY) 'numSteps' times
// testing for containment in 'rr' at each step.
// Port of: tests/RoundRectTest.cpp#L394-L411 (chrome/m156), test_direction
// The `SkIntToScalar` of the step is an int-to-float cast, exact for these small steps.
#[allow(clippy::cast_precision_loss)]
// Mirrors the C++ signature of the helper.
#[allow(clippy::too_many_arguments)]
fn test_direction(
    reporter: &mut Reporter,
    rr: &RRect,
    init_x: scalar,
    step_x: i32,
    init_y: scalar,
    step_y: i32,
    num_steps: usize,
    contains: &[bool],
) {
    let mut x = init_x;
    let mut y = init_y;
    for &expected in contains.iter().take(num_steps) {
        let mut test = Rect::from_xywh(
            x,
            y,
            if step_x != 0 { step_x as scalar } else { 1.0 },
            if step_y != 0 { step_y as scalar } else { 1.0 },
        );
        test.sort();

        reporter_assert!(reporter, expected == rr.contains(test));

        x += step_x as scalar;
        y += step_y as scalar;
    }
}

// Move a point from the start position by (stepX, stepY) 'numSteps' times
// testing for containment in 'rr' at each step.
// Port of: tests/RoundRectTest.cpp#L413-L424 (chrome/m156), test_point_direction
// The `SkIntToScalar` of the step is an int-to-float cast, exact for these small steps.
#[allow(clippy::cast_precision_loss)]
// Mirrors the C++ signature of the helper.
#[allow(clippy::too_many_arguments)]
fn test_point_direction(
    reporter: &mut Reporter,
    rr: &RRect,
    init_x: scalar,
    step_x: i32,
    init_y: scalar,
    step_y: i32,
    contains: &[bool],
) {
    let mut x = init_x;
    let mut y = init_y;
    for &expected in contains {
        reporter_assert!(reporter, expected == rr.contains_point(Point::new(x, y)));

        x += step_x as scalar;
        y += step_y as scalar;
    }
}

// Port of: tests/RoundRectTest.cpp#L426-L538 (chrome/m156), test_round_rect_contains_rect
// Mirrors the single C++ test function; splitting it would not be a 1:1 port.
#[allow(clippy::too_many_lines)]
fn test_round_rect_contains_rect(reporter: &mut Reporter) {
    const K_NUM_RRECTS: usize = 4;
    const K_NUM_STEPS: usize = 15;
    let g_radii: [[Vector; 4]; K_NUM_RRECTS] = [
        // rect
        [
            Vector::new(0.0, 0.0),
            Vector::new(0.0, 0.0),
            Vector::new(0.0, 0.0),
            Vector::new(0.0, 0.0),
        ],
        // circle
        [
            Vector::new(20.0, 20.0),
            Vector::new(20.0, 20.0),
            Vector::new(20.0, 20.0),
            Vector::new(20.0, 20.0),
        ],
        // simple
        [
            Vector::new(10.0, 10.0),
            Vector::new(10.0, 10.0),
            Vector::new(10.0, 10.0),
            Vector::new(10.0, 10.0),
        ],
        // complex
        [
            Vector::new(0.0, 0.0),
            Vector::new(20.0, 20.0),
            Vector::new(10.0, 10.0),
            Vector::new(30.0, 30.0),
        ],
    ];

    let rrects: [RRect; K_NUM_RRECTS] = std::array::from_fn(|i| {
        let mut rr = RRect::new();
        rr.set_rect_radii(Rect::from_wh(40.0, 40.0), &g_radii[i]);
        rr
    });

    // First test easy outs - boxes that are obviously out on
    // each corner and edge
    let easy_outs = [
        Rect::new(-5.0, -5.0, 5.0, 5.0),   // NW
        Rect::new(15.0, -5.0, 20.0, 5.0),  // N
        Rect::new(35.0, -5.0, 45.0, 5.0),  // NE
        Rect::new(35.0, 15.0, 45.0, 20.0), // E
        Rect::new(35.0, 45.0, 35.0, 45.0), // SE
        Rect::new(15.0, 35.0, 20.0, 45.0), // S
        Rect::new(-5.0, 35.0, 5.0, 45.0),  // SW
        Rect::new(-5.0, 15.0, 5.0, 20.0),  // W
    ];

    for rr in &rrects {
        for easy_out in easy_outs {
            reporter_assert!(reporter, !rr.contains(easy_out));
        }
    }

    // Now test non-trivial containment. For each compass
    // point walk a 1x1 rect in from the edge  of the bounding
    // rect
    #[rustfmt::skip]
    let answers: [[[bool; K_NUM_STEPS]; 8]; K_NUM_RRECTS] = [
// all the test rects are inside the degenerate rrect
        [
// rect
            [ true, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ true, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ true, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ true, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ true, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ true, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ true, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ true, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
        ],
// for the circle we expect 6 blocks to be out on the
// corners (then the rest in) and only the first block
// out on the vertical and horizontal axes (then
// the rest in)
        [
// circle
            [ false, false, false, false, false, false, true, true, true, true, true, true, true, true, true ],
            [ false, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ false, false, false, false, false, false, true, true, true, true, true, true, true, true, true ],
            [ false, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ false, false, false, false, false, false, true, true, true, true, true, true, true, true, true ],
            [ false, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ false, false, false, false, false, false, true, true, true, true, true, true, true, true, true ],
            [ false, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
        ],
// for the simple round rect we expect 3 out on
// the corners (then the rest in) and no blocks out
// on the vertical and horizontal axes
        [
// simple RR
            [ false, false, false, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ true, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ false, false, false, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ true, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ false, false, false, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ true, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ false, false, false, true, true, true, true, true, true, true, true, true, true, true, true ],
            [ true, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
        ],
// for the complex case the answer is different for each direction
        [
// complex RR
// all in for NW (rect) corner (same as rect case)
            [ true, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
// only first block out for N (same as circle case)
            [ false, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
// first 6 blocks out for NE (same as circle case)
            [ false, false, false, false, false, false, true, true, true, true, true, true, true, true, true ],
// only first block out for E (same as circle case)
            [ false, true, true, true, true, true, true, true, true, true, true, true, true, true, true ],
// first 3 blocks out for SE (same as simple case)
            [ false, false, false, true, true, true, true, true, true, true, true, true, true, true, true ],
// first two blocks out for S
            [ false, false, true, true, true, true, true, true, true, true, true, true, true, true, true ],
// first 9 blocks out for SW
            [ false, false, false, false, false, false, false, false, false, true, true, true, true, true, true ],
// first two blocks out for W (same as S)
            [ false, false, true, true, true, true, true, true, true, true, true, true, true, true, true ],
         ]
    ];

    for (i, rr) in rrects.iter().enumerate() {
        test_direction(reporter, rr, 0.0, 1, 0.0, 1, K_NUM_STEPS, &answers[i][0]); // NW
        test_direction(reporter, rr, 19.5, 0, 0.0, 1, K_NUM_STEPS, &answers[i][1]); // N
        test_direction(reporter, rr, 40.0, -1, 0.0, 1, K_NUM_STEPS, &answers[i][2]); // NE
        test_direction(reporter, rr, 40.0, -1, 19.5, 0, K_NUM_STEPS, &answers[i][3]); // E
        test_direction(
            reporter,
            rr,
            40.0,
            -1,
            40.0,
            -1,
            K_NUM_STEPS,
            &answers[i][4],
        ); // SE
        test_direction(reporter, rr, 19.5, 0, 40.0, -1, K_NUM_STEPS, &answers[i][5]); // S
        test_direction(reporter, rr, 0.0, 1, 40.0, -1, K_NUM_STEPS, &answers[i][6]); // SW
        test_direction(reporter, rr, 0.0, 1, 19.5, 0, K_NUM_STEPS, &answers[i][7]); // W
    }
}

// Port of: tests/RoundRectTest.cpp#L540-L643 (chrome/m156), test_round_rect_contains_point
// Mirrors the single C++ test function; splitting it would not be a 1:1 port.
#[allow(clippy::too_many_lines)]
fn test_round_rect_contains_point(reporter: &mut Reporter) {
    const K_NUM_RRECTS: usize = 4;
    const K_NUM_STEPS: usize = 3;
    let g_radii: [[Vector; 4]; K_NUM_RRECTS] = [
        // rect
        [
            Vector::new(0.0, 0.0),
            Vector::new(0.0, 0.0),
            Vector::new(0.0, 0.0),
            Vector::new(0.0, 0.0),
        ],
        // circle
        [
            Vector::new(20.0, 20.0),
            Vector::new(20.0, 20.0),
            Vector::new(20.0, 20.0),
            Vector::new(20.0, 20.0),
        ],
        // simple
        [
            Vector::new(10.0, 10.0),
            Vector::new(10.0, 10.0),
            Vector::new(10.0, 10.0),
            Vector::new(10.0, 10.0),
        ],
        // complex
        [
            Vector::new(0.0, 0.0),
            Vector::new(20.0, 20.0),
            Vector::new(10.0, 10.0),
            Vector::new(30.0, 30.0),
        ],
    ];

    let rrects: [RRect; K_NUM_RRECTS] = std::array::from_fn(|i| {
        let mut rr = RRect::new();
        rr.set_rect_radii(Rect::from_wh(40.0, 40.0), &g_radii[i]);
        rr
    });

    let easy_outs = [
        Point::new(-5.0, -5.0), // TL
        Point::new(20.0, -5.0), // T
        Point::new(45.0, -5.0), // TR
        Point::new(45.0, 20.0), // R
        Point::new(45.0, 45.0), // BR
        Point::new(20.0, 45.0), // B
        Point::new(-5.0, 45.0), // BL
        Point::new(-5.0, 20.0), // L
    ];

    for rr in &rrects {
        for easy_out in easy_outs {
            reporter_assert!(reporter, !rr.contains_point(easy_out));
        }
    }

    #[rustfmt::skip]
    let answers: [[[bool; K_NUM_STEPS]; 8]; K_NUM_RRECTS] = [
// for the rect we expect points on the left/top edges to be in, but
// points on the right/bottom edges to be out
        [
// rect
            [ true, true, true ],
            [ true, true, true ],
            [ false, true, true ],
            [ false, true, true ],
            [ false, true, true ],
            [ false, true, true ],
            [ false, true, true ],
            [ true, true, true ],
        ],
// for the circle we expect the first two points to be out on the
// corners (then in) and only the first point out on the right/bottom axes
        [
// circle
            [ false, false, true ],
            [ true, true, true ],
            [ false, false, true ],
            [ false, true, true ],
            [ false, false, true ],
            [ false, true, true ],
            [ false, false, true ],
            [ true, true, true ],
        ],
// for the simple round rect we expect only the first point to be out on
// corners and on the right/bottom axes
        [
// simple RR
            [ false, true, true ],
            [ true, true, true ],
            [ false, true, true ],
            [ false, true, true ],
            [ false, true, true ],
            [ false, true, true ],
            [ false, true, true ],
            [ true, true, true ],
        ],
// for the complex case the answer is different for each direction
        [
// complex RR
// all in for TL (rect) corner and T (top center band)
            [ true, true, true ],
            [ true, true, true ],
// first out for TR and BL is the excluded bound; second is outside the larger corner
            [ false, false, true ],
// first out for R, BR, and B is the excluded bound; L is due to the larger corner
            [ false, true, true ],
            [ false, true, true ],
            [ false, true, true ],
            [ false, false, true ],
            [ false, true, true ],
         ]
    ];

    for (i, rr) in rrects.iter().enumerate() {
        test_point_direction(reporter, rr, 0.0, 5, 0.0, 5, &answers[i][0]); // TL
        test_point_direction(reporter, rr, 20.0, 0, 0.0, 5, &answers[i][1]); // T
        test_point_direction(reporter, rr, 40.0, -5, 0.0, 5, &answers[i][2]); // TR
        test_point_direction(reporter, rr, 40.0, -5, 20.0, 0, &answers[i][3]); // R
        test_point_direction(reporter, rr, 40.0, -5, 40.0, -5, &answers[i][4]); // BR
        test_point_direction(reporter, rr, 20.0, 0, 40.0, -5, &answers[i][5]); // B
        test_point_direction(reporter, rr, 0.0, 5, 40.0, -5, &answers[i][6]); // BL
        test_point_direction(reporter, rr, 0.0, 5, 20.0, 0, &answers[i][7]); // L
    }
}

// Port of: tests/RoundRectTest.cpp#L644-L654 (chrome/m156), assert_transform_failure
fn assert_transform_failure(reporter: &mut Reporter, orig: &RRect, matrix: &Matrix) {
    // The test depends on the fact that the original is not empty.
    debug_assert!(!orig.is_empty());

    let copy_of_orig = *orig;
    let dst = orig.transform(matrix);
    // This transform should fail.
    reporter_assert!(reporter, dst.is_none());
    // original should not be modified.
    reporter_assert!(reporter, copy_of_orig == *orig);
}

// The radii of an `SkRRect` by corner, for the `GET_RADII` checks of `test_transform_helper`.
// Port of: tests/RoundRectTest.cpp#L656-L664 (chrome/m156), GET_RADII
struct Radii {
    ul: Vector,
    ur: Vector,
    lr: Vector,
    ll: Vector,
}

impl Radii {
    // Port of: tests/RoundRectTest.cpp#L656-L664 (chrome/m156), GET_RADII
    fn of(rr: &RRect) -> Self {
        Radii {
            ul: rr.radii(Corner::UpperLeft),
            ur: rr.radii(Corner::UpperRight),
            lr: rr.radii(Corner::LowerRight),
            ll: rr.radii(Corner::LowerLeft),
        }
    }
}

// Called to test various transforms on a single SkRRect.
// Port of: tests/RoundRectTest.cpp#L668-L900 (chrome/m156), test_transform_helper
// Mirrors the single C++ helper; splitting it would not be a 1:1 port.
#[allow(clippy::too_many_lines)]
fn test_transform_helper(reporter: &mut Reporter, orig: &RRect) {
    // The identity matrix will duplicate the rrect.
    let mut dst = orig.transform(Matrix::i());
    reporter_assert!(reporter, dst.is_some());
    reporter_assert!(reporter, Some(*orig) == dst);

    // Skew and Perspective make transform fail.
    let mut matrix = Matrix::default();
    matrix.reset();
    matrix.set_skew_x(2.0);
    assert_transform_failure(reporter, orig, &matrix);

    matrix.reset();
    matrix.set_skew_y(3.0);
    assert_transform_failure(reporter, orig, &matrix);

    matrix.reset();
    matrix.set_persp_x(4.0);
    assert_transform_failure(reporter, orig, &matrix);

    matrix.reset();
    matrix.set_persp_y(5.0);
    assert_transform_failure(reporter, orig, &matrix);

    // Rotation fails.
    matrix.reset();
    matrix.set_rotate(37.0, None);
    assert_transform_failure(reporter, orig, &matrix);

    // Translate will keep the rect moved, but otherwise the same.
    matrix.reset();
    let translate_x: scalar = 32.0;
    let translate_y: scalar = 15.0;
    matrix.set_translate_x(translate_x);
    matrix.set_translate_y(translate_y);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    let d = dst.expect("translated rrect");
    for corner in CORNERS {
        reporter_assert!(reporter, orig.radii(corner) == d.radii(corner));
    }
    reporter_assert!(reporter, orig.rect().width() == d.rect().width());
    reporter_assert!(reporter, orig.rect().height() == d.rect().height());
    reporter_assert!(reporter, d.rect().left == orig.rect().left + translate_x);
    reporter_assert!(reporter, d.rect().top == orig.rect().top + translate_y);

    // Keeping the translation, but adding skew will make transform fail.
    matrix.set_skew_y(7.0);
    assert_transform_failure(reporter, orig, &matrix);

    // Scaling in -x will flip the round rect horizontally.
    matrix.reset();
    matrix.set_scale_x(-1.0);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    {
        let d = dst.expect("flipped rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        // Radii have swapped in x.
        reporter_assert!(reporter, o.ul == dd.ur);
        reporter_assert!(reporter, o.ur == dd.ul);
        reporter_assert!(reporter, o.lr == dd.ll);
        reporter_assert!(reporter, o.ll == dd.lr);
        // Width and height remain the same.
        reporter_assert!(reporter, orig.rect().width() == d.rect().width());
        reporter_assert!(reporter, orig.rect().height() == d.rect().height());
        // Right and left have swapped (sort of)
        reporter_assert!(reporter, orig.rect().right == -d.rect().left);
        // Top has stayed the same.
        reporter_assert!(reporter, orig.rect().top == d.rect().top);
    }

    // Keeping the scale, but adding a persp will make transform fail.
    matrix.set_persp_x(7.0);
    assert_transform_failure(reporter, orig, &matrix);

    // Test out possible floating point issues w/ the radii transform
    matrix = Matrix::scale((0.999_999, 0.999_999));
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());

    // Scaling in -y will flip the round rect vertically.
    matrix.reset();
    matrix.set_scale_y(-1.0);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    {
        let d = dst.expect("flipped rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        // Radii have swapped in y.
        reporter_assert!(reporter, o.ul == dd.ll);
        reporter_assert!(reporter, o.ur == dd.lr);
        reporter_assert!(reporter, o.lr == dd.ur);
        reporter_assert!(reporter, o.ll == dd.ul);
        // Width and height remain the same.
        reporter_assert!(reporter, orig.rect().width() == d.rect().width());
        reporter_assert!(reporter, orig.rect().height() == d.rect().height());
        // Top and bottom have swapped (sort of)
        reporter_assert!(reporter, orig.rect().top == -d.rect().bottom);
        // Left has stayed the same.
        reporter_assert!(reporter, orig.rect().left == d.rect().left);
    }

    // Scaling in -x and -y will swap in both directions.
    matrix.reset();
    matrix.set_scale_y(-1.0);
    matrix.set_scale_x(-1.0);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    {
        let d = dst.expect("flipped rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        reporter_assert!(reporter, o.ul == dd.lr);
        reporter_assert!(reporter, o.ur == dd.ll);
        reporter_assert!(reporter, o.lr == dd.ul);
        reporter_assert!(reporter, o.ll == dd.ur);
        // Width and height remain the same.
        reporter_assert!(reporter, orig.rect().width() == d.rect().width());
        reporter_assert!(reporter, orig.rect().height() == d.rect().height());
        reporter_assert!(reporter, orig.rect().top == -d.rect().bottom);
        reporter_assert!(reporter, orig.rect().right == -d.rect().left);
    }

    // Scale in both directions.
    let mut x_scale: scalar = 3.0;
    let mut y_scale: scalar = 3.2;
    matrix.reset();
    matrix.set_scale_x(x_scale);
    matrix.set_scale_y(y_scale);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    let d = dst.expect("scaled rrect");
    // Radii are scaled.
    for corner in CORNERS {
        reporter_assert!(
            reporter,
            f32::nearly_equal(d.radii(corner).x, orig.radii(corner).x * x_scale, None)
        );
        reporter_assert!(
            reporter,
            f32::nearly_equal(d.radii(corner).y, orig.radii(corner).y * y_scale, None)
        );
    }
    reporter_assert!(
        reporter,
        f32::nearly_equal(d.rect().width(), orig.rect().width() * x_scale, None)
    );
    reporter_assert!(
        reporter,
        f32::nearly_equal(d.rect().height(), orig.rect().height() * y_scale, None)
    );
    reporter_assert!(
        reporter,
        f32::nearly_equal(d.rect().left, orig.rect().left * x_scale, None)
    );
    reporter_assert!(
        reporter,
        f32::nearly_equal(d.rect().top, orig.rect().top * y_scale, None)
    );

    //  a-----b            d-----a
    //  |     |     ->     |     |
    //  |     |  Rotate 90 |     |
    //  d-----c            c-----b
    matrix.reset();
    matrix.set_rotate(90.0, None);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    {
        let d = dst.expect("rotated rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        // Radii have cycled clockwise and swapped their x and y axis.
        reporter_assert!(reporter, dd.ul.x == o.ll.y);
        reporter_assert!(reporter, dd.ul.y == o.ll.x);
        reporter_assert!(reporter, dd.ur.x == o.ul.y);
        reporter_assert!(reporter, dd.ur.y == o.ul.x);
        reporter_assert!(reporter, dd.lr.x == o.ur.y);
        reporter_assert!(reporter, dd.lr.y == o.ur.x);
        reporter_assert!(reporter, dd.ll.x == o.lr.y);
        reporter_assert!(reporter, dd.ll.y == o.lr.x);
        // Width and height would get swapped.
        reporter_assert!(reporter, orig.rect().width() == d.rect().height());
        reporter_assert!(reporter, orig.rect().height() == d.rect().width());
    }

    //  a-----b           d-----a        a-----d
    //  |     |    ->     |     |    ->  |     |
    //  |     | Rotate 90 |     | Flip X |     |
    //  d-----c           c-----b        b-----c
    matrix.reset();
    matrix.set_rotate(90.0, None);
    matrix.post_scale((-1.0, 1.0), None);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    {
        let d = dst.expect("rotated rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        reporter_assert!(reporter, dd.ul.x == o.ul.y);
        reporter_assert!(reporter, dd.ul.y == o.ul.x);
        reporter_assert!(reporter, dd.ur.x == o.ll.y);
        reporter_assert!(reporter, dd.ur.y == o.ll.x);
        reporter_assert!(reporter, dd.lr.x == o.lr.y);
        reporter_assert!(reporter, dd.lr.y == o.lr.x);
        reporter_assert!(reporter, dd.ll.x == o.ur.y);
        reporter_assert!(reporter, dd.ll.y == o.ur.x);
        // Width and height would get swapped.
        reporter_assert!(reporter, orig.rect().width() == d.rect().height());
        reporter_assert!(reporter, orig.rect().height() == d.rect().width());
    }

    //  a-----b        d-----c           a-----d
    //  |     |   ->   |     |     ->    |     |
    //  |     | Flip Y |     | Rotate 90 |     |
    //  d-----c        a-----b           b-----c
    //
    // This is the same as Rotate 90 and Flip X.
    matrix.reset();
    matrix.set_scale((1.0, -1.0), None);
    matrix.post_rotate(90.0, None);
    let mut dst2 = orig.transform(&matrix);
    reporter_assert!(reporter, dst2.is_some());
    reporter_assert!(reporter, dst == dst2);

    //  a-----b        b-----a            a-----d
    //  |     |   ->   |     |    ->      |     |
    //  |     | Flip X |     | Rotate 270 |     |
    //  d-----c        c-----d            b-----c
    matrix.reset();
    matrix.set_scale((-1.0, 1.0), None);
    matrix.post_rotate(270.0, None);
    dst2 = orig.transform(&matrix);
    reporter_assert!(reporter, dst2.is_some());
    reporter_assert!(reporter, dst == dst2);

    //  a-----b            b-----c        a-----d
    //  |     |     ->     |     |   ->   |     |
    //  |     | Rotate 270 |     | Flip Y |     |
    //  d-----c            a-----d        b-----c
    matrix.reset();
    matrix.set_rotate(270.0, None);
    matrix.post_scale((1.0, -1.0), None);
    dst2 = orig.transform(&matrix);
    reporter_assert!(reporter, dst2.is_some());
    reporter_assert!(reporter, dst == dst2);

    //  a-----b        b-----a           c-----b
    //  |     |   ->   |     |    ->     |     |
    //  |     | Flip X |     | Rotate 90 |     |
    //  d-----c        c-----d           d-----a
    matrix.reset();

    matrix.set_scale((-1.0, 1.0), None);
    matrix.post_rotate(90.0, None);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    {
        let d = dst.expect("rotated rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        reporter_assert!(reporter, dd.ul.x == o.lr.y);
        reporter_assert!(reporter, dd.ul.y == o.lr.x);
        reporter_assert!(reporter, dd.ur.x == o.ur.y);
        reporter_assert!(reporter, dd.ur.y == o.ur.x);
        reporter_assert!(reporter, dd.lr.x == o.ul.y);
        reporter_assert!(reporter, dd.lr.y == o.ul.x);
        reporter_assert!(reporter, dd.ll.x == o.ll.y);
        reporter_assert!(reporter, dd.ll.y == o.ll.x);
        // Width and height would get swapped.
        reporter_assert!(reporter, orig.rect().width() == d.rect().height());
        reporter_assert!(reporter, orig.rect().height() == d.rect().width());
    }

    //  a-----b           d-----a        c-----b
    //  |     |    ->     |     |   ->   |     |
    //  |     | Rotate 90 |     | Flip Y |     |
    //  d-----c           c-----b        d-----a
    // This is the same as flip X and rotate 90
    matrix.reset();
    matrix.set_rotate(90.0, None);
    matrix.post_scale((1.0, -1.0), None);
    dst2 = orig.transform(&matrix);
    reporter_assert!(reporter, dst2.is_some());
    reporter_assert!(reporter, dst == dst2);

    //  a-----b            b-----c        c-----b
    //  |     |     ->     |     |   ->   |     |
    //  |     | Rotate 270 |     | Flip X |     |
    //  d-----c            a-----d        d-----a
    matrix.reset();
    matrix.set_rotate(270.0, None);
    matrix.post_scale((-1.0, 1.0), None);
    dst2 = orig.transform(&matrix);
    reporter_assert!(reporter, dst2.is_some());
    reporter_assert!(reporter, dst == dst2);

    //  a-----b        d-----c            c-----b
    //  |     |   ->   |     |    ->      |     |
    //  |     | Flip Y |     | Rotate 270 |     |
    //  d-----c        a-----b            d-----a
    matrix.reset();
    matrix.set_scale((1.0, -1.0), None);
    matrix.post_rotate(270.0, None);
    dst2 = orig.transform(&matrix);
    reporter_assert!(reporter, dst2.is_some());
    reporter_assert!(reporter, dst == dst2);

    //  a-----b           d-----a          b-----c
    //  |     |    ->     |     |   ->     |     |
    //  |     | Rotate 90 |     | Flip X+Y |     |
    //  d-----c           c-----b          a-----d
    // This is the same as rotation by 270.
    matrix.reset();
    matrix.set_rotate(90.0, None);
    matrix.post_scale((-1.0, -1.0), None);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    {
        let d = dst.expect("rotated rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        // Radii have cycled clockwise and swapped their x and y axis.
        reporter_assert!(reporter, dd.ul.x == o.ur.y);
        reporter_assert!(reporter, dd.ul.y == o.ur.x);
        reporter_assert!(reporter, dd.ur.x == o.lr.y);
        reporter_assert!(reporter, dd.ur.y == o.lr.x);
        reporter_assert!(reporter, dd.lr.x == o.ll.y);
        reporter_assert!(reporter, dd.lr.y == o.ll.x);
        reporter_assert!(reporter, dd.ll.x == o.ul.y);
        reporter_assert!(reporter, dd.ll.y == o.ul.x);
        // Width and height would get swapped.
        reporter_assert!(reporter, orig.rect().width() == d.rect().height());
        reporter_assert!(reporter, orig.rect().height() == d.rect().width());
    }

    //  a-----b             b-----c
    //  |     |     ->      |     |
    //  |     | Rotate 270  |     |
    //  d-----c             a-----d
    //
    matrix.reset();
    matrix.set_rotate(270.0, None);
    dst2 = orig.transform(&matrix);
    reporter_assert!(reporter, dst2.is_some());
    reporter_assert!(reporter, dst == dst2);

    //  a-----b            b-----c          d-----a
    //  |     |    ->      |     |   ->     |     |
    //  |     | Rotate 270 |     | Flip X+Y |     |
    //  d-----c            a-----d          c-----b
    // This is the same as rotation by 90 degrees.
    matrix.reset();
    matrix.set_rotate(270.0, None);
    matrix.post_scale((-1.0, -1.0), None);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());

    matrix.reset();
    matrix.set_rotate(90.0, None);
    dst2 = orig.transform(&matrix);
    reporter_assert!(reporter, dst2.is_some());
    reporter_assert!(reporter, dst == dst2);

    // -------------

    // a----b               a------b             d--a
    // |    |     ->        |      |      ->     |  |
    // |    |   Scale Y,    d------c  Rotate 90  |  |
    // d----c   Scale X                          |  |
    //                                           c--b
    y_scale = 0.5;
    x_scale = 1.5;
    matrix.reset();
    matrix.set_rotate(90.0, None);
    matrix.pre_scale((x_scale, y_scale), None);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    // Make scale factors positive for length comparisons
    y_scale = y_scale.abs();
    x_scale = x_scale.abs();
    {
        let d = dst.expect("scaled rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        // Radii have counter clock-wise and swapped their x and y axis. The dst x radii should be
        // scaled 1/2x compared to the y radii, dst y scaled 1.5 compared to src x.
        reporter_assert!(reporter, dd.ul.x == y_scale * o.ll.y);
        reporter_assert!(reporter, dd.ul.y == x_scale * o.ll.x);
        reporter_assert!(reporter, dd.ur.x == y_scale * o.ul.y);
        reporter_assert!(reporter, dd.ur.y == x_scale * o.ul.x);
        reporter_assert!(reporter, dd.lr.x == y_scale * o.ur.y);
        reporter_assert!(reporter, dd.lr.y == x_scale * o.ur.x);
        reporter_assert!(reporter, dd.ll.x == y_scale * o.lr.y);
        reporter_assert!(reporter, dd.ll.y == x_scale * o.lr.x);
        // Width and height would get swapped, with the dst width half of the original height.
        reporter_assert!(reporter, x_scale * orig.rect().width() == d.rect().height());
        reporter_assert!(reporter, y_scale * orig.rect().height() == d.rect().width());
    }

    // a----b               b------a             c--b
    // |    |     ->        |      |      ->     |  |
    // |    |   Scale Y,    c------d  Rotate 90  |  |
    // d----c Flip+Scale X                       |  |
    //                                           d--a
    y_scale = 0.5;
    x_scale = -1.5;
    matrix.reset();
    matrix.set_rotate(90.0, None);
    matrix.pre_scale((x_scale, y_scale), None);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    // Make scale factors positive for length comparisons
    y_scale = y_scale.abs();
    x_scale = x_scale.abs();
    {
        let d = dst.expect("scaled rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        reporter_assert!(reporter, dd.ul.x == y_scale * o.lr.y);
        reporter_assert!(reporter, dd.ul.y == x_scale * o.lr.x);
        reporter_assert!(reporter, dd.ur.x == y_scale * o.ur.y);
        reporter_assert!(reporter, dd.ur.y == x_scale * o.ur.x);
        reporter_assert!(reporter, dd.lr.x == y_scale * o.ul.y);
        reporter_assert!(reporter, dd.lr.y == x_scale * o.ul.x);
        reporter_assert!(reporter, dd.ll.x == y_scale * o.ll.y);
        reporter_assert!(reporter, dd.ll.y == x_scale * o.ll.x);
        // Width and height would get swapped, with the dst width half of the original height.
        reporter_assert!(reporter, x_scale * orig.rect().width() == d.rect().height());
        reporter_assert!(reporter, y_scale * orig.rect().height() == d.rect().width());
    }

    // a----b               d------c             a--d
    // |    |     ->        |      |      ->     |  |
    // |    | Flip+Scale Y, a------b  Rotate 90  |  |
    // d----c    Scale X                         |  |
    //                                           b--c
    y_scale = -0.5;
    x_scale = 1.5;
    matrix.reset();
    matrix.set_rotate(90.0, None);
    matrix.pre_scale((x_scale, y_scale), None);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    // Make scale factors positive for length comparisons
    y_scale = y_scale.abs();
    x_scale = x_scale.abs();
    {
        let d = dst.expect("scaled rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        reporter_assert!(reporter, dd.ul.x == y_scale * o.ul.y);
        reporter_assert!(reporter, dd.ul.y == x_scale * o.ul.x);
        reporter_assert!(reporter, dd.ur.x == y_scale * o.ll.y);
        reporter_assert!(reporter, dd.ur.y == x_scale * o.ll.x);
        reporter_assert!(reporter, dd.lr.x == y_scale * o.lr.y);
        reporter_assert!(reporter, dd.lr.y == x_scale * o.lr.x);
        reporter_assert!(reporter, dd.ll.x == y_scale * o.ur.y);
        reporter_assert!(reporter, dd.ll.y == x_scale * o.ur.x);
        // Width and height would get swapped, with the dst width half of the original height.
        reporter_assert!(reporter, x_scale * orig.rect().width() == d.rect().height());
        reporter_assert!(reporter, y_scale * orig.rect().height() == d.rect().width());
    }

    // a----b               c------d             b--c
    // |    |     ->        |      |      ->     |  |
    // |    | Flip+Scale Y, b------a  Rotate 90  |  |
    // d----c Flip+Scale X                       |  |
    //                                           a--d
    y_scale = -0.5;
    x_scale = -1.5;
    matrix.reset();
    matrix.set_rotate(90.0, None);
    matrix.pre_scale((x_scale, y_scale), None);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    // Make scale factors positive for length comparisons
    y_scale = y_scale.abs();
    x_scale = x_scale.abs();
    {
        let d = dst.expect("scaled rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        // With double-flip the corners rotate 90 degrees counter clockwise although the scale
        // factors are swapped.
        reporter_assert!(reporter, dd.ul.x == y_scale * o.ur.y);
        reporter_assert!(reporter, dd.ul.y == x_scale * o.ur.x);
        reporter_assert!(reporter, dd.ur.x == y_scale * o.lr.y);
        reporter_assert!(reporter, dd.ur.y == x_scale * o.lr.x);
        reporter_assert!(reporter, dd.lr.x == y_scale * o.ll.y);
        reporter_assert!(reporter, dd.lr.y == x_scale * o.ll.x);
        reporter_assert!(reporter, dd.ll.x == y_scale * o.ul.y);

        reporter_assert!(reporter, dd.ll.y == x_scale * o.ul.x);
    }
    // Width and height would get swapped, with the dst width half of the original height.
    reporter_assert!(
        reporter,
        x_scale * orig.rect().width() == dst.expect("rrect").rect().height()
    );
    reporter_assert!(
        reporter,
        y_scale * orig.rect().height() == dst.expect("rrect").rect().width()
    );

    // 90 CCW (270):
    // -------------
    // a----b               a------b              b--c
    // |    |     ->        |      |      ->      |  |
    // |    |   Scale Y,    d------c  Rotate 270  |  |
    // d----c   Scale X                           |  |
    //                                            a--d
    y_scale = 0.5;
    x_scale = 1.5;
    matrix.reset();
    matrix.set_rotate(270.0, None);
    matrix.pre_scale((x_scale, y_scale), None);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    // Make scale factors positive for length comparisons
    y_scale = y_scale.abs();
    x_scale = x_scale.abs();
    {
        let d = dst.expect("scaled rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        // Radii have cycled counter clock-wise and swapped their x and y axis. The dst x radii
        // should be scaled 1/2x compared to the y radii, dst y scaled 1.5 compared to src x.
        reporter_assert!(reporter, dd.ul.x == y_scale * o.ur.y);
        reporter_assert!(reporter, dd.ul.y == x_scale * o.ur.x);
        reporter_assert!(reporter, dd.ur.x == y_scale * o.lr.y);
        reporter_assert!(reporter, dd.ur.y == x_scale * o.lr.x);
        reporter_assert!(reporter, dd.lr.x == y_scale * o.ll.y);
        reporter_assert!(reporter, dd.lr.y == x_scale * o.ll.x);
        reporter_assert!(reporter, dd.ll.x == y_scale * o.ul.y);
        reporter_assert!(reporter, dd.ll.y == x_scale * o.ul.x);
    }
    // Width and height would get swapped, with the dst width half of the original height.
    let d = dst.expect("scaled rrect");
    reporter_assert!(reporter, x_scale * orig.rect().width() == d.rect().height());
    reporter_assert!(reporter, y_scale * orig.rect().height() == d.rect().width());

    // a----b               b------a              a--d
    // |    |     ->        |      |      ->      |  |
    // |    |   Scale Y,    c------d  Rotate 270  |  |
    // d----c Flip+Scale X                        |  |
    //                                            b--c
    y_scale = 0.5;
    x_scale = -1.5;
    matrix.reset();
    matrix.set_rotate(270.0, None);
    matrix.pre_scale((x_scale, y_scale), None);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    // Make scale factors positive for length comparisons
    y_scale = y_scale.abs();
    x_scale = x_scale.abs();
    {
        let d = dst.expect("scaled rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        reporter_assert!(reporter, dd.ul.x == y_scale * o.ul.y);
        reporter_assert!(reporter, dd.ul.y == x_scale * o.ul.x);
        reporter_assert!(reporter, dd.ur.x == y_scale * o.ll.y);
        reporter_assert!(reporter, dd.ur.y == x_scale * o.ll.x);
        reporter_assert!(reporter, dd.lr.x == y_scale * o.lr.y);
        reporter_assert!(reporter, dd.lr.y == x_scale * o.lr.x);
        reporter_assert!(reporter, dd.ll.x == y_scale * o.ur.y);
        reporter_assert!(reporter, dd.ll.y == x_scale * o.ur.x);
    }
    // Width and height would get swapped, with the dst width half of the original height.
    let d = dst.expect("scaled rrect");
    reporter_assert!(reporter, x_scale * orig.rect().width() == d.rect().height());
    reporter_assert!(reporter, y_scale * orig.rect().height() == d.rect().width());

    // a----b               d------c              c--b
    // |    |     ->        |      |      ->      |  |
    // |    | Flip+Scale Y, a------b  Rotate 270  |  |
    // d----c    Scale X                          |  |
    //                                            d--a
    y_scale = -0.5;
    x_scale = 1.5;
    matrix.reset();
    matrix.set_rotate(270.0, None);
    matrix.pre_scale((x_scale, y_scale), None);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    // Make scale factors positive for length comparisons
    y_scale = y_scale.abs();
    x_scale = x_scale.abs();
    {
        let d = dst.expect("scaled rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        reporter_assert!(reporter, dd.ul.x == y_scale * o.lr.y);
        reporter_assert!(reporter, dd.ul.y == x_scale * o.lr.x);
        reporter_assert!(reporter, dd.ur.x == y_scale * o.ur.y);
        reporter_assert!(reporter, dd.ur.y == x_scale * o.ur.x);
        reporter_assert!(reporter, dd.lr.x == y_scale * o.ul.y);
        reporter_assert!(reporter, dd.lr.y == x_scale * o.ul.x);
        reporter_assert!(reporter, dd.ll.x == y_scale * o.ll.y);
        reporter_assert!(reporter, dd.ll.y == x_scale * o.ll.x);
    }
    // Width and height would get swapped, with the dst width half of the original height.
    let d = dst.expect("scaled rrect");
    reporter_assert!(reporter, x_scale * orig.rect().width() == d.rect().height());
    reporter_assert!(reporter, y_scale * orig.rect().height() == d.rect().width());

    // a----b               c------d              d--a
    // |    |     ->        |      |      ->      |  |
    // |    | Flip+Scale Y, b------a  Rotate 270  |  |
    // d----c Flip+Scale X                        |  |
    //                                            c--b
    y_scale = -0.5;
    x_scale = -1.5;
    matrix.reset();
    matrix.set_rotate(270.0, None);
    matrix.pre_scale((x_scale, y_scale), None);
    dst = orig.transform(&matrix);
    reporter_assert!(reporter, dst.is_some());
    // Make scale factors positive for length comparisons
    y_scale = y_scale.abs();
    x_scale = x_scale.abs();
    {
        let d = dst.expect("scaled rrect");
        let o = Radii::of(orig);
        let dd = Radii::of(&d);
        // With double-flip the corners rotate 90 degrees clockwise although the scale factors
        // are swapped.
        reporter_assert!(reporter, dd.ul.x == y_scale * o.ll.y);
        reporter_assert!(reporter, dd.ul.y == x_scale * o.ll.x);
        reporter_assert!(reporter, dd.ur.x == y_scale * o.ul.y);
        reporter_assert!(reporter, dd.ur.y == x_scale * o.ul.x);
        reporter_assert!(reporter, dd.lr.x == y_scale * o.ur.y);
        reporter_assert!(reporter, dd.lr.y == x_scale * o.ur.x);
        reporter_assert!(reporter, dd.ll.x == y_scale * o.lr.y);
        reporter_assert!(reporter, dd.ll.y == x_scale * o.lr.x);
    }
    // Width and height would get swapped, with the dst width half of the original height.
    let d = dst.expect("scaled rrect");
    reporter_assert!(reporter, x_scale * orig.rect().width() == d.rect().height());
    reporter_assert!(reporter, y_scale * orig.rect().height() == d.rect().width());
}

// Port of: tests/RoundRectTest.cpp#L1264-L1294 (chrome/m156), test_round_rect_transform
fn test_round_rect_transform(reporter: &mut Reporter) {
    let mut rrect = RRect::new();
    {
        let r = Rect::new(0.0, 0.0, K_WIDTH, K_HEIGHT);
        rrect.set_rect_xy(r, 4.0, 7.0);
        test_transform_helper(reporter, &rrect);
    }
    {
        let r = Rect::new(5.0, 15.0, 27.0, 34.0);
        let radii = [
            Vector::new(0.0, 1.0),
            Vector::new(2.0, 3.0),
            Vector::new(4.0, 5.0),
            Vector::new(6.0, 7.0),
        ];
        rrect.set_rect_radii(r, &radii);
        test_transform_helper(reporter, &rrect);
    }
    {
        let r = Rect::new(760.0, 228.0, 1160.0, 1028.0);
        let radii = [
            Vector::new(400.0, 400.0),
            Vector::new(0.0, 0.0),
            Vector::new(0.0, 0.0),
            Vector::new(400.0, 400.0),
        ];
        rrect.set_rect_radii(r, &radii);
        test_transform_helper(reporter, &rrect);
    }
}

// Test out the case where an oval already off in space is translated/scaled
// further off into space - yielding numerical issues when the rect & radii
// are transformed separatly
// BUG=skbug.com/40033801
// Port of: tests/RoundRectTest.cpp#L1296-L1318 (chrome/m156), test_issue_2696
fn test_issue_2696(reporter: &mut Reporter) {
    let mut rrect = RRect::new();
    let r = Rect::new(28443.86, 53.14286, 28446.715, 56.000_004);
    rrect.set_oval(r);

    let mut xform = Matrix::default();
    xform.set_all(
        2.44, 0.0, 485_411.7, //
        0.0, 2.44, -438.7, //
        0.0, 0.0, 1.0,
    );

    let dst = rrect.transform(&xform);
    reporter_assert!(reporter, dst.is_some());
    let d = dst.expect("transformed rrect");

    let half_width = d.width() / 2.0;
    let half_height = d.height() / 2.0;

    for corner in CORNERS {
        reporter_assert!(
            reporter,
            f32::nearly_equal(d.radii(corner).x, half_width, None)
        );
        reporter_assert!(
            reporter,
            f32::nearly_equal(d.radii(corner).y, half_height, None)
        );
    }
}

// The `SkRRect` memory image of `rrect` (`writeToMemory`): the bounds and the corner radii, as
// the first 12 scalars of the struct.
fn rrect_memory_image(rrect: &RRect) -> Vec<u8> {
    let mut image = Vec::new();
    rrect.write_to_memory(&mut image);
    image
}

// Stores `value` as the scalar at `index` of the `SkRRect` memory image (the pointer stores of
// `test_read`, which write through `reinterpret_cast<SkScalar*>(&rrect)`).
fn set_scalar_in_image(image: &mut [u8], index: usize, value: scalar) {
    image[index * 4..index * 4 + 4].copy_from_slice(&value.to_ne_bytes());
}

// Port of: tests/RoundRectTest.cpp#L1320-L1332 (chrome/m156), test_read_rrect
fn test_read_rrect(reporter: &mut Reporter, buffer: &[u8], rrect: &RRect, should_equal_src: bool) {
    // It would be cleaner to call rrect.writeToMemory into a buffer. However, writeToMemory asserts
    // that the rrect is valid and our caller may have fiddled with the internals of rrect to make
    // it invalid. Here the caller's memory image is read.
    let mut deserialized = RRect::new();
    let size = deserialized.read_from_memory(buffer);
    reporter_assert!(reporter, size == RRect::SIZE_IN_MEMORY);
    reporter_assert!(reporter, deserialized.is_valid());
    if should_equal_src {
        reporter_assert!(reporter, *rrect == deserialized);
    }
}

// Port of: tests/RoundRectTest.cpp#L1334-L1380 (chrome/m156), test_read
fn test_read(reporter: &mut Reporter) {
    let k_rect = Rect::new(10.0, 10.0, 20.0, 20.0);
    let k_nan_rect = Rect::new(10.0, 10.0, 20.0, f32::NAN);
    let k_inf_rect = Rect::new(10.0, 10.0, f32::INFINITY, 20.0);

    test_read_rrect(
        reporter,
        &rrect_memory_image(&RRect::new_empty()),
        &RRect::new_empty(),
        true,
    );
    test_read_rrect(
        reporter,
        &rrect_memory_image(&RRect::new_rect(k_rect)),
        &RRect::new_rect(k_rect),
        true,
    );
    // These get coerced to empty.
    test_read_rrect(
        reporter,
        &rrect_memory_image(&RRect::new_rect(k_inf_rect)),
        &RRect::new_rect(k_inf_rect),
        true,
    );
    test_read_rrect(
        reporter,
        &rrect_memory_image(&RRect::new_rect(k_nan_rect)),
        &RRect::new_rect(k_nan_rect),
        true,
    );

    let mut rrect = RRect::new();
    rrect.set_rect(k_rect);
    // `innerRect`: the rect stored at the start of the struct.
    let mut image = rrect_memory_image(&rrect);
    set_rect_in_image(&mut image, k_inf_rect);
    test_read_rrect(reporter, &image, &rrect, false);
    set_rect_in_image(&mut image, k_nan_rect);
    test_read_rrect(reporter, &image, &rrect, false);

    test_read_rrect(
        reporter,
        &rrect_memory_image(&RRect::new_oval(k_rect)),
        &RRect::new_oval(k_rect),
        true,
    );
    test_read_rrect(
        reporter,
        &rrect_memory_image(&RRect::new_oval(k_inf_rect)),
        &RRect::new_oval(k_inf_rect),
        true,
    );
    test_read_rrect(
        reporter,
        &rrect_memory_image(&RRect::new_oval(k_nan_rect)),
        &RRect::new_oval(k_nan_rect),
        true,
    );
    rrect.set_oval(k_rect);
    let mut image = rrect_memory_image(&rrect);
    set_rect_in_image(&mut image, k_inf_rect);
    test_read_rrect(reporter, &image, &rrect, false);
    set_rect_in_image(&mut image, k_nan_rect);
    test_read_rrect(reporter, &image, &rrect, false);

    test_read_rrect(
        reporter,
        &rrect_memory_image(&RRect::new_rect_xy(k_rect, 5.0, 5.0)),
        &RRect::new_rect_xy(k_rect, 5.0, 5.0),
        true,
    );
    // rrect should scale down the radii to make this legal
    test_read_rrect(
        reporter,
        &rrect_memory_image(&RRect::new_rect_xy(k_rect, 5.0, 400.0)),
        &RRect::new_rect_xy(k_rect, 5.0, 400.0),
        true,
    );

    let k_radii = [
        Vector::new(0.5, 1.0),
        Vector::new(1.5, 2.0),
        Vector::new(2.5, 3.0),
        Vector::new(3.5, 4.0),
    ];
    rrect.set_rect_radii(k_rect, &k_radii);
    test_read_rrect(reporter, &rrect_memory_image(&rrect), &rrect, true);
    // `innerRadius`: the scalar at index 6 of the struct, which is `fRadii[1].fX`.
    let mut image = rrect_memory_image(&rrect);
    debug_assert_eq!(
        f32::from_ne_bytes(image[24..28].try_into().expect("4 bytes")),
        1.5
    );
    set_scalar_in_image(&mut image, 6, 400.0);
    test_read_rrect(reporter, &image, &rrect, false);
    set_scalar_in_image(&mut image, 6, f32::INFINITY);
    test_read_rrect(reporter, &image, &rrect, false);
    set_scalar_in_image(&mut image, 6, f32::NAN);
    test_read_rrect(reporter, &image, &rrect, false);
    set_scalar_in_image(&mut image, 6, -10.0);
    test_read_rrect(reporter, &image, &rrect, false);
}

// `*innerRect = rect` on the `SkRRect` memory image: the four bounds scalars.
fn set_rect_in_image(image: &mut [u8], rect: Rect) {
    set_scalar_in_image(image, 0, rect.left);
    set_scalar_in_image(image, 1, rect.top);
    set_scalar_in_image(image, 2, rect.right);
    set_scalar_in_image(image, 3, rect.bottom);
}

// Port of: tests/RoundRectTest.cpp#L1382-L1531 (chrome/m156), test_inner_bounds
fn test_inner_bounds(reporter: &mut Reporter) {
    // Because InnerBounds() insets the computed bounds slightly to correct for numerical inaccuracy
    // when finding the maximum inscribed point on a curve, we use a larger epsilon for comparing
    // expected areas.
    const K_EPSILON: scalar = 0.005;

    // Test that an empty rrect reports empty inner bounds
    reporter_assert!(
        reporter,
        rrect_priv::inner_bounds(&RRect::new_empty()).is_empty()
    );
    // Test that a rect rrect reports itself as the inner bounds
    let mut r = Rect::new(0.0, 1.0, 2.0, 3.0);
    reporter_assert!(reporter, rrect_priv::inner_bounds(&RRect::new_rect(r)) == r);
    // Test that a circle rrect has an inner bounds area equal to 2*radius^2
    let radius: f32 = 5.0;
    let mut inner =
        rrect_priv::inner_bounds(&RRect::new_oval(Rect::from_wh(2.0 * radius, 2.0 * radius)));
    reporter_assert!(
        reporter,
        f32::nearly_equal(
            inner.width() * inner.height(),
            2.0 * radius * radius,
            K_EPSILON
        )
    );

    let width: f32 = 20.0;
    let height: f32 = 25.0;
    r = Rect::from_wh(width, height);
    // Test that a rrect with circular corners has an area equal to:
    let expected_area = (2.0 * radius * radius) +                      // area in the 4 circular corners
        (width - 2.0 * radius) * (height - 2.0 * radius) +     // inner area excluding corners and edges
        SCALAR_SQRT2 * radius * (width - 2.0 * radius) + // two horiz. rects between corners
        SCALAR_SQRT2 * radius * (height - 2.0 * radius); // two vert. rects between corners

    inner = rrect_priv::inner_bounds(&RRect::new_rect_xy(r, radius, radius));
    reporter_assert!(
        reporter,
        f32::nearly_equal(inner.width() * inner.height(), expected_area, K_EPSILON)
    );

    // Test that a rrect with a small y radius but large x radius selects the horizontal interior
    let mut rr = RRect::new_rect_xy(r, 2.0 * radius, 0.1 * radius);
    reporter_assert!(
        reporter,
        rrect_priv::inner_bounds(&rr) == Rect::new(0.0, 0.1 * radius, width, height - 0.1 * radius)
    );
    // And vice versa with large y and small x radii
    rr = RRect::new_rect_xy(r, 0.1 * radius, 2.0 * radius);
    reporter_assert!(
        reporter,
        rrect_priv::inner_bounds(&rr) == Rect::new(0.1 * radius, 0.0, width - 0.1 * radius, height)
    );

    // Test a variety of complex round rects produce a non-empty rect that is at least contained,
    // and larger than the inner area avoiding all corners.
    let mut rng = Random::default();
    for _ in 0..1000 {
        let max_radius_x = rng.next_range_f(0.0, 40.0);
        let max_radius_y = rng.next_range_f(0.0, 40.0);

        let inner_width = rng.next_range_f(0.0, 40.0);
        let inner_height = rng.next_range_f(0.0, 40.0);

        let mut radii = [Vector::new(0.0, 0.0); 4];
        for corner in &mut radii {
            // The C++ evaluates the x then the y component, in order.
            corner.x = rng.next_range_f(0.0, max_radius_x);
            corner.y = rng.next_range_f(0.0, max_radius_y);
        }

        // std::max(a, b) is (a < b) ? b : a.
        let std_max = |a: scalar, b: scalar| if a < b { b } else { a };
        let max_left = std_max(radii[0].x, radii[3].x);
        let max_top = std_max(radii[0].y, radii[1].y);
        let max_right = std_max(radii[1].x, radii[2].x);
        let max_bottom = std_max(radii[2].y, radii[3].y);

        let outer = Rect::from_wh(
            max_left + max_right + inner_width,
            max_top + max_bottom + inner_height,
        );
        rr.set_rect_radii(outer, &radii);

        let max_inner = rrect_priv::inner_bounds(&rr);
        // Test upper limit on the size of 'maxInner'
        reporter_assert!(reporter, outer.contains(max_inner));
        reporter_assert!(reporter, rr.contains(max_inner));

        // Test lower limit on the size of 'maxInner'
        inner = Rect::from_xywh(max_left, max_top, inner_width, inner_height);
        inner.inset((K_EPSILON, K_EPSILON));

        if inner.is_sorted() {
            reporter_assert!(reporter, max_inner.contains(inner));
        } else {
            // Flipped from the inset, just test two points of inner
            let mid_x = max_left + 0.5 * inner_width;
            let mid_y = max_top + 0.5 * inner_height;
            reporter_assert!(reporter, max_inner.contains(Point::new(mid_x, max_top)));
            reporter_assert!(
                reporter,
                max_inner.contains(Point::new(mid_x, max_top + inner_height))
            );
            reporter_assert!(reporter, max_inner.contains(Point::new(max_left, mid_y)));
            reporter_assert!(
                reporter,
                max_inner.contains(Point::new(max_left + inner_width, mid_y))
            );
        }
    }
}

// Helper to test expected intersection, relying on the fact that all round rect intersections
// will have their bounds equal to the intersection of the bounds of the input round rects, and
// their corner radii will be a one of A's, B's, or rectangular.
// Port of: tests/RoundRectTest.cpp#L1534-L1541 (chrome/m156), CornerChoice
#[derive(Clone, Copy, PartialEq, Eq)]
enum CornerChoice {
    A,
    B,
    Rect,
}

// Port of: tests/RoundRectTest.cpp#L1548-L1575 (chrome/m156), verify_success
fn verify_success(
    reporter: &mut Reporter,
    a: &RRect,
    b: &RRect,
    tl: CornerChoice,
    tr: CornerChoice,
    br: CornerChoice,
    bl: CornerChoice,
) {
    let k_rect = RRect::new_empty(); // has (0,0) for all corners
    let pick = |choice: CornerChoice| match choice {
        CornerChoice::A => a,
        CornerChoice::B => b,
        CornerChoice::Rect => &k_rect,
    };

    // Compute expected round rect intersection given bounds of A and B, and the specified
    // corner choices for the 4 corners.
    let mut expected_bounds = Rect::default();
    let intersected = expected_bounds.intersect2(*a.rect(), *b.rect());
    assert!(intersected); // SkAssertResult

    let radii = [
        pick(tl).radii(Corner::UpperLeft),
        pick(tr).radii(Corner::UpperRight),
        pick(br).radii(Corner::LowerRight),
        pick(bl).radii(Corner::LowerLeft),
    ];
    // `expected` is only set up in the C++ test too (its assertions use the PathOps result).
    let mut expected = RRect::new();
    expected.set_rect_radii(expected_bounds, &radii);

    let actual = rrect_priv::conservative_intersect(a, b);
    // Intersections are commutative so ba and ab should be the same
    reporter_assert!(reporter, actual == rrect_priv::conservative_intersect(b, a));

    // Intersection of the result with either A or B should remain the intersection
    reporter_assert!(
        reporter,
        actual == rrect_priv::conservative_intersect(&actual, a)
    );
    reporter_assert!(
        reporter,
        actual == rrect_priv::conservative_intersect(&actual, b)
    );

    // Bounds of intersection round rect should equal intersection of bounds of a and b
    reporter_assert!(reporter, *actual.rect() == expected_bounds);

    // Use PathOps to confirm that the explicit round rect is correct.
    let a_path = Path::rrect(a, None);
    let b_path = Path::rrect(b, None);
    let result = op(&a_path, &b_path, PathOp::Intersect);
    assert!(result.is_some()); // SkAssertResult
    let expected_path = result.unwrap_or_default();

    // The isRRect() heuristics in SkPath are based on having called addRRect(), so a path from
    // path ops that is a rounded rectangle will return false. However, if test XOR expected is
    // empty, then we know that the shapes were the same.
    let test_path = Path::rrect(actual, None);

    let result = op(&test_path, &expected_path, PathOp::Xor);
    assert!(result.is_some()); // SkAssertResult
    reporter_assert!(reporter, result.expect("xor").is_empty());
}

// Port of: tests/RoundRectTest.cpp#L1577-L1587 (chrome/m156), verify_failure
fn verify_failure(reporter: &mut Reporter, a: &RRect, b: &RRect) {
    let intersection = rrect_priv::conservative_intersect(a, b);
    // Expected the intersection to fail (no intersection or complex intersection is not
    // disambiguated).
    reporter_assert!(reporter, intersection.is_empty());
    reporter_assert!(
        reporter,
        rrect_priv::conservative_intersect(b, a).is_empty()
    );
}

// Port of: tests/RoundRectTest.cpp#L1544-L1546 (chrome/m156), make_inset
fn make_inset(r: &RRect, dx: scalar, dy: scalar) -> RRect {
    let mut i = *r;
    i.inset((dx, dy));
    i
}

// Port of: tests/RoundRectTest.cpp#L1533-L1631 (chrome/m156), test_conservative_intersection
// Mirrors the single C++ test function; splitting it would not be a 1:1 port.
#[allow(clippy::too_many_lines)]
fn test_conservative_intersection(reporter: &mut Reporter) {
    use CornerChoice::{A as KA, B as KB, Rect as KRect};

    // A is a wide, short round rect
    let a = RRect::new_rect_xy(Rect::new(0.0, 4.0, 16.0, 12.0), 2.0, 2.0);
    // B is a narrow, tall round rect
    let b = RRect::new_rect_xy(Rect::new(4.0, 0.0, 12.0, 16.0), 3.0, 3.0);
    // NOTE: As positioned by default, A and B intersect as the rectangle {4, 4, 12, 12}.
    // There is a 2 px buffer between the corner curves of A and the vertical edges of B, and
    // a 1 px buffer between the corner curves of B and the horizontal edges of A. Since the shapes
    // form a symmetric rounded cross, we can easily test edge and corner combinations by simply
    // flipping signs and/or swapping x and y offsets.

    // Successful intersection operations:
    //  - for clarity these are formed by moving A around to intersect with B in different ways.
    //  - the expected bounds of the round rect intersection is calculated automatically
    //    in check_success, so all we have to specify are the expected corner radii

    // A and B intersect as a rectangle
    verify_success(reporter, &a, &b, KRect, KRect, KRect, KRect);
    // Move A to intersect B on a vertical edge, preserving two corners of A inside B
    verify_success(
        reporter,
        &a.with_offset((6.0, 0.0)),
        &b,
        KA,
        KRect,
        KRect,
        KA,
    );
    verify_success(
        reporter,
        &a.with_offset((-6.0, 0.0)),
        &b,
        KRect,
        KA,
        KA,
        KRect,
    );
    // Move B to intersect A on a horizontal edge, preserving two corners of B inside A
    verify_success(
        reporter,
        &a,
        &b.with_offset((0.0, 6.0)),
        KB,
        KB,
        KRect,
        KRect,
    );
    verify_success(
        reporter,
        &a,
        &b.with_offset((0.0, -6.0)),
        KRect,
        KRect,
        KB,
        KB,
    );
    // Move A to intersect B on a corner, preserving one corner of A and one of B
    verify_success(
        reporter,
        &a.with_offset((-7.0, -8.0)),
        &b,
        KB,
        KRect,
        KA,
        KRect,
    ); // TL of B
    verify_success(
        reporter,
        &a.with_offset((7.0, -8.0)),
        &b,
        KRect,
        KB,
        KRect,
        KA,
    ); // TR of B
    verify_success(
        reporter,
        &a.with_offset((7.0, 8.0)),
        &b,
        KA,
        KRect,
        KB,
        KRect,
    ); // BR of B
    verify_success(
        reporter,
        &a.with_offset((-7.0, 8.0)),
        &b,
        KRect,
        KA,
        KRect,
        KB,
    ); // BL of B
    // An inset is contained inside the original (note that SkRRect::inset modifies radii too) so
    // is returned unmodified when intersected.
    verify_success(reporter, &a, &make_inset(&a, 1.0, 1.0), KB, KB, KB, KB);
    verify_success(reporter, &make_inset(&b, 2.0, 2.0), &b, KA, KA, KA, KA);

    // A rectangle exactly matching the corners of the rrect bounds keeps the rrect radii,
    // regardless of whether or not it's the 1st or 2nd arg to ConservativeIntersect.
    let c = RRect::new_rect_xy(Rect::new(0.0, 0.0, 10.0, 10.0), 2.0, 2.0);
    let c_t = RRect::new_rect(Rect::new(0.0, 0.0, 10.0, 5.0));
    verify_success(reporter, &c, &c_t, KA, KA, KRect, KRect);
    verify_success(reporter, &c_t, &c, KB, KB, KRect, KRect);
    let c_b = RRect::new_rect(Rect::new(0.0, 5.0, 10.0, 10.0));
    verify_success(reporter, &c, &c_b, KRect, KRect, KA, KA);
    verify_success(reporter, &c_b, &c, KRect, KRect, KB, KB);
    let c_l = RRect::new_rect(Rect::new(0.0, 0.0, 5.0, 10.0));
    verify_success(reporter, &c, &c_l, KA, KRect, KRect, KA);
    verify_success(reporter, &c_l, &c, KB, KRect, KRect, KB);
    let c_r = RRect::new_rect(Rect::new(5.0, 0.0, 10.0, 10.0));
    verify_success(reporter, &c, &c_r, KRect, KA, KA, KRect);
    verify_success(reporter, &c_r, &c, KRect, KB, KB, KRect);

    // Failed intersection operations:

    // A and B's bounds do not intersect
    verify_failure(reporter, &a.with_offset((32.0, 0.0)), &b);
    // A and B's bounds intersect, but corner curves do not -> no intersection
    verify_failure(reporter, &a.with_offset((11.5, -11.5)), &b);
    // A is empty -> no intersection
    verify_failure(reporter, &RRect::new_empty(), &b);
    // A is contained in B, but is too close to the corner curves for the conservative
    // approximations to construct a valid round rect intersection.
    verify_failure(reporter, &make_inset(&b, 0.3, 0.3), &b);
    // A intersects a straight edge, but not far enough for B to contain A's corners
    verify_failure(reporter, &a.with_offset((2.5, 0.0)), &b);
    verify_failure(reporter, &a.with_offset((-2.5, 0.0)), &b);
    // And vice versa for B into A
    verify_failure(reporter, &a, &b.with_offset((0.0, 1.5)));
    verify_failure(reporter, &a, &b.with_offset((0.0, -1.5)));
    // A intersects a straight edge and part of B's corner
    verify_failure(reporter, &a.with_offset((5.0, -2.0)), &b);
    verify_failure(reporter, &a.with_offset((-5.0, -2.0)), &b);
    verify_failure(reporter, &a.with_offset((5.0, 2.0)), &b);
    verify_failure(reporter, &a.with_offset((-5.0, 2.0)), &b);
    // And vice versa
    verify_failure(reporter, &a, &b.with_offset((3.0, -5.0)));
    verify_failure(reporter, &a, &b.with_offset((-3.0, -5.0)));
    verify_failure(reporter, &a, &b.with_offset((3.0, 5.0)));
    verify_failure(reporter, &a, &b.with_offset((-3.0, 5.0)));
    // A intersects B on a corner, but the corner curves overlap each other
    verify_failure(reporter, &a.with_offset((8.0, 10.0)), &b);
    verify_failure(reporter, &a.with_offset((-8.0, 10.0)), &b);
    verify_failure(reporter, &a.with_offset((8.0, -10.0)), &b);
    verify_failure(reporter, &a.with_offset((-8.0, -10.0)), &b);

    // Another variant of corners overlapping, this is two circles of radius r that overlap by r
    // pixels (e.g. the leftmost point of the right circle touches the center of the left circle).
    // The key difference with the above case is that the intersection of the circle bounds have
    // corners that are contained in both circles, but because it is only r wide, can not satisfy
    // all corners having radii = r.
    let r: scalar = 100.0;
    let a = RRect::new_oval(Rect::from_wh(2.0 * r, 2.0 * r));
    verify_failure(reporter, &a, &a.with_offset((r, 0.0)));
}

// Port of: tests/RoundRectTest.cpp#L1633-L1650 (chrome/m156), RoundRect
def_test!(RoundRect, |reporter| {
    test_round_rect_basic(reporter);
    test_round_rect_rects(reporter);
    test_round_rect_ovals(reporter);
    test_round_rect_general(reporter);
    test_round_rect_iffy_parameters(reporter);
    test_inset(reporter);
    test_round_rect_contains_rect(reporter);
    test_round_rect_contains_point(reporter);
    test_round_rect_transform(reporter);
    test_issue_2696(reporter);
    test_tricky_radii(reporter);
    test_empty_crbug_458524(reporter);
    test_empty(reporter);
    test_read(reporter);
    test_inner_bounds(reporter);
    test_conservative_intersection(reporter);
});
