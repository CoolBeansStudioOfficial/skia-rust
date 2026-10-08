// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsSimplifyTrianglesThreadedTest.cpp (chrome/m156)

// The threaded runners are run single-threaded, in the order the C++ runnables are appended, with
// the same case enumeration. The verbose output of the generated test sources is not ported.
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::cast_precision_loss
)]
#![cfg(test)]

use skia_rust_core::path_builder::PathBuilder;

use crate::unit::path_ops_extended_test::test_simplify_threaded;
use crate::{Reporter, def_test};

/// `SkIntToScalar` of the 2-bit grid coordinates (exact in f32).
fn pt(x: i32, y: i32) -> (f32, f32) {
    (x as f32, y as f32)
}

// Port of: tests/PathOpsSimplifyTrianglesThreadedTest.cpp#L18-L75 (chrome/m156)
fn test_simplify_triangles_main(reporter: &mut Reporter, a: u8, b: u8, c: u8) {
    let (ax, ay) = (i32::from(a & 0x03), i32::from(a >> 2));
    let (bx, by) = (i32::from(b & 0x03), i32::from(b >> 2));
    let (cx, cy) = (i32::from(c & 0x03), i32::from(c >> 2));
    for d in 0..15u8 {
        let (dx, dy) = (i32::from(d & 0x03), i32::from(d >> 2));
        for e in d + 1..16 {
            let (ex, ey) = (i32::from(e & 0x03), i32::from(e >> 2));
            for f in d + 1..16 {
                if e == f {
                    continue;
                }
                let (fx, fy) = (i32::from(f & 0x03), i32::from(f >> 2));
                if (ex - dx) * (fy - dy) == (ey - dy) * (fx - dx) {
                    continue;
                }
                let mut builder = PathBuilder::new();
                builder
                    .move_to(pt(ax, ay))
                    .line_to(pt(bx, by))
                    .line_to(pt(cx, cy))
                    .close()
                    .move_to(pt(dx, dy))
                    .line_to(pt(ex, ey))
                    .line_to(pt(fx, fy))
                    .close();
                let path = builder.detach();
                test_simplify_threaded(reporter, &path, false);
                // path.setFillType(kEvenOdd) followed by testSimplify(path, true, ...).
                test_simplify_threaded(reporter, &path, true);
            }
        }
    }
}

// Port of: tests/PathOpsSimplifyTrianglesThreadedTest.cpp#L77-L103 (chrome/m156)
def_test!(PathOpsSimplifyTrianglesThreaded, |reporter| {
    // Runnables are collected first, as the C++ appends them before render(). With
    // allowExtendedTest() false the enumeration stops after the first `b` iteration.
    let mut runnables = Vec::new();
    'finish: for a in 0..15u8 {
        for b in a + 1..16 {
            for c in a + 1..16 {
                if b == c {
                    continue;
                }
                let (ax, ay) = (i32::from(a & 0x03), i32::from(a >> 2));
                let (bx, by) = (i32::from(b & 0x03), i32::from(b >> 2));
                let (cx, cy) = (i32::from(c & 0x03), i32::from(c >> 2));
                if (bx - ax) * (cy - ay) == (by - ay) * (cx - ax) {
                    continue;
                }
                runnables.push((a, b, c));
            }
            if !reporter.allow_extended_test() {
                break 'finish;
            }
        }
    }
    for (a, b, c) in runnables {
        test_simplify_triangles_main(reporter, a, b, c);
    }
});
