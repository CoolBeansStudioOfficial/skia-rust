// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsOpCircleThreadedTest.cpp (chrome/m156)

// The threaded runners are run single-threaded, in the order the C++ runnables are appended, with
// the same case enumeration. The verbose output of the generated test sources is not ported.
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::cast_precision_loss
)]
#![cfg(test)]

use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_pathops::path_op::PathOp;

use crate::unit::path_ops_extended_test::test_path_op;
use crate::{Reporter, def_test};

/// `const SkPathFillType fts[]`.
// Port of: tests/PathOpsOpCircleThreadedTest.cpp#L22-L22 (chrome/m156)
const FTS: [PathFillType; 2] = [PathFillType::Winding, PathFillType::EvenOdd];

/// `for (int op = 0 ; op <= kXOR_SkPathOp; ++op)`.
const OPS: [PathOp; 4] = [
    PathOp::Difference,
    PathOp::Intersect,
    PathOp::Union,
    PathOp::Xor,
];

/// `d ? SkPathDirection::kCW : SkPathDirection::kCCW`.
fn direction(d: i32) -> PathDirection {
    if d != 0 {
        PathDirection::CW
    } else {
        PathDirection::CCW
    }
}

/// `SkPath::Circle(x, y, r, dir).makeFillType(fill)`.
fn circle(x: i32, y: i32, r: i32, dir: i32, fill: PathFillType) -> skia_rust_core::path::Path {
    let mut builder = PathBuilder::new();
    builder.add_circle((x as f32, y as f32), r as f32, direction(dir));
    builder.set_fill_type(fill);
    builder.detach()
}

// Port of: tests/PathOpsOpCircleThreadedTest.cpp#L24-L76 (chrome/m156)
fn test_op_circles_main(reporter: &mut Reporter, state: (i32, i32, i32, i32), test_no: &mut u32) {
    let (state_a, state_b, state_c, state_d) = state;
    for a in 0..6 {
        for b in a + 1..7 {
            for c in 0..6 {
                for d in c + 1..7 {
                    for &e in &FTS {
                        for &f in &FTS {
                            let path_a = circle(state_a, state_b, state_c, state_d, e);
                            let path_b = circle(a, b, c, d, f);
                            for op in OPS {
                                *test_no += 1;
                                let test_name = format!("thread_circles{test_no}");
                                test_path_op(reporter, &path_a, &path_b, op, &test_name);
                            }
                        }
                    }
                }
            }
        }
    }
}

// Port of: tests/PathOpsOpCircleThreadedTest.cpp#L78-L96 (chrome/m156)
def_test!(PathOpsOpCircleThreaded, |reporter| {
    // Runnables are collected first, as the C++ appends them before render(). With
    // allowExtendedTest() false the enumeration stops after the first `b` iteration.
    let mut runnables = Vec::new();
    'finish: for a in 0..6 {
        for b in a + 1..7 {
            for c in 0..6 {
                for d in 0..2 {
                    runnables.push((a, b, c, d));
                }
            }
            if !reporter.allow_extended_test() {
                break 'finish;
            }
        }
    }
    let mut test_no = 0;
    for state in runnables {
        test_op_circles_main(reporter, state, &mut test_no);
    }
});
