// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsSimplifyFailTest.cpp (chrome/m156)

// The float literals below are Skia's fuzz inputs, copied as written (SkBits2Float bit patterns),
// so the constant spellings are kept to match the C++ source.
#![allow(
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::approx_constant,
    clippy::excessive_precision
)]
#![cfg(test)]

use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_pathops::simplify;

use crate::unit::path_ops_extended_test::{test_simplify, test_simplify_fuzz};
use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/PathOpsSimplifyFailTest.cpp#L18-L26 (chrome/m156)
/// `finitePts`: points whose coordinates are finite (`SK_ScalarMax`, `SK_ScalarMin`).
const FINITE_PTS: [(f32, f32); 7] = [
    (0., 0.),
    (f32::MAX, 0.),
    (0., f32::MAX),
    (f32::MAX, f32::MAX),
    (f32::MIN, 0.),
    (0., f32::MIN),
    (f32::MIN, f32::MIN),
];

// Port of: tests/PathOpsSimplifyFailTest.cpp#L28-L51 (chrome/m156)
fn dont_fail_one(reporter: &mut Reporter, index: usize) {
    let mut path = PathBuilder::new();
    let f = index % FINITE_PTS.len();
    let g = (f + 1) % FINITE_PTS.len();
    let (pf, pg) = (FINITE_PTS[f], FINITE_PTS[g]);
    match index % 11 {
        0 => {
            path.line_to(pf);
        }
        1 => {
            path.quad_to(pf, pf);
        }
        2 => {
            path.quad_to(pf, pg);
        }
        3 => {
            path.quad_to(pg, pf);
        }
        4 => {
            path.cubic_to(pf, pf, pf);
        }
        5 => {
            path.cubic_to(pf, pf, pg);
        }
        6 => {
            path.cubic_to(pf, pg, pf);
        }
        7 => {
            path.cubic_to(pf, pg, pg);
        }
        8 => {
            path.cubic_to(pg, pf, pf);
        }
        9 => {
            path.cubic_to(pg, pf, pg);
        }
        _ => {
            path.move_to(pf);
        }
    }
    let result = simplify(&path.detach());
    reporter_assert!(reporter, result.is_some());
    reporter_assert!(
        reporter,
        result.is_some_and(|r| r.fill_type() != PathFillType::Winding)
    );
    reporter.bump_test_count();
}

// The fuzz cases below are converted mechanically from the C++ statements (SkPathBuilder calls
// to PathBuilder calls, SkBits2Float to f32::from_bits).
fn fuzz_59(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x430c0000), f32::from_bits(0xce58f41c))); // 140, -9.09969e+08f
    path.line_to((f32::from_bits(0x43480000), f32::from_bits(0xce58f419))); // 200, -9.09969e+08f
    path.line_to((f32::from_bits(0x42200000), f32::from_bits(0xce58f41b))); // 40, -9.09969e+08f
    path.line_to((f32::from_bits(0x43700000), f32::from_bits(0xce58f41b))); // 240, -9.09969e+08f
    path.line_to((f32::from_bits(0x428c0000), f32::from_bits(0xce58f419))); // 70, -9.09969e+08f
    path.line_to((f32::from_bits(0x430c0000), f32::from_bits(0xce58f41c))); // 140, -9.09969e+08f
    path.close();
    test_simplify_fuzz(reporter, &path.detach(), filename);
}

fn fuzz_x1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000))); // 0, 0
    path.cubic_to(
        (f32::from_bits(0x1931204a), f32::from_bits(0x2ba1a14a)),
        (f32::from_bits(0x4a4a08ff), f32::from_bits(0x4a4a08ff)),
        (f32::from_bits(0x4a4a4a34), f32::from_bits(0x4a4a4a4a)),
    ); // 9.15721e-24f, 1.14845e-12f, 3.31014e+06f, 3.31014e+06f, 3.31432e+06f, 3.31432e+06f
    path.move_to((f32::from_bits(0x000010a1), f32::from_bits(0x19312000))); // 5.96533e-42f, 9.15715e-24f
    path.cubic_to(
        (f32::from_bits(0x4a6a4a4a), f32::from_bits(0x4a4a4a4a)),
        (f32::from_bits(0xa14a4a4a), f32::from_bits(0x08ff2ba1)),
        (f32::from_bits(0x08ff4a4a), f32::from_bits(0x4a344a4a)),
    ); // 3.83861e+06f, 3.31432e+06f, -6.85386e-19f, 1.53575e-33f, 1.53647e-33f, 2.95387e+06f
    path.cubic_to(
        (f32::from_bits(0x4a4a4a4a), f32::from_bits(0x4a4a4a4a)),
        (f32::from_bits(0x2ba1a14a), f32::from_bits(0x4e4a08ff)),
        (f32::from_bits(0x4a4a4a4a), f32::from_bits(0xa1a181ff)),
    ); // 3.31432e+06f, 3.31432e+06f, 1.14845e-12f, 8.47397e+08f, 3.31432e+06f, -1.09442e-18f
    test_simplify(reporter, &path.detach(), filename);
}

fn fuzz_x2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000))); // 0, 0
    path.cubic_to(
        (f32::from_bits(0x1931204a), f32::from_bits(0x2ba1a14a)),
        (f32::from_bits(0x4a4a08ff), f32::from_bits(0x4a4a08ff)),
        (f32::from_bits(0x4a4a4a34), f32::from_bits(0x4a4a4a4a)),
    ); // 9.15721e-24f, 1.14845e-12f, 3.31014e+06f, 3.31014e+06f, 3.31432e+06f, 3.31432e+06f
    path.move_to((f32::from_bits(0x000010a1), f32::from_bits(0x19312000))); // 5.96533e-42f, 9.15715e-24f
    path.cubic_to(
        (f32::from_bits(0x4a6a4a4a), f32::from_bits(0x4a4a4a4a)),
        (f32::from_bits(0xa14a4a4a), f32::from_bits(0x08ff2ba1)),
        (f32::from_bits(0x08ff4a4a), f32::from_bits(0x4a344a4a)),
    ); // 3.83861e+06f, 3.31432e+06f, -6.85386e-19f, 1.53575e-33f, 1.53647e-33f, 2.95387e+06f
    path.cubic_to(
        (f32::from_bits(0x4a4a4a4a), f32::from_bits(0x4a4a4a4a)),
        (f32::from_bits(0x2ba1a14a), f32::from_bits(0x4e4a08ff)),
        (f32::from_bits(0x4a4a4a4a), f32::from_bits(0xa1a181ff)),
    ); // 3.31432e+06f, 3.31432e+06f, 1.14845e-12f, 8.47397e+08f, 3.31432e+06f, -1.09442e-18f
    test_simplify(reporter, &path.detach(), filename);
}

fn fuzz763_1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000))); // 0, 0
    path.cubic_to(
        (f32::from_bits(0xbcb63000), f32::from_bits(0xb6b6b6b7)),
        (f32::from_bits(0x38b6b6b6), f32::from_bits(0xafb63a5a)),
        (f32::from_bits(0xca000087), f32::from_bits(0xe93ae9e9)),
    ); // -0.0222397f, -5.44529e-06f, 8.71247e-05f, -3.31471e-10f, -2.09719e+06f, -1.41228e+25f
    path.quad_to(
        (f32::from_bits(0xb6007fb6), f32::from_bits(0xb69fb6b6)),
        (f32::from_bits(0xe9e964b6), f32::from_bits(0xe9e9e9e9)),
    ); // -1.91478e-06f, -4.75984e-06f, -3.52694e+25f, -3.5348e+25f
    path.quad_to(
        (f32::from_bits(0xb6b6b8b7), f32::from_bits(0xb60000b6)),
        (f32::from_bits(0xb6b6b6b6), f32::from_bits(0xe9e92064)),
    ); // -5.44553e-06f, -1.90739e-06f, -5.44529e-06f, -3.52291e+25f
    path.quad_to(
        (f32::from_bits(0x000200e9), f32::from_bits(0xe9e9d100)),
        (f32::from_bits(0xe93ae9e9), f32::from_bits(0xe964b6e9)),
    ); // 1.83997e-40f, -3.53333e+25f, -1.41228e+25f, -1.72812e+25f
    path.quad_to(
        (f32::from_bits(0x40b6e9e9), f32::from_bits(0xe9b60000)),
        (f32::from_bits(0x00b6b8e9), f32::from_bits(0xe9000001)),
    ); // 5.71605f, -2.75031e+25f, 1.67804e-38f, -9.67141e+24f
    path.quad_to(
        (f32::from_bits(0xe9d3b6b2), f32::from_bits(0x40404540)),
        (f32::from_bits(0x803d4043), f32::from_bits(0xe9e9e9ff)),
    ); // -3.19933e+25f, 3.00423f, -5.62502e-39f, -3.53481e+25f
    path.cubic_to(
        (f32::from_bits(0x00000000), f32::from_bits(0xe8b3b6b6)),
        (f32::from_bits(0xe90a0003), f32::from_bits(0x4040403c)),
        (f32::from_bits(0x803d4040), f32::from_bits(0xe9e80900)),
    ); // 0, -6.78939e+24f, -1.0427e+25f, 3.00392f, -5.62501e-39f, -3.50642e+25f
    path.quad_to(
        (f32::from_bits(0xe9e910e9), f32::from_bits(0xe9e93ae9)),
        (f32::from_bits(0x0000b6b6), f32::from_bits(0xb6b6aab6)),
    ); // -3.52199e+25f, -3.52447e+25f, 6.55443e-41f, -5.4439e-06f
    path.move_to((f32::from_bits(0xe9e92064), f32::from_bits(0xe9e9d106))); // -3.52291e+25f, -3.53334e+25f
    path.quad_to(
        (f32::from_bits(0xe9e93ae9), f32::from_bits(0x0000abb6)),
        (f32::from_bits(0xb6b6bdb6), f32::from_bits(0xe92064b6)),
    ); // -3.52447e+25f, 6.15983e-41f, -5.44611e-06f, -1.2119e+25f
    path.quad_to(
        (f32::from_bits(0x0000e9e9), f32::from_bits(0xb6b6b6e9)),
        (f32::from_bits(0x05ffff05), f32::from_bits(0xe9ea06e9)),
    ); // 8.39112e-41f, -5.44532e-06f, 2.40738e-35f, -3.53652e+25f
    path.quad_to(
        (f32::from_bits(0xe93ae9e9), f32::from_bits(0x02007fe9)),
        (f32::from_bits(0xb8b7b600), f32::from_bits(0xe9e9b6b6)),
    ); // -1.41228e+25f, 9.44066e-38f, -8.76002e-05f, -3.53178e+25f
    path.quad_to(
        (f32::from_bits(0xe9e9e9b6), f32::from_bits(0xedb6b6b6)),
        (f32::from_bits(0x5a38a1b6), f32::from_bits(0xe93ae9e9)),
    ); // -3.53479e+25f, -7.06839e+27f, 1.29923e+16f, -1.41228e+25f
    path.quad_to(
        (f32::from_bits(0x0000b6b6), f32::from_bits(0xb6b6b6b6)),
        (f32::from_bits(0xe9e9e9b6), f32::from_bits(0xe9e9e954)),
    ); // 6.55443e-41f, -5.44529e-06f, -3.53479e+25f, -3.53477e+25f
    path.quad_to(
        (f32::from_bits(0xb6e9e93a), f32::from_bits(0x375837ff)),
        (f32::from_bits(0xceb6b6b6), f32::from_bits(0x0039e94f)),
    ); // -6.97109e-06f, 1.28876e-05f, -1.53271e+09f, 5.31832e-39f
    path.quad_to(
        (f32::from_bits(0xe9e9e9e9), f32::from_bits(0xe9e6e9e9)),
        (f32::from_bits(0xb6b641b6), f32::from_bits(0xede9e9e9)),
    ); // -3.5348e+25f, -3.48947e+25f, -5.43167e-06f, -9.0491e+27f
    path.move_to((f32::from_bits(0xb6b6e9e9), f32::from_bits(0xb6b60000))); // -5.45125e-06f, -5.42402e-06f
    path.move_to((f32::from_bits(0xe9b6b6b6), f32::from_bits(0xe9b6b8e9))); // -2.76109e+25f, -2.76122e+25f
    path.close();
    path.move_to((f32::from_bits(0xe9b6b6b6), f32::from_bits(0xe9b6b8e9))); // -2.76109e+25f, -2.76122e+25f
    path.quad_to(
        (f32::from_bits(0xe93ae9e9), f32::from_bits(0xe964b6e9)),
        (f32::from_bits(0x0000203a), f32::from_bits(0xb6000000)),
    ); // -1.41228e+25f, -1.72812e+25f, 1.15607e-41f, -1.90735e-06f
    path.move_to((f32::from_bits(0x64b6b6b6), f32::from_bits(0xe9e9e900))); // 2.69638e+22f, -3.53475e+25f
    path.quad_to(
        (f32::from_bits(0xb6b6b6e9), f32::from_bits(0xb6b6b6b6)),
        (f32::from_bits(0xe9e9b6ce), f32::from_bits(0xe9e93ae9)),
    ); // -5.44532e-06f, -5.44529e-06f, -3.53179e+25f, -3.52447e+25f

    test_simplify_fuzz(reporter, &path.detach(), filename);
}

fn fuzz763_2s(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000))); // 0, 0
    path.cubic_to(
        (f32::from_bits(0x76773011), f32::from_bits(0x5d66fe78)),
        (f32::from_bits(0xbbeeff66), f32::from_bits(0x637677a2)),
        (f32::from_bits(0x205266fe), f32::from_bits(0xec296fdf)),
    ); // 1.25339e+33f, 1.0403e+18f, -0.00729363f, 4.54652e+21f, 1.78218e-19f, -8.19347e+26f
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000))); // 0, 0
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000))); // 0, 0
    path.quad_to(
        (f32::from_bits(0xec4eecec), f32::from_bits(0x6e6f10ec)),
        (f32::from_bits(0xb6b6ecf7), f32::from_bits(0xb6b6b6b6)),
    ); // -1.00063e+27f, 1.84968e+28f, -5.45161e-06f, -5.44529e-06f
    path.move_to((f32::from_bits(0x002032b8), f32::from_bits(0xecfeb6b6))); // 2.95693e-39f, -2.46344e+27f
    path.move_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273))); // 1.9288e+31f, 1.9278e+31f
    path.cubic_to(
        (f32::from_bits(0x1616ece4), f32::from_bits(0xdf020018)),
        (f32::from_bits(0x77772965), f32::from_bits(0x1009db73)),
        (f32::from_bits(0x80ececec), f32::from_bits(0xf7ffffff)),
    ); // 1.21917e-25f, -9.36751e+18f, 5.01303e+33f, 2.71875e-29f, -2.17582e-38f, -1.03846e+34f
    path.line_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273))); // 1.9288e+31f, 1.9278e+31f
    path.close();
    path.move_to((f32::from_bits(0x73737300), f32::from_bits(0x73735273))); // 1.9288e+31f, 1.9278e+31f
    path.conic_to(
        (f32::from_bits(0xec0700ec), f32::from_bits(0xecececec)),
        (f32::from_bits(0xececccec), f32::from_bits(0x772965ec)),
        f32::from_bits(0x77777377),
    ); // -6.52837e+26f, -2.2914e+27f, -2.29019e+27f, 3.4358e+33f, 5.0189e+33f
    path.move_to((f32::from_bits(0xfe817477), f32::from_bits(0xdf665266))); // -8.60376e+37f, -1.65964e+19f
    path.close();
    path.move_to((f32::from_bits(0xfe817477), f32::from_bits(0xdf665266))); // -8.60376e+37f, -1.65964e+19f
    path.quad_to(
        (f32::from_bits(0x29ec02ec), f32::from_bits(0x1009ecec)),
        (f32::from_bits(0x80ececec), f32::from_bits(0xf7ffffff)),
    ); // 1.0481e-13f, 2.7201e-29f, -2.17582e-38f, -1.03846e+34f
    path.line_to((f32::from_bits(0xfe817477), f32::from_bits(0xdf665266))); // -8.60376e+37f, -1.65964e+19f
    path.close();
    path.move_to((f32::from_bits(0xfe817477), f32::from_bits(0xdf665266))); // -8.60376e+37f, -1.65964e+19f
    path.conic_to(
        (f32::from_bits(0xff003aff), f32::from_bits(0xdbec2300)),
        (f32::from_bits(0xecececec), f32::from_bits(0x6fdf6052)),
        f32::from_bits(0x41ecec29),
    ); // -1.70448e+38f, -1.32933e+17f, -2.2914e+27f, 1.38263e+29f, 29.6153f
    path.line_to((f32::from_bits(0xfe817477), f32::from_bits(0xdf665266))); // -8.60376e+37f, -1.65964e+19f
    path.close();
    path.move_to((f32::from_bits(0xfe817477), f32::from_bits(0xdf665266))); // -8.60376e+37f, -1.65964e+19f
    path.quad_to(
        (f32::from_bits(0xecf76e6f), f32::from_bits(0xeccfddec)),
        (f32::from_bits(0xecececcc), f32::from_bits(0x66000066)),
    ); // -2.39301e+27f, -2.01037e+27f, -2.2914e+27f, 1.51118e+23f
    path.line_to((f32::from_bits(0xfe817477), f32::from_bits(0xdf665266))); // -8.60376e+37f, -1.65964e+19f
    path.close();
    path.move_to((f32::from_bits(0xfe817477), f32::from_bits(0xdf665266))); // -8.60376e+37f, -1.65964e+19f
    path.cubic_to(
        (f32::from_bits(0x772965df), f32::from_bits(0x77777377)),
        (f32::from_bits(0x77777876), f32::from_bits(0x665266fe)),
        (f32::from_bits(0xecececdf), f32::from_bits(0x0285806e)),
    ); // 3.4358e+33f, 5.0189e+33f, 5.0193e+33f, 2.48399e+23f, -2.2914e+27f, 1.96163e-37f
    path.line_to((f32::from_bits(0xecececeb), f32::from_bits(0xecec0700))); // -2.2914e+27f, -2.28272e+27f
    path.line_to((f32::from_bits(0xfe817477), f32::from_bits(0xdf665266))); // -8.60376e+37f, -1.65964e+19f
    path.close();
    path.move_to((f32::from_bits(0xfe817477), f32::from_bits(0xdf665266))); // -8.60376e+37f, -1.65964e+19f
    path.line_to((f32::from_bits(0x65ecfaec), f32::from_bits(0xde777729))); // 1.39888e+23f, -4.45794e+18f
    path.conic_to(
        (f32::from_bits(0x74777777), f32::from_bits(0x66fe7876)),
        (f32::from_bits(0xecdf6660), f32::from_bits(0x726eecec)),
        f32::from_bits(0x29d610ec),
    ); // 7.84253e+31f, 6.00852e+23f, -2.16059e+27f, 4.73241e+30f, 9.50644e-14f
    path.line_to((f32::from_bits(0xfe817477), f32::from_bits(0xdf665266))); // -8.60376e+37f, -1.65964e+19f
    path.close();
    path.move_to((f32::from_bits(0xd0ecec10), f32::from_bits(0x6e6eecdb))); // -3.17991e+10f, 1.84859e+28f
    path.quad_to(
        (f32::from_bits(0x003affec), f32::from_bits(0xec2300ef)),
        (f32::from_bits(0xecececdb), f32::from_bits(0xcfececec)),
    ); // 5.41827e-39f, -7.88237e+26f, -2.2914e+27f, -7.9499e+09f
    path.line_to((f32::from_bits(0xd0ecec10), f32::from_bits(0x6e6eecdb))); // -3.17991e+10f, 1.84859e+28f
    path.close();
    path.move_to((f32::from_bits(0xd0ecec10), f32::from_bits(0x6e6eecdb))); // -3.17991e+10f, 1.84859e+28f
    path.quad_to(
        (f32::from_bits(0xecccec80), f32::from_bits(0xfa66ecec)),
        (f32::from_bits(0x66fa0000), f32::from_bits(0x772965df)),
    ); // -1.9819e+27f, -2.99758e+35f, 5.90296e+23f, 3.4358e+33f
    path.move_to((f32::from_bits(0x77777790), f32::from_bits(0x00807677))); // 5.01923e+33f, 1.17974e-38f
    path.close();
    path.move_to((f32::from_bits(0x77777790), f32::from_bits(0x00807677))); // 5.01923e+33f, 1.17974e-38f
    path.cubic_to(
        (f32::from_bits(0xecececec), f32::from_bits(0xfe66eaec)),
        (f32::from_bits(0xecdf1452), f32::from_bits(0x806eecec)),
        (f32::from_bits(0x10ececec), f32::from_bits(0xec000000)),
    ); // -2.2914e+27f, -7.67356e+37f, -2.15749e+27f, -1.01869e-38f, 9.34506e-29f, -6.1897e+26f
    path.line_to((f32::from_bits(0x77777790), f32::from_bits(0x00807677))); // 5.01923e+33f, 1.17974e-38f
    path.close();
    path.move_to((f32::from_bits(0x77777790), f32::from_bits(0x00807677))); // 5.01923e+33f, 1.17974e-38f
    path.cubic_to(
        (f32::from_bits(0x52668062), f32::from_bits(0x2965df66)),
        (f32::from_bits(0x77777377), f32::from_bits(0x76777773)),
        (f32::from_bits(0x1697fe78), f32::from_bits(0xeebfff00)),
    ); // 2.47499e+11f, 5.1042e-14f, 5.0189e+33f, 1.2548e+33f, 2.4556e-25f, -2.971e+28f
    path.line_to((f32::from_bits(0x77777790), f32::from_bits(0x00807677))); // 5.01923e+33f, 1.17974e-38f
    path.close();

    test_simplify_fuzz(reporter, &path.detach(), filename);
}

fn fuzz_x3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000))); // 0, 0
    path.cubic_to(
        (f32::from_bits(0x92743420), f32::from_bits(0x74747474)),
        (f32::from_bits(0x0f747c74), f32::from_bits(0xff538565)),
        (f32::from_bits(0x74744374), f32::from_bits(0x20437474)),
    ); // -7.70571e-28f, 7.74708e+31f, 1.20541e-29f, -2.8116e+38f, 7.74102e+31f, 1.65557e-19f
    path.conic_to(
        (f32::from_bits(0x7474926d), f32::from_bits(0x7c747474)),
        (f32::from_bits(0x00170f74), f32::from_bits(0x3a7410d7)),
        f32::from_bits(0x3a3a3a3a),
    ); // 7.7508e+31f, 5.07713e+36f, 2.11776e-39f, 0.000931037f, 0.000710401f
    path.quad_to(
        (f32::from_bits(0x203a3a3a), f32::from_bits(0x7459f43a)),
        (f32::from_bits(0x74747474), f32::from_bits(0x2043ad6e)),
    ); // 1.57741e-19f, 6.90724e+31f, 7.74708e+31f, 1.65745e-19f
    path.conic_to(
        (f32::from_bits(0x7474b374), f32::from_bits(0x74747474)),
        (f32::from_bits(0x0f747c74), f32::from_bits(0xff537065)),
        f32::from_bits(0x74744374),
    ); // 7.75488e+31f, 7.74708e+31f, 1.20541e-29f, -2.81051e+38f, 7.74102e+31f
    path.cubic_to(
        (f32::from_bits(0x3a3a3a3a), f32::from_bits(0x3a2c103a)),
        (f32::from_bits(0x7474263a), f32::from_bits(0x74976507)),
        (f32::from_bits(0x000000ff), f32::from_bits(0x00000000)),
    ); // 0.000710401f, 0.00065637f, 7.7374e+31f, 9.59578e+31f, 3.57331e-43f, 0
    test_simplify_fuzz(reporter, &path.detach(), filename);
}

fn fuzz_k1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000))); // 0, 0
    path.conic_to(
        (f32::from_bits(0x2073732f), f32::from_bits(0x73f17f00)),
        (f32::from_bits(0x737b7b73), f32::from_bits(0x73916773)),
        f32::from_bits(0x00738773),
    ); // 2.0621e-19f, 3.82666e+31f, 1.99245e+31f, 2.30402e+31f, 1.06097e-38f
    path.line_to((f32::from_bits(0x5803736d), f32::from_bits(0x807b5ba1))); // 5.78127e+14f, -1.13286e-38f
    path.cubic_to(
        (f32::from_bits(0x7b7f7f7b), f32::from_bits(0x7373737b)),
        (f32::from_bits(0x1b617380), f32::from_bits(0x48541b10)),
        (f32::from_bits(0x73817373), f32::from_bits(0x00717373)),
    ); // 1.32662e+36f, 1.92882e+31f, 1.86489e-22f, 217196, 2.05123e+31f, 1.04188e-38f
    path.move_to((f32::from_bits(0x7373739a), f32::from_bits(0x50001073))); // 1.92882e+31f, 8.59425e+09f
    path.cubic_to(
        (f32::from_bits(0x7b738364), f32::from_bits(0x73607380)),
        (f32::from_bits(0x7b738362), f32::from_bits(0x00007180)),
        (f32::from_bits(0x7373739a), f32::from_bits(0x50001073)),
    ); // 1.26439e+36f, 1.77829e+31f, 1.26439e+36f, 4.07161e-41f, 1.92882e+31f, 8.59425e+09f
    path.cubic_to(
        (f32::from_bits(0x7b737364), f32::from_bits(0x73607380)),
        (f32::from_bits(0x7b738366), f32::from_bits(0x73737380)),
        (f32::from_bits(0x73738873), f32::from_bits(0x96737353)),
    ); // 1.26407e+36f, 1.77829e+31f, 1.26439e+36f, 1.92882e+31f, 1.92947e+31f, -1.96658e-25f
    path.move_to((f32::from_bits(0x00640000), f32::from_bits(0x73737373))); // 9.18355e-39f, 1.92882e+31f
    path.line_to((f32::from_bits(0x40005d7b), f32::from_bits(0x58435460))); // 2.00571f, 8.59069e+14f
    path.cubic_to(
        (f32::from_bits(0x7b7f7f7b), f32::from_bits(0x7373737b)),
        (f32::from_bits(0x1b617380), f32::from_bits(0x48400010)),
        (f32::from_bits(0x73817373), f32::from_bits(0x00717373)),
    ); // 1.32662e+36f, 1.92882e+31f, 1.86489e-22f, 196608, 2.05123e+31f, 1.04188e-38f
    path.move_to((f32::from_bits(0x06737376), f32::from_bits(0x50001073))); // 4.5788e-35f, 8.59425e+09f
    path.cubic_to(
        (f32::from_bits(0x7b737364), f32::from_bits(0x73737373)),
        (f32::from_bits(0x53737388), f32::from_bits(0x00967373)),
        (f32::from_bits(0x00640000), f32::from_bits(0x73737373)),
    ); // 1.26407e+36f, 1.92882e+31f, 1.04562e+12f, 1.38167e-38f, 9.18355e-39f, 1.92882e+31f
    path.line_to((f32::from_bits(0x40005d7b), f32::from_bits(0x5843546d))); // 2.00571f, 8.59069e+14f
    path.cubic_to(
        (f32::from_bits(0x7b7f7f7b), f32::from_bits(0x7373737b)),
        (f32::from_bits(0x1b617380), f32::from_bits(0x4840001e)),
        (f32::from_bits(0x73817373), f32::from_bits(0x007e7373)),
    ); // 1.32662e+36f, 1.92882e+31f, 1.86489e-22f, 196608, 2.05123e+31f, 1.16127e-38f
    path.move_to((f32::from_bits(0x06737376), f32::from_bits(0x50001073))); // 4.5788e-35f, 8.59425e+09f
    path.cubic_to(
        (f32::from_bits(0x7b737364), f32::from_bits(0x73607380)),
        (f32::from_bits(0x01008366), f32::from_bits(0x73737380)),
        (f32::from_bits(0x737d8873), f32::from_bits(0x7b4e7b53)),
    ); // 1.26407e+36f, 1.77829e+31f, 2.36042e-38f, 1.92882e+31f, 2.0087e+31f, 1.07211e+36f
    path.cubic_to(
        (f32::from_bits(0x667b7b7b), f32::from_bits(0x73737b7b)),
        (f32::from_bits(0x73739167), f32::from_bits(0x40007387)),
        (f32::from_bits(0x5803736d), f32::from_bits(0x807b5ba1)),
    ); // 2.96898e+23f, 1.92907e+31f, 1.92974e+31f, 2.00705f, 5.78127e+14f, -1.13286e-38f
    path.cubic_to(
        (f32::from_bits(0x7b7f7f7b), f32::from_bits(0x7373737b)),
        (f32::from_bits(0x1b617380), f32::from_bits(0x48401b10)),
        (f32::from_bits(0x73817373), f32::from_bits(0x00717373)),
    ); // 1.32662e+36f, 1.92882e+31f, 1.86489e-22f, 196716, 2.05123e+31f, 1.04188e-38f
    path.move_to((f32::from_bits(0x7373739a), f32::from_bits(0x50001073))); // 1.92882e+31f, 8.59425e+09f
    path.cubic_to(
        (f32::from_bits(0x7b737364), f32::from_bits(0x73607380)),
        (f32::from_bits(0x7b738366), f32::from_bits(0x00007180)),
        (f32::from_bits(0x7373739a), f32::from_bits(0x50001073)),
    ); // 1.26407e+36f, 1.77829e+31f, 1.26439e+36f, 4.07161e-41f, 1.92882e+31f, 8.59425e+09f
    path.cubic_to(
        (f32::from_bits(0x7b737364), f32::from_bits(0x73607380)),
        (f32::from_bits(0x79738366), f32::from_bits(0x79797979)),
        (f32::from_bits(0xff000079), f32::from_bits(0xf2f2f2ff)),
    ); // 1.26407e+36f, 1.77829e+31f, 7.90246e+34f, 8.09591e+34f, -1.70144e+38f, -9.62421e+30f
    path.cubic_to(
        (f32::from_bits(0x6579796a), f32::from_bits(0x79795979)),
        (f32::from_bits(0x4d4d7b57), f32::from_bits(0x4d574d66)),
        (f32::from_bits(0x7968ac4d), f32::from_bits(0x79797979)),
    ); // 7.36318e+22f, 8.09185e+34f, 2.15463e+08f, 2.25761e+08f, 7.55067e+34f, 8.09591e+34f
    path.quad_to(
        (f32::from_bits(0xf2f27b79), f32::from_bits(0x867b9c7b)),
        (f32::from_bits(0xddf2f2f2), f32::from_bits(0x1379796a)),
    ); // -9.60571e+30f, -4.73228e-35f, -2.18829e+18f, 3.14881e-27f
    path.line_to((f32::from_bits(0x7373739a), f32::from_bits(0x50001073))); // 1.92882e+31f, 8.59425e+09f
    path.close();
    path.move_to((f32::from_bits(0x7373739a), f32::from_bits(0x50001073))); // 1.92882e+31f, 8.59425e+09f
    path.quad_to(
        (f32::from_bits(0xe7797979), f32::from_bits(0xf2794d4d)),
        (f32::from_bits(0x79a8ddf2), f32::from_bits(0x13132513)),
    ); // -1.17811e+24f, -4.93793e+30f, 1.09601e+35f, 1.85723e-27f
    path.line_to((f32::from_bits(0x7373739a), f32::from_bits(0x50001073))); // 1.92882e+31f, 8.59425e+09f
    path.close();
    path.move_to((f32::from_bits(0x7373739a), f32::from_bits(0x50001073))); // 1.92882e+31f, 8.59425e+09f
    path.quad_to(
        (f32::from_bits(0x7b9c7b79), f32::from_bits(0xf4f2d886)),
        (f32::from_bits(0xf4f4f4f4), f32::from_bits(0xf4f4f4f4)),
    ); // 1.62501e+36f, -1.53922e+32f, -1.5526e+32f, -1.5526e+32f
    test_simplify_fuzz(reporter, &path.detach(), filename);
}

// Port of: tests/PathOpsSimplifyFailTest.cpp#L232-L242 (chrome/m156)
def_test!(PathOpsSimplifyFail, |reporter| {
    fuzz_k1(reporter, "fuzz_k1");
    fuzz_x3(reporter, "fuzz_x3");
    fuzz763_2s(reporter, "fuzz763_2s");
    fuzz763_1(reporter, "fuzz763_1");
    fuzz_x2(reporter, "fuzz_x2");
    fuzz_x1(reporter, "fuzz_x1");
    fuzz_59(reporter, "fuzz_59");
    for index in 0..11 * FINITE_PTS.len() {
        dont_fail_one(reporter, index);
    }
});

// Port of: tests/PathOpsSimplifyFailTest.cpp#L244-L247 (chrome/m156)
def_test!(PathOpsSimplifyDontFailOne, |reporter| {
    let index = 17;
    dont_fail_one(reporter, index);
});
