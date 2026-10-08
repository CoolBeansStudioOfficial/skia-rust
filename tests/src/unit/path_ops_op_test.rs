// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsOpTest.cpp (chrome/m156), partially: the rectangle tests and testOp1d and
// testOp2d. The rest of the file is not ported yet (see notes/pathops-op-graph.md).

#![cfg(test)]

use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::rect::Rect;
use skia_rust_pathops::path_op::PathOp;

use crate::unit::path_ops_extended_test::test_path_op;
use crate::{Reporter, def_test};

/// `SkPath::Rect({l, t, r, b}, SkPathDirection::kCW)`.
fn rect_path(l: f32, t: f32, r: f32, b: f32) -> Path {
    Path::rect(Rect::new(l, t, r, b), PathDirection::CW)
}

// Port of: tests/PathOpsOpTest.cpp#L392-L397 (chrome/m156)
fn test_intersect1(reporter: &mut Reporter, filename: &str) {
    let one = rect_path(0.0, 0.0, 6.0, 6.0);
    let two = rect_path(3.0, 3.0, 9.0, 9.0);
    test_path_op(reporter, &one, &two, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L399-L403 (chrome/m156)
fn test_union1(reporter: &mut Reporter, filename: &str) {
    let one = rect_path(0.0, 0.0, 6.0, 6.0);
    let two = rect_path(3.0, 3.0, 9.0, 9.0);
    test_path_op(reporter, &one, &two, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L405-L409 (chrome/m156)
fn test_diff1(reporter: &mut Reporter, filename: &str) {
    let one = rect_path(0.0, 0.0, 6.0, 6.0);
    let two = rect_path(3.0, 3.0, 9.0, 9.0);
    test_path_op(reporter, &one, &two, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L411-L415 (chrome/m156)
fn test_xor1(reporter: &mut Reporter, filename: &str) {
    let one = rect_path(0.0, 0.0, 6.0, 6.0);
    let two = rect_path(3.0, 3.0, 9.0, 9.0);
    test_path_op(reporter, &one, &two, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L416-L421 (chrome/m156)
fn test_intersect2(reporter: &mut Reporter, filename: &str) {
    let one = rect_path(0.0, 0.0, 6.0, 6.0);
    let two = rect_path(0.0, 3.0, 9.0, 9.0);
    test_path_op(reporter, &one, &two, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L422-L427 (chrome/m156)
fn test_union2(reporter: &mut Reporter, filename: &str) {
    let one = rect_path(0.0, 0.0, 6.0, 6.0);
    let two = rect_path(0.0, 3.0, 9.0, 9.0);
    test_path_op(reporter, &one, &two, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L428-L433 (chrome/m156)
fn test_diff2(reporter: &mut Reporter, filename: &str) {
    let one = rect_path(0.0, 0.0, 6.0, 6.0);
    let two = rect_path(0.0, 3.0, 9.0, 9.0);
    test_path_op(reporter, &one, &two, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L434-L439 (chrome/m156)
fn test_xor2(reporter: &mut Reporter, filename: &str) {
    let one = rect_path(0.0, 0.0, 6.0, 6.0);
    let two = rect_path(0.0, 3.0, 9.0, 9.0);
    test_path_op(reporter, &one, &two, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L440-L449 (chrome/m156)
fn test_op1d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path_b.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L451-L461 (chrome/m156)
fn test_op2d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path_b.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

/// A `TestDesc` function: `void (*)(skiatest::Reporter*, const char* filename)`.
type TestFn = fn(&mut Reporter, &str);

/// The ported subset of `static struct TestDesc tests[]` of `PathOpsOpTest.cpp`, in order.
// Port of: tests/PathOpsOpTest.cpp#L9136-L9500 (chrome/m156), restricted to the ported entries
const PORTED_TESTS: &[(&str, TestFn)] = &[
    ("testIntersect1", test_intersect1),
    ("testUnion1", test_union1),
    ("testDiff1", test_diff1),
    ("testXor1", test_xor1),
    ("testIntersect2", test_intersect2),
    ("testUnion2", test_union2),
    ("testDiff2", test_diff2),
    ("testXor2", test_xor2),
    ("testOp1d", test_op1d),
    ("testOp2d", test_op2d),
];

// Port of: tests/PathOpsOpTest.cpp#L9513-L9523 (chrome/m156), partial: runs PORTED_TESTS only.
def_test!(PathOpsOpPartial, |reporter| {
    for &(name, test) in PORTED_TESTS {
        test(reporter, name);
    }
});
