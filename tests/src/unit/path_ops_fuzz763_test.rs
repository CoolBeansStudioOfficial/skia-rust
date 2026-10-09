// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsFuzz763Test.cpp (chrome/m156)

// The float literals below are Skia's fuzz inputs (SkBits2Float bit patterns and decimal values),
// copied as written. The `#if 0..` blocks of the C++ source are evaluated as the C preprocessor
// does (octal literals), and only the active `path.close()` calls are kept.
#![allow(
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::approx_constant,
    clippy::excessive_precision
)]
#![cfg(test)]

use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_pathops::path_op::PathOp;

use crate::unit::path_ops_extended_test::{test_path_op, test_path_op_check};
use crate::{Reporter, def_test};

// Port of: tests/PathOpsFuzz763Test.cpp#L21-L75 (chrome/m156)
fn fuzz763_3084(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new_with_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)));
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    );
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    );
    path.close();
    path.move_to((f32::from_bits(0x421d76c6), f32::from_bits(0x414d1957)));
    path.quad_to(
        (f32::from_bits(0x4229fd05), f32::from_bits(0x413bbdcc)),
        (f32::from_bits(0x4235e9b0), f32::from_bits(0x4152e45d)),
    );
    path.quad_to(
        (f32::from_bits(0x4241d65c), f32::from_bits(0x416a0aee)),
        (f32::from_bits(0x42462d3e), f32::from_bits(0x418e11f4)),
    );
    path.quad_to(
        (f32::from_bits(0x424a8421), f32::from_bits(0x41a71e71)),
        (f32::from_bits(0x4244ba7d), f32::from_bits(0x41bef7c6)),
    );
    path.quad_to(
        (f32::from_bits(0x423ef0da), f32::from_bits(0x41d6d11e)),
        (f32::from_bits(0x42326a9b), f32::from_bits(0x41df7ee4)),
    );
    path.quad_to(
        (f32::from_bits(0x42273b3e), f32::from_bits(0x41e73f0f)),
        (f32::from_bits(0x421c865e), f32::from_bits(0x41ded7e1)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41f0534a)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    );
    path.quad_to(
        (f32::from_bits(0x4209f7d0), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x420f2625), f32::from_bits(0x41cbccd7)),
    );
    path.quad_to(
        (f32::from_bits(0x420ba850), f32::from_bits(0x41c340da)),
        (f32::from_bits(0x4209b422), f32::from_bits(0x41b7f99d)),
    );
    path.quad_to(
        (f32::from_bits(0x42055d40), f32::from_bits(0x419eed20)),
        (f32::from_bits(0x420b26e4), f32::from_bits(0x418713c8)),
    );
    path.quad_to(
        (f32::from_bits(0x4210f088), f32::from_bits(0x415e74e2)),
        (f32::from_bits(0x421d76c6), f32::from_bits(0x414d1957)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42240000), f32::from_bits(0x42040000)));
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L77-L130 (chrome/m156)
fn fuzz763_1823(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)));
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    );
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x405eb92c), f32::from_bits(0x422355aa)),
        (f32::from_bits(0x3eee625a), f32::from_bits(0x4223f3e8)),
    );
    path.quad_to(
        (f32::from_bits(0x3f238500), f32::from_bits(0x4224bba7)),
        (f32::from_bits(0x3f4dcc60), f32::from_bits(0x4225921a)),
    );
    path.quad_to(
        (f32::from_bits(0x4036c5c0), f32::from_bits(0x422ffa87)),
        (f32::from_bits(0x401de138), f32::from_bits(0x423d244e)),
    );
    path.quad_to(
        (f32::from_bits(0x4004fcb0), f32::from_bits(0x424a4e17)),
        (f32::from_bits(0xbf0628a0), f32::from_bits(0x42528342)),
    );
    path.quad_to(
        (f32::from_bits(0xc04810f8), f32::from_bits(0x425ab86c)),
        (f32::from_bits(0xc0cd56bc), f32::from_bits(0x42592a22)),
    );
    path.quad_to(
        (f32::from_bits(0xc11b5280), f32::from_bits(0x42579bda)),
        (f32::from_bits(0xc13c272a), f32::from_bits(0x424d336e)),
    );
    path.quad_to(
        (f32::from_bits(0xc15cfbd4), f32::from_bits(0x4242cb00)),
        (f32::from_bits(0xc156c2ae), f32::from_bits(0x4235a138)),
    );
    path.quad_to(
        (f32::from_bits(0xc150898c), f32::from_bits(0x42287770)),
        (f32::from_bits(0xc126e7d8), f32::from_bits(0x42204246)),
    );
    path.quad_to(
        (f32::from_bits(0xc1066ae4), f32::from_bits(0x4219da96)),
        (f32::from_bits(0xc0be6f82), f32::from_bits(0x42196502)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x42106507)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    );
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)));
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41000000), f32::from_bits(0x42040000)));
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x41c80000)),
    );
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L132-L223 (chrome/m156)
fn fuzz763_378(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x41013776), f32::from_bits(0xc25007a8)));
    path.quad_to(
        (f32::from_bits(0x412f219e), f32::from_bits(0xc256a86f)),
        (f32::from_bits(0x41625842), f32::from_bits(0xc2533a60)),
    );
    path.quad_to(
        (f32::from_bits(0x418ac776), f32::from_bits(0xc24fcc52)),
        (f32::from_bits(0x41980904), f32::from_bits(0xc24451c8)),
    );
    path.quad_to(
        (f32::from_bits(0x41a54a92), f32::from_bits(0xc238d73e)),
        (f32::from_bits(0x419e6e72), f32::from_bits(0xc22c0994)),
    );
    path.quad_to(
        (f32::from_bits(0x41979256), f32::from_bits(0xc21f3bea)),
        (f32::from_bits(0x41809d42), f32::from_bits(0xc2189b23)),
    );
    path.quad_to(
        (f32::from_bits(0x4153505c), f32::from_bits(0xc211fa5c)),
        (f32::from_bits(0x412019b5), f32::from_bits(0xc215686b)),
    );
    path.quad_to(
        (f32::from_bits(0x40d9c61e), f32::from_bits(0xc218d67a)),
        (f32::from_bits(0x40a4bfe8), f32::from_bits(0xc2245104)),
    );
    path.quad_to(
        (f32::from_bits(0x405f7360), f32::from_bits(0xc22fcb8e)),
        (f32::from_bits(0x408b2a24), f32::from_bits(0xc23c9937)),
    );
    path.quad_to(
        (f32::from_bits(0x40a69a9c), f32::from_bits(0xc24966e1)),
        (f32::from_bits(0x41013776), f32::from_bits(0xc25007a8)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc21aa3d0), f32::from_bits(0xc21a9d6c)));
    path.quad_to(
        (f32::from_bits(0xc21144a0), f32::from_bits(0xc223fd00)),
        (f32::from_bits(0xc2040363), f32::from_bits(0xc223fd46)),
    );
    path.quad_to(
        (f32::from_bits(0xc1ed844d), f32::from_bits(0xc223fd8c)),
        (f32::from_bits(0xc1dac526), f32::from_bits(0xc21a9e5c)),
    );
    path.quad_to(
        (f32::from_bits(0xc1c80600), f32::from_bits(0xc2113f2c)),
        (f32::from_bits(0xc1c80574), f32::from_bits(0xc203fdef)),
    );
    path.quad_to(
        (f32::from_bits(0xc1c804e8), f32::from_bits(0xc1ed7964)),
        (f32::from_bits(0xc1dac348), f32::from_bits(0xc1daba3e)),
    );
    path.quad_to(
        (f32::from_bits(0xc1ed81a8), f32::from_bits(0xc1c7fb18)),
        (f32::from_bits(0xc2040211), f32::from_bits(0xc1c7fa8c)),
    );
    path.quad_to(
        (f32::from_bits(0xc211434e), f32::from_bits(0xc1c7fa00)),
        (f32::from_bits(0xc21aa2e0), f32::from_bits(0xc1dab860)),
    );
    path.quad_to(
        (f32::from_bits(0xc2240274), f32::from_bits(0xc1ed76bf)),
        (f32::from_bits(0xc22402ba), f32::from_bits(0xc203fc9d)),
    );
    path.quad_to(
        (f32::from_bits(0xc2240300), f32::from_bits(0xc2113dda)),
        (f32::from_bits(0xc21aa3d0), f32::from_bits(0xc21a9d6c)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)));
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    );
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)));
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    );
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    );
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    );
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    );
    path.quad_to(
        (f32::from_bits(0xc24fcaa2), f32::from_bits(0x418ace05)),
        (f32::from_bits(0xc2533929), f32::from_bits(0x41626a5d)),
    );
    path.line_to((f32::from_bits(0xc25338d1), f32::from_bits(0x41626f8c)));
    path.quad_to(
        (f32::from_bits(0xc24fca68), f32::from_bits(0x418ad2e8)),
        (f32::from_bits(0xc2444fb0), f32::from_bits(0x419813d4)),
    );
    path.quad_to(
        (f32::from_bits(0xc238d4f6), f32::from_bits(0x41a554c0)),
        (f32::from_bits(0xc22c0765), f32::from_bits(0x419e77ee)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f39d4), f32::from_bits(0x41979b1c)),
        (f32::from_bits(0xc218995e), f32::from_bits(0x4180a5aa)),
    );
    path.quad_to(
        (f32::from_bits(0xc211f8e8), f32::from_bits(0x41536071)),
        (f32::from_bits(0xc2156751), f32::from_bits(0x41202a2d)),
    );
    path.quad_to(
        (f32::from_bits(0xc2156774), f32::from_bits(0x41202819)),
        (f32::from_bits(0xc2156798), f32::from_bits(0x41202604)),
    );
    path.quad_to(
        (f32::from_bits(0xc2156d3e), f32::from_bits(0x411fd1b7)),
        (f32::from_bits(0xc2157321), f32::from_bits(0x411f7b6e)),
    );
    path.quad_to(
        (f32::from_bits(0xc218e910), f32::from_bits(0x40d986da)),
        (f32::from_bits(0xc2245097), f32::from_bits(0x40a4daf8)),
    );
    path.quad_to(
        (f32::from_bits(0xc22fcb44), f32::from_bits(0x405fad48)),
        (f32::from_bits(0xc23c98dc), f32::from_bits(0x408b493c)),
    );
    path.quad_to(
        (f32::from_bits(0xc2496673), f32::from_bits(0x40a6bbcc)),
        (f32::from_bits(0xc25006fe), f32::from_bits(0x4101489a)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a729), f32::from_bits(0x412f30b9)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    );
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc2533937), f32::from_bits(0x41626995)));
    path.quad_to(
        (f32::from_bits(0xc2533968), f32::from_bits(0x416266bf)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    );
    path.line_to((f32::from_bits(0x41dac1c6), f32::from_bits(0x41dabbc0)));
    path.quad_to(
        (f32::from_bits(0x41dac044), f32::from_bits(0x41dabd41)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x421aa1af), f32::from_bits(0x421a9f8e)),
        (f32::from_bits(0x421aa2bf), f32::from_bits(0x421a9e7d)),
    );
    path.quad_to(
        (f32::from_bits(0x42240200), f32::from_bits(0x42113efb)),
        (f32::from_bits(0x422401d1), f32::from_bits(0x4203fdbe)),
    );
    path.quad_to(
        (f32::from_bits(0x422401a3), f32::from_bits(0x41ed7902)),
        (f32::from_bits(0x421aa220), f32::from_bits(0x41daba81)),
    );
    path.quad_to(
        (f32::from_bits(0x4211429e), f32::from_bits(0x41c7fc00)),
        (f32::from_bits(0x42040161), f32::from_bits(0x41c7fc5d)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed8047), f32::from_bits(0x41c7fcbb)),
        (f32::from_bits(0x41dac1c6), f32::from_bits(0x41dabbc0)),
    );
    path.line_to((f32::from_bits(0xc2533937), f32::from_bits(0x41626995)));
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xc2444fb0), f32::from_bits(0x419813d4)));
    path.quad_to(
        (f32::from_bits(0xc24fca68), f32::from_bits(0x418ad2e8)),
        (f32::from_bits(0xc25338d1), f32::from_bits(0x41626f8c)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a73a), f32::from_bits(0x412f3944)),
        (f32::from_bits(0xc25006c4), f32::from_bits(0x41014e62)),
    );
    path.quad_to(
        (f32::from_bits(0xc249664e), f32::from_bits(0x40a6c6fc)),
        (f32::from_bits(0xc23c98bd), f32::from_bits(0x408b53b8)),
    );
    path.quad_to(
        (f32::from_bits(0xc22fcb2b), f32::from_bits(0x405fc0d8)),
        (f32::from_bits(0xc2245073), f32::from_bits(0x40a4e41c)),
    );
    path.quad_to(
        (f32::from_bits(0xc218d5ba), f32::from_bits(0x40d9e7cc)),
        (f32::from_bits(0xc2156751), f32::from_bits(0x41202a2d)),
    );
    path.quad_to(
        (f32::from_bits(0xc211f8e8), f32::from_bits(0x41536071)),
        (f32::from_bits(0xc218995e), f32::from_bits(0x4180a5aa)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f39d4), f32::from_bits(0x41979b1c)),
        (f32::from_bits(0xc22c0765), f32::from_bits(0x419e77ee)),
    );
    path.quad_to(
        (f32::from_bits(0xc238d4f6), f32::from_bits(0x41a554c0)),
        (f32::from_bits(0xc2444fb0), f32::from_bits(0x419813d4)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L225-L246 (chrome/m156)
fn fuzz763_378b(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((-47.1494f32, 4.35143f32));
    path.quad_to((-39.8075f32, 18.9486f32), (-43.0083f32, 19.8062f32));
    path.quad_to((-50.35f32, 5.21042f32), (-52.0068f32, 8.08022f32));
    path.quad_to((-53.6632f32, 10.9494f32), (-52.8062f32, 14.1494f32));
    path.quad_to((-53.6639f32, 10.9486f32), (-52.007f32, 8.07884f32));
    path.quad_to((-50.3502f32, 5.20908f32), (-47.1494f32, 4.35143f32));
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xc2444fb0), f32::from_bits(0x419813d4)));
    path.quad_to(
        (f32::from_bits(0xc24fca68), f32::from_bits(0x418ad2e8)),
        (f32::from_bits(0xc25338d1), f32::from_bits(0x41626f8c)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a73a), f32::from_bits(0x412f3944)),
        (f32::from_bits(0xc25006c4), f32::from_bits(0x41014e62)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f39d4), f32::from_bits(0x41979b1c)),
        (f32::from_bits(0xc22c0765), f32::from_bits(0x419e77ee)),
    );
    path.quad_to(
        (f32::from_bits(0xc238d4f6), f32::from_bits(0x41a554c0)),
        (f32::from_bits(0xc2444fb0), f32::from_bits(0x419813d4)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L248-L267 (chrome/m156)
fn fuzz763_378c(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((-47.1494f32, 4.35143f32));
    path.quad_to((-46.208f32, 20.6664f32), (-43.0072f32, 19.8086f32));
    path.quad_to((-39.8065f32, 18.9507f32), (-38.1498f32, 16.0809f32));
    path.quad_to((-36.4931f32, 13.211f32), (-37.3509f32, 10.0103f32));
    path.quad_to((-37.351f32, 10.0098f32), (-37.3512f32, 10.0093f32));
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((-49.0778f32, 19.0097f32));
    path.quad_to((-38.2087f32, 6.80955f32), (-37.3509f32, 10.0103f32));
    path.quad_to((-36.4931f32, 13.211f32), (-38.1498f32, 16.0809f32));
    path.quad_to((-39.8065f32, 18.9507f32), (-43.0072f32, 19.8086f32));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L269-L285 (chrome/m156)
fn fuzz763_378d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((-47.1494f32, 4.35143f32));
    path.quad_to((-38.2091f32, 6.80749f32), (-37.3514f32, 10.0083f32)); // required
    path.quad_to((-36.4938f32, 13.2091f32), (-38.1506f32, 16.0788f32)); // required
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((-49.0778f32, 19.0097f32));
    path.quad_to((-38.2087f32, 6.80955f32), (-37.3509f32, 10.0103f32));
    path.quad_to((-36.4931f32, 13.211f32), (-38.1498f32, 16.0809f32));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L287-L379 (chrome/m156)
fn fuzz763_558(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x41c95d06), f32::from_bits(0xc238e312)));
    path.quad_to(
        (f32::from_bits(0x41e37302), f32::from_bits(0xc23b3f66)),
        (f32::from_bits(0x41f93bb2), f32::from_bits(0xc233b1b3)),
    );
    path.quad_to(
        (f32::from_bits(0x42025d9e), f32::from_bits(0xc22fb50a)),
        (f32::from_bits(0x4205bcea), f32::from_bits(0xc22a30db)),
    );
    path.quad_to(
        (f32::from_bits(0x420be531), f32::from_bits(0xc22837fe)),
        (f32::from_bits(0x421120f1), f32::from_bits(0xc2239353)),
    );
    path.quad_to(
        (f32::from_bits(0x421b0b2d), f32::from_bits(0xc21ac757)),
        (f32::from_bits(0x421bd594), f32::from_bits(0xc20d8c25)),
    );
    path.quad_to(
        (f32::from_bits(0x421c9ffc), f32::from_bits(0xc20050f2)),
        (f32::from_bits(0x4213d3fe), f32::from_bits(0xc1eccd6f)),
    );
    path.quad_to(
        (f32::from_bits(0x420b0802), f32::from_bits(0xc1d8f8fa)),
        (f32::from_bits(0x41fb99a0), f32::from_bits(0xc1d7642b)),
    );
    path.quad_to(
        (f32::from_bits(0x41e1233b), f32::from_bits(0xc1d5cf5c)),
        (f32::from_bits(0x41cd4ec5), f32::from_bits(0xc1e76755)),
    );
    path.quad_to(
        (f32::from_bits(0x41c5ef3d), f32::from_bits(0xc1edf201)),
        (f32::from_bits(0x41c11591), f32::from_bits(0xc1f5b68f)),
    );
    path.quad_to(
        (f32::from_bits(0x41b863c9), f32::from_bits(0xc1f896c5)),
        (f32::from_bits(0x41b04a41), f32::from_bits(0xc1fe34bf)),
    );
    path.quad_to(
        (f32::from_bits(0x419a8190), f32::from_bits(0xc206a812)),
        (f32::from_bits(0x4195c8e8), f32::from_bits(0xc213b310)),
    );
    path.quad_to(
        (f32::from_bits(0x41911040), f32::from_bits(0xc220be0e)),
        (f32::from_bits(0x41a02ba6), f32::from_bits(0xc22ba266)),
    );
    path.quad_to(
        (f32::from_bits(0x41af470a), f32::from_bits(0xc23686bf)),
        (f32::from_bits(0x41c95d06), f32::from_bits(0xc238e312)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc2169738), f32::from_bits(0xc2131d1b)));
    path.quad_to(
        (f32::from_bits(0xc2096e21), f32::from_bits(0xc214b131)),
        (f32::from_bits(0xc1fe042e), f32::from_bits(0xc20c809d)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e92c1a), f32::from_bits(0xc204500a)),
        (f32::from_bits(0xc1e603ef), f32::from_bits(0xc1ee4de5)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e2dbc3), f32::from_bits(0xc1d3fbb6)),
        (f32::from_bits(0xc1f33ce9), f32::from_bits(0xc1bf23a3)),
    );
    path.quad_to(
        (f32::from_bits(0xc201cf08), f32::from_bits(0xc1aa4b8f)),
        (f32::from_bits(0xc20ef820), f32::from_bits(0xc1a72363)),
    );
    path.quad_to(
        (f32::from_bits(0xc21c2138), f32::from_bits(0xc1a3fb38)),
        (f32::from_bits(0xc2268d41), f32::from_bits(0xc1b45c5e)),
    );
    path.quad_to(
        (f32::from_bits(0xc230f94b), f32::from_bits(0xc1c4bd85)),
        (f32::from_bits(0xc2328d61), f32::from_bits(0xc1df0fb4)),
    );
    path.quad_to(
        (f32::from_bits(0xc2342177), f32::from_bits(0xc1f961e4)),
        (f32::from_bits(0xc22bf0e3), f32::from_bits(0xc2071cfb)),
    );
    path.quad_to(
        (f32::from_bits(0xc223c050), f32::from_bits(0xc2118905)),
        (f32::from_bits(0xc2169738), f32::from_bits(0xc2131d1b)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)));
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    );
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0xbe8f799b), f32::from_bits(0x42240000)),
        (f32::from_bits(0xbf0db675), f32::from_bits(0x4223eed6)),
    );
    path.quad_to(
        (f32::from_bits(0xc060c2a3), f32::from_bits(0x42233513)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    );
    path.line_to((f32::from_bits(0xc0c24f68), f32::from_bits(0x4218d9ff)));
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x421005d8)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    );
    path.close();
    path.move_to((f32::from_bits(0x42215fd9), f32::from_bits(0xc1c64bb4)));
    path.quad_to(
        (f32::from_bits(0x422dad8e), f32::from_bits(0xc1d0284f)),
        (f32::from_bits(0x4239dd52), f32::from_bits(0xc1c5bb1d)),
    );
    path.quad_to(
        (f32::from_bits(0x42460d14), f32::from_bits(0xc1bb4dea)),
        (f32::from_bits(0x424afb61), f32::from_bits(0xc1a2b282)),
    );
    path.quad_to(
        (f32::from_bits(0x424fe9af), f32::from_bits(0xc18a1717)),
        (f32::from_bits(0x424ab316), f32::from_bits(0xc1636f22)),
    );
    path.quad_to(
        (f32::from_bits(0x42457c7c), f32::from_bits(0xc132b016)),
        (f32::from_bits(0x42392ec8), f32::from_bits(0xc11ef6e3)),
    );
    path.quad_to(
        (f32::from_bits(0x422ce113), f32::from_bits(0xc10b3dad)),
        (f32::from_bits(0x4220b150), f32::from_bits(0xc1201812)),
    );
    path.quad_to(
        (f32::from_bits(0x4214818d), f32::from_bits(0xc134f276)),
        (f32::from_bits(0x420f9340), f32::from_bits(0xc1662949)),
    );
    path.quad_to(
        (f32::from_bits(0x420aa4f2), f32::from_bits(0xc18bb00e)),
        (f32::from_bits(0x420fdb8c), f32::from_bits(0xc1a40f94)),
    );
    path.quad_to(
        (f32::from_bits(0x42151225), f32::from_bits(0xc1bc6f1a)),
        (f32::from_bits(0x42215fd9), f32::from_bits(0xc1c64bb4)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)));
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    );
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    );
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    );
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    );
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    );
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    );
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)));
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xbfe9fe20), f32::from_bits(0x42526568)));
    path.quad_to(
        (f32::from_bits(0xc08f5cf4), f32::from_bits(0x425a57e7)),
        (f32::from_bits(0xc0f853f0), f32::from_bits(0x4258763b)),
    );
    path.quad_to(
        (f32::from_bits(0xc130a57c), f32::from_bits(0x42569490)),
        (f32::from_bits(0xc1506f76), f32::from_bits(0x424bf8e3)),
    );
    path.quad_to(
        (f32::from_bits(0xc1703970), f32::from_bits(0x42415d36)),
        (f32::from_bits(0xc168b2c0), f32::from_bits(0x42343e56)),
    );
    path.quad_to(
        (f32::from_bits(0xc1612c17), f32::from_bits(0x42271f76)),
        (f32::from_bits(0xc136bd61), f32::from_bits(0x421f2cf7)),
    );
    path.quad_to(
        (f32::from_bits(0xc10c4ead), f32::from_bits(0x42173a78)),
        (f32::from_bits(0xc0afa654), f32::from_bits(0x42191c24)),
    );
    path.quad_to(
        (f32::from_bits(0xc00d5ea8), f32::from_bits(0x421afdcf)),
        (f32::from_bits(0xbe636c00), f32::from_bits(0x4225997c)),
    );
    path.quad_to(
        (f32::from_bits(0x3fe1e250), f32::from_bits(0x42303529)),
        (f32::from_bits(0x3fa5acf0), f32::from_bits(0x423d5409)),
    );
    path.quad_to(
        (f32::from_bits(0x3f52ef00), f32::from_bits(0x424a72ea)),
        (f32::from_bits(0xbfe9fe20), f32::from_bits(0x42526568)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L381-L467 (chrome/m156)
fn fuzz763_378a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x40f4c1fc), f32::from_bits(0xc25049c6)));
    path.quad_to(
        (f32::from_bits(0x41281306), f32::from_bits(0xc25702a0)),
        (f32::from_bits(0x415b6610), f32::from_bits(0xc253af82)),
    );
    path.quad_to(
        (f32::from_bits(0x41875c90), f32::from_bits(0xc2505c66)),
        (f32::from_bits(0x4194ce44), f32::from_bits(0xc244efe4)),
    );
    path.quad_to(
        (f32::from_bits(0x41a23ff8), f32::from_bits(0xc2398363)),
        (f32::from_bits(0x419b99bc), f32::from_bits(0xc22caea0)),
    );
    path.quad_to(
        (f32::from_bits(0x4194f385), f32::from_bits(0xc21fd9dc)),
        (f32::from_bits(0x417c3502), f32::from_bits(0xc2192102)),
    );
    path.quad_to(
        (f32::from_bits(0x414e82fc), f32::from_bits(0xc2126828)),
        (f32::from_bits(0x411b2fef), f32::from_bits(0xc215bb45)),
    );
    path.quad_to(
        (f32::from_bits(0x40cfb9c4), f32::from_bits(0xc2190e62)),
        (f32::from_bits(0x4099f2f4), f32::from_bits(0xc2247ae4)),
    );
    path.quad_to(
        (f32::from_bits(0x40485848), f32::from_bits(0xc22fe766)),
        (f32::from_bits(0x407d8a18), f32::from_bits(0xc23cbc28)),
    );
    path.quad_to(
        (f32::from_bits(0x40995df0), f32::from_bits(0xc24990ec)),
        (f32::from_bits(0x40f4c1fc), f32::from_bits(0xc25049c6)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc20605f2), f32::from_bits(0xc22259cd)));
    path.quad_to(
        (f32::from_bits(0xc1f189ee), f32::from_bits(0xc22283df)),
        (f32::from_bits(0xc1de900b), f32::from_bits(0xc219426c)),
    );
    path.quad_to(
        (f32::from_bits(0xc1cb9626), f32::from_bits(0xc21000fa)),
        (f32::from_bits(0xc1cb4202), f32::from_bits(0xc202c000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1caeddd), f32::from_bits(0xc1eafe0b)),
        (f32::from_bits(0xc1dd70c3), f32::from_bits(0xc1d80427)),
    );
    path.quad_to(
        (f32::from_bits(0xc1eff3a7), f32::from_bits(0xc1c50a43)),
        (f32::from_bits(0xc2053ace), f32::from_bits(0xc1c4b61e)),
    );
    path.quad_to(
        (f32::from_bits(0xc2127bc8), f32::from_bits(0xc1c461fa)),
        (f32::from_bits(0xc21bf8ba), f32::from_bits(0xc1d6e4df)),
    );
    path.quad_to(
        (f32::from_bits(0xc22575ad), f32::from_bits(0xc1e967c4)),
        (f32::from_bits(0xc2259fbf), f32::from_bits(0xc201f4dc)),
    );
    path.quad_to(
        (f32::from_bits(0xc225c9d1), f32::from_bits(0xc20f35d6)),
        (f32::from_bits(0xc21c885e), f32::from_bits(0xc218b2c8)),
    );
    path.quad_to(
        (f32::from_bits(0xc21346ec), f32::from_bits(0xc2222fbb)),
        (f32::from_bits(0xc20605f2), f32::from_bits(0xc22259cd)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)));
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    );
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)));
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    );
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    );
    path.quad_to(
        (f32::from_bits(0xc2155d3d), f32::from_bits(0x4120c08f)),
        (f32::from_bits(0xc2155303), f32::from_bits(0x41215e9f)),
    );
    path.quad_to(
        (f32::from_bits(0xc21547f6), f32::from_bits(0x4121fb98)),
        (f32::from_bits(0xc2153d2f), f32::from_bits(0x412299db)),
    );
    path.quad_to(
        (f32::from_bits(0xc2153265), f32::from_bits(0x41233845)),
        (f32::from_bits(0xc21527fc), f32::from_bits(0x4123d684)),
    );
    path.quad_to(
        (f32::from_bits(0xc2151cc4), f32::from_bits(0x41247361)),
        (f32::from_bits(0xc21511d9), f32::from_bits(0x41251125)),
    );
    path.quad_to(
        (f32::from_bits(0xc211888d), f32::from_bits(0x41582a1e)),
        (f32::from_bits(0xc21810d2), f32::from_bits(0x4183262b)),
    );
    path.quad_to(
        (f32::from_bits(0xc21e9918), f32::from_bits(0x419a3747)),
        (f32::from_bits(0xc22b5f56), f32::from_bits(0x41a149de)),
    );
    path.quad_to(
        (f32::from_bits(0xc2382594), f32::from_bits(0x41a85c76)),
        (f32::from_bits(0xc243ae22), f32::from_bits(0x419b4beb)),
    );
    path.quad_to(
        (f32::from_bits(0xc24f36b0), f32::from_bits(0x418e3b60)),
        (f32::from_bits(0xc252bffc), f32::from_bits(0x41695dc8)),
    );
    path.quad_to(
        (f32::from_bits(0xc252cf70), f32::from_bits(0x41687e79)),
        (f32::from_bits(0xc252de2e), f32::from_bits(0x41679ef4)),
    );
    path.quad_to(
        (f32::from_bits(0xc252ee09), f32::from_bits(0x4166c0c4)),
        (f32::from_bits(0xc252fd41), f32::from_bits(0x4165e14c)),
    );
    path.quad_to(
        (f32::from_bits(0xc2530c80), f32::from_bits(0x41650165)),
        (f32::from_bits(0xc2531afd), f32::from_bits(0x416421f9)),
    );
    path.quad_to(
        (f32::from_bits(0xc2532a97), f32::from_bits(0x416343e9)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    );
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    );
    path.close();
    path.move_to((f32::from_bits(0x4204d274), f32::from_bits(0x41c5cf9d)));
    path.quad_to(
        (f32::from_bits(0x42121393), f32::from_bits(0x41c59784)),
        (f32::from_bits(0x421b86b4), f32::from_bits(0x41d82e73)),
    );
    path.quad_to(
        (f32::from_bits(0x4224f9d6), f32::from_bits(0x41eac561)),
        (f32::from_bits(0x422515e3), f32::from_bits(0x4202a3d1)),
    );
    path.quad_to(
        (f32::from_bits(0x422531ef), f32::from_bits(0x420fe4f0)),
        (f32::from_bits(0x421be677), f32::from_bits(0x42195811)),
    );
    path.quad_to(
        (f32::from_bits(0x421b94ff), f32::from_bits(0x4219aae4)),
        (f32::from_bits(0x421b423a), f32::from_bits(0x4219fb06)),
    );
    path.line_to((f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)));
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x41db32ee), f32::from_bits(0x41da4a98)),
        (f32::from_bits(0x41dba82d), f32::from_bits(0x41d9d952)),
    );
    path.quad_to(
        (f32::from_bits(0x41dc1880), f32::from_bits(0x41d9631e)),
        (f32::from_bits(0x41dc8bb9), f32::from_bits(0x41d8edf9)),
    );
    path.quad_to(
        (f32::from_bits(0x41ef22a8), f32::from_bits(0x41c607b5)),
        (f32::from_bits(0x4204d274), f32::from_bits(0x41c5cf9d)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xc243ae22), f32::from_bits(0x419b4beb)));
    path.quad_to(
        (f32::from_bits(0xc24f36b0), f32::from_bits(0x418e3b60)),
        (f32::from_bits(0xc252bffc), f32::from_bits(0x41695dc8)),
    );
    path.quad_to(
        (f32::from_bits(0xc2564948), f32::from_bits(0x413644ce)),
        (f32::from_bits(0xc24fc102), f32::from_bits(0x41082296)),
    );
    path.quad_to(
        (f32::from_bits(0xc24938bd), f32::from_bits(0x40b400bc)),
        (f32::from_bits(0xc23c727f), f32::from_bits(0x4097b660)),
    );
    path.quad_to(
        (f32::from_bits(0xc22fac40), f32::from_bits(0x4076d800)),
        (f32::from_bits(0xc22423b2), f32::from_bits(0x40afae2c)),
    );
    path.quad_to(
        (f32::from_bits(0xc2189b24), f32::from_bits(0x40e3f058)),
        (f32::from_bits(0xc21511d9), f32::from_bits(0x41251125)),
    );
    path.quad_to(
        (f32::from_bits(0xc211888d), f32::from_bits(0x41582a1e)),
        (f32::from_bits(0xc21810d2), f32::from_bits(0x4183262b)),
    );
    path.quad_to(
        (f32::from_bits(0xc21e9918), f32::from_bits(0x419a3747)),
        (f32::from_bits(0xc22b5f56), f32::from_bits(0x41a149de)),
    );
    path.quad_to(
        (f32::from_bits(0xc2382594), f32::from_bits(0x41a85c76)),
        (f32::from_bits(0xc243ae22), f32::from_bits(0x419b4beb)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L470-L508 (chrome/m156)
fn fuzz763_378a_1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)));
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    );
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    );
    path.quad_to(
        (f32::from_bits(0xc2155d3d), f32::from_bits(0x4120c08f)),
        (f32::from_bits(0xc2155303), f32::from_bits(0x41215e9f)),
    );
    path.quad_to(
        (f32::from_bits(0xc21547f6), f32::from_bits(0x4121fb98)),
        (f32::from_bits(0xc2153d2f), f32::from_bits(0x412299db)),
    );
    path.quad_to(
        (f32::from_bits(0xc2153265), f32::from_bits(0x41233845)),
        (f32::from_bits(0xc21527fc), f32::from_bits(0x4123d684)),
    );
    path.quad_to(
        (f32::from_bits(0xc2151cc4), f32::from_bits(0x41247361)),
        (f32::from_bits(0xc21511d9), f32::from_bits(0x41251125)),
    );
    path.quad_to(
        (f32::from_bits(0xc211888d), f32::from_bits(0x41582a1e)),
        (f32::from_bits(0xc21810d2), f32::from_bits(0x4183262b)),
    );
    path.quad_to(
        (f32::from_bits(0xc21e9918), f32::from_bits(0x419a3747)),
        (f32::from_bits(0xc22b5f56), f32::from_bits(0x41a149de)),
    );
    path.quad_to(
        (f32::from_bits(0xc2382594), f32::from_bits(0x41a85c76)),
        (f32::from_bits(0xc243ae22), f32::from_bits(0x419b4beb)),
    );
    path.quad_to(
        (f32::from_bits(0xc24f36b0), f32::from_bits(0x418e3b60)),
        (f32::from_bits(0xc252bffc), f32::from_bits(0x41695dc8)),
    );
    path.quad_to(
        (f32::from_bits(0xc252cf70), f32::from_bits(0x41687e79)),
        (f32::from_bits(0xc252de2e), f32::from_bits(0x41679ef4)),
    );
    path.quad_to(
        (f32::from_bits(0xc252ee09), f32::from_bits(0x4166c0c4)),
        (f32::from_bits(0xc252fd41), f32::from_bits(0x4165e14c)),
    );
    path.quad_to(
        (f32::from_bits(0xc2530c80), f32::from_bits(0x41650165)),
        (f32::from_bits(0xc2531afd), f32::from_bits(0x416421f9)),
    );
    path.quad_to(
        (f32::from_bits(0xc2532a97), f32::from_bits(0x416343e9)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    );
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xc243ae22), f32::from_bits(0x419b4beb)));
    path.quad_to(
        (f32::from_bits(0xc24f36b0), f32::from_bits(0x418e3b60)),
        (f32::from_bits(0xc252bffc), f32::from_bits(0x41695dc8)),
    );
    path.quad_to(
        (f32::from_bits(0xc2564948), f32::from_bits(0x413644ce)),
        (f32::from_bits(0xc24fc102), f32::from_bits(0x41082296)),
    );
    path.quad_to(
        (f32::from_bits(0xc24938bd), f32::from_bits(0x40b400bc)),
        (f32::from_bits(0xc23c727f), f32::from_bits(0x4097b660)),
    );
    path.quad_to(
        (f32::from_bits(0xc22fac40), f32::from_bits(0x4076d800)),
        (f32::from_bits(0xc22423b2), f32::from_bits(0x40afae2c)),
    );
    path.quad_to(
        (f32::from_bits(0xc2189b24), f32::from_bits(0x40e3f058)),
        (f32::from_bits(0xc21511d9), f32::from_bits(0x41251125)),
    );
    path.quad_to(
        (f32::from_bits(0xc211888d), f32::from_bits(0x41582a1e)),
        (f32::from_bits(0xc21810d2), f32::from_bits(0x4183262b)),
    );
    path.quad_to(
        (f32::from_bits(0xc21e9918), f32::from_bits(0x419a3747)),
        (f32::from_bits(0xc22b5f56), f32::from_bits(0x41a149de)),
    );
    path.quad_to(
        (f32::from_bits(0xc2382594), f32::from_bits(0x41a85c76)),
        (f32::from_bits(0xc243ae22), f32::from_bits(0x419b4beb)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L510-L598 (chrome/m156)
fn fuzz763_8712(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x40dce520), f32::from_bits(0xc250b45c)));
    path.quad_to(
        (f32::from_bits(0x411bc0ec), f32::from_bits(0xc25796e0)),
        (f32::from_bits(0x414f4352), f32::from_bits(0xc25472d6)),
    );
    path.quad_to(
        (f32::from_bits(0x418162dd), f32::from_bits(0xc2514ece)),
        (f32::from_bits(0x418f27e4), f32::from_bits(0xc245fb37)),
    );
    path.quad_to(
        (f32::from_bits(0x419cecea), f32::from_bits(0xc23aa7a0)),
        (f32::from_bits(0x4196a4d8), f32::from_bits(0xc22dc706)),
    );
    path.quad_to(
        (f32::from_bits(0x41905cc8), f32::from_bits(0xc220e66c)),
        (f32::from_bits(0x41736b34), f32::from_bits(0xc21a03e9)),
    );
    path.quad_to(
        (f32::from_bits(0x41461cda), f32::from_bits(0xc2132166)),
        (f32::from_bits(0x41129a71), f32::from_bits(0xc216456f)),
    );
    path.quad_to(
        (f32::from_bits(0x40be3010), f32::from_bits(0xc2196978)),
        (f32::from_bits(0x40871bf8), f32::from_bits(0xc224bd0e)),
    );
    path.quad_to(
        (f32::from_bits(0x40200fb8), f32::from_bits(0xc23010a5)),
        (f32::from_bits(0x40525050), f32::from_bits(0xc23cf13e)),
    );
    path.quad_to(
        (f32::from_bits(0x4082486c), f32::from_bits(0xc249d1d9)),
        (f32::from_bits(0x40dce520), f32::from_bits(0xc250b45c)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)));
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    );
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    );
    path.close();
    path.move_to((f32::from_bits(0x422b20ca), f32::from_bits(0xc1a252a8)));
    path.quad_to(
        (f32::from_bits(0x4237e448), f32::from_bits(0xc1a97900)),
        (f32::from_bits(0x424371e0), f32::from_bits(0xc19c7a4f)),
    );
    path.quad_to(
        (f32::from_bits(0x424eff77), f32::from_bits(0xc18f7b9e)),
        (f32::from_bits(0x425292a1), f32::from_bits(0xc16be93c)),
    );
    path.quad_to(
        (f32::from_bits(0x425625cd), f32::from_bits(0xc138db44)),
        (f32::from_bits(0x424fa674), f32::from_bits(0xc10aa4e6)),
    );
    path.quad_to(
        (f32::from_bits(0x4249271c), f32::from_bits(0xc0b8dd14)),
        (f32::from_bits(0x423c639c), f32::from_bits(0xc09c43bc)),
    );
    path.quad_to(
        (f32::from_bits(0x422fa01e), f32::from_bits(0xc07f54c8)),
        (f32::from_bits(0x42241287), f32::from_bits(0xc0b3a528)),
    );
    path.quad_to(
        (f32::from_bits(0x421884f0), f32::from_bits(0xc0e79fee)),
        (f32::from_bits(0x4214f1c4), f32::from_bits(0xc126ddf2)),
    );
    path.quad_to(
        (f32::from_bits(0x42115e99), f32::from_bits(0xc159ebed)),
        (f32::from_bits(0x4217ddf2), f32::from_bits(0xc1841124)),
    );
    path.quad_to(
        (f32::from_bits(0x421e5d4a), f32::from_bits(0xc19b2c54)),
        (f32::from_bits(0x422b20ca), f32::from_bits(0xc1a252a8)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)));
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    );
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    );
    path.quad_to(
        (f32::from_bits(0xc2152d73), f32::from_bits(0x412389fb)),
        (f32::from_bits(0xc214fe6a), f32::from_bits(0x4126ec3a)),
    );
    path.quad_to(
        (f32::from_bits(0xc214b621), f32::from_bits(0x412a3217)),
        (f32::from_bits(0xc21476d0), f32::from_bits(0x412d948d)),
    );
    path.quad_to(
        (f32::from_bits(0xc210bed3), f32::from_bits(0x41607862)),
        (f32::from_bits(0xc2171cb0), f32::from_bits(0x41877c8b)),
    );
    path.quad_to(
        (f32::from_bits(0xc21d7a8c), f32::from_bits(0x419ebce4)),
        (f32::from_bits(0xc22a3381), f32::from_bits(0x41a62cde)),
    );
    path.quad_to(
        (f32::from_bits(0xc236ec77), f32::from_bits(0x41ad9cd6)),
        (f32::from_bits(0xc2428ca4), f32::from_bits(0x41a0e11e)),
    );
    path.quad_to(
        (f32::from_bits(0xc24e2cd0), f32::from_bits(0x41942565)),
        (f32::from_bits(0xc251e4cc), f32::from_bits(0x417566f6)),
    );
    path.quad_to(
        (f32::from_bits(0xc2523ea2), f32::from_bits(0x41709990)),
        (f32::from_bits(0xc252817b), f32::from_bits(0x416bd61f)),
    );
    path.quad_to(
        (f32::from_bits(0xc252e6b7), f32::from_bits(0x41673927)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    );
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    );
    path.close();
    path.move_to((f32::from_bits(0x42074f3a), f32::from_bits(0x41bef2d8)));
    path.quad_to(
        (f32::from_bits(0x42148e85), f32::from_bits(0x41be0d1d)),
        (f32::from_bits(0x421e3dbf), f32::from_bits(0x41d026ae)),
    );
    path.quad_to(
        (f32::from_bits(0x4227ecfa), f32::from_bits(0x41e24040)),
        (f32::from_bits(0x42285fd8), f32::from_bits(0x41fcbed6)),
    );
    path.quad_to(
        (f32::from_bits(0x4228d2b5), f32::from_bits(0x420b9eb6)),
        (f32::from_bits(0x421fc5ec), f32::from_bits(0x42154df0)),
    );
    path.quad_to(
        (f32::from_bits(0x421f5958), f32::from_bits(0x4215c221)),
        (f32::from_bits(0x421eea62), f32::from_bits(0x42163126)),
    );
    path.quad_to(
        (f32::from_bits(0x421e81d1), f32::from_bits(0x4216a62c)),
        (f32::from_bits(0x421e13f4), f32::from_bits(0x4217191c)),
    );
    path.quad_to(
        (f32::from_bits(0x421d36b1), f32::from_bits(0x42180097)),
        (f32::from_bits(0x421c5020), f32::from_bits(0x4218d2e4)),
    );
    path.quad_to(
        (f32::from_bits(0x421bb44d), f32::from_bits(0x421985ae)),
        (f32::from_bits(0x421b0c17), f32::from_bits(0x421a3367)),
    );
    path.line_to((f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)));
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x41dbfdf8), f32::from_bits(0x41d97f8e)),
        (f32::from_bits(0x41dd45f8), f32::from_bits(0x41d85595)),
    );
    path.quad_to(
        (f32::from_bits(0x41de6877), f32::from_bits(0x41d706ef)),
        (f32::from_bits(0x41dfa063), f32::from_bits(0x41d5c09b)),
    );
    path.quad_to(
        (f32::from_bits(0x41e03b86), f32::from_bits(0x41d51e4d)),
        (f32::from_bits(0x41e0d904), f32::from_bits(0x41d48124)),
    );
    path.quad_to(
        (f32::from_bits(0x41e16d06), f32::from_bits(0x41d3db0f)),
        (f32::from_bits(0x41e2064d), f32::from_bits(0x41d33709)),
    );
    path.quad_to(
        (f32::from_bits(0x41f41fdf), f32::from_bits(0x41bfd894)),
        (f32::from_bits(0x42074f3a), f32::from_bits(0x41bef2d8)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xc2428ca4), f32::from_bits(0x41a0e11e)));
    path.quad_to(
        (f32::from_bits(0xc24e2cd0), f32::from_bits(0x41942565)),
        (f32::from_bits(0xc251e4cc), f32::from_bits(0x417566f6)),
    );
    path.quad_to(
        (f32::from_bits(0xc2559cca), f32::from_bits(0x4142831e)),
        (f32::from_bits(0xc24f3eed), f32::from_bits(0x4114026c)),
    );
    path.quad_to(
        (f32::from_bits(0xc248e111), f32::from_bits(0x40cb0370)),
        (f32::from_bits(0xc23c281b), f32::from_bits(0x40ad4390)),
    );
    path.quad_to(
        (f32::from_bits(0xc22f6f26), f32::from_bits(0x408f83a8)),
        (f32::from_bits(0xc223cefa), f32::from_bits(0x40c2728a)),
    );
    path.quad_to(
        (f32::from_bits(0xc2182ecc), f32::from_bits(0x40f5616e)),
        (f32::from_bits(0xc21476d0), f32::from_bits(0x412d948d)),
    );
    path.quad_to(
        (f32::from_bits(0xc210bed3), f32::from_bits(0x41607862)),
        (f32::from_bits(0xc2171cb0), f32::from_bits(0x41877c8b)),
    );
    path.quad_to(
        (f32::from_bits(0xc21d7a8c), f32::from_bits(0x419ebce4)),
        (f32::from_bits(0xc22a3381), f32::from_bits(0x41a62cde)),
    );
    path.quad_to(
        (f32::from_bits(0xc236ec77), f32::from_bits(0x41ad9cd6)),
        (f32::from_bits(0xc2428ca4), f32::from_bits(0x41a0e11e)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L600-L633 (chrome/m156)
fn fuzz763_8712a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)));
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    );
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    );
    path.quad_to(
        (f32::from_bits(0xc2152d73), f32::from_bits(0x412389fb)),
        (f32::from_bits(0xc214fe6a), f32::from_bits(0x4126ec3a)),
    );
    path.quad_to(
        (f32::from_bits(0xc214b621), f32::from_bits(0x412a3217)),
        (f32::from_bits(0xc21476d0), f32::from_bits(0x412d948d)),
    );
    path.quad_to(
        (f32::from_bits(0xc210bed3), f32::from_bits(0x41607862)),
        (f32::from_bits(0xc2171cb0), f32::from_bits(0x41877c8b)),
    );
    path.quad_to(
        (f32::from_bits(0xc21d7a8c), f32::from_bits(0x419ebce4)),
        (f32::from_bits(0xc22a3381), f32::from_bits(0x41a62cde)),
    );
    path.quad_to(
        (f32::from_bits(0xc236ec77), f32::from_bits(0x41ad9cd6)),
        (f32::from_bits(0xc2428ca4), f32::from_bits(0x41a0e11e)),
    );
    path.quad_to(
        (f32::from_bits(0xc24e2cd0), f32::from_bits(0x41942565)),
        (f32::from_bits(0xc251e4cc), f32::from_bits(0x417566f6)),
    );
    path.quad_to(
        (f32::from_bits(0xc2523ea2), f32::from_bits(0x41709990)),
        (f32::from_bits(0xc252817b), f32::from_bits(0x416bd61f)),
    );
    path.quad_to(
        (f32::from_bits(0xc252e6b7), f32::from_bits(0x41673927)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    );
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xc2428ca4), f32::from_bits(0x41a0e11e)));
    path.quad_to(
        (f32::from_bits(0xc24e2cd0), f32::from_bits(0x41942565)),
        (f32::from_bits(0xc251e4cc), f32::from_bits(0x417566f6)),
    );
    path.quad_to(
        (f32::from_bits(0xc2559cca), f32::from_bits(0x4142831e)),
        (f32::from_bits(0xc24f3eed), f32::from_bits(0x4114026c)),
    );
    path.quad_to(
        (f32::from_bits(0xc248e111), f32::from_bits(0x40cb0370)),
        (f32::from_bits(0xc23c281b), f32::from_bits(0x40ad4390)),
    );
    path.quad_to(
        (f32::from_bits(0xc22f6f26), f32::from_bits(0x408f83a8)),
        (f32::from_bits(0xc223cefa), f32::from_bits(0x40c2728a)),
    );
    path.quad_to(
        (f32::from_bits(0xc2182ecc), f32::from_bits(0x40f5616e)),
        (f32::from_bits(0xc21476d0), f32::from_bits(0x412d948d)),
    );
    path.quad_to(
        (f32::from_bits(0xc210bed3), f32::from_bits(0x41607862)),
        (f32::from_bits(0xc2171cb0), f32::from_bits(0x41877c8b)),
    );
    path.quad_to(
        (f32::from_bits(0xc21d7a8c), f32::from_bits(0x419ebce4)),
        (f32::from_bits(0xc22a3381), f32::from_bits(0x41a62cde)),
    );
    path.quad_to(
        (f32::from_bits(0xc236ec77), f32::from_bits(0x41ad9cd6)),
        (f32::from_bits(0xc2428ca4), f32::from_bits(0x41a0e11e)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L635-L722 (chrome/m156)
fn fuzz763_4014(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x4126977e), f32::from_bits(0xc24e5cc8)));
    path.quad_to(
        (f32::from_bits(0x4155a79e), f32::from_bits(0xc2547762)),
        (f32::from_bits(0x41841952), f32::from_bits(0xc250767b)),
    );
    path.quad_to(
        (f32::from_bits(0x419d5ed4), f32::from_bits(0xc24c7594)),
        (f32::from_bits(0x41a99408), f32::from_bits(0xc240b18c)),
    );
    path.quad_to(
        (f32::from_bits(0x41b5c93d), f32::from_bits(0xc234ed84)),
        (f32::from_bits(0x41adc770), f32::from_bits(0xc2284ac3)),
    );
    path.quad_to(
        (f32::from_bits(0x41a5c5a2), f32::from_bits(0xc21ba802)),
        (f32::from_bits(0x418e3d92), f32::from_bits(0xc2158d68)),
    );
    path.quad_to(
        (f32::from_bits(0x416d6b02), f32::from_bits(0xc20f72ce)),
        (f32::from_bits(0x413adfff), f32::from_bits(0xc21373b4)),
    );
    path.quad_to(
        (f32::from_bits(0x410854fa), f32::from_bits(0xc217749a)),
        (f32::from_bits(0x40dfd522), f32::from_bits(0xc22338a3)),
    );
    path.quad_to(
        (f32::from_bits(0x40af0050), f32::from_bits(0xc22efcab)),
        (f32::from_bits(0x40cf0788), f32::from_bits(0xc23b9f6c)),
    );
    path.quad_to(
        (f32::from_bits(0x40ef0eb8), f32::from_bits(0xc248422e)),
        (f32::from_bits(0x4126977e), f32::from_bits(0xc24e5cc8)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)));
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    );
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    );
    path.close();
    path.move_to((f32::from_bits(0x422dc5fa), f32::from_bits(0xc196a9b4)));
    path.quad_to(
        (f32::from_bits(0x423aa688), f32::from_bits(0xc19cf222)),
        (f32::from_bits(0x4245fa38), f32::from_bits(0xc18f2d6c)),
    );
    path.quad_to(
        (f32::from_bits(0x42514de7), f32::from_bits(0xc18168b7)),
        (f32::from_bits(0x4254721e), f32::from_bits(0xc14f4f32)),
    );
    path.quad_to(
        (f32::from_bits(0x42579654), f32::from_bits(0xc11bccf8)),
        (f32::from_bits(0x4250b3fa), f32::from_bits(0xc0dcfc74)),
    );
    path.quad_to(
        (f32::from_bits(0x4249d19f), f32::from_bits(0xc0825efc)),
        (f32::from_bits(0x423cf110), f32::from_bits(0xc0527a88)),
    );
    path.quad_to(
        (f32::from_bits(0x42301082), f32::from_bits(0xc0203718)),
        (f32::from_bits(0x4224bcd2), f32::from_bits(0xc0872e60)),
    );
    path.quad_to(
        (f32::from_bits(0x42196923), f32::from_bits(0xc0be4136)),
        (f32::from_bits(0x421644ec), f32::from_bits(0xc112a2d8)),
    );
    path.quad_to(
        (f32::from_bits(0x421320b5), f32::from_bits(0xc1462514)),
        (f32::from_bits(0x421a0310), f32::from_bits(0xc17373d0)),
    );
    path.quad_to(
        (f32::from_bits(0x4220e56a), f32::from_bits(0xc1906147)),
        (f32::from_bits(0x422dc5fa), f32::from_bits(0xc196a9b4)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc23d30a0), f32::from_bits(0x400e5c28)));
    path.quad_to(
        (f32::from_bits(0xc2303ecc), f32::from_bits(0x3fc17f10)),
        (f32::from_bits(0xc2251387), f32::from_bits(0x4052f2a8)),
    );
    path.quad_to(
        (f32::from_bits(0xc219e842), f32::from_bits(0x40a292e2)),
        (f32::from_bits(0xc2170e78), f32::from_bits(0x410510c3)),
    );
    path.quad_to(
        (f32::from_bits(0xc216ac8b), f32::from_bits(0x410c0373)),
        (f32::from_bits(0xc21678a8), f32::from_bits(0x4112d552)),
    );
    path.quad_to(
        (f32::from_bits(0xc215ddb7), f32::from_bits(0x411942a8)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    );
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    );
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    );
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    );
    path.quad_to(
        (f32::from_bits(0xc253e12c), f32::from_bits(0x41589e3d)),
        (f32::from_bits(0xc2542b01), f32::from_bits(0x414f09d7)),
    );
    path.quad_to(
        (f32::from_bits(0xc25503cd), f32::from_bits(0x414600f6)),
        (f32::from_bits(0xc2558f0e), f32::from_bits(0x413c1fa8)),
    );
    path.quad_to(
        (f32::from_bits(0xc25868d8), f32::from_bits(0x41085856)),
        (f32::from_bits(0xc25145a6), f32::from_bits(0x40b75684)),
    );
    path.quad_to(
        (f32::from_bits(0xc24a2274), f32::from_bits(0x403bf8b8)),
        (f32::from_bits(0xc23d30a0), f32::from_bits(0x400e5c28)),
    );
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)));
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x4218d6c4), f32::from_bits(0x421c6a78)),
        (f32::from_bits(0x4216e8ba), f32::from_bits(0x421ddcf4)),
    );
    path.quad_to(
        (f32::from_bits(0x42156061), f32::from_bits(0x421fb9cf)),
        (f32::from_bits(0x42138263), f32::from_bits(0x42216e93)),
    );
    path.quad_to(
        (f32::from_bits(0x42129692), f32::from_bits(0x4222460e)),
        (f32::from_bits(0x4211a2ed), f32::from_bits(0x422307e2)),
    );
    path.quad_to(
        (f32::from_bits(0x4210c6f1), f32::from_bits(0x4223e438)),
        (f32::from_bits(0x420fd652), f32::from_bits(0x4224b658)),
    );
    path.quad_to(
        (f32::from_bits(0x4205da74), f32::from_bits(0x422d6e4b)),
        (f32::from_bits(0x41f141e8), f32::from_bits(0x422c893f)),
    );
    path.quad_to(
        (f32::from_bits(0x41d6cee9), f32::from_bits(0x422ba432)),
        (f32::from_bits(0x41c55f04), f32::from_bits(0x4221a853)),
    );
    path.quad_to(
        (f32::from_bits(0x41b3ef1f), f32::from_bits(0x4217ac75)),
        (f32::from_bits(0x41b5b938), f32::from_bits(0x420a72f5)),
    );
    path.quad_to(
        (f32::from_bits(0x41b78350), f32::from_bits(0x41fa72eb)),
        (f32::from_bits(0x41cb7b0e), f32::from_bits(0x41e90306)),
    );
    path.quad_to(
        (f32::from_bits(0x41ccce3f), f32::from_bits(0x41e7dad2)),
        (f32::from_bits(0x41ce28c1), f32::from_bits(0x41e6c848)),
    );
    path.quad_to(
        (f32::from_bits(0x41cf607c), f32::from_bits(0x41e58ed0)),
        (f32::from_bits(0x41d0aced), f32::from_bits(0x41e45f0b)),
    );
    path.quad_to(
        (f32::from_bits(0x41d34d34), f32::from_bits(0x41e1f8bf)),
        (f32::from_bits(0x41d60d52), f32::from_bits(0x41dfea66)),
    );
    path.quad_to(
        (f32::from_bits(0x41d83ad5), f32::from_bits(0x41dd42b1)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xc2478a00), f32::from_bits(0x418639e0)));
    path.quad_to(
        (f32::from_bits(0xc252b546), f32::from_bits(0x416fe6f8)),
        (f32::from_bits(0xc2558f0e), f32::from_bits(0x413c1fa8)),
    );
    path.quad_to(
        (f32::from_bits(0xc25868d8), f32::from_bits(0x41085856)),
        (f32::from_bits(0xc25145a6), f32::from_bits(0x40b75684)),
    );
    path.quad_to(
        (f32::from_bits(0xc24a2274), f32::from_bits(0x403bf8b8)),
        (f32::from_bits(0xc23d30a0), f32::from_bits(0x400e5c28)),
    );
    path.quad_to(
        (f32::from_bits(0xc2303ecc), f32::from_bits(0x3fc17f10)),
        (f32::from_bits(0xc2251387), f32::from_bits(0x4052f2a8)),
    );
    path.quad_to(
        (f32::from_bits(0xc219e842), f32::from_bits(0x40a292e2)),
        (f32::from_bits(0xc2170e78), f32::from_bits(0x410510c3)),
    );
    path.quad_to(
        (f32::from_bits(0xc21434af), f32::from_bits(0x4138d815)),
        (f32::from_bits(0xc21b57e0), f32::from_bits(0x41658529)),
    );
    path.quad_to(
        (f32::from_bits(0xc2227b12), f32::from_bits(0x4189191e)),
        (f32::from_bits(0xc22f6ce6), f32::from_bits(0x418eccb0)),
    );
    path.quad_to(
        (f32::from_bits(0xc23c5ebc), f32::from_bits(0x41948044)),
        (f32::from_bits(0xc2478a00), f32::from_bits(0x418639e0)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L724-L758 (chrome/m156)
fn fuzz763_4014a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xc23d30a0), f32::from_bits(0x400e5c28))); // -47.2975f, 2.22437f
    path.quad_to(
        (f32::from_bits(0xc2303ecc), f32::from_bits(0x3fc17f10)),
        (f32::from_bits(0xc2251387), f32::from_bits(0x4052f2a8)),
    ); // -44.0613f, 1.51169f, -41.2691f, 3.29606f
    path.quad_to(
        (f32::from_bits(0xc219e842), f32::from_bits(0x40a292e2)),
        (f32::from_bits(0xc2170e78), f32::from_bits(0x410510c3)),
    ); // -38.4768f, 5.08043f, -37.7641f, 8.31659f
    path.quad_to(
        (f32::from_bits(0xc216ac8b), f32::from_bits(0x410c0373)),
        (f32::from_bits(0xc21678a8), f32::from_bits(0x4112d552)),
    ); // -37.6685f, 8.75084f, -37.6178f, 9.17708f
    path.quad_to(
        (f32::from_bits(0xc215ddb7), f32::from_bits(0x411942a8)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    ); // -37.4665f, 9.57877f, -37.3514f, 10.0083f
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    ); // -36.4938f, 13.2091f, -38.1506f, 16.0788f
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    ); // -39.8075f, 18.9486f, -43.0083f, 19.8062f
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    ); // -46.2091f, 20.6639f, -49.0788f, 19.007f
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    ); // -51.9486f, 17.3502f, -52.8062f, 14.1494f
    path.quad_to(
        (f32::from_bits(0xc253e12c), f32::from_bits(0x41589e3d)),
        (f32::from_bits(0xc2542b01), f32::from_bits(0x414f09d7)),
    ); // -52.9699f, 13.5386f, -53.042f, 12.9399f
    path.quad_to(
        (f32::from_bits(0xc25503cd), f32::from_bits(0x414600f6)),
        (f32::from_bits(0xc2558f0e), f32::from_bits(0x413c1fa8)),
    ); // -53.2537f, 12.3752f, -53.3897f, 11.7577f
    path.quad_to(
        (f32::from_bits(0xc25868d8), f32::from_bits(0x41085856)),
        (f32::from_bits(0xc25145a6), f32::from_bits(0x40b75684)),
    ); // -54.1024f, 8.52157f, -52.318f, 5.72931f
    path.quad_to(
        (f32::from_bits(0xc24a2274), f32::from_bits(0x403bf8b8)),
        (f32::from_bits(0xc23d30a0), f32::from_bits(0x400e5c28)),
    ); // -50.5336f, 2.93706f, -47.2975f, 2.22437f
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xc2478a00), f32::from_bits(0x418639e0))); // -49.8848f, 16.7783f
    path.quad_to(
        (f32::from_bits(0xc252b546), f32::from_bits(0x416fe6f8)),
        (f32::from_bits(0xc2558f0e), f32::from_bits(0x413c1fa8)),
    ); // -52.677f, 14.9939f, -53.3897f, 11.7577f
    path.quad_to(
        (f32::from_bits(0xc25868d8), f32::from_bits(0x41085856)),
        (f32::from_bits(0xc25145a6), f32::from_bits(0x40b75684)),
    ); // -54.1024f, 8.52157f, -52.318f, 5.72931f
    path.quad_to(
        (f32::from_bits(0xc24a2274), f32::from_bits(0x403bf8b8)),
        (f32::from_bits(0xc23d30a0), f32::from_bits(0x400e5c28)),
    ); // -50.5336f, 2.93706f, -47.2975f, 2.22437f
    path.quad_to(
        (f32::from_bits(0xc2303ecc), f32::from_bits(0x3fc17f10)),
        (f32::from_bits(0xc2251387), f32::from_bits(0x4052f2a8)),
    ); // -44.0613f, 1.51169f, -41.2691f, 3.29606f
    path.quad_to(
        (f32::from_bits(0xc219e842), f32::from_bits(0x40a292e2)),
        (f32::from_bits(0xc2170e78), f32::from_bits(0x410510c3)),
    ); // -38.4768f, 5.08043f, -37.7641f, 8.31659f
    path.quad_to(
        (f32::from_bits(0xc21434af), f32::from_bits(0x4138d815)),
        (f32::from_bits(0xc21b57e0), f32::from_bits(0x41658529)),
    ); // -37.0514f, 11.5528f, -38.8358f, 14.345f
    path.quad_to(
        (f32::from_bits(0xc2227b12), f32::from_bits(0x4189191e)),
        (f32::from_bits(0xc22f6ce6), f32::from_bits(0x418eccb0)),
    ); // -40.6202f, 17.1373f, -43.8563f, 17.8499f
    path.quad_to(
        (f32::from_bits(0xc23c5ebc), f32::from_bits(0x41948044)),
        (f32::from_bits(0xc2478a00), f32::from_bits(0x418639e0)),
    ); // -47.0925f, 18.5626f, -49.8848f, 16.7783f
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L760-L859 (chrome/m156)
fn fuzz763_1404(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x419b2e3e), f32::from_bits(0xc243b405)));
    path.quad_to(
        (f32::from_bits(0x41b4811d), f32::from_bits(0xc2479f9a)),
        (f32::from_bits(0x41cbf476), f32::from_bits(0xc2417131)),
    );
    path.quad_to(
        (f32::from_bits(0x41e19882), f32::from_bits(0xc23bbceb)),
        (f32::from_bits(0x41e9f15f), f32::from_bits(0xc23083cb)),
    );
    path.quad_to(
        (f32::from_bits(0x4200ef06), f32::from_bits(0xc2310b91)),
        (f32::from_bits(0x420a6762), f32::from_bits(0xc2294dac)),
    );
    path.quad_to(
        (f32::from_bits(0x4214aa80), f32::from_bits(0xc220ea0a)),
        (f32::from_bits(0x4215fd8a), f32::from_bits(0xc213b9c8)),
    );
    path.quad_to(
        (f32::from_bits(0x42175094), f32::from_bits(0xc2068986)),
        (f32::from_bits(0x420eecf0), f32::from_bits(0xc1f88cd3)),
    );
    path.quad_to(
        (f32::from_bits(0x4206894d), f32::from_bits(0xc1e4069a)),
        (f32::from_bits(0x41f2b216), f32::from_bits(0xc1e16085)),
    );
    path.quad_to(
        (f32::from_bits(0x41d85192), f32::from_bits(0xc1deba71)),
        (f32::from_bits(0x41c3cb5a), f32::from_bits(0xc1ef81b8)),
    );
    path.quad_to(
        (f32::from_bits(0x41b61bc0), f32::from_bits(0xc1fab1e0)),
        (f32::from_bits(0x41b05ee8), f32::from_bits(0xc2051350)),
    );
    path.quad_to(
        (f32::from_bits(0x419fe690), f32::from_bits(0xc204b4a5)),
        (f32::from_bits(0x4190436e), f32::from_bits(0xc208d3d1)),
    );
    path.quad_to(
        (f32::from_bits(0x4171a027), f32::from_bits(0xc20f0238)),
        (f32::from_bits(0x4161f1d7), f32::from_bits(0xc21baba8)),
    );
    path.quad_to(
        (f32::from_bits(0x41524386), f32::from_bits(0xc2285517)),
        (f32::from_bits(0x416afd23), f32::from_bits(0xc2340ec3)),
    );
    path.quad_to(
        (f32::from_bits(0x4181db5f), f32::from_bits(0xc23fc871)),
        (f32::from_bits(0x419b2e3e), f32::from_bits(0xc243b405)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc221f910), f32::from_bits(0xc2067acc)));
    path.quad_to(
        (f32::from_bits(0xc214fb93), f32::from_bits(0xc2091d7b)),
        (f32::from_bits(0xc209ef07), f32::from_bits(0xc201cb14)),
    );
    path.quad_to(
        (f32::from_bits(0xc1fdc4f6), f32::from_bits(0xc1f4f15c)),
        (f32::from_bits(0xc1f87f97), f32::from_bits(0xc1daf662)),
    );
    path.quad_to(
        (f32::from_bits(0xc1f33a38), f32::from_bits(0xc1c0fb68)),
        (f32::from_bits(0xc200ef83), f32::from_bits(0xc1aae250)),
    );
    path.quad_to(
        (f32::from_bits(0xc20841e9), f32::from_bits(0xc194c938)),
        (f32::from_bits(0xc2153f65), f32::from_bits(0xc18f83d9)),
    );
    path.quad_to(
        (f32::from_bits(0xc2223ce2), f32::from_bits(0xc18a3e7a)),
        (f32::from_bits(0xc22d496e), f32::from_bits(0xc198e348)),
    );
    path.quad_to(
        (f32::from_bits(0xc23855fb), f32::from_bits(0xc1a78814)),
        (f32::from_bits(0xc23af8aa), f32::from_bits(0xc1c1830c)),
    );
    path.quad_to(
        (f32::from_bits(0xc23d9b5a), f32::from_bits(0xc1db7e06)),
        (f32::from_bits(0xc23648f3), f32::from_bits(0xc1f1971e)),
    );
    path.quad_to(
        (f32::from_bits(0xc22ef68d), f32::from_bits(0xc203d81c)),
        (f32::from_bits(0xc221f910), f32::from_bits(0xc2067acc)),
    );
    path.close();
    path.move_to((f32::from_bits(0x4218d883), f32::from_bits(0xc1dfb2a2)));
    path.quad_to(
        (f32::from_bits(0x4224b610), f32::from_bits(0xc1eb836c)),
        (f32::from_bits(0x4231475d), f32::from_bits(0xc1e31687)),
    );
    path.quad_to(
        (f32::from_bits(0x423dd8aa), f32::from_bits(0xc1daa9a1)),
        (f32::from_bits(0x4243c10e), f32::from_bits(0xc1c2ee88)),
    );
    path.quad_to(
        (f32::from_bits(0x4249a974), f32::from_bits(0xc1ab336d)),
        (f32::from_bits(0x42457300), f32::from_bits(0xc19210d4)),
    );
    path.quad_to(
        (f32::from_bits(0x42413c8e), f32::from_bits(0xc171dc76)),
        (f32::from_bits(0x42355f01), f32::from_bits(0xc15a3ae1)),
    );
    path.quad_to(
        (f32::from_bits(0x42298174), f32::from_bits(0xc142994c)),
        (f32::from_bits(0x421cf027), f32::from_bits(0xc1537318)),
    );
    path.quad_to(
        (f32::from_bits(0x42105edb), f32::from_bits(0xc1644ce3)),
        (f32::from_bits(0x420a7675), f32::from_bits(0xc189e18c)),
    );
    path.quad_to(
        (f32::from_bits(0x42048e10), f32::from_bits(0xc1a19ca6)),
        (f32::from_bits(0x4208c483), f32::from_bits(0xc1babf40)),
    );
    path.quad_to(
        (f32::from_bits(0x420cfaf6), f32::from_bits(0xc1d3e1d8)),
        (f32::from_bits(0x4218d883), f32::from_bits(0xc1dfb2a2)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)));
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    );
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x4087d82a), f32::from_bits(0x42204637)),
        (f32::from_bits(0x401ecaaa), f32::from_bits(0x422284dc)),
    );
    path.quad_to(
        (f32::from_bits(0x4033f0a5), f32::from_bits(0x4223bc28)),
        (f32::from_bits(0x4047ae10), f32::from_bits(0x4225218a)),
    );
    path.quad_to(
        (f32::from_bits(0x40aa0f54), f32::from_bits(0x422f1027)),
        (f32::from_bits(0x40a38748), f32::from_bits(0x423c4af2)),
    );
    path.quad_to(
        (f32::from_bits(0x409cff44), f32::from_bits(0x424985be)),
        (f32::from_bits(0x401b14b8), f32::from_bits(0x42524cc7)),
    );
    path.quad_to(
        (f32::from_bits(0xbd754800), f32::from_bits(0x425b13d0)),
        (f32::from_bits(0xc05781d0), f32::from_bits(0x425a42cf)),
    );
    path.quad_to(
        (f32::from_bits(0xc0d59744), f32::from_bits(0x425971cf)),
        (f32::from_bits(0xc10de7c8), f32::from_bits(0x424f8332)),
    );
    path.quad_to(
        (f32::from_bits(0xc13103ee), f32::from_bits(0x42459494)),
        (f32::from_bits(0xc12dbfea), f32::from_bits(0x423859c9)),
    );
    path.quad_to(
        (f32::from_bits(0xc12a7be8), f32::from_bits(0x422b1efe)),
        (f32::from_bits(0xc102c172), f32::from_bits(0x422257f4)),
    );
    path.quad_to(
        (f32::from_bits(0xc0dbff18), f32::from_bits(0x421dc1e9)),
        (f32::from_bits(0xc0ab47a0), f32::from_bits(0x421bca58)),
    );
    path.quad_to(
        (f32::from_bits(0xc0ad79af), f32::from_bits(0x421b8a4a)),
        (f32::from_bits(0xc0afa610), f32::from_bits(0x421b4830)),
    );
    path.quad_to(
        (f32::from_bits(0xc0b25ad5), f32::from_bits(0x421af5e2)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)));
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    );
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    );
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    );
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    );
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    );
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    );
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)));
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x401b14b8), f32::from_bits(0x42524cc7)));
    path.quad_to(
        (f32::from_bits(0xbd754800), f32::from_bits(0x425b13d0)),
        (f32::from_bits(0xc05781d0), f32::from_bits(0x425a42cf)),
    );
    path.quad_to(
        (f32::from_bits(0xc0d59744), f32::from_bits(0x425971cf)),
        (f32::from_bits(0xc10de7c8), f32::from_bits(0x424f8332)),
    );
    path.quad_to(
        (f32::from_bits(0xc13103ee), f32::from_bits(0x42459494)),
        (f32::from_bits(0xc12dbfea), f32::from_bits(0x423859c9)),
    );
    path.quad_to(
        (f32::from_bits(0xc12a7be8), f32::from_bits(0x422b1efe)),
        (f32::from_bits(0xc102c172), f32::from_bits(0x422257f4)),
    );
    path.quad_to(
        (f32::from_bits(0xc0b60dfc), f32::from_bits(0x421990ea)),
        (f32::from_bits(0xc0186f48), f32::from_bits(0x421a61ec)),
    );
    path.quad_to(
        (f32::from_bits(0x3f6cf5e0), f32::from_bits(0x421b32ec)),
        (f32::from_bits(0x4047ae10), f32::from_bits(0x4225218a)),
    );
    path.quad_to(
        (f32::from_bits(0x40aa0f54), f32::from_bits(0x422f1027)),
        (f32::from_bits(0x40a38748), f32::from_bits(0x423c4af2)),
    );
    path.quad_to(
        (f32::from_bits(0x409cff44), f32::from_bits(0x424985be)),
        (f32::from_bits(0x401b14b8), f32::from_bits(0x42524cc7)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L861-L945 (chrome/m156)
fn fuzz763_4713(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x40f7bc68), f32::from_bits(0xc2503bb0)));
    path.quad_to(
        (f32::from_bits(0x41299c84), f32::from_bits(0xc256ef4e)),
        (f32::from_bits(0x415ce976), f32::from_bits(0xc2539652)),
    );
    path.quad_to(
        (f32::from_bits(0x41881b33), f32::from_bits(0xc2503d58)),
        (f32::from_bits(0x41958271), f32::from_bits(0xc244cdc4)),
    );
    path.quad_to(
        (f32::from_bits(0x41a2e9af), f32::from_bits(0xc2395e30)),
        (f32::from_bits(0x419c37b8), f32::from_bits(0xc22c8af3)),
    );
    path.quad_to(
        (f32::from_bits(0x419585c2), f32::from_bits(0xc21fb7b7)),
        (f32::from_bits(0x417d4d34), f32::from_bits(0xc2190418)),
    );
    path.quad_to(
        (f32::from_bits(0x414f8ee4), f32::from_bits(0xc2125079)),
        (f32::from_bits(0x411c41f2), f32::from_bits(0xc215a974)),
    );
    path.quad_to(
        (f32::from_bits(0x40d1ea00), f32::from_bits(0xc2190270)),
        (f32::from_bits(0x409c4d08), f32::from_bits(0xc2247204)),
    );
    path.quad_to(
        (f32::from_bits(0x404d6020), f32::from_bits(0xc22fe198)),
        (f32::from_bits(0x408177f0), f32::from_bits(0xc23cb4d4)),
    );
    path.quad_to(
        (f32::from_bits(0x409c3fc8), f32::from_bits(0xc2498810)),
        (f32::from_bits(0x40f7bc68), f32::from_bits(0xc2503bb0)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc20487d4), f32::from_bits(0xc2239250)));
    path.quad_to(
        (f32::from_bits(0xc1ee8d37), f32::from_bits(0xc2239d4e)),
        (f32::from_bits(0xc1dbbeef), f32::from_bits(0xc21a45b5)),
    );
    path.quad_to(
        (f32::from_bits(0xc1c8f0a7), f32::from_bits(0xc210ee1d)),
        (f32::from_bits(0xc1c8daab), f32::from_bits(0xc203ace5)),
    );
    path.quad_to(
        (f32::from_bits(0xc1c8c4af), f32::from_bits(0xc1ecd758)),
        (f32::from_bits(0xc1db73e0), f32::from_bits(0xc1da0910)),
    );
    path.quad_to(
        (f32::from_bits(0xc1ee2310), f32::from_bits(0xc1c73ac7)),
        (f32::from_bits(0xc20452c1), f32::from_bits(0xc1c724cb)),
    );
    path.quad_to(
        (f32::from_bits(0xc21193f9), f32::from_bits(0xc1c70ecf)),
        (f32::from_bits(0xc21afb1d), f32::from_bits(0xc1d9be01)),
    );
    path.quad_to(
        (f32::from_bits(0xc2246242), f32::from_bits(0xc1ec6d31)),
        (f32::from_bits(0xc2246d40), f32::from_bits(0xc20377d2)),
    );
    path.quad_to(
        (f32::from_bits(0xc224783e), f32::from_bits(0xc210b90a)),
        (f32::from_bits(0xc21b20a5), f32::from_bits(0xc21a202d)),
    );
    path.quad_to(
        (f32::from_bits(0xc211c90c), f32::from_bits(0xc2238752)),
        (f32::from_bits(0xc20487d4), f32::from_bits(0xc2239250)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)));
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    );
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc23c7b18), f32::from_bits(0x40950470)));
    path.quad_to(
        (f32::from_bits(0xc22fb33d), f32::from_bits(0x4071d1b0)),
        (f32::from_bits(0xc2242dae), f32::from_bits(0x40ad5534)),
    );
    path.quad_to(
        (f32::from_bits(0xc218a81f), f32::from_bits(0x40e1c194)),
        (f32::from_bits(0xc21524ab), f32::from_bits(0x41240037)),
    );
    path.quad_to(
        (f32::from_bits(0xc211a138), f32::from_bits(0x41571fa5)),
        (f32::from_bits(0xc2182ec4), f32::from_bits(0x41829af0)),
    );
    path.quad_to(
        (f32::from_bits(0xc21ebc50), f32::from_bits(0x4199a610)),
        (f32::from_bits(0xc22b842b), f32::from_bits(0x41a0acf4)),
    );
    path.quad_to(
        (f32::from_bits(0xc2384c07), f32::from_bits(0x41a7b3dc)),
        (f32::from_bits(0xc243d196), f32::from_bits(0x419a98c4)),
    );
    path.quad_to(
        (f32::from_bits(0xc24f5726), f32::from_bits(0x418d7dad)),
        (f32::from_bits(0xc252da98), f32::from_bits(0x4167dbea)),
    );
    path.quad_to(
        (f32::from_bits(0xc2565e0c), f32::from_bits(0x4134bc7e)),
        (f32::from_bits(0xc24fd080), f32::from_bits(0x4106a640)),
    );
    path.quad_to(
        (f32::from_bits(0xc24942f4), f32::from_bits(0x40b12008)),
        (f32::from_bits(0xc23c7b18), f32::from_bits(0x40950470)),
    );
    path.close();
    path.move_to((f32::from_bits(0x4204f72e), f32::from_bits(0x41c56cd2)));
    path.quad_to(
        (f32::from_bits(0x42123842), f32::from_bits(0x41c52adf)),
        (f32::from_bits(0x421baed7), f32::from_bits(0x41d7bac6)),
    );
    path.quad_to(
        (f32::from_bits(0x4225256d), f32::from_bits(0x41ea4aad)),
        (f32::from_bits(0x42254667), f32::from_bits(0x4202666b)),
    );
    path.quad_to(
        (f32::from_bits(0x42256760), f32::from_bits(0x420fa77f)),
        (f32::from_bits(0x421c1f6c), f32::from_bits(0x42191e14)),
    );
    path.quad_to(
        (f32::from_bits(0x421bff97), f32::from_bits(0x42193e89)),
        (f32::from_bits(0x421bdf6b), f32::from_bits(0x42195eb8)),
    );
    path.quad_to(
        (f32::from_bits(0x421bbff6), f32::from_bits(0x42197f32)),
        (f32::from_bits(0x421ba03b), f32::from_bits(0x42199f57)),
    );
    path.quad_to(
        (f32::from_bits(0x421b605e), f32::from_bits(0x4219e00a)),
        (f32::from_bits(0x421b1fa8), f32::from_bits(0x421a1f22)),
    );
    path.quad_to(
        (f32::from_bits(0x421ae0f1), f32::from_bits(0x421a604b)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x41db19b1), f32::from_bits(0x41da63d5)),
        (f32::from_bits(0x41db755b), f32::from_bits(0x41da0a9b)),
    );
    path.quad_to(
        (f32::from_bits(0x41dbce01), f32::from_bits(0x41d9ae59)),
        (f32::from_bits(0x41dc285e), f32::from_bits(0x41d952ce)),
    );
    path.quad_to(
        (f32::from_bits(0x41dc55b6), f32::from_bits(0x41d924df)),
        (f32::from_bits(0x41dc82cd), f32::from_bits(0x41d8f7cd)),
    );
    path.quad_to(
        (f32::from_bits(0x41dcaf1e), f32::from_bits(0x41d8ca01)),
        (f32::from_bits(0x41dcdc4c), f32::from_bits(0x41d89bf0)),
    );
    path.quad_to(
        (f32::from_bits(0x41ef6c33), f32::from_bits(0x41c5aec5)),
        (f32::from_bits(0x4204f72e), f32::from_bits(0x41c56cd2)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42240000), f32::from_bits(0x42040000)));
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L947-L1038 (chrome/m156)
fn fuzz763_24588(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x413a5194), f32::from_bits(0xc24d4e33))); // 11.6449f, -51.3264f
    path.quad_to(
        (f32::from_bits(0x4169f3fc), f32::from_bits(0xc2532032)),
        (f32::from_bits(0x418e0c8b), f32::from_bits(0xc24ed218)),
    ); // 14.6221f, -52.7814f, 17.7561f, -51.7052f
    path.quad_to(
        (f32::from_bits(0x41a71f17), f32::from_bits(0xc24a83ff)),
        (f32::from_bits(0x41b2c316), f32::from_bits(0xc23e9b65)),
    ); // 20.8902f, -50.6289f, 22.3453f, -47.6518f
    path.quad_to(
        (f32::from_bits(0x41be6714), f32::from_bits(0xc232b2cb)),
        (f32::from_bits(0x41b5cae0), f32::from_bits(0xc2262985)),
    ); // 23.8003f, -44.6746f, 22.7241f, -41.5405f
    path.quad_to(
        (f32::from_bits(0x41ad2ead), f32::from_bits(0xc219a03f)),
        (f32::from_bits(0x41955d79), f32::from_bits(0xc213ce40)),
    ); // 21.6478f, -38.4065f, 18.6706f, -36.9514f
    path.quad_to(
        (f32::from_bits(0x417b188a), f32::from_bits(0xc20dfc40)),
        (f32::from_bits(0x4148f373), f32::from_bits(0xc2124a5a)),
    ); // 15.6935f, -35.4963f, 12.5594f, -36.5726f
    path.quad_to(
        (f32::from_bits(0x4116ce5a), f32::from_bits(0xc2169874)),
        (f32::from_bits(0x40ff0cba), f32::from_bits(0xc222810e)),
    ); // 9.42538f, -37.6489f, 7.9703f, -40.626f
    path.quad_to(
        (f32::from_bits(0x40d07cc0), f32::from_bits(0xc22e69a8)),
        (f32::from_bits(0x40f2ed90), f32::from_bits(0xc23af2ee)),
    ); // 6.51523f, -43.6032f, 7.5915f, -46.7372f
    path.quad_to(
        (f32::from_bits(0x410aaf2c), f32::from_bits(0xc2477c34)),
        (f32::from_bits(0x413a5194), f32::from_bits(0xc24d4e33)),
    ); // 8.66777f, -49.8713f, 11.6449f, -51.3264f
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455))); // -19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    ); // -11.598f, -28, 0, -28
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    ); // 11.598f, -28, 19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    ); // 28, -11.598f, 28, 0
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    ); // 28, 11.598f, 19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    ); // 13.7163f, 25.8817f, 5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    ); // 8, 29.7635f, 8, 33
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    ); // 8, 36.3137f, 5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    ); // 3.31371f, 41, 0, 41
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    ); // -3.31371f, 41, -5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    ); // -8, 36.3137f, -8, 33
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    ); // -8, 29.7635f, -5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    ); // -13.7163f, 25.8817f, -19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    ); // -28, 11.598f, -28, 0
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    ); // -28, -11.598f, -19.799f, -19.799f
    path.close();
    path.move_to((f32::from_bits(0xc23d594a), f32::from_bits(0x3f8b9aa0))); // -47.3372f, 1.09066f
    path.quad_to(
        (f32::from_bits(0xc23056ee), f32::from_bits(0x3ee95200)),
        (f32::from_bits(0xc2255841), f32::from_bits(0x40139cf0)),
    ); // -44.0849f, 0.455704f, -41.3362f, 2.30645f
    path.quad_to(
        (f32::from_bits(0xc21a5994), f32::from_bits(0x408507d0)),
        (f32::from_bits(0xc217cf63), f32::from_bits(0x40ed1ab6)),
    ); // -38.5875f, 4.1572f, -37.9525f, 7.40951f
    path.quad_to(
        (f32::from_bits(0xc21747fe), f32::from_bits(0x41016369)),
        (f32::from_bits(0xc2172ef9), f32::from_bits(0x410bdff7)),
    ); // -37.8203f, 8.08677f, -37.7959f, 8.74218f
    path.quad_to(
        (f32::from_bits(0xc2161ebf), f32::from_bits(0x411577cf)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    ); // -37.53f, 9.34175f, -37.3514f, 10.0083f
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    ); // -36.4938f, 13.2091f, -38.1506f, 16.0788f
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    ); // -39.8075f, 18.9486f, -43.0083f, 19.8062f
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    ); // -46.2091f, 20.6639f, -49.0788f, 19.007f
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    ); // -51.9486f, 17.3502f, -52.8062f, 14.1494f
    path.quad_to(
        (f32::from_bits(0xc2543df8), f32::from_bits(0x415334fe)),
        (f32::from_bits(0xc2546005), f32::from_bits(0x41447d40)),
    ); // -53.0605f, 13.2004f, -53.0938f, 12.2806f
    path.quad_to(
        (f32::from_bits(0xc255df09), f32::from_bits(0x41370862)),
        (f32::from_bits(0xc2569fcc), f32::from_bits(0x41279af0)),
    ); // -53.4678f, 11.4395f, -53.6561f, 10.4753f
    path.quad_to(
        (f32::from_bits(0xc25929fe), f32::from_bits(0x40e722fc)),
        (f32::from_bits(0xc251c2d2), f32::from_bits(0x408f2d94)),
    ); // -54.291f, 7.22302f, -52.4403f, 4.47431f
    path.quad_to(
        (f32::from_bits(0xc24a5ba8), f32::from_bits(0x3fdce0a0)),
        (f32::from_bits(0xc23d594a), f32::from_bits(0x3f8b9aa0)),
    ); // -50.5895f, 1.72561f, -47.3372f, 1.09066f
    path.close();
    path.move_to((f32::from_bits(0xc18b14a2), f32::from_bits(0x42164b25))); // -17.3851f, 37.5734f
    path.quad_to(
        (f32::from_bits(0xc1675bab), f32::from_bits(0x421010e4)),
        (f32::from_bits(0xc134a62c), f32::from_bits(0x4213efa5)),
    ); // -14.4599f, 36.0165f, -11.2906f, 36.984f
    path.quad_to(
        (f32::from_bits(0xc101f0aa), f32::from_bits(0x4217ce66)),
        (f32::from_bits(0xc0d20f46), f32::from_bits(0x422381cc)),
    ); // -8.12126f, 37.9516f, -6.56436f, 40.8768f
    path.quad_to(
        (f32::from_bits(0xc0a03d38), f32::from_bits(0x422f3532)),
        (f32::from_bits(0xc0bf3344), f32::from_bits(0x423be292)),
    ); // -5.00747f, 43.8019f, -5.97501f, 46.9713f
    path.quad_to(
        (f32::from_bits(0xc0de294c), f32::from_bits(0x42488ff2)),
        (f32::from_bits(0xc11de23e), f32::from_bits(0x424eca34)),
    ); // -6.94254f, 50.1406f, -9.86773f, 51.6975f
    path.quad_to(
        (f32::from_bits(0xc14cafd4), f32::from_bits(0x42550476)),
        (f32::from_bits(0xc17f6556), f32::from_bits(0x425125b4)),
    ); // -12.7929f, 53.2544f, -15.9622f, 52.2868f
    path.quad_to(
        (f32::from_bits(0xc1990d6c), f32::from_bits(0x424d46f3)),
        (f32::from_bits(0xc1a581f0), f32::from_bits(0x4241938e)),
    ); // -19.1316f, 51.3193f, -20.6884f, 48.3941f
    path.quad_to(
        (f32::from_bits(0xc1b1f673), f32::from_bits(0x4235e028)),
        (f32::from_bits(0xc1aa38f0), f32::from_bits(0x422932c8)),
    ); // -22.2453f, 45.4689f, -21.2778f, 42.2996f
    path.quad_to(
        (f32::from_bits(0xc1a27b6c), f32::from_bits(0x421c8567)),
        (f32::from_bits(0xc18b14a2), f32::from_bits(0x42164b25)),
    ); // -20.3103f, 39.1303f, -17.3851f, 37.5734f
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3))); // 27.3431f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    ); // 29.6863f, 25, 33, 25
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    ); // 36.3137f, 25, 38.6569f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    ); // 41, 29.6863f, 41, 33
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    ); // 41, 36.3137f, 38.6569f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x4217d943), f32::from_bits(0x421d67f9)),
        (f32::from_bits(0x4214ba8d), f32::from_bits(0x421f5c6e)),
    ); // 37.9622f, 39.3515f, 37.1822f, 39.8403f
    path.quad_to(
        (f32::from_bits(0x42129039), f32::from_bits(0x422256bd)),
        (f32::from_bits(0x420f9986), f32::from_bits(0x4224eb5c)),
    ); // 36.6408f, 40.5847f, 35.8999f, 41.2298f
    path.quad_to(
        (f32::from_bits(0x420e25cf), f32::from_bits(0x42262f05)),
        (f32::from_bits(0x420ca0a4), f32::from_bits(0x42273ec0)),
    ); // 35.5369f, 41.5459f, 35.1569f, 41.8113f
    path.quad_to(
        (f32::from_bits(0x420b5228), f32::from_bits(0x42288f80)),
        (f32::from_bits(0x4209d382), f32::from_bits(0x4229c624)),
    ); // 34.8302f, 42.1401f, 34.4566f, 42.4435f
    path.quad_to(
        (f32::from_bits(0x41ff1232), f32::from_bits(0x423220d2)),
        (f32::from_bits(0x41e4b406), f32::from_bits(0x4230c247)),
    ); // 31.8839f, 44.5321f, 28.5879f, 44.1897f
    path.quad_to(
        (f32::from_bits(0x41ca55dc), f32::from_bits(0x422f63bd)),
        (f32::from_bits(0x41b9a084), f32::from_bits(0x42251952)),
    ); // 25.2919f, 43.8474f, 23.2034f, 41.2747f
    path.quad_to(
        (f32::from_bits(0x41a8eb2b), f32::from_bits(0x421acee9)),
        (f32::from_bits(0x41aba840), f32::from_bits(0x420d9fd3)),
    ); // 21.1148f, 38.7021f, 21.4572f, 35.4061f
    path.quad_to(
        (f32::from_bits(0x41ae6555), f32::from_bits(0x420070be)),
        (f32::from_bits(0x41c2fa28), f32::from_bits(0x41f02c24)),
    ); // 21.7995f, 32.1101f, 24.3721f, 30.0216f
    path.quad_to(
        (f32::from_bits(0x41c514db), f32::from_bits(0x41ee76d0)),
        (f32::from_bits(0x41c73f0f), f32::from_bits(0x41ecf584)),
    ); // 24.6352f, 29.808f, 24.9058f, 29.6199f
    path.quad_to(
        (f32::from_bits(0x41c919c1), f32::from_bits(0x41eb15ab)),
        (f32::from_bits(0x41cb250b), f32::from_bits(0x41e94e07)),
    ); // 25.1376f, 29.3856f, 25.3931f, 29.1631f
    path.quad_to(
        (f32::from_bits(0x41cf4ed6), f32::from_bits(0x41e5ae04)),
        (f32::from_bits(0x41d3c048), f32::from_bits(0x41e2e387)),
    ); // 25.9135f, 28.71f, 26.4689f, 28.3611f
    path.quad_to(
        (f32::from_bits(0x41d4d649), f32::from_bits(0x41e1661f)),
        (f32::from_bits(0x41d605fb), f32::from_bits(0x41dff35b)),
    ); // 26.6046f, 28.1749f, 26.7529f, 27.9938f
    path.quad_to(
        (f32::from_bits(0x41d7094e), f32::from_bits(0x41deb6c2)),
        (f32::from_bits(0x41d81f4d), f32::from_bits(0x41dd81fd)),
    ); // 26.8795f, 27.8392f, 27.0153f, 27.6885f
    path.line_to((f32::from_bits(0x41d81f53), f32::from_bits(0x41dd81f7))); // 27.0153f, 27.6885f
    path.quad_to(
        (f32::from_bits(0x41d96269), f32::from_bits(0x41dc1b1d)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    ); // 27.1731f, 27.5132f, 27.3431f, 27.3431f
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x421a8288), f32::from_bits(0x420efdef))); // 38.6275f, 35.748f
    path.quad_to(
        (f32::from_bits(0x4219989b), f32::from_bits(0x421c3719)),
        (f32::from_bits(0x420f9986), f32::from_bits(0x4224eb5c)),
    ); // 38.399f, 39.0538f, 35.8999f, 41.2298f
    path.quad_to(
        (f32::from_bits(0x42059a71), f32::from_bits(0x422d9f9f)),
        (f32::from_bits(0x41f0c28e), f32::from_bits(0x422cb5b2)),
    ); // 33.4008f, 43.4059f, 30.095f, 43.1774f
    path.quad_to(
        (f32::from_bits(0x41d65038), f32::from_bits(0x422bcbc5)),
        (f32::from_bits(0x41c4e7b3), f32::from_bits(0x4221ccb0)),
    ); // 26.7892f, 42.949f, 24.6131f, 40.4499f
    path.quad_to(
        (f32::from_bits(0x41b37f2c), f32::from_bits(0x4217cd9b)),
        (f32::from_bits(0x41b55306), f32::from_bits(0x420a9471)),
    ); // 22.4371f, 37.9508f, 22.6655f, 34.645f
    path.quad_to(
        (f32::from_bits(0x41b726e0), f32::from_bits(0x41fab68c)),
        (f32::from_bits(0x41cb250b), f32::from_bits(0x41e94e07)),
    ); // 22.894f, 31.3391f, 25.3931f, 29.1631f
    path.quad_to(
        (f32::from_bits(0x41df2336), f32::from_bits(0x41d7e580)),
        (f32::from_bits(0x41f9958b), f32::from_bits(0x41d9b95a)),
    ); // 27.8922f, 26.9871f, 31.198f, 27.2155f
    path.quad_to(
        (f32::from_bits(0x420a03ef), f32::from_bits(0x41db8d34)),
        (f32::from_bits(0x4212b832), f32::from_bits(0x41ef8b5f)),
    ); // 34.5038f, 27.4439f, 36.6799f, 29.9431f
    path.quad_to(
        (f32::from_bits(0x421b6c75), f32::from_bits(0x4201c4c5)),
        (f32::from_bits(0x421a8288), f32::from_bits(0x420efdef)),
    ); // 38.8559f, 32.4422f, 38.6275f, 35.748f
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// SkDQuadIntersection.cpp:594: failed assertion "way_roughly_zero(fT[0][index])
// Port of: tests/PathOpsFuzz763Test.cpp#L1041-L1141 (chrome/m156)
fn fuzz763_20016(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x41e88c66), f32::from_bits(0xc22f800b))); // 29.0686f, -43.875f
    path.quad_to(
        (f32::from_bits(0x420178e8), f32::from_bits(0xc230b9b9)),
        (f32::from_bits(0x420babd1), f32::from_bits(0xc228426b)),
    ); // 32.3681f, -44.1814f, 34.9178f, -42.0649f
    path.quad_to(
        (f32::from_bits(0x4215debb), f32::from_bits(0xc21fcb1e)),
        (f32::from_bits(0x42171869), f32::from_bits(0xc2129869)),
    ); // 37.4675f, -39.9484f, 37.7738f, -36.6488f
    path.quad_to(
        (f32::from_bits(0x42185217), f32::from_bits(0xc20565b3)),
        (f32::from_bits(0x420fdac9), f32::from_bits(0xc1f66594)),
    ); // 38.0802f, -33.3493f, 35.9637f, -30.7996f
    path.quad_to(
        (f32::from_bits(0x4207637c), f32::from_bits(0xc1e1ffc1)),
        (f32::from_bits(0x41f4618e), f32::from_bits(0xc1df8c65)),
    ); // 33.8472f, -28.2499f, 30.5476f, -27.9436f
    path.quad_to(
        (f32::from_bits(0x41d9fc22), f32::from_bits(0xc1dd190a)),
        (f32::from_bits(0x41c59650), f32::from_bits(0xc1ee07a4)),
    ); // 27.2481f, -27.6372f, 24.6984f, -29.7537f
    path.quad_to(
        (f32::from_bits(0x41b1307c), f32::from_bits(0xc1fef63e)),
        (f32::from_bits(0x41aebd21), f32::from_bits(0xc20cadd5)),
    ); // 22.1487f, -31.8702f, 21.8423f, -35.1698f
    path.quad_to(
        (f32::from_bits(0x41ac49c5), f32::from_bits(0xc219e08a)),
        (f32::from_bits(0x41bd3860), f32::from_bits(0xc2241373)),
    ); // 21.536f, -38.4693f, 23.6525f, -41.019f
    path.quad_to(
        (f32::from_bits(0x41ce26fa), f32::from_bits(0xc22e465d)),
        (f32::from_bits(0x41e88c66), f32::from_bits(0xc22f800b)),
    ); // 25.769f, -43.5687f, 29.0686f, -43.875f
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455))); // -19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    ); // -11.598f, -28, 0, -28
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    ); // 11.598f, -28, 19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    ); // 28, -11.598f, 28, 0
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    ); // 28, 11.598f, 19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    ); // 13.7163f, 25.8817f, 5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    ); // 8, 29.7635f, 8, 33
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x42100ef9)),
        (f32::from_bits(0x40c1f194), f32::from_bits(0x4218e765)),
    ); // 8, 36.0146f, 6.06074f, 38.226f
    path.quad_to(
        (f32::from_bits(0x4100db87), f32::from_bits(0x42186e1c)),
        (f32::from_bits(0x411cd246), f32::from_bits(0x421bbc51)),
    ); // 8.0536f, 38.1075f, 9.80134f, 38.9339f
    path.quad_to(
        (f32::from_bits(0x41326be0), f32::from_bits(0x42146b95)),
        (f32::from_bits(0x4156b15b), f32::from_bits(0x421110b0)),
    ); // 11.1513f, 37.1051f, 13.4183f, 36.2663f
    path.quad_to(
        (f32::from_bits(0x41843577), f32::from_bits(0x420c7739)),
        (f32::from_bits(0x419c4b3e), f32::from_bits(0x421200ec)),
    ); // 16.5261f, 35.1164f, 19.5367f, 36.5009f
    path.quad_to(
        (f32::from_bits(0x41b46104), f32::from_bits(0x42178a9f)),
        (f32::from_bits(0x41bd93f2), f32::from_bits(0x4223f904)),
    ); // 22.5474f, 37.8854f, 23.6972f, 40.9932f
    path.quad_to(
        (f32::from_bits(0x41c6c6e0), f32::from_bits(0x42306768)),
        (f32::from_bits(0x41bbb37b), f32::from_bits(0x423c724c)),
    ); // 24.8471f, 44.101f, 23.4626f, 47.1116f
    path.quad_to(
        (f32::from_bits(0x41b0a015), f32::from_bits(0x42487d2f)),
        (f32::from_bits(0x4197c34c), f32::from_bits(0x424d16a6)),
    ); // 22.0782f, 50.1222f, 18.9704f, 51.2721f
    path.quad_to(
        (f32::from_bits(0x417dcd04), f32::from_bits(0x4251b01e)),
        (f32::from_bits(0x414da178), f32::from_bits(0x424c266a)),
    ); // 15.8626f, 52.422f, 12.8519f, 51.0375f
    path.quad_to(
        (f32::from_bits(0x414d992c), f32::from_bits(0x424c2576)),
        (f32::from_bits(0x414d90e0), f32::from_bits(0x424c2481)),
    ); // 12.8499f, 51.0366f, 12.8479f, 51.0356f
    path.quad_to(
        (f32::from_bits(0x414d8b5f), f32::from_bits(0x424c2655)),
        (f32::from_bits(0x414d85dc), f32::from_bits(0x424c2828)),
    ); // 12.8465f, 51.0374f, 12.8452f, 51.0392f
    path.quad_to(
        (f32::from_bits(0x412d952c), f32::from_bits(0x4256bc8e)),
        (f32::from_bits(0x40f225c0), f32::from_bits(0x4258923c)),
    ); // 10.8489f, 53.6841f, 7.56711f, 54.1428f
    path.quad_to(
        (f32::from_bits(0x4089212c), f32::from_bits(0x425a67eb)),
        (f32::from_bits(0x3fd1f7e0), f32::from_bits(0x42526bbf)),
    ); // 4.2853f, 54.6015f, 1.64038f, 52.6052f
    path.quad_to(
        (f32::from_bits(0xbf8094f0), f32::from_bits(0x424a6f94)),
        (f32::from_bits(0xbfbb4ab0), f32::from_bits(0x423d4f00)),
    ); // -1.00455f, 50.609f, -1.46322f, 47.3271f
    path.quad_to(
        (f32::from_bits(0xbff60080), f32::from_bits(0x42302e6e)),
        (f32::from_bits(0x3d985000), f32::from_bits(0x42259a07)),
    ); // -1.92189f, 44.0453f, 0.0743713f, 41.4004f
    path.quad_to(
        (f32::from_bits(0x3e6fb042), f32::from_bits(0x4224c15b)),
        (f32::from_bits(0x3ecdd2ea), f32::from_bits(0x4223f703)),
    ); // 0.234071f, 41.1888f, 0.402f, 40.9912f
    path.quad_to(
        (f32::from_bits(0x3e4fb040), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    ); // 0.202821f, 41, 0, 41
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    ); // -3.31371f, 41, -5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    ); // -8, 36.3137f, -8, 33
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    ); // -8, 29.7635f, -5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    ); // -13.7163f, 25.8817f, -19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    ); // -28, 11.598f, -28, 0
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    ); // -28, -11.598f, -19.799f, -19.799f
    path.close();
    path.move_to((f32::from_bits(0xc24a80c4), f32::from_bits(0xc16637fc))); // -50.6257f, -14.3887f
    path.quad_to(
        (f32::from_bits(0xc23faf8f), f32::from_bits(0xc1826e03)),
        (f32::from_bits(0xc2329eca), f32::from_bits(0xc17bee18)),
    ); // -47.9214f, -16.3037f, -44.6551f, -15.7456f
    path.quad_to(
        (f32::from_bits(0xc2258e06), f32::from_bits(0xc173002c)),
        (f32::from_bits(0xc21de504), f32::from_bits(0xc147bb58)),
    ); // -41.3887f, -15.1875f, -39.4736f, -12.4832f
    path.quad_to(
        (f32::from_bits(0xc2163c02), f32::from_bits(0xc11c7684)),
        (f32::from_bits(0xc218777c), f32::from_bits(0xc0d066e4)),
    ); // -37.5586f, -9.77893f, -38.1167f, -6.51256f
    path.quad_to(
        (f32::from_bits(0xc21ab2f8), f32::from_bits(0xc04fc188)),
        (f32::from_bits(0xc225842d), f32::from_bits(0xbfaa62c0)),
    ); // -38.6748f, -3.24619f, -41.3791f, -1.33114f
    path.quad_to(
        (f32::from_bits(0xc2305562), f32::from_bits(0x3f157b20)),
        (f32::from_bits(0xc23d6626), f32::from_bits(0x3cd38800)),
    ); // -44.0834f, 0.58391f, -47.3498f, 0.0258217f
    path.quad_to(
        (f32::from_bits(0xc24a76ea), f32::from_bits(0xbf084280)),
        (f32::from_bits(0xc2521fed), f32::from_bits(0xc04f23f0)),
    ); // -50.6161f, -0.532265f, -52.5312f, -3.23657f
    path.quad_to(
        (f32::from_bits(0xc259c8f0), f32::from_bits(0xc0be1ba0)),
        (f32::from_bits(0xc2578d74), f32::from_bits(0xc11350e2)),
    ); // -54.4462f, -5.94087f, -53.8881f, -9.20725f
    path.quad_to(
        (f32::from_bits(0xc25551f9), f32::from_bits(0xc14793f2)),
        (f32::from_bits(0xc24a80c4), f32::from_bits(0xc16637fc)),
    ); // -53.3301f, -12.4736f, -50.6257f, -14.3887f
    path.close();
    path.move_to((f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec))); // -47.1494f, 4.35143f
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    ); // -43.9486f, 3.49378f, -41.0788f, 5.15063f
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    ); // -38.2091f, 6.80749f, -37.3514f, 10.0083f
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    ); // -36.4938f, 13.2091f, -38.1506f, 16.0788f
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    ); // -39.8075f, 18.9486f, -43.0083f, 19.8062f
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    ); // -46.2091f, 20.6639f, -49.0788f, 19.007f
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    ); // -51.9486f, 17.3502f, -52.8062f, 14.1494f
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    ); // -53.6639f, 10.9486f, -52.007f, 8.07884f
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    ); // -50.3502f, 5.20908f, -47.1494f, 4.35143f
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3))); // 27.3431f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    ); // 29.6863f, 25, 33, 25
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    ); // 36.3137f, 25, 38.6569f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    ); // 41, 29.6863f, 41, 33
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    ); // 41, 36.3137f, 38.6569f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    ); // 36.3137f, 41, 33, 41
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    ); // 29.6863f, 41, 27.3431f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    ); // 25, 36.3137f, 25, 33
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    ); // 25, 29.6863f, 27.3431f, 27.3431f
    path.close();
    path.move_to((f32::from_bits(0xc1beec1e), f32::from_bits(0x4207519a))); // -23.8653f, 33.8297f
    path.quad_to(
        (f32::from_bits(0xc1a5a92e), f32::from_bits(0x42034ca6)),
        (f32::from_bits(0xc18e1d36), f32::from_bits(0x42096379)),
    ); // -20.7076f, 32.8249f, -17.7643f, 34.3471f
    path.quad_to(
        (f32::from_bits(0xc16d2279), f32::from_bits(0x420f7a4d)),
        (f32::from_bits(0xc15d0ea8), f32::from_bits(0x421c1bc5)),
    ); // -14.8209f, 35.8694f, -13.8161f, 39.0271f
    path.quad_to(
        (f32::from_bits(0xc14cfad8), f32::from_bits(0x4228bd3d)),
        (f32::from_bits(0xc1655627), f32::from_bits(0x42348339)),
    ); // -12.8112f, 42.1848f, -14.3335f, 45.1281f
    path.quad_to(
        (f32::from_bits(0xc17db174), f32::from_bits(0x42404936)),
        (f32::from_bits(0xc1981baa), f32::from_bits(0x42444e2a)),
    ); // -15.8558f, 48.0715f, -19.0135f, 49.0763f
    path.quad_to(
        (f32::from_bits(0xc1b15e9a), f32::from_bits(0x4248531e)),
        (f32::from_bits(0xc1c8ea94), f32::from_bits(0x42423c4a)),
    ); // -22.1712f, 50.0812f, -25.1145f, 48.5589f
    path.quad_to(
        (f32::from_bits(0xc1e0768c), f32::from_bits(0x423c2577)),
        (f32::from_bits(0xc1e88074), f32::from_bits(0x422f83ff)),
    ); // -28.0579f, 47.0366f, -29.0627f, 43.8789f
    path.quad_to(
        (f32::from_bits(0xc1f08a5c), f32::from_bits(0x4222e287)),
        (f32::from_bits(0xc1e45cb6), f32::from_bits(0x42171c8a)),
    ); // -30.0676f, 40.7212f, -28.5453f, 37.7779f
    path.quad_to(
        (f32::from_bits(0xc1d82f0e), f32::from_bits(0x420b568e)),
        (f32::from_bits(0xc1beec1e), f32::from_bits(0x4207519a)),
    ); // -27.023f, 34.8345f, -23.8653f, 33.8297f
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41bbb37b), f32::from_bits(0x423c724c))); // 23.4626f, 47.1116f
    path.quad_to(
        (f32::from_bits(0x41b0a015), f32::from_bits(0x42487d2f)),
        (f32::from_bits(0x4197c34c), f32::from_bits(0x424d16a6)),
    ); // 22.0782f, 50.1222f, 18.9704f, 51.2721f
    path.quad_to(
        (f32::from_bits(0x417dcd04), f32::from_bits(0x4251b01e)),
        (f32::from_bits(0x414da178), f32::from_bits(0x424c266a)),
    ); // 15.8626f, 52.422f, 12.8519f, 51.0375f
    path.quad_to(
        (f32::from_bits(0x411d75ea), f32::from_bits(0x42469cb8)),
        (f32::from_bits(0x410b100e), f32::from_bits(0x423a2e53)),
    ); // 9.84129f, 49.653f, 8.69142f, 46.5452f
    path.quad_to(
        (f32::from_bits(0x40f15460), f32::from_bits(0x422dbfee)),
        (f32::from_bits(0x410ed0fc), f32::from_bits(0x4221b50b)),
    ); // 7.54155f, 43.4374f, 8.92602f, 40.4268f
    path.quad_to(
        (f32::from_bits(0x4124f7c7), f32::from_bits(0x4215aa28)),
        (f32::from_bits(0x4156b15b), f32::from_bits(0x421110b0)),
    ); // 10.3105f, 37.4162f, 13.4183f, 36.2663f
    path.quad_to(
        (f32::from_bits(0x41843577), f32::from_bits(0x420c7739)),
        (f32::from_bits(0x419c4b3e), f32::from_bits(0x421200ec)),
    ); // 16.5261f, 35.1164f, 19.5367f, 36.5009f
    path.quad_to(
        (f32::from_bits(0x41b46104), f32::from_bits(0x42178a9f)),
        (f32::from_bits(0x41bd93f2), f32::from_bits(0x4223f904)),
    ); // 22.5474f, 37.8854f, 23.6972f, 40.9932f
    path.quad_to(
        (f32::from_bits(0x41c6c6e0), f32::from_bits(0x42306768)),
        (f32::from_bits(0x41bbb37b), f32::from_bits(0x423c724c)),
    ); // 24.8471f, 44.101f, 23.4626f, 47.1116f
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L1143-L1249 (chrome/m156)
fn fuzz763_17370(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x41fb8980), f32::from_bits(0xc20d9cf4))); // 31.4421f, -35.4033f
    path.quad_to(
        (f32::from_bits(0x42081e43), f32::from_bits(0xc215e4e6)),
        (f32::from_bits(0x42154ac6), f32::from_bits(0xc2146e6f)),
    ); // 34.0296f, -37.4735f, 37.323f, -37.1078f
    path.quad_to(
        (f32::from_bits(0x4222774a), f32::from_bits(0xc212f7f8)),
        (f32::from_bits(0x422abf3a), f32::from_bits(0xc2089e76)),
    ); // 40.6165f, -36.7422f, 42.6867f, -34.1547f
    path.quad_to(
        (f32::from_bits(0x4233072c), f32::from_bits(0xc1fc89e5)),
        (f32::from_bits(0x423190b5), f32::from_bits(0xc1e230df)),
    ); // 44.757f, -31.5673f, 44.3913f, -28.2739f
    path.quad_to(
        (f32::from_bits(0x42301a3e), f32::from_bits(0xc1c7d7d8)),
        (f32::from_bits(0x4225c0bc), f32::from_bits(0xc1b747f6)),
    ); // 44.0256f, -24.9804f, 41.4382f, -22.9101f
    path.quad_to(
        (f32::from_bits(0x421b6738), f32::from_bits(0xc1a6b815)),
        (f32::from_bits(0x420e3ab6), f32::from_bits(0xc1a9a502)),
    ); // 38.8508f, -20.8399f, 35.5573f, -21.2056f
    path.quad_to(
        (f32::from_bits(0x42010e32), f32::from_bits(0xc1ac91ef)),
        (f32::from_bits(0x41f18c82), f32::from_bits(0xc1c144f4)),
    ); // 32.2639f, -21.5713f, 30.1936f, -24.1587f
    path.quad_to(
        (f32::from_bits(0x41e0fca1), f32::from_bits(0xc1d5f7fa)),
        (f32::from_bits(0x41e3e98e), f32::from_bits(0xc1f05101)),
    ); // 28.1234f, -26.7461f, 28.489f, -30.0396f
    path.quad_to(
        (f32::from_bits(0x41e6d67b), f32::from_bits(0xc2055504)),
        (f32::from_bits(0x41fb8980), f32::from_bits(0xc20d9cf4)),
    ); // 28.8547f, -33.333f, 31.4421f, -35.4033f
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455))); // -19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    ); // -11.598f, -28, 0, -28
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    ); // 11.598f, -28, 19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    ); // 28, -11.598f, 28, 0
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    ); // 28, 11.598f, 19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    ); // 13.7163f, 25.8817f, 5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    ); // 8, 29.7635f, 8, 33
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x420ff6ba)),
        (f32::from_bits(0x40c2ea2a), f32::from_bits(0x4218c3c6)),
    ); // 8, 35.9909f, 6.09108f, 38.1912f
    path.quad_to(
        (f32::from_bits(0x41135b8a), f32::from_bits(0x42173bf5)),
        (f32::from_bits(0x413c53fb), f32::from_bits(0x421ec49c)),
    ); // 9.20985f, 37.8086f, 11.7705f, 39.692f
    path.quad_to(
        (f32::from_bits(0x416709c9), f32::from_bits(0x42269f2a)),
        (f32::from_bits(0x416f0674), f32::from_bits(0x4233b9ad)),
    ); // 14.4399f, 41.6554f, 14.9391f, 44.9313f
    path.quad_to(
        (f32::from_bits(0x41770320), f32::from_bits(0x4240d431)),
        (f32::from_bits(0x415798ee), f32::from_bits(0x424b81a4)),
    ); // 15.4383f, 48.2072f, 13.4748f, 50.8766f
    path.quad_to(
        (f32::from_bits(0x41382eba), f32::from_bits(0x42562f18)),
        (f32::from_bits(0x4103c4ac), f32::from_bits(0x42582e42)),
    ); // 11.5114f, 53.546f, 8.23552f, 54.0452f
    path.quad_to(
        (f32::from_bits(0x409eb53c), f32::from_bits(0x425a2d6e)),
        (f32::from_bits(0x40129340), f32::from_bits(0x425252e0)),
    ); // 4.95962f, 54.5444f, 2.29024f, 52.5809f
    path.quad_to(
        (f32::from_bits(0x3ee54581), f32::from_bits(0x424ce72c)),
        (f32::from_bits(0xbeb8b807), f32::from_bits(0x4244fb36)),
    ); // 0.447796f, 51.2258f, -0.360779f, 49.2453f
    path.quad_to(
        (f32::from_bits(0xbf99615c), f32::from_bits(0x424cdad2)),
        (f32::from_bits(0xc043dd58), f32::from_bits(0x42522abc)),
    ); // -1.19828f, 51.2137f, -3.06038f, 52.5417f
    path.quad_to(
        (f32::from_bits(0xc0b84398), f32::from_bits(0x4259dd06)),
        (f32::from_bits(0xc1106c72), f32::from_bits(0x4257acc2)),
    ); // -5.75825f, 54.4658f, -9.02648f, 53.9187f
    path.quad_to(
        (f32::from_bits(0xc144b71a), f32::from_bits(0x42557c80)),
        (f32::from_bits(0xc163803e), f32::from_bits(0x424ab1e2)),
    ); // -12.2947f, 53.3716f, -14.2188f, 50.6737f
    path.quad_to(
        (f32::from_bits(0xc18124b1), f32::from_bits(0x423fe745)),
        (f32::from_bits(0xc1798856), f32::from_bits(0x4232d49b)),
    ); // -16.1429f, 47.9758f, -15.5958f, 44.7076f
    path.quad_to(
        (f32::from_bits(0xc170c74c), f32::from_bits(0x4225c1f0)),
        (f32::from_bits(0xc1459cd6), f32::from_bits(0x421e0fa8)),
    ); // -15.0487f, 41.4394f, -12.3508f, 39.5153f
    path.quad_to(
        (f32::from_bits(0xc11a7260), f32::from_bits(0x42165d5e)),
        (f32::from_bits(0xc0cc4f70), f32::from_bits(0x42188da2)),
    ); // -9.65292f, 37.5912f, -6.3847f, 38.1383f
    path.quad_to(
        (f32::from_bits(0xc0c78c19), f32::from_bits(0x4218a726)),
        (f32::from_bits(0xc0c2e01e), f32::from_bits(0x4218c538)),
    ); // -6.23585f, 38.1632f, -6.08986f, 38.1926f
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x420ff7b5)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    ); // -8, 35.9919f, -8, 33
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    ); // -8, 29.7635f, -5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    ); // -13.7163f, 25.8817f, -19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    ); // -28, 11.598f, -28, 0
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    ); // -28, -11.598f, -19.799f, -19.799f
    path.close();
    path.move_to((f32::from_bits(0xc23ab9e7), f32::from_bits(0xc1c274dd))); // -46.6815f, -24.3071f
    path.quad_to(
        (f32::from_bits(0xc22e95fe), f32::from_bits(0xc1cd18ca)),
        (f32::from_bits(0xc2223d50), f32::from_bits(0xc1c373a5)),
    ); // -43.6465f, -25.6371f, -40.5599f, -24.4315f
    path.quad_to(
        (f32::from_bits(0xc215e4a2), f32::from_bits(0xc1b9ce80)),
        (f32::from_bits(0xc21092ac), f32::from_bits(0xc1a186ac)),
    ); // -37.4733f, -23.2258f, -36.1432f, -20.1908f
    path.quad_to(
        (f32::from_bits(0xc20b40b5), f32::from_bits(0xc1893ed9)),
        (f32::from_bits(0xc2101348), f32::from_bits(0xc1611afc)),
    ); // -34.8132f, -17.1557f, -36.0188f, -14.0691f
    path.quad_to(
        (f32::from_bits(0xc214e5da), f32::from_bits(0xc12fb844)),
        (f32::from_bits(0xc22109c4), f32::from_bits(0xc11a706a)),
    ); // -37.2245f, -10.9825f, -40.2595f, -9.65244f
    path.quad_to(
        (f32::from_bits(0xc22d2dae), f32::from_bits(0xc1052890)),
        (f32::from_bits(0xc239865c), f32::from_bits(0xc11872dd)),
    ); // -43.2946f, -8.3224f, -46.3812f, -9.52804f
    path.quad_to(
        (f32::from_bits(0xc245df09), f32::from_bits(0xc12bbd26)),
        (f32::from_bits(0xc24b3100), f32::from_bits(0xc15c4ccc)),
    ); // -49.4678f, -10.7337f, -50.7979f, -13.7687f
    path.quad_to(
        (f32::from_bits(0xc25082f6), f32::from_bits(0xc1866e3a)),
        (f32::from_bits(0xc24bb063), f32::from_bits(0xc19f1f96)),
    ); // -52.1279f, -16.8038f, -50.9223f, -19.8904f
    path.quad_to(
        (f32::from_bits(0xc246ddd1), f32::from_bits(0xc1b7d0f0)),
        (f32::from_bits(0xc23ab9e7), f32::from_bits(0xc1c274dd)),
    ); // -49.7166f, -22.977f, -46.6815f, -24.3071f
    path.close();
    path.move_to((f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec))); // -47.1494f, 4.35143f
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    ); // -43.9486f, 3.49378f, -41.0788f, 5.15063f
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    ); // -38.2091f, 6.80749f, -37.3514f, 10.0083f
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    ); // -36.4938f, 13.2091f, -38.1506f, 16.0788f
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    ); // -39.8075f, 18.9486f, -43.0083f, 19.8062f
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    ); // -46.2091f, 20.6639f, -49.0788f, 19.007f
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    ); // -51.9486f, 17.3502f, -52.8062f, 14.1494f
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    ); // -53.6639f, 10.9486f, -52.007f, 8.07884f
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    ); // -50.3502f, 5.20908f, -47.1494f, 4.35143f
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3))); // 27.3431f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    ); // 29.6863f, 25, 33, 25
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    ); // 36.3137f, 25, 38.6569f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    ); // 41, 29.6863f, 41, 33
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    ); // 41, 36.3137f, 38.6569f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    ); // 36.3137f, 41, 33, 41
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    ); // 29.6863f, 41, 27.3431f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    ); // 25, 36.3137f, 25, 33
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    ); // 25, 29.6863f, 27.3431f, 27.3431f
    path.close();
    path.move_to((f32::from_bits(0xc1d961c1), f32::from_bits(0x41f9e1da))); // -27.1727f, 31.2353f
    path.quad_to(
        (f32::from_bits(0xc1bf6f78), f32::from_bits(0x41f47252)),
        (f32::from_bits(0xc1a93eb6), f32::from_bits(0x42017995)),
    ); // -23.9294f, 30.5558f, -21.1556f, 32.3687f
    path.quad_to(
        (f32::from_bits(0xc1930df4), f32::from_bits(0x4208ba01)),
        (f32::from_bits(0xc18d9e6c), f32::from_bits(0x4215b325)),
    ); // -18.3818f, 34.1816f, -17.7024f, 37.4249f
    path.quad_to(
        (f32::from_bits(0xc1882ee5), f32::from_bits(0x4222ac49)),
        (f32::from_bits(0xc196afbd), f32::from_bits(0x422dc4aa)),
    ); // -17.0229f, 40.6682f, -18.8358f, 43.4421f
    path.quad_to(
        (f32::from_bits(0xc1a53094), f32::from_bits(0x4238dd0c)),
        (f32::from_bits(0xc1bf22dd), f32::from_bits(0x423b94cf)),
    ); // -20.6487f, 46.2159f, -23.892f, 46.8953f
    path.quad_to(
        (f32::from_bits(0xc1d91525), f32::from_bits(0x423e4c93)),
        (f32::from_bits(0xc1ef45e7), f32::from_bits(0x42370c27)),
    ); // -27.1353f, 47.5748f, -29.9091f, 45.7619f
    path.quad_to(
        (f32::from_bits(0xc202bb55), f32::from_bits(0x422fcbbc)),
        (f32::from_bits(0xc2057319), f32::from_bits(0x4222d298)),
    ); // -32.6829f, 43.949f, -33.3624f, 40.7057f
    path.quad_to(
        (f32::from_bits(0xc2082adc), f32::from_bits(0x4215d973)),
        (f32::from_bits(0xc200ea70), f32::from_bits(0x420ac112)),
    ); // -34.0419f, 37.4624f, -32.2289f, 34.6885f
    path.quad_to(
        (f32::from_bits(0xc1f35409), f32::from_bits(0x41ff5161)),
        (f32::from_bits(0xc1d961c1), f32::from_bits(0x41f9e1da)),
    ); // -30.416f, 31.9147f, -27.1727f, 31.2353f
    path.close();
    path.move_to((f32::from_bits(0xbfccf162), f32::from_bits(0x42236913))); // -1.60112f, 40.8526f
    path.quad_to(
        (f32::from_bits(0xbf54c171), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    ); // -0.831077f, 41, 0, 41
    path.quad_to(
        (f32::from_bits(0x3dcda9e6), f32::from_bits(0x42240000)),
        (f32::from_bits(0x3e4cbe2e), f32::from_bits(0x4223fdcc)),
    ); // 0.100422f, 41, 0.199944f, 40.9978f
    path.quad_to(
        (f32::from_bits(0x3f12dfd9), f32::from_bits(0x4223f586)),
        (f32::from_bits(0x3f6f571b), f32::from_bits(0x4223ce2c)),
    ); // 0.573728f, 40.9898f, 0.934923f, 40.9513f
    path.quad_to(
        (f32::from_bits(0x3f4168dc), f32::from_bits(0x4224a9bb)),
        (f32::from_bits(0x3f15fde0), f32::from_bits(0x422595d9)),
    ); // 0.755506f, 41.1658f, 0.585905f, 41.3963f
    path.quad_to(
        (f32::from_bits(0x3d27275b), f32::from_bits(0x42288cb9)),
        (f32::from_bits(0xbea10331), f32::from_bits(0x422bb375)),
    ); // 0.040809f, 42.1374f, -0.314477f, 42.9253f
    path.quad_to(
        (f32::from_bits(0xbf287eab), f32::from_bits(0x4228877a)),
        (f32::from_bits(0xbf989f50), f32::from_bits(0x42258882)),
    ); // -0.658183f, 42.1323f, -1.19236f, 41.3833f
    path.quad_to(
        (f32::from_bits(0xbfb1e0f9), f32::from_bits(0x42246d34)),
        (f32::from_bits(0xbfccf162), f32::from_bits(0x42236913)),
    ); // -1.38968f, 41.1066f, -1.60112f, 40.8526f
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x415798ee), f32::from_bits(0x424b81a4))); // 13.4748f, 50.8766f
    path.quad_to(
        (f32::from_bits(0x41382eba), f32::from_bits(0x42562f18)),
        (f32::from_bits(0x4103c4ac), f32::from_bits(0x42582e42)),
    ); // 11.5114f, 53.546f, 8.23552f, 54.0452f
    path.quad_to(
        (f32::from_bits(0x409eb53c), f32::from_bits(0x425a2d6e)),
        (f32::from_bits(0x40129340), f32::from_bits(0x425252e0)),
    ); // 4.95962f, 54.5444f, 2.29024f, 52.5809f
    path.quad_to(
        (f32::from_bits(0xbec21f80), f32::from_bits(0x424a7854)),
        (f32::from_bits(0xbf60da80), f32::from_bits(0x423d5dd0)),
    ); // -0.379147f, 50.6175f, -0.878334f, 47.3416f
    path.quad_to(
        (f32::from_bits(0xbfb052b0), f32::from_bits(0x4230434d)),
        (f32::from_bits(0x3f15fde0), f32::from_bits(0x422595d9)),
    ); // -1.37752f, 44.0657f, 0.585905f, 41.3963f
    path.quad_to(
        (f32::from_bits(0x40232840), f32::from_bits(0x421ae866)),
        (f32::from_bits(0x40ba6840), f32::from_bits(0x4218e93b)),
    ); // 2.54933f, 38.727f, 5.82523f, 38.2278f
    path.quad_to(
        (f32::from_bits(0x41119e2f), f32::from_bits(0x4216ea10)),
        (f32::from_bits(0x413c53fb), f32::from_bits(0x421ec49c)),
    ); // 9.10112f, 37.7286f, 11.7705f, 39.692f
    path.quad_to(
        (f32::from_bits(0x416709c9), f32::from_bits(0x42269f2a)),
        (f32::from_bits(0x416f0674), f32::from_bits(0x4233b9ad)),
    ); // 14.4399f, 41.6554f, 14.9391f, 44.9313f
    path.quad_to(
        (f32::from_bits(0x41770320), f32::from_bits(0x4240d431)),
        (f32::from_bits(0x415798ee), f32::from_bits(0x424b81a4)),
    ); // 15.4383f, 48.2072f, 13.4748f, 50.8766f
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// SkDQuadIntersection.cpp:598: failed assertion "way_roughly_equal(fT[0][index], 1)"
// Port of: tests/PathOpsFuzz763Test.cpp#L1252-L1356 (chrome/m156)
fn fuzz763_35322(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x41042400), f32::from_bits(0xc24fea42))); // 8.25879f, -51.9788f
    path.quad_to(
        (f32::from_bits(0x413225f2), f32::from_bits(0xc25680b3)),
        (f32::from_bits(0x4165502a), f32::from_bits(0xc2530720)),
    ); // 11.1343f, -53.6257f, 14.3321f, -52.757f
    path.quad_to(
        (f32::from_bits(0x418c3d32), f32::from_bits(0xc24f8d8e)),
        (f32::from_bits(0x41996a13), f32::from_bits(0xc2440d11)),
    ); // 17.5299f, -51.8882f, 19.1768f, -49.0128f
    path.quad_to(
        (f32::from_bits(0x41a696f3), f32::from_bits(0xc2388c95)),
        (f32::from_bits(0x419fa3cc), f32::from_bits(0xc22bc206)),
    ); // 20.8237f, -46.1373f, 19.955f, -42.9395f
    path.quad_to(
        (f32::from_bits(0x4198b0a8), f32::from_bits(0xc21ef778)),
        (f32::from_bits(0x4181afaf), f32::from_bits(0xc2186108)),
    ); // 19.0863f, -39.7417f, 16.2108f, -38.0948f
    path.quad_to(
        (f32::from_bits(0x41555d6e), f32::from_bits(0xc211ca98)),
        (f32::from_bits(0x41223335), f32::from_bits(0xc215442b)),
    ); // 13.3353f, -36.4478f, 10.1375f, -37.3166f
    path.quad_to(
        (f32::from_bits(0x40de11f8), f32::from_bits(0xc218bdbe)),
        (f32::from_bits(0x40a95e78), f32::from_bits(0xc2243e3a)),
    ); // 6.93969f, -38.1853f, 5.29278f, -41.0608f
    path.quad_to(
        (f32::from_bits(0x406955e8), f32::from_bits(0xc22fbeb6)),
        (f32::from_bits(0x4090778c), f32::from_bits(0xc23c8944)),
    ); // 3.64587f, -43.9362f, 4.51459f, -47.134f
    path.quad_to(
        (f32::from_bits(0x40ac4420), f32::from_bits(0xc24953d2)),
        (f32::from_bits(0x41042400), f32::from_bits(0xc24fea42)),
    ); // 5.38332f, -50.3319f, 8.25879f, -51.9788f
    path.close();
    path.move_to((f32::from_bits(0x41cb7543), f32::from_bits(0xc2385013))); // 25.4323f, -46.0782f
    path.quad_to(
        (f32::from_bits(0x41e591fd), f32::from_bits(0xc23a9972)),
        (f32::from_bits(0x41fb44a5), f32::from_bits(0xc232fbf5)),
    ); // 28.6963f, -46.6498f, 31.4085f, -44.7461f
    path.quad_to(
        (f32::from_bits(0x42087ba8), f32::from_bits(0xc22b5e79)),
        (f32::from_bits(0x420ac507), f32::from_bits(0xc21e501c)),
    ); // 34.1208f, -42.8423f, 34.6924f, -39.5782f
    path.quad_to(
        (f32::from_bits(0x420d0e67), f32::from_bits(0xc21141bf)),
        (f32::from_bits(0x420570ea), f32::from_bits(0xc206686a)),
    ); // 35.2641f, -36.3142f, 33.3603f, -33.602f
    path.quad_to(
        (f32::from_bits(0x41fba6dc), f32::from_bits(0xc1f71e2c)),
        (f32::from_bits(0x41e18a22), f32::from_bits(0xc1f28b6c)),
    ); // 31.4565f, -30.8897f, 28.1924f, -30.3181f
    path.quad_to(
        (f32::from_bits(0x41c76d67), f32::from_bits(0xc1edf8ad)),
        (f32::from_bits(0x41b1babe), f32::from_bits(0xc1fd33a6)),
    ); // 24.9284f, -29.7464f, 22.2162f, -31.6502f
    path.quad_to(
        (f32::from_bits(0x419c0815), f32::from_bits(0xc2063750)),
        (f32::from_bits(0x41977556), f32::from_bits(0xc21345ad)),
    ); // 19.5039f, -33.554f, 18.9323f, -36.818f
    path.quad_to(
        (f32::from_bits(0x4192e296), f32::from_bits(0xc220540a)),
        (f32::from_bits(0x41a21d8f), f32::from_bits(0xc22b2d5e)),
    ); // 18.3606f, -40.0821f, 20.2644f, -42.7943f
    path.quad_to(
        (f32::from_bits(0x41b15888), f32::from_bits(0xc23606b3)),
        (f32::from_bits(0x41cb7543), f32::from_bits(0xc2385013)),
    ); // 22.1682f, -45.5065f, 25.4323f, -46.0782f
    path.close();
    path.move_to((f32::from_bits(0x4206de71), f32::from_bits(0xc204f99f))); // 33.7172f, -33.2438f
    path.quad_to(
        (f32::from_bits(0x4211be80), f32::from_bits(0xc20c8d7c)),
        (f32::from_bits(0x421ecad2), f32::from_bits(0xc20a388c)),
    ); // 36.436f, -35.1382f, 39.6981f, -34.5552f
    path.quad_to(
        (f32::from_bits(0x422bd724), f32::from_bits(0xc207e39b)),
        (f32::from_bits(0x42336b00), f32::from_bits(0xc1fa0718)),
    ); // 42.9601f, -33.9723f, 44.8545f, -31.2535f
    path.quad_to(
        (f32::from_bits(0x423afedd), f32::from_bits(0xc1e446f9)),
        (f32::from_bits(0x4238a9ec), f32::from_bits(0xc1ca2e57)),
    ); // 46.7489f, -28.5347f, 46.1659f, -25.2726f
    path.quad_to(
        (f32::from_bits(0x423654fc), f32::from_bits(0xc1b015b3)),
        (f32::from_bits(0x422b74ed), f32::from_bits(0xc1a0edfa)),
    ); // 45.583f, -22.0106f, 42.8642f, -20.1162f
    path.quad_to(
        (f32::from_bits(0x422094de), f32::from_bits(0xc191c640)),
        (f32::from_bits(0x4213888c), f32::from_bits(0xc1967021)),
    ); // 40.1454f, -18.2218f, 36.8833f, -18.8048f
    path.quad_to(
        (f32::from_bits(0x42067c3a), f32::from_bits(0xc19b1a02)),
        (f32::from_bits(0x41fdd0bc), f32::from_bits(0xc1b0da20)),
    ); // 33.6213f, -19.3877f, 31.7269f, -22.1065f
    path.quad_to(
        (f32::from_bits(0x41eea902), f32::from_bits(0xc1c69a3f)),
        (f32::from_bits(0x41f352e3), f32::from_bits(0xc1e0b2e3)),
    ); // 29.8325f, -24.8253f, 30.4155f, -28.0873f
    path.quad_to(
        (f32::from_bits(0x41f7fcc4), f32::from_bits(0xc1facb85)),
        (f32::from_bits(0x4206de71), f32::from_bits(0xc204f99f)),
    ); // 30.9984f, -31.3494f, 33.7172f, -33.2438f
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455))); // -19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    ); // -11.598f, -28, 0, -28
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    ); // 11.598f, -28, 19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    ); // 28, -11.598f, 28, 0
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    ); // 28, 11.598f, 19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    ); // 13.7163f, 25.8817f, 5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    ); // 8, 29.7635f, 8, 33
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    ); // 8, 36.3137f, 5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x40b4fa00), f32::from_bits(0x421aa1fd)),
        (f32::from_bits(0x40b4ef0c), f32::from_bits(0x421aa35b)),
    ); // 5.65552f, 38.6582f, 5.65418f, 38.6595f
    path.quad_to(
        (f32::from_bits(0x40f11f68), f32::from_bits(0x421c342c)),
        (f32::from_bits(0x4111ddac), f32::from_bits(0x42218978)),
    ); // 7.53508f, 39.0509f, 9.11662f, 40.3842f
    path.quad_to(
        (f32::from_bits(0x413a6700), f32::from_bits(0x422a1497)),
        (f32::from_bits(0x413ee6c0), f32::from_bits(0x42374996)),
    ); // 11.6501f, 42.5201f, 11.9313f, 45.8219f
    path.quad_to(
        (f32::from_bits(0x41436686), f32::from_bits(0x42447e96)),
        (f32::from_bits(0x41213a06), f32::from_bits(0x424ea0eb)),
    ); // 12.2125f, 49.1236f, 10.0767f, 51.6571f
    path.quad_to(
        (f32::from_bits(0x40fe1b10), f32::from_bits(0x4258c340)),
        (f32::from_bits(0x40947314), f32::from_bits(0x4259e330)),
    ); // 7.9408f, 54.1907f, 4.63905f, 54.4719f
    path.quad_to(
        (f32::from_bits(0x3fab2c60), f32::from_bits(0x425b0321)),
        (f32::from_bits(0xbf991e40), f32::from_bits(0x42527802)),
    ); // 1.33729f, 54.7531f, -1.19624f, 52.6172f
    path.quad_to(
        (f32::from_bits(0xc06eb470), f32::from_bits(0x4249ece2)),
        (f32::from_bits(0xc08059b8), f32::from_bits(0x423cb7e2)),
    ); // -3.72976f, 50.4813f, -4.01095f, 47.1796f
    path.quad_to(
        (f32::from_bits(0xc0895940), f32::from_bits(0x422f82e3)),
        (f32::from_bits(0xc00a0088), f32::from_bits(0x4225608e)),
    ); // -4.29214f, 43.8778f, -2.15628f, 41.3443f
    path.quad_to(
        (f32::from_bits(0xbff725c6), f32::from_bits(0x42244eb9)),
        (f32::from_bits(0xbfd8a182), f32::from_bits(0x4223569a)),
    ); // -1.93084f, 41.0769f, -1.69243f, 40.8346f
    path.quad_to(
        (f32::from_bits(0xc07bec59), f32::from_bits(0x42218277)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    ); // -3.9363f, 40.3774f, -5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    ); // -8, 36.3137f, -8, 33
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    ); // -8, 29.7635f, -5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    ); // -13.7163f, 25.8817f, -19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    ); // -28, 11.598f, -28, 0
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    ); // -28, -11.598f, -19.799f, -19.799f
    path.close();
    path.move_to((f32::from_bits(0xc233e0fb), f32::from_bits(0xc1dac1ac))); // -44.9697f, -27.3446f
    path.quad_to(
        (f32::from_bits(0xc22769b6), f32::from_bits(0xc1e3c410)),
        (f32::from_bits(0xc21b69b6), f32::from_bits(0xc1d881ca)),
    ); // -41.8532f, -28.4707f, -38.8532f, -27.0634f
    path.quad_to(
        (f32::from_bits(0xc20f69b6), f32::from_bits(0xc1cd3f84)),
        (f32::from_bits(0xc20ae884), f32::from_bits(0xc1b450fa)),
    ); // -35.8532f, -25.656f, -34.7271f, -22.5395f
    path.quad_to(
        (f32::from_bits(0xc2066752), f32::from_bits(0xc19b6270)),
        (f32::from_bits(0xc20c0875), f32::from_bits(0xc1836270)),
    ); // -33.6009f, -19.4231f, -35.0083f, -16.4231f
    path.quad_to(
        (f32::from_bits(0xc211a998), f32::from_bits(0xc156c4e1)),
        (f32::from_bits(0xc21e20dd), f32::from_bits(0xc144c018)),
    ); // -36.4156f, -13.4231f, -39.5321f, -12.2969f
    path.quad_to(
        (f32::from_bits(0xc22a9822), f32::from_bits(0xc132bb50)),
        (f32::from_bits(0xc2369821), f32::from_bits(0xc1493fdc)),
    ); // -42.6486f, -11.1707f, -45.6486f, -12.5781f
    path.quad_to(
        (f32::from_bits(0xc2429822), f32::from_bits(0xc15fc467)),
        (f32::from_bits(0xc2471954), f32::from_bits(0xc188d0be)),
    ); // -48.6486f, -13.9854f, -49.7747f, -17.1019f
    path.quad_to(
        (f32::from_bits(0xc24b9a86), f32::from_bits(0xc1a1bf48)),
        (f32::from_bits(0xc245f962), f32::from_bits(0xc1b9bf47)),
    ); // -50.9009f, -20.2184f, -49.4935f, -23.2184f
    path.quad_to(
        (f32::from_bits(0xc2405840), f32::from_bits(0xc1d1bf48)),
        (f32::from_bits(0xc233e0fb), f32::from_bits(0xc1dac1ac)),
    ); // -48.0862f, -26.2184f, -44.9697f, -27.3446f
    path.close();
    path.move_to((f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec))); // -47.1494f, 4.35143f
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    ); // -43.9486f, 3.49378f, -41.0788f, 5.15063f
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    ); // -38.2091f, 6.80749f, -37.3514f, 10.0083f
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    ); // -36.4938f, 13.2091f, -38.1506f, 16.0788f
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    ); // -39.8075f, 18.9486f, -43.0083f, 19.8062f
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    ); // -46.2091f, 20.6639f, -49.0788f, 19.007f
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    ); // -51.9486f, 17.3502f, -52.8062f, 14.1494f
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    ); // -53.6639f, 10.9486f, -52.007f, 8.07884f
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    ); // -50.3502f, 5.20908f, -47.1494f, 4.35143f
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3))); // 27.3431f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    ); // 29.6863f, 25, 33, 25
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    ); // 36.3137f, 25, 38.6569f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    ); // 41, 29.6863f, 41, 33
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    ); // 41, 36.3137f, 38.6569f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    ); // 36.3137f, 41, 33, 41
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    ); // 29.6863f, 41, 27.3431f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    ); // 25, 36.3137f, 25, 33
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    ); // 25, 29.6863f, 27.3431f, 27.3431f
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41213a06), f32::from_bits(0x424ea0eb))); // 10.0767f, 51.6571f
    path.quad_to(
        (f32::from_bits(0x40fe1b10), f32::from_bits(0x4258c340)),
        (f32::from_bits(0x40947314), f32::from_bits(0x4259e330)),
    ); // 7.9408f, 54.1907f, 4.63905f, 54.4719f
    path.quad_to(
        (f32::from_bits(0x3fab2c60), f32::from_bits(0x425b0321)),
        (f32::from_bits(0xbf991e40), f32::from_bits(0x42527802)),
    ); // 1.33729f, 54.7531f, -1.19624f, 52.6172f
    path.quad_to(
        (f32::from_bits(0xc06eb470), f32::from_bits(0x4249ece2)),
        (f32::from_bits(0xc08059b8), f32::from_bits(0x423cb7e2)),
    ); // -3.72976f, 50.4813f, -4.01095f, 47.1796f
    path.quad_to(
        (f32::from_bits(0xc0895940), f32::from_bits(0x422f82e3)),
        (f32::from_bits(0xc00a0088), f32::from_bits(0x4225608e)),
    ); // -4.29214f, 43.8778f, -2.15628f, 41.3443f
    path.quad_to(
        (f32::from_bits(0xbca74400), f32::from_bits(0x421b3e39)),
        (f32::from_bits(0x40520168), f32::from_bits(0x421a1e48)),
    ); // -0.0204182f, 38.8108f, 3.28134f, 38.5296f
    path.quad_to(
        (f32::from_bits(0x40d2a8b0), f32::from_bits(0x4218fe58)),
        (f32::from_bits(0x4111ddac), f32::from_bits(0x42218978)),
    ); // 6.58309f, 38.2484f, 9.11662f, 40.3842f
    path.quad_to(
        (f32::from_bits(0x413a6700), f32::from_bits(0x422a1497)),
        (f32::from_bits(0x413ee6c0), f32::from_bits(0x42374996)),
    ); // 11.6501f, 42.5201f, 11.9313f, 45.8219f
    path.quad_to(
        (f32::from_bits(0x41436686), f32::from_bits(0x42447e96)),
        (f32::from_bits(0x41213a06), f32::from_bits(0x424ea0eb)),
    ); // 12.2125f, 49.1236f, 10.0767f, 51.6571f
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// SkPathOpsOp.cpp:52: failed assertion "angle != firstAngle || !loop"
// Port of: tests/PathOpsFuzz763Test.cpp#L1359-L1449 (chrome/m156)
fn fuzz763_849020(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x411e4374), f32::from_bits(0xc24ec58f))); // 9.89147f, -51.6929f
    path.quad_to(
        (f32::from_bits(0x414d13fa), f32::from_bits(0xc254fe70)),
        (f32::from_bits(0x417fc7a6), f32::from_bits(0xc2511e30)),
    ); // 12.8174f, -53.2485f, 15.9862f, -52.2795f
    path.quad_to(
        (f32::from_bits(0x41993dab), f32::from_bits(0xc24d3df4)),
        (f32::from_bits(0x41a5af6e), f32::from_bits(0xc24189d2)),
    ); // 19.1551f, -51.3105f, 20.7107f, -48.3846f
    path.quad_to(
        (f32::from_bits(0x41b22132), f32::from_bits(0xc235d5b1)),
        (f32::from_bits(0x41aa60b2), f32::from_bits(0xc22928c4)),
    ); // 22.2662f, -45.4587f, 21.2972f, -42.2898f
    path.quad_to(
        (f32::from_bits(0x41a2a038), f32::from_bits(0xc21c7bda)),
        (f32::from_bits(0x418b37f4), f32::from_bits(0xc21642f8)),
    ); // 20.3282f, -39.1209f, 17.4023f, -37.5654f
    path.quad_to(
        (f32::from_bits(0x41679f65), f32::from_bits(0xc2100a16)),
        (f32::from_bits(0x4134ebb5), f32::from_bits(0xc213ea55)),
    ); // 14.4764f, -36.0098f, 11.3075f, -36.9788f
    path.quad_to(
        (f32::from_bits(0x41023808), f32::from_bits(0xc217ca94)),
        (f32::from_bits(0x40d2a902), f32::from_bits(0xc2237eb5)),
    ); // 8.13868f, -37.9478f, 6.58313f, -40.8737f
    path.quad_to(
        (f32::from_bits(0x40a0e1f4), f32::from_bits(0xc22f32d6)),
        (f32::from_bits(0x40bfe3ec), f32::from_bits(0xc23bdfc1)),
    ); // 5.02758f, -43.7996f, 5.99657f, -46.9685f
    path.quad_to(
        (f32::from_bits(0x40dee5e0), f32::from_bits(0xc2488cad)),
        (f32::from_bits(0x411e4374), f32::from_bits(0xc24ec58f)),
    ); // 6.96556f, -50.1374f, 9.89147f, -51.6929f
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455))); // -19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    ); // -11.598f, -28, 0, -28
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    ); // 11.598f, -28, 19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    ); // 28, -11.598f, 28, 0
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    ); // 28, 11.598f, 19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    ); // 13.7163f, 25.8817f, 5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    ); // 8, 29.7635f, 8, 33
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    ); // 8, 36.3137f, 5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    ); // 3.31371f, 41, 0, 41
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    ); // -3.31371f, 41, -5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    ); // -8, 36.3137f, -8, 33
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    ); // -8, 29.7635f, -5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    ); // -13.7163f, 25.8817f, -19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    ); // -28, 11.598f, -28, 0
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    ); // -28, -11.598f, -19.799f, -19.799f
    path.close();
    path.move_to((f32::from_bits(0xc23d1735), f32::from_bits(0x402cdce8))); // -47.2727f, 2.70098f
    path.quad_to(
        (f32::from_bits(0xc2302ce7), f32::from_bits(0x3ffa54f0)),
        (f32::from_bits(0xc224ef5c), f32::from_bits(0x406d8f00)),
    ); // -44.0438f, 1.95572f, -41.2337f, 3.71185f
    path.quad_to(
        (f32::from_bits(0xc219b1d2), f32::from_bits(0x40aef9c4)),
        (f32::from_bits(0xc216b6ab), f32::from_bits(0x410b261b)),
    ); // -38.4237f, 5.46799f, -37.6784f, 8.6968f
    path.quad_to(
        (f32::from_bits(0xc2166795), f32::from_bits(0x411080a8)),
        (f32::from_bits(0xc2163402), f32::from_bits(0x4115c8a0)),
    ); // -37.6012f, 9.03141f, -37.5508f, 9.36148f
    path.quad_to(
        (f32::from_bits(0xc215c2e1), f32::from_bits(0x411ad33f)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    ); // -37.4403f, 9.67657f, -37.3514f, 10.0083f
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    ); // -36.4938f, 13.2091f, -38.1506f, 16.0788f
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    ); // -39.8075f, 18.9486f, -43.0083f, 19.8062f
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    ); // -46.2091f, 20.6639f, -49.0788f, 19.007f
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    ); // -51.9486f, 17.3502f, -52.8062f, 14.1494f
    path.quad_to(
        (f32::from_bits(0xc253bae4), f32::from_bits(0x415ad9be)),
        (f32::from_bits(0xc2540461), f32::from_bits(0x41536cea)),
    ); // -52.9325f, 13.6782f, -53.0043f, 13.2141f
    path.quad_to(
        (f32::from_bits(0xc254a293), f32::from_bits(0x414c5480)),
        (f32::from_bits(0xc25512ee), f32::from_bits(0x4144b962)),
    ); // -53.1588f, 12.7706f, -53.2685f, 12.2953f
    path.quad_to(
        (f32::from_bits(0xc2580e14), f32::from_bits(0x41111028)),
        (f32::from_bits(0xc25107cc), f32::from_bits(0x40c833fc)),
    ); // -54.0137f, 9.06644f, -52.2576f, 6.25635f
    path.quad_to(
        (f32::from_bits(0xc24a0183), f32::from_bits(0x405c8f50)),
        (f32::from_bits(0xc23d1735), f32::from_bits(0x402cdce8)),
    ); // -50.5015f, 3.44625f, -47.2727f, 2.70098f
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3))); // 27.3431f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    ); // 29.6863f, 25, 33, 25
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    ); // 36.3137f, 25, 38.6569f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    ); // 41, 29.6863f, 41, 33
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    ); // 41, 36.3137f, 38.6569f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x421a60e3), f32::from_bits(0x421ae059)),
        (f32::from_bits(0x421a205f), f32::from_bits(0x421b1e77)),
    ); // 38.5946f, 38.7191f, 38.5316f, 38.7798f
    path.line_to((f32::from_bits(0x421a2049), f32::from_bits(0x421b1e8d))); // 38.5315f, 38.7798f
    path.quad_to(
        (f32::from_bits(0x4218fba8), f32::from_bits(0x421c384c)),
        (f32::from_bits(0x4217c88e), f32::from_bits(0x421d2f21)),
    ); // 38.2458f, 39.055f, 37.9459f, 39.296f
    path.quad_to(
        (f32::from_bits(0x42168e68), f32::from_bits(0x421e9b27)),
        (f32::from_bits(0x42152104), f32::from_bits(0x421fefda)),
    ); // 37.6391f, 39.6515f, 37.2822f, 39.9842f
    path.quad_to(
        (f32::from_bits(0x42146c3b), f32::from_bits(0x4220986c)),
        (f32::from_bits(0x4213b29d), f32::from_bits(0x42213416)),
    ); // 37.1057f, 40.1488f, 36.9244f, 40.3009f
    path.quad_to(
        (f32::from_bits(0x42130756), f32::from_bits(0x4221df6b)),
        (f32::from_bits(0x42124f9e), f32::from_bits(0x422284d0)),
    ); // 36.7572f, 40.4682f, 36.5778f, 40.6297f
    path.quad_to(
        (f32::from_bits(0x420875c9), f32::from_bits(0x422b6326)),
        (f32::from_bits(0x41f6726d), f32::from_bits(0x422ab150)),
    ); // 34.115f, 42.8468f, 30.8059f, 42.6732f
    path.quad_to(
        (f32::from_bits(0x41dbf947), f32::from_bits(0x4229ff79)),
        (f32::from_bits(0x41ca3c9c), f32::from_bits(0x422025a4)),
    ); // 27.4967f, 42.4995f, 25.2796f, 40.0368f
    path.quad_to(
        (f32::from_bits(0x41b87ff1), f32::from_bits(0x42164bcf)),
        (f32::from_bits(0x41b9e39e), f32::from_bits(0x42090f3c)),
    ); // 23.0625f, 37.574f, 23.2361f, 34.2649f
    path.quad_to(
        (f32::from_bits(0x41bb474a), f32::from_bits(0x41f7a551)),
        (f32::from_bits(0x41cefaf4), f32::from_bits(0x41e5e8a6)),
    ); // 23.4098f, 30.9557f, 25.8725f, 28.7386f
    path.quad_to(
        (f32::from_bits(0x41cffe18), f32::from_bits(0x41e4ff5a)),
        (f32::from_bits(0x41d105e6), f32::from_bits(0x41e422e9)),
    ); // 25.9991f, 28.6247f, 26.1279f, 28.517f
    path.quad_to(
        (f32::from_bits(0x41d1f880), f32::from_bits(0x41e32f61)),
        (f32::from_bits(0x41d2f77e), f32::from_bits(0x41e2419e)),
    ); // 26.2463f, 28.3981f, 26.3708f, 28.282f
    path.quad_to(
        (f32::from_bits(0x41d4f9d5), f32::from_bits(0x41e06208)),
        (f32::from_bits(0x41d70fba), f32::from_bits(0x41deb6ad)),
    ); // 26.622f, 28.0479f, 26.8827f, 27.8392f
    path.quad_to(
        (f32::from_bits(0x41d8cd7c), f32::from_bits(0x41dcb00a)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    ); // 27.1003f, 27.586f, 27.3431f, 27.3431f
    path.close();
    path.move_to((f32::from_bits(0xc185f1f2), f32::from_bits(0x42177488))); // -16.7431f, 37.8638f
    path.quad_to(
        (f32::from_bits(0xc15d848f), f32::from_bits(0x42110787)),
        (f32::from_bits(0xc12a8d7f), f32::from_bits(0x4214aee9)),
    ); // -13.8449f, 36.2574f, -10.6595f, 37.1708f
    path.quad_to(
        (f32::from_bits(0xc0ef2cde), f32::from_bits(0x4218564b)),
        (f32::from_bits(0xc0bbc4da), f32::from_bits(0x4223ee21)),
    ); // -7.47423f, 38.0843f, -5.86778f, 40.9825f
    path.quad_to(
        (f32::from_bits(0xc0885cd4), f32::from_bits(0x422f85f6)),
        (f32::from_bits(0xc0a597e8), f32::from_bits(0x423c43ba)),
    ); // -4.26133f, 43.8808f, -5.17479f, 47.0661f
    path.quad_to(
        (f32::from_bits(0xc0c2d2f4), f32::from_bits(0x4249017e)),
        (f32::from_bits(0xc10fc8d0), f32::from_bits(0x424f6e7e)),
    ); // -6.08825f, 50.2515f, -8.98653f, 51.8579f
    path.quad_to(
        (f32::from_bits(0xc13e2826), f32::from_bits(0x4255db7f)),
        (f32::from_bits(0xc1711f36), f32::from_bits(0x4252341c)),
    ); // -11.8848f, 53.4644f, -15.0701f, 52.5509f
    path.quad_to(
        (f32::from_bits(0xc1920b22), f32::from_bits(0x424e8cbc)),
        (f32::from_bits(0xc19ee524), f32::from_bits(0x4242f4e6)),
    ); // -18.2554f, 51.6374f, -19.8619f, 48.7392f
    path.quad_to(
        (f32::from_bits(0xc1abbf24), f32::from_bits(0x42375d10)),
        (f32::from_bits(0xc1a47060), f32::from_bits(0x422a9f4c)),
    ); // -21.4683f, 45.8409f, -20.5549f, 42.6556f
    path.quad_to(
        (f32::from_bits(0xc19d219e), f32::from_bits(0x421de188)),
        (f32::from_bits(0xc185f1f2), f32::from_bits(0x42177488)),
    ); // -19.6414f, 39.4702f, -16.7431f, 37.8638f
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x421f4961), f32::from_bits(0x4209a6a0))); // 39.8217f, 34.4127f
    path.quad_to(
        (f32::from_bits(0x421ed2ca), f32::from_bits(0x4216e5cb)),
        (f32::from_bits(0x42152104), f32::from_bits(0x421fefda)),
    ); // 39.7058f, 37.7244f, 37.2822f, 39.9842f
    path.quad_to(
        (f32::from_bits(0x420b6f41), f32::from_bits(0x4228f9ea)),
        (f32::from_bits(0x41fc602b), f32::from_bits(0x42288353)),
    ); // 34.8586f, 42.2441f, 31.547f, 42.1282f
    path.quad_to(
        (f32::from_bits(0x41e1e1d7), f32::from_bits(0x42280cbd)),
        (f32::from_bits(0x41cfcdb9), f32::from_bits(0x421e5af7)),
    ); // 28.2353f, 42.0124f, 25.9755f, 39.5888f
    path.quad_to(
        (f32::from_bits(0x41bdb999), f32::from_bits(0x4214a933)),
        (f32::from_bits(0x41bea6c6), f32::from_bits(0x42076a08)),
    ); // 23.7156f, 37.1652f, 23.8314f, 33.8535f
    path.quad_to(
        (f32::from_bits(0x41bf93f3), f32::from_bits(0x41f455bd)),
        (f32::from_bits(0x41d2f77e), f32::from_bits(0x41e2419e)),
    ); // 23.9472f, 30.5419f, 26.3708f, 28.282f
    path.quad_to(
        (f32::from_bits(0x41e65b07), f32::from_bits(0x41d02d7f)),
        (f32::from_bits(0x42006cae), f32::from_bits(0x41d11aac)),
    ); // 28.7944f, 26.0222f, 32.1061f, 26.138f
    path.quad_to(
        (f32::from_bits(0x420dabd9), f32::from_bits(0x41d207d9)),
        (f32::from_bits(0x4216b5e7), f32::from_bits(0x41e56b63)),
    ); // 35.4178f, 26.2538f, 37.6776f, 28.6774f
    path.quad_to(
        (f32::from_bits(0x421fbff7), f32::from_bits(0x41f8ceed)),
        (f32::from_bits(0x421f4961), f32::from_bits(0x4209a6a0)),
    ); // 39.9375f, 31.101f, 39.8217f, 34.4127f
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L1451-L1541 (chrome/m156)
fn fuzz763_1597464(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x4101092a), f32::from_bits(0xc2500973))); // 8.06474f, -52.0092f
    path.quad_to(
        (f32::from_bits(0x412ef1d8), f32::from_bits(0xc256aade)),
        (f32::from_bits(0x41622940), f32::from_bits(0xc2533d84)),
    ); // 10.934f, -53.6669f, 14.1351f, -52.8101f
    path.quad_to(
        (f32::from_bits(0x418ab055), f32::from_bits(0xc24fd02d)),
        (f32::from_bits(0x4197f32a), f32::from_bits(0xc2445602)),
    ); // 17.3361f, -51.9533f, 18.9937f, -49.084f
    path.quad_to(
        (f32::from_bits(0x41a535ff), f32::from_bits(0xc238dbd6)),
        (f32::from_bits(0x419e5b4c), f32::from_bits(0xc22c0dfb)),
    ); // 20.6514f, -46.2147f, 19.7946f, -43.0137f
    path.quad_to(
        (f32::from_bits(0x4197809e), f32::from_bits(0xc21f4021)),
        (f32::from_bits(0x41808c46), f32::from_bits(0xc2189eb6)),
    ); // 18.9378f, -39.8126f, 16.0685f, -38.155f
    path.quad_to(
        (f32::from_bits(0x41532fdd), f32::from_bits(0xc211fd4c)),
        (f32::from_bits(0x411ff875), f32::from_bits(0xc2156aa5)),
    ); // 13.1992f, -36.4974f, 9.99816f, -37.3541f
    path.quad_to(
        (f32::from_bits(0x40d98218), f32::from_bits(0xc218d7fd)),
        (f32::from_bits(0x40a476c4), f32::from_bits(0xc2245229)),
    ); // 6.79713f, -38.2109f, 5.1395f, -41.0802f
    path.quad_to(
        (f32::from_bits(0x405ed6e0), f32::from_bits(0xc22fcc54)),
        (f32::from_bits(0x408ad638), f32::from_bits(0xc23c9a2e)),
    ); // 3.48186f, -43.9495f, 4.33865f, -47.1506f
    path.quad_to(
        (f32::from_bits(0x40a640f4), f32::from_bits(0xc2496808)),
        (f32::from_bits(0x4101092a), f32::from_bits(0xc2500973)),
    ); // 5.19543f, -50.3516f, 8.06474f, -52.0092f
    path.close();
    path.move_to((f32::from_bits(0xc21ab0b3), f32::from_bits(0xc21a9087))); // -38.6726f, -38.6411f
    path.quad_to(
        (f32::from_bits(0xc211524c), f32::from_bits(0xc223f0e2)),
        (f32::from_bits(0xc204110f), f32::from_bits(0xc223f243)),
    ); // -36.3304f, -40.9852f, -33.0167f, -40.9866f
    path.quad_to(
        (f32::from_bits(0xc1ed9fa5), f32::from_bits(0xc223f3a4)),
        (f32::from_bits(0xc1dadeef), f32::from_bits(0xc21a953c)),
    ); // -29.703f, -40.9879f, -27.3589f, -38.6457f
    path.quad_to(
        (f32::from_bits(0xc1c81e38), f32::from_bits(0xc21136d5)),
        (f32::from_bits(0xc1c81b76), f32::from_bits(0xc203f598)),
    ); // -25.0148f, -36.3035f, -25.0134f, -32.9898f
    path.quad_to(
        (f32::from_bits(0xc1c818b4), f32::from_bits(0xc1ed68b6)),
        (f32::from_bits(0xc1dad584), f32::from_bits(0xc1daa800)),
    ); // -25.0121f, -29.6761f, -27.3543f, -27.332f
    path.quad_to(
        (f32::from_bits(0xc1ed9254), f32::from_bits(0xc1c7e74a)),
        (f32::from_bits(0xc2040a66), f32::from_bits(0xc1c7e488)),
    ); // -29.6964f, -24.9879f, -33.0102f, -24.9866f
    path.quad_to(
        (f32::from_bits(0xc2114ba3), f32::from_bits(0xc1c7e1c6)),
        (f32::from_bits(0xc21aabfe), f32::from_bits(0xc1da9e95)),
    ); // -36.3239f, -24.9852f, -38.668f, -27.3274f
    path.quad_to(
        (f32::from_bits(0xc2240c5a), f32::from_bits(0xc1ed5b65)),
        (f32::from_bits(0xc2240dbb), f32::from_bits(0xc203eeef)),
    ); // -41.0121f, -29.6696f, -41.0134f, -32.9833f
    path.quad_to(
        (f32::from_bits(0xc2240f1c), f32::from_bits(0xc211302c)),
        (f32::from_bits(0xc21ab0b3), f32::from_bits(0xc21a9087)),
    ); // -41.0148f, -36.297f, -38.6726f, -38.6411f
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455))); // -19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    ); // -11.598f, -28, 0, -28
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    ); // 11.598f, -28, 19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    ); // 28, -11.598f, 28, 0
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    ); // 28, 11.598f, 19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    ); // 13.7163f, 25.8817f, 5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    ); // 8, 29.7635f, 8, 33
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    ); // 8, 36.3137f, 5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    ); // 3.31371f, 41, 0, 41
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    ); // -3.31371f, 41, -5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    ); // -8, 36.3137f, -8, 33
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    ); // -8, 29.7635f, -5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    ); // -13.7163f, 25.8817f, -19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    ); // -28, 11.598f, -28, 0
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    ); // -28, -11.598f, -19.799f, -19.799f
    path.close();
    path.move_to((f32::from_bits(0xc21564da), f32::from_bits(0x41204f08))); // -37.3485f, 10.0193f
    path.quad_to(
        (f32::from_bits(0xc211fc8d), f32::from_bits(0x41536ca0)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    ); // -36.4966f, 13.214f, -38.1506f, 16.0788f
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    ); // -39.8075f, 18.9486f, -43.0083f, 19.8062f
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    ); // -46.2091f, 20.6639f, -49.0788f, 19.007f
    path.quad_to(
        (f32::from_bits(0xc24fc815), f32::from_bits(0x418ad0f8)),
        (f32::from_bits(0xc25337a2), f32::from_bits(0x4162812a)),
    ); // -51.9454f, 17.352f, -52.8043f, 14.1565f
    path.quad_to(
        (f32::from_bits(0xc25336a6), f32::from_bits(0x41628fdb)),
        (f32::from_bits(0xc25335aa), f32::from_bits(0x41629e8a)),
    ); // -52.8034f, 14.1601f, -52.8024f, 14.1637f
    path.quad_to(
        (f32::from_bits(0xc24fc68b), f32::from_bits(0x418aea08)),
        (f32::from_bits(0xc2444b74), f32::from_bits(0x419829ad)),
    ); // -51.9439f, 17.3643f, -49.0737f, 19.0203f
    path.quad_to(
        (f32::from_bits(0xc238d05c), f32::from_bits(0x41a56952)),
        (f32::from_bits(0xc22c02fc), f32::from_bits(0x419e8b10)),
    ); // -46.2035f, 20.6764f, -43.0029f, 19.8179f
    path.quad_to(
        (f32::from_bits(0xc21f359c), f32::from_bits(0x4197acd2)),
        (f32::from_bits(0xc21895c9), f32::from_bits(0x4180b6a4)),
    ); // -39.8024f, 18.9594f, -38.1463f, 16.0892f
    path.quad_to(
        (f32::from_bits(0xc211f634), f32::from_bits(0x41538295)),
        (f32::from_bits(0xc21564da), f32::from_bits(0x41204f08)),
    ); // -36.4904f, 13.2194f, -37.3485f, 10.0193f
    path.close();
    path.move_to((f32::from_bits(0x41dacdf1), f32::from_bits(0x41daaf93))); // 27.3506f, 27.3357f
    path.quad_to(
        (f32::from_bits(0x41ed8b66), f32::from_bits(0x41c7ef83)),
        (f32::from_bits(0x420406f0), f32::from_bits(0x41c7edac)),
    ); // 29.6931f, 24.9919f, 33.0068f, 24.9911f
    path.quad_to(
        (f32::from_bits(0x4211482d), f32::from_bits(0x41c7ebd5)),
        (f32::from_bits(0x421aa834), f32::from_bits(0x41daa94b)),
    ); // 36.3205f, 24.9902f, 38.6643f, 27.3327f
    path.quad_to(
        (f32::from_bits(0x4224083d), f32::from_bits(0x41ed66c1)),
        (f32::from_bits(0x42240928), f32::from_bits(0x4203f49d)),
    ); // 41.008f, 29.6752f, 41.0089f, 32.9889f
    path.quad_to(
        (f32::from_bits(0x42240a14), f32::from_bits(0x421135da)),
        (f32::from_bits(0x421aab58), f32::from_bits(0x421a95e2)),
    ); // 41.0098f, 36.3026f, 38.6673f, 38.6464f
    path.quad_to(
        (f32::from_bits(0x421aa5fc), f32::from_bits(0x421a9b42)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    ); // 38.6621f, 38.6516f, 38.6569f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    ); // 36.3137f, 41, 33, 41
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    ); // 29.6863f, 41, 27.3431f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    ); // 25, 36.3137f, 25, 33
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    ); // 25, 29.6863f, 27.3431f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x41dac1e5), f32::from_bits(0x41dabba1)),
        (f32::from_bits(0x41dac507), f32::from_bits(0x41dab880)),
    ); // 27.3447f, 27.3416f, 27.3462f, 27.3401f
    path.line_to((f32::from_bits(0x41dac551), f32::from_bits(0x41dab836))); // 27.3463f, 27.3399f
    path.quad_to(
        (f32::from_bits(0x41dac9a0), f32::from_bits(0x41dab3e4)),
        (f32::from_bits(0x41dacdf1), f32::from_bits(0x41daaf93)),
    ); // 27.3484f, 27.3378f, 27.3506f, 27.3357f
    path.close();
    path.move_to((f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec))); // -47.1494f, 4.35143f
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    ); // -43.9486f, 3.49378f, -41.0788f, 5.15063f
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    ); // -38.2091f, 6.80749f, -37.3514f, 10.0083f
    path.quad_to(
        (f32::from_bits(0xc215672b), f32::from_bits(0x41202c49)),
        (f32::from_bits(0xc215667a), f32::from_bits(0x412036a3)),
    ); // -37.3507f, 10.0108f, -37.3501f, 10.0133f
    path.line_to((f32::from_bits(0xc2156516), f32::from_bits(0x41204b6b))); // -37.3487f, 10.0184f
    path.line_to((f32::from_bits(0xc2533998), f32::from_bits(0x416263e8))); // -52.8062f, 14.1494f
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    ); // -53.6639f, 10.9486f, -52.007f, 8.07884f
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    ); // -50.3502f, 5.20908f, -47.1494f, 4.35143f
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xc2444b74), f32::from_bits(0x419829ad))); // -49.0737f, 19.0203f
    path.quad_to(
        (f32::from_bits(0xc24fc68b), f32::from_bits(0x418aea08)),
        (f32::from_bits(0xc25335aa), f32::from_bits(0x41629e8a)),
    ); // -51.9439f, 17.3643f, -52.8024f, 14.1637f
    path.quad_to(
        (f32::from_bits(0xc256a4ca), f32::from_bits(0x412f6908)),
        (f32::from_bits(0xc25004f8), f32::from_bits(0x41017cac)),
    ); // -53.6609f, 10.9631f, -52.0049f, 8.09294f
    path.quad_to(
        (f32::from_bits(0xc2496525), f32::from_bits(0x40a7209c)),
        (f32::from_bits(0xc23c97c4), f32::from_bits(0x408ba7a8)),
    ); // -50.3488f, 5.22273f, -47.1482f, 4.36422f
    path.quad_to(
        (f32::from_bits(0xc22fca64), f32::from_bits(0x40605d58)),
        (f32::from_bits(0xc2244f4d), f32::from_bits(0x40a52d40)),
    ); // -43.9476f, 3.5057f, -41.0774f, 5.16177f
    path.quad_to(
        (f32::from_bits(0xc218d435), f32::from_bits(0x40da2bd2)),
        (f32::from_bits(0xc2156516), f32::from_bits(0x41204b6b)),
    ); // -38.2072f, 6.81785f, -37.3487f, 10.0184f
    path.quad_to(
        (f32::from_bits(0xc211f5f7), f32::from_bits(0x415380eb)),
        (f32::from_bits(0xc21895c9), f32::from_bits(0x4180b6a4)),
    ); // -36.4902f, 13.219f, -38.1463f, 16.0892f
    path.quad_to(
        (f32::from_bits(0xc21f359c), f32::from_bits(0x4197acd2)),
        (f32::from_bits(0xc22c02fc), f32::from_bits(0x419e8b10)),
    ); // -39.8024f, 18.9594f, -43.0029f, 19.8179f
    path.quad_to(
        (f32::from_bits(0xc238d05c), f32::from_bits(0x41a56952)),
        (f32::from_bits(0xc2444b74), f32::from_bits(0x419829ad)),
    ); // -46.2035f, 20.6764f, -49.0737f, 19.0203f
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L1543-L1652 (chrome/m156)
fn fuzz763_34974(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xc2533a24), f32::from_bits(0x41625bba)));
    path.line_to((f32::from_bits(0xc2533ab2), f32::from_bits(0x4162536e)));
    path.line_to((f32::from_bits(0xc2533af7), f32::from_bits(0x41624f68)));
    path.quad_to(
        (f32::from_bits(0xc2533a8e), f32::from_bits(0x41625591)),
        (f32::from_bits(0xc2533a24), f32::from_bits(0x41625bba)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc23c9951), f32::from_bits(0x408b2180)));
    path.quad_to(
        (f32::from_bits(0xc22fcba2), f32::from_bits(0x405f6340)),
        (f32::from_bits(0xc2245122), f32::from_bits(0x40a4b85c)),
    );
    path.quad_to(
        (f32::from_bits(0xc218dd36), f32::from_bits(0x40d9a0b8)),
        (f32::from_bits(0xc2156c96), f32::from_bits(0x411fdb9a)),
    );
    path.line_to((f32::from_bits(0xc2156b9c), f32::from_bits(0x411fea15)));
    path.quad_to(
        (f32::from_bits(0xc2156a20), f32::from_bits(0x4120002c)),
        (f32::from_bits(0xc21568a5), f32::from_bits(0x41201647)),
    );
    path.line_to((f32::from_bits(0xc21568a3), f32::from_bits(0x41201660)));
    path.line_to((f32::from_bits(0xc2156841), f32::from_bits(0x41201c29)));
    path.quad_to(
        (f32::from_bits(0xc215680f), f32::from_bits(0x41201f0a)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    );
    path.quad_to(
        (f32::from_bits(0xc21562d2), f32::from_bits(0x41206d52)),
        (f32::from_bits(0xc2155ca3), f32::from_bits(0x4120cb63)),
    );
    path.quad_to(
        (f32::from_bits(0xc212057d), f32::from_bits(0x4153a15f)),
        (f32::from_bits(0xc2189adf), f32::from_bits(0x41809e82)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f3b9a), f32::from_bits(0x419793a4)),
        (f32::from_bits(0xc22c0940), f32::from_bits(0x419e6fdc)),
    );
    path.quad_to(
        (f32::from_bits(0xc238d6e6), f32::from_bits(0x41a54c16)),
        (f32::from_bits(0xc2445177), f32::from_bits(0x41980aa0)),
    );
    path.quad_to(
        (f32::from_bits(0xc24fcb1e), f32::from_bits(0x418aca39)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    );
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    );
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    );
    path.line_to((f32::from_bits(0xc2533b22), f32::from_bits(0x41624cea)));
    path.quad_to(
        (f32::from_bits(0xc256a842), f32::from_bits(0x412f19c8)),
        (f32::from_bits(0xc25007d7), f32::from_bits(0x410132b2)),
    );
    path.quad_to(
        (f32::from_bits(0xc24966ff), f32::from_bits(0x40a69160)),
        (f32::from_bits(0xc23c9951), f32::from_bits(0x408b2180)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xc2445236), f32::from_bits(0x419806c2)));
    path.quad_to(
        (f32::from_bits(0xc24fccb6), f32::from_bits(0x418ac513)),
        (f32::from_bits(0xc2533ab2), f32::from_bits(0x4162536e)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a8ae), f32::from_bits(0x412f1cb2)),
        (f32::from_bits(0xc25007d7), f32::from_bits(0x410132b2)),
    );
    path.quad_to(
        (f32::from_bits(0xc24966ff), f32::from_bits(0x40a69160)),
        (f32::from_bits(0xc23c9951), f32::from_bits(0x408b2180)),
    );
    path.quad_to(
        (f32::from_bits(0xc22fcba2), f32::from_bits(0x405f6340)),
        (f32::from_bits(0xc2245122), f32::from_bits(0x40a4b85c)),
    );
    path.quad_to(
        (f32::from_bits(0xc218d6a2), f32::from_bits(0x40d9bf1c)),
        (f32::from_bits(0xc21568a5), f32::from_bits(0x41201647)),
    );
    path.quad_to(
        (f32::from_bits(0xc211faaa), f32::from_bits(0x41534d02)),
        (f32::from_bits(0xc2189b82), f32::from_bits(0x41809b82)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f3c59), f32::from_bits(0x41979082)),
        (f32::from_bits(0xc22c0a07), f32::from_bits(0x419e6c7a)),
    );
    path.quad_to(
        (f32::from_bits(0xc238d7b6), f32::from_bits(0x41a54872)),
        (f32::from_bits(0xc2445236), f32::from_bits(0x419806c2)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L1654-L1727 (chrome/m156)
fn fuzz763_2211264(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x41017a68), f32::from_bits(0xc250050e)));
    path.quad_to(
        (f32::from_bits(0x412f66b2), f32::from_bits(0xc256a4e9)),
        (f32::from_bits(0x41629c3c), f32::from_bits(0xc25335d1)),
    );
    path.quad_to(
        (f32::from_bits(0x418ae8e6), f32::from_bits(0xc24fc6bc)),
        (f32::from_bits(0x4198289b), f32::from_bits(0xc2444ba9)),
    );
    path.quad_to(
        (f32::from_bits(0x41a56850), f32::from_bits(0xc238d096)),
        (f32::from_bits(0x419e8a20), f32::from_bits(0xc22c0333)),
    );
    path.quad_to(
        (f32::from_bits(0x4197abf4), f32::from_bits(0xc21f35d0)),
        (f32::from_bits(0x4180b5d0), f32::from_bits(0xc21895f6)),
    );
    path.quad_to(
        (f32::from_bits(0x41537f55), f32::from_bits(0xc211f61c)),
        (f32::from_bits(0x412049c9), f32::from_bits(0xc2156532)),
    );
    path.quad_to(
        (f32::from_bits(0x40da287c), f32::from_bits(0xc218d449)),
        (f32::from_bits(0x40a529a8), f32::from_bits(0xc2244f5b)),
    );
    path.quad_to(
        (f32::from_bits(0x406055a8), f32::from_bits(0xc22fca6e)),
        (f32::from_bits(0x408ba388), f32::from_bits(0xc23c97d0)),
    );
    path.quad_to(
        (f32::from_bits(0x40a71c3c), f32::from_bits(0xc2496534)),
        (f32::from_bits(0x41017a68), f32::from_bits(0xc250050e)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc21a9126), f32::from_bits(0xc21ab014)));
    path.quad_to(
        (f32::from_bits(0xc21130d5), f32::from_bits(0xc2240e86)),
        (f32::from_bits(0xc203ef98), f32::from_bits(0xc2240d33)),
    );
    path.quad_to(
        (f32::from_bits(0xc1ed5cb7), f32::from_bits(0xc2240bdf)),
        (f32::from_bits(0xc1da9fd4), f32::from_bits(0xc21aab8d)),
    );
    path.quad_to(
        (f32::from_bits(0xc1c7e2f1), f32::from_bits(0xc2114b3c)),
        (f32::from_bits(0xc1c7e598), f32::from_bits(0xc20409ff)),
    );
    path.quad_to(
        (f32::from_bits(0xc1c7e83e), f32::from_bits(0xc1ed9186)),
        (f32::from_bits(0xc1daa8e1), f32::from_bits(0xc1dad4a3)),
    );
    path.quad_to(
        (f32::from_bits(0xc1ed6984), f32::from_bits(0xc1c817c0)),
        (f32::from_bits(0xc203f5ff), f32::from_bits(0xc1c81a66)),
    );
    path.quad_to(
        (f32::from_bits(0xc211373c), f32::from_bits(0xc1c81d0d)),
        (f32::from_bits(0xc21a95ad), f32::from_bits(0xc1daddb0)),
    );
    path.quad_to(
        (f32::from_bits(0xc223f41f), f32::from_bits(0xc1ed9e53)),
        (f32::from_bits(0xc223f2cb), f32::from_bits(0xc2041066)),
    );
    path.quad_to(
        (f32::from_bits(0xc223f178), f32::from_bits(0xc21151a3)),
        (f32::from_bits(0xc21a9126), f32::from_bits(0xc21ab014)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)));
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    );
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    );
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)));
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x421a9e0c), f32::from_bits(0x421aa331)),
        (f32::from_bits(0x421a9b79), f32::from_bits(0x421aa5c3)),
    );
    path.quad_to(
        (f32::from_bits(0x421a98e4), f32::from_bits(0x421aa858)),
        (f32::from_bits(0x421a964e), f32::from_bits(0x421aaaec)),
    );
    path.line_to((f32::from_bits(0x421a943a), f32::from_bits(0x421aad00)));
    path.quad_to(
        (f32::from_bits(0x421134d5), f32::from_bits(0x422409ae)),
        (f32::from_bits(0x4203f510), f32::from_bits(0x422408cc)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed67a7), f32::from_bits(0x422407e9)),
        (f32::from_bits(0x41daaa24), f32::from_bits(0x421aa7e8)),
    );
    path.quad_to(
        (f32::from_bits(0x41c7eca1), f32::from_bits(0x421147e7)),
        (f32::from_bits(0x41c7ee66), f32::from_bits(0x420406aa)),
    );
    path.quad_to(
        (f32::from_bits(0x41c7f02a), f32::from_bits(0x41ed8ada)),
        (f32::from_bits(0x41dab02f), f32::from_bits(0x41dacd55)),
    );
    path.line_to((f32::from_bits(0x41dab02d), f32::from_bits(0x41dacd57)));
    path.quad_to(
        (f32::from_bits(0x41dab3d4), f32::from_bits(0x41dac9b0)),
        (f32::from_bits(0x41dab77c), f32::from_bits(0x41dac60a)),
    );
    path.quad_to(
        (f32::from_bits(0x41dab83b), f32::from_bits(0x41dac54b)),
        (f32::from_bits(0x41dab8fa), f32::from_bits(0x41dac48d)),
    );
    path.quad_to(
        (f32::from_bits(0x41dabbde), f32::from_bits(0x41dac1a8)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xc24455cc), f32::from_bits(0x4197f43e)));
    path.quad_to(
        (f32::from_bits(0xc24fcffc), f32::from_bits(0x418ab179)),
        (f32::from_bits(0xc2533d5e), f32::from_bits(0x41622b92)),
    );
    path.quad_to(
        (f32::from_bits(0xc256aac0), f32::from_bits(0x412ef432)),
        (f32::from_bits(0xc250095d), f32::from_bits(0x41010b70)),
    );
    path.quad_to(
        (f32::from_bits(0xc24967fb), f32::from_bits(0x40a64560)),
        (f32::from_bits(0xc23c9a22), f32::from_bits(0x408ada54)),
    );
    path.quad_to(
        (f32::from_bits(0xc22fcc4a), f32::from_bits(0x405ede90)),
        (f32::from_bits(0xc224521a), f32::from_bits(0x40a47a5c)),
    );
    path.quad_to(
        (f32::from_bits(0xc218d7ea), f32::from_bits(0x40d9856e)),
        (f32::from_bits(0xc2156a88), f32::from_bits(0x411ffa16)),
    );
    path.quad_to(
        (f32::from_bits(0xc211fd27), f32::from_bits(0x41533178)),
        (f32::from_bits(0xc2189e8a), f32::from_bits(0x41808d1c)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f3fec), f32::from_bits(0x4197817d)),
        (f32::from_bits(0xc22c0dc4), f32::from_bits(0x419e5c3f)),
    );
    path.quad_to(
        (f32::from_bits(0xc238db9c), f32::from_bits(0x41a53702)),
        (f32::from_bits(0xc24455cc), f32::from_bits(0x4197f43e)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L1729-L1820 (chrome/m156)
fn fuzz763_4628016(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x41029678), f32::from_bits(0xc24ff9f4))); // 8.16174f, -51.9941f
    path.quad_to(
        (f32::from_bits(0x41308bcc), f32::from_bits(0xc25695e3)),
        (f32::from_bits(0x4163bca4), f32::from_bits(0xc253226d)),
    ); // 11.0341f, -53.6464f, 14.2336f, -52.7836f
    path.quad_to(
        (f32::from_bits(0x4164114c), f32::from_bits(0xc2531cb8)),
        (f32::from_bits(0x41646770), f32::from_bits(0xc25316ce)),
    ); // 14.2542f, -52.778f, 14.2753f, -52.7723f
    path.quad_to(
        (f32::from_bits(0x4164bf2b), f32::from_bits(0xc25310f7)),
        (f32::from_bits(0x4165153a), f32::from_bits(0xc2530b20)),
    ); // 14.2967f, -52.7666f, 14.3177f, -52.7609f
    path.quad_to(
        (f32::from_bits(0x418c2035), f32::from_bits(0xc24f9272)),
        (f32::from_bits(0x41994eb0), f32::from_bits(0xc244126c)),
    ); // 17.5157f, -51.893f, 19.1634f, -49.018f
    path.quad_to(
        (f32::from_bits(0x41a67d2c), f32::from_bits(0xc2389265)),
        (f32::from_bits(0x419f8bd0), f32::from_bits(0xc22bc798)),
    ); // 20.8111f, -46.143f, 19.9433f, -42.9449f
    path.quad_to(
        (f32::from_bits(0x41989a74), f32::from_bits(0xc21efccc)),
        (f32::from_bits(0x41819a68), f32::from_bits(0xc218658e)),
    ); // 19.0754f, -39.7469f, 16.2004f, -38.0992f
    path.quad_to(
        (f32::from_bits(0x415534b5), f32::from_bits(0xc211ce50)),
        (f32::from_bits(0x41220985), f32::from_bits(0xc21546ff)),
    ); // 13.3254f, -36.4515f, 10.1273f, -37.3193f
    path.quad_to(
        (f32::from_bits(0x4121cc4c), f32::from_bits(0xc2154b26)),
        (f32::from_bits(0x41218f63), f32::from_bits(0xc2154f56)),
    ); // 10.1124f, -37.3234f, 10.0975f, -37.3275f
    path.quad_to(
        (f32::from_bits(0x412152b0), f32::from_bits(0xc2155360)),
        (f32::from_bits(0x412115c8), f32::from_bits(0xc215577b)),
    ); // 10.0827f, -37.3314f, 10.0678f, -37.3354f
    path.quad_to(
        (f32::from_bits(0x40dbc9e2), f32::from_bits(0xc218caf0)),
        (f32::from_bits(0x40a6ea6c), f32::from_bits(0xc2244845)),
    ); // 6.86839f, -38.1982f, 5.21612f, -41.0706f
    path.quad_to(
        (f32::from_bits(0x406415f0), f32::from_bits(0xc22fc59a)),
        (f32::from_bits(0x408da6a4), f32::from_bits(0xc23c91d0)),
    ); // 3.56384f, -43.943f, 4.42659f, -47.1424f
    path.quad_to(
        (f32::from_bits(0x40a94248), f32::from_bits(0xc2495e06)),
        (f32::from_bits(0x41029678), f32::from_bits(0xc24ff9f4)),
    ); // 5.28934f, -50.3418f, 8.16174f, -51.9941f
    path.close();
    path.move_to((f32::from_bits(0xc219e2da), f32::from_bits(0xc21b5d7a))); // -38.4715f, -38.8413f
    path.quad_to(
        (f32::from_bits(0xc2107806), f32::from_bits(0xc224b15c)),
        (f32::from_bits(0xc20336d3), f32::from_bits(0xc224a121)),
    ); // -36.1172f, -41.1732f, -32.8035f, -41.1574f
    path.quad_to(
        (f32::from_bits(0xc1ebeb41), f32::from_bits(0xc22490e7)),
        (f32::from_bits(0xc1d9437f), f32::from_bits(0xc21b2612)),
    ); // -29.4899f, -41.1415f, -27.158f, -38.7872f
    path.quad_to(
        (f32::from_bits(0xc1c69bbd), f32::from_bits(0xc211bb3f)),
        (f32::from_bits(0xc1c6bc32), f32::from_bits(0xc2047a0c)),
    ); // -24.826f, -36.4329f, -24.8419f, -33.1192f
    path.quad_to(
        (f32::from_bits(0xc1c6dca7), f32::from_bits(0xc1ee71b1)),
        (f32::from_bits(0xc1d9b24f), f32::from_bits(0xc1dbc9ef)),
    ); // -24.8577f, -29.8055f, -27.2121f, -27.4736f
    path.quad_to(
        (f32::from_bits(0xc1ec87f7), f32::from_bits(0xc1c9222d)),
        (f32::from_bits(0xc203852e), f32::from_bits(0xc1c942a2)),
    ); // -29.5664f, -25.1417f, -32.8801f, -25.1575f
    path.quad_to(
        (f32::from_bits(0xc210c661), f32::from_bits(0xc1c96317)),
        (f32::from_bits(0xc21a1a42), f32::from_bits(0xc1dc38bf)),
    ); // -36.1937f, -25.1734f, -38.5256f, -27.5277f
    path.quad_to(
        (f32::from_bits(0xc2236e23), f32::from_bits(0xc1ef0e67)),
        (f32::from_bits(0xc2235de9), f32::from_bits(0xc204c867)),
    ); // -40.8576f, -29.882f, -40.8417f, -33.1957f
    path.quad_to(
        (f32::from_bits(0xc2234dae), f32::from_bits(0xc212099a)),
        (f32::from_bits(0xc219e2da), f32::from_bits(0xc21b5d7a)),
    ); // -40.8259f, -36.5094f, -38.4715f, -38.8413f
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455))); // -19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    ); // -11.598f, -28, 0, -28
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    ); // 11.598f, -28, 19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    ); // 28, -11.598f, 28, 0
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    ); // 28, 11.598f, 19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    ); // 13.7163f, 25.8817f, 5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    ); // 8, 29.7635f, 8, 33
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    ); // 8, 36.3137f, 5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    ); // 3.31371f, 41, 0, 41
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    ); // -3.31371f, 41, -5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    ); // -8, 36.3137f, -8, 33
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    ); // -8, 29.7635f, -5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    ); // -13.7163f, 25.8817f, -19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    ); // -28, 11.598f, -28, 0
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    ); // -28, -11.598f, -19.799f, -19.799f
    path.close();
    path.move_to((f32::from_bits(0xc23ca6f0), f32::from_bits(0x40866f38))); // -47.163f, 4.20108f
    path.quad_to(
        (f32::from_bits(0xc22fd68a), f32::from_bits(0x4056a1f8)),
        (f32::from_bits(0xc2246156), f32::from_bits(0x40a0a0d4)),
    ); // -43.9595f, 3.35364f, -41.0951f, 5.01963f
    path.quad_to(
        (f32::from_bits(0xc218ec21), f32::from_bits(0x40d5f0aa)),
        (f32::from_bits(0xc2158859), f32::from_bits(0x411e39ec)),
    ); // -38.2306f, 6.68563f, -37.3832f, 9.88914f
    path.quad_to(
        (f32::from_bits(0xc2158055), f32::from_bits(0x411eb323)),
        (f32::from_bits(0xc2157870), f32::from_bits(0x411f2dc3)),
    ); // -37.3753f, 9.91873f, -37.3676f, 9.94867f
    path.quad_to(
        (f32::from_bits(0xc2157002), f32::from_bits(0x411fa855)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    ); // -37.3594f, 9.9786f, -37.3514f, 10.0083f
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    ); // -36.4938f, 13.2091f, -38.1506f, 16.0788f
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    ); // -39.8075f, 18.9486f, -43.0083f, 19.8062f
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    ); // -46.2091f, 20.6639f, -49.0788f, 19.007f
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    ); // -51.9486f, 17.3502f, -52.8062f, 14.1494f
    path.quad_to(
        (f32::from_bits(0xc2534522), f32::from_bits(0x4161b7a0)),
        (f32::from_bits(0xc253504c), f32::from_bits(0x41610a87)),
    ); // -52.8175f, 14.1073f, -52.8284f, 14.0651f
    path.quad_to(
        (f32::from_bits(0xc2535c1f), f32::from_bits(0x41605e45)),
        (f32::from_bits(0xc2536784), f32::from_bits(0x415fb1f8)),
    ); // -52.84f, 14.023f, -52.8511f, 13.9809f
    path.quad_to(
        (f32::from_bits(0xc256cb4c), f32::from_bits(0x412c7060)),
        (f32::from_bits(0xc2502151), f32::from_bits(0x40fd371c)),
    ); // -53.6985f, 10.7774f, -52.0325f, 7.91298f
    path.quad_to(
        (f32::from_bits(0xc2497756), f32::from_bits(0x40a18d74)),
        (f32::from_bits(0xc23ca6f0), f32::from_bits(0x40866f38)),
    ); // -50.3665f, 5.04852f, -47.163f, 4.20108f
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3))); // 27.3431f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    ); // 29.6863f, 25, 33, 25
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    ); // 36.3137f, 25, 38.6569f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    ); // 41, 29.6863f, 41, 33
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    ); // 41, 36.3137f, 38.6569f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x421a8115), f32::from_bits(0x421ac027)),
        (f32::from_bits(0x421a613d), f32::from_bits(0x421adf68)),
    ); // 38.6261f, 38.6876f, 38.595f, 38.7182f
    path.quad_to(
        (f32::from_bits(0x421a41da), f32::from_bits(0x421aff30)),
        (f32::from_bits(0x421a2234), f32::from_bits(0x421b1ea2)),
    ); // 38.5643f, 38.7492f, 38.5334f, 38.7799f
    path.quad_to(
        (f32::from_bits(0x4210bb30), f32::from_bits(0x4224765a)),
        (f32::from_bits(0x420379f7), f32::from_bits(0x42246b89)),
    ); // 36.1828f, 41.1156f, 32.8691f, 41.105f
    path.quad_to(
        (f32::from_bits(0x41ec717c), f32::from_bits(0x422460b7)),
        (f32::from_bits(0x41d9c20c), f32::from_bits(0x421af9b1)),
    ); // 29.5554f, 41.0944f, 27.2197f, 38.7438f
    path.quad_to(
        (f32::from_bits(0x41c7129d), f32::from_bits(0x421192ad)),
        (f32::from_bits(0x41c72841), f32::from_bits(0x42045174)),
    ); // 24.8841f, 36.3932f, 24.8947f, 33.0795f
    path.quad_to(
        (f32::from_bits(0x41c73de4), f32::from_bits(0x41ee2077)),
        (f32::from_bits(0x41da0bed), f32::from_bits(0x41db7107)),
    ); // 24.9052f, 29.7659f, 27.2558f, 27.4302f
    path.quad_to(
        (f32::from_bits(0x41da38d6), f32::from_bits(0x41db4467)),
        (f32::from_bits(0x41da65bd), f32::from_bits(0x41db185f)),
    ); // 27.2778f, 27.4084f, 27.2997f, 27.3869f
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41edbcc3)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    ); // 25, 29.7172f, 25, 33
    path.line_to((f32::from_bits(0x41da65bd), f32::from_bits(0x41db185f))); // 27.2997f, 27.3869f
    path.line_to((f32::from_bits(0x41da65c4), f32::from_bits(0x41db1859))); // 27.2997f, 27.3869f
    path.quad_to(
        (f32::from_bits(0x41da921e), f32::from_bits(0x41daeb68)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    ); // 27.3213f, 27.3649f, 27.3431f, 27.3431f
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4223940b), f32::from_bits(0x420485b1))); // 40.8946f, 33.1306f
    path.quad_to(
        (f32::from_bits(0x4223893a), f32::from_bits(0x4211c6ea)),
        (f32::from_bits(0x421a2234), f32::from_bits(0x421b1ea2)),
    ); // 40.884f, 36.4443f, 38.5334f, 38.7799f
    path.quad_to(
        (f32::from_bits(0x4210bb30), f32::from_bits(0x4224765a)),
        (f32::from_bits(0x420379f7), f32::from_bits(0x42246b89)),
    ); // 36.1828f, 41.1156f, 32.8691f, 41.105f
    path.quad_to(
        (f32::from_bits(0x41ec717c), f32::from_bits(0x422460b7)),
        (f32::from_bits(0x41d9c20c), f32::from_bits(0x421af9b1)),
    ); // 29.5554f, 41.0944f, 27.2197f, 38.7438f
    path.quad_to(
        (f32::from_bits(0x41c7129d), f32::from_bits(0x421192ad)),
        (f32::from_bits(0x41c72841), f32::from_bits(0x42045174)),
    ); // 24.8841f, 36.3932f, 24.8947f, 33.0795f
    path.quad_to(
        (f32::from_bits(0x41c73de4), f32::from_bits(0x41ee2077)),
        (f32::from_bits(0x41da0bed), f32::from_bits(0x41db7107)),
    ); // 24.9052f, 29.7659f, 27.2558f, 27.4302f
    path.quad_to(
        (f32::from_bits(0x41ecd9f7), f32::from_bits(0x41c8c198)),
        (f32::from_bits(0x4203ae34), f32::from_bits(0x41c8d73b)),
    ); // 29.6064f, 25.0945f, 32.9201f, 25.1051f
    path.quad_to(
        (f32::from_bits(0x4210ef6d), f32::from_bits(0x41c8ecdf)),
        (f32::from_bits(0x421a4725), f32::from_bits(0x41dbbae8)),
    ); // 36.2338f, 25.1157f, 38.5695f, 27.4663f
    path.quad_to(
        (f32::from_bits(0x42239edd), f32::from_bits(0x41ee88f2)),
        (f32::from_bits(0x4223940b), f32::from_bits(0x420485b1)),
    ); // 40.9051f, 29.8169f, 40.8946f, 33.1306f
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L1822-L1913 (chrome/m156)
fn fuzz763_6411089(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x410373c2), f32::from_bits(0xc24ff13e)));
    path.quad_to(
        (f32::from_bits(0x4131701e), f32::from_bits(0xc2568a1e)),
        (f32::from_bits(0x41649d46), f32::from_bits(0xc2531340)),
    );
    path.quad_to(
        (f32::from_bits(0x418be539), f32::from_bits(0xc24f9c64)),
        (f32::from_bits(0x419916f9), f32::from_bits(0xc2441d4e)),
    );
    path.quad_to(
        (f32::from_bits(0x41a648b8), f32::from_bits(0xc2389e37)),
        (f32::from_bits(0x419f5afe), f32::from_bits(0xc22bd2ec)),
    );
    path.quad_to(
        (f32::from_bits(0x41986d46), f32::from_bits(0xc21f07a2)),
        (f32::from_bits(0x41816f18), f32::from_bits(0xc2186ec2)),
    );
    path.quad_to(
        (f32::from_bits(0x4154e1d6), f32::from_bits(0xc211d5e2)),
        (f32::from_bits(0x4121b4ad), f32::from_bits(0xc2154cbf)),
    );
    path.quad_to(
        (f32::from_bits(0x40dd0f06), f32::from_bits(0xc218c39c)),
        (f32::from_bits(0x40a84808), f32::from_bits(0xc22442b2)),
    );
    path.quad_to(
        (f32::from_bits(0x40670210), f32::from_bits(0xc22fc1c9)),
        (f32::from_bits(0x408f37ec), f32::from_bits(0xc23c8d13)),
    );
    path.quad_to(
        (f32::from_bits(0x40aaeed0), f32::from_bits(0xc249585e)),
        (f32::from_bits(0x410373c2), f32::from_bits(0xc24ff13e)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)));
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    );
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc23ca48b), f32::from_bits(0x408745a4)));
    path.quad_to(
        (f32::from_bits(0xc22fd4a1), f32::from_bits(0x405831a8)),
        (f32::from_bits(0xc2245e7a), f32::from_bits(0x40a15ba4)),
    );
    path.quad_to(
        (f32::from_bits(0xc218e853), f32::from_bits(0x40d69e76)),
        (f32::from_bits(0xc21582b9), f32::from_bits(0x411e8ee7)),
    );
    path.quad_to(
        (f32::from_bits(0xc2121d1f), f32::from_bits(0x4151ce91)),
        (f32::from_bits(0xc218c57a), f32::from_bits(0x417fa72d)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f6dd4), f32::from_bits(0x4196bfe4)),
        (f32::from_bits(0xc22c3dbe), f32::from_bits(0x419d8b16)),
    );
    path.quad_to(
        (f32::from_bits(0xc2390da9), f32::from_bits(0x41a4564c)),
        (f32::from_bits(0xc24483d0), f32::from_bits(0x41970597)),
    );
    path.quad_to(
        (f32::from_bits(0xc24ff9f7), f32::from_bits(0x4189b4e3)),
        (f32::from_bits(0xc2535f90), f32::from_bits(0x41602a18)),
    );
    path.quad_to(
        (f32::from_bits(0xc256c52a), f32::from_bits(0x412cea70)),
        (f32::from_bits(0xc2501cd0), f32::from_bits(0x40fe23a8)),
    );
    path.quad_to(
        (f32::from_bits(0xc2497476), f32::from_bits(0x40a27270)),
        (f32::from_bits(0xc23ca48b), f32::from_bits(0x408745a4)),
    );
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)));
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x421a8005), f32::from_bits(0x421ac137)),
        (f32::from_bits(0x421a5f6b), f32::from_bits(0x421ae132)),
    );
    path.line_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)));
    path.close();
    path.move_to((f32::from_bits(0x421a52fa), f32::from_bits(0x421aed60)));
    path.quad_to(
        (f32::from_bits(0x421a59d5), f32::from_bits(0x421ae6ac)),
        (f32::from_bits(0x421a5f6b), f32::from_bits(0x421ae132)),
    );
    path.quad_to(
        (f32::from_bits(0x421a5a2a), f32::from_bits(0x421ae65a)),
        (f32::from_bits(0x421a52fa), f32::from_bits(0x421aed60)),
    );
    path.close();
    path.move_to((f32::from_bits(0x421a52fa), f32::from_bits(0x421aed60)));
    path.quad_to(
        (f32::from_bits(0x42110a85), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41edcbe0)),
        (f32::from_bits(0x41da509a), f32::from_bits(0x41db2dd4)),
    );
    path.quad_to(
        (f32::from_bits(0x41da18a0), f32::from_bits(0x41db6476)),
        (f32::from_bits(0x41d9e121), f32::from_bits(0x41db9b85)),
    );
    path.quad_to(
        (f32::from_bits(0x41c70f73), f32::from_bits(0x41ee474a)),
        (f32::from_bits(0x41c6f4a5), f32::from_bits(0x420464db)),
    );
    path.quad_to(
        (f32::from_bits(0x41c6d9d6), f32::from_bits(0x4211a612)),
        (f32::from_bits(0x41d9859b), f32::from_bits(0x421b0ee8)),
    );
    path.quad_to(
        (f32::from_bits(0x41ec315f), f32::from_bits(0x422477bf)),
        (f32::from_bits(0x420359e6), f32::from_bits(0x42248527)),
    );
    path.quad_to(
        (f32::from_bits(0x42109b1d), f32::from_bits(0x4224928e)),
        (f32::from_bits(0x421a03f3), f32::from_bits(0x421b3cab)),
    );
    path.quad_to(
        (f32::from_bits(0x421a2b68), f32::from_bits(0x421b1585)),
        (f32::from_bits(0x421a5203), f32::from_bits(0x421aee52)),
    );
    path.quad_to(
        (f32::from_bits(0x421a527f), f32::from_bits(0x421aedd9)),
        (f32::from_bits(0x421a52fa), f32::from_bits(0x421aed60)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc1810850), f32::from_bits(0x4218848a)));
    path.quad_to(
        (f32::from_bits(0xc1541d2b), f32::from_bits(0x4211e7ca)),
        (f32::from_bits(0xc120eb59), f32::from_bits(0x42155a57)),
    );
    path.quad_to(
        (f32::from_bits(0xc0db730e), f32::from_bits(0x4218cce3)),
        (f32::from_bits(0xc0a68d10), f32::from_bits(0x422449c0)),
    );
    path.quad_to(
        (f32::from_bits(0xc0634e28), f32::from_bits(0x422fc69d)),
        (f32::from_bits(0xc08d3b78), f32::from_bits(0x423c9311)),
    );
    path.quad_to(
        (f32::from_bits(0xc0a8cfdc), f32::from_bits(0x42495f86)),
        (f32::from_bits(0xc1025b62), f32::from_bits(0x424ffc46)),
    );
    path.quad_to(
        (f32::from_bits(0xc1304ed4), f32::from_bits(0x42569906)),
        (f32::from_bits(0xc16380a6), f32::from_bits(0x42532678)),
    );
    path.quad_to(
        (f32::from_bits(0xc18b593d), f32::from_bits(0x424fb3ec)),
        (f32::from_bits(0xc19892bc), f32::from_bits(0x42443710)),
    );
    path.quad_to(
        (f32::from_bits(0xc1a5cc3c), f32::from_bits(0x4238ba32)),
        (f32::from_bits(0xc19ee722), f32::from_bits(0x422bedbe)),
    );
    path.quad_to(
        (f32::from_bits(0xc1980208), f32::from_bits(0x421f214a)),
        (f32::from_bits(0xc1810850), f32::from_bits(0x4218848a)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)));
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    );
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    );
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    );
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    );
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    );
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L1915-L2013 (chrome/m156)
fn fuzz763_3283699(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x411032d0), f32::from_bits(0xc24f69e5)));
    path.quad_to(
        (f32::from_bits(0x413e956c), f32::from_bits(0xc255d56a)),
        (f32::from_bits(0x41718a9c), f32::from_bits(0xc2522c67)),
    );
    path.quad_to(
        (f32::from_bits(0x41755c61), f32::from_bits(0xc251e62c)),
        (f32::from_bits(0x417909ca), f32::from_bits(0xc2519487)),
    );
    path.quad_to(
        (f32::from_bits(0x417cd0ce), f32::from_bits(0xc2515870)),
        (f32::from_bits(0x41804eae), f32::from_bits(0xc2510dd3)),
    );
    path.quad_to(
        (f32::from_bits(0x4199a689), f32::from_bits(0xc24d2a58)),
        (f32::from_bits(0x41a61251), f32::from_bits(0xc24174a0)),
    );
    path.quad_to(
        (f32::from_bits(0x41b27e19), f32::from_bits(0xc235bee8)),
        (f32::from_bits(0x41aab721), f32::from_bits(0xc22912fa)),
    );
    path.quad_to(
        (f32::from_bits(0x41a2f02a), f32::from_bits(0xc21c670e)),
        (f32::from_bits(0x418b84ba), f32::from_bits(0xc216312a)),
    );
    path.quad_to(
        (f32::from_bits(0x41683297), f32::from_bits(0xc20ffb46)),
        (f32::from_bits(0x413582e0), f32::from_bits(0xc213dec1)),
    );
    path.quad_to(
        (f32::from_bits(0x4132d4e6), f32::from_bits(0xc2141362)),
        (f32::from_bits(0x4130395d), f32::from_bits(0xc2144d9c)),
    );
    path.quad_to(
        (f32::from_bits(0x412d8b06), f32::from_bits(0xc21477ef)),
        (f32::from_bits(0x412ad979), f32::from_bits(0xc214a976)),
    );
    path.quad_to(
        (f32::from_bits(0x40efc890), f32::from_bits(0xc2185278)),
        (f32::from_bits(0x40bc6c64), f32::from_bits(0xc223eb20)),
    );
    path.quad_to(
        (f32::from_bits(0x4089103c), f32::from_bits(0xc22f83c7)),
        (f32::from_bits(0x40a65850), f32::from_bits(0xc23c4113)),
    );
    path.quad_to(
        (f32::from_bits(0x40c3a064), f32::from_bits(0xc248fe60)),
        (f32::from_bits(0x411032d0), f32::from_bits(0xc24f69e5)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc2121147), f32::from_bits(0xc222bcdb)));
    path.quad_to(
        (f32::from_bits(0xc208340d), f32::from_bits(0xc22b9769)),
        (f32::from_bits(0xc1f5ef7e), f32::from_bits(0xc22ae080)),
    );
    path.quad_to(
        (f32::from_bits(0xc1db76e2), f32::from_bits(0xc22a2997)),
        (f32::from_bits(0xc1c9c1c6), f32::from_bits(0xc2204c5c)),
    );
    path.quad_to(
        (f32::from_bits(0xc1b80ca9), f32::from_bits(0xc2166f21)),
        (f32::from_bits(0xc1b97a7c), f32::from_bits(0xc20932d3)),
    );
    path.quad_to(
        (f32::from_bits(0xc1bae84e), f32::from_bits(0xc1f7ed0b)),
        (f32::from_bits(0xc1cea2c4), f32::from_bits(0xc1e637ee)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e25d39), f32::from_bits(0xc1d482d2)),
        (f32::from_bits(0xc1fcd5d5), f32::from_bits(0xc1d5f0a4)),
    );
    path.quad_to(
        (f32::from_bits(0xc20ba739), f32::from_bits(0xc1d75e77)),
        (f32::from_bits(0xc21481c6), f32::from_bits(0xc1eb18ec)),
    );
    path.quad_to(
        (f32::from_bits(0xc21d5c55), f32::from_bits(0xc1fed362)),
        (f32::from_bits(0xc21ca56c), f32::from_bits(0xc20ca5ff)),
    );
    path.quad_to(
        (f32::from_bits(0xc21bee83), f32::from_bits(0xc219e24d)),
        (f32::from_bits(0xc2121147), f32::from_bits(0xc222bcdb)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)));
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    );
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc23d19f2), f32::from_bits(0x4029d790)));
    path.quad_to(
        (f32::from_bits(0xc2302ee1), f32::from_bits(0x3ff4b3e0)),
        (f32::from_bits(0xc224f322), f32::from_bits(0x406aec68)),
    );
    path.quad_to(
        (f32::from_bits(0xc219b764), f32::from_bits(0x40adbf72)),
        (f32::from_bits(0xc216bf8a), f32::from_bits(0x410a8bfe)),
    );
    path.quad_to(
        (f32::from_bits(0xc213c7b1), f32::from_bits(0x413e3841)),
        (f32::from_bits(0xc21ad0d8), f32::from_bits(0x416b273b)),
    );
    path.quad_to(
        (f32::from_bits(0xc221da00), f32::from_bits(0x418c0b1a)),
        (f32::from_bits(0xc22ec511), f32::from_bits(0x4191facd)),
    );
    path.quad_to(
        (f32::from_bits(0xc23bb022), f32::from_bits(0x4197ea80)),
        (f32::from_bits(0xc246ebe0), f32::from_bits(0x4189d830)),
    );
    path.quad_to(
        (f32::from_bits(0xc252279f), f32::from_bits(0x41778bc2)),
        (f32::from_bits(0xc2551f78), f32::from_bits(0x4143df80)),
    );
    path.quad_to(
        (f32::from_bits(0xc2581752), f32::from_bits(0x4110333c)),
        (f32::from_bits(0xc2510e2a), f32::from_bits(0x40c68884)),
    );
    path.quad_to(
        (f32::from_bits(0xc24a0502), f32::from_bits(0x40595520)),
        (f32::from_bits(0xc23d19f2), f32::from_bits(0x4029d790)),
    );
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)));
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x421951ff), f32::from_bits(0x421bef3d)),
        (f32::from_bits(0x4217f013), f32::from_bits(0x421d0f2c)),
    );
    path.line_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)));
    path.close();
    path.move_to((f32::from_bits(0x4217c478), f32::from_bits(0x421d326a)));
    path.quad_to(
        (f32::from_bits(0x4217da16), f32::from_bits(0x421d2110)),
        (f32::from_bits(0x4217f013), f32::from_bits(0x421d0f2c)),
    );
    path.quad_to(
        (f32::from_bits(0x4217da0a), f32::from_bits(0x421d2119)),
        (f32::from_bits(0x4217c478), f32::from_bits(0x421d326a)),
    );
    path.close();
    path.move_to((f32::from_bits(0x4217b2af), f32::from_bits(0x421d40a6)));
    path.quad_to(
        (f32::from_bits(0x4217bc6a), f32::from_bits(0x421d38e1)),
        (f32::from_bits(0x4217c478), f32::from_bits(0x421d326a)),
    );
    path.quad_to(
        (f32::from_bits(0x4217bc41), f32::from_bits(0x421d3902)),
        (f32::from_bits(0x4217b2af), f32::from_bits(0x421d40a6)),
    );
    path.close();
    path.move_to((f32::from_bits(0x4217b2af), f32::from_bits(0x421d40a6)));
    path.quad_to(
        (f32::from_bits(0x42167086), f32::from_bits(0x421eb7af)),
        (f32::from_bits(0x4214f81f), f32::from_bits(0x422015f3)),
    );
    path.quad_to(
        (f32::from_bits(0x420b440b), f32::from_bits(0x42291d88)),
        (f32::from_bits(0x41fc09ff), f32::from_bits(0x4228a38f)),
    );
    path.quad_to(
        (f32::from_bits(0x41e18be8), f32::from_bits(0x42282996)),
        (f32::from_bits(0x41cf7cbe), f32::from_bits(0x421e7581)),
    );
    path.quad_to(
        (f32::from_bits(0x41bd6d94), f32::from_bits(0x4214c16e)),
        (f32::from_bits(0x41be6187), f32::from_bits(0x42078262)),
    );
    path.quad_to(
        (f32::from_bits(0x41bf557a), f32::from_bits(0x41f486ac)),
        (f32::from_bits(0x41d2bda2), f32::from_bits(0x41e27782)),
    );
    path.quad_to(
        (f32::from_bits(0x41d4cf6c), f32::from_bits(0x41e08a82)),
        (f32::from_bits(0x41d6f5de), f32::from_bits(0x41ded4d5)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41f0513a)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0x420f3f16), f32::from_bits(0x42240000)),
        (f32::from_bits(0x4217b2af), f32::from_bits(0x421d40a6)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)));
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    );
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    );
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    );
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    );
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    );
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L2015-L2106 (chrome/m156)
fn fuzz763_1026368(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x4101c02c), f32::from_bits(0xc2500256))); // 8.10942f, -52.0023f
    path.quad_to(
        (f32::from_bits(0x412faeae), f32::from_bits(0xc256a13a)),
        (f32::from_bits(0x4162e312), f32::from_bits(0xc2533110)),
    ); // 10.9801f, -53.6574f, 14.1804f, -52.7979f
    path.quad_to(
        (f32::from_bits(0x418b0bbc), f32::from_bits(0xc24fc0e8)),
        (f32::from_bits(0x41984984), f32::from_bits(0xc2444547)),
    ); // 17.3807f, -51.9384f, 19.0359f, -49.0677f
    path.quad_to(
        (f32::from_bits(0x41a5874c), f32::from_bits(0xc238c9a6)),
        (f32::from_bits(0x419ea6f8), f32::from_bits(0xc22bfc8c)),
    ); // 20.6911f, -46.1969f, 19.8315f, -42.9966f
    path.quad_to(
        (f32::from_bits(0x4197c6a7), f32::from_bits(0xc21f2f74)),
        (f32::from_bits(0x4180cf66), f32::from_bits(0xc2189090)),
    ); // 18.972f, -39.7963f, 16.1013f, -38.1412f
    path.quad_to(
        (f32::from_bits(0x4153b048), f32::from_bits(0xc211f1ac)),
        (f32::from_bits(0x41207be4), f32::from_bits(0xc21561d5)),
    ); // 13.2305f, -36.486f, 10.0302f, -37.3455f
    path.quad_to(
        (f32::from_bits(0x40da8f00), f32::from_bits(0xc218d1fe)),
        (f32::from_bits(0x40a597e0), f32::from_bits(0xc2244d9f)),
    ); // 6.82996f, -38.2051f, 5.17479f, -41.0758f
    path.quad_to(
        (f32::from_bits(0x40614180), f32::from_bits(0xc22fc940)),
        (f32::from_bits(0x408c220c), f32::from_bits(0xc23c9658)),
    ); // 3.51962f, -43.9465f, 4.37916f, -47.1468f
    path.quad_to(
        (f32::from_bits(0x40a7a350), f32::from_bits(0xc2496372)),
        (f32::from_bits(0x4101c02c), f32::from_bits(0xc2500256)),
    ); // 5.23869f, -50.3471f, 8.10942f, -52.0023f
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455))); // -19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    ); // -11.598f, -28, 0, -28
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    ); // 11.598f, -28, 19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    ); // 28, -11.598f, 28, 0
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    ); // 28, 11.598f, 19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    ); // 13.7163f, 25.8817f, 5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    ); // 8, 29.7635f, 8, 33
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    ); // 8, 36.3137f, 5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    ); // 3.31371f, 41, 0, 41
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    ); // -3.31371f, 41, -5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    ); // -8, 36.3137f, -8, 33
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    ); // -8, 29.7635f, -5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    ); // -13.7163f, 25.8817f, -19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    ); // -28, 11.598f, -28, 0
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    ); // -28, -11.598f, -19.799f, -19.799f
    path.close();
    path.move_to((f32::from_bits(0xc23c9b95), f32::from_bits(0x408a5bd8))); // -47.1519f, 4.32371f
    path.quad_to(
        (f32::from_bits(0xc22fcd74), f32::from_bits(0x405df2c8)),
        (f32::from_bits(0xc22453d3), f32::from_bits(0x40a40c28)),
    ); // -43.9506f, 3.46794f, -41.0819f, 5.12648f
    path.quad_to(
        (f32::from_bits(0xc218da31), f32::from_bits(0x40d91eee)),
        (f32::from_bits(0xc2156de2), f32::from_bits(0x411fc7fd)),
    ); // -38.2131f, 6.78503f, -37.3573f, 9.98633f
    path.quad_to(
        (f32::from_bits(0xc2120193), f32::from_bits(0x41530084)),
        (f32::from_bits(0xc218a3ec), f32::from_bits(0x41807386)),
    ); // -36.5015f, 13.1876f, -38.1601f, 16.0564f
    path.quad_to(
        (f32::from_bits(0xc21f4644), f32::from_bits(0x419766c9)),
        (f32::from_bits(0xc22c1466), f32::from_bits(0x419e3f66)),
    ); // -39.8186f, 18.9252f, -43.0199f, 19.781f
    path.quad_to(
        (f32::from_bits(0xc238e288), f32::from_bits(0x41a51804)),
        (f32::from_bits(0xc2445c2a), f32::from_bits(0x4197d353)),
    ); // -46.2212f, 20.6367f, -49.09f, 18.9782f
    path.quad_to(
        (f32::from_bits(0xc24fd5cc), f32::from_bits(0x418a8ea2)),
        (f32::from_bits(0xc253421a), f32::from_bits(0x4161e4ba)),
    ); // -51.9588f, 17.3196f, -52.8146f, 14.1183f
    path.quad_to(
        (f32::from_bits(0xc256ae69), f32::from_bits(0x412eac36)),
        (f32::from_bits(0xc2500c10), f32::from_bits(0x4100c5b0)),
    ); // -53.6703f, 10.917f, -52.0118f, 8.04826f
    path.quad_to(
        (f32::from_bits(0xc24969b8), f32::from_bits(0x40a5be50)),
        (f32::from_bits(0xc23c9b95), f32::from_bits(0x408a5bd8)),
    ); // -50.3532f, 5.17948f, -47.1519f, 4.32371f
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3))); // 27.3431f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    ); // 29.6863f, 25, 33, 25
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    ); // 36.3137f, 25, 38.6569f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    ); // 41, 29.6863f, 41, 33
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    ); // 41, 36.3137f, 38.6569f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x421a9aa6), f32::from_bits(0x421aa696)),
        (f32::from_bits(0x421a9438), f32::from_bits(0x421aacfe)),
    ); // 38.651f, 38.6627f, 38.6447f, 38.6689f
    path.quad_to(
        (f32::from_bits(0x421a8ec8), f32::from_bits(0x421ab270)),
        (f32::from_bits(0x421a8956), f32::from_bits(0x421ab7e2)),
    ); // 38.6394f, 38.6743f, 38.6341f, 38.6796f
    path.quad_to(
        (f32::from_bits(0x421a8386), f32::from_bits(0x421abdb2)),
        (f32::from_bits(0x421a7db3), f32::from_bits(0x421ac381)),
    ); // 38.6284f, 38.6852f, 38.6228f, 38.6909f
    path.quad_to(
        (f32::from_bits(0x42111c34), f32::from_bits(0x422420c6)),
        (f32::from_bits(0x4203daf7), f32::from_bits(0x42241dc8)),
    ); // 36.2775f, 41.032f, 32.9638f, 41.0291f
    path.quad_to(
        (f32::from_bits(0x41ed3376), f32::from_bits(0x42241aca)),
        (f32::from_bits(0x41da78ee), f32::from_bits(0x421ab94a)),
    ); // 29.6501f, 41.0262f, 27.309f, 38.6809f
    path.quad_to(
        (f32::from_bits(0x41c7be68), f32::from_bits(0x421157cc)),
        (f32::from_bits(0x41c7c464), f32::from_bits(0x4204168f)),
    ); // 24.968f, 36.3357f, 24.9709f, 33.022f
    path.quad_to(
        (f32::from_bits(0x41c7ca5f), f32::from_bits(0x41edaaa5)),
        (f32::from_bits(0x41da8d5d), f32::from_bits(0x41daf01d)),
    ); // 24.9738f, 29.7083f, 27.319f, 27.3672f
    path.quad_to(
        (f32::from_bits(0x41da915b), f32::from_bits(0x41daec21)),
        (f32::from_bits(0x41da955a), f32::from_bits(0x41dae825)),
    ); // 27.321f, 27.3653f, 27.3229f, 27.3634f
    path.quad_to(
        (f32::from_bits(0x41da9997), f32::from_bits(0x41dae3e8)),
        (f32::from_bits(0x41da9dd4), f32::from_bits(0x41dadfac)),
    ); // 27.325f, 27.3613f, 27.3271f, 27.3592f
    path.quad_to(
        (f32::from_bits(0x41daa667), f32::from_bits(0x41dad71b)),
        (f32::from_bits(0x41daaefc), f32::from_bits(0x41dace8e)),
    ); // 27.3313f, 27.355f, 27.3354f, 27.3509f
    path.quad_to(
        (f32::from_bits(0x41daaa91), f32::from_bits(0x41dad2fb)),
        (f32::from_bits(0x41daa628), f32::from_bits(0x41dad768)),
    ); // 27.3333f, 27.353f, 27.3311f, 27.3552f
    path.quad_to(
        (f32::from_bits(0x41dab217), f32::from_bits(0x41dacb89)),
        (f32::from_bits(0x41dabedf), f32::from_bits(0x41dabedf)),
    ); // 27.337f, 27.3494f, 27.3432f, 27.3432f
    path.line_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3))); // 27.3431f, 27.3431f
    path.close();
    path.move_to((f32::from_bits(0xc180b86e), f32::from_bits(0x42189568))); // -16.0901f, 38.1459f
    path.quad_to(
        (f32::from_bits(0xc1538459), f32::from_bits(0x4211f5a8)),
        (f32::from_bits(0xc1204eec), f32::from_bits(0x421564da)),
    ); // -13.2198f, 36.4899f, -10.0193f, 37.3485f
    path.quad_to(
        (f32::from_bits(0xc0da32fc), f32::from_bits(0x4218d40d)),
        (f32::from_bits(0xc0a534f4), f32::from_bits(0x42244f2e)),
    ); // -6.81872f, 38.2071f, -5.16271f, 41.0773f
    path.quad_to(
        (f32::from_bits(0xc0606dd0), f32::from_bits(0x422fca4f)),
        (f32::from_bits(0xc08bb080), f32::from_bits(0x423c97aa)),
    ); // -3.5067f, 43.9476f, -4.3653f, 47.1481f
    path.quad_to(
        (f32::from_bits(0xc0a72a14), f32::from_bits(0x42496506)),
        (f32::from_bits(0xc101818e), f32::from_bits(0x425004c7)),
    ); // -5.22389f, 50.3487f, -8.09413f, 52.0047f
    path.quad_to(
        (f32::from_bits(0xc12f6e12), f32::from_bits(0x4256a488)),
        (f32::from_bits(0xc162a37e), f32::from_bits(0x42533554)),
    ); // -10.9644f, 53.6607f, -14.1649f, 52.8021f
    path.quad_to(
        (f32::from_bits(0xc18aec77), f32::from_bits(0x424fc622)),
        (f32::from_bits(0xc1982bfa), f32::from_bits(0x42444b02)),
    ); // -17.3655f, 51.9435f, -19.0215f, 49.0732f
    path.quad_to(
        (f32::from_bits(0xc1a56b7c), f32::from_bits(0x4238cfe0)),
        (f32::from_bits(0xc19e8d15), f32::from_bits(0x422c0285)),
    ); // -20.6775f, 46.203f, -19.8189f, 43.0025f
    path.quad_to(
        (f32::from_bits(0xc197aeb0), f32::from_bits(0x421f352a)),
        (f32::from_bits(0xc180b86e), f32::from_bits(0x42189568)),
    ); // -18.9603f, 39.8019f, -16.0901f, 38.1459f
    path.close();
    path.move_to((f32::from_bits(0x421a8f2f), f32::from_bits(0x421ab201))); // 38.6398f, 38.6738f
    path.line_to((f32::from_bits(0x421a9433), f32::from_bits(0x421aacf9))); // 38.6447f, 38.6689f
    path.line_to((f32::from_bits(0x421a9438), f32::from_bits(0x421aacfe))); // 38.6447f, 38.6689f
    path.quad_to(
        (f32::from_bits(0x421a91b4), f32::from_bits(0x421aaf80)),
        (f32::from_bits(0x421a8f2f), f32::from_bits(0x421ab201)),
    ); // 38.6423f, 38.6714f, 38.6398f, 38.6738f
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42240000), f32::from_bits(0x42040000))); // 41, 33
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    ); // 41, 36.3137f, 38.6569f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    ); // 36.3137f, 41, 33, 41
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    ); // 29.6863f, 41, 27.3431f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    ); // 25, 36.3137f, 25, 33
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    ); // 25, 29.6863f, 27.3431f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    ); // 29.6863f, 25, 33, 25
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    ); // 36.3137f, 25, 38.6569f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    ); // 41, 29.6863f, 41, 33
    path.close();
    let path2 = path.detach();
    // DEBUG_UNDER_DEVELOPMENT  fuzz763_1026368 disable expectation check for now
    test_path_op_check(reporter, &path1, &path2, PathOp::Union, filename, true);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L2108-L2212 (chrome/m156)
fn fuzz763_5485218(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xc1b1a434), f32::from_bits(0xc247d348)));
    path.quad_to(
        (f32::from_bits(0xc1996ac1), f32::from_bits(0xc24d3588)),
        (f32::from_bits(0xc180ac87), f32::from_bits(0xc248738e)),
    );
    path.quad_to(
        (f32::from_bits(0xc14fdc9c), f32::from_bits(0xc243b194)),
        (f32::from_bits(0xc13a53a0), f32::from_bits(0xc23794da)),
    );
    path.quad_to(
        (f32::from_bits(0xc124caa4), f32::from_bits(0xc22b7821)),
        (f32::from_bits(0xc137d28c), f32::from_bits(0xc21f1904)),
    );
    path.quad_to(
        (f32::from_bits(0xc14ada74), f32::from_bits(0xc212b9e7)),
        (f32::from_bits(0xc17b4d59), f32::from_bits(0xc20d57a8)),
    );
    path.quad_to(
        (f32::from_bits(0xc195e020), f32::from_bits(0xc207f569)),
        (f32::from_bits(0xc1ae9e58), f32::from_bits(0xc20cb763)),
    );
    path.quad_to(
        (f32::from_bits(0xc1c75c92), f32::from_bits(0xc211795d)),
        (f32::from_bits(0xc1d22110), f32::from_bits(0xc21d9616)),
    );
    path.quad_to(
        (f32::from_bits(0xc1dce590), f32::from_bits(0xc229b2d0)),
        (f32::from_bits(0xc1d3619c), f32::from_bits(0xc23611ec)),
    );
    path.quad_to(
        (f32::from_bits(0xc1c9dda8), f32::from_bits(0xc242710a)),
        (f32::from_bits(0xc1b1a434), f32::from_bits(0xc247d348)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)));
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    );
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x40b3b5dc), f32::from_bits(0x421aca81)),
        (f32::from_bits(0x40b26487), f32::from_bits(0x421af3a3)),
    );
    path.quad_to(
        (f32::from_bits(0x40eb2464), f32::from_bits(0x421ca9a9)),
        (f32::from_bits(0x410d8414), f32::from_bits(0x4221c755)),
    );
    path.quad_to(
        (f32::from_bits(0x4135d252), f32::from_bits(0x422a63dc)),
        (f32::from_bits(0x4139f710), f32::from_bits(0x42379ab7)),
    );
    path.quad_to(
        (f32::from_bits(0x413e1bd4), f32::from_bits(0x4244d193)),
        (f32::from_bits(0x411ba9b8), f32::from_bits(0x424ee522)),
    );
    path.quad_to(
        (f32::from_bits(0x40f26f3c), f32::from_bits(0x4258f8b2)),
        (f32::from_bits(0x4088b85c), f32::from_bits(0x425a01e2)),
    );
    path.quad_to(
        (f32::from_bits(0x3f780c00), f32::from_bits(0x425b0b12)),
        (f32::from_bits(0xbfc66bf0), f32::from_bits(0x42526e8c)),
    );
    path.quad_to(
        (f32::from_bits(0xc0823778), f32::from_bits(0x4249d205)),
        (f32::from_bits(0xc08a80f8), f32::from_bits(0x423c9b29)),
    );
    path.quad_to(
        (f32::from_bits(0xc092ca7c), f32::from_bits(0x422f644e)),
        (f32::from_bits(0xc01bcc90), f32::from_bits(0x422550be)),
    );
    path.quad_to(
        (f32::from_bits(0xc00c0f96), f32::from_bits(0x42242a18)),
        (f32::from_bits(0xbff6b9b1), f32::from_bits(0x422321a7)),
    );
    path.quad_to(
        (f32::from_bits(0xc080dd2a), f32::from_bits(0x42212597)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    );
    path.close();
    path.move_to((f32::from_bits(0x422e1dec), f32::from_bits(0xc1951160)));
    path.quad_to(
        (f32::from_bits(0x423b0222), f32::from_bits(0xc19b3b8f)),
        (f32::from_bits(0x42464db6), f32::from_bits(0xc18d5c4c)),
    );
    path.quad_to(
        (f32::from_bits(0x42519948), f32::from_bits(0xc17efa12)),
        (f32::from_bits(0x4254ae60), f32::from_bits(0xc14b693a)),
    );
    path.quad_to(
        (f32::from_bits(0x4257c378), f32::from_bits(0xc117d862)),
        (f32::from_bits(0x4250d3d6), f32::from_bits(0xc0d5542c)),
    );
    path.quad_to(
        (f32::from_bits(0x4249e434), f32::from_bits(0xc075ef28)),
        (f32::from_bits(0x423cfffe), f32::from_bits(0xc0449db8)),
    );
    path.quad_to(
        (f32::from_bits(0x423cdcf0), f32::from_bits(0xc044179d)),
        (f32::from_bits(0x423cb9cd), f32::from_bits(0xc04395ab)),
    );
    path.quad_to(
        (f32::from_bits(0x423cdda4), f32::from_bits(0xc0435d6a)),
        (f32::from_bits(0x423d018a), f32::from_bits(0xc04320b0)),
    );
    path.quad_to(
        (f32::from_bits(0x424a2ff8), f32::from_bits(0xc02cd470)),
        (f32::from_bits(0x425285c7), f32::from_bits(0xbdfdd900)),
    );
    path.quad_to(
        (f32::from_bits(0x425adb97), f32::from_bits(0x401cf6e8)),
        (f32::from_bits(0x425976d3), f32::from_bits(0x40b7eee0)),
    );
    path.quad_to(
        (f32::from_bits(0x42581210), f32::from_bits(0x4110b128)),
        (f32::from_bits(0x424dc3b4), f32::from_bits(0x41320868)),
    );
    path.quad_to(
        (f32::from_bits(0x4243755a), f32::from_bits(0x41535fa6)),
        (f32::from_bits(0x423646ec), f32::from_bits(0x414dcc96)),
    );
    path.quad_to(
        (f32::from_bits(0x4229187e), f32::from_bits(0x41483989)),
        (f32::from_bits(0x4220c2ae), f32::from_bits(0x411f001d)),
    );
    path.quad_to(
        (f32::from_bits(0x42186cde), f32::from_bits(0x40eb8d66)),
        (f32::from_bits(0x4219d1a2), f32::from_bits(0x408219f4)),
    );
    path.quad_to(
        (f32::from_bits(0x421b3666), f32::from_bits(0x3f453420)),
        (f32::from_bits(0x422584c1), f32::from_bits(0xbfa81fe0)),
    );
    path.quad_to(
        (f32::from_bits(0x422b4324), f32::from_bits(0xc01e631a)),
        (f32::from_bits(0x4231e626), f32::from_bits(0xc0385d42)),
    );
    path.quad_to(
        (f32::from_bits(0x422b1bb8), f32::from_bits(0xc0446d07)),
        (f32::from_bits(0x4224d036), f32::from_bits(0xc0812328)),
    );
    path.quad_to(
        (f32::from_bits(0x421984a2), f32::from_bits(0xc0b8a034)),
        (f32::from_bits(0x42166f8b), f32::from_bits(0xc10fe0f3)),
    );
    path.quad_to(
        (f32::from_bits(0x42135a74), f32::from_bits(0xc14371ca)),
        (f32::from_bits(0x421a4a15), f32::from_bits(0xc170a016)),
    );
    path.quad_to(
        (f32::from_bits(0x422139b6), f32::from_bits(0xc18ee731)),
        (f32::from_bits(0x422e1dec), f32::from_bits(0xc1951160)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)));
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    );
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    );
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    );
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    );
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    );
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    );
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)));
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc1e59c15), f32::from_bits(0x41eeb1f9)));
    path.quad_to(
        (f32::from_bits(0xc1cb6c81), f32::from_bits(0x41ea9074)),
        (f32::from_bits(0xc1b5fc96), f32::from_bits(0x41fa28d2)),
    );
    path.quad_to(
        (f32::from_bits(0xc1a08cac), f32::from_bits(0x4204e098)),
        (f32::from_bits(0xc19c6b27), f32::from_bits(0x4211f862)),
    );
    path.quad_to(
        (f32::from_bits(0xc19849a3), f32::from_bits(0x421f102b)),
        (f32::from_bits(0xc1a7e200), f32::from_bits(0x4229c820)),
    );
    path.quad_to(
        (f32::from_bits(0xc1b77a5e), f32::from_bits(0x42348016)),
        (f32::from_bits(0xc1d1a9f2), f32::from_bits(0x423690d8)),
    );
    path.quad_to(
        (f32::from_bits(0xc1ebd984), f32::from_bits(0x4238a19a)),
        (f32::from_bits(0xc200a4b7), f32::from_bits(0x4230d56b)),
    );
    path.quad_to(
        (f32::from_bits(0xc20b5cae), f32::from_bits(0x4229093c)),
        (f32::from_bits(0xc20d6d70), f32::from_bits(0x421bf173)),
    );
    path.quad_to(
        (f32::from_bits(0xc20f7e32), f32::from_bits(0x420ed9a9)),
        (f32::from_bits(0xc207b202), f32::from_bits(0x420421b4)),
    );
    path.quad_to(
        (f32::from_bits(0xc1ffcba7), f32::from_bits(0x41f2d37d)),
        (f32::from_bits(0xc1e59c15), f32::from_bits(0x41eeb1f9)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x425285c7), f32::from_bits(0xbdfdd900)));
    path.quad_to(
        (f32::from_bits(0x425adb97), f32::from_bits(0x401cf6e8)),
        (f32::from_bits(0x425976d3), f32::from_bits(0x40b7eee0)),
    );
    path.quad_to(
        (f32::from_bits(0x42581210), f32::from_bits(0x4110b128)),
        (f32::from_bits(0x424dc3b4), f32::from_bits(0x41320868)),
    );
    path.quad_to(
        (f32::from_bits(0x4243755a), f32::from_bits(0x41535fa6)),
        (f32::from_bits(0x423646ec), f32::from_bits(0x414dcc96)),
    );
    path.quad_to(
        (f32::from_bits(0x4229187e), f32::from_bits(0x41483989)),
        (f32::from_bits(0x4220c2ae), f32::from_bits(0x411f001d)),
    );
    path.quad_to(
        (f32::from_bits(0x42186cde), f32::from_bits(0x40eb8d66)),
        (f32::from_bits(0x4219d1a2), f32::from_bits(0x408219f4)),
    );
    path.quad_to(
        (f32::from_bits(0x421b3666), f32::from_bits(0x3f453420)),
        (f32::from_bits(0x422584c1), f32::from_bits(0xbfa81fe0)),
    );
    path.quad_to(
        (f32::from_bits(0x422fd31c), f32::from_bits(0xc0596cf0)),
        (f32::from_bits(0x423d018a), f32::from_bits(0xc04320b0)),
    );
    path.quad_to(
        (f32::from_bits(0x424a2ff8), f32::from_bits(0xc02cd470)),
        (f32::from_bits(0x425285c7), f32::from_bits(0xbdfdd900)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L2214-L2319 (chrome/m156)
fn fuzz763_2674194(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xbfb16e10), f32::from_bits(0xc252733b)));
    path.quad_to(
        (f32::from_bits(0x3f91df50), f32::from_bits(0xc25b07b4)),
        (f32::from_bits(0x408e27f4), f32::from_bits(0xc259f3f8)),
    );
    path.quad_to(
        (f32::from_bits(0x40f7d814), f32::from_bits(0xc258e03e)),
        (f32::from_bits(0x411e3df0), f32::from_bits(0xc24ec5d2)),
    );
    path.quad_to(
        (f32::from_bits(0x41408fd4), f32::from_bits(0xc244ab67)),
        (f32::from_bits(0x413c40e6), f32::from_bits(0xc2377562)),
    );
    path.quad_to(
        (f32::from_bits(0x4137f1f8), f32::from_bits(0xc22a3f5f)),
        (f32::from_bits(0x410f884c), f32::from_bits(0xc221aae6)),
    );
    path.quad_to(
        (f32::from_bits(0x40ce3d3c), f32::from_bits(0xc219166c)),
        (f32::from_bits(0x40491a38), f32::from_bits(0xc21a2a28)),
    );
    path.quad_to(
        (f32::from_bits(0xbe246080), f32::from_bits(0xc21b3de4)),
        (f32::from_bits(0xc0138d98), f32::from_bits(0xc225584f)),
    );
    path.quad_to(
        (f32::from_bits(0xc08e6a98), f32::from_bits(0xc22f72ba)),
        (f32::from_bits(0xc085ccbc), f32::from_bits(0xc23ca8be)),
    );
    path.quad_to(
        (f32::from_bits(0xc07a5dc0), f32::from_bits(0xc249dec2)),
        (f32::from_bits(0xbfb16e10), f32::from_bits(0xc252733b)),
    );
    path.close();
    path.move_to((f32::from_bits(0x41b47dea), f32::from_bits(0xc23e32f4)));
    path.quad_to(
        (f32::from_bits(0x41ce465c), f32::from_bits(0xc24147ff)),
        (f32::from_bits(0x41e4dd74), f32::from_bits(0xc23a5854)),
    );
    path.quad_to(
        (f32::from_bits(0x41fb748e), f32::from_bits(0xc23368a8)),
        (f32::from_bits(0x4200cf52), f32::from_bits(0xc226846f)),
    );
    path.quad_to(
        (f32::from_bits(0x42022efd), f32::from_bits(0xc220c591)),
        (f32::from_bits(0x42019135), f32::from_bits(0xc21b57df)),
    );
    path.quad_to(
        (f32::from_bits(0x4206cc04), f32::from_bits(0xc21cec81)),
        (f32::from_bits(0x420cb220), f32::from_bits(0xc21c9a86)),
    );
    path.quad_to(
        (f32::from_bits(0x4219ee60), f32::from_bits(0xc21be297)),
        (f32::from_bits(0x4222c82a), f32::from_bits(0xc21204ac)),
    );
    path.quad_to(
        (f32::from_bits(0x422ba1f5), f32::from_bits(0xc20826c2)),
        (f32::from_bits(0x422aea06), f32::from_bits(0xc1f5d504)),
    );
    path.quad_to(
        (f32::from_bits(0x422a3216), f32::from_bits(0xc1db5c85)),
        (f32::from_bits(0x4220542b), f32::from_bits(0xc1c9a8f0)),
    );
    path.quad_to(
        (f32::from_bits(0x42167641), f32::from_bits(0xc1b7f55b)),
        (f32::from_bits(0x42093a01), f32::from_bits(0xc1b9653a)),
    );
    path.quad_to(
        (f32::from_bits(0x41f7fb83), f32::from_bits(0xc1bad519)),
        (f32::from_bits(0x41e647ee), f32::from_bits(0xc1ce90ee)),
    );
    path.quad_to(
        (f32::from_bits(0x41d49459), f32::from_bits(0xc1e24cc3)),
        (f32::from_bits(0x41d60438), f32::from_bits(0xc1fcc542)),
    );
    path.quad_to(
        (f32::from_bits(0x41d62223), f32::from_bits(0xc1feec5b)),
        (f32::from_bits(0x41d65f09), f32::from_bits(0xc2008251)),
    );
    path.quad_to(
        (f32::from_bits(0x41d45a68), f32::from_bits(0xc200343d)),
        (f32::from_bits(0x41d2419c), f32::from_bits(0xc1ffe823)),
    );
    path.quad_to(
        (f32::from_bits(0x41b8792a), f32::from_bits(0xc1f9be0c)),
        (f32::from_bits(0x41a1e211), f32::from_bits(0xc203ceb1)),
    );
    path.quad_to(
        (f32::from_bits(0x418b4af8), f32::from_bits(0xc20abe5d)),
        (f32::from_bits(0x418520e1), f32::from_bits(0xc217a296)),
    );
    path.quad_to(
        (f32::from_bits(0x417ded93), f32::from_bits(0xc22486cf)),
        (f32::from_bits(0x418cd620), f32::from_bits(0xc22fd25b)),
    );
    path.quad_to(
        (f32::from_bits(0x419ab578), f32::from_bits(0xc23b1de8)),
        (f32::from_bits(0x41b47dea), f32::from_bits(0xc23e32f4)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)));
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    );
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.line_to((f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)));
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    );
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    );
    path.close();
    path.move_to((f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)));
    path.quad_to(
        (f32::from_bits(0x40b3bf9e), f32::from_bits(0x421ac949)),
        (f32::from_bits(0x40b278ad), f32::from_bits(0x421af12e)),
    );
    path.quad_to(
        (f32::from_bits(0x40d4fa46), f32::from_bits(0x42187664)),
        (f32::from_bits(0x40ff010e), f32::from_bits(0x42175b2b)),
    );
    path.quad_to(
        (f32::from_bits(0x41336223), f32::from_bits(0x42149fe4)),
        (f32::from_bits(0x415fcb81), f32::from_bits(0x421bdd50)),
    );
    path.quad_to(
        (f32::from_bits(0x41861a70), f32::from_bits(0x42231aba)),
        (f32::from_bits(0x418b90fc), f32::from_bits(0x42301322)),
    );
    path.quad_to(
        (f32::from_bits(0x4191078a), f32::from_bits(0x423d0b88)),
        (f32::from_bits(0x41828cb4), f32::from_bits(0x424825e0)),
    );
    path.quad_to(
        (f32::from_bits(0x416823bc), f32::from_bits(0x42534038)),
        (f32::from_bits(0x41344220), f32::from_bits(0x4255fb7e)),
    );
    path.quad_to(
        (f32::from_bits(0x41006082), f32::from_bits(0x4258b6c5)),
        (f32::from_bits(0x40a7ee48), f32::from_bits(0x4251795a)),
    );
    path.quad_to(
        (f32::from_bits(0x401e3718), f32::from_bits(0x424a3bef)),
        (f32::from_bits(0x3fe50570), f32::from_bits(0x423d4388)),
    );
    path.quad_to(
        (f32::from_bits(0x3f8d9c90), f32::from_bits(0x42304b20)),
        (f32::from_bits(0x403aa4f8), f32::from_bits(0x422530c9)),
    );
    path.quad_to(
        (f32::from_bits(0x4059e097), f32::from_bits(0x4222326b)),
        (f32::from_bits(0x407fc660), f32::from_bits(0x421fcfdc)),
    );
    path.line_to((f32::from_bits(0x407fc672), f32::from_bits(0x421fcfdb)));
    path.quad_to(
        (f32::from_bits(0x409c2918), f32::from_bits(0x421dbc1a)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc23fe7ce), f32::from_bits(0xc1ad1dad)));
    path.quad_to(
        (f32::from_bits(0xc2341588), f32::from_bits(0xc1b91b57)),
        (f32::from_bits(0xc2277c56), f32::from_bits(0xc1b0de1d)),
    );
    path.quad_to(
        (f32::from_bits(0xc21ae326), f32::from_bits(0xc1a8a0e6)),
        (f32::from_bits(0xc214e451), f32::from_bits(0xc190fc58)),
    );
    path.quad_to(
        (f32::from_bits(0xc20ee57c), f32::from_bits(0xc172af93)),
        (f32::from_bits(0xc2130419), f32::from_bits(0xc1404ad0)),
    );
    path.quad_to(
        (f32::from_bits(0xc21722b5), f32::from_bits(0xc10de60c)),
        (f32::from_bits(0xc222f4fc), f32::from_bits(0xc0ebd570)),
    );
    path.quad_to(
        (f32::from_bits(0xc22ec743), f32::from_bits(0xc0bbdec8)),
        (f32::from_bits(0xc23b6074), f32::from_bits(0xc0dcd3b4)),
    );
    path.quad_to(
        (f32::from_bits(0xc247f9a4), f32::from_bits(0xc0fdc894)),
        (f32::from_bits(0xc24df87a), f32::from_bits(0xc12e2d66)),
    );
    path.quad_to(
        (f32::from_bits(0xc253f74e), f32::from_bits(0xc15d7682)),
        (f32::from_bits(0xc24fd8b1), f32::from_bits(0xc187eda2)),
    );
    path.quad_to(
        (f32::from_bits(0xc24bba16), f32::from_bits(0xc1a12003)),
        (f32::from_bits(0xc23fe7ce), f32::from_bits(0xc1ad1dad)),
    );
    path.close();
    path.move_to((f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)));
    path.quad_to(
        (f32::from_bits(0xc22fcb5c), f32::from_bits(0x405f9a18)),
        (f32::from_bits(0xc22450bb), f32::from_bits(0x40a4d200)),
    );
    path.quad_to(
        (f32::from_bits(0xc218d61a), f32::from_bits(0x40d9d6f4)),
        (f32::from_bits(0xc21567dd), f32::from_bits(0x412021ef)),
    );
    path.quad_to(
        (f32::from_bits(0xc211f9a2), f32::from_bits(0x41535866)),
        (f32::from_bits(0xc2189a40), f32::from_bits(0x4180a176)),
    );
    path.quad_to(
        (f32::from_bits(0xc21f3ade), f32::from_bits(0x419796b9)),
        (f32::from_bits(0xc22c087c), f32::from_bits(0x419e7330)),
    );
    path.quad_to(
        (f32::from_bits(0xc238d61a), f32::from_bits(0x41a54fa9)),
        (f32::from_bits(0xc24450bb), f32::from_bits(0x41980e6c)),
    );
    path.quad_to(
        (f32::from_bits(0xc24fcb5c), f32::from_bits(0x418acd2f)),
        (f32::from_bits(0xc2533998), f32::from_bits(0x416263e8)),
    );
    path.quad_to(
        (f32::from_bits(0xc256a7d4), f32::from_bits(0x412f2d72)),
        (f32::from_bits(0xc2500736), f32::from_bits(0x410142ec)),
    );
    path.quad_to(
        (f32::from_bits(0xc2496698), f32::from_bits(0x40a6b0cc)),
        (f32::from_bits(0xc23c98fa), f32::from_bits(0x408b3eec)),
    );
    path.close();
    path.move_to((f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)));
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    );
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    );
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    );
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41828cb4), f32::from_bits(0x424825e0)));
    path.quad_to(
        (f32::from_bits(0x416823bc), f32::from_bits(0x42534038)),
        (f32::from_bits(0x41344220), f32::from_bits(0x4255fb7e)),
    );
    path.quad_to(
        (f32::from_bits(0x41006082), f32::from_bits(0x4258b6c5)),
        (f32::from_bits(0x40a7ee48), f32::from_bits(0x4251795a)),
    );
    path.quad_to(
        (f32::from_bits(0x401e3718), f32::from_bits(0x424a3bef)),
        (f32::from_bits(0x3fe50570), f32::from_bits(0x423d4388)),
    );
    path.quad_to(
        (f32::from_bits(0x3f8d9c90), f32::from_bits(0x42304b20)),
        (f32::from_bits(0x403aa4f8), f32::from_bits(0x422530c9)),
    );
    path.quad_to(
        (f32::from_bits(0x40973dd4), f32::from_bits(0x421a1671)),
        (f32::from_bits(0x40ff010e), f32::from_bits(0x42175b2b)),
    );
    path.quad_to(
        (f32::from_bits(0x41336223), f32::from_bits(0x42149fe4)),
        (f32::from_bits(0x415fcb81), f32::from_bits(0x421bdd50)),
    );
    path.quad_to(
        (f32::from_bits(0x41861a70), f32::from_bits(0x42231aba)),
        (f32::from_bits(0x418b90fc), f32::from_bits(0x42301322)),
    );
    path.quad_to(
        (f32::from_bits(0x4191078a), f32::from_bits(0x423d0b88)),
        (f32::from_bits(0x41828cb4), f32::from_bits(0x424825e0)),
    );
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L2321-L2405 (chrome/m156)
fn fuzz763_10022998(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x40f23d54), f32::from_bits(0xc250558c))); // 7.56999f, -52.0835f
    path.quad_to(
        (f32::from_bits(0x4126c646), f32::from_bits(0xc25712d1)),
        (f32::from_bits(0x415a1e76), f32::from_bits(0xc253c4aa)),
    ); // 10.4234f, -53.7684f, 13.6324f, -52.9421f
    path.quad_to(
        (f32::from_bits(0x4186bb52), f32::from_bits(0xc2507686)),
        (f32::from_bits(0x419435db), f32::from_bits(0xc2450c9e)),
    ); // 16.8415f, -52.1157f, 18.5263f, -49.2623f
    path.quad_to(
        (f32::from_bits(0x41a1b065), f32::from_bits(0xc239a2b8)),
        (f32::from_bits(0x419b1418), f32::from_bits(0xc22cccac)),
    ); // 20.2111f, -46.4089f, 19.3848f, -43.1999f
    path.quad_to(
        (f32::from_bits(0x419477ce), f32::from_bits(0xc21ff6a0)),
        (f32::from_bits(0x417b47ff), f32::from_bits(0xc219395c)),
    ); // 18.5585f, -39.9908f, 15.7051f, -38.306f
    path.quad_to(
        (f32::from_bits(0x414da063), f32::from_bits(0xc2127c16)),
        (f32::from_bits(0x411a4835), f32::from_bits(0xc215ca3d)),
    ); // 12.8517f, -36.6212f, 9.64263f, -37.4475f
    path.quad_to(
        (f32::from_bits(0x40cde00c), f32::from_bits(0xc2191862)),
        (f32::from_bits(0x4097f5e4), f32::from_bits(0xc2248249)),
    ); // 6.4336f, -38.2738f, 4.74877f, -41.1272f
    path.quad_to(
        (f32::from_bits(0x40441780), f32::from_bits(0xc22fec30)),
        (f32::from_bits(0x4078f9e8), f32::from_bits(0xc23cc23c)),
    ); // 3.06393f, -43.9807f, 3.89025f, -47.1897f
    path.quad_to(
        (f32::from_bits(0x4096ee1c), f32::from_bits(0xc2499848)),
        (f32::from_bits(0x40f23d54), f32::from_bits(0xc250558c)),
    ); // 4.71657f, -50.3987f, 7.56999f, -52.0835f
    path.close();
    path.move_to((f32::from_bits(0xc2066415), f32::from_bits(0xc2220be8))); // -33.5977f, -40.5116f
    path.quad_to(
        (f32::from_bits(0xc1f2466b), f32::from_bits(0xc2223dac)),
        (f32::from_bits(0xc1df41cc), f32::from_bits(0xc21901bc)),
    ); // -30.2844f, -40.5602f, -27.9071f, -38.2517f
    path.quad_to(
        (f32::from_bits(0xc1cc3d2d), f32::from_bits(0xc20fc5cd)),
        (f32::from_bits(0xc1cbd9a6), f32::from_bits(0xc20284ee)),
    ); // -25.5299f, -35.9432f, -25.4813f, -32.6298f
    path.quad_to(
        (f32::from_bits(0xc1cb761f), f32::from_bits(0xc1ea881c)),
        (f32::from_bits(0xc1ddedfe), f32::from_bits(0xc1d7837e)),
    ); // -25.4327f, -29.3165f, -27.7412f, -26.9392f
    path.quad_to(
        (f32::from_bits(0xc1f065dc), f32::from_bits(0xc1c47edf)),
        (f32::from_bits(0xc20573ce), f32::from_bits(0xc1c41b58)),
    ); // -30.0497f, -24.5619f, -33.3631f, -24.5134f
    path.quad_to(
        (f32::from_bits(0xc212b4ad), f32::from_bits(0xc1c3b7d1)),
        (f32::from_bits(0xc21c36fc), f32::from_bits(0xc1d62fb0)),
    ); // -36.6764f, -24.4648f, -39.0537f, -26.7733f
    path.quad_to(
        (f32::from_bits(0xc225b94c), f32::from_bits(0xc1e8a78d)),
        (f32::from_bits(0xc225eb10), f32::from_bits(0xc20194a7)),
    ); // -41.431f, -29.0818f, -41.4796f, -32.3952f
    path.quad_to(
        (f32::from_bits(0xc2261cd3), f32::from_bits(0xc20ed586)),
        (f32::from_bits(0xc21ce0e3), f32::from_bits(0xc21857d5)),
    ); // -41.5281f, -35.7085f, -39.2196f, -38.0858f
    path.quad_to(
        (f32::from_bits(0xc213a4f4), f32::from_bits(0xc221da25)),
        (f32::from_bits(0xc2066415), f32::from_bits(0xc2220be8)),
    ); // -36.9111f, -40.463f, -33.5977f, -40.5116f
    path.close();
    path.move_to((f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455))); // -19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0xc1399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc1e00000)),
    ); // -11.598f, -28, 0, -28
    path.quad_to(
        (f32::from_bits(0x41399153), f32::from_bits(0xc1e00000)),
        (f32::from_bits(0x419e6455), f32::from_bits(0xc19e6455)),
    ); // 11.598f, -28, 19.799f, -19.799f
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0x41e00000), f32::from_bits(0x00000000)),
    ); // 28, -11.598f, 28, 0
    path.quad_to(
        (f32::from_bits(0x41e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0x419e6455), f32::from_bits(0x419e6455)),
    ); // 28, 11.598f, 19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0x415b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0x40b878fc), f32::from_bits(0x41db9f74)),
    ); // 13.7163f, 25.8817f, 5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0x41000000), f32::from_bits(0x42040000)),
    ); // 8, 29.7635f, 8, 33
    path.quad_to(
        (f32::from_bits(0x41000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x40b504f3), f32::from_bits(0x421aa09e)),
    ); // 8, 36.3137f, 5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x405413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0x00000000), f32::from_bits(0x42240000)),
    ); // 3.31371f, 41, 0, 41
    path.quad_to(
        (f32::from_bits(0xc05413cd), f32::from_bits(0x42240000)),
        (f32::from_bits(0xc0b504f3), f32::from_bits(0x421aa09e)),
    ); // -3.31371f, 41, -5.65685f, 38.6569f
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0xc1000000), f32::from_bits(0x42040000)),
    ); // -8, 36.3137f, -8, 33
    path.quad_to(
        (f32::from_bits(0xc1000000), f32::from_bits(0x41ee1ba4)),
        (f32::from_bits(0xc0b878fc), f32::from_bits(0x41db9f74)),
    ); // -8, 29.7635f, -5.76477f, 27.4529f
    path.quad_to(
        (f32::from_bits(0xc15b75ce), f32::from_bits(0x41cf0dc3)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0x419e6455)),
    ); // -13.7163f, 25.8817f, -19.799f, 19.799f
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0x41399153)),
        (f32::from_bits(0xc1e00000), f32::from_bits(0x00000000)),
    ); // -28, 11.598f, -28, 0
    path.quad_to(
        (f32::from_bits(0xc1e00000), f32::from_bits(0xc1399153)),
        (f32::from_bits(0xc19e6455), f32::from_bits(0xc19e6455)),
    ); // -28, -11.598f, -19.799f, -19.799f
    path.close();
    path.move_to((f32::from_bits(0xc23c6b1a), f32::from_bits(0x4099fd84))); // -47.1046f, 4.8122f
    path.quad_to(
        (f32::from_bits(0xc22fa63c), f32::from_bits(0x407b1738)),
        (f32::from_bits(0xc2241b28), f32::from_bits(0x40b1aa10)),
    ); // -43.9123f, 3.92329f, -41.0265f, 5.55201f
    path.quad_to(
        (f32::from_bits(0xc2189014), f32::from_bits(0x40e5c886)),
        (f32::from_bits(0xc21501d7), f32::from_bits(0x4125f7c0)),
    ); // -38.1407f, 7.18073f, -37.2518f, 10.373f
    path.quad_to(
        (f32::from_bits(0xc211739a), f32::from_bits(0x41590b3e)),
        (f32::from_bits(0xc217f769), f32::from_bits(0x41839bc7)),
    ); // -36.3629f, 13.5652f, -37.9916f, 16.4511f
    path.quad_to(
        (f32::from_bits(0xc21e7b38), f32::from_bits(0x419ab1ef)),
        (f32::from_bits(0xc22b4016), f32::from_bits(0x41a1ce67)),
    ); // -39.6203f, 19.3369f, -42.8126f, 20.2258f
    path.quad_to(
        (f32::from_bits(0xc23804f6), f32::from_bits(0x41a8eae0)),
        (f32::from_bits(0xc243900a), f32::from_bits(0x419be343)),
    ); // -46.0048f, 21.1147f, -48.8907f, 19.486f
    path.quad_to(
        (f32::from_bits(0xc24f1b1e), f32::from_bits(0x418edba6)),
        (f32::from_bits(0xc252a95a), f32::from_bits(0x416aa3d0)),
    ); // -51.7765f, 17.8573f, -52.6654f, 14.665f
    path.quad_to(
        (f32::from_bits(0xc2563797), f32::from_bits(0x41379054)),
        (f32::from_bits(0xc24fb3c8), f32::from_bits(0x41096404)),
    ); // -53.5543f, 11.4727f, -51.9256f, 8.58692f
    path.quad_to(
        (f32::from_bits(0xc2492ffa), f32::from_bits(0x40b66f68)),
        (f32::from_bits(0xc23c6b1a), f32::from_bits(0x4099fd84)),
    ); // -50.2969f, 5.7011f, -47.1046f, 4.8122f
    path.close();
    path.move_to((f32::from_bits(0x4204f8ac), f32::from_bits(0x41c568cb))); // 33.2428f, 24.6762f
    path.quad_to(
        (f32::from_bits(0x421239c0), f32::from_bits(0x41c52671)),
        (f32::from_bits(0x421bb079), f32::from_bits(0x41d7b610)),
    ); // 36.5564f, 24.6438f, 38.9223f, 26.9639f
    path.quad_to(
        (f32::from_bits(0x42252732), f32::from_bits(0x41ea45ad)),
        (f32::from_bits(0x4225485f), f32::from_bits(0x420263e9)),
    ); // 41.2883f, 29.284f, 41.3207f, 32.5976f
    path.quad_to(
        (f32::from_bits(0x4225698c), f32::from_bits(0x420fa4fd)),
        (f32::from_bits(0x421c21bd), f32::from_bits(0x42191bb7)),
    ); // 41.3531f, 35.9111f, 39.0329f, 38.2771f
    path.quad_to(
        (f32::from_bits(0x421bf1ce), f32::from_bits(0x42194c98)),
        (f32::from_bits(0x421bc136), f32::from_bits(0x42197cbe)),
    ); // 38.9861f, 38.3248f, 38.9387f, 38.3718f
    path.quad_to(
        (f32::from_bits(0x421b91ce), f32::from_bits(0x4219ae03)),
        (f32::from_bits(0x421b61a7), f32::from_bits(0x4219dea3)),
    ); // 38.8924f, 38.4199f, 38.8454f, 38.4674f
    path.quad_to(
        (f32::from_bits(0x421b3158), f32::from_bits(0x421a0f6b)),
        (f32::from_bits(0x421b00a6), f32::from_bits(0x421a3f33)),
    ); // 38.7982f, 38.5151f, 38.7506f, 38.5617f
    path.quad_to(
        (f32::from_bits(0x421ad122), f32::from_bits(0x421a701a)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    ); // 38.7042f, 38.6095f, 38.6569f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    ); // 36.3137f, 41, 33, 41
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    ); // 29.6863f, 41, 27.3431f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    ); // 25, 36.3137f, 25, 33
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    ); // 25, 29.6863f, 27.3431f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x41db0360), f32::from_bits(0x41da7a26)),
        (f32::from_bits(0x41db4867), f32::from_bits(0x41da3682)),
    ); // 27.3766f, 27.3096f, 27.4104f, 27.2766f
    path.quad_to(
        (f32::from_bits(0x41db8ba6), f32::from_bits(0x41d9f135)),
        (f32::from_bits(0x41dbcfd8), f32::from_bits(0x41d9ac58)),
    ); // 27.4432f, 27.2428f, 27.4765f, 27.2092f
    path.quad_to(
        (f32::from_bits(0x41dc13ed), f32::from_bits(0x41d96798)),
        (f32::from_bits(0x41dc58ba), f32::from_bits(0x41d92383)),
    ); // 27.5097f, 27.1756f, 27.5433f, 27.1423f
    path.quad_to(
        (f32::from_bits(0x41dc9bcc), f32::from_bits(0x41d8ddb7)),
        (f32::from_bits(0x41dcdf94), f32::from_bits(0x41d89898)),
    ); // 27.5761f, 27.1083f, 27.6092f, 27.0745f
    path.quad_to(
        (f32::from_bits(0x41ef6f31), f32::from_bits(0x41c5ab25)),
        (f32::from_bits(0x4204f8ac), f32::from_bits(0x41c568cb)),
    ); // 29.9293f, 24.7086f, 33.2428f, 24.6762f
    path.close();
    let path1 = path.detach();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42240000), f32::from_bits(0x42040000))); // 41, 33
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)),
    ); // 41, 36.3137f, 38.6569f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x42240000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x42240000)),
    ); // 36.3137f, 41, 33, 41
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)),
    ); // 29.6863f, 41, 27.3431f, 38.6569f
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)),
        (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)),
    ); // 25, 36.3137f, 25, 33
    path.quad_to(
        (f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)),
    ); // 25, 29.6863f, 27.3431f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)),
    ); // 29.6863f, 25, 33, 25
    path.quad_to(
        (f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)),
        (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)),
    ); // 36.3137f, 25, 38.6569f, 27.3431f
    path.quad_to(
        (f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)),
        (f32::from_bits(0x42240000), f32::from_bits(0x42040000)),
    ); // 41, 29.6863f, 41, 33
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsFuzz763Test.cpp#L2448-L2450 (chrome/m156)
def_test!(PathOpsFuzz763, |reporter| {
    // RunTestSet(reporter, tests, testCount, firstTest, nullptr, nullptr, false): firstTest
    // runs first, then every other test in table order.
    fuzz763_1026368(reporter, "fuzz763_1026368");
    fuzz763_10022998(reporter, "fuzz763_10022998");
    fuzz763_2674194(reporter, "fuzz763_2674194");
    fuzz763_5485218(reporter, "fuzz763_5485218");
    fuzz763_3283699(reporter, "fuzz763_3283699");
    fuzz763_6411089(reporter, "fuzz763_6411089");
    fuzz763_4628016(reporter, "fuzz763_4628016");
    fuzz763_2211264(reporter, "fuzz763_2211264");
    fuzz763_34974(reporter, "fuzz763_34974");
    fuzz763_1597464(reporter, "fuzz763_1597464");
    fuzz763_849020(reporter, "fuzz763_849020");
    fuzz763_24588(reporter, "fuzz763_24588");
    fuzz763_20016(reporter, "fuzz763_20016");
    fuzz763_17370(reporter, "fuzz763_17370");
    fuzz763_35322(reporter, "fuzz763_35322");
    fuzz763_8712(reporter, "fuzz763_8712");
    fuzz763_8712a(reporter, "fuzz763_8712a");
    fuzz763_4713(reporter, "fuzz763_4713");
    fuzz763_4014(reporter, "fuzz763_4014");
    fuzz763_4014a(reporter, "fuzz763_4014a");
    fuzz763_1404(reporter, "fuzz763_1404");
    fuzz763_378(reporter, "fuzz763_378");
    fuzz763_378b(reporter, "fuzz763_378b");
    fuzz763_378d(reporter, "fuzz763_378d");
    fuzz763_378c(reporter, "fuzz763_378c");
    fuzz763_3084(reporter, "fuzz763_3084");
    fuzz763_1823(reporter, "fuzz763_1823");
    fuzz763_558(reporter, "fuzz763_558");
    fuzz763_378a(reporter, "fuzz763_378a");
    fuzz763_378a_1(reporter, "fuzz763_378a_1");
});
