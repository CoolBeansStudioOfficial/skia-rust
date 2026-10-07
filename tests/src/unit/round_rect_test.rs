// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RoundRectTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_test, reporter_assert};
use skia_rust_core::float_bits::bits_to_float;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::rrect::rrect_priv::are_rect_and_radii_valid;

// Not ported yet (manifest stays `todo`): RoundRect (its test_conservative_intersection helper
// needs PathOps).

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
