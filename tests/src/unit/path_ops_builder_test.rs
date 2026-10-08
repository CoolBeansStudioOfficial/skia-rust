// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsBuilderTest.cpp (chrome/m156)

#![cfg(test)]
// The float literals are the exact values of Skia's test data (see path_ops_op_test.rs).
#![allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::too_many_lines
)]

use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::rect::Rect;
use skia_rust_pathops::OpBuilder;
use skia_rust_pathops::path_op::PathOp;

use crate::unit::path_ops_extended_test::compare_paths;
use crate::{def_test, reporter_assert};

// Port of: tests/PathOpsBuilderTest.cpp#L17-L88 (chrome/m156)
def_test!(PathOpsBuilder, |reporter| {
    let mut builder = OpBuilder::new();
    let mut result = builder.resolve();
    reporter_assert!(reporter, result.is_some());
    reporter_assert!(reporter, result.as_ref().is_some_and(Path::is_empty));
    builder.add(&result.clone().unwrap_or_default(), PathOp::Difference);
    result = builder.resolve();
    reporter_assert!(reporter, result.is_some());
    reporter_assert!(reporter, result.as_ref().is_some_and(Path::is_empty));
    builder.add(&result.clone().unwrap_or_default(), PathOp::Union);
    result = builder.resolve();
    reporter_assert!(reporter, result.is_some());
    reporter_assert!(reporter, result.as_ref().is_some_and(Path::is_empty));
    let rect_path = Path::rect(Rect::new(0.0, 1.0, 2.0, 3.0), PathDirection::CW)
        .with_fill_type(PathFillType::EvenOdd);
    builder.add(&rect_path, PathOp::Union);
    result = builder.resolve();
    reporter_assert!(reporter, result.is_some());
    let result_path = result.clone().unwrap_or_default();
    let is_rect = result_path.is_rect();
    reporter_assert!(reporter, is_rect.is_some());
    let (_, closed, dir) = is_rect.unwrap_or((Rect::default(), false, PathDirection::CW));
    reporter_assert!(reporter, closed);
    reporter_assert!(reporter, dir == PathDirection::CCW);
    let pixel_diff = compare_paths(&rect_path, &result_path);
    reporter_assert!(reporter, pixel_diff);
    let rect_path = Path::rect(Rect::new(0.0, 1.0, 2.0, 3.0), PathDirection::CCW)
        .with_fill_type(PathFillType::EvenOdd);
    builder.add(&rect_path, PathOp::Union);
    result = builder.resolve();
    reporter_assert!(reporter, result.is_some());
    let result_path = result.clone().unwrap_or_default();
    let is_rect = result_path.is_rect();
    reporter_assert!(reporter, is_rect.is_some());
    let (_, closed, dir) = is_rect.unwrap_or((Rect::default(), false, PathDirection::CW));
    reporter_assert!(reporter, closed);
    reporter_assert!(reporter, dir == PathDirection::CCW);
    reporter_assert!(reporter, Some(&rect_path) == result.as_ref());
    builder.add(&rect_path, PathOp::Difference);
    result = builder.resolve();
    reporter_assert!(reporter, result.is_some());
    reporter_assert!(reporter, result.as_ref().is_some_and(Path::is_empty));
    let rect2 = Path::rect(Rect::new(2.0, 1.0, 4.0, 3.0), PathDirection::CW);
    let rect3 = Path::rect(Rect::new(4.0, 1.0, 5.0, 3.0), PathDirection::CCW);
    builder.add(&rect_path, PathOp::Union);
    builder.add(&rect2, PathOp::Union);
    builder.add(&rect3, PathOp::Union);
    result = builder.resolve();
    reporter_assert!(reporter, result.is_some());
    let result_path = result.clone().unwrap_or_default();
    let is_rect = result_path.is_rect();
    reporter_assert!(reporter, is_rect.is_some());
    let (_, closed, _) = is_rect.unwrap_or((Rect::default(), false, PathDirection::CW));
    reporter_assert!(reporter, closed);
    let expected = Rect::new(0.0, 1.0, 5.0, 3.0);
    reporter_assert!(reporter, *result_path.bounds() == expected);
    let circle1 = Path::circle((5.0, 6.0), 4.0, PathDirection::CW);
    let circle2 = Path::circle((7.0, 4.0), 8.0, PathDirection::CCW);
    let circle3 = Path::circle((6.0, 5.0), 6.0, PathDirection::CW);
    let mut op_compare =
        skia_rust_pathops::op(&circle1, &circle2, PathOp::Union).unwrap_or_default();
    if let Some(res) = skia_rust_pathops::op(&op_compare, &circle3, PathOp::Difference) {
        op_compare = res;
    }
    builder.add(&circle1, PathOp::Union);
    builder.add(&circle2, PathOp::Union);
    builder.add(&circle3, PathOp::Difference);
    result = builder.resolve();
    reporter_assert!(reporter, result.is_some());
    let pixel_diff = compare_paths(&op_compare, &result.unwrap_or_default());
    reporter_assert!(reporter, pixel_diff);
});

// Port of: tests/PathOpsBuilderTest.cpp#L90-L110 (chrome/m156)
def_test!(BuilderIssue3838, |reporter| {
    let path = PathBuilder::new()
        .move_to((200.0, 170.0))
        .line_to((220.0, 170.0))
        .line_to((220.0, 230.0))
        .line_to((240.0, 230.0))
        .line_to((240.0, 210.0))
        .line_to((180.0, 210.0))
        .line_to((180.0, 190.0))
        .line_to((260.0, 190.0))
        .line_to((260.0, 250.0))
        .line_to((200.0, 250.0))
        .line_to((200.0, 170.0))
        .close()
        .detach();
    let mut builder = OpBuilder::new();
    builder.add(&path, PathOp::Union);
    let path2 = builder.resolve();
    let pixel_diff = compare_paths(&path, &path2.unwrap_or_default());
    reporter_assert!(reporter, pixel_diff);
});

// Port of: tests/PathOpsBuilderTest.cpp#L112-L122 (chrome/m156)
def_test!(BuilderIssue3838_2, |reporter| {
    let path = Path::circle((100.0, 100.0), 50.0, None);
    let mut builder = OpBuilder::new();
    builder.add(&path, PathOp::Union);
    builder.add(&path, PathOp::Union);
    let result = builder.resolve();
    let pixel_diff = compare_paths(&path, &result.unwrap_or_default());
    reporter_assert!(reporter, pixel_diff);
});

// Port of: tests/PathOpsBuilderTest.cpp#L124-L143 (chrome/m156)
def_test!(BuilderIssue3838_3, |reporter| {
    let path = PathBuilder::new()
        .move_to((40.0, 10.0))
        .line_to((60.0, 10.0))
        .line_to((60.0, 30.0))
        .line_to((40.0, 30.0))
        .line_to((40.0, 10.0))
        .move_to((41.0, 11.0))
        .line_to((41.0, 29.0))
        .line_to((59.0, 29.0))
        .line_to((59.0, 11.0))
        .line_to((41.0, 11.0))
        .detach();
    let mut builder = OpBuilder::new();
    builder.add(&path, PathOp::Union);
    let result = builder.resolve();
    let pixel_diff = compare_paths(&path, &result.unwrap_or_default());
    reporter_assert!(reporter, pixel_diff);
});

// Port of: tests/PathOpsBuilderTest.cpp#L145-L158 (chrome/m156)
def_test!(BuilderIssue502792_2, |_reporter| {
    let path = PathBuilder::new_with_fill_type(PathFillType::Winding)
        .add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None)
        .add_rect(Rect::new(2.0, 2.0, 3.0, 3.0), PathDirection::CW, None)
        .detach();
    let path_b = PathBuilder::new_with_fill_type(PathFillType::EvenOdd)
        .add_rect(Rect::new(3.0, 3.0, 4.0, 4.0), PathDirection::CW, None)
        .add_rect(Rect::new(3.0, 3.0, 4.0, 4.0), PathDirection::CW, None)
        .detach();
    let mut builder = OpBuilder::new();
    builder.add(&path, PathOp::Union);
    builder.add(&path_b, PathOp::Difference);
    let _ = builder.resolve();
});

// Port of: tests/PathOpsBuilderTest.cpp#L160-L277 (chrome/m156)
def_test!(Fuzz846, |_reporter| {
    let mut clip_rect = PathBuilder::new();
    let clip_circle = Path::circle((60.0, 60.0), 50.0, None);
    let mut inner = PathBuilder::new();
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 0.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 0.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 32668.0, 30.0 + 0.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 18446744073709551615.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 255.0, 10.0 + 100.0, 255.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(0.0, 0.0, 0.0 + 100.0, 0.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 4294967236.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    clip_rect.add_path(&inner.detach(), None);
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 0.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 0.0, 30.0 + 0.18093252719929986369568203),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 32668.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 18446744073709551615.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 255.0, 10.0 + 100.0, 255.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(2147483649.0, 30.0, 2147483649.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    clip_rect.add_path(&inner.detach(), None);
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 0.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 0.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 32668.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 18446744073709551615.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 255.0, 10.0 + 100.0, 255.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(2147483649.0, 30.0, 2147483649.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 2879753595.0, 10.0 + 100.0, 30.0 + 2879753595.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    clip_rect.add_path(&inner.detach(), None);
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 0.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 32668.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 18446744073709551615.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 255.0, 10.0 + 100.0, 255.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(2147483649.0, 30.0, 2147483649.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 4294967236.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 4294967236.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 4294967236.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 4294967236.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(10.0, 30.0, 10.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    inner.add_rect(
        Rect::new(757798030.0, 30.0, 757798030.0 + 100.0, 30.0 + 60.0),
        PathDirection::CW,
        None,
    );
    clip_rect.add_path(&inner.detach(), None);
    let mut builder = OpBuilder::new();
    builder.add(&clip_circle, PathOp::Union);
    builder.add(&clip_rect.detach(), PathOp::Difference);
    let _ = builder.resolve();
});

// Port of: tests/PathOpsBuilderTest.cpp#L279-L309 (chrome/m156)
def_test!(Issue569540, |_reporter| {
    let path1 = PathBuilder::new()
        .move_to((5.0, -225.0))
        .line_to((-225.0, 7425.0))
        .line_to((7425.0, 7425.0))
        .line_to((7425.0, -225.0))
        .line_to((-225.0, -225.0))
        .line_to((5.0, -225.0))
        .close()
        .detach();
    let path2 = PathBuilder::new()
        .move_to((5940.0, 2790.0))
        .line_to((5940.0, 2160.0))
        .line_to((5970.0, 1980.0))
        .line_to((5688.0, 773669888.0))
        .line_to((5688.0, 2160.0))
        .line_to((5688.0, 2430.0))
        .line_to((5400.0, 4590.0))
        .line_to((5220.0, 4590.0))
        .line_to((5220.0, 4920.0))
        .cubic_to(
            (5182.22900390625, 4948.328125),
            (5160.0, 4992.78662109375),
            (5160.0, 5040.00048828125),
        )
        .line_to((5940.0, 2790.0))
        .close()
        .detach();
    let mut builder = OpBuilder::new();
    builder.add(&path1, PathOp::Union);
    builder.add(&path2, PathOp::Union);
    let _ = builder.resolve();
});

// Port of: tests/PathOpsBuilderTest.cpp#L311-L330 (chrome/m156)
def_test!(SkOpBuilderFuzz665, |_reporter| {
    let path = PathBuilder::new_with_fill_type(PathFillType::EvenOdd)
        .move_to((f32::from_bits(0xcc4264a7), f32::from_bits(0x4bb12e50)))
        .line_to((f32::from_bits(0xcc4264b0), f32::from_bits(0x4bb12e48)))
        .line_to((f32::from_bits(0xcc4264a7), f32::from_bits(0x4bb12e50)))
        .close()
        .detach();
    let path1 = path.clone();
    let path = PathBuilder::new_with_fill_type(PathFillType::Winding)
        .move_to((f32::from_bits(0x43213333), f32::from_bits(0x43080000)))
        .line_to((f32::from_bits(0x43038000), f32::from_bits(0x43080000)))
        .cubic_to(
            (f32::from_bits(0x43038000), f32::from_bits(0x42f00000)),
            (f32::from_bits(0x42f16666), f32::from_bits(0x42d53333)),
            (f32::from_bits(0x42d3cccd), f32::from_bits(0x42cd6666)),
        )
        .line_to((f32::from_bits(0x42e33333), f32::from_bits(0x42940000)))
        .detach();
    let path2 = path.clone();
    let mut builder = OpBuilder::new();
    builder.add(&path1, PathOp::Union);
    builder.add(&path2, PathOp::Union);
    let _ = builder.resolve();
});

// Port of: tests/PathOpsBuilderTest.cpp#L332-L353 (chrome/m156)
def_test!(SkOpBuilder618991, |_reporter| {
    let path0 = PathBuilder::new()
        .move_to((140.0, 40.0))
        .line_to((200.0, 210.0))
        .line_to((40.0, 100.0))
        .line_to((2.22223e+07, 2.22222e+14))
        .line_to((2.22223e+07, 2.22222e+14))
        .detach();
    let path1 = PathBuilder::new()
        .move_to((160.0, 60.0))
        .line_to((220.0, 230.0))
        .line_to((60.0, 120.0))
        .line_to((2.22223e+07, 2.22222e+14))
        .line_to((2.22223e+07, 2.22222e+14))
        .detach();
    let mut builder = OpBuilder::new();
    builder.add(&path0, PathOp::Union);
    builder.add(&path1, PathOp::Union);
    let _ = builder.resolve();
});

// Port of: tests/PathOpsBuilderTest.cpp#L355-L372 (chrome/m156)
def_test!(SkOpBuilderKFuzz1, |_reporter| {
    let path = PathBuilder::new()
        .move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)))
        .line_to((f32::from_bits(0x39008001), f32::from_bits(0xd31fbc1d)))
        .conic_to(
            (f32::from_bits(0x246a205a), f32::from_bits(0x0080d3fb)),
            (f32::from_bits(0xce000001), f32::from_bits(0x04d31fbc)),
            f32::from_bits(0x57a82c00),
        )
        .detach();
    let path0 = path.clone();
    let path = PathBuilder::new()
        .move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)))
        .cubic_to(
            (f32::from_bits(0x80d3f924), f32::from_bits(0xcecece4f)),
            (f32::from_bits(0xcececece), f32::from_bits(0xcececece)),
            (f32::from_bits(0x9a9a9ace), f32::from_bits(0x9a9a9a9a)),
        )
        .move_to((f32::from_bits(0x9a9a019a), f32::from_bits(0xa59a9a9a)))
        .detach();
    let path1 = path.clone();
    let mut builder = OpBuilder::new();
    builder.add(&path0, PathOp::Union);
    builder.add(&path1, PathOp::Union);
    let _ = builder.resolve();
});

// Port of: tests/PathOpsBuilderTest.cpp#L374-L397 (chrome/m156)
def_test!(PathOpsBuilder_UnionAbuttingRotatedRects, |reporter| {
    let path1 = PathBuilder::new()
        .move_to((40005.0, -40005.0))
        .line_to((68580.0, -68580.0))
        .line_to((354330.0, 217170.0))
        .line_to((325755.0, 245745.0))
        .close()
        .detach();
    let path2 = PathBuilder::new()
        .move_to((11430.03125, -11430.03125))
        .line_to((40004.96875, -40004.96875))
        .line_to((325754.96875, 245745.03125))
        .line_to((297180.03125, 274319.96875))
        .close()
        .detach();
    let mut builder = OpBuilder::new();
    builder.add(&path1, PathOp::Union);
    builder.add(&path2, PathOp::Union);
    let result = builder.resolve();
    reporter_assert!(reporter, result.is_some());
    reporter_assert!(
        reporter,
        result.as_ref().map(|p| *p.bounds())
            == Some(Rect::new(11430.03125, -68580.0, 354330.0, 274319.96875))
    );
});
