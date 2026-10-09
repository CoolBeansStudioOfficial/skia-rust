// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsTigerTest.cpp (chrome/m156)

// The float literals below are Skia's test inputs (SkBits2Float bit patterns), copied as written.
// The threaded runner is run single-threaded, in the order the C++ runnables are appended.
#![allow(
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::cast_possible_truncation
)]
#![cfg(test)]

use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::random::Random;

use crate::unit::path_ops_extended_test::test_simplify;
use crate::{Reporter, def_test};

fn tiger8(reporter: &mut Reporter, filename: &str) {
    let mut builder = PathBuilder::new();
    builder.move_to((f32::from_bits(0x43f639c5), f32::from_bits(0x4361375a))); // 492.451f, 225.216f
    builder.quad_to(
        (f32::from_bits(0x43f58ce4), f32::from_bits(0x435d2a04)),
        (f32::from_bits(0x43f71bd9), f32::from_bits(0x435ac7d8)),
    ); // 491.101f, 221.164f, 494.218f, 218.781f
    builder.quad_to(
        (f32::from_bits(0x43f7d69d), f32::from_bits(0x4359aa35)),
        (f32::from_bits(0x43f8b3b3), f32::from_bits(0x435951c5)),
    ); // 495.677f, 217.665f, 497.404f, 217.319f
    builder.conic_to(
        (f32::from_bits(0x43f8ba67), f32::from_bits(0x43594f16)),
        (f32::from_bits(0x43f8c136), f32::from_bits(0x43594dd9)),
        f32::from_bits(0x3f7fa2b1),
    ); // 497.456f, 217.309f, 497.509f, 217.304f, 0.998576f
    builder.quad_to(
        (f32::from_bits(0x43fcc3a8), f32::from_bits(0x43589340)),
        (f32::from_bits(0x43ff01dc), f32::from_bits(0x4352e191)),
    ); // 505.529f, 216.575f, 510.015f, 210.881f
    builder.conic_to(
        (f32::from_bits(0x43ff5113), f32::from_bits(0x4352187b)),
        (f32::from_bits(0x43ffb59e), f32::from_bits(0x4352b6e9)),
        f32::from_bits(0x3f3504f3),
    ); // 510.633f, 210.096f, 511.419f, 210.714f, 0.707107f
    builder.conic_to(
        (f32::from_bits(0x43ffdc85), f32::from_bits(0x4352f435)),
        (f32::from_bits(0x43ffe4a9), f32::from_bits(0x435355e9)),
        f32::from_bits(0x3f6ec0ae),
    ); // 511.723f, 210.954f, 511.786f, 211.336f, 0.932628f
    builder.quad_to(
        (f32::from_bits(0x4400461c), f32::from_bits(0x435b3080)),
        (f32::from_bits(0x4400b692), f32::from_bits(0x4360b229)),
    ); // 513.095f, 219.189f, 514.853f, 224.696f
    builder.conic_to(
        (f32::from_bits(0x4400c662), f32::from_bits(0x43617856)),
        (f32::from_bits(0x44009920), f32::from_bits(0x4361decb)),
        f32::from_bits(0x3f46ad5b),
    ); // 515.1f, 225.47f, 514.393f, 225.87f, 0.776083f
    builder.quad_to(
        (f32::from_bits(0x43fb4920), f32::from_bits(0x43688f50)),
        (f32::from_bits(0x43f8340f), f32::from_bits(0x4365b887)),
    ); // 502.571f, 232.56f, 496.407f, 229.721f
    builder.quad_to(
        (f32::from_bits(0x43f72cd2), f32::from_bits(0x4364c612)),
        (f32::from_bits(0x43f69888), f32::from_bits(0x4362e330)),
    ); // 494.35f, 228.774f, 493.192f, 226.887f
    builder.quad_to(
        (f32::from_bits(0x43f66a00), f32::from_bits(0x43624bae)),
        (f32::from_bits(0x43f64c73), f32::from_bits(0x4361ad04)),
    ); // 492.828f, 226.296f, 492.597f, 225.676f
    builder.quad_to(
        (f32::from_bits(0x43f642ea), f32::from_bits(0x436179d2)),
        (f32::from_bits(0x43f63c1c), f32::from_bits(0x43614abe)),
    ); // 492.523f, 225.476f, 492.47f, 225.292f
    builder.quad_to(
        (f32::from_bits(0x43f639c9), f32::from_bits(0x43613aa5)),
        (f32::from_bits(0x43f63809), f32::from_bits(0x43612cda)),
    ); // 492.451f, 225.229f, 492.438f, 225.175f
    builder.quad_to(
        (f32::from_bits(0x43f63777), f32::from_bits(0x43612855)),
        (f32::from_bits(0x43f636df), f32::from_bits(0x43612357)),
    ); // 492.433f, 225.158f, 492.429f, 225.138f
    builder.quad_to(
        (f32::from_bits(0x43f6368f), f32::from_bits(0x436120b2)),
        (f32::from_bits(0x43f6367b), f32::from_bits(0x43612005)),
    ); // 492.426f, 225.128f, 492.426f, 225.125f
    builder.line_to((f32::from_bits(0x43f63656), f32::from_bits(0x43611ebc))); // 492.424f, 225.12f
    builder.line_to((f32::from_bits(0x43f63647), f32::from_bits(0x43611e34))); // 492.424f, 225.118f
    builder.line_to((f32::from_bits(0x43f6363f), f32::from_bits(0x43611df3))); // 492.424f, 225.117f
    builder.line_to((f32::from_bits(0x43f6363e), f32::from_bits(0x43611de5))); // 492.424f, 225.117f
    builder.line_to((f32::from_bits(0x43f6363f), f32::from_bits(0x43611deb))); // 492.424f, 225.117f
    builder.line_to((f32::from_bits(0x43f63647), f32::from_bits(0x43611e37))); // 492.424f, 225.118f
    builder.line_to((f32::from_bits(0x43f63644), f32::from_bits(0x43611e19))); // 492.424f, 225.118f
    builder.quad_to(
        (f32::from_bits(0x43f6365c), f32::from_bits(0x43611ee7)),
        (f32::from_bits(0x43f6365d), f32::from_bits(0x43611ef9)),
    ); // 492.425f, 225.121f, 492.425f, 225.121f
    builder.quad_to(
        (f32::from_bits(0x43f63666), f32::from_bits(0x43611f4b)),
        (f32::from_bits(0x43f63672), f32::from_bits(0x43611fb1)),
    ); // 492.425f, 225.122f, 492.425f, 225.124f
    builder.quad_to(
        (f32::from_bits(0x43f636ab), f32::from_bits(0x436121a4)),
        (f32::from_bits(0x43f636e3), f32::from_bits(0x4361236a)),
    ); // 492.427f, 225.131f, 492.429f, 225.138f
    builder.quad_to(
        (f32::from_bits(0x43f636fd), f32::from_bits(0x43612443)),
        (f32::from_bits(0x43f63705), f32::from_bits(0x4361247e)),
    ); // 492.43f, 225.142f, 492.43f, 225.143f
    builder.quad_to(
        (f32::from_bits(0x43f637d7), f32::from_bits(0x43612b15)),
        (f32::from_bits(0x43f638dc), f32::from_bits(0x436131b0)),
    ); // 492.436f, 225.168f, 492.444f, 225.194f
    builder.quad_to(
        (f32::from_bits(0x43f63b88), f32::from_bits(0x43614303)),
        (f32::from_bits(0x43f63f62), f32::from_bits(0x43615368)),
    ); // 492.465f, 225.262f, 492.495f, 225.326f
    builder.quad_to(
        (f32::from_bits(0x43f6436f), f32::from_bits(0x4361649f)),
        (f32::from_bits(0x43f648b2), f32::from_bits(0x43617468)),
    ); // 492.527f, 225.393f, 492.568f, 225.455f
    builder.quad_to(
        (f32::from_bits(0x43f68760), f32::from_bits(0x43623072)),
        (f32::from_bits(0x43f6ec71), f32::from_bits(0x4361cb60)),
    ); // 493.058f, 226.189f, 493.847f, 225.794f
    builder.quad_to(
        (f32::from_bits(0x43f722ef), f32::from_bits(0x436194e0)),
        (f32::from_bits(0x43f73027), f32::from_bits(0x43611df0)),
    ); // 494.273f, 225.582f, 494.376f, 225.117f
    builder.quad_to(
        (f32::from_bits(0x43f73334), f32::from_bits(0x43610284)),
        (f32::from_bits(0x43f73333), f32::from_bits(0x4360e667)),
    ); // 494.4f, 225.01f, 494.4f, 224.9f
    builder.line_to((f32::from_bits(0x43f63638), f32::from_bits(0x43611daf))); // 492.424f, 225.116f
    builder.line_to((f32::from_bits(0x43f6b333), f32::from_bits(0x4360e666))); // 493.4f, 224.9f
    builder.line_to((f32::from_bits(0x43f639c5), f32::from_bits(0x4361375a))); // 492.451f, 225.216f
    builder.close();
    builder.move_to((f32::from_bits(0x43f72ca1), f32::from_bits(0x43609572))); // 494.349f, 224.584f
    builder.conic_to(
        (f32::from_bits(0x43f72ebd), f32::from_bits(0x4360a219)),
        (f32::from_bits(0x43f7302e), f32::from_bits(0x4360af1f)),
        f32::from_bits(0x3f7fa741),
    ); // 494.365f, 224.633f, 494.376f, 224.684f, 0.998646f
    builder.line_to((f32::from_bits(0x43f63333), f32::from_bits(0x4360e667))); // 492.4f, 224.9f
    builder.quad_to(
        (f32::from_bits(0x43f63333), f32::from_bits(0x4360ca4b)),
        (f32::from_bits(0x43f6363f), f32::from_bits(0x4360aede)),
    ); // 492.4f, 224.79f, 492.424f, 224.683f
    builder.quad_to(
        (f32::from_bits(0x43f64377), f32::from_bits(0x436037ee)),
        (f32::from_bits(0x43f679f5), f32::from_bits(0x4360016e)),
    ); // 492.527f, 224.218f, 492.953f, 224.006f
    builder.quad_to(
        (f32::from_bits(0x43f6df06), f32::from_bits(0x435f9c5c)),
        (f32::from_bits(0x43f71db4), f32::from_bits(0x43605866)),
    ); // 493.742f, 223.611f, 494.232f, 224.345f
    builder.quad_to(
        (f32::from_bits(0x43f722f8), f32::from_bits(0x43606830)),
        (f32::from_bits(0x43f72704), f32::from_bits(0x43607966)),
    ); // 494.273f, 224.407f, 494.305f, 224.474f
    builder.quad_to(
        (f32::from_bits(0x43f72ae0), f32::from_bits(0x436089cd)),
        (f32::from_bits(0x43f72d8a), f32::from_bits(0x43609b1e)),
    ); // 494.335f, 224.538f, 494.356f, 224.606f
    builder.quad_to(
        (f32::from_bits(0x43f72e8e), f32::from_bits(0x4360a1b8)),
        (f32::from_bits(0x43f72f61), f32::from_bits(0x4360a850)),
    ); // 494.364f, 224.632f, 494.37f, 224.657f
    builder.quad_to(
        (f32::from_bits(0x43f72f68), f32::from_bits(0x4360a88a)),
        (f32::from_bits(0x43f72f83), f32::from_bits(0x4360a964)),
    ); // 494.37f, 224.658f, 494.371f, 224.662f
    builder.quad_to(
        (f32::from_bits(0x43f72fbb), f32::from_bits(0x4360ab2a)),
        (f32::from_bits(0x43f72ff4), f32::from_bits(0x4360ad1d)),
    ); // 494.373f, 224.669f, 494.375f, 224.676f
    builder.quad_to(
        (f32::from_bits(0x43f73000), f32::from_bits(0x4360ad83)),
        (f32::from_bits(0x43f73009), f32::from_bits(0x4360add5)),
    ); // 494.375f, 224.678f, 494.375f, 224.679f
    builder.quad_to(
        (f32::from_bits(0x43f7300b), f32::from_bits(0x4360ade9)),
        (f32::from_bits(0x43f73022), f32::from_bits(0x4360aeb5)),
    ); // 494.375f, 224.679f, 494.376f, 224.682f
    builder.line_to((f32::from_bits(0x43f7301f), f32::from_bits(0x4360ae97))); // 494.376f, 224.682f
    builder.line_to((f32::from_bits(0x43f73027), f32::from_bits(0x4360aee3))); // 494.376f, 224.683f
    builder.line_to((f32::from_bits(0x43f73028), f32::from_bits(0x4360aeeb))); // 494.376f, 224.683f
    builder.line_to((f32::from_bits(0x43f73027), f32::from_bits(0x4360aedf))); // 494.376f, 224.683f
    builder.line_to((f32::from_bits(0x43f73021), f32::from_bits(0x4360aeaa))); // 494.376f, 224.682f
    builder.line_to((f32::from_bits(0x43f73016), f32::from_bits(0x4360ae50))); // 494.376f, 224.681f
    builder.line_to((f32::from_bits(0x43f73007), f32::from_bits(0x4360adc1))); // 494.375f, 224.679f
    builder.line_to((f32::from_bits(0x43f72ff9), f32::from_bits(0x4360ad4d))); // 494.375f, 224.677f
    builder.quad_to(
        (f32::from_bits(0x43f7300d), f32::from_bits(0x4360adf7)),
        (f32::from_bits(0x43f73031), f32::from_bits(0x4360af12)),
    ); // 494.375f, 224.68f, 494.376f, 224.684f
    builder.quad_to(
        (f32::from_bits(0x43f730f0), f32::from_bits(0x4360b4f1)),
        (f32::from_bits(0x43f7320a), f32::from_bits(0x4360bc94)),
    ); // 494.382f, 224.707f, 494.391f, 224.737f
    builder.quad_to(
        (f32::from_bits(0x43f73625), f32::from_bits(0x4360d8fe)),
        (f32::from_bits(0x43f73c59), f32::from_bits(0x4360fa4a)),
    ); // 494.423f, 224.848f, 494.471f, 224.978f
    builder.quad_to(
        (f32::from_bits(0x43f75132), f32::from_bits(0x43616a36)),
        (f32::from_bits(0x43f772ac), f32::from_bits(0x4361d738)),
    ); // 494.634f, 225.415f, 494.896f, 225.841f
    builder.quad_to(
        (f32::from_bits(0x43f7de60), f32::from_bits(0x436335ea)),
        (f32::from_bits(0x43f89f25), f32::from_bits(0x4363e779)),
    ); // 495.737f, 227.211f, 497.243f, 227.904f
    builder.quad_to(
        (f32::from_bits(0x43fb3d30), f32::from_bits(0x436650a0)),
        (f32::from_bits(0x44005a14), f32::from_bits(0x43602133)),
    ); // 502.478f, 230.315f, 513.407f, 224.13f
    builder.line_to((f32::from_bits(0x4400799a), f32::from_bits(0x4360ffff))); // 513.9f, 225
    builder.line_to((f32::from_bits(0x44003ca2), f32::from_bits(0x43614dd5))); // 512.947f, 225.304f
    builder.quad_to(
        (f32::from_bits(0x43ff92b8), f32::from_bits(0x435ba8f8)),
        (f32::from_bits(0x43fee825), f32::from_bits(0x4353aa15)),
    ); // 511.146f, 219.66f, 509.814f, 211.664f
    builder.line_to((f32::from_bits(0x43ff6667), f32::from_bits(0x43537fff))); // 510.8f, 211.5f
    builder.line_to((f32::from_bits(0x43ffcaf2), f32::from_bits(0x43541e6d))); // 511.586f, 212.119f
    builder.quad_to(
        (f32::from_bits(0x43fd4888), f32::from_bits(0x435a7d38)),
        (f32::from_bits(0x43f8d864), f32::from_bits(0x435b4bbf)),
    ); // 506.567f, 218.489f, 497.691f, 219.296f
    builder.line_to((f32::from_bits(0x43f8cccd), f32::from_bits(0x435a4ccc))); // 497.6f, 218.3f
    builder.line_to((f32::from_bits(0x43f8e5e7), f32::from_bits(0x435b47d3))); // 497.796f, 219.281f
    builder.quad_to(
        (f32::from_bits(0x43f84300), f32::from_bits(0x435b88fd)),
        (f32::from_bits(0x43f7b75b), f32::from_bits(0x435c5e8e)),
    ); // 496.523f, 219.535f, 495.432f, 220.369f
    builder.quad_to(
        (f32::from_bits(0x43f6b984), f32::from_bits(0x435de2c4)),
        (f32::from_bits(0x43f72ca1), f32::from_bits(0x43609572)),
    ); // 493.449f, 221.886f, 494.349f, 224.584f
    builder.close();
    test_simplify(reporter, &builder.detach(), filename);
}

fn tiger8a(reporter: &mut Reporter, filename: &str) {
    let mut builder = PathBuilder::new();
    builder.move_to((f32::from_bits(0x43f639c5), f32::from_bits(0x4361375a))); // 492.451f, 225.216f
    builder.quad_to(
        (f32::from_bits(0x43f58ce4), f32::from_bits(0x435d2a04)),
        (f32::from_bits(0x43f71bd9), f32::from_bits(0x435ac7d8)),
    ); // 491.101f, 221.164f, 494.218f, 218.781f
    builder.quad_to(
        (f32::from_bits(0x43f7d69d), f32::from_bits(0x4359aa35)),
        (f32::from_bits(0x43f8b3b3), f32::from_bits(0x435951c5)),
    ); // 495.677f, 217.665f, 497.404f, 217.319f
    builder.conic_to(
        (f32::from_bits(0x43f8ba67), f32::from_bits(0x43594f16)),
        (f32::from_bits(0x43f8c136), f32::from_bits(0x43594dd9)),
        f32::from_bits(0x3f7fa2b1),
    ); // 497.456f, 217.309f, 497.509f, 217.304f, 0.998576f
    builder.quad_to(
        (f32::from_bits(0x43fcc3a8), f32::from_bits(0x43589340)),
        (f32::from_bits(0x43ff01dc), f32::from_bits(0x4352e191)),
    ); // 505.529f, 216.575f, 510.015f, 210.881f
    builder.conic_to(
        (f32::from_bits(0x43ff5113), f32::from_bits(0x4352187b)),
        (f32::from_bits(0x43ffb59e), f32::from_bits(0x4352b6e9)),
        f32::from_bits(0x3f3504f3),
    ); // 510.633f, 210.096f, 511.419f, 210.714f, 0.707107f
    builder.conic_to(
        (f32::from_bits(0x43ffdc85), f32::from_bits(0x4352f435)),
        (f32::from_bits(0x43ffe4a9), f32::from_bits(0x435355e9)),
        f32::from_bits(0x3f6ec0ae),
    ); // 511.723f, 210.954f, 511.786f, 211.336f, 0.932628f
    builder.quad_to(
        (f32::from_bits(0x4400461c), f32::from_bits(0x435b3080)),
        (f32::from_bits(0x4400b692), f32::from_bits(0x4360b229)),
    ); // 513.095f, 219.189f, 514.853f, 224.696f
    builder.conic_to(
        (f32::from_bits(0x4400c662), f32::from_bits(0x43617856)),
        (f32::from_bits(0x44009920), f32::from_bits(0x4361decb)),
        f32::from_bits(0x3f46ad5b),
    ); // 515.1f, 225.47f, 514.393f, 225.87f, 0.776083f
    builder.quad_to(
        (f32::from_bits(0x43fb4920), f32::from_bits(0x43688f50)),
        (f32::from_bits(0x43f8340f), f32::from_bits(0x4365b887)),
    ); // 502.571f, 232.56f, 496.407f, 229.721f
    builder.quad_to(
        (f32::from_bits(0x43f72cd2), f32::from_bits(0x4364c612)),
        (f32::from_bits(0x43f69888), f32::from_bits(0x4362e330)),
    ); // 494.35f, 228.774f, 493.192f, 226.887f
    builder.quad_to(
        (f32::from_bits(0x43f66a00), f32::from_bits(0x43624bae)),
        (f32::from_bits(0x43f64c73), f32::from_bits(0x4361ad04)),
    ); // 492.828f, 226.296f, 492.597f, 225.676f
    builder.quad_to(
        (f32::from_bits(0x43f642ea), f32::from_bits(0x436179d2)),
        (f32::from_bits(0x43f63c1c), f32::from_bits(0x43614abe)),
    ); // 492.523f, 225.476f, 492.47f, 225.292f
    builder.quad_to(
        (f32::from_bits(0x43f639c9), f32::from_bits(0x43613aa5)),
        (f32::from_bits(0x43f63809), f32::from_bits(0x43612cda)),
    ); // 492.451f, 225.229f, 492.438f, 225.175f
    builder.quad_to(
        (f32::from_bits(0x43f63777), f32::from_bits(0x43612855)),
        (f32::from_bits(0x43f636df), f32::from_bits(0x43612357)),
    ); // 492.433f, 225.158f, 492.429f, 225.138f
    builder.quad_to(
        (f32::from_bits(0x43f6368f), f32::from_bits(0x436120b2)),
        (f32::from_bits(0x43f6367b), f32::from_bits(0x43612005)),
    ); // 492.426f, 225.128f, 492.426f, 225.125f
    builder.line_to((f32::from_bits(0x43f63656), f32::from_bits(0x43611ebc))); // 492.424f, 225.12f
    builder.line_to((f32::from_bits(0x43f63647), f32::from_bits(0x43611e34))); // 492.424f, 225.118f
    builder.line_to((f32::from_bits(0x43f6363f), f32::from_bits(0x43611df3))); // 492.424f, 225.117f
    builder.line_to((f32::from_bits(0x43f6363e), f32::from_bits(0x43611de5))); // 492.424f, 225.117f
    builder.line_to((f32::from_bits(0x43f6363f), f32::from_bits(0x43611deb))); // 492.424f, 225.117f
    builder.line_to((f32::from_bits(0x43f63647), f32::from_bits(0x43611e37))); // 492.424f, 225.118f
    builder.line_to((f32::from_bits(0x43f63644), f32::from_bits(0x43611e19))); // 492.424f, 225.118f
    builder.quad_to(
        (f32::from_bits(0x43f6365c), f32::from_bits(0x43611ee7)),
        (f32::from_bits(0x43f6365d), f32::from_bits(0x43611ef9)),
    ); // 492.425f, 225.121f, 492.425f, 225.121f
    builder.quad_to(
        (f32::from_bits(0x43f63666), f32::from_bits(0x43611f4b)),
        (f32::from_bits(0x43f63672), f32::from_bits(0x43611fb1)),
    ); // 492.425f, 225.122f, 492.425f, 225.124f
    builder.quad_to(
        (f32::from_bits(0x43f636ab), f32::from_bits(0x436121a4)),
        (f32::from_bits(0x43f636e3), f32::from_bits(0x4361236a)),
    ); // 492.427f, 225.131f, 492.429f, 225.138f
    builder.quad_to(
        (f32::from_bits(0x43f636fd), f32::from_bits(0x43612443)),
        (f32::from_bits(0x43f63705), f32::from_bits(0x4361247e)),
    ); // 492.43f, 225.142f, 492.43f, 225.143f
    builder.quad_to(
        (f32::from_bits(0x43f637d7), f32::from_bits(0x43612b15)),
        (f32::from_bits(0x43f638dc), f32::from_bits(0x436131b0)),
    ); // 492.436f, 225.168f, 492.444f, 225.194f
    builder.quad_to(
        (f32::from_bits(0x43f63b88), f32::from_bits(0x43614303)),
        (f32::from_bits(0x43f63f62), f32::from_bits(0x43615368)),
    ); // 492.465f, 225.262f, 492.495f, 225.326f
    builder.quad_to(
        (f32::from_bits(0x43f6436f), f32::from_bits(0x4361649f)),
        (f32::from_bits(0x43f648b2), f32::from_bits(0x43617468)),
    ); // 492.527f, 225.393f, 492.568f, 225.455f
    builder.quad_to(
        (f32::from_bits(0x43f68760), f32::from_bits(0x43623072)),
        (f32::from_bits(0x43f6ec71), f32::from_bits(0x4361cb60)),
    ); // 493.058f, 226.189f, 493.847f, 225.794f
    builder.quad_to(
        (f32::from_bits(0x43f722ef), f32::from_bits(0x436194e0)),
        (f32::from_bits(0x43f73027), f32::from_bits(0x43611df0)),
    ); // 494.273f, 225.582f, 494.376f, 225.117f
    builder.quad_to(
        (f32::from_bits(0x43f73334), f32::from_bits(0x43610284)),
        (f32::from_bits(0x43f73333), f32::from_bits(0x4360e667)),
    ); // 494.4f, 225.01f, 494.4f, 224.9f
    builder.line_to((f32::from_bits(0x43f63638), f32::from_bits(0x43611daf))); // 492.424f, 225.116f
    builder.line_to((f32::from_bits(0x43f6b333), f32::from_bits(0x4360e666))); // 493.4f, 224.9f
    builder.line_to((f32::from_bits(0x43f639c5), f32::from_bits(0x4361375a))); // 492.451f, 225.216f
    builder.close();
    test_simplify(reporter, &builder.detach(), filename);
}

fn tiger8a_x(reporter: &mut Reporter, testlines: u64, test_no: &mut u32) {
    let mut builder = PathBuilder::new();
    let mut i: u32 = 0;
    let mut next_bit = || {
        let set = testlines & (1u64 << i) != 0;
        i += 1;
        set
    };
    if next_bit() {
        builder.move_to((f32::from_bits(0x43f639c5), f32::from_bits(0x4361375a)));
    } // 492.451f, 225.216f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f58ce4), f32::from_bits(0x435d2a04)),
            (f32::from_bits(0x43f71bd9), f32::from_bits(0x435ac7d8)),
        );
    } // 491.101f, 221.164f, 494.218f, 218.781f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f7d69d), f32::from_bits(0x4359aa35)),
            (f32::from_bits(0x43f8b3b3), f32::from_bits(0x435951c5)),
        );
    } // 495.677f, 217.665f, 497.404f, 217.319f
    if next_bit() {
        builder.conic_to(
            (f32::from_bits(0x43f8ba67), f32::from_bits(0x43594f16)),
            (f32::from_bits(0x43f8c136), f32::from_bits(0x43594dd9)),
            f32::from_bits(0x3f7fa2b1),
        );
    } // 497.456f, 217.309f, 497.509f, 217.304f, 0.998576f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43fcc3a8), f32::from_bits(0x43589340)),
            (f32::from_bits(0x43ff01dc), f32::from_bits(0x4352e191)),
        );
    } // 505.529f, 216.575f, 510.015f, 210.881f
    if next_bit() {
        builder.conic_to(
            (f32::from_bits(0x43ff5113), f32::from_bits(0x4352187b)),
            (f32::from_bits(0x43ffb59e), f32::from_bits(0x4352b6e9)),
            f32::from_bits(0x3f3504f3),
        );
    } // 510.633f, 210.096f, 511.419f, 210.714f, 0.707107f
    if next_bit() {
        builder.conic_to(
            (f32::from_bits(0x43ffdc85), f32::from_bits(0x4352f435)),
            (f32::from_bits(0x43ffe4a9), f32::from_bits(0x435355e9)),
            f32::from_bits(0x3f6ec0ae),
        );
    } // 511.723f, 210.954f, 511.786f, 211.336f, 0.932628f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x4400461c), f32::from_bits(0x435b3080)),
            (f32::from_bits(0x4400b692), f32::from_bits(0x4360b229)),
        );
    } // 513.095f, 219.189f, 514.853f, 224.696f
    if next_bit() {
        builder.conic_to(
            (f32::from_bits(0x4400c662), f32::from_bits(0x43617856)),
            (f32::from_bits(0x44009920), f32::from_bits(0x4361decb)),
            f32::from_bits(0x3f46ad5b),
        );
    } // 515.1f, 225.47f, 514.393f, 225.87f, 0.776083f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43fb4920), f32::from_bits(0x43688f50)),
            (f32::from_bits(0x43f8340f), f32::from_bits(0x4365b887)),
        );
    } // 502.571f, 232.56f, 496.407f, 229.721f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f72cd2), f32::from_bits(0x4364c612)),
            (f32::from_bits(0x43f69888), f32::from_bits(0x4362e330)),
        );
    } // 494.35f, 228.774f, 493.192f, 226.887f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f66a00), f32::from_bits(0x43624bae)),
            (f32::from_bits(0x43f64c73), f32::from_bits(0x4361ad04)),
        );
    } // 492.828f, 226.296f, 492.597f, 225.676f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f642ea), f32::from_bits(0x436179d2)),
            (f32::from_bits(0x43f63c1c), f32::from_bits(0x43614abe)),
        );
    } // 492.523f, 225.476f, 492.47f, 225.292f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f639c9), f32::from_bits(0x43613aa5)),
            (f32::from_bits(0x43f63809), f32::from_bits(0x43612cda)),
        );
    } // 492.451f, 225.229f, 492.438f, 225.175f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f63777), f32::from_bits(0x43612855)),
            (f32::from_bits(0x43f636df), f32::from_bits(0x43612357)),
        );
    } // 492.433f, 225.158f, 492.429f, 225.138f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f6368f), f32::from_bits(0x436120b2)),
            (f32::from_bits(0x43f6367b), f32::from_bits(0x43612005)),
        );
    } // 492.426f, 225.128f, 492.426f, 225.125f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f63656), f32::from_bits(0x43611ebc)));
    } // 492.424f, 225.12f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f63647), f32::from_bits(0x43611e34)));
    } // 492.424f, 225.118f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f6363f), f32::from_bits(0x43611df3)));
    } // 492.424f, 225.117f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f6363e), f32::from_bits(0x43611de5)));
    } // 492.424f, 225.117f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f6363f), f32::from_bits(0x43611deb)));
    } // 492.424f, 225.117f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f63647), f32::from_bits(0x43611e37)));
    } // 492.424f, 225.118f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f63644), f32::from_bits(0x43611e19)));
    } // 492.424f, 225.118f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f6365c), f32::from_bits(0x43611ee7)),
            (f32::from_bits(0x43f6365d), f32::from_bits(0x43611ef9)),
        );
    } // 492.425f, 225.121f, 492.425f, 225.121f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f63666), f32::from_bits(0x43611f4b)),
            (f32::from_bits(0x43f63672), f32::from_bits(0x43611fb1)),
        );
    } // 492.425f, 225.122f, 492.425f, 225.124f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f636ab), f32::from_bits(0x436121a4)),
            (f32::from_bits(0x43f636e3), f32::from_bits(0x4361236a)),
        );
    } // 492.427f, 225.131f, 492.429f, 225.138f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f636fd), f32::from_bits(0x43612443)),
            (f32::from_bits(0x43f63705), f32::from_bits(0x4361247e)),
        );
    } // 492.43f, 225.142f, 492.43f, 225.143f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f637d7), f32::from_bits(0x43612b15)),
            (f32::from_bits(0x43f638dc), f32::from_bits(0x436131b0)),
        );
    } // 492.436f, 225.168f, 492.444f, 225.194f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f63b88), f32::from_bits(0x43614303)),
            (f32::from_bits(0x43f63f62), f32::from_bits(0x43615368)),
        );
    } // 492.465f, 225.262f, 492.495f, 225.326f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f6436f), f32::from_bits(0x4361649f)),
            (f32::from_bits(0x43f648b2), f32::from_bits(0x43617468)),
        );
    } // 492.527f, 225.393f, 492.568f, 225.455f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f68760), f32::from_bits(0x43623072)),
            (f32::from_bits(0x43f6ec71), f32::from_bits(0x4361cb60)),
        );
    } // 493.058f, 226.189f, 493.847f, 225.794f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f722ef), f32::from_bits(0x436194e0)),
            (f32::from_bits(0x43f73027), f32::from_bits(0x43611df0)),
        );
    } // 494.273f, 225.582f, 494.376f, 225.117f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f73334), f32::from_bits(0x43610284)),
            (f32::from_bits(0x43f73333), f32::from_bits(0x4360e667)),
        );
    } // 494.4f, 225.01f, 494.4f, 224.9f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f63638), f32::from_bits(0x43611daf)));
    } // 492.424f, 225.116f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f6b333), f32::from_bits(0x4360e666)));
    } // 493.4f, 224.9f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f639c5), f32::from_bits(0x4361375a)));
    } // 492.451f, 225.216f
    if next_bit() {
        builder.close();
    }
    *test_no += 1;
    let test_name = format!("tiger8a_x{test_no}");
    test_simplify(reporter, &builder.detach(), &test_name);
}

fn tiger8b_x(reporter: &mut Reporter, testlines: u64, test_no: &mut u32) {
    let mut builder = PathBuilder::new();
    let mut i: u32 = 0;
    let mut next_bit = || {
        let set = testlines & (1u64 << i) != 0;
        i += 1;
        set
    };
    if next_bit() {
        builder.move_to((f32::from_bits(0x43f72ca1), f32::from_bits(0x43609572)));
    } // 494.349f, 224.584f
    if next_bit() {
        builder.conic_to(
            (f32::from_bits(0x43f72ebd), f32::from_bits(0x4360a219)),
            (f32::from_bits(0x43f7302e), f32::from_bits(0x4360af1f)),
            f32::from_bits(0x3f7fa741),
        );
    } // 494.365f, 224.633f, 494.376f, 224.684f, 0.998646f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f63333), f32::from_bits(0x4360e667)));
    } // 492.4f, 224.9f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f63333), f32::from_bits(0x4360ca4b)),
            (f32::from_bits(0x43f6363f), f32::from_bits(0x4360aede)),
        );
    } // 492.4f, 224.79f, 492.424f, 224.683f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f64377), f32::from_bits(0x436037ee)),
            (f32::from_bits(0x43f679f5), f32::from_bits(0x4360016e)),
        );
    } // 492.527f, 224.218f, 492.953f, 224.006f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f6df06), f32::from_bits(0x435f9c5c)),
            (f32::from_bits(0x43f71db4), f32::from_bits(0x43605866)),
        );
    } // 493.742f, 223.611f, 494.232f, 224.345f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f722f8), f32::from_bits(0x43606830)),
            (f32::from_bits(0x43f72704), f32::from_bits(0x43607966)),
        );
    } // 494.273f, 224.407f, 494.305f, 224.474f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f72ae0), f32::from_bits(0x436089cd)),
            (f32::from_bits(0x43f72d8a), f32::from_bits(0x43609b1e)),
        );
    } // 494.335f, 224.538f, 494.356f, 224.606f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f72e8e), f32::from_bits(0x4360a1b8)),
            (f32::from_bits(0x43f72f61), f32::from_bits(0x4360a850)),
        );
    } // 494.364f, 224.632f, 494.37f, 224.657f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f72f68), f32::from_bits(0x4360a88a)),
            (f32::from_bits(0x43f72f83), f32::from_bits(0x4360a964)),
        );
    } // 494.37f, 224.658f, 494.371f, 224.662f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f72fbb), f32::from_bits(0x4360ab2a)),
            (f32::from_bits(0x43f72ff4), f32::from_bits(0x4360ad1d)),
        );
    } // 494.373f, 224.669f, 494.375f, 224.676f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f73000), f32::from_bits(0x4360ad83)),
            (f32::from_bits(0x43f73009), f32::from_bits(0x4360add5)),
        );
    } // 494.375f, 224.678f, 494.375f, 224.679f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f7300b), f32::from_bits(0x4360ade9)),
            (f32::from_bits(0x43f73022), f32::from_bits(0x4360aeb5)),
        );
    } // 494.375f, 224.679f, 494.376f, 224.682f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f7301f), f32::from_bits(0x4360ae97)));
    } // 494.376f, 224.682f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f73027), f32::from_bits(0x4360aee3)));
    } // 494.376f, 224.683f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f73028), f32::from_bits(0x4360aeeb)));
    } // 494.376f, 224.683f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f73027), f32::from_bits(0x4360aedf)));
    } // 494.376f, 224.683f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f73021), f32::from_bits(0x4360aeaa)));
    } // 494.376f, 224.682f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f73016), f32::from_bits(0x4360ae50)));
    } // 494.376f, 224.681f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f73007), f32::from_bits(0x4360adc1)));
    } // 494.375f, 224.679f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f72ff9), f32::from_bits(0x4360ad4d)));
    } // 494.375f, 224.677f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f7300d), f32::from_bits(0x4360adf7)),
            (f32::from_bits(0x43f73031), f32::from_bits(0x4360af12)),
        );
    } // 494.375f, 224.68f, 494.376f, 224.684f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f730f0), f32::from_bits(0x4360b4f1)),
            (f32::from_bits(0x43f7320a), f32::from_bits(0x4360bc94)),
        );
    } // 494.382f, 224.707f, 494.391f, 224.737f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f73625), f32::from_bits(0x4360d8fe)),
            (f32::from_bits(0x43f73c59), f32::from_bits(0x4360fa4a)),
        );
    } // 494.423f, 224.848f, 494.471f, 224.978f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f75132), f32::from_bits(0x43616a36)),
            (f32::from_bits(0x43f772ac), f32::from_bits(0x4361d738)),
        );
    } // 494.634f, 225.415f, 494.896f, 225.841f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f7de60), f32::from_bits(0x436335ea)),
            (f32::from_bits(0x43f89f25), f32::from_bits(0x4363e779)),
        );
    } // 495.737f, 227.211f, 497.243f, 227.904f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43fb3d30), f32::from_bits(0x436650a0)),
            (f32::from_bits(0x44005a14), f32::from_bits(0x43602133)),
        );
    } // 502.478f, 230.315f, 513.407f, 224.13f
    if next_bit() {
        builder.line_to((f32::from_bits(0x4400799a), f32::from_bits(0x4360ffff)));
    } // 513.9f, 225
    if next_bit() {
        builder.line_to((f32::from_bits(0x44003ca2), f32::from_bits(0x43614dd5)));
    } // 512.947f, 225.304f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43ff92b8), f32::from_bits(0x435ba8f8)),
            (f32::from_bits(0x43fee825), f32::from_bits(0x4353aa15)),
        );
    } // 511.146f, 219.66f, 509.814f, 211.664f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43ff6667), f32::from_bits(0x43537fff)));
    } // 510.8f, 211.5f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43ffcaf2), f32::from_bits(0x43541e6d)));
    } // 511.586f, 212.119f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43fd4888), f32::from_bits(0x435a7d38)),
            (f32::from_bits(0x43f8d864), f32::from_bits(0x435b4bbf)),
        );
    } // 506.567f, 218.489f, 497.691f, 219.296f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f8cccd), f32::from_bits(0x435a4ccc)));
    } // 497.6f, 218.3f
    if next_bit() {
        builder.line_to((f32::from_bits(0x43f8e5e7), f32::from_bits(0x435b47d3)));
    } // 497.796f, 219.281f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f84300), f32::from_bits(0x435b88fd)),
            (f32::from_bits(0x43f7b75b), f32::from_bits(0x435c5e8e)),
        );
    } // 496.523f, 219.535f, 495.432f, 220.369f
    if next_bit() {
        builder.quad_to(
            (f32::from_bits(0x43f6b984), f32::from_bits(0x435de2c4)),
            (f32::from_bits(0x43f72ca1), f32::from_bits(0x43609572)),
        );
    } // 493.449f, 221.886f, 494.349f, 224.584f
    if next_bit() {
        builder.close();
    }
    *test_no += 1;
    let test_name = format!("tiger8b_x{test_no}");
    test_simplify(reporter, &builder.detach(), &test_name);
}

fn tiger8b(reporter: &mut Reporter, filename: &str) {
    let mut builder = PathBuilder::new();
    builder.move_to((f32::from_bits(0x43f72ca1), f32::from_bits(0x43609572))); // 494.349f, 224.584f
    builder.conic_to(
        (f32::from_bits(0x43f72ebd), f32::from_bits(0x4360a219)),
        (f32::from_bits(0x43f7302e), f32::from_bits(0x4360af1f)),
        f32::from_bits(0x3f7fa741),
    ); // 494.365f, 224.633f, 494.376f, 224.684f, 0.998646f
    builder.line_to((f32::from_bits(0x43f63333), f32::from_bits(0x4360e667))); // 492.4f, 224.9f
    builder.quad_to(
        (f32::from_bits(0x43f63333), f32::from_bits(0x4360ca4b)),
        (f32::from_bits(0x43f6363f), f32::from_bits(0x4360aede)),
    ); // 492.4f, 224.79f, 492.424f, 224.683f
    builder.quad_to(
        (f32::from_bits(0x43f64377), f32::from_bits(0x436037ee)),
        (f32::from_bits(0x43f679f5), f32::from_bits(0x4360016e)),
    ); // 492.527f, 224.218f, 492.953f, 224.006f
    builder.quad_to(
        (f32::from_bits(0x43f6df06), f32::from_bits(0x435f9c5c)),
        (f32::from_bits(0x43f71db4), f32::from_bits(0x43605866)),
    ); // 493.742f, 223.611f, 494.232f, 224.345f
    builder.quad_to(
        (f32::from_bits(0x43f722f8), f32::from_bits(0x43606830)),
        (f32::from_bits(0x43f72704), f32::from_bits(0x43607966)),
    ); // 494.273f, 224.407f, 494.305f, 224.474f
    builder.quad_to(
        (f32::from_bits(0x43f72ae0), f32::from_bits(0x436089cd)),
        (f32::from_bits(0x43f72d8a), f32::from_bits(0x43609b1e)),
    ); // 494.335f, 224.538f, 494.356f, 224.606f
    builder.quad_to(
        (f32::from_bits(0x43f72e8e), f32::from_bits(0x4360a1b8)),
        (f32::from_bits(0x43f72f61), f32::from_bits(0x4360a850)),
    ); // 494.364f, 224.632f, 494.37f, 224.657f
    builder.quad_to(
        (f32::from_bits(0x43f72f68), f32::from_bits(0x4360a88a)),
        (f32::from_bits(0x43f72f83), f32::from_bits(0x4360a964)),
    ); // 494.37f, 224.658f, 494.371f, 224.662f
    builder.quad_to(
        (f32::from_bits(0x43f72fbb), f32::from_bits(0x4360ab2a)),
        (f32::from_bits(0x43f72ff4), f32::from_bits(0x4360ad1d)),
    ); // 494.373f, 224.669f, 494.375f, 224.676f
    builder.quad_to(
        (f32::from_bits(0x43f73000), f32::from_bits(0x4360ad83)),
        (f32::from_bits(0x43f73009), f32::from_bits(0x4360add5)),
    ); // 494.375f, 224.678f, 494.375f, 224.679f
    builder.quad_to(
        (f32::from_bits(0x43f7300b), f32::from_bits(0x4360ade9)),
        (f32::from_bits(0x43f73022), f32::from_bits(0x4360aeb5)),
    ); // 494.375f, 224.679f, 494.376f, 224.682f
    builder.line_to((f32::from_bits(0x43f7301f), f32::from_bits(0x4360ae97))); // 494.376f, 224.682f
    builder.line_to((f32::from_bits(0x43f73027), f32::from_bits(0x4360aee3))); // 494.376f, 224.683f
    builder.line_to((f32::from_bits(0x43f73028), f32::from_bits(0x4360aeeb))); // 494.376f, 224.683f
    builder.line_to((f32::from_bits(0x43f73027), f32::from_bits(0x4360aedf))); // 494.376f, 224.683f
    builder.line_to((f32::from_bits(0x43f73021), f32::from_bits(0x4360aeaa))); // 494.376f, 224.682f
    builder.line_to((f32::from_bits(0x43f73016), f32::from_bits(0x4360ae50))); // 494.376f, 224.681f
    builder.line_to((f32::from_bits(0x43f73007), f32::from_bits(0x4360adc1))); // 494.375f, 224.679f
    builder.line_to((f32::from_bits(0x43f72ff9), f32::from_bits(0x4360ad4d))); // 494.375f, 224.677f
    builder.quad_to(
        (f32::from_bits(0x43f7300d), f32::from_bits(0x4360adf7)),
        (f32::from_bits(0x43f73031), f32::from_bits(0x4360af12)),
    ); // 494.375f, 224.68f, 494.376f, 224.684f
    builder.quad_to(
        (f32::from_bits(0x43f730f0), f32::from_bits(0x4360b4f1)),
        (f32::from_bits(0x43f7320a), f32::from_bits(0x4360bc94)),
    ); // 494.382f, 224.707f, 494.391f, 224.737f
    builder.quad_to(
        (f32::from_bits(0x43f73625), f32::from_bits(0x4360d8fe)),
        (f32::from_bits(0x43f73c59), f32::from_bits(0x4360fa4a)),
    ); // 494.423f, 224.848f, 494.471f, 224.978f
    builder.quad_to(
        (f32::from_bits(0x43f75132), f32::from_bits(0x43616a36)),
        (f32::from_bits(0x43f772ac), f32::from_bits(0x4361d738)),
    ); // 494.634f, 225.415f, 494.896f, 225.841f
    builder.quad_to(
        (f32::from_bits(0x43f7de60), f32::from_bits(0x436335ea)),
        (f32::from_bits(0x43f89f25), f32::from_bits(0x4363e779)),
    ); // 495.737f, 227.211f, 497.243f, 227.904f
    builder.quad_to(
        (f32::from_bits(0x43fb3d30), f32::from_bits(0x436650a0)),
        (f32::from_bits(0x44005a14), f32::from_bits(0x43602133)),
    ); // 502.478f, 230.315f, 513.407f, 224.13f
    builder.line_to((f32::from_bits(0x4400799a), f32::from_bits(0x4360ffff))); // 513.9f, 225
    builder.line_to((f32::from_bits(0x44003ca2), f32::from_bits(0x43614dd5))); // 512.947f, 225.304f
    builder.quad_to(
        (f32::from_bits(0x43ff92b8), f32::from_bits(0x435ba8f8)),
        (f32::from_bits(0x43fee825), f32::from_bits(0x4353aa15)),
    ); // 511.146f, 219.66f, 509.814f, 211.664f
    builder.line_to((f32::from_bits(0x43ff6667), f32::from_bits(0x43537fff))); // 510.8f, 211.5f
    builder.line_to((f32::from_bits(0x43ffcaf2), f32::from_bits(0x43541e6d))); // 511.586f, 212.119f
    builder.quad_to(
        (f32::from_bits(0x43fd4888), f32::from_bits(0x435a7d38)),
        (f32::from_bits(0x43f8d864), f32::from_bits(0x435b4bbf)),
    ); // 506.567f, 218.489f, 497.691f, 219.296f
    builder.line_to((f32::from_bits(0x43f8cccd), f32::from_bits(0x435a4ccc))); // 497.6f, 218.3f
    builder.line_to((f32::from_bits(0x43f8e5e7), f32::from_bits(0x435b47d3))); // 497.796f, 219.281f
    builder.quad_to(
        (f32::from_bits(0x43f84300), f32::from_bits(0x435b88fd)),
        (f32::from_bits(0x43f7b75b), f32::from_bits(0x435c5e8e)),
    ); // 496.523f, 219.535f, 495.432f, 220.369f
    builder.quad_to(
        (f32::from_bits(0x43f6b984), f32::from_bits(0x435de2c4)),
        (f32::from_bits(0x43f72ca1), f32::from_bits(0x43609572)),
    ); // 493.449f, 221.886f, 494.349f, 224.584f
    builder.close();
    test_simplify(reporter, &builder.detach(), filename);
}

// Port of: tests/PathOpsTigerTest.cpp#L194-L197 (chrome/m156)
fn tiger8a_h_1(reporter: &mut Reporter, test_no: &mut u32) {
    let testlines = 0x0000_0000_0000_2008u64; // best so far: 0x0000001d14c14bb1;
    tiger8a_x(reporter, testlines, test_no);
}

// Port of: tests/PathOpsTigerTest.cpp#L246-L253 (chrome/m156)
/// `testTiger`: `fA` and `fB` hold the low and high bytes of `testlines` (see `chalkboard`), and
/// `fC` selects the 8a or 8b builder.
fn test_tiger(reporter: &mut Reporter, a: u8, b: u8, c: u8, test_no: &mut u32) {
    let testlines = (u64::from(b) << 32) | u64::from(a);
    if c != 0 {
        tiger8b_x(reporter, testlines, test_no);
    } else {
        tiger8a_x(reporter, testlines, test_no);
    }
}

// Port of: tests/PathOpsTigerTest.cpp#L255-L280 (chrome/m156)
/// `tiger_threaded`: the runnables are collected first (drawing their bits from `SkRandom`), then
/// run in order.
fn tiger_threaded(reporter: &mut Reporter, test_no: &mut u32) {
    let mut runnables: Vec<(u8, u8, u8)> = Vec::new();
    for ab in 0..2u8 {
        let mut r = Random::default();
        let test_count = if reporter.allow_extended_test() {
            10000
        } else {
            100
        };
        for samples in 2..37 {
            for _tests in 0..test_count {
                let mut testlines = 0u64;
                for _ in 0..samples {
                    let mut bit;
                    loop {
                        bit = r.next_range_u(0, 38);
                        if testlines & (1u64 << bit) == 0 {
                            break;
                        }
                    }
                    testlines |= 1u64 << bit;
                }
                // PathOpsThreadedRunnable(..., int a, int b, ab, 0): fA = a & 0xFF, fB = b & 0xFF.
                let a = (testlines & 0xFF) as u8;
                let b = ((testlines >> 32) & 0xFF) as u8;
                runnables.push((a, b, ab));
            }
        }
    }
    for (a, b, c) in runnables {
        test_tiger(reporter, a, b, c, test_no);
    }
}

// Port of: tests/PathOpsTigerTest.cpp#L282-L285 (chrome/m156)
fn tiger8b_h_1(reporter: &mut Reporter, test_no: &mut u32) {
    let testlines = 0x0000_0000_0f27_b9e3u64; // best so far: 0x000000201304b4a3
    tiger8b_x(reporter, testlines, test_no);
}

// Port of: tests/PathOpsTigerTest.cpp#L333-L351 (chrome/m156)
def_test!(PathOpsTiger, |reporter| {
    // RunTestSet(reporter, tests, testCount, nullptr, nullptr, nullptr, false): in order.
    let mut test_no = 0;
    tiger8a_h_1(reporter, &mut test_no);
    tiger8a(reporter, "tiger8a");
    tiger8b_h_1(reporter, &mut test_no);
    tiger8b(reporter, "tiger8b");
    tiger8(reporter, "tiger8");
    tiger_threaded(reporter, &mut test_no);
});
