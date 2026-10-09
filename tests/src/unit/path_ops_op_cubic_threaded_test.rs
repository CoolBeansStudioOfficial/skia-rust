// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsOpCubicThreadedTest.cpp (chrome/m156)

// The threaded runners are run single-threaded, in the order the C++ runnables are appended, with
// the same case enumeration. The verbose output of the generated test sources is not ported.
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::cast_precision_loss
)]
#![cfg(test)]

use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_pathops::path_op::PathOp;

use crate::unit::path_ops_extended_test::test_path_op;
use crate::{Reporter, def_test};

/// `const SkPathFillType fts[]`.
const FTS: [PathFillType; 2] = [PathFillType::Winding, PathFillType::EvenOdd];

/// `for (int op = 0 ; op <= kXOR_SkPathOp; ++op)`.
const OPS: [PathOp; 4] = [
    PathOp::Difference,
    PathOp::Intersect,
    PathOp::Union,
    PathOp::Xor,
];

/// `SkIntToScalar` of the small integer coordinates (exact in f32).
fn s(v: i32) -> f32 {
    v as f32
}

// Port of: tests/PathOpsOpCubicThreadedTest.cpp#L17-L60 (chrome/m156)
fn test_op_cubics_main(reporter: &mut Reporter, state: (i32, i32, i32, i32), test_no: &mut u32) {
    let (state_a, state_b, state_c, state_d) = state;
    for a in 0..6 {
        for b in a + 1..7 {
            for c in 0..6 {
                for d in c + 1..7 {
                    for &e in &FTS {
                        for &f in &FTS {
                            let mut builder_a = PathBuilder::new_with_fill_type(e);
                            builder_a
                                .move_to((s(state_a), s(state_b)))
                                .cubic_to((s(state_c), s(state_d)), (s(b), s(a)), (s(d), s(c)))
                                .close();
                            let path_a = builder_a.detach();
                            let mut builder_b = PathBuilder::new_with_fill_type(f);
                            builder_b
                                .move_to((s(a), s(b)))
                                .cubic_to(
                                    (s(c), s(d)),
                                    (s(state_b), s(state_a)),
                                    (s(state_d), s(state_c)),
                                )
                                .close();
                            let path_b = builder_b.detach();
                            for op in OPS {
                                *test_no += 1;
                                let test_name = format!("thread_cubics{test_no}");
                                test_path_op(reporter, &path_a, &path_b, op, &test_name);
                            }
                        }
                    }
                }
            }
        }
    }
}

// Port of: tests/PathOpsOpCubicThreadedTest.cpp#L62-L86 (chrome/m156)
def_test!(PathOpsOpCubicsThreaded, |reporter| {
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
    let mut test_no = 0;
    for state in runnables {
        test_op_cubics_main(reporter, state, &mut test_no);
    }
});
