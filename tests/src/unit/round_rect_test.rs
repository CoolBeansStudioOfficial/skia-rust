// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RoundRectTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_test, reporter_assert};
use skia_rust_core::float_bits::bits_to_float;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::rrect::rrect_priv::are_rect_and_radii_valid;

// Skipped (need SkMatrix, SkPath or PathOps, none of which are ported yet):
//   RoundRect (calls test_round_rect_transform, test_issue_2696 and
//     test_empty_crbug_458524, which need SkMatrix, and test_conservative_intersection,
//     which needs SkPath/PathOps),
//   RRect_b561770646_part1 (SkPath::RRect), RRect_b561770646_part2 (SkMatrix, SkPath).

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
