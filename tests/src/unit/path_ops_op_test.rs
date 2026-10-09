// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsOpTest.cpp (chrome/m156). Every static test function of the file is
// ported 1:1 (C++ camelCase names become snake_case), with the `tests[]`, `failTests[]` and
// `repTests[]` tables, `bug_513820666`, and the `PathOpsOp`, `PathOpsFailOp` and `PathOpsRepOp`
// DEF_TESTs. The Skia file's `ops` array is `OPS` below.

#![cfg(test)]
// The float literals are the exact decimal values of Skia's PathOps test data and keep Skia's
// spelling: rounding or re-spelling them would change the inputs. The functions are the Skia test
// bodies, so they are long and their names mirror the source (`c1pair` and `c1apair`).
#![allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant,
    clippy::too_many_lines,
    clippy::similar_names
)]

use skia_rust_core::geometry::chop_cubic_at;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::utils::parse_path;
use skia_rust_pathops::cubic::DCubic;
use skia_rust_pathops::path_op::PathOp;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::{op, simplify};

use crate::unit::path_ops_extended_test::{
    test_path_op, test_path_op_check, test_path_op_fail, test_path_op_fuzz,
};
use crate::unit::path_ops_test_common::cubic_path_to_quads;
use crate::{Reporter, def_test, reporter_assert};

/// A `TestDesc` function: `void (*)(skiatest::Reporter*, const char* filename)`.
type TestFn = fn(&mut Reporter, &str);

/// `static constexpr auto ops` of `PathOpsOpTest.cpp`.
// Port of: tests/PathOpsOpTest.cpp#L1167-L1173 (chrome/m156)
const OPS: [PathOp; 5] = [
    PathOp::Union,
    PathOp::Xor,
    PathOp::ReverseDifference,
    PathOp::Xor,
    PathOp::ReverseDifference,
];

/// `ISSUE3517_STR` and `ISSUE3517_STR_B`: the two SVG strings of `issue3517`.
// Port of: tests/PathOpsOpTest.cpp#L3665 and #L3668 (chrome/m156)
const ISSUE3517_STR: &str = "M31.35 57.75L31.35 57.75C31.9 57.7486 32.45 57.7948 33 57.7413C33.55 57.6878 34.1 57.5014 34.65 57.4291C35.2 57.3569 35.75 57.3223 36.3 57.3079C36.85 57.2935 37.4 57.3143 37.95 57.3428C38.5 57.3712 39.05 57.4112 39.6 57.4786C40.15 57.546 40.7 57.7029 41.25 57.7472C41.8 57.7916 42.35 57.7962 42.9 57.7445C43.45 57.6928 44 57.5345 44.55 57.4373C45.1 57.34 45.65 57.2115 46.2 57.1611C46.75 57.1107 47.3 57.1371 47.85 57.1349C48.4 57.1327 48.95 57.144 49.5 57.1478C50.05 57.1516 50.6 57.1553 51.15 57.1579C51.7 57.1605 52.25 57.1601 52.8 57.1634C53.35 57.1667 53.9 57.1731 54.45 57.1776C55 57.182 55.55 57.1916 56.1 57.19C56.65 57.1884 57.2 57.178 57.75 57.168C58.3 57.158 58.85 57.1355 59.4 57.1299C59.95 57.1243 60.5 57.1338 61.05 57.1345C61.6 57.1352 62.15 57.124 62.7 57.134C63.25 57.1441 63.8 57.1731 64.35 57.195C64.9 57.2169 65.45 57.2532 66 57.2655C66.55 57.2778 67.1 57.2647 67.65 57.2687C68.2 57.2728 68.75 57.267 69.3 57.2896C69.85 57.3122 70.4 57.371 70.95 57.4044C71.5 57.4377 72.05 57.4668 72.6 57.4896C73.15 57.5123 73.7 57.545 74.25 57.5408C74.8 57.5365 75.35 57.5068 75.9 57.4641C76.45 57.4213 77 57.3244 77.55 57.2842C78.1 57.244 78.65 57.2163 79.2 57.2228C79.75 57.2293 80.3 57.29 80.85 57.3232C81.4 57.3563 81.95 57.396 82.5 57.4219C83.05 57.4478 83.6 57.4637 84.15 57.4787C84.7 57.4937 85.25 57.5011 85.8 57.5121C86.35 57.523 86.9 57.5411 87.45 57.5444C88 57.5477 88.55 57.5663 89.1 57.5318C89.65 57.4972 90.2 57.3126 90.75 57.337C91.3 57.3613 91.85 57.6088 92.4 57.6776C92.95 57.7465 93.5 57.7379 94.05 57.75C94.6 57.7621 95.15 57.75 95.7 57.75L95.7 57.75L31.35 57.75Z";
const ISSUE3517_STR_B: &str = "M31.35 57.75L31.35 57.75C31.9 57.7514 32.45 57.7052 33 57.7587C33.55 57.8122 34.1 57.9986 34.65 58.0709C35.2 58.1431 35.75 58.1777 36.3 58.1921C36.85 58.2065 37.4 58.1857 37.95 58.1572C38.5 58.1288 39.05 58.0888 39.6 58.0214C40.15 57.954 40.7 57.7971 41.25 57.7528C41.8 57.7084 42.35 57.7038 42.9 57.7555C43.45 57.8072 44 57.9655 44.55 58.0627C45.1 58.16 45.65 58.2885 46.2 58.3389C46.75 58.3893 47.3 58.3629 47.85 58.3651C48.4 58.3673 48.95 58.356 49.5 58.3522C50.05 58.3484 50.6 58.3447 51.15 58.3421C51.7 58.3395 52.25 58.3399 52.8 58.3366C53.35 58.3333 53.9 58.3269 54.45 58.3224C55 58.318 55.55 58.3084 56.1 58.31C56.65 58.3116 57.2 58.322 57.75 58.332C58.3 58.342 58.85 58.3645 59.4 58.3701C59.95 58.3757 60.5 58.3662 61.05 58.3655C61.6 58.3648 62.15 58.376 62.7 58.366C63.25 58.3559 63.8 58.3269 64.35 58.305C64.9 58.2831 65.45 58.2468 66 58.2345C66.55 58.2222 67.1 58.2353 67.65 58.2313C68.2 58.2272 68.75 58.233 69.3 58.2104C69.85 58.1878 70.4 58.129 70.95 58.0956C71.5 58.0623 72.05 58.0332 72.6 58.0104C73.15 57.9877 73.7 57.955 74.25 57.9592C74.8 57.9635 75.35 57.9932 75.9 58.0359C76.45 58.0787 77 58.1756 77.55 58.2158C78.1 58.256 78.65 58.2837 79.2 58.2772C79.75 58.2707 80.3 58.21 80.85 58.1768C81.4 58.1437 81.95 58.104 82.5 58.0781C83.05 58.0522 83.6 58.0363 84.15 58.0213C84.7 58.0063 85.25 57.9989 85.8 57.9879C86.35 57.977 86.9 57.9589 87.45 57.9556C88 57.9523 88.55 57.9337 89.1 57.9682C89.65 58.0028 90.2 58.1874 90.75 58.163C91.3 58.1387 91.85 57.8912 92.4 57.8224C92.95 57.7535 93.5 57.7621 94.05 57.75C94.6 57.7379 95.15 57.75 95.7 57.75L95.7 57.75L31.35 57.75Z";

// Port of: tests/PathOpsOpTest.cpp#L33-L44 (chrome/m156)
fn path_edit(from: Point, to: Point, path: &mut Path) {
    let found = path
        .points()
        .iter()
        .position(|pt| DPoint::approximately_equal_points(*pt, from));
    if let Some(index) = found {
        // we want setPt()
        let mut builder = PathBuilder::new_path(path);
        builder.set_point(index, to);
        *path = builder.detach();
    }
}

// Port of: tests/PathOpsOpTest.cpp#L3852-L3867 (chrome/m156)
fn complex_to_quads(pts: [Point; 4], path: &mut PathBuilder) {
    let mut loop_t = [0.0_f32; 3];
    if DCubic::complex_break(pts, &mut loop_t) != 0 {
        let mut cubic_pair = [Point::default(); 7];
        chop_cubic_at(&pts, &mut cubic_pair, loop_t[0]);
        let mut c1 = DCubic::default();
        c1.set([cubic_pair[0], cubic_pair[1], cubic_pair[2], cubic_pair[3]]);
        let mut c2 = DCubic::default();
        c2.set([cubic_pair[3], cubic_pair[4], cubic_pair[5], cubic_pair[6]]);
        let q1 = c1.to_quad();
        let q2 = c2.to_quad();
        path.quad_to(q1.pts[1].as_sk_point(), q1.pts[2].as_sk_point());
        path.quad_to(q2.pts[1].as_sk_point(), q2.pts[2].as_sk_point());
    } else {
        path.cubic_to(pts[1], pts[2], pts[3]);
    }
}

// Port of: tests/PathOpsOpTest.cpp#L46-L57 (chrome/m156)
fn cubic_op1d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 2.0), (1.0, 0.0), (1.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((0.0, 1.0), (1.0, 0.0), (2.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L59-L70 (chrome/m156)
fn cubic_op2d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((0.0, 1.0), (1.0, 0.0), (1.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((0.0, 1.0), (2.0, 0.0), (1.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L72-L83 (chrome/m156)
fn cubic_op3d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 3.0), (1.0, 0.0), (1.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((0.0, 1.0), (1.0, 0.0), (3.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L85-L96 (chrome/m156)
fn cubic_op5d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 2.0), (1.0, 0.0), (2.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((0.0, 2.0), (1.0, 0.0), (2.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L98-L109 (chrome/m156)
fn cubic_op6d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 6.0), (1.0, 0.0), (3.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((0.0, 3.0), (1.0, 0.0), (6.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L111-L122 (chrome/m156)
fn cubic_op7d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 4.0), (1.0, 0.0), (3.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((0.0, 3.0), (1.0, 0.0), (4.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L124-L135 (chrome/m156)
fn cubic_op8d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 5.0), (1.0, 0.0), (4.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((0.0, 4.0), (1.0, 0.0), (5.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L137-L148 (chrome/m156)
fn cubic_op9d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 6.0), (1.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((1.0, 2.0), (1.0, 0.0), (6.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L150-L163 (chrome/m156)
fn quad_op9d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.quad_to((1.0, 6.0), (1.5, 1.0));
    path.quad_to((1.5, 0.5), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.quad_to((1.0, 2.0), (1.4, 1.0));
    path_b.quad_to((3.0, 0.4), (6.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L165-L182 (chrome/m156)
fn line_op9d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.line_to((1.0, 6.0));
    path.line_to((1.5, 1.0));
    path.line_to((1.8, 0.8));
    path.line_to((2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.line_to((1.0, 2.0));
    path_b.line_to((1.4, 1.0));
    path_b.line_to((3.0, 0.4));
    path_b.line_to((6.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L184-L195 (chrome/m156)
fn cubic_op1i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 2.0), (1.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((1.0, 2.0), (1.0, 0.0), (2.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L197-L208 (chrome/m156)
fn cubic_op10d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 3.0), (1.0, 0.0), (4.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((1.0, 4.0), (1.0, 0.0), (3.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L210-L221 (chrome/m156)
fn cubic_op11d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 4.0), (1.0, 0.0), (5.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((1.0, 5.0), (1.0, 0.0), (4.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L223-L234 (chrome/m156)
fn cubic_op12d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 6.0), (1.0, 0.0), (1.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((0.0, 1.0), (1.0, 0.0), (6.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L236-L247 (chrome/m156)
fn cubic_op13d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((4.0, 5.0), (1.0, 0.0), (5.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((3.0, 5.0), (1.0, 0.0), (5.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L249-L260 (chrome/m156)
fn cubic_op14d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 2.0), (2.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((1.0, 2.0), (1.0, 0.0), (2.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L262-L273 (chrome/m156)
fn cubic_op15d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 6.0), (2.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((1.0, 2.0), (1.0, 0.0), (6.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L275-L286 (chrome/m156)
fn cubic_op16d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((0.0, 1.0), (3.0, 0.0), (1.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 3.0));
    path_b.cubic_to((0.0, 1.0), (2.0, 0.0), (1.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L288-L299 (chrome/m156)
fn cubic_op17d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((0.0, 2.0), (4.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((1.0, 2.0), (2.0, 0.0), (2.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L301-L312 (chrome/m156)
fn cubic_op18d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 5.0), (2.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((1.0, 2.0), (1.0, 0.0), (5.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L314-L325 (chrome/m156)
fn cubic_op19i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((0.0, 1.0), (2.0, 1.0), (6.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((2.0, 6.0), (2.0, 0.0), (1.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L327-L338 (chrome/m156)
fn cubic_op20d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 1.0), (6.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((1.0, 2.0), (1.0, 0.0), (1.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L340-L351 (chrome/m156)
fn cubic_op21d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 1.0), (2.0, 1.0), (6.0, 5.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((5.0, 6.0), (1.0, 0.0), (1.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L353-L364 (chrome/m156)
fn cubic_op22d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 3.0), (3.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 3.0));
    path_b.cubic_to((1.0, 2.0), (1.0, 0.0), (3.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L366-L377 (chrome/m156)
fn cubic_op23d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 2.0), (4.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((1.0, 2.0), (1.0, 0.0), (2.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L379-L390 (chrome/m156)
fn cubic_op24d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 2.0), (2.0, 0.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((2.0, 3.0), (1.0, 0.0), (2.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L392-L396 (chrome/m156)
fn test_intersect1(reporter: &mut Reporter, filename: &str) {
    let one = Path::rect(Rect::new(0.0, 0.0, 6.0, 6.0), PathDirection::CW);
    let two = Path::rect(Rect::new(3.0, 3.0, 9.0, 9.0), PathDirection::CW);
    test_path_op(reporter, &one, &two, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L398-L402 (chrome/m156)
fn test_union1(reporter: &mut Reporter, filename: &str) {
    let one = Path::rect(Rect::new(0.0, 0.0, 6.0, 6.0), PathDirection::CW);
    let two = Path::rect(Rect::new(3.0, 3.0, 9.0, 9.0), PathDirection::CW);
    test_path_op(reporter, &one, &two, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L404-L408 (chrome/m156)
fn test_diff1(reporter: &mut Reporter, filename: &str) {
    let one = Path::rect(Rect::new(0.0, 0.0, 6.0, 6.0), PathDirection::CW);
    let two = Path::rect(Rect::new(3.0, 3.0, 9.0, 9.0), PathDirection::CW);
    test_path_op(reporter, &one, &two, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L410-L414 (chrome/m156)
fn test_xor1(reporter: &mut Reporter, filename: &str) {
    let one = Path::rect(Rect::new(0.0, 0.0, 6.0, 6.0), PathDirection::CW);
    let two = Path::rect(Rect::new(3.0, 3.0, 9.0, 9.0), PathDirection::CW);
    test_path_op(reporter, &one, &two, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L416-L420 (chrome/m156)
fn test_intersect2(reporter: &mut Reporter, filename: &str) {
    let one = Path::rect(Rect::new(0.0, 0.0, 6.0, 6.0), PathDirection::CW);
    let two = Path::rect(Rect::new(0.0, 3.0, 9.0, 9.0), PathDirection::CW);
    test_path_op(reporter, &one, &two, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L422-L426 (chrome/m156)
fn test_union2(reporter: &mut Reporter, filename: &str) {
    let one = Path::rect(Rect::new(0.0, 0.0, 6.0, 6.0), PathDirection::CW);
    let two = Path::rect(Rect::new(0.0, 3.0, 9.0, 9.0), PathDirection::CW);
    test_path_op(reporter, &one, &two, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L428-L432 (chrome/m156)
fn test_diff2(reporter: &mut Reporter, filename: &str) {
    let one = Path::rect(Rect::new(0.0, 0.0, 6.0, 6.0), PathDirection::CW);
    let two = Path::rect(Rect::new(0.0, 3.0, 9.0, 9.0), PathDirection::CW);
    test_path_op(reporter, &one, &two, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L434-L438 (chrome/m156)
fn test_xor2(reporter: &mut Reporter, filename: &str) {
    let one = Path::rect(Rect::new(0.0, 0.0, 6.0, 6.0), PathDirection::CW);
    let two = Path::rect(Rect::new(0.0, 3.0, 9.0, 9.0), PathDirection::CW);
    test_path_op(reporter, &one, &two, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L440-L449 (chrome/m156)
fn test_op1d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
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

// Port of: tests/PathOpsOpTest.cpp#L451-L460 (chrome/m156)
fn test_op2d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
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

// Port of: tests/PathOpsOpTest.cpp#L462-L471 (chrome/m156)
fn test_op3d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(1.0, 1.0, 2.0, 2.0), PathDirection::CW, None);
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

// Port of: tests/PathOpsOpTest.cpp#L473-L482 (chrome/m156)
fn test_op1u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 3.0, 3.0), PathDirection::CW, None);
    path_b.set_fill_type(PathFillType::Winding);
    path_b.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path_b.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L484-L493 (chrome/m156)
fn test_op4d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 4.0, 4.0), PathDirection::CW, None);
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

// Port of: tests/PathOpsOpTest.cpp#L495-L504 (chrome/m156)
fn test_op5d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 3.0, 3.0), PathDirection::CW, None);
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

// Port of: tests/PathOpsOpTest.cpp#L506-L515 (chrome/m156)
fn test_op6d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 3.0, 3.0), PathDirection::CW, None);
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

// Port of: tests/PathOpsOpTest.cpp#L517-L526 (chrome/m156)
fn test_op7d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
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

// Port of: tests/PathOpsOpTest.cpp#L528-L537 (chrome/m156)
fn test_op2u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    path_b.set_fill_type(PathFillType::Winding);
    path_b.add_rect(Rect::new(0.0, 0.0, 3.0, 3.0), PathDirection::CW, None);
    path_b.add_rect(Rect::new(1.0, 1.0, 2.0, 2.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L539-L546 (chrome/m156)
fn test_op8d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 640.0, 480.0), PathDirection::CW, None);
    path_b.move_to((577330.0, 1971.72));
    path_b.cubic_to((10.7082, -116.596), (262.057, 45.6468), (294.694, 1.96237));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L547-L558 (chrome/m156)
fn cubic_op25i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 4.0), (5.0, 0.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((2.0, 3.0), (1.0, 0.0), (4.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L560-L571 (chrome/m156)
fn cubic_op26d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 4.0), (4.0, 0.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((2.0, 3.0), (1.0, 0.0), (4.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L573-L584 (chrome/m156)
fn cubic_op27d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 6.0), (1.0, 0.0), (5.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((2.0, 5.0), (1.0, 0.0), (6.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L586-L597 (chrome/m156)
fn cubic_op28u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 4.0), (6.0, 0.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((2.0, 3.0), (1.0, 0.0), (4.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L599-L610 (chrome/m156)
fn cubic_op29d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 5.0), (6.0, 0.0), (4.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((2.0, 4.0), (1.0, 0.0), (5.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L612-L623 (chrome/m156)
fn cubic_op30d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 5.0), (6.0, 0.0), (5.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((3.0, 5.0), (1.0, 0.0), (5.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L625-L636 (chrome/m156)
fn cubic_op31d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((0.0, 3.0), (2.0, 1.0), (4.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((0.0, 4.0), (2.0, 0.0), (3.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L638-L649 (chrome/m156)
fn cubic_op31u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((0.0, 3.0), (2.0, 1.0), (4.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((0.0, 4.0), (2.0, 0.0), (3.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L651-L662 (chrome/m156)
fn cubic_op31x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((0.0, 3.0), (2.0, 1.0), (4.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((0.0, 4.0), (2.0, 0.0), (3.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Xor,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L664-L675 (chrome/m156)
fn cubic_op32d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 2.0), (6.0, 0.0), (3.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((1.0, 3.0), (1.0, 0.0), (2.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L677-L688 (chrome/m156)
fn cubic_op33i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 2.0), (6.0, 0.0), (3.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((1.0, 3.0), (1.0, 0.0), (2.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L690-L701 (chrome/m156)
fn cubic_op34d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 5.0), (2.0, 1.0), (3.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((1.0, 3.0), (1.0, 0.0), (5.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L703-L714 (chrome/m156)
fn cubic_op35d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 5.0), (2.0, 1.0), (4.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((0.0, 4.0), (1.0, 0.0), (5.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L716-L727 (chrome/m156)
fn cubic_op36u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 6.0), (2.0, 0.0), (5.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((1.0, 5.0), (1.0, 0.0), (6.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L729-L740 (chrome/m156)
fn cubic_op37d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 6.0), (6.0, 1.0), (4.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 6.0));
    path_b.cubic_to((3.0, 4.0), (1.0, 0.0), (6.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L742-L753 (chrome/m156)
fn cubic_op38d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 6.0), (3.0, 2.0), (4.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 3.0));
    path_b.cubic_to((1.0, 4.0), (1.0, 0.0), (6.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L755-L766 (chrome/m156)
fn cubic_op39d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 3.0), (5.0, 1.0), (4.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to((3.0, 4.0), (1.0, 0.0), (3.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L768-L779 (chrome/m156)
fn cubic_op40d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 5.0), (3.0, 2.0), (4.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 3.0));
    path_b.cubic_to((2.0, 4.0), (1.0, 0.0), (5.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L781-L792 (chrome/m156)
fn cubic_op41i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 6.0), (4.0, 3.0), (6.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((3.0, 4.0));
    path_b.cubic_to((4.0, 6.0), (1.0, 0.0), (6.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L794-L805 (chrome/m156)
fn cubic_op42d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 2.0), (6.0, 5.0), (5.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((5.0, 6.0));
    path_b.cubic_to((4.0, 5.0), (1.0, 0.0), (2.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L807-L818 (chrome/m156)
fn cubic_op43d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((1.0, 2.0), (4.0, 0.0), (3.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((1.0, 3.0), (2.0, 0.0), (2.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L820-L831 (chrome/m156)
fn cubic_op44d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((3.0, 6.0), (4.0, 0.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((2.0, 3.0), (2.0, 0.0), (6.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L833-L844 (chrome/m156)
fn cubic_op45d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((2.0, 4.0), (4.0, 0.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((2.0, 3.0), (2.0, 0.0), (4.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L846-L857 (chrome/m156)
fn cubic_op46d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((3.0, 5.0), (5.0, 0.0), (4.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((2.0, 4.0), (2.0, 0.0), (5.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L859-L870 (chrome/m156)
fn cubic_op47d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 6.0), (6.0, 2.0), (5.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 6.0));
    path_b.cubic_to((4.0, 5.0), (1.0, 0.0), (6.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L872-L883 (chrome/m156)
fn cubic_op48d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((2.0, 3.0), (5.0, 1.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to((2.0, 3.0), (2.0, 0.0), (3.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L885-L896 (chrome/m156)
fn cubic_op49d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((1.0, 5.0), (3.0, 2.0), (4.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 3.0));
    path_b.cubic_to((1.0, 4.0), (2.0, 0.0), (5.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L898-L909 (chrome/m156)
fn cubic_op50d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 3.0));
    path.cubic_to((1.0, 6.0), (5.0, 0.0), (5.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((1.0, 5.0), (3.0, 0.0), (6.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L911-L922 (chrome/m156)
fn cubic_op51d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 3.0));
    path.cubic_to((1.0, 2.0), (4.0, 1.0), (6.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((0.0, 6.0), (3.0, 0.0), (2.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L924-L935 (chrome/m156)
fn cubic_op52d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((1.0, 2.0), (5.0, 4.0), (4.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((4.0, 5.0));
    path_b.cubic_to((3.0, 4.0), (2.0, 0.0), (2.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L937-L948 (chrome/m156)
fn cubic_op53d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 3.0));
    path.cubic_to((1.0, 2.0), (5.0, 3.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((3.0, 5.0));
    path_b.cubic_to((1.0, 2.0), (3.0, 0.0), (2.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L950-L961 (chrome/m156)
fn cubic_op54d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 4.0));
    path.cubic_to((1.0, 3.0), (5.0, 4.0), (4.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((4.0, 5.0));
    path_b.cubic_to((2.0, 4.0), (4.0, 0.0), (3.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L963-L974 (chrome/m156)
fn cubic_op55d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 5.0));
    path.cubic_to((1.0, 3.0), (3.0, 2.0), (5.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 3.0));
    path_b.cubic_to((0.0, 5.0), (5.0, 0.0), (3.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L976-L987 (chrome/m156)
fn cubic_op56d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 6.0), (5.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((1.0, 2.0), (1.0, 0.0), (6.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L989-L1000 (chrome/m156)
fn cubic_op57d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 5.0));
    path.cubic_to((0.0, 5.0), (5.0, 4.0), (6.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((4.0, 5.0));
    path_b.cubic_to((4.0, 6.0), (5.0, 0.0), (5.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1002-L1013 (chrome/m156)
fn cubic_op58d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 5.0));
    path.cubic_to((3.0, 4.0), (6.0, 5.0), (5.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((5.0, 6.0));
    path_b.cubic_to((3.0, 5.0), (5.0, 0.0), (4.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1015-L1026 (chrome/m156)
fn cubic_op59d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((5.0, 6.0), (4.0, 0.0), (4.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((1.0, 4.0), (1.0, 0.0), (6.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1028-L1039 (chrome/m156)
fn cubic_op60d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((4.0, 6.0), (6.0, 0.0), (5.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((2.0, 5.0), (2.0, 0.0), (6.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1041-L1052 (chrome/m156)
fn cubic_op61d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 2.0));
    path.cubic_to((0.0, 5.0), (3.0, 2.0), (6.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 3.0));
    path_b.cubic_to((1.0, 6.0), (2.0, 1.0), (5.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1054-L1065 (chrome/m156)
fn cubic_op62d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 3.0));
    path.cubic_to((5.0, 6.0), (5.0, 3.0), (5.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((3.0, 5.0));
    path_b.cubic_to((4.0, 5.0), (3.0, 1.0), (6.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1067-L1078 (chrome/m156)
fn cubic_op63d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 3.0));
    path.cubic_to((0.0, 4.0), (3.0, 2.0), (5.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 3.0));
    path_b.cubic_to((3.0, 5.0), (3.0, 2.0), (4.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1080-L1091 (chrome/m156)
fn cubic_op64d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 1.0), (1.0, 0.0), (3.0, 0.0));
    path.line_to((0.0, 1.0));
    path.close();
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((0.0, 3.0), (1.0, 0.0), (1.0, 0.0));
    path_b.line_to((0.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1093-L1104 (chrome/m156)
fn cubic_op65d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 5.0), (1.0, 0.0), (1.0, 0.0));
    path.line_to((0.0, 1.0));
    path.close();
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((0.0, 1.0), (1.0, 0.0), (5.0, 1.0));
    path_b.line_to((0.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1106-L1117 (chrome/m156)
fn rect_op1d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 1.0), (1.0, 0.0), (3.0, 0.0));
    path.line_to((0.0, 1.0));
    path.close();
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((0.0, 3.0), (1.0, 0.0), (1.0, 0.0));
    path_b.line_to((0.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1119-L1130 (chrome/m156)
fn cubic_op66u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 6.0), (4.0, 2.0), (5.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 4.0));
    path_b.cubic_to((3.0, 5.0), (1.0, 0.0), (6.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1132-L1143 (chrome/m156)
fn cubic_op67u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((3.0, 5.0));
    path.cubic_to((1.0, 6.0), (5.0, 0.0), (3.0, 1.0));
    path.line_to((3.0, 5.0));
    path.close();
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((1.0, 3.0), (5.0, 3.0), (6.0, 1.0));
    path_b.line_to((0.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1145-L1154 (chrome/m156)
fn cubic_op68u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 5.0));
    path.cubic_to((4.0, 5.0), (4.0, 1.0), (5.0, 0.0));
    path.close();
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((0.0, 5.0), (5.0, 0.0), (5.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1156-L1165 (chrome/m156)
fn cubic_op69d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((1.0, 3.0));
    path.cubic_to((0.0, 1.0), (3.0, 1.0), (2.0, 0.0));
    path.close();
    path_b.move_to((1.0, 3.0));
    path_b.cubic_to((0.0, 2.0), (3.0, 1.0), (1.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1175-L1213 (chrome/m156)
#[allow(clippy::too_many_lines)]
fn r_rect1(reporter: &mut Reporter, filename: &str) {
    let x_a: f32 = 0.65;
    let x_b: f32 = 10.65;
    let x_c: f32 = 20.65;
    let x_d: f32 = 30.65;
    let x_e: f32 = 40.65;
    let x_f: f32 = 50.65;

    let y_a: f32 = 0.65;
    let y_b: f32 = 10.65;
    let y_c: f32 = 20.65;
    let y_d: f32 = 30.65;
    let y_e: f32 = 40.65;
    let y_f: f32 = 50.65;
    let rects = [
        Rect::new(x_b, y_b, x_e, y_e),
        Rect::new(x_a, y_a, x_d, y_d),
        Rect::new(x_c, y_a, x_f, y_d),
        Rect::new(x_a, y_c, x_d, y_f),
        Rect::new(x_c, y_c, x_f, y_f),
    ];
    let paths = rects.map(|rect| Path::rrect_xy(rect, 5.0, 5.0, None));
    let mut path = Path::new_with_fill_type(PathFillType::InverseEvenOdd);
    for index in 0..5 {
        let unique_name = format!("{filename}{index}");
        test_path_op(reporter, &path, &paths[index], OPS[index], &unique_name);
        if let Some(result) = op(&path, &paths[index], OPS[index]) {
            path = result;
        } else {
            reporter_assert!(reporter, false, "Op failed for {}", unique_name);
        }
    }
}

// Port of: tests/PathOpsOpTest.cpp#L1215-L1247 (chrome/m156)
fn skp1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((189.0, 7.0));
    path.cubic_to((189.0, 5.34314585), (190.34314, 4.0), (192.0, 4.0));
    path.line_to((243.0, 4.0));
    path.cubic_to((244.65686, 4.0), (246.0, 5.34314585), (246.0, 7.0));
    path.line_to((246.0, 21.0));
    path.cubic_to((246.0, 22.6568546), (244.65686, 24.0), (243.0, 24.0));
    path.line_to((192.0, 24.0));
    path.cubic_to((190.34314, 24.0), (189.0, 22.6568546), (189.0, 21.0));
    path.line_to((189.0, 7.0));
    path.close();
    path.move_to((191.0, 8.0));
    path.cubic_to((191.0, 6.89543009), (191.895432, 6.0), (193.0, 6.0));
    path.line_to((242.0, 6.0));
    path.cubic_to((243.104568, 6.0), (244.0, 6.89543009), (244.0, 8.0));
    path.line_to((244.0, 20.0));
    path.cubic_to((244.0, 21.1045704), (243.104568, 22.0), (242.0, 22.0));
    path.line_to((193.0, 22.0));
    path.cubic_to((191.895432, 22.0), (191.0, 21.1045704), (191.0, 20.0));
    path.line_to((191.0, 8.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((189.0, 4.0));
    path_b.line_to((199.0, 14.0));
    path_b.line_to((236.0, 14.0));
    path_b.line_to((246.0, 4.0));
    path_b.line_to((189.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1249-L1267 (chrome/m156)
fn skp2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((253.000000, 11757.0000));
    path.line_to((253.000000, 222.000000));
    path.line_to((823.000000, 222.000000));
    path.line_to((823.000000, 11757.0000));
    path.line_to((253.000000, 11757.0000));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((258.000000, 1028.00000));
    path_b.line_to((258.000000, 1027.00000));
    path_b.line_to((823.000000, 1027.00000));
    path_b.line_to((823.000000, 1028.00000));
    path_b.line_to((258.000000, 1028.00000));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1269-L1299 (chrome/m156)
fn skp3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((717.000000, 507.000000));
    path.line_to((717.000000, 425.000000));
    path.line_to((973.000000, 425.000000));
    path.line_to((973.000000, 507.000000));
    path.quad_to((973.000000, 508.242645), (972.121582, 509.121613));
    path.quad_to((971.242615, 510.000000), (970.000000, 510.000000));
    path.line_to((720.000000, 510.000000));
    path.quad_to((718.757385, 510.000000), (717.878418, 509.121613));
    path.quad_to((717.000000, 508.242645), (717.000000, 507.000000));
    path.close();
    path.move_to((719.000000, 426.000000));
    path.line_to((971.000000, 426.000000));
    path.line_to((971.000000, 506.000000));
    path.cubic_to(
        (971.000000, 507.104584),
        (970.104553, 508.000000),
        (969.000000, 508.000000),
    );
    path.line_to((721.000000, 508.000000));
    path.cubic_to(
        (719.895447, 508.000000),
        (719.000000, 507.104584),
        (719.000000, 506.000000),
    );
    path.line_to((719.000000, 426.000000));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((717.000000, 510.000000));
    path_b.line_to((760.000000, 467.000000));
    path_b.line_to((930.000000, 467.000000));
    path_b.line_to((973.000000, 510.000000));
    path_b.line_to((717.000000, 510.000000));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1301-L1331 (chrome/m156)
fn skp4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((230.756805, 591.756775));
    path.quad_to((232.514725, 590.000000), (235.000000, 590.000000));
    path.line_to((300.000000, 590.000000));
    path.quad_to((302.485291, 590.000000), (304.243195, 591.756775));
    path.quad_to((306.000000, 593.514709), (306.000000, 596.000000));
    path.line_to((306.000000, 617.000000));
    path.line_to((229.000000, 617.000000));
    path.line_to((229.000000, 596.000000));
    path.quad_to((229.000000, 593.514709), (230.756805, 591.756775));
    path.close();
    path.move_to((231.000000, 597.000000));
    path.cubic_to(
        (231.000000, 594.238586),
        (233.238571, 592.000000),
        (236.000000, 592.000000),
    );
    path.line_to((299.000000, 592.000000));
    path.cubic_to(
        (301.761414, 592.000000),
        (304.000000, 594.238586),
        (304.000000, 597.000000),
    );
    path.line_to((304.000000, 616.000000));
    path.line_to((231.000000, 616.000000));
    path.line_to((231.000000, 597.000000));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((306.000000, 590.000000));
    path_b.line_to((292.000000, 604.000000));
    path_b.line_to((305.000000, 617.000000));
    path_b.line_to((306.000000, 617.000000));
    path_b.line_to((306.000000, 590.000000));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1333-L1357 (chrome/m156)
fn skp5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((18.0000000, 226.000000));
    path.quad_to((14.6862917, 226.000000), (12.3423996, 228.342407));
    path.quad_to((10.0000000, 230.686295), (10.0000000, 234.000000));
    path.line_to((10.0000000, 253.000000));
    path.line_to((1247.00000, 253.000000));
    path.line_to((1247.00000, 234.000000));
    path.quad_to((1247.00000, 230.686295), (1244.65759, 228.342407));
    path.quad_to((1242.31372, 226.000000), (1239.00000, 226.000000));
    path.line_to((18.0000000, 226.000000));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::InverseWinding);
    path_b.move_to((18.0000000, 226.000000));
    path_b.line_to((1239.00000, 226.000000));
    path_b.cubic_to(
        (1243.41833, 226.000000),
        (1247.00000, 229.581726),
        (1247.00000, 234.000000),
    );
    path_b.line_to((1247.00000, 252.000000));
    path_b.line_to((10.0000000, 252.000000));
    path_b.line_to((10.0000000, 234.000000));
    path_b.cubic_to(
        (10.0000000, 229.581726),
        (13.5817204, 226.000000),
        (18.0000000, 226.000000),
    );
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1359-L1370 (chrome/m156)
fn cubic_op70d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 5.0), (4.0, 0.0), (5.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((0.0, 5.0), (1.0, 0.0), (5.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1372-L1383 (chrome/m156)
fn cubic_op71d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 5.0), (4.0, 1.0), (6.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((4.0, 6.0), (1.0, 0.0), (5.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1385-L1396 (chrome/m156)
fn cubic_op72i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 5.0), (5.0, 2.0), (5.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 5.0));
    path_b.cubic_to((4.0, 5.0), (1.0, 0.0), (5.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1398-L1411 (chrome/m156)
fn cubic_op73d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 4.0), (4.0, 0.0), (6.0, 4.0));
    path.line_to((0.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((4.0, 6.0), (1.0, 0.0), (4.0, 3.0));
    path_b.line_to((0.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1413-L1426 (chrome/m156)
fn cubic_op74d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 5.0), (5.0, 1.0), (5.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to((1.0, 5.0), (1.0, 0.0), (5.0, 1.0));
    path_b.line_to((1.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1428-L1441 (chrome/m156)
fn cubic_op75d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 4.0), (5.0, 1.0), (6.0, 4.0));
    path.line_to((0.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to((4.0, 6.0), (1.0, 0.0), (4.0, 0.0));
    path_b.line_to((1.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1443-L1454 (chrome/m156)
fn cubic_op76u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 2.0), (2.0, 0.0), (5.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((3.0, 5.0), (1.0, 0.0), (2.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1456-L1469 (chrome/m156)
fn cubic_op77i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 3.0), (2.0, 0.0), (3.0, 2.0));
    path.line_to((0.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((2.0, 3.0), (1.0, 0.0), (3.0, 1.0));
    path_b.line_to((0.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1471-L1484 (chrome/m156)
fn cubic_op78u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1.0, 6.0));
    path.cubic_to((1.0, 6.0), (5.0, 0.0), (6.0, 1.0));
    path.line_to((1.0, 6.0));
    path.close();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((1.0, 6.0), (6.0, 1.0), (6.0, 1.0));
    path_b.line_to((0.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1486-L1497 (chrome/m156)
fn cubic_op79u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 3.0), (1.0, 0.0), (6.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((4.0, 6.0), (1.0, 0.0), (3.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1499-L1512 (chrome/m156)
fn cubic_op80i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 3.0), (2.0, 1.0), (4.0, 3.0));
    path.line_to((0.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((3.0, 4.0), (1.0, 0.0), (3.0, 2.0));
    path_b.line_to((1.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1514-L1525 (chrome/m156)
fn cubic_op81d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((4.0, 6.0), (4.0, 3.0), (5.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((3.0, 4.0));
    path_b.cubic_to((4.0, 5.0), (1.0, 0.0), (6.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1527-L1540 (chrome/m156)
fn cubic_op82i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 3.0), (5.0, 2.0), (3.0, 0.0));
    path.line_to((0.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 5.0));
    path_b.cubic_to((0.0, 3.0), (1.0, 0.0), (3.0, 2.0));
    path_b.line_to((2.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1542-L1555 (chrome/m156)
fn cubic_op83i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 3.0), (2.0, 1.0), (4.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((1.0, 4.0), (1.0, 0.0), (3.0, 0.0));
    path_b.line_to((1.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1557-L1568 (chrome/m156)
fn cubic_op84d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 4.0));
    path.cubic_to((2.0, 3.0), (6.0, 3.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((3.0, 6.0));
    path_b.cubic_to((2.0, 3.0), (4.0, 0.0), (3.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1570-L1598 (chrome/m156)
fn skp_clip1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1126.17114, 877.171204));
    path.quad_to((1127.34314, 876.000000), (1129.00000, 876.000000));
    path.line_to((1243.00000, 876.000000));
    path.quad_to((1244.65686, 876.000000), (1245.82886, 877.171204));
    path.quad_to((1247.00000, 878.343140), (1247.00000, 880.000000));
    path.line_to((1247.00000, 907.000000));
    path.line_to((1246.00000, 907.000000));
    path.line_to((1246.00000, 880.000000));
    path.cubic_to(
        (1246.00000, 878.343140),
        (1244.65686, 877.000000),
        (1243.00000, 877.000000),
    );
    path.line_to((1129.00000, 877.000000));
    path.cubic_to(
        (1127.34314, 877.000000),
        (1126.00000, 878.343140),
        (1126.00000, 880.000000),
    );
    path.line_to((1126.00000, 907.000000));
    path.line_to((1125.00000, 907.000000));
    path.line_to((1125.00000, 880.000000));
    path.quad_to((1125.00000, 878.343140), (1126.17114, 877.171204));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1247.00000, 876.000000));
    path_b.line_to((1231.00000, 892.000000));
    path_b.line_to((1246.00000, 907.000000));
    path_b.line_to((1247.00000, 907.000000));
    path_b.line_to((1247.00000, 876.000000));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1600-L1624 (chrome/m156)
fn skp_clip2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((134.000000, 11414.0000));
    path.cubic_to(
        (131.990234, 11414.0000),
        (130.326660, 11415.4824),
        (130.042755, 11417.4131),
    );
    path.cubic_to(
        (130.233124, 11418.3193),
        (131.037079, 11419.0000),
        (132.000000, 11419.0000),
    );
    path.line_to((806.000000, 11419.0000));
    path.cubic_to(
        (806.962891, 11419.0000),
        (807.766907, 11418.3193),
        (807.957275, 11417.4131),
    );
    path.cubic_to(
        (807.673401, 11415.4824),
        (806.009766, 11414.0000),
        (804.000000, 11414.0000),
    );
    path.line_to((134.000000, 11414.0000));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::InverseWinding);
    path_b.move_to((132.000000, 11415.0000));
    path_b.line_to((806.000000, 11415.0000));
    path_b.cubic_to(
        (807.104553, 11415.0000),
        (808.000000, 11415.4473),
        (808.000000, 11416.0000),
    );
    path_b.line_to((808.000000, 11417.0000));
    path_b.cubic_to(
        (808.000000, 11418.1045),
        (807.104553, 11419.0000),
        (806.000000, 11419.0000),
    );
    path_b.line_to((132.000000, 11419.0000));
    path_b.cubic_to(
        (130.895432, 11419.0000),
        (130.000000, 11418.1045),
        (130.000000, 11417.0000),
    );
    path_b.line_to((130.000000, 11416.0000));
    path_b.cubic_to(
        (130.000000, 11415.4473),
        (130.895432, 11415.0000),
        (132.000000, 11415.0000),
    );
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1626-L1654 (chrome/m156)
fn skp96prezzi1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((157.464005, 670.463989));
    path.quad_to((158.928925, 669.000000), (161.000000, 669.000000));
    path.line_to((248.000000, 669.000000));
    path.quad_to((250.071075, 669.000000), (251.535995, 670.463989));
    path.quad_to((253.000000, 671.928955), (253.000000, 674.000000));
    path.line_to((253.000000, 706.000000));
    path.line_to((251.000000, 706.000000));
    path.line_to((251.000000, 675.000000));
    path.cubic_to(
        (251.000000, 672.790833),
        (249.209137, 671.000000),
        (247.000000, 671.000000),
    );
    path.line_to((162.000000, 671.000000));
    path.cubic_to(
        (159.790863, 671.000000),
        (158.000000, 672.790833),
        (158.000000, 675.000000),
    );
    path.line_to((158.000000, 706.000000));
    path.line_to((156.000000, 706.000000));
    path.line_to((156.000000, 674.000000));
    path.quad_to((156.000000, 671.928955), (157.464005, 670.463989));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((156.000000, 669.000000));
    path_b.line_to((178.500000, 691.500000));
    path_b.line_to((230.500000, 691.500000));
    path_b.line_to((253.000000, 669.000000));
    path_b.line_to((156.000000, 669.000000));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1656-L1680 (chrome/m156)
fn skpancestry_com1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((161.000000, 925.000000));
    path.cubic_to(
        (159.874390, 925.000000),
        (158.835663, 925.371948),
        (158.000000, 925.999634),
    );
    path.line_to((158.000000, 926.000000));
    path.line_to((1108.00000, 926.000000));
    path.line_to((1108.00000, 925.999634));
    path.cubic_to(
        (1107.16443, 925.371948),
        (1106.12561, 925.000000),
        (1105.00000, 925.000000),
    );
    path.line_to((161.000000, 925.000000));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((161.000000, 926.000000));
    path_b.line_to((1105.00000, 926.000000));
    path_b.cubic_to(
        (1107.20911, 926.000000),
        (1109.00000, 927.790833),
        (1109.00000, 930.000000),
    );
    path_b.line_to((1109.00000, 956.000000));
    path_b.cubic_to(
        (1109.00000, 958.209167),
        (1107.20911, 960.000000),
        (1105.00000, 960.000000),
    );
    path_b.line_to((161.000000, 960.000000));
    path_b.cubic_to(
        (158.790863, 960.000000),
        (157.000000, 958.209167),
        (157.000000, 956.000000),
    );
    path_b.line_to((157.000000, 930.000000));
    path_b.cubic_to(
        (157.000000, 927.790833),
        (158.790863, 926.000000),
        (161.000000, 926.000000),
    );
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1682-L1704 (chrome/m156)
fn skpeldorado_com_ua1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((286.695129, 291.000000));
    path.line_to((229.304855, 561.000000));
    path.line_to((979.304871, 561.000000));
    path.line_to((1036.69507, 291.000000));
    path.line_to((286.695129, 291.000000));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1006.69513, 291.000000));
    path_b.cubic_to(
        (1023.26367, 291.000000),
        (1033.84021, 304.431458),
        (1030.31836, 321.000000),
    );
    path_b.line_to((985.681519, 531.000000));
    path_b.cubic_to(
        (982.159790, 547.568542),
        (965.873413, 561.000000),
        (949.304871, 561.000000),
    );
    path_b.line_to((259.304871, 561.000000));
    path_b.cubic_to(
        (242.736313, 561.000000),
        (232.159805, 547.568542),
        (235.681549, 531.000000),
    );
    path_b.line_to((280.318420, 321.000000));
    path_b.cubic_to(
        (283.840179, 304.431458),
        (300.126587, 291.000000),
        (316.695129, 291.000000),
    );
    path_b.line_to((1006.69513, 291.000000));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1706-L1732 (chrome/m156)
fn skpbyte_com1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((968.000000, 14.0000000));
    path.cubic_to(
        (965.238586, 14.0000000),
        (963.000000, 16.2385769),
        (963.000000, 19.0000000),
    );
    path.line_to((963.000000, 32.0000000));
    path.cubic_to(
        (963.000000, 34.7614250),
        (965.238586, 37.0000000),
        (968.000000, 37.0000000),
    );
    path.line_to((1034.00000, 37.0000000));
    path.cubic_to(
        (1036.76147, 37.0000000),
        (1039.00000, 34.7614250),
        (1039.00000, 32.0000000),
    );
    path.line_to((1039.00000, 19.0000000));
    path.cubic_to(
        (1039.00000, 16.2385769),
        (1036.76147, 14.0000000),
        (1034.00000, 14.0000000),
    );
    path.line_to((968.000000, 14.0000000));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::InverseWinding);
    path_b.move_to((968.000000, 14.0000000));
    path_b.line_to((1034.00000, 14.0000000));
    path_b.cubic_to(
        (1036.76147, 14.0000000),
        (1039.00000, 16.2385750),
        (1039.00000, 19.0000000),
    );
    path_b.line_to((1039.00000, 32.0000000));
    path_b.cubic_to(
        (1039.00000, 34.2091408),
        (1036.76147, 36.0000000),
        (1034.00000, 36.0000000),
    );
    path_b.line_to((968.000000, 36.0000000));
    path_b.cubic_to(
        (965.238586, 36.0000000),
        (963.000000, 34.2091408),
        (963.000000, 32.0000000),
    );
    path_b.line_to((963.000000, 19.0000000));
    path_b.cubic_to(
        (963.000000, 16.2385750),
        (965.238586, 14.0000000),
        (968.000000, 14.0000000),
    );
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1734-L1753 (chrome/m156)
fn skphealth_com76(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((708.099182, 7.09919119));
    path.line_to((708.099182, 7.09920025));
    path.quad_to((704.000000, 11.2010098), (704.000000, 17.0000000));
    path.line_to((704.000000, 33.0000000));
    path.line_to((705.000000, 33.0000000));
    path.line_to((705.000000, 17.0000000));
    path.cubic_to(
        (705.000000, 13.4101496),
        (706.455078, 10.1601505),
        (708.807617, 7.80761385),
    );
    path.line_to((708.099182, 7.09919119));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((704.000000, 3.00000000));
    path_b.line_to((704.000000, 33.0000000));
    path_b.line_to((705.000000, 33.0000000));
    path_b.line_to((719.500000, 3.00000000));
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1755-L1779 (chrome/m156)
fn skpahrefs_com88(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1099.82886, 7.17117119));
    path.line_to((1099.12134, 7.87867832));
    path.cubic_to(
        (1099.66418, 8.42157173),
        (1100.00000, 9.17157173),
        (1100.00000, 10.0000000),
    );
    path.line_to((1100.00000, 28.0000000));
    path.cubic_to(
        (1100.00000, 29.6568546),
        (1098.65686, 31.0000000),
        (1097.00000, 31.0000000),
    );
    path.line_to((1088.00000, 31.0000000));
    path.line_to((1088.00000, 32.0000000));
    path.line_to((1097.00000, 32.0000000));
    path.quad_to((1098.65686, 32.0000000), (1099.82886, 30.8288002));
    path.quad_to((1101.00000, 29.6568546), (1101.00000, 28.0000000));
    path.line_to((1101.00000, 10.0000000));
    path.quad_to((1101.00000, 8.34314537), (1099.82886, 7.17119980));
    path.line_to((1099.82886, 7.17117119));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1101.00000, 6.00000000));
    path_b.line_to((1088.00000, 6.00000000));
    path_b.line_to((1088.00000, 19.0000000));
    path_b.line_to((1101.00000, 32.0000000));
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1781-L1809 (chrome/m156)
fn skpahrefs_com29(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1037.17114, 7.17119980));
    path.quad_to((1038.34314, 6.00000000), (1040.00000, 6.00000000));
    path.line_to((1074.00000, 6.00000000));
    path.line_to((1074.00000, 32.0000000));
    path.line_to((1040.00000, 32.0000000));
    path.quad_to((1038.34314, 32.0000000), (1037.17114, 30.8288002));
    path.quad_to((1036.00000, 29.6568546), (1036.00000, 28.0000000));
    path.line_to((1036.00000, 10.0000000));
    path.quad_to((1036.00000, 8.34314537), (1037.17114, 7.17119980));
    path.close();
    path.move_to((1037.00000, 10.0000000));
    path.cubic_to(
        (1037.00000, 8.34314537),
        (1038.34314, 7.00000000),
        (1040.00000, 7.00000000),
    );
    path.line_to((1073.00000, 7.00000000));
    path.line_to((1073.00000, 31.0000000));
    path.line_to((1040.00000, 31.0000000));
    path.cubic_to(
        (1038.34314, 31.0000000),
        (1037.00000, 29.6568546),
        (1037.00000, 28.0000000),
    );
    path.line_to((1037.00000, 10.0000000));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1036.00000, 32.0000000));
    path_b.line_to((1049.00000, 19.0000000));
    path_b.line_to((1073.00000, 31.0000000));
    path_b.line_to((1074.00000, 32.0000000));
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1811-L1823 (chrome/m156)
fn cubic_op85d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 6.0), (1.0, 0.0), (6.0, 2.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((2.0, 6.0), (1.0, 0.0), (6.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1825-L1851 (chrome/m156)
fn skpkkiste_to98(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((96.0, 122.0));
    path.cubic_to(
        (94.6192932, 122.0),
        (93.3692932, 122.559647),
        (92.4644699, 123.46447),
    );
    path.line_to((94.1715698, 125.17157));
    path.cubic_to((94.8954315, 124.447708), (95.8954315, 124.0), (97.0, 124.0));
    path.line_to((257.0, 124.0));
    path.cubic_to(
        (258.104553, 124.0),
        (259.104584, 124.447708),
        (259.82843, 125.17157),
    );
    path.line_to((261.535522, 123.46447));
    path.cubic_to(
        (260.630707, 122.559647),
        (259.380707, 122.0),
        (258.0, 122.0),
    );
    path.line_to((96.0, 122.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((258.0, 122.0));
    path_b.cubic_to((260.761414, 122.0), (263.0, 124.238579), (263.0, 127.0));
    path_b.line_to((263.0, 284.0));
    path_b.cubic_to((263.0, 286.761414), (260.761414, 289.0), (258.0, 289.0));
    path_b.line_to((96.0, 289.0));
    path_b.cubic_to((93.2385788, 289.0), (91.0, 286.761414), (91.0, 284.0));
    path_b.line_to((91.0, 127.0));
    path_b.cubic_to((91.0, 124.238579), (93.2385788, 122.0), (96.0, 122.0));
    path_b.line_to((258.0, 122.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1853-L1963 (chrome/m156)
fn issue1417(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    path1.move_to((122.58908843994140625, 82.2836456298828125));
    path1.quad_to((129.8215789794921875, 80.0), (138.0, 80.0));
    path1.quad_to(
        (147.15692138671875, 80.0),
        (155.1280364990234375, 82.86279296875),
    );
    path1.line_to((161.1764678955078125, 100.0));
    path1.line_to((161.1764678955078125, 100.0));
    path1.line_to((115.29412078857421875, 100.0));
    path1.line_to((115.29412078857421875, 100.0));
    path1.line_to((122.58908843994140625, 82.2836456298828125));
    path1.line_to((122.58908843994140625, 82.2836456298828125));
    path1.close();
    path1.move_to((98.68194580078125, 140.343841552734375));
    path1.line_to((115.29412078857421875, 100.0));
    path1.line_to((115.29412078857421875, 100.0));
    path1.line_to((97.9337615966796875, 100.0));
    path1.line_to((97.9337615966796875, 100.0));
    path1.quad_to((88.0, 112.94264984130859375), (88.0, 130.0));
    path1.quad_to(
        (88.0, 131.544830322265625),
        (88.08148956298828125, 133.0560302734375),
    );
    path1.line_to((98.68194580078125, 140.343841552734375));
    path1.line_to((98.68194580078125, 140.343841552734375));
    path1.close();
    path1.move_to((136.969696044921875, 166.6666717529296875));
    path1.line_to((98.68194580078125, 140.343841552734375));
    path1.line_to((98.68194580078125, 140.343841552734375));
    path1.line_to((93.45894622802734375, 153.02825927734375));
    path1.line_to((93.45894622802734375, 153.02825927734375));
    path1.quad_to(
        (96.94116973876953125, 159.65185546875),
        (102.64466094970703125, 165.3553466796875),
    );
    path1.quad_to(
        (110.7924652099609375, 173.503143310546875),
        (120.8179779052734375, 177.1177825927734375),
    );
    path1.line_to((136.969696044921875, 166.6666717529296875));
    path1.line_to((136.969696044921875, 166.6666717529296875));
    path1.close();
    path1.move_to((175.8309783935546875, 141.5211334228515625));
    path1.line_to((136.969696044921875, 166.6666717529296875));
    path1.line_to((136.969696044921875, 166.6666717529296875));
    path1.line_to((153.15728759765625, 177.7956390380859375));
    path1.line_to((153.15728759765625, 177.7956390380859375));
    path1.quad_to(
        (164.392425537109375, 174.318267822265625),
        (173.3553466796875, 165.3553466796875),
    );
    path1.quad_to(
        (177.805816650390625, 160.9048614501953125),
        (180.90380859375, 155.8941650390625),
    );
    path1.line_to((175.8309783935546875, 141.5211334228515625));
    path1.line_to((175.8309783935546875, 141.5211334228515625));
    path1.close();
    path1.move_to((175.8309783935546875, 141.5211334228515625));
    path1.line_to((187.8782806396484375, 133.7258148193359375));
    path1.line_to((187.8782806396484375, 133.7258148193359375));
    path1.quad_to((188.0, 131.8880615234375), (188.0, 130.0));
    path1.quad_to((188.0, 112.942657470703125), (178.0662384033203125, 100.0));
    path1.line_to((161.1764678955078125, 100.0));
    path1.line_to((161.1764678955078125, 100.0));
    path1.line_to((175.8309783935546875, 141.5211334228515625));
    path1.line_to((175.8309783935546875, 141.5211334228515625));
    path1.close();
    let mut path2 = PathBuilder::new();
    path2.move_to((174.117645263671875, 100.0));
    path2.line_to((161.1764678955078125, 100.0));
    path2.line_to((161.1764678955078125, 100.0));
    path2.line_to((155.1280364990234375, 82.86279296875));
    path2.line_to((155.1280364990234375, 82.86279296875));
    path2.quad_to(
        (153.14971923828125, 82.15229034423828125),
        (151.098419189453125, 81.618133544921875),
    );
    path2.line_to((143.5294189453125, 100.0));
    path2.line_to((143.5294189453125, 100.0));
    path2.line_to((161.1764678955078125, 100.0));
    path2.line_to((161.1764678955078125, 100.0));
    path2.line_to((168.23529052734375, 120.0));
    path2.line_to((168.23529052734375, 120.0));
    path2.line_to((181.1764678955078125, 120.0));
    path2.line_to((181.1764678955078125, 120.0));
    path2.line_to((186.3661956787109375, 134.7042236328125));
    path2.line_to((186.3661956787109375, 134.7042236328125));
    path2.line_to((187.8782806396484375, 133.7258148193359375));
    path2.line_to((187.8782806396484375, 133.7258148193359375));
    path2.quad_to((188.0, 131.8880615234375), (188.0, 130.0));
    path2.quad_to((188.0, 124.80947113037109375), (187.080169677734375, 120.0));
    path2.line_to((181.1764678955078125, 120.0));
    path2.line_to((181.1764678955078125, 120.0));
    path2.line_to((174.117645263671875, 100.0));
    path2.line_to((174.117645263671875, 100.0));
    path2.close();
    path2.move_to((88.91983795166015625, 120.0));
    path2.line_to((107.0588226318359375, 120.0));
    path2.line_to((107.0588226318359375, 120.0));
    path2.line_to((98.68194580078125, 140.343841552734375));
    path2.line_to((98.68194580078125, 140.343841552734375));
    path2.line_to((88.08148956298828125, 133.0560302734375));
    path2.line_to((88.08148956298828125, 133.0560302734375));
    path2.quad_to((88.0, 131.544830322265625), (88.0, 130.0));
    path2.quad_to((88.0, 124.80951690673828125), (88.91983795166015625, 120.0));
    path2.close();
    path2.move_to((96.67621612548828125, 145.21490478515625));
    path2.line_to((98.68194580078125, 140.343841552734375));
    path2.line_to((98.68194580078125, 140.343841552734375));
    path2.line_to((120.68767547607421875, 155.4727783203125));
    path2.line_to((120.68767547607421875, 155.4727783203125));
    path2.line_to((118.68194580078125, 160.343841552734375));
    path2.line_to((118.68194580078125, 160.343841552734375));
    path2.line_to((96.67621612548828125, 145.21490478515625));
    path2.line_to((96.67621612548828125, 145.21490478515625));
    path2.close();
    path2.move_to((113.232177734375, 173.5789947509765625));
    path2.quad_to(
        (116.8802642822265625, 175.69805908203125),
        (120.8179779052734375, 177.1177825927734375),
    );
    path2.line_to((132.2864990234375, 169.6969757080078125));
    path2.line_to((132.2864990234375, 169.6969757080078125));
    path2.line_to((118.68194580078125, 160.343841552734375));
    path2.line_to((118.68194580078125, 160.343841552734375));
    path2.line_to((113.232177734375, 173.5789947509765625));
    path2.line_to((113.232177734375, 173.5789947509765625));
    path2.close();
    test_path_op(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1965-L1989 (chrome/m156)
fn issue1418(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    path1.move_to((0.0, 0.0));
    path1.line_to((1.0, 0.0));
    path1.line_to((1.0, 0.0));
    path1.line_to((1.0, 1.0));
    path1.line_to((1.0, 1.0));
    path1.line_to((0.0, 1.0));
    path1.line_to((0.0, 1.0));
    path1.line_to((0.0, 0.0));
    path1.line_to((0.0, 0.0));
    path1.close();
    let mut path2 = PathBuilder::new();
    path2.move_to((0.64644664525985717773, -0.35355341434478759766));
    path2.quad_to(
        (0.79289329051971435547, -0.50000005960464477539),
        (1.0000001192092895508, -0.50000005960464477539),
    );
    path2.quad_to(
        (1.2071068286895751953, -0.50000005960464477539),
        (1.3535535335540771484, -0.35355341434478759766),
    );
    path2.quad_to(
        (1.5000001192092895508, -0.20710679888725280762),
        (1.5000001192092895508, 0.0),
    );
    path2.quad_to(
        (1.5000001192092895508, 0.20710679888725280762),
        (1.3535535335540771484, 0.35355341434478759766),
    );
    path2.quad_to(
        (1.2071068286895751953, 0.50000005960464477539),
        (1.0000001192092895508, 0.50000005960464477539),
    );
    path2.quad_to(
        (0.79289329051971435547, 0.50000005960464477539),
        (0.64644664525985717773, 0.35355341434478759766),
    );
    path2.quad_to(
        (0.50000005960464477539, 0.20710679888725280762),
        (0.50000005960464477539, 0.0),
    );
    path2.quad_to(
        (0.50000005960464477539, -0.20710679888725280762),
        (0.64644664525985717773, -0.35355341434478759766),
    );
    test_path_op(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L1991-L2002 (chrome/m156)
fn cubic_op85i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((3.0, 4.0));
    path.cubic_to((1.0, 5.0), (4.0, 3.0), (6.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((3.0, 4.0));
    path_b.cubic_to((4.0, 6.0), (4.0, 3.0), (5.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2004-L2032 (chrome/m156)
fn issue1418b(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    path1.move_to((0.0, 0.0));
    path1.line_to((1.0, 0.0));
    path1.line_to((1.0, 1.0));
    path1.line_to((0.0, 1.0));
    path1.line_to((0.0, 0.0));
    path1.close();
    path1.set_fill_type(PathFillType::Winding);
    let mut path2 = PathBuilder::new();
    path2.move_to((0.646446645, -0.353553414));
    path2.quad_to((0.792893291, -0.50000006), (1.00000012, -0.50000006));
    path2.quad_to((1.20710683, -0.50000006), (1.35355353, -0.353553414));
    path2.quad_to((1.50000012, -0.207106799), (1.50000012, 0.0));
    path2.quad_to((1.50000012, 0.207106799), (1.35355353, 0.353553414));
    path2.quad_to((1.20710683, 0.50000006), (1.00000012, 0.50000006));
    path2.quad_to((0.792893291, 0.50000006), (0.646446645, 0.353553414));
    path2.quad_to((0.50000006, 0.207106799), (0.50000006, 0.0));
    path2.quad_to((0.50000006, -0.207106799), (0.646446645, -0.353553414));
    path2.close();
    path2.move_to((1.00000012, 0.50000006));
    path2.line_to((1.00000012, 1.00000012));
    path2.line_to((0.50000006, 1.00000012));
    path2.quad_to((0.50000006, 0.792893291), (0.646446645, 0.646446645));
    path2.quad_to((0.792893291, 0.50000006), (1.00000012, 0.50000006));
    path2.close();
    path2.set_fill_type(PathFillType::EvenOdd);
    test_path_op(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2034-L2043 (chrome/m156)
fn rect_op1i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 4.0, 4.0), PathDirection::CW, None);
    path_b.set_fill_type(PathFillType::Winding);
    path_b.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path_b.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2045-L2054 (chrome/m156)
fn rect_op2i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 3.0, 3.0), PathDirection::CW, None);
    path_b.set_fill_type(PathFillType::Winding);
    path_b.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    path_b.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2056-L2081 (chrome/m156)
fn rect_op3x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.line_to((3.0, 3.0));
    path.line_to((0.0, 3.0));
    path.close();
    path.move_to((2.0, 2.0));
    path.line_to((3.0, 2.0));
    path.line_to((3.0, 3.0));
    path.line_to((2.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 1.0));
    path_b.line_to((3.0, 1.0));
    path_b.line_to((3.0, 3.0));
    path_b.line_to((1.0, 3.0));
    path_b.close();
    path_b.move_to((2.0, 2.0));
    path_b.line_to((3.0, 2.0));
    path_b.line_to((3.0, 3.0));
    path_b.line_to((2.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Xor,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2083-L2132 (chrome/m156)
fn issue1435(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    path1.move_to((160.0, 60.0));
    path1.line_to((220.0, 230.0));
    path1.line_to((60.0, 120.0));
    path1.line_to((260.0, 120.0));
    path1.line_to((90.0, 230.0));
    path1.line_to((160.0, 60.0));
    path1.close();
    path1.set_fill_type(PathFillType::EvenOdd);
    let mut path2 = PathBuilder::new();
    path2.move_to((142.589081, 102.283646));
    path2.quad_to((149.821579, 100.0), (158.0, 100.0));
    path2.quad_to((167.156921, 100.0), (175.128036, 102.862793));
    path2.line_to((181.176468, 120.0));
    path2.line_to((135.294128, 120.0));
    path2.line_to((142.589081, 102.283646));
    path2.close();
    path2.move_to((118.681946, 160.343842));
    path2.line_to((135.294128, 120.0));
    path2.line_to((117.933762, 120.0));
    path2.quad_to((108.0, 132.942657), (108.0, 150.0));
    path2.quad_to((108.0, 151.54483), (108.08149, 153.05603));
    path2.line_to((118.681946, 160.343842));
    path2.close();
    path2.move_to((156.969696, 186.666672));
    path2.line_to((118.681946, 160.343842));
    path2.line_to((113.458946, 173.028259));
    path2.quad_to((116.94117, 179.651855), (122.644661, 185.355347));
    path2.quad_to((130.792465, 193.503143), (140.817978, 197.117783));
    path2.line_to((156.969696, 186.666672));
    path2.close();
    path2.move_to((195.830978, 161.521133));
    path2.line_to((156.969696, 186.666672));
    path2.line_to((173.157288, 197.795639));
    path2.quad_to((184.392426, 194.318268), (193.355347, 185.355347));
    path2.quad_to((197.805817, 180.904861), (200.903809, 175.894165));
    path2.line_to((195.830978, 161.521133));
    path2.close();
    path2.move_to((195.830978, 161.521133));
    path2.line_to((207.878281, 153.725815));
    path2.quad_to((208.0, 151.888062), (208.0, 150.0));
    path2.quad_to((208.0, 132.942657), (198.066238, 120.0));
    path2.line_to((181.176468, 120.0));
    path2.line_to((195.830978, 161.521133));
    path2.close();
    path2.set_fill_type(PathFillType::EvenOdd);
    test_path_op(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2134-L2156 (chrome/m156)
fn skpkkiste_to716(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1173.0, 284.0));
    path.cubic_to(
        (1173.0, 285.125824),
        (1173.37207, 286.164734),
        (1174.0, 287.000488),
    );
    path.line_to((1174.0, 123.999496));
    path.cubic_to(
        (1173.37207, 124.835243),
        (1173.0, 125.874168),
        (1173.0, 127.0),
    );
    path.line_to((1173.0, 284.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1340.0, 122.0));
    path_b.cubic_to((1342.76147, 122.0), (1345.0, 124.238579), (1345.0, 127.0));
    path_b.line_to((1345.0, 284.0));
    path_b.cubic_to((1345.0, 286.761414), (1342.76147, 289.0), (1340.0, 289.0));
    path_b.line_to((1178.0, 289.0));
    path_b.cubic_to((1175.23853, 289.0), (1173.0, 286.761414), (1173.0, 284.0));
    path_b.line_to((1173.0, 127.0));
    path_b.cubic_to((1173.0, 124.238579), (1175.23853, 122.0), (1178.0, 122.0));
    path_b.line_to((1340.0, 122.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2158-L2178 (chrome/m156)
fn loop_edge1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.line_to((3.0, 2.0));
    path.line_to((1.0, 2.0));
    path.line_to((1.0, 1.0));
    path.line_to((2.0, 1.0));
    path.line_to((2.0, 3.0));
    path.line_to((0.0, 3.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((1.0, 2.0));
    path_b.line_to((2.0, 2.0));
    path_b.line_to((2.0, 4.0));
    path_b.line_to((1.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2180-L2200 (chrome/m156)
fn loop_edge2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.line_to((3.0, 2.0));
    path.line_to((1.0, 2.0));
    path.line_to((1.0, 1.0));
    path.line_to((2.0, 1.0));
    path.line_to((2.0, 3.0));
    path.line_to((0.0, 3.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((1.0 - 1e-6, 2.0));
    path_b.line_to((2.0 - 1e-6, 2.0));
    path_b.line_to((2.0 - 1e-6, 4.0));
    path_b.line_to((1.0 - 1e-6, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2202-L2213 (chrome/m156)
fn cubic_op86i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 4.0));
    path.cubic_to((3.0, 4.0), (6.0, 2.0), (5.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((2.0, 6.0));
    path_b.cubic_to((2.0, 5.0), (4.0, 0.0), (4.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2215-L2226 (chrome/m156)
fn cubic_op87u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 2.0), (2.0, 0.0), (6.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((4.0, 6.0), (1.0, 0.0), (2.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2228-L2239 (chrome/m156)
fn cubic_op88u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 5.0), (5.0, 0.0), (6.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((4.0, 6.0), (1.0, 0.0), (5.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2241-L2252 (chrome/m156)
fn cubic_op89u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 3.0));
    path.cubic_to((1.0, 6.0), (5.0, 0.0), (6.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((3.0, 6.0), (3.0, 0.0), (6.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2254-L2265 (chrome/m156)
fn cubic_op90u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 5.0));
    path.cubic_to((1.0, 2.0), (5.0, 2.0), (4.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((2.0, 5.0));
    path_b.cubic_to((1.0, 4.0), (5.0, 0.0), (2.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2267-L2278 (chrome/m156)
fn cubic_op91u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 6.0));
    path.cubic_to((0.0, 3.0), (6.0, 3.0), (5.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((3.0, 6.0));
    path_b.cubic_to((0.0, 5.0), (6.0, 1.0), (3.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2280-L2299 (chrome/m156)
fn skpaaalgarve_org53(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((-1.24344979e-014, 348.0));
    path.line_to((258.0, 348.0));
    path.line_to((258.0, 322.0));
    path.quad_to((258.0, 317.857849), (255.072006, 314.928009));
    path.quad_to((252.142136, 312.0), (248.0, 312.0));
    path.line_to((1.77635684e-015, 312.0));
    path.line_to((-1.24344979e-014, 348.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 312.0));
    path_b.line_to((258.0, 312.0));
    path_b.line_to((258.0, 348.0));
    path_b.line_to((0.0, 348.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2301-L2322 (chrome/m156)
fn skpabcspark_ca103(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1.99840144e-015, 494.0));
    path.line_to((97.0, 494.0));
    path.quad_to((100.313705, 494.0), (102.6576, 491.657593));
    path.quad_to((105.0, 489.313721), (105.0, 486.0));
    path.line_to((105.0, 425.0));
    path.quad_to((105.0, 421.686279), (102.6576, 419.342407));
    path.quad_to((100.313705, 417.0), (97.0, 417.0));
    path.line_to((2.22044605e-016, 417.0));
    path.line_to((1.99840144e-015, 494.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 417.0));
    path_b.line_to((105.0, 417.0));
    path_b.line_to((105.0, 494.0));
    path_b.line_to((0.0, 494.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2324-L2348 (chrome/m156)
fn skpacesoftech_com47(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((670.537415, 285.0));
    path.line_to((670.387451, 285.0));
    path.line_to((596.315186, 314.850708));
    path.line_to((626.19696, 389.0));
    path.line_to((626.346863, 389.0));
    path.line_to((700.419189, 359.149261));
    path.line_to((670.537415, 285.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((663.318542, 374.100616));
    path_b.quad_to((647.950989, 380.293671), (632.705322, 373.806305));
    path_b.quad_to((617.459595, 367.318909), (611.266541, 351.951355));
    path_b.quad_to((605.073486, 336.58374), (611.560913, 321.338074));
    path_b.quad_to((618.048279, 306.092407), (633.415833, 299.899353));
    path_b.quad_to((648.783447, 293.706299), (664.029114, 300.193665));
    path_b.quad_to((679.27478, 306.68103), (685.467834, 322.048645));
    path_b.quad_to((691.660889, 337.416199), (685.173523, 352.661896));
    path_b.quad_to((678.686157, 367.907562), (663.318542, 374.100616));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2350-L2373 (chrome/m156)
fn skpact_com43(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1.45716772e-016, 924.336121));
    path.line_to((-1.11022302e-016, 920.0));
    path.line_to((6.0, 920.0));
    path.line_to((6.0, 926.0));
    path.line_to((1.66389287, 926.0));
    path.quad_to((1.18842196, 925.674561), (0.756800175, 925.243225));
    path.quad_to((0.325406998, 924.811523), (1.45716772e-016, 924.336121));
    path.close();
    path.move_to((1.0, 921.0));
    path.line_to((5.0, 921.0));
    path.line_to((5.0, 925.0));
    path.cubic_to((2.79086018, 925.0), (1.0, 923.209167), (1.0, 921.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((-1.0, 920.0));
    path_b.line_to((0.0, 920.0));
    path_b.line_to((3.0, 927.0));
    path_b.line_to((-1.0, 927.0));
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2375-L2397 (chrome/m156)
fn skpadbox_lt8(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((320.097229, 628.573669));
    path.line_to((610.227173, 85.7786865));
    path.line_to((946.652588, 265.601807));
    path.line_to((656.522644, 808.39679));
    path.line_to((320.097229, 628.573669));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::InverseWinding);
    path_b.move_to((333.866608, 623.496155));
    path_b.line_to((613.368042, 100.585754));
    path_b.cubic_to(
        (613.685303, 99.9921265),
        (614.423767, 99.7681885),
        (615.017395, 100.085449),
    );
    path_b.line_to((932.633057, 269.854553));
    path_b.cubic_to(
        (933.226685, 270.171875),
        (933.450623, 270.910278),
        (933.133301, 271.503906),
    );
    path_b.line_to((653.631897, 794.414307));
    path_b.cubic_to(
        (653.314636, 795.007935),
        (652.576172, 795.231934),
        (651.982544, 794.914612),
    );
    path_b.line_to((334.366943, 625.145508));
    path_b.cubic_to(
        (333.773315, 624.828247),
        (333.549286, 624.089783),
        (333.866608, 623.496155),
    );
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2399-L2416 (chrome/m156)
fn skpadindex_de4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 926.0));
    path.line_to((0.0, 0.0));
    path.line_to((1280.0, 0.0));
    path.line_to((1280.0, 926.0));
    path.line_to((0.0, 926.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 312.0));
    path_b.line_to((8.20486257e-015, 178.0));
    path_b.line_to((49.0, 178.0));
    path_b.line_to((49.0, 312.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2418-L2440 (chrome/m156)
fn skpadithya_putr4_blogspot_com551(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((205.605804, 142.334625));
    path.line_to((254.665359, 85.6058044));
    path.line_to((311.394196, 134.665359));
    path.line_to((262.334625, 191.39418));
    path.line_to((205.605804, 142.334625));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((283.407959, 110.462646));
    path_b.cubic_to(
        (298.864319, 123.829437),
        (300.558258, 147.195221),
        (287.191467, 162.651581),
    );
    path_b.line_to((286.537354, 163.407959));
    path_b.cubic_to(
        (273.170563, 178.864334),
        (249.804779, 180.558258),
        (234.348419, 167.191467),
    );
    path_b.line_to((233.592026, 166.537338));
    path_b.cubic_to(
        (218.135666, 153.170547),
        (216.441727, 129.804779),
        (229.808517, 114.348412),
    );
    path_b.line_to((230.462646, 113.592026));
    path_b.cubic_to(
        (243.829437, 98.1356659),
        (267.195221, 96.4417267),
        (282.651581, 109.808517),
    );
    path_b.line_to((283.407959, 110.462646));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2442-L2461 (chrome/m156)
fn skpadspert_de11(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((-4.4408921e-016, 682.5));
    path.line_to((30.5, 682.5));
    path.cubic_to((32.709137, 682.5), (34.5, 680.709167), (34.5, 678.5));
    path.line_to((34.5, 486.5));
    path.cubic_to((34.5, 484.290863), (32.709137, 482.5), (30.5, 482.5));
    path.line_to((0.0, 482.5));
    path.line_to((-4.4408921e-016, 682.5));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 482.0));
    path_b.line_to((35.0, 482.0));
    path_b.line_to((35.0, 683.0));
    path_b.line_to((0.0, 683.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2463-L2491 (chrome/m156)
fn skpaiaigames_com870(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((324.071075, 845.071045));
    path.cubic_to(
        (324.405151, 844.737),
        (324.715668, 844.379395),
        (325.0, 844.000977),
    );
    path.line_to((325.0, 842.127197));
    path.cubic_to(
        (324.571411, 842.956238),
        (324.017761, 843.710144),
        (323.363953, 844.363953),
    );
    path.line_to((324.071075, 845.071045));
    path.close();
    path.move_to((323.363953, 714.636047));
    path.line_to((324.071075, 713.928955));
    path.cubic_to(
        (324.405151, 714.263),
        (324.715668, 714.620605),
        (325.0, 714.999023),
    );
    path.line_to((325.0, 716.872803));
    path.cubic_to(
        (324.571411, 716.043762),
        (324.017761, 715.289856),
        (323.363953, 714.636047),
    );
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((317.0, 711.0));
    path_b.cubic_to((322.522858, 711.0), (327.0, 715.477173), (327.0, 721.0));
    path_b.line_to((327.0, 838.0));
    path_b.cubic_to((327.0, 843.522827), (322.522858, 848.0), (317.0, 848.0));
    path_b.line_to((155.0, 848.0));
    path_b.cubic_to((149.477158, 848.0), (145.0, 843.522827), (145.0, 838.0));
    path_b.line_to((145.0, 721.0));
    path_b.cubic_to((145.0, 715.477173), (149.477158, 711.0), (155.0, 711.0));
    path_b.line_to((317.0, 711.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2493-L2504 (chrome/m156)
fn cubic_op92i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 6.0), (4.0, 1.0), (5.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((4.0, 5.0), (1.0, 0.0), (6.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2506-L2517 (chrome/m156)
fn cubic_op93d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 6.0), (4.0, 1.0), (4.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((3.0, 4.0), (1.0, 0.0), (6.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2519-L2530 (chrome/m156)
fn cubic_op94u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 3.0));
    path.cubic_to((2.0, 3.0), (5.0, 0.0), (5.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((3.0, 5.0), (3.0, 0.0), (3.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2532-L2554 (chrome/m156)
fn skpadbox_lt15(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((333.292084, 624.570984));
    path.line_to((614.229797, 98.9735107));
    path.line_to((933.457764, 269.604431));
    path.line_to((652.52002, 795.201904));
    path.line_to((333.292084, 624.570984));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((613.368042, 100.585754));
    path_b.cubic_to(
        (613.685303, 99.9921265),
        (614.423767, 99.7681885),
        (615.017395, 100.085449),
    );
    path_b.line_to((932.633057, 269.854553));
    path_b.cubic_to(
        (933.226685, 270.171875),
        (933.450623, 270.910278),
        (933.133301, 271.503906),
    );
    path_b.line_to((653.631897, 794.414307));
    path_b.cubic_to(
        (653.314636, 795.007935),
        (652.576172, 795.231934),
        (651.982544, 794.914612),
    );
    path_b.line_to((334.366943, 625.145508));
    path_b.cubic_to(
        (333.773315, 624.828247),
        (333.549286, 624.089783),
        (333.866608, 623.496155),
    );
    path_b.line_to((613.368042, 100.585754));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2556-L2579 (chrome/m156)
fn skpadoption_org196(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((802.0, 367.0));
    path.line_to((802.0, 324.0));
    path.line_to((956.0, 324.0));
    path.line_to((956.0, 371.0));
    path.quad_to((956.0, 373.071075), (954.536011, 374.536011));
    path.quad_to((953.071045, 376.0), (951.0, 376.0));
    path.line_to((811.0, 376.0));
    path.cubic_to((806.029419, 376.0), (802.0, 371.970551), (802.0, 367.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::InverseWinding);
    path_b.move_to((803.0, 326.0));
    path_b.line_to((955.0, 326.0));
    path_b.line_to((955.0, 370.0));
    path_b.cubic_to((955.0, 372.761414), (952.761414, 375.0), (950.0, 375.0));
    path_b.line_to((808.0, 375.0));
    path_b.cubic_to((805.238586, 375.0), (803.0, 372.761414), (803.0, 370.0));
    path_b.line_to((803.0, 326.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2581-L2606 (chrome/m156)
fn skpadspert_net23(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((-2.220446e-018, 483.5));
    path.line_to((0.0, 482.5));
    path.line_to((30.5, 482.5));
    path.cubic_to((32.709137, 482.5), (34.5, 484.290863), (34.5, 486.5));
    path.line_to((34.5, 678.5));
    path.cubic_to((34.5, 680.709167), (32.709137, 682.5), (30.5, 682.5));
    path.line_to((-4.4408921e-016, 682.5));
    path.line_to((-4.41868766e-016, 681.5));
    path.line_to((30.5, 681.5));
    path.cubic_to((32.1568565, 681.5), (33.5, 680.15686), (33.5, 678.5));
    path.line_to((33.5, 486.5));
    path.cubic_to((33.5, 484.84314), (32.1568565, 483.5), (30.5, 483.5));
    path.line_to((-2.220446e-018, 483.5));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 482.0));
    path_b.line_to((35.0, 482.0));
    path_b.line_to((35.0, 683.0));
    path_b.line_to((0.0, 683.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2608-L2625 (chrome/m156)
fn skpadventistmission_org572(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1182.00037, 926.0));
    path.cubic_to(
        (1181.08813, 924.785583),
        (1179.63586, 924.0),
        (1178.0, 924.0),
    );
    path.line_to((938.0, 924.0));
    path.cubic_to(
        (936.364197, 924.0),
        (934.911865, 924.785583),
        (933.999634, 926.0),
    );
    path.line_to((1182.00037, 926.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((934.0, 924.0));
    path_b.line_to((1182.0, 924.0));
    path_b.line_to((1182.0, 926.0));
    path_b.line_to((934.0, 926.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2627-L2653 (chrome/m156)
fn skpagentxsites_com55(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((925.0, 27.0));
    path.cubic_to((924.447693, 27.0), (924.0, 27.4477158), (924.0, 28.0));
    path.line_to((924.0, 55.0));
    path.cubic_to((924.0, 55.5522842), (924.447693, 56.0), (925.0, 56.0));
    path.line_to((1103.0, 56.0));
    path.cubic_to((1103.55225, 56.0), (1104.0, 55.5522842), (1104.0, 55.0));
    path.line_to((1104.0, 28.0));
    path.cubic_to((1104.0, 27.4477158), (1103.55225, 27.0), (1103.0, 27.0));
    path.line_to((925.0, 27.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1103.0, 27.0));
    path_b.cubic_to((1104.10461, 27.0), (1105.0, 27.8954315), (1105.0, 29.0));
    path_b.line_to((1105.0, 54.0));
    path_b.cubic_to((1105.0, 55.1045685), (1104.10461, 56.0), (1103.0, 56.0));
    path_b.line_to((926.0, 56.0));
    path_b.cubic_to((924.895447, 56.0), (924.0, 55.1045685), (924.0, 54.0));
    path_b.line_to((924.0, 29.0));
    path_b.cubic_to((924.0, 27.8954315), (924.895447, 27.0), (926.0, 27.0));
    path_b.line_to((1103.0, 27.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2655-L2679 (chrome/m156)
fn skpbakosoft_com10(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((190.0, 170.0));
    path.cubic_to((178.9543, 170.0), (170.0, 178.9543), (170.0, 190.0));
    path.cubic_to((170.0, 201.0457), (178.9543, 210.0), (190.0, 210.0));
    path.line_to((370.0, 210.0));
    path.cubic_to((381.045685, 210.0), (390.0, 201.0457), (390.0, 190.0));
    path.cubic_to((390.0, 178.9543), (381.045685, 170.0), (370.0, 170.0));
    path.line_to((190.0, 170.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((210.0, 190.0));
    path_b.quad_to((210.0, 198.284271), (204.142136, 204.142136));
    path_b.quad_to((198.284271, 210.0), (190.0, 210.0));
    path_b.quad_to((181.715729, 210.0), (175.857864, 204.142136));
    path_b.quad_to((170.0, 198.284271), (170.0, 190.0));
    path_b.quad_to((170.0, 181.715729), (175.857864, 175.857864));
    path_b.quad_to((181.715729, 170.0), (190.0, 170.0));
    path_b.quad_to((198.284271, 170.0), (204.142136, 175.857864));
    path_b.quad_to((210.0, 181.715729), (210.0, 190.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2681-L2703 (chrome/m156)
fn skpbambootheme_com12(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((47.8780937, 58.0));
    path.line_to((0.0, 58.0));
    path.line_to((-8.65973959e-015, 96.9914017));
    path.quad_to((20.0654926, 96.6451874), (34.3553391, 82.3553391));
    path.quad_to((44.9466133, 71.764061), (47.8780937, 58.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((-1.0, -3.0));
    path_b.line_to((-1.0, -3.0));
    path_b.cubic_to((26.6142502, -3.0), (49.0, 19.3857498), (49.0, 47.0));
    path_b.line_to((49.0, 47.0));
    path_b.cubic_to((49.0, 74.6142502), (26.6142502, 97.0), (-1.0, 97.0));
    path_b.line_to((-1.0, 97.0));
    path_b.cubic_to((-28.6142502, 97.0), (-51.0, 74.6142502), (-51.0, 47.0));
    path_b.line_to((-51.0, 47.0));
    path_b.cubic_to((-51.0, 19.3857498), (-28.6142502, -3.0), (-1.0, -3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2705-L2722 (chrome/m156)
fn skpakmmos_ru100(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((693.000488, 926.0));
    path.cubic_to((692.164734, 925.37207), (691.125793, 925.0), (690.0, 925.0));
    path.line_to((578.0, 925.0));
    path.cubic_to(
        (576.874207, 925.0),
        (575.835266, 925.37207),
        (574.999512, 926.0),
    );
    path.line_to((693.000488, 926.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((575.0, 925.0));
    path_b.line_to((693.0, 925.0));
    path_b.line_to((693.0, 926.0));
    path_b.line_to((575.0, 926.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2724-L2746 (chrome/m156)
fn skpcarpetplanet_ru22(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((195.0, 785.0));
    path.cubic_to((124.307556, 785.0), (67.0, 841.859863), (67.0, 912.0));
    path.line_to((67.0, 913.0));
    path.cubic_to(
        (67.0, 917.388916),
        (67.2243805, 921.725769),
        (67.662384, 926.0),
    );
    path.line_to((322.0, 926.0));
    path.line_to((322.0, 896.048035));
    path.cubic_to((314.09201, 833.437622), (260.247131, 785.0), (195.0, 785.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((195.0, 785.0));
    path_b.cubic_to((265.140167, 785.0), (322.0, 842.307556), (322.0, 913.0));
    path_b.cubic_to((322.0, 983.692444), (265.140167, 1041.0), (195.0, 1041.0));
    path_b.line_to((194.0, 1041.0));
    path_b.cubic_to((123.85984, 1041.0), (67.0, 983.692444), (67.0, 913.0));
    path_b.cubic_to((67.0, 842.307556), (123.85984, 785.0), (194.0, 785.0));
    path_b.line_to((195.0, 785.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2748-L2772 (chrome/m156)
fn skpcarrot_is24(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((945.0, 597.0));
    path.quad_to((913.93396, 597.0), (891.96698, 618.96698));
    path.quad_to((870.0, 640.93396), (870.0, 672.0));
    path.quad_to((870.0, 703.06604), (891.96698, 725.03302));
    path.quad_to((913.93396, 747.0), (945.0, 747.0));
    path.quad_to((976.06604, 747.0), (998.03302, 725.03302));
    path.quad_to((1020.0, 703.06604), (1020.0, 672.0));
    path.quad_to((1020.0, 640.93396), (998.03302, 618.96698));
    path.quad_to((976.06604, 597.0), (945.0, 597.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((945.080994, 597.161987));
    path_b.cubic_to(
        (903.659973, 597.161987),
        (870.080994, 630.73999),
        (870.080994, 672.161987),
    );
    path_b.cubic_to(
        (870.080994, 676.096008),
        (870.387024, 679.957031),
        (870.971008, 683.726013),
    );
    path_b.cubic_to(
        (876.53302, 719.656006),
        (907.593994, 747.161987),
        (945.080994, 747.161987),
    );
    path_b.cubic_to(
        (982.567993, 747.161987),
        (1013.62903, 719.656006),
        (1019.19104, 683.726013),
    );
    path_b.cubic_to(
        (1019.77502, 679.955017),
        (1020.08099, 676.094971),
        (1020.08099, 672.161987),
    );
    path_b.cubic_to(
        (1020.08002, 630.73999),
        (986.502014, 597.161987),
        (945.080994, 597.161987),
    );
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2774-L2791 (chrome/m156)
fn skpbangalorenest_com4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 926.0));
    path.line_to((0.0, 0.0));
    path.line_to((1265.0, 0.0));
    path.line_to((1265.0, 926.0));
    path.line_to((0.0, 926.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 290.0));
    path_b.line_to((-2.64514972e-014, 146.0));
    path_b.line_to((30.0, 146.0));
    path_b.line_to((30.0, 290.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2793-L2818 (chrome/m156)
fn skpbenzoteh_ru152(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((883.0, 23.0));
    path.line_to((883.0, 0.0));
    path.line_to((1122.5, 0.0));
    path.line_to((1122.5, 25.2136822));
    path.quad_to((1122.14441, 25.9271851), (1121.53601, 26.5359993));
    path.quad_to((1120.07104, 28.0), (1118.0, 28.0));
    path.line_to((888.0, 28.0));
    path.quad_to((885.928955, 28.0), (884.463989, 26.5359993));
    path.quad_to((883.0, 25.0710678), (883.0, 23.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((883.0, 0.0));
    path_b.line_to((1123.0, 0.0));
    path_b.line_to((1123.0, 23.0));
    path_b.quad_to((1123.0, 25.0710678), (1121.53601, 26.5359993));
    path_b.quad_to((1120.07104, 28.0), (1118.0, 28.0));
    path_b.line_to((888.0, 28.0));
    path_b.quad_to((885.928955, 28.0), (884.463989, 26.5359993));
    path_b.quad_to((883.0, 25.0710678), (883.0, 23.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2820-L2845 (chrome/m156)
fn skpbestred_ru37(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((883.0, 23.0));
    path.line_to((883.0, 0.0));
    path.line_to((1122.5, 0.0));
    path.line_to((1122.5, 25.2136822));
    path.quad_to((1122.14441, 25.9271851), (1121.53601, 26.5359993));
    path.quad_to((1120.07104, 28.0), (1118.0, 28.0));
    path.line_to((888.0, 28.0));
    path.quad_to((885.928955, 28.0), (884.463989, 26.5359993));
    path.quad_to((883.0, 25.0710678), (883.0, 23.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((883.0, 0.0));
    path_b.line_to((1123.0, 0.0));
    path_b.line_to((1123.0, 23.0));
    path_b.quad_to((1123.0, 25.0710678), (1121.53601, 26.5359993));
    path_b.quad_to((1120.07104, 28.0), (1118.0, 28.0));
    path_b.line_to((888.0, 28.0));
    path_b.quad_to((885.928955, 28.0), (884.463989, 26.5359993));
    path_b.quad_to((883.0, 25.0710678), (883.0, 23.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2847-L2869 (chrome/m156)
fn skpbingoentertainment_net189(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((896.0, 745.38678));
    path.line_to((896.0, 873.38678));
    path.line_to((922.567993, 876.683716));
    path.line_to((922.567993, 748.683716));
    path.line_to((896.0, 745.38678));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((899.200928, 745.783997));
    path_b.cubic_to(
        (897.119385, 745.525696),
        (895.432007, 752.031982),
        (895.432007, 760.316284),
    );
    path_b.line_to((895.432007, 858.316284));
    path_b.cubic_to(
        (895.432007, 866.600586),
        (897.119385, 873.525696),
        (899.200928, 873.783997),
    );
    path_b.line_to((918.799133, 876.216003));
    path_b.cubic_to(
        (920.880615, 876.474304),
        (922.567993, 869.968018),
        (922.567993, 861.683716),
    );
    path_b.line_to((922.567993, 763.683716));
    path_b.cubic_to(
        (922.567993, 755.399414),
        (920.880615, 748.474304),
        (918.799133, 748.216003),
    );
    path_b.line_to((899.200928, 745.783997));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2871-L2892 (chrome/m156)
fn skpcarrefour_ro62(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1104.0, 453.0));
    path.line_to((399.0, 453.0));
    path.line_to((399.0, 657.0));
    path.cubic_to((399.0, 661.970581), (403.029449, 666.0), (408.0, 666.0));
    path.line_to((1095.0, 666.0));
    path.cubic_to((1099.97058, 666.0), (1104.0, 661.970581), (1104.0, 657.0));
    path.line_to((1104.0, 453.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::InverseWinding);
    path_b.move_to((400.0, 453.0));
    path_b.line_to((1103.0, 453.0));
    path_b.line_to((1103.0, 666.0));
    path_b.line_to((406.0, 666.0));
    path_b.cubic_to((402.686279, 666.0), (400.0, 663.313721), (400.0, 660.0));
    path_b.line_to((400.0, 453.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2894-L2919 (chrome/m156)
fn skpcaffelavazzait_com_ua21(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((883.0, 23.0));
    path.line_to((883.0, 0.0));
    path.line_to((1122.5, 0.0));
    path.line_to((1122.5, 25.2136822));
    path.quad_to((1122.14441, 25.9271851), (1121.53601, 26.5359993));
    path.quad_to((1120.07104, 28.0), (1118.0, 28.0));
    path.line_to((888.0, 28.0));
    path.quad_to((885.928955, 28.0), (884.463989, 26.5359993));
    path.quad_to((883.0, 25.0710678), (883.0, 23.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((883.0, 0.0));
    path_b.line_to((1123.0, 0.0));
    path_b.line_to((1123.0, 23.0));
    path_b.quad_to((1123.0, 25.0710678), (1121.53601, 26.5359993));
    path_b.quad_to((1120.07104, 28.0), (1118.0, 28.0));
    path_b.line_to((888.0, 28.0));
    path_b.quad_to((885.928955, 28.0), (884.463989, 26.5359993));
    path_b.quad_to((883.0, 25.0710678), (883.0, 23.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2921-L2946 (chrome/m156)
fn skpcamcorder_kz21(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((883.0, 23.0));
    path.line_to((883.0, 0.0));
    path.line_to((1122.5, 0.0));
    path.line_to((1122.5, 25.2136822));
    path.quad_to((1122.14441, 25.9271851), (1121.53601, 26.5359993));
    path.quad_to((1120.07104, 28.0), (1118.0, 28.0));
    path.line_to((888.0, 28.0));
    path.quad_to((885.928955, 28.0), (884.463989, 26.5359993));
    path.quad_to((883.0, 25.0710678), (883.0, 23.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((883.0, 0.0));
    path_b.line_to((1123.0, 0.0));
    path_b.line_to((1123.0, 23.0));
    path_b.quad_to((1123.0, 25.0710678), (1121.53601, 26.5359993));
    path_b.quad_to((1120.07104, 28.0), (1118.0, 28.0));
    path_b.line_to((888.0, 28.0));
    path_b.quad_to((885.928955, 28.0), (884.463989, 26.5359993));
    path_b.quad_to((883.0, 25.0710678), (883.0, 23.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2948-L2965 (chrome/m156)
fn skpcavablar_net563(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((160.000488, 918.0));
    path.cubic_to((159.164749, 917.37207), (158.125824, 917.0), (157.0, 917.0));
    path.line_to((94.0, 917.0));
    path.cubic_to(
        (92.874176, 917.0),
        (91.8352661, 917.37207),
        (90.9995193, 918.0),
    );
    path.line_to((160.000488, 918.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((91.0, 917.0));
    path_b.line_to((160.0, 917.0));
    path_b.line_to((160.0, 918.0));
    path_b.line_to((91.0, 918.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2967-L2984 (chrome/m156)
fn skpinsomnia_gr72(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1138.0, 231.0));
    path.line_to((1137.0, 243.625748));
    path.line_to((1137.0, 926.0));
    path.line_to((1139.0, 926.0));
    path.line_to((1139.0, 231.0));
    path.line_to((1138.0, 231.0));
    path.close();
    let mut path_b = PathBuilder::new();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1139.0, 231.0));
    path_b.line_to((1138.0, 231.0));
    path_b.line_to((633.0, 6101.0));
    path_b.line_to((1139.0, 6607.0));
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2986-L2997 (chrome/m156)
fn cubic_op95u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 2.0));
    path.cubic_to((2.0, 3.0), (5.0, 1.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to((2.0, 3.0), (2.0, 0.0), (3.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L2999-L3010 (chrome/m156)
fn cubic_op96d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1.0, 6.0));
    path.cubic_to((0.0, 3.0), (6.0, 3.0), (5.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((3.0, 6.0));
    path_b.cubic_to((0.0, 5.0), (6.0, 1.0), (3.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3012-L3023 (chrome/m156)
fn cubic_op97x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 2.0));
    path.cubic_to((0.0, 6.0), (2.0, 1.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((1.0, 2.0), (2.0, 0.0), (6.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Xor,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3025-L3036 (chrome/m156)
fn cubic_op98x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 3.0));
    path.cubic_to((3.0, 6.0), (4.0, 1.0), (6.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((3.0, 6.0), (3.0, 0.0), (6.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Xor,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3038-L3049 (chrome/m156)
fn cubic_op99(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((3.0, 6.0));
    path.cubic_to((0.0, 3.0), (6.0, 5.0), (5.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((5.0, 6.0));
    path_b.cubic_to((4.0, 5.0), (6.0, 3.0), (3.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3051-L3062 (chrome/m156)
fn cubic_op100(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 2.0), (2.0, 1.0), (4.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((2.0, 4.0), (1.0, 0.0), (2.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3064-L3075 (chrome/m156)
fn cubic_op101(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 3.0), (2.0, 1.0), (5.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((3.0, 5.0), (1.0, 0.0), (3.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3077-L3088 (chrome/m156)
fn cubic_op102(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 2.0), (1.0, 0.0), (3.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((0.0, 3.0), (1.0, 0.0), (2.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3090-L3101 (chrome/m156)
fn cubic_op103(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 5.0), (2.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((1.0, 2.0), (1.0, 0.0), (5.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3103-L3114 (chrome/m156)
fn cubic_op104(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 6.0), (4.0, 0.0), (6.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((1.0, 6.0), (1.0, 0.0), (6.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3116-L3127 (chrome/m156)
fn cubic_op105(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 4.0), (6.0, 5.0), (2.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((5.0, 6.0));
    path_b.cubic_to((0.0, 2.0), (1.0, 0.0), (4.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3129-L3140 (chrome/m156)
fn cubic_op106(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((4.0, 6.0), (2.0, 1.0), (2.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((0.0, 2.0), (1.0, 0.0), (6.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3142-L3153 (chrome/m156)
fn cubic_op107(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((4.0, 6.0), (2.0, 1.0), (2.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((0.0, 2.0), (1.0, 0.0), (6.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3155-L3166 (chrome/m156)
fn cubic_op108(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((4.0, 6.0), (2.0, 1.0), (2.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((0.0, 2.0), (1.0, 0.0), (6.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3168-L3179 (chrome/m156)
fn cubic_op109(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((4.0, 5.0), (6.0, 3.0), (5.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((3.0, 6.0));
    path_b.cubic_to((4.0, 5.0), (1.0, 0.0), (5.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3181-L3190 (chrome/m156)
fn cubic_op110(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 4.0, 4.0), PathDirection::CW, None);
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    path_b.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3192-L3203 (chrome/m156)
fn cubic_op111(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 4.0));
    path.cubic_to((0.0, 5.0), (4.0, 1.0), (3.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((1.0, 3.0), (4.0, 1.0), (5.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3205-L3216 (chrome/m156)
fn x_op1u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1.0, 4.0));
    path.cubic_to((4.0, 5.0), (3.0, 2.0), (6.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((2.0, 3.0));
    path_b.cubic_to((3.0, 6.0), (4.0, 1.0), (5.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3218-L3229 (chrome/m156)
fn x_op1i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1.0, 4.0));
    path.cubic_to((1.0, 5.0), (6.0, 0.0), (5.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((1.0, 5.0), (4.0, 1.0), (5.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3231-L3242 (chrome/m156)
fn x_op2i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1.0, 5.0));
    path.cubic_to((0.0, 4.0), (3.0, 2.0), (6.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((2.0, 3.0));
    path_b.cubic_to((1.0, 6.0), (5.0, 1.0), (4.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3244-L3255 (chrome/m156)
fn x_op3i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 4.0));
    path.cubic_to((0.0, 5.0), (4.0, 1.0), (3.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((1.0, 3.0), (4.0, 1.0), (5.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3257-L3268 (chrome/m156)
fn find_first1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 6.0), (5.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((1.0, 2.0), (1.0, 0.0), (6.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3270-L3281 (chrome/m156)
fn cubic_op112(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 4.0));
    path.cubic_to((2.0, 3.0), (6.0, 4.0), (1.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((4.0, 6.0));
    path_b.cubic_to((0.0, 1.0), (4.0, 2.0), (3.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3283-L3292 (chrome/m156)
fn cubic_op113(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((2.0, 4.0));
    path.cubic_to(
        (3.0, 5.0),
        (2.33333325, 4.33333349),
        (3.83333325, 3.83333349),
    );
    path.close();
    path_b.move_to((3.0, 5.0));
    path_b.cubic_to(
        (2.33333325, 4.33333349),
        (3.83333325, 3.83333349),
        (2.0, 4.0),
    );
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3294-L3305 (chrome/m156)
fn cubic_op114(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 3.0), (-1.0, 2.0), (3.5, 1.33333337));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 3.0));
    path_b.cubic_to((-1.0, 2.0), (3.5, 1.33333337), (0.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3307-L3320 (chrome/m156)
fn cubic_op114as_quad(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 3.0), (-1.0, 2.0), (3.5, 1.33333337));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 3.0));
    path_b.cubic_to((-1.0, 2.0), (3.5, 1.33333337), (0.0, 1.0));
    path_b.close();
    let q_path = cubic_path_to_quads(&path.detach());
    let q_path_b = cubic_path_to_quads(&path_b.detach());
    test_path_op(reporter, &q_path, &q_path_b, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L3322-L3332 (chrome/m156)
fn quad_op10i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 8.0), (3.0, 5.0));
    path.line_to((8.0, 1.0));
    path.close();
    path_b.move_to((0.0, 0.0));
    path_b.quad_to((8.0, 1.0), (4.0, 8.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3334-L3351 (chrome/m156)
fn kari1(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    path1.move_to((39.9375, -5.8359375));
    path1.line_to((40.625, -5.7890625));
    path1.line_to((37.7109375, 1.3515625));
    path1.line_to((37.203125, 0.9609375));
    path1.close();
    let mut path2 = PathBuilder::new();
    path2.move_to((37.52734375, -1.44140625));
    path2.cubic_to(
        (37.8736991882324, -1.69921875),
        (38.1640625, -2.140625),
        (38.3984375, -2.765625),
    );
    path2.line_to((38.640625, -2.609375));
    path2.cubic_to(
        (38.53125, -1.89583337306976),
        (38.0664443969727, -0.154893040657043),
        (38.0664443969727, -0.154893040657043),
    );
    path2.cubic_to(
        (38.0664443969727, -0.154893040657043),
        (37.1809883117676, -1.18359375),
        (37.52734375, -1.44140625),
    );
    path2.close();
    test_path_op(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3353-L3370 (chrome/m156)
fn issue2504(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    path1.move_to((34.2421875, -5.976562976837158203125));
    path1.line_to((35.453121185302734375, 0.0));
    path1.line_to((31.9375, 0.0));
    path1.close();
    let mut path2 = PathBuilder::new();
    path2.move_to((36.71843719482421875, 0.8886508941650390625));
    path2.cubic_to(
        (36.71843719482421875, 0.8886508941650390625),
        (35.123386383056640625, 0.554015457630157470703125),
        (34.511409759521484375, -0.1152553558349609375),
    );
    path2.cubic_to(
        (33.899425506591796875, -0.7845261096954345703125),
        (34.53484344482421875, -5.6777553558349609375),
        (34.53484344482421875, -5.6777553558349609375),
    );
    path2.close();
    test_path_op(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3372-L3390 (chrome/m156)
fn issue2540(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    path1.move_to((26.5054988861083984375, 85.73960113525390625));
    path1.cubic_to(
        (84.19739532470703125, 17.77140045166015625),
        (16.93920135498046875, 101.86199951171875),
        (12.631000518798828125, 105.24700164794921875),
    );
    path1.cubic_to(
        (11.0819997787475585937500000, 106.46399688720703125),
        (11.5260000228881835937500000, 104.464996337890625),
        (11.5260000228881835937500000, 104.464996337890625),
    );
    path1.line_to((23.1654987335205078125, 89.72879791259765625));
    path1.cubic_to(
        (23.1654987335205078125, 89.72879791259765625),
        (-10.1713008880615234375, 119.9160003662109375),
        (-17.1620006561279296875, 120.8249969482421875),
    );
    path1.cubic_to(
        (-19.1149997711181640625, 121.07900238037109375),
        (-18.0380001068115234375, 119.79299163818359375),
        (-18.0380001068115234375, 119.79299163818359375),
    );
    path1.cubic_to(
        (-18.0380001068115234375, 119.79299163818359375),
        (14.22100067138671875, 90.60700225830078125),
        (26.5054988861083984375, 85.73960113525390625),
    );
    path1.close();
    let mut path2 = PathBuilder::new();
    path2.move_to((-25.077999114990234375, 124.9120025634765625));
    path2.cubic_to(
        (-25.077999114990234375, 124.9120025634765625),
        (-25.9509983062744140625, 125.95400238037109375),
        (-24.368999481201171875, 125.7480010986328125),
    );
    path2.cubic_to(
        (-16.06999969482421875, 124.66899871826171875),
        (1.2680000066757202148437500, 91.23999786376953125),
        (37.264003753662109375, 95.35400390625),
    );
    path2.cubic_to(
        (37.264003753662109375, 95.35400390625),
        (11.3710002899169921875, 83.7339935302734375),
        (-25.077999114990234375, 124.9120025634765625),
    );
    path2.close();
    test_path_op(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3392-L3417 (chrome/m156)
fn rects1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((6.0, 0.0));
    path.line_to((6.0, 6.0));
    path.line_to((0.0, 6.0));
    path.close();
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.move_to((0.0, 0.0));
    path_b.line_to((1.0, 0.0));
    path_b.line_to((1.0, 1.0));
    path_b.line_to((0.0, 1.0));
    path_b.close();
    path_b.move_to((0.0, 0.0));
    path_b.line_to((2.0, 0.0));
    path_b.line_to((2.0, 2.0));
    path_b.line_to((0.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3419-L3444 (chrome/m156)
fn rects2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((4.0, 0.0));
    path.line_to((4.0, 4.0));
    path.line_to((0.0, 4.0));
    path.close();
    path.move_to((3.0, 3.0));
    path.line_to((4.0, 3.0));
    path.line_to((4.0, 4.0));
    path.line_to((3.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((3.0, 3.0));
    path_b.line_to((6.0, 3.0));
    path_b.line_to((6.0, 6.0));
    path_b.line_to((3.0, 6.0));
    path_b.close();
    path_b.move_to((3.0, 3.0));
    path_b.line_to((4.0, 3.0));
    path_b.line_to((4.0, 4.0));
    path_b.line_to((3.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3446-L3455 (chrome/m156)
fn rects3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 4.0, 4.0), PathDirection::CW, None);
    path_b.set_fill_type(PathFillType::Winding);
    path_b.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    path_b.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3457-L3466 (chrome/m156)
fn rects4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    path_b.set_fill_type(PathFillType::Winding);
    path_b.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    path_b.add_rect(Rect::new(0.0, 0.0, 3.0, 3.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3468-L3486 (chrome/m156)
fn issue2753(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    path1.move_to((142.701, 110.568));
    path1.line_to((142.957, 100.0));
    path1.line_to((153.835, 100.0));
    path1.line_to((154.592, 108.188));
    path1.cubic_to((154.592, 108.188), (153.173, 108.483), (152.83, 109.412));
    path1.cubic_to((152.83, 109.412), (142.701, 110.568), (142.701, 110.568));
    path1.close();
    let mut path2 = PathBuilder::new();
    path2.move_to((39.0, 124.001));
    path2.cubic_to((39.0, 124.001), (50.6, 117.001), (50.6, 117.001));
    path2.cubic_to((50.6, 117.001), (164.601, 85.2), (188.201, 117.601));
    path2.cubic_to((188.201, 117.601), (174.801, 93.0), (39.0, 124.001));
    path2.close();
    test_path_op(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3488-L3514 (chrome/m156)
fn issue2808(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    let mut path2 = PathBuilder::new();
    path1.move_to((509.20300293, 385.601989746));
    path1.quad_to((509.20300293, 415.68838501), (487.928710938, 436.96270752));
    path1.quad_to(
        (466.654388428, 458.236999512),
        (436.567993164, 458.236999512),
    );
    path1.quad_to((406.4815979, 458.236999512), (385.207275391, 436.96270752));
    path1.quad_to(
        (363.932983398, 415.68838501),
        (363.932983398, 385.601989746),
    );
    path1.quad_to(
        (363.932983398, 355.515594482),
        (385.207275391, 334.241271973),
    );
    path1.quad_to((406.4815979, 312.96697998), (436.567993164, 312.96697998));
    path1.quad_to(
        (466.654388428, 312.96697998),
        (487.928710938, 334.241271973),
    );
    path1.quad_to((509.20300293, 355.515594482), (509.20300293, 385.601989746));
    path1.close();
    path2.move_to((449.033996582, 290.87298584));
    path2.quad_to(
        (449.033996582, 301.028259277),
        (441.853149414, 308.209106445),
    );
    path2.quad_to(
        (434.672271729, 315.389984131),
        (424.516998291, 315.389984131),
    );
    path2.quad_to(
        (414.361724854, 315.389984131),
        (407.180847168, 308.209106445),
    );
    path2.quad_to((400.0, 301.028259277), (400.0, 290.87298584));
    path2.quad_to((400.0, 280.717712402), (407.180847168, 273.536865234));
    path2.quad_to(
        (414.361724854, 266.355987549),
        (424.516998291, 266.355987549),
    );
    path2.quad_to(
        (434.672271729, 266.355987549),
        (441.853149414, 273.536865234),
    );
    path2.quad_to(
        (449.033996582, 280.717712402),
        (449.033996582, 290.87298584),
    );
    path2.close();
    test_path_op(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3516-L3527 (chrome/m156)
fn cubic_op115(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 4.0), (2.0, 1.0), (5.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((3.0, 5.0), (1.0, 0.0), (4.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3529-L3537 (chrome/m156)
fn test_rect1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(30.0, 20.0, 50.0, 50.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(24.0, 20.0, 36.0, 30.0), PathDirection::CCW, None);
    let path2 = Path::new();
    test_path_op(reporter, &path.detach(), &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L3539-L3548 (chrome/m156)
fn test_rect2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 4.0, 5.0, 5.0), PathDirection::CW, None);
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.add_rect(Rect::new(0.0, 0.0, 2.0, 2.0), PathDirection::CW, None);
    path_b.add_rect(Rect::new(0.0, 0.0, 6.0, 6.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3550-L3561 (chrome/m156)
fn cubic_op116(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((4.0, 6.0), (2.0, 0.0), (2.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((0.0, 2.0), (1.0, 0.0), (6.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3563-L3574 (chrome/m156)
fn cubic_op117(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((4.0, 5.0), (6.0, 0.0), (1.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((0.0, 1.0), (1.0, 0.0), (5.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3576-L3587 (chrome/m156)
fn cubic_op118(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((4.0, 6.0), (5.0, 1.0), (6.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to((2.0, 6.0), (1.0, 0.0), (6.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3589-L3598 (chrome/m156)
fn loop1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 1.0));
    path.cubic_to(
        (1.0, 5.0),
        (-5.66666651, 3.33333349),
        (8.83333302, 2.33333349),
    );
    path.close();
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to(
        (-5.66666651, 3.33333349),
        (8.83333302, 2.33333349),
        (0.0, 1.0),
    );
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3600-L3629 (chrome/m156)
fn loop1as_quad(reporter: &mut Reporter, filename: &str) {
    let cubic1 = [
        DPoint { x: 0.0, y: 1.0 },
        DPoint { x: 1.0, y: 5.0 },
        DPoint {
            x: f64::from(-5.66666651_f32),
            y: f64::from(3.33333349_f32),
        },
        DPoint {
            x: f64::from(8.83333302_f32),
            y: f64::from(2.33333349_f32),
        },
    ];
    let cubic2 = [
        DPoint { x: 1.0, y: 5.0 },
        DPoint {
            x: f64::from(-5.66666651_f32),
            y: f64::from(3.33333349_f32),
        },
        DPoint {
            x: f64::from(8.83333302_f32),
            y: f64::from(2.33333349_f32),
        },
        DPoint { x: 0.0, y: 1.0 },
    ];
    let c1 = DCubic::new(cubic1);
    let c2 = DCubic::new(cubic2);
    let mut c1_inflection_ts = [0.0_f64; 2];
    let mut c2_inflection_ts = [0.0_f64; 2];
    let c1_inf_t_count = c1.find_inflections(&mut c1_inflection_ts);
    debug_assert_eq!(c1_inf_t_count, 2);
    let c2_inf_t_count = c2.find_inflections(&mut c2_inflection_ts);
    debug_assert_eq!(c2_inf_t_count, 1);
    debug_assert!(c1_inflection_ts[0] > c1_inflection_ts[1]);
    let c1pair = c1.chop_at(c1_inflection_ts[0]);
    let c1apair = c1pair.first().chop_at(c1_inflection_ts[1]);
    let c2pair = c2.chop_at(c2_inflection_ts[0]);
    let q1 = [c1pair.first().to_quad(), c1pair.second().to_quad()];
    let q1a = [c1apair.first().to_quad(), c1apair.second().to_quad()];
    let q2 = [c2pair.first().to_quad(), c2pair.second().to_quad()];
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to(q1a[0].pts[0].as_sk_point());
    path.quad_to(q1a[0].pts[1].as_sk_point(), q1a[0].pts[2].as_sk_point());
    path.quad_to(q1a[1].pts[1].as_sk_point(), q1a[1].pts[2].as_sk_point());
    path.quad_to(q1[1].pts[1].as_sk_point(), q1[1].pts[2].as_sk_point());
    path.close();
    path_b.move_to(q2[0].pts[0].as_sk_point());
    path_b.quad_to(q2[0].pts[1].as_sk_point(), q2[0].pts[2].as_sk_point());
    path_b.quad_to(q2[1].pts[1].as_sk_point(), q2[1].pts[2].as_sk_point());
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3631-L3640 (chrome/m156)
fn loop2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 4.0), (3.0, 4.0), (4.5, 1.5));
    path.close();
    path_b.move_to((3.0, 4.0));
    path_b.cubic_to((3.0, 4.0), (4.5, 1.5), (0.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3642-L3651 (chrome/m156)
fn loop3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 5.0), (-3.66666651, 0.0), (10.5, -1.66666651));
    path.close();
    path_b.move_to((3.0, 5.0));
    path_b.cubic_to((-3.66666651, 0.0), (10.5, -1.66666651), (0.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3653-L3662 (chrome/m156)
fn loop4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 5.0));
    path.cubic_to((1.0, 5.0), (1.0, 4.0), (0.833333313, 3.0));
    path.close();
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to((1.0, 4.0), (0.833333313, 3.0), (0.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3664-L3671 (chrome/m156)
fn issue3517(reporter: &mut Reporter, filename: &str) {
    let path = parse_path::from_svg(ISSUE3517_STR).unwrap_or_default();
    let path_b = parse_path::from_svg(ISSUE3517_STR_B).unwrap_or_default();
    test_path_op(reporter, &path, &path_b, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L3673-L3684 (chrome/m156)
fn cubic_op119(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 5.0), (2.0, 1.0), (3.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((1.0, 3.0), (1.0, 0.0), (5.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3686-L3697 (chrome/m156)
fn cubic_op120(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 4.0), (2.0, 1.0), (4.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((0.0, 4.0), (1.0, 0.0), (4.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3699-L3710 (chrome/m156)
fn cubic_op121(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 4.0), (3.0, 2.0), (4.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 3.0));
    path_b.cubic_to((3.0, 4.0), (1.0, 0.0), (4.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3713-L3724 (chrome/m156)
fn cubic_op122(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 5.0), (4.0, 1.0), (4.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((0.0, 4.0), (1.0, 0.0), (5.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3726-L3737 (chrome/m156)
fn cubic_op123(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 5.0), (2.0, 0.0), (6.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((0.0, 6.0), (1.0, 0.0), (5.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3739-L3748 (chrome/m156)
fn loop5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 2.0));
    path.cubic_to((1.0, 2.0), (1.0, 1.66666663), (0.833333313, 1.33333325));
    path.close();
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((1.0, 1.66666663), (0.833333313, 1.33333325), (0.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3750-L3759 (chrome/m156)
fn loop6(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 1.0));
    path.cubic_to(
        (1.0, 3.0),
        (-1.66666675, 1.66666663),
        (4.16666651, 1.00000012),
    );
    path.close();
    path_b.move_to((1.0, 3.0));
    path_b.cubic_to(
        (-1.66666675, 1.66666663),
        (4.16666651, 1.00000012),
        (0.0, 1.0),
    );
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3761-L3772 (chrome/m156)
fn cubic_op124(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 5.0), (6.0, 0.0), (3.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((0.0, 3.0), (1.0, 0.0), (5.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3774-L3785 (chrome/m156)
fn cubic_op125(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 6.0), (3.0, 1.0), (6.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 3.0));
    path_b.cubic_to((2.0, 6.0), (1.0, 0.0), (6.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3787-L3798 (chrome/m156)
fn cubic_op126(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 3.0), (6.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((1.0, 2.0), (1.0, 0.0), (3.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3800-L3811 (chrome/m156)
fn cubic_op127(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 5.0), (6.0, 0.0), (3.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((0.0, 3.0), (1.0, 0.0), (5.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3813-L3824 (chrome/m156)
fn cubic_op128(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((0.0, 3.0), (3.0, 2.0), (5.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 3.0));
    path_b.cubic_to((2.0, 5.0), (1.0, 0.0), (3.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3826-L3837 (chrome/m156)
fn cubic_op129(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((5.0, 6.0));
    path.cubic_to((3.0, 4.0), (2.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((1.0, 2.0), (6.0, 5.0), (4.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3839-L3850 (chrome/m156)
fn cubic_op130(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((5.0, 6.0));
    path.cubic_to((4.0, 6.0), (3.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 3.0));
    path_b.cubic_to((1.0, 2.0), (6.0, 5.0), (6.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3869-L3882 (chrome/m156)
fn cubic_op130a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((5.0, 6.0));
    let pts = [
        Point::new(5.0, 6.0),
        Point::new(4.0, 6.0),
        Point::new(3.0, 0.0),
        Point::new(2.0, 1.0),
    ];
    complex_to_quads(pts, &mut path);
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 3.0));
    let pts2 = [
        Point::new(0.0, 3.0),
        Point::new(1.0, 2.0),
        Point::new(6.0, 5.0),
        Point::new(6.0, 4.0),
    ];
    // Skia passes `&path` here, not `pathB`; kept as in the original.
    complex_to_quads(pts2, &mut path);
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3884-L3895 (chrome/m156)
fn cubic_op131(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 4.0), (3.0, 0.0), (6.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 3.0));
    path_b.cubic_to((2.0, 6.0), (1.0, 0.0), (4.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3897-L3904 (chrome/m156)
fn circles_op1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_circle((0.0, 1.0), 2.0, PathDirection::CCW);
    path_b.set_fill_type(PathFillType::Winding);
    path_b.add_circle((0.0, 1.0), 1.0, PathDirection::CW);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3906-L3913 (chrome/m156)
fn circles_op2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_circle((0.0, 1.0), 4.0, PathDirection::CCW);
    path_b.set_fill_type(PathFillType::Winding);
    path_b.add_circle((0.0, 4.0), 3.0, PathDirection::CW);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3915-L3957 (chrome/m156)
fn r_rect1x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    // `pathB` is declared but never used in Skia.
    let _path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((20.65, 5.65));
    path.conic_to((20.65, 1.13612), (25.1404, 0.65), 0.888488);
    path.line_to((25.65, 0.65));
    path.line_to((26.1596, 0.67604));
    path.conic_to((30.65, 1.13612), (30.65, 5.65), 0.888488);
    path.line_to((30.65, 25.65));
    path.conic_to((30.65, 20.65), (25.65, 20.65), 0.707107);
    path.line_to((20.65, 20.65));
    path.line_to((20.65, 5.65));
    path.close();
    path.move_to((20.65, 20.65));
    path.line_to((5.65, 20.65));
    path.conic_to((0.65, 20.65), (0.65, 25.65), 0.707107);
    path.line_to((0.65, 45.65));
    path.conic_to((0.65, 50.65), (5.65, 50.65), 0.707107);
    path.line_to((25.65, 50.65));
    path.conic_to((30.65, 50.65), (30.65, 45.65), 0.707107);
    path.line_to((30.65, 25.65));
    path.conic_to((30.65, 30.65), (25.65, 30.65), 0.707107);
    path.conic_to((20.65, 30.65), (20.65, 25.65), 0.707107);
    path.line_to((20.65, 20.65));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((20.65, 45.65));
    path.line_to((20.65, 25.65));
    path.conic_to((20.65, 20.65), (25.65, 20.65), 0.707107);
    path.line_to((45.65, 20.65));
    path.conic_to((50.65, 20.65), (50.65, 25.65), 0.707107);
    path.line_to((50.65, 45.65));
    path.conic_to((50.65, 50.65), (45.65, 50.65), 0.707107);
    path.line_to((25.65, 50.65));
    path.conic_to((20.65, 50.65), (20.65, 45.65), 0.707107);
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L3959-L3968 (chrome/m156)
fn loop7(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 4.0), (-1.0, 0.0), (8.5, -2.5));
    path.close();
    path_b.move_to((3.0, 4.0));
    path_b.cubic_to((-1.0, 0.0), (8.5, -2.5), (0.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3970-L3979 (chrome/m156)
fn rects5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_rect(Rect::new(5.0, 5.0, 6.0, 6.0), PathDirection::CW, None);
    path.add_rect(Rect::new(5.0, 5.0, 6.0, 6.0), PathDirection::CW, None);
    path_b.set_fill_type(PathFillType::EvenOdd);
    path_b.add_rect(Rect::new(0.0, 0.0, 6.0, 6.0), PathDirection::CW, None);
    path_b.add_rect(Rect::new(5.0, 5.0, 6.0, 6.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3981-L3990 (chrome/m156)
fn loop8(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 4.0), (-3.83333325, 0.166666627), (6.0, -1.0));
    path.close();
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((-3.83333325, 0.166666627), (6.0, -1.0), (0.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L3992-L4001 (chrome/m156)
fn loop9(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 3.0), (-2.5, 0.0), (3.33333325, -0.666666627));
    path.close();
    path_b.move_to((1.0, 3.0));
    path_b.cubic_to((-2.5, 0.0), (3.33333325, -0.666666627), (0.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4003-L4010 (chrome/m156)
fn circles_op3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_circle((0.0, 1.0), 2.0, PathDirection::CCW);
    path_b.set_fill_type(PathFillType::Winding);
    path_b.add_circle((3.0, 5.0), 3.0, PathDirection::CW);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4012-L4021 (chrome/m156)
fn loop10(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((5.0, 6.0));
    path.cubic_to((1.0, 2.0), (1.0, 2.0), (-3.66666651, 13.333334));
    path.close();
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((1.0, 2.0), (-3.66666651, 13.333334), (5.0, 6.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4023-L4032 (chrome/m156)
fn loop11(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 3.0), (-1.83333349, 1.33333337), (4.0, -1.0));
    path.close();
    path_b.move_to((1.0, 3.0));
    path_b.cubic_to((-1.83333349, 1.33333337), (4.0, -1.0), (0.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4034-L4045 (chrome/m156)
fn cubic_op132(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((5.0, 6.0));
    path.cubic_to((3.0, 4.0), (3.0, 0.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 3.0));
    path_b.cubic_to((2.0, 3.0), (6.0, 5.0), (4.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4047-L4056 (chrome/m156)
fn loop12(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((1.0, 2.0));
    path.cubic_to(
        (0.0, 6.0),
        (-3.16666675, 3.66666675),
        (6.33333349, 3.33333349),
    );
    path.close();
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to(
        (-3.16666675, 3.66666675),
        (6.33333349, 3.33333349),
        (1.0, 2.0),
    );
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4058-L4069 (chrome/m156)
fn cubic_op133(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((5.0, 6.0));
    path.cubic_to((5.0, 6.0), (5.0, 0.0), (4.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((1.0, 4.0), (6.0, 5.0), (6.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4071-L4082 (chrome/m156)
fn cubic_op134(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((5.0, 6.0));
    path.cubic_to((5.0, 6.0), (6.0, 0.0), (3.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((1.0, 3.0), (6.0, 5.0), (6.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4084-L4095 (chrome/m156)
fn cubic_op135(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((5.0, 6.0));
    path.cubic_to((5.0, 6.0), (6.0, 0.0), (4.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((1.0, 4.0), (6.0, 5.0), (6.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4097-L4108 (chrome/m156)
fn cubic_op136(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((5.0, 6.0));
    path.cubic_to((5.0, 6.0), (5.0, 0.0), (3.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((1.0, 3.0), (6.0, 5.0), (6.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4110-L4121 (chrome/m156)
fn cubic_op136a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((5.0, 6.0));
    path.quad_to((5.0, 0.0), (3.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((1.0, 3.0), (6.0, 5.0), (6.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4123-L4134 (chrome/m156)
fn cubics137(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 5.0));
    path.cubic_to((3.0, 6.0), (1.0, 0.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((2.0, 3.0), (5.0, 0.0), (6.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4136-L4147 (chrome/m156)
fn cubics138(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 5.0));
    path.cubic_to((3.0, 6.0), (1.0, 0.0), (4.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((2.0, 4.0), (5.0, 0.0), (6.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4150-L4161 (chrome/m156)
fn cubic_op139(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((0.0, 4.0), (3.0, 1.0), (5.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 3.0));
    path_b.cubic_to((1.0, 5.0), (2.0, 0.0), (4.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4163-L4174 (chrome/m156)
fn cubic_op140(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((1.0, 2.0), (5.0, 4.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((4.0, 5.0));
    path_b.cubic_to((2.0, 3.0), (2.0, 0.0), (2.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4176-L4187 (chrome/m156)
fn cubic_op141(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((1.0, 2.0), (6.0, 4.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((4.0, 6.0));
    path_b.cubic_to((2.0, 3.0), (2.0, 0.0), (2.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4189-L4197 (chrome/m156)
fn quad_rect1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((6.0, 15.0));
    path.quad_to((16.0, 0.0), (8.0, 4.0));
    path.quad_to((2.0, 7.0), (12.0, 12.0));
    path.close();
    path_b.add_rect(Rect::new(4.0, 11.0, 13.0, 16.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4199-L4207 (chrome/m156)
fn quad_rect2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((5.0, 12.0));
    path.quad_to((15.0, 7.0), (9.0, 4.0));
    path.quad_to((1.0, 0.0), (11.0, 15.0));
    path.close();
    path_b.add_rect(Rect::new(4.0, 11.0, 13.0, 16.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4209-L4217 (chrome/m156)
fn quad_rect3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((12.0, 12.0));
    path.quad_to((2.0, 7.0), (8.0, 4.0));
    path.quad_to((16.0, 0.0), (6.0, 15.0));
    path.close();
    path_b.add_rect(Rect::new(4.0, 11.0, 13.0, 16.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4219-L4227 (chrome/m156)
fn quad_rect4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((11.0, 15.0));
    path.quad_to((1.0, 0.0), (9.0, 4.0));
    path.quad_to((15.0, 7.0), (5.0, 12.0));
    path.close();
    path_b.add_rect(Rect::new(4.0, 11.0, 13.0, 16.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4229-L4237 (chrome/m156)
fn quad_rect5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((11.0, 13.0));
    path.quad_to((4.0, 4.0), (8.0, 4.0));
    path.quad_to((12.0, 4.0), (5.0, 13.0));
    path.close();
    path_b.add_rect(Rect::new(4.0, 11.0, 13.0, 16.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4239-L4247 (chrome/m156)
fn quad_rect6(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((5.0, 13.0));
    path.quad_to((12.0, 4.0), (8.0, 4.0));
    path.quad_to((4.0, 4.0), (11.0, 13.0));
    path.close();
    path_b.add_rect(Rect::new(4.0, 11.0, 13.0, 16.0), PathDirection::CW, None);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4249-L4260 (chrome/m156)
fn loops4i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 3.0));
    path.cubic_to((0.0, 2.0), (0.0, 2.0), (-1.66666663, 2.16666675));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((0.0, 2.0), (-1.66666663, 2.16666675), (0.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4262-L4273 (chrome/m156)
fn loops5i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 2.0));
    path.cubic_to((0.0, 2.0), (0.0, 2.0), (0.166666672, 2.66666675));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((0.0, 2.0), (0.166666672, 2.66666675), (1.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4275-L4286 (chrome/m156)
fn cubic_op142(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((5.0, 6.0));
    path.cubic_to((2.0, 5.0), (2.0, 1.0), (1.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((0.0, 1.0), (6.0, 5.0), (5.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4288-L4299 (chrome/m156)
fn cubics6d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((3.0, 5.0));
    path.cubic_to((1.0, 5.0), (4.0, 2.0), (4.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 4.0));
    path_b.cubic_to((0.0, 4.0), (5.0, 3.0), (5.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4301-L4312 (chrome/m156)
fn cubics7d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 6.0));
    path.cubic_to((2.0, 4.0), (5.0, 1.0), (3.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to((1.0, 3.0), (6.0, 2.0), (4.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4314-L4325 (chrome/m156)
fn cubics8d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 5.0));
    path.cubic_to((2.0, 4.0), (5.0, 1.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to((2.0, 3.0), (5.0, 2.0), (4.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4327-L4338 (chrome/m156)
fn cubics9d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 4.0));
    path.cubic_to((2.0, 6.0), (3.0, 1.0), (5.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 3.0));
    path_b.cubic_to((1.0, 5.0), (4.0, 2.0), (6.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4340-L4351 (chrome/m156)
fn cubics10u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 4.0));
    path.cubic_to((1.0, 6.0), (4.0, 1.0), (5.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((1.0, 5.0), (4.0, 2.0), (6.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4353-L4364 (chrome/m156)
fn cubics11i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 4.0));
    path.cubic_to((2.0, 5.0), (3.0, 2.0), (5.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 3.0));
    path_b.cubic_to((1.0, 5.0), (4.0, 2.0), (5.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4366-L4377 (chrome/m156)
fn cubics12d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 4.0));
    path.cubic_to((0.0, 4.0), (5.0, 3.0), (5.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((3.0, 5.0));
    path_b.cubic_to((1.0, 5.0), (4.0, 2.0), (4.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4379-L4390 (chrome/m156)
fn cubics13d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 3.0));
    path.cubic_to((1.0, 5.0), (4.0, 2.0), (5.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 4.0));
    path_b.cubic_to((2.0, 5.0), (3.0, 2.0), (5.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4392-L4403 (chrome/m156)
fn cubics14d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 3.0));
    path.cubic_to((0.0, 4.0), (3.0, 1.0), (3.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 3.0));
    path_b.cubic_to((0.0, 3.0), (3.0, 2.0), (4.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4405-L4416 (chrome/m156)
fn cubics15d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 5.0));
    path.cubic_to((3.0, 5.0), (4.0, 0.0), (4.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((2.0, 4.0), (5.0, 1.0), (5.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4418-L4429 (chrome/m156)
fn cubics16i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 5.0));
    path.cubic_to((2.0, 5.0), (5.0, 0.0), (4.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((2.0, 4.0), (5.0, 1.0), (5.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4431-L4442 (chrome/m156)
fn cubics17d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 5.0));
    path.cubic_to((3.0, 4.0), (4.0, 1.0), (4.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((2.0, 4.0), (5.0, 1.0), (4.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4444-L4455 (chrome/m156)
fn cubics18d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 5.0));
    path.cubic_to((1.0, 3.0), (4.0, 0.0), (2.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((0.0, 2.0), (5.0, 1.0), (3.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4457-L4468 (chrome/m156)
fn cubics19d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 5.0));
    path.cubic_to((2.0, 3.0), (5.0, 2.0), (4.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 5.0));
    path_b.cubic_to((2.0, 4.0), (5.0, 1.0), (3.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4470-L4481 (chrome/m156)
fn cubic_op157(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 5.0));
    path.cubic_to((1.0, 3.0), (6.0, 2.0), (4.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 6.0));
    path_b.cubic_to((2.0, 4.0), (5.0, 1.0), (3.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4483-L4494 (chrome/m156)
fn cubics20d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 2.0));
    path.cubic_to((0.0, 3.0), (6.0, 0.0), (3.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((2.0, 3.0), (2.0, 1.0), (3.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4496-L4507 (chrome/m156)
fn loops20i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 2.0));
    path.cubic_to((0.0, 2.0), (0.833333313, 2.0), (1.0, 3.66666651));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((0.833333313, 2.0), (1.0, 3.66666651), (1.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4509-L4520 (chrome/m156)
fn loops21i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 2.0));
    path.cubic_to((0.0, 2.0), (0.833333313, 2.0), (1.0, 4.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((0.833333313, 2.0), (1.0, 4.0), (1.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4522-L4533 (chrome/m156)
fn loops22i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 3.0));
    path.cubic_to((0.0, 3.0), (0.833333313, 3.0), (1.0, 4.66666651));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 3.0));
    path_b.cubic_to((0.833333313, 3.0), (1.0, 4.66666651), (1.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4535-L4546 (chrome/m156)
fn loops23i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 5.0));
    path.cubic_to(
        (0.0, 1.0),
        (6.16666698, 5.66666698),
        (-5.66666651, 6.66666651),
    );
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to(
        (6.16666698, 5.66666698),
        (-5.66666651, 6.66666651),
        (1.0, 5.0),
    );
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4548-L4559 (chrome/m156)
fn loops24i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 2.0));
    path.cubic_to((0.0, 2.0), (0.833333313, 2.0), (1.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((0.833333313, 2.0), (1.0, 3.0), (1.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4561-L4572 (chrome/m156)
fn loops25i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 5.0));
    path.cubic_to((0.0, 5.0), (0.833333313, 5.0), (1.0, 7.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((0.833333313, 5.0), (1.0, 7.0), (1.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4574-L4585 (chrome/m156)
fn loops26i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 6.0));
    path.cubic_to(
        (0.0, 2.0),
        (6.16666698, 6.66666698),
        (-5.66666651, 7.66666651),
    );
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to(
        (6.16666698, 6.66666698),
        (-5.66666651, 7.66666651),
        (1.0, 6.0),
    );
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4587-L4598 (chrome/m156)
fn loops27i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 3.0));
    path.cubic_to((0.0, 3.0), (0.833333313, 3.0), (1.0, 4.33333349));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 3.0));
    path_b.cubic_to((0.833333313, 3.0), (1.0, 4.33333349), (1.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4600-L4611 (chrome/m156)
fn loops28i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 3.0));
    path.cubic_to((1.0, 3.0), (1.83333337, 3.0), (2.0, 4.66666651));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 3.0));
    path_b.cubic_to((1.83333337, 3.0), (2.0, 4.66666651), (2.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4613-L4624 (chrome/m156)
fn loops29i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 4.0));
    path.cubic_to((0.0, 4.0), (1.66666663, 4.0), (2.0, 7.33333302));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((1.66666663, 4.0), (2.0, 7.33333302), (2.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4626-L4637 (chrome/m156)
fn loops30i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 4.0));
    path.cubic_to((0.0, 4.0), (1.66666663, 4.0), (2.0, 8.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((1.66666663, 4.0), (2.0, 8.0), (2.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4639-L4650 (chrome/m156)
fn loops31i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 5.0));
    path.cubic_to((1.0, 5.0), (1.83333337, 5.0), (2.0, 6.66666651));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to((1.83333337, 5.0), (2.0, 6.66666651), (2.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4652-L4663 (chrome/m156)
fn loops32i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 6.0));
    path.cubic_to((1.0, 6.0), (1.83333337, 6.0), (2.0, 8.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 6.0));
    path_b.cubic_to((1.83333337, 6.0), (2.0, 8.0), (2.0, 6.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4665-L4676 (chrome/m156)
fn loops33i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 6.0));
    path.cubic_to(
        (1.0, 2.0),
        (7.16666698, 6.66666698),
        (-4.66666651, 7.66666651),
    );
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to(
        (7.16666698, 6.66666698),
        (-4.66666651, 7.66666651),
        (2.0, 6.0),
    );
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4678-L4709 (chrome/m156)
// The loop around the body is commented out in Skia, so the state it updates is never read again.
#[allow(unused_assignments)]
fn loops33i_mod(reporter: &mut Reporter, filename: &str) {
    let mut pts = [
        Point::new(2.0, 6.0),
        Point::new(1.0, 2.0),
        Point::new(7.16666698_f32, 6.66666698_f32),
        Point::new(-4.66666651_f32, 7.66666651_f32),
        Point::new(1.0, 2.0),
        Point::new(7.16666698_f32, 6.66666698_f32),
        Point::new(-4.66666651_f32, 7.66666651_f32),
        Point::new(2.0, 6.0),
    ];
    let mut up = false;
    let mut offset: f32 = 0.0380172729_f32;
    let mut step: f32 = 7.62939453e-006_f32;
    let mut last_result = true;
    // for (int i = 0; i < 30; ++i) {
    let name = filename;
    pts[5].y = 6.66666698_f32 + offset;
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to(pts[0]);
    path.cubic_to(pts[1], pts[2], pts[3]);
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to(pts[4]);
    path_b.cubic_to(pts[5], pts[6], pts[7]);
    path_b.close();
    let result = test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        name,
    );
    if last_result != result {
        up = !up;
    }
    step /= 2.0;
    offset += if up { step } else { -step };
    last_result = result;
    // }
}

// Port of: tests/PathOpsOpTest.cpp#L4712-L4725 (chrome/m156)
fn loops33i_as_quads(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 6.0));
    path.cubic_to(
        (1.0, 2.0),
        (7.16666698, 6.66666698),
        (-4.66666651, 7.66666651),
    );
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to(
        (7.16666698, 6.66666698),
        (-4.66666651, 7.66666651),
        (2.0, 6.0),
    );
    path_b.close();
    let q_path = cubic_path_to_quads(&path.detach());
    let q_path_b = cubic_path_to_quads(&path_b.detach());
    test_path_op(reporter, &q_path, &q_path_b, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L4727-L4738 (chrome/m156)
fn loops34i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((3.0, 4.0));
    path.cubic_to((0.0, 4.0), (2.5, 4.0), (3.0, 9.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((2.5, 4.0), (3.0, 9.0), (3.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4740-L4751 (chrome/m156)
fn loops35i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((3.0, 4.0));
    path.cubic_to((0.0, 4.0), (2.5, 4.0), (3.0, 10.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((2.5, 4.0), (3.0, 10.0), (3.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4753-L4764 (chrome/m156)
fn loops36i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((3.0, 4.0));
    path.cubic_to((1.0, 4.0), (2.66666675, 4.0), (3.0, 8.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((2.66666675, 4.0), (3.0, 8.0), (3.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4766-L4777 (chrome/m156)
fn loops37i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 4.0));
    path.cubic_to((1.0, 4.0), (1.83333337, 4.0), (2.0, 5.33333349));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((1.83333337, 4.0), (2.0, 5.33333349), (2.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4779-L4790 (chrome/m156)
fn loops38i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((3.0, 4.0));
    path.cubic_to((2.0, 4.0), (2.83333325, 4.0), (3.0, 6.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 4.0));
    path_b.cubic_to((2.83333325, 4.0), (3.0, 6.0), (3.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4792-L4803 (chrome/m156)
fn loops39i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((3.0, 5.0));
    path.cubic_to((0.0, 5.0), (2.5, 5.0), (3.0, 10.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((2.5, 5.0), (3.0, 10.0), (3.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4805-L4816 (chrome/m156)
fn loops40i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((3.0, 5.0));
    path.cubic_to((0.0, 5.0), (2.5, 5.0), (3.0, 11.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((2.5, 5.0), (3.0, 11.0), (3.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4818-L4831 (chrome/m156)
fn loops40i_as_quads(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((3.0, 5.0));
    path.cubic_to((0.0, 5.0), (2.5, 5.0), (3.0, 11.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((2.5, 5.0), (3.0, 11.0), (3.0, 5.0));
    path_b.close();
    let q_path = cubic_path_to_quads(&path.detach());
    let q_path_b = cubic_path_to_quads(&path_b.detach());
    test_path_op(reporter, &q_path, &q_path_b, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L4833-L4844 (chrome/m156)
fn loops44i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 5.0));
    path.cubic_to((0.0, 1.0), (7.33333302, 5.33333349), (-7.0, 7.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((7.33333302, 5.33333349), (-7.0, 7.0), (1.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4846-L4857 (chrome/m156)
fn loops45i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 6.0));
    path.cubic_to((0.0, 2.0), (7.33333302, 6.33333302), (-7.0, 8.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((7.33333302, 6.33333302), (-7.0, 8.0), (1.0, 6.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4859-L4870 (chrome/m156)
fn loops46i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 6.0));
    path.cubic_to((1.0, 2.0), (8.33333302, 6.33333302), (-6.0, 8.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((8.33333302, 6.33333302), (-6.0, 8.0), (2.0, 6.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4874-L4885 (chrome/m156)
fn loops47i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 4.0));
    path.cubic_to((0.0, 1.0), (6.0, 5.83333302), (-4.0, 8.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((6.0, 5.83333302), (-4.0, 8.0), (2.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4887-L4898 (chrome/m156)
fn loops48i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 6.0));
    path.cubic_to(
        (0.0, 1.0),
        (9.33333302, 6.83333302),
        (-8.33333302, 9.16666603),
    );
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to(
        (9.33333302, 6.83333302),
        (-8.33333302, 9.16666603),
        (2.0, 6.0),
    );
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4900-L4911 (chrome/m156)
fn loops49i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((1.0, 4.0), (-0.166666687, 2.66666675), (1.66666675, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((-0.166666687, 2.66666675), (1.66666675, 2.0), (0.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4913-L4924 (chrome/m156)
fn loops50i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 3.0));
    path.cubic_to((1.0, 5.0), (-0.166666687, 3.66666675), (1.66666675, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to((-0.166666687, 3.66666675), (1.66666675, 3.0), (0.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4926-L4937 (chrome/m156)
fn loops51i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 2.0));
    path.cubic_to((2.0, 4.0), (0.833333313, 2.66666675), (2.66666675, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 4.0));
    path_b.cubic_to((0.833333313, 2.66666675), (2.66666675, 2.0), (1.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4939-L4950 (chrome/m156)
fn loops52i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 3.0));
    path.cubic_to((2.0, 5.0), (0.833333313, 3.66666675), (2.66666675, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 5.0));
    path_b.cubic_to((0.833333313, 3.66666675), (2.66666675, 3.0), (1.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4952-L4963 (chrome/m156)
fn loops53i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 3.0));
    path.cubic_to((3.0, 5.0), (1.83333325, 3.66666675), (3.66666651, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((3.0, 5.0));
    path_b.cubic_to((1.83333325, 3.66666675), (3.66666651, 3.0), (2.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4965-L4976 (chrome/m156)
fn loops54i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((1.0, 4.0), (0.0, 3.0), (1.66666675, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 4.0));
    path_b.cubic_to((0.0, 3.0), (1.66666675, 2.0), (0.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4978-L4989 (chrome/m156)
fn loops55i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 3.0));
    path.cubic_to((1.0, 5.0), (0.0, 4.0), (1.66666675, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to((0.0, 4.0), (1.66666675, 3.0), (0.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L4991-L5002 (chrome/m156)
fn loops56i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 2.0));
    path.cubic_to((2.0, 4.0), (0.99999994, 3.0), (2.66666675, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 4.0));
    path_b.cubic_to((0.99999994, 3.0), (2.66666675, 2.0), (1.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5004-L5015 (chrome/m156)
fn loops57i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 3.0));
    path.cubic_to((2.0, 5.0), (0.99999994, 4.0), (2.66666675, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 5.0));
    path_b.cubic_to((0.99999994, 4.0), (2.66666675, 3.0), (1.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5017-L5028 (chrome/m156)
fn loops58i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 3.0));
    path.cubic_to((3.0, 5.0), (2.0, 4.0), (3.66666651, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((3.0, 5.0));
    path_b.cubic_to((2.0, 4.0), (3.66666651, 3.0), (2.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5030-L5046 (chrome/m156)
fn loops58i_as_quads(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 3.0));
    path.cubic_to((3.0, 5.0), (2.0, 4.0), (3.66666651, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((3.0, 5.0));
    path_b.cubic_to((2.0, 4.0), (3.66666651, 3.0), (2.0, 3.0));
    path_b.close();
    let q_path = cubic_path_to_quads(&path.detach());
    let q_path_b = cubic_path_to_quads(&path_b.detach());
    test_path_op(reporter, &q_path, &q_path_b, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L5048-L5059 (chrome/m156)
fn loops59i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 6.0));
    path.cubic_to((1.0, 2.0), (7.33333302, 1.66666663), (-7.5, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((7.33333302, 1.66666663), (-7.5, 2.0), (0.0, 6.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5061-L5077 (chrome/m156)
fn loops59ias_quads(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 6.0));
    path.cubic_to((1.0, 2.0), (7.33333302_f32, 1.66666663_f32), (-7.5, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 2.0));
    path_b.cubic_to((7.33333302_f32, 1.66666663_f32), (-7.5, 2.0), (0.0, 6.0));
    path_b.close();
    let q_path = cubic_path_to_quads(&path.detach());
    let mut q_path_b = cubic_path_to_quads(&path_b.detach());
    let from = Point::new(2.61714339_f32, 1.90228665_f32);
    let to = Point::new(2.617045833359139_f32, 1.9013528935803314_f32);
    path_edit(from, to, &mut q_path_b);
    test_path_op(reporter, &q_path, &q_path_b, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L5079-L5090 (chrome/m156)
fn cubics41d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 4.0), (3.0, 0.0), (3.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 3.0));
    path_b.cubic_to((1.0, 3.0), (1.0, 0.0), (4.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5092-L5103 (chrome/m156)
fn loops61i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((1.0, 5.0), (-6.33333302, 0.666666627), (8.0, -1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to((-6.33333302, 0.666666627), (8.0, -1.0), (0.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5105-L5116 (chrome/m156)
fn loops62i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 2.0));
    path.cubic_to((1.0, 6.0), (-6.33333302, 1.66666663), (8.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 6.0));
    path_b.cubic_to((-6.33333302, 1.66666663), (8.0, 0.0), (0.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5118-L5129 (chrome/m156)
fn loops63i(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 4.0), (-4.0, -0.833333254), (6.0, -3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 4.0));
    path_b.cubic_to((-4.0, -0.833333254), (6.0, -3.0), (0.0, 1.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5131-L5142 (chrome/m156)
fn cubics44d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((3.0, 4.0));
    path.cubic_to((2.0, 5.0), (3.0, 1.0), (6.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 3.0));
    path_b.cubic_to((2.0, 6.0), (4.0, 3.0), (5.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5144-L5155 (chrome/m156)
fn cubics45u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 3.0));
    path.cubic_to((2.0, 6.0), (4.0, 3.0), (5.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((3.0, 4.0));
    path_b.cubic_to((2.0, 5.0), (3.0, 1.0), (6.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5157-L5166 (chrome/m156)
fn fuzz38(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((100.34, 303.312));
    path.line_to((-1e+08, 303.312));
    path.line_to((102.0, 310.156));
    path.line_to((100.34, 310.156));
    path.line_to((100.34, 303.312));
    path.close();
    test_path_op_check(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
        true,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5171-L5197 (chrome/m156)
fn crbug_526025(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x43b40000), f32::from_bits(0xcf000000)));
    path.cubic_to(
        (f32::from_bits(0x4e0d628f), f32::from_bits(0xceffffff)),
        (f32::from_bits(0x4e800003), f32::from_bits(0xcec6b143)),
        (f32::from_bits(0x4e800002), f32::from_bits(0xce7ffffc)),
    );
    path.cubic_to(
        (f32::from_bits(0x4e800002), f32::from_bits(0xcde53aee)),
        (f32::from_bits(0x4e0d6292), f32::from_bits(0xc307820e)),
        (f32::from_bits(0x44627d00), f32::from_bits(0x437ffff2)),
    );
    path.line_to((f32::from_bits(0x444bf3bc), f32::from_bits(0x4460537e)));
    path.line_to((f32::from_bits(0x43553abd), f32::from_bits(0x440f3cbd)));
    path.line_to((f32::from_bits(0x42000000), f32::from_bits(0x41800000)));
    path.line_to((f32::from_bits(0x42c80000), f32::from_bits(0x44000000)));
    path.line_to((f32::from_bits(0x43553abd), f32::from_bits(0x440f3cbd)));
    path.line_to((f32::from_bits(0x43b40000), f32::from_bits(0x44800000)));
    path.line_to((f32::from_bits(0x43b40000), f32::from_bits(0x45816000)));
    let path1 = path.detach();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x42fe0000), f32::from_bits(0x43a08000)));
    path.line_to((f32::from_bits(0x45d5c000), f32::from_bits(0x43870000)));
    path.line_to((f32::from_bits(0xd0a00000), f32::from_bits(0x4cbebc20)));
    path.line_to((f32::from_bits(0x451f7000), f32::from_bits(0x42800000)));
    path.line_to((f32::from_bits(0x42fe0000), f32::from_bits(0x43a08000)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L5199-L5217 (chrome/m156)
fn fuzz_x_392(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x41e80000), f32::from_bits(0x43bde212)));
    path.line_to((f32::from_bits(0x41e80000), f32::from_bits(0x43bdc7ef)));
    path.conic_to(
        (f32::from_bits(0x42a5861e), f32::from_bits(0x43c61f86)),
        (f32::from_bits(0x430b0610), f32::from_bits(0x43c61f86)),
        f32::from_bits(0x3f7d23f3),
    );
    path.conic_to(
        (f32::from_bits(0x42a58e20), f32::from_bits(0x43c61f86)),
        (f32::from_bits(0x41e80000), f32::from_bits(0x43bde212)),
        f32::from_bits(0x3f7d2cf5),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xc36c7bd8), f32::from_bits(0xc3a31d72)));
    path.line_to((f32::from_bits(0xc367a4ae), f32::from_bits(0xc3a31d72)));
    path.line_to((f32::from_bits(0x430b0610), f32::from_bits(0x43c61f86)));
    path.line_to((f32::from_bits(0xc36c7bd8), f32::from_bits(0x43c61f86)));
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L5219-L5238 (chrome/m156)
fn dean2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x3f2b74b3), f32::from_bits(0x4154a02b)));
    path.cubic_to(
        (f32::from_bits(0x3f2b74b3), f32::from_bits(0x4154a02b)),
        (f32::from_bits(0x41531912), f32::from_bits(0x3f130322)),
        (f32::from_bits(0x4154a02b), f32::from_bits(0x3f2b74b3)),
    );
    path.cubic_to(
        (f32::from_bits(0x414a835a), f32::from_bits(0x3ec07ba6)),
        (f32::from_bits(0x413fcc0d), f32::from_bits(0x3e193319)),
        (f32::from_bits(0x4134a02b), f32::from_bits(0x00000000)),
    );
    path.line_to((f32::from_bits(0x3f2b74b3), f32::from_bits(0x4154a02b)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x3f2b74b3), f32::from_bits(0x4154a02b)));
    path.cubic_to(
        (f32::from_bits(0x3f2b74b3), f32::from_bits(0x4154a02b)),
        (f32::from_bits(0x41531912), f32::from_bits(0x3f130322)),
        (f32::from_bits(0x4154a02b), f32::from_bits(0x3f2b74b3)),
    );
    path.line_to((f32::from_bits(0x417ab74b), f32::from_bits(0x4154a02b)));
    path.line_to((f32::from_bits(0x3f2b74b3), f32::from_bits(0x4154a02b)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L5240-L5251 (chrome/m156)
fn cubics_d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((3.0, 5.0), (1.0, 0.0), (3.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 1.0));
    path_b.cubic_to((0.0, 3.0), (1.0, 0.0), (5.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5253-L5264 (chrome/m156)
fn cubics_d2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 5.0), (2.0, 0.0), (2.0, 1.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((1.0, 2.0), (1.0, 0.0), (5.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5266-L5277 (chrome/m156)
fn loops_i1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 3.0));
    path.cubic_to((0.0, 4.0), (-0.333333343, 4.66666651), (3.0, 5.83333349));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 4.0));
    path_b.cubic_to((-0.333333343, 4.66666651), (3.0, 5.83333349), (2.0, 3.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5279-L5290 (chrome/m156)
fn loops_i2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 4.0));
    path.cubic_to((0.0, 5.0), (-0.333333343, 5.66666651), (3.0, 6.83333302));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((-0.333333343, 5.66666651), (3.0, 6.83333302), (2.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5292-L5303 (chrome/m156)
fn loops_i3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((2.0, 5.0));
    path.cubic_to((0.0, 6.0), (-0.333333343, 6.66666651), (3.0, 7.83333302));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 6.0));
    path_b.cubic_to((-0.333333343, 6.66666651), (3.0, 7.83333302), (2.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5305-L5316 (chrome/m156)
fn loops_i4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((3.0, 4.0));
    path.cubic_to((1.0, 5.0), (0.666666627, 5.66666651), (4.0, 6.83333302));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 5.0));
    path_b.cubic_to((0.666666627, 5.66666651), (4.0, 6.83333302), (3.0, 4.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5318-L5329 (chrome/m156)
fn loops_i5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((3.0, 5.0));
    path.cubic_to((1.0, 6.0), (0.666666627, 6.66666651), (4.0, 7.83333302));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 6.0));
    path_b.cubic_to((0.666666627, 6.66666651), (4.0, 7.83333302), (3.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5331-L5342 (chrome/m156)
fn loops_i6(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((4.0, 5.0));
    path.cubic_to((2.0, 6.0), (1.66666663, 6.66666651), (5.0, 7.83333302));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((2.0, 6.0));
    path_b.cubic_to((1.66666663, 6.66666651), (5.0, 7.83333302), (4.0, 5.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5344-L5355 (chrome/m156)
fn cubics_d3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((3.0, 4.0));
    path.cubic_to((0.0, 6.0), (6.0, 1.0), (4.0, 2.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((1.0, 6.0));
    path_b.cubic_to((2.0, 4.0), (4.0, 3.0), (6.0, 0.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5357-L5368 (chrome/m156)
fn cubics_o(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((1.0, 4.0));
    path.cubic_to((2.0, 6.0), (5.0, 0.0), (5.0, 3.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 5.0));
    path_b.cubic_to((3.0, 5.0), (4.0, 1.0), (6.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Xor,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5370-L5381 (chrome/m156)
fn cubic_op158(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 1.0));
    path.cubic_to((2.0, 4.0), (2.0, 0.0), (2.0, 0.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    path_b.move_to((0.0, 2.0));
    path_b.cubic_to((0.0, 2.0), (1.0, 0.0), (4.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5383-L5392 (chrome/m156)
fn loop17(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((1.0, 2.0));
    path.cubic_to((0.0, 3.0), (-0.333333343, 3.33333325), (0.833333373, 3.5));
    path.close();
    path_b.move_to((0.0, 3.0));
    path_b.cubic_to((-0.333333343, 3.33333325), (0.833333373, 3.5), (1.0, 2.0));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5394-L5401 (chrome/m156)
fn circles_op4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_circle((0.0, 1.0), 5.0, PathDirection::CW);
    path_b.set_fill_type(PathFillType::Winding);
    path_b.add_circle((0.0, 1.0), 0.0, PathDirection::CW);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Difference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5403-L5413 (chrome/m156)
fn bug5240(reporter: &mut Reporter, filename: &str) {
    let mut b = PathBuilder::new();
    b.move_to((815.0, 82.0));
    b.cubic_to(
        (814.4794311523438, 82.7868881225586),
        (814.5330810546875, 82.6266555786133),
        (814.5291137695312, 82.6252212524414),
    );
    b.cubic_to(
        (814.5229492187500, 82.6230010986328),
        (814.3790283203125, 83.0008087158203),
        (813.8533935546875, 82.7072601318359),
    );
    b.close();
    let path = b.detach();
    test_path_op(reporter, &path, &path, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L5415-L5430 (chrome/m156)
fn android1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.move_to((f32::from_bits(0xc0a00000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x44866000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x44866000), f32::from_bits(0x43720000)));
    path.line_to((f32::from_bits(0xc0a00000), f32::from_bits(0x43720000)));
    path.line_to((f32::from_bits(0xc0a00000), f32::from_bits(0x00000000)));
    path.close();
    path_b.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path_b.line_to((f32::from_bits(0x44870000), f32::from_bits(0x00000000)));
    path_b.line_to((f32::from_bits(0x44870000), f32::from_bits(0x43720000)));
    path_b.line_to((f32::from_bits(0x00000000), f32::from_bits(0x43720000)));
    path_b.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path_b.close();
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5432-L5724 (chrome/m156)
fn seanbug(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x45b56000), f32::from_bits(0x45bca000)));
    path.line_to((f32::from_bits(0x45b55f0a), f32::from_bits(0x45bc9fc0)));
    path.line_to((f32::from_bits(0x45b55e15), f32::from_bits(0x45bc9f7b)));
    path.line_to((f32::from_bits(0x45b55d1f), f32::from_bits(0x45bc9f32)));
    path.line_to((f32::from_bits(0x45b55c29), f32::from_bits(0x45bc9ee3)));
    path.line_to((f32::from_bits(0x45b55b34), f32::from_bits(0x45bc9e90)));
    path.line_to((f32::from_bits(0x45b55a3f), f32::from_bits(0x45bc9e38)));
    path.line_to((f32::from_bits(0x45b5594a), f32::from_bits(0x45bc9ddc)));
    path.line_to((f32::from_bits(0x45b55856), f32::from_bits(0x45bc9d7a)));
    path.line_to((f32::from_bits(0x45b55762), f32::from_bits(0x45bc9d14)));
    path.line_to((f32::from_bits(0x45b5566f), f32::from_bits(0x45bc9caa)));
    path.line_to((f32::from_bits(0x45b5557c), f32::from_bits(0x45bc9c3b)));
    path.line_to((f32::from_bits(0x45b55489), f32::from_bits(0x45bc9bc7)));
    path.line_to((f32::from_bits(0x45b55397), f32::from_bits(0x45bc9b4f)));
    path.line_to((f32::from_bits(0x45b552a6), f32::from_bits(0x45bc9ad3)));
    path.line_to((f32::from_bits(0x45b551b5), f32::from_bits(0x45bc9a52)));
    path.line_to((f32::from_bits(0x45b550c5), f32::from_bits(0x45bc99cd)));
    path.line_to((f32::from_bits(0x45b54fd6), f32::from_bits(0x45bc9943)));
    path.line_to((f32::from_bits(0x45b54ee8), f32::from_bits(0x45bc98b6)));
    path.line_to((f32::from_bits(0x45b54dfb), f32::from_bits(0x45bc9824)));
    path.line_to((f32::from_bits(0x45b54d0e), f32::from_bits(0x45bc978d)));
    path.line_to((f32::from_bits(0x45b54c23), f32::from_bits(0x45bc96f3)));
    path.line_to((f32::from_bits(0x45b54b39), f32::from_bits(0x45bc9654)));
    path.line_to((f32::from_bits(0x45b54a4f), f32::from_bits(0x45bc95b2)));
    path.line_to((f32::from_bits(0x45b54967), f32::from_bits(0x45bc950b)));
    path.line_to((f32::from_bits(0x45b54880), f32::from_bits(0x45bc9460)));
    path.line_to((f32::from_bits(0x45b5479a), f32::from_bits(0x45bc93b1)));
    path.line_to((f32::from_bits(0x45b546b6), f32::from_bits(0x45bc92fe)));
    path.line_to((f32::from_bits(0x45b545d3), f32::from_bits(0x45bc9248)));
    path.line_to((f32::from_bits(0x45b544f1), f32::from_bits(0x45bc918d)));
    path.line_to((f32::from_bits(0x45b54410), f32::from_bits(0x45bc90cf)));
    path.line_to((f32::from_bits(0x45b54331), f32::from_bits(0x45bc900d)));
    path.line_to((f32::from_bits(0x45b54254), f32::from_bits(0x45bc8f47)));
    path.line_to((f32::from_bits(0x45b54178), f32::from_bits(0x45bc8e7d)));
    path.line_to((f32::from_bits(0x45b5409e), f32::from_bits(0x45bc8db0)));
    path.line_to((f32::from_bits(0x45b53fc6), f32::from_bits(0x45bc8cde)));
    path.line_to((f32::from_bits(0x45b53eef), f32::from_bits(0x45bc8c0a)));
    path.line_to((f32::from_bits(0x45b53e1a), f32::from_bits(0x45bc8b31)));
    path.line_to((f32::from_bits(0x45b53d47), f32::from_bits(0x45bc8a56)));
    path.line_to((f32::from_bits(0x45b53c75), f32::from_bits(0x45bc8976)));
    path.line_to((f32::from_bits(0x45b53ba6), f32::from_bits(0x45bc8893)));
    path.line_to((f32::from_bits(0x45b53ad8), f32::from_bits(0x45bc87ad)));
    path.line_to((f32::from_bits(0x45b53a0d), f32::from_bits(0x45bc86c4)));
    path.line_to((f32::from_bits(0x45b53944), f32::from_bits(0x45bc85d6)));
    path.line_to((f32::from_bits(0x45b5387c), f32::from_bits(0x45bc84e6)));
    path.line_to((f32::from_bits(0x45b537b7), f32::from_bits(0x45bc83f2)));
    path.line_to((f32::from_bits(0x45b536f4), f32::from_bits(0x45bc82fc)));
    path.line_to((f32::from_bits(0x45b53634), f32::from_bits(0x45bc8201)));
    path.line_to((f32::from_bits(0x45b53575), f32::from_bits(0x45bc8104)));
    path.line_to((f32::from_bits(0x45b534ba), f32::from_bits(0x45bc8004)));
    path.line_to((f32::from_bits(0x45b53400), f32::from_bits(0x45bc7f00)));
    path.line_to((f32::from_bits(0x45b53349), f32::from_bits(0x45bc7df9)));
    path.line_to((f32::from_bits(0x45b53294), f32::from_bits(0x45bc7cf0)));
    path.line_to((f32::from_bits(0x45b531e2), f32::from_bits(0x45bc7be3)));
    path.line_to((f32::from_bits(0x45b53133), f32::from_bits(0x45bc7ad3)));
    path.line_to((f32::from_bits(0x45b53086), f32::from_bits(0x45bc79c1)));
    path.line_to((f32::from_bits(0x45b52fdc), f32::from_bits(0x45bc78ab)));
    path.line_to((f32::from_bits(0x45b52f35), f32::from_bits(0x45bc7793)));
    path.line_to((f32::from_bits(0x45b52e90), f32::from_bits(0x45bc7678)));
    path.line_to((f32::from_bits(0x45b52def), f32::from_bits(0x45bc755a)));
    path.line_to((f32::from_bits(0x45b52d50), f32::from_bits(0x45bc7439)));
    path.line_to((f32::from_bits(0x45b52cb4), f32::from_bits(0x45bc7316)));
    path.line_to((f32::from_bits(0x45b52c1b), f32::from_bits(0x45bc71f0)));
    path.line_to((f32::from_bits(0x45b52b86), f32::from_bits(0x45bc70c7)));
    path.line_to((f32::from_bits(0x45b52af3), f32::from_bits(0x45bc6f9c)));
    path.line_to((f32::from_bits(0x45b52a63), f32::from_bits(0x45bc6e6e)));
    path.line_to((f32::from_bits(0x45b529d7), f32::from_bits(0x45bc6d3e)));
    path.line_to((f32::from_bits(0x45b5294e), f32::from_bits(0x45bc6c0b)));
    path.line_to((f32::from_bits(0x45b528c8), f32::from_bits(0x45bc6ad6)));
    path.line_to((f32::from_bits(0x45b52846), f32::from_bits(0x45bc699e)));
    path.line_to((f32::from_bits(0x45b527c7), f32::from_bits(0x45bc6864)));
    path.line_to((f32::from_bits(0x45b5274b), f32::from_bits(0x45bc6728)));
    path.line_to((f32::from_bits(0x45b526d3), f32::from_bits(0x45bc65e9)));
    path.line_to((f32::from_bits(0x45b5265e), f32::from_bits(0x45bc64a8)));
    path.line_to((f32::from_bits(0x45b52600), f32::from_bits(0x45bc639b)));
    path.line_to((f32::from_bits(0x45b52600), f32::from_bits(0x45bab032)));
    path.line_to((f32::from_bits(0x45b52611), f32::from_bits(0x45baaffd)));
    path.line_to((f32::from_bits(0x45b52687), f32::from_bits(0x45baae9d)));
    path.line_to((f32::from_bits(0x45b52700), f32::from_bits(0x45baad40)));
    path.line_to((f32::from_bits(0x45b5277d), f32::from_bits(0x45baabe7)));
    path.line_to((f32::from_bits(0x45b527fe), f32::from_bits(0x45baaa91)));
    path.line_to((f32::from_bits(0x45b52883), f32::from_bits(0x45baa93f)));
    path.line_to((f32::from_bits(0x45b5290b), f32::from_bits(0x45baa7f1)));
    path.line_to((f32::from_bits(0x45b52998), f32::from_bits(0x45baa6a6)));
    path.line_to((f32::from_bits(0x45b52a28), f32::from_bits(0x45baa55f)));
    path.line_to((f32::from_bits(0x45b52abb), f32::from_bits(0x45baa41c)));
    path.line_to((f32::from_bits(0x45b52b52), f32::from_bits(0x45baa2dc)));
    path.line_to((f32::from_bits(0x45b52bed), f32::from_bits(0x45baa1a0)));
    path.line_to((f32::from_bits(0x45b52c8c), f32::from_bits(0x45baa068)));
    path.line_to((f32::from_bits(0x45b52d2e), f32::from_bits(0x45ba9f34)));
    path.line_to((f32::from_bits(0x45b52dd3), f32::from_bits(0x45ba9e04)));
    path.line_to((f32::from_bits(0x45b52e7c), f32::from_bits(0x45ba9cd8)));
    path.line_to((f32::from_bits(0x45b52f28), f32::from_bits(0x45ba9baf)));
    path.line_to((f32::from_bits(0x45b52fd8), f32::from_bits(0x45ba9a8b)));
    path.line_to((f32::from_bits(0x45b5308b), f32::from_bits(0x45ba996b)));
    path.line_to((f32::from_bits(0x45b53141), f32::from_bits(0x45ba984f)));
    path.line_to((f32::from_bits(0x45b531fa), f32::from_bits(0x45ba9736)));
    path.line_to((f32::from_bits(0x45b532b7), f32::from_bits(0x45ba9623)));
    path.line_to((f32::from_bits(0x45b53377), f32::from_bits(0x45ba9513)));
    path.line_to((f32::from_bits(0x45b5343a), f32::from_bits(0x45ba9407)));
    path.line_to((f32::from_bits(0x45b53500), f32::from_bits(0x45ba9300)));
    path.line_to((f32::from_bits(0x45b535c9), f32::from_bits(0x45ba91fd)));
    path.line_to((f32::from_bits(0x45b53695), f32::from_bits(0x45ba90fe)));
    path.line_to((f32::from_bits(0x45b53765), f32::from_bits(0x45ba9004)));
    path.line_to((f32::from_bits(0x45b53837), f32::from_bits(0x45ba8f0e)));
    path.line_to((f32::from_bits(0x45b5390c), f32::from_bits(0x45ba8e1d)));
    path.line_to((f32::from_bits(0x45b539e4), f32::from_bits(0x45ba8d30)));
    path.line_to((f32::from_bits(0x45b53abf), f32::from_bits(0x45ba8c48)));
    path.line_to((f32::from_bits(0x45b53b9d), f32::from_bits(0x45ba8b64)));
    path.line_to((f32::from_bits(0x45b53c7d), f32::from_bits(0x45ba8a85)));
    path.line_to((f32::from_bits(0x45b53d60), f32::from_bits(0x45ba89aa)));
    path.line_to((f32::from_bits(0x45b53e46), f32::from_bits(0x45ba88d4)));
    path.line_to((f32::from_bits(0x45b53f2f), f32::from_bits(0x45ba8803)));
    path.line_to((f32::from_bits(0x45b5401a), f32::from_bits(0x45ba8736)));
    path.line_to((f32::from_bits(0x45b54108), f32::from_bits(0x45ba866f)));
    path.line_to((f32::from_bits(0x45b541f8), f32::from_bits(0x45ba85ac)));
    path.line_to((f32::from_bits(0x45b542eb), f32::from_bits(0x45ba84ee)));
    path.line_to((f32::from_bits(0x45b543e0), f32::from_bits(0x45ba8435)));
    path.line_to((f32::from_bits(0x45b544d8), f32::from_bits(0x45ba8380)));
    path.line_to((f32::from_bits(0x45b545d2), f32::from_bits(0x45ba82d1)));
    path.line_to((f32::from_bits(0x45b546cf), f32::from_bits(0x45ba8227)));
    path.line_to((f32::from_bits(0x45b547ce), f32::from_bits(0x45ba8182)));
    path.line_to((f32::from_bits(0x45b548cf), f32::from_bits(0x45ba80e2)));
    path.line_to((f32::from_bits(0x45b549d2), f32::from_bits(0x45ba8047)));
    path.line_to((f32::from_bits(0x45b54ad8), f32::from_bits(0x45ba7fb1)));
    path.line_to((f32::from_bits(0x45b54be0), f32::from_bits(0x45ba7f20)));
    path.line_to((f32::from_bits(0x45b54cea), f32::from_bits(0x45ba7e95)));
    path.line_to((f32::from_bits(0x45b54df6), f32::from_bits(0x45ba7e0e)));
    path.line_to((f32::from_bits(0x45b54f04), f32::from_bits(0x45ba7d8d)));
    path.line_to((f32::from_bits(0x45b55015), f32::from_bits(0x45ba7d12)));
    path.line_to((f32::from_bits(0x45b55127), f32::from_bits(0x45ba7c9c)));
    path.line_to((f32::from_bits(0x45b551b5), f32::from_bits(0x45ba7c62)));
    path.line_to((f32::from_bits(0x45c7b29a), f32::from_bits(0x45ba7c62)));
    path.line_to((f32::from_bits(0x45c7b2f2), f32::from_bits(0x45ba7c8b)));
    path.line_to((f32::from_bits(0x45c7b3dd), f32::from_bits(0x45ba7cff)));
    path.line_to((f32::from_bits(0x45c7b4c7), f32::from_bits(0x45ba7d78)));
    path.line_to((f32::from_bits(0x45c7b5b1), f32::from_bits(0x45ba7df5)));
    path.line_to((f32::from_bits(0x45c7b699), f32::from_bits(0x45ba7e78)));
    path.line_to((f32::from_bits(0x45c7b780), f32::from_bits(0x45ba7f00)));
    path.line_to((f32::from_bits(0x45c7b866), f32::from_bits(0x45ba7f8d)));
    path.line_to((f32::from_bits(0x45c7b94a), f32::from_bits(0x45ba801e)));
    path.line_to((f32::from_bits(0x45c7ba2d), f32::from_bits(0x45ba80b5)));
    path.line_to((f32::from_bits(0x45c7bb0f), f32::from_bits(0x45ba8150)));
    path.line_to((f32::from_bits(0x45c7bbf0), f32::from_bits(0x45ba81f0)));
    path.line_to((f32::from_bits(0x45c7bccf), f32::from_bits(0x45ba8294)));
    path.line_to((f32::from_bits(0x45c7bdac), f32::from_bits(0x45ba833d)));
    path.line_to((f32::from_bits(0x45c7be88), f32::from_bits(0x45ba83eb)));
    path.line_to((f32::from_bits(0x45c7bf62), f32::from_bits(0x45ba849d)));
    path.line_to((f32::from_bits(0x45c7c03a), f32::from_bits(0x45ba8554)));
    path.line_to((f32::from_bits(0x45c7c111), f32::from_bits(0x45ba860f)));
    path.line_to((f32::from_bits(0x45c7c1e6), f32::from_bits(0x45ba86cf)));
    path.line_to((f32::from_bits(0x45c7c2b9), f32::from_bits(0x45ba8792)));
    path.line_to((f32::from_bits(0x45c7c38b), f32::from_bits(0x45ba885b)));
    path.line_to((f32::from_bits(0x45c7c45a), f32::from_bits(0x45ba8927)));
    path.line_to((f32::from_bits(0x45c7c528), f32::from_bits(0x45ba89f7)));
    path.line_to((f32::from_bits(0x45c7c5f3), f32::from_bits(0x45ba8acc)));
    path.line_to((f32::from_bits(0x45c7c6bc), f32::from_bits(0x45ba8ba5)));
    path.line_to((f32::from_bits(0x45c7c784), f32::from_bits(0x45ba8c82)));
    path.line_to((f32::from_bits(0x45c7c849), f32::from_bits(0x45ba8d62)));
    path.line_to((f32::from_bits(0x45c7c90c), f32::from_bits(0x45ba8e47)));
    path.line_to((f32::from_bits(0x45c7c9cc), f32::from_bits(0x45ba8f30)));
    path.line_to((f32::from_bits(0x45c7ca8b), f32::from_bits(0x45ba901c)));
    path.line_to((f32::from_bits(0x45c7cb46), f32::from_bits(0x45ba910c)));
    path.line_to((f32::from_bits(0x45c7cc00), f32::from_bits(0x45ba9200)));
    path.line_to((f32::from_bits(0x45c7ccb7), f32::from_bits(0x45ba92f8)));
    path.line_to((f32::from_bits(0x45c7cd6c), f32::from_bits(0x45ba93f3)));
    path.line_to((f32::from_bits(0x45c7ce1e), f32::from_bits(0x45ba94f2)));
    path.line_to((f32::from_bits(0x45c7cecd), f32::from_bits(0x45ba95f4)));
    path.line_to((f32::from_bits(0x45c7cf7a), f32::from_bits(0x45ba96fa)));
    path.line_to((f32::from_bits(0x45c7d024), f32::from_bits(0x45ba9803)));
    path.line_to((f32::from_bits(0x45c7d0cb), f32::from_bits(0x45ba9910)));
    path.line_to((f32::from_bits(0x45c7d170), f32::from_bits(0x45ba9a20)));
    path.line_to((f32::from_bits(0x45c7d211), f32::from_bits(0x45ba9b33)));
    path.line_to((f32::from_bits(0x45c7d2b0), f32::from_bits(0x45ba9c4a)));
    path.line_to((f32::from_bits(0x45c7d34c), f32::from_bits(0x45ba9d63)));
    path.line_to((f32::from_bits(0x45c7d3e5), f32::from_bits(0x45ba9e80)));
    path.line_to((f32::from_bits(0x45c7d47a), f32::from_bits(0x45ba9fa0)));
    path.line_to((f32::from_bits(0x45c7d50d), f32::from_bits(0x45baa0c3)));
    path.line_to((f32::from_bits(0x45c7d59d), f32::from_bits(0x45baa1e9)));
    path.line_to((f32::from_bits(0x45c7d629), f32::from_bits(0x45baa312)));
    path.line_to((f32::from_bits(0x45c7d6b2), f32::from_bits(0x45baa43e)));
    path.line_to((f32::from_bits(0x45c7d738), f32::from_bits(0x45baa56d)));
    path.line_to((f32::from_bits(0x45c7d7ba), f32::from_bits(0x45baa69f)));
    path.line_to((f32::from_bits(0x45c7d839), f32::from_bits(0x45baa7d3)));
    path.line_to((f32::from_bits(0x45c7d8b5), f32::from_bits(0x45baa90a)));
    path.line_to((f32::from_bits(0x45c7d92d), f32::from_bits(0x45baaa44)));
    path.line_to((f32::from_bits(0x45c7d9a2), f32::from_bits(0x45baab80)));
    path.line_to((f32::from_bits(0x45c7da13), f32::from_bits(0x45baacbf)));
    path.line_to((f32::from_bits(0x45c7da80), f32::from_bits(0x45baae00)));
    path.line_to((f32::from_bits(0x45c7daea), f32::from_bits(0x45baaf44)));
    path.line_to((f32::from_bits(0x45c7db50), f32::from_bits(0x45bab08a)));
    path.line_to((f32::from_bits(0x45c7dbb2), f32::from_bits(0x45bab1d3)));
    path.line_to((f32::from_bits(0x45c7dc10), f32::from_bits(0x45bab31d)));
    path.line_to((f32::from_bits(0x45c7dc6a), f32::from_bits(0x45bab46a)));
    path.line_to((f32::from_bits(0x45c7dc6b), f32::from_bits(0x45bc5fbe)));
    path.line_to((f32::from_bits(0x45c7dc10), f32::from_bits(0x45bc60e7)));
    path.line_to((f32::from_bits(0x45c7dbb2), f32::from_bits(0x45bc620f)));
    path.line_to((f32::from_bits(0x45c7db50), f32::from_bits(0x45bc6336)));
    path.line_to((f32::from_bits(0x45c7daea), f32::from_bits(0x45bc645c)));
    path.line_to((f32::from_bits(0x45c7da80), f32::from_bits(0x45bc6580)));
    path.line_to((f32::from_bits(0x45c7da13), f32::from_bits(0x45bc66a3)));
    path.line_to((f32::from_bits(0x45c7d9a2), f32::from_bits(0x45bc67c5)));
    path.line_to((f32::from_bits(0x45c7d92d), f32::from_bits(0x45bc68e6)));
    path.line_to((f32::from_bits(0x45c7d8b5), f32::from_bits(0x45bc6a05)));
    path.line_to((f32::from_bits(0x45c7d839), f32::from_bits(0x45bc6b23)));
    path.line_to((f32::from_bits(0x45c7d7ba), f32::from_bits(0x45bc6c3f)));
    path.line_to((f32::from_bits(0x45c7d738), f32::from_bits(0x45bc6d5a)));
    path.line_to((f32::from_bits(0x45c7d6b2), f32::from_bits(0x45bc6e73)));
    path.line_to((f32::from_bits(0x45c7d629), f32::from_bits(0x45bc6f8b)));
    path.line_to((f32::from_bits(0x45c7d59d), f32::from_bits(0x45bc70a1)));
    path.line_to((f32::from_bits(0x45c7d50d), f32::from_bits(0x45bc71b5)));
    path.line_to((f32::from_bits(0x45c7d47a), f32::from_bits(0x45bc72c7)));
    path.line_to((f32::from_bits(0x45c7d3e5), f32::from_bits(0x45bc73d8)));
    path.line_to((f32::from_bits(0x45c7d34c), f32::from_bits(0x45bc74e7)));
    path.line_to((f32::from_bits(0x45c7d2b0), f32::from_bits(0x45bc75f4)));
    path.line_to((f32::from_bits(0x45c7d211), f32::from_bits(0x45bc76ff)));
    path.line_to((f32::from_bits(0x45c7d170), f32::from_bits(0x45bc7807)));
    path.line_to((f32::from_bits(0x45c7d0cb), f32::from_bits(0x45bc790e)));
    path.line_to((f32::from_bits(0x45c7d024), f32::from_bits(0x45bc7a13)));
    path.line_to((f32::from_bits(0x45c7cf7a), f32::from_bits(0x45bc7b16)));
    path.line_to((f32::from_bits(0x45c7cecd), f32::from_bits(0x45bc7c16)));
    path.line_to((f32::from_bits(0x45c7ce1e), f32::from_bits(0x45bc7d14)));
    path.line_to((f32::from_bits(0x45c7cd6c), f32::from_bits(0x45bc7e10)));
    path.line_to((f32::from_bits(0x45c7ccb7), f32::from_bits(0x45bc7f09)));
    path.line_to((f32::from_bits(0x45c7cc00), f32::from_bits(0x45bc8000)));
    path.line_to((f32::from_bits(0x45c7cb46), f32::from_bits(0x45bc80f5)));
    path.line_to((f32::from_bits(0x45c7ca8b), f32::from_bits(0x45bc81e7)));
    path.line_to((f32::from_bits(0x45c7c9cc), f32::from_bits(0x45bc82d6)));
    path.line_to((f32::from_bits(0x45c7c90c), f32::from_bits(0x45bc83c3)));
    path.line_to((f32::from_bits(0x45c7c849), f32::from_bits(0x45bc84ad)));
    path.line_to((f32::from_bits(0x45c7c784), f32::from_bits(0x45bc8595)));
    path.line_to((f32::from_bits(0x45c7c6bc), f32::from_bits(0x45bc8679)));
    path.line_to((f32::from_bits(0x45c7c5f3), f32::from_bits(0x45bc875b)));
    path.line_to((f32::from_bits(0x45c7c528), f32::from_bits(0x45bc883a)));
    path.line_to((f32::from_bits(0x45c7c45a), f32::from_bits(0x45bc8917)));
    path.line_to((f32::from_bits(0x45c7c38b), f32::from_bits(0x45bc89f0)));
    path.line_to((f32::from_bits(0x45c7c2b9), f32::from_bits(0x45bc8ac6)));
    path.line_to((f32::from_bits(0x45c7c1e6), f32::from_bits(0x45bc8b99)));
    path.line_to((f32::from_bits(0x45c7c111), f32::from_bits(0x45bc8c69)));
    path.line_to((f32::from_bits(0x45c7c03a), f32::from_bits(0x45bc8d36)));
    path.line_to((f32::from_bits(0x45c7bf62), f32::from_bits(0x45bc8e00)));
    path.line_to((f32::from_bits(0x45c7be88), f32::from_bits(0x45bc8ec7)));
    path.line_to((f32::from_bits(0x45c7bdac), f32::from_bits(0x45bc8f8a)));
    path.line_to((f32::from_bits(0x45c7bccf), f32::from_bits(0x45bc904a)));
    path.line_to((f32::from_bits(0x45c7bbf0), f32::from_bits(0x45bc9106)));
    path.line_to((f32::from_bits(0x45c7bb0f), f32::from_bits(0x45bc91bf)));
    path.line_to((f32::from_bits(0x45c7ba2d), f32::from_bits(0x45bc9275)));
    path.line_to((f32::from_bits(0x45c7b94a), f32::from_bits(0x45bc9327)));
    path.line_to((f32::from_bits(0x45c7b866), f32::from_bits(0x45bc93d5)));
    path.line_to((f32::from_bits(0x45c7b780), f32::from_bits(0x45bc9480)));
    path.line_to((f32::from_bits(0x45c7b699), f32::from_bits(0x45bc9527)));
    path.line_to((f32::from_bits(0x45c7b5b1), f32::from_bits(0x45bc95ca)));
    path.line_to((f32::from_bits(0x45c7b4c8), f32::from_bits(0x45bc966a)));
    path.line_to((f32::from_bits(0x45c7b3dd), f32::from_bits(0x45bc9706)));
    path.line_to((f32::from_bits(0x45c7b2f2), f32::from_bits(0x45bc979e)));
    path.line_to((f32::from_bits(0x45c7b205), f32::from_bits(0x45bc9832)));
    path.line_to((f32::from_bits(0x45c7b118), f32::from_bits(0x45bc98c2)));
    path.line_to((f32::from_bits(0x45c7b02a), f32::from_bits(0x45bc994e)));
    path.line_to((f32::from_bits(0x45c7af3b), f32::from_bits(0x45bc99d5)));
    path.line_to((f32::from_bits(0x45c7ae4b), f32::from_bits(0x45bc9a59)));
    path.line_to((f32::from_bits(0x45c7ad5a), f32::from_bits(0x45bc9ad9)));
    path.line_to((f32::from_bits(0x45c7ac69), f32::from_bits(0x45bc9b54)));
    path.line_to((f32::from_bits(0x45c7ab77), f32::from_bits(0x45bc9bcb)));
    path.line_to((f32::from_bits(0x45c7aa84), f32::from_bits(0x45bc9c3e)));
    path.line_to((f32::from_bits(0x45c7a991), f32::from_bits(0x45bc9cac)));
    path.line_to((f32::from_bits(0x45c7a89e), f32::from_bits(0x45bc9d16)));
    path.line_to((f32::from_bits(0x45c7a7aa), f32::from_bits(0x45bc9d7b)));
    path.line_to((f32::from_bits(0x45c7a6b6), f32::from_bits(0x45bc9ddc)));
    path.line_to((f32::from_bits(0x45c7a5c1), f32::from_bits(0x45bc9e39)));
    path.line_to((f32::from_bits(0x45c7a4cc), f32::from_bits(0x45bc9e90)));
    path.line_to((f32::from_bits(0x45c7a3d7), f32::from_bits(0x45bc9ee3)));
    path.line_to((f32::from_bits(0x45c7a2e1), f32::from_bits(0x45bc9f32)));
    path.line_to((f32::from_bits(0x45c7a1eb), f32::from_bits(0x45bc9f7b)));
    path.line_to((f32::from_bits(0x45c7a0f6), f32::from_bits(0x45bc9fc0)));
    path.line_to((f32::from_bits(0x45c7a000), f32::from_bits(0x45bca000)));
    path.line_to((f32::from_bits(0x45b56000), f32::from_bits(0x45bca000)));
    path.close();
    let mut path2 = PathBuilder::new();
    path2.set_fill_type(PathFillType::Winding);
    path2.move_to((f32::from_bits(0x45b52600), f32::from_bits(0x45ba7c62)));
    path2.line_to((f32::from_bits(0x45c7dc6b), f32::from_bits(0x45ba7c62)));
    path2.line_to((f32::from_bits(0x45c7dc6b), f32::from_bits(0x45bca239)));
    path2.line_to((f32::from_bits(0x45b52600), f32::from_bits(0x45bca239)));
    path2.line_to((f32::from_bits(0x45b52600), f32::from_bits(0x45ba7c62)));
    path2.close();
    // `result_path` is declared but never used in Skia.
    let _result_path = Path::new();
    test_path_op(
        reporter,
        &path.detach(),
        &path2.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5726-L5732 (chrome/m156)
fn halbug(reporter: &mut Reporter, filename: &str) {
    let path = Path::rect_with_fill_type(
        Rect::new(278.653992, 155.747406, 580.15918, 593.602051),
        PathFillType::EvenOdd,
        PathDirection::CW,
    );
    let path2 = Path::rect_with_fill_type(
        Rect::new(278.657715, 155.747314, 580.238281, 594.114014),
        PathFillType::Winding,
        PathDirection::CW,
    );
    test_path_op(reporter, &path, &path2, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L5734-L5754 (chrome/m156)
fn test_rect1_u(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    let mut path_b = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 60.0));
    path.line_to((60.0, 60.0));
    path.line_to((60.0, 0.0));
    path.close();
    path.move_to((30.0, 20.0));
    path.line_to((30.0, 50.0));
    path.line_to((50.0, 50.0));
    path.line_to((50.0, 20.0));
    path.close();
    path.move_to((24.0, 20.0));
    path.line_to((24.0, 30.0));
    path.line_to((36.0, 30.0));
    path.line_to((36.0, 20.0));
    path.close();
    path_b.set_fill_type(PathFillType::Winding);
    test_path_op(
        reporter,
        &path.detach(),
        &path_b.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L5756-L5799 (chrome/m156)
fn filinmangust14(reporter: &mut Reporter, filename: &str) {
    let mut builder = PathBuilder::new_with_fill_type(PathFillType::Winding);
    builder.move_to((f32::from_bits(0x440bc02c), f32::from_bits(0x4409c000)));
    builder.line_to((f32::from_bits(0x440bc02c), f32::from_bits(0x440e8000)));
    builder.line_to((f32::from_bits(0x440bbfda), f32::from_bits(0x440e8000)));
    builder.line_to((f32::from_bits(0x440bbfda), f32::from_bits(0x4409c000)));
    builder.line_to((f32::from_bits(0x440bc02c), f32::from_bits(0x4409c000)));
    builder.close();
    let path1 = builder.detach();
    builder.set_fill_type(PathFillType::Winding);
    builder.move_to((f32::from_bits(0x45582000), f32::from_bits(0x45be9805)));
    builder.line_to((f32::from_bits(0x4554b667), f32::from_bits(0x45be9805)));
    builder.line_to((f32::from_bits(0x4554b667), f32::from_bits(0x45be97fb)));
    builder.line_to((f32::from_bits(0x45582000), f32::from_bits(0x45be97fb)));
    builder.line_to((f32::from_bits(0x45582000), f32::from_bits(0x45be9805)));
    builder.close();
    builder.move_to((f32::from_bits(0x43b60000), f32::from_bits(0x443dffd7)));
    builder.line_to((f32::from_bits(0x4554b667), f32::from_bits(0x443dffd7)));
    builder.line_to((f32::from_bits(0x4554b667), f32::from_bits(0x443e0029)));
    builder.line_to((f32::from_bits(0x43b60000), f32::from_bits(0x443e0029)));
    builder.line_to((f32::from_bits(0x43b60000), f32::from_bits(0x443dffd7)));
    builder.close();
    builder.move_to((f32::from_bits(0x4554b65d), f32::from_bits(0x45be9800)));
    builder.line_to((f32::from_bits(0x4554b65d), f32::from_bits(0x443e0000)));
    builder.line_to((f32::from_bits(0x4554b671), f32::from_bits(0x443e0000)));
    builder.line_to((f32::from_bits(0x4554b671), f32::from_bits(0x45be9800)));
    builder.line_to((f32::from_bits(0x4554b65d), f32::from_bits(0x45be9800)));
    builder.close();
    builder.move_to((f32::from_bits(0x449f4000), f32::from_bits(0x43bdffae)));
    builder.line_to((f32::from_bits(0x4554b667), f32::from_bits(0x43bdffae)));
    builder.line_to((f32::from_bits(0x4554b667), f32::from_bits(0x43be0052)));
    builder.line_to((f32::from_bits(0x449f4000), f32::from_bits(0x43be0052)));
    builder.line_to((f32::from_bits(0x449f4000), f32::from_bits(0x43bdffae)));
    builder.close();
    builder.move_to((f32::from_bits(0x4554b65d), f32::from_bits(0x443e0000)));
    builder.line_to((f32::from_bits(0x4554b65d), f32::from_bits(0x43be0000)));
    builder.line_to((f32::from_bits(0x4554b671), f32::from_bits(0x43be0000)));
    builder.line_to((f32::from_bits(0x4554b671), f32::from_bits(0x443e0000)));
    builder.line_to((f32::from_bits(0x4554b65d), f32::from_bits(0x443e0000)));
    builder.close();
    test_path_op(reporter, &path1, &builder.detach(), PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L5801-L8946 (chrome/m156)
fn grshapearcs1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((25.0098, 23.1973));
    path.line_to((25.5689, 22.3682));
    path.conic_to((26.1281, 21.5392), (26.9572, 22.0984), 0.707107);
    path.conic_to((27.7862, 22.6576), (27.227, 23.4866), 0.707107);
    path.line_to((26.6678, 24.3156));
    path.conic_to((26.1086, 25.1447), (25.2796, 24.5855), 0.707107);
    path.conic_to((24.4506, 24.0263), (25.0098, 23.1973), 0.707107);
    path.close();
    path.move_to((26.6873, 20.7101));
    path.line_to((27.2465, 19.8811));
    path.conic_to((27.8057, 19.0521), (28.6348, 19.6113), 0.707107);
    path.conic_to((29.4638, 20.1704), (28.9046, 20.9995), 0.707107);
    path.line_to((28.3454, 21.8285));
    path.conic_to((27.7862, 22.6576), (26.9572, 22.0984), 0.707107);
    path.conic_to((26.1281, 21.5392), (26.6873, 20.7101), 0.707107);
    path.close();
    path.move_to((28.3649, 18.223));
    path.line_to((28.9241, 17.394));
    path.conic_to((29.4833, 16.565), (30.3123, 17.1241), 0.707107);
    path.conic_to((31.1414, 17.6833), (30.5822, 18.5124), 0.707107);
    path.line_to((30.023, 19.3414));
    path.conic_to((29.4638, 20.1704), (28.6348, 19.6113), 0.707107);
    path.conic_to((27.8057, 19.0521), (28.3649, 18.223), 0.707107);
    path.close();
    path.move_to((30.0425, 15.7359));
    path.line_to((30.6017, 14.9069));
    path.conic_to((31.1609, 14.0778), (31.9899, 14.637), 0.707107);
    path.conic_to((32.8189, 15.1962), (32.2598, 16.0253), 0.707107);
    path.line_to((31.7006, 16.8543));
    path.conic_to((31.1414, 17.6833), (30.3123, 17.1241), 0.707107);
    path.conic_to((29.4833, 16.565), (30.0425, 15.7359), 0.707107);
    path.close();
    path.move_to((31.7201, 13.2488));
    path.line_to((32.2793, 12.4198));
    path.conic_to((32.8385, 11.5907), (33.6675, 12.1499), 0.707107);
    path.conic_to((34.4965, 12.7091), (33.9373, 13.5381), 0.707107);
    path.line_to((33.3781, 14.3672));
    path.conic_to((32.8189, 15.1962), (31.9899, 14.637), 0.707107);
    path.conic_to((31.1609, 14.0778), (31.7201, 13.2488), 0.707107);
    path.close();
    path.move_to((33.3976, 10.7617));
    path.line_to((33.9568, 9.93265));
    path.conic_to((34.516, 9.10361), (35.3451, 9.6628), 0.707107);
    path.conic_to((36.1741, 10.222), (35.6149, 11.051), 0.707107);
    path.line_to((35.0557, 11.8801));
    path.conic_to((34.4965, 12.7091), (33.6675, 12.1499), 0.707107);
    path.conic_to((32.8385, 11.5907), (33.3976, 10.7617), 0.707107);
    path.close();
    path.move_to((35.0752, 8.27457));
    path.line_to((35.6344, 7.44554));
    path.conic_to((36.1936, 6.6165), (37.0226, 7.17569), 0.707107);
    path.conic_to((37.8517, 7.73488), (37.2925, 8.56392), 0.707107);
    path.line_to((36.7333, 9.39296));
    path.conic_to((36.1741, 10.222), (35.3451, 9.6628), 0.707107);
    path.conic_to((34.516, 9.10361), (35.0752, 8.27457), 0.707107);
    path.close();
    path.move_to((36.7528, 5.78746));
    path.line_to((37.312, 4.95842));
    path.conic_to((37.8712, 4.12939), (38.7002, 4.68858), 0.707107);
    path.conic_to((39.5293, 5.24777), (38.9701, 6.07681), 0.707107);
    path.line_to((38.4109, 6.90585));
    path.conic_to((37.8517, 7.73488), (37.0226, 7.17569), 0.707107);
    path.conic_to((36.1936, 6.6165), (36.7528, 5.78746), 0.707107);
    path.close();
    path.move_to((39.9447, 3.72429));
    path.quad_to((40.3524, 4.01069), (40.7489, 4.31248));
    path.conic_to((41.5445, 4.9182), (40.9388, 5.71387), 0.707107);
    path.conic_to((40.3331, 6.50955), (39.5374, 5.90383), 0.707107);
    path.quad_to((39.1714, 5.62521), (38.7951, 5.36088));
    path.conic_to((37.9768, 4.78608), (38.5516, 3.96779), 0.707107);
    path.conic_to((39.1264, 3.14949), (39.9447, 3.72429), 0.707107);
    path.close();
    path.move_to((42.3194, 5.60826));
    path.quad_to((42.707, 5.95446), (43.0804, 6.31583));
    path.conic_to((43.7991, 7.01122), (43.1037, 7.72985), 0.707107);
    path.conic_to((42.4083, 8.44848), (41.6896, 7.75308), 0.707107);
    path.quad_to((41.3448, 7.41944), (40.9871, 7.09992));
    path.conic_to((40.2413, 6.43379), (40.9074, 5.68796), 0.707107);
    path.conic_to((41.5735, 4.94212), (42.3194, 5.60826), 0.707107);
    path.close();
    path.move_to((44.5406, 7.84871));
    path.quad_to((44.8959, 8.25352), (45.2341, 8.67266));
    path.conic_to((45.862, 9.4509), (45.0838, 10.0789), 0.707107);
    path.conic_to((44.3056, 10.7068), (43.6776, 9.9286), 0.707107);
    path.quad_to((43.3654, 9.54174), (43.0374, 9.16805));
    path.conic_to((42.3778, 8.41649), (43.1293, 7.75682), 0.707107);
    path.conic_to((43.8809, 7.09715), (44.5406, 7.84871), 0.707107);
    path.close();
    path.move_to((46.528, 10.4211));
    path.quad_to((46.815, 10.8449), (47.0851, 11.2796));
    path.conic_to((47.6128, 12.129), (46.7633, 12.6567), 0.707107);
    path.conic_to((45.9139, 13.1844), (45.3862, 12.335), 0.707107);
    path.quad_to((45.1369, 11.9337), (44.872, 11.5426));
    path.conic_to((44.3113, 10.7146), (45.1393, 10.1538), 0.707107);
    path.conic_to((45.9673, 9.5931), (46.528, 10.4211), 0.707107);
    path.close();
    path.move_to((48.1056, 13.0782));
    path.quad_to((48.3449, 13.542), (48.5654, 14.015));
    path.conic_to((48.9879, 14.9213), (48.0816, 15.3438), 0.707107);
    path.conic_to((47.1752, 15.7663), (46.7527, 14.86), 0.707107);
    path.quad_to((46.5492, 14.4234), (46.3283, 13.9953));
    path.conic_to((45.8698, 13.1066), (46.7584, 12.6481), 0.707107);
    path.conic_to((47.6471, 12.1895), (48.1056, 13.0782), 0.707107);
    path.close();
    path.move_to((49.3755, 15.9538));
    path.quad_to((49.5594, 16.4493), (49.7229, 16.9516));
    path.conic_to((50.0325, 17.9025), (49.0816, 18.2121), 0.707107);
    path.conic_to((48.1307, 18.5216), (47.8212, 17.5707), 0.707107);
    path.quad_to((47.6702, 17.1069), (47.5005, 16.6497));
    path.conic_to((47.1526, 15.7122), (48.0901, 15.3642), 0.707107);
    path.conic_to((49.0276, 15.0163), (49.3755, 15.9538), 0.707107);
    path.close();
    path.move_to((50.2964, 18.9923));
    path.quad_to((50.4191, 19.5089), (50.5206, 20.0302));
    path.conic_to((50.7117, 21.0117), (49.7302, 21.2029), 0.707107);
    path.conic_to((48.7486, 21.394), (48.5575, 20.4125), 0.707107);
    path.quad_to((48.4638, 19.9313), (48.3505, 19.4544));
    path.conic_to((48.1194, 18.4815), (49.0924, 18.2504), 0.707107);
    path.conic_to((50.0653, 18.0193), (50.2964, 18.9923), 0.707107);
    path.close();
    path.move_to((50.8373, 22.0956));
    path.quad_to((50.8955, 22.6138), (50.933, 23.1341));
    path.conic_to((51.0047, 24.1315), (50.0073, 24.2033), 0.707107);
    path.conic_to((49.0099, 24.275), (48.9381, 23.2776), 0.707107);
    path.quad_to((48.9036, 22.7975), (48.8498, 22.3191));
    path.conic_to((48.7381, 21.3253), (49.7318, 21.2136), 0.707107);
    path.conic_to((50.7255, 21.1019), (50.8373, 22.0956), 0.707107);
    path.close();
    path.move_to((50.9992, 25.2099));
    path.quad_to((50.9949, 25.7358), (50.9694, 26.2608));
    path.conic_to((50.9209, 27.2596), (49.9221, 27.2111), 0.707107);
    path.conic_to((48.9233, 27.1626), (48.9718, 26.1638), 0.707107);
    path.quad_to((48.9953, 25.679), (48.9992, 25.1938));
    path.conic_to((49.0073, 24.1938), (50.0073, 24.2019), 0.707107);
    path.conic_to((51.0072, 24.21), (50.9992, 25.2099), 0.707107);
    path.close();
    path.move_to((50.7839, 28.3454));
    path.quad_to((50.7172, 28.8596), (50.63, 29.3708));
    path.conic_to((50.4619, 30.3565), (49.4761, 30.1884), 0.707107);
    path.conic_to((48.4903, 30.0203), (48.6584, 29.0346), 0.707107);
    path.quad_to((48.7389, 28.5627), (48.8005, 28.088));
    path.conic_to((48.9292, 27.0963), (49.9209, 27.225), 0.707107);
    path.conic_to((50.9126, 27.3537), (50.7839, 28.3454), 0.707107);
    path.close();
    path.move_to((50.1906, 31.437));
    path.quad_to((50.0558, 31.9646), (49.899, 32.4861));
    path.conic_to((49.611, 33.4438), (48.6534, 33.1558), 0.707107);
    path.conic_to((47.6957, 32.8679), (47.9837, 31.9103), 0.707107);
    path.quad_to((48.1284, 31.4289), (48.2528, 30.9418));
    path.conic_to((48.5004, 29.9729), (49.4693, 30.2205), 0.707107);
    path.conic_to((50.4382, 30.4681), (50.1906, 31.437), 0.707107);
    path.close();
    path.move_to((49.1978, 34.5114));
    path.quad_to((49.0051, 35.0016), (48.7927, 35.4837));
    path.conic_to((48.3895, 36.3988), (47.4744, 35.9956), 0.707107);
    path.conic_to((46.5593, 35.5923), (46.9625, 34.6772), 0.707107);
    path.quad_to((47.1586, 34.2323), (47.3364, 33.7797));
    path.conic_to((47.7023, 32.849), (48.6329, 33.2149), 0.707107);
    path.conic_to((49.5636, 33.5807), (49.1978, 34.5114), 0.707107);
    path.close();
    path.move_to((47.8852, 37.3397));
    path.quad_to((47.6449, 37.7853), (47.3876, 38.2211));
    path.conic_to((46.879, 39.0821), (46.018, 38.5736), 0.707107);
    path.conic_to((45.1569, 38.0651), (45.6655, 37.204), 0.707107);
    path.quad_to((45.903, 36.8018), (46.1248, 36.3906));
    path.conic_to((46.5993, 35.5103), (47.4796, 35.9849), 0.707107);
    path.conic_to((48.3598, 36.4595), (47.8852, 37.3397), 0.707107);
    path.close();
    path.move_to((46.3154, 39.8881));
    path.quad_to((46.0303, 40.2962), (45.7299, 40.693));
    path.conic_to((45.1264, 41.4903), (44.3291, 40.8867), 0.707107);
    path.conic_to((43.5318, 40.2831), (44.1353, 39.4858), 0.707107);
    path.quad_to((44.4126, 39.1195), (44.6757, 38.7428));
    path.conic_to((45.2483, 37.923), (46.0682, 38.4956), 0.707107);
    path.conic_to((46.888, 39.0682), (46.3154, 39.8881), 0.707107);
    path.close();
    path.move_to((44.4398, 42.2654));
    path.quad_to((44.095, 42.6536), (43.7349, 43.0278));
    path.conic_to((43.0415, 43.7484), (42.321, 43.055), 0.707107);
    path.conic_to((41.6004, 42.3616), (42.2938, 41.641), 0.707107);
    path.quad_to((42.6261, 41.2957), (42.9444, 40.9374));
    path.conic_to((43.6084, 40.1897), (44.3561, 40.8537), 0.707107);
    path.conic_to((45.1038, 41.5177), (44.4398, 42.2654), 0.707107);
    path.close();
    path.move_to((42.2075, 44.4911));
    path.quad_to((41.804, 44.8473), (41.3862, 45.1865));
    path.conic_to((40.6098, 45.8167), (39.9795, 45.0403), 0.707107);
    path.conic_to((39.3493, 44.2639), (40.1257, 43.6336), 0.707107);
    path.quad_to((40.5114, 43.3205), (40.8838, 42.9918));
    path.conic_to((41.6335, 42.3299), (42.2953, 43.0796), 0.707107);
    path.conic_to((42.9572, 43.8292), (42.2075, 44.4911), 0.707107);
    path.close();
    path.move_to((39.6379, 46.488));
    path.quad_to((39.2151, 46.776), (38.7814, 47.0471));
    path.conic_to((37.9334, 47.5771), (37.4034, 46.7292), 0.707107);
    path.conic_to((36.8733, 45.8812), (37.7213, 45.3511), 0.707107);
    path.quad_to((38.1217, 45.1009), (38.5119, 44.835));
    path.conic_to((39.3383, 44.2721), (39.9013, 45.0985), 0.707107);
    path.conic_to((40.4643, 45.925), (39.6379, 46.488), 0.707107);
    path.close();
    path.move_to((36.9864, 48.0722));
    path.quad_to((36.5234, 48.3127), (36.0513, 48.5344));
    path.conic_to((35.1461, 48.9595), (34.7211, 48.0543), 0.707107);
    path.conic_to((34.296, 47.1491), (35.2012, 46.7241), 0.707107);
    path.quad_to((35.6371, 46.5194), (36.0644, 46.2974));
    path.conic_to((36.9518, 45.8364), (37.4128, 46.7238), 0.707107);
    path.conic_to((37.8738, 47.6112), (36.9864, 48.0722), 0.707107);
    path.close();
    path.move_to((34.1153, 49.3498));
    path.quad_to((33.6206, 49.535), (33.1187, 49.6999));
    path.conic_to((32.1687, 50.0122), (31.8565, 49.0622), 0.707107);
    path.conic_to((31.5442, 48.1122), (32.4942, 47.7999), 0.707107);
    path.quad_to((32.9575, 47.6477), (33.4141, 47.4767));
    path.conic_to((34.3507, 47.1261), (34.7012, 48.0627), 0.707107);
    path.conic_to((35.0518, 48.9992), (34.1153, 49.3498), 0.707107);
    path.close();
    path.move_to((31.08, 50.2791));
    path.quad_to((30.5637, 50.4033), (30.0427, 50.5063));
    path.conic_to((29.0617, 50.7002), (28.8678, 49.7192), 0.707107);
    path.conic_to((28.6738, 48.7382), (29.6548, 48.5443), 0.707107);
    path.quad_to((30.1357, 48.4492), (30.6122, 48.3346));
    path.conic_to((31.5845, 48.1007), (31.8184, 49.073), 0.707107);
    path.conic_to((32.0522, 50.0453), (31.08, 50.2791), 0.707107);
    path.close();
    path.move_to((27.9769, 50.829));
    path.quad_to((27.4588, 50.8887), (26.9386, 50.9276));
    path.conic_to((25.9414, 51.0022), (25.8668, 50.005), 0.707107);
    path.conic_to((25.7923, 49.0078), (26.7895, 48.9332), 0.707107);
    path.quad_to((27.2696, 48.8973), (27.7479, 48.8422));
    path.conic_to((28.7413, 48.7277), (28.8558, 49.7211), 0.707107);
    path.conic_to((28.9703, 50.7145), (27.9769, 50.829), 0.707107);
    path.close();
    path.move_to((24.8625, 50.9996));
    path.quad_to((24.3373, 50.9969), (23.8128, 50.9729));
    path.conic_to((22.8138, 50.9272), (22.8595, 49.9283), 0.707107);
    path.conic_to((22.9051, 48.9293), (23.9041, 48.975), 0.707107);
    path.quad_to((24.3884, 48.9971), (24.8731, 48.9997));
    path.conic_to((25.8731, 49.005), (25.8678, 50.005), 0.707107);
    path.conic_to((25.8624, 51.0049), (24.8625, 50.9996), 0.707107);
    path.close();
    path.move_to((21.7268, 50.7931));
    path.quad_to((21.2121, 50.7278), (20.7005, 50.642));
    path.conic_to((19.7143, 50.4767), (19.8796, 49.4905), 0.707107);
    path.conic_to((20.045, 48.5042), (21.0312, 48.6696), 0.707107);
    path.quad_to((21.5036, 48.7488), (21.9786, 48.8091));
    path.conic_to((22.9707, 48.9349), (22.8448, 49.927), 0.707107);
    path.conic_to((22.7189, 50.919), (21.7268, 50.7931), 0.707107);
    path.close();
    path.move_to((18.6372, 50.2094));
    path.quad_to((18.1089, 50.0761), (17.5865, 49.9207));
    path.conic_to((16.628, 49.6356), (16.9132, 48.6771), 0.707107);
    path.conic_to((17.1983, 47.7186), (18.1568, 48.0037), 0.707107);
    path.quad_to((18.639, 48.1472), (19.1267, 48.2702));
    path.conic_to((20.0963, 48.515), (19.8516, 49.4846), 0.707107);
    path.conic_to((19.6068, 50.4542), (18.6372, 50.2094), 0.707107);
    path.close();
    path.move_to((15.5577, 49.2248));
    path.quad_to((15.0665, 49.0334), (14.5834, 48.8222));
    path.conic_to((13.6672, 48.4215), (14.0678, 47.5053), 0.707107);
    path.conic_to((14.4684, 46.589), (15.3847, 46.9897), 0.707107);
    path.quad_to((15.8306, 47.1846), (16.284, 47.3614));
    path.conic_to((17.2158, 47.7246), (16.8526, 48.6563), 0.707107);
    path.conic_to((16.4894, 49.588), (15.5577, 49.2248), 0.707107);
    path.close();
    path.move_to((12.7231, 47.9189));
    path.quad_to((12.2765, 47.6797), (11.8395, 47.4233));
    path.conic_to((10.9771, 46.9171), (11.4833, 46.0547), 0.707107);
    path.conic_to((11.9894, 45.1922), (12.8519, 45.6984), 0.707107);
    path.quad_to((13.2552, 45.9351), (13.6675, 46.156));
    path.conic_to((14.549, 46.6282), (14.0768, 47.5096), 0.707107);
    path.conic_to((13.6046, 48.3911), (12.7231, 47.9189), 0.707107);
    path.close();
    path.move_to((10.1686, 46.3548));
    path.quad_to((9.76024, 46.0712), (9.363, 45.7722));
    path.conic_to((8.56406, 45.1708), (9.16549, 44.3718), 0.707107);
    path.conic_to((9.76691, 43.5729), (10.5658, 44.1743), 0.707107);
    path.quad_to((10.9325, 44.4504), (11.3095, 44.7122));
    path.conic_to((12.1308, 45.2826), (11.5604, 46.1039), 0.707107);
    path.conic_to((10.9899, 46.9253), (10.1686, 46.3548), 0.707107);
    path.close();
    path.move_to((7.78853, 44.4876));
    path.quad_to((7.39972, 44.1442), (7.02492, 43.7855));
    path.conic_to((6.3024, 43.0942), (6.99374, 42.3717), 0.707107);
    path.conic_to((7.68509, 41.6492), (8.40761, 42.3405), 0.707107);
    path.quad_to((8.7536, 42.6715), (9.11249, 42.9885));
    path.conic_to((9.86201, 43.6505), (9.20003, 44.4), 0.707107);
    path.conic_to((8.53805, 45.1496), (7.78853, 44.4876), 0.707107);
    path.close();
    path.move_to((5.55855, 42.2635));
    path.quad_to((5.20148, 41.8614), (4.86131, 41.4449));
    path.conic_to((4.22883, 40.6703), (5.0034, 40.0378), 0.707107);
    path.conic_to((5.77797, 39.4053), (6.41046, 40.1799), 0.707107);
    path.quad_to((6.72443, 40.5644), (7.05403, 40.9356));
    path.conic_to((7.71802, 41.6833), (6.97028, 42.3473), 0.707107);
    path.conic_to((6.22254, 43.0113), (5.55855, 42.2635), 0.707107);
    path.close();
    path.move_to((3.55261, 39.6973));
    path.quad_to((3.26341, 39.2752), (2.99107, 38.8422));
    path.conic_to((2.45867, 37.9957), (3.30517, 37.4633), 0.707107);
    path.conic_to((4.15167, 36.9309), (4.68406, 37.7774), 0.707107);
    path.quad_to((4.93548, 38.1772), (5.20241, 38.5667));
    path.conic_to((5.76769, 39.3916), (4.94279, 39.9569), 0.707107);
    path.conic_to((4.11789, 40.5222), (3.55261, 39.6973), 0.707107);
    path.close();
    path.move_to((1.96145, 37.0509));
    path.quad_to((1.71975, 36.5889), (1.49677, 36.1175));
    path.conic_to((1.06917, 35.2135), (1.97315, 34.7859), 0.707107);
    path.conic_to((2.87712, 34.3583), (3.30471, 35.2623), 0.707107);
    path.quad_to((3.51053, 35.6974), (3.73364, 36.1239));
    path.conic_to((4.19714, 37.01), (3.31105, 37.4735), 0.707107);
    path.conic_to((2.42495, 37.937), (1.96145, 37.0509), 0.707107);
    path.close();
    path.move_to((0.676191, 34.1844));
    path.quad_to((0.489621, 33.6902), (0.323275, 33.189));
    path.conic_to((0.00831527, 32.2399), (0.95742, 31.9249), 0.707107);
    path.conic_to((1.90653, 31.6099), (2.22149, 32.559), 0.707107);
    path.quad_to((2.37504, 33.0218), (2.54726, 33.4779));
    path.conic_to((2.9005, 34.4134), (1.96497, 34.7666), 0.707107);
    path.conic_to((1.02943, 35.1199), (0.676191, 34.1844), 0.707107);
    path.close();
    path.move_to((-0.261658, 31.1521));
    path.quad_to((-0.387304, 30.6362), (-0.491779, 30.1156));
    path.conic_to((-0.68853, 29.1351), (0.291923, 28.9384), 0.707107);
    path.conic_to((1.27238, 28.7416), (1.46913, 29.7221), 0.707107);
    path.quad_to((1.56557, 30.2026), (1.68155, 30.6789));
    path.conic_to((1.91817, 31.6505), (0.946565, 31.8871), 0.707107);
    path.conic_to((-0.0250367, 32.1237), (-0.261658, 31.1521), 0.707107);
    path.close();
    path.move_to((-0.820549, 28.0495));
    path.quad_to((-0.881733, 27.5314), (-0.922089, 27.0113));
    path.conic_to((-0.999449, 26.0143), (-0.00244591, 25.9369), 0.707107);
    path.conic_to((0.994557, 25.8596), (1.07192, 26.8566), 0.707107);
    path.quad_to((1.10917, 27.3367), (1.16565, 27.8149));
    path.conic_to((1.28293, 28.808), (0.289834, 28.9253), 0.707107);
    path.conic_to((-0.703265, 29.0426), (-0.820549, 28.0495), 0.707107);
    path.close();
    path.move_to((-0.999918, 24.9349));
    path.quad_to((-0.998605, 24.4104), (-0.976138, 23.8863));
    path.conic_to((-0.933305, 22.8873), (0.0657772, 22.9301), 0.707107);
    path.conic_to((1.06486, 22.9729), (1.02203, 23.972), 0.707107);
    path.quad_to((1.00129, 24.4557), (1.00008, 24.9399));
    path.conic_to((0.997572, 25.9399), (-0.0024244, 25.9374), 0.707107);
    path.conic_to((-1.00242, 25.9349), (-0.999918, 24.9349), 0.707107);
    path.close();
    path.move_to((-0.802212, 21.7991));
    path.quad_to((-0.738311, 21.284), (-0.653903, 20.7719));
    path.conic_to((-0.491283, 19.7852), (0.495406, 19.9478), 0.707107);
    path.conic_to((1.48209, 20.1104), (1.31948, 21.0971), 0.707107);
    path.quad_to((1.24156, 21.5698), (1.18257, 22.0453));
    path.conic_to((1.05946, 23.0377), (0.0670681, 22.9146), 0.707107);
    path.conic_to((-0.925325, 22.7915), (-0.802212, 21.7991), 0.707107);
    path.close();
    path.move_to((-0.228066, 18.7115));
    path.quad_to((-0.096172, 18.1824), (0.0577899, 17.6593));
    path.conic_to((0.340124, 16.7), (1.29944, 16.9823), 0.707107);
    path.conic_to((2.25876, 17.2646), (1.97642, 18.2239), 0.707107);
    path.quad_to((1.8343, 18.7068), (1.71255, 19.1953));
    path.conic_to((1.47069, 20.1656), (0.50038, 19.9237), 0.707107);
    path.conic_to((-0.46993, 19.6819), (-0.228066, 18.7115), 0.707107);
    path.close();
    path.move_to((0.74831, 15.6269));
    path.quad_to((0.938539, 15.1347), (1.14857, 14.6506));
    path.conic_to((1.54662, 13.7333), (2.46398, 14.1313), 0.707107);
    path.conic_to((3.38135, 14.5294), (2.9833, 15.4467), 0.707107);
    path.quad_to((2.78942, 15.8936), (2.61382, 16.3479));
    path.conic_to((2.25331, 17.2806), (1.32056, 16.9201), 0.707107);
    path.conic_to((0.387801, 16.5596), (0.74831, 15.6269), 0.707107);
    path.close();
    path.move_to((2.04744, 12.7861));
    path.quad_to((2.28569, 12.3384), (2.5412, 11.9003));
    path.conic_to((3.04504, 11.0365), (3.90884, 11.5403), 0.707107);
    path.conic_to((4.77264, 12.0442), (4.26881, 12.908), 0.707107);
    path.quad_to((4.03293, 13.3123), (3.81302, 13.7256));
    path.conic_to((3.34325, 14.6084), (2.46046, 14.1386), 0.707107);
    path.conic_to((1.57767, 13.6689), (2.04744, 12.7861), 0.707107);
    path.close();
    path.move_to((3.60589, 10.2253));
    path.quad_to((3.88812, 9.81661), (4.18576, 9.419));
    path.conic_to((4.78503, 8.61845), (5.58558, 9.21772), 0.707107);
    path.conic_to((6.38613, 9.81699), (5.78686, 10.6175), 0.707107);
    path.quad_to((5.51211, 10.9846), (5.25159, 11.3618));
    path.conic_to((4.68333, 12.1847), (3.86048, 11.6164), 0.707107);
    path.conic_to((3.03763, 11.0481), (3.60589, 10.2253), 0.707107);
    path.close();
    path.move_to((5.46482, 7.84259));
    path.quad_to((5.80682, 7.4532), (6.16407, 7.07773));
    path.conic_to((6.85339, 6.35327), (7.57785, 7.04259), 0.707107);
    path.conic_to((8.30231, 7.73191), (7.61299, 8.45636), 0.707107);
    path.quad_to((7.28322, 8.80295), (6.96752, 9.16239));
    path.conic_to((6.30762, 9.91375), (5.55627, 9.25385), 0.707107);
    path.conic_to((4.80492, 8.59395), (5.46482, 7.84259), 0.707107);
    path.close();
    path.move_to((7.68062, 5.60827));
    path.quad_to((8.08142, 5.25031), (8.49666, 4.90921));
    path.conic_to((9.26938, 4.27447), (9.90412, 5.04719), 0.707107);
    path.conic_to((10.5389, 5.81992), (9.76614, 6.45466), 0.707107);
    path.quad_to((9.38285, 6.76951), (9.01289, 7.09994));
    path.conic_to((8.26705, 7.76607), (7.60092, 7.02024), 0.707107);
    path.conic_to((6.93479, 6.2744), (7.68062, 5.60827), 0.707107);
    path.close();
    path.move_to((10.2392, 3.59627));
    path.quad_to((10.6626, 3.30433), (11.0971, 3.02935));
    path.conic_to((11.9421, 2.49463), (12.4768, 3.33965), 0.707107);
    path.conic_to((13.0116, 4.18467), (12.1666, 4.7194), 0.707107);
    path.quad_to((11.7654, 4.97322), (11.3747, 5.24271));
    path.conic_to((10.5515, 5.81043), (9.98373, 4.98721), 0.707107);
    path.conic_to((9.41601, 4.16399), (10.2392, 3.59627), 0.707107);
    path.close();
    path.move_to((12.8847, 1.99524));
    path.quad_to((13.3459, 1.75234), (13.8165, 1.52812));
    path.conic_to((14.7193, 1.09799), (15.1494, 2.00075), 0.707107);
    path.conic_to((15.5795, 2.90352), (14.6768, 3.33365), 0.707107);
    path.quad_to((14.2424, 3.54063), (13.8166, 3.76484));
    path.conic_to((12.9318, 4.23081), (12.4658, 3.34601), 0.707107);
    path.conic_to((11.9999, 2.46122), (12.8847, 1.99524), 0.707107);
    path.close();
    path.move_to((15.7467, 0.702339));
    path.quad_to((16.2402, 0.514409), (16.7409, 0.346672));
    path.conic_to((17.6891, 0.029011), (18.0067, 0.977215), 0.707107);
    path.conic_to((18.3244, 1.92542), (17.3762, 2.24308), 0.707107);
    path.quad_to((16.914, 2.39792), (16.4585, 2.57139));
    path.conic_to((15.524, 2.92729), (15.1681, 1.99276), 0.707107);
    path.conic_to((14.8122, 1.05824), (15.7467, 0.702339), 0.707107);
    path.close();
    path.move_to((18.7758, -0.24399));
    path.quad_to((19.2913, -0.371107), (19.8116, -0.477061));
    path.conic_to((20.7915, -0.676608), (20.9911, 0.303281), 0.707107);
    path.conic_to((21.1906, 1.28317), (20.2107, 1.48272), 0.707107);
    path.quad_to((19.7304, 1.58052), (19.2546, 1.69785));
    path.conic_to((18.2836, 1.93725), (18.0443, 0.966329), 0.707107);
    path.conic_to((17.8049, -0.00459272), (18.7758, -0.24399), 0.707107);
    path.close();
    path.move_to((21.878, -0.811882));
    path.quad_to((22.396, -0.874528), (22.916, -0.916348));
    path.conic_to((23.9128, -0.996504), (23.993, 0.000278629), 0.707107);
    path.conic_to((24.0731, 0.997061), (23.0764, 1.07722), 0.707107);
    path.quad_to((22.5963, 1.11582), (22.1182, 1.17365));
    path.conic_to((21.1254, 1.29372), (21.0053, 0.300958), 0.707107);
    path.conic_to((20.8853, -0.691807), (21.878, -0.811882), 0.707107);
    path.close();
    path.move_to((24.9926, -0.999999));
    path.quad_to((25.5166, -1.00015), (26.0401, -0.979188));
    path.conic_to((27.0393, -0.939179), (26.9992, 0.0600199), 0.707107);
    path.conic_to((26.9592, 1.05922), (25.96, 1.01921), 0.707107);
    path.quad_to((25.4768, 0.999863), (24.9932, 1.0));
    path.conic_to((23.9932, 1.00029), (23.9929, 0.000287339), 0.707107);
    path.conic_to((23.9926, -0.999713), (24.9926, -0.999999), 0.707107);
    path.close();
    path.move_to((28.1286, -0.811081));
    path.quad_to((28.6441, -0.748593), (29.1567, -0.665572));
    path.conic_to((30.1439, -0.505698), (29.984, 0.48144), 0.707107);
    path.conic_to((29.8241, 1.46858), (28.837, 1.3087), 0.707107);
    path.quad_to((28.3638, 1.23207), (27.8879, 1.17439));
    path.conic_to((26.8952, 1.05406), (27.0155, 0.0613233), 0.707107);
    path.conic_to((27.1359, -0.931411), (28.1286, -0.811081), 0.707107);
    path.close();
    path.move_to((31.214, -0.246499));
    path.quad_to((31.7439, -0.116076), (32.2679, 0.0364622));
    path.conic_to((33.228, 0.315996), (32.9485, 1.27613), 0.707107);
    path.conic_to((32.6689, 2.23627), (31.7088, 1.95673), 0.707107);
    path.quad_to((31.2252, 1.81593), (30.736, 1.69554));
    path.conic_to((29.765, 1.45654), (30.004, 0.48552), 0.707107);
    path.conic_to((30.243, -0.485499), (31.214, -0.246499), 0.707107);
    path.close();
    path.move_to((34.3038, 0.721629));
    path.quad_to((34.797, 0.910612), (35.282, 1.11946));
    path.conic_to((36.2005, 1.51493), (35.805, 2.43341), 0.707107);
    path.conic_to((35.4096, 3.35189), (34.4911, 2.95642), 0.707107);
    path.quad_to((34.0434, 2.76365), (33.5881, 2.5892));
    path.conic_to((32.6543, 2.23137), (33.0122, 1.29758), 0.707107);
    path.conic_to((33.37, 0.363796), (34.3038, 0.721629), 0.707107);
    path.close();
    path.move_to((37.1508, 2.01396));
    path.quad_to((37.5996, 2.2512), (38.0388, 2.50578));
    path.conic_to((38.904, 3.00727), (38.4025, 3.87244), 0.707107);
    path.conic_to((37.901, 4.7376), (37.0358, 4.23612), 0.707107);
    path.quad_to((36.6304, 4.00111), (36.2161, 3.78211));
    path.conic_to((35.332, 3.31476), (35.7994, 2.43069), 0.707107);
    path.conic_to((36.2667, 1.54661), (37.1508, 2.01396), 0.707107);
    path.close();
    path.move_to((39.718, 3.56681));
    path.quad_to((40.1269, 3.84765), (40.5249, 4.14392));
    path.conic_to((41.3271, 4.74104), (40.73, 5.54319), 0.707107);
    path.conic_to((40.1329, 6.34535), (39.3307, 5.74823), 0.707107);
    path.quad_to((38.9634, 5.47478), (38.5858, 5.21552));
    path.conic_to((37.7615, 4.64945), (38.3275, 3.82509), 0.707107);
    path.conic_to((38.8936, 3.00074), (39.718, 3.56681), 0.707107);
    path.close();
    path.move_to((42.1033, 5.41741));
    path.quad_to((42.4933, 5.75802), (42.8694, 6.11388));
    path.conic_to((43.5958, 6.80115), (42.9085, 7.52755), 0.707107);
    path.conic_to((42.2212, 8.25394), (41.4948, 7.56667), 0.707107);
    path.quad_to((41.1476, 7.23817), (40.7876, 6.92375));
    path.conic_to((40.0345, 6.26593), (40.6923, 5.51275), 0.707107);
    path.conic_to((41.3501, 4.75958), (42.1033, 5.41741), 0.707107);
    path.close();
    path.move_to((44.3419, 7.62498));
    path.quad_to((44.7007, 8.02444), (45.0428, 8.43835));
    path.conic_to((45.6797, 9.20922), (44.9089, 9.84622), 0.707107);
    path.conic_to((44.138, 10.4832), (43.501, 9.71234), 0.707107);
    path.quad_to((43.1852, 9.3302), (42.854, 8.96151));
    path.conic_to((42.1858, 8.21759), (42.9297, 7.54932), 0.707107);
    path.conic_to((43.6736, 6.88106), (44.3419, 7.62498), 0.707107);
    path.close();
    path.move_to((46.3599, 10.1759));
    path.quad_to((46.6546, 10.6005), (46.9322, 11.0366));
    path.conic_to((47.4693, 11.8801), (46.6257, 12.4172), 0.707107);
    path.conic_to((45.7822, 12.9542), (45.2451, 12.1107), 0.707107);
    path.quad_to((44.9889, 11.7082), (44.7168, 11.3162));
    path.conic_to((44.1467, 10.4947), (44.9682, 9.92452), 0.707107);
    path.conic_to((45.7897, 9.35435), (46.3599, 10.1759), 0.707107);
    path.close();
    path.move_to((47.9708, 12.8204));
    path.quad_to((48.2149, 13.2808), (48.4403, 13.7506));
    path.conic_to((48.873, 14.6521), (47.9715, 15.0848), 0.707107);
    path.conic_to((47.0699, 15.5174), (46.6372, 14.6159), 0.707107);
    path.quad_to((46.4291, 14.1822), (46.2038, 13.7573));
    path.conic_to((45.7354, 12.8738), (46.6188, 12.4054), 0.707107);
    path.conic_to((47.5023, 11.9369), (47.9708, 12.8204), 0.707107);
    path.close();
    path.move_to((49.2713, 15.6778));
    path.quad_to((49.4606, 16.1706), (49.6297, 16.6708));
    path.conic_to((49.9501, 17.6181), (49.0028, 17.9384), 0.707107);
    path.conic_to((48.0555, 18.2588), (47.7351, 17.3115), 0.707107);
    path.quad_to((47.5791, 16.8499), (47.4043, 16.3949));
    path.conic_to((47.0458, 15.4614), (47.9793, 15.1029), 0.707107);
    path.conic_to((48.9128, 14.7443), (49.2713, 15.6778), 0.707107);
    path.close();
    path.move_to((50.2261, 18.7037));
    path.quad_to((50.3547, 19.2188), (50.4621, 19.7388));
    path.conic_to((50.6645, 20.7182), (49.6852, 20.9205), 0.707107);
    path.conic_to((48.7059, 21.1229), (48.5035, 20.1436), 0.707107);
    path.quad_to((48.4043, 19.6636), (48.2856, 19.1881));
    path.conic_to((48.0435, 18.2178), (49.0137, 17.9757), 0.707107);
    path.conic_to((49.984, 17.7335), (50.2261, 18.7037), 0.707107);
    path.close();
    path.move_to((50.803, 21.8055));
    path.quad_to((50.8671, 22.3234), (50.9104, 22.8434));
    path.conic_to((50.9934, 23.8399), (49.9968, 23.9229), 0.707107);
    path.conic_to((49.0002, 24.0058), (48.9173, 23.0093), 0.707107);
    path.quad_to((48.8773, 22.5293), (48.8182, 22.0513));
    path.conic_to((48.6953, 21.0588), (49.6877, 20.936), 0.707107);
    path.conic_to((50.6801, 20.8131), (50.803, 21.8055), 0.707107);
    path.close();
    path.move_to((50.9999, 24.9202));
    path.quad_to((51.0015, 25.4434), (50.982, 25.9664));
    path.conic_to((50.9449, 26.9657), (49.9456, 26.9286), 0.707107);
    path.conic_to((48.9463, 26.8914), (48.9834, 25.8921), 0.707107);
    path.quad_to((49.0014, 25.4094), (48.9999, 24.9263));
    path.conic_to((48.9968, 23.9263), (49.9968, 23.9232), 0.707107);
    path.conic_to((50.9968, 23.9202), (50.9999, 24.9202), 0.707107);
    path.close();
    path.move_to((50.8198, 28.0562));
    path.quad_to((50.7587, 28.5721), (50.677, 29.0852));
    path.conic_to((50.5199, 30.0728), (49.5323, 29.9157), 0.707107);
    path.conic_to((48.5448, 29.7586), (48.7019, 28.771), 0.707107);
    path.quad_to((48.7772, 28.2974), (48.8336, 27.8211));
    path.conic_to((48.9512, 26.8281), (49.9442, 26.9456), 0.707107);
    path.conic_to((50.9373, 27.0632), (50.8198, 28.0562), 0.707107);
    path.close();
    path.move_to((50.2647, 31.1395));
    path.quad_to((50.1358, 31.6701), (49.9847, 32.1949));
    path.conic_to((49.7079, 33.1558), (48.747, 32.8791), 0.707107);
    path.conic_to((47.786, 32.6024), (48.0628, 31.6414), 0.707107);
    path.quad_to((48.2022, 31.1571), (48.3213, 30.6672));
    path.conic_to((48.5574, 29.6955), (49.5291, 29.9317), 0.707107);
    path.conic_to((50.5009, 30.1678), (50.2647, 31.1395), 0.707107);
    path.close();
    path.move_to((49.3049, 34.2343));
    path.quad_to((49.1171, 34.7285), (48.9095, 35.2145));
    path.conic_to((48.5166, 36.1341), (47.597, 35.7412), 0.707107);
    path.conic_to((46.6774, 35.3483), (47.0703, 34.4288), 0.707107);
    path.quad_to((47.262, 33.9801), (47.4353, 33.524));
    path.conic_to((47.7904, 32.5892), (48.7252, 32.9444), 0.707107);
    path.conic_to((49.66, 33.2995), (49.3049, 34.2343), 0.707107);
    path.close();
    path.move_to((48.0194, 37.0875));
    path.quad_to((47.7831, 37.5374), (47.5295, 37.9777));
    path.conic_to((47.0304, 38.8443), (46.1638, 38.3451), 0.707107);
    path.conic_to((45.2973, 37.846), (45.7965, 36.9795), 0.707107);
    path.quad_to((46.0306, 36.5729), (46.2487, 36.1577));
    path.conic_to((46.7136, 35.2723), (47.5989, 35.7372), 0.707107);
    path.conic_to((48.4843, 36.2021), (48.0194, 37.0875), 0.707107);
    path.close();
    path.move_to((46.4721, 39.6612));
    path.quad_to((46.1926, 40.0705), (45.8977, 40.4688));
    path.conic_to((45.3028, 41.2726), (44.499, 40.6776), 0.707107);
    path.conic_to((43.6953, 40.0827), (44.2902, 39.2789), 0.707107);
    path.quad_to((44.5624, 38.9112), (44.8204, 38.5334));
    path.conic_to((45.3843, 37.7075), (46.2101, 38.2714), 0.707107);
    path.conic_to((47.036, 38.8353), (46.4721, 39.6612), 0.707107);
    path.close();
    path.move_to((44.6298, 42.0491));
    path.quad_to((44.2906, 42.4396), (43.9361, 42.8164));
    path.conic_to((43.2509, 43.5447), (42.5226, 42.8595), 0.707107);
    path.conic_to((41.7942, 42.1742), (42.4795, 41.4459), 0.707107);
    path.quad_to((42.8067, 41.0981), (43.1198, 40.7376));
    path.conic_to((43.7756, 39.9826), (44.5306, 40.6383), 0.707107);
    path.conic_to((45.2856, 41.2941), (44.6298, 42.0491), 0.707107);
    path.close();
    path.move_to((42.4305, 44.2919));
    path.quad_to((42.0324, 44.6516), (41.6198, 44.9946));
    path.conic_to((40.8507, 45.6338), (40.2115, 44.8648), 0.707107);
    path.conic_to((39.5723, 44.0958), (40.3413, 43.4566), 0.707107);
    path.quad_to((40.7222, 43.1399), (41.0897, 42.8079));
    path.conic_to((41.8317, 42.1375), (42.5021, 42.8795), 0.707107);
    path.conic_to((43.1725, 43.6215), (42.4305, 44.2919), 0.707107);
    path.close();
    path.move_to((39.8873, 46.3159));
    path.quad_to((39.4613, 46.6134), (39.0238, 46.8936));
    path.conic_to((38.1818, 47.433), (37.6424, 46.5909), 0.707107);
    path.conic_to((37.103, 45.7489), (37.9451, 45.2095), 0.707107);
    path.quad_to((38.3489, 44.9508), (38.7421, 44.6763));
    path.conic_to((39.5619, 44.1037), (40.1345, 44.9235), 0.707107);
    path.conic_to((40.7071, 45.7434), (39.8873, 46.3159), 0.707107);
    path.close();
    path.move_to((37.2437, 47.9367));
    path.quad_to((36.7842, 48.182), (36.3153, 48.4086));
    path.conic_to((35.415, 48.8439), (34.9797, 47.9435), 0.707107);
    path.conic_to((34.5445, 47.0432), (35.4449, 46.608), 0.707107);
    path.quad_to((35.8778, 46.3987), (36.3019, 46.1723));
    path.conic_to((37.1841, 45.7014), (37.655, 46.5836), 0.707107);
    path.conic_to((38.1259, 47.4658), (37.2437, 47.9367), 0.707107);
    path.close();
    path.move_to((34.3909, 49.2448));
    path.quad_to((33.8988, 49.4354), (33.3992, 49.606));
    path.conic_to((32.4528, 49.929), (32.1298, 48.9826), 0.707107);
    path.conic_to((31.8068, 48.0362), (32.7532, 47.7132), 0.707107);
    path.quad_to((33.2142, 47.5558), (33.6685, 47.3798));
    path.conic_to((34.601, 47.0186), (34.9622, 47.9511), 0.707107);
    path.conic_to((35.3234, 48.8836), (34.3909, 49.2448), 0.707107);
    path.close();
    path.move_to((31.3682, 50.208));
    path.quad_to((30.8535, 50.3381), (30.3338, 50.447));
    path.conic_to((29.3551, 50.6521), (29.15, 49.6734), 0.707107);
    path.conic_to((28.9448, 48.6947), (29.9236, 48.4895), 0.707107);
    path.quad_to((30.4033, 48.389), (30.8784, 48.269));
    path.conic_to((31.8479, 48.024), (32.0929, 48.9936), 0.707107);
    path.conic_to((32.3378, 49.9631), (31.3682, 50.208), 0.707107);
    path.close();
    path.move_to((28.2669, 50.7939));
    path.quad_to((27.7491, 50.8595), (27.2292, 50.9043));
    path.conic_to((26.2329, 50.99), (26.1472, 49.9937), 0.707107);
    path.conic_to((26.0615, 48.9973), (27.0578, 48.9116), 0.707107);
    path.quad_to((27.5378, 48.8703), (28.0156, 48.8098));
    path.conic_to((29.0077, 48.6841), (29.1334, 49.6762), 0.707107);
    path.conic_to((29.259, 50.6683), (28.2669, 50.7939), 0.707107);
    path.close();
    path.move_to((25.1523, 50.9996));
    path.quad_to((24.6297, 51.0026), (24.1072, 50.9847));
    path.conic_to((23.1078, 50.9503), (23.1422, 49.9509), 0.707107);
    path.conic_to((23.1765, 48.9515), (24.1759, 48.9858), 0.707107);
    path.quad_to((24.658, 49.0024), (25.1406, 48.9996));
    path.conic_to((26.1406, 48.9937), (26.1464, 49.9937), 0.707107);
    path.conic_to((26.1523, 50.9937), (25.1523, 50.9996), 0.707107);
    path.close();
    path.move_to((22.0162, 50.8282));
    path.quad_to((21.4999, 50.7686), (20.9863, 50.6883));
    path.conic_to((19.9983, 50.5339), (20.1527, 49.5459), 0.707107);
    path.conic_to((20.307, 48.5579), (21.295, 48.7123), 0.707107);
    path.quad_to((21.7691, 48.7864), (22.2457, 48.8414));
    path.conic_to((23.2391, 48.9562), (23.1243, 49.9496), 0.707107);
    path.conic_to((23.0096, 50.943), (22.0162, 50.8282), 0.707107);
    path.close();
    path.move_to((18.9351, 50.2827));
    path.quad_to((18.4037, 50.1553), (17.8782, 50.0056));
    path.conic_to((16.9164, 49.7317), (17.1904, 48.7699), 0.707107);
    path.conic_to((17.4643, 47.8082), (18.426, 48.0821), 0.707107);
    path.quad_to((18.9112, 48.2203), (19.4016, 48.3379));
    path.conic_to((20.374, 48.5712), (20.1408, 49.5436), 0.707107);
    path.conic_to((19.9075, 50.516), (18.9351, 50.2827), 0.707107);
    path.close();
    path.move_to((15.8352, 49.3312));
    path.quad_to((15.3403, 49.1448), (14.8531, 48.9383));
    path.conic_to((13.9324, 48.548), (14.3227, 47.6273), 0.707107);
    path.conic_to((14.713, 46.7066), (15.6337, 47.0969), 0.707107);
    path.quad_to((16.0832, 47.2874), (16.5402, 47.4596));
    path.conic_to((17.476, 47.812), (17.1235, 48.7479), 0.707107);
    path.conic_to((16.771, 49.6837), (15.8352, 49.3312), 0.707107);
    path.close();
    path.move_to((12.9759, 48.0526));
    path.quad_to((12.5249, 47.8173), (12.0835, 47.5647));
    path.conic_to((11.2156, 47.0679), (11.7124, 46.2), 0.707107);
    path.conic_to((12.2092, 45.3321), (13.0771, 45.8289), 0.707107);
    path.quad_to((13.4846, 46.0622), (13.9009, 46.2793));
    path.conic_to((14.7875, 46.7418), (14.325, 47.6284), 0.707107);
    path.conic_to((13.8626, 48.5151), (12.9759, 48.0526), 0.707107);
    path.close();
    path.move_to((10.3957, 46.5108));
    path.quad_to((9.9861, 46.2327), (9.58733, 45.9392));
    path.conic_to((8.78198, 45.3464), (9.37478, 44.541), 0.707107);
    path.conic_to((9.96757, 43.7357), (10.7729, 44.3285), 0.707107);
    path.quad_to((11.141, 44.5994), (11.5191, 44.8561));
    path.conic_to((12.3464, 45.4178), (11.7847, 46.2451), 0.707107);
    path.conic_to((11.223, 47.0725), (10.3957, 46.5108), 0.707107);
    path.close();
    path.move_to((8.00525, 44.6769));
    path.quad_to((7.6141, 44.339), (7.23672, 43.9859));
    path.conic_to((6.50649, 43.3027), (7.18969, 42.5725), 0.707107);
    path.conic_to((7.87289, 41.8423), (8.60312, 42.5255), 0.707107);
    path.quad_to((8.95149, 42.8514), (9.31254, 43.1632));
    path.conic_to((10.0693, 43.8169), (9.4157, 44.5737), 0.707107);
    path.conic_to((8.76206, 45.3305), (8.00525, 44.6769), 0.707107);
    path.close();
    path.move_to((5.75818, 42.4858));
    path.quad_to((5.39763, 42.089), (5.05371, 41.6777));
    path.conic_to((4.41226, 40.9105), (5.17942, 40.2691), 0.707107);
    path.conic_to((5.94658, 39.6276), (6.58804, 40.3948), 0.707107);
    path.quad_to((6.90548, 40.7744), (7.23832, 41.1407));
    path.conic_to((7.91085, 41.8808), (7.17078, 42.5533), 0.707107);
    path.conic_to((6.43071, 43.2258), (5.75818, 42.4858), 0.707107);
    path.close();
    path.move_to((3.72821, 39.9503));
    path.quad_to((3.42794, 39.523), (3.1451, 39.0842));
    path.conic_to((2.6034, 38.2436), (3.44397, 37.7019), 0.707107);
    path.conic_to((4.28454, 37.1602), (4.82624, 38.0008), 0.707107);
    path.quad_to((5.08734, 38.4059), (5.3645, 38.8003));
    path.conic_to((5.93951, 39.6184), (5.12137, 40.1934), 0.707107);
    path.conic_to((4.30322, 40.7684), (3.72821, 39.9503), 0.707107);
    path.close();
    path.move_to((2.09762, 37.3078));
    path.quad_to((1.85114, 36.8491), (1.62324, 36.381));
    path.conic_to((1.18551, 35.4819), (2.08461, 35.0442), 0.707107);
    path.conic_to((2.98372, 34.6064), (3.42145, 35.5055), 0.707107);
    path.quad_to((3.63184, 35.9377), (3.85934, 36.361));
    path.conic_to((4.33272, 37.2419), (3.45185, 37.7153), 0.707107);
    path.conic_to((2.57099, 38.1886), (2.09762, 37.3078), 0.707107);
    path.close();
    path.move_to((0.781912, 34.4596));
    path.quad_to((0.589924, 33.9681), (0.418029, 33.4692));
    path.conic_to((0.0922952, 32.5237), (1.03776, 32.198), 0.707107);
    path.conic_to((1.98322, 31.8722), (2.30895, 32.8177), 0.707107);
    path.quad_to((2.46761, 33.2782), (2.64484, 33.7319));
    path.conic_to((3.00867, 34.6634), (2.07721, 35.0272), 0.707107);
    path.conic_to((1.14575, 35.3911), (0.781912, 34.4596), 0.707107);
    path.close();
    path.move_to((-0.189761, 31.4402));
    path.quad_to((-0.321263, 30.9258), (-0.431662, 30.4065));
    path.conic_to((-0.639608, 29.4284), (0.338532, 29.2205), 0.707107);
    path.conic_to((1.31667, 29.0125), (1.52462, 29.9906), 0.707107);
    path.quad_to((1.62653, 30.47), (1.74791, 30.9448));
    path.conic_to((1.99561, 31.9136), (1.02677, 32.1613), 0.707107);
    path.conic_to((0.0579369, 32.409), (-0.189761, 31.4402), 0.707107);
    path.close();
    path.move_to((-0.784658, 28.3394));
    path.quad_to((-0.851693, 27.8218), (-0.897902, 27.3019));
    path.conic_to((-0.986437, 26.3058), (0.00963629, 26.2173), 0.707107);
    path.conic_to((1.00571, 26.1288), (1.09424, 27.1248), 0.707107);
    path.quad_to((1.1369, 27.6047), (1.19878, 28.0825));
    path.conic_to((1.32721, 29.0742), (0.335496, 29.2027), 0.707107);
    path.conic_to((-0.656222, 29.3311), (-0.784658, 28.3394), 0.707107);
    path.close();
    path.move_to((-0.999031, 25.2248));
    path.quad_to((-1.00354, 24.7027), (-0.987098, 24.1809));
    path.conic_to((-0.955596, 23.1814), (0.0439078, 23.2129), 0.707107);
    path.conic_to((1.04341, 23.2444), (1.01191, 24.2439), 0.707107);
    path.quad_to((0.996728, 24.7256), (1.00089, 25.2075));
    path.conic_to((1.00954, 26.2075), (0.00957754, 26.2161), 0.707107);
    path.conic_to((-0.990385, 26.2248), (-0.999031, 25.2248), 0.707107);
    path.close();
    path.move_to((-0.836492, 22.0887));
    path.quad_to((-0.778263, 21.5719), (-0.699419, 21.0579));
    path.conic_to((-0.5478, 20.0695), (0.440639, 20.2211), 0.707107);
    path.conic_to((1.42908, 20.3727), (1.27746, 21.3612), 0.707107);
    path.quad_to((1.20468, 21.8356), (1.15093, 22.3126));
    path.conic_to((1.03896, 23.3063), (0.0452449, 23.1944), 0.707107);
    path.conic_to((-0.948466, 23.0824), (-0.836492, 22.0887), 0.707107);
    path.close();
    path.move_to((-0.300548, 19.0098));
    path.quad_to((-0.174573, 18.4777), (-0.0263361, 17.9514));
    path.conic_to((0.244762, 16.9889), (1.20731, 17.26), 0.707107);
    path.conic_to((2.16987, 17.5311), (1.89877, 18.4936), 0.707107);
    path.quad_to((1.76193, 18.9794), (1.64565, 19.4706));
    path.conic_to((1.41526, 20.4437), (0.442159, 20.2133), 0.707107);
    path.conic_to((-0.530939, 19.9829), (-0.300548, 19.0098), 0.707107);
    path.close();
    path.move_to((0.642658, 15.9049));
    path.quad_to((0.827861, 15.409), (1.0331, 14.9209));
    path.conic_to((1.42076, 13.9991), (2.34256, 14.3868), 0.707107);
    path.conic_to((3.26437, 14.7744), (2.87671, 15.6962), 0.707107);
    path.quad_to((2.68726, 16.1467), (2.5163, 16.6046));
    path.conic_to((2.16648, 17.5414), (1.22967, 17.1916), 0.707107);
    path.conic_to((0.292846, 16.8418), (0.642658, 15.9049), 0.707107);
    path.close();
    path.move_to((1.91434, 13.0395));
    path.quad_to((2.14856, 12.5875), (2.40031, 12.1449));
    path.conic_to((2.89473, 11.2757), (3.76395, 11.7701), 0.707107);
    path.conic_to((4.63317, 12.2645), (4.13875, 13.1337), 0.707107);
    path.quad_to((3.90637, 13.5423), (3.69016, 13.9596));
    path.conic_to((3.23014, 14.8475), (2.34223, 14.3875), 0.707107);
    path.conic_to((1.45432, 13.9275), (1.91434, 13.0395), 0.707107);
    path.close();
    path.move_to((3.45073, 10.4525));
    path.quad_to((3.72744, 10.0426), (4.01954, 9.64356));
    path.conic_to((4.61017, 8.83661), (5.41711, 9.42725), 0.707107);
    path.conic_to((6.22405, 10.0179), (5.63342, 10.8248), 0.707107);
    path.quad_to((5.36379, 11.1932), (5.10836, 11.5716));
    path.conic_to((4.54884, 12.4004), (3.72003, 11.8409), 0.707107);
    path.conic_to((2.89121, 11.2813), (3.45073, 10.4525), 0.707107);
    path.close();
    path.move_to((5.2763, 8.05964));
    path.quad_to((5.61273, 7.66793), (5.96445, 7.2899));
    path.conic_to((6.6456, 6.55776), (7.37774, 7.23892), 0.707107);
    path.conic_to((8.10988, 7.92008), (7.42872, 8.65221), 0.707107);
    path.quad_to((7.10407, 9.00116), (6.79351, 9.36274));
    path.conic_to((6.14196, 10.1213), (5.38336, 9.46979), 0.707107);
    path.conic_to((4.62475, 8.81824), (5.2763, 8.05964), 0.707107);
    path.close();
    path.move_to((7.45913, 5.80839));
    path.quad_to((7.85457, 5.44696), (8.26455, 5.10214));
    path.conic_to((9.02985, 4.45847), (9.67352, 5.22377), 0.707107);
    path.conic_to((10.3172, 5.98907), (9.5519, 6.63274), 0.707107);
    path.quad_to((9.17345, 6.95105), (8.80843, 7.28467));
    path.conic_to((8.07029, 7.95931), (7.39564, 7.22117), 0.707107);
    path.conic_to((6.72099, 6.48303), (7.45913, 5.80839), 0.707107);
    path.close();
    path.move_to((9.98688, 3.77251));
    path.quad_to((10.4153, 3.46948), (10.8557, 3.18397));
    path.conic_to((11.6948, 2.63996), (12.2388, 3.47904), 0.707107);
    path.conic_to((12.7828, 4.31812), (11.9437, 4.86213), 0.707107);
    path.quad_to((11.5373, 5.12566), (11.1417, 5.40539));
    path.conic_to((10.3253, 5.98282), (9.74787, 5.16638), 0.707107);
    path.conic_to((9.17044, 4.34994), (9.98688, 3.77251), 0.707107);
    path.close();
    path.move_to((12.6283, 2.13208));
    path.quad_to((13.0861, 1.88442), (13.5534, 1.65529));
    path.conic_to((14.4513, 1.21504), (14.8915, 2.11291), 0.707107);
    path.conic_to((15.3318, 3.01078), (14.4339, 3.45104), 0.707107);
    path.quad_to((14.0025, 3.66255), (13.58, 3.89115));
    path.conic_to((12.7005, 4.36698), (12.2246, 3.48744), 0.707107);
    path.conic_to((11.7488, 2.60791), (12.6283, 2.13208), 0.707107);
    path.close();
    path.move_to((15.4718, 0.808815));
    path.quad_to((15.9627, 0.615476), (16.461, 0.442208));
    path.conic_to((17.4055, 0.113784), (17.7339, 1.05831), 0.707107);
    path.conic_to((18.0624, 2.00284), (17.1178, 2.33127), 0.707107);
    path.quad_to((16.6578, 2.49121), (16.2047, 2.66968));
    path.conic_to((15.2743, 3.03614), (14.9078, 2.10571), 0.707107);
    path.conic_to((14.5414, 1.17528), (15.4718, 0.808815), 0.707107);
    path.close();
    path.move_to((18.4879, -0.171272));
    path.quad_to((19.0019, -0.304236), (19.5208, -0.416111));
    path.conic_to((20.4984, -0.62685), (20.7091, 0.350692), 0.707107);
    path.conic_to((20.9198, 1.32823), (19.9423, 1.53897), 0.707107);
    path.quad_to((19.4633, 1.64224), (18.9889, 1.76498));
    path.conic_to((18.0207, 2.01544), (17.7703, 1.04732), 0.707107);
    path.conic_to((17.5198, 0.0791926), (18.4879, -0.171272), 0.707107);
    path.close();
    path.move_to((21.5882, -0.77517));
    path.quad_to((22.1056, -0.843665), (22.6254, -0.891339));
    path.conic_to((23.6212, -0.982672), (23.7126, 0.0131486), 0.707107);
    path.conic_to((23.8039, 1.00897), (22.8081, 1.1003), 0.707107);
    path.quad_to((22.3283, 1.14431), (21.8506, 1.20754));
    path.conic_to((20.8592, 1.33876), (20.728, 0.347405), 0.707107);
    path.conic_to((20.5968, -0.643948), (21.5882, -0.77517), 0.707107);
    path.close();
    path.move_to((24.7026, -0.998301));
    path.quad_to((25.2241, -1.00426), (25.7453, -0.989316));
    path.conic_to((26.7449, -0.960651), (26.7162, 0.0389383), 0.707107);
    path.conic_to((26.6876, 1.03853), (25.688, 1.00986), 0.707107);
    path.quad_to((25.2068, 0.996064), (24.7255, 1.00157));
    path.conic_to((23.7256, 1.013), (23.7141, 0.0130688), 0.707107);
    path.conic_to((23.7027, -0.986866), (24.7026, -0.998301), 0.707107);
    path.close();
    path.move_to((27.8388, -0.844563));
    path.quad_to((28.3559, -0.787759), (28.8704, -0.710314));
    path.conic_to((29.8592, -0.561454), (29.7104, 0.427404), 0.707107);
    path.conic_to((29.5615, 1.41626), (28.5726, 1.2674), 0.707107);
    path.quad_to((28.0978, 1.19591), (27.6204, 1.14348));
    path.conic_to((26.6264, 1.0343), (26.7356, 0.0402742), 0.707107);
    path.conic_to((26.8447, -0.953747), (27.8388, -0.844563), 0.707107);
    path.close();
    path.move_to((30.9153, -0.318153));
    path.quad_to((31.4481, -0.193671), (31.9752, -0.046875));
    path.conic_to((32.9386, 0.221405), (32.6703, 1.18475), 0.707107);
    path.conic_to((32.402, 2.14809), (31.4387, 1.87981), 0.707107);
    path.quad_to((30.9521, 1.74431), (30.4603, 1.6294));
    path.conic_to((29.4865, 1.40189), (29.714, 0.428111), 0.707107);
    path.conic_to((29.9416, -0.545664), (30.9153, -0.318153), 0.707107);
    path.close();
    path.move_to((34.0252, 0.616677));
    path.quad_to((34.5221, 0.800609), (35.0111, 1.00465));
    path.conic_to((35.934, 1.3897), (35.549, 2.31259), 0.707107);
    path.conic_to((35.1639, 3.23549), (34.241, 2.85044), 0.707107);
    path.quad_to((33.7896, 2.66211), (33.3309, 2.49232));
    path.conic_to((32.3931, 2.1452), (32.7402, 1.20738), 0.707107);
    path.conic_to((33.0873, 0.269559), (34.0252, 0.616677), 0.707107);
    path.close();
    path.move_to((36.8967, 1.88141));
    path.quad_to((37.3499, 2.11462), (37.7936, 2.3654));
    path.conic_to((38.6641, 2.85746), (38.1721, 3.72802), 0.707107);
    path.conic_to((37.68, 4.59858), (36.8094, 4.10652), 0.707107);
    path.quad_to((36.3999, 3.87504), (35.9815, 3.65976));
    path.conic_to((35.0924, 3.2022), (35.5499, 2.31302), 0.707107);
    path.conic_to((36.0075, 1.42384), (36.8967, 1.88141), 0.707107);
    path.close();
    path.move_to((39.4914, 3.413));
    path.line_to((39.5381, 3.44439));
    path.quad_to((39.9244, 3.70494), (40.3002, 3.97845));
    path.conic_to((41.1087, 4.56692), (40.5202, 5.37544), 0.707107);
    path.conic_to((39.9317, 6.18396), (39.1232, 5.59549), 0.707107);
    path.quad_to((38.7763, 5.34298), (38.4215, 5.10371));
    path.line_to((38.3749, 5.07232));
    path.conic_to((37.5452, 4.51406), (38.1035, 3.68439), 0.707107);
    path.conic_to((38.6618, 2.85473), (39.4914, 3.413), 0.707107);
    path.close();
    path.move_to((41.8859, 5.22965));
    path.quad_to((42.2782, 5.56471), (42.6568, 5.91499));
    path.conic_to((43.3908, 6.5941), (42.7117, 7.32814), 0.707107);
    path.conic_to((42.0326, 8.06218), (41.2986, 7.38308), 0.707107);
    path.quad_to((40.949, 7.05968), (40.587, 6.75043));
    path.conic_to((39.8266, 6.10097), (40.476, 5.34058), 0.707107);
    path.conic_to((41.1255, 4.58018), (41.8859, 5.22965), 0.707107);
    path.close();
    path.move_to((44.1413, 7.40421));
    path.quad_to((44.5035, 7.79829), (44.8493, 8.20695));
    path.conic_to((45.4952, 8.97038), (44.7317, 9.61627), 0.707107);
    path.conic_to((43.9683, 10.2622), (43.3224, 9.49874), 0.707107);
    path.quad_to((43.0033, 9.1215), (42.6689, 8.75773));
    path.conic_to((41.9921, 8.02152), (42.7283, 7.34476), 0.707107);
    path.conic_to((43.4645, 6.668), (44.1413, 7.40421), 0.707107);
    path.close();
    path.move_to((46.183, 9.9242));
    path.quad_to((46.4888, 10.3539), (46.777, 10.7957));
    path.conic_to((47.3233, 11.6332), (46.4857, 12.1796), 0.707107);
    path.conic_to((45.6482, 12.7259), (45.1018, 11.8883), 0.707107);
    path.quad_to((44.8358, 11.4805), (44.5535, 11.0839));
    path.conic_to((43.9737, 10.2691), (44.7884, 9.6893), 0.707107);
    path.conic_to((45.6032, 9.10947), (46.183, 9.9242), 0.707107);
    path.close();
    path.move_to((47.8333, 12.5645));
    path.quad_to((48.0821, 13.0214), (48.3125, 13.4879));
    path.conic_to((48.7552, 14.3845), (47.8586, 14.8273), 0.707107);
    path.conic_to((46.962, 15.2701), (46.5192, 14.3734), 0.707107);
    path.quad_to((46.3065, 13.9428), (46.0769, 13.5211));
    path.conic_to((45.5986, 12.6429), (46.4768, 12.1646), 0.707107);
    path.conic_to((47.355, 11.6863), (47.8333, 12.5645), 0.707107);
    path.close();
    path.move_to((49.1641, 15.4033));
    path.quad_to((49.3588, 15.8935), (49.5334, 16.3912));
    path.conic_to((49.8645, 17.3348), (48.9209, 17.6659), 0.707107);
    path.conic_to((47.9773, 17.997), (47.6462, 17.0534), 0.707107);
    path.quad_to((47.485, 16.5939), (47.3053, 16.1415));
    path.conic_to((46.9362, 15.2121), (47.8656, 14.843), 0.707107);
    path.conic_to((48.795, 14.4739), (49.1641, 15.4033), 0.707107);
    path.close();
    path.move_to((50.1526, 18.4161));
    path.quad_to((50.287, 18.9296), (50.4003, 19.4482));
    path.conic_to((50.6139, 20.4252), (49.6369, 20.6387), 0.707107);
    path.conic_to((48.66, 20.8522), (48.4465, 19.8753), 0.707107);
    path.quad_to((48.3419, 19.3966), (48.2178, 18.9225));
    path.conic_to((47.9645, 17.9551), (48.9319, 17.7019), 0.707107);
    path.conic_to((49.8993, 17.4487), (50.1526, 18.4161), 0.707107);
    path.close();
    path.move_to((50.7655, 21.5157));
    path.quad_to((50.8354, 22.033), (50.8846, 22.5528));
    path.conic_to((50.9787, 23.5483), (49.9831, 23.6425), 0.707107);
    path.conic_to((48.9876, 23.7366), (48.8935, 22.741), 0.707107);
    path.quad_to((48.8481, 22.2613), (48.7835, 21.7837));
    path.conic_to((48.6495, 20.7928), (49.6405, 20.6587), 0.707107);
    path.conic_to((50.6315, 20.5247), (50.7655, 21.5157), 0.707107);
    path.close();
    path.move_to((50.9974, 24.6301));
    path.quad_to((51.0048, 25.1509), (50.9913, 25.6715));
    path.conic_to((50.9655, 26.6712), (49.9658, 26.6454), 0.707107);
    path.conic_to((48.9662, 26.6196), (48.992, 25.6199), 0.707107);
    path.quad_to((49.0044, 25.1393), (48.9976, 24.6585));
    path.conic_to((48.9834, 23.6586), (49.9833, 23.6444), 0.707107);
    path.conic_to((50.9832, 23.6302), (50.9974, 24.6301), 0.707107);
    path.close();
    path.move_to((50.8524, 27.7662));
    path.quad_to((50.7971, 28.2837), (50.721, 28.7986));
    path.conic_to((50.5749, 29.7879), (49.5856, 29.6418), 0.707107);
    path.conic_to((48.5963, 29.4957), (48.7425, 28.5064), 0.707107);
    path.quad_to((48.8127, 28.0311), (48.8638, 27.5534));
    path.conic_to((48.9702, 26.5591), (49.9645, 26.6655), 0.707107);
    path.conic_to((50.9588, 26.7718), (50.8524, 27.7662), 0.707107);
    path.close();
    path.move_to((50.3355, 30.8404));
    path.quad_to((50.2125, 31.3739), (50.0672, 31.9018));
    path.conic_to((49.8018, 32.8659), (48.8376, 32.6005), 0.707107);
    path.conic_to((47.8735, 32.335), (48.139, 31.3709), 0.707107);
    path.quad_to((48.2731, 30.8836), (48.3867, 30.3912));
    path.conic_to((48.6113, 29.4167), (49.5857, 29.6413), 0.707107);
    path.conic_to((50.5602, 29.866), (50.3355, 30.8404), 0.707107);
    path.close();
    path.move_to((49.4091, 33.9552));
    path.quad_to((49.2264, 34.4531), (49.0236, 34.9431));
    path.conic_to((48.6412, 35.8671), (47.7172, 35.4846), 0.707107);
    path.conic_to((46.7932, 35.1022), (47.1757, 34.1782), 0.707107);
    path.quad_to((47.3629, 33.7259), (47.5315, 33.2663));
    path.conic_to((47.8759, 32.3275), (48.8147, 32.672), 0.707107);
    path.conic_to((49.7535, 33.0164), (49.4091, 33.9552), 0.707107);
    path.close();
    path.move_to((48.1514, 36.8328));
    path.quad_to((47.9191, 37.2871), (47.6694, 37.7318));
    path.conic_to((47.1797, 38.6038), (46.3078, 38.1141), 0.707107);
    path.conic_to((45.4359, 37.6244), (45.9256, 36.7525), 0.707107);
    path.quad_to((46.1562, 36.3418), (46.3705, 35.9226));
    path.conic_to((46.8256, 35.0321), (47.716, 35.4872), 0.707107);
    path.conic_to((48.6065, 35.9423), (48.1514, 36.8328), 0.707107);
    path.close();
    path.move_to((46.6245, 39.4354));
    path.line_to((46.5563, 39.537));
    path.quad_to((46.3146, 39.8955), (46.0624, 40.2438));
    path.conic_to((45.4761, 41.0539), (44.666, 40.4676), 0.707107);
    path.conic_to((43.8559, 39.8813), (44.4422, 39.0712), 0.707107);
    path.quad_to((44.6749, 38.7498), (44.8955, 38.4226));
    path.line_to((44.9637, 38.3211));
    path.conic_to((45.5209, 37.4907), (46.3513, 38.0479), 0.707107);
    path.conic_to((47.1817, 38.605), (46.6245, 39.4354), 0.707107);
    path.close();
    path.move_to((44.8168, 41.8314));
    path.quad_to((44.4832, 42.2241), (44.1342, 42.6034));
    path.conic_to((43.4572, 43.3394), (42.7212, 42.6623), 0.707107);
    path.conic_to((41.9853, 41.9853), (42.6623, 41.2494), 0.707107);
    path.quad_to((42.9845, 40.8992), (43.2924, 40.5366));
    path.conic_to((43.9398, 39.7745), (44.702, 40.4218), 0.707107);
    path.conic_to((45.4642, 41.0692), (44.8168, 41.8314), 0.707107);
    path.close();
    path.move_to((42.6505, 44.0908));
    path.quad_to((42.2577, 44.454), (41.8504, 44.8006));
    path.conic_to((41.0888, 45.4487), (40.4408, 44.6871), 0.707107);
    path.conic_to((39.7927, 43.9256), (40.5542, 43.2775), 0.707107);
    path.quad_to((40.9302, 42.9575), (41.2928, 42.6223));
    path.conic_to((42.027, 41.9434), (42.7059, 42.6777), 0.707107);
    path.conic_to((43.3848, 43.412), (42.6505, 44.0908), 0.707107);
    path.close();
    path.move_to((40.1383, 46.1384));
    path.quad_to((39.7073, 46.4471), (39.2641, 46.7378));
    path.conic_to((38.4281, 47.2865), (37.8795, 46.4504), 0.707107);
    path.conic_to((37.3308, 45.6143), (38.1669, 45.0657), 0.707107);
    path.quad_to((38.576, 44.7972), (38.9738, 44.5124));
    path.conic_to((39.7868, 43.9301), (40.369, 44.7432), 0.707107);
    path.conic_to((40.9513, 45.5562), (40.1383, 46.1384), 0.707107);
    path.close();
    path.move_to((37.4991, 47.7985));
    path.quad_to((37.0431, 48.0485), (36.5775, 48.2801));
    path.conic_to((35.6821, 48.7254), (35.2368, 47.83), 0.707107);
    path.conic_to((34.7915, 46.9346), (35.6869, 46.4893), 0.707107);
    path.quad_to((36.1167, 46.2755), (36.5376, 46.0448));
    path.conic_to((37.4145, 45.5641), (37.8952, 46.4409), 0.707107);
    path.conic_to((38.376, 47.3178), (37.4991, 47.7985), 0.707107);
    path.close();
    path.move_to((34.6651, 49.1368));
    path.quad_to((34.1756, 49.3328), (33.6785, 49.5089));
    path.conic_to((32.7358, 49.8427), (32.402, 48.9), 0.707107);
    path.conic_to((32.0682, 47.9574), (33.0109, 47.6236), 0.707107);
    path.quad_to((33.4697, 47.4611), (33.9216, 47.2801));
    path.conic_to((34.85, 46.9084), (35.2217, 47.8368), 0.707107);
    path.conic_to((35.5934, 48.7651), (34.6651, 49.1368), 0.707107);
    path.close();
    path.move_to((31.6557, 50.1337));
    path.quad_to((31.1425, 50.2696), (30.6243, 50.3844));
    path.conic_to((29.648, 50.6007), (29.4317, 49.6244), 0.707107);
    path.conic_to((29.2153, 48.6481), (30.1917, 48.4317), 0.707107);
    path.quad_to((30.6701, 48.3257), (31.1437, 48.2003));
    path.conic_to((32.1104, 47.9443), (32.3664, 48.911), 0.707107);
    path.conic_to((32.6223, 49.8777), (31.6557, 50.1337), 0.707107);
    path.close();
    path.move_to((28.5567, 50.7556));
    path.quad_to((28.0395, 50.827), (27.5198, 50.8776));
    path.conic_to((26.5245, 50.9745), (26.4276, 49.9792), 0.707107);
    path.conic_to((26.3307, 48.9839), (27.326, 48.887), 0.707107);
    path.quad_to((27.8056, 48.8403), (28.2831, 48.7744));
    path.conic_to((29.2737, 48.6376), (29.4105, 49.6282), 0.707107);
    path.conic_to((29.5473, 50.6188), (28.5567, 50.7556), 0.707107);
    path.close();
    path.move_to((25.4424, 50.9962));
    path.quad_to((24.9222, 51.0051), (24.4022, 50.9931));
    path.conic_to((23.4025, 50.9701), (23.4255, 49.9704), 0.707107);
    path.conic_to((23.4485, 48.9707), (24.4482, 48.9937), 0.707107);
    path.quad_to((24.9283, 49.0047), (25.4084, 48.9965));
    path.conic_to((26.4083, 48.9795), (26.4253, 49.9794), 0.707107);
    path.conic_to((26.4423, 50.9792), (25.4424, 50.9962), 0.707107);
    path.close();
    path.move_to((22.3065, 50.8601));
    path.quad_to((21.7885, 50.8062), (21.2732, 50.7315));
    path.conic_to((20.2835, 50.5882), (20.4268, 49.5985), 0.707107);
    path.conic_to((20.5702, 48.6088), (21.5599, 48.7522), 0.707107);
    path.quad_to((22.0355, 48.8211), (22.5136, 48.8709));
    path.conic_to((23.5083, 48.9745), (23.4047, 49.9691), 0.707107);
    path.conic_to((23.3011, 50.9637), (22.3065, 50.8601), 0.707107);
    path.close();
    path.move_to((19.2346, 50.3527));
    path.quad_to((18.7003, 50.2312), (18.1717, 50.0873));
    path.conic_to((17.2068, 49.8247), (17.4694, 48.8598), 0.707107);
    path.conic_to((17.732, 47.8949), (18.6969, 48.1575), 0.707107);
    path.quad_to((19.185, 48.2904), (19.6781, 48.4025));
    path.conic_to((20.6532, 48.6243), (20.4314, 49.5994), 0.707107);
    path.conic_to((20.2097, 50.5745), (19.2346, 50.3527), 0.707107);
    path.close();
    path.move_to((16.1149, 49.4347));
    path.quad_to((15.6161, 49.2533), (15.1251, 49.0517));
    path.conic_to((14.2, 48.6719), (14.5798, 47.7469), 0.707107);
    path.conic_to((14.9596, 46.8218), (15.8847, 47.2016), 0.707107);
    path.quad_to((16.3379, 47.3877), (16.7984, 47.5551));
    path.conic_to((17.7382, 47.8969), (17.3964, 48.8366), 0.707107);
    path.conic_to((17.0547, 49.7764), (16.1149, 49.4347), 0.707107);
    path.close();
    path.move_to((13.2313, 48.184));
    path.quad_to((12.776, 47.9529), (12.33, 47.704));
    path.conic_to((11.4568, 47.2167), (11.9441, 46.3434), 0.707107);
    path.conic_to((12.4314, 45.4702), (13.3046, 45.9575), 0.707107);
    path.quad_to((13.7162, 46.1872), (14.1365, 46.4006));
    path.conic_to((15.0282, 46.8532), (14.5756, 47.7449), 0.707107);
    path.conic_to((14.123, 48.6366), (13.2313, 48.184), 0.707107);
    path.close();
    path.move_to((10.6208, 46.6619));
    path.line_to((10.4641, 46.5571));
    path.quad_to((10.1333, 46.334), (9.81253, 46.1031));
    path.conic_to((9.00087, 45.519), (9.585, 44.7073), 0.707107);
    path.conic_to((10.1691, 43.8957), (10.9808, 44.4798), 0.707107);
    path.quad_to((11.2769, 44.6929), (11.5763, 44.8948));
    path.line_to((11.7329, 44.9996));
    path.conic_to((12.564, 45.5557), (12.008, 46.3868), 0.707107);
    path.conic_to((11.4519, 47.2179), (10.6208, 46.6619), 0.707107);
    path.close();
    path.move_to((8.22326, 44.8631));
    path.quad_to((7.82986, 44.5308), (7.44999, 44.1833));
    path.conic_to((6.71217, 43.5082), (7.38718, 42.7704), 0.707107);
    path.conic_to((8.06219, 42.0326), (8.8, 42.7076), 0.707107);
    path.quad_to((9.15066, 43.0284), (9.51375, 43.3351));
    path.conic_to((10.2777, 43.9804), (9.63248, 44.7443), 0.707107);
    path.conic_to((8.98724, 45.5083), (8.22326, 44.8631), 0.707107);
    path.close();
    path.move_to((5.95972, 42.705));
    path.quad_to((5.59577, 42.3136), (5.24823, 41.9076));
    path.conic_to((4.59793, 41.148), (5.3576, 40.4977), 0.707107);
    path.conic_to((6.11728, 39.8473), (6.76758, 40.607), 0.707107);
    path.quad_to((7.08843, 40.9818), (7.42436, 41.3431));
    path.conic_to((8.10532, 42.0754), (7.373, 42.7564), 0.707107);
    path.conic_to((6.64068, 43.4373), (5.95972, 42.705), 0.707107);
    path.close();
    path.move_to((3.90635, 40.2006));
    path.quad_to((3.59492, 39.7684), (3.30147, 39.3239));
    path.conic_to((2.75055, 38.4893), (3.58511, 37.9384), 0.707107);
    path.conic_to((4.41967, 37.3875), (4.97059, 38.222), 0.707107);
    path.quad_to((5.24148, 38.6324), (5.52894, 39.0313));
    path.conic_to((6.11358, 39.8426), (5.30228, 40.4272), 0.707107);
    path.conic_to((4.49099, 41.0119), (3.90635, 40.2006), 0.707107);
    path.close();
    path.move_to((2.23643, 37.5626));
    path.quad_to((1.98525, 37.1075), (1.75248, 36.6427));
    path.conic_to((1.30469, 35.7486), (2.19883, 35.3008), 0.707107);
    path.conic_to((3.09296, 34.853), (3.54076, 35.7471), 0.707107);
    path.quad_to((3.75563, 36.1762), (3.98747, 36.5963));
    path.conic_to((4.47065, 37.4718), (3.59513, 37.955), 0.707107);
    path.conic_to((2.71961, 38.4382), (2.23643, 37.5626), 0.707107);
    path.close();
    path.move_to((0.890647, 34.7334));
    path.quad_to((0.69328, 34.2445), (0.515902, 33.7481));
    path.conic_to((0.179435, 32.8064), (1.12113, 32.4699), 0.707107);
    path.conic_to((2.06282, 32.1335), (2.39929, 33.0752), 0.707107);
    path.quad_to((2.56303, 33.5334), (2.74521, 33.9847));
    path.conic_to((3.11957, 34.912), (2.19229, 35.2863), 0.707107);
    path.conic_to((1.26501, 35.6607), (0.890647, 34.7334), 0.707107);
    path.close();
    path.move_to((-0.114587, 31.7274));
    path.quad_to((-0.251922, 31.2147), (-0.368218, 30.6968));
    path.conic_to((-0.587327, 29.7211), (0.388373, 29.502), 0.707107);
    path.conic_to((1.36407, 29.2829), (1.58318, 30.2586), 0.707107);
    path.quad_to((1.69053, 30.7366), (1.8173, 31.2099));
    path.conic_to((2.07605, 32.1758), (1.1101, 32.4346), 0.707107);
    path.conic_to((0.144159, 32.6933), (-0.114587, 31.7274), 0.707107);
    path.close();
    path.move_to((-0.745485, 28.6291));
    path.quad_to((-0.818367, 28.112), (-0.870432, 27.5925));
    path.conic_to((-0.970142, 26.5974), (0.0248742, 26.4977), 0.707107);
    path.conic_to((1.01989, 26.398), (1.1196, 27.393), 0.707107);
    path.quad_to((1.16766, 27.8726), (1.23494, 28.3499));
    path.conic_to((1.37452, 29.3401), (0.384305, 29.4797), 0.707107);
    path.conic_to((-0.605905, 29.6193), (-0.745485, 28.6291), 0.707107);
    path.close();
    path.move_to((-0.994901, 25.515));
    path.quad_to((-1.00519, 24.9955), (-0.994722, 24.4761));
    path.conic_to((-0.97457, 23.4763), (0.0252273, 23.4964), 0.707107);
    path.conic_to((1.02502, 23.5166), (1.00487, 24.5164), 0.707107);
    path.quad_to((0.995207, 24.9959), (1.00471, 25.4754));
    path.conic_to((1.02451, 26.4752), (0.0247103, 26.495), 0.707107);
    path.conic_to((-0.975093, 26.5148), (-0.994901, 25.515), 0.707107);
    path.close();
    path.move_to((-0.867571, 22.3792));
    path.quad_to((-0.81506, 21.8609), (-0.741825, 21.3451));
    path.conic_to((-0.60125, 20.355), (0.38882, 20.4956), 0.707107);
    path.conic_to((1.37889, 20.6361), (1.23831, 21.6262), 0.707107);
    path.quad_to((1.17071, 22.1023), (1.12224, 22.5807));
    path.conic_to((1.02144, 23.5757), (0.026537, 23.4749), 0.707107);
    path.conic_to((-0.96837, 23.3741), (-0.867571, 22.3792), 0.707107);
    path.close();
    path.move_to((-0.369678, 19.3097));
    path.quad_to((-0.249693, 18.7748), (-0.107265, 18.2453));
    path.conic_to((0.152529, 17.2797), (1.11819, 17.5395), 0.707107);
    path.conic_to((2.08386, 17.7993), (1.82406, 18.7649), 0.707107);
    path.quad_to((1.69259, 19.2536), (1.58184, 19.7474));
    path.conic_to((1.36298, 20.7232), (0.387221, 20.5043), 0.707107);
    path.conic_to((-0.588536, 20.2855), (-0.369678, 19.3097), 0.707107);
    path.close();
    path.move_to((0.539863, 16.1851));
    path.quad_to((0.719962, 15.6854), (0.920307, 15.1934));
    path.conic_to((1.29748, 14.2673), (2.22362, 14.6445), 0.707107);
    path.conic_to((3.14976, 15.0216), (2.7726, 15.9478), 0.707107);
    path.quad_to((2.58765, 16.4019), (2.42141, 16.8632));
    path.conic_to((2.08237, 17.804), (1.1416, 17.4649), 0.707107);
    path.conic_to((0.200823, 17.1259), (0.539863, 16.1851), 0.707107);
    path.close();
    path.move_to((1.78353, 13.2955));
    path.quad_to((2.01364, 12.8391), (2.26151, 12.392));
    path.conic_to((2.74643, 11.5175), (3.62099, 12.0024), 0.707107);
    path.conic_to((4.49555, 12.4873), (4.01063, 13.3618), 0.707107);
    path.quad_to((3.78183, 13.7745), (3.56941, 14.1958));
    path.conic_to((3.11923, 15.0888), (2.22629, 14.6386), 0.707107);
    path.conic_to((1.33336, 14.1884), (1.78353, 13.2955), 0.707107);
    path.close();
    path.move_to((3.30083, 10.6771));
    path.line_to((3.44218, 10.4652));
    path.quad_to((3.6466, 10.1621), (3.85641, 9.86895));
    path.conic_to((4.43837, 9.05574), (5.25159, 9.6377), 0.707107);
    path.conic_to((6.0648, 10.2197), (5.48284, 11.0329), 0.707107);
    path.quad_to((5.28917, 11.3035), (5.10592, 11.5752));
    path.line_to((4.96457, 11.787));
    path.conic_to((4.4096, 12.6189), (3.57773, 12.0639), 0.707107);
    path.conic_to((2.74586, 11.509), (3.30083, 10.6771), 0.707107);
    path.close();
    path.move_to((5.0909, 8.27793));
    path.quad_to((5.42174, 7.88403), (5.76791, 7.50353));
    path.conic_to((6.44085, 6.76383), (7.18054, 7.43678), 0.707107);
    path.conic_to((7.92024, 8.10972), (7.24729, 8.84942), 0.707107);
    path.quad_to((6.92775, 9.20065), (6.62237, 9.56424));
    path.conic_to((5.97921, 10.33), (5.21348, 9.68682), 0.707107);
    path.conic_to((4.44774, 9.04367), (5.0909, 8.27793), 0.707107);
    path.close();
    path.move_to((7.24064, 6.0104));
    path.quad_to((7.63069, 5.64561), (8.03537, 5.29717));
    path.conic_to((8.79318, 4.64469), (9.44566, 5.40249), 0.707107);
    path.conic_to((10.0981, 6.16029), (9.34034, 6.81278), 0.707107);
    path.quad_to((8.96678, 7.13442), (8.60675, 7.47113));
    path.conic_to((7.87638, 8.15419), (7.19332, 7.42382), 0.707107);
    path.conic_to((6.51027, 6.69345), (7.24064, 6.0104), 0.707107);
    path.close();
    path.move_to((9.73726, 3.95128));
    path.quad_to((10.1706, 3.63704), (10.6165, 3.34092));
    path.conic_to((11.4496, 2.78771), (12.0028, 3.62075), 0.707107);
    path.conic_to((12.556, 4.4538), (11.7229, 5.007), 0.707107);
    path.quad_to((11.3113, 5.28035), (10.9113, 5.57041));
    path.conic_to((10.1018, 6.15744), (9.51472, 5.34787), 0.707107);
    path.conic_to((8.92769, 4.53831), (9.73726, 3.95128), 0.707107);
    path.close();
    path.move_to((12.374, 2.27153));
    path.quad_to((12.8282, 2.01921), (13.2921, 1.78522));
    path.conic_to((14.185, 1.33492), (14.6353, 2.22779), 0.707107);
    path.conic_to((15.0856, 3.12067), (14.1927, 3.57097), 0.707107);
    path.quad_to((13.7645, 3.78696), (13.3452, 4.01988));
    path.conic_to((12.471, 4.5055), (11.9854, 3.63132), 0.707107);
    path.conic_to((11.4998, 2.75715), (12.374, 2.27153), 0.707107);
    path.close();
    path.move_to((15.1984, 0.918296));
    path.quad_to((15.6866, 0.719602), (16.1824, 0.540851));
    path.conic_to((17.1231, 0.20171), (17.4623, 1.14245), 0.707107);
    path.conic_to((17.8014, 2.08318), (16.8607, 2.42232), 0.707107);
    path.quad_to((16.403, 2.58733), (15.9524, 2.77074));
    path.conic_to((15.0261, 3.14772), (14.6492, 2.2215), 0.707107);
    path.conic_to((14.2722, 1.29528), (15.1984, 0.918296), 0.707107);
    path.close();
    path.move_to((18.201, -0.0952874));
    path.quad_to((18.7132, -0.234075), (19.2308, -0.351842));
    path.conic_to((20.2058, -0.573734), (20.4277, 0.401338), 0.707107);
    path.conic_to((20.6496, 1.37641), (19.6745, 1.5983), 0.707107);
    path.quad_to((19.1968, 1.70701), (18.724, 1.83512));
    path.conic_to((17.7588, 2.09662), (17.4973, 1.13142), 0.707107);
    path.conic_to((17.2358, 0.166216), (18.201, -0.0952874), 0.707107);
    path.close();
    path.move_to((21.2986, -0.73518));
    path.quad_to((21.8155, -0.809526), (22.3349, -0.863052));
    path.conic_to((23.3297, -0.965552), (23.4322, 0.029181), 0.707107);
    path.conic_to((23.5347, 1.02391), (22.5399, 1.12641), 0.707107);
    path.quad_to((22.0604, 1.17582), (21.5833, 1.24445));
    path.conic_to((20.5935, 1.38681), (20.4511, 0.397), 0.707107);
    path.conic_to((20.3088, -0.592814), (21.2986, -0.73518), 0.707107);
    path.close();
    path.move_to((24.4124, -0.993361));
    path.quad_to((24.9312, -1.00509), (25.4501, -0.996107));
    path.conic_to((26.4499, -0.978799), (26.4326, 0.0210512), 0.707107);
    path.conic_to((26.4153, 1.0209), (25.4155, 1.00359), 0.707107);
    path.quad_to((24.9365, 0.995302), (24.4576, 1.00613));
    path.conic_to((23.4578, 1.02873), (23.4352, 0.0289853), 0.707107);
    path.conic_to((23.4126, -0.970759), (24.4124, -0.993361), 0.707107);
    path.close();
    path.move_to((27.5481, -0.87484));
    path.quad_to((28.0668, -0.823762), (28.583, -0.75194));
    path.conic_to((29.5734, -0.614138), (29.4356, 0.376322), 0.707107);
    path.conic_to((29.2978, 1.36678), (28.3074, 1.22898), 0.707107);
    path.quad_to((27.8309, 1.16268), (27.3521, 1.11553));
    path.conic_to((26.3569, 1.01753), (26.4549, 0.0223428), 0.707107);
    path.conic_to((26.5529, -0.972843), (27.5481, -0.87484), 0.707107);
    path.close();
    path.move_to((30.6151, -0.386432));
    path.quad_to((31.1507, -0.267954), (31.6809, -0.126991));
    path.conic_to((32.6473, 0.129965), (32.3904, 1.09639), 0.707107);
    path.conic_to((32.1334, 2.06281), (31.167, 1.80585), 0.707107);
    path.quad_to((30.6776, 1.67574), (30.1832, 1.56637));
    path.conic_to((29.2068, 1.35041), (29.4227, 0.374005), 0.707107);
    path.conic_to((29.6387, -0.602396), (30.6151, -0.386432), 0.707107);
    path.close();
    path.move_to((33.7445, 0.514616));
    path.quad_to((34.2452, 0.693421), (34.7381, 0.892536));
    path.conic_to((35.6653, 1.26708), (35.2908, 2.19429), 0.707107);
    path.conic_to((34.9162, 3.1215), (33.989, 2.74696), 0.707107);
    path.quad_to((33.534, 2.56316), (33.0718, 2.3981));
    path.conic_to((32.1301, 2.06177), (32.4664, 1.12003), 0.707107);
    path.conic_to((32.8027, 0.178285), (33.7445, 0.514616), 0.707107);
    path.close();
    path.move_to((36.6402, 1.7512));
    path.quad_to((37.0977, 1.98026), (37.5458, 2.22715));
    path.conic_to((38.4217, 2.70968), (37.9392, 3.58556), 0.707107);
    path.conic_to((37.4566, 4.46144), (36.5808, 3.97891), 0.707107);
    path.quad_to((36.1671, 3.75102), (35.7448, 3.53956));
    path.conic_to((34.8506, 3.09185), (35.2983, 2.19767), 0.707107);
    path.conic_to((35.746, 1.30349), (36.6402, 1.7512), 0.707107);
    path.close();
    path.move_to((39.2611, 3.26012));
    path.quad_to((39.4005, 3.35159), (39.539, 3.44501));
    path.quad_to((39.8091, 3.62717), (40.0746, 3.81611));
    path.conic_to((40.8893, 4.3959), (40.3096, 5.21067), 0.707107);
    path.conic_to((39.7298, 6.02543), (38.915, 5.44564), 0.707107);
    path.quad_to((38.67, 5.2713), (38.4206, 5.10309));
    path.quad_to((38.293, 5.017), (38.164, 4.9324));
    path.conic_to((37.3279, 4.38388), (37.8764, 3.54775), 0.707107);
    path.conic_to((38.4249, 2.71161), (39.2611, 3.26012), 0.707107);
    path.close();
    path.move_to((41.6673, 5.04503));
    path.quad_to((42.0618, 5.37449), (42.4428, 5.71927));
    path.conic_to((43.1844, 6.39015), (42.5135, 7.13171), 0.707107);
    path.conic_to((41.8426, 7.87327), (41.1011, 7.20239), 0.707107);
    path.quad_to((40.7493, 6.88414), (40.3852, 6.58004));
    path.conic_to((39.6177, 5.93899), (40.2588, 5.17149), 0.707107);
    path.conic_to((40.8998, 4.40399), (41.6673, 5.04503), 0.707107);
    path.close();
    path.move_to((43.9388, 7.1865));
    path.quad_to((44.3044, 7.57519), (44.6538, 7.97856));
    path.conic_to((45.3084, 8.73448), (44.5525, 9.38914), 0.707107);
    path.conic_to((43.7966, 10.0438), (43.1419, 9.28789), 0.707107);
    path.quad_to((42.8195, 8.91555), (42.482, 8.55677));
    path.conic_to((41.7969, 7.82836), (42.5253, 7.14322), 0.707107);
    path.conic_to((43.2537, 6.45808), (43.9388, 7.1865), 0.707107);
    path.close();
    path.move_to((46.0036, 9.6753));
    path.quad_to((46.3207, 10.1098), (46.6195, 10.5571));
    path.conic_to((47.175, 11.3886), (46.3435, 11.9441), 0.707107);
    path.conic_to((45.5119, 12.4996), (44.9564, 11.6681), 0.707107);
    path.quad_to((44.6806, 11.2552), (44.388, 10.8541));
    path.conic_to((43.7986, 10.0463), (44.6064, 9.45688), 0.707107);
    path.conic_to((45.4142, 8.86747), (46.0036, 9.6753), 0.707107);
    path.close();
    path.move_to((47.6932, 12.3107));
    path.quad_to((47.9467, 12.764), (48.1819, 13.2271));
    path.conic_to((48.6347, 14.1187), (47.7431, 14.5715), 0.707107);
    path.conic_to((46.8514, 15.0243), (46.3986, 14.1327), 0.707107);
    path.quad_to((46.1816, 13.7053), (45.9476, 13.2868));
    path.conic_to((45.4595, 12.414), (46.3323, 11.9259), 0.707107);
    path.conic_to((47.2051, 11.4379), (47.6932, 12.3107), 0.707107);
    path.close();
    path.move_to((49.0539, 15.1303));
    path.quad_to((49.2539, 15.6178), (49.434, 16.113));
    path.conic_to((49.7758, 17.0527), (48.836, 17.3946), 0.707107);
    path.conic_to((47.8963, 17.7364), (47.5545, 16.7966), 0.707107);
    path.quad_to((47.3882, 16.3395), (47.2036, 15.8895));
    path.conic_to((46.824, 14.9643), (47.7491, 14.5847), 0.707107);
    path.conic_to((48.6743, 14.2051), (49.0539, 15.1303), 0.707107);
    path.close();
    path.move_to((50.0758, 18.1294));
    path.quad_to((50.216, 18.6412), (50.3352, 19.1584));
    path.conic_to((50.5599, 20.1328), (49.5855, 20.3575), 0.707107);
    path.conic_to((48.6111, 20.5821), (48.3864, 19.6077), 0.707107);
    path.quad_to((48.2763, 19.1304), (48.1469, 18.6579));
    path.conic_to((47.8826, 17.6935), (48.8471, 17.4292), 0.707107);
    path.conic_to((49.8115, 17.165), (50.0758, 18.1294), 0.707107);
    path.close();
    path.move_to((50.7247, 21.2262));
    path.quad_to((50.8005, 21.743), (50.8555, 22.2623));
    path.conic_to((50.9607, 23.2568), (49.9663, 23.3621), 0.707107);
    path.conic_to((48.9719, 23.4673), (48.8666, 22.4729), 0.707107);
    path.quad_to((48.8158, 21.9935), (48.7458, 21.5165));
    path.conic_to((48.6007, 20.5271), (49.5901, 20.382), 0.707107);
    path.conic_to((50.5795, 20.2368), (50.7247, 21.2262), 0.707107);
    path.close();
    path.move_to((50.9916, 24.3398));
    path.quad_to((51.0048, 24.858), (50.9973, 25.3762));
    path.conic_to((50.9828, 26.3761), (49.9829, 26.3616), 0.707107);
    path.conic_to((48.983, 26.3472), (48.9975, 25.3473), 0.707107);
    path.quad_to((49.0044, 24.8687), (48.9923, 24.3906));
    path.conic_to((48.9669, 23.3909), (49.9665, 23.3655), 0.707107);
    path.conic_to((50.9662, 23.3401), (50.9916, 24.3398), 0.707107);
    path.close();
    path.move_to((50.8819, 27.4753));
    path.quad_to((50.8323, 27.9943), (50.7618, 28.511));
    path.conic_to((50.6268, 29.5018), (49.636, 29.3668), 0.707107);
    path.conic_to((48.6451, 29.2317), (48.7802, 28.2409), 0.707107);
    path.quad_to((48.8452, 27.7641), (48.891, 27.2849));
    path.conic_to((48.9862, 26.2894), (49.9816, 26.3846), 0.707107);
    path.conic_to((50.9771, 26.4798), (50.8819, 27.4753), 0.707107);
    path.close();
    path.move_to((50.4023, 30.5429));
    path.quad_to((50.2856, 31.0775), (50.1465, 31.607));
    path.conic_to((49.8924, 32.5742), (48.9252, 32.3201), 0.707107);
    path.conic_to((47.9581, 32.066), (48.2122, 31.0988), 0.707107);
    path.quad_to((48.3405, 30.6102), (48.4483, 30.1165));
    path.conic_to((48.6614, 29.1395), (49.6385, 29.3527), 0.707107);
    path.conic_to((50.6155, 29.5659), (50.4023, 30.5429), 0.707107);
    path.close();
    path.move_to((49.5104, 33.674));
    path.quad_to((49.3329, 34.1756), (49.1351, 34.6695));
    path.conic_to((48.7632, 35.5977), (47.8349, 35.2258), 0.707107);
    path.conic_to((46.9066, 34.854), (47.2785, 33.9257), 0.707107);
    path.quad_to((47.4612, 33.4697), (47.625, 33.0067));
    path.conic_to((47.9587, 32.064), (48.9014, 32.3977), 0.707107);
    path.conic_to((49.8441, 32.7313), (49.5104, 33.674), 0.707107);
    path.close();
    path.move_to((48.281, 36.5756));
    path.quad_to((48.053, 37.0342), (47.8071, 37.4835));
    path.conic_to((47.3269, 38.3607), (46.4497, 37.8805), 0.707107);
    path.conic_to((45.5725, 37.4004), (46.0527, 36.5232), 0.707107);
    path.quad_to((46.2797, 36.1085), (46.4901, 35.6852));
    path.conic_to((46.9353, 34.7898), (47.8307, 35.235), 0.707107);
    path.conic_to((48.7262, 35.6802), (48.281, 36.5756), 0.707107);
    path.close();
    path.move_to((46.7777, 39.2033));
    path.quad_to((46.6677, 39.3719), (46.555, 39.539));
    path.quad_to((46.3865, 39.7888), (46.2121, 40.0349));
    path.conic_to((45.6338, 40.8507), (44.818, 40.2724), 0.707107);
    path.conic_to((44.0021, 39.6942), (44.5804, 38.8783), 0.707107);
    path.quad_to((44.7413, 38.6513), (44.8969, 38.4206));
    path.quad_to((45.0008, 38.2665), (45.1025, 38.1107));
    path.conic_to((45.6488, 37.2731), (46.4864, 37.8194), 0.707107);
    path.conic_to((47.324, 38.3657), (46.7777, 39.2033), 0.707107);
    path.close();
    path.move_to((44.9527, 41.6701));
    path.quad_to((44.6177, 42.0709), (44.267, 42.458));
    path.conic_to((43.5955, 43.1991), (42.8545, 42.5276), 0.707107);
    path.conic_to((42.1135, 41.8561), (42.7849, 41.1151), 0.707107);
    path.quad_to((43.1087, 40.7578), (43.4178, 40.3878));
    path.conic_to((44.059, 39.6203), (44.8264, 40.2615), 0.707107);
    path.conic_to((45.5938, 40.9027), (44.9527, 41.6701), 0.707107);
    path.close();
    path.move_to((42.7884, 43.9624));
    path.quad_to((42.4083, 44.319), (42.014, 44.6602));
    path.conic_to((41.2578, 45.3146), (40.6034, 44.5585), 0.707107);
    path.conic_to((39.949, 43.8023), (40.7052, 43.1479), 0.707107);
    path.quad_to((41.0691, 42.833), (41.4201, 42.5037));
    path.conic_to((42.1494, 41.8196), (42.8336, 42.5489), 0.707107);
    path.conic_to((43.5178, 43.2782), (42.7884, 43.9624), 0.707107);
    path.close();
    path.move_to((40.3892, 45.9564));
    path.quad_to((39.9683, 46.2655), (39.5354, 46.5574));
    path.conic_to((38.7062, 47.1165), (38.1472, 46.2873), 0.707107);
    path.conic_to((37.5881, 45.4582), (38.4173, 44.8992), 0.707107);
    path.quad_to((38.8169, 44.6297), (39.2054, 44.3444));
    path.conic_to((40.0114, 43.7525), (40.6033, 44.5585), 0.707107);
    path.conic_to((41.1952, 45.3645), (40.3892, 45.9564), 0.707107);
    path.close();
    path.move_to((37.7543, 47.6568));
    path.quad_to((37.2977, 47.9138), (36.8312, 48.1522));
    path.conic_to((35.9407, 48.6072), (35.4857, 47.7167), 0.707107);
    path.conic_to((35.0306, 46.8263), (35.9211, 46.3712), 0.707107);
    path.quad_to((36.3518, 46.1511), (36.7732, 45.9139));
    path.conic_to((37.6446, 45.4234), (38.1351, 46.2948), 0.707107);
    path.conic_to((38.6257, 47.1662), (37.7543, 47.6568), 0.707107);
    path.close();
    path.move_to((34.9311, 49.0286));
    path.quad_to((34.4488, 49.2279), (33.9589, 49.4077));
    path.conic_to((33.0202, 49.7523), (32.6756, 48.8136), 0.707107);
    path.conic_to((32.331, 47.8748), (33.2698, 47.5302), 0.707107);
    path.quad_to((33.722, 47.3642), (34.1672, 47.1802));
    path.conic_to((35.0914, 46.7983), (35.4733, 47.7224), 0.707107);
    path.conic_to((35.8553, 48.6466), (34.9311, 49.0286), 0.707107);
    path.close();
    path.move_to((31.9824, 50.0449));
    path.quad_to((31.4774, 50.1857), (30.9668, 50.3061));
    path.conic_to((29.9935, 50.5355), (29.764, 49.5622), 0.707107);
    path.conic_to((29.5346, 48.5889), (30.5079, 48.3594), 0.707107);
    path.quad_to((30.9789, 48.2484), (31.4453, 48.1184));
    path.conic_to((32.4086, 47.8498), (32.6771, 48.8131), 0.707107);
    path.conic_to((32.9457, 49.7763), (31.9824, 50.0449), 0.707107);
    path.close();
    path.move_to((28.899, 50.706));
    path.quad_to((28.3834, 50.7842), (27.8652, 50.8416));
    path.conic_to((26.8713, 50.9518), (26.7611, 49.9579), 0.707107);
    path.conic_to((26.6509, 48.964), (27.6448, 48.8538), 0.707107);
    path.quad_to((28.1231, 48.8008), (28.599, 48.7286));
    path.conic_to((29.5877, 48.5786), (29.7377, 49.5673), 0.707107);
    path.conic_to((29.8877, 50.556), (28.899, 50.706), 0.707107);
    path.close();
    path.move_to((25.8106, 50.9874));
    path.quad_to((25.6321, 50.9929), (25.4537, 50.996));
    path.conic_to((24.4539, 51.0135), (24.4365, 50.0136), 0.707115);
    path.line_to((24.4251, 49.3638));
    path.conic_to((24.4077, 48.364), (25.4075, 48.3465), 0.707107);
    path.conic_to((26.4073, 48.3291), (26.4248, 49.3289), 0.707107);
    path.line_to((26.4361, 49.9787));
    path.line_to((25.4363, 49.9962));
    path.line_to((25.4189, 48.9963));
    path.quad_to((25.5836, 48.9935), (25.7482, 48.9883));
    path.conic_to((26.7477, 48.9571), (26.7789, 49.9567), 0.707107);
    path.conic_to((26.8101, 50.9562), (25.8106, 50.9874), 0.707107);
    path.close();
    path.move_to((24.3902, 47.3641));
    path.line_to((24.3728, 46.3643));
    path.conic_to((24.3553, 45.3645), (25.3551, 45.347), 0.707107);
    path.conic_to((26.355, 45.3295), (26.3724, 46.3294), 0.707107);
    path.line_to((26.3899, 47.3292));
    path.conic_to((26.4074, 48.3291), (25.4075, 48.3465), 0.707107);
    path.conic_to((24.4077, 48.364), (24.3902, 47.3641), 0.707107);
    path.close();
    path.move_to((24.3378, 44.3646));
    path.line_to((24.3204, 43.3648));
    path.conic_to((24.3029, 42.3649), (25.3028, 42.3475), 0.707107);
    path.conic_to((26.3026, 42.33), (26.3201, 43.3298), 0.707107);
    path.line_to((26.3375, 44.3297));
    path.conic_to((26.355, 45.3295), (25.3551, 45.347), 0.707107);
    path.conic_to((24.3553, 45.3645), (24.3378, 44.3646), 0.707107);
    path.close();
    path.move_to((24.2855, 41.3651));
    path.line_to((24.268, 40.3652));
    path.conic_to((24.2506, 39.3654), (25.2504, 39.3479), 0.707107);
    path.conic_to((26.2503, 39.3305), (26.2677, 40.3303), 0.707107);
    path.line_to((26.2852, 41.3302));
    path.conic_to((26.3026, 42.33), (25.3028, 42.3475), 0.707107);
    path.conic_to((24.3029, 42.3649), (24.2855, 41.3651), 0.707107);
    path.close();
    path.move_to((24.2331, 38.3655));
    path.line_to((24.2157, 37.3657));
    path.conic_to((24.1982, 36.3658), (25.1981, 36.3484), 0.707107);
    path.conic_to((26.1979, 36.3309), (26.2154, 37.3308), 0.707107);
    path.line_to((26.2328, 38.3306));
    path.conic_to((26.2503, 39.3305), (25.2504, 39.3479), 0.707107);
    path.conic_to((24.2506, 39.3654), (24.2331, 38.3655), 0.707107);
    path.close();
    path.move_to((24.1808, 35.366));
    path.line_to((24.1633, 34.3661));
    path.conic_to((24.1459, 33.3663), (25.1457, 33.3488), 0.707107);
    path.conic_to((26.1456, 33.3314), (26.163, 34.3312), 0.707107);
    path.line_to((26.1805, 35.3311));
    path.conic_to((26.1979, 36.3309), (25.1981, 36.3484), 0.707107);
    path.conic_to((24.1982, 36.3658), (24.1808, 35.366), 0.707107);
    path.close();
    path.move_to((24.1284, 32.3664));
    path.line_to((24.111, 31.3666));
    path.conic_to((24.0935, 30.3667), (25.0934, 30.3493), 0.707107);
    path.conic_to((26.0932, 30.3318), (26.1107, 31.3317), 0.707107);
    path.line_to((26.1281, 32.3315));
    path.conic_to((26.1456, 33.3314), (25.1457, 33.3488), 0.707107);
    path.conic_to((24.1459, 33.3663), (24.1284, 32.3664), 0.707107);
    path.close();
    path.move_to((24.0761, 29.3669));
    path.line_to((24.0586, 28.367));
    path.conic_to((24.0412, 27.3672), (25.041, 27.3497), 0.707107);
    path.conic_to((26.0409, 27.3323), (26.0583, 28.3321), 0.707107);
    path.line_to((26.0758, 29.332));
    path.conic_to((26.0932, 30.3318), (25.0934, 30.3493), 0.707107);
    path.conic_to((24.0935, 30.3667), (24.0761, 29.3669), 0.707107);
    path.close();
    path.move_to((24.0237, 26.3673));
    path.line_to((24.0063, 25.3675));
    path.conic_to((23.9888, 24.3676), (24.9887, 24.3502), 0.707107);
    path.conic_to((25.9885, 24.3327), (26.006, 25.3326), 0.707107);
    path.line_to((26.0234, 26.3324));
    path.conic_to((26.0409, 27.3323), (25.041, 27.3497), 0.707107);
    path.conic_to((24.0412, 27.3672), (24.0237, 26.3673), 0.707107);
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((25.0098, 23.1973));
    path.line_to((25.5689, 22.3682));
    path.conic_to((26.1281, 21.5392), (26.9572, 22.0984), 0.707107);
    path.conic_to((27.7862, 22.6576), (27.227, 23.4866), 0.707107);
    path.line_to((26.6678, 24.3156));
    path.conic_to((26.1086, 25.1447), (25.2796, 24.5855), 0.707107);
    path.conic_to((24.4506, 24.0263), (25.0098, 23.1973), 0.707107);
    path.close();
    path.move_to((26.6873, 20.7101));
    path.line_to((27.2465, 19.8811));
    path.conic_to((27.8057, 19.0521), (28.6348, 19.6113), 0.707107);
    path.conic_to((29.4638, 20.1704), (28.9046, 20.9995), 0.707107);
    path.line_to((28.3454, 21.8285));
    path.conic_to((27.7862, 22.6576), (26.9572, 22.0984), 0.707107);
    path.conic_to((26.1281, 21.5392), (26.6873, 20.7101), 0.707107);
    path.close();
    path.move_to((28.3649, 18.223));
    path.line_to((28.9241, 17.394));
    path.conic_to((29.4833, 16.565), (30.3123, 17.1241), 0.707107);
    path.conic_to((31.1414, 17.6833), (30.5822, 18.5124), 0.707107);
    path.line_to((30.023, 19.3414));
    path.conic_to((29.4638, 20.1704), (28.6348, 19.6113), 0.707107);
    path.conic_to((27.8057, 19.0521), (28.3649, 18.223), 0.707107);
    path.close();
    path.move_to((30.0425, 15.7359));
    path.line_to((30.6017, 14.9069));
    path.conic_to((31.1609, 14.0778), (31.9899, 14.637), 0.707107);
    path.conic_to((32.8189, 15.1962), (32.2598, 16.0253), 0.707107);
    path.line_to((31.7006, 16.8543));
    path.conic_to((31.1414, 17.6833), (30.3123, 17.1241), 0.707107);
    path.conic_to((29.4833, 16.565), (30.0425, 15.7359), 0.707107);
    path.close();
    path.move_to((31.7201, 13.2488));
    path.line_to((32.2793, 12.4198));
    path.conic_to((32.8385, 11.5907), (33.6675, 12.1499), 0.707107);
    path.conic_to((34.4965, 12.7091), (33.9373, 13.5381), 0.707107);
    path.line_to((33.3781, 14.3672));
    path.conic_to((32.8189, 15.1962), (31.9899, 14.637), 0.707107);
    path.conic_to((31.1609, 14.0778), (31.7201, 13.2488), 0.707107);
    path.close();
    path.move_to((33.3976, 10.7617));
    path.line_to((33.9568, 9.93265));
    path.conic_to((34.516, 9.10361), (35.3451, 9.6628), 0.707107);
    path.conic_to((36.1741, 10.222), (35.6149, 11.051), 0.707107);
    path.line_to((35.0557, 11.8801));
    path.conic_to((34.4965, 12.7091), (33.6675, 12.1499), 0.707107);
    path.conic_to((32.8385, 11.5907), (33.3976, 10.7617), 0.707107);
    path.close();
    path.move_to((35.0752, 8.27457));
    path.line_to((35.6344, 7.44554));
    path.conic_to((36.1936, 6.6165), (37.0226, 7.17569), 0.707107);
    path.conic_to((37.8517, 7.73488), (37.2925, 8.56392), 0.707107);
    path.line_to((36.7333, 9.39296));
    path.conic_to((36.1741, 10.222), (35.3451, 9.6628), 0.707107);
    path.conic_to((34.516, 9.10361), (35.0752, 8.27457), 0.707107);
    path.close();
    path.move_to((36.7528, 5.78746));
    path.line_to((37.312, 4.95842));
    path.conic_to((37.8712, 4.12939), (38.7002, 4.68858), 0.707107);
    path.conic_to((39.5293, 5.24777), (38.9701, 6.07681), 0.707107);
    path.line_to((38.4109, 6.90585));
    path.conic_to((37.8517, 7.73488), (37.0226, 7.17569), 0.707107);
    path.conic_to((36.1936, 6.6165), (36.7528, 5.78746), 0.707107);
    path.close();
    path.move_to((39.9447, 3.72429));
    path.quad_to((40.3524, 4.01069), (40.7489, 4.31248));
    path.conic_to((41.5445, 4.9182), (40.9388, 5.71387), 0.707107);
    path.conic_to((40.3331, 6.50955), (39.5374, 5.90383), 0.707107);
    path.quad_to((39.1714, 5.62521), (38.7951, 5.36088));
    path.conic_to((37.9768, 4.78608), (38.5516, 3.96779), 0.707107);
    path.conic_to((39.1264, 3.14949), (39.9447, 3.72429), 0.707107);
    path.close();
    path.move_to((42.3194, 5.60826));
    path.quad_to((42.707, 5.95446), (43.0804, 6.31583));
    path.conic_to((43.7991, 7.01122), (43.1037, 7.72985), 0.707107);
    path.conic_to((42.4083, 8.44848), (41.6896, 7.75308), 0.707107);
    path.quad_to((41.3448, 7.41944), (40.9871, 7.09992));
    path.conic_to((40.2413, 6.43379), (40.9074, 5.68796), 0.707107);
    path.conic_to((41.5735, 4.94212), (42.3194, 5.60826), 0.707107);
    path.close();
    path.move_to((44.5406, 7.84871));
    path.quad_to((44.8959, 8.25352), (45.2341, 8.67266));
    path.conic_to((45.862, 9.4509), (45.0838, 10.0789), 0.707107);
    path.conic_to((44.3056, 10.7068), (43.6776, 9.9286), 0.707107);
    path.quad_to((43.3654, 9.54174), (43.0374, 9.16805));
    path.conic_to((42.3778, 8.41649), (43.1293, 7.75682), 0.707107);
    path.conic_to((43.8809, 7.09715), (44.5406, 7.84871), 0.707107);
    path.close();
    path.move_to((46.528, 10.4211));
    path.quad_to((46.815, 10.8449), (47.0851, 11.2796));
    path.conic_to((47.6128, 12.129), (46.7633, 12.6567), 0.707107);
    path.conic_to((45.9139, 13.1844), (45.3862, 12.335), 0.707107);
    path.quad_to((45.1369, 11.9337), (44.872, 11.5426));
    path.conic_to((44.3113, 10.7146), (45.1393, 10.1538), 0.707107);
    path.conic_to((45.9673, 9.5931), (46.528, 10.4211), 0.707107);
    path.close();
    path.move_to((48.1056, 13.0782));
    path.quad_to((48.3449, 13.542), (48.5654, 14.015));
    path.conic_to((48.9879, 14.9213), (48.0816, 15.3438), 0.707107);
    path.conic_to((47.1752, 15.7663), (46.7527, 14.86), 0.707107);
    path.quad_to((46.5492, 14.4234), (46.3283, 13.9953));
    path.conic_to((45.8698, 13.1066), (46.7584, 12.6481), 0.707107);
    path.conic_to((47.6471, 12.1895), (48.1056, 13.0782), 0.707107);
    path.close();
    path.move_to((49.3755, 15.9538));
    path.quad_to((49.5594, 16.4493), (49.7229, 16.9516));
    path.conic_to((50.0325, 17.9025), (49.0816, 18.2121), 0.707107);
    path.conic_to((48.1307, 18.5216), (47.8212, 17.5707), 0.707107);
    path.quad_to((47.6702, 17.1069), (47.5005, 16.6497));
    path.conic_to((47.1526, 15.7122), (48.0901, 15.3642), 0.707107);
    path.conic_to((49.0276, 15.0163), (49.3755, 15.9538), 0.707107);
    path.close();
    path.move_to((50.2964, 18.9923));
    path.quad_to((50.4191, 19.5089), (50.5206, 20.0302));
    path.conic_to((50.7117, 21.0117), (49.7302, 21.2029), 0.707107);
    path.conic_to((48.7486, 21.394), (48.5575, 20.4125), 0.707107);
    path.quad_to((48.4638, 19.9313), (48.3505, 19.4544));
    path.conic_to((48.1194, 18.4815), (49.0924, 18.2504), 0.707107);
    path.conic_to((50.0653, 18.0193), (50.2964, 18.9923), 0.707107);
    path.close();
    path.move_to((50.8373, 22.0956));
    path.quad_to((50.8955, 22.6138), (50.933, 23.1341));
    path.conic_to((51.0047, 24.1315), (50.0073, 24.2033), 0.707107);
    path.conic_to((49.0099, 24.275), (48.9381, 23.2776), 0.707107);
    path.quad_to((48.9036, 22.7975), (48.8498, 22.3191));
    path.conic_to((48.7381, 21.3253), (49.7318, 21.2136), 0.707107);
    path.conic_to((50.7255, 21.1019), (50.8373, 22.0956), 0.707107);
    path.close();
    path.move_to((50.9992, 25.2099));
    path.quad_to((50.9949, 25.7358), (50.9694, 26.2608));
    path.conic_to((50.9209, 27.2596), (49.9221, 27.2111), 0.707107);
    path.conic_to((48.9233, 27.1626), (48.9718, 26.1638), 0.707107);
    path.quad_to((48.9953, 25.679), (48.9992, 25.1938));
    path.conic_to((49.0073, 24.1938), (50.0073, 24.2019), 0.707107);
    path.conic_to((51.0072, 24.21), (50.9992, 25.2099), 0.707107);
    path.close();
    path.move_to((50.7839, 28.3454));
    path.quad_to((50.7172, 28.8596), (50.63, 29.3708));
    path.conic_to((50.4619, 30.3565), (49.4761, 30.1884), 0.707107);
    path.conic_to((48.4903, 30.0203), (48.6584, 29.0346), 0.707107);
    path.quad_to((48.7389, 28.5627), (48.8005, 28.088));
    path.conic_to((48.9292, 27.0963), (49.9209, 27.225), 0.707107);
    path.conic_to((50.9126, 27.3537), (50.7839, 28.3454), 0.707107);
    path.close();
    path.move_to((50.1906, 31.437));
    path.quad_to((50.0558, 31.9646), (49.899, 32.4861));
    path.conic_to((49.611, 33.4438), (48.6534, 33.1558), 0.707107);
    path.conic_to((47.6957, 32.8679), (47.9837, 31.9103), 0.707107);
    path.quad_to((48.1284, 31.4289), (48.2528, 30.9418));
    path.conic_to((48.5004, 29.9729), (49.4693, 30.2205), 0.707107);
    path.conic_to((50.4382, 30.4681), (50.1906, 31.437), 0.707107);
    path.close();
    path.move_to((49.1978, 34.5114));
    path.quad_to((49.0051, 35.0016), (48.7927, 35.4837));
    path.conic_to((48.3895, 36.3988), (47.4744, 35.9956), 0.707107);
    path.conic_to((46.5593, 35.5923), (46.9625, 34.6772), 0.707107);
    path.quad_to((47.1586, 34.2323), (47.3364, 33.7797));
    path.conic_to((47.7023, 32.849), (48.6329, 33.2149), 0.707107);
    path.conic_to((49.5636, 33.5807), (49.1978, 34.5114), 0.707107);
    path.close();
    path.move_to((47.8852, 37.3397));
    path.quad_to((47.6449, 37.7853), (47.3876, 38.2211));
    path.conic_to((46.879, 39.0821), (46.018, 38.5736), 0.707107);
    path.conic_to((45.1569, 38.0651), (45.6655, 37.204), 0.707107);
    path.quad_to((45.903, 36.8018), (46.1248, 36.3906));
    path.conic_to((46.5993, 35.5103), (47.4796, 35.9849), 0.707107);
    path.conic_to((48.3598, 36.4595), (47.8852, 37.3397), 0.707107);
    path.close();
    path.move_to((46.3154, 39.8881));
    path.quad_to((46.0303, 40.2962), (45.7299, 40.693));
    path.conic_to((45.1264, 41.4903), (44.3291, 40.8867), 0.707107);
    path.conic_to((43.5318, 40.2831), (44.1353, 39.4858), 0.707107);
    path.quad_to((44.4126, 39.1195), (44.6757, 38.7428));
    path.conic_to((45.2483, 37.923), (46.0682, 38.4956), 0.707107);
    path.conic_to((46.888, 39.0682), (46.3154, 39.8881), 0.707107);
    path.close();
    path.move_to((44.4398, 42.2654));
    path.quad_to((44.095, 42.6536), (43.7349, 43.0278));
    path.conic_to((43.0415, 43.7484), (42.321, 43.055), 0.707107);
    path.conic_to((41.6004, 42.3616), (42.2938, 41.641), 0.707107);
    path.quad_to((42.6261, 41.2957), (42.9444, 40.9374));
    path.conic_to((43.6084, 40.1897), (44.3561, 40.8537), 0.707107);
    path.conic_to((45.1038, 41.5177), (44.4398, 42.2654), 0.707107);
    path.close();
    path.move_to((42.2075, 44.4911));
    path.quad_to((41.804, 44.8473), (41.3862, 45.1865));
    path.conic_to((40.6098, 45.8167), (39.9795, 45.0403), 0.707107);
    path.conic_to((39.3493, 44.2639), (40.1257, 43.6336), 0.707107);
    path.quad_to((40.5114, 43.3205), (40.8838, 42.9918));
    path.conic_to((41.6335, 42.3299), (42.2953, 43.0796), 0.707107);
    path.conic_to((42.9572, 43.8292), (42.2075, 44.4911), 0.707107);
    path.close();
    path.move_to((39.6379, 46.488));
    path.quad_to((39.2151, 46.776), (38.7814, 47.0471));
    path.conic_to((37.9334, 47.5771), (37.4034, 46.7292), 0.707107);
    path.conic_to((36.8733, 45.8812), (37.7213, 45.3511), 0.707107);
    path.quad_to((38.1217, 45.1009), (38.5119, 44.835));
    path.conic_to((39.3383, 44.2721), (39.9013, 45.0985), 0.707107);
    path.conic_to((40.4643, 45.925), (39.6379, 46.488), 0.707107);
    path.close();
    path.move_to((36.9864, 48.0722));
    path.quad_to((36.5234, 48.3127), (36.0513, 48.5344));
    path.conic_to((35.1461, 48.9595), (34.7211, 48.0543), 0.707107);
    path.conic_to((34.296, 47.1491), (35.2012, 46.7241), 0.707107);
    path.quad_to((35.6371, 46.5194), (36.0644, 46.2974));
    path.conic_to((36.9518, 45.8364), (37.4128, 46.7238), 0.707107);
    path.conic_to((37.8738, 47.6112), (36.9864, 48.0722), 0.707107);
    path.close();
    path.move_to((34.1153, 49.3498));
    path.quad_to((33.6206, 49.535), (33.1187, 49.6999));
    path.conic_to((32.1687, 50.0122), (31.8565, 49.0622), 0.707107);
    path.conic_to((31.5442, 48.1122), (32.4942, 47.7999), 0.707107);
    path.quad_to((32.9575, 47.6477), (33.4141, 47.4767));
    path.conic_to((34.3507, 47.1261), (34.7012, 48.0627), 0.707107);
    path.conic_to((35.0518, 48.9992), (34.1153, 49.3498), 0.707107);
    path.close();
    path.move_to((31.08, 50.2791));
    path.quad_to((30.5637, 50.4033), (30.0427, 50.5063));
    path.conic_to((29.0617, 50.7002), (28.8678, 49.7192), 0.707107);
    path.conic_to((28.6738, 48.7382), (29.6548, 48.5443), 0.707107);
    path.quad_to((30.1357, 48.4492), (30.6122, 48.3346));
    path.conic_to((31.5845, 48.1007), (31.8184, 49.073), 0.707107);
    path.conic_to((32.0522, 50.0453), (31.08, 50.2791), 0.707107);
    path.close();
    path.move_to((27.9769, 50.829));
    path.quad_to((27.4588, 50.8887), (26.9386, 50.9276));
    path.conic_to((25.9414, 51.0022), (25.8668, 50.005), 0.707107);
    path.conic_to((25.7923, 49.0078), (26.7895, 48.9332), 0.707107);
    path.quad_to((27.2696, 48.8973), (27.7479, 48.8422));
    path.conic_to((28.7413, 48.7277), (28.8558, 49.7211), 0.707107);
    path.conic_to((28.9703, 50.7145), (27.9769, 50.829), 0.707107);
    path.close();
    path.move_to((24.8625, 50.9996));
    path.quad_to((24.3373, 50.9969), (23.8128, 50.9729));
    path.conic_to((22.8138, 50.9272), (22.8595, 49.9283), 0.707107);
    path.conic_to((22.9051, 48.9293), (23.9041, 48.975), 0.707107);
    path.quad_to((24.3884, 48.9971), (24.8731, 48.9997));
    path.conic_to((25.8731, 49.005), (25.8678, 50.005), 0.707107);
    path.conic_to((25.8624, 51.0049), (24.8625, 50.9996), 0.707107);
    path.close();
    path.move_to((21.7268, 50.7931));
    path.quad_to((21.2121, 50.7278), (20.7005, 50.642));
    path.conic_to((19.7143, 50.4767), (19.8796, 49.4905), 0.707107);
    path.conic_to((20.045, 48.5042), (21.0312, 48.6696), 0.707107);
    path.quad_to((21.5036, 48.7488), (21.9786, 48.8091));
    path.conic_to((22.9707, 48.9349), (22.8448, 49.927), 0.707107);
    path.conic_to((22.7189, 50.919), (21.7268, 50.7931), 0.707107);
    path.close();
    path.move_to((18.6372, 50.2094));
    path.quad_to((18.1089, 50.0761), (17.5865, 49.9207));
    path.conic_to((16.628, 49.6356), (16.9132, 48.6771), 0.707107);
    path.conic_to((17.1983, 47.7186), (18.1568, 48.0037), 0.707107);
    path.quad_to((18.639, 48.1472), (19.1267, 48.2702));
    path.conic_to((20.0963, 48.515), (19.8516, 49.4846), 0.707107);
    path.conic_to((19.6068, 50.4542), (18.6372, 50.2094), 0.707107);
    path.close();
    path.move_to((15.5577, 49.2248));
    path.quad_to((15.0665, 49.0334), (14.5834, 48.8222));
    path.conic_to((13.6672, 48.4215), (14.0678, 47.5053), 0.707107);
    path.conic_to((14.4684, 46.589), (15.3847, 46.9897), 0.707107);
    path.quad_to((15.8306, 47.1846), (16.284, 47.3614));
    path.conic_to((17.2158, 47.7246), (16.8526, 48.6563), 0.707107);
    path.conic_to((16.4894, 49.588), (15.5577, 49.2248), 0.707107);
    path.close();
    path.move_to((12.7231, 47.9189));
    path.quad_to((12.2765, 47.6797), (11.8395, 47.4233));
    path.conic_to((10.9771, 46.9171), (11.4833, 46.0547), 0.707107);
    path.conic_to((11.9894, 45.1922), (12.8519, 45.6984), 0.707107);
    path.quad_to((13.2552, 45.9351), (13.6675, 46.156));
    path.conic_to((14.549, 46.6282), (14.0768, 47.5096), 0.707107);
    path.conic_to((13.6046, 48.3911), (12.7231, 47.9189), 0.707107);
    path.close();
    path.move_to((10.1686, 46.3548));
    path.quad_to((9.76024, 46.0712), (9.363, 45.7722));
    path.conic_to((8.56406, 45.1708), (9.16549, 44.3718), 0.707107);
    path.conic_to((9.76691, 43.5729), (10.5658, 44.1743), 0.707107);
    path.quad_to((10.9325, 44.4504), (11.3095, 44.7122));
    path.conic_to((12.1308, 45.2826), (11.5604, 46.1039), 0.707107);
    path.conic_to((10.9899, 46.9253), (10.1686, 46.3548), 0.707107);
    path.close();
    path.move_to((7.78853, 44.4876));
    path.quad_to((7.39972, 44.1442), (7.02492, 43.7855));
    path.conic_to((6.3024, 43.0942), (6.99374, 42.3717), 0.707107);
    path.conic_to((7.68509, 41.6492), (8.40761, 42.3405), 0.707107);
    path.quad_to((8.7536, 42.6715), (9.11249, 42.9885));
    path.conic_to((9.86201, 43.6505), (9.20003, 44.4), 0.707107);
    path.conic_to((8.53805, 45.1496), (7.78853, 44.4876), 0.707107);
    path.close();
    path.move_to((5.55855, 42.2635));
    path.quad_to((5.20148, 41.8614), (4.86131, 41.4449));
    path.conic_to((4.22883, 40.6703), (5.0034, 40.0378), 0.707107);
    path.conic_to((5.77797, 39.4053), (6.41046, 40.1799), 0.707107);
    path.quad_to((6.72443, 40.5644), (7.05403, 40.9356));
    path.conic_to((7.71802, 41.6833), (6.97028, 42.3473), 0.707107);
    path.conic_to((6.22254, 43.0113), (5.55855, 42.2635), 0.707107);
    path.close();
    path.move_to((3.55261, 39.6973));
    path.quad_to((3.26341, 39.2752), (2.99107, 38.8422));
    path.conic_to((2.45867, 37.9957), (3.30517, 37.4633), 0.707107);
    path.conic_to((4.15167, 36.9309), (4.68406, 37.7774), 0.707107);
    path.quad_to((4.93548, 38.1772), (5.20241, 38.5667));
    path.conic_to((5.76769, 39.3916), (4.94279, 39.9569), 0.707107);
    path.conic_to((4.11789, 40.5222), (3.55261, 39.6973), 0.707107);
    path.close();
    path.move_to((1.96145, 37.0509));
    path.quad_to((1.71975, 36.5889), (1.49677, 36.1175));
    path.conic_to((1.06917, 35.2135), (1.97315, 34.7859), 0.707107);
    path.conic_to((2.87712, 34.3583), (3.30471, 35.2623), 0.707107);
    path.quad_to((3.51053, 35.6974), (3.73364, 36.1239));
    path.conic_to((4.19714, 37.01), (3.31105, 37.4735), 0.707107);
    path.conic_to((2.42495, 37.937), (1.96145, 37.0509), 0.707107);
    path.close();
    path.move_to((0.676191, 34.1844));
    path.quad_to((0.489621, 33.6902), (0.323275, 33.189));
    path.conic_to((0.00831527, 32.2399), (0.95742, 31.9249), 0.707107);
    path.conic_to((1.90653, 31.6099), (2.22149, 32.559), 0.707107);
    path.quad_to((2.37504, 33.0218), (2.54726, 33.4779));
    path.conic_to((2.9005, 34.4134), (1.96497, 34.7666), 0.707107);
    path.conic_to((1.02943, 35.1199), (0.676191, 34.1844), 0.707107);
    path.close();
    path.move_to((-0.261658, 31.1521));
    path.quad_to((-0.387304, 30.6362), (-0.491779, 30.1156));
    path.conic_to((-0.68853, 29.1351), (0.291923, 28.9384), 0.707107);
    path.conic_to((1.27238, 28.7416), (1.46913, 29.7221), 0.707107);
    path.quad_to((1.56557, 30.2026), (1.68155, 30.6789));
    path.conic_to((1.91817, 31.6505), (0.946565, 31.8871), 0.707107);
    path.conic_to((-0.0250367, 32.1237), (-0.261658, 31.1521), 0.707107);
    path.close();
    path.move_to((-0.820549, 28.0495));
    path.quad_to((-0.881733, 27.5314), (-0.922089, 27.0113));
    path.conic_to((-0.999449, 26.0143), (-0.00244591, 25.9369), 0.707107);
    path.conic_to((0.994557, 25.8596), (1.07192, 26.8566), 0.707107);
    path.quad_to((1.10917, 27.3367), (1.16565, 27.8149));
    path.conic_to((1.28293, 28.808), (0.289834, 28.9253), 0.707107);
    path.conic_to((-0.703265, 29.0426), (-0.820549, 28.0495), 0.707107);
    path.close();
    path.move_to((-0.999918, 24.9349));
    path.quad_to((-0.998605, 24.4104), (-0.976138, 23.8863));
    path.conic_to((-0.933305, 22.8873), (0.0657772, 22.9301), 0.707107);
    path.conic_to((1.06486, 22.9729), (1.02203, 23.972), 0.707107);
    path.quad_to((1.00129, 24.4557), (1.00008, 24.9399));
    path.conic_to((0.997572, 25.9399), (-0.0024244, 25.9374), 0.707107);
    path.conic_to((-1.00242, 25.9349), (-0.999918, 24.9349), 0.707107);
    path.close();
    path.move_to((-0.802212, 21.7991));
    path.quad_to((-0.738311, 21.284), (-0.653903, 20.7719));
    path.conic_to((-0.491283, 19.7852), (0.495406, 19.9478), 0.707107);
    path.conic_to((1.48209, 20.1104), (1.31948, 21.0971), 0.707107);
    path.quad_to((1.24156, 21.5698), (1.18257, 22.0453));
    path.conic_to((1.05946, 23.0377), (0.0670681, 22.9146), 0.707107);
    path.conic_to((-0.925325, 22.7915), (-0.802212, 21.7991), 0.707107);
    path.close();
    path.move_to((-0.228066, 18.7115));
    path.quad_to((-0.096172, 18.1824), (0.0577899, 17.6593));
    path.conic_to((0.340124, 16.7), (1.29944, 16.9823), 0.707107);
    path.conic_to((2.25876, 17.2646), (1.97642, 18.2239), 0.707107);
    path.quad_to((1.8343, 18.7068), (1.71255, 19.1953));
    path.conic_to((1.47069, 20.1656), (0.50038, 19.9237), 0.707107);
    path.conic_to((-0.46993, 19.6819), (-0.228066, 18.7115), 0.707107);
    path.close();
    path.move_to((0.74831, 15.6269));
    path.quad_to((0.938539, 15.1347), (1.14857, 14.6506));
    path.conic_to((1.54662, 13.7333), (2.46398, 14.1313), 0.707107);
    path.conic_to((3.38135, 14.5294), (2.9833, 15.4467), 0.707107);
    path.quad_to((2.78942, 15.8936), (2.61382, 16.3479));
    path.conic_to((2.25331, 17.2806), (1.32056, 16.9201), 0.707107);
    path.conic_to((0.387801, 16.5596), (0.74831, 15.6269), 0.707107);
    path.close();
    path.move_to((2.04744, 12.7861));
    path.quad_to((2.28569, 12.3384), (2.5412, 11.9003));
    path.conic_to((3.04504, 11.0365), (3.90884, 11.5403), 0.707107);
    path.conic_to((4.77264, 12.0442), (4.26881, 12.908), 0.707107);
    path.quad_to((4.03293, 13.3123), (3.81302, 13.7256));
    path.conic_to((3.34325, 14.6084), (2.46046, 14.1386), 0.707107);
    path.conic_to((1.57767, 13.6689), (2.04744, 12.7861), 0.707107);
    path.close();
    path.move_to((3.60589, 10.2253));
    path.quad_to((3.88812, 9.81661), (4.18576, 9.419));
    path.conic_to((4.78503, 8.61845), (5.58558, 9.21772), 0.707107);
    path.conic_to((6.38613, 9.81699), (5.78686, 10.6175), 0.707107);
    path.quad_to((5.51211, 10.9846), (5.25159, 11.3618));
    path.conic_to((4.68333, 12.1847), (3.86048, 11.6164), 0.707107);
    path.conic_to((3.03763, 11.0481), (3.60589, 10.2253), 0.707107);
    path.close();
    path.move_to((5.46482, 7.84259));
    path.quad_to((5.80682, 7.4532), (6.16407, 7.07773));
    path.conic_to((6.85339, 6.35327), (7.57785, 7.04259), 0.707107);
    path.conic_to((8.30231, 7.73191), (7.61299, 8.45636), 0.707107);
    path.quad_to((7.28322, 8.80295), (6.96752, 9.16239));
    path.conic_to((6.30762, 9.91375), (5.55627, 9.25385), 0.707107);
    path.conic_to((4.80492, 8.59395), (5.46482, 7.84259), 0.707107);
    path.close();
    path.move_to((7.68062, 5.60827));
    path.quad_to((8.08142, 5.25031), (8.49666, 4.90921));
    path.conic_to((9.26938, 4.27447), (9.90412, 5.04719), 0.707107);
    path.conic_to((10.5389, 5.81992), (9.76614, 6.45466), 0.707107);
    path.quad_to((9.38285, 6.76951), (9.01289, 7.09994));
    path.conic_to((8.26705, 7.76607), (7.60092, 7.02024), 0.707107);
    path.conic_to((6.93479, 6.2744), (7.68062, 5.60827), 0.707107);
    path.close();
    path.move_to((10.2392, 3.59627));
    path.quad_to((10.6626, 3.30433), (11.0971, 3.02935));
    path.conic_to((11.9421, 2.49463), (12.4768, 3.33965), 0.707107);
    path.conic_to((13.0116, 4.18467), (12.1666, 4.7194), 0.707107);
    path.quad_to((11.7654, 4.97322), (11.3747, 5.24271));
    path.conic_to((10.5515, 5.81043), (9.98373, 4.98721), 0.707107);
    path.conic_to((9.41601, 4.16399), (10.2392, 3.59627), 0.707107);
    path.close();
    path.move_to((12.8847, 1.99524));
    path.quad_to((13.3459, 1.75234), (13.8165, 1.52812));
    path.conic_to((14.7193, 1.09799), (15.1494, 2.00075), 0.707107);
    path.conic_to((15.5795, 2.90352), (14.6768, 3.33365), 0.707107);
    path.quad_to((14.2424, 3.54063), (13.8166, 3.76484));
    path.conic_to((12.9318, 4.23081), (12.4658, 3.34601), 0.707107);
    path.conic_to((11.9999, 2.46122), (12.8847, 1.99524), 0.707107);
    path.close();
    path.move_to((15.7467, 0.702339));
    path.quad_to((16.2402, 0.514409), (16.7409, 0.346672));
    path.conic_to((17.6891, 0.029011), (18.0067, 0.977215), 0.707107);
    path.conic_to((18.3244, 1.92542), (17.3762, 2.24308), 0.707107);
    path.quad_to((16.914, 2.39792), (16.4585, 2.57139));
    path.conic_to((15.524, 2.92729), (15.1681, 1.99276), 0.707107);
    path.conic_to((14.8122, 1.05824), (15.7467, 0.702339), 0.707107);
    path.close();
    path.move_to((18.7758, -0.24399));
    path.quad_to((19.2913, -0.371107), (19.8116, -0.477061));
    path.conic_to((20.7915, -0.676608), (20.9911, 0.303281), 0.707107);
    path.conic_to((21.1906, 1.28317), (20.2107, 1.48272), 0.707107);
    path.quad_to((19.7304, 1.58052), (19.2546, 1.69785));
    path.conic_to((18.2836, 1.93725), (18.0443, 0.966329), 0.707107);
    path.conic_to((17.8049, -0.00459272), (18.7758, -0.24399), 0.707107);
    path.close();
    path.move_to((21.878, -0.811882));
    path.quad_to((22.396, -0.874528), (22.916, -0.916348));
    path.conic_to((23.9128, -0.996504), (23.993, 0.000278629), 0.707107);
    path.conic_to((24.0731, 0.997061), (23.0764, 1.07722), 0.707107);
    path.quad_to((22.5963, 1.11582), (22.1182, 1.17365));
    path.conic_to((21.1254, 1.29372), (21.0053, 0.300958), 0.707107);
    path.conic_to((20.8853, -0.691807), (21.878, -0.811882), 0.707107);
    path.close();
    path.move_to((24.9926, -0.999999));
    path.quad_to((25.5166, -1.00015), (26.0401, -0.979188));
    path.conic_to((27.0393, -0.939179), (26.9992, 0.0600199), 0.707107);
    path.conic_to((26.9592, 1.05922), (25.96, 1.01921), 0.707107);
    path.quad_to((25.4768, 0.999863), (24.9932, 1.0));
    path.conic_to((23.9932, 1.00029), (23.9929, 0.000287339), 0.707107);
    path.conic_to((23.9926, -0.999713), (24.9926, -0.999999), 0.707107);
    path.close();
    path.move_to((28.1286, -0.811081));
    path.quad_to((28.6441, -0.748593), (29.1567, -0.665572));
    path.conic_to((30.1439, -0.505698), (29.984, 0.48144), 0.707107);
    path.conic_to((29.8241, 1.46858), (28.837, 1.3087), 0.707107);
    path.quad_to((28.3638, 1.23207), (27.8879, 1.17439));
    path.conic_to((26.8952, 1.05406), (27.0155, 0.0613233), 0.707107);
    path.conic_to((27.1359, -0.931411), (28.1286, -0.811081), 0.707107);
    path.close();
    path.move_to((31.214, -0.246499));
    path.quad_to((31.7439, -0.116076), (32.2679, 0.0364622));
    path.conic_to((33.228, 0.315996), (32.9485, 1.27613), 0.707107);
    path.conic_to((32.6689, 2.23627), (31.7088, 1.95673), 0.707107);
    path.quad_to((31.2252, 1.81593), (30.736, 1.69554));
    path.conic_to((29.765, 1.45654), (30.004, 0.48552), 0.707107);
    path.conic_to((30.243, -0.485499), (31.214, -0.246499), 0.707107);
    path.close();
    path.move_to((34.3038, 0.721629));
    path.quad_to((34.797, 0.910612), (35.282, 1.11946));
    path.conic_to((36.2005, 1.51493), (35.805, 2.43341), 0.707107);
    path.conic_to((35.4096, 3.35189), (34.4911, 2.95642), 0.707107);
    path.quad_to((34.0434, 2.76365), (33.5881, 2.5892));
    path.conic_to((32.6543, 2.23137), (33.0122, 1.29758), 0.707107);
    path.conic_to((33.37, 0.363796), (34.3038, 0.721629), 0.707107);
    path.close();
    path.move_to((37.1508, 2.01396));
    path.quad_to((37.5996, 2.2512), (38.0388, 2.50578));
    path.conic_to((38.904, 3.00727), (38.4025, 3.87244), 0.707107);
    path.conic_to((37.901, 4.7376), (37.0358, 4.23612), 0.707107);
    path.quad_to((36.6304, 4.00111), (36.2161, 3.78211));
    path.conic_to((35.332, 3.31476), (35.7994, 2.43069), 0.707107);
    path.conic_to((36.2667, 1.54661), (37.1508, 2.01396), 0.707107);
    path.close();
    path.move_to((39.718, 3.56681));
    path.quad_to((40.1269, 3.84765), (40.5249, 4.14392));
    path.conic_to((41.3271, 4.74104), (40.73, 5.54319), 0.707107);
    path.conic_to((40.1329, 6.34535), (39.3307, 5.74823), 0.707107);
    path.quad_to((38.9634, 5.47478), (38.5858, 5.21552));
    path.conic_to((37.7615, 4.64945), (38.3275, 3.82509), 0.707107);
    path.conic_to((38.8936, 3.00074), (39.718, 3.56681), 0.707107);
    path.close();
    path.move_to((42.1033, 5.41741));
    path.quad_to((42.4933, 5.75802), (42.8694, 6.11388));
    path.conic_to((43.5958, 6.80115), (42.9085, 7.52755), 0.707107);
    path.conic_to((42.2212, 8.25394), (41.4948, 7.56667), 0.707107);
    path.quad_to((41.1476, 7.23817), (40.7876, 6.92375));
    path.conic_to((40.0345, 6.26593), (40.6923, 5.51275), 0.707107);
    path.conic_to((41.3501, 4.75958), (42.1033, 5.41741), 0.707107);
    path.close();
    path.move_to((44.3419, 7.62498));
    path.quad_to((44.7007, 8.02444), (45.0428, 8.43835));
    path.conic_to((45.6797, 9.20922), (44.9089, 9.84622), 0.707107);
    path.conic_to((44.138, 10.4832), (43.501, 9.71234), 0.707107);
    path.quad_to((43.1852, 9.3302), (42.854, 8.96151));
    path.conic_to((42.1858, 8.21759), (42.9297, 7.54932), 0.707107);
    path.conic_to((43.6736, 6.88106), (44.3419, 7.62498), 0.707107);
    path.close();
    path.move_to((46.3599, 10.1759));
    path.quad_to((46.6546, 10.6005), (46.9322, 11.0366));
    path.conic_to((47.4693, 11.8801), (46.6257, 12.4172), 0.707107);
    path.conic_to((45.7822, 12.9542), (45.2451, 12.1107), 0.707107);
    path.quad_to((44.9889, 11.7082), (44.7168, 11.3162));
    path.conic_to((44.1467, 10.4947), (44.9682, 9.92452), 0.707107);
    path.conic_to((45.7897, 9.35435), (46.3599, 10.1759), 0.707107);
    path.close();
    path.move_to((47.9708, 12.8204));
    path.quad_to((48.2149, 13.2808), (48.4403, 13.7506));
    path.conic_to((48.873, 14.6521), (47.9715, 15.0848), 0.707107);
    path.conic_to((47.0699, 15.5174), (46.6372, 14.6159), 0.707107);
    path.quad_to((46.4291, 14.1822), (46.2038, 13.7573));
    path.conic_to((45.7354, 12.8738), (46.6188, 12.4054), 0.707107);
    path.conic_to((47.5023, 11.9369), (47.9708, 12.8204), 0.707107);
    path.close();
    path.move_to((49.2713, 15.6778));
    path.quad_to((49.4606, 16.1706), (49.6297, 16.6708));
    path.conic_to((49.9501, 17.6181), (49.0028, 17.9384), 0.707107);
    path.conic_to((48.0555, 18.2588), (47.7351, 17.3115), 0.707107);
    path.quad_to((47.5791, 16.8499), (47.4043, 16.3949));
    path.conic_to((47.0458, 15.4614), (47.9793, 15.1029), 0.707107);
    path.conic_to((48.9128, 14.7443), (49.2713, 15.6778), 0.707107);
    path.close();
    path.move_to((50.2261, 18.7037));
    path.quad_to((50.3547, 19.2188), (50.4621, 19.7388));
    path.conic_to((50.6645, 20.7182), (49.6852, 20.9205), 0.707107);
    path.conic_to((48.7059, 21.1229), (48.5035, 20.1436), 0.707107);
    path.quad_to((48.4043, 19.6636), (48.2856, 19.1881));
    path.conic_to((48.0435, 18.2178), (49.0137, 17.9757), 0.707107);
    path.conic_to((49.984, 17.7335), (50.2261, 18.7037), 0.707107);
    path.close();
    path.move_to((50.803, 21.8055));
    path.quad_to((50.8671, 22.3234), (50.9104, 22.8434));
    path.conic_to((50.9934, 23.8399), (49.9968, 23.9229), 0.707107);
    path.conic_to((49.0002, 24.0058), (48.9173, 23.0093), 0.707107);
    path.quad_to((48.8773, 22.5293), (48.8182, 22.0513));
    path.conic_to((48.6953, 21.0588), (49.6877, 20.936), 0.707107);
    path.conic_to((50.6801, 20.8131), (50.803, 21.8055), 0.707107);
    path.close();
    path.move_to((50.9999, 24.9202));
    path.quad_to((51.0015, 25.4434), (50.982, 25.9664));
    path.conic_to((50.9449, 26.9657), (49.9456, 26.9286), 0.707107);
    path.conic_to((48.9463, 26.8914), (48.9834, 25.8921), 0.707107);
    path.quad_to((49.0014, 25.4094), (48.9999, 24.9263));
    path.conic_to((48.9968, 23.9263), (49.9968, 23.9232), 0.707107);
    path.conic_to((50.9968, 23.9202), (50.9999, 24.9202), 0.707107);
    path.close();
    path.move_to((50.8198, 28.0562));
    path.quad_to((50.7587, 28.5721), (50.677, 29.0852));
    path.conic_to((50.5199, 30.0728), (49.5323, 29.9157), 0.707107);
    path.conic_to((48.5448, 29.7586), (48.7019, 28.771), 0.707107);
    path.quad_to((48.7772, 28.2974), (48.8336, 27.8211));
    path.conic_to((48.9512, 26.8281), (49.9442, 26.9456), 0.707107);
    path.conic_to((50.9373, 27.0632), (50.8198, 28.0562), 0.707107);
    path.close();
    path.move_to((50.2647, 31.1395));
    path.quad_to((50.1358, 31.6701), (49.9847, 32.1949));
    path.conic_to((49.7079, 33.1558), (48.747, 32.8791), 0.707107);
    path.conic_to((47.786, 32.6024), (48.0628, 31.6414), 0.707107);
    path.quad_to((48.2022, 31.1571), (48.3213, 30.6672));
    path.conic_to((48.5574, 29.6955), (49.5291, 29.9317), 0.707107);
    path.conic_to((50.5009, 30.1678), (50.2647, 31.1395), 0.707107);
    path.close();
    path.move_to((49.3049, 34.2343));
    path.quad_to((49.1171, 34.7285), (48.9095, 35.2145));
    path.conic_to((48.5166, 36.1341), (47.597, 35.7412), 0.707107);
    path.conic_to((46.6774, 35.3483), (47.0703, 34.4288), 0.707107);
    path.quad_to((47.262, 33.9801), (47.4353, 33.524));
    path.conic_to((47.7904, 32.5892), (48.7252, 32.9444), 0.707107);
    path.conic_to((49.66, 33.2995), (49.3049, 34.2343), 0.707107);
    path.close();
    path.move_to((48.0194, 37.0875));
    path.quad_to((47.7831, 37.5374), (47.5295, 37.9777));
    path.conic_to((47.0304, 38.8443), (46.1638, 38.3451), 0.707107);
    path.conic_to((45.2973, 37.846), (45.7965, 36.9795), 0.707107);
    path.quad_to((46.0306, 36.5729), (46.2487, 36.1577));
    path.conic_to((46.7136, 35.2723), (47.5989, 35.7372), 0.707107);
    path.conic_to((48.4843, 36.2021), (48.0194, 37.0875), 0.707107);
    path.close();
    path.move_to((46.4721, 39.6612));
    path.quad_to((46.1926, 40.0705), (45.8977, 40.4688));
    path.conic_to((45.3028, 41.2726), (44.499, 40.6776), 0.707107);
    path.conic_to((43.6953, 40.0827), (44.2902, 39.2789), 0.707107);
    path.quad_to((44.5624, 38.9112), (44.8204, 38.5334));
    path.conic_to((45.3843, 37.7075), (46.2101, 38.2714), 0.707107);
    path.conic_to((47.036, 38.8353), (46.4721, 39.6612), 0.707107);
    path.close();
    path.move_to((44.6298, 42.0491));
    path.quad_to((44.2906, 42.4396), (43.9361, 42.8164));
    path.conic_to((43.2509, 43.5447), (42.5226, 42.8595), 0.707107);
    path.conic_to((41.7942, 42.1742), (42.4795, 41.4459), 0.707107);
    path.quad_to((42.8067, 41.0981), (43.1198, 40.7376));
    path.conic_to((43.7756, 39.9826), (44.5306, 40.6383), 0.707107);
    path.conic_to((45.2856, 41.2941), (44.6298, 42.0491), 0.707107);
    path.close();
    path.move_to((42.4305, 44.2919));
    path.quad_to((42.0324, 44.6516), (41.6198, 44.9946));
    path.conic_to((40.8507, 45.6338), (40.2115, 44.8648), 0.707107);
    path.conic_to((39.5723, 44.0958), (40.3413, 43.4566), 0.707107);
    path.quad_to((40.7222, 43.1399), (41.0897, 42.8079));
    path.conic_to((41.8317, 42.1375), (42.5021, 42.8795), 0.707107);
    path.conic_to((43.1725, 43.6215), (42.4305, 44.2919), 0.707107);
    path.close();
    path.move_to((39.8873, 46.3159));
    path.quad_to((39.4613, 46.6134), (39.0238, 46.8936));
    path.conic_to((38.1818, 47.433), (37.6424, 46.5909), 0.707107);
    path.conic_to((37.103, 45.7489), (37.9451, 45.2095), 0.707107);
    path.quad_to((38.3489, 44.9508), (38.7421, 44.6763));
    path.conic_to((39.5619, 44.1037), (40.1345, 44.9235), 0.707107);
    path.conic_to((40.7071, 45.7434), (39.8873, 46.3159), 0.707107);
    path.close();
    path.move_to((37.2437, 47.9367));
    path.quad_to((36.7842, 48.182), (36.3153, 48.4086));
    path.conic_to((35.415, 48.8439), (34.9797, 47.9435), 0.707107);
    path.conic_to((34.5445, 47.0432), (35.4449, 46.608), 0.707107);
    path.quad_to((35.8778, 46.3987), (36.3019, 46.1723));
    path.conic_to((37.1841, 45.7014), (37.655, 46.5836), 0.707107);
    path.conic_to((38.1259, 47.4658), (37.2437, 47.9367), 0.707107);
    path.close();
    path.move_to((34.3909, 49.2448));
    path.quad_to((33.8988, 49.4354), (33.3992, 49.606));
    path.conic_to((32.4528, 49.929), (32.1298, 48.9826), 0.707107);
    path.conic_to((31.8068, 48.0362), (32.7532, 47.7132), 0.707107);
    path.quad_to((33.2142, 47.5558), (33.6685, 47.3798));
    path.conic_to((34.601, 47.0186), (34.9622, 47.9511), 0.707107);
    path.conic_to((35.3234, 48.8836), (34.3909, 49.2448), 0.707107);
    path.close();
    path.move_to((31.3682, 50.208));
    path.quad_to((30.8535, 50.3381), (30.3338, 50.447));
    path.conic_to((29.3551, 50.6521), (29.15, 49.6734), 0.707107);
    path.conic_to((28.9448, 48.6947), (29.9236, 48.4895), 0.707107);
    path.quad_to((30.4033, 48.389), (30.8784, 48.269));
    path.conic_to((31.8479, 48.024), (32.0929, 48.9936), 0.707107);
    path.conic_to((32.3378, 49.9631), (31.3682, 50.208), 0.707107);
    path.close();
    path.move_to((28.2669, 50.7939));
    path.quad_to((27.7491, 50.8595), (27.2292, 50.9043));
    path.conic_to((26.2329, 50.99), (26.1472, 49.9937), 0.707107);
    path.conic_to((26.0615, 48.9973), (27.0578, 48.9116), 0.707107);
    path.quad_to((27.5378, 48.8703), (28.0156, 48.8098));
    path.conic_to((29.0077, 48.6841), (29.1334, 49.6762), 0.707107);
    path.conic_to((29.259, 50.6683), (28.2669, 50.7939), 0.707107);
    path.close();
    path.move_to((25.1523, 50.9996));
    path.quad_to((24.6297, 51.0026), (24.1072, 50.9847));
    path.conic_to((23.1078, 50.9503), (23.1422, 49.9509), 0.707107);
    path.conic_to((23.1765, 48.9515), (24.1759, 48.9858), 0.707107);
    path.quad_to((24.658, 49.0024), (25.1406, 48.9996));
    path.conic_to((26.1406, 48.9937), (26.1464, 49.9937), 0.707107);
    path.conic_to((26.1523, 50.9937), (25.1523, 50.9996), 0.707107);
    path.close();
    path.move_to((22.0162, 50.8282));
    path.quad_to((21.4999, 50.7686), (20.9863, 50.6883));
    path.conic_to((19.9983, 50.5339), (20.1527, 49.5459), 0.707107);
    path.conic_to((20.307, 48.5579), (21.295, 48.7123), 0.707107);
    path.quad_to((21.7691, 48.7864), (22.2457, 48.8414));
    path.conic_to((23.2391, 48.9562), (23.1243, 49.9496), 0.707107);
    path.conic_to((23.0096, 50.943), (22.0162, 50.8282), 0.707107);
    path.close();
    path.move_to((18.9351, 50.2827));
    path.quad_to((18.4037, 50.1553), (17.8782, 50.0056));
    path.conic_to((16.9164, 49.7317), (17.1904, 48.7699), 0.707107);
    path.conic_to((17.4643, 47.8082), (18.426, 48.0821), 0.707107);
    path.quad_to((18.9112, 48.2203), (19.4016, 48.3379));
    path.conic_to((20.374, 48.5712), (20.1408, 49.5436), 0.707107);
    path.conic_to((19.9075, 50.516), (18.9351, 50.2827), 0.707107);
    path.close();
    path.move_to((15.8352, 49.3312));
    path.quad_to((15.3403, 49.1448), (14.8531, 48.9383));
    path.conic_to((13.9324, 48.548), (14.3227, 47.6273), 0.707107);
    path.conic_to((14.713, 46.7066), (15.6337, 47.0969), 0.707107);
    path.quad_to((16.0832, 47.2874), (16.5402, 47.4596));
    path.conic_to((17.476, 47.812), (17.1235, 48.7479), 0.707107);
    path.conic_to((16.771, 49.6837), (15.8352, 49.3312), 0.707107);
    path.close();
    path.move_to((12.9759, 48.0526));
    path.quad_to((12.5249, 47.8173), (12.0835, 47.5647));
    path.conic_to((11.2156, 47.0679), (11.7124, 46.2), 0.707107);
    path.conic_to((12.2092, 45.3321), (13.0771, 45.8289), 0.707107);
    path.quad_to((13.4846, 46.0622), (13.9009, 46.2793));
    path.conic_to((14.7875, 46.7418), (14.325, 47.6284), 0.707107);
    path.conic_to((13.8626, 48.5151), (12.9759, 48.0526), 0.707107);
    path.close();
    path.move_to((10.3957, 46.5108));
    path.quad_to((9.9861, 46.2327), (9.58733, 45.9392));
    path.conic_to((8.78198, 45.3464), (9.37478, 44.541), 0.707107);
    path.conic_to((9.96757, 43.7357), (10.7729, 44.3285), 0.707107);
    path.quad_to((11.141, 44.5994), (11.5191, 44.8561));
    path.conic_to((12.3464, 45.4178), (11.7847, 46.2451), 0.707107);
    path.conic_to((11.223, 47.0725), (10.3957, 46.5108), 0.707107);
    path.close();
    path.move_to((8.00525, 44.6769));
    path.quad_to((7.6141, 44.339), (7.23672, 43.9859));
    path.conic_to((6.50649, 43.3027), (7.18969, 42.5725), 0.707107);
    path.conic_to((7.87289, 41.8423), (8.60312, 42.5255), 0.707107);
    path.quad_to((8.95149, 42.8514), (9.31254, 43.1632));
    path.conic_to((10.0693, 43.8169), (9.4157, 44.5737), 0.707107);
    path.conic_to((8.76206, 45.3305), (8.00525, 44.6769), 0.707107);
    path.close();
    path.move_to((5.75818, 42.4858));
    path.quad_to((5.39763, 42.089), (5.05371, 41.6777));
    path.conic_to((4.41226, 40.9105), (5.17942, 40.2691), 0.707107);
    path.conic_to((5.94658, 39.6276), (6.58804, 40.3948), 0.707107);
    path.quad_to((6.90548, 40.7744), (7.23832, 41.1407));
    path.conic_to((7.91085, 41.8808), (7.17078, 42.5533), 0.707107);
    path.conic_to((6.43071, 43.2258), (5.75818, 42.4858), 0.707107);
    path.close();
    path.move_to((3.72821, 39.9503));
    path.quad_to((3.42794, 39.523), (3.1451, 39.0842));
    path.conic_to((2.6034, 38.2436), (3.44397, 37.7019), 0.707107);
    path.conic_to((4.28454, 37.1602), (4.82624, 38.0008), 0.707107);
    path.quad_to((5.08734, 38.4059), (5.3645, 38.8003));
    path.conic_to((5.93951, 39.6184), (5.12137, 40.1934), 0.707107);
    path.conic_to((4.30322, 40.7684), (3.72821, 39.9503), 0.707107);
    path.close();
    path.move_to((2.09762, 37.3078));
    path.quad_to((1.85114, 36.8491), (1.62324, 36.381));
    path.conic_to((1.18551, 35.4819), (2.08461, 35.0442), 0.707107);
    path.conic_to((2.98372, 34.6064), (3.42145, 35.5055), 0.707107);
    path.quad_to((3.63184, 35.9377), (3.85934, 36.361));
    path.conic_to((4.33272, 37.2419), (3.45185, 37.7153), 0.707107);
    path.conic_to((2.57099, 38.1886), (2.09762, 37.3078), 0.707107);
    path.close();
    path.move_to((0.781912, 34.4596));
    path.quad_to((0.589924, 33.9681), (0.418029, 33.4692));
    path.conic_to((0.0922952, 32.5237), (1.03776, 32.198), 0.707107);
    path.conic_to((1.98322, 31.8722), (2.30895, 32.8177), 0.707107);
    path.quad_to((2.46761, 33.2782), (2.64484, 33.7319));
    path.conic_to((3.00867, 34.6634), (2.07721, 35.0272), 0.707107);
    path.conic_to((1.14575, 35.3911), (0.781912, 34.4596), 0.707107);
    path.close();
    path.move_to((-0.189761, 31.4402));
    path.quad_to((-0.321263, 30.9258), (-0.431662, 30.4065));
    path.conic_to((-0.639608, 29.4284), (0.338532, 29.2205), 0.707107);
    path.conic_to((1.31667, 29.0125), (1.52462, 29.9906), 0.707107);
    path.quad_to((1.62653, 30.47), (1.74791, 30.9448));
    path.conic_to((1.99561, 31.9136), (1.02677, 32.1613), 0.707107);
    path.conic_to((0.0579369, 32.409), (-0.189761, 31.4402), 0.707107);
    path.close();
    path.move_to((-0.784658, 28.3394));
    path.quad_to((-0.851693, 27.8218), (-0.897902, 27.3019));
    path.conic_to((-0.986437, 26.3058), (0.00963629, 26.2173), 0.707107);
    path.conic_to((1.00571, 26.1288), (1.09424, 27.1248), 0.707107);
    path.quad_to((1.1369, 27.6047), (1.19878, 28.0825));
    path.conic_to((1.32721, 29.0742), (0.335496, 29.2027), 0.707107);
    path.conic_to((-0.656222, 29.3311), (-0.784658, 28.3394), 0.707107);
    path.close();
    path.move_to((-0.999031, 25.2248));
    path.quad_to((-1.00354, 24.7027), (-0.987098, 24.1809));
    path.conic_to((-0.955596, 23.1814), (0.0439078, 23.2129), 0.707107);
    path.conic_to((1.04341, 23.2444), (1.01191, 24.2439), 0.707107);
    path.quad_to((0.996728, 24.7256), (1.00089, 25.2075));
    path.conic_to((1.00954, 26.2075), (0.00957754, 26.2161), 0.707107);
    path.conic_to((-0.990385, 26.2248), (-0.999031, 25.2248), 0.707107);
    path.close();
    path.move_to((-0.836492, 22.0887));
    path.quad_to((-0.778263, 21.5719), (-0.699419, 21.0579));
    path.conic_to((-0.5478, 20.0695), (0.440639, 20.2211), 0.707107);
    path.conic_to((1.42908, 20.3727), (1.27746, 21.3612), 0.707107);
    path.quad_to((1.20468, 21.8356), (1.15093, 22.3126));
    path.conic_to((1.03896, 23.3063), (0.0452449, 23.1944), 0.707107);
    path.conic_to((-0.948466, 23.0824), (-0.836492, 22.0887), 0.707107);
    path.close();
    path.move_to((-0.300548, 19.0098));
    path.quad_to((-0.174573, 18.4777), (-0.0263361, 17.9514));
    path.conic_to((0.244762, 16.9889), (1.20731, 17.26), 0.707107);
    path.conic_to((2.16987, 17.5311), (1.89877, 18.4936), 0.707107);
    path.quad_to((1.76193, 18.9794), (1.64565, 19.4706));
    path.conic_to((1.41526, 20.4437), (0.442159, 20.2133), 0.707107);
    path.conic_to((-0.530939, 19.9829), (-0.300548, 19.0098), 0.707107);
    path.close();
    path.move_to((0.642658, 15.9049));
    path.quad_to((0.827861, 15.409), (1.0331, 14.9209));
    path.conic_to((1.42076, 13.9991), (2.34256, 14.3868), 0.707107);
    path.conic_to((3.26437, 14.7744), (2.87671, 15.6962), 0.707107);
    path.quad_to((2.68726, 16.1467), (2.5163, 16.6046));
    path.conic_to((2.16648, 17.5414), (1.22967, 17.1916), 0.707107);
    path.conic_to((0.292846, 16.8418), (0.642658, 15.9049), 0.707107);
    path.close();
    path.move_to((1.91434, 13.0395));
    path.quad_to((2.14856, 12.5875), (2.40031, 12.1449));
    path.conic_to((2.89473, 11.2757), (3.76395, 11.7701), 0.707107);
    path.conic_to((4.63317, 12.2645), (4.13875, 13.1337), 0.707107);
    path.quad_to((3.90637, 13.5423), (3.69016, 13.9596));
    path.conic_to((3.23014, 14.8475), (2.34223, 14.3875), 0.707107);
    path.conic_to((1.45432, 13.9275), (1.91434, 13.0395), 0.707107);
    path.close();
    path.move_to((3.45073, 10.4525));
    path.quad_to((3.72744, 10.0426), (4.01954, 9.64356));
    path.conic_to((4.61017, 8.83661), (5.41711, 9.42725), 0.707107);
    path.conic_to((6.22405, 10.0179), (5.63342, 10.8248), 0.707107);
    path.quad_to((5.36379, 11.1932), (5.10836, 11.5716));
    path.conic_to((4.54884, 12.4004), (3.72003, 11.8409), 0.707107);
    path.conic_to((2.89121, 11.2813), (3.45073, 10.4525), 0.707107);
    path.close();
    path.move_to((5.2763, 8.05964));
    path.quad_to((5.61273, 7.66793), (5.96445, 7.2899));
    path.conic_to((6.6456, 6.55776), (7.37774, 7.23892), 0.707107);
    path.conic_to((8.10988, 7.92008), (7.42872, 8.65221), 0.707107);
    path.quad_to((7.10407, 9.00116), (6.79351, 9.36274));
    path.conic_to((6.14196, 10.1213), (5.38336, 9.46979), 0.707107);
    path.conic_to((4.62475, 8.81824), (5.2763, 8.05964), 0.707107);
    path.close();
    path.move_to((7.45913, 5.80839));
    path.quad_to((7.85457, 5.44696), (8.26455, 5.10214));
    path.conic_to((9.02985, 4.45847), (9.67352, 5.22377), 0.707107);
    path.conic_to((10.3172, 5.98907), (9.5519, 6.63274), 0.707107);
    path.quad_to((9.17345, 6.95105), (8.80843, 7.28467));
    path.conic_to((8.07029, 7.95931), (7.39564, 7.22117), 0.707107);
    path.conic_to((6.72099, 6.48303), (7.45913, 5.80839), 0.707107);
    path.close();
    path.move_to((9.98688, 3.77251));
    path.quad_to((10.4153, 3.46948), (10.8557, 3.18397));
    path.conic_to((11.6948, 2.63996), (12.2388, 3.47904), 0.707107);
    path.conic_to((12.7828, 4.31812), (11.9437, 4.86213), 0.707107);
    path.quad_to((11.5373, 5.12566), (11.1417, 5.40539));
    path.conic_to((10.3253, 5.98282), (9.74787, 5.16638), 0.707107);
    path.conic_to((9.17044, 4.34994), (9.98688, 3.77251), 0.707107);
    path.close();
    path.move_to((12.6283, 2.13208));
    path.quad_to((13.0861, 1.88442), (13.5534, 1.65529));
    path.conic_to((14.4513, 1.21504), (14.8915, 2.11291), 0.707107);
    path.conic_to((15.3318, 3.01078), (14.4339, 3.45104), 0.707107);
    path.quad_to((14.0025, 3.66255), (13.58, 3.89115));
    path.conic_to((12.7005, 4.36698), (12.2246, 3.48744), 0.707107);
    path.conic_to((11.7488, 2.60791), (12.6283, 2.13208), 0.707107);
    path.close();
    path.move_to((15.4718, 0.808815));
    path.quad_to((15.9627, 0.615476), (16.461, 0.442208));
    path.conic_to((17.4055, 0.113784), (17.7339, 1.05831), 0.707107);
    path.conic_to((18.0624, 2.00284), (17.1178, 2.33127), 0.707107);
    path.quad_to((16.6578, 2.49121), (16.2047, 2.66968));
    path.conic_to((15.2743, 3.03614), (14.9078, 2.10571), 0.707107);
    path.conic_to((14.5414, 1.17528), (15.4718, 0.808815), 0.707107);
    path.close();
    path.move_to((18.4879, -0.171272));
    path.quad_to((19.0019, -0.304236), (19.5208, -0.416111));
    path.conic_to((20.4984, -0.62685), (20.7091, 0.350692), 0.707107);
    path.conic_to((20.9198, 1.32823), (19.9423, 1.53897), 0.707107);
    path.quad_to((19.4633, 1.64224), (18.9889, 1.76498));
    path.conic_to((18.0207, 2.01544), (17.7703, 1.04732), 0.707107);
    path.conic_to((17.5198, 0.0791926), (18.4879, -0.171272), 0.707107);
    path.close();
    path.move_to((21.5882, -0.77517));
    path.quad_to((22.1056, -0.843665), (22.6254, -0.891339));
    path.conic_to((23.6212, -0.982672), (23.7126, 0.0131486), 0.707107);
    path.conic_to((23.8039, 1.00897), (22.8081, 1.1003), 0.707107);
    path.quad_to((22.3283, 1.14431), (21.8506, 1.20754));
    path.conic_to((20.8592, 1.33876), (20.728, 0.347405), 0.707107);
    path.conic_to((20.5968, -0.643948), (21.5882, -0.77517), 0.707107);
    path.close();
    path.move_to((24.7026, -0.998301));
    path.quad_to((25.2241, -1.00426), (25.7453, -0.989316));
    path.conic_to((26.7449, -0.960651), (26.7162, 0.0389383), 0.707107);
    path.conic_to((26.6876, 1.03853), (25.688, 1.00986), 0.707107);
    path.quad_to((25.2068, 0.996064), (24.7255, 1.00157));
    path.conic_to((23.7256, 1.013), (23.7141, 0.0130688), 0.707107);
    path.conic_to((23.7027, -0.986866), (24.7026, -0.998301), 0.707107);
    path.close();
    path.move_to((27.8388, -0.844563));
    path.quad_to((28.3559, -0.787759), (28.8704, -0.710314));
    path.conic_to((29.8592, -0.561454), (29.7104, 0.427404), 0.707107);
    path.conic_to((29.5615, 1.41626), (28.5726, 1.2674), 0.707107);
    path.quad_to((28.0978, 1.19591), (27.6204, 1.14348));
    path.conic_to((26.6264, 1.0343), (26.7356, 0.0402742), 0.707107);
    path.conic_to((26.8447, -0.953747), (27.8388, -0.844563), 0.707107);
    path.close();
    path.move_to((30.9153, -0.318153));
    path.quad_to((31.4481, -0.193671), (31.9752, -0.046875));
    path.conic_to((32.9386, 0.221405), (32.6703, 1.18475), 0.707107);
    path.conic_to((32.402, 2.14809), (31.4387, 1.87981), 0.707107);
    path.quad_to((30.9521, 1.74431), (30.4603, 1.6294));
    path.conic_to((29.4865, 1.40189), (29.714, 0.428111), 0.707107);
    path.conic_to((29.9416, -0.545664), (30.9153, -0.318153), 0.707107);
    path.close();
    path.move_to((34.0252, 0.616677));
    path.quad_to((34.5221, 0.800609), (35.0111, 1.00465));
    path.conic_to((35.934, 1.3897), (35.549, 2.31259), 0.707107);
    path.conic_to((35.1639, 3.23549), (34.241, 2.85044), 0.707107);
    path.quad_to((33.7896, 2.66211), (33.3309, 2.49232));
    path.conic_to((32.3931, 2.1452), (32.7402, 1.20738), 0.707107);
    path.conic_to((33.0873, 0.269559), (34.0252, 0.616677), 0.707107);
    path.close();
    path.move_to((36.8967, 1.88141));
    path.quad_to((37.3499, 2.11462), (37.7936, 2.3654));
    path.conic_to((38.6641, 2.85746), (38.1721, 3.72802), 0.707107);
    path.conic_to((37.68, 4.59858), (36.8094, 4.10652), 0.707107);
    path.quad_to((36.3999, 3.87504), (35.9815, 3.65976));
    path.conic_to((35.0924, 3.2022), (35.5499, 2.31302), 0.707107);
    path.conic_to((36.0075, 1.42384), (36.8967, 1.88141), 0.707107);
    path.close();
    path.move_to((39.4914, 3.413));
    path.line_to((39.5381, 3.44439));
    path.quad_to((39.9244, 3.70494), (40.3002, 3.97845));
    path.conic_to((41.1087, 4.56692), (40.5202, 5.37544), 0.707107);
    path.conic_to((39.9317, 6.18396), (39.1232, 5.59549), 0.707107);
    path.quad_to((38.7763, 5.34298), (38.4215, 5.10371));
    path.line_to((38.3749, 5.07232));
    path.conic_to((37.5452, 4.51406), (38.1035, 3.68439), 0.707107);
    path.conic_to((38.6618, 2.85473), (39.4914, 3.413), 0.707107);
    path.close();
    path.move_to((41.8859, 5.22965));
    path.quad_to((42.2782, 5.56471), (42.6568, 5.91499));
    path.conic_to((43.3908, 6.5941), (42.7117, 7.32814), 0.707107);
    path.conic_to((42.0326, 8.06218), (41.2986, 7.38308), 0.707107);
    path.quad_to((40.949, 7.05968), (40.587, 6.75043));
    path.conic_to((39.8266, 6.10097), (40.476, 5.34058), 0.707107);
    path.conic_to((41.1255, 4.58018), (41.8859, 5.22965), 0.707107);
    path.close();
    path.move_to((44.1413, 7.40421));
    path.quad_to((44.5035, 7.79829), (44.8493, 8.20695));
    path.conic_to((45.4952, 8.97038), (44.7317, 9.61627), 0.707107);
    path.conic_to((43.9683, 10.2622), (43.3224, 9.49874), 0.707107);
    path.quad_to((43.0033, 9.1215), (42.6689, 8.75773));
    path.conic_to((41.9921, 8.02152), (42.7283, 7.34476), 0.707107);
    path.conic_to((43.4645, 6.668), (44.1413, 7.40421), 0.707107);
    path.close();
    path.move_to((46.183, 9.9242));
    path.quad_to((46.4888, 10.3539), (46.777, 10.7957));
    path.conic_to((47.3233, 11.6332), (46.4857, 12.1796), 0.707107);
    path.conic_to((45.6482, 12.7259), (45.1018, 11.8883), 0.707107);
    path.quad_to((44.8358, 11.4805), (44.5535, 11.0839));
    path.conic_to((43.9737, 10.2691), (44.7884, 9.6893), 0.707107);
    path.conic_to((45.6032, 9.10947), (46.183, 9.9242), 0.707107);
    path.close();
    path.move_to((47.8333, 12.5645));
    path.quad_to((48.0821, 13.0214), (48.3125, 13.4879));
    path.conic_to((48.7552, 14.3845), (47.8586, 14.8273), 0.707107);
    path.conic_to((46.962, 15.2701), (46.5192, 14.3734), 0.707107);
    path.quad_to((46.3065, 13.9428), (46.0769, 13.5211));
    path.conic_to((45.5986, 12.6429), (46.4768, 12.1646), 0.707107);
    path.conic_to((47.355, 11.6863), (47.8333, 12.5645), 0.707107);
    path.close();
    path.move_to((49.1641, 15.4033));
    path.quad_to((49.3588, 15.8935), (49.5334, 16.3912));
    path.conic_to((49.8645, 17.3348), (48.9209, 17.6659), 0.707107);
    path.conic_to((47.9773, 17.997), (47.6462, 17.0534), 0.707107);
    path.quad_to((47.485, 16.5939), (47.3053, 16.1415));
    path.conic_to((46.9362, 15.2121), (47.8656, 14.843), 0.707107);
    path.conic_to((48.795, 14.4739), (49.1641, 15.4033), 0.707107);
    path.close();
    path.move_to((50.1526, 18.4161));
    path.quad_to((50.287, 18.9296), (50.4003, 19.4482));
    path.conic_to((50.6139, 20.4252), (49.6369, 20.6387), 0.707107);
    path.conic_to((48.66, 20.8522), (48.4465, 19.8753), 0.707107);
    path.quad_to((48.3419, 19.3966), (48.2178, 18.9225));
    path.conic_to((47.9645, 17.9551), (48.9319, 17.7019), 0.707107);
    path.conic_to((49.8993, 17.4487), (50.1526, 18.4161), 0.707107);
    path.close();
    path.move_to((50.7655, 21.5157));
    path.quad_to((50.8354, 22.033), (50.8846, 22.5528));
    path.conic_to((50.9787, 23.5483), (49.9831, 23.6425), 0.707107);
    path.conic_to((48.9876, 23.7366), (48.8935, 22.741), 0.707107);
    path.quad_to((48.8481, 22.2613), (48.7835, 21.7837));
    path.conic_to((48.6495, 20.7928), (49.6405, 20.6587), 0.707107);
    path.conic_to((50.6315, 20.5247), (50.7655, 21.5157), 0.707107);
    path.close();
    path.move_to((50.9974, 24.6301));
    path.quad_to((51.0048, 25.1509), (50.9913, 25.6715));
    path.conic_to((50.9655, 26.6712), (49.9658, 26.6454), 0.707107);
    path.conic_to((48.9662, 26.6196), (48.992, 25.6199), 0.707107);
    path.quad_to((49.0044, 25.1393), (48.9976, 24.6585));
    path.conic_to((48.9834, 23.6586), (49.9833, 23.6444), 0.707107);
    path.conic_to((50.9832, 23.6302), (50.9974, 24.6301), 0.707107);
    path.close();
    path.move_to((50.8524, 27.7662));
    path.quad_to((50.7971, 28.2837), (50.721, 28.7986));
    path.conic_to((50.5749, 29.7879), (49.5856, 29.6418), 0.707107);
    path.conic_to((48.5963, 29.4957), (48.7425, 28.5064), 0.707107);
    path.quad_to((48.8127, 28.0311), (48.8638, 27.5534));
    path.conic_to((48.9702, 26.5591), (49.9645, 26.6655), 0.707107);
    path.conic_to((50.9588, 26.7718), (50.8524, 27.7662), 0.707107);
    path.close();
    path.move_to((50.3355, 30.8404));
    path.quad_to((50.2125, 31.3739), (50.0672, 31.9018));
    path.conic_to((49.8018, 32.8659), (48.8376, 32.6005), 0.707107);
    path.conic_to((47.8735, 32.335), (48.139, 31.3709), 0.707107);
    path.quad_to((48.2731, 30.8836), (48.3867, 30.3912));
    path.conic_to((48.6113, 29.4167), (49.5857, 29.6413), 0.707107);
    path.conic_to((50.5602, 29.866), (50.3355, 30.8404), 0.707107);
    path.close();
    path.move_to((49.4091, 33.9552));
    path.quad_to((49.2264, 34.4531), (49.0236, 34.9431));
    path.conic_to((48.6412, 35.8671), (47.7172, 35.4846), 0.707107);
    path.conic_to((46.7932, 35.1022), (47.1757, 34.1782), 0.707107);
    path.quad_to((47.3629, 33.7259), (47.5315, 33.2663));
    path.conic_to((47.8759, 32.3275), (48.8147, 32.672), 0.707107);
    path.conic_to((49.7535, 33.0164), (49.4091, 33.9552), 0.707107);
    path.close();
    path.move_to((48.1514, 36.8328));
    path.quad_to((47.9191, 37.2871), (47.6694, 37.7318));
    path.conic_to((47.1797, 38.6038), (46.3078, 38.1141), 0.707107);
    path.conic_to((45.4359, 37.6244), (45.9256, 36.7525), 0.707107);
    path.quad_to((46.1562, 36.3418), (46.3705, 35.9226));
    path.conic_to((46.8256, 35.0321), (47.716, 35.4872), 0.707107);
    path.conic_to((48.6065, 35.9423), (48.1514, 36.8328), 0.707107);
    path.close();
    path.move_to((46.6245, 39.4354));
    path.line_to((46.5563, 39.537));
    path.quad_to((46.3146, 39.8955), (46.0624, 40.2438));
    path.conic_to((45.4761, 41.0539), (44.666, 40.4676), 0.707107);
    path.conic_to((43.8559, 39.8813), (44.4422, 39.0712), 0.707107);
    path.quad_to((44.6749, 38.7498), (44.8955, 38.4226));
    path.line_to((44.9637, 38.3211));
    path.conic_to((45.5209, 37.4907), (46.3513, 38.0479), 0.707107);
    path.conic_to((47.1817, 38.605), (46.6245, 39.4354), 0.707107);
    path.close();
    path.move_to((44.8168, 41.8314));
    path.quad_to((44.4832, 42.2241), (44.1342, 42.6034));
    path.conic_to((43.4572, 43.3394), (42.7212, 42.6623), 0.707107);
    path.conic_to((41.9853, 41.9853), (42.6623, 41.2494), 0.707107);
    path.quad_to((42.9845, 40.8992), (43.2924, 40.5366));
    path.conic_to((43.9398, 39.7745), (44.702, 40.4218), 0.707107);
    path.conic_to((45.4642, 41.0692), (44.8168, 41.8314), 0.707107);
    path.close();
    path.move_to((42.6505, 44.0908));
    path.quad_to((42.2577, 44.454), (41.8504, 44.8006));
    path.conic_to((41.0888, 45.4487), (40.4408, 44.6871), 0.707107);
    path.conic_to((39.7927, 43.9256), (40.5542, 43.2775), 0.707107);
    path.quad_to((40.9302, 42.9575), (41.2928, 42.6223));
    path.conic_to((42.027, 41.9434), (42.7059, 42.6777), 0.707107);
    path.conic_to((43.3848, 43.412), (42.6505, 44.0908), 0.707107);
    path.close();
    path.move_to((40.1383, 46.1384));
    path.quad_to((39.7073, 46.4471), (39.2641, 46.7378));
    path.conic_to((38.4281, 47.2865), (37.8795, 46.4504), 0.707107);
    path.conic_to((37.3308, 45.6143), (38.1669, 45.0657), 0.707107);
    path.quad_to((38.576, 44.7972), (38.9738, 44.5124));
    path.conic_to((39.7868, 43.9301), (40.369, 44.7432), 0.707107);
    path.conic_to((40.9513, 45.5562), (40.1383, 46.1384), 0.707107);
    path.close();
    path.move_to((37.4991, 47.7985));
    path.quad_to((37.0431, 48.0485), (36.5775, 48.2801));
    path.conic_to((35.6821, 48.7254), (35.2368, 47.83), 0.707107);
    path.conic_to((34.7915, 46.9346), (35.6869, 46.4893), 0.707107);
    path.quad_to((36.1167, 46.2755), (36.5376, 46.0448));
    path.conic_to((37.4145, 45.5641), (37.8952, 46.4409), 0.707107);
    path.conic_to((38.376, 47.3178), (37.4991, 47.7985), 0.707107);
    path.close();
    path.move_to((34.6651, 49.1368));
    path.quad_to((34.1756, 49.3328), (33.6785, 49.5089));
    path.conic_to((32.7358, 49.8427), (32.402, 48.9), 0.707107);
    path.conic_to((32.0682, 47.9574), (33.0109, 47.6236), 0.707107);
    path.quad_to((33.4697, 47.4611), (33.9216, 47.2801));
    path.conic_to((34.85, 46.9084), (35.2217, 47.8368), 0.707107);
    path.conic_to((35.5934, 48.7651), (34.6651, 49.1368), 0.707107);
    path.close();
    path.move_to((31.6557, 50.1337));
    path.quad_to((31.1425, 50.2696), (30.6243, 50.3844));
    path.conic_to((29.648, 50.6007), (29.4317, 49.6244), 0.707107);
    path.conic_to((29.2153, 48.6481), (30.1917, 48.4317), 0.707107);
    path.quad_to((30.6701, 48.3257), (31.1437, 48.2003));
    path.conic_to((32.1104, 47.9443), (32.3664, 48.911), 0.707107);
    path.conic_to((32.6223, 49.8777), (31.6557, 50.1337), 0.707107);
    path.close();
    path.move_to((28.5567, 50.7556));
    path.quad_to((28.0395, 50.827), (27.5198, 50.8776));
    path.conic_to((26.5245, 50.9745), (26.4276, 49.9792), 0.707107);
    path.conic_to((26.3307, 48.9839), (27.326, 48.887), 0.707107);
    path.quad_to((27.8056, 48.8403), (28.2831, 48.7744));
    path.conic_to((29.2737, 48.6376), (29.4105, 49.6282), 0.707107);
    path.conic_to((29.5473, 50.6188), (28.5567, 50.7556), 0.707107);
    path.close();
    path.move_to((25.4424, 50.9962));
    path.quad_to((24.9222, 51.0051), (24.4022, 50.9931));
    path.conic_to((23.4025, 50.9701), (23.4255, 49.9704), 0.707107);
    path.conic_to((23.4485, 48.9707), (24.4482, 48.9937), 0.707107);
    path.quad_to((24.9283, 49.0047), (25.4084, 48.9965));
    path.conic_to((26.4083, 48.9795), (26.4253, 49.9794), 0.707107);
    path.conic_to((26.4423, 50.9792), (25.4424, 50.9962), 0.707107);
    path.close();
    path.move_to((22.3065, 50.8601));
    path.quad_to((21.7885, 50.8062), (21.2732, 50.7315));
    path.conic_to((20.2835, 50.5882), (20.4268, 49.5985), 0.707107);
    path.conic_to((20.5702, 48.6088), (21.5599, 48.7522), 0.707107);
    path.quad_to((22.0355, 48.8211), (22.5136, 48.8709));
    path.conic_to((23.5083, 48.9745), (23.4047, 49.9691), 0.707107);
    path.conic_to((23.3011, 50.9637), (22.3065, 50.8601), 0.707107);
    path.close();
    path.move_to((19.2346, 50.3527));
    path.quad_to((18.7003, 50.2312), (18.1717, 50.0873));
    path.conic_to((17.2068, 49.8247), (17.4694, 48.8598), 0.707107);
    path.conic_to((17.732, 47.8949), (18.6969, 48.1575), 0.707107);
    path.quad_to((19.185, 48.2904), (19.6781, 48.4025));
    path.conic_to((20.6532, 48.6243), (20.4314, 49.5994), 0.707107);
    path.conic_to((20.2097, 50.5745), (19.2346, 50.3527), 0.707107);
    path.close();
    path.move_to((16.1149, 49.4347));
    path.quad_to((15.6161, 49.2533), (15.1251, 49.0517));
    path.conic_to((14.2, 48.6719), (14.5798, 47.7469), 0.707107);
    path.conic_to((14.9596, 46.8218), (15.8847, 47.2016), 0.707107);
    path.quad_to((16.3379, 47.3877), (16.7984, 47.5551));
    path.conic_to((17.7382, 47.8969), (17.3964, 48.8366), 0.707107);
    path.conic_to((17.0547, 49.7764), (16.1149, 49.4347), 0.707107);
    path.close();
    path.move_to((13.2313, 48.184));
    path.quad_to((12.776, 47.9529), (12.33, 47.704));
    path.conic_to((11.4568, 47.2167), (11.9441, 46.3434), 0.707107);
    path.conic_to((12.4314, 45.4702), (13.3046, 45.9575), 0.707107);
    path.quad_to((13.7162, 46.1872), (14.1365, 46.4006));
    path.conic_to((15.0282, 46.8532), (14.5756, 47.7449), 0.707107);
    path.conic_to((14.123, 48.6366), (13.2313, 48.184), 0.707107);
    path.close();
    path.move_to((10.6208, 46.6619));
    path.line_to((10.4641, 46.5571));
    path.quad_to((10.1333, 46.334), (9.81253, 46.1031));
    path.conic_to((9.00087, 45.519), (9.585, 44.7073), 0.707107);
    path.conic_to((10.1691, 43.8957), (10.9808, 44.4798), 0.707107);
    path.quad_to((11.2769, 44.6929), (11.5763, 44.8948));
    path.line_to((11.7329, 44.9996));
    path.conic_to((12.564, 45.5557), (12.008, 46.3868), 0.707107);
    path.conic_to((11.4519, 47.2179), (10.6208, 46.6619), 0.707107);
    path.close();
    path.move_to((8.22326, 44.8631));
    path.quad_to((7.82986, 44.5308), (7.44999, 44.1833));
    path.conic_to((6.71217, 43.5082), (7.38718, 42.7704), 0.707107);
    path.conic_to((8.06219, 42.0326), (8.8, 42.7076), 0.707107);
    path.quad_to((9.15066, 43.0284), (9.51375, 43.3351));
    path.conic_to((10.2777, 43.9804), (9.63248, 44.7443), 0.707107);
    path.conic_to((8.98724, 45.5083), (8.22326, 44.8631), 0.707107);
    path.close();
    path.move_to((5.95972, 42.705));
    path.quad_to((5.59577, 42.3136), (5.24823, 41.9076));
    path.conic_to((4.59793, 41.148), (5.3576, 40.4977), 0.707107);
    path.conic_to((6.11728, 39.8473), (6.76758, 40.607), 0.707107);
    path.quad_to((7.08843, 40.9818), (7.42436, 41.3431));
    path.conic_to((8.10532, 42.0754), (7.373, 42.7564), 0.707107);
    path.conic_to((6.64068, 43.4373), (5.95972, 42.705), 0.707107);
    path.close();
    path.move_to((3.90635, 40.2006));
    path.quad_to((3.59492, 39.7684), (3.30147, 39.3239));
    path.conic_to((2.75055, 38.4893), (3.58511, 37.9384), 0.707107);
    path.conic_to((4.41967, 37.3875), (4.97059, 38.222), 0.707107);
    path.quad_to((5.24148, 38.6324), (5.52894, 39.0313));
    path.conic_to((6.11358, 39.8426), (5.30228, 40.4272), 0.707107);
    path.conic_to((4.49099, 41.0119), (3.90635, 40.2006), 0.707107);
    path.close();
    path.move_to((2.23643, 37.5626));
    path.quad_to((1.98525, 37.1075), (1.75248, 36.6427));
    path.conic_to((1.30469, 35.7486), (2.19883, 35.3008), 0.707107);
    path.conic_to((3.09296, 34.853), (3.54076, 35.7471), 0.707107);
    path.quad_to((3.75563, 36.1762), (3.98747, 36.5963));
    path.conic_to((4.47065, 37.4718), (3.59513, 37.955), 0.707107);
    path.conic_to((2.71961, 38.4382), (2.23643, 37.5626), 0.707107);
    path.close();
    path.move_to((0.890647, 34.7334));
    path.quad_to((0.69328, 34.2445), (0.515902, 33.7481));
    path.conic_to((0.179435, 32.8064), (1.12113, 32.4699), 0.707107);
    path.conic_to((2.06282, 32.1335), (2.39929, 33.0752), 0.707107);
    path.quad_to((2.56303, 33.5334), (2.74521, 33.9847));
    path.conic_to((3.11957, 34.912), (2.19229, 35.2863), 0.707107);
    path.conic_to((1.26501, 35.6607), (0.890647, 34.7334), 0.707107);
    path.close();
    path.move_to((-0.114587, 31.7274));
    path.quad_to((-0.251922, 31.2147), (-0.368218, 30.6968));
    path.conic_to((-0.587327, 29.7211), (0.388373, 29.502), 0.707107);
    path.conic_to((1.36407, 29.2829), (1.58318, 30.2586), 0.707107);
    path.quad_to((1.69053, 30.7366), (1.8173, 31.2099));
    path.conic_to((2.07605, 32.1758), (1.1101, 32.4346), 0.707107);
    path.conic_to((0.144159, 32.6933), (-0.114587, 31.7274), 0.707107);
    path.close();
    path.move_to((-0.745485, 28.6291));
    path.quad_to((-0.818367, 28.112), (-0.870432, 27.5925));
    path.conic_to((-0.970142, 26.5974), (0.0248742, 26.4977), 0.707107);
    path.conic_to((1.01989, 26.398), (1.1196, 27.393), 0.707107);
    path.quad_to((1.16766, 27.8726), (1.23494, 28.3499));
    path.conic_to((1.37452, 29.3401), (0.384305, 29.4797), 0.707107);
    path.conic_to((-0.605905, 29.6193), (-0.745485, 28.6291), 0.707107);
    path.close();
    path.move_to((-0.994901, 25.515));
    path.quad_to((-1.00519, 24.9955), (-0.994722, 24.4761));
    path.conic_to((-0.97457, 23.4763), (0.0252273, 23.4964), 0.707107);
    path.conic_to((1.02502, 23.5166), (1.00487, 24.5164), 0.707107);
    path.quad_to((0.995207, 24.9959), (1.00471, 25.4754));
    path.conic_to((1.02451, 26.4752), (0.0247103, 26.495), 0.707107);
    path.conic_to((-0.975093, 26.5148), (-0.994901, 25.515), 0.707107);
    path.close();
    path.move_to((-0.867571, 22.3792));
    path.quad_to((-0.81506, 21.8609), (-0.741825, 21.3451));
    path.conic_to((-0.60125, 20.355), (0.38882, 20.4956), 0.707107);
    path.conic_to((1.37889, 20.6361), (1.23831, 21.6262), 0.707107);
    path.quad_to((1.17071, 22.1023), (1.12224, 22.5807));
    path.conic_to((1.02144, 23.5757), (0.026537, 23.4749), 0.707107);
    path.conic_to((-0.96837, 23.3741), (-0.867571, 22.3792), 0.707107);
    path.close();
    path.move_to((-0.369678, 19.3097));
    path.quad_to((-0.249693, 18.7748), (-0.107265, 18.2453));
    path.conic_to((0.152529, 17.2797), (1.11819, 17.5395), 0.707107);
    path.conic_to((2.08386, 17.7993), (1.82406, 18.7649), 0.707107);
    path.quad_to((1.69259, 19.2536), (1.58184, 19.7474));
    path.conic_to((1.36298, 20.7232), (0.387221, 20.5043), 0.707107);
    path.conic_to((-0.588536, 20.2855), (-0.369678, 19.3097), 0.707107);
    path.close();
    path.move_to((0.539863, 16.1851));
    path.quad_to((0.719962, 15.6854), (0.920307, 15.1934));
    path.conic_to((1.29748, 14.2673), (2.22362, 14.6445), 0.707107);
    path.conic_to((3.14976, 15.0216), (2.7726, 15.9478), 0.707107);
    path.quad_to((2.58765, 16.4019), (2.42141, 16.8632));
    path.conic_to((2.08237, 17.804), (1.1416, 17.4649), 0.707107);
    path.conic_to((0.200823, 17.1259), (0.539863, 16.1851), 0.707107);
    path.close();
    path.move_to((1.78353, 13.2955));
    path.quad_to((2.01364, 12.8391), (2.26151, 12.392));
    path.conic_to((2.74643, 11.5175), (3.62099, 12.0024), 0.707107);
    path.conic_to((4.49555, 12.4873), (4.01063, 13.3618), 0.707107);
    path.quad_to((3.78183, 13.7745), (3.56941, 14.1958));
    path.conic_to((3.11923, 15.0888), (2.22629, 14.6386), 0.707107);
    path.conic_to((1.33336, 14.1884), (1.78353, 13.2955), 0.707107);
    path.close();
    path.move_to((3.30083, 10.6771));
    path.line_to((3.44218, 10.4652));
    path.quad_to((3.6466, 10.1621), (3.85641, 9.86895));
    path.conic_to((4.43837, 9.05574), (5.25159, 9.6377), 0.707107);
    path.conic_to((6.0648, 10.2197), (5.48284, 11.0329), 0.707107);
    path.quad_to((5.28917, 11.3035), (5.10592, 11.5752));
    path.line_to((4.96457, 11.787));
    path.conic_to((4.4096, 12.6189), (3.57773, 12.0639), 0.707107);
    path.conic_to((2.74586, 11.509), (3.30083, 10.6771), 0.707107);
    path.close();
    path.move_to((5.0909, 8.27793));
    path.quad_to((5.42174, 7.88403), (5.76791, 7.50353));
    path.conic_to((6.44085, 6.76383), (7.18054, 7.43678), 0.707107);
    path.conic_to((7.92024, 8.10972), (7.24729, 8.84942), 0.707107);
    path.quad_to((6.92775, 9.20065), (6.62237, 9.56424));
    path.conic_to((5.97921, 10.33), (5.21348, 9.68682), 0.707107);
    path.conic_to((4.44774, 9.04367), (5.0909, 8.27793), 0.707107);
    path.close();
    path.move_to((7.24064, 6.0104));
    path.quad_to((7.63069, 5.64561), (8.03537, 5.29717));
    path.conic_to((8.79318, 4.64469), (9.44566, 5.40249), 0.707107);
    path.conic_to((10.0981, 6.16029), (9.34034, 6.81278), 0.707107);
    path.quad_to((8.96678, 7.13442), (8.60675, 7.47113));
    path.conic_to((7.87638, 8.15419), (7.19332, 7.42382), 0.707107);
    path.conic_to((6.51027, 6.69345), (7.24064, 6.0104), 0.707107);
    path.close();
    path.move_to((9.73726, 3.95128));
    path.quad_to((10.1706, 3.63704), (10.6165, 3.34092));
    path.conic_to((11.4496, 2.78771), (12.0028, 3.62075), 0.707107);
    path.conic_to((12.556, 4.4538), (11.7229, 5.007), 0.707107);
    path.quad_to((11.3113, 5.28035), (10.9113, 5.57041));
    path.conic_to((10.1018, 6.15744), (9.51472, 5.34787), 0.707107);
    path.conic_to((8.92769, 4.53831), (9.73726, 3.95128), 0.707107);
    path.close();
    path.move_to((12.374, 2.27153));
    path.quad_to((12.8282, 2.01921), (13.2921, 1.78522));
    path.conic_to((14.185, 1.33492), (14.6353, 2.22779), 0.707107);
    path.conic_to((15.0856, 3.12067), (14.1927, 3.57097), 0.707107);
    path.quad_to((13.7645, 3.78696), (13.3452, 4.01988));
    path.conic_to((12.471, 4.5055), (11.9854, 3.63132), 0.707107);
    path.conic_to((11.4998, 2.75715), (12.374, 2.27153), 0.707107);
    path.close();
    path.move_to((15.1984, 0.918296));
    path.quad_to((15.6866, 0.719602), (16.1824, 0.540851));
    path.conic_to((17.1231, 0.20171), (17.4623, 1.14245), 0.707107);
    path.conic_to((17.8014, 2.08318), (16.8607, 2.42232), 0.707107);
    path.quad_to((16.403, 2.58733), (15.9524, 2.77074));
    path.conic_to((15.0261, 3.14772), (14.6492, 2.2215), 0.707107);
    path.conic_to((14.2722, 1.29528), (15.1984, 0.918296), 0.707107);
    path.close();
    path.move_to((18.201, -0.0952874));
    path.quad_to((18.7132, -0.234075), (19.2308, -0.351842));
    path.conic_to((20.2058, -0.573734), (20.4277, 0.401338), 0.707107);
    path.conic_to((20.6496, 1.37641), (19.6745, 1.5983), 0.707107);
    path.quad_to((19.1968, 1.70701), (18.724, 1.83512));
    path.conic_to((17.7588, 2.09662), (17.4973, 1.13142), 0.707107);
    path.conic_to((17.2358, 0.166216), (18.201, -0.0952874), 0.707107);
    path.close();
    path.move_to((21.2986, -0.73518));
    path.quad_to((21.8155, -0.809526), (22.3349, -0.863052));
    path.conic_to((23.3297, -0.965552), (23.4322, 0.029181), 0.707107);
    path.conic_to((23.5347, 1.02391), (22.5399, 1.12641), 0.707107);
    path.quad_to((22.0604, 1.17582), (21.5833, 1.24445));
    path.conic_to((20.5935, 1.38681), (20.4511, 0.397), 0.707107);
    path.conic_to((20.3088, -0.592814), (21.2986, -0.73518), 0.707107);
    path.close();
    path.move_to((24.4124, -0.993361));
    path.quad_to((24.9312, -1.00509), (25.4501, -0.996107));
    path.conic_to((26.4499, -0.978799), (26.4326, 0.0210512), 0.707107);
    path.conic_to((26.4153, 1.0209), (25.4155, 1.00359), 0.707107);
    path.quad_to((24.9365, 0.995302), (24.4576, 1.00613));
    path.conic_to((23.4578, 1.02873), (23.4352, 0.0289853), 0.707107);
    path.conic_to((23.4126, -0.970759), (24.4124, -0.993361), 0.707107);
    path.close();
    path.move_to((27.5481, -0.87484));
    path.quad_to((28.0668, -0.823762), (28.583, -0.75194));
    path.conic_to((29.5734, -0.614138), (29.4356, 0.376322), 0.707107);
    path.conic_to((29.2978, 1.36678), (28.3074, 1.22898), 0.707107);
    path.quad_to((27.8309, 1.16268), (27.3521, 1.11553));
    path.conic_to((26.3569, 1.01753), (26.4549, 0.0223428), 0.707107);
    path.conic_to((26.5529, -0.972843), (27.5481, -0.87484), 0.707107);
    path.close();
    path.move_to((30.6151, -0.386432));
    path.quad_to((31.1507, -0.267954), (31.6809, -0.126991));
    path.conic_to((32.6473, 0.129965), (32.3904, 1.09639), 0.707107);
    path.conic_to((32.1334, 2.06281), (31.167, 1.80585), 0.707107);
    path.quad_to((30.6776, 1.67574), (30.1832, 1.56637));
    path.conic_to((29.2068, 1.35041), (29.4227, 0.374005), 0.707107);
    path.conic_to((29.6387, -0.602396), (30.6151, -0.386432), 0.707107);
    path.close();
    path.move_to((33.7445, 0.514616));
    path.quad_to((34.2452, 0.693421), (34.7381, 0.892536));
    path.conic_to((35.6653, 1.26708), (35.2908, 2.19429), 0.707107);
    path.conic_to((34.9162, 3.1215), (33.989, 2.74696), 0.707107);
    path.quad_to((33.534, 2.56316), (33.0718, 2.3981));
    path.conic_to((32.1301, 2.06177), (32.4664, 1.12003), 0.707107);
    path.conic_to((32.8027, 0.178285), (33.7445, 0.514616), 0.707107);
    path.close();
    path.move_to((36.6402, 1.7512));
    path.quad_to((37.0977, 1.98026), (37.5458, 2.22715));
    path.conic_to((38.4217, 2.70968), (37.9392, 3.58556), 0.707107);
    path.conic_to((37.4566, 4.46144), (36.5808, 3.97891), 0.707107);
    path.quad_to((36.1671, 3.75102), (35.7448, 3.53956));
    path.conic_to((34.8506, 3.09185), (35.2983, 2.19767), 0.707107);
    path.conic_to((35.746, 1.30349), (36.6402, 1.7512), 0.707107);
    path.close();
    path.move_to((39.2611, 3.26012));
    path.quad_to((39.4005, 3.35159), (39.539, 3.44501));
    path.quad_to((39.8091, 3.62717), (40.0746, 3.81611));
    path.conic_to((40.8893, 4.3959), (40.3096, 5.21067), 0.707107);
    path.conic_to((39.7298, 6.02543), (38.915, 5.44564), 0.707107);
    path.quad_to((38.67, 5.2713), (38.4206, 5.10309));
    path.quad_to((38.293, 5.017), (38.164, 4.9324));
    path.conic_to((37.3279, 4.38388), (37.8764, 3.54775), 0.707107);
    path.conic_to((38.4249, 2.71161), (39.2611, 3.26012), 0.707107);
    path.close();
    path.move_to((41.6673, 5.04503));
    path.quad_to((42.0618, 5.37449), (42.4428, 5.71927));
    path.conic_to((43.1844, 6.39015), (42.5135, 7.13171), 0.707107);
    path.conic_to((41.8426, 7.87327), (41.1011, 7.20239), 0.707107);
    path.quad_to((40.7493, 6.88414), (40.3852, 6.58004));
    path.conic_to((39.6177, 5.93899), (40.2588, 5.17149), 0.707107);
    path.conic_to((40.8998, 4.40399), (41.6673, 5.04503), 0.707107);
    path.close();
    path.move_to((43.9388, 7.1865));
    path.quad_to((44.3044, 7.57519), (44.6538, 7.97856));
    path.conic_to((45.3084, 8.73448), (44.5525, 9.38914), 0.707107);
    path.conic_to((43.7966, 10.0438), (43.1419, 9.28789), 0.707107);
    path.quad_to((42.8195, 8.91555), (42.482, 8.55677));
    path.conic_to((41.7969, 7.82836), (42.5253, 7.14322), 0.707107);
    path.conic_to((43.2537, 6.45808), (43.9388, 7.1865), 0.707107);
    path.close();
    path.move_to((46.0036, 9.6753));
    path.quad_to((46.3207, 10.1098), (46.6195, 10.5571));
    path.conic_to((47.175, 11.3886), (46.3435, 11.9441), 0.707107);
    path.conic_to((45.5119, 12.4996), (44.9564, 11.6681), 0.707107);
    path.quad_to((44.6806, 11.2552), (44.388, 10.8541));
    path.conic_to((43.7986, 10.0463), (44.6064, 9.45688), 0.707107);
    path.conic_to((45.4142, 8.86747), (46.0036, 9.6753), 0.707107);
    path.close();
    path.move_to((47.6932, 12.3107));
    path.quad_to((47.9467, 12.764), (48.1819, 13.2271));
    path.conic_to((48.6347, 14.1187), (47.7431, 14.5715), 0.707107);
    path.conic_to((46.8514, 15.0243), (46.3986, 14.1327), 0.707107);
    path.quad_to((46.1816, 13.7053), (45.9476, 13.2868));
    path.conic_to((45.4595, 12.414), (46.3323, 11.9259), 0.707107);
    path.conic_to((47.2051, 11.4379), (47.6932, 12.3107), 0.707107);
    path.close();
    path.move_to((49.0539, 15.1303));
    path.quad_to((49.2539, 15.6178), (49.434, 16.113));
    path.conic_to((49.7758, 17.0527), (48.836, 17.3946), 0.707107);
    path.conic_to((47.8963, 17.7364), (47.5545, 16.7966), 0.707107);
    path.quad_to((47.3882, 16.3395), (47.2036, 15.8895));
    path.conic_to((46.824, 14.9643), (47.7491, 14.5847), 0.707107);
    path.conic_to((48.6743, 14.2051), (49.0539, 15.1303), 0.707107);
    path.close();
    path.move_to((50.0758, 18.1294));
    path.quad_to((50.216, 18.6412), (50.3352, 19.1584));
    path.conic_to((50.5599, 20.1328), (49.5855, 20.3575), 0.707107);
    path.conic_to((48.6111, 20.5821), (48.3864, 19.6077), 0.707107);
    path.quad_to((48.2763, 19.1304), (48.1469, 18.6579));
    path.conic_to((47.8826, 17.6935), (48.8471, 17.4292), 0.707107);
    path.conic_to((49.8115, 17.165), (50.0758, 18.1294), 0.707107);
    path.close();
    path.move_to((50.7247, 21.2262));
    path.quad_to((50.8005, 21.743), (50.8555, 22.2623));
    path.conic_to((50.9607, 23.2568), (49.9663, 23.3621), 0.707107);
    path.conic_to((48.9719, 23.4673), (48.8666, 22.4729), 0.707107);
    path.quad_to((48.8158, 21.9935), (48.7458, 21.5165));
    path.conic_to((48.6007, 20.5271), (49.5901, 20.382), 0.707107);
    path.conic_to((50.5795, 20.2368), (50.7247, 21.2262), 0.707107);
    path.close();
    path.move_to((50.9916, 24.3398));
    path.quad_to((51.0048, 24.858), (50.9973, 25.3762));
    path.conic_to((50.9828, 26.3761), (49.9829, 26.3616), 0.707107);
    path.conic_to((48.983, 26.3472), (48.9975, 25.3473), 0.707107);
    path.quad_to((49.0044, 24.8687), (48.9923, 24.3906));
    path.conic_to((48.9669, 23.3909), (49.9665, 23.3655), 0.707107);
    path.conic_to((50.9662, 23.3401), (50.9916, 24.3398), 0.707107);
    path.close();
    path.move_to((50.8819, 27.4753));
    path.quad_to((50.8323, 27.9943), (50.7618, 28.511));
    path.conic_to((50.6268, 29.5018), (49.636, 29.3668), 0.707107);
    path.conic_to((48.6451, 29.2317), (48.7802, 28.2409), 0.707107);
    path.quad_to((48.8452, 27.7641), (48.891, 27.2849));
    path.conic_to((48.9862, 26.2894), (49.9816, 26.3846), 0.707107);
    path.conic_to((50.9771, 26.4798), (50.8819, 27.4753), 0.707107);
    path.close();
    path.move_to((50.4023, 30.5429));
    path.quad_to((50.2856, 31.0775), (50.1465, 31.607));
    path.conic_to((49.8924, 32.5742), (48.9252, 32.3201), 0.707107);
    path.conic_to((47.9581, 32.066), (48.2122, 31.0988), 0.707107);
    path.quad_to((48.3405, 30.6102), (48.4483, 30.1165));
    path.conic_to((48.6614, 29.1395), (49.6385, 29.3527), 0.707107);
    path.conic_to((50.6155, 29.5659), (50.4023, 30.5429), 0.707107);
    path.close();
    path.move_to((49.5104, 33.674));
    path.quad_to((49.3329, 34.1756), (49.1351, 34.6695));
    path.conic_to((48.7632, 35.5977), (47.8349, 35.2258), 0.707107);
    path.conic_to((46.9066, 34.854), (47.2785, 33.9257), 0.707107);
    path.quad_to((47.4612, 33.4697), (47.625, 33.0067));
    path.conic_to((47.9587, 32.064), (48.9014, 32.3977), 0.707107);
    path.conic_to((49.8441, 32.7313), (49.5104, 33.674), 0.707107);
    path.close();
    path.move_to((48.281, 36.5756));
    path.quad_to((48.053, 37.0342), (47.8071, 37.4835));
    path.conic_to((47.3269, 38.3607), (46.4497, 37.8805), 0.707107);
    path.conic_to((45.5725, 37.4004), (46.0527, 36.5232), 0.707107);
    path.quad_to((46.2797, 36.1085), (46.4901, 35.6852));
    path.conic_to((46.9353, 34.7898), (47.8307, 35.235), 0.707107);
    path.conic_to((48.7262, 35.6802), (48.281, 36.5756), 0.707107);
    path.close();
    path.move_to((46.7777, 39.2033));
    path.quad_to((46.6677, 39.3719), (46.555, 39.539));
    path.quad_to((46.3865, 39.7888), (46.2121, 40.0349));
    path.conic_to((45.6338, 40.8507), (44.818, 40.2724), 0.707107);
    path.conic_to((44.0021, 39.6942), (44.5804, 38.8783), 0.707107);
    path.quad_to((44.7413, 38.6513), (44.8969, 38.4206));
    path.quad_to((45.0008, 38.2665), (45.1025, 38.1107));
    path.conic_to((45.6488, 37.2731), (46.4864, 37.8194), 0.707107);
    path.conic_to((47.324, 38.3657), (46.7777, 39.2033), 0.707107);
    path.close();
    path.move_to((44.9527, 41.6701));
    path.quad_to((44.6177, 42.0709), (44.267, 42.458));
    path.conic_to((43.5955, 43.1991), (42.8545, 42.5276), 0.707107);
    path.conic_to((42.1135, 41.8561), (42.7849, 41.1151), 0.707107);
    path.quad_to((43.1087, 40.7578), (43.4178, 40.3878));
    path.conic_to((44.059, 39.6203), (44.8264, 40.2615), 0.707107);
    path.conic_to((45.5938, 40.9027), (44.9527, 41.6701), 0.707107);
    path.close();
    path.move_to((42.7884, 43.9624));
    path.quad_to((42.4083, 44.319), (42.014, 44.6602));
    path.conic_to((41.2578, 45.3146), (40.6034, 44.5585), 0.707107);
    path.conic_to((39.949, 43.8023), (40.7052, 43.1479), 0.707107);
    path.quad_to((41.0691, 42.833), (41.4201, 42.5037));
    path.conic_to((42.1494, 41.8196), (42.8336, 42.5489), 0.707107);
    path.conic_to((43.5178, 43.2782), (42.7884, 43.9624), 0.707107);
    path.close();
    path.move_to((40.3892, 45.9564));
    path.quad_to((39.9683, 46.2655), (39.5354, 46.5574));
    path.conic_to((38.7062, 47.1165), (38.1472, 46.2873), 0.707107);
    path.conic_to((37.5881, 45.4582), (38.4173, 44.8992), 0.707107);
    path.quad_to((38.8169, 44.6297), (39.2054, 44.3444));
    path.conic_to((40.0114, 43.7525), (40.6033, 44.5585), 0.707107);
    path.conic_to((41.1952, 45.3645), (40.3892, 45.9564), 0.707107);
    path.close();
    path.move_to((37.7543, 47.6568));
    path.quad_to((37.2977, 47.9138), (36.8312, 48.1522));
    path.conic_to((35.9407, 48.6072), (35.4857, 47.7167), 0.707107);
    path.conic_to((35.0306, 46.8263), (35.9211, 46.3712), 0.707107);
    path.quad_to((36.3518, 46.1511), (36.7732, 45.9139));
    path.conic_to((37.6446, 45.4234), (38.1351, 46.2948), 0.707107);
    path.conic_to((38.6257, 47.1662), (37.7543, 47.6568), 0.707107);
    path.close();
    path.move_to((34.9311, 49.0286));
    path.quad_to((34.4488, 49.2279), (33.9589, 49.4077));
    path.conic_to((33.0202, 49.7523), (32.6756, 48.8136), 0.707107);
    path.conic_to((32.331, 47.8748), (33.2698, 47.5302), 0.707107);
    path.quad_to((33.722, 47.3642), (34.1672, 47.1802));
    path.conic_to((35.0914, 46.7983), (35.4733, 47.7224), 0.707107);
    path.conic_to((35.8553, 48.6466), (34.9311, 49.0286), 0.707107);
    path.close();
    path.move_to((31.9824, 50.0449));
    path.quad_to((31.4774, 50.1857), (30.9668, 50.3061));
    path.conic_to((29.9935, 50.5355), (29.764, 49.5622), 0.707107);
    path.conic_to((29.5346, 48.5889), (30.5079, 48.3594), 0.707107);
    path.quad_to((30.9789, 48.2484), (31.4453, 48.1184));
    path.conic_to((32.4086, 47.8498), (32.6771, 48.8131), 0.707107);
    path.conic_to((32.9457, 49.7763), (31.9824, 50.0449), 0.707107);
    path.close();
    path.move_to((28.899, 50.706));
    path.quad_to((28.3834, 50.7842), (27.8652, 50.8416));
    path.conic_to((26.8713, 50.9518), (26.7611, 49.9579), 0.707107);
    path.conic_to((26.6509, 48.964), (27.6448, 48.8538), 0.707107);
    path.quad_to((28.1231, 48.8008), (28.599, 48.7286));
    path.conic_to((29.5877, 48.5786), (29.7377, 49.5673), 0.707107);
    path.conic_to((29.8877, 50.556), (28.899, 50.706), 0.707107);
    path.close();
    path.move_to((25.8106, 50.9874));
    path.quad_to((25.6321, 50.9929), (25.4537, 50.996));
    path.conic_to((24.4539, 51.0135), (24.4365, 50.0136), 0.707115);
    path.line_to((24.4251, 49.3638));
    path.conic_to((24.4077, 48.364), (25.4075, 48.3465), 0.707107);
    path.conic_to((26.4073, 48.3291), (26.4248, 49.3289), 0.707107);
    path.line_to((26.4361, 49.9787));
    path.line_to((25.4363, 49.9962));
    path.line_to((25.4189, 48.9963));
    path.quad_to((25.5836, 48.9935), (25.7482, 48.9883));
    path.conic_to((26.7477, 48.9571), (26.7789, 49.9567), 0.707107);
    path.conic_to((26.8101, 50.9562), (25.8106, 50.9874), 0.707107);
    path.close();
    path.move_to((24.3902, 47.3641));
    path.line_to((24.3728, 46.3643));
    path.conic_to((24.3553, 45.3645), (25.3551, 45.347), 0.707107);
    path.conic_to((26.355, 45.3295), (26.3724, 46.3294), 0.707107);
    path.line_to((26.3899, 47.3292));
    path.conic_to((26.4074, 48.3291), (25.4075, 48.3465), 0.707107);
    path.conic_to((24.4077, 48.364), (24.3902, 47.3641), 0.707107);
    path.close();
    path.move_to((24.3378, 44.3646));
    path.line_to((24.3204, 43.3648));
    path.conic_to((24.3029, 42.3649), (25.3028, 42.3475), 0.707107);
    path.conic_to((26.3026, 42.33), (26.3201, 43.3298), 0.707107);
    path.line_to((26.3375, 44.3297));
    path.conic_to((26.355, 45.3295), (25.3551, 45.347), 0.707107);
    path.conic_to((24.3553, 45.3645), (24.3378, 44.3646), 0.707107);
    path.close();
    path.move_to((24.2855, 41.3651));
    path.line_to((24.268, 40.3652));
    path.conic_to((24.2506, 39.3654), (25.2504, 39.3479), 0.707107);
    path.conic_to((26.2503, 39.3305), (26.2677, 40.3303), 0.707107);
    path.line_to((26.2852, 41.3302));
    path.conic_to((26.3026, 42.33), (25.3028, 42.3475), 0.707107);
    path.conic_to((24.3029, 42.3649), (24.2855, 41.3651), 0.707107);
    path.close();
    path.move_to((24.2331, 38.3655));
    path.line_to((24.2157, 37.3657));
    path.conic_to((24.1982, 36.3658), (25.1981, 36.3484), 0.707107);
    path.conic_to((26.1979, 36.3309), (26.2154, 37.3308), 0.707107);
    path.line_to((26.2328, 38.3306));
    path.conic_to((26.2503, 39.3305), (25.2504, 39.3479), 0.707107);
    path.conic_to((24.2506, 39.3654), (24.2331, 38.3655), 0.707107);
    path.close();
    path.move_to((24.1808, 35.366));
    path.line_to((24.1633, 34.3661));
    path.conic_to((24.1459, 33.3663), (25.1457, 33.3488), 0.707107);
    path.conic_to((26.1456, 33.3314), (26.163, 34.3312), 0.707107);
    path.line_to((26.1805, 35.3311));
    path.conic_to((26.1979, 36.3309), (25.1981, 36.3484), 0.707107);
    path.conic_to((24.1982, 36.3658), (24.1808, 35.366), 0.707107);
    path.close();
    path.move_to((24.1284, 32.3664));
    path.line_to((24.111, 31.3666));
    path.conic_to((24.0935, 30.3667), (25.0934, 30.3493), 0.707107);
    path.conic_to((26.0932, 30.3318), (26.1107, 31.3317), 0.707107);
    path.line_to((26.1281, 32.3315));
    path.conic_to((26.1456, 33.3314), (25.1457, 33.3488), 0.707107);
    path.conic_to((24.1459, 33.3663), (24.1284, 32.3664), 0.707107);
    path.close();
    path.move_to((24.0761, 29.3669));
    path.line_to((24.0586, 28.367));
    path.conic_to((24.0412, 27.3672), (25.041, 27.3497), 0.707107);
    path.conic_to((26.0409, 27.3323), (26.0583, 28.3321), 0.707107);
    path.line_to((26.0758, 29.332));
    path.conic_to((26.0932, 30.3318), (25.0934, 30.3493), 0.707107);
    path.conic_to((24.0935, 30.3667), (24.0761, 29.3669), 0.707107);
    path.close();
    path.move_to((24.0237, 26.3673));
    path.line_to((24.0063, 25.3675));
    path.conic_to((23.9888, 24.3676), (24.9887, 24.3502), 0.707107);
    path.conic_to((25.9885, 24.3327), (26.006, 25.3326), 0.707107);
    path.line_to((26.0234, 26.3324));
    path.conic_to((26.0409, 27.3323), (25.041, 27.3497), 0.707107);
    path.conic_to((24.0412, 27.3672), (24.0237, 26.3673), 0.707107);
    path.close();
    test_path_op_fail(reporter, &path.detach(), &path1, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L8948-L8964 (chrome/m156)
fn op_1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x15e80300), f32::from_bits(0x400004dc)));
    path.quad_to(
        (f32::from_bits(0xe56c206c), f32::from_bits(0x646c5f40)),
        (f32::from_bits(0x6c80885e), f32::from_bits(0xb4bc576c)),
    );
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x1b000010), f32::from_bits(0x6e5a5a1b)));
    path.quad_to(
        (f32::from_bits(0xef646464), f32::from_bits(0xefefefef)),
        (f32::from_bits(0x000000ef), f32::from_bits(0x1bb4bc00)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L8967-L8988 (chrome/m156)
fn op_2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xeee3ef57), f32::from_bits(0xef6300f8)));
    path.quad_to(
        (f32::from_bits(0xeeee9c6e), f32::from_bits(0xef609993)),
        (f32::from_bits(0x00000000), f32::from_bits(0x6e5a5a1b)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0xe56c206c), f32::from_bits(0x646c5f40)),
        (f32::from_bits(0x6c80885e), f32::from_bits(0x00000000)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0xeeda2c5a), f32::from_bits(0xef6533a7)),
        (f32::from_bits(0xeee3ef57), f32::from_bits(0xef6300f8)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x1b1b1b00), f32::from_bits(0x1b5a5a1b)));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L8991-L9013 (chrome/m156)
fn op_3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x6e5a5a1b)));
    path.quad_to(
        (f32::from_bits(0xeeee9c6e), f32::from_bits(0xef609993)),
        (f32::from_bits(0xeee3ef57), f32::from_bits(0xef6300f8)),
    );
    path.quad_to(
        (f32::from_bits(0xeeda2c5a), f32::from_bits(0xef6533a7)),
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x6e5a5a1b)));
    path.close();
    path.move_to((f32::from_bits(0x6c80885e), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0xe56c206c), f32::from_bits(0x646c5f40)),
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x6c80885e), f32::from_bits(0x00000000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9015-L9036 (chrome/m156)
fn op_4(reporter: &mut Reporter, filename: &str) {
    let mut b = PathBuilder::new_with_fill_type(PathFillType::EvenOdd);
    b.move_to((f32::from_bits(0x40d7ea90), f32::from_bits(0x3fa58930)));
    b.line_to((f32::from_bits(0x40ad3d93), f32::from_bits(0x3fa58930)));
    b.line_to((f32::from_bits(0x40ad3d93), f32::from_bits(0x3edba819)));
    b.line_to((f32::from_bits(0x40fc41e0), f32::from_bits(0x3edba819)));
    b.line_to((f32::from_bits(0x40fc41e0), f32::from_bits(0x3f3b7c94)));
    b.line_to((f32::from_bits(0x40d7ea90), f32::from_bits(0x3f3b7c94)));
    b.line_to((f32::from_bits(0x40d7ea90), f32::from_bits(0x3fa58930)));
    b.close();
    let patha = b.detach();
    let mut b = PathBuilder::new_with_fill_type(PathFillType::EvenOdd);
    b.move_to((f32::from_bits(0x40d7ea89), f32::from_bits(0x409a721d)));
    b.line_to((f32::from_bits(0x411a9d73), f32::from_bits(0x409a721d)));
    b.line_to((f32::from_bits(0x411a9d73), f32::from_bits(0x3f3b7c9a)));
    b.line_to((f32::from_bits(0x40d7ea89), f32::from_bits(0x3f3b7c9a)));
    b.line_to((f32::from_bits(0x40d7ea89), f32::from_bits(0x409a721d)));
    b.close();
    let pathb = b.detach();
    test_path_op(reporter, &patha, &pathb, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9038-L9075 (chrome/m156)
fn bug8228(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    path1.move_to((f32::from_bits(0x41fd5557), f32::from_bits(0x4292aaab)));
    path1.line_to((f32::from_bits(0x41fd5557), f32::from_bits(0x41555556)));
    path1.conic_to(
        (f32::from_bits(0x41fd5557), f32::from_bits(0x41200002)),
        (f32::from_bits(0x420c0000), f32::from_bits(0x41200002)),
        f32::from_bits(0x3f3504f3),
    );
    path1.line_to((f32::from_bits(0x426071c7), f32::from_bits(0x41200002)));
    path1.conic_to(
        (f32::from_bits(0x426dc71d), f32::from_bits(0x41200002)),
        (f32::from_bits(0x426dc71d), f32::from_bits(0x41555556)),
        f32::from_bits(0x3f3504f3),
    );
    path1.line_to((f32::from_bits(0x426dc71d), f32::from_bits(0x4292aaab)));
    path1.conic_to(
        (f32::from_bits(0x426dc71d), f32::from_bits(0x42995555)),
        (f32::from_bits(0x426071c7), f32::from_bits(0x42995555)),
        f32::from_bits(0x3f3504f3),
    );
    path1.line_to((f32::from_bits(0x420c0000), f32::from_bits(0x42995555)));
    path1.conic_to(
        (f32::from_bits(0x41fd5557), f32::from_bits(0x42995555)),
        (f32::from_bits(0x41fd5557), f32::from_bits(0x4292aaab)),
        f32::from_bits(0x3f3504f3),
    );
    path1.close();
    let mut path2 = PathBuilder::new();
    path2.move_to((f32::from_bits(0x41200000), f32::from_bits(0x41200000)));
    path2.line_to((f32::from_bits(0x41eb2366), f32::from_bits(0x41200000)));
    path2.conic_to(
        (f32::from_bits(0x41e9d2b6), f32::from_bits(0x4127bdec)),
        (f32::from_bits(0x41e9d2b6), f32::from_bits(0x412feb1c)),
        f32::from_bits(0x3f7c9333),
    );
    path2.line_to((f32::from_bits(0x41e9d2b6), f32::from_bits(0x42855349)));
    path2.conic_to(
        (f32::from_bits(0x41e9d2b6), f32::from_bits(0x428b82b9)),
        (f32::from_bits(0x4201483b), f32::from_bits(0x428b82b9)),
        f32::from_bits(0x3f3504f3),
    );
    path2.line_to((f32::from_bits(0x424fa11f), f32::from_bits(0x428b82b9)));
    path2.conic_to(
        (f32::from_bits(0x425bffff), f32::from_bits(0x428b82b9)),
        (f32::from_bits(0x425bffff), f32::from_bits(0x42855349)),
        f32::from_bits(0x3f3504f3),
    );
    path2.line_to((f32::from_bits(0x425bffff), f32::from_bits(0x412feb1c)));
    path2.conic_to(
        (f32::from_bits(0x425bffff), f32::from_bits(0x4127bdec)),
        (f32::from_bits(0x425b57a7), f32::from_bits(0x41200000)),
        f32::from_bits(0x3f7c9333),
    );
    path2.line_to((f32::from_bits(0x4282f24d), f32::from_bits(0x41200000)));
    path2.conic_to(
        (f32::from_bits(0x42829e21), f32::from_bits(0x4127bdec)),
        (f32::from_bits(0x42829e21), f32::from_bits(0x412feb1c)),
        f32::from_bits(0x3f7c9333),
    );
    path2.line_to((f32::from_bits(0x42829e21), f32::from_bits(0x42855349)));
    path2.conic_to(
        (f32::from_bits(0x42829e21), f32::from_bits(0x428b82b9)),
        (f32::from_bits(0x4288cd91), f32::from_bits(0x428b82b9)),
        f32::from_bits(0x3f3504f3),
    );
    path2.line_to((f32::from_bits(0x42affa03), f32::from_bits(0x428b82b9)));
    path2.conic_to(
        (f32::from_bits(0x42b62973), f32::from_bits(0x428b82b9)),
        (f32::from_bits(0x42b62973), f32::from_bits(0x42855349)),
        f32::from_bits(0x3f3504f3),
    );
    path2.line_to((f32::from_bits(0x42b62973), f32::from_bits(0x412feb1c)));
    path2.conic_to(
        (f32::from_bits(0x42b62973), f32::from_bits(0x4127bdec)),
        (f32::from_bits(0x42b5d547), f32::from_bits(0x41200000)),
        f32::from_bits(0x3f7c9333),
    );
    path2.line_to((f32::from_bits(0x42dc0000), f32::from_bits(0x41200000)));
    path2.line_to((f32::from_bits(0x42dc0000), f32::from_bits(0x42dc0000)));
    path2.line_to((f32::from_bits(0x41200000), f32::from_bits(0x42dc0000)));
    path2.line_to((f32::from_bits(0x41200000), f32::from_bits(0x41200000)));
    path2.close();
    test_path_op(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L9077-L9100 (chrome/m156)
fn bug8380(reporter: &mut Reporter, filename: &str) {
    let mut b = PathBuilder::new_with_fill_type(PathFillType::EvenOdd);
    b.move_to((f32::from_bits(0xa6800000), f32::from_bits(0x43b0f22d)));
    b.line_to((f32::from_bits(0x42fc0000), f32::from_bits(0x4116566d)));
    b.cubic_to(
        (f32::from_bits(0x42fb439d), f32::from_bits(0x4114bbc7)),
        (f32::from_bits(0x42fa3ed7), f32::from_bits(0x411565bd)),
        (f32::from_bits(0x42f934d2), f32::from_bits(0x4116131e)),
    );
    b.cubic_to(
        (f32::from_bits(0x42f84915), f32::from_bits(0x4116acc3)),
        (f32::from_bits(0x42f75939), f32::from_bits(0x41174918)),
        (f32::from_bits(0x42f693f8), f32::from_bits(0x4116566d)),
    );
    b.line_to((f32::from_bits(0x42ec3cee), f32::from_bits(0x410127bb)));
    b.line_to((f32::from_bits(0x4102c0ec), f32::from_bits(0x42d06d0e)));
    b.line_to((f32::from_bits(0xa6000000), f32::from_bits(0x4381a63d)));
    b.line_to((f32::from_bits(0x00000000), f32::from_bits(0x43b0f22d)));
    b.line_to((f32::from_bits(0xa6800000), f32::from_bits(0x43b0f22d)));
    b.close();
    let path = b.detach();
    let mut b = PathBuilder::new_with_fill_type(PathFillType::EvenOdd);
    b.move_to((f32::from_bits(0x4102c0ec), f32::from_bits(0x42d06d0e)));
    b.line_to((f32::from_bits(0xc0ba5a1d), f32::from_bits(0x43b8e831)));
    b.line_to((f32::from_bits(0x42fc0000), f32::from_bits(0x411656d6)));
    b.cubic_to(
        (f32::from_bits(0x42fa9cac), f32::from_bits(0x41134fdf)),
        (f32::from_bits(0x42f837cf), f32::from_bits(0x41185aee)),
        (f32::from_bits(0x42f693f8), f32::from_bits(0x411656d6)),
    );
    b.line_to((f32::from_bits(0x42ec3cee), f32::from_bits(0x410127bb)));
    b.line_to((f32::from_bits(0x4102c0ec), f32::from_bits(0x42d06d0e)));
    b.close();
    let path2 = b.detach();
    test_path_op(reporter, &path, &path2, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9528-L9631 (chrome/m156)
fn fuzz767834(reporter: &mut Reporter, filename: &str) {
    let mut one = PathBuilder::new();
    one.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    one.conic_to(
        (f32::from_bits(0x02807252), f32::from_bits(0xee23000a)),
        (f32::from_bits(0x00000000), f32::from_bits(0x0fe00008)),
        f32::from_bits(0x52526831),
    );
    one.cubic_to(
        (f32::from_bits(0x474d475a), f32::from_bits(0x72727252)),
        (f32::from_bits(0x72267272), f32::from_bits(0x535202ff)),
        (f32::from_bits(0x53535353), f32::from_bits(0x58943353)),
    );
    one.quad_to(
        (f32::from_bits(0x52727272), f32::from_bits(0x52595252)),
        (f32::from_bits(0x8e460900), f32::from_bits(0x7272db72)),
    );
    one.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    one.close();
    one.move_to((f32::from_bits(0x72000400), f32::from_bits(0x72727272)));
    one.quad_to(
        (f32::from_bits(0x60727272), f32::from_bits(0x72727272)),
        (f32::from_bits(0x2a527272), f32::from_bits(0x72525252)),
    );
    one.cubic_to(
        (f32::from_bits(0x72727251), f32::from_bits(0x52617272)),
        (f32::from_bits(0x46032352), f32::from_bits(0x7272728e)),
        (f32::from_bits(0x5c527272), f32::from_bits(0x72726552)),
    );
    one.cubic_to(
        (f32::from_bits(0x2b7280ff), f32::from_bits(0x7240ffff)),
        (f32::from_bits(0x72724960), f32::from_bits(0x52008072)),
        (f32::from_bits(0x72725230), f32::from_bits(0x5f727272)),
    );
    one.line_to((f32::from_bits(0x72000400), f32::from_bits(0x72727272)));
    one.close();
    one.move_to((f32::from_bits(0x8e524603), f32::from_bits(0x72727272)));
    one.close();
    one.move_to((f32::from_bits(0x8e524603), f32::from_bits(0x72727272)));
    one.quad_to(
        (f32::from_bits(0x72725d72), f32::from_bits(0x52008072)),
        (f32::from_bits(0x00016552), f32::from_bits(0x72724000)),
    );
    one.quad_to(
        (f32::from_bits(0x00807272), f32::from_bits(0x392a5b25)),
        (f32::from_bits(0x72685768), f32::from_bits(0x000000ff)),
    );
    one.move_to((f32::from_bits(0xe0e060e0), f32::from_bits(0x728f5740)));
    one.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0xd2008072)),
        (f32::from_bits(0x8e460900), f32::from_bits(0x72727072)),
    );
    one.cubic_to(
        (f32::from_bits(0xe0e060e0), f32::from_bits(0x58943303)),
        (f32::from_bits(0x72727272), f32::from_bits(0x59525252)),
        (f32::from_bits(0x00090052), f32::from_bits(0x72000000)),
    );
    one.quad_to(
        (f32::from_bits(0x005252ec), f32::from_bits(0x72000400)),
        (f32::from_bits(0x72727272), f32::from_bits(0x72727272)),
    );
    one.line_to((f32::from_bits(0xe0e060e0), f32::from_bits(0x728f5740)));
    one.close();
    one.move_to((f32::from_bits(0xe0e060e0), f32::from_bits(0x728f5740)));
    one.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x522a5272)),
        (f32::from_bits(0x20725252), f32::from_bits(0x72727251)),
    );
    one.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x59525252)),
        (f32::from_bits(0x46090052), f32::from_bits(0x72db728e)),
    );
    one.quad_to(
        (f32::from_bits(0x005252ec), f32::from_bits(0x72000400)),
        (f32::from_bits(0x72727272), f32::from_bits(0x72727272)),
    );
    one.line_to((f32::from_bits(0xe0e060e0), f32::from_bits(0x728f5740)));
    one.close();
    one.move_to((f32::from_bits(0xe0e060e0), f32::from_bits(0x728f5740)));
    one.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x522a5272)),
        (f32::from_bits(0x20725252), f32::from_bits(0x72727251)),
    );
    one.quad_to(
        (f32::from_bits(0x52526172), f32::from_bits(0x8e460323)),
        (f32::from_bits(0x72727272), f32::from_bits(0x525c5272)),
    );
    one.conic_to(
        (f32::from_bits(0xff727272), f32::from_bits(0xff2b549b)),
        (f32::from_bits(0x607240ff), f32::from_bits(0x72727249)),
        f32::from_bits(0x30520080),
    );
    one.line_to((f32::from_bits(0xe0e060e0), f32::from_bits(0x728f5740)));
    one.close();
    one.move_to((f32::from_bits(0xe0e060e0), f32::from_bits(0x728f5740)));
    one.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x0052525f)),
        (f32::from_bits(0x8e524603), f32::from_bits(0x72727272)),
    );
    one.line_to((f32::from_bits(0xe0e060e0), f32::from_bits(0x728f5740)));
    one.close();
    one.move_to((f32::from_bits(0xe0e060e0), f32::from_bits(0x728f5740)));
    one.quad_to(
        (f32::from_bits(0x72725d72), f32::from_bits(0x52008072)),
        (f32::from_bits(0x00016552), f32::from_bits(0x72724000)),
    );
    one.quad_to(
        (f32::from_bits(0x00807272), f32::from_bits(0x392a5b25)),
        (f32::from_bits(0x72685768), f32::from_bits(0x000000ff)),
    );
    one.move_to((f32::from_bits(0xe0e060e0), f32::from_bits(0x728f5740)));
    one.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0xd2008072)),
        (f32::from_bits(0x8e460900), f32::from_bits(0x72727072)),
    );
    one.cubic_to(
        (f32::from_bits(0xe0e060e0), f32::from_bits(0x58943303)),
        (f32::from_bits(0x72727272), f32::from_bits(0x59525252)),
        (f32::from_bits(0x46090052), f32::from_bits(0x72db728e)),
    );
    one.quad_to(
        (f32::from_bits(0x005252ec), f32::from_bits(0x72000400)),
        (f32::from_bits(0x72727272), f32::from_bits(0x72727272)),
    );
    one.line_to((f32::from_bits(0xe0e060e0), f32::from_bits(0x728f5740)));
    one.close();
    one.move_to((f32::from_bits(0xe0e060e0), f32::from_bits(0x728f5740)));
    one.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x522a5272)),
        (f32::from_bits(0x20725252), f32::from_bits(0x72727251)),
    );
    test_path_op_fuzz(
        reporter,
        &Path::new(),
        &one.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L9633-L9641 (chrome/m156)
fn fuzz535151(reporter: &mut Reporter, filename: &str) {
    let one = Path::new_with_fill_type(PathFillType::Winding);
    let mut b = PathBuilder::new_with_fill_type(PathFillType::Winding);
    b.move_to((0.0, 0.0));
    b.line_to((0.0, 50.0));
    b.line_to((4.29497e+09, 50.0));
    let two = b.detach();
    test_path_op_fuzz(reporter, &one, &two, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9643-L9647 (chrome/m156)
fn buffer_overflow(reporter: &mut Reporter, filename: &str) {
    let path = Path::rect(
        Rect::new(0.0, 0.0, 300.0, 170141183460469231731687303715884105728.0),
        PathDirection::CW,
    );
    let path_b = Path::rect(Rect::new(0.0, 0.0, 300.0, 16.0), PathDirection::CW);
    test_path_op_fuzz(reporter, &path, &path_b, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9650-L9667 (chrome/m156)
fn fuzz433(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    let mut path2 = PathBuilder::new();
    path1.move_to((100.0, 0.0));
    path1.line_to((60.0, 170.0));
    path1.line_to((-160.0, -110.0));
    path1.line_to((200.0, 0.0));
    path1.line_to((-170.0, 11000000000.0));
    path1.close();
    path2.move_to((100.0 + 20.0, 0.0 + 20.0));
    path2.line_to((60.0 + 20.0, 170.0 + 20.0));
    path2.line_to((-160.0 + 20.0, -110.0 + 20.0));
    path2.line_to((200.0 + 20.0, 0.0 + 20.0));
    path2.line_to((-170.0 + 20.0, 11000000000.0 + 20.0));
    path2.close();
    test_path_op_fuzz(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Intersect,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L9669-L9690 (chrome/m156)
fn fuzz433b(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    let mut path2 = PathBuilder::new();
    path1.set_fill_type(PathFillType::EvenOdd);
    path1.move_to((140.0, 40.0));
    path1.line_to((200.0, 210.0));
    path1.line_to((40.0, 100.0));
    path1.line_to((240.0, 100.0));
    path1.line_to((70.0, 1.1e+10));
    path1.line_to((140.0, 40.0));
    path1.close();
    path1.set_fill_type(PathFillType::Winding);
    path2.move_to((190.0, 60.0));
    path2.line_to((250.0, 230.0));
    path2.line_to((90.0, 120.0));
    path2.line_to((290.0, 120.0));
    path2.line_to((120.0, 1.1e+10));
    path2.line_to((190.0, 60.0));
    path2.close();
    test_path_op_fuzz(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L9692-L9736 (chrome/m156)
fn fuzz487a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x432c8000), f32::from_bits(0x42c00000)));
    path.line_to((f32::from_bits(0x4309999a), f32::from_bits(0x42c00000)));
    path.cubic_to(
        (f32::from_bits(0x4309999a), f32::from_bits(0x429a6666)),
        (f32::from_bits(0x42f9999a), f32::from_bits(0x4275999a)),
        (f32::from_bits(0x42d70001), f32::from_bits(0x42633333)),
    );
    path.line_to((f32::from_bits(0x42e90001), f32::from_bits(0x41b8cccc)));
    path.cubic_to(
        (f32::from_bits(0x42dc6667), f32::from_bits(0x41ab3332)),
        (f32::from_bits(0x42cf3334), f32::from_bits(0x41a3ffff)),
        (f32::from_bits(0x42c20001), f32::from_bits(0x41a3ffff)),
    );
    path.line_to((f32::from_bits(0x42c20001), f32::from_bits(0x425d999a)));
    path.line_to((f32::from_bits(0x42c20001), f32::from_bits(0x425d999a)));
    path.cubic_to(
        (f32::from_bits(0x429c6668), f32::from_bits(0x425d999a)),
        (f32::from_bits(0x4279999c), f32::from_bits(0x42886667)),
        (f32::from_bits(0x42673335), f32::from_bits(0x42ab0000)),
    );
    path.line_to((f32::from_bits(0x41c0ccd0), f32::from_bits(0x42990000)));
    path.cubic_to(
        (f32::from_bits(0x41b33336), f32::from_bits(0x42a5999a)),
        (f32::from_bits(0x41ac0003), f32::from_bits(0x42b2cccd)),
        (f32::from_bits(0x41ac0003), f32::from_bits(0x42c00000)),
    );
    path.line_to((f32::from_bits(0x4261999c), f32::from_bits(0x42c00000)));
    path.line_to((f32::from_bits(0x4261999c), f32::from_bits(0x42c00000)));
    path.cubic_to(
        (f32::from_bits(0x4261999c), f32::from_bits(0x434d3333)),
        (f32::from_bits(0x4364e667), f32::from_bits(0x4346b333)),
        (f32::from_bits(0x4364e667), f32::from_bits(0x43400000)),
    );
    path.line_to((f32::from_bits(0x432c8000), f32::from_bits(0x42c00000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x432c8000), f32::from_bits(0x42c00000)));
    path.line_to((f32::from_bits(0x4309999a), f32::from_bits(0x42c00000)));
    path.cubic_to(
        (f32::from_bits(0x4309999a), f32::from_bits(0x42a20000)),
        (f32::from_bits(0x43016667), f32::from_bits(0x4287cccd)),
        (f32::from_bits(0x42ea999a), f32::from_bits(0x4273999a)),
    );
    path.line_to((f32::from_bits(0x4306cccd), f32::from_bits(0x41f5999a)));
    path.cubic_to(
        (f32::from_bits(0x42f76667), f32::from_bits(0x41c26667)),
        (f32::from_bits(0x42dd999a), f32::from_bits(0x41a4cccd)),
        (f32::from_bits(0x42c23334), f32::from_bits(0x41a4cccd)),
    );
    path.line_to((f32::from_bits(0x42c23334), f32::from_bits(0x425e0000)));
    path.cubic_to(
        (f32::from_bits(0x42a43334), f32::from_bits(0x425e0000)),
        (f32::from_bits(0x428a0001), f32::from_bits(0x427ecccd)),
        (f32::from_bits(0x42780002), f32::from_bits(0x4297999a)),
    );
    path.line_to((f32::from_bits(0x41fccccd), f32::from_bits(0x42693333)));
    path.cubic_to(
        (f32::from_bits(0x41c9999a), f32::from_bits(0x428acccd)),
        (f32::from_bits(0x41ac0000), f32::from_bits(0x42a4999a)),
        (f32::from_bits(0x41ac0000), f32::from_bits(0x42c00000)),
    );
    path.line_to((f32::from_bits(0x4261999a), f32::from_bits(0x42c00000)));
    path.cubic_to(
        (f32::from_bits(0x4261999a), f32::from_bits(0x42de0000)),
        (f32::from_bits(0x42813333), f32::from_bits(0x42f83333)),
        (f32::from_bits(0x42996666), f32::from_bits(0x4303199a)),
    );
    path.cubic_to(
        (f32::from_bits(0x4272cccc), f32::from_bits(0x4303199a)),
        (f32::from_bits(0x423d3332), f32::from_bits(0x430de667)),
        (f32::from_bits(0x422d9999), f32::from_bits(0x431cb334)),
    );
    path.line_to((f32::from_bits(0x7086a1dc), f32::from_bits(0x42eecccd)));
    path.line_to((f32::from_bits(0x41eb3333), f32::from_bits(0xc12ccccd)));
    path.line_to((f32::from_bits(0x42053333), f32::from_bits(0xc1cccccd)));
    path.line_to((f32::from_bits(0x42780000), f32::from_bits(0xc18f3334)));
    path.cubic_to(
        (f32::from_bits(0x43206666), f32::from_bits(0x43134ccd)),
        (f32::from_bits(0x43213333), f32::from_bits(0x430db333)),
        (f32::from_bits(0x43213333), f32::from_bits(0x43080000)),
    );
    path.line_to((f32::from_bits(0x432c8000), f32::from_bits(0x42c00000)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9738-L9782 (chrome/m156)
fn fuzz487b(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x432c8000), f32::from_bits(0x42c00000)));
    path.line_to((f32::from_bits(0x4309999a), f32::from_bits(0x42c00000)));
    path.cubic_to(
        (f32::from_bits(0x4309999a), f32::from_bits(0x429a6666)),
        (f32::from_bits(0x42f9999a), f32::from_bits(0x4275999a)),
        (f32::from_bits(0x42d70001), f32::from_bits(0x42633333)),
    );
    path.line_to((f32::from_bits(0x42e90001), f32::from_bits(0x41b8cccc)));
    path.cubic_to(
        (f32::from_bits(0x42dc6667), f32::from_bits(0x41ab3332)),
        (f32::from_bits(0x42cf3334), f32::from_bits(0x41a3ffff)),
        (f32::from_bits(0x42c20001), f32::from_bits(0x41a3ffff)),
    );
    path.line_to((f32::from_bits(0x42c20001), f32::from_bits(0x425d999a)));
    path.line_to((f32::from_bits(0x42c20001), f32::from_bits(0x425d999a)));
    path.cubic_to(
        (f32::from_bits(0x429c6668), f32::from_bits(0x425d999a)),
        (f32::from_bits(0x4279999c), f32::from_bits(0x42886667)),
        (f32::from_bits(0x42673335), f32::from_bits(0x42ab0000)),
    );
    path.line_to((f32::from_bits(0x41c0ccd0), f32::from_bits(0x42990000)));
    path.cubic_to(
        (f32::from_bits(0x41b33336), f32::from_bits(0x42a5999a)),
        (f32::from_bits(0x41ac0003), f32::from_bits(0x42b2cccd)),
        (f32::from_bits(0x41ac0003), f32::from_bits(0x42c00000)),
    );
    path.line_to((f32::from_bits(0x4261999c), f32::from_bits(0x42c00000)));
    path.line_to((f32::from_bits(0x4261999c), f32::from_bits(0x42c00000)));
    path.cubic_to(
        (f32::from_bits(0x4261999c), f32::from_bits(0x434d3333)),
        (f32::from_bits(0x4364e667), f32::from_bits(0x4346b333)),
        (f32::from_bits(0x4364e667), f32::from_bits(0x43400000)),
    );
    path.line_to((f32::from_bits(0x432c8000), f32::from_bits(0x42c00000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x432c8000), f32::from_bits(0x42c00000)));
    path.line_to((f32::from_bits(0x4309999a), f32::from_bits(0x42c00000)));
    path.cubic_to(
        (f32::from_bits(0x4309999a), f32::from_bits(0x42a20000)),
        (f32::from_bits(0x43016667), f32::from_bits(0x4287cccd)),
        (f32::from_bits(0x42ea999a), f32::from_bits(0x4273999a)),
    );
    path.line_to((f32::from_bits(0x4306cccd), f32::from_bits(0x41f5999a)));
    path.cubic_to(
        (f32::from_bits(0x42f76667), f32::from_bits(0x41c26667)),
        (f32::from_bits(0x42dd999a), f32::from_bits(0x41a4cccd)),
        (f32::from_bits(0x42c23334), f32::from_bits(0x41a4cccd)),
    );
    path.line_to((f32::from_bits(0x42c23334), f32::from_bits(0x425e0000)));
    path.cubic_to(
        (f32::from_bits(0x42a43334), f32::from_bits(0x425e0000)),
        (f32::from_bits(0x428a0001), f32::from_bits(0x427ecccd)),
        (f32::from_bits(0x42780002), f32::from_bits(0x4297999a)),
    );
    path.line_to((f32::from_bits(0x41fccccd), f32::from_bits(0x42693333)));
    path.cubic_to(
        (f32::from_bits(0x41c9999a), f32::from_bits(0x428acccd)),
        (f32::from_bits(0x41ac0000), f32::from_bits(0x42a4999a)),
        (f32::from_bits(0x41ac0000), f32::from_bits(0x42c00000)),
    );
    path.line_to((f32::from_bits(0x4261999a), f32::from_bits(0x42c00000)));
    path.cubic_to(
        (f32::from_bits(0x4261999a), f32::from_bits(0x42de0000)),
        (f32::from_bits(0x42813333), f32::from_bits(0x42f83333)),
        (f32::from_bits(0x42996666), f32::from_bits(0x4303199a)),
    );
    path.cubic_to(
        (f32::from_bits(0x4272cccc), f32::from_bits(0x4303199a)),
        (f32::from_bits(0x423d3332), f32::from_bits(0x430de667)),
        (f32::from_bits(0x422d9999), f32::from_bits(0x431cb334)),
    );
    path.line_to((f32::from_bits(0x7086a1dc), f32::from_bits(0x42eecccd)));
    path.line_to((f32::from_bits(0x41eb3333), f32::from_bits(0xc12ccccd)));
    path.line_to((f32::from_bits(0x42053333), f32::from_bits(0xc1cccccd)));
    path.line_to((f32::from_bits(0x42780000), f32::from_bits(0xc18f3334)));
    path.cubic_to(
        (f32::from_bits(0x43206666), f32::from_bits(0x43134ccd)),
        (f32::from_bits(0x43213333), f32::from_bits(0x430db333)),
        (f32::from_bits(0x43213333), f32::from_bits(0x43080000)),
    );
    path.line_to((f32::from_bits(0x432c8000), f32::from_bits(0x42c00000)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9784-L9808 (chrome/m156)
fn fuzz714(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x430c0000), f32::from_bits(0x42200000)));
    path.line_to((f32::from_bits(0x43480000), f32::from_bits(0x43520000)));
    path.line_to((f32::from_bits(0x42200000), f32::from_bits(0x42c80000)));
    path.line_to((f32::from_bits(0x64969569), f32::from_bits(0x42c80000)));
    path.line_to((f32::from_bits(0x64969569), f32::from_bits(0x43520000)));
    path.line_to((f32::from_bits(0x430c0000), f32::from_bits(0x42200000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x43200000), f32::from_bits(0x42700000)));
    path.line_to((f32::from_bits(0x435c0000), f32::from_bits(0x43660000)));
    path.line_to((f32::from_bits(0x42700000), f32::from_bits(0x42f00000)));
    path.line_to((f32::from_bits(0x64969569), f32::from_bits(0x42f00000)));
    path.line_to((f32::from_bits(0x64969569), f32::from_bits(0x43660000)));
    path.line_to((f32::from_bits(0x43200000), f32::from_bits(0x42700000)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9810-L9830 (chrome/m156)
fn fuzz1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x7f800000), f32::from_bits(0x7f800000)));
    path.quad_to(
        (f32::from_bits(0x7f800000), f32::from_bits(0x7f800000)),
        (f32::from_bits(0x7f800000), f32::from_bits(0x7f800000)),
    );
    path.quad_to(
        (f32::from_bits(0x7f800000), f32::from_bits(0x7f800000)),
        (f32::from_bits(0x7f800000), f32::from_bits(0x7f800000)),
    );
    path.quad_to(
        (f32::from_bits(0xffc00000), f32::from_bits(0x7f800000)),
        (f32::from_bits(0xffc00000), f32::from_bits(0x7f800000)),
    );
    path.quad_to(
        (f32::from_bits(0xff000001), f32::from_bits(0x7f800000)),
        (f32::from_bits(0xff000001), f32::from_bits(0x7f800000)),
    );
    path.quad_to(
        (f32::from_bits(0xff000001), f32::from_bits(0xffc00000)),
        (f32::from_bits(0xffc00000), f32::from_bits(0xffc00000)),
    );
    path.quad_to(
        (f32::from_bits(0xffc00000), f32::from_bits(0xff000001)),
        (f32::from_bits(0x7f800000), f32::from_bits(0xff000001)),
    );
    path.quad_to(
        (f32::from_bits(0x7f800000), f32::from_bits(0xff000001)),
        (f32::from_bits(0x7f800000), f32::from_bits(0xffc00000)),
    );
    path.quad_to(
        (f32::from_bits(0x7f800000), f32::from_bits(0xffc00000)),
        (f32::from_bits(0x7f800000), f32::from_bits(0x7f800000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9833-L9855 (chrome/m156)
fn fuzz753_91(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x42910000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x42166668), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0x42166668), f32::from_bits(0xc1966668)),
        (f32::from_bits(0x41c66668), f32::from_bits(0xc20a6666)),
        (f32::from_bits(0x40f00010), f32::from_bits(0xc21ccccd)),
    );
    path.line_to((f32::from_bits(0x41840004), f32::from_bits(0xc291cccd)));
    path.line_to((f32::from_bits(0x42fb6668), f32::from_bits(0x42c73334)));
    path.line_to((f32::from_bits(0x43646668), f32::from_bits(0x43880ccd)));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x428bf702), f32::from_bits(0xcf223cbf)));
    path.line_to((f32::from_bits(0x42112d68), f32::from_bits(0xcf223cbf)));
    path.cubic_to(
        (f32::from_bits(0x4220d9fc), f32::from_bits(0xcf223cc0)),
        (f32::from_bits(0x420ee118), f32::from_bits(0xcf223cc0)),
        (f32::from_bits(0x41cef2f8), f32::from_bits(0xcf223cc0)),
    );
    path.line_to((f32::from_bits(0x424a99e0), f32::from_bits(0xcf223cc0)));
    path.cubic_to(
        (f32::from_bits(0x42266e32), f32::from_bits(0xcf223cc0)),
        (f32::from_bits(0x41f0fa20), f32::from_bits(0xcf223cc0)),
        (f32::from_bits(0x41872ed4), f32::from_bits(0xcf223cc0)),
    );
    path.line_to((f32::from_bits(0x40f8fbe0), f32::from_bits(0xcf223cc0)));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9857-L9878 (chrome/m156)
fn bug597926_0(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x43b38000), f32::from_bits(0x433e0000)));
    path.line_to((f32::from_bits(0x40c00000), f32::from_bits(0x449ce000)));
    path.cubic_to(
        (f32::from_bits(0x438c0000), f32::from_bits(0x4497a000)),
        (f32::from_bits(0x43e40000), f32::from_bits(0x44750000)),
        (f32::from_bits(0x41000000), f32::from_bits(0x44aa2000)),
    );
    path.move_to((f32::from_bits(0x43290000), f32::from_bits(0x4431c000)));
    path.line_to((f32::from_bits(0xd987d6ba), f32::from_bits(0xd93d0ad4)));
    path.conic_to(
        (f32::from_bits(0x43cc8000), f32::from_bits(0x445b8000)),
        (f32::from_bits(0xd888b096), f32::from_bits(0xd9a1ebfa)),
        f32::from_bits(0x3ebcb199),
    );
    path.cubic_to(
        (f32::from_bits(0x43c00000), f32::from_bits(0x443a8000)),
        (f32::from_bits(0x42380000), f32::from_bits(0x4421c000)),
        (f32::from_bits(0x42500000), f32::from_bits(0x448ca000)),
    );
    path.quad_to(
        (f32::from_bits(0x43948000), f32::from_bits(0x42ac0000)),
        (f32::from_bits(0x43880000), f32::from_bits(0x4487e000)),
    );
    let path1 = path.detach();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0xc51d735c), f32::from_bits(0xc49db029)));
    path.cubic_to(
        (f32::from_bits(0xc51d1dbd), f32::from_bits(0xc49d7a3f)),
        (f32::from_bits(0xc51c524a), f32::from_bits(0xc49d1610)),
        (f32::from_bits(0xc51d1a96), f32::from_bits(0xc49d86a6)),
    );
    path.cubic_to(
        (f32::from_bits(0xc51cd471), f32::from_bits(0xc49d54d0)),
        (f32::from_bits(0xc51c2e51), f32::from_bits(0xc49d0081)),
        (f32::from_bits(0xc51d197b), f32::from_bits(0xc49d7927)),
    );
    path.quad_to(
        (f32::from_bits(0xc51bf7eb), f32::from_bits(0xc49cf010)),
        (f32::from_bits(0xc51ba866), f32::from_bits(0xc49cb9e6)),
    );
    path.cubic_to(
        (f32::from_bits(0xc51bac0d), f32::from_bits(0xc49cc50e)),
        (f32::from_bits(0xc51c29eb), f32::from_bits(0xc49cfb01)),
        (f32::from_bits(0xc51c5bca), f32::from_bits(0xc49d1fa6)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9880-L9897 (chrome/m156)
fn fuzz1450_0(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x43b40000), f32::from_bits(0xcf000000)));
    path.conic_to(
        (f32::from_bits(0x4e800002), f32::from_bits(0xcf000000)),
        (f32::from_bits(0x4e800002), f32::from_bits(0xce7ffffe)),
        f32::from_bits(0x3f3504f4),
    );
    path.conic_to(
        (f32::from_bits(0x4e800002), f32::from_bits(0x43800001)),
        (f32::from_bits(0x43348000), f32::from_bits(0x43800001)),
        f32::from_bits(0x3f3504f4),
    );
    let path1 = path.detach();
    path.move_to((f32::from_bits(0x43b40000), f32::from_bits(0x45816000)));
    path.conic_to(
        (f32::from_bits(0x43b40005), f32::from_bits(0x458a945d)),
        (f32::from_bits(0x45610000), f32::from_bits(0x458a945d)),
        f32::from_bits(0x3f3504f3),
    );
    path.conic_to(
        (f32::from_bits(0x45d5bfff), f32::from_bits(0x458a945d)),
        (f32::from_bits(0x45d5bfff), f32::from_bits(0x45816000)),
        f32::from_bits(0x3f3504f3),
    );
    path.line_to((f32::from_bits(0x42c80000), f32::from_bits(0x44000000)));
    path.line_to((f32::from_bits(0x42000000), f32::from_bits(0x41800000)));
    path.line_to((f32::from_bits(0x43b40000), f32::from_bits(0x44800000)));
    path.line_to((f32::from_bits(0x43b40000), f32::from_bits(0x45816000)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9899-L9926 (chrome/m156)
fn fuzz1450_1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x4e800002), f32::from_bits(0xce7ffffe)));
    path.conic_to(
        (f32::from_bits(0x4e800002), f32::from_bits(0xcf000000)),
        (f32::from_bits(0x43b40000), f32::from_bits(0xcf000000)),
        f32::from_bits(0x3f3504f4),
    );
    path.line_to((f32::from_bits(0x43348000), f32::from_bits(0x43800001)));
    path.line_to((f32::from_bits(0x42000000), f32::from_bits(0x41800000)));
    path.line_to((f32::from_bits(0x42c80000), f32::from_bits(0x44000000)));
    path.line_to((f32::from_bits(0x43553abd), f32::from_bits(0x440f3cbd)));
    path.line_to((f32::from_bits(0x43b40000), f32::from_bits(0x44800000)));
    path.line_to((f32::from_bits(0x43b40000), f32::from_bits(0x45816000)));
    path.conic_to(
        (f32::from_bits(0x43b40005), f32::from_bits(0x458a945d)),
        (f32::from_bits(0x45610000), f32::from_bits(0x458a945d)),
        f32::from_bits(0x3f3504f3),
    );
    path.conic_to(
        (f32::from_bits(0x45d5bfff), f32::from_bits(0x458a945d)),
        (f32::from_bits(0x45d5bfff), f32::from_bits(0x45816000)),
        f32::from_bits(0x3f3504f3),
    );
    path.line_to((f32::from_bits(0x43553abd), f32::from_bits(0x440f3cbd)));
    path.line_to((f32::from_bits(0x43348000), f32::from_bits(0x43800001)));
    path.conic_to(
        (f32::from_bits(0x4e800002), f32::from_bits(0x43800001)),
        (f32::from_bits(0x4e800002), f32::from_bits(0xce7ffffe)),
        f32::from_bits(0x3f3504f4),
    );
    path.close();
    let path1 = path.detach();
    path.move_to((f32::from_bits(0x42fe0000), f32::from_bits(0x43a08000)));
    path.line_to((f32::from_bits(0x45d5c000), f32::from_bits(0x43870000)));
    path.line_to((f32::from_bits(0xd0a00000), f32::from_bits(0x4cbebc20)));
    path.line_to((f32::from_bits(0x451f7000), f32::from_bits(0x42800000)));
    path.line_to((f32::from_bits(0x42fe0000), f32::from_bits(0x43a08000)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9928-L9953 (chrome/m156)
fn fuzz763_9(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x081f2a21)),
        (f32::from_bits(0x7bc00321), f32::from_bits(0xed7a6a4b)),
        f32::from_bits(0x1f212a8c),
    );
    path.line_to((f32::from_bits(0x7bc00321), f32::from_bits(0xed7a6a4b)));
    path.line_to((f32::from_bits(0x282a3a21), f32::from_bits(0x3a21df28)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x8a284f9a), f32::from_bits(0x3ac23ab3)),
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x63962be6)),
    );
    path.move_to((f32::from_bits(0x29272a81), f32::from_bits(0x2ab03a55)));
    path.quad_to(
        (f32::from_bits(0x2720213b), f32::from_bits(0x3a214729)),
        (f32::from_bits(0xdf28282a), f32::from_bits(0x8a2f2121)),
    );
    path.quad_to(
        (f32::from_bits(0x373b3a27), f32::from_bits(0x201fc4c1)),
        (f32::from_bits(0x27576c2a), f32::from_bits(0x5921c25d)),
    );
    path.quad_to(
        (f32::from_bits(0x2720213b), f32::from_bits(0x3a214729)),
        (f32::from_bits(0xdf28282a), f32::from_bits(0x3a8a3a21)),
    );
    path.cubic_to(
        (f32::from_bits(0x373b3ac5), f32::from_bits(0x201fc422)),
        (f32::from_bits(0x523a702a), f32::from_bits(0x27576c51)),
        (f32::from_bits(0x5921c25d), f32::from_bits(0x51523a70)),
    );
    path.quad_to(
        (f32::from_bits(0xd912102a), f32::from_bits(0x284f9a28)),
        (f32::from_bits(0xb38a1f30), f32::from_bits(0x3a3ac23a)),
    );
    path.line_to((f32::from_bits(0xc809272a), f32::from_bits(0x29b02829)));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9956-L9987 (chrome/m156)
fn fuzz763_4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x555b3a2d), f32::from_bits(0x2a212a8c)));
    path.conic_to(
        (f32::from_bits(0xc0032108), f32::from_bits(0x7a6a4b7b)),
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        f32::from_bits(0x6a3a7bc0),
    );
    path.conic_to(
        (f32::from_bits(0x3a2147ed), f32::from_bits(0xdf28282a)),
        (f32::from_bits(0x3a8a3a21), f32::from_bits(0x8a284f9a)),
        f32::from_bits(0x3ac2b33a),
    );
    path.cubic_to(
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x63962be6)),
        (f32::from_bits(0x295b2d2a), f32::from_bits(0x68295b2d)),
        (f32::from_bits(0x2d296855), f32::from_bits(0x2a8c275b)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.close();
    path.move_to((f32::from_bits(0x55685b1f), f32::from_bits(0x5b2d2968)));
    path.line_to((f32::from_bits(0x2a212a8c), f32::from_bits(0x2a21081f)));
    path.conic_to(
        (f32::from_bits(0xde6a4b7b), f32::from_bits(0x2a8ced7a)),
        (f32::from_bits(0x21081f21), f32::from_bits(0x3a7bc003)),
        f32::from_bits(0x47ed7a6a),
    );
    path.line_to((f32::from_bits(0x55685b1f), f32::from_bits(0x5b2d2968)));
    path.close();
    path.move_to((f32::from_bits(0x55685b1f), f32::from_bits(0x5b2d2968)));
    path.quad_to(
        (f32::from_bits(0xdf28282a), f32::from_bits(0x3a8a3a21)),
        (f32::from_bits(0x8a284f9a), f32::from_bits(0x3ac23ab3)),
    );
    path.line_to((f32::from_bits(0x2928088c), f32::from_bits(0x2be61d2a)));
    path.conic_to(
        (f32::from_bits(0x2a812a63), f32::from_bits(0x2d292a27)),
        (f32::from_bits(0x5568295b), f32::from_bits(0x5b2d2968)),
        f32::from_bits(0x552d6829),
    );
    path.conic_to(
        (f32::from_bits(0x395b2d5b), f32::from_bits(0x68552768)),
        (f32::from_bits(0x555b2df0), f32::from_bits(0x1f722a8c)),
        f32::from_bits(0x082a212a),
    );
    path.line_to((f32::from_bits(0x55685b1f), f32::from_bits(0x5b2d2968)));
    path.close();
    path.move_to((f32::from_bits(0x212a8c55), f32::from_bits(0x21081f2a)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2147ed7a)),
        (f32::from_bits(0x28282a3a), f32::from_bits(0x21df212a)),
        f32::from_bits(0x033a8a3a),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9989-L10022 (chrome/m156)
fn fuzz763_3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x555b292d), f32::from_bits(0x2a212a8c)));
    path.conic_to(
        (f32::from_bits(0xc0032108), f32::from_bits(0x7a6a4b7b)),
        (f32::from_bits(0x212a8ced), f32::from_bits(0x295b2d1f)),
        f32::from_bits(0x29685568),
    );
    path.conic_to(
        (f32::from_bits(0x8c28295b), f32::from_bits(0x1f21212a)),
        (f32::from_bits(0xc0032a08), f32::from_bits(0x7a6a4b7b)),
        f32::from_bits(0x212a8ced),
    );
    path.move_to((f32::from_bits(0x25682929), f32::from_bits(0x212a8c5b)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4a7bc0)));
    path.conic_to(
        (f32::from_bits(0x032108ed), f32::from_bits(0x283a7bc0)),
        (f32::from_bits(0x47ed7a6a), f32::from_bits(0x282a3a21)),
        f32::from_bits(0x3a21ff28),
    );
    path.quad_to(
        (f32::from_bits(0x8a284f9a), f32::from_bits(0x3ac23ab3)),
        (f32::from_bits(0x2a292827), f32::from_bits(0x962be61d)),
    );
    path.line_to((f32::from_bits(0x295b2d2a), f32::from_bits(0x2d296868)));
    path.move_to((f32::from_bits(0x212a8c55), f32::from_bits(0x21081f2a)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x898ced7a)),
        (f32::from_bits(0x21081f21), f32::from_bits(0x3a7bc003)),
        f32::from_bits(0x47ed7a6a),
    );
    path.line_to((f32::from_bits(0x212a8c55), f32::from_bits(0x21081f2a)));
    path.close();
    path.move_to((f32::from_bits(0x212a8c55), f32::from_bits(0x21081f2a)));
    path.quad_to(
        (f32::from_bits(0xdf28282a), f32::from_bits(0x3a8a3a21)),
        (f32::from_bits(0xb38a281a), f32::from_bits(0x29283ac2)),
    );
    path.move_to((f32::from_bits(0x962be61d), f32::from_bits(0x432a2927)));
    path.conic_to(
        (f32::from_bits(0x3a2a552a), f32::from_bits(0x3b1e2ab0)),
        (f32::from_bits(0x29272021), f32::from_bits(0x3b3ac527)),
        f32::from_bits(0x1fc42236),
    );
    path.cubic_to(
        (f32::from_bits(0x27576c2a), f32::from_bits(0x5921c25d)),
        (f32::from_bits(0x51503a70), f32::from_bits(0x12102a10)),
        (f32::from_bits(0x633a28d9), f32::from_bits(0x29c80927)),
    );
    path.line_to((f32::from_bits(0x272927b0), f32::from_bits(0x5b392929)));
    path.move_to((f32::from_bits(0x3a1127b4), f32::from_bits(0x2921ee3b)));
    path.cubic_to(
        (f32::from_bits(0x5e215d3b), f32::from_bits(0x7828ee3a)),
        (f32::from_bits(0x8e28b03b), f32::from_bits(0x50783be8)),
        (f32::from_bits(0x9e0b8a3a), f32::from_bits(0x555b2d68)),
    );
    path.move_to((f32::from_bits(0x21081f3f), f32::from_bits(0x9fd4e62a)));
    path.cubic_to(
        (f32::from_bits(0x3a293a2a), f32::from_bits(0x0e3bf0c5)),
        (f32::from_bits(0x3b29d42a), f32::from_bits(0x0f217265)),
        (f32::from_bits(0x2d5d2921), f32::from_bits(0x5568295b)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10024-L10047 (chrome/m156)
fn fuzz763_5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x5b292d55), f32::from_bits(0x2a2a8c55)));
    path.conic_to(
        (f32::from_bits(0xc0032108), f32::from_bits(0x7a6a4b79)),
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        f32::from_bits(0x6a3a7bc0),
    );
    path.conic_to(
        (f32::from_bits(0x3a2147ed), f32::from_bits(0xdf28282a)),
        (f32::from_bits(0x3a8a3a21), f32::from_bits(0x8a284f9a)),
        f32::from_bits(0x3ac23ab3),
    );
    path.cubic_to(
        (f32::from_bits(0xe62a2928), f32::from_bits(0x2a63962b)),
        (f32::from_bits(0x68295b2d), f32::from_bits(0x2d296855)),
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x001f2a21)),
    );
    path.line_to((f32::from_bits(0x5b292d55), f32::from_bits(0x2a2a8c55)));
    path.close();
    path.move_to((f32::from_bits(0x5b292d55), f32::from_bits(0x2a2a8c55)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2a8ced7a)),
        (f32::from_bits(0x21081f21), f32::from_bits(0x3a7bc003)),
        f32::from_bits(0x47ed7a6a),
    );
    path.line_to((f32::from_bits(0x5b292d55), f32::from_bits(0x2a2a8c55)));
    path.close();
    path.move_to((f32::from_bits(0x5b292d55), f32::from_bits(0x2a2a8c55)));
    path.quad_to(
        (f32::from_bits(0xdf28282a), f32::from_bits(0x3a8a3b21)),
        (f32::from_bits(0x28ee4f9a), f32::from_bits(0x68293b78)),
    );
    path.line_to((f32::from_bits(0x5b2d2968), f32::from_bits(0x5b2d8c55)));
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L10049-L10086 (chrome/m156)
fn fuzz763_2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x555b292d), f32::from_bits(0x2a212a8c)));
    path.conic_to(
        (f32::from_bits(0xc0032108), f32::from_bits(0x7a6a4b7b)),
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        f32::from_bits(0x6a3a7bc0),
    );
    path.line_to((f32::from_bits(0x081f2ad7), f32::from_bits(0x7bc00321)));
    path.move_to((f32::from_bits(0x2a3a2147), f32::from_bits(0xdf212828)));
    path.quad_to(
        (f32::from_bits(0x4f1a3a8a), f32::from_bits(0x3ab38a28)),
        (f32::from_bits(0x29283ac2), f32::from_bits(0x962be62a)),
    );
    path.cubic_to(
        (f32::from_bits(0x272a812a), f32::from_bits(0x3a2a5529)),
        (f32::from_bits(0x3b1e2ab0), f32::from_bits(0x29272021)),
        (f32::from_bits(0x3b3ac527), f32::from_bits(0x1fc42237)),
    );
    path.cubic_to(
        (f32::from_bits(0x27576c2a), f32::from_bits(0x5921c25d)),
        (f32::from_bits(0x51523a70), f32::from_bits(0x12102a10)),
        (f32::from_bits(0x633a28d9), f32::from_bits(0x29c80927)),
    );
    path.line_to((f32::from_bits(0x29292727), f32::from_bits(0x21475b3b)));
    path.quad_to(
        (f32::from_bits(0xdf28282a), f32::from_bits(0x3a8a3a21)),
        (f32::from_bits(0x8a284f9a), f32::from_bits(0x3ac23ab3)),
    );
    path.cubic_to(
        (f32::from_bits(0x682d2928), f32::from_bits(0x555b6829)),
        (f32::from_bits(0x555b292d), f32::from_bits(0x2a212a8c)),
        (f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x295b2ded), f32::from_bits(0x29685568)),
        (f32::from_bits(0x8c555b2d), f32::from_bits(0xe61d2a2a)),
        f32::from_bits(0x2a63962b),
    );
    path.conic_to(
        (f32::from_bits(0x5568295b), f32::from_bits(0x5b2d2968)),
        (f32::from_bits(0x212a8c55), f32::from_bits(0x21081f2a)),
        f32::from_bits(0x4b7bc003),
    );
    path.line_to((f32::from_bits(0x2a8ced7a), f32::from_bits(0x21081f21)));
    path.conic_to(
        (f32::from_bits(0x6a3a7bc0), f32::from_bits(0x2147ed7a)),
        (f32::from_bits(0x28282a3a), f32::from_bits(0x8a3a21df)),
        f32::from_bits(0x27b42a3a),
    );
    path.conic_to(
        (f32::from_bits(0x2921217d), f32::from_bits(0x5e3a3b35)),
        (f32::from_bits(0x7828ee3a), f32::from_bits(0x8e28b03b)),
        f32::from_bits(0x783be82a),
    );
    path.conic_to(
        (f32::from_bits(0x8e0b8a3a), f32::from_bits(0x279fd4e6)),
        (f32::from_bits(0x7a293a2a), f32::from_bits(0x2a0ef0c5)),
        f32::from_bits(0x653b29d4),
    );
    path.quad_to(
        (f32::from_bits(0x29210f21), f32::from_bits(0x282a085d)),
        (f32::from_bits(0xc2ab2127), f32::from_bits(0xa6800028)),
    );
    path.line_to((f32::from_bits(0x2a3a2147), f32::from_bits(0xdf212828)));
    path.close();
    path.move_to((f32::from_bits(0x2a3a2147), f32::from_bits(0xdf212828)));
    path.quad_to(
        (f32::from_bits(0x216a2770), f32::from_bits(0x2ab73b28)),
        (f32::from_bits(0x4b28f427), f32::from_bits(0x283b5b28)),
    );
    path.line_to((f32::from_bits(0x2a3a2147), f32::from_bits(0xdf212828)));
    path.close();
    path.move_to((f32::from_bits(0x2a3a2147), f32::from_bits(0xdf212828)));
    path.conic_to(
        (f32::from_bits(0xf86d273b), f32::from_bits(0x27e523e3)),
        (f32::from_bits(0x2927e0f5), f32::from_bits(0x2ac0e729)),
        f32::from_bits(0x6b492128),
    );
    path.cubic_to(
        (f32::from_bits(0x2f273927), f32::from_bits(0xa83a2c21)),
        (f32::from_bits(0xd7122121), f32::from_bits(0x21212921)),
        (f32::from_bits(0x3be3db3a), f32::from_bits(0xa9deb63b)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10089-L10104 (chrome/m156)
fn fuzz763_1c(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0x1931204a), f32::from_bits(0x2ba1a14a)),
        (f32::from_bits(0x4a4a08ff), f32::from_bits(0x4a4a08ff)),
        (f32::from_bits(0x4a4a4a34), f32::from_bits(0x4a4a4a4a)),
    );
    path.move_to((f32::from_bits(0x000010a1), f32::from_bits(0x19312000)));
    path.cubic_to(
        (f32::from_bits(0x4a4a4a4a), f32::from_bits(0x4a4a4a4a)),
        (f32::from_bits(0xa14a4a4a), f32::from_bits(0x08ff2ba1)),
        (f32::from_bits(0x08ff4a4a), f32::from_bits(0x4a344a4a)),
    );
    path.cubic_to(
        (f32::from_bits(0x4a4a4a4a), f32::from_bits(0x4a4a4a4a)),
        (f32::from_bits(0x2ba1a14a), f32::from_bits(0x4e4a08ff)),
        (f32::from_bits(0x4a4a4a4a), f32::from_bits(0xa1a181ff)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10107-L10128 (chrome/m156)
fn fuzz763_1b(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0x0000ff07), f32::from_bits(0xf9f9ff00)),
        (f32::from_bits(0xfe0ef9f4), f32::from_bits(0xd9b105fb)),
        (f32::from_bits(0x000000f9), f32::from_bits(0xfe11f901)),
    );
    path.line_to((f32::from_bits(0xda1905ed), f32::from_bits(0x3c05fbfb)));
    path.cubic_to(
        (f32::from_bits(0x3c3c3c3c), f32::from_bits(0x3c3c3c3c)),
        (f32::from_bits(0x253c7f00), f32::from_bits(0xfa00d3fa)),
        (f32::from_bits(0x250025fe), f32::from_bits(0x00000006)),
    );
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x3c3c3c3c), f32::from_bits(0xfa253c3c)),
        (f32::from_bits(0xfefa00d3), f32::from_bits(0x25fad9df)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x8dfefa00), f32::from_bits(0xf0f9fad9)));
    path.cubic_to(
        (f32::from_bits(0x20fe58f9), f32::from_bits(0x0525fbed)),
        (f32::from_bits(0x1905ffff), f32::from_bits(0x01f9f9f9)),
        (f32::from_bits(0xfbfe0ef9), f32::from_bits(0xfb212fff)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10130-L10155 (chrome/m156)
fn fuzz763_1a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0x154be880), f32::from_bits(0x80000640)),
        (f32::from_bits(0x5559a419), f32::from_bits(0x59d55928)),
        (f32::from_bits(0x80045959), f32::from_bits(0x40154be8)),
    );
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x5559a419), f32::from_bits(0x59d55928)),
        (f32::from_bits(0xbd595959), f32::from_bits(0x3f3f3f09)),
    );
    path.move_to((f32::from_bits(0x3f3f3f3f), f32::from_bits(0x3f3f3f3f)));
    path.move_to((f32::from_bits(0x3f3f3f3f), f32::from_bits(0xff3f3f3f)));
    path.line_to((f32::from_bits(0x09090909), f32::from_bits(0x3038d509)));
    path.conic_to(
        (f32::from_bits(0x5947ffff), f32::from_bits(0x40e88004)),
        (f32::from_bits(0x00002059), f32::from_bits(0x28555900)),
        f32::from_bits(0x5959d559),
    );
    path.line_to((f32::from_bits(0x3f3f3f3f), f32::from_bits(0xff3f3f3f)));
    path.close();
    path.move_to((f32::from_bits(0x3f3f3f3f), f32::from_bits(0xff3f3f3f)));
    path.line_to((f32::from_bits(0x38d57f4b), f32::from_bits(0x59597f4b)));
    path.line_to((f32::from_bits(0x3f3f3f3f), f32::from_bits(0xff3f3f3f)));
    path.close();
    path.move_to((f32::from_bits(0x384700ff), f32::from_bits(0x0108804b)));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10158-L10176 (chrome/m156)
fn fuzz763_3a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x555b292d), f32::from_bits(0x2a212a8c)));
    path.conic_to(
        (f32::from_bits(0xc0032108), f32::from_bits(0x7a6a4b7b)),
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        f32::from_bits(0x6a3a7bc0),
    );
    path.conic_to(
        (f32::from_bits(0x3a2147ed), f32::from_bits(0xdf28282a)),
        (f32::from_bits(0x3a8a3a21), f32::from_bits(0x8a284f9a)),
        f32::from_bits(0x3ac23ab3),
    );
    path.cubic_to(
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x63962be6)),
        (f32::from_bits(0x272a812a), f32::from_bits(0x295b2d29)),
        (f32::from_bits(0x2a685568), f32::from_bits(0x68295b2d)),
    );
    path.conic_to(
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x081f2a21)),
        (f32::from_bits(0x7bc00321), f32::from_bits(0x7a6a4b77)),
        f32::from_bits(0x3a214726),
    );
    path.move_to((f32::from_bits(0x8adf2028), f32::from_bits(0x3a219a3a)));
    path.quad_to(
        (f32::from_bits(0x3ab38e28), f32::from_bits(0x29283ac2)),
        (f32::from_bits(0x2be61d2a), f32::from_bits(0x812a4396)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10179-L10200 (chrome/m156)
fn fuzz763_5a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x38bd8610), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x4183d871), f32::from_bits(0x41fea321)),
        (f32::from_bits(0xb700ff00), f32::from_bits(0x4240b8b8)),
        f32::from_bits(0x3b058283),
    );
    path.line_to((f32::from_bits(0x3a3a3ab8), f32::from_bits(0xb8b8b8b8)));
    path.conic_to(
        (f32::from_bits(0x3a455ec8), f32::from_bits(0xb8b8b8b3)),
        (f32::from_bits(0x38b2418d), f32::from_bits(0xb730d014)),
        f32::from_bits(0x3f7ffff3),
    );
    path.quad_to(
        (f32::from_bits(0x3a51246a), f32::from_bits(0xb6da45a3)),
        (f32::from_bits(0x38bc5c3c), f32::from_bits(0x00000000)),
    );
    path.line_to((f32::from_bits(0x3a3a3ab8), f32::from_bits(0xb8b8b8b8)));
    path.quad_to(
        (f32::from_bits(0x39a32d2d), f32::from_bits(0x00000000)),
        (f32::from_bits(0xb8a13a00), f32::from_bits(0x00000000)),
    );
    path.line_to((f32::from_bits(0x3a3a3ab8), f32::from_bits(0xb8b8b8b8)));
    path.quad_to(
        (f32::from_bits(0x39ba814c), f32::from_bits(0xb838fed2)),
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
    );
    path.line_to((f32::from_bits(0x38bd8610), f32::from_bits(0x00000000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L10203-L10220 (chrome/m156)
fn fuzz763_2a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x3e484500), f32::from_bits(0x164f3a30)),
        (f32::from_bits(0x49484801), f32::from_bits(0x7d0100c8)),
    );
    path.conic_to(
        (f32::from_bits(0xff7f36fd), f32::from_bits(0x3e647d01)),
        (f32::from_bits(0x0c00f430), f32::from_bits(0x486b6448)),
        f32::from_bits(0x00484848),
    );
    path.line_to((f32::from_bits(0x4f4f557d), f32::from_bits(0x48480112)));
    path.line_to((f32::from_bits(0xf40c01ff), f32::from_bits(0x45008000)));
    path.move_to((f32::from_bits(0x4bfffa00), f32::from_bits(0x7d4ac859)));
    path.conic_to(
        (f32::from_bits(0x7d014f3e), f32::from_bits(0x00f4ff01)),
        (f32::from_bits(0x6b64480c), f32::from_bits(0x48484848)),
        f32::from_bits(0x557d0100),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10223-L10268 (chrome/m156)
fn fuzz763_2b(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x5b292d55), f32::from_bits(0x212a8c55)));
    path.move_to((f32::from_bits(0x3b21081f), f32::from_bits(0x4b7bc003)));
    path.line_to((f32::from_bits(0x2a8ced7a), f32::from_bits(0x21081f21)));
    path.conic_to(
        (f32::from_bits(0x6a3a7bc0), f32::from_bits(0x4721ed7a)),
        (f32::from_bits(0x282a3a21), f32::from_bits(0x3a21df28)),
        f32::from_bits(0x4f9a3a8a),
    );
    path.line_to((f32::from_bits(0x3b21081f), f32::from_bits(0x4b7bc003)));
    path.close();
    path.move_to((f32::from_bits(0x3b21081f), f32::from_bits(0x4b7bc003)));
    path.cubic_to(
        (f32::from_bits(0x273ac23a), f32::from_bits(0x1d2a2928)),
        (f32::from_bits(0x63962be6), f32::from_bits(0x272a812a)),
        (f32::from_bits(0x295b2d29), f32::from_bits(0x29685568)),
    );
    path.line_to((f32::from_bits(0x081f2a21), f32::from_bits(0x7bc00321)));
    path.line_to((f32::from_bits(0x282a3a21), f32::from_bits(0x3a21df28)));
    path.line_to((f32::from_bits(0x3b21081f), f32::from_bits(0x4b7bc003)));
    path.close();
    path.move_to((f32::from_bits(0x3b21081f), f32::from_bits(0x4b7bc003)));
    path.quad_to(
        (f32::from_bits(0x8a4fc29a), f32::from_bits(0x3ab3283a)),
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x43962be6)),
    );
    path.move_to((f32::from_bits(0x5b2d2a81), f32::from_bits(0x29276829)));
    path.conic_to(
        (f32::from_bits(0x1e2ab03a), f32::from_bits(0x2920213b)),
        (f32::from_bits(0x3b3ac527), f32::from_bits(0xc422333b)),
        f32::from_bits(0x6c2a9f1f),
    );
    path.quad_to(
        (f32::from_bits(0xc25d2757), f32::from_bits(0x3a705921)),
        (f32::from_bits(0x2a105152), f32::from_bits(0x28d91210)),
    );
    path.quad_to(
        (f32::from_bits(0x68295b2d), f32::from_bits(0x2d296855)),
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x081f2a21)),
    );
    path.line_to((f32::from_bits(0x5b2d2a81), f32::from_bits(0x29276829)));
    path.close();
    path.move_to((f32::from_bits(0x5b2d2a81), f32::from_bits(0x29276829)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2a8ced7a)),
        (f32::from_bits(0x21081f21), f32::from_bits(0xcb7bc003)),
        f32::from_bits(0x47ed7a6a),
    );
    path.line_to((f32::from_bits(0x5b2d2a81), f32::from_bits(0x29276829)));
    path.close();
    path.move_to((f32::from_bits(0x5b2d2a81), f32::from_bits(0x29276829)));
    path.quad_to(
        (f32::from_bits(0xdf28282a), f32::from_bits(0x2d8a3a21)),
        (f32::from_bits(0x5b682b68), f32::from_bits(0x5b292d55)),
    );
    path.line_to((f32::from_bits(0x2a212a8c), f32::from_bits(0x0321081f)));
    path.conic_to(
        (f32::from_bits(0x7a6a4b7b), f32::from_bits(0x212a8ced)),
        (f32::from_bits(0x0321081f), f32::from_bits(0x6a3a7bc0)),
        f32::from_bits(0x3a21477a),
    );
    path.move_to((f32::from_bits(0x21df2828), f32::from_bits(0x9a3a8a3a)));
    path.quad_to(
        (f32::from_bits(0x3ab38a28), f32::from_bits(0x28273ac2)),
        (f32::from_bits(0xe61d2a29), f32::from_bits(0x2a63962b)),
    );
    path.conic_to(
        (f32::from_bits(0x2d29272a), f32::from_bits(0x5568295b)),
        (f32::from_bits(0x5b2d2968), f32::from_bits(0x5b2d6829)),
        f32::from_bits(0x212a8c55),
    );
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.conic_to(
        (f32::from_bits(0x3a2147ed), f32::from_bits(0xdf28282a)),
        (f32::from_bits(0x3a8a3a21), f32::from_bits(0x8a284f9a)),
        f32::from_bits(0x3ac23ab3),
    );
    path.line_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L10270-L10293 (chrome/m156)
fn fuzz763_2c(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x36344a4a)));
    path.cubic_to(
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
        (f32::from_bits(0x364a4a4a), f32::from_bits(0x364a4a4a)),
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
    );
    path.line_to((f32::from_bits(0x364a4a4a), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0x364a30f0), f32::from_bits(0x344ac7fb)),
        (f32::from_bits(0x3656d432), f32::from_bits(0x34cabb48)),
        (f32::from_bits(0x367031a9), f32::from_bits(0x351802f1)),
    );
    path.cubic_to(
        (f32::from_bits(0x36a7b150), f32::from_bits(0x35ab09db)),
        (f32::from_bits(0x371874ed), f32::from_bits(0x3604f2c7)),
        (f32::from_bits(0x3784e0c7), f32::from_bits(0x36344a51)),
    );
    path.cubic_to(
        (f32::from_bits(0x3743dc9a), f32::from_bits(0x36344a4f)),
        (f32::from_bits(0x36fbef33), f32::from_bits(0x36344a4e)),
        (f32::from_bits(0x36604a35), f32::from_bits(0x36344a4c)),
    );
    path.cubic_to(
        (f32::from_bits(0x36531715), f32::from_bits(0x36344a4c)),
        (f32::from_bits(0x3645e3f5), f32::from_bits(0x36344a4b)),
        (f32::from_bits(0x3638b0d4), f32::from_bits(0x36344a4b)),
    );
    path.cubic_to(
        (f32::from_bits(0x35f64120), f32::from_bits(0x36344a4b)),
        (f32::from_bits(0x35764124), f32::from_bits(0x36344a4a)),
        (f32::from_bits(0x00000000), f32::from_bits(0x36344a4a)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0x1931204a), f32::from_bits(0x2ba1a14a)),
        (f32::from_bits(0x4a4a08ff), f32::from_bits(0x4a4a08ff)),
        (f32::from_bits(0x4a4a4a34), f32::from_bits(0x4a4a4a4a)),
    );
    path.move_to((f32::from_bits(0x000010a1), f32::from_bits(0x19312000)));
    path.cubic_to(
        (f32::from_bits(0x4a4a4a4a), f32::from_bits(0x4a4a4a4a)),
        (f32::from_bits(0xa14a4a4a), f32::from_bits(0x08ff2ba1)),
        (f32::from_bits(0x08ff4a4a), f32::from_bits(0x4a344a4a)),
    );
    path.cubic_to(
        (f32::from_bits(0x544a4a4a), f32::from_bits(0x4a4a4a4a)),
        (f32::from_bits(0x2ba1a14a), f32::from_bits(0x4e4a08ff)),
        (f32::from_bits(0x4a4a4a4a), f32::from_bits(0xa1a181ff)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L10295-L10319 (chrome/m156)
fn fuzz763_6(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x6a2a291f)));
    path.cubic_to(
        (f32::from_bits(0x68295b2d), f32::from_bits(0x00000000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x68556829)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x68555b2a)));
    path.cubic_to(
        (f32::from_bits(0x00000000), f32::from_bits(0x67d55b2a)),
        (f32::from_bits(0x67296a4b), f32::from_bits(0x67555b2a)),
        (f32::from_bits(0x677e1f70), f32::from_bits(0x66d55b2a)),
    );
    path.cubic_to(
        (f32::from_bits(0x678f0684), f32::from_bits(0x6684f008)),
        (f32::from_bits(0x6798f8ea), f32::from_bits(0x6625a942)),
        (f32::from_bits(0x67961914), f32::from_bits(0x65ce709a)),
    );
    path.cubic_to(
        (f32::from_bits(0x679174f7), f32::from_bits(0x63199132)),
        (f32::from_bits(0x6756c79f), f32::from_bits(0x606478de)),
        (f32::from_bits(0x65682bcf), f32::from_bits(0x00000000)),
    );
    path.conic_to(
        (f32::from_bits(0x68295b02), f32::from_bits(0x60f7f28b)),
        (f32::from_bits(0x00000000), f32::from_bits(0x6a2a291f)),
        f32::from_bits(0x42784f5a),
    );
    path.close();
    path.move_to((f32::from_bits(0x654d6d10), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x6a4b7bc0), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x6a4b7bc0)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x3ac23a55), f32::from_bits(0x2a292827)));
    path.line_to((f32::from_bits(0x63962be6), f32::from_bits(0x272a812a)));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10321-L10373 (chrome/m156)
fn fuzz763_7(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x0f2a312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68392d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a1f2a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x68345b2d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0xef2a8c55)),
        (f32::from_bits(0x295b2d2a), f32::from_bits(0x08685568)),
        f32::from_bits(0x7bc00321),
    );
    path.line_to((f32::from_bits(0x68345b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x68345b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x212a8c55)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.line_to((f32::from_bits(0x3a8a3adf), f32::from_bits(0x8a281a4f)));
    path.quad_to(
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x43962be6)),
        (f32::from_bits(0x272a812a), f32::from_bits(0x3a2a5529)),
    );
    path.line_to((f32::from_bits(0x213b1e2a), f32::from_bits(0x27292720)));
    path.conic_to(
        (f32::from_bits(0xba1f203a), f32::from_bits(0xc422c538)),
        (f32::from_bits(0x215d5927), f32::from_bits(0x70ec2ac2)),
        f32::from_bits(0x2a51523a),
    );
    path.quad_to(
        (f32::from_bits(0x633ad912), f32::from_bits(0x29c80927)),
        (f32::from_bits(0x272927b0), f32::from_bits(0x683a5b2d)),
    );
    path.line_to((f32::from_bits(0x295b2d68), f32::from_bits(0x29685568)));
    path.conic_to(
        (f32::from_bits(0xaa8c555b), f32::from_bits(0x081f2a21)),
        (f32::from_bits(0x5b2d0321), f32::from_bits(0x68556829)),
        f32::from_bits(0x2a552d29),
    );
    path.cubic_to(
        (f32::from_bits(0x21295b2d), f32::from_bits(0x2a688c5b)),
        (f32::from_bits(0x68295b2d), f32::from_bits(0x2d296855)),
        (f32::from_bits(0x8c08555b), f32::from_bits(0x2a2a29ca)),
    );
    path.quad_to(
        (f32::from_bits(0x68295b21), f32::from_bits(0x2d296855)),
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x081f2a21)),
    );
    path.line_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.close();
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x5b2d6829)),
        (f32::from_bits(0x212a8c55), f32::from_bits(0xed7aba1f)),
        f32::from_bits(0x2a212a8c),
    );
    path.move_to((f32::from_bits(0x2d212d08), f32::from_bits(0x5568295b)));
    path.move_to((f32::from_bits(0x5529685b), f32::from_bits(0x11295b68)));
    path.conic_to(
        (f32::from_bits(0x5b782968), f32::from_bits(0x3a292d55)),
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x68295a2d)),
        f32::from_bits(0x2d296855),
    );
    path.move_to((f32::from_bits(0x555b8c55), f32::from_bits(0x21682929)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.conic_to(
        (f32::from_bits(0xac2d8ced), f32::from_bits(0x5b682968)),
        (f32::from_bits(0x5b292d55), f32::from_bits(0x212a8c55)),
        f32::from_bits(0x081f282a),
    );
    path.line_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.close();
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2a8ced7a)),
        (f32::from_bits(0x03081f21), f32::from_bits(0x6a3a7bc0)),
        f32::from_bits(0x2147ed7a),
    );
    path.line_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.close();
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.quad_to(
        (f32::from_bits(0x2d28282a), f32::from_bits(0x5568295b)),
        (f32::from_bits(0x3a21df68), f32::from_bits(0x4f9a3a8a)),
    );
    path.line_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.close();
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.cubic_to(
        (f32::from_bits(0x5568c23a), f32::from_bits(0x5b2d2968)),
        (f32::from_bits(0x212a8c55), f32::from_bits(0x21081f2a)),
        (f32::from_bits(0x3a7bc003), f32::from_bits(0x294b2827)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10375-L10406 (chrome/m156)
fn kfuzz2(reporter: &mut Reporter, filename: &str) {
    let path1 = Path::new();
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xfafadbfa)));
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xfafadbfa)));
    path.cubic_to(
        (f32::from_bits(0xe3000000), f32::from_bits(0xf19e92c7)),
        (f32::from_bits(0xf17febcb), f32::from_bits(0xff7febcb)),
        (f32::from_bits(0x60600100), f32::from_bits(0x0100ff60)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xfafadbfa)));
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xfafadbfa)));
    path.line_to((f32::from_bits(0x60601a1d), f32::from_bits(0x60606060)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xfafadbfa)));
    path.close();
    path.move_to((f32::from_bits(0xe5e2f300), f32::from_bits(0xee244a40)));
    path.move_to((f32::from_bits(0xba98ffee), f32::from_bits(0xfafafa1a)));
    path.close();
    path.move_to((f32::from_bits(0xba98ffee), f32::from_bits(0xfafafa1a)));
    path.line_to((f32::from_bits(0xfafafafa), f32::from_bits(0xe30000fa)));
    path.conic_to(
        (f32::from_bits(0x92e592e5), f32::from_bits(0xfafafafb)),
        (f32::from_bits(0xc4fa0000), f32::from_bits(0x6060fafa)),
        f32::from_bits(0x60606060),
    );
    path.line_to((f32::from_bits(0xba98ffee), f32::from_bits(0xfafafa1a)));
    path.close();
    path.move_to((f32::from_bits(0xba98ffee), f32::from_bits(0xfafafa1a)));
    path.cubic_to(
        (f32::from_bits(0xe3000000), f32::from_bits(0xf19e92c7)),
        (f32::from_bits(0xf17febcb), f32::from_bits(0xff7febcb)),
        (f32::from_bits(0xfafafa00), f32::from_bits(0xfafafafa)),
    );
    path.line_to((f32::from_bits(0xba98ffee), f32::from_bits(0xfafafa1a)));
    path.close();
    path.move_to((f32::from_bits(0xba98ffee), f32::from_bits(0xfafafa1a)));
    path.cubic_to(
        (f32::from_bits(0xe3000000), f32::from_bits(0xe39e92c7)),
        (f32::from_bits(0xf17febcb), f32::from_bits(0xff7febcb)),
        (f32::from_bits(0xeed0ee9a), f32::from_bits(0x9a98ffca)),
    );
    path.line_to((f32::from_bits(0xba98ffee), f32::from_bits(0xfafafa1a)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10408-L10449 (chrome/m156)
fn fuzz763_10(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x68556829)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x00000000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x6a4b7bc4)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x68556829)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x5b2d2968), f32::from_bits(0x2a8c8f55)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.close();
    path.move_to((f32::from_bits(0xc021211f), f32::from_bits(0x6a4b7b03)));
    path.conic_to(
        (f32::from_bits(0x682d2fed), f32::from_bits(0x755b6829)),
        (f32::from_bits(0x5b292d2b), f32::from_bits(0xc92a8c55)),
        f32::from_bits(0x081f2a21),
    );
    path.line_to((f32::from_bits(0xc021211f), f32::from_bits(0x6a4b7b03)));
    path.close();
    path.move_to((f32::from_bits(0xc021211f), f32::from_bits(0x6a4b7b03)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2a8ced7a)),
        (f32::from_bits(0x21081f21), f32::from_bits(0x3a7bc003)),
        f32::from_bits(0x47ed7a29),
    );
    path.line_to((f32::from_bits(0xc021211f), f32::from_bits(0x6a4b7b03)));
    path.close();
    path.move_to((f32::from_bits(0xc021211f), f32::from_bits(0x6a4b7b03)));
    path.quad_to(
        (f32::from_bits(0x6829682d), f32::from_bits(0x292d555b)),
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x081f2a29)),
    );
    path.conic_to(
        (f32::from_bits(0x6a497b19), f32::from_bits(0x218ced7a)),
        (f32::from_bits(0x0321081f), f32::from_bits(0x6a3a7bc0)),
        f32::from_bits(0x47ed3a7a),
    );
    path.line_to((f32::from_bits(0xc021211f), f32::from_bits(0x6a4b7b03)));
    path.close();
    path.move_to((f32::from_bits(0xc021211f), f32::from_bits(0x6a4b7b03)));
    path.quad_to(
        (f32::from_bits(0x282a282a), f32::from_bits(0x8a3a21df)),
        (f32::from_bits(0x2728282a), f32::from_bits(0x8a3a2129)),
    );
    path.quad_to(
        (f32::from_bits(0x8a284f9a), f32::from_bits(0x3a3ac2b3)),
        (f32::from_bits(0x2a292827), f32::from_bits(0x962be61d)),
    );
    path.line_to((f32::from_bits(0x272a802a), f32::from_bits(0x2a8c2d29)));
    path.line_to((f32::from_bits(0xc021211f), f32::from_bits(0x6a4b7b03)));
    path.close();
    path.move_to((f32::from_bits(0x4f9a3a29), f32::from_bits(0x3ab38a28)));
    path.quad_to(
        (f32::from_bits(0xc368305b), f32::from_bits(0x5b296855)),
        (f32::from_bits(0x2d8c5568), f32::from_bits(0x1f2a2172)),
    );
    path.line_to((f32::from_bits(0x29c00321), f32::from_bits(0x5b4b7b13)));
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L10451-L10500 (chrome/m156)
fn fuzz763_11(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x2a0f312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68392d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a1f2a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7b21), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x68385b2d), f32::from_bits(0x70682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0xef2a8c55)),
        (f32::from_bits(0x295b2d2a), f32::from_bits(0x08685568)),
        f32::from_bits(0x7bc00321),
    );
    path.line_to((f32::from_bits(0x68385b2d), f32::from_bits(0x70682955)));
    path.close();
    path.move_to((f32::from_bits(0x68385b2d), f32::from_bits(0x70682955)));
    path.line_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x212a8c55)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.line_to((f32::from_bits(0x3a8a3adf), f32::from_bits(0x8a281a4f)));
    path.quad_to(
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x43962be6)),
        (f32::from_bits(0x2a812a3b), f32::from_bits(0x2a552927)),
    );
    path.quad_to(
        (f32::from_bits(0x3b1e2ab0), f32::from_bits(0x29272021)),
        (f32::from_bits(0x203a3b27), f32::from_bits(0x22c5381f)),
    );
    path.move_to((f32::from_bits(0x5d27ec2a), f32::from_bits(0x705921c2)));
    path.quad_to(
        (f32::from_bits(0x102a5152), f32::from_bits(0x5b2dd912)),
        (f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)),
    );
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032127)));
    path.cubic_to(
        (f32::from_bits(0x68392d55), f32::from_bits(0x2a8c684b)),
        (f32::from_bits(0xf05b272d), f32::from_bits(0x2a1f1555)),
        (f32::from_bits(0x21082a21), f32::from_bits(0x6a4b7b03)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x2a395b2d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0xef2a8c55)),
        (f32::from_bits(0x295b2d2a), f32::from_bits(0x68210368)),
        f32::from_bits(0x7bc05508),
    );
    path.line_to((f32::from_bits(0x2a395b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x2a395b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x2a21211f)));
    path.line_to((f32::from_bits(0x03552a8c), f32::from_bits(0x6a4f7b28)));
    path.conic_to(
        (f32::from_bits(0x2347ed93), f32::from_bits(0x282a3a21)),
        (f32::from_bits(0x3adf2128), f32::from_bits(0x4f1a3a8a)),
        f32::from_bits(0x3ab38a28),
    );
    path.line_to((f32::from_bits(0x2a395b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x2a395b2d), f32::from_bits(0xf0682955)));
    path.quad_to(
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x43962be6)),
        (f32::from_bits(0x262a812a), f32::from_bits(0x3a2a5529)),
    );
    path.line_to((f32::from_bits(0x213b1e2a), f32::from_bits(0x27292720)));
    path.conic_to(
        (f32::from_bits(0x371f203a), f32::from_bits(0xc52a22c4)),
        (f32::from_bits(0xc25d27ec), f32::from_bits(0x3a705921)),
        f32::from_bits(0x5210513a),
    );
    path.cubic_to(
        (f32::from_bits(0x63102ad9), f32::from_bits(0x29c80927)),
        (f32::from_bits(0x633a27b0), f32::from_bits(0x2909c827)),
        (f32::from_bits(0x272927b1), f32::from_bits(0x3a685b2d)),
    );
    path.move_to((f32::from_bits(0x682d6829), f32::from_bits(0x29685555)));
    path.conic_to(
        (f32::from_bits(0xaa8c555b), f32::from_bits(0x081f2a21)),
        (f32::from_bits(0x5b2d0321), f32::from_bits(0x68556829)),
        f32::from_bits(0x5b2d2729),
    );
    path.quad_to(
        (f32::from_bits(0x2d685568), f32::from_bits(0x5568295b)),
        (f32::from_bits(0x2a552d29), f32::from_bits(0x295b2d27)),
    );
    path.line_to((f32::from_bits(0x682d6829), f32::from_bits(0x29685555)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10502-L10538 (chrome/m156)
fn fuzz763_12(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x6a29082a)));
    path.conic_to(
        (f32::from_bits(0x6a295ac3), f32::from_bits(0x61bb988e)),
        (f32::from_bits(0x6829682d), f32::from_bits(0x5f3ba76a)),
        f32::from_bits(0x42730a87),
    );
    path.conic_to(
        (f32::from_bits(0x67aedf99), f32::from_bits(0x00000000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
        f32::from_bits(0x3f801112),
    );
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x00000000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x68556829)),
        f32::from_bits(0x555b2d29),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x68555b2a)));
    path.cubic_to(
        (f32::from_bits(0x00000000), f32::from_bits(0x67d55b2a)),
        (f32::from_bits(0x67296a4b), f32::from_bits(0x67555b2a)),
        (f32::from_bits(0x677e1f70), f32::from_bits(0x66d55b2a)),
    );
    path.cubic_to(
        (f32::from_bits(0x678f0684), f32::from_bits(0x6684f008)),
        (f32::from_bits(0x6798f8ea), f32::from_bits(0x6625a942)),
        (f32::from_bits(0x67961914), f32::from_bits(0x65ce709a)),
    );
    path.cubic_to(
        (f32::from_bits(0x679158b0), f32::from_bits(0x00000000)),
        (f32::from_bits(0x67531e34), f32::from_bits(0x00000000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x21081f21), f32::from_bits(0x4b7bc003)),
        (f32::from_bits(0xed237a6a), f32::from_bits(0x2d682967)),
        f32::from_bits(0x2a8c555b),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x3a6821df), f32::from_bits(0x2a8c3a8a)));
    path.move_to((f32::from_bits(0x29272a1d), f32::from_bits(0xb03a2a55)));
    path.move_to((f32::from_bits(0x20213b1e), f32::from_bits(0xc5272927)));
    path.quad_to(
        (f32::from_bits(0xc422373b), f32::from_bits(0xec2a201f)),
        (f32::from_bits(0x21c25d27), f32::from_bits(0x523a7059)),
    );
    path.cubic_to(
        (f32::from_bits(0x12102a10), f32::from_bits(0xe73a28d9)),
        (f32::from_bits(0xc8092763), f32::from_bits(0x2927b029)),
        (f32::from_bits(0x295b2d27), f32::from_bits(0x2d685568)),
    );
    path.move_to((f32::from_bits(0x68556809), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a212a), f32::from_bits(0x2d032108)));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x2a552d29)));
    path.cubic_to(
        (f32::from_bits(0x21295b2d), f32::from_bits(0x2a528c5b)),
        (f32::from_bits(0x284f5b2d), f32::from_bits(0x218aa621)),
        (f32::from_bits(0x3f2d2db3), f32::from_bits(0x68293a2a)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10540-L10574 (chrome/m156)
fn fuzz763_13(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x212a8c55), f32::from_bits(0x21081f2a)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x4793ed7a)),
        (f32::from_bits(0x282a3a21), f32::from_bits(0x3adf2128)),
        f32::from_bits(0x4f1a3a8a),
    );
    path.line_to((f32::from_bits(0x212a8c55), f32::from_bits(0x21081f2a)));
    path.close();
    path.move_to((f32::from_bits(0x212a8c55), f32::from_bits(0x21081f2a)));
    path.cubic_to(
        (f32::from_bits(0x3ac2213a), f32::from_bits(0x1d2a2928)),
        (f32::from_bits(0x43962be6), f32::from_bits(0x272a8128)),
        (f32::from_bits(0x3a2a5529), f32::from_bits(0x3b1e2ab0)),
    );
    path.line_to((f32::from_bits(0x212a8c55), f32::from_bits(0x21081f2a)));
    path.close();
    path.move_to((f32::from_bits(0x212a8c55), f32::from_bits(0x21081f2a)));
    path.cubic_to(
        (f32::from_bits(0x3b272927), f32::from_bits(0x381f203a)),
        (f32::from_bits(0x2ac422c5), f32::from_bits(0xc25d27ec)),
        (f32::from_bits(0x3a705921), f32::from_bits(0x2a105152)),
    );
    path.quad_to(
        (f32::from_bits(0x633ad912), f32::from_bits(0x29c80927)),
        (f32::from_bits(0x272927b0), f32::from_bits(0x68295b2d)),
    );
    path.line_to((f32::from_bits(0x295b2d68), f32::from_bits(0x29685568)));
    path.conic_to(
        (f32::from_bits(0xaa8c555b), f32::from_bits(0x081f2a21)),
        (f32::from_bits(0x5b2d0321), f32::from_bits(0x68556829)),
        f32::from_bits(0x2a552d29),
    );
    path.cubic_to(
        (f32::from_bits(0x21295b2d), f32::from_bits(0x2a688c5b)),
        (f32::from_bits(0x6829292d), f32::from_bits(0x2d296855)),
        (f32::from_bits(0x8c08555b), f32::from_bits(0x2a2a291f)),
    );
    path.conic_to(
        (f32::from_bits(0x68295b21), f32::from_bits(0x2d296855)),
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x081f2a21)),
        f32::from_bits(0x7bc00321),
    );
    path.line_to((f32::from_bits(0x212a8c55), f32::from_bits(0x21081f2a)));
    path.close();
    path.move_to((f32::from_bits(0x212a8c55), f32::from_bits(0x21081f2a)));
    path.line_to((f32::from_bits(0x5b2d6829), f32::from_bits(0x212a8c55)));
    path.conic_to(
        (f32::from_bits(0x8ced7aba), f32::from_bits(0x3f2a212a)),
        (f32::from_bits(0x2d212d08), f32::from_bits(0x5568295b)),
        f32::from_bits(0x29685b2d),
    );
    path.line_to((f32::from_bits(0x68295b68), f32::from_bits(0x2d296855)));
    path.move_to((f32::from_bits(0x212a8c55), f32::from_bits(0x21081f2a)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2a8ced7a)),
        (f32::from_bits(0x21081f21), f32::from_bits(0x6aba7b03)),
        f32::from_bits(0x2147ed7a),
    );
    path.quad_to(
        (f32::from_bits(0x6028282a), f32::from_bits(0x68292ddf)),
        (f32::from_bits(0x5b2d555b), f32::from_bits(0x68556829)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L10576-L10589 (chrome/m156)
fn fuzz763_14(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0xf45b2d29)));
    path.move_to((f32::from_bits(0x1f2a302a), f32::from_bits(0xc8032108)));
    path.cubic_to(
        (f32::from_bits(0x68392d55), f32::from_bits(0xf0db684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x302a5b25), f32::from_bits(0xf0685568)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10591-L10644 (chrome/m156)
fn fuzz763_15(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x5b292d55), f32::from_bits(0x212a8c55)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a6b7bc4)));
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x2c6829c0), f32::from_bits(0x2a8c555b)),
        f32::from_bits(0x081f2a29),
    );
    path.line_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a6b7bc4)));
    path.close();
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a6b7bc4)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x4793ed7a)),
        (f32::from_bits(0x282a3a21), f32::from_bits(0xdf218a28)),
        f32::from_bits(0x4f1a3a3a),
    );
    path.quad_to(
        (f32::from_bits(0x3ab38a28), f32::from_bits(0x283ac221)),
        (f32::from_bits(0xe6432a29), f32::from_bits(0x2a96812b)),
    );
    path.line_to((f32::from_bits(0x5529272a), f32::from_bits(0x1eb03a2a)));
    path.conic_to(
        (f32::from_bits(0x2a272021), f32::from_bits(0x3ac52729)),
        (f32::from_bits(0xc422313b), f32::from_bits(0xec2a201f)),
        f32::from_bits(0x21c25d27),
    );
    path.line_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a6b7bc4)));
    path.close();
    path.move_to((f32::from_bits(0x1051523a), f32::from_bits(0xd912102a)));
    path.close();
    path.move_to((f32::from_bits(0x1051523a), f32::from_bits(0xd912102a)));
    path.quad_to(
        (f32::from_bits(0xc82763e7), f32::from_bits(0x2927b029)),
        (f32::from_bits(0x295b2d27), f32::from_bits(0x2d685568)),
    );
    path.move_to((f32::from_bits(0x68556809), f32::from_bits(0x8c555b2d)));
    path.move_to((f32::from_bits(0x081f2a21), f32::from_bits(0x252d0321)));
    path.move_to((f32::from_bits(0x5568392a), f32::from_bits(0x5b2df068)));
    path.quad_to(
        (f32::from_bits(0x2a1f2a8c), f32::from_bits(0x21482a21)),
        (f32::from_bits(0x4b7bc003), f32::from_bits(0x8ced3a6a)),
    );
    path.move_to((f32::from_bits(0x21481f21), f32::from_bits(0x4b7bc003)));
    path.conic_to(
        (f32::from_bits(0x6829ed27), f32::from_bits(0x2d155b2d)),
        (f32::from_bits(0x5568295b), f32::from_bits(0x5b2d2968)),
        f32::from_bits(0x2a8c8f55),
    );
    path.line_to((f32::from_bits(0x21481f21), f32::from_bits(0x4b7bc003)));
    path.close();
    path.move_to((f32::from_bits(0xc021211f), f32::from_bits(0x6a4b7b03)));
    path.conic_to(
        (f32::from_bits(0x682d2fed), f32::from_bits(0x755b6829)),
        (f32::from_bits(0x5b292d2b), f32::from_bits(0xc92a8c55)),
        f32::from_bits(0x081f2a21),
    );
    path.line_to((f32::from_bits(0xc021211f), f32::from_bits(0x6a4b7b03)));
    path.close();
    path.move_to((f32::from_bits(0xc021211f), f32::from_bits(0x6a4b7b03)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x212aed7a)),
        (f32::from_bits(0x0321081f), f32::from_bits(0x293a7bc0)),
        f32::from_bits(0x2147ed7a),
    );
    path.quad_to(
        (f32::from_bits(0x6829682d), f32::from_bits(0x292d555b)),
        (f32::from_bits(0x292a8c55), f32::from_bits(0x21081f2a)),
    );
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x218ced7a)),
        (f32::from_bits(0x0321081f), f32::from_bits(0x6a3a7bc0)),
        f32::from_bits(0x47ed3a7a),
    );
    path.line_to((f32::from_bits(0xc021211f), f32::from_bits(0x6a4b7b03)));
    path.close();
    path.move_to((f32::from_bits(0xc021211f), f32::from_bits(0x6a4b7b03)));
    path.quad_to(
        (f32::from_bits(0x282a282a), f32::from_bits(0x8a3a21df)),
        (f32::from_bits(0x2728282a), f32::from_bits(0x8a3a21df)),
    );
    path.quad_to(
        (f32::from_bits(0x8a284f9a), f32::from_bits(0x3a3ac2b3)),
        (f32::from_bits(0x2a292827), f32::from_bits(0x962be61d)),
    );
    path.line_to((f32::from_bits(0x272a802a), f32::from_bits(0x2a8c2d29)));
    path.line_to((f32::from_bits(0xc021211f), f32::from_bits(0x6a4b7b03)));
    path.close();
    path.move_to((f32::from_bits(0x4f9a3a29), f32::from_bits(0x3ab38a28)));
    path.quad_to(
        (f32::from_bits(0xc368305b), f32::from_bits(0x5b296855)),
        (f32::from_bits(0x2d8c5568), f32::from_bits(0x1f2a2172)),
    );
    path.line_to((f32::from_bits(0x29c00321), f32::from_bits(0x5b4b7b13)));
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L10646-L10699 (chrome/m156)
fn fuzz763_16(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68372d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c552775), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x69555b2d),
    );
    path.move_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x8cef552a)),
        (f32::from_bits(0x295b2d2a), f32::from_bits(0x68210368)),
        f32::from_bits(0x7bc05508),
    );
    path.line_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x212a8c55)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.conic_to(
        (f32::from_bits(0x68385b2d), f32::from_bits(0x555bf055)),
        (f32::from_bits(0x2a1f2a8c), f32::from_bits(0x03212a21)),
        f32::from_bits(0x5a4b7bc0),
    );
    path.conic_to(
        (f32::from_bits(0xc08c2aed), f32::from_bits(0x211f2108)),
        (f32::from_bits(0x6a4b7b03), f32::from_bits(0x6829ed27)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x6e2a1f72)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
        f32::from_bits(0x4793ed7a),
    );
    path.line_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.quad_to(
        (f32::from_bits(0x2128282a), f32::from_bits(0x3a8a3adf)),
        (f32::from_bits(0x8a284f1a), f32::from_bits(0x2c213ab3)),
    );
    path.line_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.quad_to(
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x43962be6)),
        (f32::from_bits(0x3a2a812a), f32::from_bits(0x2a8ced29)),
    );
    path.line_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x03210831), f32::from_bits(0x6a4b7bc0)),
        (f32::from_bits(0x681aed27), f32::from_bits(0x55555b2d)),
        f32::from_bits(0x1e2a3a2a),
    );
    path.conic_to(
        (f32::from_bits(0x27202140), f32::from_bits(0x3a3b2769)),
        (f32::from_bits(0xc4371f20), f32::from_bits(0xecc52a22)),
        f32::from_bits(0x21512727),
    );
    path.line_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x6829523a), f32::from_bits(0x2d555b2d)));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a322a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68572d55), f32::from_bits(0xf05bd24b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0xed4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8c6a), f32::from_bits(0x0329081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x8cef552a)),
        (f32::from_bits(0x295b2d2a), f32::from_bits(0x68210368)),
        f32::from_bits(0x7bc05508),
    );
    path.line_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x555b6829), f32::from_bits(0x6c212a8c)));
    path.conic_to(
        (f32::from_bits(0x084b0321), f32::from_bits(0x6ac07b2a)),
        (f32::from_bits(0x395b2d7a), f32::from_bits(0x5bf05568)),
        f32::from_bits(0x212a3a8c),
    );
    path.line_to((f32::from_bits(0x8c558c55), f32::from_bits(0x212a1f2a)));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10701-L10751 (chrome/m156)
fn fuzz763_17(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68392d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x69555b2d),
    );
    path.move_to((f32::from_bits(0x6835282d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x2aef552a)),
        (f32::from_bits(0x68295b2d), f32::from_bits(0x08682103)),
        f32::from_bits(0x4b7bc055),
    );
    path.line_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x212a8c55)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.conic_to(
        (f32::from_bits(0x68395b2d), f32::from_bits(0x555bf055)),
        (f32::from_bits(0x2a1f2a8c), f32::from_bits(0x03212a21)),
        f32::from_bits(0x5a4b7bc0),
    );
    path.conic_to(
        (f32::from_bits(0xc08c2aed), f32::from_bits(0x211f2108)),
        (f32::from_bits(0x6a4b7b03), f32::from_bits(0x6829ed27)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x212a1f72)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
        f32::from_bits(0x254793ed),
    );
    path.quad_to(
        (f32::from_bits(0x2128282a), f32::from_bits(0x3a8a3adf)),
        (f32::from_bits(0x8a284f1a), f32::from_bits(0xc2213ab3)),
    );
    path.quad_to(
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x43962be6)),
        (f32::from_bits(0x3a2a812a), f32::from_bits(0x2a8ced29)),
    );
    path.line_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x03210831), f32::from_bits(0x6a4b7bc0)),
        (f32::from_bits(0x6829ed27), f32::from_bits(0x55555b2d)),
        f32::from_bits(0x1e2a3a2a),
    );
    path.conic_to(
        (f32::from_bits(0x27202140), f32::from_bits(0x3a3b2729)),
        (f32::from_bits(0xc4371f20), f32::from_bits(0x16c52a22)),
        f32::from_bits(0x515d27ec),
    );
    path.line_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x6829523a), f32::from_bits(0x2d555b2d)));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68572d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x2a8c555b), f32::from_bits(0x2a212a1f)));
    path.line_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x8c2aed7a), f32::from_bits(0x2a1f08c0)));
    path.line_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x2a8cef55), f32::from_bits(0x68295b2d)));
    path.conic_to(
        (f32::from_bits(0x55086821), f32::from_bits(0x6a4b7bc0)),
        (f32::from_bits(0x5b2c6829), f32::from_bits(0x21218c55)),
        f32::from_bits(0x2a6c1f03),
    );
    path.line_to((f32::from_bits(0x2a8cef55), f32::from_bits(0x68295b2d)));
    path.close();
    path.move_to((f32::from_bits(0x2a8cef55), f32::from_bits(0x68295b2d)));
    path.line_to((f32::from_bits(0x6ac07b2a), f32::from_bits(0x395b2d7a)));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10753-L10802 (chrome/m156)
fn fuzz763_18(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68392d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x69555b2d),
    );
    path.move_to((f32::from_bits(0x6835282d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x2aef552a)),
        (f32::from_bits(0x68295b2d), f32::from_bits(0x08682103)),
        f32::from_bits(0x4b7bc055),
    );
    path.line_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x212a8c55)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.conic_to(
        (f32::from_bits(0x68395b2d), f32::from_bits(0x555bf055)),
        (f32::from_bits(0x2a1f2a8c), f32::from_bits(0x03212a21)),
        f32::from_bits(0x5a4b7bc0),
    );
    path.conic_to(
        (f32::from_bits(0xc08c2aed), f32::from_bits(0x211f2108)),
        (f32::from_bits(0x6a4b7b03), f32::from_bits(0x6829ed27)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x212a1f72)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
        f32::from_bits(0x254793ed),
    );
    path.quad_to(
        (f32::from_bits(0x2128282a), f32::from_bits(0x3a8a3adf)),
        (f32::from_bits(0x8a284f1a), f32::from_bits(0xc2213ab3)),
    );
    path.quad_to(
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x43962be6)),
        (f32::from_bits(0x3a2a812a), f32::from_bits(0x2a8ced29)),
    );
    path.line_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x03210831), f32::from_bits(0x6a4b7bc0)),
        (f32::from_bits(0x6829ed27), f32::from_bits(0x55555b2d)),
        f32::from_bits(0x1e2a3a2a),
    );
    path.conic_to(
        (f32::from_bits(0x27202140), f32::from_bits(0x3a3b2729)),
        (f32::from_bits(0xc4371f20), f32::from_bits(0x16c52a22)),
        f32::from_bits(0x515d27ec),
    );
    path.line_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x6829523a), f32::from_bits(0x2d555b2d)));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68572d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x2a8c555b), f32::from_bits(0x2a212a1f)));
    path.line_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x8c2aed7a), f32::from_bits(0x2a1f08c0)));
    path.move_to((f32::from_bits(0x6829523a), f32::from_bits(0x2d555b2d)));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68572d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x2a8c54ed), f32::from_bits(0x21081f21)),
        (f32::from_bits(0x4b7bc003), f32::from_bits(0x29ed846a)),
        f32::from_bits(0x555b2d28),
    );
    path.conic_to(
        (f32::from_bits(0x68392d5b), f32::from_bits(0xf0682955)),
        (f32::from_bits(0x2a1f5b2d), f32::from_bits(0xef552a21)),
        f32::from_bits(0x5b2d2a8c),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10804-L10852 (chrome/m156)
fn fuzz763_19(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x21081f21), f32::from_bits(0x4b7bc003)));
    path.line_to((f32::from_bits(0x2829ed84), f32::from_bits(0x69555b2d)));
    path.move_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x2aef552a)),
        (f32::from_bits(0x68295b2d), f32::from_bits(0x08682103)),
        f32::from_bits(0x4b7bc055),
    );
    path.move_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x212a8c55)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.conic_to(
        (f32::from_bits(0x68395b2d), f32::from_bits(0x8c5bf055)),
        (f32::from_bits(0x2a1f2a55), f32::from_bits(0x03212a21)),
        f32::from_bits(0x5a4b7bc0),
    );
    path.conic_to(
        (f32::from_bits(0xc08c2aed), f32::from_bits(0x211f2108)),
        (f32::from_bits(0x6a4b7b03), f32::from_bits(0x6829ed27)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x212a1f72)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
        f32::from_bits(0x2547937a),
    );
    path.quad_to(
        (f32::from_bits(0x2128282a), f32::from_bits(0x3a8a3adf)),
        (f32::from_bits(0x8a284f1a), f32::from_bits(0xc2213ab3)),
    );
    path.quad_to(
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x43962be6)),
        (f32::from_bits(0x3a2a812a), f32::from_bits(0x2a8ced29)),
    );
    path.line_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x03210831), f32::from_bits(0x6a4b7bc0)),
        (f32::from_bits(0x6829ed27), f32::from_bits(0x55555b2d)),
        f32::from_bits(0x1e2a3a2a),
    );
    path.conic_to(
        (f32::from_bits(0x27202140), f32::from_bits(0x3a3b2729)),
        (f32::from_bits(0xc4371f20), f32::from_bits(0xecc52a22)),
        f32::from_bits(0x21515d27),
    );
    path.line_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x6829523a), f32::from_bits(0x2d555b2d)));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68572d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x8cef552a)),
        (f32::from_bits(0x295b2d2a), f32::from_bits(0x68210368)),
        f32::from_bits(0x7bc05508),
    );
    path.line_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x555b2c29), f32::from_bits(0x6c212a8c)));
    path.conic_to(
        (f32::from_bits(0x084b0321), f32::from_bits(0x6ac07b2a)),
        (f32::from_bits(0x395b2d7a), f32::from_bits(0xf05b5568)),
        f32::from_bits(0x212a3a8c),
    );
    path.conic_to(
        (f32::from_bits(0x290321d9), f32::from_bits(0x555b2d68)),
        (f32::from_bits(0x2a8c558c), f32::from_bits(0x2abe2a1f)),
        f32::from_bits(0x7bc00321),
    );
    path.line_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x8c2aed7a), f32::from_bits(0x1f2128c0)));
    path.line_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10854-L10910 (chrome/m156)
fn fuzz763_20(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68392d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x69555b2d),
    );
    path.move_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x2a8c552a)),
        (f32::from_bits(0x68295b2d), f32::from_bits(0x08682103)),
        f32::from_bits(0x4b7bc055),
    );
    path.line_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x212a8c55)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.conic_to(
        (f32::from_bits(0x68395b2d), f32::from_bits(0x555bf055)),
        (f32::from_bits(0x2a1f2a8c), f32::from_bits(0x03212a21)),
        f32::from_bits(0x5a4b7bc0),
    );
    path.conic_to(
        (f32::from_bits(0xc08c2aed), f32::from_bits(0x211f2108)),
        (f32::from_bits(0x6a4b7b03), f32::from_bits(0x6829ed27)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x6e2a1f72)),
        (f32::from_bits(0x0321182a), f32::from_bits(0x6a4b7bc0)),
        f32::from_bits(0x4793ed7a),
    );
    path.line_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.quad_to(
        (f32::from_bits(0x2128282a), f32::from_bits(0x3a8a3adf)),
        (f32::from_bits(0x8a284f1a), f32::from_bits(0x2c213ab3)),
    );
    path.line_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.quad_to(
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x43962be6)),
        (f32::from_bits(0x3a2a812a), f32::from_bits(0x2a8ced29)),
    );
    path.line_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x03210831), f32::from_bits(0x6a4b7bc0)),
        (f32::from_bits(0x6829ed27), f32::from_bits(0x55555b2d)),
        f32::from_bits(0x1e2a3a2a),
    );
    path.conic_to(
        (f32::from_bits(0x27202140), f32::from_bits(0x3a3b2769)),
        (f32::from_bits(0xc4371f20), f32::from_bits(0xecc52a22)),
        f32::from_bits(0x51282727),
    );
    path.line_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x6829523a), f32::from_bits(0x2d555b2d)));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x8c555b2d)));
    path.move_to((f32::from_bits(0x081f2a31), f32::from_bits(0xc0032921)));
    path.cubic_to(
        (f32::from_bits(0x68572d55), f32::from_bits(0xf05bd24b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0xed4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8c6a), f32::from_bits(0x4329081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x5b2d2d55),
    );
    path.move_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x8cef552a)),
        (f32::from_bits(0x295b2d2a), f32::from_bits(0x3a210368)),
        f32::from_bits(0x7bc05508),
    );
    path.line_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x555b6829), f32::from_bits(0x6c212a8c)));
    path.line_to((f32::from_bits(0x5b2d7a6a), f32::from_bits(0xf0556830)));
    path.line_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x0321d90a), f32::from_bits(0x555b2d68)),
        (f32::from_bits(0x2a8c558c), f32::from_bits(0x212a2a1f)),
        f32::from_bits(0x4b7bc003),
    );
    path.line_to((f32::from_bits(0x8c2aed7a), f32::from_bits(0x212128c0)));
    path.line_to((f32::from_bits(0x68395b2d), f32::from_bits(0xf0682955)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10912-L10953 (chrome/m156)
fn fuzz763_21(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x6828c6f9), f32::from_bits(0x6614dc9e)));
    path.cubic_to(
        (f32::from_bits(0x68303469), f32::from_bits(0x661f92fc)),
        (f32::from_bits(0x6837d3c3), f32::from_bits(0x662b0eb2)),
        (f32::from_bits(0x683fa268), f32::from_bits(0x663759e1)),
    );
    path.cubic_to(
        (f32::from_bits(0x68c4391f), f32::from_bits(0x672c5c9f)),
        (f32::from_bits(0x688b20ab), f32::from_bits(0x6804b825)),
        (f32::from_bits(0x681ddb5e), f32::from_bits(0x6838dc00)),
    );
    path.line_to((f32::from_bits(0x6828c6f9), f32::from_bits(0x6614dc9e)));
    path.close();
    path.move_to((f32::from_bits(0x68226c73), f32::from_bits(0x660bd15e)));
    path.cubic_to(
        (f32::from_bits(0x6823b0e1), f32::from_bits(0x660d990f)),
        (f32::from_bits(0x6824f6d5), f32::from_bits(0x660f668c)),
        (f32::from_bits(0x68263e4e), f32::from_bits(0x66113632)),
    );
    path.cubic_to(
        (f32::from_bits(0x682715e4), f32::from_bits(0x6612676d)),
        (f32::from_bits(0x6827ee22), f32::from_bits(0x66139997)),
        (f32::from_bits(0x6828c709), f32::from_bits(0x6614cba5)),
    );
    path.line_to((f32::from_bits(0x6828d720), f32::from_bits(0x6604a1a2)));
    path.cubic_to(
        (f32::from_bits(0x68270421), f32::from_bits(0x6601102c)),
        (f32::from_bits(0x68252b97), f32::from_bits(0x65fb1edd)),
        (f32::from_bits(0x68234ce5), f32::from_bits(0x65f4367f)),
    );
    path.conic_to(
        (f32::from_bits(0x6822e012), f32::from_bits(0x6602acc5)),
        (f32::from_bits(0x68226c73), f32::from_bits(0x660bd15e)),
        f32::from_bits(0x3f7ffa04),
    );
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x6a2a291f)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x68555b2a)));
    path.cubic_to(
        (f32::from_bits(0x00000000), f32::from_bits(0x68617414)),
        (f32::from_bits(0x66af1c42), f32::from_bits(0x68624f96)),
        (f32::from_bits(0x6757755b), f32::from_bits(0x685b93f2)),
    );
    path.cubic_to(
        (f32::from_bits(0x67a63a84), f32::from_bits(0x68fe1c37)),
        (f32::from_bits(0x67c05eed), f32::from_bits(0x69930962)),
        (f32::from_bits(0x00000000), f32::from_bits(0x6a2a291f)),
    );
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x6a2a291f)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x6a4b7bc4)));
    path.cubic_to(
        (f32::from_bits(0x6a2c8798), f32::from_bits(0x68f7a144)),
        (f32::from_bits(0x6951f5ea), f32::from_bits(0x6796ad55)),
        (f32::from_bits(0x683fa268), f32::from_bits(0x663759e1)),
    );
    path.cubic_to(
        (f32::from_bits(0x683871e3), f32::from_bits(0x66253b4f)),
        (f32::from_bits(0x6830da01), f32::from_bits(0x66144d3e)),
        (f32::from_bits(0x6828d720), f32::from_bits(0x6604a1a2)),
    );
    path.conic_to(
        (f32::from_bits(0x68295b21), f32::from_bits(0x00000000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
        f32::from_bits(0x492bb324),
    );
    path.cubic_to(
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
        (f32::from_bits(0x677b84f0), f32::from_bits(0x00000000)),
        (f32::from_bits(0x68226c73), f32::from_bits(0x660bd15e)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x68156829)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x68555b2a)));
    path.line_to((f32::from_bits(0x673918f3), f32::from_bits(0x681b0f5f)));
    path.line_to((f32::from_bits(0x67391759), f32::from_bits(0x681b0fae)));
    path.cubic_to(
        (f32::from_bits(0x674384e7), f32::from_bits(0x682e2068)),
        (f32::from_bits(0x674db698), f32::from_bits(0x6843893b)),
        (f32::from_bits(0x6757755b), f32::from_bits(0x685b93f2)),
    );
    path.cubic_to(
        (f32::from_bits(0x67a63484), f32::from_bits(0x68556bdd)),
        (f32::from_bits(0x67f18c5f), f32::from_bits(0x6848eb25)),
        (f32::from_bits(0x681ddb5e), f32::from_bits(0x6838dc00)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x6a2a291f)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10955-L10979 (chrome/m156)
fn fuzz763_22(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x68295b2d)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x6a3a7bc0), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x6a034b21)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x68295b2d)));
    path.close();
    path.move_to((f32::from_bits(0x6617da56), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x5e704d09), f32::from_bits(0x5e3a4dfd)),
        (f32::from_bits(0x00000000), f32::from_bits(0x65eb62ef)),
        f32::from_bits(0x430fa5e6),
    );
    path.conic_to(
        (f32::from_bits(0x5e798b32), f32::from_bits(0x627a95c0)),
        (f32::from_bits(0x61f5014c), f32::from_bits(0x61fba0fd)),
        f32::from_bits(0x40f8a1a1),
    );
    path.conic_to(
        (f32::from_bits(0x62743d2d), f32::from_bits(0x5e49b862)),
        (f32::from_bits(0x6617da56), f32::from_bits(0x00000000)),
        f32::from_bits(0x410ef54c),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x4f9a3a8a), f32::from_bits(0xc28a0d28)),
        (f32::from_bits(0x273a3ab3), f32::from_bits(0x8b2a2928)),
    );
    path.line_to((f32::from_bits(0x63283ae6), f32::from_bits(0x27282a81)));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L10981-L11020 (chrome/m156)
fn fuzz763_23(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68392d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x03210c2a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x69555b2d),
    );
    path.move_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x2aef552a)),
        (f32::from_bits(0x29295b2d), f32::from_bits(0x68210368)),
        f32::from_bits(0x7bc05508),
    );
    path.line_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x212a8c55)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.conic_to(
        (f32::from_bits(0x68395b2d), f32::from_bits(0x8c5bf055)),
        (f32::from_bits(0x2a1f2a55), f32::from_bits(0x03212a21)),
        f32::from_bits(0x5a4b7bc0),
    );
    path.conic_to(
        (f32::from_bits(0xc08c2aed), f32::from_bits(0x211f2108)),
        (f32::from_bits(0x6a4b7b03), f32::from_bits(0x6829ed27)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x08211f72)),
        (f32::from_bits(0x032a2a21), f32::from_bits(0x6a4b7bc0)),
        f32::from_bits(0x2547937a),
    );
    path.quad_to(
        (f32::from_bits(0x2128282a), f32::from_bits(0x3a8a3adf)),
        (f32::from_bits(0x8a284f1a), f32::from_bits(0xc2213ab3)),
    );
    path.quad_to(
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x43962be6)),
        (f32::from_bits(0x3a2a812a), f32::from_bits(0x2a8ced29)),
    );
    path.line_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x03210831), f32::from_bits(0x6a4b7bc0)),
        (f32::from_bits(0x6829ed27), f32::from_bits(0x55555b2d)),
        f32::from_bits(0x1e2a3a2a),
    );
    path.conic_to(
        (f32::from_bits(0x27202140), f32::from_bits(0x3a3b2729)),
        (f32::from_bits(0xc4371f20), f32::from_bits(0xecc52a22)),
        f32::from_bits(0x21515d27),
    );
    path.line_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x6829523a), f32::from_bits(0x2d555b2d)));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68572d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x2a8c54ed), f32::from_bits(0x21081f21)),
        (f32::from_bits(0x4b7bc003), f32::from_bits(0x29ed846a)),
        f32::from_bits(0x555b2d28),
    );
    path.conic_to(
        (f32::from_bits(0x68392d5b), f32::from_bits(0xf0682955)),
        (f32::from_bits(0x2a1f5b2d), f32::from_bits(0xef552a21)),
        f32::from_bits(0x5b2d2a8c),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11022-L11069 (chrome/m156)
fn fuzz763_24(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0xededed02), f32::from_bits(0xedededed)));
    path.close();
    path.move_to((f32::from_bits(0xededed02), f32::from_bits(0xedededed)));
    path.quad_to(
        (f32::from_bits(0x9fb9c16e), f32::from_bits(0x27737375)),
        (f32::from_bits(0xb7c5ff00), f32::from_bits(0x00ff9908)),
    );
    path.move_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273)));
    path.cubic_to(
        (f32::from_bits(0x1616ecec), f32::from_bits(0x2c321616)),
        (f32::from_bits(0x3516c616), f32::from_bits(0x6e161616)),
        (f32::from_bits(0x4c416033), f32::from_bits(0xf6000000)),
    );
    path.quad_to(
        (f32::from_bits(0x04007f41), f32::from_bits(0xecececec)),
        (f32::from_bits(0xecececec), f32::from_bits(0xecec41ec)),
    );
    path.line_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273)));
    path.close();
    path.move_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273)));
    path.quad_to(
        (f32::from_bits(0x000000ec), f32::from_bits(0xececcc00)),
        (f32::from_bits(0x48ececec), f32::from_bits(0x0278806e)),
    );
    path.line_to((f32::from_bits(0x72ececec), f32::from_bits(0xecec02ec)));
    path.quad_to(
        (f32::from_bits(0xec04007f), f32::from_bits(0xecececec)),
        (f32::from_bits(0xecececec), f32::from_bits(0xecec0400)),
    );
    path.line_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273)));
    path.close();
    path.move_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273)));
    path.quad_to(
        (f32::from_bits(0x000040ec), f32::from_bits(0x3a333300)),
        (f32::from_bits(0xecec3333), f32::from_bits(0xececdbec)),
    );
    path.line_to((f32::from_bits(0x3300007f), f32::from_bits(0x33d83333)));
    path.line_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273)));
    path.close();
    path.move_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273)));
    path.quad_to(
        (f32::from_bits(0x9e9ea900), f32::from_bits(0x33ececec)),
        (f32::from_bits(0xececec33), f32::from_bits(0xec336e6e)),
    );
    path.line_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273)));
    path.close();
    path.move_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273)));
    path.line_to((f32::from_bits(0xedededed), f32::from_bits(0xedededed)));
    path.line_to((f32::from_bits(0xecececec), f32::from_bits(0xecececec)));
    path.line_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273)));
    path.close();
    path.move_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273)));
    path.line_to((f32::from_bits(0x01003300), f32::from_bits(0x33d83333)));
    path.quad_to(
        (f32::from_bits(0xecec3333), f32::from_bits(0x04eeedec)),
        (f32::from_bits(0xe0e0e0e0), f32::from_bits(0x9ee0e0e0)),
    );
    path.line_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273)));
    path.close();
    path.move_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273)));
    path.cubic_to(
        (f32::from_bits(0x299e9e9e), f32::from_bits(0xecececec)),
        (f32::from_bits(0xececb6ec), f32::from_bits(0xf0ececec)),
        (f32::from_bits(0x0000ecec), f32::from_bits(0x9ebe6e6e)),
    );
    path.cubic_to(
        (f32::from_bits(0x9e9e9e9e), f32::from_bits(0xe8009e9e)),
        (f32::from_bits(0x9e9e9e9e), f32::from_bits(0xecec9e9e)),
        (f32::from_bits(0xec3333ec), f32::from_bits(0xececf0ec)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11071-L11093 (chrome/m156)
fn fuzz763_25(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x6a4b7bc4)));
    path.conic_to(
        (f32::from_bits(0x653140d9), f32::from_bits(0x6a4b4f74)),
        (f32::from_bits(0x65906630), f32::from_bits(0x6a25a070)),
        f32::from_bits(0x3f6728a2),
    );
    path.cubic_to(
        (f32::from_bits(0x68295bc5), f32::from_bits(0x00000000)),
        (f32::from_bits(0x682958ff), f32::from_bits(0x00000000)),
        (f32::from_bits(0x68286829), f32::from_bits(0x00000000)),
    );
    path.line_to((f32::from_bits(0x68555b29), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x00000000), f32::from_bits(0x682d2927)),
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
        f32::from_bits(0x6829686f),
    );
    path.line_to((f32::from_bits(0xdf218a28), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x6a4b7bc4)));
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x6642c40c), f32::from_bits(0x00000000)),
        (f32::from_bits(0x65906630), f32::from_bits(0x6a25a070)),
        f32::from_bits(0x3edcd74d),
    );
    path.conic_to(
        (f32::from_bits(0x68295afa), f32::from_bits(0x00000000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
        f32::from_bits(0x4277a57b),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L11096-L11152 (chrome/m156)
fn fuzz763_26(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc003210a)));
    path.cubic_to(
        (f32::from_bits(0x68372d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x69555b2d),
    );
    path.move_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x8cef552a)),
        (f32::from_bits(0x295b2d2a), f32::from_bits(0x68210368)),
        f32::from_bits(0x7bc05508),
    );
    path.line_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x68315b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x212a8c55)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.conic_to(
        (f32::from_bits(0x68385b2d), f32::from_bits(0x555bf055)),
        (f32::from_bits(0x2a1f2a8c), f32::from_bits(0x03212121)),
        f32::from_bits(0x5a4b7bc0),
    );
    path.conic_to(
        (f32::from_bits(0xc08c2aed), f32::from_bits(0x211f2108)),
        (f32::from_bits(0x6a4b7b03), f32::from_bits(0x6829ed27)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x68355b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x6e2a1f72)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
        f32::from_bits(0x4793ed7a),
    );
    path.line_to((f32::from_bits(0x68355b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68355b2d), f32::from_bits(0xf0685527)));
    path.quad_to(
        (f32::from_bits(0x2128282a), f32::from_bits(0x3a8a3adf)),
        (f32::from_bits(0x8a284f1a), f32::from_bits(0x2c213ab3)),
    );
    path.line_to((f32::from_bits(0x68355b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68355b2d), f32::from_bits(0xf0685527)));
    path.quad_to(
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x43962be6)),
        (f32::from_bits(0x3a2a812a), f32::from_bits(0x2127ed29)),
    );
    path.conic_to(
        (f32::from_bits(0x03210831), f32::from_bits(0x6a4b7bc0)),
        (f32::from_bits(0x6829ed27), f32::from_bits(0x55555b2d)),
        f32::from_bits(0x1e2a3a2a),
    );
    path.conic_to(
        (f32::from_bits(0x27202140), f32::from_bits(0x3a3b2769)),
        (f32::from_bits(0xc4371f20), f32::from_bits(0xecc52a22)),
        f32::from_bits(0x21512727),
    );
    path.line_to((f32::from_bits(0x68355b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x6829523a), f32::from_bits(0x2d555b2d)));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x5b2d5529)));
    path.move_to((f32::from_bits(0x1f2a322a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68572d55), f32::from_bits(0xf05bd24b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0xed4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8c6a), f32::from_bits(0x0329081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0x68385b2d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x8cef552a)),
        (f32::from_bits(0x295b2d2a), f32::from_bits(0x68210368)),
        f32::from_bits(0x7bc05508),
    );
    path.line_to((f32::from_bits(0x68385b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x68385b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x555b1b29), f32::from_bits(0x6c212a8c)));
    path.conic_to(
        (f32::from_bits(0x084b0321), f32::from_bits(0x6ac07b2a)),
        (f32::from_bits(0x395b2d7a), f32::from_bits(0x8c5bf055)),
        f32::from_bits(0x1f212a3a),
    );
    path.conic_to(
        (f32::from_bits(0x290321d9), f32::from_bits(0x555b2d68)),
        (f32::from_bits(0x2a8c558c), f32::from_bits(0x2a212a1f)),
        f32::from_bits(0x7bc00321),
    );
    path.line_to((f32::from_bits(0x68385b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0x68385b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x8c2aed7a), f32::from_bits(0x1f2128c0)));
    path.line_to((f32::from_bits(0x68385b2d), f32::from_bits(0xf0682955)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11154-L11190 (chrome/m156)
fn fuzz763_28(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68302d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a1f2a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6aa37bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2d28ed84)),
        f32::from_bits(0x5b2d2955),
    );
    path.move_to((f32::from_bits(0x6c395b2d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x2aef8c55)),
        (f32::from_bits(0x68295b2d), f32::from_bits(0x21086855)),
        f32::from_bits(0x4b7bc003),
    );
    path.line_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x212a8c55)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.line_to((f32::from_bits(0x8a283a28), f32::from_bits(0x284f1a3a)));
    path.quad_to(
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x43962be6)),
        (f32::from_bits(0x272a812a), f32::from_bits(0x3a2a5529)),
    );
    path.line_to((f32::from_bits(0x213b1e2a), f32::from_bits(0x27292720)));
    path.conic_to(
        (f32::from_bits(0x381f203a), f32::from_bits(0x2ac422c5)),
        (f32::from_bits(0xc25d27ec), f32::from_bits(0x3a705921)),
        f32::from_bits(0x2a105152),
    );
    path.quad_to(
        (f32::from_bits(0x633ad912), f32::from_bits(0x29c80927)),
        (f32::from_bits(0x272927b0), f32::from_bits(0x683a5b2d)),
    );
    path.line_to((f32::from_bits(0x295b2d68), f32::from_bits(0x29685568)));
    path.conic_to(
        (f32::from_bits(0xaa8c555b), f32::from_bits(0x081f2a21)),
        (f32::from_bits(0x5b2d0321), f32::from_bits(0x68556829)),
        f32::from_bits(0x2a552d29),
    );
    path.cubic_to(
        (f32::from_bits(0x21295b2d), f32::from_bits(0x2a688c5b)),
        (f32::from_bits(0x68295b2d), f32::from_bits(0x2d296855)),
        (f32::from_bits(0x8c08555b), f32::from_bits(0x2a2a29ca)),
    );
    path.quad_to(
        (f32::from_bits(0x68295b21), f32::from_bits(0x2d296855)),
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x081f2a21)),
    );
    path.line_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.close();
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x5b2d6829)),
        (f32::from_bits(0x1f212a55), f32::from_bits(0x8ced7aba)),
        f32::from_bits(0x3f2a212a),
    );
    path.line_to((f32::from_bits(0x5b2d212d), f32::from_bits(0x2d556829)));
    path.move_to((f32::from_bits(0x68552968), f32::from_bits(0x5568295b)));
    path.move_to((f32::from_bits(0x5b2d2968), f32::from_bits(0x212a8c55)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x6a4b7bc0)));
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a3a7bc0), f32::from_bits(0x2147ed7a)),
        f32::from_bits(0x28282a3a),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11192-L11206 (chrome/m156)
fn fuzz763_27(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x30309ab8), f32::from_bits(0x305b3030)),
        (f32::from_bits(0x00f53030), f32::from_bits(0x3a3a0000)),
    );
    path.quad_to(
        (f32::from_bits(0xb8b8d5b8), f32::from_bits(0x0b0b0b03)),
        (f32::from_bits(0x0b0b0b0b), f32::from_bits(0x3a3a0b0b)),
    );
    path.quad_to(
        (f32::from_bits(0xb8b8b8b8), f32::from_bits(0x0b1203b8)),
        (f32::from_bits(0x0b0b0b0b), f32::from_bits(0x3a3a2110)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L11208-L11238 (chrome/m156)
fn fuzz763_29(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x743e0000)));
    path.cubic_to(
        (f32::from_bits(0x74083cf1), f32::from_bits(0x74536e73)),
        (f32::from_bits(0x742ac4e4), f32::from_bits(0x7415f5be)),
        (f32::from_bits(0x7433ee3c), f32::from_bits(0x7405a69a)),
    );
    path.quad_to(
        (f32::from_bits(0x74360ca0), f32::from_bits(0x7401e10c)),
        (f32::from_bits(0x7436a382), f32::from_bits(0x7401cc18)),
    );
    path.cubic_to(
        (f32::from_bits(0x74374a91), f32::from_bits(0x7401ef19)),
        (f32::from_bits(0x74375c84), f32::from_bits(0x7404d9b9)),
        (f32::from_bits(0x7437868f), f32::from_bits(0x740bae8a)),
    );
    path.cubic_to(
        (f32::from_bits(0x7437d6c1), f32::from_bits(0x7418b629)),
        (f32::from_bits(0x74387e9b), f32::from_bits(0x7433fbc5)),
        (f32::from_bits(0x743e2ff7), f32::from_bits(0x74655fa2)),
    );
    path.cubic_to(
        (f32::from_bits(0x741ada75), f32::from_bits(0x74745717)),
        (f32::from_bits(0x73c106b4), f32::from_bits(0x74744e64)),
        (f32::from_bits(0x00000000), f32::from_bits(0x74744006)),
    );
    path.cubic_to(
        (f32::from_bits(0x00000000), f32::from_bits(0x74746c7c)),
        (f32::from_bits(0x74244dce), f32::from_bits(0x7474733e)),
        (f32::from_bits(0x74400000), f32::from_bits(0x74747445)),
    );
    path.cubic_to(
        (f32::from_bits(0x743f5854), f32::from_bits(0x746f3659)),
        (f32::from_bits(0x743ebe05), f32::from_bits(0x746a3017)),
        (f32::from_bits(0x743e2ff7), f32::from_bits(0x74655fa2)),
    );
    path.cubic_to(
        (f32::from_bits(0x7447a582), f32::from_bits(0x74615dee)),
        (f32::from_bits(0x744f74f6), f32::from_bits(0x745c4903)),
        (f32::from_bits(0x7455e7e6), f32::from_bits(0x7455d751)),
    );
    path.cubic_to(
        (f32::from_bits(0x74747474), f32::from_bits(0x743750a4)),
        (f32::from_bits(0x74747474), f32::from_bits(0x73f46f0d)),
        (f32::from_bits(0x74747474), f32::from_bits(0x00000000)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0xf0682955), f32::from_bits(0x211f5b2d)));
    path.move_to((f32::from_bits(0x2d2aff2d), f32::from_bits(0x74747474)));
    path.cubic_to(
        (f32::from_bits(0x7474748e), f32::from_bits(0x74747490)),
        (f32::from_bits(0x8c722174), f32::from_bits(0x181f0080)),
        (f32::from_bits(0x74c0e520), f32::from_bits(0x747d7463)),
    );
    path.cubic_to(
        (f32::from_bits(0x7b005e4b), f32::from_bits(0xdf3a6a3a)),
        (f32::from_bits(0x2a3a2848), f32::from_bits(0x2d2d7821)),
        (f32::from_bits(0x8c55212d), f32::from_bits(0x2d2d2d24)),
    );
    path.conic_to(
        (f32::from_bits(0xde28804c), f32::from_bits(0x28e03721)),
        (f32::from_bits(0x3329df28), f32::from_bits(0x2d291515)),
        f32::from_bits(0x0568295b),
    );
    path.conic_to(
        (f32::from_bits(0x556a2d21), f32::from_bits(0x21088c2a)),
        (f32::from_bits(0x3a333303), f32::from_bits(0x5b293a8a)),
        f32::from_bits(0x6855683b),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11240-L11268 (chrome/m156)
fn fuzz763_30(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x1f2108c0), f32::from_bits(0x4b7b0321)));
    path.line_to((f32::from_bits(0x6829ed27), f32::from_bits(0x2d555b2d)));
    path.move_to((f32::from_bits(0x68305b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x6e2a1f72)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x2a4b7bc0)),
        f32::from_bits(0x68295b2d),
    );
    path.line_to((f32::from_bits(0x5b2d2968), f32::from_bits(0x212a8c55)));
    path.move_to((f32::from_bits(0x0321081f), f32::from_bits(0x4b7b28c0)));
    path.line_to((f32::from_bits(0x2a8ced7a), f32::from_bits(0x2d081f21)));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x69392d55), f32::from_bits(0x2d5b684b)),
        (f32::from_bits(0x8c5527f0), f32::from_bits(0x212a1f2a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0xed7a6a1f)),
        (f32::from_bits(0x3a214793), f32::from_bits(0x3328282a)),
        f32::from_bits(0x3a8a3adf),
    );
    path.conic_to(
        (f32::from_bits(0x4be80304), f32::from_bits(0xdcdcdc15)),
        (f32::from_bits(0xdcdcdcdc), f32::from_bits(0x71dcdcdc)),
        f32::from_bits(0x6c107164),
    );
    path.conic_to(
        (f32::from_bits(0x6c0f1d6c), f32::from_bits(0x8e406c6e)),
        (f32::from_bits(0x6c6c0200), f32::from_bits(0x6c6ce46c)),
        f32::from_bits(0x6c6c6c6c),
    );
    path.line_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.close();
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.quad_to(
        (f32::from_bits(0x3ab38a28), f32::from_bits(0x3ac22c21)),
        (f32::from_bits(0x6c401057), f32::from_bits(0x6d6d6b64)),
    );
    path.cubic_to(
        (f32::from_bits(0x6d6d6d6d), f32::from_bits(0x6d6d6d6d)),
        (f32::from_bits(0x286d6d6d), f32::from_bits(0x081d2a29)),
        (f32::from_bits(0x6d690321), f32::from_bits(0x6b6b026d)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11270-L11293 (chrome/m156)
fn fuzz763_31(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0xd72a8c55), f32::from_bits(0x61081f2a)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x4793ed7a)),
        (f32::from_bits(0x282a3a21), f32::from_bits(0xdf3a2128)),
        f32::from_bits(0x471ac575),
    );
    path.line_to((f32::from_bits(0x28404040), f32::from_bits(0x552a298a)));
    path.move_to((f32::from_bits(0x212c685b), f32::from_bits(0x21081f2a)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x80ed7a3a)),
        (f32::from_bits(0x2a3a2147), f32::from_bits(0xdf212828)),
        f32::from_bits(0x4f1a3a3a),
    );
    path.line_to((f32::from_bits(0x212c685b), f32::from_bits(0x21081f2a)));
    path.close();
    path.move_to((f32::from_bits(0x212c685b), f32::from_bits(0x21081f2a)));
    path.cubic_to(
        (f32::from_bits(0x3ac2213a), f32::from_bits(0x432a2928)),
        (f32::from_bits(0x96812be6), f32::from_bits(0x272a1d2a)),
        (f32::from_bits(0x3a2a3529), f32::from_bits(0x3b1e2ab0)),
    );
    path.line_to((f32::from_bits(0x212c685b), f32::from_bits(0x21081f2a)));
    path.close();
    path.move_to((f32::from_bits(0x212c685b), f32::from_bits(0x21081f2a)));
    path.cubic_to(
        (f32::from_bits(0xc5272927), f32::from_bits(0x22383b39)),
        (f32::from_bits(0x1051523a), f32::from_bits(0x2927b029)),
        (f32::from_bits(0x685b2d27), f32::from_bits(0x5b2d6855)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L11295-L11340 (chrome/m156)
fn fuzz763_33(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x72c185d5), f32::from_bits(0x72c184e8)));
    path.quad_to(
        (f32::from_bits(0x724341bf), f32::from_bits(0x72433fc4)),
        (f32::from_bits(0x6d757575), f32::from_bits(0x6d6d6d6d)),
    );
    path.cubic_to(
        (f32::from_bits(0x6d18b5e5), f32::from_bits(0x6d6d6d6d)),
        (f32::from_bits(0x6cbe03bd), f32::from_bits(0x6d4b455b)),
        (f32::from_bits(0x6c6c69d8), f32::from_bits(0x6d20df31)),
    );
    path.conic_to(
        (f32::from_bits(0x6c6c8b72), f32::from_bits(0x00000000)),
        (f32::from_bits(0x6c6c6c6c), f32::from_bits(0x00000000)),
        f32::from_bits(0x400812df),
    );
    path.quad_to(
        (f32::from_bits(0x72432acb), f32::from_bits(0x72432295)),
        (f32::from_bits(0x72c185d5), f32::from_bits(0x72c184e8)),
    );
    path.close();
    path.move_to((f32::from_bits(0x72c185d5), f32::from_bits(0x72c184e8)));
    path.cubic_to(
        (f32::from_bits(0x74f97d76), f32::from_bits(0x74f97d90)),
        (f32::from_bits(0x75381628), f32::from_bits(0x7538182c)),
        (f32::from_bits(0x7538153b), f32::from_bits(0x75381835)),
    );
    path.cubic_to(
        (f32::from_bits(0x7538144e), f32::from_bits(0x7538183f)),
        (f32::from_bits(0x74f9760f), f32::from_bits(0x74f97ddd)),
        (f32::from_bits(0x72c185d5), f32::from_bits(0x72c184e8)),
    );
    path.close();
    path.move_to((f32::from_bits(0x6c6c69d8), f32::from_bits(0x6d20df31)));
    path.conic_to(
        (f32::from_bits(0x6c6c55ae), f32::from_bits(0x6d80b520)),
        (f32::from_bits(0x6c6c1071), f32::from_bits(0x6e0f1d6c)),
        f32::from_bits(0x3f96e656),
    );
    path.line_to((f32::from_bits(0x6a674231), f32::from_bits(0x6c0c3394)));
    path.cubic_to(
        (f32::from_bits(0x6b12c63f), f32::from_bits(0x6c881439)),
        (f32::from_bits(0x6bba4ae5), f32::from_bits(0x6ced1e23)),
        (f32::from_bits(0x6c6c69d8), f32::from_bits(0x6d20df31)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x6c6b6ba7), f32::from_bits(0x886b6b6b)));
    path.quad_to(
        (f32::from_bits(0x0000206b), f32::from_bits(0x6d6d6d6d)),
        (f32::from_bits(0x6d6d6d6d), f32::from_bits(0x6d6d6d6d)),
    );
    path.conic_to(
        (f32::from_bits(0x3e3e3e3e), f32::from_bits(0xafbcad20)),
        (f32::from_bits(0x78787878), f32::from_bits(0x78787829)),
        f32::from_bits(0x78787878),
    );
    path.line_to((f32::from_bits(0x78787878), f32::from_bits(0x95066b78)));
    path.line_to((f32::from_bits(0x6c6b6ba7), f32::from_bits(0x886b6b6b)));
    path.quad_to(
        (f32::from_bits(0x0000206b), f32::from_bits(0x6d6d6d6d)),
        (f32::from_bits(0x6d6d6d6d), f32::from_bits(0x6d6d6d6d)),
    );
    path.conic_to(
        (f32::from_bits(0x3e3e3e3e), f32::from_bits(0xafbcad20)),
        (f32::from_bits(0x78787878), f32::from_bits(0x78787829)),
        f32::from_bits(0x78787878),
    );
    path.line_to((f32::from_bits(0x8787878f), f32::from_bits(0x87878787)));
    path.line_to((f32::from_bits(0x78787878), f32::from_bits(0x78787878)));
    path.line_to((f32::from_bits(0x78787878), f32::from_bits(0x78787878)));
    path.line_to((f32::from_bits(0x6c105778), f32::from_bits(0x6d406b64)));
    path.cubic_to(
        (f32::from_bits(0x7575756d), f32::from_bits(0x75757575)),
        (f32::from_bits(0x75757575), f32::from_bits(0x75757575)),
        (f32::from_bits(0x6d6d7575), f32::from_bits(0x6d6d6d6d)),
    );
    path.cubic_to(
        (f32::from_bits(0x6d696d6d), f32::from_bits(0x026d6d6d)),
        (f32::from_bits(0x80bc6b6b), f32::from_bits(0xaebcdfd0)),
        (f32::from_bits(0x7878bcac), f32::from_bits(0x78787878)),
    );
    path.line_to((f32::from_bits(0x78787878), f32::from_bits(0x78787878)));
    path.line_to((f32::from_bits(0x78787878), f32::from_bits(0x78787878)));
    path.line_to((f32::from_bits(0x78787878), f32::from_bits(0x78787878)));
    path.line_to((f32::from_bits(0xb4bcacbc), f32::from_bits(0xbcadbcbc)));
    path.move_to((f32::from_bits(0xa03aacbc), f32::from_bits(0x757575a0)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L11342-L11360 (chrome/m156)
fn fuzz763_32(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0xdedcdcdc), f32::from_bits(0xdcdcdcdc)),
        (f32::from_bits(0xdcdcdcdc), f32::from_bits(0xdcdcdcdc)),
        (f32::from_bits(0x55dcdcdc), f32::from_bits(0x29407d7f)),
    );
    path.cubic_to(
        (f32::from_bits(0x7b93ed4b), f32::from_bits(0x29521472)),
        (f32::from_bits(0xdfc83c28), f32::from_bits(0x1a3a834e)),
        (f32::from_bits(0x6855e84f), f32::from_bits(0xf2f22a80)),
    );
    path.move_to((f32::from_bits(0xe0f2f210), f32::from_bits(0xc3f2eef2)));
    path.cubic_to(
        (f32::from_bits(0x108ced7a), f32::from_bits(0x7bc00308)),
        (f32::from_bits(0x287a6a3a), f32::from_bits(0x242847ed)),
        (f32::from_bits(0x2bcb302a), f32::from_bits(0xf21003e8)),
    );
    path.move_to((f32::from_bits(0x556c0010), f32::from_bits(0x002a8768)));
    path.quad_to(
        (f32::from_bits(0xf2f22021), f32::from_bits(0xf2f2f56e)),
        (f32::from_bits(0xf2f2f2f2), f32::from_bits(0xf22040d9)),
    );
    path.line_to((f32::from_bits(0xc013f2f2), f32::from_bits(0x0000294d)));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11362-L11388 (chrome/m156)
fn fuzz763_34(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x63a95a6c), f32::from_bits(0x6cc8e7e2)));
    path.quad_to(
        (f32::from_bits(0x63690f37), f32::from_bits(0x6d0a3d9b)),
        (f32::from_bits(0x00000000), f32::from_bits(0x6d3e3e3e)),
    );
    path.conic_to(
        (f32::from_bits(0x6b9253fc), f32::from_bits(0x6c956a8b)),
        (f32::from_bits(0x6c6ac798), f32::from_bits(0x692a5d27)),
        f32::from_bits(0x3e56eb72),
    );
    path.line_to((f32::from_bits(0x6c6c586c), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x6c8c6c6c), f32::from_bits(0x00000000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x6cc8e82a)),
        f32::from_bits(0x5b684b68),
    );
    path.line_to((f32::from_bits(0x63a95a6c), f32::from_bits(0x6cc8e7e2)));
    path.close();
    path.move_to((f32::from_bits(0x63a95a6c), f32::from_bits(0x6cc8e7e2)));
    path.quad_to(
        (f32::from_bits(0x641ae35f), f32::from_bits(0x00000000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
    );
    path.line_to((f32::from_bits(0x6c6c586c), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x6c6ba1fc), f32::from_bits(0x688c9eb1)),
        (f32::from_bits(0x6c6ac798), f32::from_bits(0x692a5d27)),
        f32::from_bits(0x3f7fec32),
    );
    path.line_to((f32::from_bits(0x63a95a6c), f32::from_bits(0x6cc8e7e2)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x6c3e3e3e), f32::from_bits(0x586c79ff)));
    path.quad_to(
        (f32::from_bits(0x6c6c4a6c), f32::from_bits(0x6c6c6c6c)),
        (f32::from_bits(0xc83e6c6c), f32::from_bits(0x3e313e3e)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11390-L11442 (chrome/m156)
fn fuzz763_36(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x68556829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a312a), f32::from_bits(0xc0032108)));
    path.cubic_to(
        (f32::from_bits(0x68392d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x2d555b2d),
    );
    path.move_to((f32::from_bits(0xe8355b2d), f32::from_bits(0xf0682955)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x8cef552a)),
        (f32::from_bits(0x295b2d2a), f32::from_bits(0x68210368)),
        f32::from_bits(0x7bc05508),
    );
    path.line_to((f32::from_bits(0xe8355b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0xe8355b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x212a8c55)));
    path.conic_to(
        (f32::from_bits(0x212a081f), f32::from_bits(0x4b7bc003)),
        (f32::from_bits(0x5b2d7a6a), f32::from_bits(0xf0556839)),
        f32::from_bits(0x2a8c555b),
    );
    path.conic_to(
        (f32::from_bits(0xf42a212a), f32::from_bits(0x4b7bc003)),
        (f32::from_bits(0x2aed7a39), f32::from_bits(0x2108c08c)),
        f32::from_bits(0x7b03211f),
    );
    path.line_to((f32::from_bits(0xe8355b2d), f32::from_bits(0xf0682955)));
    path.close();
    path.move_to((f32::from_bits(0xe8355b2d), f32::from_bits(0xf0682955)));
    path.line_to((f32::from_bits(0x6829ed27), f32::from_bits(0x2d555b2d)));
    path.move_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x2a8c555b), f32::from_bits(0x212a1f72)),
        (f32::from_bits(0x03210807), f32::from_bits(0x6a4b7b28)),
        f32::from_bits(0x4793ed7a),
    );
    path.line_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.quad_to(
        (f32::from_bits(0x282a282a), f32::from_bits(0x8a3adf21)),
        (f32::from_bits(0x284f1a3a), f32::from_bits(0x213ab38a)),
    );
    path.line_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.quad_to(
        (f32::from_bits(0x1d2a2928), f32::from_bits(0x43962be6)),
        (f32::from_bits(0x3a20002a), f32::from_bits(0x2a8ced29)),
    );
    path.line_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0xed210830), f32::from_bits(0xc04b6a03)),
        (f32::from_bits(0x68297b27), f32::from_bits(0x55555b2d)),
        f32::from_bits(0x2ab03a2a),
    );
    path.quad_to(
        (f32::from_bits(0x2720213b), f32::from_bits(0x3a3b2729)),
        (f32::from_bits(0xc4341f20), f32::from_bits(0xecc52a22)),
    );
    path.cubic_to(
        (f32::from_bits(0x5921c25d), f32::from_bits(0x29523a70)),
        (f32::from_bits(0x555b2d68), f32::from_bits(0x1f212a8c)),
        (f32::from_bits(0x0321d90a), f32::from_bits(0x5b2d6829)),
    );
    path.line_to((f32::from_bits(0x1f2a2a8c), f32::from_bits(0x03210821)));
    path.line_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x2eed6a7a), f32::from_bits(0x282a3a21)),
        (f32::from_bits(0x3a21df28), f32::from_bits(0x4f1a3a8a)),
        f32::from_bits(0x3ab38a28),
    );
    path.line_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.quad_to(
        (f32::from_bits(0xe61d2a28), f32::from_bits(0x2a43962b)),
        (f32::from_bits(0x29272a81), f32::from_bits(0x2bb02a55)),
    );
    path.quad_to(
        (f32::from_bits(0x2720213b), f32::from_bits(0x3ac52729)),
        (f32::from_bits(0xc4223b32), f32::from_bits(0x6c2a201f)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11444-L11481 (chrome/m156)
fn fuzz763_35(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x2aed2a8c), f32::from_bits(0x03210a1f)));
    path.conic_to(
        (f32::from_bits(0x0000007b), f32::from_bits(0x7474747f)),
        (f32::from_bits(0x74747474), f32::from_bits(0x747474c4)),
        f32::from_bits(0x74747474),
    );
    path.quad_to(
        (f32::from_bits(0x74747474), f32::from_bits(0x74747474)),
        (f32::from_bits(0x20437474), f32::from_bits(0x43a52b02)),
    );
    path.move_to((f32::from_bits(0x3a214781), f32::from_bits(0x2128282a)));
    path.line_to((f32::from_bits(0x4b7bd603), f32::from_bits(0x6cf33b6a)));
    path.conic_to(
        (f32::from_bits(0x35778caa), f32::from_bits(0x0000002a)),
        (f32::from_bits(0x74742164), f32::from_bits(0x2a3a7474)),
        f32::from_bits(0x4cc22157),
    );
    path.cubic_to(
        (f32::from_bits(0x21479321), f32::from_bits(0x23434cc2)),
        (f32::from_bits(0x3a214793), f32::from_bits(0x2128282a)),
        (f32::from_bits(0x323adf81), f32::from_bits(0x77291a3a)),
    );
    path.conic_to(
        (f32::from_bits(0x0000002a), f32::from_bits(0x7474743e)),
        (f32::from_bits(0x74747474), f32::from_bits(0x74746474)),
        f32::from_bits(0x74747474),
    );
    path.cubic_to(
        (f32::from_bits(0x21e7fc06), f32::from_bits(0x2a212a59)),
        (f32::from_bits(0x0321081f), f32::from_bits(0x00002a35)),
        (f32::from_bits(0x74744000), f32::from_bits(0x2974e874)),
    );
    path.cubic_to(
        (f32::from_bits(0x74647474), f32::from_bits(0x74747474)),
        (f32::from_bits(0x12ec7474), f32::from_bits(0x4cc22147)),
        (f32::from_bits(0x47932343), f32::from_bits(0x282a3a21)),
    );
    path.line_to((f32::from_bits(0x3a214781), f32::from_bits(0x2128282a)));
    path.close();
    path.move_to((f32::from_bits(0x3a214781), f32::from_bits(0x2128282a)));
    path.conic_to(
        (f32::from_bits(0x3a323adf), f32::from_bits(0x4977291a)),
        (f32::from_bits(0x0000002a), f32::from_bits(0x7474743e)),
        f32::from_bits(0x74747474),
    );
    path.cubic_to(
        (f32::from_bits(0x74747464), f32::from_bits(0x74747474)),
        (f32::from_bits(0x21e7fc06), f32::from_bits(0x2a212a59)),
        (f32::from_bits(0x0321081f), f32::from_bits(0x00002a35)),
    );
    path.move_to((f32::from_bits(0x74747440), f32::from_bits(0x742974e8)));
    path.cubic_to(
        (f32::from_bits(0x74746474), f32::from_bits(0x74747474)),
        (f32::from_bits(0xd912ec74), f32::from_bits(0x553a3728)),
        (f32::from_bits(0x29202a8c), f32::from_bits(0x5555201b)),
    );
    path.move_to((f32::from_bits(0x31292768), f32::from_bits(0x212d2aff)));
    path.quad_to(
        (f32::from_bits(0x2128282a), f32::from_bits(0x323adf81)),
        (f32::from_bits(0x77291a3a), f32::from_bits(0x00002a49)),
    );
    path.move_to((f32::from_bits(0x7474743e), f32::from_bits(0x74747474)));
    path.cubic_to(
        (f32::from_bits(0x74747464), f32::from_bits(0x74747474)),
        (f32::from_bits(0x21e7fc06), f32::from_bits(0x2a212a59)),
        (f32::from_bits(0x0321081f), f32::from_bits(0x00002a35)),
    );
    path.move_to((f32::from_bits(0x74747440), f32::from_bits(0x74747474)));
    path.cubic_to(
        (f32::from_bits(0x74747464), f32::from_bits(0x74747474)),
        (f32::from_bits(0x43747474), f32::from_bits(0xa52b0220)),
        (f32::from_bits(0x47812a43), f32::from_bits(0x282a3a21)),
    );
    path.line_to((f32::from_bits(0x74747440), f32::from_bits(0x74747474)));
    path.close();
    path.move_to((f32::from_bits(0x74747440), f32::from_bits(0x74747474)));
    path.conic_to(
        (f32::from_bits(0x3a323adf), f32::from_bits(0x19433b1a)),
        (f32::from_bits(0x5921e7fc), f32::from_bits(0x1f2a212a)),
        f32::from_bits(0x35032108),
    );
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L11483-L11535 (chrome/m156)
fn fuzz763_37(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x5568392a), f32::from_bits(0x5b2d3368)));
    path.conic_to(
        (f32::from_bits(0x5b2d555b), f32::from_bits(0x68275b2d)),
        (f32::from_bits(0x21685527), f32::from_bits(0x0321082a)),
        f32::from_bits(0x6ab485c0),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x5b2d2d55),
    );
    path.move_to((f32::from_bits(0x6839552d), f32::from_bits(0xf0683b5b)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x2a8cef2a)),
        (f32::from_bits(0x682d2953), f32::from_bits(0xee682103)),
        f32::from_bits(0x4b7bc055),
    );
    path.line_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x212a8c55)));
    path.conic_to(
        (f32::from_bits(0x4b03213b), f32::from_bits(0xc07b2a08)),
        (f32::from_bits(0x5b2d7a6a), f32::from_bits(0xf0556830)),
        f32::from_bits(0x2a8c555b),
    );
    path.conic_to(
        (f32::from_bits(0x0321212a), f32::from_bits(0x4b7bd2c0)),
        (f32::from_bits(0xed7ac039), f32::from_bits(0x2f218c08)),
        f32::from_bits(0x1f037b2a),
    );
    path.line_to((f32::from_bits(0x6839552d), f32::from_bits(0xf0683b5b)));
    path.close();
    path.move_to((f32::from_bits(0x6839552d), f32::from_bits(0xf0683b5b)));
    path.line_to((f32::from_bits(0x6829ed27), f32::from_bits(0x2d555b2d)));
    path.move_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x721f2a5b), f32::from_bits(0x212a8c55)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7b28)),
        f32::from_bits(0x4793ed7a),
    );
    path.line_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.quad_to(
        (f32::from_bits(0x28282a2a), f32::from_bits(0x2c682921)),
        (f32::from_bits(0x8c555bf6), f32::from_bits(0x6d03de30)),
    );
    path.cubic_to(
        (f32::from_bits(0x68392d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x081f2a21)),
    );
    path.line_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0xdf93ed7a)),
        (f32::from_bits(0x1a3a803a), f32::from_bits(0xb38a294f)),
        f32::from_bits(0x3ac2213a),
    );
    path.line_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0xe62b291d), f32::from_bits(0x2a812a43)),
        (f32::from_bits(0x8ced093a), f32::from_bits(0xb38a5c5c)),
        f32::from_bits(0x3ac2213a),
    );
    path.line_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.line_to((f32::from_bits(0x8ced293a), f32::from_bits(0x5c5c5c5c)));
    path.move_to((f32::from_bits(0x21081f21), f32::from_bits(0x4b7bc003)));
    path.line_to((f32::from_bits(0x2829ed84), f32::from_bits(0x5b2d2d55)));
    path.move_to((f32::from_bits(0x6839552d), f32::from_bits(0xf0683b5a)));
    path.line_to((f32::from_bits(0x682d2952), f32::from_bits(0xee682103)));
    path.line_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x2a3b0355)));
    path.line_to((f32::from_bits(0x6839552d), f32::from_bits(0xf0683b5a)));
    path.close();
    path.move_to((f32::from_bits(0x6839552d), f32::from_bits(0xf0683b5a)));
    path.conic_to(
        (f32::from_bits(0x084b218c), f32::from_bits(0x6ac07b2a)),
        (f32::from_bits(0x395b2d7a), f32::from_bits(0x5bf05568)),
        f32::from_bits(0x1f2a8c55),
    );
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0xbcb4bcac), f32::from_bits(0x000029ff)),
        (f32::from_bits(0x010000bc), f32::from_bits(0x00bcbc00)),
        (f32::from_bits(0xbebcbcbc), f32::from_bits(0xb6aebcae)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L11537-L11598 (chrome/m156)
fn fuzz763_38(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x5b682968), f32::from_bits(0x5b292d11)),
        (f32::from_bits(0x212a8c55), f32::from_bits(0x555b2d2d)),
        f32::from_bits(0x52525268),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.close();
    path.move_to((f32::from_bits(0xa5252600), f32::from_bits(0x52b4adad)));
    path.close();
    path.move_to((f32::from_bits(0xa5252600), f32::from_bits(0x52b4adad)));
    path.quad_to(
        (f32::from_bits(0x72727270), f32::from_bits(0x52525272)),
        (f32::from_bits(0x2ac05252), f32::from_bits(0x727fb721)),
    );
    path.line_to((f32::from_bits(0x73727322), f32::from_bits(0x555b2d29)));
    path.line_to((f32::from_bits(0xab2a212e), f32::from_bits(0x7a27872a)));
    path.move_to((f32::from_bits(0x25fffefb), f32::from_bits(0x7bc00321)));
    path.quad_to(
        (f32::from_bits(0x52524852), f32::from_bits(0x72525228)),
        (f32::from_bits(0x72727272), f32::from_bits(0x3a727272)),
    );
    path.line_to((f32::from_bits(0x25fffefb), f32::from_bits(0x7bc00321)));
    path.close();
    path.move_to((f32::from_bits(0x25fffefb), f32::from_bits(0x7bc00321)));
    path.quad_to(
        (f32::from_bits(0x2a292827), f32::from_bits(0x962b0080)),
        (f32::from_bits(0x5252752a), f32::from_bits(0x72725252)),
    );
    path.quad_to(
        (f32::from_bits(0x72725252), f32::from_bits(0x52525272)),
        (f32::from_bits(0x72525252), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x72727255), f32::from_bits(0xda000072)),
        (f32::from_bits(0x52525ada), f32::from_bits(0x52525252)),
    );
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x52525272)),
        (f32::from_bits(0x72525248), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x72727255), f32::from_bits(0xda007b72)),
        (f32::from_bits(0x52525ada), f32::from_bits(0x52525252)),
    );
    path.quad_to(
        (f32::from_bits(0x86727272), f32::from_bits(0x5252528d)),
        (f32::from_bits(0x72525252), f32::from_bits(0x72727227)),
    );
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x29217272)),
        (f32::from_bits(0xc003211c), f32::from_bits(0x556a4b7b)),
    );
    path.move_to((f32::from_bits(0x72557272), f32::from_bits(0x00727272)));
    path.move_to((f32::from_bits(0x5a61dada), f32::from_bits(0x52525252)));
    path.close();
    path.move_to((f32::from_bits(0x5a61dada), f32::from_bits(0x52525252)));
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x3a727272)),
        (f32::from_bits(0x28273ac2), f32::from_bits(0x00802a29)),
    );
    path.line_to((f32::from_bits(0x52752a96), f32::from_bits(0x72525252)));
    path.quad_to(
        (f32::from_bits(0x72525272), f32::from_bits(0x52527272)),
        (f32::from_bits(0x52525252), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x72725572), f32::from_bits(0x00007272)),
        (f32::from_bits(0x525adada), f32::from_bits(0x52525252)),
    );
    path.line_to((f32::from_bits(0x5a61dada), f32::from_bits(0x52525252)));
    path.close();
    path.move_to((f32::from_bits(0x5a61dada), f32::from_bits(0x52525252)));
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x52525272)),
        (f32::from_bits(0x72525248), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x72727255), f32::from_bits(0xda007b72)),
        (f32::from_bits(0x52525ada), f32::from_bits(0x72525252)),
    );
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x72727252)),
        (f32::from_bits(0xda007b72), f32::from_bits(0x52525ada)),
    );
    path.line_to((f32::from_bits(0x5a61dada), f32::from_bits(0x52525252)));
    path.close();
    path.move_to((f32::from_bits(0x5a61dada), f32::from_bits(0x52525252)));
    path.quad_to(
        (f32::from_bits(0x86727272), f32::from_bits(0x5252528d)),
        (f32::from_bits(0x72525252), f32::from_bits(0x72727227)),
    );
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x29217272)),
        (f32::from_bits(0xc003211c), f32::from_bits(0x556a4b7b)),
    );
    path.move_to((f32::from_bits(0x72557272), f32::from_bits(0x00727272)));
    path.move_to((f32::from_bits(0x525adada), f32::from_bits(0x52525252)));
    path.close();
    path.move_to((f32::from_bits(0xa5252600), f32::from_bits(0x52b4adad)));
    path.close();
    path.move_to((f32::from_bits(0xa5252600), f32::from_bits(0x52b4adad)));
    path.quad_to(
        (f32::from_bits(0x72727270), f32::from_bits(0x52525272)),
        (f32::from_bits(0x72525252), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x72727255), f32::from_bits(0xda007b72)),
        (f32::from_bits(0x52525ada), f32::from_bits(0x52525252)),
    );
    path.quad_to(
        (f32::from_bits(0x52525272), f32::from_bits(0x3b3b0052)),
        (f32::from_bits(0x5b2d553a), f32::from_bits(0x68556829)),
    );
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x52528c55), f32::from_bits(0x29215252)));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11600-L11637 (chrome/m156)
fn fuzz763_41(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x7a057c72), f32::from_bits(0x72727272)),
        (f32::from_bits(0x725b5e72), f32::from_bits(0x055f0089)),
    );
    path.quad_to(
        (f32::from_bits(0x00057272), f32::from_bits(0x72ff0000)),
        (f32::from_bits(0xba405e72), f32::from_bits(0x031b0074)),
    );
    path.line_to((f32::from_bits(0x664af700), f32::from_bits(0x56397d39)));
    path.quad_to(
        (f32::from_bits(0x7a057273), f32::from_bits(0x057300e4)),
        (f32::from_bits(0x257c0c9f), f32::from_bits(0x72400006)),
    );
    path.quad_to(
        (f32::from_bits(0xba5b5e72), f32::from_bits(0x030000ff)),
        (f32::from_bits(0x74ba00e8), f32::from_bits(0xe8ec4000)),
    );
    path.move_to((f32::from_bits(0x39724aff), f32::from_bits(0x7200397d)));
    path.quad_to(
        (f32::from_bits(0x827a0572), f32::from_bits(0x08727272)),
        (f32::from_bits(0x08080808), f32::from_bits(0x08080808)),
    );
    path.line_to((f32::from_bits(0x08080808), f32::from_bits(0x08080808)));
    path.line_to((f32::from_bits(0x08080808), f32::from_bits(0x08080808)));
    path.conic_to(
        (f32::from_bits(0x72728c08), f32::from_bits(0x5b5e7272)),
        (f32::from_bits(0x000074ba), f32::from_bits(0x03f8e300)),
        f32::from_bits(0x5aff00e8),
    );
    path.quad_to(
        (f32::from_bits(0x00800039), f32::from_bits(0x72100039)),
        (f32::from_bits(0x727a0572), f32::from_bits(0x7a727272)),
    );
    path.line_to((f32::from_bits(0x7272727a), f32::from_bits(0xdb5e6472)));
    path.move_to((f32::from_bits(0x440039fc), f32::from_bits(0x0000f647)));
    path.line_to((f32::from_bits(0x666d0100), f32::from_bits(0x726efe62)));
    path.line_to((f32::from_bits(0x440039fc), f32::from_bits(0x0000f647)));
    path.close();
    path.move_to((f32::from_bits(0x440039fc), f32::from_bits(0x0000f647)));
    path.conic_to(
        (f32::from_bits(0x72727272), f32::from_bits(0xf3db5e64)),
        (f32::from_bits(0x475afc16), f32::from_bits(0x170100ad)),
        f32::from_bits(0x01008000),
    );
    path.quad_to(
        (f32::from_bits(0x72057272), f32::from_bits(0x8c7a3472)),
        (f32::from_bits(0x72727272), f32::from_bits(0x00f6475e)),
    );
    path.move_to((f32::from_bits(0x6d106d43), f32::from_bits(0x6efe6266)));
    path.quad_to(
        (f32::from_bits(0x72727a05), f32::from_bits(0xba5b7272)),
        (f32::from_bits(0x03000074), f32::from_bits(0x5aff00e8)),
    );
    path.quad_to(
        (f32::from_bits(0x00da0039), f32::from_bits(0x72100039)),
        (f32::from_bits(0x727a0572), f32::from_bits(0x7a727272)),
    );
    path.line_to((f32::from_bits(0x7272727a), f32::from_bits(0xdb5e6472)));
    path.line_to((f32::from_bits(0xfc5b97fc), f32::from_bits(0x47440039)));
    path.line_to((f32::from_bits(0x00710000), f32::from_bits(0x62766d01)));
    path.quad_to(
        (f32::from_bits(0x7a05726e), f32::from_bits(0x72727272)),
        (f32::from_bits(0xf3db5e64), f32::from_bits(0x4a5afc16)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L11639-L11664 (chrome/m156)
fn fuzz763_40(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x10190004), f32::from_bits(0x7272727a)));
    path.quad_to(
        (f32::from_bits(0xf3db5e64), f32::from_bits(0x5b97fc16)),
        (f32::from_bits(0x000039fc), f32::from_bits(0x01008000)),
    );
    path.quad_to(
        (f32::from_bits(0x7a057272), f32::from_bits(0x72727272)),
        (f32::from_bits(0x725b5e72), f32::from_bits(0x41720089)),
    );
    path.line_to((f32::from_bits(0x63636363), f32::from_bits(0x63606363)));
    path.line_to((f32::from_bits(0x01000000), f32::from_bits(0x10010004)));
    path.conic_to(
        (f32::from_bits(0x72727272), f32::from_bits(0xf3db5e64)),
        (f32::from_bits(0x4a5afc16), f32::from_bits(0x0000d07d)),
        f32::from_bits(0x01008000),
    );
    path.quad_to(
        (f32::from_bits(0x7a057272), f32::from_bits(0x72727272)),
        (f32::from_bits(0x725b5e72), f32::from_bits(0x63720089)),
    );
    path.line_to((f32::from_bits(0x63636363), f32::from_bits(0x63606363)));
    path.line_to((f32::from_bits(0x72000000), f32::from_bits(0x5b5e72b4)));
    path.quad_to(
        (f32::from_bits(0x05720089), f32::from_bits(0x05727272)),
        (f32::from_bits(0x7272727a), f32::from_bits(0x5b5e7272)),
    );
    path.cubic_to(
        (f32::from_bits(0x03000074), f32::from_bits(0x4aff00e8)),
        (f32::from_bits(0x397d3972), f32::from_bits(0x01727200)),
        (f32::from_bits(0x72727a00), f32::from_bits(0x5e8d7272)),
    );
    path.move_to((f32::from_bits(0x72008972), f32::from_bits(0x458fe705)));
    path.quad_to(
        (f32::from_bits(0x7a057272), f32::from_bits(0xe8727272)),
        (f32::from_bits(0xba5b5e03), f32::from_bits(0x03000074)),
    );
    path.line_to((f32::from_bits(0xf3dbff00), f32::from_bits(0x00397d16)));
    path.cubic_to(
        (f32::from_bits(0x7a101900), f32::from_bits(0x72727272)),
        (f32::from_bits(0xf3db5e64), f32::from_bits(0x0197fc16)),
        (f32::from_bits(0x200c2010), f32::from_bits(0x20203620)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11666-L11687 (chrome/m156)
fn fuzz763_39(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x7a057c72), f32::from_bits(0x72727272)),
        (f32::from_bits(0x725b5e72), f32::from_bits(0x055f0089)),
    );
    path.quad_to(
        (f32::from_bits(0x7a057272), f32::from_bits(0x72727272)),
        (f32::from_bits(0xba405e72), f32::from_bits(0x03000074)),
    );
    path.line_to((f32::from_bits(0x664aff00), f32::from_bits(0x56397d39)));
    path.quad_to(
        (f32::from_bits(0x7a057273), f32::from_bits(0x057300ff)),
        (f32::from_bits(0x257c0c9f), f32::from_bits(0x72787257)),
    );
    path.quad_to(
        (f32::from_bits(0xba5b5e72), f32::from_bits(0x03000093)),
        (f32::from_bits(0x74ba00e8), f32::from_bits(0xe8ecff00)),
    );
    path.move_to((f32::from_bits(0x39724aff), f32::from_bits(0x7200397d)));
    path.quad_to(
        (f32::from_bits(0x827a0572), f32::from_bits(0x72727272)),
        (f32::from_bits(0x724adf00), f32::from_bits(0x00397d39)),
    );
    path.quad_to(
        (f32::from_bits(0x7a057272), f32::from_bits(0x16f3abab)),
        (f32::from_bits(0xfc5b97fc), f32::from_bits(0x47440039)),
    );
    path.line_to((f32::from_bits(0x00710000), f32::from_bits(0x62767201)));
    path.quad_to(
        (f32::from_bits(0x7a05726e), f32::from_bits(0x72727272)),
        (f32::from_bits(0xf3db5e64), f32::from_bits(0x4a5afc16)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L11690-L11719 (chrome/m156)
fn fuzz763_42(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x7a057272), f32::from_bits(0x72727272)),
        (f32::from_bits(0x725b5e72), f32::from_bits(0x05720089)),
    );
    path.quad_to(
        (f32::from_bits(0x7a057272), f32::from_bits(0x72727272)),
        (f32::from_bits(0xba405e72), f32::from_bits(0x03000074)),
    );
    path.line_to((f32::from_bits(0x724aff00), f32::from_bits(0x56397d39)));
    path.quad_to(
        (f32::from_bits(0x7a057272), f32::from_bits(0xfa8d00ff)),
        (f32::from_bits(0x25727272), f32::from_bits(0x7272727a)),
    );
    path.quad_to(
        (f32::from_bits(0xba5b5e72), f32::from_bits(0x03000093)),
        (f32::from_bits(0x74ba00e8), f32::from_bits(0xe8e0ff00)),
    );
    path.conic_to(
        (f32::from_bits(0x39724aff), f32::from_bits(0x7200397d)),
        (f32::from_bits(0x7a057272), f32::from_bits(0x72727272)),
        f32::from_bits(0x4aff0072),
    );
    path.quad_to(
        (f32::from_bits(0x00397d39), f32::from_bits(0x05727272)),
        (f32::from_bits(0x7272727a), f32::from_bits(0x385e7272)),
    );
    path.quad_to(
        (f32::from_bits(0x057200ff), f32::from_bits(0x25727272)),
        (f32::from_bits(0x7272727a), f32::from_bits(0x5b5e7272)),
    );
    path.cubic_to(
        (f32::from_bits(0x03000074), f32::from_bits(0x4aff00e8)),
        (f32::from_bits(0x397d3972), f32::from_bits(0x01000400)),
        (f32::from_bits(0x72727a10), f32::from_bits(0x5e647272)),
    );
    path.quad_to(
        (f32::from_bits(0x2b2d16f3), f32::from_bits(0x0039fc4d)),
        (f32::from_bits(0x68800000), f32::from_bits(0x0100fafa)),
    );
    path.quad_to(
        (f32::from_bits(0x7a057272), f32::from_bits(0x72727272)),
        (f32::from_bits(0x725b5e72), f32::from_bits(0x63720089)),
    );
    path.line_to((f32::from_bits(0x63636363), f32::from_bits(0x63606363)));
    path.line_to((f32::from_bits(0x72720000), f32::from_bits(0xff725b5e)));
    path.move_to((f32::from_bits(0x72720572), f32::from_bits(0x5b5e2572)));
    path.quad_to(
        (f32::from_bits(0x05720089), f32::from_bits(0x25727272)),
        (f32::from_bits(0x72728c7a), f32::from_bits(0x5b5e7272)),
    );
    path.cubic_to(
        (f32::from_bits(0x03000074), f32::from_bits(0x4aff00e8)),
        (f32::from_bits(0x397d3972), f32::from_bits(0x01000400)),
        (f32::from_bits(0x72727a10), f32::from_bits(0x5e827272)),
    );
    path.quad_to(
        (f32::from_bits(0x97fc16f3), f32::from_bits(0x0039fc5b)),
        (f32::from_bits(0x00f6472e), f32::from_bits(0x01008000)),
    );
    path.quad_to(
        (f32::from_bits(0x7a057272), f32::from_bits(0x72727272)),
        (f32::from_bits(0xf3db5e64), f32::from_bits(0x4a5afc16)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L11721-L11754 (chrome/m156)
fn fuzz763_43(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x5c386c3a), f32::from_bits(0x4e691a3e)));
    path.cubic_to(
        (f32::from_bits(0x6f69f9f5), f32::from_bits(0x18ff8791)),
        (f32::from_bits(0x2492263c), f32::from_bits(0xbc6fdb48)),
        (f32::from_bits(0xc2f82107), f32::from_bits(0x729a18e1)),
    );
    path.cubic_to(
        (f32::from_bits(0x07d729d1), f32::from_bits(0xdea6db48)),
        (f32::from_bits(0xcd1dfb88), f32::from_bits(0x90826769)),
        (f32::from_bits(0x1c20e5a4), f32::from_bits(0xa4c3ba9b)),
    );
    path.move_to((f32::from_bits(0xcc2084b7), f32::from_bits(0x19f68bdb)));
    path.close();
    path.move_to((f32::from_bits(0xcc2084b7), f32::from_bits(0x19f68bdb)));
    path.cubic_to(
        (f32::from_bits(0xdeea1d6e), f32::from_bits(0xc7774804)),
        (f32::from_bits(0x27cf0dcf), f32::from_bits(0x6ae8b99f)),
        (f32::from_bits(0x24ac3260), f32::from_bits(0x062fa93c)),
    );
    path.line_to((f32::from_bits(0x438a0b9c), f32::from_bits(0x60a1d2c8)));
    path.quad_to(
        (f32::from_bits(0xe13fb902), f32::from_bits(0x07ee536f)),
        (f32::from_bits(0x971d8ac1), f32::from_bits(0x2f9f174b)),
    );
    path.line_to((f32::from_bits(0x0f2cf5d8), f32::from_bits(0xe271654c)));
    path.line_to((f32::from_bits(0xe6cf24d2), f32::from_bits(0xd9537742)));
    path.cubic_to(
        (f32::from_bits(0x1aaaee04), f32::from_bits(0x9e3b804c)),
        (f32::from_bits(0x84cba87d), f32::from_bits(0x4e0e8ccc)),
        (f32::from_bits(0x2aec611a), f32::from_bits(0x7ae4b639)),
    );
    path.conic_to(
        (f32::from_bits(0x73357921), f32::from_bits(0x6f163021)),
        (f32::from_bits(0x70ea542c), f32::from_bits(0xe008f404)),
        f32::from_bits(0x1f6c5e52),
    );
    path.line_to((f32::from_bits(0xda45ad4e), f32::from_bits(0xedce4a04)));
    path.line_to((f32::from_bits(0xac0e45da), f32::from_bits(0x8f632841)));
    path.line_to((f32::from_bits(0xcc2084b7), f32::from_bits(0x19f68bdb)));
    path.close();
    path.move_to((f32::from_bits(0xcc2084b7), f32::from_bits(0x19f68bdb)));
    path.quad_to(
        (f32::from_bits(0xf35c4ad5), f32::from_bits(0x0692f251)),
        (f32::from_bits(0x69632126), f32::from_bits(0xb927af67)),
    );
    path.move_to((f32::from_bits(0x6534bff9), f32::from_bits(0x434a9986)));
    path.quad_to(
        (f32::from_bits(0x37c603e5), f32::from_bits(0xa0683953)),
        (f32::from_bits(0x751915e4), f32::from_bits(0x831c911a)),
    );
    path.cubic_to(
        (f32::from_bits(0xba4f10f1), f32::from_bits(0x5a7571df)),
        (f32::from_bits(0x4ec67459), f32::from_bits(0x33c58827)),
        (f32::from_bits(0x10b78ccb), f32::from_bits(0xedbd2748)),
    );
    path.cubic_to(
        (f32::from_bits(0x6d06f06a), f32::from_bits(0xe30465cf)),
        (f32::from_bits(0xc5458fe7), f32::from_bits(0xca488dc4)),
        (f32::from_bits(0x38f9021c), f32::from_bits(0x3e8d58db)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L11756-L11775 (chrome/m156)
fn fuzz763_44(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x7c223bab), f32::from_bits(0x7cf35966)));
    path.quad_to(
        (f32::from_bits(0x00000000), f32::from_bits(0x7ccaca6d)),
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
    );
    path.line_to((f32::from_bits(0x7d7d7d7d), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x7ccacab0), f32::from_bits(0x7d1817f4)),
        (f32::from_bits(0x7c223bab), f32::from_bits(0x7cf35966)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x109d0000), f32::from_bits(0xff7bc000)));
    path.conic_to(
        (f32::from_bits(0x979797ed), f32::from_bits(0x3a214797)),
        (f32::from_bits(0x28aa217a), f32::from_bits(0x01007272)),
        f32::from_bits(0x00000072),
    );
    path.quad_to(
        (f32::from_bits(0x72728302), f32::from_bits(0x8b727272)),
        (f32::from_bits(0x72727272), f32::from_bits(0xc00308f6)),
    );
    path.conic_to(
        (f32::from_bits(0x7f52753a), f32::from_bits(0x8072ffff)),
        (f32::from_bits(0x67af2103), f32::from_bits(0x7d2a6847)),
        f32::from_bits(0x7d7d7d7d),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11777-L11810 (chrome/m156)
fn fuzz763_45(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x30303030), f32::from_bits(0x7a303030)),
        (f32::from_bits(0x7a303030), f32::from_bits(0x30303030)),
    );
    path.conic_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x74303030)),
        (f32::from_bits(0x74303030), f32::from_bits(0x30303030)),
        f32::from_bits(0x74303030),
    );
    path.conic_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        f32::from_bits(0x30303030),
    );
    path.move_to((f32::from_bits(0x30303030), f32::from_bits(0x30303030)));
    path.move_to((f32::from_bits(0x77773030), f32::from_bits(0x30303030)));
    path.conic_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x7a743030), f32::from_bits(0x74303030)),
        f32::from_bits(0x30303030),
    );
    path.line_to((f32::from_bits(0x77773030), f32::from_bits(0x30303030)));
    path.close();
    path.move_to((f32::from_bits(0x77773030), f32::from_bits(0x30303030)));
    path.line_to((f32::from_bits(0x7f303030), f32::from_bits(0x7a303030)));
    path.conic_to(
        (f32::from_bits(0x77303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x30303030), f32::from_bits(0xf9303030)),
        f32::from_bits(0x7a303030),
    );
    path.conic_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        f32::from_bits(0x30303030),
    );
    path.quad_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
    );
    path.quad_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
    );
    path.conic_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        f32::from_bits(0x30303030),
    );
    path.conic_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x30303030), f32::from_bits(0x7a303030)),
        f32::from_bits(0x30303030),
    );
    path.cubic_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x7a303030), f32::from_bits(0x30303030)),
    );
    path.conic_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        f32::from_bits(0x30303030),
    );
    path.move_to((f32::from_bits(0x77303030), f32::from_bits(0xff303030)));
    path.conic_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x7f773030), f32::from_bits(0x7a7a3030)),
        f32::from_bits(0x7a303030),
    );
    path.quad_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x77303030), f32::from_bits(0x30303030)),
    );
    path.conic_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x7b303030), f32::from_bits(0x73303030)),
        f32::from_bits(0x30303030),
    );
    path.quad_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x30303030)),
        (f32::from_bits(0x30303030), f32::from_bits(0x7a7a3030)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11812-L11825 (chrome/m156)
fn fuzz763_46(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x44444444), f32::from_bits(0x44444444)),
        (f32::from_bits(0x44263030), f32::from_bits(0x44304430)),
        f32::from_bits(0x4c444430),
    );
    path.move_to((f32::from_bits(0x44444444), f32::from_bits(0x44444444)));
    path.cubic_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x44444444)),
        (f32::from_bits(0x30303030), f32::from_bits(0x44444444)),
        (f32::from_bits(0x44444444), f32::from_bits(0x4444444c)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11827-L11850 (chrome/m156)
fn fuzz763_47(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0x7272728e), f32::from_bits(0x52527272)),
        (f32::from_bits(0x2d555252), f32::from_bits(0x68556829)),
        (f32::from_bits(0x555b2d29), f32::from_bits(0x2a212a8c)),
    );
    path.conic_to(
        (f32::from_bits(0x00296808), f32::from_bits(0x00000002)),
        (f32::from_bits(0x52525252), f32::from_bits(0x72007272)),
        f32::from_bits(0x52527272),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x2a212a8c), f32::from_bits(0x7272081f)));
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x5974fa80)),
        (f32::from_bits(0x00747474), f32::from_bits(0x59585264)),
    );
    path.cubic_to(
        (f32::from_bits(0x64007474), f32::from_bits(0x088c5852)),
        (f32::from_bits(0x80808021), f32::from_bits(0x8c808080)),
        (f32::from_bits(0x80802108), f32::from_bits(0x80808080)),
    );
    path.quad_to(
        (f32::from_bits(0x80807d80), f32::from_bits(0x80808080)),
        (f32::from_bits(0xff7f0000), f32::from_bits(0x80808080)),
    );
    path.quad_to(
        (f32::from_bits(0x80808080), f32::from_bits(0x80808080)),
        (f32::from_bits(0xed842b00), f32::from_bits(0x7252ff6d)),
    );
    path.quad_to(
        (f32::from_bits(0x72577200), f32::from_bits(0x55525352)),
        (f32::from_bits(0x2a212a8c), f32::from_bits(0x7272081f)),
    );
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x6f740080)),
        (f32::from_bits(0x8c556874), f32::from_bits(0x2982ffff)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11852-L11888 (chrome/m156)
fn fuzz763_48(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0xed0081bc), f32::from_bits(0x1b2d8040)));
    path.move_to((f32::from_bits(0x74747403), f32::from_bits(0x29747474)));
    path.close();
    path.move_to((f32::from_bits(0x74747403), f32::from_bits(0x29747474)));
    path.conic_to(
        (f32::from_bits(0x662d5576), f32::from_bits(0x2d804066)),
        (f32::from_bits(0x8068291b), f32::from_bits(0x740315ff)),
        f32::from_bits(0x74747474),
    );
    path.cubic_to(
        (f32::from_bits(0x762d0529), f32::from_bits(0x72525252)),
        (f32::from_bits(0x007b7272), f32::from_bits(0x525adada)),
        (f32::from_bits(0x52525252), f32::from_bits(0x52727252)),
    );
    path.line_to((f32::from_bits(0x74747403), f32::from_bits(0x29747474)));
    path.close();
    path.move_to((f32::from_bits(0xa5252620), f32::from_bits(0x52b4adad)));
    path.close();
    path.move_to((f32::from_bits(0xa5252620), f32::from_bits(0x52b4adad)));
    path.quad_to(
        (f32::from_bits(0x72727270), f32::from_bits(0x52524872)),
        (f32::from_bits(0x72525252), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x72727255), f32::from_bits(0x80406666)),
        (f32::from_bits(0x68291b2d), f32::from_bits(0x0315ff80)),
    );
    path.cubic_to(
        (f32::from_bits(0x74747474), f32::from_bits(0x7b722974)),
        (f32::from_bits(0x5adada00), f32::from_bits(0x52525252)),
        (f32::from_bits(0x72720052), f32::from_bits(0x72727272)),
    );
    path.line_to((f32::from_bits(0xa5252620), f32::from_bits(0x52b4adad)));
    path.close();
    path.move_to((f32::from_bits(0xa5252620), f32::from_bits(0x52b4adad)));
    path.quad_to(
        (f32::from_bits(0x72727227), f32::from_bits(0x72727272)),
        (f32::from_bits(0x74727272), f32::from_bits(0x55747421)),
    );
    path.line_to((f32::from_bits(0xa5252620), f32::from_bits(0x52b4adad)));
    path.close();
    path.move_to((f32::from_bits(0x724b0000), f32::from_bits(0x00725f72)));
    path.line_to((f32::from_bits(0x52525252), f32::from_bits(0x72725252)));
    path.quad_to(
        (f32::from_bits(0x26727272), f32::from_bits(0x0303a525)),
        (f32::from_bits(0x52005c03), f32::from_bits(0x72525252)),
    );
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x1ff07255)),
        (f32::from_bits(0x2a8c5572), f32::from_bits(0x21082a21)),
    );
    path.line_to((f32::from_bits(0x2a2a3a21), f32::from_bits(0x29212828)));
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L11890-L11921 (chrome/m156)
fn fuzz763_49(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x78303030)),
        (f32::from_bits(0x78787881), f32::from_bits(0x78787878)),
        f32::from_bits(0x30303030),
    );
    path.line_to((f32::from_bits(0x78787878), f32::from_bits(0x78787878)));
    path.line_to((f32::from_bits(0x78787878), f32::from_bits(0x78787878)));
    path.line_to((f32::from_bits(0x78787878), f32::from_bits(0x78787878)));
    path.quad_to(
        (f32::from_bits(0x30303030), f32::from_bits(0x78787878)),
        (f32::from_bits(0x78787878), f32::from_bits(0x78787878)),
    );
    path.line_to((f32::from_bits(0x30303030), f32::from_bits(0x30303030)));
    path.line_to((f32::from_bits(0x30303030), f32::from_bits(0x30303030)));
    path.line_to((f32::from_bits(0x30303030), f32::from_bits(0x30303030)));
    path.line_to((f32::from_bits(0x30303030), f32::from_bits(0x30303030)));
    path.line_to((f32::from_bits(0x30303030), f32::from_bits(0x30303030)));
    path.line_to((f32::from_bits(0x30303030), f32::from_bits(0x30303030)));
    path.line_to((f32::from_bits(0x30303030), f32::from_bits(0x30303030)));
    path.line_to((f32::from_bits(0x30303030), f32::from_bits(0x30303030)));
    path.line_to((f32::from_bits(0x30303030), f32::from_bits(0x30303030)));
    path.line_to((f32::from_bits(0x30303030), f32::from_bits(0x30303030)));
    path.line_to((f32::from_bits(0x30303030), f32::from_bits(0x30303030)));
    path.line_to((f32::from_bits(0x30303030), f32::from_bits(0x30303030)));
    path.line_to((f32::from_bits(0x30303030), f32::from_bits(0x30303030)));
    path.line_to((f32::from_bits(0x78787878), f32::from_bits(0x7878788d)));
    path.line_to((f32::from_bits(0x78787878), f32::from_bits(0x30303030)));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11923-L11942 (chrome/m156)
fn fuzz763_50(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x70621ede), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x74fc5b97)),
        f32::from_bits(0x7d458fe4),
    );
    path.line_to((f32::from_bits(0xefea1ffe), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x70621ede), f32::from_bits(0x00000000)));
    path.close();
    path.move_to((f32::from_bits(0xefea1ffe), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0xefea1ffe), f32::from_bits(0x00000000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11944-L11958 (chrome/m156)
fn fuzz763_51(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x868b5aae), f32::from_bits(0x626c45ab)),
        (f32::from_bits(0xefea1ffe), f32::from_bits(0x0029fc76)),
    );
    path.move_to((f32::from_bits(0xfacbff01), f32::from_bits(0x56fc5b97)));
    path.cubic_to(
        (f32::from_bits(0x7d4559c9), f32::from_bits(0xad801c39)),
        (f32::from_bits(0xfbe2091a), f32::from_bits(0x7268e394)),
        (f32::from_bits(0x7c800079), f32::from_bits(0xa1d75590)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11960-L11987 (chrome/m156)
fn fuzz763_52(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x29ff4bae), f32::from_bits(0xa1d75590)),
        (f32::from_bits(0x9fd6f6c3), f32::from_bits(0x70621ede)),
    );
    path.quad_to(
        (f32::from_bits(0x57a839d3), f32::from_bits(0x1a80d34b)),
        (f32::from_bits(0x0147a31b), f32::from_bits(0xff7fffff)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.close();
    path.move_to((f32::from_bits(0x1ab8e97c), f32::from_bits(0x94fbe3ef)));
    path.conic_to(
        (f32::from_bits(0x75757568), f32::from_bits(0x7575755e)),
        (f32::from_bits(0x75757575), f32::from_bits(0x75757575)),
        f32::from_bits(0x75756575),
    );
    path.line_to((f32::from_bits(0x1ab8e97c), f32::from_bits(0x94fbe3ef)));
    path.close();
    path.move_to((f32::from_bits(0x1ab8e97c), f32::from_bits(0x94fbe3ef)));
    path.conic_to(
        (f32::from_bits(0x75757575), f32::from_bits(0x75757575)),
        (f32::from_bits(0x75757575), f32::from_bits(0x75917575)),
        f32::from_bits(0x75757575),
    );
    path.line_to((f32::from_bits(0x1ab8e97c), f32::from_bits(0x94fbe3ef)));
    path.close();
    path.move_to((f32::from_bits(0x1ab8e97c), f32::from_bits(0x94fbe3ef)));
    path.conic_to(
        (f32::from_bits(0x75757575), f32::from_bits(0x7575758f)),
        (f32::from_bits(0x7f757575), f32::from_bits(0x75757575)),
        f32::from_bits(0x75757575),
    );
    path.line_to((f32::from_bits(0x1ab8e97c), f32::from_bits(0x94fbe3ef)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L11989-L12016 (chrome/m156)
fn fuzz763_53(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x7644b829), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x74fc5b97), f32::from_bits(0x77df944a)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xf8fbe3ff)));
    path.line_to((f32::from_bits(0x7644b829), f32::from_bits(0x00000000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x45ab86ae), f32::from_bits(0xd6d6626c)),
        (f32::from_bits(0xd6d6d6d6), f32::from_bits(0x7644d6d6)),
    );
    path.move_to((f32::from_bits(0xd6d6d6d6), f32::from_bits(0xd6d6d6d6)));
    path.cubic_to(
        (f32::from_bits(0xd6d6d6d6), f32::from_bits(0x64fed6d6)),
        (f32::from_bits(0x7644ef40), f32::from_bits(0x290877fc)),
        (f32::from_bits(0x447644b8), f32::from_bits(0x80fafc76)),
    );
    path.conic_to(
        (f32::from_bits(0x87808080), f32::from_bits(0x764400ae)),
        (f32::from_bits(0x764400fc), f32::from_bits(0x450080fc)),
        f32::from_bits(0x3636366c),
    );
    path.line_to((f32::from_bits(0xd6d6d6d6), f32::from_bits(0xd6d6d6d6)));
    path.close();
    path.move_to((f32::from_bits(0xef08a412), f32::from_bits(0x5aaeff7f)));
    path.conic_to(
        (f32::from_bits(0x7644626c), f32::from_bits(0x088912fc)),
        (f32::from_bits(0xae8744ef), f32::from_bits(0x76571f5a)),
        f32::from_bits(0x45ab86fc),
    );
    path.conic_to(
        (f32::from_bits(0x4064fe62), f32::from_bits(0x290877ef)),
        (f32::from_bits(0x780080b8), f32::from_bits(0x553c7644)),
        f32::from_bits(0x644eae87),
    );
    path.line_to((f32::from_bits(0xef08a412), f32::from_bits(0x5aaeff7f)));
    path.close();
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L12019-L12058 (chrome/m156)
fn fuzz763_54(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x5b682968), f32::from_bits(0xb3b32d11)),
        (f32::from_bits(0xb3b3b3b3), f32::from_bits(0x5b29b3b3)),
        f32::from_bits(0x212a8c55),
    );
    path.conic_to(
        (f32::from_bits(0x68555b2d), f32::from_bits(0x28296869)),
        (f32::from_bits(0x5b252a08), f32::from_bits(0x5d68392a)),
        f32::from_bits(0x29282780),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0x52727272), f32::from_bits(0x72727252)),
        (f32::from_bits(0x525252c7), f32::from_bits(0x72725252)),
        (f32::from_bits(0x72727272), f32::from_bits(0x72727255)),
    );
    path.quad_to(
        (f32::from_bits(0xd7da0000), f32::from_bits(0x5252525a)),
        (f32::from_bits(0x72525252), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x48525252), f32::from_bits(0x72725252)),
        (f32::from_bits(0x72727272), f32::from_bits(0x72727255)),
    );
    path.quad_to(
        (f32::from_bits(0xdada007b), f32::from_bits(0x5252525a)),
        (f32::from_bits(0x72675252), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x52525252), f32::from_bits(0x27725252)),
        (f32::from_bits(0x72727272), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x1c292172), f32::from_bits(0x7bc00321)),
        (f32::from_bits(0x9aaaaaaa), f32::from_bits(0x8c556a4b)),
    );
    path.quad_to(
        (f32::from_bits(0x72725572), f32::from_bits(0x00007272)),
        (f32::from_bits(0x525adada), f32::from_bits(0x52525252)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.close();
    path.move_to((f32::from_bits(0xa5252600), f32::from_bits(0x52b4adad)));
    path.close();
    path.move_to((f32::from_bits(0xa5252600), f32::from_bits(0x52b4adad)));
    path.quad_to(
        (f32::from_bits(0x72725570), f32::from_bits(0x52525272)),
        (f32::from_bits(0x72525252), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x72727255), f32::from_bits(0x555bb672)),
        (f32::from_bits(0x29686968), f32::from_bits(0x252a081f)),
    );
    path.move_to((f32::from_bits(0x5d68392a), f32::from_bits(0x01002780)));
    path.move_to((f32::from_bits(0x72727200), f32::from_bits(0x72725252)));
    path.quad_to(
        (f32::from_bits(0x5adada00), f32::from_bits(0xa5252652)),
        (f32::from_bits(0x727272ad), f32::from_bits(0xda007b72)),
    );
    path.line_to((f32::from_bits(0x5252525a), f32::from_bits(0x72525252)));
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x52525252)),
        (f32::from_bits(0x27725252), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x74217472)),
        (f32::from_bits(0x005b5574), f32::from_bits(0x72680000)),
    );
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x52525252)),
        (f32::from_bits(0x007b7272), f32::from_bits(0x525adada)),
    );
    path.line_to((f32::from_bits(0x72727200), f32::from_bits(0x72725252)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    let path2 = path.detach();
    test_path_op_fuzz(
        reporter,
        &path1,
        &path2,
        PathOp::ReverseDifference,
        filename,
    );
}

// Port of: tests/PathOpsOpTest.cpp#L12062-L12089 (chrome/m156)
fn fuzz763_55(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x55415500)));
    path.line_to((f32::from_bits(0x55555568), f32::from_bits(0x55555555)));
    path.line_to((f32::from_bits(0x98989898), f32::from_bits(0x55989898)));
    path.line_to((f32::from_bits(0xf6f65555), f32::from_bits(0x101006f6)));
    path.quad_to(
        (f32::from_bits(0xdca33f10), f32::from_bits(0xf6f6f6f6)),
        (f32::from_bits(0xf621f6f6), f32::from_bits(0xf70ff6f6)),
    );
    path.line_to((f32::from_bits(0x9400f6f6), f32::from_bits(0x10530000)));
    path.quad_to(
        (f32::from_bits(0x0f101010), f32::from_bits(0x00101010)),
        (f32::from_bits(0xf610f720), f32::from_bits(0xf6f6f6f6)),
    );
    path.line_to((f32::from_bits(0x105352f6), f32::from_bits(0x1cf6ff10)));
    path.line_to((f32::from_bits(0xf6f6220a), f32::from_bits(0x003700f6)));
    path.cubic_to(
        (f32::from_bits(0x0000001e), f32::from_bits(0x00fff4f6)),
        (f32::from_bits(0xff101064), f32::from_bits(0xf6b6ac7f)),
        (f32::from_bits(0xf6f629f6), f32::from_bits(0x10f6f6f6)),
    );
    path.quad_to(
        (f32::from_bits(0x10101007), f32::from_bits(0x10f7fd10)),
        (f32::from_bits(0xf6f6f6f6), f32::from_bits(0xf6f645e0)),
    );
    path.line_to((f32::from_bits(0xed9ef6f6), f32::from_bits(0x53535353)));
    path.line_to((f32::from_bits(0x53006cf6), f32::from_bits(0x53295353)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x55415500)));
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x55415500)));
    path.line_to((f32::from_bits(0xf6f6f6f6), f32::from_bits(0x5353d9f6)));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L12092-L12127 (chrome/m156)
fn fuzz763_56(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.conic_to(
        (f32::from_bits(0x5b682968), f32::from_bits(0xb3b32d11)),
        (f32::from_bits(0xb3b3b3b3), f32::from_bits(0x5b29b3b3)),
        f32::from_bits(0x72725255),
    );
    path.quad_to(
        (f32::from_bits(0x525252c7), f32::from_bits(0x72725252)),
        (f32::from_bits(0x72727272), f32::from_bits(0x72727255)),
    );
    path.quad_to(
        (f32::from_bits(0xd7da0000), f32::from_bits(0x5adada00)),
        (f32::from_bits(0x52525252), f32::from_bits(0x00005252)),
    );
    path.conic_to(
        (f32::from_bits(0xadada525), f32::from_bits(0x52525ab4)),
        (f32::from_bits(0x52525252), f32::from_bits(0x72727272)),
        f32::from_bits(0x52527272),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x72725252), f32::from_bits(0x72727272)),
        (f32::from_bits(0x72727255), f32::from_bits(0xda007b72)),
    );
    path.line_to((f32::from_bits(0x5252525a), f32::from_bits(0x72525252)));
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x52525252)),
        (f32::from_bits(0x27725252), f32::from_bits(0x72727272)),
    );
    path.line_to((f32::from_bits(0x7bc00321), f32::from_bits(0x9aaaaaaa)));
    path.quad_to(
        (f32::from_bits(0x72725572), f32::from_bits(0x00007272)),
        (f32::from_bits(0x525adada), f32::from_bits(0x52525252)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.close();
    path.move_to((f32::from_bits(0xa5252600), f32::from_bits(0x52b4adad)));
    path.close();
    path.move_to((f32::from_bits(0xa5252600), f32::from_bits(0x52b4adad)));
    path.quad_to(
        (f32::from_bits(0x72727270), f32::from_bits(0x52525272)),
        (f32::from_bits(0x72525252), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x72727255), f32::from_bits(0xda007b72)),
        (f32::from_bits(0x26525ada), f32::from_bits(0x72ada525)),
    );
    path.quad_to(
        (f32::from_bits(0x007b7272), f32::from_bits(0x525adada)),
        (f32::from_bits(0x52525252), f32::from_bits(0x72727252)),
    );
    path.quad_to(
        (f32::from_bits(0x52527272), f32::from_bits(0x52525252)),
        (f32::from_bits(0x72722772), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x74727272), f32::from_bits(0x55747421)),
        (f32::from_bits(0x0000005b), f32::from_bits(0x72727268)),
    );
    path.quad_to(
        (f32::from_bits(0x52527272), f32::from_bits(0x52525252)),
        (f32::from_bits(0x72727272), f32::from_bits(0x72557272)),
    );
    path.quad_to(
        (f32::from_bits(0x5adada72), f32::from_bits(0x52525252)),
        (f32::from_bits(0x72725252), f32::from_bits(0x72727272)),
    );
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L12129-L12176 (chrome/m156)
fn fuzz763_57(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x68546829), f32::from_bits(0x555b2d29)));
    path.move_to((f32::from_bits(0x1f2a322a), f32::from_bits(0x4b7b2108)));
    path.line_to((f32::from_bits(0x2829ed84), f32::from_bits(0x5b2d2d55)));
    path.move_to((f32::from_bits(0x6838552d), f32::from_bits(0xf0684f5b)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x2a8cef2a)),
        (f32::from_bits(0x682d2953), f32::from_bits(0xce682103)),
        f32::from_bits(0x4b7bc055),
    );
    path.line_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x3b2a8c55)));
    path.line_to((f32::from_bits(0x6838552d), f32::from_bits(0xf0684f5b)));
    path.close();
    path.move_to((f32::from_bits(0x6838552d), f32::from_bits(0xf0684f5b)));
    path.conic_to(
        (f32::from_bits(0xd2c00321), f32::from_bits(0xc0394b7b)),
        (f32::from_bits(0x8c08ed7a), f32::from_bits(0x211f2f2a)),
        f32::from_bits(0x704b7b03),
    );
    path.cubic_to(
        (f32::from_bits(0x2d6829ed), f32::from_bits(0x5b2d555b)),
        (f32::from_bits(0x68275b2d), f32::from_bits(0x21685527)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7bc0)),
    );
    path.conic_to(
        (f32::from_bits(0x212a8ced), f32::from_bits(0x0321081f)),
        (f32::from_bits(0x6a4b7bc0), f32::from_bits(0x2829ed84)),
        f32::from_bits(0x5b2d2d55),
    );
    path.move_to((f32::from_bits(0x6839552d), f32::from_bits(0xf0683b5b)));
    path.conic_to(
        (f32::from_bits(0x212a1f5b), f32::from_bits(0x228cef2a)),
        (f32::from_bits(0x682d2953), f32::from_bits(0xee682103)),
        f32::from_bits(0x287bc055),
    );
    path.line_to((f32::from_bits(0x5b2c6829), f32::from_bits(0x212a8c55)));
    path.conic_to(
        (f32::from_bits(0x4b03213b), f32::from_bits(0xc07b2a08)),
        (f32::from_bits(0x5b2d7a6a), f32::from_bits(0xf0556830)),
        f32::from_bits(0x2a8c555b),
    );
    path.conic_to(
        (f32::from_bits(0x0321212a), f32::from_bits(0x4b7bd2c0)),
        (f32::from_bits(0xed7ac039), f32::from_bits(0x2f2a8c08)),
        f32::from_bits(0x7b03211f),
    );
    path.line_to((f32::from_bits(0x6839552d), f32::from_bits(0xf0683b5b)));
    path.close();
    path.move_to((f32::from_bits(0x6839552d), f32::from_bits(0xf0683b5b)));
    path.line_to((f32::from_bits(0x6829ed27), f32::from_bits(0x2d555b2d)));
    path.move_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.conic_to(
        (f32::from_bits(0x721f2a5b), f32::from_bits(0x212a8c55)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x6a4b7b28)),
        f32::from_bits(0x4797ed7a),
    );
    path.line_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.quad_to(
        (f32::from_bits(0x2828102a), f32::from_bits(0x2c682921)),
        (f32::from_bits(0x8c555bf6), f32::from_bits(0x6d03de30)),
    );
    path.cubic_to(
        (f32::from_bits(0x683f2d55), f32::from_bits(0xf05b684b)),
        (f32::from_bits(0x8c55272d), f32::from_bits(0x212a292a)),
        (f32::from_bits(0x0321082a), f32::from_bits(0x211f2a21)),
    );
    path.line_to((f32::from_bits(0x3a803adf), f32::from_bits(0x8a294f1a)));
    path.quad_to(
        (f32::from_bits(0x291d9628), f32::from_bits(0x2a43e62b)),
        (f32::from_bits(0x093a2a81), f32::from_bits(0x5c5c8ced)),
    );
    path.line_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.close();
    path.move_to((f32::from_bits(0x68275b2d), f32::from_bits(0xf0685527)));
    path.cubic_to(
        (f32::from_bits(0x3ac2213a), f32::from_bits(0x291d9628)),
        (f32::from_bits(0x2a43e62b), f32::from_bits(0x293a2a81)),
        (f32::from_bits(0x5c5c8ced), f32::from_bits(0x5c5c6e5c)),
    );
    path.line_to((f32::from_bits(0x1f212a8c), f32::from_bits(0xc0032108)));
    path.line_to((f32::from_bits(0xed847b4b), f32::from_bits(0x2d552829)));
    path.conic_to(
        (f32::from_bits(0x552d5b5b), f32::from_bits(0x3b5a6839)),
        (f32::from_bits(0x5b2df068), f32::from_bits(0x2a212a1f)),
        f32::from_bits(0x532a8cef),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L12178-L12202 (chrome/m156)
fn fuzzhang_1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
        (f32::from_bits(0x668ece09), f32::from_bits(0x00000000)),
        (f32::from_bits(0x6751c81a), f32::from_bits(0x61c4b0fb)),
    );
    path.conic_to(
        (f32::from_bits(0x66f837a9), f32::from_bits(0x00000000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
        f32::from_bits(0x3f823406),
    );
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x675b1bfe), f32::from_bits(0x00000000)),
        (f32::from_bits(0x67d76c42), f32::from_bits(0x6292c469)),
    );
    path.cubic_to(
        (f32::from_bits(0x6a16df68), f32::from_bits(0x651a2f15)),
        (f32::from_bits(0x6c1e7f31), f32::from_bits(0x67a1f9b4)),
        (f32::from_bits(0x00000000), f32::from_bits(0x6a2a291f)),
    );
    path.conic_to(
        (f32::from_bits(0x680dcb75), f32::from_bits(0x68dd898d)),
        (f32::from_bits(0x681a434a), f32::from_bits(0x6871046b)),
        f32::from_bits(0x3fea0440),
    );
    path.quad_to(
        (f32::from_bits(0x679e1b26), f32::from_bits(0x687703c4)),
        (f32::from_bits(0x00000000), f32::from_bits(0x687d2968)),
    );
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0x535353ec), f32::from_bits(0x98989898)),
        (f32::from_bits(0x98989898), f32::from_bits(0xf207f36e)),
        (f32::from_bits(0xf3f2f2f2), f32::from_bits(0xed3a9781)),
    );
    path.quad_to(
        (f32::from_bits(0xf8f8c0ed), f32::from_bits(0xf8f8f8f8)),
        (f32::from_bits(0x9f9f9f9f), f32::from_bits(0x3014149f)),
    );
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Difference, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L12204-L12278 (chrome/m156)
fn release_13(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xd4438848), f32::from_bits(0xd488cf64)));
    path.line_to((f32::from_bits(0xd43a056e), f32::from_bits(0xd4851696)));
    path.quad_to(
        (f32::from_bits(0xd3d48e79), f32::from_bits(0xd49fb136)),
        (f32::from_bits(0x00000000), f32::from_bits(0xd4d4d4d4)),
    );
    path.quad_to(
        (f32::from_bits(0xd3d06670), f32::from_bits(0xd4a0bb38)),
        (f32::from_bits(0xd41d628f), f32::from_bits(0xd472c531)),
    );
    path.line_to((f32::from_bits(0xd43a0559), f32::from_bits(0xd485168e)));
    path.line_to((f32::from_bits(0xd446958b), f32::from_bits(0xd4810278)));
    path.line_to((f32::from_bits(0xd443884a), f32::from_bits(0xd488cf65)));
    path.quad_to(
        (f32::from_bits(0xd47efa09), f32::from_bits(0xd49fd72a)),
        (f32::from_bits(0xd4a63f0f), f32::from_bits(0xd4b83ab3)),
    );
    path.line_to((f32::from_bits(0xd497ca70), f32::from_bits(0xd4c4d4ae)));
    path.line_to((f32::from_bits(0xd459d4d4), f32::from_bits(0xd4c4d4d4)));
    path.line_to((f32::from_bits(0xd440daf9), f32::from_bits(0xd4c632d3)));
    path.line_to((f32::from_bits(0xd4438848), f32::from_bits(0xd488cf64)));
    path.close();
    path.move_to((f32::from_bits(0xd4767560), f32::from_bits(0xd4d1ca84)));
    path.line_to((f32::from_bits(0xd4422174), f32::from_bits(0xd4d02069)));
    path.line_to((f32::from_bits(0xd440daa3), f32::from_bits(0xd4c632d9)));
    path.line_to((f32::from_bits(0xd41017bc), f32::from_bits(0xd4cb99b6)));
    path.line_to((f32::from_bits(0xd442213b), f32::from_bits(0xd4d02067)));
    path.line_to((f32::from_bits(0xd442d4d4), f32::from_bits(0xd4d4d4d4)));
    path.line_to((f32::from_bits(0xd4767560), f32::from_bits(0xd4d1ca84)));
    path.close();
    path.move_to((f32::from_bits(0xd46c7a11), f32::from_bits(0xd46c7a2e)));
    path.line_to((f32::from_bits(0xd484e02c), f32::from_bits(0xd45fafcd)));
    path.line_to((f32::from_bits(0xd462c867), f32::from_bits(0xd45655f7)));
    path.line_to((f32::from_bits(0xd45ac463), f32::from_bits(0xd45ac505)));
    path.line_to((f32::from_bits(0xd43d2fa9), f32::from_bits(0xd43d2fb5)));
    path.line_to((f32::from_bits(0xd41d6287), f32::from_bits(0xd472c52a)));
    path.quad_to(
        (f32::from_bits(0x00000000), f32::from_bits(0xd3db1b95)),
        (f32::from_bits(0x00000000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xd4b7efac), f32::from_bits(0x00000000)),
        (f32::from_bits(0xd4d0e88f), f32::from_bits(0xd40b8b46)),
    );
    path.line_to((f32::from_bits(0xd4d4d4d4), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0xdcdc154b), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0xd4d4d4d4), f32::from_bits(0xd4c4d477)));
    path.line_to((f32::from_bits(0xd4d4d4d4), f32::from_bits(0xd4d4d442)));
    path.line_to((f32::from_bits(0xd4d4a691), f32::from_bits(0xd4d4d442)));
    path.line_to((f32::from_bits(0xd454d4d4), f32::from_bits(0xd4d4aa30)));
    path.line_to((f32::from_bits(0xd4bd9def), f32::from_bits(0xd4d43df0)));
    path.line_to((f32::from_bits(0xd4767560), f32::from_bits(0xd4d1ca84)));
    path.line_to((f32::from_bits(0xd497ca70), f32::from_bits(0xd4c4d4ae)));
    path.line_to((f32::from_bits(0xd4bab953), f32::from_bits(0xd4c4d48e)));
    path.line_to((f32::from_bits(0xd4a63f0f), f32::from_bits(0xd4b83ab3)));
    path.line_to((f32::from_bits(0xd4ae61eb), f32::from_bits(0xd4ae61f4)));
    path.line_to((f32::from_bits(0xd46c7a11), f32::from_bits(0xd46c7a2e)));
    path.close();
    path.move_to((f32::from_bits(0xd46c7a11), f32::from_bits(0xd46c7a2e)));
    path.line_to((f32::from_bits(0xd446965c), f32::from_bits(0xd4810237)));
    path.line_to((f32::from_bits(0xd45ac549), f32::from_bits(0xd45ac55f)));
    path.line_to((f32::from_bits(0xd46c7a11), f32::from_bits(0xd46c7a2e)));
    path.close();
    path.move_to((f32::from_bits(0xd4b46028), f32::from_bits(0xd41e572a)));
    path.line_to((f32::from_bits(0xd4cde20a), f32::from_bits(0xd434bb57)));
    path.line_to((f32::from_bits(0xd4c75ffe), f32::from_bits(0xd46f215d)));
    path.line_to((f32::from_bits(0xd4b46028), f32::from_bits(0xd41e572a)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.quad_to(
        (f32::from_bits(0x00000000), f32::from_bits(0xa5a50000)),
        (f32::from_bits(0xd4d4a5a5), f32::from_bits(0xd4d4d4d4)),
    );
    path.quad_to(
        (f32::from_bits(0xd4d4d4d4), f32::from_bits(0xd4d4d4d4)),
        (f32::from_bits(0xd4cfd4d4), f32::from_bits(0xd4d41dd4)),
    );
    path.quad_to(
        (f32::from_bits(0xd4d4d4d4), f32::from_bits(0xd4d432d4)),
        (f32::from_bits(0xd4d4d4d4), f32::from_bits(0xd4a5a5d4)),
    );
    path.quad_to(
        (f32::from_bits(0xd4d4d4d4), f32::from_bits(0xd4d4d4d4)),
        (f32::from_bits(0xd4d4d4d4), f32::from_bits(0x00000000)),
    );
    path.move_to((f32::from_bits(0xa5a5a500), f32::from_bits(0xd4d4d4a5)));
    path.quad_to(
        (f32::from_bits(0xd4d4d4d4), f32::from_bits(0x2ad4d4d4)),
        (f32::from_bits(0xd4d4d4d4), f32::from_bits(0xd4cfd4d4)),
    );
    path.quad_to(
        (f32::from_bits(0xd4d4d4d4), f32::from_bits(0xd4d4d4d4)),
        (f32::from_bits(0xd4d4d4d4), f32::from_bits(0xd4d4d4d4)),
    );
    path.quad_to(
        (f32::from_bits(0xd4d40000), f32::from_bits(0xd4d4d4d4)),
        (f32::from_bits(0xd4d4d4d4), f32::from_bits(0xd4d4d4d4)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L12280-L12316 (chrome/m156)
fn fuzzhang_2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x5568392a), f32::from_bits(0x72837268)));
    path.quad_to(
        (f32::from_bits(0xe0e02972), f32::from_bits(0xe0e060e0)),
        (f32::from_bits(0x728e4603), f32::from_bits(0x72727272)),
    );
    path.line_to((f32::from_bits(0x5568392a), f32::from_bits(0x72837268)));
    path.close();
    path.move_to((f32::from_bits(0x5568392a), f32::from_bits(0x72837268)));
    path.quad_to(
        (f32::from_bits(0x68720052), f32::from_bits(0x52527372)),
        (f32::from_bits(0x00527252), f32::from_bits(0x728e4601)),
    );
    path.quad_to(
        (f32::from_bits(0x52ec7272), f32::from_bits(0x6265527f)),
        (f32::from_bits(0x8e460152), f32::from_bits(0x72ff8072)),
    );
    path.line_to((f32::from_bits(0x5568392a), f32::from_bits(0x72837268)));
    path.close();
    path.move_to((f32::from_bits(0x5568392a), f32::from_bits(0x72837268)));
    path.line_to((f32::from_bits(0x52626552), f32::from_bits(0x72727272)));
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x62727272)),
        (f32::from_bits(0x39393939), f32::from_bits(0x728bc739)),
    );
    path.cubic_to(
        (f32::from_bits(0x72728092), f32::from_bits(0x72727260)),
        (f32::from_bits(0x4d727272), f32::from_bits(0x5252522a)),
        (f32::from_bits(0x72735252), f32::from_bits(0x72707272)),
    );
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x56727272)),
        (f32::from_bits(0x72720152), f32::from_bits(0x72727270)),
    );
    path.quad_to(
        (f32::from_bits(0x52526172), f32::from_bits(0x8e460300)),
        (f32::from_bits(0x72727272), f32::from_bits(0x52525272)),
    );
    path.conic_to(
        (f32::from_bits(0xb5727272), f32::from_bits(0x7f2b727f)),
        (f32::from_bits(0x607272ff), f32::from_bits(0x72727276)),
        f32::from_bits(0x2a527272),
    );
    path.line_to((f32::from_bits(0x5568392a), f32::from_bits(0x72837268)));
    path.close();
    path.move_to((f32::from_bits(0x5568392a), f32::from_bits(0x72837268)));
    path.line_to((f32::from_bits(0x72727272), f32::from_bits(0x52525f72)));
    path.line_to((f32::from_bits(0x5568392a), f32::from_bits(0x72837268)));
    path.close();
    path.move_to((f32::from_bits(0x5568392a), f32::from_bits(0x72837268)));
    path.quad_to(
        (f32::from_bits(0x52727272), f32::from_bits(0x64655252)),
        (f32::from_bits(0x72c1c152), f32::from_bits(0x72727272)),
    );
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Intersect, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L12318-L12368 (chrome/m156)
fn fuzzhang_3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x46090052), f32::from_bits(0x7270726c)));
    path.move_to((f32::from_bits(0xe0437272), f32::from_bits(0x03e0e060)));
    path.close();
    path.move_to((f32::from_bits(0xe0437272), f32::from_bits(0x03e0e060)));
    path.line_to((f32::from_bits(0x77727272), f32::from_bits(0x52520072)));
    path.line_to((f32::from_bits(0x46090052), f32::from_bits(0x727272ce)));
    path.quad_to(
        (f32::from_bits(0x725252ec), f32::from_bits(0x72727272)),
        (f32::from_bits(0x72727272), f32::from_bits(0x39393962)),
    );
    path.line_to((f32::from_bits(0x6c460900), f32::from_bits(0x72727072)));
    path.cubic_to(
        (f32::from_bits(0xe0e060e0), f32::from_bits(0x72943603)),
        (f32::from_bits(0x72777272), f32::from_bits(0x5c525200)),
        (f32::from_bits(0x46090052), f32::from_bits(0x727272ce)),
    );
    path.quad_to(
        (f32::from_bits(0x725252ec), f32::from_bits(0x72727272)),
        (f32::from_bits(0x72727272), f32::from_bits(0x39393962)),
    );
    path.line_to((f32::from_bits(0x0052ca00), f32::from_bits(0x728e4603)));
    path.quad_to(
        (f32::from_bits(0xff727272), f32::from_bits(0x52527272)),
        (f32::from_bits(0x39392072), f32::from_bits(0xe0393939)),
    );
    path.line_to((f32::from_bits(0xe0437272), f32::from_bits(0x03e0e060)));
    path.close();
    path.move_to((f32::from_bits(0xe0437272), f32::from_bits(0x03e0e060)));
    path.cubic_to(
        (f32::from_bits(0xdada7272), f32::from_bits(0x2dff7272)),
        (f32::from_bits(0x767272f0), f32::from_bits(0x72727272)),
        (f32::from_bits(0x21727f72), f32::from_bits(0x0b210929)),
    );
    path.cubic_to(
        (f32::from_bits(0xd6d6d6d6), f32::from_bits(0x72a5d6d6)),
        (f32::from_bits(0x72553872), f32::from_bits(0xdada7072)),
        (f32::from_bits(0x5252525a), f32::from_bits(0x72727252)),
    );
    path.quad_to(
        (f32::from_bits(0x72725572), f32::from_bits(0xdada0072)),
        (f32::from_bits(0x52524b5a), f32::from_bits(0x72528000)),
    );
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0xca005252)),
        (f32::from_bits(0x46030052), f32::from_bits(0x7272728e)),
    );
    path.quad_to(
        (f32::from_bits(0x7272ff72), f32::from_bits(0x20725252)),
        (f32::from_bits(0x39393939), f32::from_bits(0xd76ee039)),
    );
    path.cubic_to(
        (f32::from_bits(0xdada7272), f32::from_bits(0x2dff7272)),
        (f32::from_bits(0x767272f0), f32::from_bits(0x72727272)),
        (f32::from_bits(0x21727f72), f32::from_bits(0x0b210929)),
    );
    path.cubic_to(
        (f32::from_bits(0xd6d6d6d6), f32::from_bits(0x72a5d6d6)),
        (f32::from_bits(0x72553872), f32::from_bits(0xdada7072)),
        (f32::from_bits(0x5252525a), f32::from_bits(0x72727252)),
    );
    path.quad_to(
        (f32::from_bits(0x72725572), f32::from_bits(0xdada0072)),
        (f32::from_bits(0x52524b5a), f32::from_bits(0x72528000)),
    );
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x52525252)),
        (f32::from_bits(0x27725252), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x72667254), f32::from_bits(0x00000040)),
        (f32::from_bits(0x00a70155), f32::from_bits(0x726800ff)),
    );
    path.quad_to(
        (f32::from_bits(0x7b727272), f32::from_bits(0xad000c52)),
        (f32::from_bits(0x1c10adad), f32::from_bits(0x72728d8a)),
    );
    path.quad_to(
        (f32::from_bits(0xff056546), f32::from_bits(0x727205ff)),
        (f32::from_bits(0x524b5aff), f32::from_bits(0x64005252)),
    );
    path.quad_to(
        (f32::from_bits(0x72524872), f32::from_bits(0xdada7272)),
        (f32::from_bits(0x5252525a), f32::from_bits(0x72727252)),
    );
    path.quad_to(
        (f32::from_bits(0x72724172), f32::from_bits(0xdad10072)),
        (f32::from_bits(0x52524b5a), f32::from_bits(0x725b8000)),
    );
    path.quad_to(
        (f32::from_bits(0x72727272), f32::from_bits(0x52525252)),
        (f32::from_bits(0x27725252), f32::from_bits(0x72727272)),
    );
    path.quad_to(
        (f32::from_bits(0x72728372), f32::from_bits(0x00000040)),
        (f32::from_bits(0xf6a70147), f32::from_bits(0xc2c2c256)),
    );
    path.line_to((f32::from_bits(0xe0437272), f32::from_bits(0x03e0e060)));
    path.close();
    path.move_to((f32::from_bits(0x7a787a7a), f32::from_bits(0x7a3a7a7a)));
    path.line_to((f32::from_bits(0x8f4603e0), f32::from_bits(0x72727272)));
    path.quad_to(
        (f32::from_bits(0x00807272), f32::from_bits(0x46090052)),
        (f32::from_bits(0x7270726c), f32::from_bits(0x60e04372)),
    );
    path.move_to((f32::from_bits(0x943603e0), f32::from_bits(0x77727272)));
    path.quad_to(
        (f32::from_bits(0x5c525200), f32::from_bits(0x46090052)),
        (f32::from_bits(0x727272ce), f32::from_bits(0x5252ec72)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L12370-L12388 (chrome/m156)
fn fuzz754434_1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0x535e5372), f32::from_bits(0x53536153)),
        (f32::from_bits(0x79530f53), f32::from_bits(0x101b6c88)),
        (f32::from_bits(0x5353735e), f32::from_bits(0x006df653)),
    );
    path.cubic_to(
        (f32::from_bits(0xf26df46d), f32::from_bits(0xf6f6f6f6)),
        (f32::from_bits(0x5656f666), f32::from_bits(0x5a565656)),
        (f32::from_bits(0x00000056), f32::from_bits(0xf66e5600)),
    );
    path.line_to((f32::from_bits(0xff00ff56), f32::from_bits(0x00faf6f6)));
    path.move_to((f32::from_bits(0x60576bfa), f32::from_bits(0x006df653)));
    path.cubic_to(
        (f32::from_bits(0xf26df46d), f32::from_bits(0xf653f6f6)),
        (f32::from_bits(0x563ef666), f32::from_bits(0x56565656)),
        (f32::from_bits(0x65565656), f32::from_bits(0xf6765656)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L12390-L12412 (chrome/m156)
fn fuzz754434_2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xff00ff56), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xf66e5600)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xf629168b)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0xff00ff56), f32::from_bits(0x00000000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x03e8f6f6), f32::from_bits(0xf7060000)));
    path.line_to((f32::from_bits(0x4ff6f6f6), f32::from_bits(0x3e3e3e2a)));
    path.conic_to(
        (f32::from_bits(0x6c8879ff), f32::from_bits(0x08761b1b)),
        (f32::from_bits(0x7066662d), f32::from_bits(0x70707070)),
        f32::from_bits(0x70707070),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L12414-L12432 (chrome/m156)
fn fuzz754434_3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.cubic_to(
        (f32::from_bits(0x535e5372), f32::from_bits(0x53536153)),
        (f32::from_bits(0x79530f53), f32::from_bits(0x101b6c88)),
        (f32::from_bits(0x5353735e), f32::from_bits(0x006df653)),
    );
    path.cubic_to(
        (f32::from_bits(0xf26df46d), f32::from_bits(0xf6f6f6f6)),
        (f32::from_bits(0x5656f666), f32::from_bits(0x5a565656)),
        (f32::from_bits(0x00000056), f32::from_bits(0xf66e5600)),
    );
    path.line_to((f32::from_bits(0xff00ff56), f32::from_bits(0x00faf6f6)));
    path.move_to((f32::from_bits(0x60576bfa), f32::from_bits(0x006df653)));
    path.cubic_to(
        (f32::from_bits(0xf26df46d), f32::from_bits(0xf653f6f6)),
        (f32::from_bits(0x563ef666), f32::from_bits(0x56565656)),
        (f32::from_bits(0x65565656), f32::from_bits(0xf6765656)),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Xor, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L12434-L12456 (chrome/m156)
fn fuzz754434_4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::from_bits(1));
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xff00ff56), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xf66e5600)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xf629168b)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0xff00ff56), f32::from_bits(0x00000000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::from_bits(0));
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    path.line_to((f32::from_bits(0x03e8f6f6), f32::from_bits(0xf7060000)));
    path.line_to((f32::from_bits(0x4ff6f6f6), f32::from_bits(0x3e3e3e2a)));
    path.conic_to(
        (f32::from_bits(0x6c8879ff), f32::from_bits(0x08761b1b)),
        (f32::from_bits(0x7066662d), f32::from_bits(0x70707070)),
        f32::from_bits(0x70707070),
    );
    let path2 = path.detach();
    test_path_op_fuzz(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsOpTest.cpp#L9136-L9499 (chrome/m156), the `tests[]` table
const TESTS: &[(&str, TestFn)] = &[
    ("bug8380", bug8380),
    ("crbug_526025", crbug_526025),
    ("bug8228", bug8228),
    ("op_4", op_4),
    ("op_1", op_1),
    ("op_2", op_2),
    ("op_3", op_3),
    ("grshapearcs1", grshapearcs1),
    ("filinmangust14", filinmangust14),
    ("testRect1_u", test_rect1_u),
    ("halbug", halbug),
    ("seanbug", seanbug),
    ("android1", android1),
    ("bug5240", bug5240),
    ("circlesOp4", circles_op4),
    ("loop17", loop17),
    ("cubicOp158", cubic_op158),
    ("loops_i1", loops_i1),
    ("loops_i2", loops_i2),
    ("loops_i3", loops_i3),
    ("loops_i4", loops_i4),
    ("loops_i5", loops_i5),
    ("loops_i6", loops_i6),
    ("cubics_d3", cubics_d3),
    ("cubics_o", cubics_o),
    ("cubics_d2", cubics_d2),
    ("cubics_d", cubics_d),
    ("dean2", dean2),
    ("fuzzX_392", fuzz_x_392),
    ("fuzz38", fuzz38),
    ("cubics44d", cubics44d),
    ("cubics45u", cubics45u),
    ("loops61i", loops61i),
    ("loops62i", loops62i),
    ("loops63i", loops63i),
    ("loops58iAsQuads", loops58i_as_quads),
    ("cubics41d", cubics41d),
    ("loops59iasQuads", loops59ias_quads),
    ("loops59i", loops59i),
    ("loops44i", loops44i),
    ("loops45i", loops45i),
    ("loops46i", loops46i),
    ("loops47i", loops47i),
    ("loops48i", loops48i),
    ("loops49i", loops49i),
    ("loops50i", loops50i),
    ("loops51i", loops51i),
    ("loops52i", loops52i),
    ("loops53i", loops53i),
    ("loops54i", loops54i),
    ("loops55i", loops55i),
    ("loops56i", loops56i),
    ("loops57i", loops57i),
    ("loops58i", loops58i),
    ("loops33iMod", loops33i_mod),
    ("loops33iAsQuads", loops33i_as_quads),
    ("loops33i", loops33i),
    ("loops40i", loops40i),
    ("loops40iAsQuads", loops40i_as_quads),
    ("loops39i", loops39i),
    ("loops38i", loops38i),
    ("loops37i", loops37i),
    ("loops36i", loops36i),
    ("loops35i", loops35i),
    ("loops34i", loops34i),
    ("loops32i", loops32i),
    ("loops31i", loops31i),
    ("loops30i", loops30i),
    ("loops29i", loops29i),
    ("loops28i", loops28i),
    ("loops27i", loops27i),
    ("loops26i", loops26i),
    ("loops25i", loops25i),
    ("loops24i", loops24i),
    ("loops23i", loops23i),
    ("loops22i", loops22i),
    ("loops21i", loops21i),
    ("loops20i", loops20i),
    ("cubics20d", cubics20d),
    ("cubics6d", cubics6d),
    ("cubics7d", cubics7d),
    ("cubics8d", cubics8d),
    ("cubics9d", cubics9d),
    ("cubics10u", cubics10u),
    ("cubics11i", cubics11i),
    ("cubics12d", cubics12d),
    ("cubics13d", cubics13d),
    ("cubics14d", cubics14d),
    ("cubics15d", cubics15d),
    ("cubics16i", cubics16i),
    ("cubics17d", cubics17d),
    ("cubics18d", cubics18d),
    ("cubics19d", cubics19d),
    ("cubicOp157", cubic_op157),
    ("cubicOp142", cubic_op142),
    ("loops4i", loops4i),
    ("quadRect1", quad_rect1),
    ("quadRect2", quad_rect2),
    ("quadRect3", quad_rect3),
    ("quadRect4", quad_rect4),
    ("quadRect5", quad_rect5),
    ("quadRect6", quad_rect6),
    ("cubicOp141", cubic_op141),
    ("cubicOp58d", cubic_op58d),
    ("loops5i", loops5i),
    ("cubicOp140", cubic_op140),
    ("cubicOp139", cubic_op139),
    ("cubics138", cubics138),
    ("cubics137", cubics137),
    ("cubicOp136a", cubic_op136a),
    ("cubicOp136", cubic_op136),
    ("cubicOp135", cubic_op135),
    ("cubicOp134", cubic_op134),
    ("cubicOp133", cubic_op133),
    ("loop12", loop12),
    ("cubicOp132", cubic_op132),
    ("loop11", loop11),
    ("loop10", loop10),
    ("circlesOp3", circles_op3),
    ("loop9", loop9),
    ("loop8", loop8),
    ("rects5", rects5),
    ("loop7", loop7),
    ("cubicOp130a", cubic_op130a),
    ("rRect1x", r_rect1x),
    ("circlesOp2", circles_op2),
    ("circlesOp1", circles_op1),
    ("cubicOp131", cubic_op131),
    ("cubicOp130", cubic_op130),
    ("cubicOp129", cubic_op129),
    ("cubicOp128", cubic_op128),
    ("cubicOp127", cubic_op127),
    ("cubicOp126", cubic_op126),
    ("cubicOp125", cubic_op125),
    ("cubicOp124", cubic_op124),
    ("loop6", loop6),
    ("loop5", loop5),
    ("cubicOp123", cubic_op123),
    ("cubicOp122", cubic_op122),
    ("cubicOp121", cubic_op121),
    ("cubicOp120", cubic_op120),
    ("cubicOp119", cubic_op119),
    ("loop4", loop4),
    ("loop3", loop3),
    ("loop2", loop2),
    ("loop1asQuad", loop1as_quad),
    ("loop1", loop1),
    ("issue3517", issue3517),
    ("cubicOp118", cubic_op118),
    ("cubicOp117", cubic_op117),
    ("cubicOp116", cubic_op116),
    ("testRect2", test_rect2),
    ("testRect1", test_rect1),
    ("cubicOp115", cubic_op115),
    ("issue2753", issue2753),
    ("cubicOp114", cubic_op114),
    ("issue2808", issue2808),
    ("cubicOp114asQuad", cubic_op114as_quad),
    ("rects4", rects4),
    ("rects3", rects3),
    ("rects2", rects2),
    ("rects1", rects1),
    ("issue2540", issue2540),
    ("issue2504", issue2504),
    ("kari1", kari1),
    ("quadOp10i", quad_op10i),
    ("cubicOp113", cubic_op113),
    ("skpcarrot_is24", skpcarrot_is24),
    ("issue1417", issue1417),
    ("cubicOp112", cubic_op112),
    ("skpadspert_net23", skpadspert_net23),
    ("skpadspert_de11", skpadspert_de11),
    ("findFirst1", find_first1),
    ("xOp2i", x_op2i),
    ("xOp3i", x_op3i),
    ("xOp1u", x_op1u),
    ("xOp1i", x_op1i),
    ("cubicOp111", cubic_op111),
    ("cubicOp110", cubic_op110),
    ("cubicOp109", cubic_op109),
    ("cubicOp108", cubic_op108),
    ("cubicOp107", cubic_op107),
    ("cubicOp106", cubic_op106),
    ("cubicOp105", cubic_op105),
    ("cubicOp104", cubic_op104),
    ("cubicOp103", cubic_op103),
    ("cubicOp102", cubic_op102),
    ("cubicOp101", cubic_op101),
    ("cubicOp100", cubic_op100),
    ("cubicOp99", cubic_op99),
    ("issue1435", issue1435),
    ("cubicOp98x", cubic_op98x),
    ("cubicOp97x", cubic_op97x),
    ("skpcarpetplanet_ru22", skpcarpetplanet_ru22),
    ("cubicOp96d", cubic_op96d),
    ("cubicOp95u", cubic_op95u),
    ("skpadbox_lt15", skpadbox_lt15),
    ("skpagentxsites_com55", skpagentxsites_com55),
    ("skpadventistmission_org572", skpadventistmission_org572),
    ("skpadoption_org196", skpadoption_org196),
    ("skpbambootheme_com12", skpbambootheme_com12),
    ("skpbakosoft_com10", skpbakosoft_com10),
    ("skpakmmos_ru100", skpakmmos_ru100),
    ("skpbangalorenest_com4", skpbangalorenest_com4),
    ("skpbingoentertainment_net189", skpbingoentertainment_net189),
    ("skpbestred_ru37", skpbestred_ru37),
    ("skpbenzoteh_ru152", skpbenzoteh_ru152),
    ("skpcamcorder_kz21", skpcamcorder_kz21),
    ("skpcaffelavazzait_com_ua21", skpcaffelavazzait_com_ua21),
    ("skpcarrefour_ro62", skpcarrefour_ro62),
    ("skpcavablar_net563", skpcavablar_net563),
    ("skpinsomnia_gr72", skpinsomnia_gr72),
    ("skpadbox_lt8", skpadbox_lt8),
    ("skpact_com43", skpact_com43),
    ("skpacesoftech_com47", skpacesoftech_com47),
    ("skpabcspark_ca103", skpabcspark_ca103),
    ("cubicOp94u", cubic_op94u),
    ("cubicOp93d", cubic_op93d),
    ("cubicOp92i", cubic_op92i),
    (
        "skpadithya_putr4_blogspot_com551",
        skpadithya_putr4_blogspot_com551,
    ),
    ("skpadindex_de4", skpadindex_de4),
    ("skpaiaigames_com870", skpaiaigames_com870),
    ("skpaaalgarve_org53", skpaaalgarve_org53),
    ("skpkkiste_to716", skpkkiste_to716),
    ("cubicOp91u", cubic_op91u),
    ("cubicOp90u", cubic_op90u),
    ("cubicOp89u", cubic_op89u),
    ("cubicOp88u", cubic_op88u),
    ("cubicOp87u", cubic_op87u),
    ("cubicOp86i", cubic_op86i),
    ("loopEdge2", loop_edge2),
    ("loopEdge1", loop_edge1),
    ("rectOp3x", rect_op3x),
    ("rectOp2i", rect_op2i),
    ("rectOp1i", rect_op1i),
    ("issue1418b", issue1418b),
    ("cubicOp85i", cubic_op85i),
    ("issue1418", issue1418),
    ("skpkkiste_to98", skpkkiste_to98),
    ("skpahrefs_com29", skpahrefs_com29),
    ("cubicOp85d", cubic_op85d),
    ("skpahrefs_com88", skpahrefs_com88),
    ("skphealth_com76", skphealth_com76),
    ("skpancestry_com1", skpancestry_com1),
    ("skpbyte_com1", skpbyte_com1),
    ("skpeldorado_com_ua1", skpeldorado_com_ua1),
    ("skp96prezzi1", skp96prezzi1),
    ("skpClip2", skp_clip2),
    ("skpClip1", skp_clip1),
    ("cubicOp84d", cubic_op84d),
    ("cubicOp83i", cubic_op83i),
    ("cubicOp82i", cubic_op82i),
    ("cubicOp81d", cubic_op81d),
    ("cubicOp80i", cubic_op80i),
    ("cubicOp79u", cubic_op79u),
    ("cubicOp78u", cubic_op78u),
    ("cubicOp77i", cubic_op77i),
    ("cubicOp76u", cubic_op76u),
    ("cubicOp75d", cubic_op75d),
    ("cubicOp74d", cubic_op74d),
    ("cubicOp73d", cubic_op73d),
    ("cubicOp72i", cubic_op72i),
    ("cubicOp71d", cubic_op71d),
    ("skp5", skp5),
    ("skp4", skp4),
    ("skp3", skp3),
    ("skp2", skp2),
    ("skp1", skp1),
    ("rRect1", r_rect1),
    ("cubicOp70d", cubic_op70d),
    ("cubicOp69d", cubic_op69d),
    ("cubicOp68u", cubic_op68u),
    ("cubicOp67u", cubic_op67u),
    ("cubicOp66u", cubic_op66u),
    ("rectOp1d", rect_op1d),
    ("cubicOp65d", cubic_op65d),
    ("cubicOp64d", cubic_op64d),
    ("cubicOp63d", cubic_op63d),
    ("cubicOp62d", cubic_op62d),
    ("cubicOp61d", cubic_op61d),
    ("cubicOp60d", cubic_op60d),
    ("cubicOp59d", cubic_op59d),
    ("cubicOp57d", cubic_op57d),
    ("cubicOp56d", cubic_op56d),
    ("cubicOp55d", cubic_op55d),
    ("cubicOp54d", cubic_op54d),
    ("cubicOp53d", cubic_op53d),
    ("cubicOp52d", cubic_op52d),
    ("cubicOp51d", cubic_op51d),
    ("cubicOp50d", cubic_op50d),
    ("cubicOp49d", cubic_op49d),
    ("cubicOp48d", cubic_op48d),
    ("cubicOp47d", cubic_op47d),
    ("cubicOp46d", cubic_op46d),
    ("cubicOp45d", cubic_op45d),
    ("cubicOp44d", cubic_op44d),
    ("cubicOp43d", cubic_op43d),
    ("cubicOp42d", cubic_op42d),
    ("cubicOp41i", cubic_op41i),
    ("cubicOp40d", cubic_op40d),
    ("cubicOp39d", cubic_op39d),
    ("cubicOp38d", cubic_op38d),
    ("cubicOp37d", cubic_op37d),
    ("cubicOp36u", cubic_op36u),
    ("cubicOp35d", cubic_op35d),
    ("cubicOp34d", cubic_op34d),
    ("cubicOp33i", cubic_op33i),
    ("cubicOp32d", cubic_op32d),
    ("cubicOp31d", cubic_op31d),
    ("cubicOp31x", cubic_op31x),
    ("cubicOp31u", cubic_op31u),
    ("cubicOp30d", cubic_op30d),
    ("cubicOp29d", cubic_op29d),
    ("cubicOp28u", cubic_op28u),
    ("cubicOp27d", cubic_op27d),
    ("cubicOp26d", cubic_op26d),
    ("cubicOp25i", cubic_op25i),
    ("testOp8d", test_op8d),
    ("testDiff1", test_diff1),
    ("testIntersect1", test_intersect1),
    ("testUnion1", test_union1),
    ("testXor1", test_xor1),
    ("testDiff2", test_diff2),
    ("testIntersect2", test_intersect2),
    ("testUnion2", test_union2),
    ("testXor2", test_xor2),
    ("testOp1d", test_op1d),
    ("testOp2d", test_op2d),
    ("testOp3d", test_op3d),
    ("testOp1u", test_op1u),
    ("testOp4d", test_op4d),
    ("testOp5d", test_op5d),
    ("testOp6d", test_op6d),
    ("testOp7d", test_op7d),
    ("testOp2u", test_op2u),
    ("cubicOp24d", cubic_op24d),
    ("cubicOp23d", cubic_op23d),
    ("cubicOp22d", cubic_op22d),
    ("cubicOp21d", cubic_op21d),
    ("cubicOp20d", cubic_op20d),
    ("cubicOp19i", cubic_op19i),
    ("cubicOp18d", cubic_op18d),
    ("cubicOp17d", cubic_op17d),
    ("cubicOp16d", cubic_op16d),
    ("cubicOp15d", cubic_op15d),
    ("cubicOp14d", cubic_op14d),
    ("cubicOp13d", cubic_op13d),
    ("cubicOp12d", cubic_op12d),
    ("cubicOp11d", cubic_op11d),
    ("cubicOp10d", cubic_op10d),
    ("cubicOp1i", cubic_op1i),
    ("cubicOp9d", cubic_op9d),
    ("quadOp9d", quad_op9d),
    ("lineOp9d", line_op9d),
    ("cubicOp8d", cubic_op8d),
    ("cubicOp7d", cubic_op7d),
    ("cubicOp6d", cubic_op6d),
    ("cubicOp5d", cubic_op5d),
    ("cubicOp3d", cubic_op3d),
    ("cubicOp2d", cubic_op2d),
    ("cubicOp1d", cubic_op1d),
];

// Port of: tests/PathOpsOpTest.cpp#L12458-L12544 (chrome/m156), the `failTests[]` table
const FAIL_TESTS: &[(&str, TestFn)] = &[
    ("fuzz767834", fuzz767834),
    ("fuzz754434_1", fuzz754434_1),
    ("fuzz754434_2", fuzz754434_2),
    ("fuzz754434_3", fuzz754434_3),
    ("fuzz754434_4", fuzz754434_4),
    ("fuzzhang_3", fuzzhang_3),
    ("fuzzhang_2", fuzzhang_2),
    ("release_13", release_13),
    ("fuzzhang_1", fuzzhang_1),
    ("fuzz763_57", fuzz763_57),
    ("fuzz763_56", fuzz763_56),
    ("fuzz763_55", fuzz763_55),
    ("fuzz763_54", fuzz763_54),
    ("fuzz763_53", fuzz763_53),
    ("fuzz763_52", fuzz763_52),
    ("fuzz763_51", fuzz763_51),
    ("fuzz763_50", fuzz763_50),
    ("fuzz763_49", fuzz763_49),
    ("fuzz763_48", fuzz763_48),
    ("fuzz763_47", fuzz763_47),
    ("fuzz763_46", fuzz763_46),
    ("fuzz763_45", fuzz763_45),
    ("fuzz763_44", fuzz763_44),
    ("fuzz763_43", fuzz763_43),
    ("fuzz763_42", fuzz763_42),
    ("fuzz763_41", fuzz763_41),
    ("fuzz763_40", fuzz763_40),
    ("fuzz763_39", fuzz763_39),
    ("fuzz763_38", fuzz763_38),
    ("fuzz763_37", fuzz763_37),
    ("fuzz763_36", fuzz763_36),
    ("fuzz763_35", fuzz763_35),
    ("fuzz763_34", fuzz763_34),
    ("fuzz763_33", fuzz763_33),
    ("fuzz763_32", fuzz763_32),
    ("fuzz763_31", fuzz763_31),
    ("fuzz763_30", fuzz763_30),
    ("fuzz763_29", fuzz763_29),
    ("fuzz763_28", fuzz763_28),
    ("fuzz763_27", fuzz763_27),
    ("fuzz763_26", fuzz763_26),
    ("fuzz763_25", fuzz763_25),
    ("fuzz763_24", fuzz763_24),
    ("fuzz763_23", fuzz763_23),
    ("fuzz763_22", fuzz763_22),
    ("fuzz763_21", fuzz763_21),
    ("fuzz763_20", fuzz763_20),
    ("fuzz763_19", fuzz763_19),
    ("fuzz763_18", fuzz763_18),
    ("fuzz763_17", fuzz763_17),
    ("fuzz763_16", fuzz763_16),
    ("fuzz763_15", fuzz763_15),
    ("fuzz763_14", fuzz763_14),
    ("fuzz763_13", fuzz763_13),
    ("fuzz763_12", fuzz763_12),
    ("fuzz763_11", fuzz763_11),
    ("fuzz763_10", fuzz763_10),
    ("kfuzz2", kfuzz2),
    ("fuzz763_7", fuzz763_7),
    ("fuzz763_6", fuzz763_6),
    ("fuzz763_2c", fuzz763_2c),
    ("fuzz763_2b", fuzz763_2b),
    ("fuzz763_2a", fuzz763_2a),
    ("fuzz763_5a", fuzz763_5a),
    ("fuzz763_3a", fuzz763_3a),
    ("fuzz763_1a", fuzz763_1a),
    ("fuzz763_1b", fuzz763_1b),
    ("fuzz763_1c", fuzz763_1c),
    ("fuzz763_2", fuzz763_2),
    ("fuzz763_5", fuzz763_5),
    ("fuzz763_3", fuzz763_3),
    ("fuzz763_4", fuzz763_4),
    ("fuzz763_9", fuzz763_9),
    ("fuzz1450_1", fuzz1450_1),
    ("fuzz1450_0", fuzz1450_0),
    ("bug597926_0", bug597926_0),
    ("fuzz535151", fuzz535151),
    ("fuzz753_91", fuzz753_91),
    ("fuzz714", fuzz714),
    ("fuzz487a", fuzz487a),
    ("fuzz433", fuzz433),
    ("fuzz1", fuzz1),
    ("fuzz487b", fuzz487b),
    ("fuzz433b", fuzz433b),
    ("bufferOverflow", buffer_overflow),
];

// Port of: tests/PathOpsOpTest.cpp#L12552-L12554 (chrome/m156), the `repTests[]` table
const REP_TESTS: &[(&str, TestFn)] = &[("fuzz763_5a", fuzz763_5a)];

/// `RunTestSet` as `PathOpsOp`, `PathOpsFailOp` and `PathOpsRepOp` call it: `firstTest`,
/// `skipTest` and `stopTest` are null and `reverse` is false, so each test runs once in order.
/// The `subTests[]` set and the reverse order are only reached through `runSubTests` and
/// `runReverse`, which are false in Skia, so they are not ported.
// Port of: tests/PathOpsExtendedTest.cpp#L666-L700 (chrome/m156), null-argument path only
fn run_test_set(reporter: &mut Reporter, tests: &[(&str, TestFn)]) {
    for &(name, test) in tests {
        test(reporter, name);
    }
}

// Port of: tests/PathOpsOpTest.cpp#L9518-L9526 (chrome/m156)
def_test!(PathOpsOp, |reporter| {
    run_test_set(reporter, TESTS);
});

// Port of: tests/PathOpsOpTest.cpp#L12548-L12550 (chrome/m156)
def_test!(PathOpsFailOp, |reporter| {
    run_test_set(reporter, FAIL_TESTS);
});

// Port of: tests/PathOpsOpTest.cpp#L12556-L12562 (chrome/m156)
def_test!(PathOpsRepOp, |reporter| {
    run_test_set(reporter, REP_TESTS);
});

// Port of: tests/PathOpsOpTest.cpp#L9108-L9134 (chrome/m156)
def_test!(bug_513820666, |_reporter| {
    let mut path = PathBuilder::new();
    for i in 0_u8..5 {
        let off = f32::from(i) * 0.0001_f32;
        path.set_fill_type(PathFillType::EvenOdd);
        path.move_to((f32::from_bits(0x43b40000) + off, f32::from_bits(0xcf000000)));
        path.cubic_to(
            (f32::from_bits(0x4e0d628f), f32::from_bits(0xceffffff)),
            (f32::from_bits(0x4e800003), f32::from_bits(0xcec6b143)),
            (f32::from_bits(0x4e800002), f32::from_bits(0xce7ffffc)),
        );
        path.cubic_to(
            (f32::from_bits(0x4e800002), f32::from_bits(0xcde53aee)),
            (f32::from_bits(0x4e0d6292), f32::from_bits(0xc307820e)),
            (f32::from_bits(0x44627d00), f32::from_bits(0x437ffff2)),
        );
        path.line_to((f32::from_bits(0x444bf3bc), f32::from_bits(0x4460537e)));
        path.line_to((f32::from_bits(0x43553abd), f32::from_bits(0x440f3cbd)));
        path.line_to((f32::from_bits(0x42000000), f32::from_bits(0x41800000)));
        path.line_to((f32::from_bits(0x42c80000), f32::from_bits(0x44000000)));
        path.line_to((f32::from_bits(0x43553abd), f32::from_bits(0x440f3cbd)));
        path.line_to((f32::from_bits(0x43b40000), f32::from_bits(0x44800000)));
        path.line_to((f32::from_bits(0x43b40000), f32::from_bits(0x45816000)));

        path.set_fill_type(PathFillType::Winding);
        path.move_to((f32::from_bits(0x42fe0000) + off, f32::from_bits(0x43a08000)));
        path.line_to((f32::from_bits(0x45d5c000), f32::from_bits(0x43870000)));
        path.line_to((f32::from_bits(0xd0a00000), f32::from_bits(0x4cbebc20)));
        path.line_to((f32::from_bits(0x451f7000), f32::from_bits(0x42800000)));
        path.line_to((f32::from_bits(0x42fe0000), f32::from_bits(0x43a08000)));
        path.close();
    }
    // This caused a corruption/assert w/o the fix
    let _ = simplify(&path.detach());
});
