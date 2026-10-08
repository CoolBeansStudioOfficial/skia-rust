// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsAsWindingTest.cpp (chrome/m156)

#![cfg(test)]
// The float literals are the exact values of Skia's test data (see path_ops_op_test.rs).
#![allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::manual_midpoint,
    clippy::cast_precision_loss
)]

use skia_rust_core::path::{Path, Verb};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_priv;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::utils::parse_path;
use skia_rust_pathops::as_winding;
use skia_rust_pathops::path_op::PathOp;
use skia_rust_pathops::{op, simplify};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/PathOpsAsWindingTest.cpp#L23-L86 (chrome/m156)
fn build_squircle(verb: Verb, rect: Rect, dir: PathDirection) -> Path {
    let mut builder = PathBuilder::new();
    let mut reverse = dir == PathDirection::CCW;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    let center_x = (rect.left + rect.right) * 0.5;
    let center_y = (rect.top + rect.bottom) * 0.5;
    match verb {
        Verb::Line => {
            builder.add_rect(rect, dir, None);
            reverse = false;
        }
        Verb::Quad => {
            builder.move_to((center_x, rect.top));
            builder.quad_to((rect.right, rect.top), (rect.right, center_y));
            builder.quad_to((rect.right, rect.bottom), (center_x, rect.bottom));
            builder.quad_to((rect.left, rect.bottom), (rect.left, center_y));
            builder.quad_to((rect.left, rect.top), (center_x, rect.top));
        }
        Verb::Conic => {
            builder.add_circle((center_x, center_y), width / 2.0, dir);
            reverse = false;
        }
        Verb::Cubic => {
            let a_x14 = rect.left + width * 1.0 / 4.0;
            let a_x34 = rect.left + width * 3.0 / 4.0;
            let a_y14 = rect.top + height * 1.0 / 4.0;
            let a_y34 = rect.top + height * 3.0 / 4.0;
            builder.move_to((center_x, rect.top));
            builder.cubic_to(
                (a_x34, rect.top),
                (rect.right, a_y14),
                (rect.right, center_y),
            );
            builder.cubic_to(
                (rect.right, a_y34),
                (a_x34, rect.bottom),
                (center_x, rect.bottom),
            );
            builder.cubic_to(
                (a_x14, rect.bottom),
                (rect.left, a_y34),
                (rect.left, center_y),
            );
            builder.cubic_to((rect.left, a_y14), (a_x14, rect.top), (center_x, rect.top));
        }
        _ => unreachable!("build_squircle takes line, quad, conic or cubic"),
    }
    let path = builder.detach();
    if reverse {
        path_priv::reverse_add_path(&mut builder, &path);
        return builder.detach();
    }
    path
}

// Port of: tests/PathOpsAsWindingTest.cpp#L64-L137 (chrome/m156)
fn bug12040_1(reporter: &mut Reporter) {
    let path = {
        let mut b = PathBuilder::new_with_fill_type(PathFillType::Winding);
        b.move_to((375.0, -30.0));
        b.cubic_to((578.0, -30.0), (749.0, 176.0), (749.0, 422.0));
        b.cubic_to((749.0, 583.0), (666.0, 706.0), (518.0, 765.0));
        b.line_to((163.0, 611.0));
        b.line_to((163.0, 579.0));
        b.line_to((405.0, 684.0));
        b.cubic_to((551.0, 609.0), (645.0, 468.0), (645.0, 322.0));
        b.cubic_to((645.0, 183.0), (563.0, 82.0), (450.0, 82.0));
        b.cubic_to((303.0, 82.0), (179.0, 249.0), (179.0, 446.0));
        b.cubic_to((179.0, 579.0), (235.0, 689.0), (341.0, 768.0));
        b.line_to((327.0, 786.0));
        b.cubic_to((165.0, 717.0), (56.0, 536.0), (56.0, 335.0));
        b.cubic_to((56.0, 125.0), (192.0, -30.0), (375.0, -30.0));
        b.close();
        b.move_to((214.0, 225.0));
        b.cubic_to((333.0, 248.0), (396.0, 311.0), (396.0, 405.0));
        b.line_to((396.0, 695.0));
        b.line_to((267.0, 641.0));
        b.line_to((267.0, 395.0));
        b.cubic_to((267.0, 324.0), (249.0, 285.0), (201.0, 254.0));
        b.cubic_to((201.0, 254.0), (214.0, 225.0), (214.0, 225.0));
        b.close();
        b.move_to((682.0, -106.0));
        b.line_to((832.0, 12.0));
        b.line_to((813.0, 33.0));
        b.line_to((772.0, 0.0));
        b.cubic_to((716.0, 29.0), (668.0, 76.0), (628.0, 140.0));
        b.line_to((527.0, 44.0));
        b.cubic_to((575.0, -26.0), (628.0, -77.0), (682.0, -106.0));
        b.close();
        b.move_to((450.0, 59.0));
        b.line_to((480.0, 59.0));
        b.line_to((480.0, 678.0));
        b.line_to((450.0, 678.0));
        b.cubic_to((450.0, 678.0), (450.0, 59.0), (450.0, 59.0));
        b.close();
        b.move_to((463.0, 374.0));
        b.line_to((633.0, 459.0));
        b.line_to((633.0, 490.0));
        b.line_to((463.0, 406.0));
        b.cubic_to((463.0, 406.0), (463.0, 374.0), (463.0, 374.0));
        b.close();
        b.move_to((463.0, 269.0));
        b.line_to((667.0, 372.0));
        b.line_to((667.0, 403.0));
        b.line_to((463.0, 301.0));
        b.cubic_to((463.0, 301.0), (463.0, 269.0), (463.0, 269.0));
        b.close();
        b.detach()
    };
    let path2 = {
        let mut b = PathBuilder::new_with_fill_type(PathFillType::Winding);
        b.move_to((-83.5464, 188.0));
        b.cubic_to(
            (-83.5464, 184.285),
            (-84.8599, 181.114),
            (-87.4868, 178.487),
        );
        b.cubic_to((-90.1138, 175.86), (-93.2849, 174.546), (-97.0, 174.546));
        b.cubic_to((-100.715, 174.546), (-103.886, 175.86), (-106.513, 178.487));
        b.cubic_to((-109.14, 181.114), (-110.454, 184.285), (-110.454, 188.0));
        b.cubic_to((-110.454, 191.715), (-109.14, 194.886), (-106.513, 197.513));
        b.cubic_to((-103.886, 200.14), (-100.715, 201.454), (-97.0, 201.454));
        b.cubic_to((-93.2849, 201.454), (-90.1138, 200.14), (-87.4868, 197.513));
        b.cubic_to((-84.8599, 194.886), (-83.5464, 191.715), (-83.5464, 188.0));
        b.close();
        b.detach()
    };
    let op_result = op(&path, &path2, PathOp::Difference).unwrap_or_default();
    let winding = as_winding(&op_result);
    reporter_assert!(reporter, winding.is_some());
    let winding = winding.unwrap_or_default();
    reporter_assert!(reporter, winding.fill_type() == PathFillType::Winding);
    let difference = op(&winding, &op_result, PathOp::Xor).unwrap_or_default();
    reporter_assert!(reporter, difference.is_empty());
}

// Port of: tests/PathOpsAsWindingTest.cpp#L139-L217 (chrome/m156)
fn bug12040_2(reporter: &mut Reporter) {
    let path = {
        let mut b = PathBuilder::new_with_fill_type(PathFillType::Winding);
        b.move_to((375.0, -30.0));
        b.cubic_to((578.0, -30.0), (749.0, 176.0), (749.0, 422.0));
        b.cubic_to((749.0, 583.0), (666.0, 706.0), (518.0, 765.0));
        b.line_to((163.0, 611.0));
        b.line_to((163.0, 579.0));
        b.line_to((405.0, 684.0));
        b.cubic_to((551.0, 609.0), (645.0, 468.0), (645.0, 322.0));
        b.cubic_to((645.0, 183.0), (563.0, 82.0), (450.0, 82.0));
        b.cubic_to((303.0, 82.0), (179.0, 249.0), (179.0, 446.0));
        b.cubic_to((179.0, 579.0), (235.0, 689.0), (341.0, 768.0));
        b.line_to((327.0, 786.0));
        b.cubic_to((165.0, 717.0), (56.0, 536.0), (56.0, 335.0));
        b.cubic_to((56.0, 125.0), (192.0, -30.0), (375.0, -30.0));
        b.close();
        b.move_to((214.0, 225.0));
        b.cubic_to((333.0, 248.0), (396.0, 311.0), (396.0, 405.0));
        b.line_to((396.0, 695.0));
        b.line_to((267.0, 641.0));
        b.line_to((267.0, 395.0));
        b.cubic_to((267.0, 324.0), (249.0, 285.0), (201.0, 254.0));
        b.cubic_to((201.0, 254.0), (214.0, 225.0), (214.0, 225.0));
        b.close();
        b.move_to((682.0, -106.0));
        b.line_to((832.0, 12.0));
        b.line_to((813.0, 33.0));
        b.line_to((772.0, 0.0));
        b.cubic_to((716.0, 29.0), (668.0, 76.0), (628.0, 140.0));
        b.line_to((527.0, 44.0));
        b.cubic_to((575.0, -26.0), (628.0, -77.0), (682.0, -106.0));
        b.close();
        b.move_to((450.0, 59.0));
        b.line_to((480.0, 59.0));
        b.line_to((480.0, 678.0));
        b.line_to((450.0, 678.0));
        b.cubic_to((450.0, 678.0), (450.0, 59.0), (450.0, 59.0));
        b.close();
        b.move_to((463.0, 374.0));
        b.line_to((633.0, 459.0));
        b.line_to((633.0, 490.0));
        b.line_to((463.0, 406.0));
        b.cubic_to((463.0, 406.0), (463.0, 374.0), (463.0, 374.0));
        b.close();
        b.move_to((463.0, 269.0));
        b.line_to((667.0, 372.0));
        b.line_to((667.0, 403.0));
        b.line_to((463.0, 301.0));
        b.cubic_to((463.0, 301.0), (463.0, 269.0), (463.0, 269.0));
        b.close();
        b.detach()
    };
    let path2 = {
        let mut b = PathBuilder::new_with_fill_type(PathFillType::Winding);
        b.move_to((269.134, 71.3392));
        b.cubic_to((269.134, 67.6241), (267.82, 64.453), (265.193, 61.826));
        b.cubic_to((262.566, 59.1991), (259.395, 57.8856), (255.68, 57.8856));
        b.cubic_to((251.965, 57.8856), (248.794, 59.1991), (246.167, 61.826));
        b.cubic_to((243.54, 64.453), (242.226, 67.6241), (242.226, 71.3392));
        b.cubic_to((242.226, 75.0543), (243.54, 78.2255), (246.167, 80.8524));
        b.cubic_to((248.794, 83.4794), (251.965, 84.7928), (255.68, 84.7928));
        b.cubic_to((259.395, 84.7928), (262.566, 83.4794), (265.193, 80.8524));
        b.cubic_to((267.82, 78.2255), (269.134, 75.0543), (269.134, 71.3392));
        b.close();
        b.detach()
    };
    let op_result = op(&path, &path2, PathOp::Difference).unwrap_or_default();
    let winding = as_winding(&op_result);
    reporter_assert!(reporter, winding.is_some());
    let winding = winding.unwrap_or_default();
    reporter_assert!(reporter, winding.fill_type() == PathFillType::Winding);
    let difference = op(&winding, &op_result, PathOp::Xor).unwrap_or_default();
    reporter_assert!(reporter, difference.is_empty());
}

// Port of: tests/PathOpsAsWindingTest.cpp#L219-L293 (chrome/m156)
fn bug12040_3(reporter: &mut Reporter) {
    let path = {
        let mut b = PathBuilder::new_with_fill_type(PathFillType::Winding);
        b.move_to((375.0, -30.0));
        b.cubic_to((578.0, -30.0), (749.0, 176.0), (749.0, 422.0));
        b.cubic_to((749.0, 583.0), (666.0, 706.0), (518.0, 765.0));
        b.line_to((163.0, 611.0));
        b.line_to((163.0, 579.0));
        b.line_to((405.0, 684.0));
        b.cubic_to((551.0, 609.0), (645.0, 468.0), (645.0, 322.0));
        b.cubic_to((645.0, 183.0), (563.0, 82.0), (450.0, 82.0));
        b.cubic_to((303.0, 82.0), (179.0, 249.0), (179.0, 446.0));
        b.cubic_to((179.0, 579.0), (235.0, 689.0), (341.0, 768.0));
        b.line_to((327.0, 786.0));
        b.cubic_to((165.0, 717.0), (56.0, 536.0), (56.0, 335.0));
        b.cubic_to((56.0, 125.0), (192.0, -30.0), (375.0, -30.0));
        b.close();
        b.move_to((214.0, 225.0));
        b.cubic_to((333.0, 248.0), (396.0, 311.0), (396.0, 405.0));
        b.line_to((396.0, 695.0));
        b.line_to((267.0, 641.0));
        b.line_to((267.0, 395.0));
        b.cubic_to((267.0, 324.0), (249.0, 285.0), (201.0, 254.0));
        b.cubic_to((201.0, 254.0), (214.0, 225.0), (214.0, 225.0));
        b.close();
        b.move_to((682.0, -106.0));
        b.line_to((832.0, 12.0));
        b.line_to((813.0, 33.0));
        b.line_to((772.0, 0.0));
        b.cubic_to((716.0, 29.0), (668.0, 76.0), (628.0, 140.0));
        b.line_to((527.0, 44.0));
        b.cubic_to((575.0, -26.0), (628.0, -77.0), (682.0, -106.0));
        b.close();
        b.move_to((450.0, 59.0));
        b.line_to((480.0, 59.0));
        b.line_to((480.0, 678.0));
        b.line_to((450.0, 678.0));
        b.cubic_to((450.0, 678.0), (450.0, 59.0), (450.0, 59.0));
        b.close();
        b.move_to((463.0, 374.0));
        b.line_to((633.0, 459.0));
        b.line_to((633.0, 490.0));
        b.line_to((463.0, 406.0));
        b.cubic_to((463.0, 406.0), (463.0, 374.0), (463.0, 374.0));
        b.close();
        b.move_to((463.0, 269.0));
        b.line_to((667.0, 372.0));
        b.line_to((667.0, 403.0));
        b.line_to((463.0, 301.0));
        b.cubic_to((463.0, 301.0), (463.0, 269.0), (463.0, 269.0));
        b.close();
        b.detach()
    };
    let path2 = {
        let mut b = PathBuilder::new_with_fill_type(PathFillType::Winding);
        b.move_to((492.041, 525.339));
        b.cubic_to((492.041, 521.624), (490.727, 518.453), (488.1, 515.826));
        b.cubic_to((485.473, 513.199), (482.302, 511.886), (478.587, 511.886));
        b.cubic_to((474.872, 511.886), (471.701, 513.199), (469.074, 515.826));
        b.cubic_to((466.447, 518.453), (465.134, 521.624), (465.134, 525.339));
        b.cubic_to((465.134, 529.054), (466.447, 532.226), (469.074, 534.853));
        b.cubic_to((471.701, 537.479), (474.872, 538.793), (478.587, 538.793));
        b.cubic_to((482.302, 538.793), (485.473, 537.479), (488.1, 534.853));
        b.cubic_to((490.727, 532.226), (492.041, 529.054), (492.041, 525.339));
        b.close();
        b.detach()
    };
    let op_result = op(&path, &path2, PathOp::Difference).unwrap_or_default();
    let winding = as_winding(&op_result);
    reporter_assert!(reporter, winding.is_some());
    let winding = winding.unwrap_or_default();
    reporter_assert!(reporter, winding.fill_type() == PathFillType::Winding);
    let difference = op(&winding, &op_result, PathOp::Xor).unwrap_or_default();
    reporter_assert!(reporter, difference.is_empty());
}

// Port of: tests/PathOpsAsWindingTest.cpp#L295-L321 (chrome/m156)
fn bug12040_4(reporter: &mut Reporter) {
    for move_x_i in 199..=201 {
        let move_x = move_x_i as f32;
        for move_y_i in 299..=301 {
            let move_y = move_y_i as f32;
            for line_x_i in 199..=201 {
                let line_x = line_x_i as f32;
                for line_y_i in 199..=201 {
                    let line_y = line_y_i as f32;
                    let path = Path::circle((250.0, 250.0), 150.0, None);
                    let path2 = {
                        let mut b = PathBuilder::new_with_fill_type(PathFillType::Winding);
                        b.move_to((move_x, move_y));
                        b.line_to((line_x, line_y));
                        b.line_to((300.0, 300.0));
                        b.close();
                        b.detach()
                    };
                    let op_result = op(&path, &path2, PathOp::Difference).unwrap_or_default();
                    let winding = as_winding(&op_result);
                    reporter_assert!(reporter, winding.is_some());
                    let winding = winding.unwrap_or_default();
                    reporter_assert!(reporter, winding.fill_type() == PathFillType::Winding);
                    let difference = op(&winding, &op_result, PathOp::Xor).unwrap_or_default();
                    reporter_assert!(reporter, difference.is_empty());
                }
            }
        }
    }
}

// Port of: tests/PathOpsAsWindingTest.cpp#L323-L349 (chrome/m156)
fn bug12040_5(reporter: &mut Reporter) {
    for move_x_i in 199..=201 {
        let move_x = move_x_i as f32;
        for move_y_i in 299..=301 {
            let move_y = move_y_i as f32;
            for line_x_i in 199..=201 {
                let line_x = line_x_i as f32;
                for line_y_i in 199..=201 {
                    let line_y = line_y_i as f32;
                    let path = Path::rect(Rect::new(100.0, 100.0, 400.0, 400.0), None);
                    let path2 = {
                        let mut b = PathBuilder::new_with_fill_type(PathFillType::Winding);
                        b.move_to((move_x, move_y));
                        b.line_to((line_x, line_y));
                        b.line_to((300.0, 300.0));
                        b.close();
                        b.detach()
                    };
                    let op_result = op(&path, &path2, PathOp::Difference).unwrap_or_default();
                    let winding = as_winding(&op_result);
                    reporter_assert!(reporter, winding.is_some());
                    let winding = winding.unwrap_or_default();
                    reporter_assert!(reporter, winding.fill_type() == PathFillType::Winding);
                    let difference = op(&winding, &op_result, PathOp::Xor).unwrap_or_default();
                    reporter_assert!(reporter, difference.is_empty());
                }
            }
        }
    }
}

// Port of: tests/PathOpsAsWindingTest.cpp#L351-L369 (chrome/m156)
fn bug13496_1(reporter: &mut Reporter) {
    let path = parse_path::from_svg("M5.93 -3.12C5.93 -5.03 4.73 -6.06 3.5 -6.06C2.67 -6.06 1.98 -5.59 1.76 -5.34L1.67 -5.93L0.75 -5.93L0.75 2.23L1.87 2.04L1.87 -0.12C2.12 -0.03 2.62 0.07 3.18 0.07C4.57 0.07 5.93 -1.06 5.93 -3.12ZM4.81 -3.09C4.81 -1.51 4.18 -0.85 3.17 -0.85C2.57 -0.85 2.15 -0.98 1.87 -1.12L1.87 -4.15C2.34 -4.73 2.75 -5.09 3.42 -5.09C4.31 -5.09 4.81 -4.46 4.81 -3.09Z").unwrap_or_default();
    let simplified_path = simplify(&path).unwrap_or_default();
    let winding_path = as_winding(&simplified_path);
    reporter_assert!(reporter, winding_path.is_some());
    let winding_path = winding_path.unwrap_or_default();
    reporter_assert!(reporter, winding_path.fill_type() == PathFillType::Winding);
    let difference = op(&winding_path, &simplified_path, PathOp::Xor).unwrap_or_default();
    reporter_assert!(reporter, difference.is_empty());
}

// Port of: tests/PathOpsAsWindingTest.cpp#L371-L394 (chrome/m156)
fn bug13496_2(reporter: &mut Reporter) {
    let path = parse_path::from_svg("M4 0L0 0L0 5L4 4ZM3 3L1 3L1 1L3 1Z").unwrap_or_default();
    let simplified_path = simplify(&path).unwrap_or_default();
    let winding_path = as_winding(&simplified_path);
    reporter_assert!(reporter, winding_path.is_some());
    let winding_path = winding_path.unwrap_or_default();
    reporter_assert!(reporter, winding_path.fill_type() == PathFillType::Winding);
    let difference = op(&winding_path, &simplified_path, PathOp::Xor).unwrap_or_default();
    reporter_assert!(reporter, difference.is_empty());
}

// Port of: tests/PathOpsAsWindingTest.cpp#L396-L419 (chrome/m156)
fn bug13496_3(reporter: &mut Reporter) {
    let path = parse_path::from_svg("M4 0L0 0L0 4L4 4ZM3 3L1 3L1 1L3 1Z").unwrap_or_default();
    let simplified_path = simplify(&path).unwrap_or_default();
    let winding_path = as_winding(&simplified_path);
    reporter_assert!(reporter, winding_path.is_some());
    let winding_path = winding_path.unwrap_or_default();
    reporter_assert!(reporter, winding_path.fill_type() == PathFillType::Winding);
    let difference = op(&winding_path, &simplified_path, PathOp::Xor).unwrap_or_default();
    reporter_assert!(reporter, difference.is_empty());
}

// Port of: tests/PathOpsAsWindingTest.cpp#L421-L569 (chrome/m156)
def_test!(PathOpsAsWinding, |reporter| {
    let mut test = Path::rect(Rect::new(1.0, 2.0, 3.0, 4.0), None);
    // if test is winding
    let mut result = as_winding(&test);
    reporter_assert!(reporter, result.is_some());
    reporter_assert!(reporter, result.as_ref() == Some(&test));
    // if test is empty
    test = Path::new().make_fill_type(PathFillType::EvenOdd);
    result = as_winding(&test);
    reporter_assert!(reporter, result.is_some());
    reporter_assert!(reporter, result.as_ref().is_some_and(Path::is_empty));
    reporter_assert!(
        reporter,
        result
            .as_ref()
            .is_some_and(|r| r.fill_type() == PathFillType::Winding)
    );
    // if test is convex
    test = Path::circle((5.0, 5.0), 10.0, None).make_fill_type(PathFillType::EvenOdd);
    result = as_winding(&test);
    reporter_assert!(reporter, result.is_some());
    reporter_assert!(reporter, result.as_ref().is_some_and(Path::is_convex));
    test = test.make_fill_type(PathFillType::Winding);
    reporter_assert!(reporter, result.as_ref() == Some(&test));
    // if test has infinity
    test = Path::rect(Rect::new(1.0, 2.0, 3.0, f32::INFINITY), None)
        .make_fill_type(PathFillType::EvenOdd);
    result = as_winding(&test);
    reporter_assert!(reporter, result.is_none());
    // if test has only one contour
    let ell = [
        Point::new(0.0, 0.0),
        Point::new(4.0, 0.0),
        Point::new(4.0, 1.0),
        Point::new(1.0, 1.0),
        Point::new(1.0, 4.0),
        Point::new(0.0, 4.0),
    ];
    let mut builder = PathBuilder::new_with_fill_type(PathFillType::EvenOdd);
    builder.add_polygon(&ell, true);
    test = builder.snapshot();
    result = as_winding(&test);
    reporter_assert!(reporter, result.is_some());
    reporter_assert!(reporter, result.as_ref().is_some_and(|r| !r.is_convex()));
    test = test.make_fill_type(PathFillType::Winding);
    reporter_assert!(reporter, result.as_ref() == Some(&test));
    // test two contours that do not overlap or share bounds
    builder.add_rect(Rect::new(5.0, 2.0, 6.0, 3.0), PathDirection::CW, None);
    test = builder.detach();
    result = as_winding(&test);
    reporter_assert!(reporter, result.is_some());
    reporter_assert!(reporter, result.as_ref().is_some_and(|r| !r.is_convex()));
    test = test.make_fill_type(PathFillType::Winding);
    reporter_assert!(reporter, result.as_ref() == Some(&test));
    // test two contours that do not overlap but share bounds
    let mut b = PathBuilder::new_with_fill_type(PathFillType::EvenOdd);
    b.add_polygon(&ell, true);
    b.add_rect(Rect::new(2.0, 2.0, 3.0, 3.0), PathDirection::CW, None);
    test = b.detach();
    result = as_winding(&test);
    reporter_assert!(reporter, result.is_some());
    reporter_assert!(reporter, result.as_ref().is_some_and(|r| !r.is_convex()));
    test = test.make_fill_type(PathFillType::Winding);
    reporter_assert!(reporter, result.as_ref() == Some(&test));
    // test two contours that partially overlap
    let mut b = PathBuilder::new_with_fill_type(PathFillType::EvenOdd);
    b.add_rect(Rect::new(0.0, 0.0, 3.0, 3.0), PathDirection::CW, None);
    b.add_rect(Rect::new(1.0, 1.0, 4.0, 4.0), PathDirection::CW, None);
    test = b.detach();
    result = as_winding(&test);
    reporter_assert!(reporter, result.is_some());
    reporter_assert!(reporter, result.as_ref().is_some_and(|r| !r.is_convex()));
    test = test.make_fill_type(PathFillType::Winding);
    reporter_assert!(reporter, result.as_ref() == Some(&test));
    // test a in b, b in a, cw/ccw
    let rect_a = Rect::new(0.0, 0.0, 3.0, 3.0);
    let rect_b = Rect::new(1.0, 1.0, 2.0, 2.0);
    let rev_bccw = [
        Point::new(1.0, 2.0),
        Point::new(2.0, 2.0),
        Point::new(2.0, 1.0),
        Point::new(1.0, 1.0),
    ];
    let rev_bcw = [
        Point::new(2.0, 1.0),
        Point::new(2.0, 2.0),
        Point::new(1.0, 2.0),
        Point::new(1.0, 1.0),
    ];
    for a_first in [false, true] {
        for dir_a in [PathDirection::CW, PathDirection::CCW] {
            for dir_b in [PathDirection::CW, PathDirection::CCW] {
                builder.reset();
                builder.set_fill_type(PathFillType::EvenOdd);
                if a_first {
                    builder.add_rect(rect_a, dir_a, None);
                    builder.add_rect(rect_b, dir_b, None);
                } else {
                    builder.add_rect(rect_b, dir_b, None);
                    builder.add_rect(rect_a, dir_a, None);
                }
                test = builder.detach();
                result = as_winding(&test);
                reporter_assert!(reporter, result.is_some());
                reporter_assert!(
                    reporter,
                    result
                        .as_ref()
                        .is_some_and(|r| r.fill_type() == PathFillType::Winding)
                );
                if a_first {
                    builder.add_rect(rect_a, dir_a, None);
                }
                if dir_a == dir_b {
                    let pts: &[Point] = if dir_a == PathDirection::CW {
                        &rev_bccw
                    } else {
                        &rev_bcw
                    };
                    builder.add_polygon(pts, true);
                } else {
                    builder.add_rect(rect_b, dir_b, None);
                }
                if !a_first {
                    builder.add_rect(rect_a, dir_a, None);
                }
                test = builder.detach();
                reporter_assert!(reporter, result.as_ref() == Some(&test));
            }
        }
    }
    // Test curve types with donuts. Create a donut with outer and hole in all directions.
    // After converting to winding, all donuts should have a hole in the middle.
    let curves = [Verb::Line, Verb::Quad, Verb::Conic, Verb::Cubic];
    for a_first in [false, true] {
        for dir_a in [PathDirection::CW, PathDirection::CCW] {
            for dir_b in [PathDirection::CW, PathDirection::CCW] {
                for curve_a in curves {
                    let path_a = build_squircle(curve_a, rect_a, dir_a);
                    for curve_b in curves {
                        builder.reset();
                        if a_first {
                            builder.assign_path(&path_a);
                        }
                        builder.add_path(&build_squircle(curve_b, rect_b, dir_b), None);
                        if !a_first {
                            builder.add_path(&path_a, None);
                        }
                        builder.set_fill_type(PathFillType::EvenOdd);
                        test = builder.detach();
                        result = as_winding(&test);
                        reporter_assert!(reporter, result.is_some());
                        let Some(winding) = result.as_ref() else {
                            continue;
                        };
                        reporter_assert!(reporter, winding.fill_type() == PathFillType::Winding);
                        let mut x = rect_a.left - 1.0;
                        while x <= rect_a.right + 1.0 {
                            let mut y = rect_a.top - 1.0;
                            while y <= rect_a.bottom + 1.0 {
                                let even_odd_contains = test.contains((x, y));
                                let winding_contains = winding.contains((x, y));
                                reporter_assert!(reporter, even_odd_contains == winding_contains);
                                y += 1.0;
                            }
                            x += 1.0;
                        }
                    }
                }
            }
        }
    }
    // test https://bugs.chromium.org/p/skia/issues/detail?id=12040
    bug12040_1(reporter);
    bug12040_2(reporter);
    bug12040_3(reporter);
    bug12040_4(reporter);
    bug12040_5(reporter);
    // test https://bugs.chromium.org/p/skia/issues/detail?id=13496
    bug13496_1(reporter);
    bug13496_2(reporter);
    bug13496_3(reporter);
});
