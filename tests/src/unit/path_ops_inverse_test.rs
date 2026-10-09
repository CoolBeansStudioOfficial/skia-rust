// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsInverseTest.cpp (chrome/m156)
#![cfg(test)]

use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::rect::Rect;
use skia_rust_pathops::path_op::PathOp;

use crate::def_test;
use crate::unit::path_ops_extended_test::test_path_op;

/// `SkPath::Rect(r, dir).makeFillType(fill)`.
fn rect_path(r: Rect, dir: PathDirection, fill: PathFillType) -> skia_rust_core::path::Path {
    let mut builder = PathBuilder::new();
    builder.add_rect(r, dir, None);
    builder.set_fill_type(fill);
    builder.detach()
}

// Port of: tests/PathOpsInverseTest.cpp#L8-L36 (chrome/m156)
def_test!(PathOpsInverse, |reporter| {
    let dirs = [PathDirection::CW, PathDirection::CCW];
    let fts = [
        PathFillType::Winding,
        PathFillType::EvenOdd,
        PathFillType::InverseWinding,
        PathFillType::InverseEvenOdd,
    ];
    let mut test_count = 0;
    for op in [
        PathOp::Difference,
        PathOp::Intersect,
        PathOp::Union,
        PathOp::Xor,
        PathOp::ReverseDifference,
    ] {
        for one_fill in fts {
            for one_dir in dirs {
                let one = rect_path(Rect::new(0., 0., 6., 6.), one_dir, one_fill);
                for two_fill in fts {
                    for two_dir in dirs {
                        let two = rect_path(Rect::new(3., 3., 9., 9.), two_dir, two_fill);
                        test_count += 1;
                        let test_name = format!("inverseTest{test_count}");
                        test_path_op(reporter, &one, &two, op, &test_name);
                    }
                }
            }
        }
    }
});
