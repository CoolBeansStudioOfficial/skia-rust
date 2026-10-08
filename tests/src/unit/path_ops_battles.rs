// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsBattles.cpp (chrome/m156)

// The float literals below are Skia's battle inputs, copied as written. The functions keep the
// C++ names (`battleOp1`, `issue414409`, ...). The `// crashed` / `// hung` annotations of the
// C++ source are kept as comments.
#![allow(
    non_snake_case,
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::excessive_precision,
    clippy::approx_constant,
    clippy::float_cmp
)]
#![cfg(test)]

use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_pathops::path_op::PathOp;

use crate::unit::path_ops_extended_test::test_path_op;
use crate::{Reporter, def_test};

// Port of: tests/PathOpsBattles.cpp#L20-L48 (chrome/m156)
fn issue414409(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    let mut path2 = PathBuilder::new();
    path1.move_to((9.53595e-07f32, -60.0));
    path1.line_to((5.08228e-15f32, -83.0));
    path1.cubic_to(
        (32.8673f32, -83.0),
        (62.6386f32, -63.6055f32),
        (75.9208f32, -33.5416f32),
    );
    path1.cubic_to(
        (89.2029f32, -3.47759f32),
        (83.4937f32, 31.5921f32),
        (61.3615f32, 55.8907f32),
    );
    path1.line_to((46.9383f32, 68.4529f32));
    path1.line_to((33.9313f32, 49.484f32));
    path1.cubic_to(
        (37.7451f32, 46.8689f32),
        (41.2438f32, 43.8216f32),
        (44.3577f32, 40.4029f32),
    );
    path1.line_to((44.3577f32, 40.4029f32));
    path1.cubic_to(
        (60.3569f32, 22.8376f32),
        (64.4841f32, -2.51392f32),
        (54.8825f32, -24.2469f32),
    );
    path1.cubic_to(
        (45.2809f32, -45.9799f32),
        (23.7595f32, -60.0),
        (9.53595e-07f32, -60.0),
    );
    path1.close();
    path2.move_to((46.9383f32, 68.4529f32));
    path2.cubic_to(
        (17.5117f32, 88.6307f32),
        (-21.518f32, 87.7442f32),
        (-49.9981f32, 66.251f32),
    );
    path2.cubic_to(
        (-78.4781f32, 44.7578f32),
        (-90.035f32, 7.46781f32),
        (-78.7014f32, -26.3644f32),
    );
    path2.cubic_to(
        (-67.3679f32, -60.1967f32),
        (-35.6801f32, -83.0),
        (-1.48383e-06f32, -83.0),
    );
    path2.line_to((4.22689e-14f32, -60.0));
    path2.cubic_to(
        (-25.7929f32, -60.0),
        (-48.6997f32, -43.5157f32),
        (-56.8926f32, -19.0586f32),
    );
    path2.cubic_to(
        (-65.0855f32, 5.39842f32),
        (-56.7312f32, 32.355f32),
        (-36.1432f32, 47.8923f32),
    );
    path2.cubic_to(
        (-15.5552f32, 63.4296f32),
        (12.6591f32, 64.0704f32),
        (33.9313f32, 49.484f32),
    );
    path2.line_to((46.9383f32, 68.4529f32));
    path2.close();
    test_path_op(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsBattles.cpp#L50-L71 (chrome/m156)
fn issue414409b(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    let mut path2 = PathBuilder::new();
    path1.set_fill_type(PathFillType::Winding);
    path1.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path1.cubic_to(
        (f32::from_bits(0x41f12edc), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4267b362), f32::from_bits(0xc2854e1f)),
        (f32::from_bits(0x42911faa), f32::from_bits(0xc2212f3b)),
    );
    path1.cubic_to(
        (f32::from_bits(0x42ae65a2), f32::from_bits(0xc15f08de)),
        (f32::from_bits(0x42acc913), f32::from_bits(0x41923f59)),
        (f32::from_bits(0x428ce9f0), f32::from_bits(0x422f7dc4)),
    );
    path1.line_to((f32::from_bits(0x424bbb16), f32::from_bits(0x41fdb8ed)));
    path1.cubic_to(
        (f32::from_bits(0x4279cf6e), f32::from_bits(0x41537137)),
        (f32::from_bits(0x427c23ea), f32::from_bits(0xc1213ad2)),
        (f32::from_bits(0x4251d142), f32::from_bits(0xc1e909ae)),
    );
    path1.cubic_to(
        (f32::from_bits(0x42277e9a), f32::from_bits(0xc240baf8)),
        (f32::from_bits(0x41ae5968), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path1.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path1.close();
    path2.set_fill_type(PathFillType::EvenOdd);
    path2.move_to((f32::from_bits(0x428ce9ef), f32::from_bits(0x422f7dc6)));
    path2.cubic_to(
        (f32::from_bits(0x4286af43), f32::from_bits(0x42437fa7)),
        (f32::from_bits(0x427ed0d6), f32::from_bits(0x42561f5a)),
        (f32::from_bits(0x426e69d2), f32::from_bits(0x42670c39)),
    );
    path2.line_to((f32::from_bits(0x422c58d6), f32::from_bits(0x422705c1)));
    path2.cubic_to(
        (f32::from_bits(0x42383446), f32::from_bits(0x421ac98f)),
        (f32::from_bits(0x4242b98a), f32::from_bits(0x420d5308)),
        (f32::from_bits(0x424bbb17), f32::from_bits(0x41fdb8ee)),
    );
    path2.line_to((f32::from_bits(0x428ce9ef), f32::from_bits(0x422f7dc6)));
    path2.close();
    test_path_op(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsBattles.cpp#L73-L96 (chrome/m156)
fn issue414409c(reporter: &mut Reporter, filename: &str) {
    let mut path1 = PathBuilder::new();
    let mut path2 = PathBuilder::new();
    path1.set_fill_type(PathFillType::EvenOdd);
    path1.move_to((f32::from_bits(0x36961ef0), f32::from_bits(0xc2700000)));
    path1.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path1.cubic_to(
        (f32::from_bits(0x3df86648), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3e786777), f32::from_bits(0xc2a5ffdc)),
        (f32::from_bits(0x3eba4dc2), f32::from_bits(0xc2a5ff96)),
    );
    path1.line_to((f32::from_bits(0x3eba4dc3), f32::from_bits(0xc2a5ff97)));
    path1.cubic_to(
        (f32::from_bits(0x3ec08370), f32::from_bits(0xc2a5ff8f)),
        (f32::from_bits(0x3ec6b964), f32::from_bits(0xc2a5ff88)),
        (f32::from_bits(0x3eccef58), f32::from_bits(0xc2a5ff80)),
    );
    path1.line_to((f32::from_bits(0x3e942522), f32::from_bits(0xc26fff49)));
    path1.cubic_to(
        (f32::from_bits(0x3e8fa7da), f32::from_bits(0xc26fff56)),
        (f32::from_bits(0x3e8b2acd), f32::from_bits(0xc26fff61)),
        (f32::from_bits(0x3e86adc0), f32::from_bits(0xc26fff6b)),
    );
    path1.line_to((f32::from_bits(0x3e86ad6a), f32::from_bits(0xc26fff69)));
    path1.cubic_to(
        (f32::from_bits(0x3e3391e9), f32::from_bits(0xc26fffce)),
        (f32::from_bits(0x3db3931e), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36961ef0), f32::from_bits(0xc2700000)),
    );
    path1.close();
    path2.set_fill_type(PathFillType::Winding);
    path2.move_to((f32::from_bits(0x3eccef1a), f32::from_bits(0xc2a5ff81)));
    path2.cubic_to(
        (f32::from_bits(0x3f18c8a9), f32::from_bits(0xc2a5ff04)),
        (f32::from_bits(0x3f4b19b0), f32::from_bits(0xc2a5fe2d)),
        (f32::from_bits(0x3f7d6a37), f32::from_bits(0xc2a5fcfa)),
    );
    path2.line_to((f32::from_bits(0x3f3730f2), f32::from_bits(0xc26ffba1)));
    path2.cubic_to(
        (f32::from_bits(0x3f12d1c8), f32::from_bits(0xc26ffd5d)),
        (f32::from_bits(0x3edce4b4), f32::from_bits(0xc26ffe95)),
        (f32::from_bits(0x3e942577), f32::from_bits(0xc26fff49)),
    );
    path2.line_to((f32::from_bits(0x3eccef1a), f32::from_bits(0xc2a5ff81)));
    path2.close();
    test_path_op(
        reporter,
        &path1.detach(),
        &path2.detach(),
        PathOp::Union,
        filename,
    );
}

// Port of: tests/PathOpsBattles.cpp#L99-L121 (chrome/m156)
fn battleOp1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3ea4d9f5), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3f24d9a9), f32::from_bits(0xc2a5ff0a)),
        (f32::from_bits(0x3f774519), f32::from_bits(0xc2a5fd1f)),
    );
    path.line_to((f32::from_bits(0x3f32bfc3), f32::from_bits(0xc26ffbd7)));
    path.cubic_to(
        (f32::from_bits(0x3eee5669), f32::from_bits(0xc26ffe9e)),
        (f32::from_bits(0x3e6e56cc), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffb40), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f774503), f32::from_bits(0xc2a5fd1f)));
    path.cubic_to(
        (f32::from_bits(0x3f7f82ff), f32::from_bits(0xc2a5fcee)),
        (f32::from_bits(0x3f83e06d), f32::from_bits(0xc2a5fcbb)),
        (f32::from_bits(0x3f87ff59), f32::from_bits(0xc2a5fc85)),
    );
    path.line_to((f32::from_bits(0x3f449f80), f32::from_bits(0xc26ffaf7)));
    path.cubic_to(
        (f32::from_bits(0x3f3eaa52), f32::from_bits(0xc26ffb47)),
        (f32::from_bits(0x3f38b4f5), f32::from_bits(0xc26ffb92)),
        (f32::from_bits(0x3f32bf98), f32::from_bits(0xc26ffbd9)),
    );
    path.line_to((f32::from_bits(0x3f774503), f32::from_bits(0xc2a5fd1f)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L124-L143 (chrome/m156)
fn battleOp2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3ea4d9e6), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3f24d99a), f32::from_bits(0xc2a5ff0a)),
        (f32::from_bits(0x3f774503), f32::from_bits(0xc2a5fd1f)),
    );
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f87ff64), f32::from_bits(0xc2a5fc85)));
    path.cubic_to(
        (f32::from_bits(0x3fcac720), f32::from_bits(0xc2a5f91a)),
        (f32::from_bits(0x4006c62a), f32::from_bits(0xc2a5f329)),
        (f32::from_bits(0x40282667), f32::from_bits(0xc2a5eab4)),
    );
    path.line_to((f32::from_bits(0x3ff31bb9), f32::from_bits(0xc26fe136)));
    path.cubic_to(
        (f32::from_bits(0x3fc2da88), f32::from_bits(0xc26fed71)),
        (f32::from_bits(0x3f9295ff), f32::from_bits(0xc26ff607)),
        (f32::from_bits(0x3f449f66), f32::from_bits(0xc26ffaf9)),
    );
    path.line_to((f32::from_bits(0x3f87ff64), f32::from_bits(0xc2a5fc85)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L146-L168 (chrome/m156)
fn battleOp3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f19f03c), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3f99ef95), f32::from_bits(0xc2a5fca7)),
        (f32::from_bits(0x3fe6e2fa), f32::from_bits(0xc2a5f5f7)),
    );
    path.line_to((f32::from_bits(0x3fa6e80c), f32::from_bits(0xc26ff17d)));
    path.cubic_to(
        (f32::from_bits(0x3f5e8ed4), f32::from_bits(0xc26ffb2a)),
        (f32::from_bits(0x3ede8fc6), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x35d9fd64), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3fe6e322), f32::from_bits(0xc2a5f5f7)));
    path.cubic_to(
        (f32::from_bits(0x3fee94fb), f32::from_bits(0xc2a5f54c)),
        (f32::from_bits(0x3ff646db), f32::from_bits(0xc2a5f497)),
        (f32::from_bits(0x3ffdf8ad), f32::from_bits(0xc2a5f3db)),
    );
    path.line_to((f32::from_bits(0x3fb79813), f32::from_bits(0xc26fee71)));
    path.cubic_to(
        (f32::from_bits(0x3fb20800), f32::from_bits(0xc26fef82)),
        (f32::from_bits(0x3fac77ff), f32::from_bits(0xc26ff085)),
        (f32::from_bits(0x3fa6e7f4), f32::from_bits(0xc26ff17d)),
    );
    path.line_to((f32::from_bits(0x3fe6e322), f32::from_bits(0xc2a5f5f7)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L171-L196 (chrome/m156)
fn battleOp4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f19f03c), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3f99ef95), f32::from_bits(0xc2a5fca7)),
        (f32::from_bits(0x3fe6e322), f32::from_bits(0xc2a5f5f7)),
    );
    path.cubic_to(
        (f32::from_bits(0x3fee94fb), f32::from_bits(0xc2a5f54c)),
        (f32::from_bits(0x3ff646db), f32::from_bits(0xc2a5f497)),
        (f32::from_bits(0x3ffdf8ad), f32::from_bits(0xc2a5f3db)),
    );
    path.line_to((f32::from_bits(0x3fb79813), f32::from_bits(0xc26fee71)));
    path.cubic_to(
        (f32::from_bits(0x3fb20808), f32::from_bits(0xc26fef82)),
        (f32::from_bits(0x3fac780f), f32::from_bits(0xc26ff085)),
        (f32::from_bits(0x3fa6e80c), f32::from_bits(0xc26ff17d)),
    );
    path.line_to((f32::from_bits(0x3fa6e7f4), f32::from_bits(0xc26ff17d)));
    path.cubic_to(
        (f32::from_bits(0x3f5e8eb4), f32::from_bits(0xc26ffb2a)),
        (f32::from_bits(0x3ede8fa6), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ffdf8c6), f32::from_bits(0xc2a5f3db)));
    path.cubic_to(
        (f32::from_bits(0x403d5556), f32::from_bits(0xc2a5e7ed)),
        (f32::from_bits(0x407ba65a), f32::from_bits(0xc2a5d338)),
        (f32::from_bits(0x409cf3fe), f32::from_bits(0xc2a5b5bc)),
    );
    path.line_to((f32::from_bits(0x4062eb8a), f32::from_bits(0xc26f94a1)));
    path.cubic_to(
        (f32::from_bits(0x4035ea63), f32::from_bits(0xc26fbf44)),
        (f32::from_bits(0x4008de16), f32::from_bits(0xc26fdd35)),
        (f32::from_bits(0x3fb79810), f32::from_bits(0xc26fee74)),
    );
    path.line_to((f32::from_bits(0x3ffdf8c6), f32::from_bits(0xc2a5f3db)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L199-L221 (chrome/m156)
fn battleOp5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3fe06a9b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40606368), f32::from_bits(0xc2a5e38e)),
        (f32::from_bits(0x40a82f8a), f32::from_bits(0xc2a5aab6)),
    );
    path.line_to((f32::from_bits(0x40732902), f32::from_bits(0xc26f84b2)));
    path.cubic_to(
        (f32::from_bits(0x4022355b), f32::from_bits(0xc26fd6e1)),
        (f32::from_bits(0x3fa23a8f), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb5600574), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x40a82f91), f32::from_bits(0xc2a5aab7)));
    path.cubic_to(
        (f32::from_bits(0x40adc8dc), f32::from_bits(0xc2a5a508)),
        (f32::from_bits(0x40b361d8), f32::from_bits(0xc2a59f10)),
        (f32::from_bits(0x40b8fa82), f32::from_bits(0xc2a598d0)),
    );
    path.line_to((f32::from_bits(0x4085b825), f32::from_bits(0xc26f6ad0)));
    path.cubic_to(
        (f32::from_bits(0x4081ac7b), f32::from_bits(0xc26f73dc)),
        (f32::from_bits(0x407b412c), f32::from_bits(0xc26f7c7c)),
        (f32::from_bits(0x407328f8), f32::from_bits(0xc26f84b3)),
    );
    path.line_to((f32::from_bits(0x40a82f91), f32::from_bits(0xc2a5aab7)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L224-L253 (chrome/m156)
fn battleOp6(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3fe06a9b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40606368), f32::from_bits(0xc2a5e38e)),
        (f32::from_bits(0x40a82f91), f32::from_bits(0xc2a5aab7)),
    );
    path.cubic_to(
        (f32::from_bits(0x40adc8dc), f32::from_bits(0xc2a5a508)),
        (f32::from_bits(0x40b361d8), f32::from_bits(0xc2a59f10)),
        (f32::from_bits(0x40b8fa82), f32::from_bits(0xc2a598d0)),
    );
    path.line_to((f32::from_bits(0x4085b825), f32::from_bits(0xc26f6ad0)));
    path.cubic_to(
        (f32::from_bits(0x4081ac7d), f32::from_bits(0xc26f73dc)),
        (f32::from_bits(0x407b4133), f32::from_bits(0xc26f7c7c)),
        (f32::from_bits(0x40732902), f32::from_bits(0xc26f84b2)),
    );
    path.cubic_to(
        (f32::from_bits(0x4022355b), f32::from_bits(0xc26fd6e1)),
        (f32::from_bits(0x3fa23a8f), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    path.move_to((f32::from_bits(0x408fea52), f32::from_bits(0xc28dc28a)));
    path.line_to((f32::from_bits(0x407328f8), f32::from_bits(0xc26f84b3)));
    path.line_to((f32::from_bits(0x40732903), f32::from_bits(0xc26f84b3)));
    path.line_to((f32::from_bits(0x408fea52), f32::from_bits(0xc28dc28a)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x40b8fa77), f32::from_bits(0xc2a598d0)));
    path.cubic_to(
        (f32::from_bits(0x4109d7e9), f32::from_bits(0xc2a5337c)),
        (f32::from_bits(0x4137014a), f32::from_bits(0xc2a483b2)),
        (f32::from_bits(0x4163cbb6), f32::from_bits(0xc2a38a24)),
    );
    path.line_to((f32::from_bits(0x4124abf0), f32::from_bits(0xc26c715c)));
    path.cubic_to(
        (f32::from_bits(0x41044af8), f32::from_bits(0xc26dda2b)),
        (f32::from_bits(0x40c74ab0), f32::from_bits(0xc26ed852)),
        (f32::from_bits(0x4085b82e), f32::from_bits(0xc26f6ad1)),
    );
    path.line_to((f32::from_bits(0x40b8fa77), f32::from_bits(0xc2a598d0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L255-L277 (chrome/m156)
fn battleOp7(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3de5c884), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3e65c882), f32::from_bits(0xc2a5ffe2)),
        (f32::from_bits(0x3eac5645), f32::from_bits(0xc2a5ffa7)),
    );
    path.line_to((f32::from_bits(0x3e79297e), f32::from_bits(0xc26fff7f)));
    path.cubic_to(
        (f32::from_bits(0x3e261bbd), f32::from_bits(0xc26fffd7)),
        (f32::from_bits(0x3da61bbf), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb3244c00), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3eac564d), f32::from_bits(0xc2a5ffa7)));
    path.cubic_to(
        (f32::from_bits(0x3eb21458), f32::from_bits(0xc2a5ffa1)),
        (f32::from_bits(0x3eb7d2fc), f32::from_bits(0xc2a5ff9b)),
        (f32::from_bits(0x3ebd91a0), f32::from_bits(0xc2a5ff94)),
    );
    path.line_to((f32::from_bits(0x3e8909ff), f32::from_bits(0xc26fff64)));
    path.cubic_to(
        (f32::from_bits(0x3e84e2cf), f32::from_bits(0xc26fff6d)),
        (f32::from_bits(0x3e80bc02), f32::from_bits(0xc26fff76)),
        (f32::from_bits(0x3e792a69), f32::from_bits(0xc26fff7f)),
    );
    path.line_to((f32::from_bits(0x3eac564d), f32::from_bits(0xc2a5ffa7)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L280-L304 (chrome/m156)
fn battleOp8(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3de5c884), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3e65c882), f32::from_bits(0xc2a5ffe2)),
        (f32::from_bits(0x3eac564d), f32::from_bits(0xc2a5ffa7)),
    );
    path.cubic_to(
        (f32::from_bits(0x3eb21458), f32::from_bits(0xc2a5ffa1)),
        (f32::from_bits(0x3eb7d2fc), f32::from_bits(0xc2a5ff9b)),
        (f32::from_bits(0x3ebd91a0), f32::from_bits(0xc2a5ff94)),
    );
    path.line_to((f32::from_bits(0x3e8909ff), f32::from_bits(0xc26fff64)));
    path.line_to((f32::from_bits(0x3e792a69), f32::from_bits(0xc26fff7f)));
    path.cubic_to(
        (f32::from_bits(0x3e261bbd), f32::from_bits(0xc26fffd7)),
        (f32::from_bits(0x3da61bbf), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ebd921a), f32::from_bits(0xc2a5ff94)));
    path.cubic_to(
        (f32::from_bits(0x3f0d545f), f32::from_bits(0xc2a5ff29)),
        (f32::from_bits(0x3f3bdfbd), f32::from_bits(0xc2a5fe71)),
        (f32::from_bits(0x3f6a6ab6), f32::from_bits(0xc2a5fd69)),
    );
    path.line_to((f32::from_bits(0x3f297558), f32::from_bits(0xc26ffc43)));
    path.cubic_to(
        (f32::from_bits(0x3f07d00d), f32::from_bits(0xc26ffdc0)),
        (f32::from_bits(0x3ecc550f), f32::from_bits(0xc26ffecc)),
        (f32::from_bits(0x3e8909b7), f32::from_bits(0xc26fff65)),
    );
    path.line_to((f32::from_bits(0x3ebd921a), f32::from_bits(0xc2a5ff94)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L307-L329 (chrome/m156)
fn battleOp9(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3ecc43bf), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3f4c4385), f32::from_bits(0xc2a5fe87)),
        (f32::from_bits(0x3f993163), f32::from_bits(0xc2a5fb95)),
    );
    path.line_to((f32::from_bits(0x3f5d7bc4), f32::from_bits(0xc26ff99d)));
    path.cubic_to(
        (f32::from_bits(0x3f13a919), f32::from_bits(0xc26ffdde)),
        (f32::from_bits(0x3e93a998), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x367b7ed0), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f993156), f32::from_bits(0xc2a5fb95)));
    path.cubic_to(
        (f32::from_bits(0x3f9e4c7a), f32::from_bits(0xc2a5fb49)),
        (f32::from_bits(0x3fa36794), f32::from_bits(0xc2a5fafa)),
        (f32::from_bits(0x3fa882aa), f32::from_bits(0xc2a5faa7)),
    );
    path.line_to((f32::from_bits(0x3f73a149), f32::from_bits(0xc26ff845)));
    path.cubic_to(
        (f32::from_bits(0x3f6c3f64), f32::from_bits(0xc26ff8bf)),
        (f32::from_bits(0x3f64dd9d), f32::from_bits(0xc26ff931)),
        (f32::from_bits(0x3f5d7bcf), f32::from_bits(0xc26ff99f)),
    );
    path.line_to((f32::from_bits(0x3f993156), f32::from_bits(0xc2a5fb95)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L331-L353 (chrome/m156)
fn battleOp10(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3ddcd524), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3e5cd462), f32::from_bits(0xc2a5ffe3)),
        (f32::from_bits(0x3ea59eff), f32::from_bits(0xc2a5ffac)),
    );
    path.line_to((f32::from_bits(0x3e6f74a3), f32::from_bits(0xc26fff89)));
    path.cubic_to(
        (f32::from_bits(0x3e1fa33e), f32::from_bits(0xc26fffd9)),
        (f32::from_bits(0x3d9fa303), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb580e440), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ea59f9c), f32::from_bits(0xc2a5ffad)));
    path.cubic_to(
        (f32::from_bits(0x3eab24c0), f32::from_bits(0xc2a5ffa7)),
        (f32::from_bits(0x3eb0aa54), f32::from_bits(0xc2a5ffa1)),
        (f32::from_bits(0x3eb62fe9), f32::from_bits(0xc2a5ff9b)),
    );
    path.line_to((f32::from_bits(0x3e83b355), f32::from_bits(0xc26fff6f)));
    path.cubic_to(
        (f32::from_bits(0x3e7f6bdb), f32::from_bits(0xc26fff79)),
        (f32::from_bits(0x3e777021), f32::from_bits(0xc26fff81)),
        (f32::from_bits(0x3e6f7465), f32::from_bits(0xc26fff8a)),
    );
    path.line_to((f32::from_bits(0x3ea59f9c), f32::from_bits(0xc2a5ffad)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L356-L385 (chrome/m156)
fn battleOp11(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3ddcd524), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3e5cd462), f32::from_bits(0xc2a5ffe3)),
        (f32::from_bits(0x3ea59f9c), f32::from_bits(0xc2a5ffad)),
    );
    path.line_to((f32::from_bits(0x3eb62fe9), f32::from_bits(0xc2a5ff9b)));
    path.line_to((f32::from_bits(0x3e83b355), f32::from_bits(0xc26fff6f)));
    path.cubic_to(
        (f32::from_bits(0x3e7f6bf0), f32::from_bits(0xc26fff79)),
        (f32::from_bits(0x3e77704b), f32::from_bits(0xc26fff81)),
        (f32::from_bits(0x3e6f74a3), f32::from_bits(0xc26fff89)),
    );
    path.cubic_to(
        (f32::from_bits(0x3e1fa33e), f32::from_bits(0xc26fffd9)),
        (f32::from_bits(0x3d9fa303), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    path.move_to((f32::from_bits(0x3e7ee007), f32::from_bits(0xc27f7413)));
    path.line_to((f32::from_bits(0x3e6f7465), f32::from_bits(0xc26fff8a)));
    path.line_to((f32::from_bits(0x3e6f74a4), f32::from_bits(0xc26fff8a)));
    path.line_to((f32::from_bits(0x3e7ee007), f32::from_bits(0xc27f7413)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3eb62f8c), f32::from_bits(0xc2a5ff9c)));
    path.cubic_to(
        (f32::from_bits(0x3f07d31d), f32::from_bits(0xc2a5ff3a)),
        (f32::from_bits(0x3f348e3e), f32::from_bits(0xc2a5fe8f)),
        (f32::from_bits(0x3f614904), f32::from_bits(0xc2a5fd9c)),
    );
    path.line_to((f32::from_bits(0x3f22db6c), f32::from_bits(0xc26ffc8c)));
    path.cubic_to(
        (f32::from_bits(0x3f0285bf), f32::from_bits(0xc26ffdeb)),
        (f32::from_bits(0x3ec45fa5), f32::from_bits(0xc26ffee1)),
        (f32::from_bits(0x3e83b387), f32::from_bits(0xc26fff6f)),
    );
    path.line_to((f32::from_bits(0x3eb62f8c), f32::from_bits(0xc2a5ff9c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L388-L410 (chrome/m156)
fn battleOp12(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3ecc43bf), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3f4c4385), f32::from_bits(0xc2a5fe87)),
        (f32::from_bits(0x3f993163), f32::from_bits(0xc2a5fb95)),
    );
    path.line_to((f32::from_bits(0x3f5d7bc4), f32::from_bits(0xc26ff99d)));
    path.cubic_to(
        (f32::from_bits(0x3f13a919), f32::from_bits(0xc26ffdde)),
        (f32::from_bits(0x3e93a998), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x367b7ed0), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f993156), f32::from_bits(0xc2a5fb95)));
    path.cubic_to(
        (f32::from_bits(0x3f9e4c7a), f32::from_bits(0xc2a5fb49)),
        (f32::from_bits(0x3fa36794), f32::from_bits(0xc2a5fafa)),
        (f32::from_bits(0x3fa882aa), f32::from_bits(0xc2a5faa7)),
    );
    path.line_to((f32::from_bits(0x3f73a149), f32::from_bits(0xc26ff845)));
    path.cubic_to(
        (f32::from_bits(0x3f6c3f64), f32::from_bits(0xc26ff8bf)),
        (f32::from_bits(0x3f64dd9d), f32::from_bits(0xc26ff931)),
        (f32::from_bits(0x3f5d7bcf), f32::from_bits(0xc26ff99f)),
    );
    path.line_to((f32::from_bits(0x3f993156), f32::from_bits(0xc2a5fb95)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L413-L435 (chrome/m156)
fn battleOp13(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3ddcd524), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3e5cd462), f32::from_bits(0xc2a5ffe3)),
        (f32::from_bits(0x3ea59eff), f32::from_bits(0xc2a5ffac)),
    );
    path.line_to((f32::from_bits(0x3e6f74a3), f32::from_bits(0xc26fff89)));
    path.cubic_to(
        (f32::from_bits(0x3e1fa33e), f32::from_bits(0xc26fffd9)),
        (f32::from_bits(0x3d9fa303), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb580e440), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ea59f9c), f32::from_bits(0xc2a5ffad)));
    path.cubic_to(
        (f32::from_bits(0x3eab24c0), f32::from_bits(0xc2a5ffa7)),
        (f32::from_bits(0x3eb0aa54), f32::from_bits(0xc2a5ffa1)),
        (f32::from_bits(0x3eb62fe9), f32::from_bits(0xc2a5ff9b)),
    );
    path.line_to((f32::from_bits(0x3e83b355), f32::from_bits(0xc26fff6f)));
    path.cubic_to(
        (f32::from_bits(0x3e7f6bdb), f32::from_bits(0xc26fff79)),
        (f32::from_bits(0x3e777021), f32::from_bits(0xc26fff81)),
        (f32::from_bits(0x3e6f7465), f32::from_bits(0xc26fff8a)),
    );
    path.line_to((f32::from_bits(0x3ea59f9c), f32::from_bits(0xc2a5ffad)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L438-L467 (chrome/m156)
fn battleOp14(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3ddcd524), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3e5cd462), f32::from_bits(0xc2a5ffe3)),
        (f32::from_bits(0x3ea59f9c), f32::from_bits(0xc2a5ffad)),
    );
    path.line_to((f32::from_bits(0x3eb62fe9), f32::from_bits(0xc2a5ff9b)));
    path.line_to((f32::from_bits(0x3e83b355), f32::from_bits(0xc26fff6f)));
    path.cubic_to(
        (f32::from_bits(0x3e7f6bf0), f32::from_bits(0xc26fff79)),
        (f32::from_bits(0x3e77704b), f32::from_bits(0xc26fff81)),
        (f32::from_bits(0x3e6f74a3), f32::from_bits(0xc26fff89)),
    );
    path.cubic_to(
        (f32::from_bits(0x3e1fa33e), f32::from_bits(0xc26fffd9)),
        (f32::from_bits(0x3d9fa303), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    path.move_to((f32::from_bits(0x3e7ee007), f32::from_bits(0xc27f7413)));
    path.line_to((f32::from_bits(0x3e6f7465), f32::from_bits(0xc26fff8a)));
    path.line_to((f32::from_bits(0x3e6f74a4), f32::from_bits(0xc26fff8a)));
    path.line_to((f32::from_bits(0x3e7ee007), f32::from_bits(0xc27f7413)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3eb62f8c), f32::from_bits(0xc2a5ff9c)));
    path.cubic_to(
        (f32::from_bits(0x3f07d31d), f32::from_bits(0xc2a5ff3a)),
        (f32::from_bits(0x3f348e3e), f32::from_bits(0xc2a5fe8f)),
        (f32::from_bits(0x3f614904), f32::from_bits(0xc2a5fd9c)),
    );
    path.line_to((f32::from_bits(0x3f22db6c), f32::from_bits(0xc26ffc8c)));
    path.cubic_to(
        (f32::from_bits(0x3f0285bf), f32::from_bits(0xc26ffdeb)),
        (f32::from_bits(0x3ec45fa5), f32::from_bits(0xc26ffee1)),
        (f32::from_bits(0x3e83b387), f32::from_bits(0xc26fff6f)),
    );
    path.line_to((f32::from_bits(0x3eb62f8c), f32::from_bits(0xc2a5ff9c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L470-L492 (chrome/m156)
fn battleOp15(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f19f03c), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3f99ef95), f32::from_bits(0xc2a5fca7)),
        (f32::from_bits(0x3fe6e2fa), f32::from_bits(0xc2a5f5f7)),
    );
    path.line_to((f32::from_bits(0x3fa6e80c), f32::from_bits(0xc26ff17d)));
    path.cubic_to(
        (f32::from_bits(0x3f5e8ed4), f32::from_bits(0xc26ffb2a)),
        (f32::from_bits(0x3ede8fc6), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x35d9fd64), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3fe6e322), f32::from_bits(0xc2a5f5f7)));
    path.cubic_to(
        (f32::from_bits(0x3fee94fb), f32::from_bits(0xc2a5f54c)),
        (f32::from_bits(0x3ff646db), f32::from_bits(0xc2a5f497)),
        (f32::from_bits(0x3ffdf8ad), f32::from_bits(0xc2a5f3db)),
    );
    path.line_to((f32::from_bits(0x3fb79813), f32::from_bits(0xc26fee71)));
    path.cubic_to(
        (f32::from_bits(0x3fb20800), f32::from_bits(0xc26fef82)),
        (f32::from_bits(0x3fac77ff), f32::from_bits(0xc26ff085)),
        (f32::from_bits(0x3fa6e7f4), f32::from_bits(0xc26ff17d)),
    );
    path.line_to((f32::from_bits(0x3fe6e322), f32::from_bits(0xc2a5f5f7)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L495-L520 (chrome/m156)
fn battleOp16(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f19f03c), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3f99ef95), f32::from_bits(0xc2a5fca7)),
        (f32::from_bits(0x3fe6e322), f32::from_bits(0xc2a5f5f7)),
    );
    path.cubic_to(
        (f32::from_bits(0x3fee94fb), f32::from_bits(0xc2a5f54c)),
        (f32::from_bits(0x3ff646db), f32::from_bits(0xc2a5f497)),
        (f32::from_bits(0x3ffdf8ad), f32::from_bits(0xc2a5f3db)),
    );
    path.line_to((f32::from_bits(0x3fb79813), f32::from_bits(0xc26fee71)));
    path.cubic_to(
        (f32::from_bits(0x3fb20808), f32::from_bits(0xc26fef82)),
        (f32::from_bits(0x3fac780f), f32::from_bits(0xc26ff085)),
        (f32::from_bits(0x3fa6e80c), f32::from_bits(0xc26ff17d)),
    );
    path.line_to((f32::from_bits(0x3fa6e7f4), f32::from_bits(0xc26ff17d)));
    path.cubic_to(
        (f32::from_bits(0x3f5e8eb4), f32::from_bits(0xc26ffb2a)),
        (f32::from_bits(0x3ede8fa6), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ffdf8c6), f32::from_bits(0xc2a5f3db)));
    path.cubic_to(
        (f32::from_bits(0x403d5556), f32::from_bits(0xc2a5e7ed)),
        (f32::from_bits(0x407ba65a), f32::from_bits(0xc2a5d338)),
        (f32::from_bits(0x409cf3fe), f32::from_bits(0xc2a5b5bc)),
    );
    path.line_to((f32::from_bits(0x4062eb8a), f32::from_bits(0xc26f94a1)));
    path.cubic_to(
        (f32::from_bits(0x4035ea63), f32::from_bits(0xc26fbf44)),
        (f32::from_bits(0x4008de16), f32::from_bits(0xc26fdd35)),
        (f32::from_bits(0x3fb79810), f32::from_bits(0xc26fee74)),
    );
    path.line_to((f32::from_bits(0x3ffdf8c6), f32::from_bits(0xc2a5f3db)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L523-L545 (chrome/m156)
fn battleOp17(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f9860dc), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40185ea2), f32::from_bits(0xc2a5f2e2)),
        (f32::from_bits(0x40647d09), f32::from_bits(0xc2a5d8aa)),
    );
    path.line_to((f32::from_bits(0x40252c2a), f32::from_bits(0xc26fc723)));
    path.cubic_to(
        (f32::from_bits(0x3fdc4b47), f32::from_bits(0xc26fed09)),
        (f32::from_bits(0x3f5c4ea6), f32::from_bits(0xc26ffffe)),
        (f32::from_bits(0x3664fea3), f32::from_bits(0xc26ffffe)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x40647d17), f32::from_bits(0xc2a5d8ab)));
    path.cubic_to(
        (f32::from_bits(0x406c19ae), f32::from_bits(0xc2a5d60b)),
        (f32::from_bits(0x4073b608), f32::from_bits(0xc2a5d34a)),
        (f32::from_bits(0x407b5230), f32::from_bits(0xc2a5d069)),
    );
    path.line_to((f32::from_bits(0x4035ad90), f32::from_bits(0xc26fbb32)));
    path.cubic_to(
        (f32::from_bits(0x40302d3b), f32::from_bits(0xc26fbf5d)),
        (f32::from_bits(0x402aacbf), f32::from_bits(0xc26fc358)),
        (f32::from_bits(0x40252c21), f32::from_bits(0xc26fc722)),
    );
    path.line_to((f32::from_bits(0x40647d17), f32::from_bits(0xc2a5d8ab)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L548-L572 (chrome/m156)
fn battleOp18(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3664fea3), f32::from_bits(0xc26ffffe)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f9860dc), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40185ea2), f32::from_bits(0xc2a5f2e2)),
        (f32::from_bits(0x40647d17), f32::from_bits(0xc2a5d8ab)),
    );
    path.cubic_to(
        (f32::from_bits(0x406c19ae), f32::from_bits(0xc2a5d60b)),
        (f32::from_bits(0x4073b608), f32::from_bits(0xc2a5d34a)),
        (f32::from_bits(0x407b5230), f32::from_bits(0xc2a5d069)),
    );
    path.line_to((f32::from_bits(0x4035ad90), f32::from_bits(0xc26fbb32)));
    path.cubic_to(
        (f32::from_bits(0x40302d3b), f32::from_bits(0xc26fbf5d)),
        (f32::from_bits(0x402aacbf), f32::from_bits(0xc26fc358)),
        (f32::from_bits(0x40252c2a), f32::from_bits(0xc26fc723)),
    );
    path.cubic_to(
        (f32::from_bits(0x3fdc4b47), f32::from_bits(0xc26fed09)),
        (f32::from_bits(0x3f5c4ea6), f32::from_bits(0xc26ffffe)),
        (f32::from_bits(0x3664fea3), f32::from_bits(0xc26ffffe)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x407b523a), f32::from_bits(0xc2a5d069)));
    path.cubic_to(
        (f32::from_bits(0x40bb53e8), f32::from_bits(0xc2a5a1ad)),
        (f32::from_bits(0x40f8dfd1), f32::from_bits(0xc2a5508e)),
        (f32::from_bits(0x411b1813), f32::from_bits(0xc2a4dd32)),
    );
    path.line_to((f32::from_bits(0x40e03b7c), f32::from_bits(0xc26e5b8f)));
    path.cubic_to(
        (f32::from_bits(0x40b3e8bb), f32::from_bits(0xc26f0259)),
        (f32::from_bits(0x40876aeb), f32::from_bits(0xc26f77a1)),
        (f32::from_bits(0x4035ad92), f32::from_bits(0xc26fbb33)),
    );
    path.line_to((f32::from_bits(0x407b523a), f32::from_bits(0xc2a5d069)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L575-L597 (chrome/m156)
fn battleOp19(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40272e66), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40a7227d), f32::from_bits(0xc2a5c0db)),
        (f32::from_bits(0x40fa5a70), f32::from_bits(0xc2a542ca)),
    );
    path.line_to((f32::from_bits(0x40b4fa6e), f32::from_bits(0xc26eee73)));
    path.cubic_to(
        (f32::from_bits(0x4071a3f5), f32::from_bits(0xc26fa4b8)),
        (f32::from_bits(0x3ff1b53c), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x359dfd46), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x40fa5a6d), f32::from_bits(0xc2a542cb)));
    path.cubic_to(
        (f32::from_bits(0x4101563b), f32::from_bits(0xc2a5362f)),
        (f32::from_bits(0x41057ec0), f32::from_bits(0xc2a528f4)),
        (f32::from_bits(0x4109a6c0), f32::from_bits(0xc2a51b18)),
    );
    path.line_to((f32::from_bits(0x40c70391), f32::from_bits(0xc26eb50e)));
    path.cubic_to(
        (f32::from_bits(0x40c10142), f32::from_bits(0xc26ec918)),
        (f32::from_bits(0x40bafe32), f32::from_bits(0xc26edc3a)),
        (f32::from_bits(0x40b4fa70), f32::from_bits(0xc26eee73)),
    );
    path.line_to((f32::from_bits(0x40fa5a6d), f32::from_bits(0xc2a542cb)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L600-L625 (chrome/m156)
fn battleOp20(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40272e63), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x40a7227a), f32::from_bits(0xc2a5c0db)),
        (f32::from_bits(0x40fa5a6c), f32::from_bits(0xc2a542ca)),
    );
    path.line_to((f32::from_bits(0x40fa5a6d), f32::from_bits(0xc2a542cb)));
    path.cubic_to(
        (f32::from_bits(0x4101563b), f32::from_bits(0xc2a5362f)),
        (f32::from_bits(0x41057ec0), f32::from_bits(0xc2a528f4)),
        (f32::from_bits(0x4109a6c0), f32::from_bits(0xc2a51b18)),
    );
    path.line_to((f32::from_bits(0x40c70391), f32::from_bits(0xc26eb50e)));
    path.cubic_to(
        (f32::from_bits(0x40c10142), f32::from_bits(0xc26ec918)),
        (f32::from_bits(0x40bafe32), f32::from_bits(0xc26edc3a)),
        (f32::from_bits(0x40b4fa6e), f32::from_bits(0xc26eee73)),
    );
    path.cubic_to(
        (f32::from_bits(0x4071a3f5), f32::from_bits(0xc26fa4b8)),
        (f32::from_bits(0x3ff1b53c), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4109a6bc), f32::from_bits(0xc2a51b19)));
    path.cubic_to(
        (f32::from_bits(0x414d093d), f32::from_bits(0xc2a43a61)),
        (f32::from_bits(0x4187e474), f32::from_bits(0xc2a2b4fa)),
        (f32::from_bits(0x41a8a805), f32::from_bits(0xc2a08e4d)),
    );
    path.line_to((f32::from_bits(0x4173d72c), f32::from_bits(0xc2682105)));
    path.cubic_to(
        (f32::from_bits(0x41447890), f32::from_bits(0xc26b3d2d)),
        (f32::from_bits(0x4114380c), f32::from_bits(0xc26d702b)),
        (f32::from_bits(0x40c70392), f32::from_bits(0xc26eb510)),
    );
    path.line_to((f32::from_bits(0x4109a6bc), f32::from_bits(0xc2a51b19)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L628-L650 (chrome/m156)
fn battleOp21(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x404ef9c5), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40cee321), f32::from_bits(0xc2a59f3a)),
        (f32::from_bits(0x411ad5ab), f32::from_bits(0xc2a4de2c)),
    );
    path.line_to((f32::from_bits(0x40dfdb77), f32::from_bits(0xc26e5cf8)));
    path.cubic_to(
        (f32::from_bits(0x40958e99), f32::from_bits(0xc26f7414)),
        (f32::from_bits(0x40159f04), f32::from_bits(0xc26ffffe)),
        (f32::from_bits(0x36ae7f52), f32::from_bits(0xc26ffffe)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x411ad5aa), f32::from_bits(0xc2a4de2c)));
    path.cubic_to(
        (f32::from_bits(0x411ff8ea), f32::from_bits(0xc2a4cadf)),
        (f32::from_bits(0x41251b3e), f32::from_bits(0xc2a4b69c)),
        (f32::from_bits(0x412a3c98), f32::from_bits(0xc2a4a163)),
    );
    path.line_to((f32::from_bits(0x40f6200f), f32::from_bits(0xc26e0518)));
    path.cubic_to(
        (f32::from_bits(0x40eeb53e), f32::from_bits(0xc26e23c6)),
        (f32::from_bits(0x40e74902), f32::from_bits(0xc26e4112)),
        (f32::from_bits(0x40dfdb73), f32::from_bits(0xc26e5cf8)),
    );
    path.line_to((f32::from_bits(0x411ad5aa), f32::from_bits(0xc2a4de2c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L653-L675 (chrome/m156)
fn battleOp22(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x407fb41a), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40ff895b), f32::from_bits(0xc2a56c4b)),
        (f32::from_bits(0x413f077c), f32::from_bits(0xc2a44609)),
    );
    path.line_to((f32::from_bits(0x410a17ee), f32::from_bits(0xc26d8104)));
    path.cubic_to(
        (f32::from_bits(0x40b8b9ab), f32::from_bits(0xc26f2a74)),
        (f32::from_bits(0x4038d88b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x337fa8c0), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x413f0780), f32::from_bits(0xc2a44609)));
    path.cubic_to(
        (f32::from_bits(0x41455a4a), f32::from_bits(0xc2a4289f)),
        (f32::from_bits(0x414bab5a), f32::from_bits(0xc2a409bf)),
        (f32::from_bits(0x4151fa92), f32::from_bits(0xc2a3e96b)),
    );
    path.line_to((f32::from_bits(0x4117cabb), f32::from_bits(0xc26cfb1d)));
    path.cubic_to(
        (f32::from_bits(0x41133b1d), f32::from_bits(0xc26d29dc)),
        (f32::from_bits(0x410eaa27), f32::from_bits(0xc26d567f)),
        (f32::from_bits(0x410a17f1), f32::from_bits(0xc26d8105)),
    );
    path.line_to((f32::from_bits(0x413f0780), f32::from_bits(0xc2a44609)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L678-L702 (chrome/m156)
fn battleOp23(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x407fb41a), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40ff895b), f32::from_bits(0xc2a56c4b)),
        (f32::from_bits(0x413f0780), f32::from_bits(0xc2a44609)),
    );
    path.cubic_to(
        (f32::from_bits(0x41455a4a), f32::from_bits(0xc2a4289f)),
        (f32::from_bits(0x414bab5a), f32::from_bits(0xc2a409bf)),
        (f32::from_bits(0x4151fa92), f32::from_bits(0xc2a3e96b)),
    );
    path.line_to((f32::from_bits(0x4117cabb), f32::from_bits(0xc26cfb1d)));
    path.cubic_to(
        (f32::from_bits(0x41133b1d), f32::from_bits(0xc26d29dc)),
        (f32::from_bits(0x410eaa27), f32::from_bits(0xc26d567f)),
        (f32::from_bits(0x410a17ee), f32::from_bits(0xc26d8104)),
    );
    path.cubic_to(
        (f32::from_bits(0x40b8b9ab), f32::from_bits(0xc26f2a74)),
        (f32::from_bits(0x4038d88b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4151fa93), f32::from_bits(0xc2a3e96b)));
    path.cubic_to(
        (f32::from_bits(0x419c2b7d), f32::from_bits(0xc2a1dce5)),
        (f32::from_bits(0x41ce36f8), f32::from_bits(0xc29e52a6)),
        (f32::from_bits(0x41fe1a0a), f32::from_bits(0xc2995d2e)),
    );
    path.line_to((f32::from_bits(0x41b7b024), f32::from_bits(0xc25dbb29)));
    path.cubic_to(
        (f32::from_bits(0x41951228), f32::from_bits(0xc264e68b)),
        (f32::from_bits(0x4161c9b2), f32::from_bits(0xc26a04c8)),
        (f32::from_bits(0x4117cabf), f32::from_bits(0xc26cfb1e)),
    );
    path.line_to((f32::from_bits(0x4151fa93), f32::from_bits(0xc2a3e96b)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L705-L727 (chrome/m156)
fn battleOp24(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x409bc7b0), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x411ba103), f32::from_bits(0xc2a524b6)),
        (f32::from_bits(0x4168515c), f32::from_bits(0xc2a370af)),
    );
    path.line_to((f32::from_bits(0x4127f0cc), f32::from_bits(0xc26c4c8f)));
    path.cubic_to(
        (f32::from_bits(0x40e1017a), f32::from_bits(0xc26ec2f6)),
        (f32::from_bits(0x40613965), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3655fea5), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4168515e), f32::from_bits(0xc2a370b0)));
    path.cubic_to(
        (f32::from_bits(0x416ffb5b), f32::from_bits(0xc2a3451c)),
        (f32::from_bits(0x4177a23d), f32::from_bits(0xc2a31761)),
        (f32::from_bits(0x417f45ca), f32::from_bits(0xc2a2e77f)),
    );
    path.line_to((f32::from_bits(0x413888ce), f32::from_bits(0xc26b8638)));
    path.cubic_to(
        (f32::from_bits(0x41330328), f32::from_bits(0xc26bcb72)),
        (f32::from_bits(0x412d7b1a), f32::from_bits(0xc26c0d90)),
        (f32::from_bits(0x4127f0cb), f32::from_bits(0xc26c4c90)),
    );
    path.line_to((f32::from_bits(0x4168515e), f32::from_bits(0xc2a370b0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L730-L754 (chrome/m156)
fn battleOp25(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3655fea5), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x409bc7b0), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x411ba103), f32::from_bits(0xc2a524b6)),
        (f32::from_bits(0x4168515e), f32::from_bits(0xc2a370b0)),
    );
    path.cubic_to(
        (f32::from_bits(0x416ffb5b), f32::from_bits(0xc2a3451c)),
        (f32::from_bits(0x4177a23d), f32::from_bits(0xc2a31761)),
        (f32::from_bits(0x417f45ca), f32::from_bits(0xc2a2e77f)),
    );
    path.line_to((f32::from_bits(0x413888ce), f32::from_bits(0xc26b8638)));
    path.cubic_to(
        (f32::from_bits(0x41330328), f32::from_bits(0xc26bcb72)),
        (f32::from_bits(0x412d7b1a), f32::from_bits(0xc26c0d90)),
        (f32::from_bits(0x4127f0cc), f32::from_bits(0xc26c4c8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x40e1017a), f32::from_bits(0xc26ec2f6)),
        (f32::from_bits(0x40613965), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3655fea5), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x417f45c8), f32::from_bits(0xc2a2e780)));
    path.cubic_to(
        (f32::from_bits(0x41bda27d), f32::from_bits(0xc29fde49)),
        (f32::from_bits(0x41f99531), f32::from_bits(0xc29aa2c4)),
        (f32::from_bits(0x4218d569), f32::from_bits(0xc2935d77)),
    );
    path.line_to((f32::from_bits(0x41dcf6db), f32::from_bits(0xc2550ed7)));
    path.cubic_to(
        (f32::from_bits(0x41b46bda), f32::from_bits(0xc25f91e2)),
        (f32::from_bits(0x418915db), f32::from_bits(0xc2672288)),
        (f32::from_bits(0x413888d2), f32::from_bits(0xc26b8639)),
    );
    path.line_to((f32::from_bits(0x417f45c8), f32::from_bits(0xc2a2e780)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L757-L779 (chrome/m156)
fn battleOp26(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40b98c15), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x41394aaf), f32::from_bits(0xc2a4c8e8)),
        (f32::from_bits(0x418a04fa), f32::from_bits(0xc2a25fd2)),
    );
    path.line_to((f32::from_bits(0x41478bd6), f32::from_bits(0xc26ac20e)));
    path.cubic_to(
        (f32::from_bits(0x4105f224), f32::from_bits(0xc26e3e3c)),
        (f32::from_bits(0x40862167), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb4d00ae8), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x418a04fd), f32::from_bits(0xc2a25fd2)));
    path.cubic_to(
        (f32::from_bits(0x418e8d81), f32::from_bits(0xc2a2222a)),
        (f32::from_bits(0x41931368), f32::from_bits(0xc2a1e17a)),
        (f32::from_bits(0x41979681), f32::from_bits(0xc2a19dc3)),
    );
    path.line_to((f32::from_bits(0x415b29c8), f32::from_bits(0xc269a97e)));
    path.cubic_to(
        (f32::from_bits(0x4154a3c3), f32::from_bits(0xc26a0b66)),
        (f32::from_bits(0x414e19b0), f32::from_bits(0xc26a68ed)),
        (f32::from_bits(0x41478bd5), f32::from_bits(0xc26ac20f)),
    );
    path.line_to((f32::from_bits(0x418a04fd), f32::from_bits(0xc2a25fd2)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L782-L806 (chrome/m156)
fn battleOp27(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40b98c15), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x41394aaf), f32::from_bits(0xc2a4c8e8)),
        (f32::from_bits(0x418a04fd), f32::from_bits(0xc2a25fd2)),
    );
    path.cubic_to(
        (f32::from_bits(0x418e8d81), f32::from_bits(0xc2a2222a)),
        (f32::from_bits(0x41931368), f32::from_bits(0xc2a1e17a)),
        (f32::from_bits(0x41979681), f32::from_bits(0xc2a19dc3)),
    );
    path.line_to((f32::from_bits(0x415b29c8), f32::from_bits(0xc269a97e)));
    path.cubic_to(
        (f32::from_bits(0x4154a3c3), f32::from_bits(0xc26a0b66)),
        (f32::from_bits(0x414e19b0), f32::from_bits(0xc26a68ed)),
        (f32::from_bits(0x41478bd6), f32::from_bits(0xc26ac20e)),
    );
    path.cubic_to(
        (f32::from_bits(0x4105f224), f32::from_bits(0xc26e3e3c)),
        (f32::from_bits(0x40862167), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41979680), f32::from_bits(0xc2a19dc4)));
    path.cubic_to(
        (f32::from_bits(0x41e0e1b2), f32::from_bits(0xc29d51d4)),
        (f32::from_bits(0x42135c08), f32::from_bits(0xc295f036)),
        (f32::from_bits(0x42330e86), f32::from_bits(0xc28bc9b7)),
    );
    path.line_to((f32::from_bits(0x42017048), f32::from_bits(0xc24a1a63)));
    path.cubic_to(
        (f32::from_bits(0x41d50cc4), f32::from_bits(0xc258c742)),
        (f32::from_bits(0x41a290a5), f32::from_bits(0xc263733c)),
        (f32::from_bits(0x415b29c7), f32::from_bits(0xc269a980)),
    );
    path.line_to((f32::from_bits(0x41979680), f32::from_bits(0xc2a19dc4)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L809-L831 (chrome/m156)
fn battleOp28(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40dd1e63), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x415caf98), f32::from_bits(0xc2a44632)),
        (f32::from_bits(0x41a3e96c), f32::from_bits(0xc2a0dcda)),
    );
    path.line_to((f32::from_bits(0x416cfb1c), f32::from_bits(0xc2689294)));
    path.cubic_to(
        (f32::from_bits(0x411f8831), f32::from_bits(0xc26d8140)),
        (f32::from_bits(0x409fd849), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41a3e96b), f32::from_bits(0xc2a0dcda)));
    path.cubic_to(
        (f32::from_bits(0x41a94306), f32::from_bits(0xc2a085a1)),
        (f32::from_bits(0x41ae9839), f32::from_bits(0xc2a02a23)),
        (f32::from_bits(0x41b3e8b2), f32::from_bits(0xc29fca67)),
    );
    path.line_to((f32::from_bits(0x41820dff), f32::from_bits(0xc26705ca)));
    path.cubic_to(
        (f32::from_bits(0x417c6d0a), f32::from_bits(0xc2679035)),
        (f32::from_bits(0x4174b742), f32::from_bits(0xc268147b)),
        (f32::from_bits(0x416cfb1d), f32::from_bits(0xc2689296)),
    );
    path.line_to((f32::from_bits(0x41a3e96b), f32::from_bits(0xc2a0dcda)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L834-L860 (chrome/m156)
fn battleOp29(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40dd1e62), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x415caf97), f32::from_bits(0xc2a44632)),
        (f32::from_bits(0x41a3e96b), f32::from_bits(0xc2a0dcda)),
    );
    path.line_to((f32::from_bits(0x416cfb1d), f32::from_bits(0xc2689296)));
    path.cubic_to(
        (f32::from_bits(0x4174b742), f32::from_bits(0xc268147b)),
        (f32::from_bits(0x417c6d0a), f32::from_bits(0xc2679035)),
        (f32::from_bits(0x41820dff), f32::from_bits(0xc26705ca)),
    );
    path.line_to((f32::from_bits(0x41b3e8b2), f32::from_bits(0xc29fca67)));
    path.cubic_to(
        (f32::from_bits(0x41ae9839), f32::from_bits(0xc2a02a23)),
        (f32::from_bits(0x41a94307), f32::from_bits(0xc2a085a1)),
        (f32::from_bits(0x41a3e96c), f32::from_bits(0xc2a0dcda)),
    );
    path.line_to((f32::from_bits(0x416cfb1c), f32::from_bits(0xc2689294)));
    path.cubic_to(
        (f32::from_bits(0x411f8831), f32::from_bits(0xc26d8140)),
        (f32::from_bits(0x409fd849), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41b3e8b1), f32::from_bits(0xc29fca67)));
    path.cubic_to(
        (f32::from_bits(0x4205291f), f32::from_bits(0xc299b5bb)),
        (f32::from_bits(0x422d73c0), f32::from_bits(0xc28f4fcf)),
        (f32::from_bits(0x425064bf), f32::from_bits(0xc2813989)),
    );
    path.line_to((f32::from_bits(0x4216a55b), f32::from_bits(0xc23ad4b9)));
    path.cubic_to(
        (f32::from_bits(0x41fac62f), f32::from_bits(0xc24f329e)),
        (f32::from_bits(0x41c0857c), f32::from_bits(0xc25e3b2e)),
        (f32::from_bits(0x41820dfe), f32::from_bits(0xc26705cb)),
    );
    path.line_to((f32::from_bits(0x41b3e8b1), f32::from_bits(0xc29fca67)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L863-L885 (chrome/m156)
fn battleOp30(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41028186), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4182264a), f32::from_bits(0xc2a39869)),
        (f32::from_bits(0x41c098e8), f32::from_bits(0xc29edd15)),
    );
    path.line_to((f32::from_bits(0x418b3a1a), f32::from_bits(0xc265aeac)));
    path.cubic_to(
        (f32::from_bits(0x413c2b06), f32::from_bits(0xc26c85fe)),
        (f32::from_bits(0x40bcaeed), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x337fa8c0), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41c098e9), f32::from_bits(0xc29edd15)));
    path.cubic_to(
        (f32::from_bits(0x41c6d4b6), f32::from_bits(0xc29e642a)),
        (f32::from_bits(0x41cd0950), f32::from_bits(0xc29de562)),
        (f32::from_bits(0x41d33633), f32::from_bits(0xc29d60c8)),
    );
    path.line_to((f32::from_bits(0x4198aee4), f32::from_bits(0xc26388d7)));
    path.cubic_to(
        (f32::from_bits(0x41943815), f32::from_bits(0xc264488f)),
        (f32::from_bits(0x418fbbb2), f32::from_bits(0xc264ffdc)),
        (f32::from_bits(0x418b3a19), f32::from_bits(0xc265aeae)),
    );
    path.line_to((f32::from_bits(0x41c098e9), f32::from_bits(0xc29edd15)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L888-L912 (chrome/m156)
fn battleOp31(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41028186), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4182264a), f32::from_bits(0xc2a39869)),
        (f32::from_bits(0x41c098e9), f32::from_bits(0xc29edd15)),
    );
    path.cubic_to(
        (f32::from_bits(0x41c6d4b6), f32::from_bits(0xc29e642a)),
        (f32::from_bits(0x41cd0950), f32::from_bits(0xc29de562)),
        (f32::from_bits(0x41d33633), f32::from_bits(0xc29d60c8)),
    );
    path.line_to((f32::from_bits(0x4198aee4), f32::from_bits(0xc26388d7)));
    path.cubic_to(
        (f32::from_bits(0x41943816), f32::from_bits(0xc264488f)),
        (f32::from_bits(0x418fbbb2), f32::from_bits(0xc264ffda)),
        (f32::from_bits(0x418b3a1a), f32::from_bits(0xc265aeac)),
    );
    path.cubic_to(
        (f32::from_bits(0x413c2b06), f32::from_bits(0xc26c85fe)),
        (f32::from_bits(0x40bcaeed), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41d33633), f32::from_bits(0xc29d60c8)));
    path.cubic_to(
        (f32::from_bits(0x421be102), f32::from_bits(0xc294f1be)),
        (f32::from_bits(0x4249615f), f32::from_bits(0xc2869cbc)),
        (f32::from_bits(0x426e4d45), f32::from_bits(0xc26729aa)),
    );
    path.line_to((f32::from_bits(0x422c4432), f32::from_bits(0xc2271b0a)));
    path.cubic_to(
        (f32::from_bits(0x42119380), f32::from_bits(0xc2429ec2)),
        (f32::from_bits(0x41e15dfd), f32::from_bits(0xc257575a)),
        (f32::from_bits(0x4198aee4), f32::from_bits(0xc26388d8)),
    );
    path.line_to((f32::from_bits(0x41d33633), f32::from_bits(0xc29d60c8)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L915-L937 (chrome/m156)
fn battleOp32(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4118c001), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x41982d6e), f32::from_bits(0xc2a2b4b2)),
        (f32::from_bits(0x41e01284), f32::from_bits(0xc29c4333)),
    );
    path.line_to((f32::from_bits(0x41a1fae3), f32::from_bits(0xc261ebf5)));
    path.cubic_to(
        (f32::from_bits(0x415c0406), f32::from_bits(0xc26b3cc7)),
        (f32::from_bits(0x40dcd7ee), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x35f7fd46), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41e01286), f32::from_bits(0xc29c4334)));
    path.cubic_to(
        (f32::from_bits(0x41e73e86), f32::from_bits(0xc29b9ea8)),
        (f32::from_bits(0x41ee5f11), f32::from_bits(0xc29af239)),
        (f32::from_bits(0x41f57356), f32::from_bits(0xc29a3dfa)),
    );
    path.line_to((f32::from_bits(0x41b16f25), f32::from_bits(0xc25f0029)));
    path.cubic_to(
        (f32::from_bits(0x41ac5112), f32::from_bits(0xc26004c3)),
        (f32::from_bits(0x41a72a20), f32::from_bits(0xc260fe11)),
        (f32::from_bits(0x41a1fae3), f32::from_bits(0xc261ebf7)),
    );
    path.line_to((f32::from_bits(0x41e01286), f32::from_bits(0xc29c4334)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L940-L965 (chrome/m156)
fn battleOp33(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4118c001), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x41982d6e), f32::from_bits(0xc2a2b4b2)),
        (f32::from_bits(0x41e01286), f32::from_bits(0xc29c4334)),
    );
    path.cubic_to(
        (f32::from_bits(0x41e73e86), f32::from_bits(0xc29b9ea8)),
        (f32::from_bits(0x41ee5f11), f32::from_bits(0xc29af239)),
        (f32::from_bits(0x41f57356), f32::from_bits(0xc29a3dfa)),
    );
    path.line_to((f32::from_bits(0x41b16f25), f32::from_bits(0xc25f0029)));
    path.cubic_to(
        (f32::from_bits(0x41ac5112), f32::from_bits(0xc26004c3)),
        (f32::from_bits(0x41a72a20), f32::from_bits(0xc260fe11)),
        (f32::from_bits(0x41a1fae3), f32::from_bits(0xc261ebf7)),
    );
    path.line_to((f32::from_bits(0x41a1fae3), f32::from_bits(0xc261ebf5)));
    path.cubic_to(
        (f32::from_bits(0x415c0406), f32::from_bits(0xc26b3cc7)),
        (f32::from_bits(0x40dcd7ee), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41f57359), f32::from_bits(0xc29a3dfa)));
    path.cubic_to(
        (f32::from_bits(0x42347528), f32::from_bits(0xc28ec218)),
        (f32::from_bits(0x42669614), f32::from_bits(0xc276cf04)),
        (f32::from_bits(0x4285b481), f32::from_bits(0xc244c364)),
    );
    path.line_to((f32::from_bits(0x42414f00), f32::from_bits(0xc20e3d0e)));
    path.cubic_to(
        (f32::from_bits(0x4226b05a), f32::from_bits(0xc2326a79)),
        (f32::from_bits(0x4202738a), f32::from_bits(0xc24e65b9)),
        (f32::from_bits(0x41b16f25), f32::from_bits(0xc25f0028)),
    );
    path.line_to((f32::from_bits(0x41f57359), f32::from_bits(0xc29a3dfa)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L968-L990 (chrome/m156)
fn battleOp34(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41360dec), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x41b5150e), f32::from_bits(0xc2a1522b)),
        (f32::from_bits(0x42044925), f32::from_bits(0xc29840e5)),
    );
    path.line_to((f32::from_bits(0x41bf41a8), f32::from_bits(0xc25c2022)));
    path.cubic_to(
        (f32::from_bits(0x4182e721), f32::from_bits(0xc2693c30)),
        (f32::from_bits(0x41039b08), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3673fea3), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42044925), f32::from_bits(0xc29840e4)));
    path.cubic_to(
        (f32::from_bits(0x4208721a), f32::from_bits(0xc2975992)),
        (f32::from_bits(0x420c9178), f32::from_bits(0xc296675c)),
        (f32::from_bits(0x4210a695), f32::from_bits(0xc2956a6a)),
    );
    path.line_to((f32::from_bits(0x41d1222e), f32::from_bits(0xc25805ce)));
    path.cubic_to(
        (f32::from_bits(0x41cb3b2f), f32::from_bits(0xc2597382)),
        (f32::from_bits(0x41c5455b), f32::from_bits(0xc25ad1b2)),
        (f32::from_bits(0x41bf41a9), f32::from_bits(0xc25c2023)),
    );
    path.line_to((f32::from_bits(0x42044925), f32::from_bits(0xc29840e4)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L993-L1018 (chrome/m156)
fn battleOp35(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3673fea3), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41360dec), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x41b5150e), f32::from_bits(0xc2a1522b)),
        (f32::from_bits(0x42044925), f32::from_bits(0xc29840e5)),
    );
    path.line_to((f32::from_bits(0x4210a695), f32::from_bits(0xc2956a6a)));
    path.line_to((f32::from_bits(0x41d1222e), f32::from_bits(0xc25805ce)));
    path.cubic_to(
        (f32::from_bits(0x41cb3b2f), f32::from_bits(0xc2597382)),
        (f32::from_bits(0x41c5455b), f32::from_bits(0xc25ad1b2)),
        (f32::from_bits(0x41bf41a9), f32::from_bits(0xc25c2023)),
    );
    path.line_to((f32::from_bits(0x41bf41a8), f32::from_bits(0xc25c2022)));
    path.cubic_to(
        (f32::from_bits(0x4182e721), f32::from_bits(0xc2693c30)),
        (f32::from_bits(0x41039b08), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3673fea3), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4210a693), f32::from_bits(0xc2956a6a)));
    path.cubic_to(
        (f32::from_bits(0x42536b4d), f32::from_bits(0xc2854182)),
        (f32::from_bits(0x4284b863), f32::from_bits(0xc254c33a)),
        (f32::from_bits(0x42950c68), f32::from_bits(0xc2122882)),
    );
    path.line_to((f32::from_bits(0x42577de3), f32::from_bits(0xc1d35027)));
    path.cubic_to(
        (f32::from_bits(0x423fe27d), f32::from_bits(0xc219cde7)),
        (f32::from_bits(0x4218d548), f32::from_bits(0xc240a8bd)),
        (f32::from_bits(0x41d1222f), f32::from_bits(0xc25805ce)),
    );
    path.line_to((f32::from_bits(0x4210a693), f32::from_bits(0xc2956a6a)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1021-L1043 (chrome/m156)
fn battleOp36(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x414e6589), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x41ccf9e5), f32::from_bits(0xc29ffc89)),
        (f32::from_bits(0x4214a0bb), f32::from_bits(0xc2946fc8)),
    );
    path.line_to((f32::from_bits(0x41d6e236), f32::from_bits(0xc2569b72)));
    path.cubic_to(
        (f32::from_bits(0x41942cf0), f32::from_bits(0xc2674e45)),
        (f32::from_bits(0x411533d1), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4214a0bb), f32::from_bits(0xc2946fc9)));
    path.cubic_to(
        (f32::from_bits(0x421938a6), f32::from_bits(0xc293496b)),
        (f32::from_bits(0x421dc2c1), f32::from_bits(0xc2921574)),
        (f32::from_bits(0x42223e19), f32::from_bits(0xc290d421)),
    );
    path.line_to((f32::from_bits(0x41ea914d), f32::from_bits(0xc251640c)));
    path.cubic_to(
        (f32::from_bits(0x41e4167f), f32::from_bits(0xc253349e)),
        (f32::from_bits(0x41dd8659), f32::from_bits(0xc254f1de)),
        (f32::from_bits(0x41d6e239), f32::from_bits(0xc2569b73)),
    );
    path.line_to((f32::from_bits(0x4214a0bb), f32::from_bits(0xc2946fc9)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1046-L1071 (chrome/m156)
fn battleOp37(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x414e6589), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x41ccf9e5), f32::from_bits(0xc29ffc89)),
        (f32::from_bits(0x4214a0bb), f32::from_bits(0xc2946fc9)),
    );
    path.cubic_to(
        (f32::from_bits(0x421938a6), f32::from_bits(0xc293496b)),
        (f32::from_bits(0x421dc2c1), f32::from_bits(0xc2921574)),
        (f32::from_bits(0x42223e19), f32::from_bits(0xc290d421)),
    );
    path.line_to((f32::from_bits(0x41ea914d), f32::from_bits(0xc251640c)));
    path.cubic_to(
        (f32::from_bits(0x41e4167f), f32::from_bits(0xc253349e)),
        (f32::from_bits(0x41dd8659), f32::from_bits(0xc254f1de)),
        (f32::from_bits(0x41d6e239), f32::from_bits(0xc2569b73)),
    );
    path.line_to((f32::from_bits(0x41d6e236), f32::from_bits(0xc2569b72)));
    path.cubic_to(
        (f32::from_bits(0x41942cf0), f32::from_bits(0xc2674e45)),
        (f32::from_bits(0x411533d1), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42223e19), f32::from_bits(0xc290d422)));
    path.cubic_to(
        (f32::from_bits(0x426bbc38), f32::from_bits(0xc2787e1d)),
        (f32::from_bits(0x42916a94), f32::from_bits(0xc234ee59)),
        (f32::from_bits(0x429e2fac), f32::from_bits(0xc1c951fc)),
    );
    path.line_to((f32::from_bits(0x4264b3f7), f32::from_bits(0xc191885f)));
    path.cubic_to(
        (f32::from_bits(0x42523d91), f32::from_bits(0xc202cb25)),
        (f32::from_bits(0x422a6939), f32::from_bits(0xc233a21b)),
        (f32::from_bits(0x41ea914d), f32::from_bits(0xc251640d)),
    );
    path.line_to((f32::from_bits(0x42223e19), f32::from_bits(0xc290d422)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1074-L1096 (chrome/m156)
fn battleOp38(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x416c96cf), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x41ea70fe), f32::from_bits(0xc29e1973)),
        (f32::from_bits(0x422836c6), f32::from_bits(0xc28f1d8a)),
    );
    path.line_to((f32::from_bits(0x41f3336d), f32::from_bits(0xc24ee9f1)));
    path.cubic_to(
        (f32::from_bits(0x41a979c6), f32::from_bits(0xc26493d6)),
        (f32::from_bits(0x412b073c), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x422836c5), f32::from_bits(0xc28f1d8b)));
    path.cubic_to(
        (f32::from_bits(0x422d4896), f32::from_bits(0xc28da02f)),
        (f32::from_bits(0x423245ea), f32::from_bits(0xc28c11a8)),
        (f32::from_bits(0x42372d65), f32::from_bits(0xc28a7261)),
    );
    path.line_to((f32::from_bits(0x42046ad7), f32::from_bits(0xc24829ff)));
    path.cubic_to(
        (f32::from_bits(0x4200df44), f32::from_bits(0xc24a8267)),
        (f32::from_bits(0x41fa87ca), f32::from_bits(0xc24cc296)),
        (f32::from_bits(0x41f3336d), f32::from_bits(0xc24ee9f1)),
    );
    path.line_to((f32::from_bits(0x422836c5), f32::from_bits(0xc28f1d8b)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1099-L1123 (chrome/m156)
fn battleOp39(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x416c96cf), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x41ea70fe), f32::from_bits(0xc29e1973)),
        (f32::from_bits(0x422836c5), f32::from_bits(0xc28f1d8b)),
    );
    path.cubic_to(
        (f32::from_bits(0x422d4896), f32::from_bits(0xc28da02f)),
        (f32::from_bits(0x423245ea), f32::from_bits(0xc28c11a8)),
        (f32::from_bits(0x42372d65), f32::from_bits(0xc28a7261)),
    );
    path.line_to((f32::from_bits(0x42046ad7), f32::from_bits(0xc24829ff)));
    path.cubic_to(
        (f32::from_bits(0x4200df44), f32::from_bits(0xc24a8267)),
        (f32::from_bits(0x41fa87ca), f32::from_bits(0xc24cc296)),
        (f32::from_bits(0x41f3336d), f32::from_bits(0xc24ee9f1)),
    );
    path.cubic_to(
        (f32::from_bits(0x41a979c6), f32::from_bits(0xc26493d6)),
        (f32::from_bits(0x412b073c), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42372d65), f32::from_bits(0xc28a7262)));
    path.cubic_to(
        (f32::from_bits(0x4283f2b3), f32::from_bits(0xc25f7e9c)),
        (f32::from_bits(0x429ea5c2), f32::from_bits(0xc2098801)),
        (f32::from_bits(0x42a4b292), f32::from_bits(0xc12607b1)),
    );
    path.line_to((f32::from_bits(0x426e1def), f32::from_bits(0xc0f00b21)));
    path.cubic_to(
        (f32::from_bits(0x42655eb1), f32::from_bits(0xc1c6d725)),
        (f32::from_bits(0x423ec4ad), f32::from_bits(0xc2218ff6)),
        (f32::from_bits(0x42046ad7), f32::from_bits(0xc2482a00)),
    );
    path.line_to((f32::from_bits(0x42372d65), f32::from_bits(0xc28a7262)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1126-L1148 (chrome/m156)
fn battleOp40(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4184d4a8), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42034ddf), f32::from_bits(0xc29c0a4c)),
        (f32::from_bits(0x423a47b2), f32::from_bits(0xc289686d)),
    );
    path.line_to((f32::from_bits(0x4206a908), f32::from_bits(0xc246a97c)));
    path.cubic_to(
        (f32::from_bits(0x41bdd65f), f32::from_bits(0xc26199af)),
        (f32::from_bits(0x41400b5c), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb560056c), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423a47b2), f32::from_bits(0xc289686d)));
    path.cubic_to(
        (f32::from_bits(0x423fbcc3), f32::from_bits(0xc2878eef)),
        (f32::from_bits(0x4245154e), f32::from_bits(0xc285a0be)),
        (f32::from_bits(0x424a4f85), f32::from_bits(0xc2839e81)),
    );
    path.line_to((f32::from_bits(0x42123fa7), f32::from_bits(0xc23e4af2)));
    path.cubic_to(
        (f32::from_bits(0x420e7846), f32::from_bits(0xc241326c)),
        (f32::from_bits(0x420a9af5), f32::from_bits(0xc243fcec)),
        (f32::from_bits(0x4206a907), f32::from_bits(0xc246a97c)),
    );
    path.line_to((f32::from_bits(0x423a47b2), f32::from_bits(0xc289686d)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1151-L1173 (chrome/m156)
fn battleOp41(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4196c4f9), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42148669), f32::from_bits(0xc2992c23)),
        (f32::from_bits(0x424f6452), f32::from_bits(0xc281a081)),
    );
    path.line_to((f32::from_bits(0x4215ebfd), f32::from_bits(0xc23b6999)));
    path.cubic_to(
        (f32::from_bits(0x41d6bc2a), f32::from_bits(0xc25d7441)),
        (f32::from_bits(0x4159fada), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb560056c), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x424f6452), f32::from_bits(0xc281a081)));
    path.cubic_to(
        (f32::from_bits(0x42553921), f32::from_bits(0xc27e96d1)),
        (f32::from_bits(0x425ae53b), f32::from_bits(0xc279ba9d)),
        (f32::from_bits(0x42606622), f32::from_bits(0xc274ae80)),
    );
    path.line_to((f32::from_bits(0x42223753), f32::from_bits(0xc230e0d8)));
    path.cubic_to(
        (f32::from_bits(0x421e3cd8), f32::from_bits(0xc23486e8)),
        (f32::from_bits(0x421a2322), f32::from_bits(0xc2380a55)),
        (f32::from_bits(0x4215ebfe), f32::from_bits(0xc23b6999)),
    );
    path.line_to((f32::from_bits(0x424f6452), f32::from_bits(0xc281a081)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1176-L1200 (chrome/m156)
fn battleOp42(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4196c4f9), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42148669), f32::from_bits(0xc2992c23)),
        (f32::from_bits(0x424f6452), f32::from_bits(0xc281a081)),
    );
    path.cubic_to(
        (f32::from_bits(0x42553921), f32::from_bits(0xc27e96d1)),
        (f32::from_bits(0x425ae53b), f32::from_bits(0xc279ba9d)),
        (f32::from_bits(0x42606622), f32::from_bits(0xc274ae80)),
    );
    path.line_to((f32::from_bits(0x42223753), f32::from_bits(0xc230e0d8)));
    path.cubic_to(
        (f32::from_bits(0x421e3cd8), f32::from_bits(0xc23486e8)),
        (f32::from_bits(0x421a2322), f32::from_bits(0xc2380a55)),
        (f32::from_bits(0x4215ebfd), f32::from_bits(0xc23b6999)),
    );
    path.cubic_to(
        (f32::from_bits(0x41d6bc2a), f32::from_bits(0xc25d7441)),
        (f32::from_bits(0x4159fada), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42606622), f32::from_bits(0xc274ae80)));
    path.cubic_to(
        (f32::from_bits(0x429deeac), f32::from_bits(0xc220cc44)),
        (f32::from_bits(0x42b0742c), f32::from_bits(0xc1039d5c)),
        (f32::from_bits(0x42a03731), f32::from_bits(0x41adc1b3)),
    );
    path.line_to((f32::from_bits(0x4267a314), f32::from_bits(0x417b36e3)));
    path.cubic_to(
        (f32::from_bits(0x427f1d2c), f32::from_bits(0xc0be4950)),
        (f32::from_bits(0x426455fc), f32::from_bits(0xc1e87a9a)),
        (f32::from_bits(0x42223754), f32::from_bits(0xc230e0d7)),
    );
    path.line_to((f32::from_bits(0x42606622), f32::from_bits(0xc274ae80)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1203-L1225 (chrome/m156)
fn battleOp43(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41aa5d9e), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42271b56), f32::from_bits(0xc295a109)),
        (f32::from_bits(0x4264d340), f32::from_bits(0xc2708c1d)),
    );
    path.line_to((f32::from_bits(0x42256a74), f32::from_bits(0xc22de3bf)));
    path.cubic_to(
        (f32::from_bits(0x41f199ac), f32::from_bits(0xc25854c9)),
        (f32::from_bits(0x41764fdb), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4264d342), f32::from_bits(0xc2708c1d)));
    path.cubic_to(
        (f32::from_bits(0x426aec59), f32::from_bits(0xc26abf16)),
        (f32::from_bits(0x4270cc6c), f32::from_bits(0xc264b73d)),
        (f32::from_bits(0x42767031), f32::from_bits(0xc25e77e8)),
    );
    path.line_to((f32::from_bits(0x423225ec), f32::from_bits(0xc220d20e)));
    path.cubic_to(
        (f32::from_bits(0x422e123c), f32::from_bits(0xc2255633)),
        (f32::from_bits(0x4229d2f5), f32::from_bits(0xc229b23c)),
        (f32::from_bits(0x42256a74), f32::from_bits(0xc22de3c0)),
    );
    path.line_to((f32::from_bits(0x4264d342), f32::from_bits(0xc2708c1d)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1228-L1254 (chrome/m156)
fn battleOp44(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41aa5d9e), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42271b56), f32::from_bits(0xc295a109)),
        (f32::from_bits(0x4264d340), f32::from_bits(0xc2708c1d)),
    );
    path.line_to((f32::from_bits(0x4264d342), f32::from_bits(0xc2708c1d)));
    path.cubic_to(
        (f32::from_bits(0x426aec59), f32::from_bits(0xc26abf16)),
        (f32::from_bits(0x4270cc6c), f32::from_bits(0xc264b73d)),
        (f32::from_bits(0x42767031), f32::from_bits(0xc25e77e8)),
    );
    path.line_to((f32::from_bits(0x423225ec), f32::from_bits(0xc220d20e)));
    path.cubic_to(
        (f32::from_bits(0x422e123c), f32::from_bits(0xc2255633)),
        (f32::from_bits(0x4229d2f5), f32::from_bits(0xc229b23c)),
        (f32::from_bits(0x42256a74), f32::from_bits(0xc22de3c0)),
    );
    path.line_to((f32::from_bits(0x42256a74), f32::from_bits(0xc22de3bf)));
    path.cubic_to(
        (f32::from_bits(0x41f199ac), f32::from_bits(0xc25854c9)),
        (f32::from_bits(0x41764fdb), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42767032), f32::from_bits(0xc25e77e8)));
    path.cubic_to(
        (f32::from_bits(0x42aa697a), f32::from_bits(0xc1ebd370)),
        (f32::from_bits(0x42b37ad4), f32::from_bits(0x410b48c2)),
        (f32::from_bits(0x4291d766), f32::from_bits(0x421e927b)),
    );
    path.line_to((f32::from_bits(0x4252dae4), f32::from_bits(0x41e542d2)));
    path.cubic_to(
        (f32::from_bits(0x4281be95), f32::from_bits(0x40c95ff9)),
        (f32::from_bits(0x427660fe), f32::from_bits(0xc1aa7a03)),
        (f32::from_bits(0x423225ed), f32::from_bits(0xc220d20e)),
    );
    path.line_to((f32::from_bits(0x42767032), f32::from_bits(0xc25e77e8)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1257-L1279 (chrome/m156)
fn battleOp45(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41bfbd07), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x423b0ef1), f32::from_bits(0xc2914772)),
        (f32::from_bits(0x427a1b1d), f32::from_bits(0xc25a5641)),
    );
    path.line_to((f32::from_bits(0x4234ccaa), f32::from_bits(0xc21dd57d)));
    path.cubic_to(
        (f32::from_bits(0x42073912), f32::from_bits(0xc2520ac5)),
        (f32::from_bits(0x418a9b2a), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x427a1b1e), f32::from_bits(0xc25a5642)));
    path.cubic_to(
        (f32::from_bits(0x4280286a), f32::from_bits(0xc253393c)),
        (f32::from_bits(0x42831c11), f32::from_bits(0xc24bd939)),
        (f32::from_bits(0x4285e673), f32::from_bits(0xc2443b5f)),
    );
    path.line_to((f32::from_bits(0x42419733), f32::from_bits(0xc20ddaba)));
    path.cubic_to(
        (f32::from_bits(0x423d8e5d), f32::from_bits(0xc2135c44)),
        (f32::from_bits(0x423949dc), f32::from_bits(0xc218b118)),
        (f32::from_bits(0x4234ccac), f32::from_bits(0xc21dd57e)),
    );
    path.line_to((f32::from_bits(0x427a1b1e), f32::from_bits(0xc25a5642)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1282-L1307 (chrome/m156)
fn battleOp46(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3697ff52), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41bfbd07), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x423b0ef1), f32::from_bits(0xc2914772)),
        (f32::from_bits(0x427a1b1e), f32::from_bits(0xc25a5642)),
    );
    path.cubic_to(
        (f32::from_bits(0x4280286a), f32::from_bits(0xc253393c)),
        (f32::from_bits(0x42831c11), f32::from_bits(0xc24bd939)),
        (f32::from_bits(0x4285e673), f32::from_bits(0xc2443b5f)),
    );
    path.line_to((f32::from_bits(0x42419733), f32::from_bits(0xc20ddaba)));
    path.cubic_to(
        (f32::from_bits(0x423d8e5d), f32::from_bits(0xc2135c44)),
        (f32::from_bits(0x423949dc), f32::from_bits(0xc218b118)),
        (f32::from_bits(0x4234ccac), f32::from_bits(0xc21dd57e)),
    );
    path.line_to((f32::from_bits(0x4234ccaa), f32::from_bits(0xc21dd57d)));
    path.cubic_to(
        (f32::from_bits(0x42073912), f32::from_bits(0xc2520ac5)),
        (f32::from_bits(0x418a9b2a), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4285e672), f32::from_bits(0xc2443b5f)));
    path.cubic_to(
        (f32::from_bits(0x42b50145), f32::from_bits(0xc1875361)),
        (f32::from_bits(0x42afc74e), f32::from_bits(0x41db6d5e)),
        (f32::from_bits(0x4272e616), f32::from_bits(0x426253de)),
    );
    path.line_to((f32::from_bits(0x422f96e8), f32::from_bits(0x42239c3e)));
    path.cubic_to(
        (f32::from_bits(0x427e233c), f32::from_bits(0x419e9f42)),
        (f32::from_bits(0x4282d8d3), f32::from_bits(0xc143a6d1)),
        (f32::from_bits(0x42419734), f32::from_bits(0xc20ddabb)),
    );
    path.line_to((f32::from_bits(0x4285e672), f32::from_bits(0xc2443b5f)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1310-L1332 (chrome/m156)
fn battleOp47(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41d59904), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x424f13ae), f32::from_bits(0xc28c4fb7)),
        (f32::from_bits(0x4286bb70), f32::from_bits(0xc241f0ca)),
    );
    path.line_to((f32::from_bits(0x4242cb24), f32::from_bits(0xc20c32b1)));
    path.cubic_to(
        (f32::from_bits(0x4215b1b4), f32::from_bits(0xc24adc20)),
        (f32::from_bits(0x419a6875), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4286bb71), f32::from_bits(0xc241f0ca)));
    path.cubic_to(
        (f32::from_bits(0x4289cb2b), f32::from_bits(0xc2396eee)),
        (f32::from_bits(0x428ca6e5), f32::from_bits(0xc230a410)),
        (f32::from_bits(0x428f4c27), f32::from_bits(0xc22797c0)),
    );
    path.line_to((f32::from_bits(0x424f2d54), f32::from_bits(0xc1f24d85)));
    path.cubic_to(
        (f32::from_bits(0x424b5a2a), f32::from_bits(0xc1ff6268)),
        (f32::from_bits(0x42473840), f32::from_bits(0xc2060c56)),
        (f32::from_bits(0x4242cb25), f32::from_bits(0xc20c32b2)),
    );
    path.line_to((f32::from_bits(0x4286bb71), f32::from_bits(0xc241f0ca)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1335-L1359 (chrome/m156)
fn battleOp48(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41d59904), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x424f13ae), f32::from_bits(0xc28c4fb7)),
        (f32::from_bits(0x4286bb71), f32::from_bits(0xc241f0ca)),
    );
    path.cubic_to(
        (f32::from_bits(0x4289cb2b), f32::from_bits(0xc2396eee)),
        (f32::from_bits(0x428ca6e5), f32::from_bits(0xc230a410)),
        (f32::from_bits(0x428f4c27), f32::from_bits(0xc22797c0)),
    );
    path.line_to((f32::from_bits(0x424f2d54), f32::from_bits(0xc1f24d85)));
    path.cubic_to(
        (f32::from_bits(0x424b5a2a), f32::from_bits(0xc1ff6268)),
        (f32::from_bits(0x42473840), f32::from_bits(0xc2060c56)),
        (f32::from_bits(0x4242cb24), f32::from_bits(0xc20c32b1)),
    );
    path.cubic_to(
        (f32::from_bits(0x4215b1b4), f32::from_bits(0xc24adc20)),
        (f32::from_bits(0x419a6875), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x428f4c27), f32::from_bits(0xc22797c0)));
    path.cubic_to(
        (f32::from_bits(0x42bc6513), f32::from_bits(0xc055a915)),
        (f32::from_bits(0x42a45eb2), f32::from_bits(0x42389acf)),
        (f32::from_bits(0x4231df29), f32::from_bits(0x428c2a69)),
    );
    path.line_to((f32::from_bits(0x420094fc), f32::from_bits(0x424aa62f)));
    path.cubic_to(
        (f32::from_bits(0x426da4ad), f32::from_bits(0x42057300)),
        (f32::from_bits(0x42883065), f32::from_bits(0xc01a7416)),
        (f32::from_bits(0x424f2d56), f32::from_bits(0xc1f24d87)),
    );
    path.line_to((f32::from_bits(0x428f4c27), f32::from_bits(0xc22797c0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1362-L1384 (chrome/m156)
fn battleOp49(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41eed329), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4265a038), f32::from_bits(0xc285ef96)),
        (f32::from_bits(0x42905111), f32::from_bits(0xc2240eac)),
    );
    path.line_to((f32::from_bits(0x4250a68d), f32::from_bits(0xc1ed30fa)));
    path.cubic_to(
        (f32::from_bits(0x4225fe9e), f32::from_bits(0xc241a46c)),
        (f32::from_bits(0x41aca4fc), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42905111), f32::from_bits(0xc2240ead)));
    path.cubic_to(
        (f32::from_bits(0x429332f8), f32::from_bits(0xc219ea36)),
        (f32::from_bits(0x4295cfef), f32::from_bits(0xc20f79c4)),
        (f32::from_bits(0x4298252c), f32::from_bits(0xc204c875)),
    );
    path.line_to((f32::from_bits(0x425bf80f), f32::from_bits(0xc1bff9b9)));
    path.cubic_to(
        (f32::from_bits(0x42589896), f32::from_bits(0xc1cf6f48)),
        (f32::from_bits(0x4254d168), f32::from_bits(0xc1de8710)),
        (f32::from_bits(0x4250a68e), f32::from_bits(0xc1ed30fc)),
    );
    path.line_to((f32::from_bits(0x42905111), f32::from_bits(0xc2240ead)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1387-L1414 (chrome/m156)
fn battleOp50(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41eed328), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4265a038), f32::from_bits(0xc285ef96)),
        (f32::from_bits(0x42905111), f32::from_bits(0xc2240ead)),
    );
    path.line_to((f32::from_bits(0x42905111), f32::from_bits(0xc2240eac)));
    path.cubic_to(
        (f32::from_bits(0x429332f8), f32::from_bits(0xc219ea35)),
        (f32::from_bits(0x4295cfef), f32::from_bits(0xc20f79c4)),
        (f32::from_bits(0x4298252c), f32::from_bits(0xc204c875)),
    );
    path.line_to((f32::from_bits(0x425bf80f), f32::from_bits(0xc1bff9b9)));
    path.cubic_to(
        (f32::from_bits(0x42589896), f32::from_bits(0xc1cf6f48)),
        (f32::from_bits(0x4254d168), f32::from_bits(0xc1de8710)),
        (f32::from_bits(0x4250a68d), f32::from_bits(0xc1ed30fa)),
    );
    path.cubic_to(
        (f32::from_bits(0x4225fe9e), f32::from_bits(0xc241a46c)),
        (f32::from_bits(0x41aca4fc), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4298252d), f32::from_bits(0xc204c875)));
    path.cubic_to(
        (f32::from_bits(0x42ab560c), f32::from_bits(0xc1334da0)),
        (f32::from_bits(0x42aa8ee6), f32::from_bits(0x415dbf57)),
        (f32::from_bits(0x4296030d), f32::from_bits(0x420e292a)),
    );
    path.cubic_to(
        (f32::from_bits(0x42817734), f32::from_bits(0x4264e27f)),
        (f32::from_bits(0x42365290), f32::from_bits(0x4292cae0)),
        (f32::from_bits(0x41b3e39e), f32::from_bits(0x429fcac3)),
    );
    path.line_to((f32::from_bits(0x41820a52), f32::from_bits(0x4267064e)));
    path.cubic_to(
        (f32::from_bits(0x4203cca7), f32::from_bits(0x42543ae9)),
        (f32::from_bits(0x423b2de4), f32::from_bits(0x42257578)),
        (f32::from_bits(0x4258e27d), f32::from_bits(0x41cd88a1)),
    );
    path.cubic_to(
        (f32::from_bits(0x42769717), f32::from_bits(0x41204ca2)),
        (f32::from_bits(0x4277b705), f32::from_bits(0xc1019de9)),
        (f32::from_bits(0x425bf810), f32::from_bits(0xc1bff9bb)),
    );
    path.line_to((f32::from_bits(0x4298252d), f32::from_bits(0xc204c875)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1417-L1439 (chrome/m156)
fn battleOp51(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42044d64), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x427bf9ef), f32::from_bits(0xc27d72ab)),
        (f32::from_bits(0x42984d42), f32::from_bits(0xc2041029)),
    );
    path.line_to((f32::from_bits(0x425c3202), f32::from_bits(0xc1beef44)));
    path.cubic_to(
        (f32::from_bits(0x423626cb), f32::from_bits(0xc2373722)),
        (f32::from_bits(0x41bf47cb), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42984d42), f32::from_bits(0xc2041029)));
    path.cubic_to(
        (f32::from_bits(0x429adc06), f32::from_bits(0xc1f08771)),
        (f32::from_bits(0x429d127e), f32::from_bits(0xc1d85b80)),
        (f32::from_bits(0x429eedcc), f32::from_bits(0xc1bfbbc5)),
    );
    path.line_to((f32::from_bits(0x4265c6d6), f32::from_bits(0xc18a9a3f)));
    path.cubic_to(
        (f32::from_bits(0x426317a7), f32::from_bits(0xc19c6729)),
        (f32::from_bits(0x425fe4aa), f32::from_bits(0xc1ade05f)),
        (f32::from_bits(0x425c3203), f32::from_bits(0xc1beef45)),
    );
    path.line_to((f32::from_bits(0x42984d42), f32::from_bits(0xc2041029)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1442-L1468 (chrome/m156)
fn battleOp52(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42044d64), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x427bf9ef), f32::from_bits(0xc27d72ab)),
        (f32::from_bits(0x42984d42), f32::from_bits(0xc2041029)),
    );
    path.cubic_to(
        (f32::from_bits(0x429adc06), f32::from_bits(0xc1f08771)),
        (f32::from_bits(0x429d127e), f32::from_bits(0xc1d85b80)),
        (f32::from_bits(0x429eedcc), f32::from_bits(0xc1bfbbc5)),
    );
    path.line_to((f32::from_bits(0x4265c6d6), f32::from_bits(0xc18a9a3f)));
    path.cubic_to(
        (f32::from_bits(0x426317a7), f32::from_bits(0xc19c6729)),
        (f32::from_bits(0x425fe4aa), f32::from_bits(0xc1ade05f)),
        (f32::from_bits(0x425c3202), f32::from_bits(0xc1beef44)),
    );
    path.cubic_to(
        (f32::from_bits(0x423626cb), f32::from_bits(0xc2373722)),
        (f32::from_bits(0x41bf47cb), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429eedcc), f32::from_bits(0xc1bfbbc6)));
    path.cubic_to(
        (f32::from_bits(0x42ae408c), f32::from_bits(0x3fb7daeb)),
        (f32::from_bits(0x42a45c89), f32::from_bits(0x41e7c57e)),
        (f32::from_bits(0x42845101), f32::from_bits(0x42487bac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42488af1), f32::from_bits(0x428e8a4c)),
        (f32::from_bits(0x41c7bd0e), f32::from_bits(0x42a6f806)),
        (f32::from_bits(0xbfc7d871), f32::from_bits(0x42a5f87b)),
    );
    path.line_to((f32::from_bits(0xbf90777c), f32::from_bits(0x426ff521)));
    path.cubic_to(
        (f32::from_bits(0x419063a9), f32::from_bits(0x42716698)),
        (f32::from_bits(0x4210f87e), f32::from_bits(0x424e1511)),
        (f32::from_bits(0x423f4d05), f32::from_bits(0x4210ed75)),
    );
    path.cubic_to(
        (f32::from_bits(0x426da18c), f32::from_bits(0x41a78bb1)),
        (f32::from_bits(0x427bee4d), f32::from_bits(0x3f84e856)),
        (f32::from_bits(0x4265c6d8), f32::from_bits(0xc18a9a40)),
    );
    path.line_to((f32::from_bits(0x429eedcc), f32::from_bits(0xc1bfbbc6)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1471-L1493 (chrome/m156)
fn battleOp53(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x421216db), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4289817d), f32::from_bits(0xc26c814f)),
        (f32::from_bits(0x429ecb3a), f32::from_bits(0xc1c183ed)),
    );
    path.line_to((f32::from_bits(0x426594dc), f32::from_bits(0xc18be3fc)));
    path.cubic_to(
        (f32::from_bits(0x4246cdba), f32::from_bits(0xc22af7b1)),
        (f32::from_bits(0x41d336a3), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429ecb3a), f32::from_bits(0xc1c183e9)));
    path.cubic_to(
        (f32::from_bits(0x42a0d9cb), f32::from_bits(0xc1a68281)),
        (f32::from_bits(0x42a27999), f32::from_bits(0xc18b01ce)),
        (f32::from_bits(0x42a3a81d), f32::from_bits(0xc15e595d)),
    );
    path.line_to((f32::from_bits(0x426c9cb2), f32::from_bits(0xc120bbfa)));
    path.cubic_to(
        (f32::from_bits(0x426ae754), f32::from_bits(0xc148f95c)),
        (f32::from_bits(0x42688e2a), f32::from_bits(0xc170bcb0)),
        (f32::from_bits(0x426594dd), f32::from_bits(0xc18be3fd)),
    );
    path.line_to((f32::from_bits(0x429ecb3a), f32::from_bits(0xc1c183e9)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1496-L1523 (chrome/m156)
fn battleOp54(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x421216db), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4289817d), f32::from_bits(0xc26c814f)),
        (f32::from_bits(0x429ecb3a), f32::from_bits(0xc1c183ed)),
    );
    path.line_to((f32::from_bits(0x42a3a81d), f32::from_bits(0xc15e595d)));
    path.line_to((f32::from_bits(0x426c9cb2), f32::from_bits(0xc120bbfa)));
    path.cubic_to(
        (f32::from_bits(0x426ae754), f32::from_bits(0xc148f95c)),
        (f32::from_bits(0x42688e2a), f32::from_bits(0xc170bcb0)),
        (f32::from_bits(0x426594dd), f32::from_bits(0xc18be3fd)),
    );
    path.line_to((f32::from_bits(0x426594dc), f32::from_bits(0xc18be3fc)));
    path.cubic_to(
        (f32::from_bits(0x4246cdba), f32::from_bits(0xc22af7b1)),
        (f32::from_bits(0x41d336a3), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a3a81d), f32::from_bits(0xc15e595e)));
    path.cubic_to(
        (f32::from_bits(0x42ad725e), f32::from_bits(0x416ed313)),
        (f32::from_bits(0x42982fa2), f32::from_bits(0x4230cc44)),
        (f32::from_bits(0x42575fca), f32::from_bits(0x427ca963)),
    );
    path.cubic_to(
        (f32::from_bits(0x41fcc0a1), f32::from_bits(0x42a44341)),
        (f32::from_bits(0x3f80ed4e), f32::from_bits(0x42affc4e)),
        (f32::from_bits(0xc1d56b7f), f32::from_bits(0x429d3115)),
    );
    path.line_to((f32::from_bits(0xc19a478e), f32::from_bits(0x426343e2)));
    path.cubic_to(
        (f32::from_bits(0x3f3a6666), f32::from_bits(0x427e6fe0)),
        (f32::from_bits(0x41b6b66f), f32::from_bits(0x426d7d04)),
        (f32::from_bits(0x421bb135), f32::from_bits(0x4236a5a5)),
    );
    path.cubic_to(
        (f32::from_bits(0x425c0733), f32::from_bits(0x41ff9c8c)),
        (f32::from_bits(0x427ac435), f32::from_bits(0x412ca4f2)),
        (f32::from_bits(0x426c9cb3), f32::from_bits(0xc120bbf8)),
    );
    path.line_to((f32::from_bits(0x42a3a81d), f32::from_bits(0xc15e595e)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1526-L1548 (chrome/m156)
fn battleOp55(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4220aa02), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42952310), f32::from_bits(0xc258f48d)),
        (f32::from_bits(0x42a35f68), f32::from_bits(0xc16b5614)),
    );
    path.line_to((f32::from_bits(0x426c3395), f32::from_bits(0xc12a1f61)));
    path.cubic_to(
        (f32::from_bits(0x42579ea8), f32::from_bits(0xc21cd5ce)),
        (f32::from_bits(0x41e84916), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a35f69), f32::from_bits(0xc16b5613)));
    path.cubic_to(
        (f32::from_bits(0x42a4bd24), f32::from_bits(0xc12ea3c2)),
        (f32::from_bits(0x42a59325), f32::from_bits(0xc0e282d6)),
        (f32::from_bits(0x42a5dfdf), f32::from_bits(0xc04e84a0)),
    );
    path.line_to((f32::from_bits(0x426fd18d), f32::from_bits(0xc0154a48)));
    path.cubic_to(
        (f32::from_bits(0x426f62a1), f32::from_bits(0xc0a3be33)),
        (f32::from_bits(0x426e2d39), f32::from_bits(0xc0fc7dbb)),
        (f32::from_bits(0x426c3397), f32::from_bits(0xc12a1f63)),
    );
    path.line_to((f32::from_bits(0x42a35f69), f32::from_bits(0xc16b5613)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1551-L1578 (chrome/m156)
fn battleOp56(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4220aa02), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42952310), f32::from_bits(0xc258f48d)),
        (f32::from_bits(0x42a35f69), f32::from_bits(0xc16b5613)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a4bd24), f32::from_bits(0xc12ea3c2)),
        (f32::from_bits(0x42a59325), f32::from_bits(0xc0e282d6)),
        (f32::from_bits(0x42a5dfdf), f32::from_bits(0xc04e84a0)),
    );
    path.line_to((f32::from_bits(0x426fd18d), f32::from_bits(0xc0154a48)));
    path.cubic_to(
        (f32::from_bits(0x426f62a1), f32::from_bits(0xc0a3be33)),
        (f32::from_bits(0x426e2d39), f32::from_bits(0xc0fc7dbb)),
        (f32::from_bits(0x426c3397), f32::from_bits(0xc12a1f63)),
    );
    path.line_to((f32::from_bits(0x426c3395), f32::from_bits(0xc12a1f61)));
    path.cubic_to(
        (f32::from_bits(0x42579ea8), f32::from_bits(0xc21cd5ce)),
        (f32::from_bits(0x41e84916), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a5dfdf), f32::from_bits(0xc04e84a0)));
    path.cubic_to(
        (f32::from_bits(0x42a85e4f), f32::from_bits(0x41e6959e)),
        (f32::from_bits(0x4285b4e3), f32::from_bits(0x426ae44f)),
        (f32::from_bits(0x4219b105), f32::from_bits(0x42932450)),
    );
    path.cubic_to(
        (f32::from_bits(0x411fe111), f32::from_bits(0x42b0d679)),
        (f32::from_bits(0xc1c3966b), f32::from_bits(0x42ab1d42)),
        (f32::from_bits(0xc2482755), f32::from_bits(0x428470e8)),
    );
    path.line_to((f32::from_bits(0xc210b07c), f32::from_bits(0x423f7b24)));
    path.cubic_to(
        (f32::from_bits(0xc18d6382), f32::from_bits(0x427764e8)),
        (f32::from_bits(0x40e72680), f32::from_bits(0x427fab4e)),
        (f32::from_bits(0x41de345e), f32::from_bits(0x4254bc3b)),
    );
    path.cubic_to(
        (f32::from_bits(0x42414f8e), f32::from_bits(0x4229cd28)),
        (f32::from_bits(0x42736c9d), f32::from_bits(0x41a6b008)),
        (f32::from_bits(0x426fd18e), f32::from_bits(0xc0154a3f)),
    );
    path.line_to((f32::from_bits(0x42a5dfdf), f32::from_bits(0xc04e84a0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1581-L1603 (chrome/m156)
fn battleOp57(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x422b8e0b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x429d6dbc), f32::from_bits(0xc2494bad)),
        (f32::from_bits(0x42a54cb6), f32::from_bits(0xc0f3b760)),
    );
    path.line_to((f32::from_bits(0x426efcca), f32::from_bits(0xc0b02e2c)));
    path.cubic_to(
        (f32::from_bits(0x42639b94), f32::from_bits(0xc21183d2)),
        (f32::from_bits(0x41f807f9), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a54cb7), f32::from_bits(0xc0f3b757)));
    path.cubic_to(
        (f32::from_bits(0x42a60d08), f32::from_bits(0xc0628d9e)),
        (f32::from_bits(0x42a632b1), f32::from_bits(0x3f0efcd8)),
        (f32::from_bits(0x42a5bd61), f32::from_bits(0x4094a90a)),
    );
    path.line_to((f32::from_bits(0x426f9faf), f32::from_bits(0x4056ee3d)));
    path.cubic_to(
        (f32::from_bits(0x42704949), f32::from_bits(0x3ecebaba)),
        (f32::from_bits(0x427012d8), f32::from_bits(0xc023c5fe)),
        (f32::from_bits(0x426efccb), f32::from_bits(0xc0b02e2d)),
    );
    path.line_to((f32::from_bits(0x42a54cb7), f32::from_bits(0xc0f3b757)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1606-L1632 (chrome/m156)
fn battleOp58(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb630015b), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x422b8e0b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x429d6dbc), f32::from_bits(0xc2494bad)),
        (f32::from_bits(0x42a54cb7), f32::from_bits(0xc0f3b757)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a60d08), f32::from_bits(0xc0628d9e)),
        (f32::from_bits(0x42a632b1), f32::from_bits(0x3f0efcd8)),
        (f32::from_bits(0x42a5bd61), f32::from_bits(0x4094a90a)),
    );
    path.line_to((f32::from_bits(0x426f9faf), f32::from_bits(0x4056ee3d)));
    path.cubic_to(
        (f32::from_bits(0x42704949), f32::from_bits(0x3ecebaba)),
        (f32::from_bits(0x427012d8), f32::from_bits(0xc023c5fe)),
        (f32::from_bits(0x426efcca), f32::from_bits(0xc0b02e2c)),
    );
    path.cubic_to(
        (f32::from_bits(0x42639b94), f32::from_bits(0xc21183d2)),
        (f32::from_bits(0x41f807f9), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a5bd62), f32::from_bits(0x4094a90c)));
    path.cubic_to(
        (f32::from_bits(0x42a1e9d4), f32::from_bits(0x421b17cd)),
        (f32::from_bits(0x426944f3), f32::from_bits(0x428879ea)),
        (f32::from_bits(0x41ceac14), f32::from_bits(0x429dc116)),
    );
    path.cubic_to(
        (f32::from_bits(0xc0d4c6f5), f32::from_bits(0x42b30843)),
        (f32::from_bits(0xc2295516), f32::from_bits(0x429e4e8b)),
        (f32::from_bits(0xc2802142), f32::from_bits(0x4253148e)),
    );
    path.line_to((f32::from_bits(0xc2393f81), f32::from_bits(0x42189693)));
    path.cubic_to(
        (f32::from_bits(0xc1f4d162), f32::from_bits(0x4264e09b)),
        (f32::from_bits(0xc099d099), f32::from_bits(0x42816bc3)),
        (f32::from_bits(0x419566d0), f32::from_bits(0x42641418)),
    );
    path.cubic_to(
        (f32::from_bits(0x4228a0e3), f32::from_bits(0x424550a9)),
        (f32::from_bits(0x426a177b), f32::from_bits(0x41e03b19)),
        (f32::from_bits(0x426f9fb0), f32::from_bits(0x4056ee3a)),
    );
    path.line_to((f32::from_bits(0x42a5bd62), f32::from_bits(0x4094a90c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1635-L1657 (chrome/m156)
fn battleOp59(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x423693bc), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42a57249), f32::from_bits(0xc2389374)),
        (f32::from_bits(0x42a5ff3a), f32::from_bits(0xbf002494)),
    );
    path.line_to((f32::from_bits(0x426ffee2), f32::from_bits(0xbeb944c3)));
    path.cubic_to(
        (f32::from_bits(0x426f331d), f32::from_bits(0xc2056daf)),
        (f32::from_bits(0x4203fbc4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb560056c), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a5ff3a), f32::from_bits(0xbf0024e6)));
    path.cubic_to(
        (f32::from_bits(0x42a60c9b), f32::from_bits(0x40752b0d)),
        (f32::from_bits(0x42a56c5d), f32::from_bits(0x410284fd)),
        (f32::from_bits(0x42a41ffb), f32::from_bits(0x414709fb)),
    );
    path.line_to((f32::from_bits(0x426d49ff), f32::from_bits(0x410fe233)));
    path.cubic_to(
        (f32::from_bits(0x426f2a8e), f32::from_bits(0x40bcb3f0)),
        (f32::from_bits(0x42701239), f32::from_bits(0x40313ae3)),
        (f32::from_bits(0x426ffee3), f32::from_bits(0xbeb944c6)),
    );
    path.line_to((f32::from_bits(0x42a5ff3a), f32::from_bits(0xbf0024e6)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1659-L1681 (chrome/m156)
fn battleOp60(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e9334c2), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3f13342a), f32::from_bits(0xc2a5ff3c)),
        (f32::from_bits(0x3f5ccd0d), f32::from_bits(0xc2a5fdb4)),
    );
    path.line_to((f32::from_bits(0x3f1f9d85), f32::from_bits(0xc26ffcaf)));
    path.cubic_to(
        (f32::from_bits(0x3ed4d324), f32::from_bits(0xc26ffee7)),
        (f32::from_bits(0x3e54d404), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b23f68), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f5ccd1a), f32::from_bits(0xc2a5fdb5)));
    path.cubic_to(
        (f32::from_bits(0x3f642956), f32::from_bits(0xc2a5fd8c)),
        (f32::from_bits(0x3f6b855d), f32::from_bits(0xc2a5fd63)),
        (f32::from_bits(0x3f72e163), f32::from_bits(0xc2a5fd38)),
    );
    path.line_to((f32::from_bits(0x3f2f9381), f32::from_bits(0xc26ffbfc)));
    path.cubic_to(
        (f32::from_bits(0x3f2a4188), f32::from_bits(0xc26ffc3b)),
        (f32::from_bits(0x3f24ef95), f32::from_bits(0xc26ffc76)),
        (f32::from_bits(0x3f1f9da0), f32::from_bits(0xc26ffcb0)),
    );
    path.line_to((f32::from_bits(0x3f5ccd1a), f32::from_bits(0xc2a5fdb5)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1684-L1708 (chrome/m156)
fn battleOp61(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x36b23f68), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e9334c2), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3f13342a), f32::from_bits(0xc2a5ff3c)),
        (f32::from_bits(0x3f5ccd1a), f32::from_bits(0xc2a5fdb5)),
    );
    path.cubic_to(
        (f32::from_bits(0x3f642956), f32::from_bits(0xc2a5fd8c)),
        (f32::from_bits(0x3f6b855d), f32::from_bits(0xc2a5fd63)),
        (f32::from_bits(0x3f72e163), f32::from_bits(0xc2a5fd38)),
    );
    path.line_to((f32::from_bits(0x3f2f9381), f32::from_bits(0xc26ffbfc)));
    path.cubic_to(
        (f32::from_bits(0x3f2a4188), f32::from_bits(0xc26ffc3b)),
        (f32::from_bits(0x3f24ef95), f32::from_bits(0xc26ffc76)),
        (f32::from_bits(0x3f1f9d85), f32::from_bits(0xc26ffcaf)),
    );
    path.cubic_to(
        (f32::from_bits(0x3ed4d324), f32::from_bits(0xc26ffee7)),
        (f32::from_bits(0x3e54d404), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b23f68), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f72e162), f32::from_bits(0xc2a5fd39)));
    path.cubic_to(
        (f32::from_bits(0x3fb51288), f32::from_bits(0xc2a5fa80)),
        (f32::from_bits(0x3ff0b297), f32::from_bits(0xc2a5f5c4)),
        (f32::from_bits(0x401627a5), f32::from_bits(0xc2a5ef06)),
    );
    path.line_to((f32::from_bits(0x3fd9177b), f32::from_bits(0xc26fe773)));
    path.cubic_to(
        (f32::from_bits(0x3fadff90), f32::from_bits(0xc26ff134)),
        (f32::from_bits(0x3f82e54e), f32::from_bits(0xc26ff80c)),
        (f32::from_bits(0x3f2f9393), f32::from_bits(0xc26ffbfc)),
    );
    path.line_to((f32::from_bits(0x3f72e162), f32::from_bits(0xc2a5fd39)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1711-L1733 (chrome/m156)
fn battleOp62(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f614848), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3fe14683), f32::from_bits(0xc2a5f8d5)),
        (f32::from_bits(0x4028ee0f), f32::from_bits(0xc2a5ea81)),
    );
    path.line_to((f32::from_bits(0x3ff43c76), f32::from_bits(0xc26fe0ec)));
    path.cubic_to(
        (f32::from_bits(0x3fa2d98a), f32::from_bits(0xc26ff5a4)),
        (f32::from_bits(0x3f22dad5), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb5420574), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4028ee15), f32::from_bits(0xc2a5ea81)));
    path.cubic_to(
        (f32::from_bits(0x402e8f25), f32::from_bits(0xc2a5e912)),
        (f32::from_bits(0x40343026), f32::from_bits(0xc2a5e791)),
        (f32::from_bits(0x4039d111), f32::from_bits(0xc2a5e5fd)),
    );
    path.line_to((f32::from_bits(0x4006533c), f32::from_bits(0xc26fda66)));
    path.cubic_to(
        (f32::from_bits(0x4002419e), f32::from_bits(0xc26fdcaf)),
        (f32::from_bits(0x3ffc5fdb), f32::from_bits(0xc26fdedc)),
        (f32::from_bits(0x3ff43c61), f32::from_bits(0xc26fe0ed)),
    );
    path.line_to((f32::from_bits(0x4028ee15), f32::from_bits(0xc2a5ea81)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1736-L1765 (chrome/m156)
fn battleOp63(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f614848), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3fe14683), f32::from_bits(0xc2a5f8d5)),
        (f32::from_bits(0x4028ee15), f32::from_bits(0xc2a5ea81)),
    );
    path.cubic_to(
        (f32::from_bits(0x402e8f25), f32::from_bits(0xc2a5e912)),
        (f32::from_bits(0x40343026), f32::from_bits(0xc2a5e791)),
        (f32::from_bits(0x4039d111), f32::from_bits(0xc2a5e5fd)),
    );
    path.line_to((f32::from_bits(0x4006533c), f32::from_bits(0xc26fda66)));
    path.cubic_to(
        (f32::from_bits(0x400241a2), f32::from_bits(0xc26fdcaf)),
        (f32::from_bits(0x3ffc5fea), f32::from_bits(0xc26fdedc)),
        (f32::from_bits(0x3ff43c76), f32::from_bits(0xc26fe0ec)),
    );
    path.cubic_to(
        (f32::from_bits(0x3fa2d98a), f32::from_bits(0xc26ff5a4)),
        (f32::from_bits(0x3f22dad5), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    path.move_to((f32::from_bits(0x40186abb), f32::from_bits(0xc295b297)));
    path.line_to((f32::from_bits(0x3ff43c61), f32::from_bits(0xc26fe0ed)));
    path.line_to((f32::from_bits(0x3ff43c77), f32::from_bits(0xc26fe0ed)));
    path.line_to((f32::from_bits(0x40186abb), f32::from_bits(0xc295b297)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4039d102), f32::from_bits(0xc2a5e5fe)));
    path.cubic_to(
        (f32::from_bits(0x408a83ff), f32::from_bits(0xc2a5cc72)),
        (f32::from_bits(0x40b8130f), f32::from_bits(0xc2a5a01a)),
        (f32::from_bits(0x40e58a06), f32::from_bits(0xc2a56100)),
    );
    path.line_to((f32::from_bits(0x40a5ee90), f32::from_bits(0xc26f1a20)));
    path.cubic_to(
        (f32::from_bits(0x408510de), f32::from_bits(0xc26f755e)),
        (f32::from_bits(0x40484386), f32::from_bits(0xc26fb57a)),
        (f32::from_bits(0x40065347), f32::from_bits(0xc26fda68)),
    );
    path.line_to((f32::from_bits(0x4039d102), f32::from_bits(0xc2a5e5fe)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1768-L1790 (chrome/m156)
fn battleOp64(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3faf587e), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x402f5505), f32::from_bits(0xc2a5eea1)),
        (f32::from_bits(0x408372de), f32::from_bits(0xc2a5cbeb)),
    );
    path.line_to((f32::from_bits(0x403e0bd0), f32::from_bits(0xc26fb4b6)));
    path.cubic_to(
        (f32::from_bits(0x3ffd7de6), f32::from_bits(0xc26fe6e6)),
        (f32::from_bits(0x3f7d82fb), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x363f7eb2), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x408372d6), f32::from_bits(0xc2a5cbec)));
    path.cubic_to(
        (f32::from_bits(0x4087d39d), f32::from_bits(0xc2a5c874)),
        (f32::from_bits(0x408c3440), f32::from_bits(0xc2a5c4cf)),
        (f32::from_bits(0x409094bd), f32::from_bits(0xc2a5c0fe)),
    );
    path.line_to((f32::from_bits(0x40510866), f32::from_bits(0xc26fa4e7)));
    path.cubic_to(
        (f32::from_bits(0x404ab468), f32::from_bits(0xc26faa6c)),
        (f32::from_bits(0x40446037), f32::from_bits(0xc26fafb2)),
        (f32::from_bits(0x403e0bd2), f32::from_bits(0xc26fb4b7)),
    );
    path.line_to((f32::from_bits(0x408372d6), f32::from_bits(0xc2a5cbec)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1793-L1818 (chrome/m156)
fn battleOp65(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x363f7eb2), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3faf5872), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x402f54f9), f32::from_bits(0xc2a5eea1)),
        (f32::from_bits(0x408372d5), f32::from_bits(0xc2a5cbeb)),
    );
    path.line_to((f32::from_bits(0x408372d6), f32::from_bits(0xc2a5cbec)));
    path.cubic_to(
        (f32::from_bits(0x4087d39d), f32::from_bits(0xc2a5c874)),
        (f32::from_bits(0x408c3440), f32::from_bits(0xc2a5c4cf)),
        (f32::from_bits(0x409094bd), f32::from_bits(0xc2a5c0fe)),
    );
    path.line_to((f32::from_bits(0x40510866), f32::from_bits(0xc26fa4e7)));
    path.cubic_to(
        (f32::from_bits(0x404ab468), f32::from_bits(0xc26faa6c)),
        (f32::from_bits(0x40446037), f32::from_bits(0xc26fafb2)),
        (f32::from_bits(0x403e0bd0), f32::from_bits(0xc26fb4b6)),
    );
    path.cubic_to(
        (f32::from_bits(0x3ffd7de6), f32::from_bits(0xc26fe6e6)),
        (f32::from_bits(0x3f7d82fb), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x363f7eb2), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x409094be), f32::from_bits(0xc2a5c0fe)));
    path.cubic_to(
        (f32::from_bits(0x40d784bb), f32::from_bits(0xc2a5831d)),
        (f32::from_bits(0x410f22d3), f32::from_bits(0xc2a517ba)),
        (f32::from_bits(0x413255ec), f32::from_bits(0xc2a47f15)),
    );
    path.line_to((f32::from_bits(0x4100ead4), f32::from_bits(0xc26dd37e)));
    path.cubic_to(
        (f32::from_bits(0x40cef193), f32::from_bits(0xc26eb02f)),
        (f32::from_bits(0x409bcbdf), f32::from_bits(0xc26f4b72)),
        (f32::from_bits(0x40510859), f32::from_bits(0xc26fa4e8)),
    );
    path.line_to((f32::from_bits(0x409094be), f32::from_bits(0xc2a5c0fe)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1821-L1843 (chrome/m156)
fn battleOp66(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4037e518), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40b7d534), f32::from_bits(0xc2a5b39a)),
        (f32::from_bits(0x4109a47d), f32::from_bits(0xc2a51b1f)),
    );
    path.line_to((f32::from_bits(0x40c70051), f32::from_bits(0xc26eb519)));
    path.cubic_to(
        (f32::from_bits(0x4084e427), f32::from_bits(0xc26f918c)),
        (f32::from_bits(0x4004efa4), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3543fa8c), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4109a47c), f32::from_bits(0xc2a51b20)));
    path.cubic_to(
        (f32::from_bits(0x410e36d1), f32::from_bits(0xc2a50be2)),
        (f32::from_bits(0x4112c883), f32::from_bits(0xc2a4fbe1)),
        (f32::from_bits(0x41175985), f32::from_bits(0xc2a4eb1d)),
    );
    path.line_to((f32::from_bits(0x40dad196), f32::from_bits(0xc26e6faf)));
    path.cubic_to(
        (f32::from_bits(0x40d4377d), f32::from_bits(0xc26e87ed)),
        (f32::from_bits(0x40cd9c5c), f32::from_bits(0xc26e9f10)),
        (f32::from_bits(0x40c7004e), f32::from_bits(0xc26eb51a)),
    );
    path.line_to((f32::from_bits(0x4109a47c), f32::from_bits(0xc2a51b20)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1846-L1871 (chrome/m156)
fn battleOp67(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4037e518), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40b7d534), f32::from_bits(0xc2a5b39a)),
        (f32::from_bits(0x4109a47c), f32::from_bits(0xc2a51b20)),
    );
    path.cubic_to(
        (f32::from_bits(0x410e36d1), f32::from_bits(0xc2a50be2)),
        (f32::from_bits(0x4112c883), f32::from_bits(0xc2a4fbe1)),
        (f32::from_bits(0x41175985), f32::from_bits(0xc2a4eb1d)),
    );
    path.line_to((f32::from_bits(0x40dad196), f32::from_bits(0xc26e6faf)));
    path.cubic_to(
        (f32::from_bits(0x40d4377e), f32::from_bits(0xc26e87ed)),
        (f32::from_bits(0x40cd9c5f), f32::from_bits(0xc26e9f10)),
        (f32::from_bits(0x40c70052), f32::from_bits(0xc26eb51a)),
    );
    path.line_to((f32::from_bits(0x40c70051), f32::from_bits(0xc26eb519)));
    path.cubic_to(
        (f32::from_bits(0x4084e427), f32::from_bits(0xc26f918c)),
        (f32::from_bits(0x4004efa4), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4117597f), f32::from_bits(0xc2a4eb1d)));
    path.cubic_to(
        (f32::from_bits(0x41616445), f32::from_bits(0xc2a3db51)),
        (f32::from_bits(0x41954b2d), f32::from_bits(0xc2a2048b)),
        (f32::from_bits(0x41b914a4), f32::from_bits(0xc29f6bcb)),
    );
    path.line_to((f32::from_bits(0x4185cb10), f32::from_bits(0xc2667d00)));
    path.cubic_to(
        (f32::from_bits(0x4157d8a2), f32::from_bits(0xc26a3e17)),
        (f32::from_bits(0x4122ef07), f32::from_bits(0xc26ce6b9)),
        (f32::from_bits(0x40dad195), f32::from_bits(0xc26e6faf)),
    );
    path.line_to((f32::from_bits(0x4117597f), f32::from_bits(0xc2a4eb1d)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1873-L1895 (chrome/m156)
fn battleOp68(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e1b2207), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3e9b2105), f32::from_bits(0xc2a5ffca)),
        (f32::from_bits(0x3ee8b0c0), f32::from_bits(0xc2a5ff5d)),
    );
    path.line_to((f32::from_bits(0x3ea83563), f32::from_bits(0xc26fff14)));
    path.cubic_to(
        (f32::from_bits(0x3e60486a), f32::from_bits(0xc26fffb2)),
        (f32::from_bits(0x3de049e3), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b67768), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ee8b040), f32::from_bits(0xc2a5ff5d)));
    path.cubic_to(
        (f32::from_bits(0x3ef0720a), f32::from_bits(0xc2a5ff52)),
        (f32::from_bits(0x3ef83386), f32::from_bits(0xc2a5ff47)),
        (f32::from_bits(0x3efff501), f32::from_bits(0xc2a5ff3b)),
    );
    path.line_to((f32::from_bits(0x3eb90778), f32::from_bits(0xc26ffee3)));
    path.cubic_to(
        (f32::from_bits(0x3eb36c27), f32::from_bits(0xc26ffef6)),
        (f32::from_bits(0x3eadd0dd), f32::from_bits(0xc26fff07)),
        (f32::from_bits(0x3ea83592), f32::from_bits(0xc26fff16)),
    );
    path.line_to((f32::from_bits(0x3ee8b040), f32::from_bits(0xc2a5ff5d)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1898-L1923 (chrome/m156)
fn battleOp69(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x36b67768), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e1b21b2), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3e9b20b0), f32::from_bits(0xc2a5ffca)),
        (f32::from_bits(0x3ee8b040), f32::from_bits(0xc2a5ff5d)),
    );
    path.cubic_to(
        (f32::from_bits(0x3ef0720a), f32::from_bits(0xc2a5ff52)),
        (f32::from_bits(0x3ef83386), f32::from_bits(0xc2a5ff47)),
        (f32::from_bits(0x3efff501), f32::from_bits(0xc2a5ff3b)),
    );
    path.line_to((f32::from_bits(0x3eb90778), f32::from_bits(0xc26ffee3)));
    path.line_to((f32::from_bits(0x3ea83592), f32::from_bits(0xc26fff16)));
    path.line_to((f32::from_bits(0x3ea83563), f32::from_bits(0xc26fff14)));
    path.cubic_to(
        (f32::from_bits(0x3e60486a), f32::from_bits(0xc26fffb2)),
        (f32::from_bits(0x3de049e3), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b67768), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3efff501), f32::from_bits(0xc2a5ff3b)));
    path.cubic_to(
        (f32::from_bits(0x3f3ed289), f32::from_bits(0xc2a5fe79)),
        (f32::from_bits(0x3f7daa5c), f32::from_bits(0xc2a5fd28)),
        (f32::from_bits(0x3f9e4099), f32::from_bits(0xc2a5fb49)),
    );
    path.line_to((f32::from_bits(0x3f64cc5f), f32::from_bits(0xc26ff92f)));
    path.cubic_to(
        (f32::from_bits(0x3f375f8f), f32::from_bits(0xc26ffbe5)),
        (f32::from_bits(0x3f09f1cf), f32::from_bits(0xc26ffdcc)),
        (f32::from_bits(0x3eb9075f), f32::from_bits(0xc26ffee4)),
    );
    path.line_to((f32::from_bits(0x3efff501), f32::from_bits(0xc2a5ff3b)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1926-L1948 (chrome/m156)
fn battleOp70(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f0938d2), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3f893841), f32::from_bits(0xc2a5fd56)),
        (f32::from_bits(0x3fcdd137), f32::from_bits(0xc2a5f805)),
    );
    path.line_to((f32::from_bits(0x3f94c89b), f32::from_bits(0xc26ff478)));
    path.cubic_to(
        (f32::from_bits(0x3f4663c1), f32::from_bits(0xc26ffc29)),
        (f32::from_bits(0x3ec6647d), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x360ebeb2), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3fcdd13c), f32::from_bits(0xc2a5f806)));
    path.cubic_to(
        (f32::from_bits(0x3fd4ad55), f32::from_bits(0xc2a5f77d)),
        (f32::from_bits(0x3fdb895f), f32::from_bits(0xc2a5f6ef)),
        (f32::from_bits(0x3fe26560), f32::from_bits(0xc2a5f659)),
    );
    path.line_to((f32::from_bits(0x3fa3a8ea), f32::from_bits(0xc26ff20c)));
    path.cubic_to(
        (f32::from_bits(0x3f9eb37e), f32::from_bits(0xc26ff2e6)),
        (f32::from_bits(0x3f99be11), f32::from_bits(0xc26ff3b4)),
        (f32::from_bits(0x3f94c89e), f32::from_bits(0xc26ff479)),
    );
    path.line_to((f32::from_bits(0x3fcdd13c), f32::from_bits(0xc2a5f806)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1951-L1975 (chrome/m156)
fn battleOp71(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x360ebeb2), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f0938d2), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3f893841), f32::from_bits(0xc2a5fd56)),
        (f32::from_bits(0x3fcdd13c), f32::from_bits(0xc2a5f806)),
    );
    path.cubic_to(
        (f32::from_bits(0x3fd4ad55), f32::from_bits(0xc2a5f77d)),
        (f32::from_bits(0x3fdb895f), f32::from_bits(0xc2a5f6ef)),
        (f32::from_bits(0x3fe26560), f32::from_bits(0xc2a5f659)),
    );
    path.line_to((f32::from_bits(0x3fa3a8ea), f32::from_bits(0xc26ff20c)));
    path.cubic_to(
        (f32::from_bits(0x3f9eb37e), f32::from_bits(0xc26ff2e6)),
        (f32::from_bits(0x3f99be11), f32::from_bits(0xc26ff3b4)),
        (f32::from_bits(0x3f94c89b), f32::from_bits(0xc26ff478)),
    );
    path.cubic_to(
        (f32::from_bits(0x3f4663c1), f32::from_bits(0xc26ffc29)),
        (f32::from_bits(0x3ec6647d), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x360ebeb2), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3fe26566), f32::from_bits(0xc2a5f65a)));
    path.cubic_to(
        (f32::from_bits(0x4028c729), f32::from_bits(0xc2a5ecdf)),
        (f32::from_bits(0x406055f2), f32::from_bits(0xc2a5dc6a)),
        (f32::from_bits(0x408beceb), f32::from_bits(0xc2a5c4fb)),
    );
    path.line_to((f32::from_bits(0x404a4d47), f32::from_bits(0xc26faaae)));
    path.cubic_to(
        (f32::from_bits(0x40222b9c), f32::from_bits(0xc26fcc90)),
        (f32::from_bits(0x3ff40427), f32::from_bits(0xc26fe45b)),
        (f32::from_bits(0x3fa3a8ee), f32::from_bits(0xc26ff20e)),
    );
    path.line_to((f32::from_bits(0x3fe26566), f32::from_bits(0xc2a5f65a)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L1978-L2000 (chrome/m156)
fn battleOp72(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f73aa4a), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3ff3a7f0), f32::from_bits(0xc2a5f79e)),
        (f32::from_bits(0x4036b54b), f32::from_bits(0xc2a5e6db)),
    );
    path.line_to((f32::from_bits(0x40041412), f32::from_bits(0xc26fdba5)));
    path.cubic_to(
        (f32::from_bits(0x3fb0230c), f32::from_bits(0xc26ff3e0)),
        (f32::from_bits(0x3f3024c1), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x359dfd4a), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4036b55d), f32::from_bits(0xc2a5e6db)));
    path.cubic_to(
        (f32::from_bits(0x403ccbdf), f32::from_bits(0xc2a5e52d)),
        (f32::from_bits(0x4042e24c), f32::from_bits(0xc2a5e36a)),
        (f32::from_bits(0x4048f89e), f32::from_bits(0xc2a5e192)),
    );
    path.line_to((f32::from_bits(0x401147bc), f32::from_bits(0xc26fd403)));
    path.cubic_to(
        (f32::from_bits(0x400ce144), f32::from_bits(0xc26fd6ae)),
        (f32::from_bits(0x40087ab2), f32::from_bits(0xc26fd939)),
        (f32::from_bits(0x4004140f), f32::from_bits(0xc26fdba5)),
    );
    path.line_to((f32::from_bits(0x4036b55d), f32::from_bits(0xc2a5e6db)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2003-L2025 (chrome/m156)
fn battleOp73(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40447e19), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40c46ab2), f32::from_bits(0xc2a5a8c7)),
        (f32::from_bits(0x4113078c), f32::from_bits(0xc2a4fabe)),
    );
    path.line_to((f32::from_bits(0x40d4929e), f32::from_bits(0xc26e8647)));
    path.cubic_to(
        (f32::from_bits(0x408dfcf1), f32::from_bits(0xc26f81e6)),
        (f32::from_bits(0x400e0af8), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3655fea5), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4113078b), f32::from_bits(0xc2a4fabe)));
    path.cubic_to(
        (f32::from_bits(0x4117e908), f32::from_bits(0xc2a4e957)),
        (f32::from_bits(0x411cc9c0), f32::from_bits(0xc2a4d714)),
        (f32::from_bits(0x4121a9a1), f32::from_bits(0xc2a4c3f3)),
    );
    path.line_to((f32::from_bits(0x40e9baad), f32::from_bits(0xc26e370e)));
    path.cubic_to(
        (f32::from_bits(0x40e2ae85), f32::from_bits(0xc26e52b6)),
        (f32::from_bits(0x40dba120), f32::from_bits(0xc26e6d20)),
        (f32::from_bits(0x40d4929a), f32::from_bits(0xc26e8647)),
    );
    path.line_to((f32::from_bits(0x4113078b), f32::from_bits(0xc2a4fabe)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2028-L2050 (chrome/m156)
fn battleOp74(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x406db78d), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x40ed953d), f32::from_bits(0xc2a58058)),
        (f32::from_bits(0x4131afb7), f32::from_bits(0xc2a481e4)),
    );
    path.line_to((f32::from_bits(0x410072b2), f32::from_bits(0xc26dd78e)));
    path.cubic_to(
        (f32::from_bits(0x40abbf2e), f32::from_bits(0xc26f4770)),
        (f32::from_bits(0x402bd807), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4131afba), f32::from_bits(0xc2a481e4)));
    path.cubic_to(
        (f32::from_bits(0x413792dd), f32::from_bits(0xc2a46874)),
        (f32::from_bits(0x413d74a2), f32::from_bits(0xc2a44dc1)),
        (f32::from_bits(0x414354e9), f32::from_bits(0xc2a431ca)),
    );
    path.line_to((f32::from_bits(0x410d3424), f32::from_bits(0xc26d63c0)));
    path.cubic_to(
        (f32::from_bits(0x4108f4b6), f32::from_bits(0xc26d8c2e)),
        (f32::from_bits(0x4104b435), f32::from_bits(0xc26db2c8)),
        (f32::from_bits(0x410072b4), f32::from_bits(0xc26dd78e)),
    );
    path.line_to((f32::from_bits(0x4131afba), f32::from_bits(0xc2a481e4)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2053-L2077 (chrome/m156)
fn battleOp75(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x406db78d), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x40ed953d), f32::from_bits(0xc2a58058)),
        (f32::from_bits(0x4131afba), f32::from_bits(0xc2a481e4)),
    );
    path.cubic_to(
        (f32::from_bits(0x413792dd), f32::from_bits(0xc2a46874)),
        (f32::from_bits(0x413d74a2), f32::from_bits(0xc2a44dc1)),
        (f32::from_bits(0x414354e9), f32::from_bits(0xc2a431ca)),
    );
    path.line_to((f32::from_bits(0x410d3424), f32::from_bits(0xc26d63c0)));
    path.cubic_to(
        (f32::from_bits(0x4108f4b6), f32::from_bits(0xc26d8c2e)),
        (f32::from_bits(0x4104b435), f32::from_bits(0xc26db2c8)),
        (f32::from_bits(0x410072b2), f32::from_bits(0xc26dd78e)),
    );
    path.cubic_to(
        (f32::from_bits(0x40abbf2e), f32::from_bits(0xc26f4770)),
        (f32::from_bits(0x402bd807), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x414354ed), f32::from_bits(0xc2a431cb)));
    path.cubic_to(
        (f32::from_bits(0x419152e5), f32::from_bits(0xc2a26c3a)),
        (f32::from_bits(0x41c0119b), f32::from_bits(0xc29f5c06)),
        (f32::from_bits(0x41ed1335), f32::from_bits(0xc29b0f0a)),
    );
    path.line_to((f32::from_bits(0x41ab612b), f32::from_bits(0xc2602e6b)));
    path.cubic_to(
        (f32::from_bits(0x418ad84d), f32::from_bits(0xc2666635)),
        (f32::from_bits(0x41521b54), f32::from_bits(0xc26ad3fe)),
        (f32::from_bits(0x410d3426), f32::from_bits(0xc26d63c0)),
    );
    path.line_to((f32::from_bits(0x414354ed), f32::from_bits(0xc2a431cb)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2080-L2102 (chrome/m156)
fn battleOp76(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40932e58), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x41130dbc), f32::from_bits(0xc2a53c41)),
        (f32::from_bits(0x415ba178), f32::from_bits(0xc2a3b6ca)),
    );
    path.line_to((f32::from_bits(0x411ec4eb), f32::from_bits(0xc26cb1eb)));
    path.cubic_to(
        (f32::from_bits(0x40d49b93), f32::from_bits(0xc26ee4ff)),
        (f32::from_bits(0x4054cab9), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x35f7fd46), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x415ba178), f32::from_bits(0xc2a3b6cb)));
    path.cubic_to(
        (f32::from_bits(0x4162e261), f32::from_bits(0xc2a38fde)),
        (f32::from_bits(0x416a20aa), f32::from_bits(0xc2a36704)),
        (f32::from_bits(0x41715c23), f32::from_bits(0xc2a33c3e)),
    );
    path.line_to((f32::from_bits(0x412e7a25), f32::from_bits(0xc26c00bd)));
    path.cubic_to(
        (f32::from_bits(0x41293fb6), f32::from_bits(0xc26c3e94)),
        (f32::from_bits(0x41240342), f32::from_bits(0xc26c79a4)),
        (f32::from_bits(0x411ec4e8), f32::from_bits(0xc26cb1eb)),
    );
    path.line_to((f32::from_bits(0x415ba178), f32::from_bits(0xc2a3b6cb)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2105-L2127 (chrome/m156)
fn battleOp77(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40d0158a), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x414fb944), f32::from_bits(0xc2a478c0)),
        (f32::from_bits(0x419a74b5), f32::from_bits(0xc2a1724b)),
    );
    path.line_to((f32::from_bits(0x415f4f4c), f32::from_bits(0xc2696aa5)));
    path.cubic_to(
        (f32::from_bits(0x41162967), f32::from_bits(0xc26dca57)),
        (f32::from_bits(0x40966c1f), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3655fea3), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x419a74b6), f32::from_bits(0xc2a1724b)));
    path.cubic_to(
        (f32::from_bits(0x419f8274), f32::from_bits(0xc2a124ef)),
        (f32::from_bits(0x41a48c82), f32::from_bits(0xc2a0d3c9)),
        (f32::from_bits(0x41a9929f), f32::from_bits(0xc2a07edb)),
    );
    path.line_to((f32::from_bits(0x41752a58), f32::from_bits(0xc2680ab0)));
    path.cubic_to(
        (f32::from_bits(0x416de6e6), f32::from_bits(0xc268857b)),
        (f32::from_bits(0x41669dc0), f32::from_bits(0xc268facf)),
        (f32::from_bits(0x415f4f4b), f32::from_bits(0xc2696aa6)),
    );
    path.line_to((f32::from_bits(0x419a74b6), f32::from_bits(0xc2a1724b)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2130-L2154 (chrome/m156)
fn battleOp78(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3655fea3), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40d0158a), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x414fb944), f32::from_bits(0xc2a478c0)),
        (f32::from_bits(0x419a74b6), f32::from_bits(0xc2a1724b)),
    );
    path.cubic_to(
        (f32::from_bits(0x419f8274), f32::from_bits(0xc2a124ef)),
        (f32::from_bits(0x41a48c82), f32::from_bits(0xc2a0d3c9)),
        (f32::from_bits(0x41a9929f), f32::from_bits(0xc2a07edb)),
    );
    path.line_to((f32::from_bits(0x41752a58), f32::from_bits(0xc2680ab0)));
    path.cubic_to(
        (f32::from_bits(0x416de6e6), f32::from_bits(0xc268857b)),
        (f32::from_bits(0x41669dc0), f32::from_bits(0xc268facf)),
        (f32::from_bits(0x415f4f4c), f32::from_bits(0xc2696aa5)),
    );
    path.cubic_to(
        (f32::from_bits(0x41162967), f32::from_bits(0xc26dca57)),
        (f32::from_bits(0x40966c1f), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3655fea3), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41a9929f), f32::from_bits(0xc2a07edc)));
    path.cubic_to(
        (f32::from_bits(0x41fb3aee), f32::from_bits(0xc29b1a71)),
        (f32::from_bits(0x422402f4), f32::from_bits(0xc291ddaf)),
        (f32::from_bits(0x4245eaa6), f32::from_bits(0xc2854763)),
    );
    path.line_to((f32::from_bits(0x420f1280), f32::from_bits(0xc240b13c)));
    path.cubic_to(
        (f32::from_bits(0x41ed200b), f32::from_bits(0xc252e3f9)),
        (f32::from_bits(0x41b59cbb), f32::from_bits(0xc2603ee8)),
        (f32::from_bits(0x41752a58), f32::from_bits(0xc2680aaf)),
    );
    path.line_to((f32::from_bits(0x41a9929f), f32::from_bits(0xc2a07edc)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2157-L2179 (chrome/m156)
fn battleOp79(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4110a0cc), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4190247a), f32::from_bits(0xc2a30bfe)),
        (f32::from_bits(0x41d4a5dc), f32::from_bits(0xc29d41d4)),
    );
    path.line_to((f32::from_bits(0x4199b8a9), f32::from_bits(0xc2635c16)));
    path.cubic_to(
        (f32::from_bits(0x4150660f), f32::from_bits(0xc26bbaf8)),
        (f32::from_bits(0x40d119d0), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3673fea3), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41d4a5d9), f32::from_bits(0xc29d41d4)));
    path.cubic_to(
        (f32::from_bits(0x41db7bbd), f32::from_bits(0xc29cadef)),
        (f32::from_bits(0x41e247df), f32::from_bits(0xc29c12ec)),
        (f32::from_bits(0x41e9098d), f32::from_bits(0xc29b70d9)),
    );
    path.line_to((f32::from_bits(0x41a875f1), f32::from_bits(0xc260bbd5)));
    path.cubic_to(
        (f32::from_bits(0x41a39393), f32::from_bits(0xc261a627)),
        (f32::from_bits(0x419ea9a6), f32::from_bits(0xc2628645)),
        (f32::from_bits(0x4199b8ab), f32::from_bits(0xc2635c17)),
    );
    path.line_to((f32::from_bits(0x41d4a5d9), f32::from_bits(0xc29d41d4)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2181-L2203 (chrome/m156)
fn battleOp80(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e15a675), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3e95a67a), f32::from_bits(0xc2a5ffcd)),
        (f32::from_bits(0x3ee07980), f32::from_bits(0xc2a5ff68)),
    );
    path.line_to((f32::from_bits(0x3ea245bb), f32::from_bits(0xc26fff25)));
    path.cubic_to(
        (f32::from_bits(0x3e585de0), f32::from_bits(0xc26fffb9)),
        (f32::from_bits(0x3dd85f11), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3691e768), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ee07a10), f32::from_bits(0xc2a5ff68)));
    path.cubic_to(
        (f32::from_bits(0x3ee7f565), f32::from_bits(0xc2a5ff5d)),
        (f32::from_bits(0x3eef70d9), f32::from_bits(0xc2a5ff52)),
        (f32::from_bits(0x3ef6ec4d), f32::from_bits(0xc2a5ff47)),
    );
    path.line_to((f32::from_bits(0x3eb27fdb), f32::from_bits(0xc26ffef6)));
    path.cubic_to(
        (f32::from_bits(0x3ead1768), f32::from_bits(0xc26fff07)),
        (f32::from_bits(0x3ea7aebe), f32::from_bits(0xc26fff17)),
        (f32::from_bits(0x3ea24612), f32::from_bits(0xc26fff26)),
    );
    path.line_to((f32::from_bits(0x3ee07a10), f32::from_bits(0xc2a5ff68)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2206-L2230 (chrome/m156)
fn battleOp81(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3691e768), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e15a675), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3e95a67a), f32::from_bits(0xc2a5ffcd)),
        (f32::from_bits(0x3ee07a10), f32::from_bits(0xc2a5ff68)),
    );
    path.line_to((f32::from_bits(0x3ef6ec4d), f32::from_bits(0xc2a5ff47)));
    path.line_to((f32::from_bits(0x3eb27fdb), f32::from_bits(0xc26ffef6)));
    path.cubic_to(
        (f32::from_bits(0x3ead1768), f32::from_bits(0xc26fff07)),
        (f32::from_bits(0x3ea7aebe), f32::from_bits(0xc26fff17)),
        (f32::from_bits(0x3ea245bb), f32::from_bits(0xc26fff25)),
    );
    path.cubic_to(
        (f32::from_bits(0x3e585de0), f32::from_bits(0xc26fffb9)),
        (f32::from_bits(0x3dd85f11), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3691e768), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ef6ec9b), f32::from_bits(0xc2a5ff48)));
    path.cubic_to(
        (f32::from_bits(0x3f3816c9), f32::from_bits(0xc2a5fe94)),
        (f32::from_bits(0x3f74b6e1), f32::from_bits(0xc2a5fd5b)),
        (f32::from_bits(0x3f98ab0b), f32::from_bits(0xc2a5fb9d)),
    );
    path.line_to((f32::from_bits(0x3f5cb973), f32::from_bits(0xc26ff9a8)));
    path.cubic_to(
        (f32::from_bits(0x3f30e6e7), f32::from_bits(0xc26ffc2e)),
        (f32::from_bits(0x3f05138e), f32::from_bits(0xc26ffdf2)),
        (f32::from_bits(0x3eb27fc6), f32::from_bits(0xc26ffef7)),
    );
    path.line_to((f32::from_bits(0x3ef6ec9b), f32::from_bits(0xc2a5ff48)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2233-L2255 (chrome/m156)
fn battleOp82(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3eff98a5), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3f7f97b3), f32::from_bits(0xc2a5fdb1)),
        (f32::from_bits(0x3fbfaf38), f32::from_bits(0xc2a5f914)),
    );
    path.line_to((f32::from_bits(0x3f8a9112), f32::from_bits(0xc26ff600)));
    path.cubic_to(
        (f32::from_bits(0x3f38c3e7), f32::from_bits(0xc26ffcab)),
        (f32::from_bits(0x3eb8c475), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x35877d28), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3fbfaf15), f32::from_bits(0xc2a5f915)));
    path.cubic_to(
        (f32::from_bits(0x3fc612b4), f32::from_bits(0xc2a5f8a0)),
        (f32::from_bits(0x3fcc7634), f32::from_bits(0xc2a5f824)),
        (f32::from_bits(0x3fd2d9ad), f32::from_bits(0xc2a5f7a2)),
    );
    path.line_to((f32::from_bits(0x3f986bef), f32::from_bits(0xc26ff3e6)));
    path.cubic_to(
        (f32::from_bits(0x3f93cdb9), f32::from_bits(0xc26ff4a2)),
        (f32::from_bits(0x3f8f2f70), f32::from_bits(0xc26ff556)),
        (f32::from_bits(0x3f8a9121), f32::from_bits(0xc26ff601)),
    );
    path.line_to((f32::from_bits(0x3fbfaf15), f32::from_bits(0xc2a5f915)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2258-L2283 (chrome/m156)
fn battleOp83(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3eff9875), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3f7f9783), f32::from_bits(0xc2a5fdb1)),
        (f32::from_bits(0x3fbfaf14), f32::from_bits(0xc2a5f914)),
    );
    path.line_to((f32::from_bits(0x3fbfaf15), f32::from_bits(0xc2a5f915)));
    path.cubic_to(
        (f32::from_bits(0x3fc612b4), f32::from_bits(0xc2a5f8a0)),
        (f32::from_bits(0x3fcc7634), f32::from_bits(0xc2a5f824)),
        (f32::from_bits(0x3fd2d9ad), f32::from_bits(0xc2a5f7a2)),
    );
    path.line_to((f32::from_bits(0x3f986bef), f32::from_bits(0xc26ff3e6)));
    path.cubic_to(
        (f32::from_bits(0x3f93cdb9), f32::from_bits(0xc26ff4a2)),
        (f32::from_bits(0x3f8f2f70), f32::from_bits(0xc26ff556)),
        (f32::from_bits(0x3f8a9112), f32::from_bits(0xc26ff600)),
    );
    path.cubic_to(
        (f32::from_bits(0x3f38c3e7), f32::from_bits(0xc26ffcab)),
        (f32::from_bits(0x3eb8c475), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3fd2d994), f32::from_bits(0xc2a5f7a1)));
    path.cubic_to(
        (f32::from_bits(0x401d305c), f32::from_bits(0xc2a5ef69)),
        (f32::from_bits(0x4050ef71), f32::from_bits(0xc2a5e123)),
        (f32::from_bits(0x408252dc), f32::from_bits(0xc2a5ccd0)),
    );
    path.line_to((f32::from_bits(0x403c6b7d), f32::from_bits(0xc26fb5fe)));
    path.cubic_to(
        (f32::from_bits(0x401709a2), f32::from_bits(0xc26fd362)),
        (f32::from_bits(0x3fe342dd), f32::from_bits(0xc26fe805)),
        (f32::from_bits(0x3f986be0), f32::from_bits(0xc26ff3e7)),
    );
    path.line_to((f32::from_bits(0x3fd2d994), f32::from_bits(0xc2a5f7a1)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2286-L2308 (chrome/m156)
fn battleOp84(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f541e8b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3fd41d19), f32::from_bits(0xc2a5f9a6)),
        (f32::from_bits(0x401f1022), f32::from_bits(0xc2a5ecf2)),
    );
    path.line_to((f32::from_bits(0x3fe5f882), f32::from_bits(0xc26fe473)));
    path.cubic_to(
        (f32::from_bits(0x3f9955cf), f32::from_bits(0xc26ff6d2)),
        (f32::from_bits(0x3f1956dc), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb5bb02d8), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x401f1027), f32::from_bits(0xc2a5ecf2)));
    path.cubic_to(
        (f32::from_bits(0x40245d21), f32::from_bits(0xc2a5ebac)),
        (f32::from_bits(0x4029aa04), f32::from_bits(0xc2a5ea57)),
        (f32::from_bits(0x402ef6d6), f32::from_bits(0xc2a5e8f1)),
    );
    path.line_to((f32::from_bits(0x3ffcf5ba), f32::from_bits(0xc26fdeaa)));
    path.cubic_to(
        (f32::from_bits(0x3ff54c2d), f32::from_bits(0xc26fe0b0)),
        (f32::from_bits(0x3feda268), f32::from_bits(0xc26fe29e)),
        (f32::from_bits(0x3fe5f88e), f32::from_bits(0xc26fe474)),
    );
    path.line_to((f32::from_bits(0x401f1027), f32::from_bits(0xc2a5ecf2)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2311-L2335 (chrome/m156)
fn battleOp85(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f541e8b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3fd41d19), f32::from_bits(0xc2a5f9a6)),
        (f32::from_bits(0x401f1027), f32::from_bits(0xc2a5ecf2)),
    );
    path.cubic_to(
        (f32::from_bits(0x40245d21), f32::from_bits(0xc2a5ebac)),
        (f32::from_bits(0x4029aa04), f32::from_bits(0xc2a5ea57)),
        (f32::from_bits(0x402ef6d6), f32::from_bits(0xc2a5e8f1)),
    );
    path.line_to((f32::from_bits(0x3ffcf5ba), f32::from_bits(0xc26fdeaa)));
    path.cubic_to(
        (f32::from_bits(0x3ff54c2d), f32::from_bits(0xc26fe0b0)),
        (f32::from_bits(0x3feda268), f32::from_bits(0xc26fe29e)),
        (f32::from_bits(0x3fe5f882), f32::from_bits(0xc26fe473)),
    );
    path.cubic_to(
        (f32::from_bits(0x3f9955cf), f32::from_bits(0xc26ff6d2)),
        (f32::from_bits(0x3f1956dc), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x402ef6c3), f32::from_bits(0xc2a5e8f1)));
    path.cubic_to(
        (f32::from_bits(0x40826d68), f32::from_bits(0xc2a5d24c)),
        (f32::from_bits(0x40ad550a), f32::from_bits(0xc2a5aafb)),
        (f32::from_bits(0x40d82890), f32::from_bits(0xc2a57308)),
    );
    path.line_to((f32::from_bits(0x409c425c), f32::from_bits(0xc26f3430)));
    path.cubic_to(
        (f32::from_bits(0x407a99d8), f32::from_bits(0xc26f8515)),
        (f32::from_bits(0x403c91e6), f32::from_bits(0xc26fbded)),
        (f32::from_bits(0x3ffcf5ca), f32::from_bits(0xc26fdeaa)),
    );
    path.line_to((f32::from_bits(0x402ef6c3), f32::from_bits(0xc2a5e8f1)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2338-L2360 (chrome/m156)
fn battleOp86(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40155bee), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40955364), f32::from_bits(0xc2a5cd99)),
        (f32::from_bits(0x40dfbd5f), f32::from_bits(0xc2a568f2)),
    );
    path.line_to((f32::from_bits(0x40a1bd53), f32::from_bits(0xc26f259d)));
    path.cubic_to(
        (f32::from_bits(0x4057e483), f32::from_bits(0xc26fb724)),
        (f32::from_bits(0x3fd7f0d9), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3619fea3), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x40dfbd5e), f32::from_bits(0xc2a568f3)));
    path.cubic_to(
        (f32::from_bits(0x40e72e1b), f32::from_bits(0xc2a55ee2)),
        (f32::from_bits(0x40ee9e1c), f32::from_bits(0xc2a55452)),
        (f32::from_bits(0x40f60d62), f32::from_bits(0xc2a54941)),
    );
    path.line_to((f32::from_bits(0x40b1de84), f32::from_bits(0xc26ef7c9)));
    path.cubic_to(
        (f32::from_bits(0x40ac7ea0), f32::from_bits(0xc26f07cb)),
        (f32::from_bits(0x40a71e37), f32::from_bits(0xc26f1712)),
        (f32::from_bits(0x40a1bd4f), f32::from_bits(0xc26f259f)),
    );
    path.line_to((f32::from_bits(0x40dfbd5e), f32::from_bits(0xc2a568f3)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2363-L2388 (chrome/m156)
fn battleOp87(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3619fea3), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40155bee), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40955364), f32::from_bits(0xc2a5cd99)),
        (f32::from_bits(0x40dfbd5e), f32::from_bits(0xc2a568f3)),
    );
    path.cubic_to(
        (f32::from_bits(0x40e72e1b), f32::from_bits(0xc2a55ee2)),
        (f32::from_bits(0x40ee9e1c), f32::from_bits(0xc2a55452)),
        (f32::from_bits(0x40f60d62), f32::from_bits(0xc2a54941)),
    );
    path.line_to((f32::from_bits(0x40b1de84), f32::from_bits(0xc26ef7c9)));
    path.cubic_to(
        (f32::from_bits(0x40ac7ea2), f32::from_bits(0xc26f07cb)),
        (f32::from_bits(0x40a71e3a), f32::from_bits(0xc26f1712)),
        (f32::from_bits(0x40a1bd54), f32::from_bits(0xc26f259f)),
    );
    path.line_to((f32::from_bits(0x40a1bd53), f32::from_bits(0xc26f259d)));
    path.cubic_to(
        (f32::from_bits(0x4057e483), f32::from_bits(0xc26fb724)),
        (f32::from_bits(0x3fd7f0d9), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3619fea3), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x40f60d69), f32::from_bits(0xc2a54941)));
    path.cubic_to(
        (f32::from_bits(0x41374a21), f32::from_bits(0xc2a495d5)),
        (f32::from_bits(0x41731962), f32::from_bits(0xc2a35eca)),
        (f32::from_bits(0x419704b1), f32::from_bits(0xc2a1a64c)),
    );
    path.line_to((f32::from_bits(0x415a56f5), f32::from_bits(0xc269b5d4)));
    path.cubic_to(
        (f32::from_bits(0x412fbbfb), f32::from_bits(0xc26c32af)),
        (f32::from_bits(0x41047f9a), f32::from_bits(0xc26df463)),
        (f32::from_bits(0x40b1de7e), f32::from_bits(0xc26ef7cb)),
    );
    path.line_to((f32::from_bits(0x40f60d69), f32::from_bits(0xc2a54941)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2391-L2413 (chrome/m156)
fn battleOp88(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4059d383), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40d9b918), f32::from_bits(0xc2a594d0)),
        (f32::from_bits(0x4122e820), f32::from_bits(0xc2a4bf0c)),
    );
    path.line_to((f32::from_bits(0x40eb871c), f32::from_bits(0xc26e2ff8)));
    path.cubic_to(
        (f32::from_bits(0x409d63e0), f32::from_bits(0xc26f6508)),
        (f32::from_bits(0x401d76fa), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x35f7fd4a), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4122e81e), f32::from_bits(0xc2a4bf0c)));
    path.cubic_to(
        (f32::from_bits(0x41284f3c), f32::from_bits(0xc2a4a9ac)),
        (f32::from_bits(0x412db549), f32::from_bits(0xc2a4933e)),
        (f32::from_bits(0x41331a33), f32::from_bits(0xc2a47bbf)),
    );
    path.line_to((f32::from_bits(0x410178be), f32::from_bits(0xc26dceac)));
    path.cubic_to(
        (f32::from_bits(0x40fb24f7), f32::from_bits(0xc26df0a4)),
        (f32::from_bits(0x40f356d1), f32::from_bits(0xc26e1114)),
        (f32::from_bits(0x40eb871f), f32::from_bits(0xc26e2ff8)),
    );
    path.line_to((f32::from_bits(0x4122e81e), f32::from_bits(0xc2a4bf0c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2415-L2437 (chrome/m156)
fn battleOp89(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3dd41fb8), f32::from_bits(0xc2a5fffe)),
        (f32::from_bits(0x3e541e5b), f32::from_bits(0xc2a5ffe5)),
        (f32::from_bits(0x3e9f1657), f32::from_bits(0xc2a5ffb2)),
    );
    path.line_to((f32::from_bits(0x3e66012b), f32::from_bits(0xc26fff92)));
    path.cubic_to(
        (f32::from_bits(0x3e1955e2), f32::from_bits(0xc26fffdc)),
        (f32::from_bits(0x3d99560b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x350f7780), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3e9f1626), f32::from_bits(0xc2a5ffb4)));
    path.cubic_to(
        (f32::from_bits(0x3ea463a8), f32::from_bits(0xc2a5ffae)),
        (f32::from_bits(0x3ea9b10b), f32::from_bits(0xc2a5ffa8)),
        (f32::from_bits(0x3eaefe6d), f32::from_bits(0xc2a5ffa3)),
    );
    path.line_to((f32::from_bits(0x3e7d0144), f32::from_bits(0xc26fff7b)));
    path.cubic_to(
        (f32::from_bits(0x3e75568f), f32::from_bits(0xc26fff84)),
        (f32::from_bits(0x3e6dac12), f32::from_bits(0xc26fff8c)),
        (f32::from_bits(0x3e660197), f32::from_bits(0xc26fff93)),
    );
    path.line_to((f32::from_bits(0x3e9f1626), f32::from_bits(0xc2a5ffb4)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2440-L2465 (chrome/m156)
fn battleOp90(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3dd41f74), f32::from_bits(0xc2a5fffe)),
        (f32::from_bits(0x3e541e17), f32::from_bits(0xc2a5ffe5)),
        (f32::from_bits(0x3e9f1624), f32::from_bits(0xc2a5ffb2)),
    );
    path.line_to((f32::from_bits(0x3e9f1626), f32::from_bits(0xc2a5ffb4)));
    path.cubic_to(
        (f32::from_bits(0x3ea463a8), f32::from_bits(0xc2a5ffae)),
        (f32::from_bits(0x3ea9b10b), f32::from_bits(0xc2a5ffa8)),
        (f32::from_bits(0x3eaefe6d), f32::from_bits(0xc2a5ffa3)),
    );
    path.line_to((f32::from_bits(0x3e7d0144), f32::from_bits(0xc26fff7b)));
    path.cubic_to(
        (f32::from_bits(0x3e75568f), f32::from_bits(0xc26fff84)),
        (f32::from_bits(0x3e6dac12), f32::from_bits(0xc26fff8c)),
        (f32::from_bits(0x3e66012b), f32::from_bits(0xc26fff92)),
    );
    path.cubic_to(
        (f32::from_bits(0x3e1955e2), f32::from_bits(0xc26fffdc)),
        (f32::from_bits(0x3d99560b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3eaefebc), f32::from_bits(0xc2a5ffa4)));
    path.cubic_to(
        (f32::from_bits(0x3f0276b7), f32::from_bits(0xc2a5ff4a)),
        (f32::from_bits(0x3f2d6dea), f32::from_bits(0xc2a5feac)),
        (f32::from_bits(0x3f5864cc), f32::from_bits(0xc2a5fdcd)),
    );
    path.line_to((f32::from_bits(0x3f1c6df6), f32::from_bits(0xc26ffcd0)));
    path.cubic_to(
        (f32::from_bits(0x3efabdec), f32::from_bits(0xc26ffe15)),
        (f32::from_bits(0x3ebc9f78), f32::from_bits(0xc26ffef9)),
        (f32::from_bits(0x3e7d0190), f32::from_bits(0xc26fff7c)),
    );
    path.line_to((f32::from_bits(0x3eaefebc), f32::from_bits(0xc2a5ffa4)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2468-L2490 (chrome/m156)
fn battleOp91(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3ec1e1ad), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3f41e136), f32::from_bits(0xc2a5feac)),
        (f32::from_bits(0x3f9167c6), f32::from_bits(0xc2a5fc05)),
    );
    path.line_to((f32::from_bits(0x3f523979), f32::from_bits(0xc26ffa3f)));
    path.cubic_to(
        (f32::from_bits(0x3f0c2737), f32::from_bits(0xc26ffe17)),
        (f32::from_bits(0x3e8c2756), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb5b74260), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f9167c1), f32::from_bits(0xc2a5fc05)));
    path.cubic_to(
        (f32::from_bits(0x3f96406f), f32::from_bits(0xc2a5fbc1)),
        (f32::from_bits(0x3f9b1917), f32::from_bits(0xc2a5fb79)),
        (f32::from_bits(0x3f9ff1bc), f32::from_bits(0xc2a5fb2f)),
    );
    path.line_to((f32::from_bits(0x3f673ed7), f32::from_bits(0xc26ff909)));
    path.cubic_to(
        (f32::from_bits(0x3f603cf4), f32::from_bits(0xc26ff977)),
        (f32::from_bits(0x3f593b3c), f32::from_bits(0xc26ff9dd)),
        (f32::from_bits(0x3f52397f), f32::from_bits(0xc26ffa3f)),
    );
    path.line_to((f32::from_bits(0x3f9167c1), f32::from_bits(0xc2a5fc05)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2492-L2514 (chrome/m156)
fn battleOp92(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e2c5962), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3eac58ef), f32::from_bits(0xc2a5ffbd)),
        (f32::from_bits(0x3f014269), f32::from_bits(0xc2a5ff37)),
    );
    path.line_to((f32::from_bits(0x3ebae1ca), f32::from_bits(0xc26ffedd)));
    path.cubic_to(
        (f32::from_bits(0x3e792d51), f32::from_bits(0xc26fff9f)),
        (f32::from_bits(0x3df92dfa), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36163ed0), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f014292), f32::from_bits(0xc2a5ff37)));
    path.cubic_to(
        (f32::from_bits(0x3f0591a2), f32::from_bits(0xc2a5ff28)),
        (f32::from_bits(0x3f09e09b), f32::from_bits(0xc2a5ff1a)),
        (f32::from_bits(0x3f0e2f92), f32::from_bits(0xc2a5ff0b)),
    );
    path.line_to((f32::from_bits(0x3ecd91e5), f32::from_bits(0xc26ffea0)));
    path.cubic_to(
        (f32::from_bits(0x3ec75718), f32::from_bits(0xc26ffeb6)),
        (f32::from_bits(0x3ec11c70), f32::from_bits(0xc26ffeca)),
        (f32::from_bits(0x3ebae1c7), f32::from_bits(0xc26ffedd)),
    );
    path.line_to((f32::from_bits(0x3f014292), f32::from_bits(0xc2a5ff37)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2517-L2541 (chrome/m156)
fn battleOp93(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x36163ed0), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.quad_to(
        (f32::from_bits(0x3e81430a), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3f014292), f32::from_bits(0xc2a5ff37)),
    );
    path.cubic_to(
        (f32::from_bits(0x3f0591a2), f32::from_bits(0xc2a5ff28)),
        (f32::from_bits(0x3f09e09b), f32::from_bits(0xc2a5ff1a)),
        (f32::from_bits(0x3f0e2f92), f32::from_bits(0xc2a5ff0b)),
    );
    path.line_to((f32::from_bits(0x3ecd91e5), f32::from_bits(0xc26ffea0)));
    path.cubic_to(
        (f32::from_bits(0x3ec75719), f32::from_bits(0xc26ffeb6)),
        (f32::from_bits(0x3ec11c72), f32::from_bits(0xc26ffeca)),
        (f32::from_bits(0x3ebae1ca), f32::from_bits(0xc26ffedd)),
    );
    path.quad_to(
        (f32::from_bits(0x3e3ae230), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36163ed0), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f0e2f94), f32::from_bits(0xc2a5ff0c)));
    path.cubic_to(
        (f32::from_bits(0x3f5401b9), f32::from_bits(0xc2a5fe1c)),
        (f32::from_bits(0x3f8ce9a3), f32::from_bits(0xc2a5fc7d)),
        (f32::from_bits(0x3fafd1bd), f32::from_bits(0xc2a5fa2d)),
    );
    path.line_to((f32::from_bits(0x3f7e3238), f32::from_bits(0xc26ff796)));
    path.cubic_to(
        (f32::from_bits(0x3f4bbaca), f32::from_bits(0xc26ffaee)),
        (f32::from_bits(0x3f194226), f32::from_bits(0xc26ffd46)),
        (f32::from_bits(0x3ecd9202), f32::from_bits(0xc26ffea0)),
    );
    path.line_to((f32::from_bits(0x3f0e2f94), f32::from_bits(0xc2a5ff0c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2544-L2566 (chrome/m156)
fn battleOp94(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f167e4a), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3f967d97), f32::from_bits(0xc2a5fcce)),
        (f32::from_bits(0x3fe1b83b), f32::from_bits(0xc2a5f668)),
    );
    path.line_to((f32::from_bits(0x3fa32ba2), f32::from_bits(0xc26ff222)));
    path.cubic_to(
        (f32::from_bits(0x3f599370), f32::from_bits(0xc26ffb61)),
        (f32::from_bits(0x3ed9943c), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3437e940), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3fe1b817), f32::from_bits(0xc2a5f668)));
    path.cubic_to(
        (f32::from_bits(0x3fe93dd6), f32::from_bits(0xc2a5f5c4)),
        (f32::from_bits(0x3ff0c3a7), f32::from_bits(0xc2a5f518)),
        (f32::from_bits(0x3ff8496b), f32::from_bits(0xc2a5f464)),
    );
    path.line_to((f32::from_bits(0x3fb37c11), f32::from_bits(0xc26fef38)));
    path.cubic_to(
        (f32::from_bits(0x3fae0bf9), f32::from_bits(0xc26ff03c)),
        (f32::from_bits(0x3fa89bd2), f32::from_bits(0xc26ff134)),
        (f32::from_bits(0x3fa32ba2), f32::from_bits(0xc26ff222)),
    );
    path.line_to((f32::from_bits(0x3fe1b817), f32::from_bits(0xc2a5f668)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2569-L2588 (chrome/m156)
fn battleOp95(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f167e32), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3f967d7f), f32::from_bits(0xc2a5fcce)),
        (f32::from_bits(0x3fe1b817), f32::from_bits(0xc2a5f668)),
    );
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ff8497f), f32::from_bits(0xc2a5f465)));
    path.cubic_to(
        (f32::from_bits(0x40391895), f32::from_bits(0xc2a5e8fe)),
        (f32::from_bits(0x407604f1), f32::from_bits(0xc2a5d533)),
        (f32::from_bits(0x40997177), f32::from_bits(0xc2a5b905)),
    );
    path.line_to((f32::from_bits(0x405dd87f), f32::from_bits(0xc26f9962)));
    path.cubic_to(
        (f32::from_bits(0x4031d867), f32::from_bits(0xc26fc221)),
        (f32::from_bits(0x4005cdec), f32::from_bits(0xc26fdebf)),
        (f32::from_bits(0x3fb37c22), f32::from_bits(0xc26fef39)),
    );
    path.line_to((f32::from_bits(0x3ff8497f), f32::from_bits(0xc2a5f465)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2591-L2613 (chrome/m156)
fn battleOp96(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3fa966bb), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x402963a4), f32::from_bits(0xc2a5efcb)),
        (f32::from_bits(0x407dfe39), f32::from_bits(0xc2a5cf64)),
    );
    path.line_to((f32::from_bits(0x40379c05), f32::from_bits(0xc26fb9ba)));
    path.cubic_to(
        (f32::from_bits(0x3ff4e689), f32::from_bits(0xc26fe893)),
        (f32::from_bits(0x3f74eb1f), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x363f7e94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x407dfe3a), f32::from_bits(0xc2a5cf65)));
    path.cubic_to(
        (f32::from_bits(0x40833a01), f32::from_bits(0xc2a5cc27)),
        (f32::from_bits(0x408774bf), f32::from_bits(0xc2a5c8c0)),
        (f32::from_bits(0x408baf5a), f32::from_bits(0xc2a5c52f)),
    );
    path.line_to((f32::from_bits(0x4049f448), f32::from_bits(0xc26faaf9)));
    path.cubic_to(
        (f32::from_bits(0x4043d713), f32::from_bits(0xc26fb022)),
        (f32::from_bits(0x403db99f), f32::from_bits(0xc26fb50d)),
        (f32::from_bits(0x40379bfe), f32::from_bits(0xc26fb9bc)),
    );
    path.line_to((f32::from_bits(0x407dfe3a), f32::from_bits(0xc2a5cf65)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2616-L2641 (chrome/m156)
fn battleOp97(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x363f7e94), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3fa966bb), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x402963a4), f32::from_bits(0xc2a5efcb)),
        (f32::from_bits(0x407dfe3a), f32::from_bits(0xc2a5cf65)),
    );
    path.cubic_to(
        (f32::from_bits(0x40833a01), f32::from_bits(0xc2a5cc27)),
        (f32::from_bits(0x408774bf), f32::from_bits(0xc2a5c8c0)),
        (f32::from_bits(0x408baf5a), f32::from_bits(0xc2a5c52f)),
    );
    path.line_to((f32::from_bits(0x4049f448), f32::from_bits(0xc26faaf9)));
    path.cubic_to(
        (f32::from_bits(0x4043d716), f32::from_bits(0xc26fb022)),
        (f32::from_bits(0x403db9a5), f32::from_bits(0xc26fb50d)),
        (f32::from_bits(0x40379c07), f32::from_bits(0xc26fb9bc)),
    );
    path.line_to((f32::from_bits(0x40379c05), f32::from_bits(0xc26fb9ba)));
    path.cubic_to(
        (f32::from_bits(0x3ff4e689), f32::from_bits(0xc26fe893)),
        (f32::from_bits(0x3f74eb1f), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x363f7e94), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x408baf5c), f32::from_bits(0xc2a5c530)));
    path.cubic_to(
        (f32::from_bits(0x40d03963), f32::from_bits(0xc2a58b6e)),
        (f32::from_bits(0x410a4c7d), f32::from_bits(0xc2a52732)),
        (f32::from_bits(0x412c535f), f32::from_bits(0xc2a498b2)),
    );
    path.line_to((f32::from_bits(0x40f9253d), f32::from_bits(0xc26df886)));
    path.cubic_to(
        (f32::from_bits(0x40c7f32d), f32::from_bits(0xc26ec68d)),
        (f32::from_bits(0x409685fb), f32::from_bits(0xc26f577a)),
        (f32::from_bits(0x4049f441), f32::from_bits(0xc26faafa)),
    );
    path.line_to((f32::from_bits(0x408baf5c), f32::from_bits(0xc2a5c530)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2644-L2666 (chrome/m156)
fn battleOp98(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40155bee), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40955364), f32::from_bits(0xc2a5cd99)),
        (f32::from_bits(0x40dfbd5f), f32::from_bits(0xc2a568f2)),
    );
    path.line_to((f32::from_bits(0x40a1bd53), f32::from_bits(0xc26f259d)));
    path.cubic_to(
        (f32::from_bits(0x4057e483), f32::from_bits(0xc26fb724)),
        (f32::from_bits(0x3fd7f0d9), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3619fea3), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x40dfbd5e), f32::from_bits(0xc2a568f3)));
    path.cubic_to(
        (f32::from_bits(0x40e72e1b), f32::from_bits(0xc2a55ee2)),
        (f32::from_bits(0x40ee9e1c), f32::from_bits(0xc2a55452)),
        (f32::from_bits(0x40f60d62), f32::from_bits(0xc2a54941)),
    );
    path.line_to((f32::from_bits(0x40b1de84), f32::from_bits(0xc26ef7c9)));
    path.cubic_to(
        (f32::from_bits(0x40ac7ea0), f32::from_bits(0xc26f07cb)),
        (f32::from_bits(0x40a71e37), f32::from_bits(0xc26f1712)),
        (f32::from_bits(0x40a1bd4f), f32::from_bits(0xc26f259f)),
    );
    path.line_to((f32::from_bits(0x40dfbd5e), f32::from_bits(0xc2a568f3)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2669-L2694 (chrome/m156)
fn battleOp99(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3619fea3), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40155bee), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40955364), f32::from_bits(0xc2a5cd99)),
        (f32::from_bits(0x40dfbd5e), f32::from_bits(0xc2a568f3)),
    );
    path.cubic_to(
        (f32::from_bits(0x40e72e1b), f32::from_bits(0xc2a55ee2)),
        (f32::from_bits(0x40ee9e1c), f32::from_bits(0xc2a55452)),
        (f32::from_bits(0x40f60d62), f32::from_bits(0xc2a54941)),
    );
    path.line_to((f32::from_bits(0x40b1de84), f32::from_bits(0xc26ef7c9)));
    path.cubic_to(
        (f32::from_bits(0x40ac7ea2), f32::from_bits(0xc26f07cb)),
        (f32::from_bits(0x40a71e3a), f32::from_bits(0xc26f1712)),
        (f32::from_bits(0x40a1bd54), f32::from_bits(0xc26f259f)),
    );
    path.line_to((f32::from_bits(0x40a1bd53), f32::from_bits(0xc26f259d)));
    path.cubic_to(
        (f32::from_bits(0x4057e483), f32::from_bits(0xc26fb724)),
        (f32::from_bits(0x3fd7f0d9), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3619fea3), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x40f60d69), f32::from_bits(0xc2a54941)));
    path.cubic_to(
        (f32::from_bits(0x41374a21), f32::from_bits(0xc2a495d5)),
        (f32::from_bits(0x41731962), f32::from_bits(0xc2a35eca)),
        (f32::from_bits(0x419704b1), f32::from_bits(0xc2a1a64c)),
    );
    path.line_to((f32::from_bits(0x415a56f5), f32::from_bits(0xc269b5d4)));
    path.cubic_to(
        (f32::from_bits(0x412fbbfb), f32::from_bits(0xc26c32af)),
        (f32::from_bits(0x41047f9a), f32::from_bits(0xc26df463)),
        (f32::from_bits(0x40b1de7e), f32::from_bits(0xc26ef7cb)),
    );
    path.line_to((f32::from_bits(0x40f60d69), f32::from_bits(0xc2a54941)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2697-L2719 (chrome/m156)
fn battleOp100(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x403cde0b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40bcccc9), f32::from_bits(0xc2a5af6a)),
        (f32::from_bits(0x410d5936), f32::from_bits(0xc2a50e98)),
    );
    path.line_to((f32::from_bits(0x40cc5bf6), f32::from_bits(0xc26ea2fc)));
    path.cubic_to(
        (f32::from_bits(0x40887b5e), f32::from_bits(0xc26f8b7f)),
        (f32::from_bits(0x400887d8), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x410d5935), f32::from_bits(0xc2a50e99)));
    path.cubic_to(
        (f32::from_bits(0x41120ace), f32::from_bits(0xc2a4fe85)),
        (f32::from_bits(0x4116bbb5), f32::from_bits(0xc2a4eda4)),
        (f32::from_bits(0x411b6bdd), f32::from_bits(0xc2a4dbf6)),
    );
    path.line_to((f32::from_bits(0x40e0b4a3), f32::from_bits(0xc26e59c7)));
    path.cubic_to(
        (f32::from_bits(0x40d9ed7a), f32::from_bits(0xc26e7357)),
        (f32::from_bits(0x40d32536), f32::from_bits(0xc26e8bbe)),
        (f32::from_bits(0x40cc5bf1), f32::from_bits(0xc26ea2fc)),
    );
    path.line_to((f32::from_bits(0x410d5935), f32::from_bits(0xc2a50e99)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2722-L2744 (chrome/m156)
fn battleOp101(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x406db78d), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x40ed953d), f32::from_bits(0xc2a58058)),
        (f32::from_bits(0x4131afb7), f32::from_bits(0xc2a481e4)),
    );
    path.line_to((f32::from_bits(0x410072b2), f32::from_bits(0xc26dd78e)));
    path.cubic_to(
        (f32::from_bits(0x40abbf2e), f32::from_bits(0xc26f4770)),
        (f32::from_bits(0x402bd807), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4131afba), f32::from_bits(0xc2a481e4)));
    path.cubic_to(
        (f32::from_bits(0x413792dd), f32::from_bits(0xc2a46874)),
        (f32::from_bits(0x413d74a2), f32::from_bits(0xc2a44dc1)),
        (f32::from_bits(0x414354e9), f32::from_bits(0xc2a431ca)),
    );
    path.line_to((f32::from_bits(0x410d3424), f32::from_bits(0xc26d63c0)));
    path.cubic_to(
        (f32::from_bits(0x4108f4b6), f32::from_bits(0xc26d8c2e)),
        (f32::from_bits(0x4104b435), f32::from_bits(0xc26db2c8)),
        (f32::from_bits(0x410072b4), f32::from_bits(0xc26dd78e)),
    );
    path.line_to((f32::from_bits(0x4131afba), f32::from_bits(0xc2a481e4)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2747-L2771 (chrome/m156)
fn battleOp102(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x406db78d), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x40ed953d), f32::from_bits(0xc2a58058)),
        (f32::from_bits(0x4131afba), f32::from_bits(0xc2a481e4)),
    );
    path.cubic_to(
        (f32::from_bits(0x413792dd), f32::from_bits(0xc2a46874)),
        (f32::from_bits(0x413d74a2), f32::from_bits(0xc2a44dc1)),
        (f32::from_bits(0x414354e9), f32::from_bits(0xc2a431ca)),
    );
    path.line_to((f32::from_bits(0x410d3424), f32::from_bits(0xc26d63c0)));
    path.cubic_to(
        (f32::from_bits(0x4108f4b6), f32::from_bits(0xc26d8c2e)),
        (f32::from_bits(0x4104b435), f32::from_bits(0xc26db2c8)),
        (f32::from_bits(0x410072b2), f32::from_bits(0xc26dd78e)),
    );
    path.cubic_to(
        (f32::from_bits(0x40abbf2e), f32::from_bits(0xc26f4770)),
        (f32::from_bits(0x402bd807), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x414354ed), f32::from_bits(0xc2a431cb)));
    path.cubic_to(
        (f32::from_bits(0x419152e5), f32::from_bits(0xc2a26c3a)),
        (f32::from_bits(0x41c0119b), f32::from_bits(0xc29f5c06)),
        (f32::from_bits(0x41ed1335), f32::from_bits(0xc29b0f0a)),
    );
    path.line_to((f32::from_bits(0x41ab612b), f32::from_bits(0xc2602e6b)));
    path.cubic_to(
        (f32::from_bits(0x418ad84d), f32::from_bits(0xc2666635)),
        (f32::from_bits(0x41521b54), f32::from_bits(0xc26ad3fe)),
        (f32::from_bits(0x410d3426), f32::from_bits(0xc26d63c0)),
    );
    path.line_to((f32::from_bits(0x414354ed), f32::from_bits(0xc2a431cb)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2774-L2796 (chrome/m156)
fn battleOp103(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x408e2d73), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x410e100a), f32::from_bits(0xc2a54957)),
        (f32::from_bits(0x41543cd2), f32::from_bits(0xc2a3ddc8)),
    );
    path.line_to((f32::from_bits(0x41196cba), f32::from_bits(0xc26cea49)));
    path.cubic_to(
        (f32::from_bits(0x40cd643f), f32::from_bits(0xc26ef7e9)),
        (f32::from_bits(0x404d8eb8), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0xb5ac02ba), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41543cce), f32::from_bits(0xc2a3ddc8)));
    path.cubic_to(
        (f32::from_bits(0x415b4057), f32::from_bits(0xc2a3b973)),
        (f32::from_bits(0x41624181), f32::from_bits(0xc2a39350)),
        (f32::from_bits(0x41694022), f32::from_bits(0xc2a36b60)),
    );
    path.line_to((f32::from_bits(0x41289d63), f32::from_bits(0xc26c44e1)));
    path.cubic_to(
        (f32::from_bits(0x41238ef8), f32::from_bits(0xc26c7e9e)),
        (f32::from_bits(0x411e7eb5), f32::from_bits(0xc26cb5c1)),
        (f32::from_bits(0x41196cbd), f32::from_bits(0xc26cea4a)),
    );
    path.line_to((f32::from_bits(0x41543cce), f32::from_bits(0xc2a3ddc8)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2798-L2820 (chrome/m156)
fn battleOp104(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3dd41fb8), f32::from_bits(0xc2a5fffe)),
        (f32::from_bits(0x3e541e5b), f32::from_bits(0xc2a5ffe5)),
        (f32::from_bits(0x3e9f1657), f32::from_bits(0xc2a5ffb2)),
    );
    path.line_to((f32::from_bits(0x3e66012b), f32::from_bits(0xc26fff92)));
    path.cubic_to(
        (f32::from_bits(0x3e1955e2), f32::from_bits(0xc26fffdc)),
        (f32::from_bits(0x3d99560b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x350f7780), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3e9f1626), f32::from_bits(0xc2a5ffb4)));
    path.cubic_to(
        (f32::from_bits(0x3ea463a8), f32::from_bits(0xc2a5ffae)),
        (f32::from_bits(0x3ea9b10b), f32::from_bits(0xc2a5ffa8)),
        (f32::from_bits(0x3eaefe6d), f32::from_bits(0xc2a5ffa3)),
    );
    path.line_to((f32::from_bits(0x3e7d0144), f32::from_bits(0xc26fff7b)));
    path.cubic_to(
        (f32::from_bits(0x3e75568f), f32::from_bits(0xc26fff84)),
        (f32::from_bits(0x3e6dac12), f32::from_bits(0xc26fff8c)),
        (f32::from_bits(0x3e660197), f32::from_bits(0xc26fff93)),
    );
    path.line_to((f32::from_bits(0x3e9f1626), f32::from_bits(0xc2a5ffb4)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2823-L2848 (chrome/m156)
fn battleOp105(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3dd41f74), f32::from_bits(0xc2a5fffe)),
        (f32::from_bits(0x3e541e17), f32::from_bits(0xc2a5ffe5)),
        (f32::from_bits(0x3e9f1624), f32::from_bits(0xc2a5ffb2)),
    );
    path.line_to((f32::from_bits(0x3e9f1626), f32::from_bits(0xc2a5ffb4)));
    path.cubic_to(
        (f32::from_bits(0x3ea463a8), f32::from_bits(0xc2a5ffae)),
        (f32::from_bits(0x3ea9b10b), f32::from_bits(0xc2a5ffa8)),
        (f32::from_bits(0x3eaefe6d), f32::from_bits(0xc2a5ffa3)),
    );
    path.line_to((f32::from_bits(0x3e7d0144), f32::from_bits(0xc26fff7b)));
    path.cubic_to(
        (f32::from_bits(0x3e75568f), f32::from_bits(0xc26fff84)),
        (f32::from_bits(0x3e6dac12), f32::from_bits(0xc26fff8c)),
        (f32::from_bits(0x3e66012b), f32::from_bits(0xc26fff92)),
    );
    path.cubic_to(
        (f32::from_bits(0x3e1955e2), f32::from_bits(0xc26fffdc)),
        (f32::from_bits(0x3d99560b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3eaefebc), f32::from_bits(0xc2a5ffa4)));
    path.cubic_to(
        (f32::from_bits(0x3f0276b7), f32::from_bits(0xc2a5ff4a)),
        (f32::from_bits(0x3f2d6dea), f32::from_bits(0xc2a5feac)),
        (f32::from_bits(0x3f5864cc), f32::from_bits(0xc2a5fdcd)),
    );
    path.line_to((f32::from_bits(0x3f1c6df6), f32::from_bits(0xc26ffcd0)));
    path.cubic_to(
        (f32::from_bits(0x3efabdec), f32::from_bits(0xc26ffe15)),
        (f32::from_bits(0x3ebc9f78), f32::from_bits(0xc26ffef9)),
        (f32::from_bits(0x3e7d0190), f32::from_bits(0xc26fff7c)),
    );
    path.line_to((f32::from_bits(0x3eaefebc), f32::from_bits(0xc2a5ffa4)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2851-L2873 (chrome/m156)
fn battleOp106(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3ee221f0), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3f622166), f32::from_bits(0xc2a5fe31)),
        (f32::from_bits(0x3fa9974d), f32::from_bits(0xc2a5fa95)),
    );
    path.line_to((f32::from_bits(0x3f753159), f32::from_bits(0xc26ff82c)));
    path.cubic_to(
        (f32::from_bits(0x3f237814), f32::from_bits(0xc26ffd64)),
        (f32::from_bits(0x3ea3787a), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa50), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3fa99777), f32::from_bits(0xc2a5fa96)));
    path.cubic_to(
        (f32::from_bits(0x3faf3e7a), f32::from_bits(0xc2a5fa39)),
        (f32::from_bits(0x3fb4e596), f32::from_bits(0xc2a5f9d8)),
        (f32::from_bits(0x3fba8cad), f32::from_bits(0xc2a5f972)),
    );
    path.line_to((f32::from_bits(0x3f86dad5), f32::from_bits(0xc26ff687)));
    path.cubic_to(
        (f32::from_bits(0x3f82c4d9), f32::from_bits(0xc26ff71a)),
        (f32::from_bits(0x3f7d5da4), f32::from_bits(0xc26ff7a6)),
        (f32::from_bits(0x3f753191), f32::from_bits(0xc26ff82c)),
    );
    path.line_to((f32::from_bits(0x3fa99777), f32::from_bits(0xc2a5fa96)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2876-L2900 (chrome/m156)
fn battleOp107(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3ee221f0), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3f622166), f32::from_bits(0xc2a5fe31)),
        (f32::from_bits(0x3fa99777), f32::from_bits(0xc2a5fa96)),
    );
    path.cubic_to(
        (f32::from_bits(0x3faf3e7a), f32::from_bits(0xc2a5fa39)),
        (f32::from_bits(0x3fb4e596), f32::from_bits(0xc2a5f9d8)),
        (f32::from_bits(0x3fba8cad), f32::from_bits(0xc2a5f972)),
    );
    path.line_to((f32::from_bits(0x3f86dad5), f32::from_bits(0xc26ff687)));
    path.cubic_to(
        (f32::from_bits(0x3f82c4d9), f32::from_bits(0xc26ff71a)),
        (f32::from_bits(0x3f7d5da4), f32::from_bits(0xc26ff7a6)),
        (f32::from_bits(0x3f753159), f32::from_bits(0xc26ff82c)),
    );
    path.cubic_to(
        (f32::from_bits(0x3f237814), f32::from_bits(0xc26ffd64)),
        (f32::from_bits(0x3ea3787a), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3fba8c96), f32::from_bits(0xc2a5f973)));
    path.cubic_to(
        (f32::from_bits(0x400b1301), f32::from_bits(0xc2a5f303)),
        (f32::from_bits(0x4038dc7e), f32::from_bits(0xc2a5e7d6)),
        (f32::from_bits(0x40669fe4), f32::from_bits(0xc2a5d7ed)),
    );
    path.line_to((f32::from_bits(0x4026b765), f32::from_bits(0xc26fc611)));
    path.cubic_to(
        (f32::from_bits(0x4005a27d), f32::from_bits(0xc26fdd13)),
        (f32::from_bits(0x3fc9123c), f32::from_bits(0xc26fed3b)),
        (f32::from_bits(0x3f86daf1), f32::from_bits(0xc26ff689)),
    );
    path.line_to((f32::from_bits(0x3fba8c96), f32::from_bits(0xc2a5f973)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2903-L2925 (chrome/m156)
fn battleOp108(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f587304), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3fd8713e), f32::from_bits(0xc2a5f962)),
        (f32::from_bits(0x40224ed5), f32::from_bits(0xc2a5ec27)),
    );
    path.line_to((f32::from_bits(0x3feaa996), f32::from_bits(0xc26fe350)));
    path.cubic_to(
        (f32::from_bits(0x3f9c76e4), f32::from_bits(0xc26ff671)),
        (f32::from_bits(0x3f1c780b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb5510538), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x40224ee4), f32::from_bits(0xc2a5ec28)));
    path.cubic_to(
        (f32::from_bits(0x4027b77a), f32::from_bits(0xc2a5ead6)),
        (f32::from_bits(0x402d1ffd), f32::from_bits(0xc2a5e972)),
        (f32::from_bits(0x4032886f), f32::from_bits(0xc2a5e7fe)),
    );
    path.line_to((f32::from_bits(0x40010f64), f32::from_bits(0xc26fdd4a)));
    path.cubic_to(
        (f32::from_bits(0x3ffa4d23), f32::from_bits(0xc26fdf64)),
        (f32::from_bits(0x3ff27b6d), f32::from_bits(0xc26fe166)),
        (f32::from_bits(0x3feaa9a1), f32::from_bits(0xc26fe350)),
    );
    path.line_to((f32::from_bits(0x40224ee4), f32::from_bits(0xc2a5ec28)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2928-L2952 (chrome/m156)
fn battleOp109(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f587304), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3fd8713e), f32::from_bits(0xc2a5f962)),
        (f32::from_bits(0x40224ee4), f32::from_bits(0xc2a5ec28)),
    );
    path.cubic_to(
        (f32::from_bits(0x4027b77a), f32::from_bits(0xc2a5ead6)),
        (f32::from_bits(0x402d1ffd), f32::from_bits(0xc2a5e972)),
        (f32::from_bits(0x4032886f), f32::from_bits(0xc2a5e7fe)),
    );
    path.line_to((f32::from_bits(0x40010f64), f32::from_bits(0xc26fdd4a)));
    path.cubic_to(
        (f32::from_bits(0x3ffa4d23), f32::from_bits(0xc26fdf64)),
        (f32::from_bits(0x3ff27b6d), f32::from_bits(0xc26fe166)),
        (f32::from_bits(0x3feaa996), f32::from_bits(0xc26fe350)),
    );
    path.cubic_to(
        (f32::from_bits(0x3f9c76e4), f32::from_bits(0xc26ff671)),
        (f32::from_bits(0x3f1c780b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4032887d), f32::from_bits(0xc2a5e7fe)));
    path.cubic_to(
        (f32::from_bits(0x4085166b), f32::from_bits(0xc2a5d069)),
        (f32::from_bits(0x40b0dd8e), f32::from_bits(0xc2a5a77a)),
        (f32::from_bits(0x40dc8f53), f32::from_bits(0xc2a56d38)),
    );
    path.line_to((f32::from_bits(0x409f70d9), f32::from_bits(0xc26f2bca)));
    path.cubic_to(
        (f32::from_bits(0x407fb58c), f32::from_bits(0xc26f8005)),
        (f32::from_bits(0x40406a74), f32::from_bits(0xc26fbb35)),
        (f32::from_bits(0x40010f5f), f32::from_bits(0xc26fdd4b)),
    );
    path.line_to((f32::from_bits(0x4032887d), f32::from_bits(0xc2a5e7fe)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2955-L2977 (chrome/m156)
fn battleOp110(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x400cf1ae), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x408cea87), f32::from_bits(0xc2a5d31f)),
        (f32::from_bits(0x40d32a40), f32::from_bits(0xc2a57979)),
    );
    path.line_to((f32::from_bits(0x4098a645), f32::from_bits(0xc26f3d83)));
    path.cubic_to(
        (f32::from_bits(0x404bbc01), f32::from_bits(0xc26fbf1e)),
        (f32::from_bits(0x3fcbc669), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff59), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x40d32a46), f32::from_bits(0xc2a5797a)));
    path.cubic_to(
        (f32::from_bits(0x40da306e), f32::from_bits(0xc2a57083)),
        (f32::from_bits(0x40e135fe), f32::from_bits(0xc2a5671a)),
        (f32::from_bits(0x40e83aef), f32::from_bits(0xc2a55d3f)),
    );
    path.line_to((f32::from_bits(0x40a7e090), f32::from_bits(0xc26f14b1)));
    path.cubic_to(
        (f32::from_bits(0x40a2cd8d), f32::from_bits(0xc26f22f4)),
        (f32::from_bits(0x409dba1d), f32::from_bits(0xc26f308e)),
        (f32::from_bits(0x4098a641), f32::from_bits(0xc26f3d84)),
    );
    path.line_to((f32::from_bits(0x40d32a46), f32::from_bits(0xc2a5797a)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L2980-L3009 (chrome/m156)
fn battleOp111(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3697ff59), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x400cf1ae), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x408cea87), f32::from_bits(0xc2a5d31f)),
        (f32::from_bits(0x40d32a46), f32::from_bits(0xc2a5797a)),
    );
    path.cubic_to(
        (f32::from_bits(0x40da306e), f32::from_bits(0xc2a57083)),
        (f32::from_bits(0x40e135fe), f32::from_bits(0xc2a5671a)),
        (f32::from_bits(0x40e83aef), f32::from_bits(0xc2a55d3f)),
    );
    path.line_to((f32::from_bits(0x40a7e090), f32::from_bits(0xc26f14b1)));
    path.cubic_to(
        (f32::from_bits(0x40a2cd8f), f32::from_bits(0xc26f22f4)),
        (f32::from_bits(0x409dba20), f32::from_bits(0xc26f308e)),
        (f32::from_bits(0x4098a645), f32::from_bits(0xc26f3d83)),
    );
    path.cubic_to(
        (f32::from_bits(0x404bbc01), f32::from_bits(0xc26fbf1e)),
        (f32::from_bits(0x3fcbc669), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff59), f32::from_bits(0xc26fffff)),
    );
    path.close();
    path.move_to((f32::from_bits(0x40b5a39a), f32::from_bits(0xc28e5650)));
    path.line_to((f32::from_bits(0x4098a641), f32::from_bits(0xc26f3d84)));
    path.line_to((f32::from_bits(0x4098a646), f32::from_bits(0xc26f3d84)));
    path.line_to((f32::from_bits(0x40b5a39a), f32::from_bits(0xc28e5650)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x40e83ae9), f32::from_bits(0xc2a55d3f)));
    path.cubic_to(
        (f32::from_bits(0x412d0232), f32::from_bits(0xc2a4bd73)),
        (f32::from_bits(0x4165854a), f32::from_bits(0xc2a3a860)),
        (f32::from_bits(0x418ea651), f32::from_bits(0xc2a21fbf)),
    );
    path.line_to((f32::from_bits(0x414e3d91), f32::from_bits(0xc26a656a)));
    path.cubic_to(
        (f32::from_bits(0x4125eb27), f32::from_bits(0xc26c9d13)),
        (f32::from_bits(0x40fa2207), f32::from_bits(0xc26e2daa)),
        (f32::from_bits(0x40a7e094), f32::from_bits(0xc26f14b2)),
    );
    path.line_to((f32::from_bits(0x40e83ae9), f32::from_bits(0xc2a55d3f)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3012-L3034 (chrome/m156)
fn battleOp112(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4035711d), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40b561d9), f32::from_bits(0xc2a5b5a1)),
        (f32::from_bits(0x4107d050), f32::from_bits(0xc2a5212f)),
    );
    path.line_to((f32::from_bits(0x40c45b76), f32::from_bits(0xc26ebddb)));
    path.cubic_to(
        (f32::from_bits(0x40831ea4), f32::from_bits(0xc26f947a)),
        (f32::from_bits(0x400329ad), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x35bbfd46), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4107d054), f32::from_bits(0xc2a5212f)));
    path.cubic_to(
        (f32::from_bits(0x410c5332), f32::from_bits(0xc2a51258)),
        (f32::from_bits(0x4110d578), f32::from_bits(0xc2a502c3)),
        (f32::from_bits(0x41155714), f32::from_bits(0xc2a4f271)),
    );
    path.line_to((f32::from_bits(0x40d7e9e2), f32::from_bits(0xc26e7a46)));
    path.cubic_to(
        (f32::from_bits(0x40d16605), f32::from_bits(0xc26e91e0)),
        (f32::from_bits(0x40cae131), f32::from_bits(0xc26ea866)),
        (f32::from_bits(0x40c45b7a), f32::from_bits(0xc26ebddc)),
    );
    path.line_to((f32::from_bits(0x4107d054), f32::from_bits(0xc2a5212f)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3037-L3061 (chrome/m156)
fn battleOp113(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4035711d), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40b561d9), f32::from_bits(0xc2a5b5a1)),
        (f32::from_bits(0x4107d054), f32::from_bits(0xc2a5212f)),
    );
    path.cubic_to(
        (f32::from_bits(0x410c5332), f32::from_bits(0xc2a51258)),
        (f32::from_bits(0x4110d578), f32::from_bits(0xc2a502c3)),
        (f32::from_bits(0x41155714), f32::from_bits(0xc2a4f271)),
    );
    path.line_to((f32::from_bits(0x40d7e9e2), f32::from_bits(0xc26e7a46)));
    path.cubic_to(
        (f32::from_bits(0x40d16605), f32::from_bits(0xc26e91e0)),
        (f32::from_bits(0x40cae131), f32::from_bits(0xc26ea866)),
        (f32::from_bits(0x40c45b76), f32::from_bits(0xc26ebddb)),
    );
    path.cubic_to(
        (f32::from_bits(0x40831ea4), f32::from_bits(0xc26f947a)),
        (f32::from_bits(0x400329ad), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4115571a), f32::from_bits(0xc2a4f271)));
    path.cubic_to(
        (f32::from_bits(0x415e6818), f32::from_bits(0xc2a3e9d4)),
        (f32::from_bits(0x41935478), f32::from_bits(0xc2a21f7a)),
        (f32::from_bits(0x41b6ad74), f32::from_bits(0xc29f981d)),
    );
    path.line_to((f32::from_bits(0x41840e5b), f32::from_bits(0xc266bd14)));
    path.cubic_to(
        (f32::from_bits(0x415501d6), f32::from_bits(0xc26a6507)),
        (f32::from_bits(0x4120c6a0), f32::from_bits(0xc26cfbb4)),
        (f32::from_bits(0x40d7e9e6), f32::from_bits(0xc26e7a47)),
    );
    path.line_to((f32::from_bits(0x4115571a), f32::from_bits(0xc2a4f271)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3064-L3086 (chrome/m156)
fn battleOp114(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x405f6414), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x40df4798), f32::from_bits(0xc2a58f44)),
        (f32::from_bits(0x41270b42), f32::from_bits(0xc2a4ae78)),
    );
    path.line_to((f32::from_bits(0x40f1826b), f32::from_bits(0xc26e1801)));
    path.cubic_to(
        (f32::from_bits(0x40a16831), f32::from_bits(0xc26f5d03)),
        (f32::from_bits(0x40217cc8), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3507fa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41270b46), f32::from_bits(0xc2a4ae78)));
    path.cubic_to(
        (f32::from_bits(0x412c952a), f32::from_bits(0xc2a497ff)),
        (f32::from_bits(0x41321de3), f32::from_bits(0xc2a48068)),
        (f32::from_bits(0x4137a563), f32::from_bits(0xc2a467b4)),
    );
    path.line_to((f32::from_bits(0x4104c195), f32::from_bits(0xc26db1b1)));
    path.cubic_to(
        (f32::from_bits(0x4100c256), f32::from_bits(0xc26dd569)),
        (f32::from_bits(0x40f98465), f32::from_bits(0xc26df784)),
        (f32::from_bits(0x40f18273), f32::from_bits(0xc26e1801)),
    );
    path.line_to((f32::from_bits(0x41270b46), f32::from_bits(0xc2a4ae78)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3089-L3113 (chrome/m156)
fn battleOp115(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x405f6414), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x40df4798), f32::from_bits(0xc2a58f44)),
        (f32::from_bits(0x41270b46), f32::from_bits(0xc2a4ae78)),
    );
    path.cubic_to(
        (f32::from_bits(0x412c952a), f32::from_bits(0xc2a497ff)),
        (f32::from_bits(0x41321de3), f32::from_bits(0xc2a48068)),
        (f32::from_bits(0x4137a563), f32::from_bits(0xc2a467b4)),
    );
    path.line_to((f32::from_bits(0x4104c195), f32::from_bits(0xc26db1b1)));
    path.cubic_to(
        (f32::from_bits(0x4100c256), f32::from_bits(0xc26dd569)),
        (f32::from_bits(0x40f98465), f32::from_bits(0xc26df784)),
        (f32::from_bits(0x40f1826b), f32::from_bits(0xc26e1801)),
    );
    path.cubic_to(
        (f32::from_bits(0x40a16831), f32::from_bits(0xc26f5d03)),
        (f32::from_bits(0x40217cc8), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4137a563), f32::from_bits(0xc2a467b4)));
    path.cubic_to(
        (f32::from_bits(0x4188a9bf), f32::from_bits(0xc2a2d700)),
        (f32::from_bits(0x41b4bec4), f32::from_bits(0xc2a021d5)),
        (f32::from_bits(0x41df619b), f32::from_bits(0xc29c5308)),
    );
    path.line_to((f32::from_bits(0x41a17afe), f32::from_bits(0xc26202d7)));
    path.cubic_to(
        (f32::from_bits(0x4182a8c1), f32::from_bits(0xc2678433)),
        (f32::from_bits(0x414595cf), f32::from_bits(0xc26b6e5e)),
        (f32::from_bits(0x4104c197), f32::from_bits(0xc26db1b2)),
    );
    path.line_to((f32::from_bits(0x4137a563), f32::from_bits(0xc2a467b4)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3116-L3138 (chrome/m156)
fn battleOp116(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40894a00), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x41092f84), f32::from_bits(0xc2a555af)),
        (f32::from_bits(0x414d01d5), f32::from_bits(0xc2a40295)),
    );
    path.line_to((f32::from_bits(0x411432a9), f32::from_bits(0xc26d1f80)));
    path.cubic_to(
        (f32::from_bits(0x40c65728), f32::from_bits(0xc26f09c3)),
        (f32::from_bits(0x40467d64), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb5600574), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x414d01d1), f32::from_bits(0xc2a40296)));
    path.cubic_to(
        (f32::from_bits(0x4153c92e), f32::from_bits(0xc2a3e0b1)),
        (f32::from_bits(0x415a8e6d), f32::from_bits(0xc2a3bd1e)),
        (f32::from_bits(0x41615162), f32::from_bits(0xc2a397de)),
    );
    path.line_to((f32::from_bits(0x4122e164), f32::from_bits(0xc26c8535)));
    path.cubic_to(
        (f32::from_bits(0x411dfe19), f32::from_bits(0xc26cbb11)),
        (f32::from_bits(0x41191928), f32::from_bits(0xc26cee7f)),
        (f32::from_bits(0x411432ab), f32::from_bits(0xc26d1f80)),
    );
    path.line_to((f32::from_bits(0x414d01d1), f32::from_bits(0xc2a40296)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3141-L3166 (chrome/m156)
fn battleOp117(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x408949fd), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x41092f81), f32::from_bits(0xc2a555af)),
        (f32::from_bits(0x414d01d0), f32::from_bits(0xc2a40295)),
    );
    path.line_to((f32::from_bits(0x414d01d1), f32::from_bits(0xc2a40296)));
    path.cubic_to(
        (f32::from_bits(0x4153c92e), f32::from_bits(0xc2a3e0b1)),
        (f32::from_bits(0x415a8e6d), f32::from_bits(0xc2a3bd1e)),
        (f32::from_bits(0x41615162), f32::from_bits(0xc2a397de)),
    );
    path.line_to((f32::from_bits(0x4122e164), f32::from_bits(0xc26c8535)));
    path.cubic_to(
        (f32::from_bits(0x411dfe19), f32::from_bits(0xc26cbb11)),
        (f32::from_bits(0x41191928), f32::from_bits(0xc26cee7f)),
        (f32::from_bits(0x411432a9), f32::from_bits(0xc26d1f80)),
    );
    path.cubic_to(
        (f32::from_bits(0x40c65728), f32::from_bits(0xc26f09c3)),
        (f32::from_bits(0x40467d64), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41615164), f32::from_bits(0xc2a397de)));
    path.cubic_to(
        (f32::from_bits(0x41a78432), f32::from_bits(0xc2a13b6d)),
        (f32::from_bits(0x41dcf7f2), f32::from_bits(0xc29d27e8)),
        (f32::from_bits(0x4207e0f5), f32::from_bits(0xc29775db)),
    );
    path.line_to((f32::from_bits(0x41c47380), f32::from_bits(0xc25afa96)));
    path.cubic_to(
        (f32::from_bits(0x419fbc7e), f32::from_bits(0xc263369d)),
        (f32::from_bits(0x41723143), f32::from_bits(0xc2691b52)),
        (f32::from_bits(0x4122e168), f32::from_bits(0xc26c8537)),
    );
    path.line_to((f32::from_bits(0x41615164), f32::from_bits(0xc2a397de)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3169-L3191 (chrome/m156)
fn battleOp118(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40a2e582), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4122b94f), f32::from_bits(0xc2a51039)),
        (f32::from_bits(0x4172cca0), f32::from_bits(0xc2a333b4)),
    );
    path.line_to((f32::from_bits(0x412f847d), f32::from_bits(0xc26bf464)));
    path.cubic_to(
        (f32::from_bits(0x40eb4376), f32::from_bits(0xc26ea556)),
        (f32::from_bits(0x406b836d), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4172cc9b), f32::from_bits(0xc2a333b4)));
    path.cubic_to(
        (f32::from_bits(0x417acd1a), f32::from_bits(0xc2a30415)),
        (f32::from_bits(0x41816508), f32::from_bits(0xc2a2d21d)),
        (f32::from_bits(0x4185619b), f32::from_bits(0xc2a29dcb)),
    );
    path.line_to((f32::from_bits(0x4140d724), f32::from_bits(0xc26b1ba8)));
    path.cubic_to(
        (f32::from_bits(0x413b139d), f32::from_bits(0xc26b674c)),
        (f32::from_bits(0x41354d54), f32::from_bits(0xc26baf8b)),
        (f32::from_bits(0x412f847c), f32::from_bits(0xc26bf463)),
    );
    path.line_to((f32::from_bits(0x4172cc9b), f32::from_bits(0xc2a333b4)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3194-L3220 (chrome/m156)
fn battleOp119(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40a2e57f), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4122b94c), f32::from_bits(0xc2a51039)),
        (f32::from_bits(0x4172cc9b), f32::from_bits(0xc2a333b4)),
    );
    path.line_to((f32::from_bits(0x4172cca0), f32::from_bits(0xc2a333b4)));
    path.cubic_to(
        (f32::from_bits(0x417acd1d), f32::from_bits(0xc2a30415)),
        (f32::from_bits(0x41816509), f32::from_bits(0xc2a2d21d)),
        (f32::from_bits(0x4185619b), f32::from_bits(0xc2a29dcb)),
    );
    path.line_to((f32::from_bits(0x4140d724), f32::from_bits(0xc26b1ba8)));
    path.cubic_to(
        (f32::from_bits(0x413b139d), f32::from_bits(0xc26b674c)),
        (f32::from_bits(0x41354d54), f32::from_bits(0xc26baf8b)),
        (f32::from_bits(0x412f847c), f32::from_bits(0xc26bf463)),
    );
    path.line_to((f32::from_bits(0x412f847d), f32::from_bits(0xc26bf464)));
    path.cubic_to(
        (f32::from_bits(0x40eb4376), f32::from_bits(0xc26ea556)),
        (f32::from_bits(0x406b836d), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4185619b), f32::from_bits(0xc2a29dcc)));
    path.cubic_to(
        (f32::from_bits(0x41c61a92), f32::from_bits(0xc29f4c69)),
        (f32::from_bits(0x42023dd6), f32::from_bits(0xc299958f)),
        (f32::from_bits(0x421f3a98), f32::from_bits(0xc291a994)),
    );
    path.line_to((f32::from_bits(0x41e635e1), f32::from_bits(0xc25298a5)));
    path.cubic_to(
        (f32::from_bits(0x41bc4d11), f32::from_bits(0xc25e0caa)),
        (f32::from_bits(0x418f3524), f32::from_bits(0xc2664fa2)),
        (f32::from_bits(0x4140d729), f32::from_bits(0xc26b1ba9)),
    );
    path.line_to((f32::from_bits(0x4185619b), f32::from_bits(0xc2a29dcc)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3223-L3245 (chrome/m156)
fn battleOp120(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40c39389), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x414346f4), f32::from_bits(0xc2a4a65f)),
        (f32::from_bits(0x419158cf), f32::from_bits(0xc2a1f965)),
    );
    path.line_to((f32::from_bits(0x415223e0), f32::from_bits(0xc26a2df8)));
    path.cubic_to(
        (f32::from_bits(0x410d2a0c), f32::from_bits(0xc26e0c4b)),
        (f32::from_bits(0x408d616c), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x35bbfd46), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x419158d0), f32::from_bits(0xc2a1f965)));
    path.cubic_to(
        (f32::from_bits(0x41961cea), f32::from_bits(0xc2a1b4f6)),
        (f32::from_bits(0x419addf6), f32::from_bits(0xc2a16d2c)),
        (f32::from_bits(0x419f9bbb), f32::from_bits(0xc2a12207)),
    );
    path.line_to((f32::from_bits(0x4166c251), f32::from_bits(0xc268f69a)));
    path.cubic_to(
        (f32::from_bits(0x415fe778), f32::from_bits(0xc269633e)),
        (f32::from_bits(0x415907e2), f32::from_bits(0xc269cb09)),
        (f32::from_bits(0x415223e0), f32::from_bits(0xc26a2df8)),
    );
    path.line_to((f32::from_bits(0x419158d0), f32::from_bits(0xc2a1f965)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3248-L3272 (chrome/m156)
fn battleOp121(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40c39389), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x414346f4), f32::from_bits(0xc2a4a65f)),
        (f32::from_bits(0x419158d0), f32::from_bits(0xc2a1f965)),
    );
    path.cubic_to(
        (f32::from_bits(0x41961cea), f32::from_bits(0xc2a1b4f6)),
        (f32::from_bits(0x419addf6), f32::from_bits(0xc2a16d2c)),
        (f32::from_bits(0x419f9bbb), f32::from_bits(0xc2a12207)),
    );
    path.line_to((f32::from_bits(0x4166c251), f32::from_bits(0xc268f69a)));
    path.cubic_to(
        (f32::from_bits(0x415fe778), f32::from_bits(0xc269633e)),
        (f32::from_bits(0x415907e2), f32::from_bits(0xc269cb09)),
        (f32::from_bits(0x415223e0), f32::from_bits(0xc26a2df8)),
    );
    path.cubic_to(
        (f32::from_bits(0x410d2a0c), f32::from_bits(0xc26e0c4b)),
        (f32::from_bits(0x408d616c), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x419f9bbc), f32::from_bits(0xc2a12208)));
    path.cubic_to(
        (f32::from_bits(0x41eca53e), f32::from_bits(0xc29c5d1a)),
        (f32::from_bits(0x421ad1be), f32::from_bits(0xc2942e2b)),
        (f32::from_bits(0x423b8fe1), f32::from_bits(0xc288f8a3)),
    );
    path.line_to((f32::from_bits(0x42079647), f32::from_bits(0xc24607dc)));
    path.cubic_to(
        (f32::from_bits(0x41dfd5cc), f32::from_bits(0xc2563c94)),
        (f32::from_bits(0x41ab11aa), f32::from_bits(0xc2621167)),
        (f32::from_bits(0x4166c24e), f32::from_bits(0xc268f69b)),
    );
    path.line_to((f32::from_bits(0x419f9bbc), f32::from_bits(0xc2a12208)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3275-L3297 (chrome/m156)
fn battleOp122(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x410a1653), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4189aa2f), f32::from_bits(0xc2a34ed0)),
        (f32::from_bits(0x41cb63be), f32::from_bits(0xc29e054b)),
    );
    path.line_to((f32::from_bits(0x41930758), f32::from_bits(0xc26476b2)));
    path.cubic_to(
        (f32::from_bits(0x41470896), f32::from_bits(0xc26c1b98)),
        (f32::from_bits(0x40c7a4f2), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea3), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41cb63c3), f32::from_bits(0xc29e054c)));
    path.cubic_to(
        (f32::from_bits(0x41d1f2f3), f32::from_bits(0xc29d7e37)),
        (f32::from_bits(0x41d879a0), f32::from_bits(0xc29cf09c)),
        (f32::from_bits(0x41def72d), f32::from_bits(0xc29c5c87)),
    );
    path.line_to((f32::from_bits(0x41a12e10), f32::from_bits(0xc2621091)));
    path.cubic_to(
        (f32::from_bits(0x419c7cee), f32::from_bits(0xc262e6aa)),
        (f32::from_bits(0x4197c536), f32::from_bits(0xc263b366)),
        (f32::from_bits(0x41930757), f32::from_bits(0xc26476b3)),
    );
    path.line_to((f32::from_bits(0x41cb63c3), f32::from_bits(0xc29e054c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3300-L3326 (chrome/m156)
fn battleOp123(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea3), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x410a1653), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4189aa2f), f32::from_bits(0xc2a34ed0)),
        (f32::from_bits(0x41cb63be), f32::from_bits(0xc29e054b)),
    );
    path.line_to((f32::from_bits(0x41cb63c3), f32::from_bits(0xc29e054c)));
    path.cubic_to(
        (f32::from_bits(0x41d1f2f3), f32::from_bits(0xc29d7e37)),
        (f32::from_bits(0x41d879a0), f32::from_bits(0xc29cf09c)),
        (f32::from_bits(0x41def72d), f32::from_bits(0xc29c5c87)),
    );
    path.line_to((f32::from_bits(0x41a12e10), f32::from_bits(0xc2621091)));
    path.cubic_to(
        (f32::from_bits(0x419c7cee), f32::from_bits(0xc262e6aa)),
        (f32::from_bits(0x4197c536), f32::from_bits(0xc263b366)),
        (f32::from_bits(0x41930757), f32::from_bits(0xc26476b3)),
    );
    path.line_to((f32::from_bits(0x41930758), f32::from_bits(0xc26476b2)));
    path.cubic_to(
        (f32::from_bits(0x41470896), f32::from_bits(0xc26c1b98)),
        (f32::from_bits(0x40c7a4f2), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea3), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41def730), f32::from_bits(0xc29c5c87)));
    path.cubic_to(
        (f32::from_bits(0x422459f2), f32::from_bits(0xc292f017)),
        (f32::from_bits(0x42539427), f32::from_bits(0xc282f764)),
        (f32::from_bits(0x4278c050), f32::from_bits(0xc25be110)),
    );
    path.line_to((f32::from_bits(0x4233d1f5), f32::from_bits(0xc21ef2e3)));
    path.cubic_to(
        (f32::from_bits(0x4218f2cf), f32::from_bits(0xc23d5956)),
        (f32::from_bits(0x41ed9dce), f32::from_bits(0xc25470b6)),
        (f32::from_bits(0x41a12e11), f32::from_bits(0xc2621092)),
    );
    path.line_to((f32::from_bits(0x41def730), f32::from_bits(0xc29c5c87)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3329-L3351 (chrome/m156)
fn battleOp124(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x411fc00b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x419f1845), f32::from_bits(0xc2a265a5)),
        (f32::from_bits(0x41e9da2b), f32::from_bits(0xc29b5d43)),
    );
    path.line_to((f32::from_bits(0x41a90cc1), f32::from_bits(0xc2609f84)));
    path.cubic_to(
        (f32::from_bits(0x41660440), f32::from_bits(0xc26aca7c)),
        (f32::from_bits(0x40e6f6cd), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa8c), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41e9da2e), f32::from_bits(0xc29b5d44)));
    path.cubic_to(
        (f32::from_bits(0x41f14eda), f32::from_bits(0xc29aa9b5)),
        (f32::from_bits(0x41f8b671), f32::from_bits(0xc299ed94)),
        (f32::from_bits(0x42000805), f32::from_bits(0xc29928f7)),
    );
    path.line_to((f32::from_bits(0x41b91b05), f32::from_bits(0xc25d6faa)));
    path.cubic_to(
        (f32::from_bits(0x41b3cad4), f32::from_bits(0xc25e8bec)),
        (f32::from_bits(0x41ae7086), f32::from_bits(0xc25f9beb)),
        (f32::from_bits(0x41a90cc3), f32::from_bits(0xc2609f85)),
    );
    path.line_to((f32::from_bits(0x41e9da2e), f32::from_bits(0xc29b5d44)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3354-L3378 (chrome/m156)
fn battleOp125(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x411fc00b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x419f1845), f32::from_bits(0xc2a265a5)),
        (f32::from_bits(0x41e9da2e), f32::from_bits(0xc29b5d44)),
    );
    path.cubic_to(
        (f32::from_bits(0x41f14eda), f32::from_bits(0xc29aa9b5)),
        (f32::from_bits(0x41f8b671), f32::from_bits(0xc299ed94)),
        (f32::from_bits(0x42000805), f32::from_bits(0xc29928f7)),
    );
    path.line_to((f32::from_bits(0x41b91b05), f32::from_bits(0xc25d6faa)));
    path.cubic_to(
        (f32::from_bits(0x41b3cad4), f32::from_bits(0xc25e8bec)),
        (f32::from_bits(0x41ae7086), f32::from_bits(0xc25f9beb)),
        (f32::from_bits(0x41a90cc1), f32::from_bits(0xc2609f84)),
    );
    path.cubic_to(
        (f32::from_bits(0x41660440), f32::from_bits(0xc26aca7c)),
        (f32::from_bits(0x40e6f6cd), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42000806), f32::from_bits(0xc29928f8)));
    path.cubic_to(
        (f32::from_bits(0x423c0231), f32::from_bits(0xc28ca034)),
        (f32::from_bits(0x426f4e95), f32::from_bits(0xc26f2095)),
        (f32::from_bits(0x4289c821), f32::from_bits(0xc2392c12)),
    );
    path.line_to((f32::from_bits(0x424733db), f32::from_bits(0xc205dc02)));
    path.cubic_to(
        (f32::from_bits(0x422cfe35), f32::from_bits(0xc22cdcf5)),
        (f32::from_bits(0x4207e8ea), f32::from_bits(0xc24b507f)),
        (f32::from_bits(0x41b91b06), f32::from_bits(0xc25d6faa)),
    );
    path.line_to((f32::from_bits(0x42000806), f32::from_bits(0xc29928f8)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3381-L3403 (chrome/m156)
fn battleOp126(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41379cd4), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x41b69d77), f32::from_bits(0xc2a13d93)),
        (f32::from_bits(0x42055871), f32::from_bits(0xc29805ae)),
    );
    path.line_to((f32::from_bits(0x41c0c9e6), f32::from_bits(0xc25bca86)));
    path.cubic_to(
        (f32::from_bits(0x418402cc), f32::from_bits(0xc2691e6b)),
        (f32::from_bits(0x4104bb66), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3673fea5), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42055872), f32::from_bits(0xc29805ae)));
    path.cubic_to(
        (f32::from_bits(0x420988d2), f32::from_bits(0xc2971a85)),
        (f32::from_bits(0x420daf5c), f32::from_bits(0xc296244f)),
        (f32::from_bits(0x4211cb64), f32::from_bits(0xc2952332)),
    );
    path.line_to((f32::from_bits(0x41d2c988), f32::from_bits(0xc2579ed7)));
    path.cubic_to(
        (f32::from_bits(0x41ccd887), f32::from_bits(0xc2591291)),
        (f32::from_bits(0x41c6d852), f32::from_bits(0xc25a7689)),
        (f32::from_bits(0x41c0c9e6), f32::from_bits(0xc25bca86)),
    );
    path.line_to((f32::from_bits(0x42055872), f32::from_bits(0xc29805ae)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3406-L3430 (chrome/m156)
fn battleOp127(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3673fea5), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41379cd4), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x41b69d77), f32::from_bits(0xc2a13d93)),
        (f32::from_bits(0x42055872), f32::from_bits(0xc29805ae)),
    );
    path.cubic_to(
        (f32::from_bits(0x420988d2), f32::from_bits(0xc2971a85)),
        (f32::from_bits(0x420daf5c), f32::from_bits(0xc296244f)),
        (f32::from_bits(0x4211cb64), f32::from_bits(0xc2952332)),
    );
    path.line_to((f32::from_bits(0x41d2c988), f32::from_bits(0xc2579ed7)));
    path.cubic_to(
        (f32::from_bits(0x41ccd887), f32::from_bits(0xc2591291)),
        (f32::from_bits(0x41c6d852), f32::from_bits(0xc25a7689)),
        (f32::from_bits(0x41c0c9e6), f32::from_bits(0xc25bca86)),
    );
    path.cubic_to(
        (f32::from_bits(0x418402cc), f32::from_bits(0xc2691e6b)),
        (f32::from_bits(0x4104bb66), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3673fea5), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4211cb65), f32::from_bits(0xc2952332)));
    path.cubic_to(
        (f32::from_bits(0x42550406), f32::from_bits(0xc284b578)),
        (f32::from_bits(0x42859569), f32::from_bits(0xc252d13a)),
        (f32::from_bits(0x4295bbf4), f32::from_bits(0xc20f53bf)),
    );
    path.line_to((f32::from_bits(0x42587bb2), f32::from_bits(0xc1cf3850)));
    path.cubic_to(
        (f32::from_bits(0x4241220a), f32::from_bits(0xc21865e8)),
        (f32::from_bits(0x4219fcbd), f32::from_bits(0xc23fde48)),
        (f32::from_bits(0x41d2c988), f32::from_bits(0xc2579ed8)),
    );
    path.line_to((f32::from_bits(0x4211cb65), f32::from_bits(0xc2952332)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3433-L3455 (chrome/m156)
fn battleOp128(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4151cd59), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x41d04f3f), f32::from_bits(0xc29fc954)),
        (f32::from_bits(0x4216e058), f32::from_bits(0xc293de54)),
    );
    path.line_to((f32::from_bits(0x41da226b), f32::from_bits(0xc255c926)));
    path.cubic_to(
        (f32::from_bits(0x419695d1), f32::from_bits(0xc267043d)),
        (f32::from_bits(0x4117aa0a), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4216e057), f32::from_bits(0xc293de54)));
    path.cubic_to(
        (f32::from_bits(0x421b86ea), f32::from_bits(0xc292aea0)),
        (f32::from_bits(0x42201eff), f32::from_bits(0xc29170ed)),
        (f32::from_bits(0x4224a79b), f32::from_bits(0xc290257e)),
    );
    path.line_to((f32::from_bits(0x41ee0e15), f32::from_bits(0xc2506790)));
    path.cubic_to(
        (f32::from_bits(0x41e78019), f32::from_bits(0xc25246bf)),
        (f32::from_bits(0x41e0dbbc), f32::from_bits(0xc2541212)),
        (f32::from_bits(0x41da226b), f32::from_bits(0xc255c927)),
    );
    path.line_to((f32::from_bits(0x4216e057), f32::from_bits(0xc293de54)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3458-L3484 (chrome/m156)
fn battleOp129(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4151cd58), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x41d04f3d), f32::from_bits(0xc29fc954)),
        (f32::from_bits(0x4216e057), f32::from_bits(0xc293de54)),
    );
    path.line_to((f32::from_bits(0x4216e058), f32::from_bits(0xc293de54)));
    path.cubic_to(
        (f32::from_bits(0x421b86eb), f32::from_bits(0xc292aea0)),
        (f32::from_bits(0x42201eff), f32::from_bits(0xc29170ed)),
        (f32::from_bits(0x4224a79b), f32::from_bits(0xc290257e)),
    );
    path.line_to((f32::from_bits(0x41ee0e15), f32::from_bits(0xc2506790)));
    path.cubic_to(
        (f32::from_bits(0x41e78019), f32::from_bits(0xc25246bf)),
        (f32::from_bits(0x41e0dbbc), f32::from_bits(0xc2541212)),
        (f32::from_bits(0x41da226b), f32::from_bits(0xc255c927)),
    );
    path.line_to((f32::from_bits(0x41da226b), f32::from_bits(0xc255c926)));
    path.cubic_to(
        (f32::from_bits(0x419695d1), f32::from_bits(0xc267043d)),
        (f32::from_bits(0x4117aa0a), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4224a79b), f32::from_bits(0xc290257f)));
    path.cubic_to(
        (f32::from_bits(0x426f06c3), f32::from_bits(0xc275d105)),
        (f32::from_bits(0x42930d85), f32::from_bits(0xc2303df6)),
        (f32::from_bits(0x429f3103), f32::from_bits(0xc1bc373f)),
    );
    path.line_to((f32::from_bits(0x42662806), f32::from_bits(0xc1880f44)));
    path.cubic_to(
        (f32::from_bits(0x42549b44), f32::from_bits(0xc1fececc)),
        (f32::from_bits(0x422cca4c), f32::from_bits(0xc231b2de)),
        (f32::from_bits(0x41ee0e18), f32::from_bits(0xc2506792)),
    );
    path.line_to((f32::from_bits(0x4224a79b), f32::from_bits(0xc290257f)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3487-L3509 (chrome/m156)
fn battleOp130(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x417054a2), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x41ee1405), f32::from_bits(0xc29dd904)),
        (f32::from_bits(0x422a9595), f32::from_bits(0xc28e6989)),
    );
    path.line_to((f32::from_bits(0x41f6a0c0), f32::from_bits(0xc24de5b0)));
    path.cubic_to(
        (f32::from_bits(0x41ac1ad0), f32::from_bits(0xc26436ad)),
        (f32::from_bits(0x412dbba0), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x422a9596), f32::from_bits(0xc28e6989)));
    path.cubic_to(
        (f32::from_bits(0x422fb535), f32::from_bits(0xc28ce0c4)),
        (f32::from_bits(0x4234bf65), f32::from_bits(0xc28b465e)),
        (f32::from_bits(0x4239b2bc), f32::from_bits(0xc2899acc)),
    );
    path.line_to((f32::from_bits(0x42063d5a), f32::from_bits(0xc246f24e)));
    path.cubic_to(
        (f32::from_bits(0x4202a934), f32::from_bits(0xc2495c7c)),
        (f32::from_bits(0x41fe0912), f32::from_bits(0xc24badd5)),
        (f32::from_bits(0x41f6a0c0), f32::from_bits(0xc24de5b1)),
    );
    path.line_to((f32::from_bits(0x422a9596), f32::from_bits(0xc28e6989)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3512-L3536 (chrome/m156)
fn battleOp131(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb630015b), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x417054a2), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x41ee1405), f32::from_bits(0xc29dd904)),
        (f32::from_bits(0x422a9596), f32::from_bits(0xc28e6989)),
    );
    path.cubic_to(
        (f32::from_bits(0x422fb535), f32::from_bits(0xc28ce0c4)),
        (f32::from_bits(0x4234bf65), f32::from_bits(0xc28b465e)),
        (f32::from_bits(0x4239b2bc), f32::from_bits(0xc2899acc)),
    );
    path.line_to((f32::from_bits(0x42063d5a), f32::from_bits(0xc246f24e)));
    path.cubic_to(
        (f32::from_bits(0x4202a934), f32::from_bits(0xc2495c7c)),
        (f32::from_bits(0x41fe0912), f32::from_bits(0xc24badd5)),
        (f32::from_bits(0x41f6a0c0), f32::from_bits(0xc24de5b0)),
    );
    path.cubic_to(
        (f32::from_bits(0x41ac1ad0), f32::from_bits(0xc26436ad)),
        (f32::from_bits(0x412dbba0), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4239b2bd), f32::from_bits(0xc2899acc)));
    path.cubic_to(
        (f32::from_bits(0x42859c2b), f32::from_bits(0xc25c33ca)),
        (f32::from_bits(0x42a01474), f32::from_bits(0xc203e23a)),
        (f32::from_bits(0x42a51fce), f32::from_bits(0xc1083bae)),
    );
    path.line_to((f32::from_bits(0x426ebbdb), f32::from_bits(0xc0c4f6ab)));
    path.cubic_to(
        (f32::from_bits(0x426770d9), f32::from_bits(0xc1beacda)),
        (f32::from_bits(0x42412bce), f32::from_bits(0xc21f2eb0)),
        (f32::from_bits(0x42063d5a), f32::from_bits(0xc246f24e)),
    );
    path.line_to((f32::from_bits(0x4239b2bd), f32::from_bits(0xc2899acc)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3539-L3561 (chrome/m156)
fn battleOp132(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4187e175), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42063ec3), f32::from_bits(0xc29b93fb)),
        (f32::from_bits(0x423df6fd), f32::from_bits(0xc2882410)),
    );
    path.line_to((f32::from_bits(0x420952ef), f32::from_bits(0xc244d488)));
    path.cubic_to(
        (f32::from_bits(0x41c216e4), f32::from_bits(0xc260eea0)),
        (f32::from_bits(0x4144743c), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423df6fe), f32::from_bits(0xc2882411)));
    path.cubic_to(
        (f32::from_bits(0x42437e7a), f32::from_bits(0xc286364a)),
        (f32::from_bits(0x4248e78f), f32::from_bits(0xc2843312)),
        (f32::from_bits(0x424e304d), f32::from_bits(0xc2821b20)),
    );
    path.line_to((f32::from_bits(0x42150d53), f32::from_bits(0xc23c1ae0)));
    path.cubic_to(
        (f32::from_bits(0x42113b72), f32::from_bits(0xc23f21be)),
        (f32::from_bits(0x420d522e), f32::from_bits(0xc2420aa4)),
        (f32::from_bits(0x420952ef), f32::from_bits(0xc244d48a)),
    );
    path.line_to((f32::from_bits(0x423df6fe), f32::from_bits(0xc2882411)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3564-L3589 (chrome/m156)
fn battleOp133(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4187e175), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42063ec3), f32::from_bits(0xc29b93fb)),
        (f32::from_bits(0x423df6fe), f32::from_bits(0xc2882411)),
    );
    path.cubic_to(
        (f32::from_bits(0x42437e7a), f32::from_bits(0xc286364a)),
        (f32::from_bits(0x4248e78f), f32::from_bits(0xc2843312)),
        (f32::from_bits(0x424e304d), f32::from_bits(0xc2821b20)),
    );
    path.line_to((f32::from_bits(0x42150d53), f32::from_bits(0xc23c1ae0)));
    path.cubic_to(
        (f32::from_bits(0x42113b72), f32::from_bits(0xc23f21be)),
        (f32::from_bits(0x420d522e), f32::from_bits(0xc2420aa4)),
        (f32::from_bits(0x420952ef), f32::from_bits(0xc244d48a)),
    );
    path.line_to((f32::from_bits(0x420952ef), f32::from_bits(0xc244d488)));
    path.cubic_to(
        (f32::from_bits(0x41c216e4), f32::from_bits(0xc260eea0)),
        (f32::from_bits(0x4144743c), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x424e304d), f32::from_bits(0xc2821b20)));
    path.cubic_to(
        (f32::from_bits(0x4292cbf1), f32::from_bits(0xc23ef41d)),
        (f32::from_bits(0x42aa31a6), f32::from_bits(0xc1a4e14c)),
        (f32::from_bits(0x42a56158), f32::from_bits(0x40e54b3a)),
    );
    path.line_to((f32::from_bits(0x426f1a9e), f32::from_bits(0x40a5c12f)));
    path.cubic_to(
        (f32::from_bits(0x42761044), f32::from_bits(0xc16e617c)),
        (f32::from_bits(0x42543c73), f32::from_bits(0xc20a09ea)),
        (f32::from_bits(0x42150d54), f32::from_bits(0xc23c1ae1)),
    );
    path.line_to((f32::from_bits(0x424e304d), f32::from_bits(0xc2821b20)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3592-L3614 (chrome/m156)
fn battleOp134(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x419c5b1f), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4219d929), f32::from_bits(0xc29834b3)),
        (f32::from_bits(0x4255ae76), f32::from_bits(0xc27e184c)),
    );
    path.line_to((f32::from_bits(0x421a77f2), f32::from_bits(0xc237aede)));
    path.cubic_to(
        (f32::from_bits(0x41de6e66), f32::from_bits(0xc25c0e82)),
        (f32::from_bits(0x41620e8a), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4255ae76), f32::from_bits(0xc27e184c)));
    path.cubic_to(
        (f32::from_bits(0x425b9ab5), f32::from_bits(0xc2791d33)),
        (f32::from_bits(0x426159ea), f32::from_bits(0xc273ed7b)),
        (f32::from_bits(0x4266e960), f32::from_bits(0xc26e8b92)),
    );
    path.line_to((f32::from_bits(0x4226ec90), f32::from_bits(0xc22c713c)));
    path.cubic_to(
        (f32::from_bits(0x4222e78d), f32::from_bits(0xc2305550)),
        (f32::from_bits(0x421ec008), f32::from_bits(0xc234151d)),
        (f32::from_bits(0x421a77f3), f32::from_bits(0xc237aedd)),
    );
    path.line_to((f32::from_bits(0x4255ae76), f32::from_bits(0xc27e184c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3617-L3641 (chrome/m156)
fn battleOp135(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x419c5b1f), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4219d929), f32::from_bits(0xc29834b3)),
        (f32::from_bits(0x4255ae76), f32::from_bits(0xc27e184c)),
    );
    path.cubic_to(
        (f32::from_bits(0x425b9ab5), f32::from_bits(0xc2791d33)),
        (f32::from_bits(0x426159ea), f32::from_bits(0xc273ed7b)),
        (f32::from_bits(0x4266e960), f32::from_bits(0xc26e8b92)),
    );
    path.line_to((f32::from_bits(0x4226ec90), f32::from_bits(0xc22c713c)));
    path.cubic_to(
        (f32::from_bits(0x4222e78d), f32::from_bits(0xc2305550)),
        (f32::from_bits(0x421ec008), f32::from_bits(0xc234151d)),
        (f32::from_bits(0x421a77f2), f32::from_bits(0xc237aede)),
    );
    path.cubic_to(
        (f32::from_bits(0x41de6e66), f32::from_bits(0xc25c0e82)),
        (f32::from_bits(0x41620e8a), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4266e961), f32::from_bits(0xc26e8b93)));
    path.cubic_to(
        (f32::from_bits(0x42a1bfce), f32::from_bits(0xc214ebcf)),
        (f32::from_bits(0x42b1ee5a), f32::from_bits(0xc05d1412)),
        (f32::from_bits(0x429cf75a), f32::from_bits(0x41d80f2c)),
    );
    path.line_to((f32::from_bits(0x4262f06b), f32::from_bits(0x419c2ffb)));
    path.cubic_to(
        (f32::from_bits(0x42809ff9), f32::from_bits(0xc01fd0e5)),
        (f32::from_bits(0x4269dab8), f32::from_bits(0xc1d74ec6)),
        (f32::from_bits(0x4226ec91), f32::from_bits(0xc22c713d)),
    );
    path.line_to((f32::from_bits(0x4266e961), f32::from_bits(0xc26e8b93)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3644-L3666 (chrome/m156)
fn battleOp136(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41ae0130), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x422a8737), f32::from_bits(0xc294ec91)),
        (f32::from_bits(0x42689b67), f32::from_bits(0xc26ce46c)),
    );
    path.line_to((f32::from_bits(0x42282651), f32::from_bits(0xc22b3f58)));
    path.cubic_to(
        (f32::from_bits(0x41f68bfb), f32::from_bits(0xc2574fdc)),
        (f32::from_bits(0x417b92b3), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42689b68), f32::from_bits(0xc26ce46d)));
    path.cubic_to(
        (f32::from_bits(0x426ebcd2), f32::from_bits(0xc266df67)),
        (f32::from_bits(0x4274a1d2), f32::from_bits(0xc2609e09)),
        (f32::from_bits(0x427a4701), f32::from_bits(0xc25a23f2)),
    );
    path.line_to((f32::from_bits(0x4234ec64), f32::from_bits(0xc21db11e)));
    path.cubic_to(
        (f32::from_bits(0x4230d7ae), f32::from_bits(0xc2225fbc)),
        (f32::from_bits(0x422c94d6), f32::from_bits(0xc226e55a)),
        (f32::from_bits(0x42282652), f32::from_bits(0xc22b3f58)),
    );
    path.line_to((f32::from_bits(0x42689b68), f32::from_bits(0xc26ce46d)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3669-L3693 (chrome/m156)
fn battleOp137(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41ae0130), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x422a8737), f32::from_bits(0xc294ec91)),
        (f32::from_bits(0x42689b68), f32::from_bits(0xc26ce46d)),
    );
    path.cubic_to(
        (f32::from_bits(0x426ebcd2), f32::from_bits(0xc266df67)),
        (f32::from_bits(0x4274a1d2), f32::from_bits(0xc2609e09)),
        (f32::from_bits(0x427a4701), f32::from_bits(0xc25a23f2)),
    );
    path.line_to((f32::from_bits(0x4234ec64), f32::from_bits(0xc21db11e)));
    path.cubic_to(
        (f32::from_bits(0x4230d7ae), f32::from_bits(0xc2225fbc)),
        (f32::from_bits(0x422c94d6), f32::from_bits(0xc226e55a)),
        (f32::from_bits(0x42282651), f32::from_bits(0xc22b3f58)),
    );
    path.cubic_to(
        (f32::from_bits(0x41f68bfb), f32::from_bits(0xc2574fdc)),
        (f32::from_bits(0x417b92b3), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x427a4702), f32::from_bits(0xc25a23f2)));
    path.cubic_to(
        (f32::from_bits(0x42ac7185), f32::from_bits(0xc1db2f83)),
        (f32::from_bits(0x42b35ed0), f32::from_bits(0x413e447a)),
        (f32::from_bits(0x428e4a3d), f32::from_bits(0x422afde8)),
    );
    path.line_to((f32::from_bits(0x424db871), f32::from_bits(0x41f73799)));
    path.cubic_to(
        (f32::from_bits(0x4281aa54), f32::from_bits(0x41098afa)),
        (f32::from_bits(0x427950da), f32::from_bits(0xc19e728d)),
        (f32::from_bits(0x4234ec66), f32::from_bits(0xc21db120)),
    );
    path.line_to((f32::from_bits(0x427a4702), f32::from_bits(0xc25a23f2)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3696-L3718 (chrome/m156)
fn battleOp138(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41c2602d), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x423d7ece), f32::from_bits(0xc290b51a)),
        (f32::from_bits(0x427c92bc), f32::from_bits(0xc2577a5f)),
    );
    path.line_to((f32::from_bits(0x42369543), f32::from_bits(0xc21bc469)));
    path.cubic_to(
        (f32::from_bits(0x4208fc10), f32::from_bits(0xc2513731)),
        (f32::from_bits(0x418c8338), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x427c92be), f32::from_bits(0xc2577a5f)));
    path.cubic_to(
        (f32::from_bits(0x42816448), f32::from_bits(0xc25032db)),
        (f32::from_bits(0x42845689), f32::from_bits(0xc248a77c)),
        (f32::from_bits(0x42871e08), f32::from_bits(0xc240ddaa)),
    );
    path.line_to((f32::from_bits(0x424359af), f32::from_bits(0xc20b6bce)));
    path.cubic_to(
        (f32::from_bits(0x423f5505), f32::from_bits(0xc2110d1f)),
        (f32::from_bits(0x423b1287), f32::from_bits(0xc216814b)),
        (f32::from_bits(0x42369543), f32::from_bits(0xc21bc46a)),
    );
    path.line_to((f32::from_bits(0x427c92be), f32::from_bits(0xc2577a5f)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3721-L3747 (chrome/m156)
fn battleOp139(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41c2602d), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x423d7ece), f32::from_bits(0xc290b51a)),
        (f32::from_bits(0x427c92bc), f32::from_bits(0xc2577a5f)),
    );
    path.line_to((f32::from_bits(0x427c92be), f32::from_bits(0xc2577a5f)));
    path.cubic_to(
        (f32::from_bits(0x42816448), f32::from_bits(0xc25032db)),
        (f32::from_bits(0x42845689), f32::from_bits(0xc248a77c)),
        (f32::from_bits(0x42871e08), f32::from_bits(0xc240ddaa)),
    );
    path.line_to((f32::from_bits(0x424359af), f32::from_bits(0xc20b6bce)));
    path.cubic_to(
        (f32::from_bits(0x423f5505), f32::from_bits(0xc2110d1f)),
        (f32::from_bits(0x423b1287), f32::from_bits(0xc216814a)),
        (f32::from_bits(0x42369543), f32::from_bits(0xc21bc469)),
    );
    path.line_to((f32::from_bits(0x42369543), f32::from_bits(0xc21bc46a)));
    path.cubic_to(
        (f32::from_bits(0x4208fc10), f32::from_bits(0xc2513732)),
        (f32::from_bits(0x418c8337), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42871e08), f32::from_bits(0xc240ddaa)));
    path.cubic_to(
        (f32::from_bits(0x42b615a2), f32::from_bits(0xc174ff4e)),
        (f32::from_bits(0x42aecf41), f32::from_bits(0x41edcc49)),
        (f32::from_bits(0x426bc7a7), f32::from_bits(0x4269bc09)),
    );
    path.line_to((f32::from_bits(0x422a717e), f32::from_bits(0x4228f6f7)));
    path.cubic_to(
        (f32::from_bits(0x427cbca0), f32::from_bits(0x41abe6f4)),
        (f32::from_bits(0x4283a09b), f32::from_bits(0xc1311b44)),
        (f32::from_bits(0x424359af), f32::from_bits(0xc20b6bcd)),
    );
    path.line_to((f32::from_bits(0x42871e08), f32::from_bits(0xc240ddaa)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3750-L3772 (chrome/m156)
fn battleOp140(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41d9e52a), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4252f644), f32::from_bits(0xc28b460f)),
        (f32::from_bits(0x42887c98), f32::from_bits(0xc23cf83b)),
    );
    path.line_to((f32::from_bits(0x42455485), f32::from_bits(0xc2089ac5)));
    path.cubic_to(
        (f32::from_bits(0x421880ae), f32::from_bits(0xc2495c0a)),
        (f32::from_bits(0x419d83bb), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb560056c), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42887c98), f32::from_bits(0xc23cf83b)));
    path.cubic_to(
        (f32::from_bits(0x428b8706), f32::from_bits(0xc2342f4a)),
        (f32::from_bits(0x428e5ab7), f32::from_bits(0xc22b1c84)),
        (f32::from_bits(0x4290f525), f32::from_bits(0xc221c800)),
    );
    path.line_to((f32::from_bits(0x425193c7), f32::from_bits(0xc1e9e68d)));
    path.cubic_to(
        (f32::from_bits(0x424dd044), f32::from_bits(0xc1f763d3)),
        (f32::from_bits(0x4249b9f6), f32::from_bits(0xc2024108)),
        (f32::from_bits(0x42455485), f32::from_bits(0xc2089ac6)),
    );
    path.line_to((f32::from_bits(0x42887c98), f32::from_bits(0xc23cf83b)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3775-L3800 (chrome/m156)
fn battleOp141(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41d9e52a), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4252f644), f32::from_bits(0xc28b460f)),
        (f32::from_bits(0x42887c98), f32::from_bits(0xc23cf83b)),
    );
    path.cubic_to(
        (f32::from_bits(0x428b8706), f32::from_bits(0xc2342f4a)),
        (f32::from_bits(0x428e5ab7), f32::from_bits(0xc22b1c84)),
        (f32::from_bits(0x4290f525), f32::from_bits(0xc221c800)),
    );
    path.line_to((f32::from_bits(0x425193c7), f32::from_bits(0xc1e9e68d)));
    path.cubic_to(
        (f32::from_bits(0x424dd044), f32::from_bits(0xc1f763d3)),
        (f32::from_bits(0x4249b9f6), f32::from_bits(0xc2024107)),
        (f32::from_bits(0x42455485), f32::from_bits(0xc2089ac5)),
    );
    path.line_to((f32::from_bits(0x42455485), f32::from_bits(0xc2089ac6)));
    path.cubic_to(
        (f32::from_bits(0x421880ae), f32::from_bits(0xc2495c0b)),
        (f32::from_bits(0x419d83ba), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4290f526), f32::from_bits(0xc221c800)));
    path.cubic_to(
        (f32::from_bits(0x42bd6cdd), f32::from_bits(0xbf1a1474)),
        (f32::from_bits(0x42a13baa), f32::from_bits(0x4246de93)),
        (f32::from_bits(0x4223add7), f32::from_bits(0x42906c8a)),
    );
    path.line_to((f32::from_bits(0x41eca4f8), f32::from_bits(0x4250ce48)));
    path.cubic_to(
        (f32::from_bits(0x42691bac), f32::from_bits(0x420fc2d7)),
        (f32::from_bits(0x4288ef16), f32::from_bits(0xbedec420)),
        (f32::from_bits(0x425193c9), f32::from_bits(0xc1e9e690)),
    );
    path.line_to((f32::from_bits(0x4290f526), f32::from_bits(0xc221c800)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3803-L3825 (chrome/m156)
fn battleOp142(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f6a97d), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x426c7f9e), f32::from_bits(0xc283d12f)),
        (f32::from_bits(0x4292f07c), f32::from_bits(0xc21a76e5)),
    );
    path.line_to((f32::from_bits(0x42547147), f32::from_bits(0xc1df5274)));
    path.cubic_to(
        (f32::from_bits(0x422af677), f32::from_bits(0xc23e9438)),
        (f32::from_bits(0x41b24f58), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4292f07c), f32::from_bits(0xc21a76e5)));
    path.cubic_to(
        (f32::from_bits(0x4295bcf6), f32::from_bits(0xc20fd099)),
        (f32::from_bits(0x42983ed1), f32::from_bits(0xc204de6d)),
        (f32::from_bits(0x429a7333), f32::from_bits(0xc1f3598c)),
    );
    path.line_to((f32::from_bits(0x425f4d1c), f32::from_bits(0xc1afea60)));
    path.cubic_to(
        (f32::from_bits(0x425c1d22), f32::from_bits(0xc1c0197b)),
        (f32::from_bits(0x42587d28), f32::from_bits(0xc1cfecd2)),
        (f32::from_bits(0x42547148), f32::from_bits(0xc1df5275)),
    );
    path.line_to((f32::from_bits(0x4292f07c), f32::from_bits(0xc21a76e5)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3828-L3854 (chrome/m156)
fn battleOp143(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f6a97d), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x426c7f9e), f32::from_bits(0xc283d12f)),
        (f32::from_bits(0x4292f07c), f32::from_bits(0xc21a76e5)),
    );
    path.cubic_to(
        (f32::from_bits(0x4295bcf6), f32::from_bits(0xc20fd099)),
        (f32::from_bits(0x42983ed1), f32::from_bits(0xc204de6d)),
        (f32::from_bits(0x429a7333), f32::from_bits(0xc1f3598c)),
    );
    path.line_to((f32::from_bits(0x425f4d1c), f32::from_bits(0xc1afea60)));
    path.cubic_to(
        (f32::from_bits(0x425c1d22), f32::from_bits(0xc1c0197b)),
        (f32::from_bits(0x42587d28), f32::from_bits(0xc1cfecd2)),
        (f32::from_bits(0x42547147), f32::from_bits(0xc1df5274)),
    );
    path.cubic_to(
        (f32::from_bits(0x422af677), f32::from_bits(0xc23e9438)),
        (f32::from_bits(0x41b24f58), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429a7334), f32::from_bits(0xc1f3598d)));
    path.cubic_to(
        (f32::from_bits(0x42ac9a56), f32::from_bits(0xc0ec08d5)),
        (f32::from_bits(0x42a93a4b), f32::from_bits(0x4194209c)),
        (f32::from_bits(0x42913f11), f32::from_bits(0x4220bdeb)),
    );
    path.cubic_to(
        (f32::from_bits(0x427287b0), f32::from_bits(0x42776b87)),
        (f32::from_bits(0x421e5dc6), f32::from_bits(0x429a1372)),
        (f32::from_bits(0x4173f4a4), f32::from_bits(0x42a32ccd)),
    );
    path.line_to((f32::from_bits(0x41305a7f), f32::from_bits(0x426bea6b)));
    path.cubic_to(
        (f32::from_bits(0x41e4f69e), f32::from_bits(0x425ec2af)),
        (f32::from_bits(0x422f52ad), f32::from_bits(0x4232db9e)),
        (f32::from_bits(0x4251feaa), f32::from_bits(0x41e865df)),
    );
    path.cubic_to(
        (f32::from_bits(0x4274aaa7), f32::from_bits(0x41562902)),
        (f32::from_bits(0x42798bdd), f32::from_bits(0xc0aaa09a)),
        (f32::from_bits(0x425f4d1d), f32::from_bits(0xc1afea60)),
    );
    path.line_to((f32::from_bits(0x429a7334), f32::from_bits(0xc1f3598d)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3857-L3879 (chrome/m156)
fn battleOp144(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42079c39), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4280cb64), f32::from_bits(0xc279860f)),
        (f32::from_bits(0x429a0d79), f32::from_bits(0xc1f758df)),
    );
    path.line_to((f32::from_bits(0x425eba08), f32::from_bits(0xc1b2ce1f)));
    path.cubic_to(
        (f32::from_bits(0x423a357b), f32::from_bits(0xc23460ea)),
        (f32::from_bits(0x41c41023), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429a0d79), f32::from_bits(0xc1f758de)));
    path.cubic_to(
        (f32::from_bits(0x429c811b), f32::from_bits(0xc1deea6e)),
        (f32::from_bits(0x429e9731), f32::from_bits(0xc1c5ec3a)),
        (f32::from_bits(0x42a04ce7), f32::from_bits(0xc1ac8024)),
    );
    path.line_to((f32::from_bits(0x4267c277), f32::from_bits(0xc17965fc)));
    path.cubic_to(
        (f32::from_bits(0x426549a1), f32::from_bits(0xc18f13a3)),
        (f32::from_bits(0x42624575), f32::from_bits(0xc1a124d8)),
        (f32::from_bits(0x425eba09), f32::from_bits(0xc1b2ce1e)),
    );
    path.line_to((f32::from_bits(0x429a0d79), f32::from_bits(0xc1f758de)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3882-L3909 (chrome/m156)
fn battleOp145(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42079c39), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4280cb64), f32::from_bits(0xc279860f)),
        (f32::from_bits(0x429a0d79), f32::from_bits(0xc1f758df)),
    );
    path.line_to((f32::from_bits(0x42a04ce7), f32::from_bits(0xc1ac8024)));
    path.line_to((f32::from_bits(0x4267c277), f32::from_bits(0xc17965fc)));
    path.cubic_to(
        (f32::from_bits(0x426549a1), f32::from_bits(0xc18f13a3)),
        (f32::from_bits(0x42624575), f32::from_bits(0xc1a124d8)),
        (f32::from_bits(0x425eba09), f32::from_bits(0xc1b2ce1e)),
    );
    path.line_to((f32::from_bits(0x425eba08), f32::from_bits(0xc1b2ce1f)));
    path.cubic_to(
        (f32::from_bits(0x423a357b), f32::from_bits(0xc23460ea)),
        (f32::from_bits(0x41c41023), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a04ce8), f32::from_bits(0xc1ac8024)));
    path.cubic_to(
        (f32::from_bits(0x42ae6ca1), f32::from_bits(0x4095ff41)),
        (f32::from_bits(0x42a1f1fa), f32::from_bits(0x4202ed54)),
        (f32::from_bits(0x427dc9de), f32::from_bits(0x42560b98)),
    );
    path.cubic_to(
        (f32::from_bits(0x4237afc7), f32::from_bits(0x429494ee)),
        (f32::from_bits(0x419aa752), f32::from_bits(0x42aa57e8)),
        (f32::from_bits(0xc0f777b3), f32::from_bits(0x42a54724)),
    );
    path.line_to((f32::from_bits(0xc0b2e472), f32::from_bits(0x426ef4bb)));
    path.cubic_to(
        (f32::from_bits(0x415f9870), f32::from_bits(0x42764794)),
        (f32::from_bits(0x4204c916), f32::from_bits(0x4256d126)),
        (f32::from_bits(0x4237762a), f32::from_bits(0x421abb46)),
    );
    path.cubic_to(
        (f32::from_bits(0x426a233f), f32::from_bits(0x41bd4acb)),
        (f32::from_bits(0x427c2e04), f32::from_bits(0x4058dcfe)),
        (f32::from_bits(0x4267c279), f32::from_bits(0xc17965fc)),
    );
    path.line_to((f32::from_bits(0x42a04ce8), f32::from_bits(0xc1ac8024)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3912-L3934 (chrome/m156)
fn battleOp146(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x421472e7), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x428b6da4), f32::from_bits(0xc26973d7)),
        (f32::from_bits(0x429fb179), f32::from_bits(0xc1b54986)),
    );
    path.line_to((f32::from_bits(0x4266e1be), f32::from_bits(0xc1830d0f)));
    path.cubic_to(
        (f32::from_bits(0x42499544), f32::from_bits(0xc228c2c8)),
        (f32::from_bits(0x41d69ff6), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429fb179), f32::from_bits(0xc1b54988)));
    path.cubic_to(
        (f32::from_bits(0x42a1a632), f32::from_bits(0xc199b837)),
        (f32::from_bits(0x42a3282f), f32::from_bits(0xc17b594e)),
        (f32::from_bits(0x42a43501), f32::from_bits(0xc142a7ba)),
    );
    path.line_to((f32::from_bits(0x426d6865), f32::from_bits(0xc10cb6f0)));
    path.cubic_to(
        (f32::from_bits(0x426be3bc), f32::from_bits(0xc135b2ae)),
        (f32::from_bits(0x4269b5af), f32::from_bits(0xc15e3ec8)),
        (f32::from_bits(0x4266e1be), f32::from_bits(0xc1830d0f)),
    );
    path.line_to((f32::from_bits(0x429fb179), f32::from_bits(0xc1b54988)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3937-L3964 (chrome/m156)
fn battleOp147(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x421472e7), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x428b6da4), f32::from_bits(0xc26973d8)),
        (f32::from_bits(0x429fb179), f32::from_bits(0xc1b54988)),
    );
    path.line_to((f32::from_bits(0x429fb179), f32::from_bits(0xc1b54986)));
    path.cubic_to(
        (f32::from_bits(0x42a1a632), f32::from_bits(0xc199b836)),
        (f32::from_bits(0x42a3282f), f32::from_bits(0xc17b594d)),
        (f32::from_bits(0x42a43501), f32::from_bits(0xc142a7ba)),
    );
    path.line_to((f32::from_bits(0x426d6865), f32::from_bits(0xc10cb6f0)));
    path.cubic_to(
        (f32::from_bits(0x426be3bc), f32::from_bits(0xc135b2ae)),
        (f32::from_bits(0x4269b5af), f32::from_bits(0xc15e3ec8)),
        (f32::from_bits(0x4266e1be), f32::from_bits(0xc1830d0f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42499544), f32::from_bits(0xc228c2c8)),
        (f32::from_bits(0x41d69ff6), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a43502), f32::from_bits(0xc142a7bb)));
    path.cubic_to(
        (f32::from_bits(0x42ace9b0), f32::from_bits(0x4189ae79)),
        (f32::from_bits(0x429590d6), f32::from_bits(0x423ab1c1)),
        (f32::from_bits(0x424df762), f32::from_bits(0x428231a6)),
    );
    path.cubic_to(
        (f32::from_bits(0x41e19a31), f32::from_bits(0x42a70a69)),
        (f32::from_bits(0xc04a3289), f32::from_bits(0x42b03133)),
        (f32::from_bits(0xc1f5f36e), f32::from_bits(0x429a3139)),
    );
    path.line_to((f32::from_bits(0xc1b1cbb9), f32::from_bits(0x425eedb9)));
    path.cubic_to(
        (f32::from_bits(0xc0122aac), f32::from_bits(0x427ebc5a)),
        (f32::from_bits(0x41a31606), f32::from_bits(0x42718130)),
        (f32::from_bits(0x4214e430), f32::from_bits(0x423c3b73)),
    );
    path.cubic_to(
        (f32::from_bits(0x42583d5c), f32::from_bits(0x4206f5b6)),
        (f32::from_bits(0x4279fe97), f32::from_bits(0x41470ec8)),
        (f32::from_bits(0x426d6866), f32::from_bits(0xc10cb6eb)),
    );
    path.line_to((f32::from_bits(0x42a43502), f32::from_bits(0xc142a7bb)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3967-L3989 (chrome/m156)
fn battleOp148(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42216831), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4295b6bc), f32::from_bits(0xc257ea44)),
        (f32::from_bits(0x42a38b53), f32::from_bits(0xc1639572)),
    );
    path.line_to((f32::from_bits(0x426c7311), f32::from_bits(0xc12484b9)));
    path.cubic_to(
        (f32::from_bits(0x42587424), f32::from_bits(0xc21c154e)),
        (f32::from_bits(0x41e95c08), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb560056c), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a38b52), f32::from_bits(0xc1639578)));
    path.cubic_to(
        (f32::from_bits(0x42a4def8), f32::from_bits(0xc1269090)),
        (f32::from_bits(0x42a5a99a), f32::from_bits(0xc0d1c16f)),
        (f32::from_bits(0x42a5e9be), f32::from_bits(0xc02be63c)),
    );
    path.line_to((f32::from_bits(0x426fdfd2), f32::from_bits(0xbff8877d)));
    path.cubic_to(
        (f32::from_bits(0x426f8319), f32::from_bits(0xc097a16e)),
        (f32::from_bits(0x426e5e22), f32::from_bits(0xc0f0d105)),
        (f32::from_bits(0x426c7311), f32::from_bits(0xc12484ba)),
    );
    path.line_to((f32::from_bits(0x42a38b52), f32::from_bits(0xc1639578)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L3992-L4016 (chrome/m156)
fn battleOp149(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42216831), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4295b6bc), f32::from_bits(0xc257ea44)),
        (f32::from_bits(0x42a38b52), f32::from_bits(0xc1639578)),
    );
    path.line_to((f32::from_bits(0x426c7311), f32::from_bits(0xc12484ba)));
    path.cubic_to(
        (f32::from_bits(0x42587424), f32::from_bits(0xc21c154e)),
        (f32::from_bits(0x41e95c08), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a5e9be), f32::from_bits(0xc02be63f)));
    path.cubic_to(
        (f32::from_bits(0x42a7ff8e), f32::from_bits(0x41ec1faa)),
        (f32::from_bits(0x42849fff), f32::from_bits(0x426da4e1)),
        (f32::from_bits(0x4216595b), f32::from_bits(0x429400af)),
    );
    path.cubic_to(
        (f32::from_bits(0x410dcade), f32::from_bits(0x42b12eec)),
        (f32::from_bits(0xc1cdb135), f32::from_bits(0x42aa7b1c)),
        (f32::from_bits(0xc24c6646), f32::from_bits(0x4282cf52)),
    );
    path.line_to((f32::from_bits(0xc213c238), f32::from_bits(0x423d1f66)));
    path.cubic_to(
        (f32::from_bits(0xc194b176), f32::from_bits(0x42767a79)),
        (f32::from_bits(0x40cd0045), f32::from_bits(0x42801597)),
        (f32::from_bits(0x41d95f44), f32::from_bits(0x4255fad4)),
    );
    path.cubic_to(
        (f32::from_bits(0x423fbf3c), f32::from_bits(0x422bca7a)),
        (f32::from_bits(0x4272e39a), f32::from_bits(0x41aab11f)),
        (f32::from_bits(0x426fdfd3), f32::from_bits(0xbff88758)),
    );
    path.line_to((f32::from_bits(0x42a5e9be), f32::from_bits(0xc02be63f)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4019-L4041 (chrome/m156)
fn battleOp150(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x422dab0f), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x429efeec), f32::from_bits(0xc2462810)),
        (f32::from_bits(0x42a58789), f32::from_bits(0xc0c7d837)),
    );
    path.line_to((f32::from_bits(0x426f51d5), f32::from_bits(0xc0907750)));
    path.cubic_to(
        (f32::from_bits(0x4265df9a), f32::from_bits(0xc20f3ee4)),
        (f32::from_bits(0x41fb162c), f32::from_bits(0xc26ffffe)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a58789), f32::from_bits(0xc0c7d840)));
    path.cubic_to(
        (f32::from_bits(0x42a626ff), f32::from_bits(0xc0078454)),
        (f32::from_bits(0x42a62824), f32::from_bits(0x4001c6d5)),
        (f32::from_bits(0x42a58af5), f32::from_bits(0x40c4fc3c)),
    );
    path.line_to((f32::from_bits(0x426f56ca), f32::from_bits(0x408e6626)));
    path.cubic_to(
        (f32::from_bits(0x42703a0b), f32::from_bits(0x3fbba106)),
        (f32::from_bits(0x42703864), f32::from_bits(0xbfc3ed93)),
        (f32::from_bits(0x426f51d4), f32::from_bits(0xc090774f)),
    );
    path.line_to((f32::from_bits(0x42a58789), f32::from_bits(0xc0c7d840)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4044-L4072 (chrome/m156)
fn battleOp151(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea5), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x422dab0f), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x429efeec), f32::from_bits(0xc2462811)),
        (f32::from_bits(0x42a58789), f32::from_bits(0xc0c7d840)),
    );
    path.line_to((f32::from_bits(0x42a58789), f32::from_bits(0xc0c7d837)));
    path.cubic_to(
        (f32::from_bits(0x42a626ff), f32::from_bits(0xc0078448)),
        (f32::from_bits(0x42a62824), f32::from_bits(0x4001c6db)),
        (f32::from_bits(0x42a58af5), f32::from_bits(0x40c4fc3c)),
    );
    path.line_to((f32::from_bits(0x426f56ca), f32::from_bits(0x408e6626)));
    path.cubic_to(
        (f32::from_bits(0x42703a0b), f32::from_bits(0x3fbba106)),
        (f32::from_bits(0x42703864), f32::from_bits(0xbfc3ed93)),
        (f32::from_bits(0x426f51d4), f32::from_bits(0xc090774f)),
    );
    path.line_to((f32::from_bits(0x426f51d5), f32::from_bits(0xc0907750)));
    path.cubic_to(
        (f32::from_bits(0x4265df9a), f32::from_bits(0xc20f3ee4)),
        (f32::from_bits(0x41fb162c), f32::from_bits(0xc26ffffe)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a58af6), f32::from_bits(0x40c4fc3d)));
    path.cubic_to(
        (f32::from_bits(0x42a06986), f32::from_bits(0x422298c3)),
        (f32::from_bits(0x42621341), f32::from_bits(0x428bdf10)),
        (f32::from_bits(0x41ba9762), f32::from_bits(0x429f4f99)),
    );
    path.cubic_to(
        (f32::from_bits(0xc11def80), f32::from_bits(0x42b2c022)),
        (f32::from_bits(0xc236745f), f32::from_bits(0x429afb1c)),
        (f32::from_bits(0xc284c1e2), f32::from_bits(0x4247504a)),
    );
    path.line_to((f32::from_bits(0xc23ff038), f32::from_bits(0x42101509)));
    path.cubic_to(
        (f32::from_bits(0xc203e517), f32::from_bits(0x4260119e)),
        (f32::from_bits(0xc0e45731), f32::from_bits(0x428137a0)),
        (f32::from_bits(0x4186e2a5), f32::from_bits(0x42665443)),
    );
    path.cubic_to(
        (f32::from_bits(0x42236d8c), f32::from_bits(0x424a3945)),
        (f32::from_bits(0x4267ebda), f32::from_bits(0x41eb1462)),
        (f32::from_bits(0x426f56cb), f32::from_bits(0x408e661a)),
    );
    path.line_to((f32::from_bits(0x42a58af6), f32::from_bits(0x40c4fc3d)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4075-L4099 (chrome/m156)
fn battleOp152(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41b12ed4), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x422d822c), f32::from_bits(0xc2944bde)),
        (f32::from_bits(0x426bdb91), f32::from_bits(0xc269a7f3)),
    );
    path.cubic_to(
        (f32::from_bits(0x42951a7b), f32::from_bits(0xc22ab829)),
        (f32::from_bits(0x42a66879), f32::from_bits(0xc1aaf2b1)),
        (f32::from_bits(0x42a5fe21), f32::from_bits(0x3f4744a4)),
    );
    path.line_to((f32::from_bits(0x426ffd4c), f32::from_bits(0x3f100c99)));
    path.cubic_to(
        (f32::from_bits(0x4270970c), f32::from_bits(0xc177275d)),
        (f32::from_bits(0x4257923d), f32::from_bits(0xc1f6d2bd)),
        (f32::from_bits(0x422a7fe2), f32::from_bits(0xc228e872)),
    );
    path.cubic_to(
        (f32::from_bits(0x41fadb0b), f32::from_bits(0xc2566785)),
        (f32::from_bits(0x41801584), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb560056c), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a5fe22), f32::from_bits(0x3f4744a1)));
    path.cubic_to(
        (f32::from_bits(0x42a5e921), f32::from_bits(0x40a4df91)),
        (f32::from_bits(0x42a52322), f32::from_bits(0x411841f7)),
        (f32::from_bits(0x42a3adfe), f32::from_bits(0x415d43d0)),
    );
    path.line_to((f32::from_bits(0x426ca531), f32::from_bits(0x411ff355)));
    path.cubic_to(
        (f32::from_bits(0x426ec0ad), f32::from_bits(0x40dc21ae)),
        (f32::from_bits(0x426fdeef), f32::from_bits(0x406e5efe)),
        (f32::from_bits(0x426ffd4d), f32::from_bits(0x3f100c9b)),
    );
    path.line_to((f32::from_bits(0x42a5fe22), f32::from_bits(0x3f4744a1)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4102-L4128 (chrome/m156)
fn battleOp153(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41b12ed4), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x422d822c), f32::from_bits(0xc2944bde)),
        (f32::from_bits(0x426bdb91), f32::from_bits(0xc269a7f3)),
    );
    path.cubic_to(
        (f32::from_bits(0x42951a7b), f32::from_bits(0xc22ab829)),
        (f32::from_bits(0x42a66879), f32::from_bits(0xc1aaf2b1)),
        (f32::from_bits(0x42a5fe21), f32::from_bits(0x3f4744a0)),
    );
    path.line_to((f32::from_bits(0x426ffd4c), f32::from_bits(0x3f100c99)));
    path.cubic_to(
        (f32::from_bits(0x4270970c), f32::from_bits(0xc177275d)),
        (f32::from_bits(0x4257923d), f32::from_bits(0xc1f6d2bd)),
        (f32::from_bits(0x422a7fe2), f32::from_bits(0xc228e872)),
    );
    path.cubic_to(
        (f32::from_bits(0x41fadb0b), f32::from_bits(0xc2566785)),
        (f32::from_bits(0x41801584), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a3adfe), f32::from_bits(0x415d43d0)));
    path.cubic_to(
        (f32::from_bits(0x42977493), f32::from_bits(0x42480062)),
        (f32::from_bits(0x423a617c), f32::from_bits(0x429bbd03)),
        (f32::from_bits(0x4123044a), f32::from_bits(0x42a4be9a)),
    );
    path.cubic_to(
        (f32::from_bits(0xc1d1beaf), f32::from_bits(0x42adc030)),
        (f32::from_bits(0xc2750d30), f32::from_bits(0x4285e3a3)),
        (f32::from_bits(0xc2980208), f32::from_bits(0x42056911)),
    );
    path.line_to((f32::from_bits(0xc25bc541), f32::from_bits(0x41c0e1ed)));
    path.cubic_to(
        (f32::from_bits(0xc231254e), f32::from_bits(0x42419328)),
        (f32::from_bits(0xc1979f72), f32::from_bits(0x427b34be)),
        (f32::from_bits(0x40ebafde), f32::from_bits(0x426e2f5c)),
    );
    path.cubic_to(
        (f32::from_bits(0x4206bbb1), f32::from_bits(0x426129fa)),
        (f32::from_bits(0x425af8c2), f32::from_bits(0x42109457)),
        (f32::from_bits(0x426ca533), f32::from_bits(0x411ff35b)),
    );
    path.line_to((f32::from_bits(0x42a3adfe), f32::from_bits(0x415d43d0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4131-L4155 (chrome/m156)
fn battleOp154(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41bb5603), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4236fa4e), f32::from_bits(0xc2923760)),
        (f32::from_bits(0x4275e892), f32::from_bits(0xc25f0dc8)),
    );
    path.cubic_to(
        (f32::from_bits(0x429a6b6b), f32::from_bits(0xc219acd0)),
        (f32::from_bits(0x42a9c473), f32::from_bits(0xc173c3a6)),
        (f32::from_bits(0x42a5369d), f32::from_bits(0x410121d8)),
    );
    path.line_to((f32::from_bits(0x426edcd8), f32::from_bits(0x40bab276)));
    path.cubic_to(
        (f32::from_bits(0x42757264), f32::from_bits(0xc1303715)),
        (f32::from_bits(0x425f41dd), f32::from_bits(0xc1de2e4a)),
        (f32::from_bits(0x4231c3e2), f32::from_bits(0xc2213e66)),
    );
    path.cubic_to(
        (f32::from_bits(0x420445e8), f32::from_bits(0xc25365a8)),
        (f32::from_bits(0x41876c72), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb560056c), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a5369e), f32::from_bits(0x410121d6)));
    path.cubic_to(
        (f32::from_bits(0x42a450b5), f32::from_bits(0x414aab85)),
        (f32::from_bits(0x42a2a6cd), f32::from_bits(0x4189bd6e)),
        (f32::from_bits(0x42a03d57), f32::from_bits(0x41ad66e6)),
    );
    path.line_to((f32::from_bits(0x4267abf7), f32::from_bits(0x417ab39f)));
    path.cubic_to(
        (f32::from_bits(0x426b28ae), f32::from_bits(0x41472463)),
        (f32::from_bits(0x426d9071), f32::from_bits(0x41128229)),
        (f32::from_bits(0x426edcd8), f32::from_bits(0x40bab277)),
    );
    path.line_to((f32::from_bits(0x42a5369e), f32::from_bits(0x410121d6)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4158-L4187 (chrome/m156)
fn battleOp155(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41bb5603), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4236fa4e), f32::from_bits(0xc2923760)),
        (f32::from_bits(0x4275e892), f32::from_bits(0xc25f0dc8)),
    );
    path.cubic_to(
        (f32::from_bits(0x429a6b6b), f32::from_bits(0xc219acd0)),
        (f32::from_bits(0x42a9c473), f32::from_bits(0xc173c3a8)),
        (f32::from_bits(0x42a5369d), f32::from_bits(0x410121d5)),
    );
    path.line_to((f32::from_bits(0x42a5369e), f32::from_bits(0x410121d6)));
    path.cubic_to(
        (f32::from_bits(0x42a450b5), f32::from_bits(0x414aab85)),
        (f32::from_bits(0x42a2a6cd), f32::from_bits(0x4189bd6e)),
        (f32::from_bits(0x42a03d57), f32::from_bits(0x41ad66e6)),
    );
    path.line_to((f32::from_bits(0x4267abf7), f32::from_bits(0x417ab39f)));
    path.cubic_to(
        (f32::from_bits(0x426b28ae), f32::from_bits(0x41472463)),
        (f32::from_bits(0x426d9071), f32::from_bits(0x41128229)),
        (f32::from_bits(0x426edcd8), f32::from_bits(0x40bab276)),
    );
    path.cubic_to(
        (f32::from_bits(0x42757264), f32::from_bits(0xc1303715)),
        (f32::from_bits(0x425f41dd), f32::from_bits(0xc1de2e4a)),
        (f32::from_bits(0x4231c3e2), f32::from_bits(0xc2213e66)),
    );
    path.cubic_to(
        (f32::from_bits(0x420445e8), f32::from_bits(0xc25365a8)),
        (f32::from_bits(0x41876c72), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a03d58), f32::from_bits(0x41ad66e7)));
    path.cubic_to(
        (f32::from_bits(0x428bedd4), f32::from_bits(0x426cda0a)),
        (f32::from_bits(0x420c6f35), f32::from_bits(0x42a955c4)),
        (f32::from_bits(0xc06f4c79), f32::from_bits(0x42a5d4d6)),
    );
    path.cubic_to(
        (f32::from_bits(0xc22a58c2), f32::from_bits(0x42a253e8)),
        (f32::from_bits(0xc2960525), f32::from_bits(0x4252b394)),
        (f32::from_bits(0xc2a37db3), f32::from_bits(0x41660422)),
    );
    path.line_to((f32::from_bits(0xc26c5f63), f32::from_bits(0x412646cf)));
    path.cubic_to(
        (f32::from_bits(0xc258e58a), f32::from_bits(0x4218507a)),
        (f32::from_bits(0xc1f648da), f32::from_bits(0x426ab0dc)),
        (f32::from_bits(0xc02cfcc3), f32::from_bits(0x426fc1a0)),
    );
    path.cubic_to(
        (f32::from_bits(0x41cb09aa), f32::from_bits(0x4274d265)),
        (f32::from_bits(0x424a4e9e), f32::from_bits(0x422b37da)),
        (f32::from_bits(0x4267abf8), f32::from_bits(0x417ab398)),
    );
    path.line_to((f32::from_bits(0x42a03d58), f32::from_bits(0x41ad66e7)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4190-L4214 (chrome/m156)
fn battleOp156(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41c3ae1a), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x423eb2d3), f32::from_bits(0xc2906c00)),
        (f32::from_bits(0x427dc7c2), f32::from_bits(0xc2560e13)),
    );
    path.cubic_to(
        (f32::from_bits(0x429e6e58), f32::from_bits(0xc20b4426)),
        (f32::from_bits(0x42abdf2b), f32::from_bits(0xc121d7a7)),
        (f32::from_bits(0x42a39f93), f32::from_bits(0x415fea21)),
    );
    path.line_to((f32::from_bits(0x426c905a), f32::from_bits(0x4121ddae)));
    path.cubic_to(
        (f32::from_bits(0x42787d42), f32::from_bits(0xc0e9fd34)),
        (f32::from_bits(0x42650e94), f32::from_bits(0xc1c95949)),
        (f32::from_bits(0x423774a6), f32::from_bits(0xc21abd13)),
    );
    path.cubic_to(
        (f32::from_bits(0x4209dab9), f32::from_bits(0xc250cd81)),
        (f32::from_bits(0x418d749b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb560056c), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a39f93), f32::from_bits(0x415fea20)));
    path.cubic_to(
        (f32::from_bits(0x42a1ffad), f32::from_bits(0x4195f252)),
        (f32::from_bits(0x429f8ce1), f32::from_bits(0x41bb4c45)),
        (f32::from_bits(0x429c4e4c), f32::from_bits(0x41df969a)),
    );
    path.line_to((f32::from_bits(0x4261fbff), f32::from_bits(0x41a1a14e)));
    path.cubic_to(
        (f32::from_bits(0x4266acd9), f32::from_bits(0x41876566)),
        (f32::from_bits(0x426a370e), f32::from_bits(0x4158ca4c)),
        (f32::from_bits(0x426c905b), f32::from_bits(0x4121ddaf)),
    );
    path.line_to((f32::from_bits(0x42a39f93), f32::from_bits(0x415fea20)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4217-L4247 (chrome/m156)
fn battleOp157(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41c3ae1a), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x423eb2d3), f32::from_bits(0xc2906c00)),
        (f32::from_bits(0x427dc7c2), f32::from_bits(0xc2560e13)),
    );
    path.cubic_to(
        (f32::from_bits(0x429e6e58), f32::from_bits(0xc20b4426)),
        (f32::from_bits(0x42abdf2b), f32::from_bits(0xc121d7a8)),
        (f32::from_bits(0x42a39f93), f32::from_bits(0x415fea20)),
    );
    path.line_to((f32::from_bits(0x42a39f93), f32::from_bits(0x415fea21)));
    path.cubic_to(
        (f32::from_bits(0x42a1ffad), f32::from_bits(0x4195f252)),
        (f32::from_bits(0x429f8ce1), f32::from_bits(0x41bb4c45)),
        (f32::from_bits(0x429c4e4c), f32::from_bits(0x41df969a)),
    );
    path.line_to((f32::from_bits(0x4261fbff), f32::from_bits(0x41a1a14e)));
    path.cubic_to(
        (f32::from_bits(0x4266acd9), f32::from_bits(0x41876566)),
        (f32::from_bits(0x426a370e), f32::from_bits(0x4158ca4c)),
        (f32::from_bits(0x426c905b), f32::from_bits(0x4121ddaf)),
    );
    path.line_to((f32::from_bits(0x426c905a), f32::from_bits(0x4121ddae)));
    path.cubic_to(
        (f32::from_bits(0x42787d42), f32::from_bits(0xc0e9fd34)),
        (f32::from_bits(0x42650e94), f32::from_bits(0xc1c95949)),
        (f32::from_bits(0x423774a6), f32::from_bits(0xc21abd13)),
    );
    path.cubic_to(
        (f32::from_bits(0x4209dab9), f32::from_bits(0xc250cd81)),
        (f32::from_bits(0x418d749b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429c4e4c), f32::from_bits(0x41df969b)));
    path.cubic_to(
        (f32::from_bits(0x4280e391), f32::from_bits(0x4284903f)),
        (f32::from_bits(0x41c7a851), f32::from_bits(0x42b2072e)),
        (f32::from_bits(0xc1713833), f32::from_bits(0x42a33d14)),
    );
    path.cubic_to(
        (f32::from_bits(0xc25c7040), f32::from_bits(0x429472fb)),
        (f32::from_bits(0xc2a7bda2), f32::from_bits(0x421b8b2e)),
        (f32::from_bits(0xc2a5f5d6), f32::from_bits(0xbfe85110)),
    );
    path.line_to((f32::from_bits(0xc26ff14f), f32::from_bits(0xbfa7f00b)));
    path.cubic_to(
        (f32::from_bits(0xc272844c), f32::from_bits(0x41e0e1f3)),
        (f32::from_bits(0xc21f5a65), f32::from_bits(0x4256a019)),
        (f32::from_bits(0xc12e6015), f32::from_bits(0x426c01f9)),
    );
    path.cubic_to(
        (f32::from_bits(0x419054b7), f32::from_bits(0x4280b1ec)),
        (f32::from_bits(0x423a5877), f32::from_bits(0x423fa872)),
        (f32::from_bits(0x4261fc02), f32::from_bits(0x41a1a142)),
    );
    path.line_to((f32::from_bits(0x429c4e4c), f32::from_bits(0x41df969b)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4250-L4274 (chrome/m156)
fn battleOp158(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41cb677f), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4245cb36), f32::from_bits(0xc28eb15b)),
        (f32::from_bits(0x42825fc2), f32::from_bits(0xc24d8299)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a1d9e8), f32::from_bits(0xc1fb44f8)),
        (f32::from_bits(0x42ad4967), f32::from_bits(0xc0aa7cf8)),
        (f32::from_bits(0x42a1679f), f32::from_bits(0x419b26cf)),
    );
    path.line_to((f32::from_bits(0x42695b36), f32::from_bits(0x416050ca)));
    path.cubic_to(
        (f32::from_bits(0x427a88f8), f32::from_bits(0xc0767d2a)),
        (f32::from_bits(0x426a0074), f32::from_bits(0xc1b5a3f9)),
        (f32::from_bits(0x423c7e1d), f32::from_bits(0xc2148fc2)),
    );
    path.cubic_to(
        (f32::from_bits(0x420efbc6), f32::from_bits(0xc24e4d87)),
        (f32::from_bits(0x41930a0e), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb560056c), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a1679f), f32::from_bits(0x419b26d0)));
    path.cubic_to(
        (f32::from_bits(0x429f113c), f32::from_bits(0x41c20ede)),
        (f32::from_bits(0x429bdafe), f32::from_bits(0x41e80a2e)),
        (f32::from_bits(0x4297ceee), f32::from_bits(0x42065107)),
    );
    path.line_to((f32::from_bits(0x425b7b5f), f32::from_bits(0x41c2314a)));
    path.cubic_to(
        (f32::from_bits(0x4261554b), f32::from_bits(0x41a7bd56)),
        (f32::from_bits(0x4265fa14), f32::from_bits(0x418c4870)),
        (f32::from_bits(0x42695b37), f32::from_bits(0x416050cb)),
    );
    path.line_to((f32::from_bits(0x42a1679f), f32::from_bits(0x419b26d0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4277-L4305 (chrome/m156)
fn battleOp159(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41cb677f), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4245cb36), f32::from_bits(0xc28eb15b)),
        (f32::from_bits(0x42825fc2), f32::from_bits(0xc24d8299)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a1d9e8), f32::from_bits(0xc1fb44f8)),
        (f32::from_bits(0x42ad4967), f32::from_bits(0xc0aa7cf8)),
        (f32::from_bits(0x42a1679f), f32::from_bits(0x419b26d0)),
    );
    path.cubic_to(
        (f32::from_bits(0x429f113c), f32::from_bits(0x41c20ede)),
        (f32::from_bits(0x429bdafe), f32::from_bits(0x41e80a2e)),
        (f32::from_bits(0x4297ceee), f32::from_bits(0x42065107)),
    );
    path.line_to((f32::from_bits(0x425b7b5f), f32::from_bits(0x41c2314a)));
    path.cubic_to(
        (f32::from_bits(0x4261554b), f32::from_bits(0x41a7bd56)),
        (f32::from_bits(0x4265fa14), f32::from_bits(0x418c4870)),
        (f32::from_bits(0x42695b36), f32::from_bits(0x416050ca)),
    );
    path.cubic_to(
        (f32::from_bits(0x427a88f8), f32::from_bits(0xc0767d2a)),
        (f32::from_bits(0x426a0074), f32::from_bits(0xc1b5a3f9)),
        (f32::from_bits(0x423c7e1d), f32::from_bits(0xc2148fc2)),
    );
    path.cubic_to(
        (f32::from_bits(0x420efbc6), f32::from_bits(0xc24e4d87)),
        (f32::from_bits(0x41930a0e), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4297ceef), f32::from_bits(0x42065107)));
    path.cubic_to(
        (f32::from_bits(0x426afc81), f32::from_bits(0x4290b9e3)),
        (f32::from_bits(0x4171c53f), f32::from_bits(0x42b7f2c1)),
        (f32::from_bits(0xc1ca446b), f32::from_bits(0x429e1c54)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2835add), f32::from_bits(0x428445e8)),
        (f32::from_bits(0xc2b3ab9e), f32::from_bits(0x41c6c009)),
        (f32::from_bits(0xc2a29b10), f32::from_bits(0xc18596e4)),
    );
    path.line_to((f32::from_bits(0xc26b17b4), f32::from_bits(0xc141242b)));
    path.cubic_to(
        (f32::from_bits(0xc281e1de), f32::from_bits(0x418faccb)),
        (f32::from_bits(0xc23de932), f32::from_bits(0x423f3d09)),
        (f32::from_bits(0xc19237aa), f32::from_bits(0x42649810)),
    );
    path.cubic_to(
        (f32::from_bits(0x412ec628), f32::from_bits(0x4284f98c)),
        (f32::from_bits(0x4229deab), f32::from_bits(0x42513e23)),
        (f32::from_bits(0x425b7b62), f32::from_bits(0x41c23147)),
    );
    path.line_to((f32::from_bits(0x4297ceef), f32::from_bits(0x42065107)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4308-L4332 (chrome/m156)
fn battleOp160(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41d3ccce), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x424d7252), f32::from_bits(0xc28cbd55)),
        (f32::from_bits(0x4285fbcc), f32::from_bits(0xc244010c)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a53e6e), f32::from_bits(0xc1dd0edd)),
        (f32::from_bits(0x42ae3d82), f32::from_bits(0xbdb630d0)),
        (f32::from_bits(0x429e3366), f32::from_bits(0x41c92323)),
    );
    path.line_to((f32::from_bits(0x4264b95a), f32::from_bits(0x41916681)));
    path.cubic_to(
        (f32::from_bits(0x427be9e4), f32::from_bits(0xbd83b620)),
        (f32::from_bits(0x426ee823), f32::from_bits(0xc19fcd11)),
        (f32::from_bits(0x4241b610), f32::from_bits(0xc20db091)),
    );
    path.cubic_to(
        (f32::from_bits(0x421483fd), f32::from_bits(0xc24b7a9a)),
        (f32::from_bits(0x41991bc1), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429e3367), f32::from_bits(0x41c92322)));
    path.cubic_to(
        (f32::from_bits(0x429b0cbc), f32::from_bits(0x41f0ca9b)),
        (f32::from_bits(0x4296f94f), f32::from_bits(0x420b9629)),
        (f32::from_bits(0x429206e2), f32::from_bits(0x421de34f)),
    );
    path.line_to((f32::from_bits(0x42531f8a), f32::from_bits(0x41e4458f)));
    path.cubic_to(
        (f32::from_bits(0x425a4685), f32::from_bits(0x41c9cfd9)),
        (f32::from_bits(0x42602b18), f32::from_bits(0x41ae10ed)),
        (f32::from_bits(0x4264b95a), f32::from_bits(0x41916682)),
    );
    path.line_to((f32::from_bits(0x429e3367), f32::from_bits(0x41c92322)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4335-L4363 (chrome/m156)
fn battleOp161(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb630015b), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41d3ccce), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x424d7252), f32::from_bits(0xc28cbd55)),
        (f32::from_bits(0x4285fbcc), f32::from_bits(0xc244010c)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a53e6e), f32::from_bits(0xc1dd0edd)),
        (f32::from_bits(0x42ae3d82), f32::from_bits(0xbdb630d0)),
        (f32::from_bits(0x429e3367), f32::from_bits(0x41c92322)),
    );
    path.cubic_to(
        (f32::from_bits(0x429b0cbc), f32::from_bits(0x41f0ca9b)),
        (f32::from_bits(0x4296f94f), f32::from_bits(0x420b9629)),
        (f32::from_bits(0x429206e2), f32::from_bits(0x421de34f)),
    );
    path.line_to((f32::from_bits(0x42531f8a), f32::from_bits(0x41e4458f)));
    path.cubic_to(
        (f32::from_bits(0x425a4685), f32::from_bits(0x41c9cfd9)),
        (f32::from_bits(0x42602b18), f32::from_bits(0x41ae10ed)),
        (f32::from_bits(0x4264b95a), f32::from_bits(0x41916681)),
    );
    path.cubic_to(
        (f32::from_bits(0x427be9e4), f32::from_bits(0xbd83b620)),
        (f32::from_bits(0x426ee823), f32::from_bits(0xc19fcd11)),
        (f32::from_bits(0x4241b610), f32::from_bits(0xc20db091)),
    );
    path.cubic_to(
        (f32::from_bits(0x421483fd), f32::from_bits(0xc24b7a9a)),
        (f32::from_bits(0x41991bc1), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429206e2), f32::from_bits(0x421de34f)));
    path.cubic_to(
        (f32::from_bits(0x424fd7be), f32::from_bits(0x429cd433)),
        (f32::from_bits(0x40819da9), f32::from_bits(0x42bbf605)),
        (f32::from_bits(0xc20f7b98), f32::from_bits(0x4295b271)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2979573), f32::from_bits(0x425eddba)),
        (f32::from_bits(0xc2bb57fe), f32::from_bits(0x4109ef62)),
        (f32::from_bits(0xc2990315), f32::from_bits(0xc200bcbb)),
    );
    path.line_to((f32::from_bits(0xc25d38e3), f32::from_bits(0xc1ba2048)));
    path.cubic_to(
        (f32::from_bits(0xc2876de1), f32::from_bits(0x40c76c9c)),
        (f32::from_bits(0xc25b2842), f32::from_bits(0x42211baa)),
        (f32::from_bits(0xc1cf71e5), f32::from_bits(0x42586df1)),
    );
    path.cubic_to(
        (f32::from_bits(0x403b65b7), f32::from_bits(0x4287e01c)),
        (f32::from_bits(0x42163f6f), f32::from_bits(0x4262bd95)),
        (f32::from_bits(0x42531f8c), f32::from_bits(0x41e4458b)),
    );
    path.line_to((f32::from_bits(0x429206e2), f32::from_bits(0x421de34f)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4366-L4390 (chrome/m156)
fn battleOp162(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41da3d7f), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x425345ee), f32::from_bits(0xc28b3082)),
        (f32::from_bits(0x4288a01b), f32::from_bits(0xc23c9177)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a79d3f), f32::from_bits(0xc1c583d9)),
        (f32::from_bits(0x42ae8eeb), f32::from_bits(0x407c6461)),
        (f32::from_bits(0x429b333a), f32::from_bits(0x41eb9731)),
    );
    path.line_to((f32::from_bits(0x426062bb), f32::from_bits(0x41aa4e75)));
    path.cubic_to(
        (f32::from_bits(0x427c5f9a), f32::from_bits(0x403673d5)),
        (f32::from_bits(0x4272557b), f32::from_bits(0xc18ec82c)),
        (f32::from_bits(0x424587e0), f32::from_bits(0xc208507b)),
    );
    path.cubic_to(
        (f32::from_bits(0x4218ba46), f32::from_bits(0xc2493ce1)),
        (f32::from_bits(0x419dc399), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429b3339), f32::from_bits(0x41eb9733)));
    path.cubic_to(
        (f32::from_bits(0x429766b3), f32::from_bits(0x4209d0f3)),
        (f32::from_bits(0x4292a485), f32::from_bits(0x421d0e17)),
        (f32::from_bits(0x428cfdb5), f32::from_bits(0x422f3e33)),
    );
    path.line_to((f32::from_bits(0x424bd7ac), f32::from_bits(0x41fd5d06)));
    path.cubic_to(
        (f32::from_bits(0x42540374), f32::from_bits(0x41e3114e)),
        (f32::from_bits(0x425ae4ae), f32::from_bits(0x41c7409b)),
        (f32::from_bits(0x426062bc), f32::from_bits(0x41aa4e76)),
    );
    path.line_to((f32::from_bits(0x429b3339), f32::from_bits(0x41eb9733)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4393-L4421 (chrome/m156)
fn battleOp163(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41da3d7f), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x425345ee), f32::from_bits(0xc28b3082)),
        (f32::from_bits(0x4288a01b), f32::from_bits(0xc23c9177)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a79d3f), f32::from_bits(0xc1c583d9)),
        (f32::from_bits(0x42ae8eeb), f32::from_bits(0x407c6461)),
        (f32::from_bits(0x429b3339), f32::from_bits(0x41eb9733)),
    );
    path.cubic_to(
        (f32::from_bits(0x429766b3), f32::from_bits(0x4209d0f3)),
        (f32::from_bits(0x4292a485), f32::from_bits(0x421d0e17)),
        (f32::from_bits(0x428cfdb5), f32::from_bits(0x422f3e33)),
    );
    path.line_to((f32::from_bits(0x424bd7ac), f32::from_bits(0x41fd5d06)));
    path.cubic_to(
        (f32::from_bits(0x42540374), f32::from_bits(0x41e3114e)),
        (f32::from_bits(0x425ae4ae), f32::from_bits(0x41c7409b)),
        (f32::from_bits(0x426062bb), f32::from_bits(0x41aa4e75)),
    );
    path.cubic_to(
        (f32::from_bits(0x427c5f9a), f32::from_bits(0x403673d5)),
        (f32::from_bits(0x4272557b), f32::from_bits(0xc18ec82c)),
        (f32::from_bits(0x424587e0), f32::from_bits(0xc208507b)),
    );
    path.cubic_to(
        (f32::from_bits(0x4218ba46), f32::from_bits(0xc2493ce1)),
        (f32::from_bits(0x419dc399), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x428cfdb5), f32::from_bits(0x422f3e36)));
    path.cubic_to(
        (f32::from_bits(0x42397b9c), f32::from_bits(0x42a54202)),
        (f32::from_bits(0xc0931849), f32::from_bits(0x42bd474f)),
        (f32::from_bits(0xc22e0fe8), f32::from_bits(0x428d5ab7)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2a4de63), f32::from_bits(0x423adc3f)),
        (f32::from_bits(0xc2bd50df), f32::from_bits(0xc08673c0)),
        (f32::from_bits(0xc28db7cd), f32::from_bits(0xc22ce1b4)),
    );
    path.line_to((f32::from_bits(0xc24ce4bb), f32::from_bits(0xc1f9f306)));
    path.cubic_to(
        (f32::from_bits(0xc288db72), f32::from_bits(0xc0426216)),
        (f32::from_bits(0xc26e5ec8), f32::from_bits(0x42071590)),
        (f32::from_bits(0xc1fba9c9), f32::from_bits(0x424c5fa5)),
    );
    path.cubic_to(
        (f32::from_bits(0xc054b001), f32::from_bits(0x4288d4dc)),
        (f32::from_bits(0x420615fc), f32::from_bits(0x426eee67)),
        (f32::from_bits(0x424bd7af), f32::from_bits(0x41fd5d01)),
    );
    path.line_to((f32::from_bits(0x428cfdb5), f32::from_bits(0x422f3e36)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4424-L4448 (chrome/m156)
fn battleOp164(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41e183ec), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4259cec4), f32::from_bits(0xc2896274)),
        (f32::from_bits(0x428b79bc), f32::from_bits(0xc2340753)),
    );
    path.cubic_to(
        (f32::from_bits(0x42aa0c16), f32::from_bits(0xc1aa937d)),
        (f32::from_bits(0x42ae7c71), f32::from_bits(0x41080a55)),
        (f32::from_bits(0x42974339), f32::from_bits(0x4208c1d5)),
    );
    path.line_to((f32::from_bits(0x425ab161), f32::from_bits(0x41c5b8a2)));
    path.cubic_to(
        (f32::from_bits(0x427c44e4), f32::from_bits(0x40c4af5a)),
        (f32::from_bits(0x4275d9f7), f32::from_bits(0xc1769dba)),
        (f32::from_bits(0x4249a6c2), f32::from_bits(0xc2022424)),
    );
    path.cubic_to(
        (f32::from_bits(0x421d738b), f32::from_bits(0xc246a0db)),
        (f32::from_bits(0x41a305f1), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3725ffa9), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42974339), f32::from_bits(0x4208c1d6)));
    path.cubic_to(
        (f32::from_bits(0x4292b5f8), f32::from_bits(0x421ce537)),
        (f32::from_bits(0x428d2a3f), f32::from_bits(0x42301305)),
        (f32::from_bits(0x4286b52e), f32::from_bits(0x4242022c)),
    );
    path.line_to((f32::from_bits(0x4242c218), f32::from_bits(0x420c3f43)));
    path.cubic_to(
        (f32::from_bits(0x424c1813), f32::from_bits(0x41fe90b7)),
        (f32::from_bits(0x42541cae), f32::from_bits(0x41e2d634)),
        (f32::from_bits(0x425ab162), f32::from_bits(0x41c5b8a3)),
    );
    path.line_to((f32::from_bits(0x42974339), f32::from_bits(0x4208c1d6)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4451-L4481 (chrome/m156)
fn battleOp165(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3725ffa9), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41e183ec), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4259cec4), f32::from_bits(0xc2896274)),
        (f32::from_bits(0x428b79bc), f32::from_bits(0xc2340753)),
    );
    path.cubic_to(
        (f32::from_bits(0x42aa0c16), f32::from_bits(0xc1aa937d)),
        (f32::from_bits(0x42ae7c71), f32::from_bits(0x41080a55)),
        (f32::from_bits(0x42974339), f32::from_bits(0x4208c1d6)),
    );
    path.cubic_to(
        (f32::from_bits(0x4292b5f8), f32::from_bits(0x421ce537)),
        (f32::from_bits(0x428d2a3f), f32::from_bits(0x42301305)),
        (f32::from_bits(0x4286b52e), f32::from_bits(0x4242022c)),
    );
    path.line_to((f32::from_bits(0x4242c218), f32::from_bits(0x420c3f43)));
    path.cubic_to(
        (f32::from_bits(0x424c1813), f32::from_bits(0x41fe90b7)),
        (f32::from_bits(0x42541cae), f32::from_bits(0x41e2d634)),
        (f32::from_bits(0x425ab161), f32::from_bits(0x41c5b8a2)),
    );
    path.cubic_to(
        (f32::from_bits(0x427c44e4), f32::from_bits(0x40c4af5a)),
        (f32::from_bits(0x4275d9f7), f32::from_bits(0xc1769dba)),
        (f32::from_bits(0x4249a6c2), f32::from_bits(0xc2022424)),
    );
    path.cubic_to(
        (f32::from_bits(0x421d738b), f32::from_bits(0xc246a0db)),
        (f32::from_bits(0x41a305f1), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3725ffa9), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4286b52e), f32::from_bits(0x4242022d)));
    path.cubic_to(
        (f32::from_bits(0x4245f9c6), f32::from_bits(0x42929b97)),
        (f32::from_bits(0x419b96e9), f32::from_bits(0x42ac9135)),
        (f32::from_bits(0xc12da222), f32::from_bits(0x42a4933a)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2249c85), f32::from_bits(0x429c9540)),
        (f32::from_bits(0xc2859c99), f32::from_bits(0x4267dd85)),
        (f32::from_bits(0xc29b4028), f32::from_bits(0x41eb0f05)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2b0e3b8), f32::from_bits(0x3f4c608a)),
        (f32::from_bits(0xc2a55c16), f32::from_bits(0xc1fb5a07)),
        (f32::from_bits(0xc27a7a78), f32::from_bits(0xc259e8d8)),
    );
    path.line_to((f32::from_bits(0xc2351199), f32::from_bits(0xc21d8664)));
    path.cubic_to(
        (f32::from_bits(0xc26f12eb), f32::from_bits(0xc1b5b32d)),
        (f32::from_bits(0xc27fbe43), f32::from_bits(0x3f13bb74)),
        (f32::from_bits(0xc2607541), f32::from_bits(0x41a9ebcd)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2412c3e), f32::from_bits(0x42279ce1)),
        (f32::from_bits(0xc1edfdc7), f32::from_bits(0x4262625e)),
        (f32::from_bits(0xc0fb089d), f32::from_bits(0x426df06d)),
    );
    path.cubic_to(
        (f32::from_bits(0x4160f2f1), f32::from_bits(0x42797e7c)),
        (f32::from_bits(0x420f1d6a), f32::from_bits(0x4253f671)),
        (f32::from_bits(0x4242c21c), f32::from_bits(0x420c3f41)),
    );
    path.line_to((f32::from_bits(0x4286b52e), f32::from_bits(0x4242022d)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4484-L4508 (chrome/m156)
fn battleOp166(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41e5cd16), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x425da203), f32::from_bits(0xc2884b73)),
        (f32::from_bits(0x428d165b), f32::from_bits(0xc22eeec9)),
    );
    path.cubic_to(
        (f32::from_bits(0x42ab5bb4), f32::from_bits(0xc19a8d5b)),
        (f32::from_bits(0x42ae3add), f32::from_bits(0x4132f7c2)),
        (f32::from_bits(0x4294adf4), f32::from_bits(0x4213a75b)),
    );
    path.line_to((f32::from_bits(0x4256f554), f32::from_bits(0x41d579ab)));
    path.cubic_to(
        (f32::from_bits(0x427be612), f32::from_bits(0x41015fcf)),
        (f32::from_bits(0x4277bf2e), f32::from_bits(0xc15f72f6)),
        (f32::from_bits(0x424bfb4d), f32::from_bits(0xc1fcea38)),
    );
    path.cubic_to(
        (f32::from_bits(0x4220376c), f32::from_bits(0xc2450d7a)),
        (f32::from_bits(0x41a61f08), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb7060057), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4294adf4), f32::from_bits(0x4213a75b)));
    path.cubic_to(
        (f32::from_bits(0x428facea), f32::from_bits(0x4227cf1b)),
        (f32::from_bits(0x4289a8e5), f32::from_bits(0x423ae500)),
        (f32::from_bits(0x4282b9a7), f32::from_bits(0x424c9dab)),
    );
    path.line_to((f32::from_bits(0x423d0015), f32::from_bits(0x4213ea45)));
    path.cubic_to(
        (f32::from_bits(0x424706b3), f32::from_bits(0x42071ac0)),
        (f32::from_bits(0x424fb93a), f32::from_bits(0x41f29d8f)),
        (f32::from_bits(0x4256f555), f32::from_bits(0x41d579ac)),
    );
    path.line_to((f32::from_bits(0x4294adf4), f32::from_bits(0x4213a75b)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4511-L4541 (chrome/m156)
fn battleOp167(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb7060057), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41e5cd16), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x425da203), f32::from_bits(0xc2884b73)),
        (f32::from_bits(0x428d165b), f32::from_bits(0xc22eeec9)),
    );
    path.cubic_to(
        (f32::from_bits(0x42ab5bb4), f32::from_bits(0xc19a8d5b)),
        (f32::from_bits(0x42ae3add), f32::from_bits(0x4132f7c2)),
        (f32::from_bits(0x4294adf4), f32::from_bits(0x4213a75b)),
    );
    path.cubic_to(
        (f32::from_bits(0x428facea), f32::from_bits(0x4227cf1b)),
        (f32::from_bits(0x4289a8e5), f32::from_bits(0x423ae500)),
        (f32::from_bits(0x4282b9a7), f32::from_bits(0x424c9dab)),
    );
    path.line_to((f32::from_bits(0x423d0015), f32::from_bits(0x4213ea45)));
    path.cubic_to(
        (f32::from_bits(0x424706b3), f32::from_bits(0x42071ac0)),
        (f32::from_bits(0x424fb93a), f32::from_bits(0x41f29d8f)),
        (f32::from_bits(0x4256f554), f32::from_bits(0x41d579ab)),
    );
    path.cubic_to(
        (f32::from_bits(0x427be612), f32::from_bits(0x41015fcf)),
        (f32::from_bits(0x4277bf2e), f32::from_bits(0xc15f72f6)),
        (f32::from_bits(0x424bfb4d), f32::from_bits(0xc1fcea38)),
    );
    path.cubic_to(
        (f32::from_bits(0x4220376c), f32::from_bits(0xc2450d7a)),
        (f32::from_bits(0x41a61f08), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb7060057), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4282b9a8), f32::from_bits(0x424c9dac)));
    path.cubic_to(
        (f32::from_bits(0x4238a98e), f32::from_bits(0x42975dcd)),
        (f32::from_bits(0x416d9db4), f32::from_bits(0x42aecc7f)),
        (f32::from_bits(0xc17bb856), f32::from_bits(0x42a2fd9a)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2394396), f32::from_bits(0x42972eb6)),
        (f32::from_bits(0xc28e09e8), f32::from_bits(0x42543e5a)),
        (f32::from_bits(0xc29f69c3), f32::from_bits(0x41b9307a)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2b0c99f), f32::from_bits(0xc0d86efe)),
        (f32::from_bits(0xc29f345f), f32::from_bits(0xc21c161b)),
        (f32::from_bits(0xc263c1d4), f32::from_bits(0xc2718f13)),
    );
    path.line_to((f32::from_bits(0xc224a4cd), f32::from_bits(0xc22e9eef)));
    path.cubic_to(
        (f32::from_bits(0xc2662cd7), f32::from_bits(0xc1e1aab7)),
        (f32::from_bits(0xc27f98a3), f32::from_bits(0xc09c754c)),
        (f32::from_bits(0xc26679fe), f32::from_bits(0x4185df20)),
    );
    path.cubic_to(
        (f32::from_bits(0xc24d5b58), f32::from_bits(0x42196dcb)),
        (f32::from_bits(0xc205ecef), f32::from_bits(0x425a93a6)),
        (f32::from_bits(0xc135f72f), f32::from_bits(0x426ba619)),
    );
    path.cubic_to(
        (f32::from_bits(0x412bc560), f32::from_bits(0x427cb88a)),
        (f32::from_bits(0x42057da8), f32::from_bits(0x425ad7c5)),
        (f32::from_bits(0x423d0018), f32::from_bits(0x4213ea45)),
    );
    path.line_to((f32::from_bits(0x4282b9a8), f32::from_bits(0x424c9dac)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4544-L4568 (chrome/m156)
fn battleOp168(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41ea54b9), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4261a7de), f32::from_bits(0xc2871f16)),
        (f32::from_bits(0x428ebc81), f32::from_bits(0xc2297f4d)),
    );
    path.cubic_to(
        (f32::from_bits(0x42aca513), f32::from_bits(0xc18980da)),
        (f32::from_bits(0x42adc9a4), f32::from_bits(0x41604127)),
        (f32::from_bits(0x4291be57), f32::from_bits(0x421eee87)),
    );
    path.line_to((f32::from_bits(0x4252b6a9), f32::from_bits(0x41e5c7e9)));
    path.cubic_to(
        (f32::from_bits(0x427b4260), f32::from_bits(0x41221c9f)),
        (f32::from_bits(0x42799b62), f32::from_bits(0xc146ccc2)),
        (f32::from_bits(0x424e5da6), f32::from_bits(0xc1f50e65)),
    );
    path.cubic_to(
        (f32::from_bits(0x42231fea), f32::from_bits(0xc2435b34)),
        (f32::from_bits(0x41a9655c), f32::from_bits(0xc26ffffe)),
        (f32::from_bits(0x3725ffa9), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4291be57), f32::from_bits(0x421eee8a)));
    path.cubic_to(
        (f32::from_bits(0x428c4169), f32::from_bits(0x42330feb)),
        (f32::from_bits(0x4285bd57), f32::from_bits(0x4246005c)),
        (f32::from_bits(0x427c99ac), f32::from_bits(0x4257723d)),
    );
    path.line_to((f32::from_bits(0x42369a46), f32::from_bits(0x421bbe89)));
    path.cubic_to(
        (f32::from_bits(0x42415bc7), f32::from_bits(0x420f2230)),
        (f32::from_bits(0x424ac771), f32::from_bits(0x4201714b)),
        (f32::from_bits(0x4252b6a9), f32::from_bits(0x41e5c7e9)),
    );
    path.line_to((f32::from_bits(0x4291be57), f32::from_bits(0x421eee8a)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4571-L4601 (chrome/m156)
fn battleOp169(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3725ffa9), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41ea54b9), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4261a7de), f32::from_bits(0xc2871f16)),
        (f32::from_bits(0x428ebc81), f32::from_bits(0xc2297f4d)),
    );
    path.cubic_to(
        (f32::from_bits(0x42aca513), f32::from_bits(0xc18980da)),
        (f32::from_bits(0x42adc9a4), f32::from_bits(0x41604127)),
        (f32::from_bits(0x4291be57), f32::from_bits(0x421eee8a)),
    );
    path.cubic_to(
        (f32::from_bits(0x428c4169), f32::from_bits(0x42330feb)),
        (f32::from_bits(0x4285bd57), f32::from_bits(0x4246005c)),
        (f32::from_bits(0x427c99ac), f32::from_bits(0x4257723d)),
    );
    path.line_to((f32::from_bits(0x42369a46), f32::from_bits(0x421bbe89)));
    path.cubic_to(
        (f32::from_bits(0x42415bc7), f32::from_bits(0x420f2230)),
        (f32::from_bits(0x424ac771), f32::from_bits(0x4201714b)),
        (f32::from_bits(0x4252b6a9), f32::from_bits(0x41e5c7e9)),
    );
    path.cubic_to(
        (f32::from_bits(0x427b4260), f32::from_bits(0x41221c9f)),
        (f32::from_bits(0x42799b62), f32::from_bits(0xc146ccc2)),
        (f32::from_bits(0x424e5da6), f32::from_bits(0xc1f50e65)),
    );
    path.cubic_to(
        (f32::from_bits(0x42231fea), f32::from_bits(0xc2435b34)),
        (f32::from_bits(0x41a9655c), f32::from_bits(0xc26ffffe)),
        (f32::from_bits(0x3725ffa9), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x427c99ad), f32::from_bits(0x4257723e)));
    path.cubic_to(
        (f32::from_bits(0x422a2459), f32::from_bits(0x429c0ff6)),
        (f32::from_bits(0x411ef0c1), f32::from_bits(0x42b0a109)),
        (f32::from_bits(0xc1a68a7f), f32::from_bits(0x42a0b1a2)),
    );
    path.cubic_to(
        (f32::from_bits(0xc24e46af), f32::from_bits(0x4290c23b)),
        (f32::from_bits(0xc296269a), f32::from_bits(0x423e3c04)),
        (f32::from_bits(0xc2a2b82b), f32::from_bits(0x41835b51)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2af49bc), f32::from_bits(0xc16b82d9)),
        (f32::from_bits(0xc2973524), f32::from_bits(0xc23adb29)),
        (f32::from_bits(0xc24965c6), f32::from_bits(0xc283f801)),
    );
    path.line_to((f32::from_bits(0xc21196ae), f32::from_bits(0xc23ecc58)));
    path.cubic_to(
        (f32::from_bits(0xc25a9cfe), f32::from_bits(0xc20713a1)),
        (f32::from_bits(0xc27d6da1), f32::from_bits(0xc12a3fcc)),
        (f32::from_bits(0xc26b41bb), f32::from_bits(0x413de9a9)),
    );
    path.cubic_to(
        (f32::from_bits(0xc25915d3), f32::from_bits(0x420984c8)),
        (f32::from_bits(0xc2151d75), f32::from_bits(0x42514a1b)),
        (f32::from_bits(0xc170c819), f32::from_bits(0x4268540a)),
    );
    path.cubic_to(
        (f32::from_bits(0x40e5cb46), f32::from_bits(0x427f5dfa)),
        (f32::from_bits(0x41f5fd0c), f32::from_bits(0x4261a1d8)),
        (f32::from_bits(0x42369a4a), f32::from_bits(0x421bbe87)),
    );
    path.line_to((f32::from_bits(0x427c99ad), f32::from_bits(0x4257723e)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4604-L4628 (chrome/m156)
fn battleOp170(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41ef3488), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4265f5fc), f32::from_bits(0xc285d5a4)),
        (f32::from_bits(0x429072a6), f32::from_bits(0xc2239841)),
    );
    path.cubic_to(
        (f32::from_bits(0x42adea4e), f32::from_bits(0xc16e14e5)),
        (f32::from_bits(0x42ad1da2), f32::from_bits(0x41886b20)),
        (f32::from_bits(0x428e5adb), f32::from_bits(0x422ac68e)),
    );
    path.line_to((f32::from_bits(0x424dd078), f32::from_bits(0x41f6e790)));
    path.cubic_to(
        (f32::from_bits(0x427a49b4), f32::from_bits(0x41453b4b)),
        (f32::from_bits(0x427b719d), f32::from_bits(0xc12c1b6e)),
        (f32::from_bits(0x4250d71f), f32::from_bits(0xc1ec85c5)),
    );
    path.cubic_to(
        (f32::from_bits(0x42263ca0), f32::from_bits(0xc2417eea)),
        (f32::from_bits(0x41aceb63), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x428e5adb), f32::from_bits(0x422ac690)));
    path.cubic_to(
        (f32::from_bits(0x42885732), f32::from_bits(0x423ed443)),
        (f32::from_bits(0x428148a8), f32::from_bits(0x42518e43)),
        (f32::from_bits(0x42729aa0), f32::from_bits(0x4262a4bd)),
    );
    path.line_to((f32::from_bits(0x422f605c), f32::from_bits(0x4223d6b5)));
    path.cubic_to(
        (f32::from_bits(0x423aea98), f32::from_bits(0x42177c70)),
        (f32::from_bits(0x42451e76), f32::from_bits(0x4209f2e4)),
        (f32::from_bits(0x424dd078), f32::from_bits(0x41f6e792)),
    );
    path.line_to((f32::from_bits(0x428e5adb), f32::from_bits(0x422ac690)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4631-L4661 (chrome/m156)
fn battleOp171(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41ef3488), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4265f5fc), f32::from_bits(0xc285d5a4)),
        (f32::from_bits(0x429072a6), f32::from_bits(0xc2239841)),
    );
    path.cubic_to(
        (f32::from_bits(0x42adea4e), f32::from_bits(0xc16e14e5)),
        (f32::from_bits(0x42ad1da2), f32::from_bits(0x41886b20)),
        (f32::from_bits(0x428e5adb), f32::from_bits(0x422ac690)),
    );
    path.cubic_to(
        (f32::from_bits(0x42885732), f32::from_bits(0x423ed443)),
        (f32::from_bits(0x428148a8), f32::from_bits(0x42518e43)),
        (f32::from_bits(0x42729aa0), f32::from_bits(0x4262a4bd)),
    );
    path.line_to((f32::from_bits(0x422f605c), f32::from_bits(0x4223d6b5)));
    path.cubic_to(
        (f32::from_bits(0x423aea98), f32::from_bits(0x42177c70)),
        (f32::from_bits(0x42451e76), f32::from_bits(0x4209f2e4)),
        (f32::from_bits(0x424dd078), f32::from_bits(0x41f6e790)),
    );
    path.cubic_to(
        (f32::from_bits(0x427a49b4), f32::from_bits(0x41453b4b)),
        (f32::from_bits(0x427b719d), f32::from_bits(0xc12c1b6e)),
        (f32::from_bits(0x4250d71f), f32::from_bits(0xc1ec85c5)),
    );
    path.cubic_to(
        (f32::from_bits(0x42263ca0), f32::from_bits(0xc2417eea)),
        (f32::from_bits(0x41aceb63), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42729aa1), f32::from_bits(0x4262a4be)));
    path.cubic_to(
        (f32::from_bits(0x421a0aa1), f32::from_bits(0x42a0b8ab)),
        (f32::from_bits(0x4092ff14), f32::from_bits(0x42b1fc82)),
        (f32::from_bits(0xc1d17709), f32::from_bits(0x429d861f)),
    );
    path.cubic_to(
        (f32::from_bits(0xc263d6eb), f32::from_bits(0x42890fbc)),
        (f32::from_bits(0xc29dea71), f32::from_bits(0x42253dbf)),
        (f32::from_bits(0xc2a5016a), f32::from_bits(0x4111261a)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2ac1862), f32::from_bits(0xc1b95567)),
        (f32::from_bits(0xc28cface), f32::from_bits(0xc25a1117)),
        (f32::from_bits(0xc22aafa6), f32::from_bits(0xc28e61ba)),
    );
    path.line_to((f32::from_bits(0xc1f6c679), f32::from_bits(0xc24dda63)));
    path.cubic_to(
        (f32::from_bits(0xc24bd376), f32::from_bits(0xc21da377)),
        (f32::from_bits(0xc278cff1), f32::from_bits(0xc185f9db)),
        (f32::from_bits(0xc26e8fe1), f32::from_bits(0x40d1da84)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2644fd1), f32::from_bits(0x41eee71d)),
        (f32::from_bits(0xc224b3fc), f32::from_bits(0x4246293b)),
        (f32::from_bits(0xc1976b90), f32::from_bits(0x4263becd)),
    );
    path.cubic_to(
        (f32::from_bits(0x405486c0), f32::from_bits(0x4280aa2f)),
        (f32::from_bits(0x41deb5f2), f32::from_bits(0x42685e3e)),
        (f32::from_bits(0x422f605e), f32::from_bits(0x4223d6b6)),
    );
    path.line_to((f32::from_bits(0x42729aa1), f32::from_bits(0x4262a4be)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4664-L4688 (chrome/m156)
fn battleOp172(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f30c96), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x426956a5), f32::from_bits(0xc284cd4a)),
        (f32::from_bits(0x4291c05e), f32::from_bits(0xc21ee718)),
    );
    path.cubic_to(
        (f32::from_bits(0x42aed56a), f32::from_bits(0xc150ce71)),
        (f32::from_bits(0x42ac7181), f32::from_bits(0x419b8107)),
        (f32::from_bits(0x428b8516), f32::from_bits(0x4233e422)),
    );
    path.line_to((f32::from_bits(0x4249b729), f32::from_bits(0x42020ab3)));
    path.cubic_to(
        (f32::from_bits(0x427950d3), f32::from_bits(0x4160d339)),
        (f32::from_bits(0x427cc584), f32::from_bits(0xc116f1c4)),
        (f32::from_bits(0x4252b998), f32::from_bits(0xc1e5bd26)),
    );
    path.cubic_to(
        (f32::from_bits(0x4228adad), f32::from_bits(0xc24000b5)),
        (f32::from_bits(0x41afb2be), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb560056c), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x428b8516), f32::from_bits(0x4233e422)));
    path.cubic_to(
        (f32::from_bits(0x4285165c), f32::from_bits(0x4247d8d0)),
        (f32::from_bits(0x427b34bd), f32::from_bits(0x425a5d74)),
        (f32::from_bits(0x426a6401), f32::from_bits(0x426b20b1)),
    );
    path.line_to((f32::from_bits(0x42297063), f32::from_bits(0x4229f8c9)));
    path.cubic_to(
        (f32::from_bits(0x42359840), f32::from_bits(0x421ddab1)),
        (f32::from_bits(0x42406a5a), f32::from_bits(0x421077b9)),
        (f32::from_bits(0x4249b72b), f32::from_bits(0x42020ab4)),
    );
    path.line_to((f32::from_bits(0x428b8516), f32::from_bits(0x4233e422)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4691-L4722 (chrome/m156)
fn battleOp173(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f30c96), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x426956a5), f32::from_bits(0xc284cd4a)),
        (f32::from_bits(0x4291c05e), f32::from_bits(0xc21ee718)),
    );
    path.cubic_to(
        (f32::from_bits(0x42aed56a), f32::from_bits(0xc150ce71)),
        (f32::from_bits(0x42ac7181), f32::from_bits(0x419b8107)),
        (f32::from_bits(0x428b8516), f32::from_bits(0x4233e422)),
    );
    path.cubic_to(
        (f32::from_bits(0x4285165c), f32::from_bits(0x4247d8d0)),
        (f32::from_bits(0x427b34bd), f32::from_bits(0x425a5d74)),
        (f32::from_bits(0x426a6401), f32::from_bits(0x426b20b1)),
    );
    path.line_to((f32::from_bits(0x42297063), f32::from_bits(0x4229f8c9)));
    path.cubic_to(
        (f32::from_bits(0x42359840), f32::from_bits(0x421ddab1)),
        (f32::from_bits(0x42406a5a), f32::from_bits(0x421077b9)),
        (f32::from_bits(0x4249b72b), f32::from_bits(0x42020ab4)),
    );
    path.line_to((f32::from_bits(0x4249b729), f32::from_bits(0x42020ab3)));
    path.cubic_to(
        (f32::from_bits(0x427950d3), f32::from_bits(0x4160d339)),
        (f32::from_bits(0x427cc584), f32::from_bits(0xc116f1c4)),
        (f32::from_bits(0x4252b998), f32::from_bits(0xc1e5bd26)),
    );
    path.cubic_to(
        (f32::from_bits(0x4228adad), f32::from_bits(0xc24000b5)),
        (f32::from_bits(0x41afb2be), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x426a6401), f32::from_bits(0x426b20b0)));
    path.cubic_to(
        (f32::from_bits(0x420d0644), f32::from_bits(0x42a419c2)),
        (f32::from_bits(0x3eb79d8f), f32::from_bits(0x42b29b69)),
        (f32::from_bits(0xc1f292a7), f32::from_bits(0x429a86c6)),
    );
    path.cubic_to(
        (f32::from_bits(0xc27401e4), f32::from_bits(0x42827223)),
        (f32::from_bits(0xc2a34d81), f32::from_bits(0x4210aea0)),
        (f32::from_bits(0xc2a5dfaf), f32::from_bits(0x404f3106)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2a871dd), f32::from_bits(0xc1ed90fa)),
        (f32::from_bits(0xc283ccf3), f32::from_bits(0xc27113da)),
        (f32::from_bits(0xc21101fe), f32::from_bits(0xc2955440)),
    );
    path.line_to((f32::from_bits(0xc1d1a65c), f32::from_bits(0xc257e5c3)));
    path.cubic_to(
        (f32::from_bits(0xc23e8e16), f32::from_bits(0xc22e45d9)),
        (f32::from_bits(0xc27388d2), f32::from_bits(0xc1abbc0d)),
        (f32::from_bits(0xc26fd138), f32::from_bits(0x4015c6fe)),
    );
    path.cubic_to(
        (f32::from_bits(0xc26c199f), f32::from_bits(0x41d12dcc)),
        (f32::from_bits(0xc2306400), f32::from_bits(0x423c98a5)),
        (f32::from_bits(0xc1af5a7e), f32::from_bits(0x425f695f)),
    );
    path.cubic_to(
        (f32::from_bits(0x3e84bf70), f32::from_bits(0x42811d0c)),
        (f32::from_bits(0x41cbe40c), f32::from_bits(0x426d40fa)),
        (f32::from_bits(0x42297064), f32::from_bits(0x4229f8cc)),
    );
    path.line_to((f32::from_bits(0x426a6401), f32::from_bits(0x426b20b0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4725-L4749 (chrome/m156)
fn battleOp174(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f67553), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x426c5214), f32::from_bits(0xc283df7d)),
        (f32::from_bits(0x4292df93), f32::from_bits(0xc21ab724)),
    );
    path.cubic_to(
        (f32::from_bits(0x42af961c), f32::from_bits(0xc136bd38)),
        (f32::from_bits(0x42abbe10), f32::from_bits(0x41ac5dd5)),
        (f32::from_bits(0x4288e395), f32::from_bits(0x423bcd53)),
    );
    path.line_to((f32::from_bits(0x4245e96c), f32::from_bits(0x4207c2b1)));
    path.cubic_to(
        (f32::from_bits(0x42784d66), f32::from_bits(0x41793464)),
        (f32::from_bits(0x427ddc1f), f32::from_bits(0xc10419c2)),
        (f32::from_bits(0x425458d8), f32::from_bits(0xc1dfaf58)),
    );
    path.cubic_to(
        (f32::from_bits(0x422ad590), f32::from_bits(0xc23ea8e8)),
        (f32::from_bits(0x41b229a4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4288e396), f32::from_bits(0x423bcd52)));
    path.cubic_to(
        (f32::from_bits(0x42821571), f32::from_bits(0x424fa4b8)),
        (f32::from_bits(0x427470be), f32::from_bits(0x4261f24c)),
        (f32::from_bits(0x4262dfb6), f32::from_bits(0x4272637b)),
    );
    path.line_to((f32::from_bits(0x42240156), f32::from_bits(0x422f387f)));
    path.cubic_to(
        (f32::from_bits(0x4230b436), f32::from_bits(0x422355b8)),
        (f32::from_bits(0x423c12ab), f32::from_bits(0x42161a8d)),
        (f32::from_bits(0x4245e96e), f32::from_bits(0x4207c2b2)),
    );
    path.line_to((f32::from_bits(0x4288e396), f32::from_bits(0x423bcd52)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4752-L4783 (chrome/m156)
fn battleOp175(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f67553), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x426c5214), f32::from_bits(0xc283df7d)),
        (f32::from_bits(0x4292df93), f32::from_bits(0xc21ab724)),
    );
    path.cubic_to(
        (f32::from_bits(0x42af961c), f32::from_bits(0xc136bd38)),
        (f32::from_bits(0x42abbe10), f32::from_bits(0x41ac5dd5)),
        (f32::from_bits(0x4288e396), f32::from_bits(0x423bcd52)),
    );
    path.cubic_to(
        (f32::from_bits(0x42821571), f32::from_bits(0x424fa4b8)),
        (f32::from_bits(0x427470be), f32::from_bits(0x4261f24c)),
        (f32::from_bits(0x4262dfb6), f32::from_bits(0x4272637b)),
    );
    path.line_to((f32::from_bits(0x42240156), f32::from_bits(0x422f387f)));
    path.cubic_to(
        (f32::from_bits(0x4230b436), f32::from_bits(0x422355b8)),
        (f32::from_bits(0x423c12ab), f32::from_bits(0x42161a8d)),
        (f32::from_bits(0x4245e96e), f32::from_bits(0x4207c2b2)),
    );
    path.line_to((f32::from_bits(0x4245e96c), f32::from_bits(0x4207c2b1)));
    path.cubic_to(
        (f32::from_bits(0x42784d66), f32::from_bits(0x41793464)),
        (f32::from_bits(0x427ddc1f), f32::from_bits(0xc10419c2)),
        (f32::from_bits(0x425458d8), f32::from_bits(0xc1dfaf58)),
    );
    path.cubic_to(
        (f32::from_bits(0x422ad590), f32::from_bits(0xc23ea8e8)),
        (f32::from_bits(0x41b229a4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4262dfb7), f32::from_bits(0x4272637c)));
    path.cubic_to(
        (f32::from_bits(0x4201435c), f32::from_bits(0x42a6e035)),
        (f32::from_bits(0xc05a052a), f32::from_bits(0x42b2d330)),
        (f32::from_bits(0xc207a774), f32::from_bits(0x429782c3)),
    );
    path.cubic_to(
        (f32::from_bits(0xc280d74a), f32::from_bits(0x427864aa)),
        (f32::from_bits(0xc2a78489), f32::from_bits(0x41fbcc10)),
        (f32::from_bits(0xc2a5f467), f32::from_bits(0xbff86670)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2a46445), f32::from_bits(0xc20d6c6d)),
        (f32::from_bits(0xc275c9b5), f32::from_bits(0xc2821580)),
        (f32::from_bits(0xc1f2ade6), f32::from_bits(0xc29a8413)),
    );
    path.line_to((f32::from_bits(0xc1af6e4e), f32::from_bits(0xc25f6582)));
    path.cubic_to(
        (f32::from_bits(0xc231ad90), f32::from_bits(0xc23c12bd)),
        (f32::from_bits(0xc26dacb3), f32::from_bits(0xc1cc77b7)),
        (f32::from_bits(0xc26fef30), f32::from_bits(0xbfb390a5)),
    );
    path.cubic_to(
        (f32::from_bits(0xc27231ae), f32::from_bits(0x41b605a0)),
        (f32::from_bits(0xc23a46a0), f32::from_bits(0x42338faf)),
        (f32::from_bits(0xc1c42047), f32::from_bits(0x425b0d36)),
    );
    path.cubic_to(
        (f32::from_bits(0xc01d9a6d), f32::from_bits(0x4281455e)),
        (f32::from_bits(0x41bae2f1), f32::from_bits(0x42714420)),
        (f32::from_bits(0x42240157), f32::from_bits(0x422f387f)),
    );
    path.line_to((f32::from_bits(0x4262dfb7), f32::from_bits(0x4272637c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4786-L4810 (chrome/m156)
fn battleOp176(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f9cdf3), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x426f3c43), f32::from_bits(0xc282f30b)),
        (f32::from_bits(0x4293f176), f32::from_bits(0xc2169536)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b044ca), f32::from_bits(0xc11d115b)),
        (f32::from_bits(0x42aaf59e), f32::from_bits(0x41bcd986)),
        (f32::from_bits(0x428633ff), f32::from_bits(0x42436703)),
    );
    path.line_to((f32::from_bits(0x42420751), f32::from_bits(0x420d4138)));
    path.cubic_to(
        (f32::from_bits(0x42772b98), f32::from_bits(0x41888496)),
        (f32::from_bits(0x427ed8af), f32::from_bits(0xc0e315f7)),
        (f32::from_bits(0x4255e4d4), f32::from_bits(0xc1d9b5cc)),
    );
    path.cubic_to(
        (f32::from_bits(0x422cf0fb), f32::from_bits(0xc23d530d)),
        (f32::from_bits(0x41b494e9), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3743ffa9), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x428633ff), f32::from_bits(0x42436705)));
    path.cubic_to(
        (f32::from_bits(0x427e0fd0), f32::from_bits(0x42571b29)),
        (f32::from_bits(0x426d975d), f32::from_bits(0x42692b9b)),
        (f32::from_bits(0x425b4ae0), f32::from_bits(0x427944c1)),
    );
    path.line_to((f32::from_bits(0x421e8652), f32::from_bits(0x423431b3)));
    path.cubic_to(
        (f32::from_bits(0x422bc0b3), f32::from_bits(0x42288e8e)),
        (f32::from_bits(0x4237a8bb), f32::from_bits(0x421b7f95)),
        (f32::from_bits(0x42420752), f32::from_bits(0x420d4138)),
    );
    path.line_to((f32::from_bits(0x428633ff), f32::from_bits(0x42436705)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4813-L4843 (chrome/m156)
fn battleOp177(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3743ffa9), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f9cdf3), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x426f3c43), f32::from_bits(0xc282f30b)),
        (f32::from_bits(0x4293f176), f32::from_bits(0xc2169536)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b044ca), f32::from_bits(0xc11d115b)),
        (f32::from_bits(0x42aaf59e), f32::from_bits(0x41bcd986)),
        (f32::from_bits(0x428633ff), f32::from_bits(0x42436705)),
    );
    path.cubic_to(
        (f32::from_bits(0x427e0fd0), f32::from_bits(0x42571b29)),
        (f32::from_bits(0x426d975d), f32::from_bits(0x42692b9b)),
        (f32::from_bits(0x425b4ae0), f32::from_bits(0x427944c1)),
    );
    path.line_to((f32::from_bits(0x421e8652), f32::from_bits(0x423431b3)));
    path.cubic_to(
        (f32::from_bits(0x422bc0b3), f32::from_bits(0x42288e8e)),
        (f32::from_bits(0x4237a8bb), f32::from_bits(0x421b7f95)),
        (f32::from_bits(0x42420751), f32::from_bits(0x420d4138)),
    );
    path.cubic_to(
        (f32::from_bits(0x42772b98), f32::from_bits(0x41888496)),
        (f32::from_bits(0x427ed8af), f32::from_bits(0xc0e315f7)),
        (f32::from_bits(0x4255e4d4), f32::from_bits(0xc1d9b5cc)),
    );
    path.cubic_to(
        (f32::from_bits(0x422cf0fb), f32::from_bits(0xc23d530d)),
        (f32::from_bits(0x41b494e9), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3743ffa9), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x425b4ae0), f32::from_bits(0x427944c0)));
    path.cubic_to(
        (f32::from_bits(0x41eb12b8), f32::from_bits(0x42a964d5)),
        (f32::from_bits(0xc0e3546a), f32::from_bits(0x42b2bc1c)),
        (f32::from_bits(0xc2157060), f32::from_bits(0x42943ba4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2873b19), f32::from_bits(0x426b7658)),
        (f32::from_bits(0xc2ab209f), f32::from_bits(0x41d60b1d)),
        (f32::from_bits(0xc2a5685b), f32::from_bits(0xc0e02f3c)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29fb018), f32::from_bits(0xc223115c)),
        (f32::from_bits(0xc263001e), f32::from_bits(0xc28acd07)),
        (f32::from_bits(0xc1c2e1a0), f32::from_bits(0xc29eb07c)),
    );
    path.line_to((f32::from_bits(0xc18ce0d1), f32::from_bits(0xc2656e32)));
    path.cubic_to(
        (f32::from_bits(0xc22418c2), f32::from_bits(0xc248ad0a)),
        (f32::from_bits(0xc266dfbc), f32::from_bits(0xc1ebc2b6)),
        (f32::from_bits(0xc26f24bb), f32::from_bits(0xc0a20f94)),
    );
    path.cubic_to(
        (f32::from_bits(0xc27769ba), f32::from_bits(0x419abaee)),
        (f32::from_bits(0xc24383ac), f32::from_bits(0x422a36b0)),
        (f32::from_bits(0xc1d80e5c), f32::from_bits(0x4256500a)),
    );
    path.cubic_to(
        (f32::from_bits(0xc0a45587), f32::from_bits(0x428134b2)),
        (f32::from_bits(0x41a9eeb8), f32::from_bits(0x4274e820)),
        (f32::from_bits(0x421e8655), f32::from_bits(0x423431b1)),
    );
    path.line_to((f32::from_bits(0x425b4ae0), f32::from_bits(0x427944c0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4846-L4870 (chrome/m156)
fn battleOp178(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41fc5f30), f32::from_bits(0xc2a5fffe)),
        (f32::from_bits(0x427176a0), f32::from_bits(0xc2823b95)),
        (f32::from_bits(0x4294be35), f32::from_bits(0xc21365c9)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b0c118), f32::from_bits(0xc1095198)),
        (f32::from_bits(0x42aa4b8f), f32::from_bits(0x41c9721a)),
        (f32::from_bits(0x42841312), f32::from_bits(0x42491ec0)),
    );
    path.line_to((f32::from_bits(0x423ef37b), f32::from_bits(0x42116356)));
    path.cubic_to(
        (f32::from_bits(0x427635bc), f32::from_bits(0x41919f96)),
        (f32::from_bits(0x427f8c66), f32::from_bits(0xc0c68887)),
        (f32::from_bits(0x42570cd6), f32::from_bits(0xc1d51ae4)),
    );
    path.cubic_to(
        (f32::from_bits(0x422e8d45), f32::from_bits(0xc23c49d3)),
        (f32::from_bits(0x41b66ffd), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb7060057), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42841313), f32::from_bits(0x42491ebf)));
    path.cubic_to(
        (f32::from_bits(0x42793d8e), f32::from_bits(0x425cb36e)),
        (f32::from_bits(0x4268336d), f32::from_bits(0x426e9032)),
        (f32::from_bits(0x4255582b), f32::from_bits(0x427e60c5)),
    );
    path.line_to((f32::from_bits(0x421a3990), f32::from_bits(0x4237e342)));
    path.cubic_to(
        (f32::from_bits(0x4227db27), f32::from_bits(0x422c7494)),
        (f32::from_bits(0x42342c7f), f32::from_bits(0x421f8af7)),
        (f32::from_bits(0x423ef37c), f32::from_bits(0x42116357)),
    );
    path.line_to((f32::from_bits(0x42841313), f32::from_bits(0x42491ebf)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4873-L4903 (chrome/m156)
fn battleOp179(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb7060057), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41fc5f30), f32::from_bits(0xc2a5fffe)),
        (f32::from_bits(0x427176a0), f32::from_bits(0xc2823b95)),
        (f32::from_bits(0x4294be35), f32::from_bits(0xc21365c9)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b0c118), f32::from_bits(0xc1095198)),
        (f32::from_bits(0x42aa4b8f), f32::from_bits(0x41c9721a)),
        (f32::from_bits(0x42841313), f32::from_bits(0x42491ebf)),
    );
    path.cubic_to(
        (f32::from_bits(0x42793d8e), f32::from_bits(0x425cb36e)),
        (f32::from_bits(0x4268336d), f32::from_bits(0x426e9032)),
        (f32::from_bits(0x4255582b), f32::from_bits(0x427e60c5)),
    );
    path.line_to((f32::from_bits(0x421a3990), f32::from_bits(0x4237e342)));
    path.cubic_to(
        (f32::from_bits(0x4227db27), f32::from_bits(0x422c7494)),
        (f32::from_bits(0x42342c7f), f32::from_bits(0x421f8af7)),
        (f32::from_bits(0x423ef37b), f32::from_bits(0x42116356)),
    );
    path.cubic_to(
        (f32::from_bits(0x427635bc), f32::from_bits(0x41919f96)),
        (f32::from_bits(0x427f8c66), f32::from_bits(0xc0c68887)),
        (f32::from_bits(0x42570cd6), f32::from_bits(0xc1d51ae4)),
    );
    path.cubic_to(
        (f32::from_bits(0x422e8d45), f32::from_bits(0xc23c49d3)),
        (f32::from_bits(0x41b66ffd), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb7060057), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4255582a), f32::from_bits(0x427e60c6)));
    path.cubic_to(
        (f32::from_bits(0x41d8da26), f32::from_bits(0x42ab2f9f)),
        (f32::from_bits(0xc11f0392), f32::from_bits(0x42b2763a)),
        (f32::from_bits(0xc21fc8f1), f32::from_bits(0x4291829a)),
    );
    path.cubic_to(
        (f32::from_bits(0xc28be87e), f32::from_bits(0x42611df4)),
        (f32::from_bits(0xc2ad8941), f32::from_bits(0x41b88f93)),
        (f32::from_bits(0xc2a49219), f32::from_bits(0xc12de56c)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29b9af2), f32::from_bits(0xc2333a80)),
        (f32::from_bits(0xc253c58e), f32::from_bits(0xc2910614)),
        (f32::from_bits(0xc19d7dc6), f32::from_bits(0xc2a14359)),
    );
    path.line_to((f32::from_bits(0xc163b2c9), f32::from_bits(0xc26926c4)));
    path.cubic_to(
        (f32::from_bits(0xc2191685), f32::from_bits(0xc251ac40)),
        (f32::from_bits(0xc260f8ae), f32::from_bits(0xc201900e)),
        (f32::from_bits(0xc26deef7), f32::from_bits(0xc0fb6a70)),
    );
    path.cubic_to(
        (f32::from_bits(0xc27ae541), f32::from_bits(0x41856ae3)),
        (f32::from_bits(0xc24a46d8), f32::from_bits(0x4222bc35)),
        (f32::from_bits(0xc1e7039a), f32::from_bits(0x42526049)),
    );
    path.cubic_to(
        (f32::from_bits(0xc0e5e60c), f32::from_bits(0x4281022e)),
        (f32::from_bits(0x419cc2c4), f32::from_bits(0x42777f70)),
        (f32::from_bits(0x421a3996), f32::from_bits(0x4237e33e)),
    );
    path.line_to((f32::from_bits(0x4255582a), f32::from_bits(0x427e60c6)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4906-L4930 (chrome/m156)
fn battleOp180(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41fed5d1), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4273981d), f32::from_bits(0xc28189e8)),
        (f32::from_bits(0x42957e40), f32::from_bits(0xc210547e)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b13073), f32::from_bits(0xc0eca961)),
        (f32::from_bits(0x42a99b35), f32::from_bits(0x41d57c6c)),
        (f32::from_bits(0x4281fa62), f32::from_bits(0x424e82d3)),
    );
    path.line_to((f32::from_bits(0x423beb8b), f32::from_bits(0x421548fc)));
    path.cubic_to(
        (f32::from_bits(0x427536c2), f32::from_bits(0x419a53c7)),
        (f32::from_bits(0x428016af), f32::from_bits(0xc0ab14a9)),
        (f32::from_bits(0x4258227d), f32::from_bits(0xc1d0ab83)),
    );
    path.cubic_to(
        (f32::from_bits(0x4230179a), f32::from_bits(0xc23b48ee)),
        (f32::from_bits(0x41b837da), f32::from_bits(0xc2700002)),
        (f32::from_bits(0xb7060057), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4281fa62), f32::from_bits(0x424e82d5)));
    path.cubic_to(
        (f32::from_bits(0x4274817d), f32::from_bits(0x4261f5b7)),
        (f32::from_bits(0x4262ebfa), f32::from_bits(0x42739d02)),
        (f32::from_bits(0x424f88b8), f32::from_bits(0x428191ef)),
    );
    path.line_to((f32::from_bits(0x4216064f), f32::from_bits(0x423b5489)));
    path.cubic_to(
        (f32::from_bits(0x42240a35), f32::from_bits(0x42301b25)),
        (f32::from_bits(0x4230c051), f32::from_bits(0x4223582f)),
        (f32::from_bits(0x423beb8c), f32::from_bits(0x421548fc)),
    );
    path.line_to((f32::from_bits(0x4281fa62), f32::from_bits(0x424e82d5)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4933-L4963 (chrome/m156)
fn battleOp181(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb7060057), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41fed5d1), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4273981d), f32::from_bits(0xc28189e8)),
        (f32::from_bits(0x42957e40), f32::from_bits(0xc210547e)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b13073), f32::from_bits(0xc0eca961)),
        (f32::from_bits(0x42a99b35), f32::from_bits(0x41d57c6c)),
        (f32::from_bits(0x4281fa62), f32::from_bits(0x424e82d5)),
    );
    path.cubic_to(
        (f32::from_bits(0x4274817d), f32::from_bits(0x4261f5b7)),
        (f32::from_bits(0x4262ebfa), f32::from_bits(0x42739d02)),
        (f32::from_bits(0x424f88b8), f32::from_bits(0x428191ef)),
    );
    path.line_to((f32::from_bits(0x4216064f), f32::from_bits(0x423b5489)));
    path.cubic_to(
        (f32::from_bits(0x42240a35), f32::from_bits(0x42301b25)),
        (f32::from_bits(0x4230c051), f32::from_bits(0x4223582f)),
        (f32::from_bits(0x423beb8b), f32::from_bits(0x421548fc)),
    );
    path.cubic_to(
        (f32::from_bits(0x427536c2), f32::from_bits(0x419a53c7)),
        (f32::from_bits(0x428016af), f32::from_bits(0xc0ab14a9)),
        (f32::from_bits(0x4258227d), f32::from_bits(0xc1d0ab83)),
    );
    path.cubic_to(
        (f32::from_bits(0x4230179a), f32::from_bits(0xc23b48ee)),
        (f32::from_bits(0x41b837da), f32::from_bits(0xc2700002)),
        (f32::from_bits(0xb7060057), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x424f88ba), f32::from_bits(0x428191f0)));
    path.cubic_to(
        (f32::from_bits(0x41c732b7), f32::from_bits(0x42acca52)),
        (f32::from_bits(0xc14a7268), f32::from_bits(0x42b208b4)),
        (f32::from_bits(0xc22982dc), f32::from_bits(0x428ebb75)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2903490), f32::from_bits(0x4256dc6c)),
        (f32::from_bits(0xc2af8c6f), f32::from_bits(0x419be833)),
        (f32::from_bits(0xc2a36e37), f32::from_bits(0xc168c0a6)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2974fff), f32::from_bits(0xc242546a)),
        (f32::from_bits(0xc2448acf), f32::from_bits(0xc29698ac)),
        (f32::from_bits(0xc17253d7), f32::from_bits(0xc2a33682)),
    );
    path.line_to((f32::from_bits(0xc12f2d38), f32::from_bits(0xc26bf872)));
    path.cubic_to(
        (f32::from_bits(0xc20e1427), f32::from_bits(0xc259bacc)),
        (f32::from_bits(0xc25ac3d7), f32::from_bits(0xc20c7ab2)),
        (f32::from_bits(0xc26c48f7), f32::from_bits(0xc1284130)),
    );
    path.cubic_to(
        (f32::from_bits(0xc27dce17), f32::from_bits(0x41616864)),
        (f32::from_bits(0xc2507d50), f32::from_bits(0x421b5239)),
        (f32::from_bits(0xc1f51386), f32::from_bits(0x424e5c1e)),
    );
    path.cubic_to(
        (f32::from_bits(0xc11258cd), f32::from_bits(0x4280b301)),
        (f32::from_bits(0x418fffac), f32::from_bits(0x4279d13a)),
        (f32::from_bits(0x42160652), f32::from_bits(0x423b5488)),
    );
    path.line_to((f32::from_bits(0x424f88ba), f32::from_bits(0x428191f0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4966-L4990 (chrome/m156)
fn battleOp182(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x420048ef), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4275172d), f32::from_bits(0xc2810bd2)),
        (f32::from_bits(0x429602e3), f32::from_bits(0xc20e29dc)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b17a30), f32::from_bits(0xc0d1e0a1)),
        (f32::from_bits(0x42a9174e), f32::from_bits(0x41ddef9e)),
        (f32::from_bits(0x4280787d), f32::from_bits(0x4252400e)),
    );
    path.line_to((f32::from_bits(0x4239bd9f), f32::from_bits(0x4217fcf6)));
    path.cubic_to(
        (f32::from_bits(0x4274780f), f32::from_bits(0x41a06f8c)),
        (f32::from_bits(0x42804bfe), f32::from_bits(0xc097b7f0)),
        (f32::from_bits(0x4258e240), f32::from_bits(0xc1cd899e)),
    );
    path.cubic_to(
        (f32::from_bits(0x42312c84), f32::from_bits(0xc23a929f)),
        (f32::from_bits(0x41b978e3), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36d3ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4280787d), f32::from_bits(0x42524010)));
    path.cubic_to(
        (f32::from_bits(0x42711c0e), f32::from_bits(0x42659909)),
        (f32::from_bits(0x425f24ad), f32::from_bits(0x42771864)),
        (f32::from_bits(0x424b624a), f32::from_bits(0x4283347a)),
    );
    path.line_to((f32::from_bits(0x42130648), f32::from_bits(0x423db1a5)));
    path.cubic_to(
        (f32::from_bits(0x42214ef3), f32::from_bits(0x42329f82)),
        (f32::from_bits(0x422e4bcd), f32::from_bits(0x4225f96c)),
        (f32::from_bits(0x4239bd9f), f32::from_bits(0x4217fcf7)),
    );
    path.line_to((f32::from_bits(0x4280787d), f32::from_bits(0x42524010)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L4993-L5023 (chrome/m156)
fn battleOp183(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x36d3ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x420048ef), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4275172d), f32::from_bits(0xc2810bd2)),
        (f32::from_bits(0x429602e3), f32::from_bits(0xc20e29dc)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b17a30), f32::from_bits(0xc0d1e0a1)),
        (f32::from_bits(0x42a9174e), f32::from_bits(0x41ddef9e)),
        (f32::from_bits(0x4280787d), f32::from_bits(0x42524010)),
    );
    path.cubic_to(
        (f32::from_bits(0x42711c0e), f32::from_bits(0x42659909)),
        (f32::from_bits(0x425f24ad), f32::from_bits(0x42771864)),
        (f32::from_bits(0x424b624a), f32::from_bits(0x4283347a)),
    );
    path.line_to((f32::from_bits(0x42130648), f32::from_bits(0x423db1a5)));
    path.cubic_to(
        (f32::from_bits(0x42214ef3), f32::from_bits(0x42329f82)),
        (f32::from_bits(0x422e4bcd), f32::from_bits(0x4225f96c)),
        (f32::from_bits(0x4239bd9f), f32::from_bits(0x4217fcf6)),
    );
    path.cubic_to(
        (f32::from_bits(0x4274780f), f32::from_bits(0x41a06f8c)),
        (f32::from_bits(0x42804bfe), f32::from_bits(0xc097b7f0)),
        (f32::from_bits(0x4258e240), f32::from_bits(0xc1cd899e)),
    );
    path.cubic_to(
        (f32::from_bits(0x42312c84), f32::from_bits(0xc23a929f)),
        (f32::from_bits(0x41b978e3), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36d3ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x424b624a), f32::from_bits(0x42833479)));
    path.cubic_to(
        (f32::from_bits(0x41baac2f), f32::from_bits(0x42adda12)),
        (f32::from_bits(0xc168f6a7), f32::from_bits(0x42b1a2b3)),
        (f32::from_bits(0xc2303c92), f32::from_bits(0x428cae5c)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2931dbe), f32::from_bits(0x424f7409)),
        (f32::from_bits(0xc2b0c9d8), f32::from_bits(0x41878abe)),
        (f32::from_bits(0xc2a26e7f), f32::from_bits(0xc188ef9a)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2941327), f32::from_bits(0xc24cb4f5)),
        (f32::from_bits(0xc2397a7c), f32::from_bits(0xc29a4742)),
        (f32::from_bits(0xc13ec328), f32::from_bits(0xc2a44746)),
    );
    path.line_to((f32::from_bits(0xc109e67a), f32::from_bits(0xc26d82d0)));
    path.cubic_to(
        (f32::from_bits(0xc20614b0), f32::from_bits(0xc25f0d94)),
        (f32::from_bits(0xc2561585), f32::from_bits(0xc213fb18)),
        (f32::from_bits(0xc26ad744), f32::from_bits(0xc145fabb)),
    );
    path.cubic_to(
        (f32::from_bits(0xc27f9901), f32::from_bits(0x4143f6e8)),
        (f32::from_bits(0xc254b2af), f32::from_bits(0x4215f75b)),
        (f32::from_bits(0xc1feccbb), f32::from_bits(0x424b64f3)),
    );
    path.cubic_to(
        (f32::from_bits(0xc128682f), f32::from_bits(0x42806945)),
        (f32::from_bits(0x4186f1ba), f32::from_bits(0x427b5a1e)),
        (f32::from_bits(0x4213064f), f32::from_bits(0x423db1a2)),
    );
    path.line_to((f32::from_bits(0x424b624a), f32::from_bits(0x42833479)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5026-L5050 (chrome/m156)
fn battleOp184(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42011b87), f32::from_bits(0xc2a5fffe)),
        (f32::from_bits(0x427681ab), f32::from_bits(0xc280937a)),
        (f32::from_bits(0x42967eb3), f32::from_bits(0xc20c1a94)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b1bc91), f32::from_bits(0xc0b87191)),
        (f32::from_bits(0x42a89454), f32::from_bits(0x41e5ed6f)),
        (f32::from_bits(0x427e0902), f32::from_bits(0x4255c0a2)),
    );
    path.line_to((f32::from_bits(0x4237a3d0), f32::from_bits(0x421a8517)));
    path.cubic_to(
        (f32::from_bits(0x4273bab4), f32::from_bits(0x41a63674)),
        (f32::from_bits(0x42807bfc), f32::from_bits(0xc0855530)),
        (f32::from_bits(0x42599545), f32::from_bits(0xc1ca8f4f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42323293), f32::from_bits(0xc239e4a8)),
        (f32::from_bits(0x41baa959), f32::from_bits(0xc2700002)),
        (f32::from_bits(0xb5600574), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x427e0901), f32::from_bits(0x4255c0a4)));
    path.cubic_to(
        (f32::from_bits(0x426dd77c), f32::from_bits(0x4268ff65)),
        (f32::from_bits(0x425b838b), f32::from_bits(0x427a571f)),
        (f32::from_bits(0x42476779), f32::from_bits(0x4284b92f)),
    );
    path.line_to((f32::from_bits(0x421025c9), f32::from_bits(0x423fe3a3)));
    path.cubic_to(
        (f32::from_bits(0x421eaf4b), f32::from_bits(0x4234f80b)),
        (f32::from_bits(0x422bef10), f32::from_bits(0x42286e9a)),
        (f32::from_bits(0x4237a3d2), f32::from_bits(0x421a8517)),
    );
    path.line_to((f32::from_bits(0x427e0901), f32::from_bits(0x4255c0a4)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5053-L5085 (chrome/m156)
fn battleOp185(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42011b87), f32::from_bits(0xc2a5fffe)),
        (f32::from_bits(0x427681ab), f32::from_bits(0xc280937a)),
        (f32::from_bits(0x42967eb3), f32::from_bits(0xc20c1a94)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b1bc91), f32::from_bits(0xc0b87191)),
        (f32::from_bits(0x42a89454), f32::from_bits(0x41e5ed6f)),
        (f32::from_bits(0x427e0902), f32::from_bits(0x4255c0a2)),
    );
    path.line_to((f32::from_bits(0x427e0901), f32::from_bits(0x4255c0a4)));
    path.cubic_to(
        (f32::from_bits(0x426dd77c), f32::from_bits(0x4268ff65)),
        (f32::from_bits(0x425b838b), f32::from_bits(0x427a571f)),
        (f32::from_bits(0x42476779), f32::from_bits(0x4284b92f)),
    );
    path.line_to((f32::from_bits(0x421025c9), f32::from_bits(0x423fe3a3)));
    path.cubic_to(
        (f32::from_bits(0x421eaf4b), f32::from_bits(0x4234f80b)),
        (f32::from_bits(0x422bef10), f32::from_bits(0x42286e9a)),
        (f32::from_bits(0x4237a3d2), f32::from_bits(0x421a8517)),
    );
    path.line_to((f32::from_bits(0x4237a3d0), f32::from_bits(0x421a8517)));
    path.cubic_to(
        (f32::from_bits(0x4273bab4), f32::from_bits(0x41a63674)),
        (f32::from_bits(0x42807bfc), f32::from_bits(0xc0855530)),
        (f32::from_bits(0x42599545), f32::from_bits(0xc1ca8f4f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42323293), f32::from_bits(0xc239e4a8)),
        (f32::from_bits(0x41baa959), f32::from_bits(0xc2700002)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42476779), f32::from_bits(0x4284b92f)));
    path.cubic_to(
        (f32::from_bits(0x41aeb99d), f32::from_bits(0x42aece6d)),
        (f32::from_bits(0xc182ebc7), f32::from_bits(0x42b12f04)),
        (f32::from_bits(0xc236847b), f32::from_bits(0x428aaa1d)),
    );
    path.cubic_to(
        (f32::from_bits(0xc295c989), f32::from_bits(0x42484a6d)),
        (f32::from_bits(0xc2b1d401), f32::from_bits(0x41683386)),
        (f32::from_bits(0xc2a15607), f32::from_bits(0xc19c4a77)),
    );
    path.cubic_to(
        (f32::from_bits(0xc290d80f), f32::from_bits(0xc2565754)),
        (f32::from_bits(0xc22ebdc1), f32::from_bits(0xc29d94aa)),
        (f32::from_bits(0xc10da15c), f32::from_bits(0xc2a50da2)),
    );
    path.line_to((f32::from_bits(0xc0ccc448), f32::from_bits(0xc26ea197)));
    path.cubic_to(
        (f32::from_bits(0xc1fca350), f32::from_bits(0xc263d3da)),
        (f32::from_bits(0xc25169ba), f32::from_bits(0xc21af203)),
        (f32::from_bits(0xc26941c7), f32::from_bits(0xc161f664)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2808cea), f32::from_bits(0x4127db45)),
        (f32::from_bits(0xc2588f4e), f32::from_bits(0x4210c9da)),
        (f32::from_bits(0xc203f0b6), f32::from_bits(0x42487a91)),
    );
    path.cubic_to(
        (f32::from_bits(0xc13d487f), f32::from_bits(0x428015a4)),
        (f32::from_bits(0x417c9d5c), f32::from_bits(0x427cbb65)),
        (f32::from_bits(0x421025ca), f32::from_bits(0x423fe3a2)),
    );
    path.line_to((f32::from_bits(0x42476779), f32::from_bits(0x4284b92f)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5088-L5112 (chrome/m156)
fn battleOp186(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4201bd60), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x427797bb), f32::from_bits(0xc2803682)),
        (f32::from_bits(0x4296dc8c), f32::from_bits(0xc20a848f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b1ed3b), f32::from_bits(0xc0a4e0c3)),
        (f32::from_bits(0x42a82bcd), f32::from_bits(0x41ec0db8)),
        (f32::from_bits(0x427bc56e), f32::from_bits(0x42586a20)),
    );
    path.line_to((f32::from_bits(0x423600d6), f32::from_bits(0x421c71bc)));
    path.cubic_to(
        (f32::from_bits(0x42732394), f32::from_bits(0x41aaa425)),
        (f32::from_bits(0x42809f29), f32::from_bits(0xc06e60a8)),
        (f32::from_bits(0x425a1cf3), f32::from_bits(0xc1c84447)),
    );
    path.cubic_to(
        (f32::from_bits(0x4232fb94), f32::from_bits(0xc2395e3c)),
        (f32::from_bits(0x41bb9357), f32::from_bits(0xc2700002)),
        (f32::from_bits(0xb69400ae), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x427bc56c), f32::from_bits(0x42586a22)));
    path.cubic_to(
        (f32::from_bits(0x426b4cc6), f32::from_bits(0x426b93ad)),
        (f32::from_bits(0x4258b1e1), f32::from_bits(0x427ccbca)),
        (f32::from_bits(0x42445140), f32::from_bits(0x4285de6e)),
    );
    path.line_to((f32::from_bits(0x420dea8b), f32::from_bits(0x42418b9b)));
    path.cubic_to(
        (f32::from_bits(0x421ca599), f32::from_bits(0x4236be7f)),
        (f32::from_bits(0x422a18a8), f32::from_bits(0x422a4be8)),
        (f32::from_bits(0x423600d6), f32::from_bits(0x421c71bc)),
    );
    path.line_to((f32::from_bits(0x427bc56c), f32::from_bits(0x42586a22)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5115-L5149 (chrome/m156)
fn battleOp187(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb69400ae), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4201bd60), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x427797bb), f32::from_bits(0xc2803682)),
        (f32::from_bits(0x4296dc8c), f32::from_bits(0xc20a848f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b1ed3b), f32::from_bits(0xc0a4e0c3)),
        (f32::from_bits(0x42a82bcd), f32::from_bits(0x41ec0db8)),
        (f32::from_bits(0x427bc56e), f32::from_bits(0x42586a20)),
    );
    path.line_to((f32::from_bits(0x423600d6), f32::from_bits(0x421c71bc)));
    path.cubic_to(
        (f32::from_bits(0x42732394), f32::from_bits(0x41aaa425)),
        (f32::from_bits(0x42809f29), f32::from_bits(0xc06e60a8)),
        (f32::from_bits(0x425a1cf3), f32::from_bits(0xc1c84447)),
    );
    path.cubic_to(
        (f32::from_bits(0x4232fb94), f32::from_bits(0xc2395e3c)),
        (f32::from_bits(0x41bb9357), f32::from_bits(0xc2700002)),
        (f32::from_bits(0xb69400ae), f32::from_bits(0xc2700000)),
    );
    path.close();
    path.move_to((f32::from_bits(0x423600d6), f32::from_bits(0x421c71bc)));
    path.line_to((f32::from_bits(0x427bc56c), f32::from_bits(0x42586a22)));
    path.cubic_to(
        (f32::from_bits(0x426b4cc6), f32::from_bits(0x426b93ad)),
        (f32::from_bits(0x4258b1e1), f32::from_bits(0x427ccbca)),
        (f32::from_bits(0x42445140), f32::from_bits(0x4285de6e)),
    );
    path.line_to((f32::from_bits(0x420dea8b), f32::from_bits(0x42418b9b)));
    path.cubic_to(
        (f32::from_bits(0x421ca599), f32::from_bits(0x4236be7f)),
        (f32::from_bits(0x422a18a8), f32::from_bits(0x422a4be8)),
        (f32::from_bits(0x423600d6), f32::from_bits(0x421c71bc)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42445140), f32::from_bits(0x4285de6e)));
    path.cubic_to(
        (f32::from_bits(0x41a5801a), f32::from_bits(0x42af8153)),
        (f32::from_bits(0xc18dfe3b), f32::from_bits(0x42b0c99d)),
        (f32::from_bits(0xc23b472e), f32::from_bits(0x42891183)),
    );
    path.cubic_to(
        (f32::from_bits(0xc297c79f), f32::from_bits(0x4242b2d1)),
        (f32::from_bits(0xc2b28961), f32::from_bits(0x414a2ba6)),
        (f32::from_bits(0xc2a0659f), f32::from_bits(0xc1ab0f22)),
    );
    path.cubic_to(
        (f32::from_bits(0xc28e41db), f32::from_bits(0xc25d9a0f)),
        (f32::from_bits(0xc2265613), f32::from_bits(0xc29ffd9f)),
        (f32::from_bits(0xc0cf8787), f32::from_bits(0xc2a57e12)),
    );
    path.line_to((f32::from_bits(0xc09605ca), f32::from_bits(0xc26f4428)));
    path.cubic_to(
        (f32::from_bits(0xc1f07c7d), f32::from_bits(0xc2674fd1)),
        (f32::from_bits(0xc24dac50), f32::from_bits(0xc22031a9)),
        (f32::from_bits(0xc267e62b), f32::from_bits(0xc1775074)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2811003), f32::from_bits(0x411225be)),
        (f32::from_bits(0xc25b70c1), f32::from_bits(0x420cbef2)),
        (f32::from_bits(0xc20761ad), f32::from_bits(0x42462bd0)),
    );
    path.cubic_to(
        (f32::from_bits(0xc14d4a68), f32::from_bits(0x427f98ac)),
        (f32::from_bits(0x416f472e), f32::from_bits(0x427dbe0b)),
        (f32::from_bits(0x420dea8f), f32::from_bits(0x42418b9b)),
    );
    path.line_to((f32::from_bits(0x42445140), f32::from_bits(0x4285de6e)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5152-L5176 (chrome/m156)
fn battleOp188(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42025498), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42789b1b), f32::from_bits(0xc27fbe84)),
        (f32::from_bits(0x42973334), f32::from_bits(0xc2090897)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b218da), f32::from_bits(0xc092954a)),
        (f32::from_bits(0x42a7c71a), f32::from_bits(0x41f1c3b5)),
        (f32::from_bits(0x4279a1de), f32::from_bits(0x425ae0d9)),
    );
    path.line_to((f32::from_bits(0x42347503), f32::from_bits(0x421e39ac)));
    path.cubic_to(
        (f32::from_bits(0x427291fe), f32::from_bits(0x41aec4fe)),
        (f32::from_bits(0x4280beb1), f32::from_bits(0xc053ed89)),
        (f32::from_bits(0x425a9a3a), f32::from_bits(0xc1c61ef1)),
    );
    path.cubic_to(
        (f32::from_bits(0x4233b713), f32::from_bits(0xc238e018)),
        (f32::from_bits(0x41bc6df5), f32::from_bits(0xc2700002)),
        (f32::from_bits(0xb7240057), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4279a1de), f32::from_bits(0x425ae0d9)));
    path.cubic_to(
        (f32::from_bits(0x4268e6ce), f32::from_bits(0x426df5b7)),
        (f32::from_bits(0x425609c8), f32::from_bits(0x427f0f64)),
        (f32::from_bits(0x42416967), f32::from_bits(0x4286ec0f)),
    );
    path.line_to((f32::from_bits(0x420bd0d2), f32::from_bits(0x42431170)));
    path.cubic_to(
        (f32::from_bits(0x421ab9f8), f32::from_bits(0x4238617e)),
        (f32::from_bits(0x42285cd4), f32::from_bits(0x422c04e7)),
        (f32::from_bits(0x42347505), f32::from_bits(0x421e39ac)),
    );
    path.line_to((f32::from_bits(0x4279a1de), f32::from_bits(0x425ae0d9)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5179-L5210 (chrome/m156)
fn battleOp189(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb7240057), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42025498), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42789b1b), f32::from_bits(0xc27fbe84)),
        (f32::from_bits(0x42973334), f32::from_bits(0xc2090897)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b218da), f32::from_bits(0xc092954a)),
        (f32::from_bits(0x42a7c71a), f32::from_bits(0x41f1c3b5)),
        (f32::from_bits(0x4279a1de), f32::from_bits(0x425ae0d9)),
    );
    path.cubic_to(
        (f32::from_bits(0x4268e6ce), f32::from_bits(0x426df5b7)),
        (f32::from_bits(0x425609c8), f32::from_bits(0x427f0f64)),
        (f32::from_bits(0x42416967), f32::from_bits(0x4286ec0f)),
    );
    path.line_to((f32::from_bits(0x420bd0d2), f32::from_bits(0x42431170)));
    path.cubic_to(
        (f32::from_bits(0x421ab9f8), f32::from_bits(0x4238617e)),
        (f32::from_bits(0x42285cd4), f32::from_bits(0x422c04e7)),
        (f32::from_bits(0x42347505), f32::from_bits(0x421e39ac)),
    );
    path.line_to((f32::from_bits(0x42347503), f32::from_bits(0x421e39ac)));
    path.cubic_to(
        (f32::from_bits(0x427291fe), f32::from_bits(0x41aec4fe)),
        (f32::from_bits(0x4280beb1), f32::from_bits(0xc053ed89)),
        (f32::from_bits(0x425a9a3a), f32::from_bits(0xc1c61ef1)),
    );
    path.cubic_to(
        (f32::from_bits(0x4233b713), f32::from_bits(0xc238e018)),
        (f32::from_bits(0x41bc6df5), f32::from_bits(0xc2700002)),
        (f32::from_bits(0xb7240057), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42416967), f32::from_bits(0x4286ec0f)));
    path.cubic_to(
        (f32::from_bits(0x419cd99a), f32::from_bits(0x42b02173)),
        (f32::from_bits(0xc19850b8), f32::from_bits(0x42b06117)),
        (f32::from_bits(0xc23fac11), f32::from_bits(0x42878a96)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29997e3), f32::from_bits(0x423d682a)),
        (f32::from_bits(0xc2b3208c), f32::from_bits(0x412e025f)),
        (f32::from_bits(0xc29f71a3), f32::from_bits(0xc1b8c415)),
    );
    path.cubic_to(
        (f32::from_bits(0xc28bc2ba), f32::from_bits(0xc26444ae)),
        (f32::from_bits(0xc21e5e96), f32::from_bits(0xc2a223df)),
        (f32::from_bits(0xc088ac52), f32::from_bits(0xc2a5c7b3)),
    );
    path.line_to((f32::from_bits(0xc0459a01), f32::from_bits(0xc26fae99)));
    path.cubic_to(
        (f32::from_bits(0xc1e4f7d0), f32::from_bits(0xc26a6b5c)),
        (f32::from_bits(0xc24a1045), f32::from_bits(0xc225035c)),
        (f32::from_bits(0xc266856e), f32::from_bits(0xc18590cd)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2817d4a), f32::from_bits(0x40fb9475)),
        (f32::from_bits(0xc25e0ffd), f32::from_bits(0x4208ebae)),
        (f32::from_bits(0xc20a8edd), f32::from_bits(0x4243f69e)),
    );
    path.cubic_to(
        (f32::from_bits(0xc15c36ee), f32::from_bits(0x427f018f)),
        (f32::from_bits(0x4162c57c), f32::from_bits(0x427ea58e)),
        (f32::from_bits(0x420bd0d7), f32::from_bits(0x4243116e)),
    );
    path.line_to((f32::from_bits(0x42416967), f32::from_bits(0x4286ec0f)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5213-L5237 (chrome/m156)
fn battleOp190(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4202b56e), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427940ff), f32::from_bits(0xc27f4e67)),
        (f32::from_bits(0x42976a2d), f32::from_bits(0xc20814ff)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b233da), f32::from_bits(0xc086dcb5)),
        (f32::from_bits(0x42a78518), f32::from_bits(0x41f56a27)),
        (f32::from_bits(0x42784037), f32::from_bits(0x425c71a4)),
    );
    path.line_to((f32::from_bits(0x4233755d), f32::from_bits(0x421f5b67)));
    path.cubic_to(
        (f32::from_bits(0x4272328d), f32::from_bits(0x41b16880)),
        (f32::from_bits(0x4280d235), f32::from_bits(0xc042fb32)),
        (f32::from_bits(0x425ae9b3), f32::from_bits(0xc1c4bebc)),
    );
    path.cubic_to(
        (f32::from_bits(0x42342efc), f32::from_bits(0xc2388f09)),
        (f32::from_bits(0x41bcf9fa), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42784038), f32::from_bits(0x425c71a4)));
    path.cubic_to(
        (f32::from_bits(0x42675aa4), f32::from_bits(0x426f78d5)),
        (f32::from_bits(0x4254535c), f32::from_bits(0x42803f48)),
        (f32::from_bits(0x423f8a54), f32::from_bits(0x4287967e)),
    );
    path.line_to((f32::from_bits(0x420a7682), f32::from_bits(0x424407da)));
    path.cubic_to(
        (f32::from_bits(0x42197d0c), f32::from_bits(0x42396aed)),
        (f32::from_bits(0x42273e74), f32::from_bits(0x422d1cc3)),
        (f32::from_bits(0x4233755f), f32::from_bits(0x421f5b68)),
    );
    path.line_to((f32::from_bits(0x42784038), f32::from_bits(0x425c71a4)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5240-L5271 (chrome/m156)
fn battleOp191(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4202b56e), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427940ff), f32::from_bits(0xc27f4e67)),
        (f32::from_bits(0x42976a2d), f32::from_bits(0xc20814ff)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b233da), f32::from_bits(0xc086dcb5)),
        (f32::from_bits(0x42a78518), f32::from_bits(0x41f56a27)),
        (f32::from_bits(0x42784038), f32::from_bits(0x425c71a4)),
    );
    path.cubic_to(
        (f32::from_bits(0x42675aa4), f32::from_bits(0x426f78d5)),
        (f32::from_bits(0x4254535c), f32::from_bits(0x42803f48)),
        (f32::from_bits(0x423f8a54), f32::from_bits(0x4287967e)),
    );
    path.line_to((f32::from_bits(0x420a7682), f32::from_bits(0x424407da)));
    path.cubic_to(
        (f32::from_bits(0x42197d0c), f32::from_bits(0x42396aed)),
        (f32::from_bits(0x42273e74), f32::from_bits(0x422d1cc3)),
        (f32::from_bits(0x4233755f), f32::from_bits(0x421f5b68)),
    );
    path.line_to((f32::from_bits(0x4233755d), f32::from_bits(0x421f5b67)));
    path.cubic_to(
        (f32::from_bits(0x4272328d), f32::from_bits(0x41b16880)),
        (f32::from_bits(0x4280d235), f32::from_bits(0xc042fb32)),
        (f32::from_bits(0x425ae9b3), f32::from_bits(0xc1c4bebc)),
    );
    path.cubic_to(
        (f32::from_bits(0x42342efc), f32::from_bits(0xc2388f09)),
        (f32::from_bits(0x41bcf9fa), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423f8a55), f32::from_bits(0x4287967f)));
    path.cubic_to(
        (f32::from_bits(0x41974ba2), f32::from_bits(0x42b0846d)),
        (f32::from_bits(0xc19ee9a3), f32::from_bits(0x42b01937)),
        (f32::from_bits(0xc2427547), f32::from_bits(0x42868bae)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29abade), f32::from_bits(0x4239fc4c)),
        (f32::from_bits(0xc2b3780d), f32::from_bits(0x411bee16)),
        (f32::from_bits(0xc29ecbab), f32::from_bits(0xc1c17e4f)),
    );
    path.cubic_to(
        (f32::from_bits(0xc28a1f48), f32::from_bits(0xc26879d6)),
        (f32::from_bits(0xc2193674), f32::from_bits(0xc2a376c5)),
        (f32::from_bits(0xc0368c8c), f32::from_bits(0xc2a5e6e5)),
    );
    path.line_to((f32::from_bits(0xc003f6b5), f32::from_bits(0xc26fdbb6)));
    path.cubic_to(
        (f32::from_bits(0xc1dd8323), f32::from_bits(0xc26c555a)),
        (f32::from_bits(0xc247b1d3), f32::from_bits(0xc2280e0b)),
        (f32::from_bits(0xc2659575), f32::from_bits(0xc18bdff2)),
    );
    path.cubic_to(
        (f32::from_bits(0xc281bc8c), f32::from_bits(0x40e170d0)),
        (f32::from_bits(0xc25fb4ae), f32::from_bits(0x42067283)),
        (f32::from_bits(0xc20c926e), f32::from_bits(0x42428613)),
    );
    path.cubic_to(
        (f32::from_bits(0xc165c0b5), f32::from_bits(0x427e99a3)),
        (f32::from_bits(0x415abda1), f32::from_bits(0x427f34a6)),
        (f32::from_bits(0x420a7686), f32::from_bits(0x424407d8)),
    );
    path.line_to((f32::from_bits(0x423f8a55), f32::from_bits(0x4287967f)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5274-L5298 (chrome/m156)
fn battleOp192(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4202fa25), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4279b699), f32::from_bits(0xc27efea4)),
        (f32::from_bits(0x429790ee), f32::from_bits(0xc20767f9)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b24690), f32::from_bits(0xc07d14fa)),
        (f32::from_bits(0x42a75587), f32::from_bits(0x41f80076)),
        (f32::from_bits(0x427743d2), f32::from_bits(0x425d8c9b)),
    );
    path.line_to((f32::from_bits(0x4232bee9), f32::from_bits(0x422027f2)));
    path.cubic_to(
        (f32::from_bits(0x4271edc7), f32::from_bits(0x41b34741)),
        (f32::from_bits(0x4280dfbb), f32::from_bits(0xc036f37a)),
        (f32::from_bits(0x425b21bb), f32::from_bits(0xc1c3c49a)),
    );
    path.cubic_to(
        (f32::from_bits(0x423483ff), f32::from_bits(0xc2385562)),
        (f32::from_bits(0x41bd5d54), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36d3ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x427743d4), f32::from_bits(0x425d8c98)));
    path.cubic_to(
        (f32::from_bits(0x4266401a), f32::from_bits(0x427089e5)),
        (f32::from_bits(0x42531ae2), f32::from_bits(0x4280c0a0)),
        (f32::from_bits(0x423e3514), f32::from_bits(0x42880e64)),
    );
    path.line_to((f32::from_bits(0x42097fd1), f32::from_bits(0x4244b531)));
    path.cubic_to(
        (f32::from_bits(0x42189b26), f32::from_bits(0x423a25ea)),
        (f32::from_bits(0x42267233), f32::from_bits(0x422de224)),
        (f32::from_bits(0x4232beea), f32::from_bits(0x422027f3)),
    );
    path.line_to((f32::from_bits(0x427743d4), f32::from_bits(0x425d8c98)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5300-L5322 (chrome/m156)
fn battleOp193(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e15a675), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3e95a67a), f32::from_bits(0xc2a5ffcd)),
        (f32::from_bits(0x3ee07980), f32::from_bits(0xc2a5ff68)),
    );
    path.line_to((f32::from_bits(0x3ea245bb), f32::from_bits(0xc26fff25)));
    path.cubic_to(
        (f32::from_bits(0x3e585de0), f32::from_bits(0xc26fffb9)),
        (f32::from_bits(0x3dd85f11), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3691e768), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ee07a10), f32::from_bits(0xc2a5ff68)));
    path.cubic_to(
        (f32::from_bits(0x3ee7f565), f32::from_bits(0xc2a5ff5d)),
        (f32::from_bits(0x3eef70d9), f32::from_bits(0xc2a5ff52)),
        (f32::from_bits(0x3ef6ec4d), f32::from_bits(0xc2a5ff47)),
    );
    path.line_to((f32::from_bits(0x3eb27fdb), f32::from_bits(0xc26ffef6)));
    path.cubic_to(
        (f32::from_bits(0x3ead1768), f32::from_bits(0xc26fff07)),
        (f32::from_bits(0x3ea7aebe), f32::from_bits(0xc26fff17)),
        (f32::from_bits(0x3ea24612), f32::from_bits(0xc26fff26)),
    );
    path.line_to((f32::from_bits(0x3ee07a10), f32::from_bits(0xc2a5ff68)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5325-L5349 (chrome/m156)
fn battleOp194(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3691e768), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e15a675), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3e95a67a), f32::from_bits(0xc2a5ffcd)),
        (f32::from_bits(0x3ee07a10), f32::from_bits(0xc2a5ff68)),
    );
    path.line_to((f32::from_bits(0x3ef6ec4d), f32::from_bits(0xc2a5ff47)));
    path.line_to((f32::from_bits(0x3eb27fdb), f32::from_bits(0xc26ffef6)));
    path.cubic_to(
        (f32::from_bits(0x3ead1768), f32::from_bits(0xc26fff07)),
        (f32::from_bits(0x3ea7aebe), f32::from_bits(0xc26fff17)),
        (f32::from_bits(0x3ea245bb), f32::from_bits(0xc26fff25)),
    );
    path.cubic_to(
        (f32::from_bits(0x3e585de0), f32::from_bits(0xc26fffb9)),
        (f32::from_bits(0x3dd85f11), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3691e768), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ef6ec9b), f32::from_bits(0xc2a5ff48)));
    path.cubic_to(
        (f32::from_bits(0x3f3816c9), f32::from_bits(0xc2a5fe94)),
        (f32::from_bits(0x3f74b6e1), f32::from_bits(0xc2a5fd5b)),
        (f32::from_bits(0x3f98ab0b), f32::from_bits(0xc2a5fb9d)),
    );
    path.line_to((f32::from_bits(0x3f5cb973), f32::from_bits(0xc26ff9a8)));
    path.cubic_to(
        (f32::from_bits(0x3f30e6e7), f32::from_bits(0xc26ffc2e)),
        (f32::from_bits(0x3f05138e), f32::from_bits(0xc26ffdf2)),
        (f32::from_bits(0x3eb27fc6), f32::from_bits(0xc26ffef7)),
    );
    path.line_to((f32::from_bits(0x3ef6ec9b), f32::from_bits(0xc2a5ff48)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5352-L5374 (chrome/m156)
fn battleOp195(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f0607d9), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3f860760), f32::from_bits(0xc2a5fd76)),
        (f32::from_bits(0x3fc90825), f32::from_bits(0xc2a5f863)),
    );
    path.line_to((f32::from_bits(0x3f9152f7), f32::from_bits(0xc26ff500)));
    path.cubic_to(
        (f32::from_bits(0x3f41c6b2), f32::from_bits(0xc26ffc55)),
        (f32::from_bits(0x3ec1c794), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x36a51f4a), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3fc9081a), f32::from_bits(0xc2a5f864)));
    path.cubic_to(
        (f32::from_bits(0x3fcfbb75), f32::from_bits(0xc2a5f7e2)),
        (f32::from_bits(0x3fd66eab), f32::from_bits(0xc2a5f75a)),
        (f32::from_bits(0x3fdd21d8), f32::from_bits(0xc2a5f6cb)),
    );
    path.line_to((f32::from_bits(0x3f9fdac0), f32::from_bits(0xc26ff2b1)));
    path.cubic_to(
        (f32::from_bits(0x3f9b02da), f32::from_bits(0xc26ff37f)),
        (f32::from_bits(0x3f962add), f32::from_bits(0xc26ff444)),
        (f32::from_bits(0x3f9152da), f32::from_bits(0xc26ff500)),
    );
    path.line_to((f32::from_bits(0x3fc9081a), f32::from_bits(0xc2a5f864)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5377-L5396 (chrome/m156)
fn battleOp196(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x36a51f4a), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3f0607d1), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3f860758), f32::from_bits(0xc2a5fd76)),
        (f32::from_bits(0x3fc9081a), f32::from_bits(0xc2a5f864)),
    );
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3fdd21ce), f32::from_bits(0xc2a5f6cb)));
    path.cubic_to(
        (f32::from_bits(0x4024daa1), f32::from_bits(0xc2a5edc0)),
        (f32::from_bits(0x405b1f05), f32::from_bits(0xc2a5de0d)),
        (f32::from_bits(0x4088aca3), f32::from_bits(0xc2a5c7b3)),
    );
    path.line_to((f32::from_bits(0x40459a01), f32::from_bits(0xc26fae99)));
    path.cubic_to(
        (f32::from_bits(0x401e66a3), f32::from_bits(0xc26fceed)),
        (f32::from_bits(0x3fee57cd), f32::from_bits(0xc26fe5a0)),
        (f32::from_bits(0x3f9fdaba), f32::from_bits(0xc26ff2b3)),
    );
    path.line_to((f32::from_bits(0x3fdd21ce), f32::from_bits(0xc2a5f6cb)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5399-L5421 (chrome/m156)
fn battleOp197(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3fa0bd52), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4020babd), f32::from_bits(0xc2a5f168)),
        (f32::from_bits(0x40710446), f32::from_bits(0xc2a5d43c)),
    );
    path.line_to((f32::from_bits(0x402e3a94), f32::from_bits(0xc26fc0ba)));
    path.cubic_to(
        (f32::from_bits(0x3fe86158), f32::from_bits(0xc26feae9)),
        (f32::from_bits(0x3f686554), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x369bbf59), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4071043c), f32::from_bits(0xc2a5d43c)));
    path.cubic_to(
        (f32::from_bits(0x40790b78), f32::from_bits(0xc2a5d151)),
        (f32::from_bits(0x40808943), f32::from_bits(0xc2a5ce41)),
        (f32::from_bits(0x40848cac), f32::from_bits(0xc2a5cb0c)),
    );
    path.line_to((f32::from_bits(0x403fa34c), f32::from_bits(0xc26fb371)));
    path.cubic_to(
        (f32::from_bits(0x4039d5dd), f32::from_bits(0xc26fb815)),
        (f32::from_bits(0x40340849), f32::from_bits(0xc26fbc83)),
        (f32::from_bits(0x402e3a8d), f32::from_bits(0xc26fc0bb)),
    );
    path.line_to((f32::from_bits(0x4071043c), f32::from_bits(0xc2a5d43c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5424-L5450 (chrome/m156)
fn battleOp198(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x369bbf59), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3fa0bd4b), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4020bab6), f32::from_bits(0xc2a5f168)),
        (f32::from_bits(0x4071043c), f32::from_bits(0xc2a5d43c)),
    );
    path.line_to((f32::from_bits(0x40710446), f32::from_bits(0xc2a5d43c)));
    path.cubic_to(
        (f32::from_bits(0x40790b7f), f32::from_bits(0xc2a5d151)),
        (f32::from_bits(0x40808945), f32::from_bits(0xc2a5ce41)),
        (f32::from_bits(0x40848cac), f32::from_bits(0xc2a5cb0c)),
    );
    path.line_to((f32::from_bits(0x403fa34c), f32::from_bits(0xc26fb371)));
    path.quad_to(
        (f32::from_bits(0x4036ef2a), f32::from_bits(0xc26fba67)),
        (f32::from_bits(0x402e3a95), f32::from_bits(0xc26fc0bb)),
    );
    path.line_to((f32::from_bits(0x402e3a94), f32::from_bits(0xc26fc0ba)));
    path.cubic_to(
        (f32::from_bits(0x3fe86158), f32::from_bits(0xc26feae9)),
        (f32::from_bits(0x3f686554), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x369bbf59), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x40848cae), f32::from_bits(0xc2a5cb0c)));
    path.cubic_to(
        (f32::from_bits(0x40c597bc), f32::from_bits(0xc2a5970c)),
        (f32::from_bits(0x41033f43), f32::from_bits(0xc2a53cca)),
        (f32::from_bits(0x41238fb3), f32::from_bits(0xc2a4bc74)),
    );
    path.line_to((f32::from_bits(0x40ec7963), f32::from_bits(0xc26e2c38)));
    path.cubic_to(
        (f32::from_bits(0x40bdc13f), f32::from_bits(0xc26ee5c4)),
        (f32::from_bits(0x408ed689), f32::from_bits(0xc26f6843)),
        (f32::from_bits(0x403fa341), f32::from_bits(0xc26fb372)),
    );
    path.line_to((f32::from_bits(0x40848cae), f32::from_bits(0xc2a5cb0c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5453-L5475 (chrome/m156)
fn battleOp199(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3ffdfad4), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x407df074), f32::from_bits(0xc2a5db92)),
        (f32::from_bits(0x40be4d32), f32::from_bits(0xc2a592c7)),
    );
    path.line_to((f32::from_bits(0x40899143), f32::from_bits(0xc26f6217)));
    path.cubic_to(
        (f32::from_bits(0x40379219), f32::from_bits(0xc26fcb54)),
        (f32::from_bits(0x3fb799b8), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3673fea3), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x40be4d37), f32::from_bits(0xc2a592c7)));
    path.cubic_to(
        (f32::from_bits(0x40c4a257), f32::from_bits(0xc2a58b80)),
        (f32::from_bits(0x40caf70c), f32::from_bits(0xc2a583db)),
        (f32::from_bits(0x40d14b4e), f32::from_bits(0xc2a57bda)),
    );
    path.line_to((f32::from_bits(0x40974c04), f32::from_bits(0xc26f40f2)));
    path.cubic_to(
        (f32::from_bits(0x4092b8c1), f32::from_bits(0xc26f4c86)),
        (f32::from_bits(0x408e252c), f32::from_bits(0xc26f5792)),
        (f32::from_bits(0x4089914a), f32::from_bits(0xc26f6219)),
    );
    path.line_to((f32::from_bits(0x40be4d37), f32::from_bits(0xc2a592c7)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5478-L5503 (chrome/m156)
fn battleOp200(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3673fea3), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3ffdfad4), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x407df074), f32::from_bits(0xc2a5db92)),
        (f32::from_bits(0x40be4d37), f32::from_bits(0xc2a592c7)),
    );
    path.cubic_to(
        (f32::from_bits(0x40c4a257), f32::from_bits(0xc2a58b80)),
        (f32::from_bits(0x40caf70c), f32::from_bits(0xc2a583db)),
        (f32::from_bits(0x40d14b4e), f32::from_bits(0xc2a57bda)),
    );
    path.line_to((f32::from_bits(0x40974c04), f32::from_bits(0xc26f40f2)));
    path.cubic_to(
        (f32::from_bits(0x4092b8c1), f32::from_bits(0xc26f4c86)),
        (f32::from_bits(0x408e252c), f32::from_bits(0xc26f5792)),
        (f32::from_bits(0x4089914a), f32::from_bits(0xc26f6219)),
    );
    path.line_to((f32::from_bits(0x40899143), f32::from_bits(0xc26f6217)));
    path.cubic_to(
        (f32::from_bits(0x40379219), f32::from_bits(0xc26fcb54)),
        (f32::from_bits(0x3fb799b8), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3673fea3), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x40d14b4a), f32::from_bits(0xc2a57bdb)));
    path.cubic_to(
        (f32::from_bits(0x411bf161), f32::from_bits(0xc2a4fa1a)),
        (f32::from_bits(0x414ef5ad), f32::from_bits(0xc2a4190e)),
        (f32::from_bits(0x4180b83e), f32::from_bits(0xc2a2d9dc)),
    );
    path.line_to((f32::from_bits(0x413a19cf), f32::from_bits(0xc26b727f)));
    path.cubic_to(
        (f32::from_bits(0x41159c04), f32::from_bits(0xc26d3fff)),
        (f32::from_bits(0x40e175a8), f32::from_bits(0xc26e855c)),
        (f32::from_bits(0x40974c02), f32::from_bits(0xc26f40f4)),
    );
    path.line_to((f32::from_bits(0x40d14b4a), f32::from_bits(0xc2a57bdb)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5506-L5528 (chrome/m156)
fn battleOp201(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4059d383), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x40d9b918), f32::from_bits(0xc2a594d0)),
        (f32::from_bits(0x4122e820), f32::from_bits(0xc2a4bf0c)),
    );
    path.line_to((f32::from_bits(0x40eb871c), f32::from_bits(0xc26e2ff8)));
    path.cubic_to(
        (f32::from_bits(0x409d63e0), f32::from_bits(0xc26f6508)),
        (f32::from_bits(0x401d76fa), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x35f7fd4a), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4122e81e), f32::from_bits(0xc2a4bf0c)));
    path.cubic_to(
        (f32::from_bits(0x41284f3c), f32::from_bits(0xc2a4a9ac)),
        (f32::from_bits(0x412db549), f32::from_bits(0xc2a4933e)),
        (f32::from_bits(0x41331a33), f32::from_bits(0xc2a47bbf)),
    );
    path.line_to((f32::from_bits(0x410178be), f32::from_bits(0xc26dceac)));
    path.cubic_to(
        (f32::from_bits(0x40fb24f7), f32::from_bits(0xc26df0a4)),
        (f32::from_bits(0x40f356d1), f32::from_bits(0xc26e1114)),
        (f32::from_bits(0x40eb871f), f32::from_bits(0xc26e2ff8)),
    );
    path.line_to((f32::from_bits(0x4122e81e), f32::from_bits(0xc2a4bf0c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5531-L5557 (chrome/m156)
fn battleOp202(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4059d380), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x40d9b915), f32::from_bits(0xc2a594d0)),
        (f32::from_bits(0x4122e81e), f32::from_bits(0xc2a4bf0c)),
    );
    path.line_to((f32::from_bits(0x4122e820), f32::from_bits(0xc2a4bf0c)));
    path.cubic_to(
        (f32::from_bits(0x41284f3d), f32::from_bits(0xc2a4a9ac)),
        (f32::from_bits(0x412db54a), f32::from_bits(0xc2a4933e)),
        (f32::from_bits(0x41331a33), f32::from_bits(0xc2a47bbf)),
    );
    path.line_to((f32::from_bits(0x410178be), f32::from_bits(0xc26dceac)));
    path.cubic_to(
        (f32::from_bits(0x40fb24f7), f32::from_bits(0xc26df0a4)),
        (f32::from_bits(0x40f356d1), f32::from_bits(0xc26e1114)),
        (f32::from_bits(0x40eb871f), f32::from_bits(0xc26e2ff8)),
    );
    path.line_to((f32::from_bits(0x40eb871c), f32::from_bits(0xc26e2ff8)));
    path.cubic_to(
        (f32::from_bits(0x409d63e0), f32::from_bits(0xc26f6508)),
        (f32::from_bits(0x401d76fa), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41331a39), f32::from_bits(0xc2a47bc0)));
    path.cubic_to(
        (f32::from_bits(0x41854b40), f32::from_bits(0xc2a2feb5)),
        (f32::from_bits(0x41b05576), f32::from_bits(0xc2a06b6c)),
        (f32::from_bits(0x41da0834), f32::from_bits(0xc29ccbb1)),
    );
    path.line_to((f32::from_bits(0x419d9d10), f32::from_bits(0xc262b148)));
    path.cubic_to(
        (f32::from_bits(0x417ef0c0), f32::from_bits(0xc267ee96)),
        (f32::from_bits(0x4140b6cf), f32::from_bits(0xc26ba7c4)),
        (f32::from_bits(0x410178c0), f32::from_bits(0xc26dcead)),
    );
    path.line_to((f32::from_bits(0x41331a39), f32::from_bits(0xc2a47bc0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5560-L5582 (chrome/m156)
fn battleOp203(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4087af55), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x410795c5), f32::from_bits(0xc2a559a4)),
        (f32::from_bits(0x414aa20a), f32::from_bits(0xc2a40e63)),
    );
    path.line_to((f32::from_bits(0x41127b4b), f32::from_bits(0xc26d308f)));
    path.cubic_to(
        (f32::from_bits(0x40c406cd), f32::from_bits(0xc26f0f7b)),
        (f32::from_bits(0x40442bc2), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x36b5ff52), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x414aa206), f32::from_bits(0xc2a40e63)));
    path.cubic_to(
        (f32::from_bits(0x4151559c), f32::from_bits(0xc2a3ed46)),
        (f32::from_bits(0x41580726), f32::from_bits(0xc2a3ca86)),
        (f32::from_bits(0x415eb67b), f32::from_bits(0xc2a3a622)),
    );
    path.line_to((f32::from_bits(0x4120ff4d), f32::from_bits(0xc26c99d6)));
    path.cubic_to(
        (f32::from_bits(0x411c2a2f), f32::from_bits(0xc26cce74)),
        (f32::from_bits(0x41175378), f32::from_bits(0xc26d00b1)),
        (f32::from_bits(0x41127b46), f32::from_bits(0xc26d308f)),
    );
    path.line_to((f32::from_bits(0x414aa206), f32::from_bits(0xc2a40e63)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5585-L5611 (chrome/m156)
fn battleOp204(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x36b5ff52), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4087af52), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x410795c2), f32::from_bits(0xc2a559a4)),
        (f32::from_bits(0x414aa206), f32::from_bits(0xc2a40e63)),
    );
    path.line_to((f32::from_bits(0x414aa20a), f32::from_bits(0xc2a40e63)));
    path.cubic_to(
        (f32::from_bits(0x4151559f), f32::from_bits(0xc2a3ed46)),
        (f32::from_bits(0x41580727), f32::from_bits(0xc2a3ca86)),
        (f32::from_bits(0x415eb67b), f32::from_bits(0xc2a3a622)),
    );
    path.line_to((f32::from_bits(0x4120ff4d), f32::from_bits(0xc26c99d6)));
    path.cubic_to(
        (f32::from_bits(0x411c2a31), f32::from_bits(0xc26cce74)),
        (f32::from_bits(0x4117537b), f32::from_bits(0xc26d00b1)),
        (f32::from_bits(0x41127b4b), f32::from_bits(0xc26d308f)),
    );
    path.line_to((f32::from_bits(0x41127b46), f32::from_bits(0xc26d308f)));
    path.cubic_to(
        (f32::from_bits(0x40c406c6), f32::from_bits(0xc26f0f7b)),
        (f32::from_bits(0x40442bbb), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x36b5ff52), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x415eb680), f32::from_bits(0xc2a3a623)));
    path.cubic_to(
        (f32::from_bits(0x41a59721), f32::from_bits(0xc2a157ad)),
        (f32::from_bits(0x41da77ab), f32::from_bits(0xc29d5c25)),
        (f32::from_bits(0x420662d7), f32::from_bits(0xc297cafd)),
    );
    path.line_to((f32::from_bits(0x41c24b0d), f32::from_bits(0xc25b75ac)));
    path.cubic_to(
        (f32::from_bits(0x419deda5), f32::from_bits(0xc2638226)),
        (f32::from_bits(0x416f6860), f32::from_bits(0xc269442a)),
        (f32::from_bits(0x4120ff4a), f32::from_bits(0xc26c99d9)),
    );
    path.line_to((f32::from_bits(0x415eb680), f32::from_bits(0xc2a3a623)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5614-L5636 (chrome/m156)
fn battleOp205(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40a2e582), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4122b94f), f32::from_bits(0xc2a51039)),
        (f32::from_bits(0x4172cca0), f32::from_bits(0xc2a333b4)),
    );
    path.line_to((f32::from_bits(0x412f847d), f32::from_bits(0xc26bf464)));
    path.cubic_to(
        (f32::from_bits(0x40eb4376), f32::from_bits(0xc26ea556)),
        (f32::from_bits(0x406b836d), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4172cc9b), f32::from_bits(0xc2a333b4)));
    path.cubic_to(
        (f32::from_bits(0x417acd1a), f32::from_bits(0xc2a30415)),
        (f32::from_bits(0x41816508), f32::from_bits(0xc2a2d21d)),
        (f32::from_bits(0x4185619b), f32::from_bits(0xc2a29dcb)),
    );
    path.line_to((f32::from_bits(0x4140d724), f32::from_bits(0xc26b1ba8)));
    path.cubic_to(
        (f32::from_bits(0x413b139d), f32::from_bits(0xc26b674c)),
        (f32::from_bits(0x41354d54), f32::from_bits(0xc26baf8b)),
        (f32::from_bits(0x412f847c), f32::from_bits(0xc26bf463)),
    );
    path.line_to((f32::from_bits(0x4172cc9b), f32::from_bits(0xc2a333b4)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5639-L5665 (chrome/m156)
fn battleOp206(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40a2e57f), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4122b94c), f32::from_bits(0xc2a51039)),
        (f32::from_bits(0x4172cc9b), f32::from_bits(0xc2a333b4)),
    );
    path.line_to((f32::from_bits(0x4172cca0), f32::from_bits(0xc2a333b4)));
    path.cubic_to(
        (f32::from_bits(0x417acd1d), f32::from_bits(0xc2a30415)),
        (f32::from_bits(0x41816509), f32::from_bits(0xc2a2d21d)),
        (f32::from_bits(0x4185619b), f32::from_bits(0xc2a29dcb)),
    );
    path.line_to((f32::from_bits(0x4140d724), f32::from_bits(0xc26b1ba8)));
    path.cubic_to(
        (f32::from_bits(0x413b139d), f32::from_bits(0xc26b674c)),
        (f32::from_bits(0x41354d54), f32::from_bits(0xc26baf8b)),
        (f32::from_bits(0x412f847c), f32::from_bits(0xc26bf463)),
    );
    path.line_to((f32::from_bits(0x412f847d), f32::from_bits(0xc26bf464)));
    path.cubic_to(
        (f32::from_bits(0x40eb4376), f32::from_bits(0xc26ea556)),
        (f32::from_bits(0x406b836d), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36b5ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4185619b), f32::from_bits(0xc2a29dcc)));
    path.cubic_to(
        (f32::from_bits(0x41c61a92), f32::from_bits(0xc29f4c69)),
        (f32::from_bits(0x42023dd6), f32::from_bits(0xc299958f)),
        (f32::from_bits(0x421f3a98), f32::from_bits(0xc291a994)),
    );
    path.line_to((f32::from_bits(0x41e635e1), f32::from_bits(0xc25298a5)));
    path.cubic_to(
        (f32::from_bits(0x41bc4d11), f32::from_bits(0xc25e0caa)),
        (f32::from_bits(0x418f3524), f32::from_bits(0xc2664fa2)),
        (f32::from_bits(0x4140d729), f32::from_bits(0xc26b1ba9)),
    );
    path.line_to((f32::from_bits(0x4185619b), f32::from_bits(0xc2a29dcc)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5668-L5690 (chrome/m156)
fn battleOp207(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40c39389), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x414346f4), f32::from_bits(0xc2a4a65f)),
        (f32::from_bits(0x419158cf), f32::from_bits(0xc2a1f965)),
    );
    path.line_to((f32::from_bits(0x415223e0), f32::from_bits(0xc26a2df8)));
    path.cubic_to(
        (f32::from_bits(0x410d2a0c), f32::from_bits(0xc26e0c4b)),
        (f32::from_bits(0x408d616c), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x35bbfd46), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x419158d0), f32::from_bits(0xc2a1f965)));
    path.cubic_to(
        (f32::from_bits(0x41961cea), f32::from_bits(0xc2a1b4f6)),
        (f32::from_bits(0x419addf6), f32::from_bits(0xc2a16d2c)),
        (f32::from_bits(0x419f9bbb), f32::from_bits(0xc2a12207)),
    );
    path.line_to((f32::from_bits(0x4166c251), f32::from_bits(0xc268f69a)));
    path.cubic_to(
        (f32::from_bits(0x415fe778), f32::from_bits(0xc269633e)),
        (f32::from_bits(0x415907e2), f32::from_bits(0xc269cb09)),
        (f32::from_bits(0x415223e0), f32::from_bits(0xc26a2df8)),
    );
    path.line_to((f32::from_bits(0x419158d0), f32::from_bits(0xc2a1f965)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5693-L5717 (chrome/m156)
fn battleOp208(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40c39389), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x414346f4), f32::from_bits(0xc2a4a65f)),
        (f32::from_bits(0x419158d0), f32::from_bits(0xc2a1f965)),
    );
    path.cubic_to(
        (f32::from_bits(0x41961cea), f32::from_bits(0xc2a1b4f6)),
        (f32::from_bits(0x419addf6), f32::from_bits(0xc2a16d2c)),
        (f32::from_bits(0x419f9bbb), f32::from_bits(0xc2a12207)),
    );
    path.line_to((f32::from_bits(0x4166c251), f32::from_bits(0xc268f69a)));
    path.cubic_to(
        (f32::from_bits(0x415fe778), f32::from_bits(0xc269633e)),
        (f32::from_bits(0x415907e2), f32::from_bits(0xc269cb09)),
        (f32::from_bits(0x415223e0), f32::from_bits(0xc26a2df8)),
    );
    path.cubic_to(
        (f32::from_bits(0x410d2a0c), f32::from_bits(0xc26e0c4b)),
        (f32::from_bits(0x408d616c), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x419f9bbc), f32::from_bits(0xc2a12208)));
    path.cubic_to(
        (f32::from_bits(0x41eca53e), f32::from_bits(0xc29c5d1a)),
        (f32::from_bits(0x421ad1be), f32::from_bits(0xc2942e2b)),
        (f32::from_bits(0x423b8fe1), f32::from_bits(0xc288f8a3)),
    );
    path.line_to((f32::from_bits(0x42079647), f32::from_bits(0xc24607dc)));
    path.cubic_to(
        (f32::from_bits(0x41dfd5cc), f32::from_bits(0xc2563c94)),
        (f32::from_bits(0x41ab11aa), f32::from_bits(0xc2621167)),
        (f32::from_bits(0x4166c24e), f32::from_bits(0xc268f69b)),
    );
    path.line_to((f32::from_bits(0x419f9bbc), f32::from_bits(0xc2a12208)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5720-L5742 (chrome/m156)
fn battleOp209(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40e86425), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4167e385), f32::from_bits(0xc2a41801)),
        (f32::from_bits(0x41ac0ecd), f32::from_bits(0xc2a05484)),
    );
    path.line_to((f32::from_bits(0x4178c21d), f32::from_bits(0xc267cd79)));
    path.cubic_to(
        (f32::from_bits(0x4127a168), f32::from_bits(0xc26d3e79)),
        (f32::from_bits(0x40a7fe68), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3673fea3), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41ac0ecb), f32::from_bits(0xc2a05485)));
    path.cubic_to(
        (f32::from_bits(0x41b1a941), f32::from_bits(0xc29ff44e)),
        (f32::from_bits(0x41b73ea0), f32::from_bits(0xc29f8f65)),
        (f32::from_bits(0x41bcce84), f32::from_bits(0xc29f25d1)),
    );
    path.line_to((f32::from_bits(0x41887c9d), f32::from_bits(0xc26617d6)));
    path.cubic_to(
        (f32::from_bits(0x4184774a), f32::from_bits(0xc266b07c)),
        (f32::from_bits(0x41806e06), f32::from_bits(0xc2674260)),
        (f32::from_bits(0x4178c21e), f32::from_bits(0xc267cd7a)),
    );
    path.line_to((f32::from_bits(0x41ac0ecb), f32::from_bits(0xc2a05485)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5745-L5772 (chrome/m156)
fn battleOp210(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3673fea3), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x40e86421), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4167e381), f32::from_bits(0xc2a41801)),
        (f32::from_bits(0x41ac0eca), f32::from_bits(0xc2a05484)),
    );
    path.line_to((f32::from_bits(0x41ac0ecd), f32::from_bits(0xc2a05484)));
    path.line_to((f32::from_bits(0x4178c21e), f32::from_bits(0xc267cd7a)));
    path.line_to((f32::from_bits(0x41ac0ecb), f32::from_bits(0xc2a05485)));
    path.cubic_to(
        (f32::from_bits(0x41b1a941), f32::from_bits(0xc29ff44e)),
        (f32::from_bits(0x41b73ea0), f32::from_bits(0xc29f8f65)),
        (f32::from_bits(0x41bcce84), f32::from_bits(0xc29f25d1)),
    );
    path.line_to((f32::from_bits(0x41887c9d), f32::from_bits(0xc26617d6)));
    path.cubic_to(
        (f32::from_bits(0x4184774a), f32::from_bits(0xc266b07c)),
        (f32::from_bits(0x41806e06), f32::from_bits(0xc2674260)),
        (f32::from_bits(0x4178c21d), f32::from_bits(0xc267cd79)),
    );
    path.cubic_to(
        (f32::from_bits(0x4127a168), f32::from_bits(0xc26d3e79)),
        (f32::from_bits(0x40a7fe68), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3673fea3), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41bcce83), f32::from_bits(0xc29f25d2)));
    path.cubic_to(
        (f32::from_bits(0x420ba3b4), f32::from_bits(0xc2987080)),
        (f32::from_bits(0x42357f09), f32::from_bits(0xc28cfcb1)),
        (f32::from_bits(0x42592f07), f32::from_bits(0xc27b1ba7)),
    );
    path.line_to((f32::from_bits(0x421d0012), f32::from_bits(0xc235861c)));
    path.cubic_to(
        (f32::from_bits(0x420333bc), f32::from_bits(0xc24bd636)),
        (f32::from_bits(0x41c9e36e), f32::from_bits(0xc25c64f6)),
        (f32::from_bits(0x41887c9c), f32::from_bits(0xc26617d7)),
    );
    path.line_to((f32::from_bits(0x41bcce83), f32::from_bits(0xc29f25d2)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5775-L5797 (chrome/m156)
fn battleOp211(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x411e5541), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x419db1ee), f32::from_bits(0xc2a275ef)),
        (f32::from_bits(0x41e7e0a3), f32::from_bits(0xc29b8c98)),
    );
    path.line_to((f32::from_bits(0x41a79f51), f32::from_bits(0xc260e3f1)));
    path.cubic_to(
        (f32::from_bits(0x4163fe32), f32::from_bits(0xc26ae208)),
        (f32::from_bits(0x40e4ea54), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea3), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41e7e0a8), f32::from_bits(0xc29b8c98)));
    path.cubic_to(
        (f32::from_bits(0x41ef46bb), f32::from_bits(0xc29adc20)),
        (f32::from_bits(0x41f6a013), f32::from_bits(0xc29a2338)),
        (f32::from_bits(0x41fdebc8), f32::from_bits(0xc29961f8)),
    );
    path.line_to((f32::from_bits(0x41b78eb0), f32::from_bits(0xc25dc215)));
    path.cubic_to(
        (f32::from_bits(0x41b2488a), f32::from_bits(0xc25ed97a)),
        (f32::from_bits(0x41acf889), f32::from_bits(0xc25fe4cd)),
        (f32::from_bits(0x41a79f51), f32::from_bits(0xc260e3f1)),
    );
    path.line_to((f32::from_bits(0x41e7e0a8), f32::from_bits(0xc29b8c98)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5800-L5824 (chrome/m156)
fn battleOp212(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea3), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x411e5541), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x419db1ee), f32::from_bits(0xc2a275ef)),
        (f32::from_bits(0x41e7e0a8), f32::from_bits(0xc29b8c98)),
    );
    path.cubic_to(
        (f32::from_bits(0x41ef46bb), f32::from_bits(0xc29adc20)),
        (f32::from_bits(0x41f6a013), f32::from_bits(0xc29a2338)),
        (f32::from_bits(0x41fdebc8), f32::from_bits(0xc29961f8)),
    );
    path.line_to((f32::from_bits(0x41b78eb0), f32::from_bits(0xc25dc215)));
    path.cubic_to(
        (f32::from_bits(0x41b2488a), f32::from_bits(0xc25ed97a)),
        (f32::from_bits(0x41acf889), f32::from_bits(0xc25fe4cd)),
        (f32::from_bits(0x41a79f51), f32::from_bits(0xc260e3f1)),
    );
    path.cubic_to(
        (f32::from_bits(0x4163fe32), f32::from_bits(0xc26ae208)),
        (f32::from_bits(0x40e4ea54), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea3), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41fdebc9), f32::from_bits(0xc29961f9)));
    path.cubic_to(
        (f32::from_bits(0x423a7ccd), f32::from_bits(0xc28d1085)),
        (f32::from_bits(0x426d8f8d), f32::from_bits(0xc270b4b0)),
        (f32::from_bits(0x4288fa0c), f32::from_bits(0xc23b8bbf)),
    );
    path.line_to((f32::from_bits(0x424609e8), f32::from_bits(0xc207934a)));
    path.cubic_to(
        (f32::from_bits(0x422bbb0d), f32::from_bits(0xc22e0114)),
        (f32::from_bits(0x4206cf6b), f32::from_bits(0xc24bf2e1)),
        (f32::from_bits(0x41b78eaf), f32::from_bits(0xc25dc216)),
    );
    path.line_to((f32::from_bits(0x41fdebc9), f32::from_bits(0xc29961f9)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5827-L5849 (chrome/m156)
fn battleOp213(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4151cd59), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x41d04f3f), f32::from_bits(0xc29fc954)),
        (f32::from_bits(0x4216e058), f32::from_bits(0xc293de54)),
    );
    path.line_to((f32::from_bits(0x41da226b), f32::from_bits(0xc255c926)));
    path.cubic_to(
        (f32::from_bits(0x419695d1), f32::from_bits(0xc267043d)),
        (f32::from_bits(0x4117aa0a), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4216e057), f32::from_bits(0xc293de54)));
    path.cubic_to(
        (f32::from_bits(0x421b86ea), f32::from_bits(0xc292aea0)),
        (f32::from_bits(0x42201eff), f32::from_bits(0xc29170ed)),
        (f32::from_bits(0x4224a79b), f32::from_bits(0xc290257e)),
    );
    path.line_to((f32::from_bits(0x41ee0e15), f32::from_bits(0xc2506790)));
    path.cubic_to(
        (f32::from_bits(0x41e78019), f32::from_bits(0xc25246bf)),
        (f32::from_bits(0x41e0dbbc), f32::from_bits(0xc2541212)),
        (f32::from_bits(0x41da226b), f32::from_bits(0xc255c927)),
    );
    path.line_to((f32::from_bits(0x4216e057), f32::from_bits(0xc293de54)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5852-L5878 (chrome/m156)
fn battleOp214(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4151cd58), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x41d04f3d), f32::from_bits(0xc29fc954)),
        (f32::from_bits(0x4216e057), f32::from_bits(0xc293de54)),
    );
    path.line_to((f32::from_bits(0x4216e058), f32::from_bits(0xc293de54)));
    path.cubic_to(
        (f32::from_bits(0x421b86eb), f32::from_bits(0xc292aea0)),
        (f32::from_bits(0x42201eff), f32::from_bits(0xc29170ed)),
        (f32::from_bits(0x4224a79b), f32::from_bits(0xc290257e)),
    );
    path.line_to((f32::from_bits(0x41ee0e15), f32::from_bits(0xc2506790)));
    path.cubic_to(
        (f32::from_bits(0x41e78019), f32::from_bits(0xc25246bf)),
        (f32::from_bits(0x41e0dbbc), f32::from_bits(0xc2541212)),
        (f32::from_bits(0x41da226b), f32::from_bits(0xc255c927)),
    );
    path.line_to((f32::from_bits(0x41da226b), f32::from_bits(0xc255c926)));
    path.cubic_to(
        (f32::from_bits(0x419695d1), f32::from_bits(0xc267043d)),
        (f32::from_bits(0x4117aa0a), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4224a79b), f32::from_bits(0xc290257f)));
    path.cubic_to(
        (f32::from_bits(0x426f06c3), f32::from_bits(0xc275d105)),
        (f32::from_bits(0x42930d85), f32::from_bits(0xc2303df6)),
        (f32::from_bits(0x429f3103), f32::from_bits(0xc1bc373f)),
    );
    path.line_to((f32::from_bits(0x42662806), f32::from_bits(0xc1880f44)));
    path.cubic_to(
        (f32::from_bits(0x42549b44), f32::from_bits(0xc1fececc)),
        (f32::from_bits(0x422cca4c), f32::from_bits(0xc231b2de)),
        (f32::from_bits(0x41ee0e18), f32::from_bits(0xc2506792)),
    );
    path.line_to((f32::from_bits(0x4224a79b), f32::from_bits(0xc290257f)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5881-L5903 (chrome/m156)
fn battleOp215(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41741cf0), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x41f1c060), f32::from_bits(0xc29d96da)),
        (f32::from_bits(0x422cf7a2), f32::from_bits(0xc28db11c)),
    );
    path.line_to((f32::from_bits(0x41fa12be), f32::from_bits(0xc24cdb0d)));
    path.cubic_to(
        (f32::from_bits(0x41aec295), f32::from_bits(0xc263d704)),
        (f32::from_bits(0x413077a0), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x422cf7a1), f32::from_bits(0xc28db11c)));
    path.cubic_to(
        (f32::from_bits(0x423224e7), f32::from_bits(0xc28c1ca8)),
        (f32::from_bits(0x42373bc3), f32::from_bits(0xc28a7620)),
        (f32::from_bits(0x423c3abd), f32::from_bits(0xc288bdfd)),
    );
    path.line_to((f32::from_bits(0x420811ca), f32::from_bits(0xc245b313)));
    path.cubic_to(
        (f32::from_bits(0x4204753a), f32::from_bits(0xc2482f6b)),
        (f32::from_bits(0x4200c767), f32::from_bits(0xc24a924f)),
        (f32::from_bits(0x41fa12c1), f32::from_bits(0xc24cdb0e)),
    );
    path.line_to((f32::from_bits(0x422cf7a1), f32::from_bits(0xc28db11c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5906-L5932 (chrome/m156)
fn battleOp216(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41741cef), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x41f1c05e), f32::from_bits(0xc29d96da)),
        (f32::from_bits(0x422cf7a1), f32::from_bits(0xc28db11c)),
    );
    path.line_to((f32::from_bits(0x422cf7a2), f32::from_bits(0xc28db11c)));
    path.cubic_to(
        (f32::from_bits(0x423224e8), f32::from_bits(0xc28c1ca8)),
        (f32::from_bits(0x42373bc3), f32::from_bits(0xc28a7620)),
        (f32::from_bits(0x423c3abd), f32::from_bits(0xc288bdfd)),
    );
    path.line_to((f32::from_bits(0x420811ca), f32::from_bits(0xc245b313)));
    path.cubic_to(
        (f32::from_bits(0x4204753a), f32::from_bits(0xc2482f6b)),
        (f32::from_bits(0x4200c767), f32::from_bits(0xc24a924f)),
        (f32::from_bits(0x41fa12c1), f32::from_bits(0xc24cdb0e)),
    );
    path.line_to((f32::from_bits(0x41fa12be), f32::from_bits(0xc24cdb0d)));
    path.cubic_to(
        (f32::from_bits(0x41aec295), f32::from_bits(0xc263d704)),
        (f32::from_bits(0x413077a0), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423c3abe), f32::from_bits(0xc288bdfe)));
    path.cubic_to(
        (f32::from_bits(0x42874551), f32::from_bits(0xc258d4f5)),
        (f32::from_bits(0x42a17ace), f32::from_bits(0xc1fc3ce7)),
        (f32::from_bits(0x42a57844), f32::from_bits(0xc0d41d22)),
    );
    path.line_to((f32::from_bits(0x426f3bc1), f32::from_bits(0xc09955d3)));
    path.cubic_to(
        (f32::from_bits(0x426976f3), f32::from_bits(0xc1b65735)),
        (f32::from_bits(0x4243927c), f32::from_bits(0xc21cbef5)),
        (f32::from_bits(0x420811ca), f32::from_bits(0xc245b314)),
    );
    path.line_to((f32::from_bits(0x423c3abe), f32::from_bits(0xc288bdfe)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5935-L5957 (chrome/m156)
fn battleOp217(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4188e880), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x42073c1a), f32::from_bits(0xc29b6b86)),
        (f32::from_bits(0x423f3295), f32::from_bits(0xc287b573)),
    );
    path.line_to((f32::from_bits(0x420a3712), f32::from_bits(0xc2443499)));
    path.cubic_to(
        (f32::from_bits(0x41c3852b), f32::from_bits(0xc260b421)),
        (f32::from_bits(0x4145f08c), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423f3294), f32::from_bits(0xc287b572)));
    path.cubic_to(
        (f32::from_bits(0x4244c015), f32::from_bits(0xc285c0c3)),
        (f32::from_bits(0x424a2e84), f32::from_bits(0xc283b664)),
        (f32::from_bits(0x424f7bec), f32::from_bits(0xc281970f)),
    );
    path.line_to((f32::from_bits(0x4215fd0e), f32::from_bits(0xc23b5bf1)));
    path.cubic_to(
        (f32::from_bits(0x421227cb), f32::from_bits(0xc23e6d7a)),
        (f32::from_bits(0x420e3aa9), f32::from_bits(0xc24160b8)),
        (f32::from_bits(0x420a3713), f32::from_bits(0xc2443498)),
    );
    path.line_to((f32::from_bits(0x423f3294), f32::from_bits(0xc287b572)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5960-L5985 (chrome/m156)
fn battleOp218(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4188e880), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x42073c1a), f32::from_bits(0xc29b6b86)),
        (f32::from_bits(0x423f3295), f32::from_bits(0xc287b573)),
    );
    path.line_to((f32::from_bits(0x424f7bec), f32::from_bits(0xc281970f)));
    path.line_to((f32::from_bits(0x4215fd0e), f32::from_bits(0xc23b5bf1)));
    path.cubic_to(
        (f32::from_bits(0x421227cb), f32::from_bits(0xc23e6d7a)),
        (f32::from_bits(0x420e3aa9), f32::from_bits(0xc24160b8)),
        (f32::from_bits(0x420a3713), f32::from_bits(0xc2443498)),
    );
    path.line_to((f32::from_bits(0x420a3712), f32::from_bits(0xc2443499)));
    path.cubic_to(
        (f32::from_bits(0x41c3852b), f32::from_bits(0xc260b421)),
        (f32::from_bits(0x4145f08c), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x424f7bed), f32::from_bits(0xc281970f)));
    path.cubic_to(
        (f32::from_bits(0x42939bdb), f32::from_bits(0xc23cf22a)),
        (f32::from_bits(0x42aabb70), f32::from_bits(0xc19e30f8)),
        (f32::from_bits(0x42a530dd), f32::from_bits(0x4102f5b1)),
    );
    path.line_to((f32::from_bits(0x426ed486), f32::from_bits(0x40bd56e4)));
    path.cubic_to(
        (f32::from_bits(0x4276d778), f32::from_bits(0xc164b5d6)),
        (f32::from_bits(0x4255690c), f32::from_bits(0xc2089663)),
        (f32::from_bits(0x4215fd0d), f32::from_bits(0xc23b5bf2)),
    );
    path.line_to((f32::from_bits(0x424f7bed), f32::from_bits(0xc281970f)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L5988-L6010 (chrome/m156)
fn battleOp219(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4198fc97), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4216a3e3), f32::from_bits(0xc298caff)),
        (f32::from_bits(0x4251e7a7), f32::from_bits(0xc2809c9b)),
    );
    path.line_to((f32::from_bits(0x4217bd0d), f32::from_bits(0xc239f1d8)));
    path.cubic_to(
        (f32::from_bits(0x41d9cb04), f32::from_bits(0xc25ce7ce)),
        (f32::from_bits(0x415d2f7f), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4251e7a7), f32::from_bits(0xc2809c9c)));
    path.cubic_to(
        (f32::from_bits(0x4257c623), f32::from_bits(0xc27c6f1e)),
        (f32::from_bits(0x425d7a38), f32::from_bits(0xc27771f7)),
        (f32::from_bits(0x42630157), f32::from_bits(0xc27243fd)),
    );
    path.line_to((f32::from_bits(0x422419a4), f32::from_bits(0xc22f21bb)));
    path.cubic_to(
        (f32::from_bits(0x42201aab), f32::from_bits(0xc232e046)),
        (f32::from_bits(0x421bfb30), f32::from_bits(0xc2367b84)),
        (f32::from_bits(0x4217bd0d), f32::from_bits(0xc239f1d8)),
    );
    path.line_to((f32::from_bits(0x4251e7a7), f32::from_bits(0xc2809c9c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6013-L6037 (chrome/m156)
fn battleOp220(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb630015b), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4198fc97), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4216a3e3), f32::from_bits(0xc298caff)),
        (f32::from_bits(0x4251e7a7), f32::from_bits(0xc2809c9c)),
    );
    path.cubic_to(
        (f32::from_bits(0x4257c623), f32::from_bits(0xc27c6f1e)),
        (f32::from_bits(0x425d7a38), f32::from_bits(0xc27771f7)),
        (f32::from_bits(0x42630157), f32::from_bits(0xc27243fd)),
    );
    path.line_to((f32::from_bits(0x422419a4), f32::from_bits(0xc22f21bb)));
    path.cubic_to(
        (f32::from_bits(0x42201aab), f32::from_bits(0xc232e046)),
        (f32::from_bits(0x421bfb30), f32::from_bits(0xc2367b84)),
        (f32::from_bits(0x4217bd0d), f32::from_bits(0xc239f1d8)),
    );
    path.cubic_to(
        (f32::from_bits(0x41d9cb04), f32::from_bits(0xc25ce7ce)),
        (f32::from_bits(0x415d2f7f), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42630157), f32::from_bits(0xc27243ff)));
    path.cubic_to(
        (f32::from_bits(0x429f78af), f32::from_bits(0xc21c1e80)),
        (f32::from_bits(0x42b11918), f32::from_bits(0xc0cad7ee)),
        (f32::from_bits(0x429f0274), f32::from_bits(0x41bea8f4)),
    );
    path.line_to((f32::from_bits(0x4265e4b4), f32::from_bits(0x4189d394)));
    path.cubic_to(
        (f32::from_bits(0x428005cc), f32::from_bits(0xc092a249)),
        (f32::from_bits(0x42668fa3), f32::from_bits(0xc1e1b6e5)),
        (f32::from_bits(0x422419a4), f32::from_bits(0xc22f21bb)),
    );
    path.line_to((f32::from_bits(0x42630157), f32::from_bits(0xc27243ff)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6040-L6062 (chrome/m156)
fn battleOp221(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41ae0130), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x422a8737), f32::from_bits(0xc294ec91)),
        (f32::from_bits(0x42689b67), f32::from_bits(0xc26ce46c)),
    );
    path.line_to((f32::from_bits(0x42282651), f32::from_bits(0xc22b3f58)));
    path.cubic_to(
        (f32::from_bits(0x41f68bfb), f32::from_bits(0xc2574fdc)),
        (f32::from_bits(0x417b92b3), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42689b68), f32::from_bits(0xc26ce46d)));
    path.cubic_to(
        (f32::from_bits(0x426ebcd2), f32::from_bits(0xc266df67)),
        (f32::from_bits(0x4274a1d2), f32::from_bits(0xc2609e09)),
        (f32::from_bits(0x427a4701), f32::from_bits(0xc25a23f2)),
    );
    path.line_to((f32::from_bits(0x4234ec64), f32::from_bits(0xc21db11e)));
    path.cubic_to(
        (f32::from_bits(0x4230d7ae), f32::from_bits(0xc2225fbc)),
        (f32::from_bits(0x422c94d6), f32::from_bits(0xc226e55a)),
        (f32::from_bits(0x42282652), f32::from_bits(0xc22b3f58)),
    );
    path.line_to((f32::from_bits(0x42689b68), f32::from_bits(0xc26ce46d)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6065-L6089 (chrome/m156)
fn battleOp222(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41ae0130), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x422a8737), f32::from_bits(0xc294ec91)),
        (f32::from_bits(0x42689b68), f32::from_bits(0xc26ce46d)),
    );
    path.cubic_to(
        (f32::from_bits(0x426ebcd2), f32::from_bits(0xc266df67)),
        (f32::from_bits(0x4274a1d2), f32::from_bits(0xc2609e09)),
        (f32::from_bits(0x427a4701), f32::from_bits(0xc25a23f2)),
    );
    path.line_to((f32::from_bits(0x4234ec64), f32::from_bits(0xc21db11e)));
    path.cubic_to(
        (f32::from_bits(0x4230d7ae), f32::from_bits(0xc2225fbc)),
        (f32::from_bits(0x422c94d6), f32::from_bits(0xc226e55a)),
        (f32::from_bits(0x42282651), f32::from_bits(0xc22b3f58)),
    );
    path.cubic_to(
        (f32::from_bits(0x41f68bfb), f32::from_bits(0xc2574fdc)),
        (f32::from_bits(0x417b92b3), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x427a4702), f32::from_bits(0xc25a23f2)));
    path.cubic_to(
        (f32::from_bits(0x42ac7185), f32::from_bits(0xc1db2f83)),
        (f32::from_bits(0x42b35ed0), f32::from_bits(0x413e447a)),
        (f32::from_bits(0x428e4a3d), f32::from_bits(0x422afde8)),
    );
    path.line_to((f32::from_bits(0x424db871), f32::from_bits(0x41f73799)));
    path.cubic_to(
        (f32::from_bits(0x4281aa54), f32::from_bits(0x41098afa)),
        (f32::from_bits(0x427950da), f32::from_bits(0xc19e728d)),
        (f32::from_bits(0x4234ec66), f32::from_bits(0xc21db120)),
    );
    path.line_to((f32::from_bits(0x427a4702), f32::from_bits(0xc25a23f2)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6092-L6114 (chrome/m156)
fn battleOp223(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41c50a2c), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x423ff37f), f32::from_bits(0xc2901f4e)),
        (f32::from_bits(0x427f077c), f32::from_bits(0xc25490c6)),
    );
    path.line_to((f32::from_bits(0x42385bc5), f32::from_bits(0xc219a96d)));
    path.cubic_to(
        (f32::from_bits(0x420ac287), f32::from_bits(0xc2505e9c)),
        (f32::from_bits(0x418e7039), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x427f077b), f32::from_bits(0xc25490c6)));
    path.cubic_to(
        (f32::from_bits(0x42829e52), f32::from_bits(0xc24d1e28)),
        (f32::from_bits(0x42858ec1), f32::from_bits(0xc24566d6)),
        (f32::from_bits(0x428852e3), f32::from_bits(0xc23d7081)),
    );
    path.line_to((f32::from_bits(0x42451839), f32::from_bits(0xc208f1b7)));
    path.cubic_to(
        (f32::from_bits(0x4241186a), f32::from_bits(0xc20eb335)),
        (f32::from_bits(0x423cd88e), f32::from_bits(0xc2144725)),
        (f32::from_bits(0x42385bc4), f32::from_bits(0xc219a96c)),
    );
    path.line_to((f32::from_bits(0x427f077b), f32::from_bits(0xc25490c6)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6117-L6142 (chrome/m156)
fn battleOp224(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41c50a2c), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x423ff37f), f32::from_bits(0xc2901f4e)),
        (f32::from_bits(0x427f077c), f32::from_bits(0xc25490c6)),
    );
    path.line_to((f32::from_bits(0x428852e3), f32::from_bits(0xc23d7081)));
    path.line_to((f32::from_bits(0x42451839), f32::from_bits(0xc208f1b7)));
    path.cubic_to(
        (f32::from_bits(0x4241186a), f32::from_bits(0xc20eb335)),
        (f32::from_bits(0x423cd88e), f32::from_bits(0xc2144725)),
        (f32::from_bits(0x42385bc4), f32::from_bits(0xc219a96c)),
    );
    path.line_to((f32::from_bits(0x42385bc5), f32::from_bits(0xc219a96d)));
    path.cubic_to(
        (f32::from_bits(0x420ac287), f32::from_bits(0xc2505e9c)),
        (f32::from_bits(0x418e7039), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x428852e3), f32::from_bits(0xc23d7081)));
    path.cubic_to(
        (f32::from_bits(0x42b71f8a), f32::from_bits(0xc15aea65)),
        (f32::from_bits(0x42adb77f), f32::from_bits(0x42002593)),
        (f32::from_bits(0x42645e8b), f32::from_bits(0x4270faee)),
    );
    path.line_to((f32::from_bits(0x42251616), f32::from_bits(0x422e33d9)));
    path.cubic_to(
        (f32::from_bits(0x427b2825), f32::from_bits(0x41b945be)),
        (f32::from_bits(0x428460d4), f32::from_bits(0xc11e4099)),
        (f32::from_bits(0x4245183a), f32::from_bits(0xc208f1b8)),
    );
    path.line_to((f32::from_bits(0x428852e3), f32::from_bits(0xc23d7081)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6145-L6167 (chrome/m156)
fn battleOp225(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41d8749b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4251a993), f32::from_bits(0xc28b9f9f)),
        (f32::from_bits(0x4287e789), f32::from_bits(0xc23ea40d)),
    );
    path.line_to((f32::from_bits(0x42447d05), f32::from_bits(0xc209d00a)));
    path.cubic_to(
        (f32::from_bits(0x4217902d), f32::from_bits(0xc249dd89)),
        (f32::from_bits(0x419c7951), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4287e78a), f32::from_bits(0xc23ea40e)));
    path.cubic_to(
        (f32::from_bits(0x428af3dc), f32::from_bits(0xc235f2f3)),
        (f32::from_bits(0x428dca5e), f32::from_bits(0xc22cf844)),
        (f32::from_bits(0x4290688d), f32::from_bits(0xc223bbef)),
    );
    path.line_to((f32::from_bits(0x4250c881), f32::from_bits(0xc1ecb95a)));
    path.cubic_to(
        (f32::from_bits(0x424cff91), f32::from_bits(0xc1fa13ac)),
        (f32::from_bits(0x4248e532), f32::from_bits(0xc2038788)),
        (f32::from_bits(0x42447d06), f32::from_bits(0xc209d00a)),
    );
    path.line_to((f32::from_bits(0x4287e78a), f32::from_bits(0xc23ea40e)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6170-L6194 (chrome/m156)
fn battleOp226(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41d8749b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4251a993), f32::from_bits(0xc28b9f9f)),
        (f32::from_bits(0x4287e78a), f32::from_bits(0xc23ea40e)),
    );
    path.cubic_to(
        (f32::from_bits(0x428af3dc), f32::from_bits(0xc235f2f3)),
        (f32::from_bits(0x428dca5e), f32::from_bits(0xc22cf844)),
        (f32::from_bits(0x4290688d), f32::from_bits(0xc223bbef)),
    );
    path.line_to((f32::from_bits(0x4250c881), f32::from_bits(0xc1ecb95a)));
    path.cubic_to(
        (f32::from_bits(0x424cff91), f32::from_bits(0xc1fa13ac)),
        (f32::from_bits(0x4248e532), f32::from_bits(0xc2038788)),
        (f32::from_bits(0x42447d05), f32::from_bits(0xc209d00a)),
    );
    path.cubic_to(
        (f32::from_bits(0x4217902d), f32::from_bits(0xc249dd89)),
        (f32::from_bits(0x419c7951), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4290688d), f32::from_bits(0xc223bbef)));
    path.cubic_to(
        (f32::from_bits(0x42bd187d), f32::from_bits(0xbfc2a74a)),
        (f32::from_bits(0x42a250ed), f32::from_bits(0x42421cbf)),
        (f32::from_bits(0x42287a28), f32::from_bits(0x428f09b7)),
    );
    path.line_to((f32::from_bits(0x41f394da), f32::from_bits(0x424ecd48)));
    path.cubic_to(
        (f32::from_bits(0x426aac8a), f32::from_bits(0x420c527b)),
        (f32::from_bits(0x4288b219), f32::from_bits(0xbf8cb68f)),
        (f32::from_bits(0x4250c882), f32::from_bits(0xc1ecb95c)),
    );
    path.line_to((f32::from_bits(0x4290688d), f32::from_bits(0xc223bbef)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6197-L6219 (chrome/m156)
fn battleOp227(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f1efaa), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42685cb5), f32::from_bits(0xc2851a3e)),
        (f32::from_bits(0x429160d2), f32::from_bits(0xc22043b6)),
    );
    path.line_to((f32::from_bits(0x42522f73), f32::from_bits(0xc1e7b52d)));
    path.cubic_to(
        (f32::from_bits(0x4227f8ff), f32::from_bits(0xc2406ff8)),
        (f32::from_bits(0x41aee4c7), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429160d2), f32::from_bits(0xc22043b7)));
    path.cubic_to(
        (f32::from_bits(0x42943aa0), f32::from_bits(0xc215eba6)),
        (f32::from_bits(0x4296cd42), f32::from_bits(0xc20b4794)),
        (f32::from_bits(0x429915e6), f32::from_bits(0xc200631e)),
    );
    path.line_to((f32::from_bits(0x425d5418), f32::from_bits(0xc1b99eb9)));
    path.cubic_to(
        (f32::from_bits(0x425a06d4), f32::from_bits(0xc1c95e3a)),
        (f32::from_bits(0x42564e98), f32::from_bits(0xc1d8c0a6)),
        (f32::from_bits(0x42522f74), f32::from_bits(0xc1e7b52e)),
    );
    path.line_to((f32::from_bits(0x429160d2), f32::from_bits(0xc22043b7)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6222-L6250 (chrome/m156)
fn battleOp228(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f1efa9), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x42685cb5), f32::from_bits(0xc2851a3e)),
        (f32::from_bits(0x429160d2), f32::from_bits(0xc22043b7)),
    );
    path.line_to((f32::from_bits(0x429160d2), f32::from_bits(0xc22043b6)));
    path.cubic_to(
        (f32::from_bits(0x42943aa0), f32::from_bits(0xc215eba5)),
        (f32::from_bits(0x4296cd42), f32::from_bits(0xc20b4794)),
        (f32::from_bits(0x429915e6), f32::from_bits(0xc200631e)),
    );
    path.line_to((f32::from_bits(0x425d5418), f32::from_bits(0xc1b99eb9)));
    path.cubic_to(
        (f32::from_bits(0x425a06d4), f32::from_bits(0xc1c95e3a)),
        (f32::from_bits(0x42564e98), f32::from_bits(0xc1d8c0a6)),
        (f32::from_bits(0x42522f74), f32::from_bits(0xc1e7b52e)),
    );
    path.line_to((f32::from_bits(0x42522f73), f32::from_bits(0xc1e7b52d)));
    path.cubic_to(
        (f32::from_bits(0x4227f8ff), f32::from_bits(0xc2406ff8)),
        (f32::from_bits(0x41aee4c7), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429915e6), f32::from_bits(0xc200631e)));
    path.cubic_to(
        (f32::from_bits(0x42abe101), f32::from_bits(0xc11b0235)),
        (f32::from_bits(0x42aa16bb), f32::from_bits(0x417b685c)),
        (f32::from_bits(0x42942fff), f32::from_bits(0x42159e77)),
    );
    path.cubic_to(
        (f32::from_bits(0x427c9284), f32::from_bits(0x426c62d8)),
        (f32::from_bits(0x422cf27d), f32::from_bits(0x4295ccdb)),
        (f32::from_bits(0x419d039e), f32::from_bits(0x42a14aca)),
    );
    path.line_to((f32::from_bits(0x4163022c), f32::from_bits(0x42693188)));
    path.cubic_to(
        (f32::from_bits(0x41fa0b56), f32::from_bits(0x42589424)),
        (f32::from_bits(0x4236951c), f32::from_bits(0x422ae1ad)),
        (f32::from_bits(0x42563f3c), f32::from_bits(0x41d85112)),
    );
    path.cubic_to(
        (f32::from_bits(0x4275e95c), f32::from_bits(0x4135bd94)),
        (f32::from_bits(0x42787fea), f32::from_bits(0xc0e01be1)),
        (f32::from_bits(0x425d5419), f32::from_bits(0xc1b99eba)),
    );
    path.line_to((f32::from_bits(0x429915e6), f32::from_bits(0xc200631e)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6253-L6275 (chrome/m156)
fn battleOp229(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4206c976), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x42801937), f32::from_bits(0xc27a823c)),
        (f32::from_bits(0x4299a0d7), f32::from_bits(0xc1fb88d1)),
    );
    path.line_to((f32::from_bits(0x425e1cfa), f32::from_bits(0xc1b5d505)));
    path.cubic_to(
        (f32::from_bits(0x423933e1), f32::from_bits(0xc2351735)),
        (f32::from_bits(0x41c2df6b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb560056c), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4299a0d8), f32::from_bits(0xc1fb88d0)));
    path.cubic_to(
        (f32::from_bits(0x429c1b73), f32::from_bits(0xc1e34f53)),
        (f32::from_bits(0x429e39d2), f32::from_bits(0xc1ca8528)),
        (f32::from_bits(0x429ff920), f32::from_bits(0xc1b14b8c)),
    );
    path.line_to((f32::from_bits(0x42674955), f32::from_bits(0xc1802a45)));
    path.cubic_to(
        (f32::from_bits(0x4264c2a3), f32::from_bits(0xc192666d)),
        (f32::from_bits(0x4261b27b), f32::from_bits(0xc1a45204)),
        (f32::from_bits(0x425e1cfb), f32::from_bits(0xc1b5d506)),
    );
    path.line_to((f32::from_bits(0x4299a0d8), f32::from_bits(0xc1fb88d0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6278-L6304 (chrome/m156)
fn battleOp230(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4206c976), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x42801937), f32::from_bits(0xc27a823c)),
        (f32::from_bits(0x4299a0d8), f32::from_bits(0xc1fb88d0)),
    );
    path.cubic_to(
        (f32::from_bits(0x429c1b73), f32::from_bits(0xc1e34f53)),
        (f32::from_bits(0x429e39d2), f32::from_bits(0xc1ca8528)),
        (f32::from_bits(0x429ff920), f32::from_bits(0xc1b14b8c)),
    );
    path.line_to((f32::from_bits(0x42674955), f32::from_bits(0xc1802a45)));
    path.cubic_to(
        (f32::from_bits(0x4264c2a3), f32::from_bits(0xc192666d)),
        (f32::from_bits(0x4261b27b), f32::from_bits(0xc1a45204)),
        (f32::from_bits(0x425e1cfa), f32::from_bits(0xc1b5d505)),
    );
    path.cubic_to(
        (f32::from_bits(0x423933e1), f32::from_bits(0xc2351735)),
        (f32::from_bits(0x41c2df6b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429ff91f), f32::from_bits(0xc1b14b8a)));
    path.cubic_to(
        (f32::from_bits(0x42ae673b), f32::from_bits(0x40783c41)),
        (f32::from_bits(0x42a293c2), f32::from_bits(0x41fe6960)),
        (f32::from_bits(0x4280464e), f32::from_bits(0x4252ba7b)),
    );
    path.cubic_to(
        (f32::from_bits(0x423bf1b3), f32::from_bits(0x42932023)),
        (f32::from_bits(0x41a5f32c), f32::from_bits(0x42a99309)),
        (f32::from_bits(0xc0c67989), f32::from_bits(0x42a5892f)),
    );
    path.line_to((f32::from_bits(0xc08f79c7), f32::from_bits(0x426f5437)));
    path.cubic_to(
        (f32::from_bits(0x416fed74), f32::from_bits(0x42752af2)),
        (f32::from_bits(0x4207dcfc), f32::from_bits(0x4254b62d)),
        (f32::from_bits(0x42397512), f32::from_bits(0x42185575)),
    );
    path.cubic_to(
        (f32::from_bits(0x426b0d26), f32::from_bits(0x41b7e97d)),
        (f32::from_bits(0x427c2639), f32::from_bits(0x40337286)),
        (f32::from_bits(0x42674956), f32::from_bits(0xc1802a46)),
    );
    path.line_to((f32::from_bits(0x429ff91f), f32::from_bits(0xc1b14b8a)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6307-L6329 (chrome/m156)
fn battleOp231(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x421472e7), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x428b6da4), f32::from_bits(0xc26973d7)),
        (f32::from_bits(0x429fb179), f32::from_bits(0xc1b54986)),
    );
    path.line_to((f32::from_bits(0x4266e1be), f32::from_bits(0xc1830d0f)));
    path.cubic_to(
        (f32::from_bits(0x42499544), f32::from_bits(0xc228c2c8)),
        (f32::from_bits(0x41d69ff6), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429fb179), f32::from_bits(0xc1b54988)));
    path.cubic_to(
        (f32::from_bits(0x42a1a632), f32::from_bits(0xc199b837)),
        (f32::from_bits(0x42a3282f), f32::from_bits(0xc17b594e)),
        (f32::from_bits(0x42a43501), f32::from_bits(0xc142a7ba)),
    );
    path.line_to((f32::from_bits(0x426d6865), f32::from_bits(0xc10cb6f0)));
    path.cubic_to(
        (f32::from_bits(0x426be3bc), f32::from_bits(0xc135b2ae)),
        (f32::from_bits(0x4269b5af), f32::from_bits(0xc15e3ec8)),
        (f32::from_bits(0x4266e1be), f32::from_bits(0xc1830d0f)),
    );
    path.line_to((f32::from_bits(0x429fb179), f32::from_bits(0xc1b54988)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6332-L6359 (chrome/m156)
fn battleOp232(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x421472e7), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x428b6da4), f32::from_bits(0xc26973d8)),
        (f32::from_bits(0x429fb179), f32::from_bits(0xc1b54988)),
    );
    path.line_to((f32::from_bits(0x429fb179), f32::from_bits(0xc1b54986)));
    path.cubic_to(
        (f32::from_bits(0x42a1a632), f32::from_bits(0xc199b836)),
        (f32::from_bits(0x42a3282f), f32::from_bits(0xc17b594d)),
        (f32::from_bits(0x42a43501), f32::from_bits(0xc142a7ba)),
    );
    path.line_to((f32::from_bits(0x426d6865), f32::from_bits(0xc10cb6f0)));
    path.cubic_to(
        (f32::from_bits(0x426be3bc), f32::from_bits(0xc135b2ae)),
        (f32::from_bits(0x4269b5af), f32::from_bits(0xc15e3ec8)),
        (f32::from_bits(0x4266e1be), f32::from_bits(0xc1830d0f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42499544), f32::from_bits(0xc228c2c8)),
        (f32::from_bits(0x41d69ff6), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a43502), f32::from_bits(0xc142a7bb)));
    path.cubic_to(
        (f32::from_bits(0x42ace9b0), f32::from_bits(0x4189ae79)),
        (f32::from_bits(0x429590d6), f32::from_bits(0x423ab1c1)),
        (f32::from_bits(0x424df762), f32::from_bits(0x428231a6)),
    );
    path.cubic_to(
        (f32::from_bits(0x41e19a31), f32::from_bits(0x42a70a69)),
        (f32::from_bits(0xc04a3289), f32::from_bits(0x42b03133)),
        (f32::from_bits(0xc1f5f36e), f32::from_bits(0x429a3139)),
    );
    path.line_to((f32::from_bits(0xc1b1cbb9), f32::from_bits(0x425eedb9)));
    path.cubic_to(
        (f32::from_bits(0xc0122aac), f32::from_bits(0x427ebc5a)),
        (f32::from_bits(0x41a31606), f32::from_bits(0x42718130)),
        (f32::from_bits(0x4214e430), f32::from_bits(0x423c3b73)),
    );
    path.cubic_to(
        (f32::from_bits(0x42583d5c), f32::from_bits(0x4206f5b6)),
        (f32::from_bits(0x4279fe97), f32::from_bits(0x41470ec8)),
        (f32::from_bits(0x426d6866), f32::from_bits(0xc10cb6eb)),
    );
    path.line_to((f32::from_bits(0x42a43502), f32::from_bits(0xc142a7bb)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6362-L6384 (chrome/m156)
fn battleOp233(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4220aa02), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42952310), f32::from_bits(0xc258f48d)),
        (f32::from_bits(0x42a35f68), f32::from_bits(0xc16b5614)),
    );
    path.line_to((f32::from_bits(0x426c3395), f32::from_bits(0xc12a1f61)));
    path.cubic_to(
        (f32::from_bits(0x42579ea8), f32::from_bits(0xc21cd5ce)),
        (f32::from_bits(0x41e84916), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a35f69), f32::from_bits(0xc16b5613)));
    path.cubic_to(
        (f32::from_bits(0x42a4bd24), f32::from_bits(0xc12ea3c2)),
        (f32::from_bits(0x42a59325), f32::from_bits(0xc0e282d6)),
        (f32::from_bits(0x42a5dfdf), f32::from_bits(0xc04e84a0)),
    );
    path.line_to((f32::from_bits(0x426fd18d), f32::from_bits(0xc0154a48)));
    path.cubic_to(
        (f32::from_bits(0x426f62a1), f32::from_bits(0xc0a3be33)),
        (f32::from_bits(0x426e2d39), f32::from_bits(0xc0fc7dbb)),
        (f32::from_bits(0x426c3397), f32::from_bits(0xc12a1f63)),
    );
    path.line_to((f32::from_bits(0x42a35f69), f32::from_bits(0xc16b5613)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6387-L6414 (chrome/m156)
fn battleOp234(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4220aa02), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42952310), f32::from_bits(0xc258f48d)),
        (f32::from_bits(0x42a35f69), f32::from_bits(0xc16b5613)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a4bd24), f32::from_bits(0xc12ea3c2)),
        (f32::from_bits(0x42a59325), f32::from_bits(0xc0e282d6)),
        (f32::from_bits(0x42a5dfdf), f32::from_bits(0xc04e84a0)),
    );
    path.line_to((f32::from_bits(0x426fd18d), f32::from_bits(0xc0154a48)));
    path.cubic_to(
        (f32::from_bits(0x426f62a1), f32::from_bits(0xc0a3be33)),
        (f32::from_bits(0x426e2d39), f32::from_bits(0xc0fc7dbb)),
        (f32::from_bits(0x426c3397), f32::from_bits(0xc12a1f63)),
    );
    path.line_to((f32::from_bits(0x426c3395), f32::from_bits(0xc12a1f61)));
    path.cubic_to(
        (f32::from_bits(0x42579ea8), f32::from_bits(0xc21cd5ce)),
        (f32::from_bits(0x41e84916), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a5dfdf), f32::from_bits(0xc04e84a0)));
    path.cubic_to(
        (f32::from_bits(0x42a85e4f), f32::from_bits(0x41e6959e)),
        (f32::from_bits(0x4285b4e3), f32::from_bits(0x426ae44f)),
        (f32::from_bits(0x4219b105), f32::from_bits(0x42932450)),
    );
    path.cubic_to(
        (f32::from_bits(0x411fe111), f32::from_bits(0x42b0d679)),
        (f32::from_bits(0xc1c3966b), f32::from_bits(0x42ab1d42)),
        (f32::from_bits(0xc2482755), f32::from_bits(0x428470e8)),
    );
    path.line_to((f32::from_bits(0xc210b07c), f32::from_bits(0x423f7b24)));
    path.cubic_to(
        (f32::from_bits(0xc18d6382), f32::from_bits(0x427764e8)),
        (f32::from_bits(0x40e72680), f32::from_bits(0x427fab4e)),
        (f32::from_bits(0x41de345e), f32::from_bits(0x4254bc3b)),
    );
    path.cubic_to(
        (f32::from_bits(0x42414f8e), f32::from_bits(0x4229cd28)),
        (f32::from_bits(0x42736c9d), f32::from_bits(0x41a6b008)),
        (f32::from_bits(0x426fd18e), f32::from_bits(0xc0154a3f)),
    );
    path.line_to((f32::from_bits(0x42a5dfdf), f32::from_bits(0xc04e84a0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6417-L6439 (chrome/m156)
fn battleOp235(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x422e5e2d), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x429f82f2), f32::from_bits(0xc2451c35)),
        (f32::from_bits(0x42a59867), f32::from_bits(0xc0b956c5)),
    );
    path.line_to((f32::from_bits(0x426f6a3b), f32::from_bits(0xc085fae3)));
    path.cubic_to(
        (f32::from_bits(0x42669e7e), f32::from_bits(0xc20e7d42)),
        (f32::from_bits(0x41fc1920), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a59868), f32::from_bits(0xc0b956ca)));
    path.cubic_to(
        (f32::from_bits(0x42a62cd8), f32::from_bits(0xbfd2dd07)),
        (f32::from_bits(0x42a621be), f32::from_bits(0x4020d557)),
        (f32::from_bits(0x42a57734), f32::from_bits(0x40d4ef9c)),
    );
    path.line_to((f32::from_bits(0x426f3a3b), f32::from_bits(0x4099edfc)));
    path.cubic_to(
        (f32::from_bits(0x427030cb), f32::from_bits(0x3fe887ba)),
        (f32::from_bits(0x427040d6), f32::from_bits(0xbf986e77)),
        (f32::from_bits(0x426f6a3b), f32::from_bits(0xc085fae4)),
    );
    path.line_to((f32::from_bits(0x42a59868), f32::from_bits(0xc0b956ca)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6442-L6469 (chrome/m156)
fn battleOp236(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x422e5e2d), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x429f82f2), f32::from_bits(0xc2451c35)),
        (f32::from_bits(0x42a59868), f32::from_bits(0xc0b956ca)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a62cd8), f32::from_bits(0xbfd2dd07)),
        (f32::from_bits(0x42a621be), f32::from_bits(0x4020d557)),
        (f32::from_bits(0x42a57734), f32::from_bits(0x40d4ef9c)),
    );
    path.line_to((f32::from_bits(0x426f3a3b), f32::from_bits(0x4099edfc)));
    path.cubic_to(
        (f32::from_bits(0x427030cb), f32::from_bits(0x3fe887bb)),
        (f32::from_bits(0x427040d6), f32::from_bits(0xbf986e74)),
        (f32::from_bits(0x426f6a3b), f32::from_bits(0xc085fae3)),
    );
    path.line_to((f32::from_bits(0x426f6a3b), f32::from_bits(0xc085fae4)));
    path.cubic_to(
        (f32::from_bits(0x42669e7e), f32::from_bits(0xc20e7d42)),
        (f32::from_bits(0x41fc1920), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a57735), f32::from_bits(0x40d4ef9d)));
    path.cubic_to(
        (f32::from_bits(0x429fe5e1), f32::from_bits(0x4225104d)),
        (f32::from_bits(0x425fa7d9), f32::from_bits(0x428cf91a)),
        (f32::from_bits(0x41b3ea58), f32::from_bits(0x429fca49)),
    );
    path.cubic_to(
        (f32::from_bits(0xc12ef606), f32::from_bits(0x42b29b77)),
        (f32::from_bits(0xc23abc07), f32::from_bits(0x4299d29d)),
        (f32::from_bits(0xc2863a28), f32::from_bits(0x42435615)),
    );
    path.line_to((f32::from_bits(0xc242103b), f32::from_bits(0x420d34fa)));
    path.cubic_to(
        (f32::from_bits(0xc206fd22), f32::from_bits(0x425e64f1)),
        (f32::from_bits(0xc0fcf4a4), f32::from_bits(0x42811d1e)),
        (f32::from_bits(0x41820f34), f32::from_bits(0x426705a2)),
    );
    path.cubic_to(
        (f32::from_bits(0x4221adc8), f32::from_bits(0x424bd107)),
        (f32::from_bits(0x42672d88), f32::from_bits(0x41eea576)),
        (f32::from_bits(0x426f3a3c), f32::from_bits(0x4099edfe)),
    );
    path.line_to((f32::from_bits(0x42a57735), f32::from_bits(0x40d4ef9d)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6472-L6496 (chrome/m156)
fn battleOp237(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41b25a1b), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x422e9a51), f32::from_bits(0xc294100b)),
        (f32::from_bits(0x426d0a79), f32::from_bits(0xc26874a1)),
    );
    path.cubic_to(
        (f32::from_bits(0x4295bd51), f32::from_bits(0xc228c92e)),
        (f32::from_bits(0x42a6d6d5), f32::from_bits(0xc1a5596e)),
        (f32::from_bits(0x42a5f7e5), f32::from_bits(0x3fcf7f4c)),
    );
    path.line_to((f32::from_bits(0x426ff448), f32::from_bits(0x3f95ff69)));
    path.cubic_to(
        (f32::from_bits(0x4271369b), f32::from_bits(0xc16f0f30)),
        (f32::from_bits(0x42587daa), f32::from_bits(0xc1f4071e)),
        (f32::from_bits(0x422b5ada), f32::from_bits(0xc2280a4b)),
    );
    path.cubic_to(
        (f32::from_bits(0x41fc7014), f32::from_bits(0xc2561107)),
        (f32::from_bits(0x4180eddd), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a5f7e5), f32::from_bits(0x3fcf7f2e)));
    path.cubic_to(
        (f32::from_bits(0x42a5cbdf), f32::from_bits(0x40c0b7f8)),
        (f32::from_bits(0x42a4eca2), f32::from_bits(0x41268f7d)),
        (f32::from_bits(0x42a35c4c), f32::from_bits(0x416be04e)),
    );
    path.line_to((f32::from_bits(0x426c2f14), f32::from_bits(0x412a834e)));
    path.cubic_to(
        (f32::from_bits(0x426e71e2), f32::from_bits(0x40f0cf74)),
        (f32::from_bits(0x426fb4a3), f32::from_bits(0x408b5090)),
        (f32::from_bits(0x426ff449), f32::from_bits(0x3f95ff6b)),
    );
    path.line_to((f32::from_bits(0x42a5f7e5), f32::from_bits(0x3fcf7f2e)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6499-L6527 (chrome/m156)
fn battleOp238(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41b25a1b), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x422e9a51), f32::from_bits(0xc294100b)),
        (f32::from_bits(0x426d0a79), f32::from_bits(0xc26874a1)),
    );
    path.cubic_to(
        (f32::from_bits(0x4295bd51), f32::from_bits(0xc228c92e)),
        (f32::from_bits(0x42a6d6d5), f32::from_bits(0xc1a5596f)),
        (f32::from_bits(0x42a5f7e5), f32::from_bits(0x3fcf7f2e)),
    );
    path.line_to((f32::from_bits(0x426c2f14), f32::from_bits(0x412a834e)));
    path.cubic_to(
        (f32::from_bits(0x426e71e2), f32::from_bits(0x40f0cf74)),
        (f32::from_bits(0x426fb4a3), f32::from_bits(0x408b5090)),
        (f32::from_bits(0x426ff449), f32::from_bits(0x3f95ff6b)),
    );
    path.line_to((f32::from_bits(0x426ff448), f32::from_bits(0x3f95ff69)));
    path.cubic_to(
        (f32::from_bits(0x4271369b), f32::from_bits(0xc16f0f30)),
        (f32::from_bits(0x42587daa), f32::from_bits(0xc1f4071e)),
        (f32::from_bits(0x422b5ada), f32::from_bits(0xc2280a4b)),
    );
    path.cubic_to(
        (f32::from_bits(0x41fc7014), f32::from_bits(0xc2561107)),
        (f32::from_bits(0x4180eddd), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a35c4c), f32::from_bits(0x416be04e)));
    path.cubic_to(
        (f32::from_bits(0x42963d3f), f32::from_bits(0x424c5e0d)),
        (f32::from_bits(0x42354f77), f32::from_bits(0x429d76d6)),
        (f32::from_bits(0x41096c90), f32::from_bits(0x42a51bdb)),
    );
    path.cubic_to(
        (f32::from_bits(0xc1e1325f), f32::from_bits(0x42acc0e0)),
        (f32::from_bits(0xc27bf938), f32::from_bits(0x4282ec23)),
        (f32::from_bits(0xc299cad8), f32::from_bits(0x41f9ecd8)),
    );
    path.line_to((f32::from_bits(0xc25e59b3), f32::from_bits(0x41b4ab36)));
    path.cubic_to(
        (f32::from_bits(0xc2362649), f32::from_bits(0x423d4911)),
        (f32::from_bits(0xc1a2caf7), f32::from_bits(0x4279c398)),
        (f32::from_bits(0x40c6af7d), f32::from_bits(0x426eb62b)),
    );
    path.cubic_to(
        (f32::from_bits(0x4203115b), f32::from_bits(0x4263a8be)),
        (f32::from_bits(0x425936a2), f32::from_bits(0x4213bc4a)),
        (f32::from_bits(0x426c2f16), f32::from_bits(0x412a8350)),
    );
    path.line_to((f32::from_bits(0x42a35c4c), f32::from_bits(0x416be04e)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6530-L6554 (chrome/m156)
fn battleOp239(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41ba3f99), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4235f79d), f32::from_bits(0xc29271cf)),
        (f32::from_bits(0x4274db3f), f32::from_bits(0xc260354d)),
    );
    path.cubic_to(
        (f32::from_bits(0x4299df70), f32::from_bits(0xc21b86fd)),
        (f32::from_bits(0x42a97305), f32::from_bits(0xc17e5d7a)),
        (f32::from_bits(0x42a55ba0), f32::from_bits(0x40e961b4)),
    );
    path.line_to((f32::from_bits(0x426f1259), f32::from_bits(0x40a8b5ae)));
    path.cubic_to(
        (f32::from_bits(0x4274fca8), f32::from_bits(0xc137e0e1)),
        (f32::from_bits(0x425e777b), f32::from_bits(0xc1e0dbdb)),
        (f32::from_bits(0x42310131), f32::from_bits(0xc2221408)),
    );
    path.cubic_to(
        (f32::from_bits(0x42038ae6), f32::from_bits(0xc253ba22)),
        (f32::from_bits(0x4186a32c), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb560056c), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a55ba0), f32::from_bits(0x40e961b9)));
    path.cubic_to(
        (f32::from_bits(0x42a48d09), f32::from_bits(0x413de0a1)),
        (f32::from_bits(0x42a2fc74), f32::from_bits(0x41833376)),
        (f32::from_bits(0x42a0adff), f32::from_bits(0x41a6c250)),
    );
    path.line_to((f32::from_bits(0x42684ed9), f32::from_bits(0x417118ef)));
    path.cubic_to(
        (f32::from_bits(0x426ba483), f32::from_bits(0x413db02f)),
        (f32::from_bits(0x426de7aa), f32::from_bits(0x410942c3)),
        (f32::from_bits(0x426f1258), f32::from_bits(0x40a8b5ad)),
    );
    path.line_to((f32::from_bits(0x42a55ba0), f32::from_bits(0x40e961b9)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6557-L6585 (chrome/m156)
fn battleOp240(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41ba3f99), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4235f79d), f32::from_bits(0xc29271cf)),
        (f32::from_bits(0x4274db3f), f32::from_bits(0xc260354d)),
    );
    path.cubic_to(
        (f32::from_bits(0x4299df70), f32::from_bits(0xc21b86fd)),
        (f32::from_bits(0x42a97305), f32::from_bits(0xc17e5d7a)),
        (f32::from_bits(0x42a55ba0), f32::from_bits(0x40e961b9)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a48d09), f32::from_bits(0x413de0a1)),
        (f32::from_bits(0x42a2fc74), f32::from_bits(0x41833376)),
        (f32::from_bits(0x42a0adff), f32::from_bits(0x41a6c250)),
    );
    path.line_to((f32::from_bits(0x42684ed9), f32::from_bits(0x417118ef)));
    path.cubic_to(
        (f32::from_bits(0x426ba483), f32::from_bits(0x413db02f)),
        (f32::from_bits(0x426de7aa), f32::from_bits(0x410942c3)),
        (f32::from_bits(0x426f1259), f32::from_bits(0x40a8b5ae)),
    );
    path.cubic_to(
        (f32::from_bits(0x4274fca8), f32::from_bits(0xc137e0e1)),
        (f32::from_bits(0x425e777b), f32::from_bits(0xc1e0dbdb)),
        (f32::from_bits(0x42310131), f32::from_bits(0xc2221408)),
    );
    path.cubic_to(
        (f32::from_bits(0x42038ae6), f32::from_bits(0xc253ba22)),
        (f32::from_bits(0x4186a32c), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a0ae00), f32::from_bits(0x41a6c250)));
    path.cubic_to(
        (f32::from_bits(0x428d4422), f32::from_bits(0x4269069e)),
        (f32::from_bits(0x42118d33), f32::from_bits(0x42a8086f)),
        (f32::from_bits(0xc00fe376), f32::from_bits(0x42a5f066)),
    );
    path.cubic_to(
        (f32::from_bits(0xc22389a2), f32::from_bits(0x42a3d85e)),
        (f32::from_bits(0xc2935e5d), f32::from_bits(0x42596224)),
        (f32::from_bits(0xc2a2b39d), f32::from_bits(0x4183b53a)),
    );
    path.line_to((f32::from_bits(0xc26b3b33), f32::from_bits(0x413e6bca)));
    path.cubic_to(
        (f32::from_bits(0xc2551027), f32::from_bits(0x421d2508)),
        (f32::from_bits(0xc1ec70a3), f32::from_bits(0x426ce27d)),
        (f32::from_bits(0xbfd007ff), f32::from_bits(0x426fe979)),
    );
    path.cubic_to(
        (f32::from_bits(0x41d26fa4), f32::from_bits(0x4272f076)),
        (f32::from_bits(0x424c3d84), f32::from_bits(0x422873d5)),
        (f32::from_bits(0x42684eda), f32::from_bits(0x417118ee)),
    );
    path.line_to((f32::from_bits(0x42a0ae00), f32::from_bits(0x41a6c250)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6588-L6612 (chrome/m156)
fn battleOp241(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41c2abe0), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x423dc4ab), f32::from_bits(0xc290a493)),
        (f32::from_bits(0x427cd8fd), f32::from_bits(0xc25727eb)),
    );
    path.cubic_to(
        (f32::from_bits(0x429df6a6), f32::from_bits(0xc20d06b1)),
        (f32::from_bits(0x42aba628), f32::from_bits(0xc12bcbe5)),
        (f32::from_bits(0x42a3dc46), f32::from_bits(0x4154872f)),
    );
    path.line_to((f32::from_bits(0x426ce81c), f32::from_bits(0x4119a283)));
    path.cubic_to(
        (f32::from_bits(0x42782ad8), f32::from_bits(0xc0f86165)),
        (f32::from_bits(0x42646188), f32::from_bits(0xc1cbe4ab)),
        (f32::from_bits(0x4236c80c), f32::from_bits(0xc21b88d1)),
    );
    path.cubic_to(
        (f32::from_bits(0x42092e8f), f32::from_bits(0xc2511f4c)),
        (f32::from_bits(0x418cb9f2), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a3dc46), f32::from_bits(0x41548735)));
    path.cubic_to(
        (f32::from_bits(0x42a2537f), f32::from_bits(0x41901e3f)),
        (f32::from_bits(0x429ff996), f32::from_bits(0x41b55e92)),
        (f32::from_bits(0x429cd549), f32::from_bits(0x41d999a0)),
    );
    path.line_to((f32::from_bits(0x4262bf29), f32::from_bits(0x419d4d21)));
    path.cubic_to(
        (f32::from_bits(0x42674a02), f32::from_bits(0x41831c46)),
        (f32::from_bits(0x426ab03e), f32::from_bits(0x41505d16)),
        (f32::from_bits(0x426ce81d), f32::from_bits(0x4119a283)),
    );
    path.line_to((f32::from_bits(0x42a3dc46), f32::from_bits(0x41548735)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6615-L6643 (chrome/m156)
fn battleOp242(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41c2abe0), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x423dc4ab), f32::from_bits(0xc290a493)),
        (f32::from_bits(0x427cd8fd), f32::from_bits(0xc25727eb)),
    );
    path.cubic_to(
        (f32::from_bits(0x429df6a6), f32::from_bits(0xc20d06b1)),
        (f32::from_bits(0x42aba628), f32::from_bits(0xc12bcbe5)),
        (f32::from_bits(0x42a3dc46), f32::from_bits(0x41548735)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a2537f), f32::from_bits(0x41901e3f)),
        (f32::from_bits(0x429ff996), f32::from_bits(0x41b55e92)),
        (f32::from_bits(0x429cd549), f32::from_bits(0x41d999a0)),
    );
    path.line_to((f32::from_bits(0x4262bf29), f32::from_bits(0x419d4d21)));
    path.cubic_to(
        (f32::from_bits(0x42674a02), f32::from_bits(0x41831c46)),
        (f32::from_bits(0x426ab03e), f32::from_bits(0x41505d16)),
        (f32::from_bits(0x426ce81c), f32::from_bits(0x4119a283)),
    );
    path.cubic_to(
        (f32::from_bits(0x42782ad8), f32::from_bits(0xc0f86165)),
        (f32::from_bits(0x42646188), f32::from_bits(0xc1cbe4ab)),
        (f32::from_bits(0x4236c80c), f32::from_bits(0xc21b88d1)),
    );
    path.cubic_to(
        (f32::from_bits(0x42092e8f), f32::from_bits(0xc2511f4c)),
        (f32::from_bits(0x418cb9f2), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429cd549), f32::from_bits(0x41d999a0)));
    path.cubic_to(
        (f32::from_bits(0x42824b9e), f32::from_bits(0x4282e841)),
        (f32::from_bits(0x41d1b597), f32::from_bits(0x42b119ff)),
        (f32::from_bits(0xc15b80c3), f32::from_bits(0x42a3b776)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2569b2d), f32::from_bits(0x429654ee)),
        (f32::from_bits(0xc2a5db0b), f32::from_bits(0x42228c64)),
        (f32::from_bits(0xc2a5ffee), f32::from_bits(0x3e172efd)),
    );
    path.line_to((f32::from_bits(0xc26fffe7), f32::from_bits(0x3dda91a4)));
    path.cubic_to(
        (f32::from_bits(0xc26fca99), f32::from_bits(0x41eb0285)),
        (f32::from_bits(0xc21b2317), f32::from_bits(0x425958e5)),
        (f32::from_bits(0xc11ead4d), f32::from_bits(0x426cb2ed)),
    );
    path.cubic_to(
        (f32::from_bits(0x419798e1), f32::from_bits(0x4280067a)),
        (f32::from_bits(0x423c6102), f32::from_bits(0x423d4379)),
        (f32::from_bits(0x4262bf29), f32::from_bits(0x419d4d1f)),
    );
    path.line_to((f32::from_bits(0x429cd549), f32::from_bits(0x41d999a0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6646-L6670 (chrome/m156)
fn battleOp243(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41caf078), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42455e40), f32::from_bits(0xc28ecc78)),
        (f32::from_bits(0x42822b31), f32::from_bits(0xc24e07b4)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a1a743), f32::from_bits(0xc1fcecee)),
        (f32::from_bits(0x42ad3753), f32::from_bits(0xc0b3be45)),
        (f32::from_bits(0x42a18eed), f32::from_bits(0x419892cb)),
    );
    path.line_to((f32::from_bits(0x42699409), f32::from_bits(0x415c9689)));
    path.cubic_to(
        (f32::from_bits(0x427a6ed6), f32::from_bits(0xc081ef5b)),
        (f32::from_bits(0x4269b739), f32::from_bits(0xc1b6d67a)),
        (f32::from_bits(0x423c321c), f32::from_bits(0xc214effc)),
    );
    path.cubic_to(
        (f32::from_bits(0x420eacff), f32::from_bits(0xc24e74bc)),
        (f32::from_bits(0x4192b3ff), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42a18eed), f32::from_bits(0x419892ca)));
    path.cubic_to(
        (f32::from_bits(0x429f43c9), f32::from_bits(0x41bf6e44)),
        (f32::from_bits(0x429c198b), f32::from_bits(0x41e561a5)),
        (f32::from_bits(0x42981a0b), f32::from_bits(0x4204fb6e)),
    );
    path.line_to((f32::from_bits(0x425be7f8), f32::from_bits(0x41c0436a)));
    path.cubic_to(
        (f32::from_bits(0x4261afba), f32::from_bits(0x41a5d162)),
        (f32::from_bits(0x42664329), f32::from_bits(0x418a6237)),
        (f32::from_bits(0x4269940a), f32::from_bits(0x415c968a)),
    );
    path.line_to((f32::from_bits(0x42a18eed), f32::from_bits(0x419892ca)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6673-L6703 (chrome/m156)
fn battleOp244(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb630015b), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41caf078), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42455e40), f32::from_bits(0xc28ecc78)),
        (f32::from_bits(0x42822b31), f32::from_bits(0xc24e07b4)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a1a743), f32::from_bits(0xc1fcecee)),
        (f32::from_bits(0x42ad3753), f32::from_bits(0xc0b3be48)),
        (f32::from_bits(0x42a18eed), f32::from_bits(0x419892ca)),
    );
    path.line_to((f32::from_bits(0x42a18eed), f32::from_bits(0x419892cb)));
    path.cubic_to(
        (f32::from_bits(0x429f43c9), f32::from_bits(0x41bf6e45)),
        (f32::from_bits(0x429c198b), f32::from_bits(0x41e561a5)),
        (f32::from_bits(0x42981a0b), f32::from_bits(0x4204fb6e)),
    );
    path.line_to((f32::from_bits(0x425be7f8), f32::from_bits(0x41c0436a)));
    path.cubic_to(
        (f32::from_bits(0x4261afba), f32::from_bits(0x41a5d162)),
        (f32::from_bits(0x42664329), f32::from_bits(0x418a6237)),
        (f32::from_bits(0x4269940a), f32::from_bits(0x415c968a)),
    );
    path.line_to((f32::from_bits(0x42699409), f32::from_bits(0x415c9689)));
    path.cubic_to(
        (f32::from_bits(0x427a6ed6), f32::from_bits(0xc081ef5b)),
        (f32::from_bits(0x4269b739), f32::from_bits(0xc1b6d67a)),
        (f32::from_bits(0x423c321c), f32::from_bits(0xc214effc)),
    );
    path.cubic_to(
        (f32::from_bits(0x420eacff), f32::from_bits(0xc24e74bc)),
        (f32::from_bits(0x4192b3ff), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42981a0b), f32::from_bits(0x4204fb6e)));
    path.cubic_to(
        (f32::from_bits(0x426c6b55), f32::from_bits(0x42900555)),
        (f32::from_bits(0x417b6a9f), f32::from_bits(0x42b7a6c3)),
        (f32::from_bits(0xc1c57072), f32::from_bits(0x429e7dd7)),
    );
    path.cubic_to(
        (f32::from_bits(0xc282258c), f32::from_bits(0x428554eb)),
        (f32::from_bits(0xc2b314c4), f32::from_bits(0x41cdbc89)),
        (f32::from_bits(0xc2a2f571), f32::from_bits(0xc17d09b6)),
    );
    path.line_to((f32::from_bits(0xc26b9a61), f32::from_bits(0xc136eb32)));
    path.cubic_to(
        (f32::from_bits(0xc28174d0), f32::from_bits(0x4194b9b3)),
        (f32::from_bits(0xc23c29fc), f32::from_bits(0x4240c4dc)),
        (f32::from_bits(0xc18eba2f), f32::from_bits(0x4265250a)),
    );
    path.cubic_to(
        (f32::from_bits(0x4135bf41), f32::from_bits(0x4284c29d)),
        (f32::from_bits(0x422ae7d8), f32::from_bits(0x42503918)),
        (f32::from_bits(0x425be7f9), f32::from_bits(0x41c04367)),
    );
    path.line_to((f32::from_bits(0x42981a0b), f32::from_bits(0x4204fb6e)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6706-L6730 (chrome/m156)
fn battleOp245(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41d28773), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x424c4acf), f32::from_bits(0xc28d0a47)),
        (f32::from_bits(0x428572fc), f32::from_bits(0xc24574fc)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a4c090), f32::from_bits(0xc1e1aad9)),
        (f32::from_bits(0x42ae2294), f32::from_bits(0xbf62367e)),
        (f32::from_bits(0x429ebce0), f32::from_bits(0x41c23fec)),
    );
    path.line_to((f32::from_bits(0x4265801d), f32::from_bits(0x418c6be6)));
    path.cubic_to(
        (f32::from_bits(0x427bc2fb), f32::from_bits(0xbf238720)),
        (f32::from_bits(0x426e322e), f32::from_bits(0xc1a32211)),
        (f32::from_bits(0x4240f046), f32::from_bits(0xc20ebd71)),
    );
    path.cubic_to(
        (f32::from_bits(0x4213ae61), f32::from_bits(0xc24be9da)),
        (f32::from_bits(0x41983095), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429ebce1), f32::from_bits(0x41c23fee)));
    path.cubic_to(
        (f32::from_bits(0x429bb658), f32::from_bits(0x41e9cedc)),
        (f32::from_bits(0x4297c4ea), f32::from_bits(0x4208130e)),
        (f32::from_bits(0x4292f5c0), f32::from_bits(0x421a62d5)),
    );
    path.line_to((f32::from_bits(0x425478e6), f32::from_bits(0x41df3573)));
    path.cubic_to(
        (f32::from_bits(0x425b6ce6), f32::from_bits(0x41c4bbf1)),
        (f32::from_bits(0x42612050), f32::from_bits(0x41a90494)),
        (f32::from_bits(0x4265801e), f32::from_bits(0x418c6be6)),
    );
    path.line_to((f32::from_bits(0x429ebce1), f32::from_bits(0x41c23fee)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6733-L6761 (chrome/m156)
fn battleOp246(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41d28773), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x424c4acf), f32::from_bits(0xc28d0a47)),
        (f32::from_bits(0x428572fc), f32::from_bits(0xc24574fc)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a4c090), f32::from_bits(0xc1e1aad9)),
        (f32::from_bits(0x42ae2294), f32::from_bits(0xbf62367e)),
        (f32::from_bits(0x429ebce1), f32::from_bits(0x41c23fee)),
    );
    path.cubic_to(
        (f32::from_bits(0x429bb658), f32::from_bits(0x41e9cedc)),
        (f32::from_bits(0x4297c4ea), f32::from_bits(0x4208130e)),
        (f32::from_bits(0x4292f5c0), f32::from_bits(0x421a62d5)),
    );
    path.line_to((f32::from_bits(0x425478e6), f32::from_bits(0x41df3573)));
    path.cubic_to(
        (f32::from_bits(0x425b6ce6), f32::from_bits(0x41c4bbf1)),
        (f32::from_bits(0x42612050), f32::from_bits(0x41a90494)),
        (f32::from_bits(0x4265801d), f32::from_bits(0x418c6be6)),
    );
    path.cubic_to(
        (f32::from_bits(0x427bc2fb), f32::from_bits(0xbf238720)),
        (f32::from_bits(0x426e322e), f32::from_bits(0xc1a32211)),
        (f32::from_bits(0x4240f046), f32::from_bits(0xc20ebd71)),
    );
    path.cubic_to(
        (f32::from_bits(0x4213ae61), f32::from_bits(0xc24be9da)),
        (f32::from_bits(0x41983095), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4292f5c1), f32::from_bits(0x421a62d6)));
    path.cubic_to(
        (f32::from_bits(0x42541a09), f32::from_bits(0x429b1363)),
        (f32::from_bits(0x40b7c75d), f32::from_bits(0x42bb84d6)),
        (f32::from_bits(0xc2093cef), f32::from_bits(0x42972755)),
    );
    path.cubic_to(
        (f32::from_bits(0xc294b966), f32::from_bits(0x426593a9)),
        (f32::from_bits(0xc2ba8c7c), f32::from_bits(0x4131f51c)),
        (f32::from_bits(0xc29ad8fe), f32::from_bits(0xc1ef45cd)),
    );
    path.line_to((f32::from_bits(0xc25fe048), f32::from_bits(0xc1acf7d7)));
    path.cubic_to(
        (f32::from_bits(0xc286dac7), f32::from_bits(0x4100a4f0)),
        (f32::from_bits(0xc25705ec), f32::from_bits(0x4225f597)),
        (f32::from_bits(0xc1c66aa8), f32::from_bits(0x425a891e)),
    );
    path.cubic_to(
        (f32::from_bits(0x4084da24), f32::from_bits(0x42878e54)),
        (f32::from_bits(0x4219539e), f32::from_bits(0x426034bf)),
        (f32::from_bits(0x425478e7), f32::from_bits(0x41df3571)),
    );
    path.line_to((f32::from_bits(0x4292f5c1), f32::from_bits(0x421a62d6)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6764-L6788 (chrome/m156)
fn battleOp247(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41d91350), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x425238e3), f32::from_bits(0xc28b791f)),
        (f32::from_bits(0x428827e4), f32::from_bits(0xc23dec02)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a73357), f32::from_bits(0xc1c9cb8b)),
        (f32::from_bits(0x42ae86ff), f32::from_bits(0x404daf5b)),
        (f32::from_bits(0x429bc6e8), f32::from_bits(0x41e56ae9)),
    );
    path.line_to((f32::from_bits(0x42613841), f32::from_bits(0x41a5d816)));
    path.cubic_to(
        (f32::from_bits(0x427c5425), f32::from_bits(0x4014b024)),
        (f32::from_bits(0x4271bc5c), f32::from_bits(0xc191e03e)),
        (f32::from_bits(0x4244da12), f32::from_bits(0xc2094aff)),
    );
    path.cubic_to(
        (f32::from_bits(0x4217f7c8), f32::from_bits(0xc249a5df)),
        (f32::from_bits(0x419cec09), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x429bc6e9), f32::from_bits(0x41e56aeb)));
    path.cubic_to(
        (f32::from_bits(0x429818bd), f32::from_bits(0x4206b36a)),
        (f32::from_bits(0x42937671), f32::from_bits(0x4219f01e)),
        (f32::from_bits(0x428df070), f32::from_bits(0x422c2771)),
    );
    path.line_to((f32::from_bits(0x424d369d), f32::from_bits(0x41f8e5bf)));
    path.cubic_to(
        (f32::from_bits(0x425532f6), f32::from_bits(0x41de8f99)),
        (f32::from_bits(0x425be616), f32::from_bits(0x41c2bf8b)),
        (f32::from_bits(0x42613843), f32::from_bits(0x41a5d816)),
    );
    path.line_to((f32::from_bits(0x429bc6e9), f32::from_bits(0x41e56aeb)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6791-L6820 (chrome/m156)
fn battleOp248(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb630015b), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41d91350), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x425238e3), f32::from_bits(0xc28b791f)),
        (f32::from_bits(0x428827e4), f32::from_bits(0xc23dec02)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a73357), f32::from_bits(0xc1c9cb8b)),
        (f32::from_bits(0x42ae86ff), f32::from_bits(0x404daf5b)),
        (f32::from_bits(0x429bc6e9), f32::from_bits(0x41e56aeb)),
    );
    path.cubic_to(
        (f32::from_bits(0x429818bd), f32::from_bits(0x4206b36a)),
        (f32::from_bits(0x42937671), f32::from_bits(0x4219f01e)),
        (f32::from_bits(0x428df070), f32::from_bits(0x422c2771)),
    );
    path.line_to((f32::from_bits(0x424d369d), f32::from_bits(0x41f8e5bf)));
    path.cubic_to(
        (f32::from_bits(0x425532f6), f32::from_bits(0x41de8f99)),
        (f32::from_bits(0x425be616), f32::from_bits(0x41c2bf8b)),
        (f32::from_bits(0x42613843), f32::from_bits(0x41a5d816)),
    );
    path.line_to((f32::from_bits(0x42613841), f32::from_bits(0x41a5d816)));
    path.cubic_to(
        (f32::from_bits(0x427c5425), f32::from_bits(0x4014b024)),
        (f32::from_bits(0x4271bc5c), f32::from_bits(0xc191e03e)),
        (f32::from_bits(0x4244da12), f32::from_bits(0xc2094aff)),
    );
    path.cubic_to(
        (f32::from_bits(0x4217f7c8), f32::from_bits(0xc249a5df)),
        (f32::from_bits(0x419cec09), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x428df071), f32::from_bits(0x422c2771)));
    path.cubic_to(
        (f32::from_bits(0x423d9ebb), f32::from_bits(0x42a3ca6a)),
        (f32::from_bits(0xc041a78f), f32::from_bits(0x42bd279e)),
        (f32::from_bits(0xc228abe7), f32::from_bits(0x428efaad)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2a29eac), f32::from_bits(0x42419b78)),
        (f32::from_bits(0xc2bd3710), f32::from_bits(0xbfef63d4)),
        (f32::from_bits(0xc2900003), f32::from_bits(0xc2252a98)),
    );
    path.line_to((f32::from_bits(0xc250315d), f32::from_bits(0xc1eecb7c)));
    path.cubic_to(
        (f32::from_bits(0xc288c864), f32::from_bits(0xbfad0c79)),
        (f32::from_bits(0xc26b1d6b), f32::from_bits(0x420bf56b)),
        (f32::from_bits(0xc1f3dd5d), f32::from_bits(0x424eb80d)),
    );
    path.cubic_to(
        (f32::from_bits(0xc00bff34), f32::from_bits(0x4288bd57)),
        (f32::from_bits(0x4209134e), f32::from_bits(0x426ccea7)),
        (f32::from_bits(0x424d369e), f32::from_bits(0x41f8e5bd)),
    );
    path.line_to((f32::from_bits(0x428df071), f32::from_bits(0x422c2771)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6823-L6847 (chrome/m156)
fn battleOp249(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41df6bc7), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4257ee8b), f32::from_bits(0xc289e8f6)),
        (f32::from_bits(0x428aab73), f32::from_bits(0xc2368066)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a95fa1), f32::from_bits(0xc1b25dc1)),
        (f32::from_bits(0x42ae8dc1), f32::from_bits(0x40e61789)),
        (f32::from_bits(0x42987459), f32::from_bits(0x42035b41)),
    );
    path.line_to((f32::from_bits(0x425c6a87), f32::from_bits(0x41bde9b7)));
    path.cubic_to(
        (f32::from_bits(0x427c5dea), f32::from_bits(0x40a654db)),
        (f32::from_bits(0x4274e0a0), f32::from_bits(0xc180f082)),
        (f32::from_bits(0x42487c82), f32::from_bits(0xc203edca)),
    );
    path.cubic_to(
        (f32::from_bits(0x421c1865), f32::from_bits(0xc2476353)),
        (f32::from_bits(0x41a18256), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb69400ae), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42987459), f32::from_bits(0x42035b41)));
    path.cubic_to(
        (f32::from_bits(0x42941f1a), f32::from_bits(0x421778e1)),
        (f32::from_bits(0x428ecdc9), f32::from_bits(0x422aae55)),
        (f32::from_bits(0x42889449), f32::from_bits(0x423cb3b9)),
    );
    path.line_to((f32::from_bits(0x424576c5), f32::from_bits(0x4208693e)));
    path.cubic_to(
        (f32::from_bits(0x424e76a2), f32::from_bits(0x41f6c488)),
        (f32::from_bits(0x425626ce), f32::from_bits(0x41dafef6)),
        (f32::from_bits(0x425c6a88), f32::from_bits(0x41bde9b8)),
    );
    path.line_to((f32::from_bits(0x42987459), f32::from_bits(0x42035b41)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6850-L6880 (chrome/m156)
fn battleOp250(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb69400ae), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41df6bc7), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4257ee8b), f32::from_bits(0xc289e8f6)),
        (f32::from_bits(0x428aab73), f32::from_bits(0xc2368066)),
    );
    path.cubic_to(
        (f32::from_bits(0x42a95fa1), f32::from_bits(0xc1b25dc1)),
        (f32::from_bits(0x42ae8dc1), f32::from_bits(0x40e61789)),
        (f32::from_bits(0x42987459), f32::from_bits(0x42035b41)),
    );
    path.cubic_to(
        (f32::from_bits(0x42941f1a), f32::from_bits(0x421778e1)),
        (f32::from_bits(0x428ecdc9), f32::from_bits(0x422aae55)),
        (f32::from_bits(0x42889449), f32::from_bits(0x423cb3b9)),
    );
    path.line_to((f32::from_bits(0x424576c5), f32::from_bits(0x4208693e)));
    path.cubic_to(
        (f32::from_bits(0x424e76a2), f32::from_bits(0x41f6c488)),
        (f32::from_bits(0x425626ce), f32::from_bits(0x41dafef6)),
        (f32::from_bits(0x425c6a87), f32::from_bits(0x41bde9b7)),
    );
    path.cubic_to(
        (f32::from_bits(0x427c5dea), f32::from_bits(0x40a654db)),
        (f32::from_bits(0x4274e0a0), f32::from_bits(0xc180f082)),
        (f32::from_bits(0x42487c82), f32::from_bits(0xc203edca)),
    );
    path.cubic_to(
        (f32::from_bits(0x421c1865), f32::from_bits(0xc2476353)),
        (f32::from_bits(0x41a18256), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb69400ae), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42889449), f32::from_bits(0x423cb3b8)));
    path.cubic_to(
        (f32::from_bits(0x424c5291), f32::from_bits(0x42902c61)),
        (f32::from_bits(0x41ad609d), f32::from_bits(0x42ab4d26)),
        (f32::from_bits(0xc1072a9c), f32::from_bits(0x42a52356)),
    );
    path.cubic_to(
        (f32::from_bits(0xc21a459c), f32::from_bits(0x429ef985)),
        (f32::from_bits(0xc2813d9b), f32::from_bits(0x4270fef6)),
        (f32::from_bits(0xc298db30), f32::from_bits(0x420179e4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2b078c6), f32::from_bits(0x408fa686)),
        (f32::from_bits(0xc2a7d9d7), f32::from_bits(0xc1dcde62)),
        (f32::from_bits(0xc2825c7e), f32::from_bits(0xc24d8ae0)),
    );
    path.line_to((f32::from_bits(0xc23c7965), f32::from_bits(0xc21495bd)));
    path.cubic_to(
        (f32::from_bits(0xc272ad07), f32::from_bits(0xc19fa9fe)),
        (f32::from_bits(0xc27f23bc), f32::from_bits(0x404faf9e)),
        (f32::from_bits(0xc25cff22), f32::from_bits(0x41bb31a8)),
    );
    path.cubic_to(
        (f32::from_bits(0xc23ada86), f32::from_bits(0x422e36b1)),
        (f32::from_bits(0xc1df0b0c), f32::from_bits(0x4265d7b2)),
        (f32::from_bits(0xc0c36b6f), f32::from_bits(0x426ec0e0)),
    );
    path.cubic_to(
        (f32::from_bits(0x417aaa9e), f32::from_bits(0x4277aa0e)),
        (f32::from_bits(0x4213b3f9), f32::from_bits(0x42507175)),
        (f32::from_bits(0x424576c8), f32::from_bits(0x4208693c)),
    );
    path.line_to((f32::from_bits(0x42889449), f32::from_bits(0x423cb3b8)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6883-L6907 (chrome/m156)
fn battleOp251(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41e529f0), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x425d10b2), f32::from_bits(0xc2887541)),
        (f32::from_bits(0x428cd9cf), f32::from_bits(0xc22fb184)),
    );
    path.cubic_to(
        (f32::from_bits(0x42ab2b45), f32::from_bits(0xc19cf10c)),
        (f32::from_bits(0x42ae472d), f32::from_bits(0x412c96c0)),
        (f32::from_bits(0x42951360), f32::from_bits(0x42120c0d)),
    );
    path.line_to((f32::from_bits(0x425787f7), f32::from_bits(0x41d32707)));
    path.cubic_to(
        (f32::from_bits(0x427bf7e0), f32::from_bits(0x40f986c2)),
        (f32::from_bits(0x4277792b), f32::from_bits(0xc162e746)),
        (f32::from_bits(0x424ba3c8), f32::from_bits(0xc1fe03ba)),
    );
    path.cubic_to(
        (f32::from_bits(0x421fce66), f32::from_bits(0xc24549e8)),
        (f32::from_bits(0x41a5a922), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3725ffa9), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42951360), f32::from_bits(0x42120c0f)));
    path.cubic_to(
        (f32::from_bits(0x429023a5), f32::from_bits(0x422633cd)),
        (f32::from_bits(0x428a3193), f32::from_bits(0x42394df4)),
        (f32::from_bits(0x42835484), f32::from_bits(0x424b0f7e)),
    );
    path.line_to((f32::from_bits(0x423ddffa), f32::from_bits(0x4212ca6e)));
    path.cubic_to(
        (f32::from_bits(0x4247cc4f), f32::from_bits(0x4205f480)),
        (f32::from_bits(0x425064e4), f32::from_bits(0x41f04ae6)),
        (f32::from_bits(0x425787f8), f32::from_bits(0x41d32708)),
    );
    path.line_to((f32::from_bits(0x42951360), f32::from_bits(0x42120c0f)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6910-L6940 (chrome/m156)
fn battleOp252(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3725ffa9), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41e529f0), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x425d10b2), f32::from_bits(0xc2887541)),
        (f32::from_bits(0x428cd9cf), f32::from_bits(0xc22fb184)),
    );
    path.cubic_to(
        (f32::from_bits(0x42ab2b45), f32::from_bits(0xc19cf10c)),
        (f32::from_bits(0x42ae472d), f32::from_bits(0x412c96c0)),
        (f32::from_bits(0x42951360), f32::from_bits(0x42120c0f)),
    );
    path.cubic_to(
        (f32::from_bits(0x429023a5), f32::from_bits(0x422633cd)),
        (f32::from_bits(0x428a3193), f32::from_bits(0x42394df4)),
        (f32::from_bits(0x42835484), f32::from_bits(0x424b0f7e)),
    );
    path.line_to((f32::from_bits(0x423ddffa), f32::from_bits(0x4212ca6e)));
    path.cubic_to(
        (f32::from_bits(0x4247cc4f), f32::from_bits(0x4205f480)),
        (f32::from_bits(0x425064e4), f32::from_bits(0x41f04ae6)),
        (f32::from_bits(0x425787f7), f32::from_bits(0x41d32707)),
    );
    path.cubic_to(
        (f32::from_bits(0x427bf7e0), f32::from_bits(0x40f986c2)),
        (f32::from_bits(0x4277792b), f32::from_bits(0xc162e746)),
        (f32::from_bits(0x424ba3c8), f32::from_bits(0xc1fe03ba)),
    );
    path.cubic_to(
        (f32::from_bits(0x421fce66), f32::from_bits(0xc24549e8)),
        (f32::from_bits(0x41a5a922), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3725ffa9), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42835484), f32::from_bits(0x424b0f7e)));
    path.cubic_to(
        (f32::from_bits(0x423aab34), f32::from_bits(0x4296ad9b)),
        (f32::from_bits(0x41789cf4), f32::from_bits(0x42ae7f70)),
        (f32::from_bits(0xc1702bd2), f32::from_bits(0x42a3434e)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2363d27), f32::from_bits(0x4298072c)),
        (f32::from_bits(0xc28cd4c4), f32::from_bits(0x42573cf7)),
        (f32::from_bits(0xc29edb8e), f32::from_bits(0x41c0adb0)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2b0e257), f32::from_bits(0xc0b47a14)),
        (f32::from_bits(0xc2a03550), f32::from_bits(0xc217a35b)),
        (f32::from_bits(0xc2674746), f32::from_bits(0xc26e3089)),
    );
    path.line_to((f32::from_bits(0xc2273070), f32::from_bits(0xc22c2f6e)));
    path.cubic_to(
        (f32::from_bits(0xc267a050), f32::from_bits(0xc1db3c5e)),
        (f32::from_bits(0xc27fbc5f), f32::from_bits(0xc0827737)),
        (f32::from_bits(0xc265ac62), f32::from_bits(0x418b490c)),
    );
    path.cubic_to(
        (f32::from_bits(0xc24b9c64), f32::from_bits(0x421b97f2)),
        (f32::from_bits(0xc203bd1c), f32::from_bits(0x425bcc95)),
        (f32::from_bits(0xc12d9e08), f32::from_bits(0x426c0adc)),
    );
    path.cubic_to(
        (f32::from_bits(0x4133b85e), f32::from_bits(0x427c4921)),
        (f32::from_bits(0x4206f0f2), f32::from_bits(0x4259d90a)),
        (f32::from_bits(0x423ddff7), f32::from_bits(0x4212ca73)),
    );
    path.line_to((f32::from_bits(0x42835484), f32::from_bits(0x424b0f7e)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6943-L6967 (chrome/m156)
fn battleOp253(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41ea9e19), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4261e8db), f32::from_bits(0xc2870be6)),
        (f32::from_bits(0x428ed6bc), f32::from_bits(0xc22926d7)),
    );
    path.cubic_to(
        (f32::from_bits(0x42acb90a), f32::from_bits(0xc1886bc1)),
        (f32::from_bits(0x42adc0f7), f32::from_bits(0x41631db6)),
        (f32::from_bits(0x42918cff), f32::from_bits(0x421fa302)),
    );
    path.line_to((f32::from_bits(0x42526f53), f32::from_bits(0x41e6ccd4)));
    path.cubic_to(
        (f32::from_bits(0x427b35d6), f32::from_bits(0x41242e26)),
        (f32::from_bits(0x4279b842), f32::from_bits(0xc1453c2f)),
        (f32::from_bits(0x424e8393), f32::from_bits(0xc1f48e84)),
    );
    path.cubic_to(
        (f32::from_bits(0x42234ee4), f32::from_bits(0xc2433f78)),
        (f32::from_bits(0x41a99a66), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42918d00), f32::from_bits(0x421fa301)));
    path.cubic_to(
        (f32::from_bits(0x428c0830), f32::from_bits(0x4233c399)),
        (f32::from_bits(0x42857bfe), f32::from_bits(0x4246b13f)),
        (f32::from_bits(0x427c06a0), f32::from_bits(0x42581e30)),
    );
    path.line_to((f32::from_bits(0x42362ff8), f32::from_bits(0x421c3ad6)));
    path.cubic_to(
        (f32::from_bits(0x4240fd4a), f32::from_bits(0x420fa210)),
        (f32::from_bits(0x424a74b5), f32::from_bits(0x4201f32f)),
        (f32::from_bits(0x42526f54), f32::from_bits(0x41e6ccd5)),
    );
    path.line_to((f32::from_bits(0x42918d00), f32::from_bits(0x421fa301)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L6970-L7000 (chrome/m156)
fn battleOp254(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41ea9e19), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4261e8db), f32::from_bits(0xc2870be6)),
        (f32::from_bits(0x428ed6bc), f32::from_bits(0xc22926d7)),
    );
    path.cubic_to(
        (f32::from_bits(0x42acb90a), f32::from_bits(0xc1886bc1)),
        (f32::from_bits(0x42adc0f7), f32::from_bits(0x41631db6)),
        (f32::from_bits(0x42918d00), f32::from_bits(0x421fa301)),
    );
    path.cubic_to(
        (f32::from_bits(0x428c0830), f32::from_bits(0x4233c399)),
        (f32::from_bits(0x42857bfe), f32::from_bits(0x4246b13f)),
        (f32::from_bits(0x427c06a0), f32::from_bits(0x42581e30)),
    );
    path.line_to((f32::from_bits(0x42362ff8), f32::from_bits(0x421c3ad6)));
    path.cubic_to(
        (f32::from_bits(0x4240fd4a), f32::from_bits(0x420fa210)),
        (f32::from_bits(0x424a74b5), f32::from_bits(0x4201f32f)),
        (f32::from_bits(0x42526f53), f32::from_bits(0x41e6ccd4)),
    );
    path.cubic_to(
        (f32::from_bits(0x427b35d6), f32::from_bits(0x41242e26)),
        (f32::from_bits(0x4279b842), f32::from_bits(0xc1453c2f)),
        (f32::from_bits(0x424e8393), f32::from_bits(0xc1f48e84)),
    );
    path.cubic_to(
        (f32::from_bits(0x42234ee4), f32::from_bits(0xc2433f78)),
        (f32::from_bits(0x41a99a66), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x427c069f), f32::from_bits(0x42581e31)));
    path.cubic_to(
        (f32::from_bits(0x4229355f), f32::from_bits(0x429c5901)),
        (f32::from_bits(0x4119ef9b), f32::from_bits(0x42b0b9f6)),
        (f32::from_bits(0xc1a91754), f32::from_bits(0x42a086fc)),
    );
    path.cubic_to(
        (f32::from_bits(0xc24f933a), f32::from_bits(0x42905402)),
        (f32::from_bits(0xc296a2af), f32::from_bits(0x423cccf9)),
        (f32::from_bits(0xc2a2e3f0), f32::from_bits(0x417fd713)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2af2532), f32::from_bits(0xc17385be)),
        (f32::from_bits(0xc296a6d5), f32::from_bits(0xc23cbfbd)),
        (f32::from_bits(0xc247a7c9), f32::from_bits(0xc284a101)),
    );
    path.line_to((f32::from_bits(0xc210544b), f32::from_bits(0xc23fc0ab)));
    path.cubic_to(
        (f32::from_bits(0xc259cf4c), f32::from_bits(0xc20871e9)),
        (f32::from_bits(0xc27d38da), f32::from_bits(0xc1300a36)),
        (f32::from_bits(0xc26b810f), f32::from_bits(0x4138f1f1)),
    );
    path.cubic_to(
        (f32::from_bits(0xc259c944), f32::from_bits(0x42087b85)),
        (f32::from_bits(0xc2160de3), f32::from_bits(0x4250aad1)),
        (f32::from_bits(0xc174780b), f32::from_bits(0x42681670)),
    );
    path.cubic_to(
        (f32::from_bits(0x40de8efd), f32::from_bits(0x427f820e)),
        (f32::from_bits(0x41f4a392), f32::from_bits(0x42620b79)),
        (f32::from_bits(0x42362ffc), f32::from_bits(0x421c3ad2)),
    );
    path.line_to((f32::from_bits(0x427c069f), f32::from_bits(0x42581e31)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7003-L7027 (chrome/m156)
fn battleOp255(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41eeb164), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42658277), f32::from_bits(0xc285f892)),
        (f32::from_bits(0x42904565), f32::from_bits(0xc22437b5)),
    );
    path.cubic_to(
        (f32::from_bits(0x42adc98d), f32::from_bits(0xc171f916)),
        (f32::from_bits(0x42ad3226), f32::from_bits(0x4185deb6)),
        (f32::from_bits(0x428eb8d5), f32::from_bits(0x42298bae)),
    );
    path.line_to((f32::from_bits(0x424e5857), f32::from_bits(0x41f5204e)));
    path.cubic_to(
        (f32::from_bits(0x427a675d), f32::from_bits(0x41418c03)),
        (f32::from_bits(0x427b4242), f32::from_bits(0xc12eeb9a)),
        (f32::from_bits(0x425095b0), f32::from_bits(0xc1ed6c50)),
    );
    path.cubic_to(
        (f32::from_bits(0x4225e91e), f32::from_bits(0xc241b169)),
        (f32::from_bits(0x41ac8c92), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb69400ae), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x428eb8d5), f32::from_bits(0x42298bad)));
    path.cubic_to(
        (f32::from_bits(0x4288c365), f32::from_bits(0x423d9c15)),
        (f32::from_bits(0x4281c36f), f32::from_bits(0x42505c7e)),
        (f32::from_bits(0x4273ad50), f32::from_bits(0x42617d52)),
    );
    path.line_to((f32::from_bits(0x423026ec), f32::from_bits(0x42230126)));
    path.cubic_to(
        (f32::from_bits(0x423b9c18), f32::from_bits(0x42169f65)),
        (f32::from_bits(0x4245bae4), f32::from_bits(0x42091136)),
        (f32::from_bits(0x424e5858), f32::from_bits(0x41f5204d)),
    );
    path.line_to((f32::from_bits(0x428eb8d5), f32::from_bits(0x42298bad)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7030-L7061 (chrome/m156)
fn battleOp256(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb69400ae), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41eeb164), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42658277), f32::from_bits(0xc285f892)),
        (f32::from_bits(0x42904565), f32::from_bits(0xc22437b5)),
    );
    path.cubic_to(
        (f32::from_bits(0x42adc98d), f32::from_bits(0xc171f917)),
        (f32::from_bits(0x42ad3226), f32::from_bits(0x4185deb4)),
        (f32::from_bits(0x428eb8d5), f32::from_bits(0x42298bad)),
    );
    path.line_to((f32::from_bits(0x428eb8d5), f32::from_bits(0x42298bae)));
    path.cubic_to(
        (f32::from_bits(0x4288c365), f32::from_bits(0x423d9c16)),
        (f32::from_bits(0x4281c36f), f32::from_bits(0x42505c7e)),
        (f32::from_bits(0x4273ad50), f32::from_bits(0x42617d52)),
    );
    path.line_to((f32::from_bits(0x423026ec), f32::from_bits(0x42230126)));
    path.cubic_to(
        (f32::from_bits(0x423b9c18), f32::from_bits(0x42169f65)),
        (f32::from_bits(0x4245bae4), f32::from_bits(0x42091136)),
        (f32::from_bits(0x424e5858), f32::from_bits(0x41f5204d)),
    );
    path.cubic_to(
        (f32::from_bits(0x427a675e), f32::from_bits(0x41418c02)),
        (f32::from_bits(0x427b4242), f32::from_bits(0xc12eeb9b)),
        (f32::from_bits(0x425095b0), f32::from_bits(0xc1ed6c50)),
    );
    path.cubic_to(
        (f32::from_bits(0x4225e91e), f32::from_bits(0xc241b169)),
        (f32::from_bits(0x41ac8c92), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xb69400ae), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4273ad4f), f32::from_bits(0x42617d52)));
    path.cubic_to(
        (f32::from_bits(0x421bc173), f32::from_bits(0x42a0404f)),
        (f32::from_bits(0x40a50405), f32::from_bits(0x42b1dfaa)),
        (f32::from_bits(0xc1cd0022), f32::from_bits(0x429de3fd)),
    );
    path.cubic_to(
        (f32::from_bits(0xc261a0a2), f32::from_bits(0x4289e850)),
        (f32::from_bits(0xc29d25ee), f32::from_bits(0x4227ed4e)),
        (f32::from_bits(0xc2a4d3d8), f32::from_bits(0x411d8f80)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2ac81c3), f32::from_bits(0xc1b24b1c)),
        (f32::from_bits(0xc28e216c), f32::from_bits(0xc256e38c)),
        (f32::from_bits(0xc22e0453), f32::from_bits(0xc28d5ec3)),
    );
    path.line_to((f32::from_bits(0xc1fb9743), f32::from_bits(0xc24c63fd)));
    path.cubic_to(
        (f32::from_bits(0xc24d7d6b), f32::from_bits(0xc21b575f)),
        (f32::from_bits(0xc279684a), f32::from_bits(0xc180e302)),
        (f32::from_bits(0xc26e4dff), f32::from_bits(0x40e3cc4e)),
    );
    path.cubic_to(
        (f32::from_bits(0xc26333b4), f32::from_bits(0x41f2c929)),
        (f32::from_bits(0xc2231aa4), f32::from_bits(0x42476256)),
        (f32::from_bits(0xc1943166), f32::from_bits(0x4264467e)),
    );
    path.cubic_to(
        (f32::from_bits(0x406e93d1), f32::from_bits(0x42809553)),
        (f32::from_bits(0x41e1305a), f32::from_bits(0x4267b03c)),
        (f32::from_bits(0x423026ed), f32::from_bits(0x42230127)),
    );
    path.line_to((f32::from_bits(0x4273ad4f), f32::from_bits(0x42617d52)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7064-L7088 (chrome/m156)
fn battleOp257(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f2d268), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x426923a2), f32::from_bits(0xc284dd06)),
        (f32::from_bits(0x4291aced), f32::from_bits(0xc21f2e53)),
    );
    path.cubic_to(
        (f32::from_bits(0x42aec809), f32::from_bits(0xc1528a66)),
        (f32::from_bits(0x42ac7c90), f32::from_bits(0x419a60b1)),
        (f32::from_bits(0x428bb0fe), f32::from_bits(0x42335ba0)),
    );
    path.line_to((f32::from_bits(0x4249f6a4), f32::from_bits(0x4201a806)));
    path.cubic_to(
        (f32::from_bits(0x427960d2), f32::from_bits(0x415f325f)),
        (f32::from_bits(0x427cb22e), f32::from_bits(0xc11832b1)),
        (f32::from_bits(0x42529d7e), f32::from_bits(0xc1e62422)),
    );
    path.cubic_to(
        (f32::from_bits(0x422888ce), f32::from_bits(0xc2401775)),
        (f32::from_bits(0x41af88b3), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36d3ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x428bb0ff), f32::from_bits(0x42335ba2)));
    path.cubic_to(
        (f32::from_bits(0x4285489d), f32::from_bits(0x42475206)),
        (f32::from_bits(0x427ba631), f32::from_bits(0x4259da14)),
        (f32::from_bits(0x426ae250), f32::from_bits(0x426aa282)),
    );
    path.line_to((f32::from_bits(0x4229cbb3), f32::from_bits(0x42299d92)));
    path.cubic_to(
        (f32::from_bits(0x4235ea43), f32::from_bits(0x421d7bb7)),
        (f32::from_bits(0x4240b302), f32::from_bits(0x42101649)),
        (f32::from_bits(0x4249f6a5), f32::from_bits(0x4201a807)),
    );
    path.line_to((f32::from_bits(0x428bb0ff), f32::from_bits(0x42335ba2)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7091-L7121 (chrome/m156)
fn battleOp258(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x36d3ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f2d268), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x426923a2), f32::from_bits(0xc284dd06)),
        (f32::from_bits(0x4291aced), f32::from_bits(0xc21f2e53)),
    );
    path.cubic_to(
        (f32::from_bits(0x42aec809), f32::from_bits(0xc1528a66)),
        (f32::from_bits(0x42ac7c90), f32::from_bits(0x419a60b1)),
        (f32::from_bits(0x428bb0ff), f32::from_bits(0x42335ba2)),
    );
    path.cubic_to(
        (f32::from_bits(0x4285489d), f32::from_bits(0x42475206)),
        (f32::from_bits(0x427ba631), f32::from_bits(0x4259da14)),
        (f32::from_bits(0x426ae250), f32::from_bits(0x426aa282)),
    );
    path.line_to((f32::from_bits(0x4229cbb3), f32::from_bits(0x42299d92)));
    path.cubic_to(
        (f32::from_bits(0x4235ea43), f32::from_bits(0x421d7bb7)),
        (f32::from_bits(0x4240b302), f32::from_bits(0x42101649)),
        (f32::from_bits(0x4249f6a4), f32::from_bits(0x4201a806)),
    );
    path.cubic_to(
        (f32::from_bits(0x427960d2), f32::from_bits(0x415f325f)),
        (f32::from_bits(0x427cb22e), f32::from_bits(0xc11832b1)),
        (f32::from_bits(0x42529d7e), f32::from_bits(0xc1e62422)),
    );
    path.cubic_to(
        (f32::from_bits(0x422888ce), f32::from_bits(0xc2401775)),
        (f32::from_bits(0x41af88b3), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36d3ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x426ae251), f32::from_bits(0x426aa281)));
    path.cubic_to(
        (f32::from_bits(0x420dcd2c), f32::from_bits(0x42a3e87c)),
        (f32::from_bits(0x3f1c0197), f32::from_bits(0x42b294d6)),
        (f32::from_bits(0xc1f0a2ab), f32::from_bits(0x429ab731)),
    );
    path.cubic_to(
        (f32::from_bits(0xc27312b1), f32::from_bits(0x4282d98e)),
        (f32::from_bits(0xc2a300b1), f32::from_bits(0x4211eaa7)),
        (f32::from_bits(0xc2a5d865), f32::from_bits(0x40654aaf)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2a8b018), f32::from_bits(0xc1ea82a2)),
        (f32::from_bits(0xc2845e8a), f32::from_bits(0xc26fc272)),
        (f32::from_bits(0xc2128ebb), f32::from_bits(0xc294f34d)),
    );
    path.line_to((f32::from_bits(0xc1d3e3ef), f32::from_bits(0xc2575999)));
    path.cubic_to(
        (f32::from_bits(0xc23f6093), f32::from_bits(0xc22d51f6)),
        (f32::from_bits(0xc273e2d0), f32::from_bits(0xc1a9868a)),
        (f32::from_bits(0xc26fc6b5), f32::from_bits(0x4025c090)),
    );
    path.cubic_to(
        (f32::from_bits(0xc26baa9a), f32::from_bits(0x41d2f6ae)),
        (f32::from_bits(0xc22fb71e), f32::from_bits(0x423d2e2a)),
        (f32::from_bits(0xc1adf403), f32::from_bits(0x425faf61)),
    );
    path.cubic_to(
        (f32::from_bits(0x3ee18e9e), f32::from_bits(0x4281184d)),
        (f32::from_bits(0x41cd03a3), f32::from_bits(0x426cf9bf)),
        (f32::from_bits(0x4229cbb7), f32::from_bits(0x42299d90)),
    );
    path.line_to((f32::from_bits(0x426ae251), f32::from_bits(0x426aa281)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7124-L7148 (chrome/m156)
fn battleOp259(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f70d18), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x426cd682), f32::from_bits(0xc283b5d2)),
        (f32::from_bits(0x429310ae), f32::from_bits(0xc219fc22)),
    );
    path.cubic_to(
        (f32::from_bits(0x42afb61c), f32::from_bits(0xc132327f)),
        (f32::from_bits(0x42ab9c4e), f32::from_bits(0x41af4ab2)),
        (f32::from_bits(0x42886baa), f32::from_bits(0x423d2918)),
    );
    path.line_to((f32::from_bits(0x42453c0d), f32::from_bits(0x4208be17)));
    path.cubic_to(
        (f32::from_bits(0x42781c98), f32::from_bits(0x417d6f0f)),
        (f32::from_bits(0x427e0a5e), f32::from_bits(0xc100d142)),
        (f32::from_bits(0x42549fd3), f32::from_bits(0xc1dea0fa)),
    );
    path.cubic_to(
        (f32::from_bits(0x422b3547), f32::from_bits(0xc23e6ca9)),
        (f32::from_bits(0x41b29756), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42886bab), f32::from_bits(0x423d2917)));
    path.cubic_to(
        (f32::from_bits(0x42818ce6), f32::from_bits(0x4250fab6)),
        (f32::from_bits(0x42733ded), f32::from_bits(0x42633df9)),
        (f32::from_bits(0x42618b96), f32::from_bits(0x4273a01b)),
    );
    path.line_to((f32::from_bits(0x42230b75), f32::from_bits(0x42301d61)));
    path.cubic_to(
        (f32::from_bits(0x422fd668), f32::from_bits(0x4224457a)),
        (f32::from_bits(0x423b4d41), f32::from_bits(0x421711c6)),
        (f32::from_bits(0x42453c0e), f32::from_bits(0x4208be17)),
    );
    path.line_to((f32::from_bits(0x42886bab), f32::from_bits(0x423d2917)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7151-L7181 (chrome/m156)
fn battleOp260(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb630015b), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f70d18), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x426cd682), f32::from_bits(0xc283b5d2)),
        (f32::from_bits(0x429310ae), f32::from_bits(0xc219fc22)),
    );
    path.cubic_to(
        (f32::from_bits(0x42afb61c), f32::from_bits(0xc132327f)),
        (f32::from_bits(0x42ab9c4e), f32::from_bits(0x41af4ab2)),
        (f32::from_bits(0x42886bab), f32::from_bits(0x423d2917)),
    );
    path.cubic_to(
        (f32::from_bits(0x42818ce6), f32::from_bits(0x4250fab6)),
        (f32::from_bits(0x42733ded), f32::from_bits(0x42633df9)),
        (f32::from_bits(0x42618b96), f32::from_bits(0x4273a01b)),
    );
    path.line_to((f32::from_bits(0x42230b75), f32::from_bits(0x42301d61)));
    path.cubic_to(
        (f32::from_bits(0x422fd668), f32::from_bits(0x4224457a)),
        (f32::from_bits(0x423b4d41), f32::from_bits(0x421711c6)),
        (f32::from_bits(0x42453c0d), f32::from_bits(0x4208be17)),
    );
    path.cubic_to(
        (f32::from_bits(0x42781c98), f32::from_bits(0x417d6f0f)),
        (f32::from_bits(0x427e0a5e), f32::from_bits(0xc100d142)),
        (f32::from_bits(0x42549fd3), f32::from_bits(0xc1dea0fa)),
    );
    path.cubic_to(
        (f32::from_bits(0x422b3547), f32::from_bits(0xc23e6ca9)),
        (f32::from_bits(0x41b29756), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42618b95), f32::from_bits(0x4273a01c)));
    path.cubic_to(
        (f32::from_bits(0x41fe659e), f32::from_bits(0x42a75638)),
        (f32::from_bits(0xc081f8cf), f32::from_bits(0x42b2d4b3)),
        (f32::from_bits(0xc20a1eaa), f32::from_bits(0x4296f3e7)),
    );
    path.cubic_to(
        (f32::from_bits(0xc281ff1e), f32::from_bits(0x42762634)),
        (f32::from_bits(0xc2a8320c), f32::from_bits(0x41f52b39)),
        (f32::from_bits(0xc2a5e71e), f32::from_bits(0xc035be80)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2a39c30), f32::from_bits(0xc2114d6a)),
        (f32::from_bits(0xc2728d06), f32::from_bits(0xc283ad37)),
        (f32::from_bits(0xc1ea4cbe), f32::from_bits(0xc29b5279)),
    );
    path.line_to((f32::from_bits(0xc1a95f99), f32::from_bits(0xc2608fe9)));
    path.cubic_to(
        (f32::from_bits(0xc22f5688), f32::from_bits(0xc23e6034)),
        (f32::from_bits(0xc26c8b72), f32::from_bits(0xc1d2135a)),
        (f32::from_bits(0xc26fdc03), f32::from_bits(0xc003615b)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2732c96), f32::from_bits(0x41b13b02)),
        (f32::from_bits(0xc23bf25c), f32::from_bits(0x4231f06e)),
        (f32::from_bits(0xc1c7b0f0), f32::from_bits(0x425a3eb1)),
    );
    path.cubic_to(
        (f32::from_bits(0xc03be91a), f32::from_bits(0x4281467b)),
        (f32::from_bits(0x41b7e6c5), f32::from_bits(0x4271eec4)),
        (f32::from_bits(0x42230b77), f32::from_bits(0x42301d61)),
    );
    path.line_to((f32::from_bits(0x42618b95), f32::from_bits(0x4273a01c)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7184-L7208 (chrome/m156)
fn battleOp261(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f9750b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x426eeefa), f32::from_bits(0xc2830bb8)),
        (f32::from_bits(0x4293d569), f32::from_bits(0xc2170343)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b03354), f32::from_bits(0xc11fbc55)),
        (f32::from_bits(0x42ab0b89), f32::from_bits(0x41bb247a)),
        (f32::from_bits(0x42867c8e), f32::from_bits(0x42429f12)),
    );
    path.line_to((f32::from_bits(0x42427039), f32::from_bits(0x420cb0ae)));
    path.cubic_to(
        (f32::from_bits(0x42774b4a), f32::from_bits(0x418748a6)),
        (f32::from_bits(0x427ebf70), f32::from_bits(0xc0e6f16a)),
        (f32::from_bits(0x4255bc46), f32::from_bits(0xc1da54e8)),
    );
    path.cubic_to(
        (f32::from_bits(0x422cb91b), f32::from_bits(0xc23d76ba)),
        (f32::from_bits(0x41b454a4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3725ffa9), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42867c8e), f32::from_bits(0x42429f13)));
    path.cubic_to(
        (f32::from_bits(0x427eb473), f32::from_bits(0x4256572c)),
        (f32::from_bits(0x426e4fbb), f32::from_bits(0x42686e49)),
        (f32::from_bits(0x425c16a2), f32::from_bits(0x427890ea)),
    );
    path.line_to((f32::from_bits(0x421f199c), f32::from_bits(0x4233afb3)));
    path.cubic_to(
        (f32::from_bits(0x422c45f9), f32::from_bits(0x422805b5)),
        (f32::from_bits(0x42381fbf), f32::from_bits(0x421af1ea)),
        (f32::from_bits(0x4242703a), f32::from_bits(0x420cb0af)),
    );
    path.line_to((f32::from_bits(0x42867c8e), f32::from_bits(0x42429f13)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7211-L7241 (chrome/m156)
fn battleOp262(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3725ffa9), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41f9750b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x426eeefa), f32::from_bits(0xc2830bb8)),
        (f32::from_bits(0x4293d569), f32::from_bits(0xc2170343)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b03354), f32::from_bits(0xc11fbc55)),
        (f32::from_bits(0x42ab0b89), f32::from_bits(0x41bb247a)),
        (f32::from_bits(0x42867c8e), f32::from_bits(0x42429f13)),
    );
    path.cubic_to(
        (f32::from_bits(0x427eb473), f32::from_bits(0x4256572c)),
        (f32::from_bits(0x426e4fbb), f32::from_bits(0x42686e49)),
        (f32::from_bits(0x425c16a2), f32::from_bits(0x427890ea)),
    );
    path.line_to((f32::from_bits(0x421f199c), f32::from_bits(0x4233afb3)));
    path.cubic_to(
        (f32::from_bits(0x422c45f9), f32::from_bits(0x422805b5)),
        (f32::from_bits(0x42381fbf), f32::from_bits(0x421af1ea)),
        (f32::from_bits(0x42427039), f32::from_bits(0x420cb0ae)),
    );
    path.cubic_to(
        (f32::from_bits(0x42774b4a), f32::from_bits(0x418748a6)),
        (f32::from_bits(0x427ebf70), f32::from_bits(0xc0e6f16a)),
        (f32::from_bits(0x4255bc46), f32::from_bits(0xc1da54e8)),
    );
    path.cubic_to(
        (f32::from_bits(0x422cb91b), f32::from_bits(0xc23d76ba)),
        (f32::from_bits(0x41b454a4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3725ffa9), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x425c16a1), f32::from_bits(0x427890eb)));
    path.cubic_to(
        (f32::from_bits(0x41ed85e5), f32::from_bits(0x42a9245e)),
        (f32::from_bits(0xc0d70d9a), f32::from_bits(0x42b2c211)),
        (f32::from_bits(0xc2140612), f32::from_bits(0x42949665)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2869539), f32::from_bits(0x426cd56f)),
        (f32::from_bits(0xc2aac701), f32::from_bits(0x41d9ff9c)),
        (f32::from_bits(0xc2a57e3b), f32::from_bits(0xc0cf6824)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2a03574), f32::from_bits(0xc220d9d7)),
        (f32::from_bits(0xc26501e3), f32::from_bits(0xc289ed78)),
        (f32::from_bits(0xc1c7e516), f32::from_bits(0xc29e4c97)),
    );
    path.line_to((f32::from_bits(0xc190809e), f32::from_bits(0xc264ddc3)));
    path.cubic_to(
        (f32::from_bits(0xc2258c2b), f32::from_bits(0xc24769d4)),
        (f32::from_bits(0xc267a08f), f32::from_bits(0xc1e88e39)),
        (f32::from_bits(0xc26f4461), f32::from_bits(0xc095eec9)),
    );
    path.cubic_to(
        (f32::from_bits(0xc276e835), f32::from_bits(0x419d96da)),
        (f32::from_bits(0xc24293e3), f32::from_bits(0x422b3483)),
        (f32::from_bits(0xc1d60298), f32::from_bits(0x4256d347)),
    );
    path.cubic_to(
        (f32::from_bits(0xc09b75b0), f32::from_bits(0x42813905)),
        (f32::from_bits(0x41abb417), f32::from_bits(0x42748af0)),
        (f32::from_bits(0x421f199e), f32::from_bits(0x4233afb2)),
    );
    path.line_to((f32::from_bits(0x425c16a1), f32::from_bits(0x427890eb)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7244-L7268 (chrome/m156)
fn battleOp263(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41fc38da), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4271556b), f32::from_bits(0xc2824656)),
        (f32::from_bits(0x4294b266), f32::from_bits(0xc213956f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b0ba15), f32::from_bits(0xc10a78c9)),
        (f32::from_bits(0x42aa55de), f32::from_bits(0x41c8b65d)),
        (f32::from_bits(0x42843343), f32::from_bits(0x4248ca15)),
    );
    path.line_to((f32::from_bits(0x423f2206), f32::from_bits(0x42112621)));
    path.cubic_to(
        (f32::from_bits(0x427644a6), f32::from_bits(0x419117e2)),
        (f32::from_bits(0x427f8241), f32::from_bits(0xc0c83353)),
        (f32::from_bits(0x4256fbc4), f32::from_bits(0xc1d55fc8)),
    );
    path.cubic_to(
        (f32::from_bits(0x422e7546), f32::from_bits(0xc23c595d)),
        (f32::from_bits(0x41b6544b), f32::from_bits(0xc2700002)),
        (f32::from_bits(0x357ffa8c), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42843344), f32::from_bits(0x4248ca14)));
    path.cubic_to(
        (f32::from_bits(0x4279865a), f32::from_bits(0x425c60b2)),
        (f32::from_bits(0x426884b7), f32::from_bits(0x426e4097)),
        (f32::from_bits(0x4255b1c1), f32::from_bits(0x427e1584)),
    );
    path.line_to((f32::from_bits(0x421a7a55), f32::from_bits(0x4237acdc)));
    path.cubic_to(
        (f32::from_bits(0x422815ec), f32::from_bits(0x422c3b08)),
        (f32::from_bits(0x42346121), f32::from_bits(0x421f4f28)),
        (f32::from_bits(0x423f2207), f32::from_bits(0x42112621)),
    );
    path.line_to((f32::from_bits(0x42843344), f32::from_bits(0x4248ca14)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7271-L7301 (chrome/m156)
fn battleOp264(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41fc38da), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4271556b), f32::from_bits(0xc2824656)),
        (f32::from_bits(0x4294b266), f32::from_bits(0xc213956f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b0ba15), f32::from_bits(0xc10a78c9)),
        (f32::from_bits(0x42aa55de), f32::from_bits(0x41c8b65d)),
        (f32::from_bits(0x42843344), f32::from_bits(0x4248ca14)),
    );
    path.cubic_to(
        (f32::from_bits(0x4279865a), f32::from_bits(0x425c60b2)),
        (f32::from_bits(0x426884b7), f32::from_bits(0x426e4097)),
        (f32::from_bits(0x4255b1c1), f32::from_bits(0x427e1584)),
    );
    path.line_to((f32::from_bits(0x421a7a55), f32::from_bits(0x4237acdc)));
    path.cubic_to(
        (f32::from_bits(0x422815ec), f32::from_bits(0x422c3b08)),
        (f32::from_bits(0x42346121), f32::from_bits(0x421f4f28)),
        (f32::from_bits(0x423f2206), f32::from_bits(0x42112621)),
    );
    path.cubic_to(
        (f32::from_bits(0x427644a6), f32::from_bits(0x419117e2)),
        (f32::from_bits(0x427f8241), f32::from_bits(0xc0c83353)),
        (f32::from_bits(0x4256fbc4), f32::from_bits(0xc1d55fc8)),
    );
    path.cubic_to(
        (f32::from_bits(0x422e7546), f32::from_bits(0xc23c595d)),
        (f32::from_bits(0x41b6544b), f32::from_bits(0xc2700002)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4255b1c2), f32::from_bits(0x427e1586)));
    path.cubic_to(
        (f32::from_bits(0x41d9eb88), f32::from_bits(0x42ab15b8)),
        (f32::from_bits(0xc11c5ee2), f32::from_bits(0x42b27b8c)),
        (f32::from_bits(0xc21f2fec), f32::from_bits(0x4291ac82)),
    );
    path.cubic_to(
        (f32::from_bits(0xc28ba40f), f32::from_bits(0x4261baf0)),
        (f32::from_bits(0xc2ad6782), f32::from_bits(0x41ba4aab)),
        (f32::from_bits(0xc2a4a120), f32::from_bits(0xc12a4d95)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29bdabd), f32::from_bits(0xc2324c20)),
        (f32::from_bits(0xc254adab), f32::from_bits(0xc290ac19)),
        (f32::from_bits(0xc19fafc0), f32::from_bits(0xc2a120ca)),
    );
    path.line_to((f32::from_bits(0xc166df50), f32::from_bits(0xc268f4ce)));
    path.cubic_to(
        (f32::from_bits(0xc219be54), f32::from_bits(0xc2512a28)),
        (f32::from_bits(0xc26154eb), f32::from_bits(0xc200e3bb)),
        (f32::from_bits(0xc26e04b2), f32::from_bits(0xc0f6387e)),
    );
    path.cubic_to(
        (f32::from_bits(0xc27ab479), f32::from_bits(0x4186ab35)),
        (f32::from_bits(0xc249e3ea), f32::from_bits(0x42232db1)),
        (f32::from_bits(0xc1e62664), f32::from_bits(0x42529ce0)),
    );
    path.cubic_to(
        (f32::from_bits(0xc0e213c9), f32::from_bits(0x42810608)),
        (f32::from_bits(0x419d8860), f32::from_bits(0x427759fd)),
        (f32::from_bits(0x421a7a58), f32::from_bits(0x4237acda)),
    );
    path.line_to((f32::from_bits(0x4255b1c2), f32::from_bits(0x427e1586)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7304-L7328 (chrome/m156)
fn battleOp265(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41fe7454), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x427343e8), f32::from_bits(0xc281a57b)),
        (f32::from_bits(0x429560d9), f32::from_bits(0xc210ce12)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b11fbd), f32::from_bits(0xc0f2896e)),
        (f32::from_bits(0x42a9b750), f32::from_bits(0x41d3a0ba)),
        (f32::from_bits(0x42824e39), f32::from_bits(0x424daf12)),
    );
    path.line_to((f32::from_bits(0x423c64bf), f32::from_bits(0x4214afea)));
    path.cubic_to(
        (f32::from_bits(0x42755f66), f32::from_bits(0x4198fbec)),
        (f32::from_bits(0x42800a9d), f32::from_bits(0xc0af53e2)),
        (f32::from_bits(0x4257f7fc), f32::from_bits(0xc1d15b49)),
    );
    path.cubic_to(
        (f32::from_bits(0x422fdabc), f32::from_bits(0xc23b70cc)),
        (f32::from_bits(0x41b7f168), f32::from_bits(0xc2700002)),
        (f32::from_bits(0xb5600574), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42824e38), f32::from_bits(0x424daf15)));
    path.cubic_to(
        (f32::from_bits(0x42753e9a), f32::from_bits(0x4261276c)),
        (f32::from_bits(0x4263be9a), f32::from_bits(0x4272d73c)),
        (f32::from_bits(0x4250704b), f32::from_bits(0x428134df)),
    );
    path.line_to((f32::from_bits(0x4216adb6), f32::from_bits(0x423acdfc)));
    path.cubic_to(
        (f32::from_bits(0x4224a276), f32::from_bits(0x422f8c2c)),
        (f32::from_bits(0x42314905), f32::from_bits(0x4222c30f)),
        (f32::from_bits(0x423c64c0), f32::from_bits(0x4214afec)),
    );
    path.line_to((f32::from_bits(0x42824e38), f32::from_bits(0x424daf15)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7331-L7363 (chrome/m156)
fn battleOp266(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x41fe7454), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x427343e8), f32::from_bits(0xc281a57b)),
        (f32::from_bits(0x429560d9), f32::from_bits(0xc210ce12)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b11fbd), f32::from_bits(0xc0f2896e)),
        (f32::from_bits(0x42a9b750), f32::from_bits(0x41d3a0ba)),
        (f32::from_bits(0x42824e39), f32::from_bits(0x424daf12)),
    );
    path.line_to((f32::from_bits(0x42824e38), f32::from_bits(0x424daf15)));
    path.cubic_to(
        (f32::from_bits(0x42753e9a), f32::from_bits(0x4261276c)),
        (f32::from_bits(0x4263be9a), f32::from_bits(0x4272d73c)),
        (f32::from_bits(0x4250704b), f32::from_bits(0x428134df)),
    );
    path.line_to((f32::from_bits(0x4216adb6), f32::from_bits(0x423acdfc)));
    path.cubic_to(
        (f32::from_bits(0x4224a276), f32::from_bits(0x422f8c2c)),
        (f32::from_bits(0x42314905), f32::from_bits(0x4222c30f)),
        (f32::from_bits(0x423c64c0), f32::from_bits(0x4214afec)),
    );
    path.line_to((f32::from_bits(0x423c64bf), f32::from_bits(0x4214afea)));
    path.cubic_to(
        (f32::from_bits(0x42755f66), f32::from_bits(0x4198fbec)),
        (f32::from_bits(0x42800a9d), f32::from_bits(0xc0af53e2)),
        (f32::from_bits(0x4257f7fc), f32::from_bits(0xc1d15b49)),
    );
    path.cubic_to(
        (f32::from_bits(0x422fdabc), f32::from_bits(0xc23b70cc)),
        (f32::from_bits(0x41b7f168), f32::from_bits(0xc2700002)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4250704d), f32::from_bits(0x428134e0)));
    path.cubic_to(
        (f32::from_bits(0x41c9effb), f32::from_bits(0x42ac8cba)),
        (f32::from_bits(0xc143bd6b), f32::from_bits(0x42b21c58)),
        (f32::from_bits(0xc2280561), f32::from_bits(0x428f2c0c)),
    );
    path.cubic_to(
        (f32::from_bits(0xc28f8db2), f32::from_bits(0x42587782)),
        (f32::from_bits(0xc2af41ba), f32::from_bits(0x41a05b8a)),
        (f32::from_bits(0xc2a3a0d2), f32::from_bits(0xc15fb01a)),
    );
    path.cubic_to(
        (f32::from_bits(0xc297ffea), f32::from_bits(0xc24005d3)),
        (f32::from_bits(0xc246ef26), f32::from_bits(0xc295c2d5)),
        (f32::from_bits(0xc17d9b57), f32::from_bits(0xc2a2f1e8)),
    );
    path.line_to((f32::from_bits(0xc1375488), f32::from_bits(0xc26b9543)));
    path.cubic_to(
        (f32::from_bits(0xc20fcecd), f32::from_bits(0xc25885a3)),
        (f32::from_bits(0xc25bc22e), f32::from_bits(0xc20acfc5)),
        (f32::from_bits(0xc26c9222), f32::from_bits(0xc121b3b7)),
    );
    path.cubic_to(
        (f32::from_bits(0xc27d6216), f32::from_bits(0x4167d7a5)),
        (f32::from_bits(0xc24f8c13), f32::from_bits(0x421c7b68)),
        (f32::from_bits(0xc1f2ebf9), f32::from_bits(0x424efee8)),
    );
    path.cubic_to(
        (f32::from_bits(0xc10d7f99), f32::from_bits(0x4280c134)),
        (f32::from_bits(0x4191fa9e), f32::from_bits(0x4279782f)),
        (f32::from_bits(0x4216adb8), f32::from_bits(0x423acdfc)),
    );
    path.line_to((f32::from_bits(0x4250704d), f32::from_bits(0x428134e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7366-L7390 (chrome/m156)
fn battleOp267(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42003b3a), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4274ff8d), f32::from_bits(0xc28113a0)),
        (f32::from_bits(0x4295fac2), f32::from_bits(0xc20e4c24)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b175be), f32::from_bits(0xc0d38840)),
        (f32::from_bits(0x42a91fa3), f32::from_bits(0x41dd6a3d)),
        (f32::from_bits(0x42809081), f32::from_bits(0x4252054f)),
    );
    path.line_to((f32::from_bits(0x4239e059), f32::from_bits(0x4217d27c)));
    path.cubic_to(
        (f32::from_bits(0x4274841b), f32::from_bits(0x41a00f1c)),
        (f32::from_bits(0x428048c8), f32::from_bits(0xc098ea38)),
        (f32::from_bits(0x4258d681), f32::from_bits(0xc1cdbb32)),
    );
    path.cubic_to(
        (f32::from_bits(0x42311b71), f32::from_bits(0xc23a9deb)),
        (f32::from_bits(0x41b96511), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42809082), f32::from_bits(0x4252054e)));
    path.cubic_to(
        (f32::from_bits(0x4271521d), f32::from_bits(0x42655feb)),
        (f32::from_bits(0x425f60c7), f32::from_bits(0x4276e1ca)),
        (f32::from_bits(0x424ba43f), f32::from_bits(0x42831ae1)),
    );
    path.line_to((f32::from_bits(0x421335f7), f32::from_bits(0x423d8ca7)));
    path.cubic_to(
        (f32::from_bits(0x42217a65), f32::from_bits(0x4232780c)),
        (f32::from_bits(0x422e72e3), f32::from_bits(0x4225d023)),
        (f32::from_bits(0x4239e05a), f32::from_bits(0x4217d27c)),
    );
    path.line_to((f32::from_bits(0x42809082), f32::from_bits(0x4252054e)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7393-L7423 (chrome/m156)
fn battleOp268(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42003b3a), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x4274ff8d), f32::from_bits(0xc28113a0)),
        (f32::from_bits(0x4295fac2), f32::from_bits(0xc20e4c24)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b175be), f32::from_bits(0xc0d38840)),
        (f32::from_bits(0x42a91fa3), f32::from_bits(0x41dd6a3d)),
        (f32::from_bits(0x42809082), f32::from_bits(0x4252054e)),
    );
    path.cubic_to(
        (f32::from_bits(0x4271521d), f32::from_bits(0x42655feb)),
        (f32::from_bits(0x425f60c7), f32::from_bits(0x4276e1ca)),
        (f32::from_bits(0x424ba43f), f32::from_bits(0x42831ae1)),
    );
    path.line_to((f32::from_bits(0x421335f7), f32::from_bits(0x423d8ca7)));
    path.cubic_to(
        (f32::from_bits(0x42217a65), f32::from_bits(0x4232780c)),
        (f32::from_bits(0x422e72e3), f32::from_bits(0x4225d023)),
        (f32::from_bits(0x4239e059), f32::from_bits(0x4217d27c)),
    );
    path.cubic_to(
        (f32::from_bits(0x4274841b), f32::from_bits(0x41a00f1c)),
        (f32::from_bits(0x428048c8), f32::from_bits(0xc098ea38)),
        (f32::from_bits(0x4258d681), f32::from_bits(0xc1cdbb32)),
    );
    path.cubic_to(
        (f32::from_bits(0x42311b71), f32::from_bits(0xc23a9deb)),
        (f32::from_bits(0x41b96511), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3697ff52), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x424ba440), f32::from_bits(0x42831ae2)));
    path.cubic_to(
        (f32::from_bits(0x41bb72ba), f32::from_bits(0x42adc9b8)),
        (f32::from_bits(0xc16714ca), f32::from_bits(0x42b1a998)),
        (f32::from_bits(0xc22fd30d), f32::from_bits(0x428ccf5c)),
    );
    path.cubic_to(
        (f32::from_bits(0xc292f074), f32::from_bits(0x424fea41)),
        (f32::from_bits(0xc2b0b757), f32::from_bits(0x4188cdbd)),
        (f32::from_bits(0xc2a27f7d), f32::from_bits(0xc187abb1)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29447a3), f32::from_bits(0xc24c1290)),
        (f32::from_bits(0xc23a2b5e), f32::from_bits(0xc29a0e93)),
        (f32::from_bits(0xc141f42b), f32::from_bits(0xc2a43853)),
    );
    path.line_to((f32::from_bits(0xc10c3538), f32::from_bits(0xc26d6d31)));
    path.cubic_to(
        (f32::from_bits(0xc2069491), f32::from_bits(0xc25ebb9d)),
        (f32::from_bits(0xc2566164), f32::from_bits(0xc21385b2)),
        (f32::from_bits(0xc26aefd1), f32::from_bits(0xc1442672)),
    );
    path.cubic_to(
        (f32::from_bits(0xc27f7e3e), f32::from_bits(0x4145c9dc)),
        (f32::from_bits(0xc2547130), f32::from_bits(0x42164ccc)),
        (f32::from_bits(0xc1fe3427), f32::from_bits(0x424b94a6)),
    );
    path.cubic_to(
        (f32::from_bits(0xc1270bd9), f32::from_bits(0x42806e40)),
        (f32::from_bits(0x41878138), f32::from_bits(0x427b4278)),
        (f32::from_bits(0x421335f8), f32::from_bits(0x423d8ca8)),
    );
    path.line_to((f32::from_bits(0x424ba440), f32::from_bits(0x42831ae2)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7426-L7450 (chrome/m156)
fn battleOp269(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42011047), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x42766e56), f32::from_bits(0xc28099ef)),
        (f32::from_bits(0x42967824), f32::from_bits(0xc20c36c8)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b1b91c), f32::from_bits(0xc0b9cd9b)),
        (f32::from_bits(0x42a89b7a), f32::from_bits(0x41e5804f)),
        (f32::from_bits(0x427e310b), f32::from_bits(0x42559106)),
    );
    path.line_to((f32::from_bits(0x4237c0bf), f32::from_bits(0x421a62ac)));
    path.cubic_to(
        (f32::from_bits(0x4273c506), f32::from_bits(0x41a5e791)),
        (f32::from_bits(0x4280797a), f32::from_bits(0xc08650bf)),
        (f32::from_bits(0x42598bc5), f32::from_bits(0xc1cab811)),
    );
    path.cubic_to(
        (f32::from_bits(0x42322494), f32::from_bits(0xc239edfa)),
        (f32::from_bits(0x41ba9913), f32::from_bits(0xc2700002)),
        (f32::from_bits(0xb7060057), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x427e3109), f32::from_bits(0x42559108)));
    path.cubic_to(
        (f32::from_bits(0x426e0477), f32::from_bits(0x4268d13b)),
        (f32::from_bits(0x425bb575), f32::from_bits(0x427a2b1d)),
        (f32::from_bits(0x42479e2a), f32::from_bits(0x4284a4a0)),
    );
    path.line_to((f32::from_bits(0x42104d52), f32::from_bits(0x423fc5ea)));
    path.cubic_to(
        (f32::from_bits(0x421ed35e), f32::from_bits(0x4234d83a)),
        (f32::from_bits(0x422c0f91), f32::from_bits(0x42284d3a)),
        (f32::from_bits(0x4237c0bf), f32::from_bits(0x421a62ad)),
    );
    path.line_to((f32::from_bits(0x427e3109), f32::from_bits(0x42559108)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7453-L7478 (chrome/m156)
fn battleOp270(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb7060057), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42011047), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x42766e56), f32::from_bits(0xc28099ef)),
        (f32::from_bits(0x42967824), f32::from_bits(0xc20c36c8)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b1b91c), f32::from_bits(0xc0b9cd9b)),
        (f32::from_bits(0x42a89b7a), f32::from_bits(0x41e5804f)),
        (f32::from_bits(0x427e310b), f32::from_bits(0x42559106)),
    );
    path.line_to((f32::from_bits(0x4237c0bf), f32::from_bits(0x421a62ad)));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42479e29), f32::from_bits(0x4284a4a0)));
    path.cubic_to(
        (f32::from_bits(0x41af5d68), f32::from_bits(0x42aec1b4)),
        (f32::from_bits(0xc1822698), f32::from_bits(0x42b135a9)),
        (f32::from_bits(0xc2362f3e), f32::from_bits(0x428ac623)),
    );
    path.cubic_to(
        (f32::from_bits(0xc295a599), f32::from_bits(0x4248ad36)),
        (f32::from_bits(0xc2b1c6ab), f32::from_bits(0x416a48a9)),
        (f32::from_bits(0xc2a165f3), f32::from_bits(0xc19b42cf)),
    );
    path.cubic_to(
        (f32::from_bits(0xc291053c), f32::from_bits(0xc255d4f6)),
        (f32::from_bits(0xc22f520a), f32::from_bits(0xc29d68ba)),
        (f32::from_bits(0xc110422a), f32::from_bits(0xc2a50486)),
    );
    path.line_to((f32::from_bits(0xc0d09136), f32::from_bits(0xc26e946c)));
    path.cubic_to(
        (f32::from_bits(0xc1fd79b9), f32::from_bits(0xc2639452)),
        (f32::from_bits(0xc251ab0b), f32::from_bits(0xc21a93c1)),
        (f32::from_bits(0xc26958c8), f32::from_bits(0xc1607927)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2808342), f32::from_bits(0x41295cae)),
        (f32::from_bits(0xc2585b55), f32::from_bits(0x42111142)),
        (f32::from_bits(0xc203b318), f32::from_bits(0x4248a313)),
    );
    path.cubic_to(
        (f32::from_bits(0xc13c2b63), f32::from_bits(0x42801a73)),
        (f32::from_bits(0x417d8a30), f32::from_bits(0x427ca903)),
        (f32::from_bits(0x42104d56), f32::from_bits(0x423fc5e8)),
    );
    path.line_to((f32::from_bits(0x42479e29), f32::from_bits(0x4284a4a0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7481-L7505 (chrome/m156)
fn battleOp271(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4201b43a), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4277880a), f32::from_bits(0xc2803bc7)),
        (f32::from_bits(0x4296d747), f32::from_bits(0xc20a9b85)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b1ea89), f32::from_bits(0xc0a5fbe3)),
        (f32::from_bits(0x42a831cc), f32::from_bits(0x41ebb52f)),
        (f32::from_bits(0x427be65b), f32::from_bits(0x425843c9)),
    );
    path.line_to((f32::from_bits(0x423618a6), f32::from_bits(0x421c5604)));
    path.cubic_to(
        (f32::from_bits(0x42732c40), f32::from_bits(0x41aa6424)),
        (f32::from_bits(0x42809d37), f32::from_bits(0xc06ffa1c)),
        (f32::from_bits(0x425a1555), f32::from_bits(0xc1c8657d)),
    );
    path.cubic_to(
        (f32::from_bits(0x4232f03c), f32::from_bits(0xc23965db)),
        (f32::from_bits(0x41bb8620), f32::from_bits(0xc2700002)),
        (f32::from_bits(0xb5600574), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x427be65e), f32::from_bits(0x425843c9)));
    path.cubic_to(
        (f32::from_bits(0x426b71bd), f32::from_bits(0x426b6e8c)),
        (f32::from_bits(0x4258dad9), f32::from_bits(0x427ca87a)),
        (f32::from_bits(0x42447e14), f32::from_bits(0x4285cdfb)),
    );
    path.line_to((f32::from_bits(0x420e0af4), f32::from_bits(0x424173d3)));
    path.cubic_to(
        (f32::from_bits(0x421cc338), f32::from_bits(0x4236a4f9)),
        (f32::from_bits(0x422a3361), f32::from_bits(0x422a3113)),
        (f32::from_bits(0x423618a6), f32::from_bits(0x421c5605)),
    );
    path.line_to((f32::from_bits(0x427be65e), f32::from_bits(0x425843c9)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7508-L7540 (chrome/m156)
fn battleOp272(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4201b43a), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4277880a), f32::from_bits(0xc2803bc7)),
        (f32::from_bits(0x4296d747), f32::from_bits(0xc20a9b85)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b1ea89), f32::from_bits(0xc0a5fbe3)),
        (f32::from_bits(0x42a831cc), f32::from_bits(0x41ebb52f)),
        (f32::from_bits(0x427be65b), f32::from_bits(0x425843c9)),
    );
    path.line_to((f32::from_bits(0x427be65e), f32::from_bits(0x425843c9)));
    path.cubic_to(
        (f32::from_bits(0x426b71bd), f32::from_bits(0x426b6e8c)),
        (f32::from_bits(0x4258dad9), f32::from_bits(0x427ca87a)),
        (f32::from_bits(0x42447e14), f32::from_bits(0x4285cdfb)),
    );
    path.line_to((f32::from_bits(0x420e0af4), f32::from_bits(0x424173d3)));
    path.cubic_to(
        (f32::from_bits(0x421cc338), f32::from_bits(0x4236a4f9)),
        (f32::from_bits(0x422a3361), f32::from_bits(0x422a3113)),
        (f32::from_bits(0x423618a6), f32::from_bits(0x421c5605)),
    );
    path.line_to((f32::from_bits(0x423618a6), f32::from_bits(0x421c5604)));
    path.cubic_to(
        (f32::from_bits(0x42732c40), f32::from_bits(0x41aa6424)),
        (f32::from_bits(0x42809d37), f32::from_bits(0xc06ffa1c)),
        (f32::from_bits(0x425a1555), f32::from_bits(0xc1c8657d)),
    );
    path.cubic_to(
        (f32::from_bits(0x4232f03c), f32::from_bits(0xc23965db)),
        (f32::from_bits(0x41bb8620), f32::from_bits(0xc2700002)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42447e16), f32::from_bits(0x4285cdfb)));
    path.cubic_to(
        (f32::from_bits(0x41a605d7), f32::from_bits(0x42af776a)),
        (f32::from_bits(0xc18d5e26), f32::from_bits(0x42b0cfa2)),
        (f32::from_bits(0xc23b02ad), f32::from_bits(0x428928e1)),
    );
    path.cubic_to(
        (f32::from_bits(0xc297ab24), f32::from_bits(0x42430442)),
        (f32::from_bits(0xc2b27fa9), f32::from_bits(0x414bdf0d)),
        (f32::from_bits(0xc2a073c8), f32::from_bits(0xc1aa3a13)),
    );
    path.cubic_to(
        (f32::from_bits(0xc28e67e7), f32::from_bits(0xc25d31d4)),
        (f32::from_bits(0xc226d0a4), f32::from_bits(0xc29fdb7e)),
        (f32::from_bits(0xc0d3d11a), f32::from_bits(0xc2a578a5)),
    );
    path.line_to((f32::from_bits(0xc0991eb2), f32::from_bits(0xc26f3c4f)));
    path.cubic_to(
        (f32::from_bits(0xc1f12d9c), f32::from_bits(0xc2671e82)),
        (f32::from_bits(0xc24de350), f32::from_bits(0xc21fe656)),
        (f32::from_bits(0xc267faa7), f32::from_bits(0xc1761c74)),
    );
    path.cubic_to(
        (f32::from_bits(0xc28108ff), f32::from_bits(0x4113607a)),
        (f32::from_bits(0xc25b4798), f32::from_bits(0x420cf9d1)),
        (f32::from_bits(0xc207302c), f32::from_bits(0x42464d9a)),
    );
    path.cubic_to(
        (f32::from_bits(0xc14c6303), f32::from_bits(0x427fa162)),
        (f32::from_bits(0x4170087f), f32::from_bits(0x427dafb7)),
        (f32::from_bits(0x420e0af6), f32::from_bits(0x424173d2)),
    );
    path.line_to((f32::from_bits(0x42447e16), f32::from_bits(0x4285cdfb)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7543-L7567 (chrome/m156)
fn battleOp273(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42023f77), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x427876e4), f32::from_bits(0xc27fd6f4)),
        (f32::from_bits(0x42972728), f32::from_bits(0xc2093dbb)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b212de), f32::from_bits(0xc0952410)),
        (f32::from_bits(0x42a7d55b), f32::from_bits(0x41f0f791)),
        (f32::from_bits(0x4279eebf), f32::from_bits(0x425a890b)),
    );
    path.line_to((f32::from_bits(0x4234ac95), f32::from_bits(0x421dfa35)));
    path.cubic_to(
        (f32::from_bits(0x4272a697), f32::from_bits(0x41ae3171)),
        (f32::from_bits(0x4280ba5e), f32::from_bits(0xc057a00f)),
        (f32::from_bits(0x425a88d0), f32::from_bits(0xc1c66bc2)),
    );
    path.cubic_to(
        (f32::from_bits(0x42339ce5), f32::from_bits(0xc238f1c1)),
        (f32::from_bits(0x41bc4f6b), f32::from_bits(0xc2700002)),
        (f32::from_bits(0xb630015d), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4279eebd), f32::from_bits(0x425a890e)));
    path.cubic_to(
        (f32::from_bits(0x42693cf3), f32::from_bits(0x426da0dc)),
        (f32::from_bits(0x42566929), f32::from_bits(0x427ebed8)),
        (f32::from_bits(0x4241d1ac), f32::from_bits(0x4286c6a2)),
    );
    path.line_to((f32::from_bits(0x420c1c33), f32::from_bits(0x4242db53)));
    path.cubic_to(
        (f32::from_bits(0x421afee9), f32::from_bits(0x42382742)),
        (f32::from_bits(0x42289b18), f32::from_bits(0x422bc78f)),
        (f32::from_bits(0x4234ac94), f32::from_bits(0x421dfa34)),
    );
    path.line_to((f32::from_bits(0x4279eebd), f32::from_bits(0x425a890e)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7570-L7598 (chrome/m156)
fn battleOp274(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb630015d), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42023f77), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x427876e4), f32::from_bits(0xc27fd6f4)),
        (f32::from_bits(0x42972728), f32::from_bits(0xc2093dbb)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b212de), f32::from_bits(0xc0952410)),
        (f32::from_bits(0x42a7d55b), f32::from_bits(0x41f0f791)),
        (f32::from_bits(0x4279eebf), f32::from_bits(0x425a890b)),
    );
    path.line_to((f32::from_bits(0x4234ac95), f32::from_bits(0x421dfa35)));
    path.cubic_to(
        (f32::from_bits(0x4272a697), f32::from_bits(0x41ae3171)),
        (f32::from_bits(0x4280ba5e), f32::from_bits(0xc057a00f)),
        (f32::from_bits(0x425a88d0), f32::from_bits(0xc1c66bc2)),
    );
    path.cubic_to(
        (f32::from_bits(0x42339ce5), f32::from_bits(0xc238f1c1)),
        (f32::from_bits(0x41bc4f6b), f32::from_bits(0xc2700002)),
        (f32::from_bits(0xb630015d), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4241d1ad), f32::from_bits(0x4286c6a2)));
    path.cubic_to(
        (f32::from_bits(0x419e0f8e), f32::from_bits(0x42b00b7b)),
        (f32::from_bits(0xc196dfc4), f32::from_bits(0x42b07042)),
        (f32::from_bits(0xc23f0fa7), f32::from_bits(0x4287c1be)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29957b6), f32::from_bits(0x423e2672)),
        (f32::from_bits(0xc2b30c7a), f32::from_bits(0x4131f351)),
        (f32::from_bits(0xc29f94d8), f32::from_bits(0xc1b6db1d)),
    );
    path.cubic_to(
        (f32::from_bits(0xc28c1d38), f32::from_bits(0xc26357ee)),
        (f32::from_bits(0xc21f7d48), f32::from_bits(0xc2a1d87d)),
        (f32::from_bits(0xc09294c7), f32::from_bits(0xc2a5bf3c)),
    );
    path.line_to((f32::from_bits(0xc053ec94), f32::from_bits(0xc26fa25d)));
    path.cubic_to(
        (f32::from_bits(0xc1e69644), f32::from_bits(0xc269fe64)),
        (f32::from_bits(0xc24a931a), f32::from_bits(0xc224583b)),
        (f32::from_bits(0xc266b858), f32::from_bits(0xc1842f59)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2816ecb), f32::from_bits(0x4100a388)),
        (f32::from_bits(0xc25db33b), f32::from_bits(0x42097539)),
        (f32::from_bits(0xc20a1dd2), f32::from_bits(0x4244465c)),
    );
    path.cubic_to(
        (f32::from_bits(0xc15a2194), f32::from_bits(0x427f177f)),
        (f32::from_bits(0x41648588), f32::from_bits(0x427e85cc)),
        (f32::from_bits(0x420c1c35), f32::from_bits(0x4242db52)),
    );
    path.line_to((f32::from_bits(0x4241d1ad), f32::from_bits(0x4286c6a2)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7601-L7625 (chrome/m156)
fn battleOp275(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4202aab9), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42792ea4), f32::from_bits(0xc27f5acc)),
        (f32::from_bits(0x4297641b), f32::from_bits(0xc2082fee)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b230e5), f32::from_bits(0xc0882884)),
        (f32::from_bits(0x42a78c73), f32::from_bits(0x41f502e3)),
        (f32::from_bits(0x4278676f), f32::from_bits(0x425c4571)),
    );
    path.line_to((f32::from_bits(0x423391b8), f32::from_bits(0x421f3b73)));
    path.cubic_to(
        (f32::from_bits(0x42723d33), f32::from_bits(0x41b11ddb)),
        (f32::from_bits(0x4280d014), f32::from_bits(0xc044db05)),
        (f32::from_bits(0x425ae0f2), f32::from_bits(0xc1c4e5b3)),
    );
    path.cubic_to(
        (f32::from_bits(0x423421be), f32::from_bits(0xc2389802)),
        (f32::from_bits(0x41bcea83), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3725ffa9), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42786771), f32::from_bits(0x425c4570)));
    path.cubic_to(
        (f32::from_bits(0x42678692), f32::from_bits(0x426f4e2b)),
        (f32::from_bits(0x425483f6), f32::from_bits(0x42802b0f)),
        (f32::from_bits(0x423fbf6b), f32::from_bits(0x428783bc)),
    );
    path.line_to((f32::from_bits(0x420a9ce1), f32::from_bits(0x4243ecb9)));
    path.cubic_to(
        (f32::from_bits(0x4219a02a), f32::from_bits(0x42394dac)),
        (f32::from_bits(0x42275e32), f32::from_bits(0x422cfde6)),
        (f32::from_bits(0x423391b8), f32::from_bits(0x421f3b72)),
    );
    path.line_to((f32::from_bits(0x42786771), f32::from_bits(0x425c4570)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7628-L7659 (chrome/m156)
fn battleOp276(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3725ffa9), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4202aab9), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x42792ea4), f32::from_bits(0xc27f5acc)),
        (f32::from_bits(0x4297641b), f32::from_bits(0xc2082fee)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b230e5), f32::from_bits(0xc0882884)),
        (f32::from_bits(0x42a78c73), f32::from_bits(0x41f502e3)),
        (f32::from_bits(0x4278676f), f32::from_bits(0x425c4571)),
    );
    path.cubic_to(
        (f32::from_bits(0x42678690), f32::from_bits(0x426f4e2b)),
        (f32::from_bits(0x425483f5), f32::from_bits(0x42802b0f)),
        (f32::from_bits(0x423fbf6b), f32::from_bits(0x428783bc)),
    );
    path.line_to((f32::from_bits(0x420a9ce1), f32::from_bits(0x4243ecb9)));
    path.cubic_to(
        (f32::from_bits(0x4219a02a), f32::from_bits(0x42394dac)),
        (f32::from_bits(0x42275e32), f32::from_bits(0x422cfde7)),
        (f32::from_bits(0x423391b8), f32::from_bits(0x421f3b73)),
    );
    path.line_to((f32::from_bits(0x423391b8), f32::from_bits(0x421f3b72)));
    path.cubic_to(
        (f32::from_bits(0x42723d33), f32::from_bits(0x41b11dd9)),
        (f32::from_bits(0x4280d014), f32::from_bits(0xc044db09)),
        (f32::from_bits(0x425ae0f2), f32::from_bits(0xc1c4e5b3)),
    );
    path.cubic_to(
        (f32::from_bits(0x423421be), f32::from_bits(0xc2389802)),
        (f32::from_bits(0x41bcea83), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3725ffa9), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423fbf6b), f32::from_bits(0x428783bc)));
    path.cubic_to(
        (f32::from_bits(0x4197e908), f32::from_bits(0x42b0799e)),
        (f32::from_bits(0xc19e2f01), f32::from_bits(0x42b0215b)),
        (f32::from_bits(0xc24226b0), f32::from_bits(0x4286a80b)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29a9aef), f32::from_bits(0x423a5d79)),
        (f32::from_bits(0xc2b36ebb), f32::from_bits(0x411dee4a)),
        (f32::from_bits(0xc29ede64), f32::from_bits(0xc1c087c1)),
    );
    path.cubic_to(
        (f32::from_bits(0xc28a4e0d), f32::from_bits(0xc2680353)),
        (f32::from_bits(0xc219c8f7), f32::from_bits(0xc2a351d0)),
        (f32::from_bits(0xc0409740), f32::from_bits(0xc2a5e40e)),
    );
    path.line_to((f32::from_bits(0xc00b391c), f32::from_bits(0xc26fd79b)));
    path.cubic_to(
        (f32::from_bits(0xc1de5701), f32::from_bits(0xc26c1feb)),
        (f32::from_bits(0xc247f576), f32::from_bits(0xc227b85e)),
        (f32::from_bits(0xc265b08d), f32::from_bits(0xc18b2dac)),
    );
    path.cubic_to(
        (f32::from_bits(0xc281b5d1), f32::from_bits(0x40e45588)),
        (f32::from_bits(0xc25f8687), f32::from_bits(0x4206b8c8)),
        (f32::from_bits(0xc20c59a1), f32::from_bits(0x4242af19)),
    );
    path.cubic_to(
        (f32::from_bits(0xc164b2eb), f32::from_bits(0x427ea56a)),
        (f32::from_bits(0x415ba119), f32::from_bits(0x427f2508)),
        (f32::from_bits(0x420a9ce0), f32::from_bits(0x4243ecba)),
    );
    path.line_to((f32::from_bits(0x423fbf6b), f32::from_bits(0x428783bc)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7662-L7686 (chrome/m156)
fn battleOp277(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4202f62b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4279afc7), f32::from_bits(0xc27f0340)),
        (f32::from_bits(0x42978eaf), f32::from_bits(0xc20771fd)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b2457b), f32::from_bits(0xc07e0b91)),
        (f32::from_bits(0x42a7584a), f32::from_bits(0x41f7da1e)),
        (f32::from_bits(0x42775276), f32::from_bits(0x425d7c3f)),
    );
    path.line_to((f32::from_bits(0x4232c97e), f32::from_bits(0x42201c22)));
    path.cubic_to(
        (f32::from_bits(0x4271f1c7), f32::from_bits(0x41b32b8d)),
        (f32::from_bits(0x4280def3), f32::from_bits(0xc037a5cf)),
        (f32::from_bits(0x425b1e7c), f32::from_bits(0xc1c3d316)),
    );
    path.cubic_to(
        (f32::from_bits(0x42347f10), f32::from_bits(0xc23858b9)),
        (f32::from_bits(0x41bd578b), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0xb7240057), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42775277), f32::from_bits(0x425d7c41)));
    path.cubic_to(
        (f32::from_bits(0x4266507b), f32::from_bits(0x42707a20)),
        (f32::from_bits(0x42532cff), f32::from_bits(0x4280b928)),
        (f32::from_bits(0x423e48db), f32::from_bits(0x42880779)),
    );
    path.line_to((f32::from_bits(0x42098e1c), f32::from_bits(0x4244ab32)));
    path.cubic_to(
        (f32::from_bits(0x4218a83e), f32::from_bits(0x423a1b21)),
        (f32::from_bits(0x42267e0b), f32::from_bits(0x422dd6be)),
        (f32::from_bits(0x4232c97e), f32::from_bits(0x42201c22)),
    );
    path.line_to((f32::from_bits(0x42775277), f32::from_bits(0x425d7c41)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7689-L7720 (chrome/m156)
fn battleOp278(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb7240057), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x4202f62b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x4279afc7), f32::from_bits(0xc27f0340)),
        (f32::from_bits(0x42978eaf), f32::from_bits(0xc20771fd)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b2457b), f32::from_bits(0xc07e0b91)),
        (f32::from_bits(0x42a7584a), f32::from_bits(0x41f7da1e)),
        (f32::from_bits(0x42775276), f32::from_bits(0x425d7c3f)),
    );
    path.line_to((f32::from_bits(0x42775277), f32::from_bits(0x425d7c41)));
    path.cubic_to(
        (f32::from_bits(0x4266507b), f32::from_bits(0x42707a20)),
        (f32::from_bits(0x42532cff), f32::from_bits(0x4280b928)),
        (f32::from_bits(0x423e48db), f32::from_bits(0x42880779)),
    );
    path.line_to((f32::from_bits(0x42098e1c), f32::from_bits(0x4244ab32)));
    path.cubic_to(
        (f32::from_bits(0x4218a83e), f32::from_bits(0x423a1b21)),
        (f32::from_bits(0x42267e0b), f32::from_bits(0x422dd6be)),
        (f32::from_bits(0x4232c97e), f32::from_bits(0x42201c22)),
    );
    path.cubic_to(
        (f32::from_bits(0x4271f1c7), f32::from_bits(0x41b32b8d)),
        (f32::from_bits(0x4280def3), f32::from_bits(0xc037a5cf)),
        (f32::from_bits(0x425b1e7c), f32::from_bits(0xc1c3d316)),
    );
    path.cubic_to(
        (f32::from_bits(0x42347f10), f32::from_bits(0xc23858b9)),
        (f32::from_bits(0x41bd578b), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0xb7240057), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423e48db), f32::from_bits(0x4288077a)));
    path.cubic_to(
        (f32::from_bits(0x41939344), f32::from_bits(0x42b0c509)),
        (f32::from_bits(0xc1a3515b), f32::from_bits(0x42afe6ff)),
        (f32::from_bits(0xc2444efb), f32::from_bits(0x4285df44)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29b7aa2), f32::from_bits(0x4237af14)),
        (f32::from_bits(0xc2b3ae7d), f32::from_bits(0x410fd2d1)),
        (f32::from_bits(0xc29e5879), f32::from_bits(0xc1c74e5b)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2890275), f32::from_bits(0xc26b4310)),
        (f32::from_bits(0xc215bdd9), f32::from_bits(0xc2a45375)),
        (f32::from_bits(0xbff3abc7), f32::from_bits(0xc2a5f4d2)),
    );
    path.line_to((f32::from_bits(0xbfb025f0), f32::from_bits(0xc26fefd6)));
    path.cubic_to(
        (f32::from_bits(0xc1d87e6f), f32::from_bits(0xc26d946b)),
        (f32::from_bits(0xc246160c), f32::from_bits(0xc22a11a0)),
        (f32::from_bits(0xc264eef0), f32::from_bits(0xc190139e)),
    );
    path.cubic_to(
        (f32::from_bits(0xc281e3ea), f32::from_bits(0x40cff015)),
        (f32::from_bits(0xc260c9f8), f32::from_bits(0x4204c898)),
        (f32::from_bits(0xc20de8e2), f32::from_bits(0x42418cd3)),
    );
    path.cubic_to(
        (f32::from_bits(0xc16c1f36), f32::from_bits(0x427e510e)),
        (f32::from_bits(0x41555c9e), f32::from_bits(0x427f9213)),
        (f32::from_bits(0x42098e1b), f32::from_bits(0x4244ab33)),
    );
    path.line_to((f32::from_bits(0x423e48db), f32::from_bits(0x4288077a)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7723-L7747 (chrome/m156)
fn battleOp279(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x420331e6), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a15f4), f32::from_bits(0xc27ebdd3)),
        (f32::from_bits(0x4297b03a), f32::from_bits(0xc206db86)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b2557a), f32::from_bits(0xc06f9378)),
        (f32::from_bits(0x42a72e7e), f32::from_bits(0x41fa194a)),
        (f32::from_bits(0x4276762d), f32::from_bits(0x425e7148)),
    );
    path.line_to((f32::from_bits(0x42322a40), f32::from_bits(0x4220cd43)));
    path.cubic_to(
        (f32::from_bits(0x4271b558), f32::from_bits(0x41b4cb56)),
        (f32::from_bits(0x4280ea83), f32::from_bits(0xc02d3004)),
        (f32::from_bits(0x425b4efa), f32::from_bits(0xc1c2f986)),
    );
    path.cubic_to(
        (f32::from_bits(0x4234c8ee), f32::from_bits(0xc2382686)),
        (f32::from_bits(0x41bdadf1), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3707ffa9), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4276762e), f32::from_bits(0x425e7147)));
    path.cubic_to(
        (f32::from_bits(0x42655a01), f32::from_bits(0x42716669)),
        (f32::from_bits(0x42521c84), f32::from_bits(0x428128fd)),
        (f32::from_bits(0x423d1f69), f32::from_bits(0x42886f05)),
    );
    path.line_to((f32::from_bits(0x4208b718), f32::from_bits(0x424540e7)));
    path.cubic_to(
        (f32::from_bits(0x4217e344), f32::from_bits(0x423abccf)),
        (f32::from_bits(0x4225cbdd), f32::from_bits(0x422e818f)),
        (f32::from_bits(0x42322a41), f32::from_bits(0x4220cd43)),
    );
    path.line_to((f32::from_bits(0x4276762e), f32::from_bits(0x425e7147)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7750-L7782 (chrome/m156)
fn battleOp280(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3707ffa9), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x420331e6), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a15f4), f32::from_bits(0xc27ebdd3)),
        (f32::from_bits(0x4297b03a), f32::from_bits(0xc206db86)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b2557a), f32::from_bits(0xc06f937f)),
        (f32::from_bits(0x42a72e7e), f32::from_bits(0x41fa1948)),
        (f32::from_bits(0x4276762e), f32::from_bits(0x425e7147)),
    );
    path.line_to((f32::from_bits(0x4276762d), f32::from_bits(0x425e7148)));
    path.cubic_to(
        (f32::from_bits(0x42655a00), f32::from_bits(0x4271666a)),
        (f32::from_bits(0x42521c84), f32::from_bits(0x428128fd)),
        (f32::from_bits(0x423d1f69), f32::from_bits(0x42886f05)),
    );
    path.line_to((f32::from_bits(0x4208b718), f32::from_bits(0x424540e7)));
    path.cubic_to(
        (f32::from_bits(0x4217e344), f32::from_bits(0x423abccf)),
        (f32::from_bits(0x4225cbdd), f32::from_bits(0x422e818f)),
        (f32::from_bits(0x42322a41), f32::from_bits(0x4220cd43)),
    );
    path.line_to((f32::from_bits(0x42322a40), f32::from_bits(0x4220cd43)));
    path.cubic_to(
        (f32::from_bits(0x4271b558), f32::from_bits(0x41b4cb56)),
        (f32::from_bits(0x4280ea83), f32::from_bits(0xc02d3004)),
        (f32::from_bits(0x425b4efa), f32::from_bits(0xc1c2f986)),
    );
    path.cubic_to(
        (f32::from_bits(0x4234c8ee), f32::from_bits(0xc2382686)),
        (f32::from_bits(0x41bdadf1), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3707ffa9), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423d1f69), f32::from_bits(0x42886f06)));
    path.cubic_to(
        (f32::from_bits(0x4190236c), f32::from_bits(0x42b0ff8c)),
        (f32::from_bits(0xc1a760b7), f32::from_bits(0x42afb726)),
        (f32::from_bits(0xc24601c7), f32::from_bits(0x42853ece)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29c2998), f32::from_bits(0x42358ced)),
        (f32::from_bits(0xc2b3ddd5), f32::from_bits(0x4104a433)),
        (f32::from_bits(0xc29deb35), f32::from_bits(0xc1cca70e)),
    );
    path.cubic_to(
        (f32::from_bits(0xc287f895), f32::from_bits(0xc26dd020)),
        (f32::from_bits(0xc21285d2), f32::from_bits(0xc2a51ade)),
        (f32::from_bits(0xbf83a2cf), f32::from_bits(0xc2a5fcbd)),
    );
    path.line_to((f32::from_bits(0xbf3e53cf), f32::from_bits(0xc26ffb48)));
    path.cubic_to(
        (f32::from_bits(0xc1d3d71b), f32::from_bits(0xc26eb4b2)),
        (f32::from_bits(0xc24495a7), f32::from_bits(0xc22be9b4)),
        (f32::from_bits(0xc26450f5), f32::from_bits(0xc193f109)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2820621), f32::from_bits(0x40bfc558)),
        (f32::from_bits(0xc261c6ea), f32::from_bits(0x42033dc6)),
        (f32::from_bits(0xc20f2333), f32::from_bits(0x4240a4d2)),
    );
    path.cubic_to(
        (f32::from_bits(0xc171fde8), f32::from_bits(0x427e0bde)),
        (f32::from_bits(0x4150649d), f32::from_bits(0x427fe6ab)),
        (f32::from_bits(0x4208b71a), f32::from_bits(0x424540e8)),
    );
    path.line_to((f32::from_bits(0x423d1f69), f32::from_bits(0x42886f06)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7785-L7809 (chrome/m156)
fn battleOp281(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42035955), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x427a595d), f32::from_bits(0xc27e8fe6)),
        (f32::from_bits(0x4297c647), f32::from_bits(0xc206781b)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b25fdf), f32::from_bits(0xc0660504)),
        (f32::from_bits(0x42a712a2), f32::from_bits(0x41fb94c7)),
        (f32::from_bits(0x4275e43b), f32::from_bits(0x425f1290)),
    );
    path.line_to((f32::from_bits(0x4231c0be), f32::from_bits(0x422141dc)));
    path.cubic_to(
        (f32::from_bits(0x42718d10), f32::from_bits(0x41b5ddaf)),
        (f32::from_bits(0x4280f208), f32::from_bits(0xc026476c)),
        (f32::from_bits(0x425b6edc), f32::from_bits(0xc1c269cb)),
    );
    path.cubic_to(
        (f32::from_bits(0x4234f9ab), f32::from_bits(0xc2380553)),
        (f32::from_bits(0x41bde6f3), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4275e43b), f32::from_bits(0x425f1292)));
    path.cubic_to(
        (f32::from_bits(0x4264b6c3), f32::from_bits(0x427201df)),
        (f32::from_bits(0x4251681e), f32::from_bits(0x42817283)),
        (f32::from_bits(0x423c5a8f), f32::from_bits(0x4288b309)),
    );
    path.line_to((f32::from_bits(0x420828ca), f32::from_bits(0x4245a33c)));
    path.cubic_to(
        (f32::from_bits(0x421760db), f32::from_bits(0x423b2719)),
        (f32::from_bits(0x422555d9), f32::from_bits(0x422ef1ee)),
        (f32::from_bits(0x4231c0be), f32::from_bits(0x422141da)),
    );
    path.line_to((f32::from_bits(0x4275e43b), f32::from_bits(0x425f1292)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7812-L7843 (chrome/m156)
fn battleOp282(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42035955), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x427a595d), f32::from_bits(0xc27e8fe6)),
        (f32::from_bits(0x4297c647), f32::from_bits(0xc206781b)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b25fdf), f32::from_bits(0xc0660504)),
        (f32::from_bits(0x42a712a2), f32::from_bits(0x41fb94c7)),
        (f32::from_bits(0x4275e43b), f32::from_bits(0x425f1290)),
    );
    path.line_to((f32::from_bits(0x4275e43b), f32::from_bits(0x425f1292)));
    path.cubic_to(
        (f32::from_bits(0x4264b6c3), f32::from_bits(0x427201df)),
        (f32::from_bits(0x4251681e), f32::from_bits(0x42817283)),
        (f32::from_bits(0x423c5a8f), f32::from_bits(0x4288b309)),
    );
    path.line_to((f32::from_bits(0x420828ca), f32::from_bits(0x4245a33c)));
    path.cubic_to(
        (f32::from_bits(0x421760db), f32::from_bits(0x423b2719)),
        (f32::from_bits(0x422555d9), f32::from_bits(0x422ef1f0)),
        (f32::from_bits(0x4231c0be), f32::from_bits(0x422141dc)),
    );
    path.cubic_to(
        (f32::from_bits(0x42718d10), f32::from_bits(0x41b5ddaf)),
        (f32::from_bits(0x4280f208), f32::from_bits(0xc026476c)),
        (f32::from_bits(0x425b6edc), f32::from_bits(0xc1c269cb)),
    );
    path.cubic_to(
        (f32::from_bits(0x4234f9ab), f32::from_bits(0xc2380553)),
        (f32::from_bits(0x41bde6f3), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3637fea5), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423c5a8f), f32::from_bits(0x4288b30a)));
    path.cubic_to(
        (f32::from_bits(0x418dddd4), f32::from_bits(0x42b12599)),
        (f32::from_bits(0xc1aa0e7c), f32::from_bits(0x42af96c0)),
        (f32::from_bits(0xc2471fb7), f32::from_bits(0x4284d41e)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29c9c18), f32::from_bits(0x423422f8)),
        (f32::from_bits(0xc2b3fb95), f32::from_bits(0x40fa8096)),
        (f32::from_bits(0xc29da17e), f32::from_bits(0xc1d02ca0)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2874768), f32::from_bits(0xc26f7cb1)),
        (f32::from_bits(0xc2106396), f32::from_bits(0xc2a59c4c)),
        (f32::from_bits(0xbee6b152), f32::from_bits(0xc2a5ff5f)),
    );
    path.line_to((f32::from_bits(0xbea6c49b), f32::from_bits(0xc26fff18)));
    path.cubic_to(
        (f32::from_bits(0xc1d0c156), f32::from_bits(0xc26f6fd8)),
        (f32::from_bits(0xc2439580), f32::from_bits(0xc22d1f86)),
        (f32::from_bits(0xc263e663), f32::from_bits(0xc1967cc0)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2821ba4), f32::from_bits(0x40b51622)),
        (f32::from_bits(0xc2626c73), f32::from_bits(0x4202381f)),
        (f32::from_bits(0xc20ff1e5), f32::from_bits(0x42400a93)),
    );
    path.cubic_to(
        (f32::from_bits(0xc175dd55), f32::from_bits(0x427ddd08)),
        (f32::from_bits(0x414d1bd1), f32::from_bits(0x42800ed7)),
        (f32::from_bits(0x420828d0), f32::from_bits(0x4245a338)),
    );
    path.line_to((f32::from_bits(0x423c5a8f), f32::from_bits(0x4288b30a)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7846-L7870 (chrome/m156)
fn battleOp283(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42036bf7), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a7934), f32::from_bits(0xc27e7a35)),
        (f32::from_bits(0x4297d0ad), f32::from_bits(0xc2064926)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b264c0), f32::from_bits(0xc061818a)),
        (f32::from_bits(0x42a70569), f32::from_bits(0x41fc47ee)),
        (f32::from_bits(0x42759f2d), f32::from_bits(0x425f5e99)),
    );
    path.line_to((f32::from_bits(0x42318ed2), f32::from_bits(0x422178d2)));
    path.cubic_to(
        (f32::from_bits(0x427179f2), f32::from_bits(0x41b65f2f)),
        (f32::from_bits(0x4280f58f), f32::from_bits(0xc0230424)),
        (f32::from_bits(0x425b7de6), f32::from_bits(0xc1c225e6)),
    );
    path.cubic_to(
        (f32::from_bits(0x423510af), f32::from_bits(0xc237f5a4)),
        (f32::from_bits(0x41be01e5), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3707ffa9), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42759f2b), f32::from_bits(0x425f5e9b)));
    path.cubic_to(
        (f32::from_bits(0x42646988), f32::from_bits(0x42724b20)),
        (f32::from_bits(0x425112cb), f32::from_bits(0x42819524)),
        (f32::from_bits(0x423bfd7a), f32::from_bits(0x4288d30e)),
    );
    path.line_to((f32::from_bits(0x4207e580), f32::from_bits(0x4245d187)));
    path.cubic_to(
        (f32::from_bits(0x4217232e), f32::from_bits(0x423b592c)),
        (f32::from_bits(0x42251e07), f32::from_bits(0x422f26e4)),
        (f32::from_bits(0x42318ed3), f32::from_bits(0x422178d2)),
    );
    path.line_to((f32::from_bits(0x42759f2b), f32::from_bits(0x425f5e9b)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7873-L7898 (chrome/m156)
fn battleOp284(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3707ffa9), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42036bf7), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a7934), f32::from_bits(0xc27e7a35)),
        (f32::from_bits(0x4297d0ad), f32::from_bits(0xc2064926)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b264c0), f32::from_bits(0xc061818a)),
        (f32::from_bits(0x42a70569), f32::from_bits(0x41fc47ee)),
        (f32::from_bits(0x42759f2d), f32::from_bits(0x425f5e99)),
    );
    path.line_to((f32::from_bits(0x42318ed3), f32::from_bits(0x422178d2)));
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bfd7a), f32::from_bits(0x4288d30e)));
    path.cubic_to(
        (f32::from_bits(0x418ccafd), f32::from_bits(0x42b13768)),
        (f32::from_bits(0xc1ab522b), f32::from_bits(0x42af873b)),
        (f32::from_bits(0xc247a66c), f32::from_bits(0x4284a188)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cd1e0), f32::from_bits(0x423377ac)),
        (f32::from_bits(0xc2b40936), f32::from_bits(0x40f384e7)),
        (f32::from_bits(0xc29d7e41), f32::from_bits(0xc1d1d5b9)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286f34a), f32::from_bits(0xc2704657)),
        (f32::from_bits(0xc20f6108), f32::from_bits(0xc2a5d8cf)),
        (f32::from_bits(0xbe35f437), f32::from_bits(0xc2a5ffe6)),
    );
    path.line_to((f32::from_bits(0xbe038989), f32::from_bits(0xc26fffdc)));
    path.cubic_to(
        (f32::from_bits(0xc1cf4b80), f32::from_bits(0xc26fc755)),
        (f32::from_bits(0xc2431bdf), f32::from_bits(0xc22db14d)),
        (f32::from_bits(0xc263b36c), f32::from_bits(0xc197b016)),
    );
    path.cubic_to(
        (f32::from_bits(0xc282257d), f32::from_bits(0x40b009af)),
        (f32::from_bits(0xc262ba31), f32::from_bits(0x4201bc49)),
        (f32::from_bits(0xc2105343), f32::from_bits(0x423fc16f)),
    );
    path.cubic_to(
        (f32::from_bits(0xc177b158), f32::from_bits(0x427dc695)),
        (f32::from_bits(0x414b8e67), f32::from_bits(0x42801bb6)),
        (f32::from_bits(0x4207e581), f32::from_bits(0x4245d188)),
    );
    path.line_to((f32::from_bits(0x423bfd7a), f32::from_bits(0x4288d30e)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7901-L7925 (chrome/m156)
fn battleOp285(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x420374f9), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x427a8897), f32::from_bits(0xc27e6fb3)),
        (f32::from_bits(0x4297d5b1), f32::from_bits(0xc2063270)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b26718), f32::from_bits(0xc05f52ba)),
        (f32::from_bits(0x42a6ff00), f32::from_bits(0x41fc9e87)),
        (f32::from_bits(0x42757dbf), f32::from_bits(0x425f8353)),
    );
    path.line_to((f32::from_bits(0x423176ab), f32::from_bits(0x4221935e)));
    path.cubic_to(
        (f32::from_bits(0x427170b0), f32::from_bits(0x41b69dc5)),
        (f32::from_bits(0x4280f73f), f32::from_bits(0xc0217057)),
        (f32::from_bits(0x425b8525), f32::from_bits(0xc1c20512)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351bcc), f32::from_bits(0xc237ee0d)),
        (f32::from_bits(0x41be0ee4), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757dc1), f32::from_bits(0x425f8353)));
    path.cubic_to(
        (f32::from_bits(0x4264442b), f32::from_bits(0x42726e80)),
        (f32::from_bits(0x4250e985), f32::from_bits(0x4281a5dc)),
        (f32::from_bits(0x423bd072), f32::from_bits(0x4288e283)),
    );
    path.line_to((f32::from_bits(0x4207c4f4), f32::from_bits(0x4245e7df)));
    path.cubic_to(
        (f32::from_bits(0x42170559), f32::from_bits(0x423b7158)),
        (f32::from_bits(0x42250305), f32::from_bits(0x422f4076)),
        (f32::from_bits(0x423176ac), f32::from_bits(0x4221935e)),
    );
    path.line_to((f32::from_bits(0x42757dc1), f32::from_bits(0x425f8353)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7928-L7958 (chrome/m156)
fn battleOp286(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb630015b), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x420374f9), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x427a8897), f32::from_bits(0xc27e6fb3)),
        (f32::from_bits(0x4297d5b1), f32::from_bits(0xc2063270)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b26718), f32::from_bits(0xc05f52c1)),
        (f32::from_bits(0x42a6ff01), f32::from_bits(0x41fc9e87)),
        (f32::from_bits(0x42757dc1), f32::from_bits(0x425f8353)),
    );
    path.cubic_to(
        (f32::from_bits(0x4264442b), f32::from_bits(0x42726e80)),
        (f32::from_bits(0x4250e985), f32::from_bits(0x4281a5dc)),
        (f32::from_bits(0x423bd072), f32::from_bits(0x4288e283)),
    );
    path.line_to((f32::from_bits(0x4207c4f4), f32::from_bits(0x4245e7df)));
    path.cubic_to(
        (f32::from_bits(0x42170559), f32::from_bits(0x423b7158)),
        (f32::from_bits(0x42250305), f32::from_bits(0x422f4076)),
        (f32::from_bits(0x423176ab), f32::from_bits(0x4221935e)),
    );
    path.cubic_to(
        (f32::from_bits(0x427170b0), f32::from_bits(0x41b69dc5)),
        (f32::from_bits(0x4280f73f), f32::from_bits(0xc0217057)),
        (f32::from_bits(0x425b8525), f32::from_bits(0xc1c20512)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351bcc), f32::from_bits(0xc237ee0d)),
        (f32::from_bits(0x41be0ee4), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0xb630015b), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bd073), f32::from_bits(0x4288e283)));
    path.cubic_to(
        (f32::from_bits(0x418c461b), f32::from_bits(0x42b13ffc)),
        (f32::from_bits(0xc1abee9c), f32::from_bits(0x42af7fac)),
        (f32::from_bits(0xc247e775), f32::from_bits(0x42848907)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cebcd), f32::from_bits(0x423324c4)),
        (f32::from_bits(0xc2b40fb2), f32::from_bits(0x40f02474)),
        (f32::from_bits(0xc29d6d1c), f32::from_bits(0xc1d2a316)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286ca87), f32::from_bits(0xc270a7a6)),
        (f32::from_bits(0xc20ee3ea), f32::from_bits(0xc2a5f5e9)),
        (f32::from_bits(0xbd3ba09e), f32::from_bits(0xc2a5fffd)),
    );
    path.line_to((f32::from_bits(0xbd0796d7), f32::from_bits(0xc26ffffe)));
    path.cubic_to(
        (f32::from_bits(0xc1ce9695), f32::from_bits(0xc26ff16b)),
        (f32::from_bits(0xc242e0ee), f32::from_bits(0xc22df7a5)),
        (f32::from_bits(0xc2639aa3), f32::from_bits(0xc198448c)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822a2c), f32::from_bits(0x40ad98d0)),
        (f32::from_bits(0xc262dfac), f32::from_bits(0x4201805e)),
        (f32::from_bits(0xc2108243), f32::from_bits(0x423f9e03)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178936c), f32::from_bits(0x427dbba8)),
        (f32::from_bits(0x414ace5d), f32::from_bits(0x428021e8)),
        (f32::from_bits(0x4207c4fa), f32::from_bits(0x4245e7dc)),
    );
    path.line_to((f32::from_bits(0x423bd073), f32::from_bits(0x4288e283)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7961-L7985 (chrome/m156)
fn battleOp287(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x420377c9), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8d67), f32::from_bits(0xc27e6c6d)),
        (f32::from_bits(0x4297d744), f32::from_bits(0xc2062b59)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267d3), f32::from_bits(0xc05ea43d)),
        (f32::from_bits(0x42a6fd01), f32::from_bits(0x41fcb991)),
        (f32::from_bits(0x42757351), f32::from_bits(0x425f8ecb)),
    );
    path.line_to((f32::from_bits(0x42316f1e), f32::from_bits(0x42219ba8)));
    path.cubic_to(
        (f32::from_bits(0x42716dc9), f32::from_bits(0x41b6b154)),
        (f32::from_bits(0x4280f7c8), f32::from_bits(0xc020f212)),
        (f32::from_bits(0x425b876b), f32::from_bits(0xc1c1fad0)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351f48), f32::from_bits(0xc237ebae)),
        (f32::from_bits(0x41be12f9), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757350), f32::from_bits(0x425f8ecb)));
    path.cubic_to(
        (f32::from_bits(0x42643881), f32::from_bits(0x4272798e)),
        (f32::from_bits(0x4250dca0), f32::from_bits(0x4281ab15)),
        (f32::from_bits(0x423bc262), f32::from_bits(0x4288e756)),
    );
    path.line_to((f32::from_bits(0x4207bac8), f32::from_bits(0x4245eed9)));
    path.cubic_to(
        (f32::from_bits(0x4216fc05), f32::from_bits(0x423b78e5)),
        (f32::from_bits(0x4224fa94), f32::from_bits(0x422f4874)),
        (f32::from_bits(0x42316f1f), f32::from_bits(0x42219baa)),
    );
    path.line_to((f32::from_bits(0x42757350), f32::from_bits(0x425f8ecb)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L7988-L8019 (chrome/m156)
fn battleOp288(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x420377c9), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8d67), f32::from_bits(0xc27e6c6d)),
        (f32::from_bits(0x4297d744), f32::from_bits(0xc2062b59)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267d3), f32::from_bits(0xc05ea43d)),
        (f32::from_bits(0x42a6fd01), f32::from_bits(0x41fcb991)),
        (f32::from_bits(0x42757351), f32::from_bits(0x425f8ecb)),
    );
    path.line_to((f32::from_bits(0x423bc262), f32::from_bits(0x4288e756)));
    path.line_to((f32::from_bits(0x4207bac8), f32::from_bits(0x4245eed9)));
    path.cubic_to(
        (f32::from_bits(0x4216fc05), f32::from_bits(0x423b78e5)),
        (f32::from_bits(0x4224fa94), f32::from_bits(0x422f4874)),
        (f32::from_bits(0x42316f1f), f32::from_bits(0x42219baa)),
    );
    path.line_to((f32::from_bits(0x42316f1e), f32::from_bits(0x42219ba8)));
    path.cubic_to(
        (f32::from_bits(0x42716dc9), f32::from_bits(0x41b6b154)),
        (f32::from_bits(0x4280f7c8), f32::from_bits(0xc020f212)),
        (f32::from_bits(0x425b876b), f32::from_bits(0xc1c1fad0)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351f48), f32::from_bits(0xc237ebae)),
        (f32::from_bits(0x41be12f9), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc261), f32::from_bits(0x4288e756)));
    path.cubic_to(
        (f32::from_bits(0x418c1c95), f32::from_bits(0x42b142a6)),
        (f32::from_bits(0xc1ac1f7e), f32::from_bits(0x42af7d4d)),
        (f32::from_bits(0xc247fbc6), f32::from_bits(0x4284815d)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf3e6), f32::from_bits(0x42330ad8)),
        (f32::from_bits(0xc2b411b5), f32::from_bits(0x40ef163d)),
        (f32::from_bits(0xc29d67bc), f32::from_bits(0xc1d2e345)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bdc4), f32::from_bits(0xc270c60d)),
        (f32::from_bits(0xc20ebcc7), f32::from_bits(0xc2a5feff)),
        (f32::from_bits(0xbb958372), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0xbb591ee2), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce5e0c), f32::from_bits(0xc26ffe8b)),
        (f32::from_bits(0xc242ce80), f32::from_bits(0xc22e0d9d)),
        (f32::from_bits(0xc26392e3), f32::from_bits(0xc19872ed)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822ba3), f32::from_bits(0x40acd588)),
        (f32::from_bits(0xc262eb66), f32::from_bits(0x42016da1)),
        (f32::from_bits(0xc21090f8), f32::from_bits(0x423f92f0)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178da2a), f32::from_bits(0x427db83e)),
        (f32::from_bits(0x414a923f), f32::from_bits(0x428023d8)),
        (f32::from_bits(0x4207baca), f32::from_bits(0x4245eed8)),
    );
    path.line_to((f32::from_bits(0x423bc261), f32::from_bits(0x4288e756)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8022-L8046 (chrome/m156)
fn battleOp289(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8049-L8080 (chrome/m156)
fn battleOp290(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8083-L8107 (chrome/m156)
fn battleOp291(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8110-L8141 (chrome/m156)
fn battleOp292(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8144-L8168 (chrome/m156)
fn battleOp293(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8171-L8202 (chrome/m156)
fn battleOp294(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8205-L8229 (chrome/m156)
fn battleOp295(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8232-L8263 (chrome/m156)
fn battleOp296(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8266-L8290 (chrome/m156)
fn battleOp297(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8293-L8324 (chrome/m156)
fn battleOp298(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8327-L8351 (chrome/m156)
fn battleOp299(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8354-L8385 (chrome/m156)
fn battleOp300(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8388-L8412 (chrome/m156)
fn battleOp301(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8415-L8446 (chrome/m156)
fn battleOp302(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8449-L8473 (chrome/m156)
fn battleOp303(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8476-L8507 (chrome/m156)
fn battleOp304(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8510-L8534 (chrome/m156)
fn battleOp305(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8537-L8568 (chrome/m156)
fn battleOp306(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8571-L8595 (chrome/m156)
fn battleOp307(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8598-L8629 (chrome/m156)
fn battleOp308(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8632-L8656 (chrome/m156)
fn battleOp309(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8659-L8690 (chrome/m156)
fn battleOp310(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8693-L8717 (chrome/m156)
fn battleOp311(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8720-L8751 (chrome/m156)
fn battleOp312(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8754-L8778 (chrome/m156)
fn battleOp313(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8781-L8812 (chrome/m156)
fn battleOp314(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8815-L8839 (chrome/m156)
fn battleOp315(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8842-L8873 (chrome/m156)
fn battleOp316(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8876-L8900 (chrome/m156)
fn battleOp317(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8903-L8934 (chrome/m156)
fn battleOp318(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8937-L8961 (chrome/m156)
fn battleOp319(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8964-L8995 (chrome/m156)
fn battleOp320(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L8998-L9022 (chrome/m156)
fn battleOp321(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9025-L9056 (chrome/m156)
fn battleOp322(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9059-L9083 (chrome/m156)
fn battleOp323(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9086-L9117 (chrome/m156)
fn battleOp324(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9120-L9144 (chrome/m156)
fn battleOp325(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9147-L9178 (chrome/m156)
fn battleOp326(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9181-L9205 (chrome/m156)
fn battleOp327(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9208-L9239 (chrome/m156)
fn battleOp328(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9242-L9266 (chrome/m156)
fn battleOp329(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9269-L9300 (chrome/m156)
fn battleOp330(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9303-L9327 (chrome/m156)
fn battleOp331(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9330-L9361 (chrome/m156)
fn battleOp332(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9364-L9388 (chrome/m156)
fn battleOp333(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9391-L9422 (chrome/m156)
fn battleOp334(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9425-L9449 (chrome/m156)
fn battleOp335(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9452-L9483 (chrome/m156)
fn battleOp336(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9486-L9510 (chrome/m156)
fn battleOp337(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9513-L9544 (chrome/m156)
fn battleOp338(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9547-L9571 (chrome/m156)
fn battleOp339(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9574-L9605 (chrome/m156)
fn battleOp340(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9608-L9632 (chrome/m156)
fn battleOp341(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9635-L9666 (chrome/m156)
fn battleOp342(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9669-L9693 (chrome/m156)
fn battleOp343(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9696-L9727 (chrome/m156)
fn battleOp344(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9730-L9754 (chrome/m156)
fn battleOp345(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9757-L9788 (chrome/m156)
fn battleOp346(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9791-L9813 (chrome/m156)
fn battleOp347(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3d570205), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3dd7026d), f32::from_bits(0xc2a5fffa)),
        (f32::from_bits(0x3e2141e6), f32::from_bits(0xc2a5ffed)),
    );
    path.line_to((f32::from_bits(0x3de92565), f32::from_bits(0xc26fffe4)));
    path.cubic_to(
        (f32::from_bits(0x3d9b6fac), f32::from_bits(0xc26ffff9)),
        (f32::from_bits(0x3d1b715b), f32::from_bits(0xc2700002)),
        (f32::from_bits(0x365677c0), f32::from_bits(0xc2700002)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3e214267), f32::from_bits(0xc2a5ffec)));
    path.cubic_to(
        (f32::from_bits(0x3e26a1f2), f32::from_bits(0xc2a5ffeb)),
        (f32::from_bits(0x3e2c025b), f32::from_bits(0xc2a5ffe9)),
        (f32::from_bits(0x3e3162c6), f32::from_bits(0xc2a5ffe7)),
    );
    path.line_to((f32::from_bits(0x3e003af5), f32::from_bits(0xc26fffde)));
    path.cubic_to(
        (f32::from_bits(0x3df8b0d2), f32::from_bits(0xc26fffe0)),
        (f32::from_bits(0x3df0ead2), f32::from_bits(0xc26fffe2)),
        (f32::from_bits(0x3de924d4), f32::from_bits(0xc26fffe4)),
    );
    path.line_to((f32::from_bits(0x3e214267), f32::from_bits(0xc2a5ffec)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9816-L9843 (chrome/m156)
fn battleOp348(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x365677c0), f32::from_bits(0xc2700002)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3d570205), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3dd7026d), f32::from_bits(0xc2a5fffa)),
        (f32::from_bits(0x3e2141e6), f32::from_bits(0xc2a5ffed)),
    );
    path.line_to((f32::from_bits(0x3e0492ca), f32::from_bits(0xc28878a2)));
    path.line_to((f32::from_bits(0x3e214267), f32::from_bits(0xc2a5ffec)));
    path.cubic_to(
        (f32::from_bits(0x3e26a1f2), f32::from_bits(0xc2a5ffeb)),
        (f32::from_bits(0x3e2c025b), f32::from_bits(0xc2a5ffe9)),
        (f32::from_bits(0x3e3162c6), f32::from_bits(0xc2a5ffe7)),
    );
    path.line_to((f32::from_bits(0x3e003af5), f32::from_bits(0xc26fffde)));
    path.line_to((f32::from_bits(0x3de92565), f32::from_bits(0xc26fffe4)));
    path.line_to((f32::from_bits(0x3de924d4), f32::from_bits(0xc26fffe4)));
    path.cubic_to(
        (f32::from_bits(0x3d9b6f4b), f32::from_bits(0xc26ffff9)),
        (f32::from_bits(0x3d1b70fa), f32::from_bits(0xc2700002)),
        (f32::from_bits(0x365677c0), f32::from_bits(0xc2700002)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3e3162a4), f32::from_bits(0xc2a5ffe8)));
    path.cubic_to(
        (f32::from_bits(0x3e843f51), f32::from_bits(0xc2a5ffd1)),
        (f32::from_bits(0x3eafcce9), f32::from_bits(0xc2a5ffa8)),
        (f32::from_bits(0x3edb5a6f), f32::from_bits(0xc2a5ff6f)),
    );
    path.line_to((f32::from_bits(0x3e9e9160), f32::from_bits(0xc26fff2e)));
    path.cubic_to(
        (f32::from_bits(0x3e7e2aec), f32::from_bits(0xc26fff82)),
        (f32::from_bits(0x3e3f3306), f32::from_bits(0xc26fffbd)),
        (f32::from_bits(0x3e003b0e), f32::from_bits(0xc26fffdf)),
    );
    path.line_to((f32::from_bits(0x3e3162a4), f32::from_bits(0xc2a5ffe8)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9846-L9868 (chrome/m156)
fn battleOp349(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e678fda), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3ee78f7d), f32::from_bits(0xc2a5ff87)),
        (f32::from_bits(0x3f2dab18), f32::from_bits(0xc2a5fe96)),
    );
    path.line_to((f32::from_bits(0x3efb15d4), f32::from_bits(0xc26ffdf3)));
    path.cubic_to(
        (f32::from_bits(0x3ea764ab), f32::from_bits(0xc26fff52)),
        (f32::from_bits(0x3e2764f3), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x35c73da0), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f2daad3), f32::from_bits(0xc2a5fe95)));
    path.cubic_to(
        (f32::from_bits(0x3f3374d8), f32::from_bits(0xc2a5fe7b)),
        (f32::from_bits(0x3f393eae), f32::from_bits(0xc2a5fe62)),
        (f32::from_bits(0x3f3f0885), f32::from_bits(0xc2a5fe46)),
    );
    path.line_to((f32::from_bits(0x3f0a18b8), f32::from_bits(0xc26ffd84)));
    path.cubic_to(
        (f32::from_bits(0x3f05e964), f32::from_bits(0xc26ffdad)),
        (f32::from_bits(0x3f01ba2f), f32::from_bits(0xc26ffdd1)),
        (f32::from_bits(0x3efb15f0), f32::from_bits(0xc26ffdf5)),
    );
    path.line_to((f32::from_bits(0x3f2daad3), f32::from_bits(0xc2a5fe95)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9871-L9896 (chrome/m156)
fn battleOp350(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e678fda), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3ee78f7d), f32::from_bits(0xc2a5ff87)),
        (f32::from_bits(0x3f2dab18), f32::from_bits(0xc2a5fe96)),
    );
    path.cubic_to(
        (f32::from_bits(0x3f3374d8), f32::from_bits(0xc2a5fe7b)),
        (f32::from_bits(0x3f393eae), f32::from_bits(0xc2a5fe62)),
        (f32::from_bits(0x3f3f0885), f32::from_bits(0xc2a5fe46)),
    );
    path.line_to((f32::from_bits(0x3f0a18b8), f32::from_bits(0xc26ffd84)));
    path.cubic_to(
        (f32::from_bits(0x3f05e964), f32::from_bits(0xc26ffdad)),
        (f32::from_bits(0x3f01ba2f), f32::from_bits(0xc26ffdd1)),
        (f32::from_bits(0x3efb15f0), f32::from_bits(0xc26ffdf5)),
    );
    path.line_to((f32::from_bits(0x3efb15d4), f32::from_bits(0xc26ffdf3)));
    path.cubic_to(
        (f32::from_bits(0x3ea764ab), f32::from_bits(0xc26fff52)),
        (f32::from_bits(0x3e2764f3), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f3f0899), f32::from_bits(0xc2a5fe48)));
    path.cubic_to(
        (f32::from_bits(0x3f8e6b81), f32::from_bits(0xc2a5fc98)),
        (f32::from_bits(0x3fbd51fb), f32::from_bits(0xc2a5f9aa)),
        (f32::from_bits(0x3fec36d3), f32::from_bits(0xc2a5f57e)),
    );
    path.line_to((f32::from_bits(0x3faac1d7), f32::from_bits(0xc26ff0d0)));
    path.cubic_to(
        (f32::from_bits(0x3f88dbac), f32::from_bits(0xc26ff6d7)),
        (f32::from_bits(0x3f4de8bb), f32::from_bits(0xc26ffb13)),
        (f32::from_bits(0x3f0a18e7), f32::from_bits(0xc26ffd83)),
    );
    path.line_to((f32::from_bits(0x3f3f0899), f32::from_bits(0xc2a5fe48)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9899-L9921 (chrome/m156)
fn battleOp351(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x403f62fc), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x40bf510b), f32::from_bits(0xc2a5ad41)),
        (f32::from_bits(0x410f39cc), f32::from_bits(0xc2a50821)),
    );
    path.line_to((f32::from_bits(0x40cf12cc), f32::from_bits(0xc26e99a0)));
    path.cubic_to(
        (f32::from_bits(0x408a4d18), f32::from_bits(0xc26f885f)),
        (f32::from_bits(0x400a5a13), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36a6ff52), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x410f39cd), f32::from_bits(0xc2a50820)));
    path.cubic_to(
        (f32::from_bits(0x4113fb3b), f32::from_bits(0xc2a4f79d)),
        (f32::from_bits(0x4118bbf1), f32::from_bits(0xc2a4e648)),
        (f32::from_bits(0x411d7be1), f32::from_bits(0xc2a4d421)),
    );
    path.line_to((f32::from_bits(0x40e3b008), f32::from_bits(0xc26e4e75)));
    path.cubic_to(
        (f32::from_bits(0x40dcd206), f32::from_bits(0xc26e68b4)),
        (f32::from_bits(0x40d5f2eb), f32::from_bits(0xc26e81c3)),
        (f32::from_bits(0x40cf12c6), f32::from_bits(0xc26e99a1)),
    );
    path.line_to((f32::from_bits(0x410f39cd), f32::from_bits(0xc2a50820)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9923-L9949 (chrome/m156)
fn battleOp352(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e0b17a8), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3e8b179e), f32::from_bits(0xc2a5ffd4)),
        (f32::from_bits(0x3ed0a337), f32::from_bits(0xc2a5ff7c)),
    );
    path.line_to((f32::from_bits(0x3ed0a338), f32::from_bits(0xc2a5ff7d)));
    path.cubic_to(
        (f32::from_bits(0x3ed797a0), f32::from_bits(0xc2a5ff73)),
        (f32::from_bits(0x3ede8c36), f32::from_bits(0xc2a5ff6a)),
        (f32::from_bits(0x3ee580cb), f32::from_bits(0xc2a5ff60)),
    );
    path.line_to((f32::from_bits(0x3ea5e78a), f32::from_bits(0xc26fff1b)));
    path.cubic_to(
        (f32::from_bits(0x3ea0e0bb), f32::from_bits(0xc26fff29)),
        (f32::from_bits(0x3e9bd9a1), f32::from_bits(0xc26fff36)),
        (f32::from_bits(0x3e96d286), f32::from_bits(0xc26fff43)),
    );
    path.line_to((f32::from_bits(0x3e96d285), f32::from_bits(0xc26fff42)));
    path.cubic_to(
        (f32::from_bits(0x3e491945), f32::from_bits(0xc26fffc2)),
        (f32::from_bits(0x3dc91958), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ee58048), f32::from_bits(0xc2a5ff61)));
    path.cubic_to(
        (f32::from_bits(0x3f2b1987), f32::from_bits(0xc2a5fec4)),
        (f32::from_bits(0x3f637253), f32::from_bits(0xc2a5fdb6)),
        (f32::from_bits(0x3f8de535), f32::from_bits(0xc2a5fc35)),
    );
    path.line_to((f32::from_bits(0x3f4d269a), f32::from_bits(0xc26ffa85)));
    path.cubic_to(
        (f32::from_bits(0x3f246b51), f32::from_bits(0xc26ffcb3)),
        (f32::from_bits(0x3ef75f30), f32::from_bits(0xc26ffe3a)),
        (f32::from_bits(0x3ea5e737), f32::from_bits(0xc26fff1c)),
    );
    path.line_to((f32::from_bits(0x3ee58048), f32::from_bits(0xc2a5ff61)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9951-L9981 (chrome/m156)
fn battleOp1390(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0xb7240057), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x420377ff), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x427a8dc0), f32::from_bits(0xc27e6c2f)),
        (f32::from_bits(0x4297d760), f32::from_bits(0xc2062ad2)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e1), f32::from_bits(0xc05e974f)),
        (f32::from_bits(0x42a6fcda), f32::from_bits(0x41fcbb92)),
        (f32::from_bits(0x42757289), f32::from_bits(0x425f8fa5)),
    );
    path.cubic_to(
        (f32::from_bits(0x426437a0), f32::from_bits(0x42727a5f)),
        (f32::from_bits(0x4250dbaa), f32::from_bits(0x4281ab79)),
        (f32::from_bits(0x423bc155), f32::from_bits(0x4288e7b2)),
    );
    path.line_to((f32::from_bits(0x4207ba06), f32::from_bits(0x4245ef5e)));
    path.cubic_to(
        (f32::from_bits(0x4216fb52), f32::from_bits(0x423b7973)),
        (f32::from_bits(0x4224f9f2), f32::from_bits(0x422f490a)),
        (f32::from_bits(0x42316e8e), f32::from_bits(0x42219c46)),
    );
    path.cubic_to(
        (f32::from_bits(0x42716d91), f32::from_bits(0x41b6b2c9)),
        (f32::from_bits(0x4280f7d1), f32::from_bits(0xc020e8c8)),
        (f32::from_bits(0x425b8794), f32::from_bits(0xc1c1fa0e)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351f87), f32::from_bits(0xc237eb83)),
        (f32::from_bits(0x41be1342), f32::from_bits(0xc2700002)),
        (f32::from_bits(0xb7240057), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc156), f32::from_bits(0x4288e7b2)));
    path.cubic_to(
        (f32::from_bits(0x418c1984), f32::from_bits(0x42b142da)),
        (f32::from_bits(0xc1ac2314), f32::from_bits(0x42af7d21)),
        (f32::from_bits(0xc247fd43), f32::from_bits(0x428480ce)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf47f), f32::from_bits(0x423308f3)),
        (f32::from_bits(0xc2b411dd), f32::from_bits(0x40ef0242)),
        (f32::from_bits(0xc29d6757), f32::from_bits(0xc1d2e807)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bcd2), f32::from_bits(0xc270c84c)),
        (f32::from_bits(0xc20eb9e2), f32::from_bits(0xc2a5ffaa)),
        (f32::from_bits(0xbac6f0ca), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0xba901698), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce59d7), f32::from_bits(0xc26fff83)),
        (f32::from_bits(0xc242cd21), f32::from_bits(0xc22e0f3f)),
        (f32::from_bits(0xc263924f), f32::from_bits(0xc1987661)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bbf), f32::from_bits(0x40acc6fd)),
        (f32::from_bits(0xc262ec43), f32::from_bits(0x42016c3b)),
        (f32::from_bits(0xc2109210), f32::from_bits(0x423f921c)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178df72), f32::from_bits(0x427db7fc)),
        (f32::from_bits(0x414a8dba), f32::from_bits(0x428023fd)),
        (f32::from_bits(0x4207ba05), f32::from_bits(0x4245ef60)),
    );
    path.line_to((f32::from_bits(0x423bc156), f32::from_bits(0x4288e7b2)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L9984-L10008 (chrome/m156)
fn battleOp1391(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10011-L10042 (chrome/m156)
fn battleOp1392(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10045-L10067 (chrome/m156)
fn battleOp1393(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3c436965), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3cc36072), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3d128619), f32::from_bits(0xc2a5fffe)),
    );
    path.line_to((f32::from_bits(0x3cd3db06), f32::from_bits(0xc26fffff)));
    path.cubic_to(
        (f32::from_bits(0x3c8d3d03), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3c0d4407), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36606a00), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3d12888d), f32::from_bits(0xc2a5ffff)));
    path.cubic_to(
        (f32::from_bits(0x3d176d55), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3d1c4dcb), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3d212e40), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x3ce90a84), f32::from_bits(0xc26ffffe)));
    path.cubic_to(
        (f32::from_bits(0x3ce1ffb6), f32::from_bits(0xc26ffffe)),
        (f32::from_bits(0x3cdaedb6), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3cd3dbb7), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x3d12888d), f32::from_bits(0xc2a5ffff)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10070-L10095 (chrome/m156)
fn battleOp1394(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x36606a00), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3c436965), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3cc36072), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3d128619), f32::from_bits(0xc2a5fffe)),
    );
    path.line_to((f32::from_bits(0x3d12888d), f32::from_bits(0xc2a5ffff)));
    path.line_to((f32::from_bits(0x3d212e40), f32::from_bits(0xc2a5ffff)));
    path.line_to((f32::from_bits(0x3ce90a84), f32::from_bits(0xc26ffffe)));
    path.cubic_to(
        (f32::from_bits(0x3ce1ffb6), f32::from_bits(0xc26ffffe)),
        (f32::from_bits(0x3cdaedb6), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3cd3db06), f32::from_bits(0xc26fffff)),
    );
    path.cubic_to(
        (f32::from_bits(0x3c8d3d03), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3c0d4407), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x36606a00), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3d212fd0), f32::from_bits(0xc2a5ffff)));
    path.cubic_to(
        (f32::from_bits(0x3d705530), f32::from_bits(0xc2a5fffe)),
        (f32::from_bits(0x3d9fbf82), f32::from_bits(0xc2a5fffc)),
        (f32::from_bits(0x3dc7546b), f32::from_bits(0xc2a5fffa)),
    );
    path.line_to((f32::from_bits(0x3d901696), f32::from_bits(0xc26ffff5)));
    path.cubic_to(
        (f32::from_bits(0x3d66f230), f32::from_bits(0xc26ffff9)),
        (f32::from_bits(0x3d2dbab1), f32::from_bits(0xc26ffffc)),
        (f32::from_bits(0x3ce90664), f32::from_bits(0xc26ffffe)),
    );
    path.line_to((f32::from_bits(0x3d212fd0), f32::from_bits(0xc2a5ffff)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10098-L10120 (chrome/m156)
fn battleOp1395(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e06023f), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3e860192), f32::from_bits(0xc2a5ffd6)),
        (f32::from_bits(0x3ec901db), f32::from_bits(0xc2a5ff85)),
    );
    path.line_to((f32::from_bits(0x3e914e16), f32::from_bits(0xc26fff50)));
    path.cubic_to(
        (f32::from_bits(0x3e41bddf), f32::from_bits(0xc26fffc5)),
        (f32::from_bits(0x3dc1be4c), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x35c55da0), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ec9015b), f32::from_bits(0xc2a5ff86)));
    path.cubic_to(
        (f32::from_bits(0x3ecfb4f0), f32::from_bits(0xc2a5ff7d)),
        (f32::from_bits(0x3ed66842), f32::from_bits(0xc2a5ff75)),
        (f32::from_bits(0x3edd1b92), f32::from_bits(0xc2a5ff6c)),
    );
    path.line_to((f32::from_bits(0x3e9fd5de), f32::from_bits(0xc26fff2b)));
    path.cubic_to(
        (f32::from_bits(0x3e9afe3a), f32::from_bits(0xc26fff39)),
        (f32::from_bits(0x3e96263d), f32::from_bits(0xc26fff45)),
        (f32::from_bits(0x3e914e41), f32::from_bits(0xc26fff51)),
    );
    path.line_to((f32::from_bits(0x3ec9015b), f32::from_bits(0xc2a5ff86)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10123-L10148 (chrome/m156)
fn battleOp1396(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e0601e9), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3e86013c), f32::from_bits(0xc2a5ffd6)),
        (f32::from_bits(0x3ec9015a), f32::from_bits(0xc2a5ff85)),
    );
    path.line_to((f32::from_bits(0x3ec9015b), f32::from_bits(0xc2a5ff86)));
    path.cubic_to(
        (f32::from_bits(0x3ecfb4f0), f32::from_bits(0xc2a5ff7d)),
        (f32::from_bits(0x3ed66842), f32::from_bits(0xc2a5ff75)),
        (f32::from_bits(0x3edd1b92), f32::from_bits(0xc2a5ff6c)),
    );
    path.line_to((f32::from_bits(0x3e9fd5de), f32::from_bits(0xc26fff2b)));
    path.cubic_to(
        (f32::from_bits(0x3e9afe3a), f32::from_bits(0xc26fff39)),
        (f32::from_bits(0x3e96263d), f32::from_bits(0xc26fff45)),
        (f32::from_bits(0x3e914e16), f32::from_bits(0xc26fff50)),
    );
    path.cubic_to(
        (f32::from_bits(0x3e41bddf), f32::from_bits(0xc26fffc5)),
        (f32::from_bits(0x3dc1be4c), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3edd1b0d), f32::from_bits(0xc2a5ff6d)));
    path.cubic_to(
        (f32::from_bits(0x3f24d70e), f32::from_bits(0xc2a5fedc)),
        (f32::from_bits(0x3f5b204e), f32::from_bits(0xc2a5fde1)),
        (f32::from_bits(0x3f88b475), f32::from_bits(0xc2a5fc7b)),
    );
    path.line_to((f32::from_bits(0x3f45a57e), f32::from_bits(0xc26ffaea)));
    path.cubic_to(
        (f32::from_bits(0x3f1e67a6), f32::from_bits(0xc26ffcf1)),
        (f32::from_bits(0x3eee52e7), f32::from_bits(0xc26ffe5c)),
        (f32::from_bits(0x3e9fd606), f32::from_bits(0xc26fff2d)),
    );
    path.line_to((f32::from_bits(0x3edd1b0d), f32::from_bits(0xc2a5ff6d)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10150-L10172 (chrome/m156)
fn battleOp2193(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e3881bc), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3eb88238), f32::from_bits(0xc2a5ffb3)),
        (f32::from_bits(0x3f0a6190), f32::from_bits(0xc2a5ff19)),
    );
    path.line_to((f32::from_bits(0x3ec8119b), f32::from_bits(0xc26ffeb2)));
    path.cubic_to(
        (f32::from_bits(0x3e856151), f32::from_bits(0xc26fff91)),
        (f32::from_bits(0x3e0561b2), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3629eed0), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f0a6183), f32::from_bits(0xc2a5ff19)));
    path.cubic_to(
        (f32::from_bits(0x3f0efe46), f32::from_bits(0xc2a5ff0a)),
        (f32::from_bits(0x3f139b44), f32::from_bits(0xc2a5fef9)),
        (f32::from_bits(0x3f183842), f32::from_bits(0xc2a5fee9)),
    );
    path.line_to((f32::from_bits(0x3edc1349), f32::from_bits(0xc26ffe6c)));
    path.cubic_to(
        (f32::from_bits(0x3ed567f5), f32::from_bits(0xc26ffe84)),
        (f32::from_bits(0x3ecebccf), f32::from_bits(0xc26ffe9c)),
        (f32::from_bits(0x3ec811a8), f32::from_bits(0xc26ffeb2)),
    );
    path.line_to((f32::from_bits(0x3f0a6183), f32::from_bits(0xc2a5ff19)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10175-L10201 (chrome/m156)
fn battleOp2194(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3629eed0), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e3881ab), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3eb88227), f32::from_bits(0xc2a5ffb3)),
        (f32::from_bits(0x3f0a6183), f32::from_bits(0xc2a5ff19)),
    );
    path.line_to((f32::from_bits(0x3f0a6190), f32::from_bits(0xc2a5ff19)));
    path.cubic_to(
        (f32::from_bits(0x3f0efe4f), f32::from_bits(0xc2a5ff0a)),
        (f32::from_bits(0x3f139b48), f32::from_bits(0xc2a5fef9)),
        (f32::from_bits(0x3f183842), f32::from_bits(0xc2a5fee9)),
    );
    path.line_to((f32::from_bits(0x3edc1349), f32::from_bits(0xc26ffe6c)));
    path.cubic_to(
        (f32::from_bits(0x3ed567f5), f32::from_bits(0xc26ffe84)),
        (f32::from_bits(0x3ecebccf), f32::from_bits(0xc26ffe9c)),
        (f32::from_bits(0x3ec811a8), f32::from_bits(0xc26ffeb2)),
    );
    path.line_to((f32::from_bits(0x3ec8119b), f32::from_bits(0xc26ffeb2)));
    path.cubic_to(
        (f32::from_bits(0x3e856151), f32::from_bits(0xc26fff91)),
        (f32::from_bits(0x3e0561b2), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3629eed0), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f183800), f32::from_bits(0xc2a5fee9)));
    path.cubic_to(
        (f32::from_bits(0x3f62f7a2), f32::from_bits(0xc2a5fdd7)),
        (f32::from_bits(0x3f96db12), f32::from_bits(0xc2a5fbfa)),
        (f32::from_bits(0x3fbc3981), f32::from_bits(0xc2a5f954)),
    );
    path.line_to((f32::from_bits(0x3f8810cc), f32::from_bits(0xc26ff65b)));
    path.cubic_to(
        (f32::from_bits(0x3f5a1a86), f32::from_bits(0xc26ffa2f)),
        (f32::from_bits(0x3f241256), f32::from_bits(0xc26ffcdf)),
        (f32::from_bits(0x3edc1312), f32::from_bits(0xc26ffe6c)),
    );
    path.line_to((f32::from_bits(0x3f183800), f32::from_bits(0xc2a5fee9)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10203-L10234 (chrome/m156)
fn battleOp3368(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10237-L10261 (chrome/m156)
fn battleOp3369(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10264-L10295 (chrome/m156)
fn battleOp3370(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10298-L10320 (chrome/m156)
fn battleOp3371(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3c85f8a2), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3d05fda5), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3d48fefa), f32::from_bits(0xc2a5fffd)),
    );
    path.line_to((f32::from_bits(0x3d114e3a), f32::from_bits(0xc26ffffd)));
    path.cubic_to(
        (f32::from_bits(0x3cc1c2c0), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3c41c57e), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x35afaa00), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3d49018c), f32::from_bits(0xc2a5fffe)));
    path.cubic_to(
        (f32::from_bits(0x3d4fb7df), f32::from_bits(0xc2a5fffd)),
        (f32::from_bits(0x3d5667bf), f32::from_bits(0xc2a5fffd)),
        (f32::from_bits(0x3d5d179f), f32::from_bits(0xc2a5fffd)),
    );
    path.line_to((f32::from_bits(0x3d1fd60d), f32::from_bits(0xc26ffffd)));
    path.cubic_to(
        (f32::from_bits(0x3d1afde4), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3d162864), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3d1152e4), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x3d49018c), f32::from_bits(0xc2a5fffe)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10323-L10349 (chrome/m156)
fn battleOp3372(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3c85f8a2), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3d05fda5), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3d48fefa), f32::from_bits(0xc2a5fffd)),
    );
    path.line_to((f32::from_bits(0x3d49018c), f32::from_bits(0xc2a5fffe)));
    path.cubic_to(
        (f32::from_bits(0x3d4fb7df), f32::from_bits(0xc2a5fffd)),
        (f32::from_bits(0x3d5667bf), f32::from_bits(0xc2a5fffd)),
        (f32::from_bits(0x3d5d179f), f32::from_bits(0xc2a5fffd)),
    );
    path.line_to((f32::from_bits(0x3d1fd60d), f32::from_bits(0xc26ffffd)));
    path.cubic_to(
        (f32::from_bits(0x3d1afde4), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3d162864), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3d1152e4), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x3d114e3a), f32::from_bits(0xc26ffffd)));
    path.cubic_to(
        (f32::from_bits(0x3cc1c2c0), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x3c41c57e), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3d5d1b4e), f32::from_bits(0xc2a5fffe)));
    path.cubic_to(
        (f32::from_bits(0x3da4d661), f32::from_bits(0xc2a5fffc)),
        (f32::from_bits(0x3ddb1fb1), f32::from_bits(0xc2a5fff8)),
        (f32::from_bits(0x3e08b47e), f32::from_bits(0xc2a5fff2)),
    );
    path.line_to((f32::from_bits(0x3dc5a6e0), f32::from_bits(0xc26fffec)));
    path.cubic_to(
        (f32::from_bits(0x3d9e671d), f32::from_bits(0xc26ffff6)),
        (f32::from_bits(0x3d6e51bc), f32::from_bits(0xc26ffffb)),
        (f32::from_bits(0x3d1fd53d), f32::from_bits(0xc26ffffe)),
    );
    path.line_to((f32::from_bits(0x3d5d1b4e), f32::from_bits(0xc2a5fffe)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10351-L10382 (chrome/m156)
fn battleOp4290(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10385-L10409 (chrome/m156)
fn battleOp4291(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10412-L10443 (chrome/m156)
fn battleOp4292(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10446-L10470 (chrome/m156)
fn battleOp4293(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x357ffa94), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.cubic_to(
        (f32::from_bits(0x42643732), f32::from_bits(0x42727ac8)),
        (f32::from_bits(0x4250db30), f32::from_bits(0x4281abaa)),
        (f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)),
    );
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42757226), f32::from_bits(0x425f9012)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10473-L10504 (chrome/m156)
fn battleOp4294(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x42037818), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x427a8dee), f32::from_bits(0xc27e6c10)),
        (f32::from_bits(0x4297d76f), f32::from_bits(0xc2062a8f)),
    );
    path.cubic_to(
        (f32::from_bits(0x42b267e8), f32::from_bits(0xc05e90e8)),
        (f32::from_bits(0x42a6fcc7), f32::from_bits(0x41fcbc94)),
        (f32::from_bits(0x42757227), f32::from_bits(0x425f9011)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.line_to((f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)));
    path.cubic_to(
        (f32::from_bits(0x4216fafb), f32::from_bits(0x423b79ba)),
        (f32::from_bits(0x4224f9a4), f32::from_bits(0x422f4956)),
        (f32::from_bits(0x42316e48), f32::from_bits(0x42219c94)),
    );
    path.line_to((f32::from_bits(0x42316e47), f32::from_bits(0x42219c94)));
    path.cubic_to(
        (f32::from_bits(0x42716d77), f32::from_bits(0x41b6b381)),
        (f32::from_bits(0x4280f7d6), f32::from_bits(0xc020e418)),
        (f32::from_bits(0x425b87ab), f32::from_bits(0xc1c1f9ac)),
    );
    path.cubic_to(
        (f32::from_bits(0x42351faa), f32::from_bits(0xc237eb6b)),
        (f32::from_bits(0x41be136b), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.cubic_to(
        (f32::from_bits(0x418c17fd), f32::from_bits(0x42b142f1)),
        (f32::from_bits(0xc1ac24e4), f32::from_bits(0x42af7d09)),
        (f32::from_bits(0xc247fe03), f32::from_bits(0x42848083)),
    );
    path.cubic_to(
        (f32::from_bits(0xc29cf4c9), f32::from_bits(0x423307fa)),
        (f32::from_bits(0xc2b411ee), f32::from_bits(0x40eef84a)),
        (f32::from_bits(0xc29d6723), f32::from_bits(0xc1d2ea61)),
    );
    path.cubic_to(
        (f32::from_bits(0xc286bc59), f32::from_bits(0xc270c968)),
        (f32::from_bits(0xc20eb871), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0xb5c727ee), f32::from_bits(0xc2a5ffff)),
    );
    path.line_to((f32::from_bits(0x293e5cb4), f32::from_bits(0xc2700000)));
    path.cubic_to(
        (f32::from_bits(0xc1ce57c4), f32::from_bits(0xc2700000)),
        (f32::from_bits(0xc242cc76), f32::from_bits(0xc22e100c)),
        (f32::from_bits(0xc2639208), f32::from_bits(0xc1987810)),
    );
    path.cubic_to(
        (f32::from_bits(0xc2822bcd), f32::from_bits(0x40acbfe2)),
        (f32::from_bits(0xc262ecb3), f32::from_bits(0x42016b8c)),
        (f32::from_bits(0xc210929c), f32::from_bits(0x423f91b4)),
    );
    path.cubic_to(
        (f32::from_bits(0xc178e211), f32::from_bits(0x427db7dc)),
        (f32::from_bits(0x414a8b85), f32::from_bits(0x4280240f)),
        (f32::from_bits(0x4207b9a6), f32::from_bits(0x4245efa0)),
    );
    path.line_to((f32::from_bits(0x423bc0d1), f32::from_bits(0x4288e7e0)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10507-L10529 (chrome/m156)
fn battleOp4295(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e3881bc), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3eb88238), f32::from_bits(0xc2a5ffb3)),
        (f32::from_bits(0x3f0a6190), f32::from_bits(0xc2a5ff19)),
    );
    path.line_to((f32::from_bits(0x3ec8119b), f32::from_bits(0xc26ffeb2)));
    path.cubic_to(
        (f32::from_bits(0x3e856151), f32::from_bits(0xc26fff91)),
        (f32::from_bits(0x3e0561b2), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3629eed0), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f0a6183), f32::from_bits(0xc2a5ff19)));
    path.cubic_to(
        (f32::from_bits(0x3f0efe46), f32::from_bits(0xc2a5ff0a)),
        (f32::from_bits(0x3f139b44), f32::from_bits(0xc2a5fef9)),
        (f32::from_bits(0x3f183842), f32::from_bits(0xc2a5fee9)),
    );
    path.line_to((f32::from_bits(0x3edc1349), f32::from_bits(0xc26ffe6c)));
    path.cubic_to(
        (f32::from_bits(0x3ed567f5), f32::from_bits(0xc26ffe84)),
        (f32::from_bits(0x3ecebccf), f32::from_bits(0xc26ffe9c)),
        (f32::from_bits(0x3ec811a8), f32::from_bits(0xc26ffeb2)),
    );
    path.line_to((f32::from_bits(0x3f0a6183), f32::from_bits(0xc2a5ff19)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10532-L10558 (chrome/m156)
fn battleOp4296(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x3629eed0), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e3881ab), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3eb88227), f32::from_bits(0xc2a5ffb3)),
        (f32::from_bits(0x3f0a6183), f32::from_bits(0xc2a5ff19)),
    );
    path.line_to((f32::from_bits(0x3f0a6190), f32::from_bits(0xc2a5ff19)));
    path.cubic_to(
        (f32::from_bits(0x3f0efe4f), f32::from_bits(0xc2a5ff0a)),
        (f32::from_bits(0x3f139b48), f32::from_bits(0xc2a5fef9)),
        (f32::from_bits(0x3f183842), f32::from_bits(0xc2a5fee9)),
    );
    path.line_to((f32::from_bits(0x3edc1349), f32::from_bits(0xc26ffe6c)));
    path.cubic_to(
        (f32::from_bits(0x3ed567f5), f32::from_bits(0xc26ffe84)),
        (f32::from_bits(0x3ecebccf), f32::from_bits(0xc26ffe9c)),
        (f32::from_bits(0x3ec811a8), f32::from_bits(0xc26ffeb2)),
    );
    path.line_to((f32::from_bits(0x3ec8119b), f32::from_bits(0xc26ffeb2)));
    path.cubic_to(
        (f32::from_bits(0x3e856151), f32::from_bits(0xc26fff91)),
        (f32::from_bits(0x3e0561b2), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x3629eed0), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3f183800), f32::from_bits(0xc2a5fee9)));
    path.cubic_to(
        (f32::from_bits(0x3f62f7a2), f32::from_bits(0xc2a5fdd7)),
        (f32::from_bits(0x3f96db12), f32::from_bits(0xc2a5fbfa)),
        (f32::from_bits(0x3fbc3981), f32::from_bits(0xc2a5f954)),
    );
    path.line_to((f32::from_bits(0x3f8810cc), f32::from_bits(0xc26ff65b)));
    path.cubic_to(
        (f32::from_bits(0x3f5a1a86), f32::from_bits(0xc26ffa2f)),
        (f32::from_bits(0x3f241256), f32::from_bits(0xc26ffcdf)),
        (f32::from_bits(0x3edc1312), f32::from_bits(0xc26ffe6c)),
    );
    path.line_to((f32::from_bits(0x3f183800), f32::from_bits(0xc2a5fee9)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10560-L10582 (chrome/m156)
fn battleOp5193(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e0b17ea), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3e8b17df), f32::from_bits(0xc2a5ffd4)),
        (f32::from_bits(0x3ed0a399), f32::from_bits(0xc2a5ff7c)),
    );
    path.line_to((f32::from_bits(0x3e96d285), f32::from_bits(0xc26fff42)));
    path.cubic_to(
        (f32::from_bits(0x3e491945), f32::from_bits(0xc26fffc2)),
        (f32::from_bits(0x3dc91958), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x340ae940), f32::from_bits(0xc2700000)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ed0a338), f32::from_bits(0xc2a5ff7d)));
    path.cubic_to(
        (f32::from_bits(0x3ed797a0), f32::from_bits(0xc2a5ff73)),
        (f32::from_bits(0x3ede8c36), f32::from_bits(0xc2a5ff6a)),
        (f32::from_bits(0x3ee580cb), f32::from_bits(0xc2a5ff60)),
    );
    path.line_to((f32::from_bits(0x3ea5e78a), f32::from_bits(0xc26fff1b)));
    path.cubic_to(
        (f32::from_bits(0x3ea0e0aa), f32::from_bits(0xc26fff29)),
        (f32::from_bits(0x3e9bd97e), f32::from_bits(0xc26fff36)),
        (f32::from_bits(0x3e96d252), f32::from_bits(0xc26fff43)),
    );
    path.line_to((f32::from_bits(0x3ed0a338), f32::from_bits(0xc2a5ff7d)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10585-L10611 (chrome/m156)
fn battleOp5194(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e0b17a8), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3e8b179e), f32::from_bits(0xc2a5ffd4)),
        (f32::from_bits(0x3ed0a337), f32::from_bits(0xc2a5ff7c)),
    );
    path.line_to((f32::from_bits(0x3ed0a338), f32::from_bits(0xc2a5ff7d)));
    path.cubic_to(
        (f32::from_bits(0x3ed797a0), f32::from_bits(0xc2a5ff73)),
        (f32::from_bits(0x3ede8c36), f32::from_bits(0xc2a5ff6a)),
        (f32::from_bits(0x3ee580cb), f32::from_bits(0xc2a5ff60)),
    );
    path.line_to((f32::from_bits(0x3ea5e78a), f32::from_bits(0xc26fff1b)));
    path.cubic_to(
        (f32::from_bits(0x3ea0e0bb), f32::from_bits(0xc26fff29)),
        (f32::from_bits(0x3e9bd9a1), f32::from_bits(0xc26fff36)),
        (f32::from_bits(0x3e96d286), f32::from_bits(0xc26fff43)),
    );
    path.line_to((f32::from_bits(0x3e96d285), f32::from_bits(0xc26fff42)));
    path.cubic_to(
        (f32::from_bits(0x3e491945), f32::from_bits(0xc26fffc2)),
        (f32::from_bits(0x3dc91958), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ee58048), f32::from_bits(0xc2a5ff61)));
    path.cubic_to(
        (f32::from_bits(0x3f2b1987), f32::from_bits(0xc2a5fec4)),
        (f32::from_bits(0x3f637253), f32::from_bits(0xc2a5fdb6)),
        (f32::from_bits(0x3f8de535), f32::from_bits(0xc2a5fc35)),
    );
    path.line_to((f32::from_bits(0x3f4d269a), f32::from_bits(0xc26ffa85)));
    path.cubic_to(
        (f32::from_bits(0x3f246b51), f32::from_bits(0xc26ffcb3)),
        (f32::from_bits(0x3ef75f30), f32::from_bits(0xc26ffe3a)),
        (f32::from_bits(0x3ea5e737), f32::from_bits(0xc26fff1c)),
    );
    path.line_to((f32::from_bits(0x3ee58048), f32::from_bits(0xc2a5ff61)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10613-L10639 (chrome/m156)
fn battleOp402(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc2700000)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3e0b17a8), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3e8b179e), f32::from_bits(0xc2a5ffd4)),
        (f32::from_bits(0x3ed0a337), f32::from_bits(0xc2a5ff7c)),
    );
    path.line_to((f32::from_bits(0x3ed0a338), f32::from_bits(0xc2a5ff7d)));
    path.cubic_to(
        (f32::from_bits(0x3ed797a0), f32::from_bits(0xc2a5ff73)),
        (f32::from_bits(0x3ede8c36), f32::from_bits(0xc2a5ff6a)),
        (f32::from_bits(0x3ee580cb), f32::from_bits(0xc2a5ff60)),
    );
    path.line_to((f32::from_bits(0x3ea5e78a), f32::from_bits(0xc26fff1b)));
    path.cubic_to(
        (f32::from_bits(0x3ea0e0bb), f32::from_bits(0xc26fff29)),
        (f32::from_bits(0x3e9bd9a1), f32::from_bits(0xc26fff36)),
        (f32::from_bits(0x3e96d286), f32::from_bits(0xc26fff43)),
    );
    path.line_to((f32::from_bits(0x3e96d285), f32::from_bits(0xc26fff42)));
    path.cubic_to(
        (f32::from_bits(0x3e491945), f32::from_bits(0xc26fffc2)),
        (f32::from_bits(0x3dc91958), f32::from_bits(0xc2700000)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc2700000)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3ee58048), f32::from_bits(0xc2a5ff61)));
    path.cubic_to(
        (f32::from_bits(0x3f2b1987), f32::from_bits(0xc2a5fec4)),
        (f32::from_bits(0x3f637253), f32::from_bits(0xc2a5fdb6)),
        (f32::from_bits(0x3f8de535), f32::from_bits(0xc2a5fc35)),
    );
    path.line_to((f32::from_bits(0x3f4d269a), f32::from_bits(0xc26ffa85)));
    path.cubic_to(
        (f32::from_bits(0x3f246b51), f32::from_bits(0xc26ffcb3)),
        (f32::from_bits(0x3ef75f30), f32::from_bits(0xc26ffe3a)),
        (f32::from_bits(0x3ea5e737), f32::from_bits(0xc26fff1c)),
    );
    path.line_to((f32::from_bits(0x3ee58048), f32::from_bits(0xc2a5ff61)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10641-L10663 (chrome/m156)
fn battleOp6000(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3c9b2383), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3d1b200b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3d68ae54), f32::from_bits(0xc2a5fffd)),
    );
    path.line_to((f32::from_bits(0x3d283599), f32::from_bits(0xc26ffffc)));
    path.cubic_to(
        (f32::from_bits(0x3ce049ca), f32::from_bits(0xc26ffffe)),
        (f32::from_bits(0x3c604794), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0xb58d9000), f32::from_bits(0xc26fffff)),
    );
    path.line_to((f32::from_bits(0x27b71bcd), f32::from_bits(0xc2a60000)));
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3d68b08b), f32::from_bits(0xc2a5fffd)));
    path.cubic_to(
        (f32::from_bits(0x3d707589), f32::from_bits(0xc2a5fffd)),
        (f32::from_bits(0x3d783329), f32::from_bits(0xc2a5fffd)),
        (f32::from_bits(0x3d7ff0c9), f32::from_bits(0xc2a5fffd)),
    );
    path.line_to((f32::from_bits(0x3d3907c2), f32::from_bits(0xc26ffffc)));
    path.cubic_to(
        (f32::from_bits(0x3d336bee), f32::from_bits(0xc26ffffd)),
        (f32::from_bits(0x3d2dd36e), f32::from_bits(0xc26ffffd)),
        (f32::from_bits(0x3d283aee), f32::from_bits(0xc26ffffd)),
    );
    path.line_to((f32::from_bits(0x3d68b08b), f32::from_bits(0xc2a5fffd)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L10665-L10690 (chrome/m156)
fn battleOp6001(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0xc26fffff)));
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0xc2a60000)));
    path.cubic_to(
        (f32::from_bits(0x3c9b2383), f32::from_bits(0xc2a60000)),
        (f32::from_bits(0x3d1b200b), f32::from_bits(0xc2a5ffff)),
        (f32::from_bits(0x3d68ae54), f32::from_bits(0xc2a5fffd)),
    );
    path.line_to((f32::from_bits(0x3d7ff0c9), f32::from_bits(0xc2a5fffd)));
    path.line_to((f32::from_bits(0x3d3907c2), f32::from_bits(0xc26ffffc)));
    path.cubic_to(
        (f32::from_bits(0x3d336bee), f32::from_bits(0xc26ffffd)),
        (f32::from_bits(0x3d2dd36e), f32::from_bits(0xc26ffffd)),
        (f32::from_bits(0x3d283aee), f32::from_bits(0xc26ffffd)),
    );
    path.line_to((f32::from_bits(0x3d283599), f32::from_bits(0xc26ffffc)));
    path.cubic_to(
        (f32::from_bits(0x3ce049ca), f32::from_bits(0xc26ffffe)),
        (f32::from_bits(0x3c604794), f32::from_bits(0xc26fffff)),
        (f32::from_bits(0x00000000), f32::from_bits(0xc26fffff)),
    );
    path.close();
    let path1 = path.detach();
    path.reset();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x3d7ff566), f32::from_bits(0xc2a5fffd)));
    path.cubic_to(
        (f32::from_bits(0x3dbed1a5), f32::from_bits(0xc2a5fffa)),
        (f32::from_bits(0x3dfda9cc), f32::from_bits(0xc2a5fff4)),
        (f32::from_bits(0x3e1e40f8), f32::from_bits(0xc2a5ffed)),
    );
    path.line_to((f32::from_bits(0x3de4ce81), f32::from_bits(0xc26fffe5)));
    path.cubic_to(
        (f32::from_bits(0x3db75eff), f32::from_bits(0xc26ffff0)),
        (f32::from_bits(0x3d89f101), f32::from_bits(0xc26ffff8)),
        (f32::from_bits(0x3d390604), f32::from_bits(0xc26ffffc)),
    );
    path.line_to((f32::from_bits(0x3d7ff566), f32::from_bits(0xc2a5fffd)));
    path.close();
    let path2 = path.detach();
    test_path_op(reporter, &path1, &path2, PathOp::Union, filename);
}

// Port of: tests/PathOpsBattles.cpp#L11130-L11132 (chrome/m156)
def_test!(PathOpsBattle, |reporter| {
    battleOp183(reporter, "battleOp183");
    battleOp1(reporter, "battleOp1");
    battleOp2(reporter, "battleOp2");
    battleOp3(reporter, "battleOp3");
    battleOp4(reporter, "battleOp4");
    battleOp5(reporter, "battleOp5");
    battleOp6(reporter, "battleOp6");
    battleOp7(reporter, "battleOp7");
    battleOp8(reporter, "battleOp8");
    battleOp9(reporter, "battleOp9");
    battleOp10(reporter, "battleOp10");
    battleOp11(reporter, "battleOp11");
    battleOp12(reporter, "battleOp12");
    battleOp13(reporter, "battleOp13");
    battleOp14(reporter, "battleOp14");
    battleOp15(reporter, "battleOp15");
    battleOp16(reporter, "battleOp16");
    battleOp17(reporter, "battleOp17");
    battleOp18(reporter, "battleOp18");
    battleOp19(reporter, "battleOp19");
    battleOp20(reporter, "battleOp20");
    battleOp21(reporter, "battleOp21");
    battleOp22(reporter, "battleOp22");
    battleOp23(reporter, "battleOp23");
    battleOp24(reporter, "battleOp24");
    battleOp25(reporter, "battleOp25");
    battleOp26(reporter, "battleOp26");
    battleOp27(reporter, "battleOp27");
    battleOp28(reporter, "battleOp28");
    battleOp29(reporter, "battleOp29");
    battleOp30(reporter, "battleOp30");
    battleOp31(reporter, "battleOp31");
    battleOp32(reporter, "battleOp32");
    battleOp33(reporter, "battleOp33");
    battleOp34(reporter, "battleOp34");
    battleOp35(reporter, "battleOp35");
    battleOp36(reporter, "battleOp36");
    battleOp37(reporter, "battleOp37");
    battleOp38(reporter, "battleOp38");
    battleOp39(reporter, "battleOp39");
    battleOp40(reporter, "battleOp40");
    battleOp41(reporter, "battleOp41");
    battleOp42(reporter, "battleOp42");
    battleOp43(reporter, "battleOp43");
    battleOp44(reporter, "battleOp44");
    battleOp45(reporter, "battleOp45");
    battleOp47(reporter, "battleOp47");
    battleOp48(reporter, "battleOp48");
    battleOp49(reporter, "battleOp49");
    battleOp50(reporter, "battleOp50");
    battleOp51(reporter, "battleOp51");
    battleOp52(reporter, "battleOp52");
    battleOp53(reporter, "battleOp53");
    battleOp55(reporter, "battleOp55");
    battleOp56(reporter, "battleOp56");
    battleOp57(reporter, "battleOp57");
    battleOp58(reporter, "battleOp58");
    battleOp59(reporter, "battleOp59");
    battleOp60(reporter, "battleOp60");
    battleOp61(reporter, "battleOp61");
    battleOp62(reporter, "battleOp62");
    battleOp64(reporter, "battleOp64");
    battleOp65(reporter, "battleOp65");
    battleOp66(reporter, "battleOp66");
    battleOp67(reporter, "battleOp67");
    battleOp68(reporter, "battleOp68");
    battleOp69(reporter, "battleOp69");
    battleOp70(reporter, "battleOp70");
    battleOp71(reporter, "battleOp71");
    battleOp72(reporter, "battleOp72");
    battleOp73(reporter, "battleOp73");
    battleOp74(reporter, "battleOp74");
    battleOp75(reporter, "battleOp75");
    battleOp76(reporter, "battleOp76");
    battleOp77(reporter, "battleOp77");
    battleOp78(reporter, "battleOp78");
    battleOp79(reporter, "battleOp79");
    battleOp80(reporter, "battleOp80");
    battleOp81(reporter, "battleOp81");
    battleOp82(reporter, "battleOp82");
    battleOp83(reporter, "battleOp83");
    battleOp84(reporter, "battleOp84");
    battleOp85(reporter, "battleOp85");
    battleOp86(reporter, "battleOp86");
    battleOp87(reporter, "battleOp87");
    battleOp88(reporter, "battleOp88");
    battleOp89(reporter, "battleOp89");
    battleOp90(reporter, "battleOp90");
    battleOp91(reporter, "battleOp91");
    battleOp92(reporter, "battleOp92");
    battleOp93(reporter, "battleOp93");
    battleOp94(reporter, "battleOp94");
    battleOp95(reporter, "battleOp95");
    battleOp96(reporter, "battleOp96");
    battleOp97(reporter, "battleOp97");
    battleOp98(reporter, "battleOp98");
    battleOp99(reporter, "battleOp99");
    battleOp100(reporter, "battleOp100");
    battleOp101(reporter, "battleOp101");
    battleOp102(reporter, "battleOp102");
    battleOp103(reporter, "battleOp103");
    battleOp104(reporter, "battleOp104");
    battleOp105(reporter, "battleOp105");
    battleOp106(reporter, "battleOp106");
    battleOp107(reporter, "battleOp107");
    battleOp108(reporter, "battleOp108");
    battleOp109(reporter, "battleOp109");
    battleOp110(reporter, "battleOp110");
    battleOp111(reporter, "battleOp111");
    battleOp112(reporter, "battleOp112");
    battleOp113(reporter, "battleOp113");
    battleOp114(reporter, "battleOp114");
    battleOp115(reporter, "battleOp115");
    battleOp116(reporter, "battleOp116");
    battleOp117(reporter, "battleOp117");
    battleOp118(reporter, "battleOp118");
    battleOp119(reporter, "battleOp119");
    battleOp120(reporter, "battleOp120");
    battleOp121(reporter, "battleOp121");
    battleOp122(reporter, "battleOp122");
    battleOp123(reporter, "battleOp123");
    battleOp124(reporter, "battleOp124");
    battleOp125(reporter, "battleOp125");
    battleOp126(reporter, "battleOp126");
    battleOp127(reporter, "battleOp127");
    battleOp128(reporter, "battleOp128");
    battleOp129(reporter, "battleOp129");
    battleOp130(reporter, "battleOp130");
    battleOp131(reporter, "battleOp131");
    battleOp132(reporter, "battleOp132");
    battleOp133(reporter, "battleOp133");
    battleOp134(reporter, "battleOp134");
    battleOp135(reporter, "battleOp135");
    battleOp136(reporter, "battleOp136");
    battleOp137(reporter, "battleOp137");
    battleOp138(reporter, "battleOp138");
    battleOp139(reporter, "battleOp139");
    battleOp140(reporter, "battleOp140");
    battleOp141(reporter, "battleOp141");
    battleOp142(reporter, "battleOp142");
    battleOp143(reporter, "battleOp143");
    battleOp144(reporter, "battleOp144");
    battleOp145(reporter, "battleOp145");
    battleOp146(reporter, "battleOp146");
    battleOp147(reporter, "battleOp147");
    battleOp149(reporter, "battleOp149");
    battleOp150(reporter, "battleOp150");
    battleOp151(reporter, "battleOp151");
    battleOp153(reporter, "battleOp153");
    battleOp154(reporter, "battleOp154");
    battleOp155(reporter, "battleOp155");
    battleOp156(reporter, "battleOp156");
    battleOp158(reporter, "battleOp158");
    battleOp159(reporter, "battleOp159");
    battleOp160(reporter, "battleOp160");
    battleOp161(reporter, "battleOp161");
    battleOp162(reporter, "battleOp162");
    battleOp164(reporter, "battleOp164");
    battleOp165(reporter, "battleOp165");
    battleOp166(reporter, "battleOp166");
    battleOp167(reporter, "battleOp167");
    battleOp168(reporter, "battleOp168");
    battleOp169(reporter, "battleOp169");
    battleOp170(reporter, "battleOp170");
    battleOp171(reporter, "battleOp171");
    battleOp172(reporter, "battleOp172");
    battleOp173(reporter, "battleOp173");
    battleOp174(reporter, "battleOp174");
    battleOp175(reporter, "battleOp175");
    battleOp176(reporter, "battleOp176");
    battleOp177(reporter, "battleOp177");
    battleOp178(reporter, "battleOp178");
    battleOp179(reporter, "battleOp179");
    battleOp180(reporter, "battleOp180");
    battleOp182(reporter, "battleOp182");
    battleOp184(reporter, "battleOp184");
    battleOp185(reporter, "battleOp185");
    battleOp186(reporter, "battleOp186");
    battleOp187(reporter, "battleOp187");
    battleOp188(reporter, "battleOp188");
    battleOp189(reporter, "battleOp189");
    battleOp190(reporter, "battleOp190");
    battleOp191(reporter, "battleOp191");
    battleOp192(reporter, "battleOp192");
    battleOp193(reporter, "battleOp193");
    battleOp194(reporter, "battleOp194");
    battleOp196(reporter, "battleOp196");
    battleOp197(reporter, "battleOp197");
    battleOp199(reporter, "battleOp199");
    battleOp200(reporter, "battleOp200");
    battleOp201(reporter, "battleOp201");
    battleOp202(reporter, "battleOp202");
    battleOp203(reporter, "battleOp203");
    battleOp204(reporter, "battleOp204");
    battleOp205(reporter, "battleOp205");
    battleOp206(reporter, "battleOp206");
    battleOp207(reporter, "battleOp207");
    battleOp208(reporter, "battleOp208");
    battleOp209(reporter, "battleOp209");
    battleOp210(reporter, "battleOp210");
    battleOp211(reporter, "battleOp211");
    battleOp212(reporter, "battleOp212");
    battleOp213(reporter, "battleOp213");
    battleOp214(reporter, "battleOp214");
    battleOp215(reporter, "battleOp215");
    battleOp216(reporter, "battleOp216");
    battleOp217(reporter, "battleOp217");
    battleOp218(reporter, "battleOp218");
    battleOp219(reporter, "battleOp219");
    battleOp220(reporter, "battleOp220");
    battleOp221(reporter, "battleOp221");
    battleOp222(reporter, "battleOp222");
    battleOp223(reporter, "battleOp223");
    battleOp224(reporter, "battleOp224");
    battleOp225(reporter, "battleOp225");
    battleOp226(reporter, "battleOp226");
    battleOp227(reporter, "battleOp227");
    battleOp228(reporter, "battleOp228");
    battleOp229(reporter, "battleOp229");
    battleOp231(reporter, "battleOp231");
    battleOp232(reporter, "battleOp232");
    battleOp233(reporter, "battleOp233");
    battleOp234(reporter, "battleOp234");
    battleOp235(reporter, "battleOp235");
    battleOp236(reporter, "battleOp236");
    battleOp237(reporter, "battleOp237");
    battleOp238(reporter, "battleOp238");
    battleOp239(reporter, "battleOp239");
    battleOp240(reporter, "battleOp240");
    battleOp241(reporter, "battleOp241");
    battleOp242(reporter, "battleOp242");
    battleOp243(reporter, "battleOp243");
    battleOp244(reporter, "battleOp244");
    battleOp245(reporter, "battleOp245");
    battleOp246(reporter, "battleOp246");
    battleOp247(reporter, "battleOp247");
    battleOp248(reporter, "battleOp248");
    battleOp249(reporter, "battleOp249");
    battleOp250(reporter, "battleOp250");
    battleOp251(reporter, "battleOp251");
    battleOp252(reporter, "battleOp252");
    battleOp253(reporter, "battleOp253");
    battleOp254(reporter, "battleOp254");
    battleOp255(reporter, "battleOp255");
    battleOp257(reporter, "battleOp257");
    battleOp258(reporter, "battleOp258");
    battleOp259(reporter, "battleOp259");
    battleOp260(reporter, "battleOp260");
    battleOp261(reporter, "battleOp261");
    battleOp262(reporter, "battleOp262");
    battleOp263(reporter, "battleOp263");
    battleOp264(reporter, "battleOp264");
    battleOp265(reporter, "battleOp265");
    battleOp266(reporter, "battleOp266");
    battleOp267(reporter, "battleOp267");
    battleOp268(reporter, "battleOp268");
    battleOp270(reporter, "battleOp270");
    battleOp271(reporter, "battleOp271");
    battleOp272(reporter, "battleOp272");
    battleOp274(reporter, "battleOp274");
    battleOp275(reporter, "battleOp275");
    battleOp276(reporter, "battleOp276");
    battleOp277(reporter, "battleOp277");
    battleOp278(reporter, "battleOp278");
    battleOp279(reporter, "battleOp279");
    battleOp280(reporter, "battleOp280");
    battleOp281(reporter, "battleOp281");
    battleOp282(reporter, "battleOp282");
    battleOp284(reporter, "battleOp284");
    battleOp285(reporter, "battleOp285");
    battleOp286(reporter, "battleOp286");
    battleOp287(reporter, "battleOp287");
    battleOp288(reporter, "battleOp288");
    battleOp289(reporter, "battleOp289");
    battleOp290(reporter, "battleOp290");
    battleOp291(reporter, "battleOp291");
    battleOp292(reporter, "battleOp292");
    battleOp293(reporter, "battleOp293");
    battleOp294(reporter, "battleOp294");
    battleOp295(reporter, "battleOp295");
    battleOp296(reporter, "battleOp296");
    battleOp297(reporter, "battleOp297");
    battleOp298(reporter, "battleOp298");
    battleOp299(reporter, "battleOp299");
    battleOp300(reporter, "battleOp300");
    battleOp301(reporter, "battleOp301");
    battleOp302(reporter, "battleOp302");
    battleOp303(reporter, "battleOp303");
    battleOp304(reporter, "battleOp304");
    battleOp305(reporter, "battleOp305");
    battleOp306(reporter, "battleOp306");
    battleOp307(reporter, "battleOp307");
    battleOp308(reporter, "battleOp308");
    battleOp309(reporter, "battleOp309");
    battleOp310(reporter, "battleOp310");
    battleOp311(reporter, "battleOp311");
    battleOp312(reporter, "battleOp312");
    battleOp313(reporter, "battleOp313");
    battleOp314(reporter, "battleOp314");
    battleOp315(reporter, "battleOp315");
    battleOp316(reporter, "battleOp316");
    battleOp317(reporter, "battleOp317");
    battleOp318(reporter, "battleOp318");
    battleOp319(reporter, "battleOp319");
    battleOp320(reporter, "battleOp320");
    battleOp321(reporter, "battleOp321");
    battleOp322(reporter, "battleOp322");
    battleOp323(reporter, "battleOp323");
    battleOp324(reporter, "battleOp324");
    battleOp325(reporter, "battleOp325");
    battleOp326(reporter, "battleOp326");
    battleOp327(reporter, "battleOp327");
    battleOp328(reporter, "battleOp328");
    battleOp329(reporter, "battleOp329");
    battleOp330(reporter, "battleOp330");
    battleOp331(reporter, "battleOp331");
    battleOp332(reporter, "battleOp332");
    battleOp333(reporter, "battleOp333");
    battleOp334(reporter, "battleOp334");
    battleOp335(reporter, "battleOp335");
    battleOp336(reporter, "battleOp336");
    battleOp337(reporter, "battleOp337");
    battleOp338(reporter, "battleOp338");
    battleOp339(reporter, "battleOp339");
    battleOp340(reporter, "battleOp340");
    battleOp341(reporter, "battleOp341");
    battleOp342(reporter, "battleOp342");
    battleOp343(reporter, "battleOp343");
    battleOp344(reporter, "battleOp344");
    battleOp345(reporter, "battleOp345");
    battleOp346(reporter, "battleOp346");
    battleOp347(reporter, "battleOp347");
    battleOp348(reporter, "battleOp348");
    battleOp349(reporter, "battleOp349");
    battleOp350(reporter, "battleOp350");
    battleOp351(reporter, "battleOp351");
    battleOp352(reporter, "battleOp352");
    battleOp402(reporter, "battleOp402");
    battleOp1390(reporter, "battleOp1390");
    battleOp1391(reporter, "battleOp1391");
    battleOp1392(reporter, "battleOp1392");
    battleOp1393(reporter, "battleOp1393");
    battleOp1394(reporter, "battleOp1394");
    battleOp1395(reporter, "battleOp1395");
    battleOp1396(reporter, "battleOp1396");
    battleOp2193(reporter, "battleOp2193");
    battleOp2194(reporter, "battleOp2194");
    battleOp3368(reporter, "battleOp3368");
    battleOp3369(reporter, "battleOp3369");
    battleOp3370(reporter, "battleOp3370");
    battleOp3371(reporter, "battleOp3371");
    battleOp3372(reporter, "battleOp3372");
    battleOp4290(reporter, "battleOp4290");
    battleOp4291(reporter, "battleOp4291");
    battleOp4292(reporter, "battleOp4292");
    battleOp4293(reporter, "battleOp4293");
    battleOp4294(reporter, "battleOp4294");
    battleOp4295(reporter, "battleOp4295");
    battleOp4296(reporter, "battleOp4296");
    battleOp5193(reporter, "battleOp5193");
    battleOp5194(reporter, "battleOp5194");
    battleOp6000(reporter, "battleOp6000");
    battleOp6001(reporter, "battleOp6001");
    issue414409c(reporter, "issue414409c");
    issue414409b(reporter, "issue414409b");
    issue414409(reporter, "issue414409");
    battleOp46(reporter, "battleOp46");
    battleOp54(reporter, "battleOp54");
    battleOp63(reporter, "battleOp63");
    battleOp152(reporter, "battleOp152");
    battleOp157(reporter, "battleOp157");
    battleOp163(reporter, "battleOp163");
    battleOp181(reporter, "battleOp181");
    battleOp195(reporter, "battleOp195");
    battleOp198(reporter, "battleOp198");
    battleOp230(reporter, "battleOp230");
    battleOp256(reporter, "battleOp256");
    battleOp269(reporter, "battleOp269");
    battleOp273(reporter, "battleOp273");
    battleOp148(reporter, "battleOp148");
    battleOp283(reporter, "battleOp283");
});
