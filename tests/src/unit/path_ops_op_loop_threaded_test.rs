// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsOpLoopThreadedTest.cpp (chrome/m156)

// The threaded runners are run single-threaded, in the order the C++ runnables are appended, with
// the same case enumeration. The verbose output of the generated test sources is not ported.
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::cast_precision_loss
)]
#![cfg(test)]

use skia_rust_core::path_builder::PathBuilder;
use skia_rust_pathops::path_op::PathOp;

use crate::unit::path_ops_extended_test::test_path_op;
use crate::{Reporter, def_test};

// Port of: tests/PathOpsOpLoopThreadedTest.cpp#L47-L92 (chrome/m156)
fn test_op_loops_main(reporter: &mut Reporter, state: (i32, i32, i32, i32)) {
    let (state_a, state_b, state_c, state_d) = state;
    for a in 0..6 {
        for b in a + 1..7 {
            for c in 0..6 {
                for d in c + 1..7 {
                    // define 4 points that form two lines that often cross; one line is (a, b)
                    // (c, d)
                    let v = ((a - c) as f32, (b - d) as f32);
                    let mid_a = (
                        (a * state_a + c * (6 - state_a)) as f32 / 6.0,
                        (b * state_a + d * (6 - state_a)) as f32 / 6.0,
                    );
                    let mid_b = (
                        (a * state_b + c * (6 - state_b)) as f32 / 6.0,
                        (b * state_b + d * (6 - state_b)) as f32 / 6.0,
                    );
                    let end_c = (
                        mid_a.0 + v.1 * state_c as f32 / 3.0,
                        mid_a.1 + v.0 * state_c as f32 / 3.0,
                    );
                    let end_d = (
                        mid_b.0 - v.1 * state_d as f32 / 3.0,
                        mid_b.1 + v.0 * state_d as f32 / 3.0,
                    );
                    let mut builder_a = PathBuilder::new();
                    builder_a
                        .move_to((a as f32, b as f32))
                        .cubic_to((c as f32, d as f32), end_c, end_d)
                        .close();
                    let path_a = builder_a.detach();
                    let mut builder_b = PathBuilder::new();
                    builder_b
                        .move_to((c as f32, d as f32))
                        .cubic_to(end_c, end_d, (a as f32, b as f32))
                        .close();
                    let path_b = builder_b.detach();
                    test_path_op(reporter, &path_a, &path_b, PathOp::Intersect, "");
                }
            }
        }
    }
}

// Port of: tests/PathOpsOpLoopThreadedTest.cpp#L94-L121 (chrome/m156)
def_test!(PathOpsOpLoopsThreaded, |reporter| {
    // Runnables are collected first, as the C++ appends them before render(). With
    // allowExtendedTest() false the enumeration stops after the first `b` iteration.
    let mut runnables = Vec::new();
    'finish: for a in 0..6 {
        for b in a + 1..7 {
            for c in 0..6 {
                for d in c + 1..7 {
                    runnables.push((a, b, c, d));
                }
            }
            if !reporter.allow_extended_test() {
                break 'finish;
            }
        }
    }
    for state in runnables {
        test_op_loops_main(reporter, state);
    }
});
