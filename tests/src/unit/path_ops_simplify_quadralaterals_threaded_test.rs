// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsSimplifyQuadralateralsThreadedTest.cpp (chrome/m156)

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

// Port of: tests/PathOpsSimplifyQuadralateralsThreadedTest.cpp#L20-L86 (chrome/m156)
fn test_simplify_quadralaterals_main(reporter: &mut Reporter, a: u8, b: u8, c: u8, d: u8) {
    let (ax, ay) = (i32::from(a & 0x03), i32::from(a >> 2));
    let (bx, by) = (i32::from(b & 0x03), i32::from(b >> 2));
    let (cx, cy) = (i32::from(c & 0x03), i32::from(c >> 2));
    let (dx, dy) = (i32::from(d & 0x03), i32::from(d >> 2));
    for e in 0..16u8 {
        let (ex, ey) = (i32::from(e & 0x03), i32::from(e >> 2));
        for f in e..16u8 {
            let (fx, fy) = (i32::from(f & 0x03), i32::from(f >> 2));
            for g in f..16u8 {
                let (gx, gy) = (i32::from(g & 0x03), i32::from(g >> 2));
                for h in g..16u8 {
                    let (hx, hy) = (i32::from(h & 0x03), i32::from(h >> 2));
                    let mut builder = PathBuilder::new();
                    builder
                        .move_to(pt(ax, ay))
                        .line_to(pt(bx, by))
                        .line_to(pt(cx, cy))
                        .line_to(pt(dx, dy))
                        .close()
                        .move_to(pt(ex, ey))
                        .line_to(pt(fx, fy))
                        .line_to(pt(gx, gy))
                        .line_to(pt(hx, hy))
                        .close();
                    let path = builder.detach();
                    test_simplify_threaded(reporter, &path, false);
                    // path.setFillType(kEvenOdd) followed by testSimplify(path, true, ...).
                    test_simplify_threaded(reporter, &path, true);
                }
            }
        }
    }
}

// Port of: tests/PathOpsSimplifyQuadralateralsThreadedTest.cpp#L88-L104 (chrome/m156)
def_test!(PathOpsSimplifyQuadralateralsThreaded, |reporter| {
    // Runnables are collected first, as the C++ appends them before render(). With
    // allowExtendedTest() false the enumeration stops after the first `c` iteration.
    let mut runnables = Vec::new();
    'finish: for a in 0..16u8 {
        for b in a..16 {
            for c in b..16 {
                for d in c..16 {
                    runnables.push((a, b, c, d));
                }
                if !reporter.allow_extended_test() {
                    break 'finish;
                }
            }
        }
    }
    for (a, b, c, d) in runnables {
        test_simplify_quadralaterals_main(reporter, a, b, c, d);
    }
});
