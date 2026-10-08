// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsOpRectThreadedTest.cpp (chrome/m156)

// The threaded runners are run single-threaded, in the order the C++ runnables are appended, with
// the same case enumeration. The verbose output of the generated test sources is not ported.
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::cast_precision_loss,
    clippy::too_many_lines
)]
#![cfg(test)]

use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::rect::Rect;
use skia_rust_pathops::path_op::PathOp;

use crate::unit::path_ops_extended_test::test_path_op;
use crate::{Reporter, def_test};

/// `const SkPathFillType fts[]` of the rects test.
const FTS: [PathFillType; 2] = [PathFillType::Winding, PathFillType::EvenOdd];

/// `const SkPathFillType fts[]` of the fast test.
const FAST_FTS: [PathFillType; 4] = [
    PathFillType::Winding,
    PathFillType::EvenOdd,
    PathFillType::InverseWinding,
    PathFillType::InverseEvenOdd,
];

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

// Port of: tests/PathOpsOpRectThreadedTest.cpp#L31-L72 (chrome/m156)
fn test_path_ops_rects_main(reporter: &mut Reporter, state: (i32, i32, i32, i32)) {
    let (state_a, state_b, state_c, state_d) = state;
    for a in 0..6 {
        for b in a + 1..7 {
            for c in 0..6 {
                for d in c + 1..7 {
                    for &e in &FTS {
                        for &f in &FTS {
                            let mut builder_a = PathBuilder::new_with_fill_type(e);
                            builder_a
                                .add_rect(
                                    Rect::new(s(state_a), s(state_a), s(state_b), s(state_b)),
                                    PathDirection::CW,
                                    None,
                                )
                                .add_rect(
                                    Rect::new(s(state_c), s(state_c), s(state_d), s(state_d)),
                                    PathDirection::CW,
                                    None,
                                )
                                .close();
                            let path_a = builder_a.detach();
                            let mut builder_b = PathBuilder::new_with_fill_type(f);
                            builder_b
                                .add_rect(
                                    Rect::new(s(a), s(a), s(b), s(b)),
                                    PathDirection::CW,
                                    None,
                                )
                                .add_rect(
                                    Rect::new(s(c), s(c), s(d), s(d)),
                                    PathDirection::CW,
                                    None,
                                )
                                .close();
                            let path_b = builder_b.detach();
                            for op in OPS {
                                test_path_op(reporter, &path_a, &path_b, op, "");
                            }
                        }
                    }
                }
            }
        }
    }
}

// Port of: tests/PathOpsOpRectThreadedTest.cpp#L74-L118 (chrome/m156)
def_test!(PathOpsRectsThreaded, |reporter| {
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
        test_path_ops_rects_main(reporter, state);
    }
});

// Port of: tests/PathOpsOpRectThreadedTest.cpp#L120-L196 (chrome/m156)
/// `testPathOpsFastMain`: `step` is 5 without `--extendedTest`.
fn test_path_ops_fast_main(reporter: &mut Reporter, state: (i32, i32, i32, i32)) {
    let (state_a, state_b, state_c, state_d) = state;
    let step = if reporter.allow_extended_test() { 2 } else { 5 };
    for a_flag in [false, true] {
        for b_flag in [false, true] {
            for c in (0..6).step_by(step) {
                for d in (0..6).step_by(step) {
                    for &e in &FAST_FTS {
                        for &f in &FAST_FTS {
                            let mut builder = PathBuilder::new_with_fill_type(e);
                            if a_flag {
                                builder.add_rect(
                                    Rect::new(
                                        s(state_a),
                                        s(state_a),
                                        s(state_b) + s(c),
                                        s(state_b),
                                    ),
                                    PathDirection::CW,
                                    None,
                                );
                            }
                            builder.close();
                            let path_a = builder.detach();
                            builder.set_fill_type(f);
                            if b_flag {
                                builder.add_rect(
                                    Rect::new(
                                        s(state_c),
                                        s(state_c),
                                        s(state_d) + s(d),
                                        s(state_d),
                                    ),
                                    PathDirection::CW,
                                    None,
                                );
                            }
                            builder.close();
                            let path_b: Path = builder.detach();
                            for op in OPS {
                                test_path_op(reporter, &path_a, &path_b, op, "");
                            }
                        }
                    }
                }
            }
        }
    }
}

// Port of: tests/PathOpsOpRectThreadedTest.cpp#L198-L217 (chrome/m156)
def_test!(PathOpsFastThreaded, |reporter| {
    let step = if reporter.allow_extended_test() { 2 } else { 5 };
    let mut runnables = Vec::new();
    for a in (0..6).step_by(step) {
        for b in (a + 1..7).step_by(step) {
            for c in (0..6).step_by(step) {
                for d in (c + 1..7).step_by(step) {
                    runnables.push((a, b, c, d));
                }
            }
        }
    }
    for state in runnables {
        test_path_ops_fast_main(reporter, state);
    }
});
