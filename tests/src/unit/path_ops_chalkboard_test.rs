// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsChalkboardTest.cpp (chrome/m156)

// The float literals below are Skia's test inputs (SkBits2Float bit patterns), copied as written.
// The threaded runners are run single-threaded, in the order the C++ runnables are appended.
#![allow(
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation
)]
#![cfg(test)]

use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::random::Random;

use crate::unit::path_ops_extended_test::test_simplify;
use crate::{Reporter, def_test};

// Port of: tests/PathOpsChalkboardTest.cpp#L26-L100 (chrome/m156)
/// `chalkboard(reporter, testlines)`: bit `i` of `testlines` selects whether segment `i` of the
/// chalkboard path is added. The C++ `i++` in each `if` condition is `next_bit()` here.
fn chalkboard(reporter: &mut Reporter, testlines: u64, test_no: &mut u32) {
    *test_no += 1;
    let test_name = format!("chalkboard{test_no}");
    let mut path = PathBuilder::new();
    let mut i: u32 = 0;
    let mut next_bit = || {
        let set = testlines & (1u64 << i) != 0;
        i += 1;
        set
    };
    path.move_to((f32::from_bits(0x4470eed9), f32::from_bits(0x439c1ac1))); // 963.732f, 312.209f
    if next_bit() {
        path.line_to((f32::from_bits(0x4470dde3), f32::from_bits(0x439c63d8)));
    } // 963.467f, 312.78f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x4470dbd7), f32::from_bits(0x439c3e57)),
            (f32::from_bits(0x4470c893), f32::from_bits(0x439c69fd)),
            (f32::from_bits(0x4470cfcf), f32::from_bits(0x439c297a)),
        );
    } // 963.435f, 312.487f, 963.134f, 312.828f, 963.247f, 312.324f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x4470c46b), f32::from_bits(0x439c8149)),
            (f32::from_bits(0x4470b137), f32::from_bits(0x439c2938)),
            (f32::from_bits(0x4470b5f4), f32::from_bits(0x439ca99b)),
        );
    } // 963.069f, 313.01f, 962.769f, 312.322f, 962.843f, 313.325f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x4470e842), f32::from_bits(0x439c8335)),
            (f32::from_bits(0x447125a2), f32::from_bits(0x439cce78)),
            (f32::from_bits(0x44715a2d), f32::from_bits(0x439c61ed)),
        );
    } // 963.629f, 313.025f, 964.588f, 313.613f, 965.409f, 312.765f
    if next_bit() {
        path.line_to((f32::from_bits(0x447150d5), f32::from_bits(0x439c945c)));
    } // 965.263f, 313.159f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x4471546b), f32::from_bits(0x439c87f2)),
            (f32::from_bits(0x4471579e), f32::from_bits(0x439c8085)),
            (f32::from_bits(0x44715a8f), f32::from_bits(0x439c7c4c)),
        );
    } // 965.319f, 313.062f, 965.369f, 313.004f, 965.415f, 312.971f
    if next_bit() {
        path.line_to((f32::from_bits(0x44715cbc), f32::from_bits(0x439c79dd)));
    } // 965.449f, 312.952f
    if next_bit() {
        path.line_to((f32::from_bits(0x44715dd3), f32::from_bits(0x439c7918)));
    } // 965.466f, 312.946f
    if next_bit() {
        path.line_to((f32::from_bits(0x44715e56), f32::from_bits(0x439c78d6)));
    } // 965.474f, 312.944f
    if next_bit() {
        path.line_to((f32::from_bits(0x44715e77), f32::from_bits(0x439c78b5)));
    } // 965.476f, 312.943f
    if next_bit() {
        path.line_to((f32::from_bits(0x44715e77), f32::from_bits(0x439c78b5)));
    } // 965.476f, 312.943f
    if next_bit() {
        path.line_to((f32::from_bits(0x44715e87), f32::from_bits(0x439c78b5)));
    } // 965.477f, 312.943f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x4471a50e), f32::from_bits(0x439d05c3)),
            (f32::from_bits(0x4470fe77), f32::from_bits(0x439bb894)),
            (f32::from_bits(0x44710f9e), f32::from_bits(0x439bdb03)),
        );
    } // 966.579f, 314.045f, 963.976f, 311.442f, 964.244f, 311.711f
    if next_bit() {
        path.line_to((f32::from_bits(0x44710fae), f32::from_bits(0x439bdb24)));
    } // 964.245f, 311.712f
    if next_bit() {
        path.line_to((f32::from_bits(0x44710fbe), f32::from_bits(0x439bdba7)));
    } // 964.246f, 311.716f
    if next_bit() {
        path.line_to((f32::from_bits(0x44710fce), f32::from_bits(0x439be397)));
    } // 964.247f, 311.778f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44710eb7), f32::from_bits(0x439bedf5)),
            (f32::from_bits(0x44710978), f32::from_bits(0x439bf74d)),
            (f32::from_bits(0x447105e2), f32::from_bits(0x439c0064)),
        );
    } // 964.23f, 311.859f, 964.148f, 311.932f, 964.092f, 312.003f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x4470fe86), f32::from_bits(0x439c1270)),
            (f32::from_bits(0x4470fd4f), f32::from_bits(0x439c2250)),
            (f32::from_bits(0x44712fde), f32::from_bits(0x439c33d9)),
        );
    } // 963.977f, 312.144f, 963.958f, 312.268f, 964.748f, 312.405f
    if next_bit() {
        path.line_to((f32::from_bits(0x4470fc48), f32::from_bits(0x439c3271)));
    } // 963.942f, 312.394f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x4470ee13), f32::from_bits(0x439c4c2b)),
            (f32::from_bits(0x4471476b), f32::from_bits(0x439c5c0b)),
            (f32::from_bits(0x44711177), f32::from_bits(0x439c7a40)),
        );
    } // 963.72f, 312.595f, 965.116f, 312.719f, 964.273f, 312.955f
    if next_bit() {
        path.line_to((f32::from_bits(0x44712685), f32::from_bits(0x439c7648)));
    } // 964.602f, 312.924f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x447126a6), f32::from_bits(0x439c7d31)),
            (f32::from_bits(0x44711d2d), f32::from_bits(0x439c8085)),
            (f32::from_bits(0x44711d1d), f32::from_bits(0x439c8790)),
        );
    } // 964.604f, 312.978f, 964.456f, 313.004f, 964.455f, 313.059f
    if next_bit() {
        path.line_to((f32::from_bits(0x44712675), f32::from_bits(0x439c843c)));
    } // 964.601f, 313.033f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44713bd5), f32::from_bits(0x439c94e0)),
            (f32::from_bits(0x44713956), f32::from_bits(0x439ca065)),
            (f32::from_bits(0x44712b63), f32::from_bits(0x439cb357)),
        );
    } // 964.935f, 313.163f, 964.896f, 313.253f, 964.678f, 313.401f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44711af0), f32::from_bits(0x439cb5a5)),
            (f32::from_bits(0x44712459), f32::from_bits(0x439cab47)),
            (f32::from_bits(0x44711fad), f32::from_bits(0x439ca607)),
        );
    } // 964.421f, 313.419f, 964.568f, 313.338f, 964.495f, 313.297f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44710f1a), f32::from_bits(0x439caf3e)),
            (f32::from_bits(0x4471325d), f32::from_bits(0x439cbb26)),
            (f32::from_bits(0x4471326e), f32::from_bits(0x439cc93a)),
        );
    } // 964.236f, 313.369f, 964.787f, 313.462f, 964.788f, 313.572f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44712428), f32::from_bits(0x439cd501)),
            (f32::from_bits(0x44711ad0), f32::from_bits(0x439cca82)),
            (f32::from_bits(0x447113b6), f32::from_bits(0x439cc95b)),
        );
    } // 964.565f, 313.664f, 964.419f, 313.582f, 964.308f, 313.573f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44712b95), f32::from_bits(0x439cf20f)),
            (f32::from_bits(0x4470f550), f32::from_bits(0x439d0790)),
            (f32::from_bits(0x4471426e), f32::from_bits(0x439d21ce)),
        );
    } // 964.681f, 313.891f, 963.833f, 314.059f, 965.038f, 314.264f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44715072), f32::from_bits(0x439d241c)),
            (f32::from_bits(0x44715c6a), f32::from_bits(0x439d15a5)),
            (f32::from_bits(0x44716364), f32::from_bits(0x439d24c0)),
        );
    } // 965.257f, 314.282f, 965.444f, 314.169f, 965.553f, 314.287f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44717b22), f32::from_bits(0x439d0791)),
            (f32::from_bits(0x44715cbc), f32::from_bits(0x439cf231)),
            (f32::from_bits(0x4471475c), f32::from_bits(0x439cda20)),
        );
    } // 965.924f, 314.059f, 965.449f, 313.892f, 965.115f, 313.704f
    if next_bit() {
        path.line_to((f32::from_bits(0x4471477d), f32::from_bits(0x439ce12a)));
    } // 965.117f, 313.759f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x4470fc4a), f32::from_bits(0x439cd14b)),
            (f32::from_bits(0x44715810), f32::from_bits(0x439cd0e8)),
            (f32::from_bits(0x4471372b), f32::from_bits(0x439cb272)),
        );
    } // 963.942f, 313.635f, 965.376f, 313.632f, 964.862f, 313.394f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x447155b2), f32::from_bits(0x439cb91a)),
            (f32::from_bits(0x44715581), f32::from_bits(0x439cc72e)),
            (f32::from_bits(0x447165f4), f32::from_bits(0x439ccbeb)),
        );
    } // 965.339f, 313.446f, 965.336f, 313.556f, 965.593f, 313.593f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44719e77), f32::from_bits(0x439ca2b4)),
            (f32::from_bits(0x44713979), f32::from_bits(0x439c993b)),
            (f32::from_bits(0x4471821d), f32::from_bits(0x439c7b47)),
        );
    } // 966.476f, 313.271f, 964.898f, 313.197f, 966.033f, 312.963f
    if next_bit() {
        path.line_to((f32::from_bits(0x4471847b), f32::from_bits(0x439c7dd6)));
    } // 966.07f, 312.983f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44718b96), f32::from_bits(0x439c77b1)),
            (f32::from_bits(0x44717d81), f32::from_bits(0x439c6ebb)),
            (f32::from_bits(0x44717667), f32::from_bits(0x439c66ab)),
        );
    } // 966.181f, 312.935f, 965.961f, 312.865f, 965.85f, 312.802f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44716cff), f32::from_bits(0x439c6a41)),
            (f32::from_bits(0x44716842), f32::from_bits(0x439c7315)),
            (f32::from_bits(0x44716159), f32::from_bits(0x439c793a)),
        );
    } // 965.703f, 312.83f, 965.629f, 312.899f, 965.521f, 312.947f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44715a0d), f32::from_bits(0x439c712a)),
            (f32::from_bits(0x44713938), f32::from_bits(0x439c6f3e)),
            (f32::from_bits(0x44712b34), f32::from_bits(0x439c6d73)),
        );
    } // 965.407f, 312.884f, 964.894f, 312.869f, 964.675f, 312.855f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44714c19), f32::from_bits(0x439c614a)),
            (f32::from_bits(0x44711af2), f32::from_bits(0x439c61ee)),
            (f32::from_bits(0x44712b34), f32::from_bits(0x439c518c)),
        );
    } // 965.189f, 312.76f, 964.421f, 312.765f, 964.675f, 312.637f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x447149ab), f32::from_bits(0x439c499c)),
            (f32::from_bits(0x4471474d), f32::from_bits(0x439c5c0b)),
            (f32::from_bits(0x447157d0), f32::from_bits(0x439c6065)),
        );
    } // 965.151f, 312.575f, 965.114f, 312.719f, 965.372f, 312.753f
    if next_bit() {
        path.line_to((f32::from_bits(0x447142b1), f32::from_bits(0x439c4fa0)));
    } // 965.042f, 312.622f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44714053), f32::from_bits(0x439c3f1d)),
            (f32::from_bits(0x44716396), f32::from_bits(0x439c3c6d)),
            (f32::from_bits(0x447173f9), f32::from_bits(0x439c3292)),
        );
    } // 965.005f, 312.493f, 965.556f, 312.472f, 965.812f, 312.395f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44715c7c), f32::from_bits(0x439c2628)),
            (f32::from_bits(0x44716397), f32::from_bits(0x439c3c4c)),
            (f32::from_bits(0x447142b1), f32::from_bits(0x439c3398)),
        );
    } // 965.445f, 312.298f, 965.556f, 312.471f, 965.042f, 312.403f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44715572), f32::from_bits(0x439c2919)),
            (f32::from_bits(0x44715bd8), f32::from_bits(0x439c10a6)),
            (f32::from_bits(0x447159bb), f32::from_bits(0x439bf68a)),
        );
    } // 965.335f, 312.321f, 965.435f, 312.13f, 965.402f, 311.926f
    if next_bit() {
        path.line_to((f32::from_bits(0x44715698), f32::from_bits(0x439be2f4)));
    } // 965.353f, 311.773f
    if next_bit() {
        path.line_to((f32::from_bits(0x447153f8), f32::from_bits(0x439bd95a)));
    } // 965.312f, 311.698f
    if next_bit() {
        path.line_to((f32::from_bits(0x4471526f), f32::from_bits(0x439bd49e)));
    } // 965.288f, 311.661f
    if next_bit() {
        path.line_to((f32::from_bits(0x4471524e), f32::from_bits(0x439bd45c)));
    } // 965.286f, 311.659f
    if next_bit() {
        path.line_to((f32::from_bits(0x4471523e), f32::from_bits(0x439bd41a)));
    } // 965.285f, 311.657f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44717148), f32::from_bits(0x439c124f)),
            (f32::from_bits(0x44715ae2), f32::from_bits(0x439be562)),
            (f32::from_bits(0x447161cb), f32::from_bits(0x439bf335)),
        );
    } // 965.77f, 312.143f, 965.42f, 311.792f, 965.528f, 311.9f
    if next_bit() {
        path.line_to((f32::from_bits(0x447161bb), f32::from_bits(0x439bf356)));
    } // 965.527f, 311.901f
    if next_bit() {
        path.line_to((f32::from_bits(0x447161bb), f32::from_bits(0x439bf356)));
    } // 965.527f, 311.901f
    if next_bit() {
        path.line_to((f32::from_bits(0x44716169), f32::from_bits(0x439bf3b8)));
    } // 965.522f, 311.904f
    if next_bit() {
        path.line_to((f32::from_bits(0x447160c5), f32::from_bits(0x439bf47d)));
    } // 965.512f, 311.91f
    if next_bit() {
        path.line_to((f32::from_bits(0x44715f7d), f32::from_bits(0x439bf627)));
    } // 965.492f, 311.923f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x447158f6), f32::from_bits(0x439bfeba)),
            (f32::from_bits(0x447152e1), f32::from_bits(0x439c0ac3)),
            (f32::from_bits(0x44714e15), f32::from_bits(0x439c1919)),
        );
    } // 965.39f, 311.99f, 965.295f, 312.084f, 965.22f, 312.196f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x4471548c), f32::from_bits(0x439c10c7)),
            (f32::from_bits(0x447151bb), f32::from_bits(0x439bd7f2)),
            (f32::from_bits(0x44715927), f32::from_bits(0x439be271)),
        );
    } // 965.321f, 312.131f, 965.277f, 311.687f, 965.393f, 311.769f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x447156b8), f32::from_bits(0x439bd41b)),
            (f32::from_bits(0x44714c19), f32::from_bits(0x439bf356)),
            (f32::from_bits(0x44714b13), f32::from_bits(0x439c222f)),
        );
    } // 965.355f, 311.657f, 965.189f, 311.901f, 965.173f, 312.267f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44713dd4), f32::from_bits(0x439c4aa2)),
            (f32::from_bits(0x44712ea9), f32::from_bits(0x439c2be9)),
            (f32::from_bits(0x44712344), f32::from_bits(0x439c0085)),
        );
    } // 964.966f, 312.583f, 964.729f, 312.343f, 964.551f, 312.004f
    if next_bit() {
        path.line_to((f32::from_bits(0x44712605), f32::from_bits(0x439c2fa0)));
    } // 964.594f, 312.372f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x44711af3), f32::from_bits(0x439c7e9a)),
            (f32::from_bits(0x44710de4), f32::from_bits(0x439bf41b)),
            (f32::from_bits(0x4470fb65), f32::from_bits(0x439c20c7)),
        );
    } // 964.421f, 312.989f, 964.217f, 311.907f, 963.928f, 312.256f
    if next_bit() {
        path.line_to((f32::from_bits(0x4470fbb7), f32::from_bits(0x439c220f)));
    } // 963.933f, 312.266f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x4470f5e4), f32::from_bits(0x439c2bc9)),
            (f32::from_bits(0x4470ef5d), f32::from_bits(0x439c9e59)),
            (f32::from_bits(0x4470e50f), f32::from_bits(0x439c85e6)),
        );
    } // 963.842f, 312.342f, 963.74f, 313.237f, 963.579f, 313.046f
    if next_bit() {
        path.cubic_to(
            (f32::from_bits(0x4470e8f6), f32::from_bits(0x439c4e35)),
            (f32::from_bits(0x4470ee98), f32::from_bits(0x439c5333)),
            (f32::from_bits(0x4470eed9), f32::from_bits(0x439c1ac1)),
        );
    } // 963.64f, 312.611f, 963.728f, 312.65f, 963.732f, 312.209f
    debug_assert_eq!(i, 64);
    path.close();
    test_simplify(reporter, &path.detach(), &test_name);
}
// Port of: tests/PathOpsChalkboardTest.cpp#L102-L105 (chrome/m156)
/// `testChalkboard(PathOpsThreadState*)`. `PathOpsThreadState` stores `fA` and `fB` as
/// `unsigned char`, so each runnable keeps only the low byte of the low and high 32-bit halves
/// of `testlines`; that truncation is kept as Skia has it.
fn test_chalkboard(reporter: &mut Reporter, a: u8, b: u8, test_no: &mut u32) {
    let testlines = (u64::from(b) << 32) | u64::from(a);
    chalkboard(reporter, testlines, test_no);
}

// Port of: tests/PathOpsChalkboardTest.cpp#L107-L185 (chrome/m156)
/// `chalkboard_threaded`: the runnables are collected first (drawing their bits from `SkRandom`),
/// then run in order, as the C++ `render()` does.
fn chalkboard_threaded(reporter: &mut Reporter, test_no: &mut u32) {
    let mut runnables: Vec<(u8, u8)> = Vec::new();
    let mut r = Random::default();
    let extended = reporter.allow_extended_test();
    for samples in 0..=64u32 {
        let bit_count = if samples < 32 { samples } else { 64 - samples };
        let test_count = match bit_count {
            0 => 1,
            1 => 64,
            2 => {
                if extended {
                    63 * 62 / 2
                } else {
                    100
                }
            }
            _ => {
                if extended {
                    10000
                } else {
                    100
                }
            }
        };
        let mut index1: i32 = 63;
        let mut index2: i32 = 62;
        for test in 0..test_count {
            let mut testlines: u64 = match bit_count {
                0 => 0,
                1 => 1u64 << test,
                2 if extended => {
                    debug_assert!(index1 >= 1);
                    debug_assert!(index2 >= 0);
                    let mut lines = 1u64 << index1;
                    lines |= 1u64 << index2;
                    index2 -= 1;
                    if index2 < 0 {
                        index1 -= 1;
                        index2 = index1 - 1;
                    }
                    lines
                }
                // The C++ `[[fallthrough]]` into the random default case.
                _ => {
                    let mut lines = 0u64;
                    for _ in 0..bit_count {
                        let mut bit;
                        loop {
                            bit = r.next_range_u(0, 63);
                            if lines & (1u64 << bit) == 0 {
                                break;
                            }
                        }
                        lines |= 1u64 << bit;
                    }
                    lines
                }
            };
            if samples >= 32 {
                testlines ^= 0xFFFF_FFFF_FFFF_FFFF;
            }
            // PathOpsThreadedRunnable(..., int a, int b, 0, 0): fA = a & 0xFF, fB = b & 0xFF.
            let a = (testlines & 0xFFFF_FFFF) as u8;
            let b = ((testlines >> 32) & 0xFF) as u8;
            runnables.push((a, b));
        }
    }
    for (a, b) in runnables {
        test_chalkboard(reporter, a, b, test_no);
    }
}

// Port of: tests/PathOpsChalkboardTest.cpp#L187-L190 (chrome/m156)
fn chalkboard_1(reporter: &mut Reporter, test_no: &mut u32) {
    let testlines = 0xFFFF_FFFF_FFFF_FFFFu64;
    chalkboard(reporter, testlines, test_no);
}

// Port of: tests/PathOpsChalkboardTest.cpp#L192-L197 (chrome/m156)
def_test!(PathOpsChalkboard, |reporter| {
    // RunTestSet(reporter, tests, testCount, nullptr, nullptr, nullptr, false): in order.
    let mut test_no = 0;
    chalkboard_1(reporter, &mut test_no);
    chalkboard_threaded(reporter, &mut test_no);
});
