// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RRectInPathTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::float_bits::bits_to_float;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_priv;
use skia_rust_core::path_ref::PathRRectInfo;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::{RRect, Type};
use skia_rust_core::scalar::scalar;

// Port of: tests/RRectInPathTest.cpp#L28-L50 (chrome/m156)
fn path_contains_rrect(reporter: &mut Reporter, path: &Path) -> PathRRectInfo {
    let info = path_priv::is_rrect(path);
    reporter_assert!(reporter, info.is_some());
    let info = info.expect("path was built as a rrect");
    let recreated_path =
        Path::rrect_with_start_index(info.rrect, info.direction, usize::from(info.start_index));
    reporter_assert!(reporter, *path == recreated_path);
    // Test that rotations/mirrors of the rrect path are still rrect paths and the returned
    // parameters for the transformed paths are correct.
    let matrices = [
        Matrix::scale((1.0, 1.0)),
        Matrix::scale((-1.0, 1.0)),
        Matrix::scale((1.0, -1.0)),
        Matrix::scale((-1.0, -1.0)),
    ];
    for m in &matrices {
        let xformed = path.make_transform(m);
        let xinfo = path_priv::is_rrect(&xformed);
        reporter_assert!(reporter, xinfo.is_some());
        let xinfo = xinfo.expect("transformed path is still a rrect");
        let recreated_path = Path::rrect_with_start_index(
            xinfo.rrect,
            xinfo.direction,
            usize::from(xinfo.start_index),
        );
        reporter_assert!(reporter, recreated_path == xformed);
    }
    info
}

// Port of: tests/RRectInPathTest.cpp#L52-L65 (chrome/m156)
fn inner_path_contains_rrect(
    reporter: &mut Reporter,
    input: &RRect,
    dir: PathDirection,
    start: usize,
) -> RRect {
    match input.get_type() {
        Type::Empty | Type::Rect | Type::Oval => return *input,
        _ => {}
    }
    let path = Path::rrect_with_start_index(input, dir, start);
    let rrect = path_contains_rrect(reporter, &path);
    reporter_assert!(
        reporter,
        rrect.direction == dir && usize::from(rrect.start_index) == start
    );
    rrect.rrect
}

// Port of: tests/RRectInPathTest.cpp#L67-L74 (chrome/m156)
fn path_contains_rrect_check(
    reporter: &mut Reporter,
    input: &RRect,
    dir: PathDirection,
    start: usize,
) {
    let out = inner_path_contains_rrect(reporter, input, dir, start);
    // (C++: `if (in != out) SkDebugf("%s", "");`)
    reporter_assert!(reporter, *input == out);
}

// Port of: tests/RRectInPathTest.cpp#L76-L82 (chrome/m156)
fn path_contains_rrect_nocheck(
    reporter: &mut Reporter,
    input: &RRect,
    dir: PathDirection,
    start: usize,
) {
    let _out = inner_path_contains_rrect(reporter, input, dir, start);
    // (C++: `if (in == out) SkDebugf("%s", "");`)
}

// Port of: tests/RRectInPathTest.cpp#L84-L89 (chrome/m156)
fn path_contains_rrect_check_radii(
    reporter: &mut Reporter,
    r: &Rect,
    v: &[Vector; 4],
    dir: PathDirection,
    start: usize,
) {
    let mut rrect = RRect::default();
    rrect.set_rect_radii(r, v);
    path_contains_rrect_check(reporter, &rrect, dir, start);
}

const WIDTH: scalar = 100.0;
const HEIGHT: scalar = 100.0;

const DIRS: [PathDirection; 2] = [PathDirection::CW, PathDirection::CCW];

// Port of: tests/RRectInPathTest.cpp#L94-L120 (chrome/m156)
fn test_tricky_radii(reporter: &mut Reporter) {
    for dir in DIRS {
        for start in 0..8 {
            {
                // crbug.com/458522
                let mut rr = RRect::default();
                let bounds = Rect::new(3709.0, 3709.0, 3709.0 + 7402.0, 3709.0 + 29825.0);
                let rad = 12814.0;
                let vec = [
                    Vector::new(rad, rad),
                    Vector::new(0.0, rad),
                    Vector::new(rad, rad),
                    Vector::new(0.0, rad),
                ];
                rr.set_rect_radii(bounds, &vec);
                path_contains_rrect_check(reporter, &rr, dir, start);
            }

            {
                // crbug.com//463920
                let r = Rect::from_ltrb(0.0, 0.0, 1009.0, 33_554_432.0);
                let radii = [
                    Vector::new(13.0, 8.0),
                    Vector::new(170.0, 2.0),
                    Vector::new(256.0, 33_554_432.0),
                    Vector::new(110.0, 5.0),
                ];
                let mut rr = RRect::default();
                rr.set_rect_radii(r, &radii);
                path_contains_rrect_nocheck(reporter, &rr, dir, start);
            }
        }
    }
}

// Port of: tests/RRectInPathTest.cpp#L122-L138 (chrome/m156)
fn test_empty_crbug_458524(reporter: &mut Reporter) {
    for dir in DIRS {
        for start in 0..8 {
            let mut rr = RRect::default();
            let bounds = Rect::new(3709.0, 3709.0, 3709.0 + 7402.0, 3709.0 + 29825.0);
            let rad = 40.0;
            rr.set_rect_xy(bounds, rad, rad);
            path_contains_rrect_check(reporter, &rr, dir, start);

            let mut matrix = Matrix::default();
            matrix.set_scale((0.0, 1.0), None);
            // (void)rr.transform(matrix): the const transform does not change rr.
            let _ = rr.transform(&matrix);
            path_contains_rrect_check(reporter, &rr, dir, start);
        }
    }
}

// Port of: tests/RRectInPathTest.cpp#L140-L163 (chrome/m156)
fn test_inset(reporter: &mut Reporter) {
    for dir in DIRS {
        for start in 0..8 {
            let mut rr = RRect::default();
            let r = Rect::new(0.0, 0.0, 100.0, 100.0);

            // `rr.inset(dx, dy, &rr2)` writes rr2 and leaves rr unchanged.
            rr.set_rect(r);
            let _rr2 = rr.with_inset((-20.0, -20.0));
            path_contains_rrect_check(reporter, &rr, dir, start);

            let _rr2 = rr.with_inset((20.0, 20.0));
            path_contains_rrect_check(reporter, &rr, dir, start);

            let _rr2 = rr.with_inset((r.width() / 2.0, r.height() / 2.0));
            path_contains_rrect_check(reporter, &rr, dir, start);

            rr.set_rect_xy(r, 20.0, 20.0);
            let _rr2 = rr.with_inset((19.0, 19.0));
            path_contains_rrect_check(reporter, &rr, dir, start);
            let _rr2 = rr.with_inset((20.0, 20.0));
            path_contains_rrect_check(reporter, &rr, dir, start);
        }
    }
}

// Port of: tests/RRectInPathTest.cpp#L166-L190 (chrome/m156)
fn test_9patch_rrect(
    reporter: &mut Reporter,
    rect: &Rect,
    l: scalar,
    t: scalar,
    r: scalar,
    b: scalar,
    check_radii: bool,
) {
    for dir in DIRS {
        for start in 0..8 {
            let mut rr = RRect::default();
            rr.set_nine_patch(rect, l, t, r, b);
            if check_radii {
                path_contains_rrect_check(reporter, &rr, dir, start);
            } else {
                path_contains_rrect_nocheck(reporter, &rr, dir, start);
            }

            let mut rr2 = RRect::default(); // construct the same RR using the most general set function
            let radii = [
                Vector::new(l, t),
                Vector::new(r, t),
                Vector::new(r, b),
                Vector::new(l, b),
            ];
            rr2.set_rect_radii(rect, &radii);
            if check_radii {
                path_contains_rrect_check(reporter, &rr, dir, start);
            } else {
                path_contains_rrect_nocheck(reporter, &rr, dir, start);
            }
        }
    }
}

// Test out the basic API entry points
// Port of: tests/RRectInPathTest.cpp#L193-L268 (chrome/m156)
#[allow(clippy::excessive_precision)] // float literals are copied verbatim from the C++
fn test_round_rect_basic(reporter: &mut Reporter) {
    for dir in DIRS {
        for start in 0..8 {
            //----
            let rect = Rect::from_ltrb(0.0, 0.0, WIDTH, HEIGHT);

            let mut rr1 = RRect::default();
            rr1.set_rect(rect);
            path_contains_rrect_check(reporter, &rr1, dir, start);

            let mut rr1_2 = RRect::default(); // construct the same RR using the most general set function
            let rr1_2_radii = [Vector::new(0.0, 0.0); 4];
            rr1_2.set_rect_radii(rect, &rr1_2_radii);
            path_contains_rrect_check(reporter, &rr1_2, dir, start);
            let mut rr1_3 = RRect::default(); // construct the same RR using the nine patch set function
            rr1_3.set_nine_patch(rect, 0.0, 0.0, 0.0, 0.0);
            path_contains_rrect_check(reporter, &rr1_2, dir, start);

            //----
            let half_point = Point::new(WIDTH / 2.0, HEIGHT / 2.0);
            let mut rr2 = RRect::default();
            rr2.set_oval(rect);
            path_contains_rrect_check(reporter, &rr2, dir, start);

            let mut rr2_2 = RRect::default(); // construct the same RR using the most general set function
            let rr2_2_radii = [Vector::new(half_point.x, half_point.y); 4];
            rr2_2.set_rect_radii(rect, &rr2_2_radii);
            path_contains_rrect_check(reporter, &rr2_2, dir, start);
            let mut rr2_3 = RRect::default(); // construct the same RR using the nine patch set function
            rr2_3.set_nine_patch(rect, half_point.x, half_point.y, half_point.x, half_point.y);
            path_contains_rrect_check(reporter, &rr2_3, dir, start);

            //----
            let p = Point::new(5.0, 5.0);
            let mut rr3 = RRect::default();
            rr3.set_rect_xy(rect, p.x, p.y);
            path_contains_rrect_check(reporter, &rr3, dir, start);

            let mut rr3_2 = RRect::default(); // construct the same RR using the most general set function
            let rr3_2_radii = [Vector::new(5.0, 5.0); 4];
            rr3_2.set_rect_radii(rect, &rr3_2_radii);
            path_contains_rrect_check(reporter, &rr3_2, dir, start);
            let mut rr3_3 = RRect::default(); // construct the same RR using the nine patch set function
            rr3_3.set_nine_patch(rect, 5.0, 5.0, 5.0, 5.0);
            path_contains_rrect_check(reporter, &rr3_3, dir, start);

            //----
            test_9patch_rrect(reporter, &rect, 10.0, 9.0, 8.0, 7.0, true);

            {
                // Test out the rrect from skbug.com/40034587
                let rect2 =
                    Rect::from_ltrb(0.358_211_994, 0.755_430_222, 0.872_866_154, 0.806_214_333);

                test_9patch_rrect(
                    reporter,
                    &rect2,
                    0.926_942_348,
                    0.642_850_280,
                    0.529_063_463,
                    0.587_844_372,
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

            let mut rr5 = RRect::default();
            rr5.set_rect_radii(rect, &radii2);
            path_contains_rrect_check(reporter, &rr5, dir, start);
        }
    }
}

// Test out the cases when the RR degenerates to a rect
// Port of: tests/RRectInPathTest.cpp#L271-L298 (chrome/m156)
fn test_round_rect_rects(reporter: &mut Reporter) {
    for dir in DIRS {
        for start in 0..8 {
            //----
            let rect = Rect::from_ltrb(0.0, 0.0, WIDTH, HEIGHT);
            let mut rr1 = RRect::default();
            rr1.set_rect_xy(rect, 0.0, 0.0);

            path_contains_rrect_check(reporter, &rr1, dir, start);

            //----
            let radii = [Point::new(0.0, 0.0); 4];

            let mut rr2 = RRect::default();
            rr2.set_rect_radii(rect, &radii);

            path_contains_rrect_check(reporter, &rr2, dir, start);

            //----
            let radii2 = [
                Point::new(0.0, 0.0),
                Point::new(20.0, 20.0),
                Point::new(50.0, 50.0),
                Point::new(20.0, 50.0),
            ];

            let mut rr3 = RRect::default();
            rr3.set_rect_radii(rect, &radii2);
            path_contains_rrect_check(reporter, &rr3, dir, start);
        }
    }
}

// Test out the cases when the RR degenerates to an oval
// Port of: tests/RRectInPathTest.cpp#L301-L312 (chrome/m156)
fn test_round_rect_ovals(reporter: &mut Reporter) {
    for dir in DIRS {
        for start in 0..8 {
            //----
            let rect = Rect::from_ltrb(0.0, 0.0, WIDTH, HEIGHT);
            let mut rr1 = RRect::default();
            rr1.set_rect_xy(rect, WIDTH / 2.0, HEIGHT / 2.0);

            path_contains_rrect_check(reporter, &rr1, dir, start);
        }
    }
}

// Test out the non-degenerate RR cases
// Port of: tests/RRectInPathTest.cpp#L315-L334 (chrome/m156)
fn test_round_rect_general(reporter: &mut Reporter) {
    for dir in DIRS {
        for start in 0..8 {
            //----
            let rect = Rect::from_ltrb(0.0, 0.0, WIDTH, HEIGHT);
            let mut rr1 = RRect::default();
            rr1.set_rect_xy(rect, 20.0, 20.0);

            path_contains_rrect_check(reporter, &rr1, dir, start);

            //----
            let radii = [
                Point::new(0.0, 0.0),
                Point::new(20.0, 20.0),
                Point::new(50.0, 50.0),
                Point::new(20.0, 50.0),
            ];

            let mut rr2 = RRect::default();
            rr2.set_rect_radii(rect, &radii);

            path_contains_rrect_check(reporter, &rr2, dir, start);
        }
    }
}

// Port of: tests/RRectInPathTest.cpp#L336-L346 (chrome/m156)
fn test_round_rect_iffy_parameters(reporter: &mut Reporter) {
    for dir in DIRS {
        for start in 0..8 {
            let rect = Rect::from_ltrb(0.0, 0.0, WIDTH, HEIGHT);
            let radii = [
                Point::new(50.0, 100.0),
                Point::new(100.0, 50.0),
                Point::new(50.0, 100.0),
                Point::new(100.0, 50.0),
            ];
            let mut rr1 = RRect::default();
            rr1.set_rect_radii(rect, &radii);
            path_contains_rrect_nocheck(reporter, &rr1, dir, start);
        }
    }
}

// Port of: tests/RRectInPathTest.cpp#L348-L351 (chrome/m156)
fn set_radii(radii: &mut [Vector; 4], index: usize, rad: f32) {
    *radii = [Vector::new(0.0, 0.0); 4];
    radii[index].set(rad, rad);
}

// Port of: tests/RRectInPathTest.cpp#L353-L372 (chrome/m156)
#[allow(clippy::similar_names)] // names follow the C++
fn test_skbug_3239(reporter: &mut Reporter) {
    let min = bits_to_float(0xcb7f_16c8); /* -16717512.000000 */
    let max = bits_to_float(0x4b7f_1c1d); /*  16718877.000000 */
    let big = bits_to_float(0x4b7f_1bd7); /*  16718807.000000 */

    let rad = 33_436_320.0;

    let rectx = Rect::from_ltrb(min, min, max, big);
    let recty = Rect::from_ltrb(min, min, big, max);

    for dir in DIRS {
        for start in 0..8 {
            let mut radii = [Vector::default(); 4];
            for i in 0..4 {
                set_radii(&mut radii, i, rad);
                path_contains_rrect_check_radii(reporter, &rectx, &radii, dir, start);
                path_contains_rrect_check_radii(reporter, &recty, &radii, dir, start);
            }
        }
    }
}

// Port of: tests/RRectInPathTest.cpp#L374-L385 (chrome/m156)
fn test_mix(reporter: &mut Reporter) {
    for dir in DIRS {
        for start in 0..8 {
            // Test out mixed degenerate and non-degenerate geometry with Conics
            let radii = [
                Vector::new(0.0, 0.0),
                Vector::new(0.0, 0.0),
                Vector::new(0.0, 0.0),
                Vector::new(100.0, 100.0),
            ];
            let r = Rect::from_wh(100.0, 100.0);
            let mut rr = RRect::default();
            rr.set_rect_radii(r, &radii);
            path_contains_rrect_check(reporter, &rr, dir, start);
        }
    }
}

// Port of: tests/RRectInPathTest.cpp#L387-L398 (chrome/m156)
def_test!(RoundRectInPath, |reporter| {
    test_tricky_radii(reporter);
    test_empty_crbug_458524(reporter);
    test_inset(reporter);
    test_round_rect_basic(reporter);
    test_round_rect_rects(reporter);
    test_round_rect_ovals(reporter);
    test_round_rect_general(reporter);
    test_round_rect_iffy_parameters(reporter);
    test_skbug_3239(reporter);
    test_mix(reporter);
});

// Port of: tests/RRectInPathTest.cpp#L400-L415 (chrome/m156)
def_test!(RRect_fragile, |_reporter| {
    let rect = Rect::new(
        bits_to_float(0x1f80_0000), // 0x003F0000 was the starter value that also fails
        bits_to_float(0x1400_001C),
        bits_to_float(0x3F00_0004),
        bits_to_float(0x3F00_0004),
    );

    let radii = [
        Point::new(bits_to_float(0x0000_0001), bits_to_float(0x0000_0001)),
        Point::new(bits_to_float(0x0000_0020), bits_to_float(0x0000_0001)),
        Point::new(bits_to_float(0x0000_0000), bits_to_float(0x0000_0000)),
        Point::new(bits_to_float(0x3F00_0004), bits_to_float(0x3F00_0004)),
    ];

    let mut rr = RRect::default();
    // please don't assert
    // C++: `if ((false))` -- disabled until we fix this
    #[allow(clippy::overly_complex_bool_expr)]
    if false {
        rr.set_rect_radii(rect, &radii);
    }
    let _ = (&rr, &rect, &radii);
});
