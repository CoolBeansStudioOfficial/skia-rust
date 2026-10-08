// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsIssue3651.cpp (chrome/m156)

// The float literals below are Skia's inputs (SkBits2Float bit patterns), copied as written.
#![allow(
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::excessive_precision,
    clippy::approx_constant,
    clippy::float_cmp,
    non_snake_case
)]
#![cfg(test)]

use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_pathops::path_op::PathOp;

use crate::unit::path_ops_extended_test::{test_path_op, test_path_op_check};
use crate::{Reporter, def_test};

// Port of: tests/PathOpsIssue3651.cpp#L20-L197 (chrome/m156)
fn path1() -> Path {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x431d8000), f32::from_bits(0x42823333))); // 157.5f, 65.1f
    path.line_to((f32::from_bits(0x431d8000), f32::from_bits(0x42823333))); // 157.5f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x431e3333), f32::from_bits(0x42823333)),
        (f32::from_bits(0x431ee666), f32::from_bits(0x4282368d)),
        (f32::from_bits(0x431f999a), f32::from_bits(0x42823333)),
    ); // 158.2f, 65.1f, 158.9f, 65.1065f, 159.6f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x43204ccd), f32::from_bits(0x42822fd9)),
        (f32::from_bits(0x43210000), f32::from_bits(0x42822861)),
        (f32::from_bits(0x4321b333), f32::from_bits(0x42821f17)),
    ); // 160.3f, 65.0935f, 161, 65.0789f, 161.7f, 65.0607f
    path.cubic_to(
        (f32::from_bits(0x43226666), f32::from_bits(0x428215ce)),
        (f32::from_bits(0x4323199a), f32::from_bits(0x4282071d)),
        (f32::from_bits(0x4323cccd), f32::from_bits(0x4281fb7b)),
    ); // 162.4f, 65.0426f, 163.1f, 65.0139f, 163.8f, 64.9912f
    path.cubic_to(
        (f32::from_bits(0x43248000), f32::from_bits(0x4281efd8)),
        (f32::from_bits(0x43253333), f32::from_bits(0x4281e467)),
        (f32::from_bits(0x4325e666), f32::from_bits(0x4281d94a)),
    ); // 164.5f, 64.9684f, 165.2f, 64.9461f, 165.9f, 64.9244f
    path.cubic_to(
        (f32::from_bits(0x4326999a), f32::from_bits(0x4281ce2c)),
        (f32::from_bits(0x43274ccd), f32::from_bits(0x4281c15d)),
        (f32::from_bits(0x43280000), f32::from_bits(0x4281b8cb)),
    ); // 166.6f, 64.9027f, 167.3f, 64.8777f, 168, 64.8609f
    path.cubic_to(
        (f32::from_bits(0x4328b333), f32::from_bits(0x4281b039)),
        (f32::from_bits(0x43296666), f32::from_bits(0x4281a66d)),
        (f32::from_bits(0x432a199a), f32::from_bits(0x4281a5dd)),
    ); // 168.7f, 64.8442f, 169.4f, 64.8251f, 170.1f, 64.824f
    path.cubic_to(
        (f32::from_bits(0x432acccd), f32::from_bits(0x4281a54c)),
        (f32::from_bits(0x432b8000), f32::from_bits(0x4281aecf)),
        (f32::from_bits(0x432c3333), f32::from_bits(0x4281b566)),
    ); // 170.8f, 64.8228f, 171.5f, 64.8414f, 172.2f, 64.8543f
    path.cubic_to(
        (f32::from_bits(0x432ce666), f32::from_bits(0x4281bbfe)),
        (f32::from_bits(0x432d999a), f32::from_bits(0x4281c612)),
        (f32::from_bits(0x432e4ccd), f32::from_bits(0x4281cd6b)),
    ); // 172.9f, 64.8672f, 173.6f, 64.8869f, 174.3f, 64.9012f
    path.cubic_to(
        (f32::from_bits(0x432f0000), f32::from_bits(0x4281d4c4)),
        (f32::from_bits(0x432fb333), f32::from_bits(0x4281dc73)),
        (f32::from_bits(0x43306666), f32::from_bits(0x4281e17e)),
    ); // 175, 64.9156f, 175.7f, 64.9306f, 176.4f, 64.9404f
    path.cubic_to(
        (f32::from_bits(0x4331199a), f32::from_bits(0x4281e688)),
        (f32::from_bits(0x4331cccd), f32::from_bits(0x4281e967)),
        (f32::from_bits(0x43328000), f32::from_bits(0x4281ebaa)),
    ); // 177.1f, 64.9503f, 177.8f, 64.9559f, 178.5f, 64.9603f
    path.cubic_to(
        (f32::from_bits(0x43333333), f32::from_bits(0x4281eded)),
        (f32::from_bits(0x4333e666), f32::from_bits(0x4281eec6)),
        (f32::from_bits(0x4334999a), f32::from_bits(0x4281ef0f)),
    ); // 179.2f, 64.9647f, 179.9f, 64.9664f, 180.6f, 64.9669f
    path.cubic_to(
        (f32::from_bits(0x43354ccd), f32::from_bits(0x4281ef57)),
        (f32::from_bits(0x43360000), f32::from_bits(0x4281eeba)),
        (f32::from_bits(0x4336b333), f32::from_bits(0x4281ed5c)),
    ); // 181.3f, 64.9675f, 182, 64.9663f, 182.7f, 64.9636f
    path.cubic_to(
        (f32::from_bits(0x43376666), f32::from_bits(0x4281ebfe)),
        (f32::from_bits(0x4338199a), f32::from_bits(0x4281e8c9)),
        (f32::from_bits(0x4338cccd), f32::from_bits(0x4281e6db)),
    ); // 183.4f, 64.9609f, 184.1f, 64.9547f, 184.8f, 64.9509f
    path.cubic_to(
        (f32::from_bits(0x43398000), f32::from_bits(0x4281e4ec)),
        (f32::from_bits(0x433a3333), f32::from_bits(0x4281e29d)),
        (f32::from_bits(0x433ae666), f32::from_bits(0x4281e1c4)),
    ); // 185.5f, 64.9471f, 186.2f, 64.9426f, 186.9f, 64.9409f
    path.cubic_to(
        (f32::from_bits(0x433b999a), f32::from_bits(0x4281e0eb)),
        (f32::from_bits(0x433c4ccd), f32::from_bits(0x4281e188)),
        (f32::from_bits(0x433d0000), f32::from_bits(0x4281e1c4)),
    ); // 187.6f, 64.9393f, 188.3f, 64.9405f, 189, 64.9409f
    path.cubic_to(
        (f32::from_bits(0x433db333), f32::from_bits(0x4281e201)),
        (f32::from_bits(0x433e6666), f32::from_bits(0x4281e415)),
        (f32::from_bits(0x433f199a), f32::from_bits(0x4281e330)),
    ); // 189.7f, 64.9414f, 190.4f, 64.9455f, 191.1f, 64.9437f
    path.cubic_to(
        (f32::from_bits(0x433fcccd), f32::from_bits(0x4281e24b)),
        (f32::from_bits(0x43408000), f32::from_bits(0x4281df77)),
        (f32::from_bits(0x43413333), f32::from_bits(0x4281dc67)),
    ); // 191.8f, 64.942f, 192.5f, 64.9365f, 193.2f, 64.9305f
    path.cubic_to(
        (f32::from_bits(0x4341e666), f32::from_bits(0x4281d957)),
        (f32::from_bits(0x4342999a), f32::from_bits(0x4281d35a)),
        (f32::from_bits(0x43434ccd), f32::from_bits(0x4281d0cf)),
    ); // 193.9f, 64.9245f, 194.6f, 64.9128f, 195.3f, 64.9078f
    path.cubic_to(
        (f32::from_bits(0x43440000), f32::from_bits(0x4281ce44)),
        (f32::from_bits(0x4344b333), f32::from_bits(0x4281cd6c)),
        (f32::from_bits(0x43456666), f32::from_bits(0x4281cd24)),
    ); // 196, 64.9029f, 196.7f, 64.9012f, 197.4f, 64.9007f
    path.cubic_to(
        (f32::from_bits(0x4346199a), f32::from_bits(0x4281ccdc)),
        (f32::from_bits(0x4346cccd), f32::from_bits(0x4281cf1d)),
        (f32::from_bits(0x43478000), f32::from_bits(0x4281cf1d)),
    ); // 198.1f, 64.9001f, 198.8f, 64.9045f, 199.5f, 64.9045f
    path.cubic_to(
        (f32::from_bits(0x43483333), f32::from_bits(0x4281cf1d)),
        (f32::from_bits(0x4348e666), f32::from_bits(0x4281ce8e)),
        (f32::from_bits(0x4349999a), f32::from_bits(0x4281cd24)),
    ); // 200.2f, 64.9045f, 200.9f, 64.9034f, 201.6f, 64.9007f
    path.cubic_to(
        (f32::from_bits(0x434a4ccd), f32::from_bits(0x4281cbba)),
        (f32::from_bits(0x434b0000), f32::from_bits(0x4281c854)),
        (f32::from_bits(0x434bb333), f32::from_bits(0x4281c6a2)),
    ); // 202.3f, 64.8979f, 203, 64.8913f, 203.7f, 64.888f
    path.cubic_to(
        (f32::from_bits(0x434c6666), f32::from_bits(0x4281c4f0)),
        (f32::from_bits(0x434d199a), f32::from_bits(0x4281c46d)),
        (f32::from_bits(0x434dcccd), f32::from_bits(0x4281c2f7)),
    ); // 204.4f, 64.8846f, 205.1f, 64.8836f, 205.8f, 64.8808f
    path.cubic_to(
        (f32::from_bits(0x434e8000), f32::from_bits(0x4281c182)),
        (f32::from_bits(0x434f3333), f32::from_bits(0x4281bf4b)),
        (f32::from_bits(0x434fe666), f32::from_bits(0x4281bde1)),
    ); // 206.5f, 64.8779f, 207.2f, 64.8736f, 207.9f, 64.8709f
    path.cubic_to(
        (f32::from_bits(0x4350999a), f32::from_bits(0x4281bc77)),
        (f32::from_bits(0x43514ccd), f32::from_bits(0x4281bb92)),
        (f32::from_bits(0x43520000), f32::from_bits(0x4281ba7d)),
    ); // 208.6f, 64.8681f, 209.3f, 64.8663f, 210, 64.8642f
    path.cubic_to(
        (f32::from_bits(0x4352b333), f32::from_bits(0x4281b967)),
        (f32::from_bits(0x43536666), f32::from_bits(0x4281b95a)),
        (f32::from_bits(0x4354199a), f32::from_bits(0x4281b75f)),
    ); // 210.7f, 64.8621f, 211.4f, 64.862f, 212.1f, 64.8581f
    path.cubic_to(
        (f32::from_bits(0x4354cccd), f32::from_bits(0x4281b565)),
        (f32::from_bits(0x43558000), f32::from_bits(0x4281b0a4)),
        (f32::from_bits(0x43563333), f32::from_bits(0x4281ae9e)),
    ); // 212.8f, 64.8543f, 213.5f, 64.845f, 214.2f, 64.841f
    path.cubic_to(
        (f32::from_bits(0x4356e666), f32::from_bits(0x4281ac98)),
        (f32::from_bits(0x4357999a), f32::from_bits(0x4281aca3)),
        (f32::from_bits(0x43584ccd), f32::from_bits(0x4281ab3a)),
    ); // 214.9f, 64.8371f, 215.6f, 64.8372f, 216.3f, 64.8344f
    path.cubic_to(
        (f32::from_bits(0x43590000), f32::from_bits(0x4281a9d0)),
        (f32::from_bits(0x4359b333), f32::from_bits(0x4281a82a)),
        (f32::from_bits(0x435a6666), f32::from_bits(0x4281a623)),
    ); // 217, 64.8317f, 217.7f, 64.8284f, 218.4f, 64.8245f
    path.cubic_to(
        (f32::from_bits(0x435b199a), f32::from_bits(0x4281a41d)),
        (f32::from_bits(0x435bcccd), f32::from_bits(0x4281a157)),
        (f32::from_bits(0x435c8000), f32::from_bits(0x42819f14)),
    ); // 219.1f, 64.8205f, 219.8f, 64.8151f, 220.5f, 64.8107f
    path.cubic_to(
        (f32::from_bits(0x435d3333), f32::from_bits(0x42819cd1)),
        (f32::from_bits(0x435de666), f32::from_bits(0x42819a39)),
        (f32::from_bits(0x435e999a), f32::from_bits(0x42819892)),
    ); // 221.2f, 64.8063f, 221.9f, 64.8012f, 222.6f, 64.798f
    path.cubic_to(
        (f32::from_bits(0x435f4ccd), f32::from_bits(0x428196ec)),
        (f32::from_bits(0x43600000), f32::from_bits(0x42819455)),
        (f32::from_bits(0x4360b333), f32::from_bits(0x4281952e)),
    ); // 223.3f, 64.7948f, 224, 64.7897f, 224.7f, 64.7914f
    path.cubic_to(
        (f32::from_bits(0x43616666), f32::from_bits(0x42819607)),
        (f32::from_bits(0x4362199a), f32::from_bits(0x428198e7)),
        (f32::from_bits(0x4362cccd), f32::from_bits(0x42819da9)),
    ); // 225.4f, 64.793f, 226.1f, 64.7986f, 226.8f, 64.8079f
    path.cubic_to(
        (f32::from_bits(0x43638000), f32::from_bits(0x4281a26b)),
        (f32::from_bits(0x43643333), f32::from_bits(0x4281ad8a)),
        (f32::from_bits(0x4364e666), f32::from_bits(0x4281b1bc)),
    ); // 227.5f, 64.8172f, 228.2f, 64.8389f, 228.9f, 64.8471f
    path.cubic_to(
        (f32::from_bits(0x4365999a), f32::from_bits(0x4281b5ed)),
        (f32::from_bits(0x43664ccd), f32::from_bits(0x4281b70f)),
        (f32::from_bits(0x43670000), f32::from_bits(0x4281b6d2)),
    ); // 229.6f, 64.8553f, 230.3f, 64.8575f, 231, 64.8571f
    path.cubic_to(
        (f32::from_bits(0x4367b333), f32::from_bits(0x4281b695)),
        (f32::from_bits(0x43686666), f32::from_bits(0x4281b2db)),
        (f32::from_bits(0x4369199a), f32::from_bits(0x4281b050)),
    ); // 231.7f, 64.8566f, 232.4f, 64.8493f, 233.1f, 64.8444f
    path.cubic_to(
        (f32::from_bits(0x4369cccd), f32::from_bits(0x4281adc5)),
        (f32::from_bits(0x436a8000), f32::from_bits(0x4281a9e9)),
        (f32::from_bits(0x436b3333), f32::from_bits(0x4281a78f)),
    ); // 233.8f, 64.8394f, 234.5f, 64.8319f, 235.2f, 64.8273f
    path.cubic_to(
        (f32::from_bits(0x436be666), f32::from_bits(0x4281a535)),
        (f32::from_bits(0x436c999a), f32::from_bits(0x4281a55a)),
        (f32::from_bits(0x436d4ccd), f32::from_bits(0x4281a232)),
    ); // 235.9f, 64.8227f, 236.6f, 64.823f, 237.3f, 64.8168f
    path.cubic_to(
        (f32::from_bits(0x436e0000), f32::from_bits(0x42819f0a)),
        (f32::from_bits(0x436eb333), f32::from_bits(0x42819ad9)),
        (f32::from_bits(0x436f6666), f32::from_bits(0x428194a1)),
    ); // 238, 64.8106f, 238.7f, 64.8024f, 239.4f, 64.7903f
    path.cubic_to(
        (f32::from_bits(0x4370199a), f32::from_bits(0x42818e69)),
        (f32::from_bits(0x4370cccd), f32::from_bits(0x4281843c)),
        (f32::from_bits(0x43718000), f32::from_bits(0x42817ce3)),
    ); // 240.1f, 64.7781f, 240.8f, 64.7583f, 241.5f, 64.7439f
    path.cubic_to(
        (f32::from_bits(0x43723333), f32::from_bits(0x4281758a)),
        (f32::from_bits(0x4372e666), f32::from_bits(0x42816c36)),
        (f32::from_bits(0x4373999a), f32::from_bits(0x4281688a)),
    ); // 242.2f, 64.7296f, 242.9f, 64.7113f, 243.6f, 64.7042f
    path.cubic_to(
        (f32::from_bits(0x43744ccd), f32::from_bits(0x428164dd)),
        (f32::from_bits(0x43750000), f32::from_bits(0x428167a5)),
        (f32::from_bits(0x4375b333), f32::from_bits(0x428166d8)),
    ); // 244.3f, 64.697f, 245, 64.7024f, 245.7f, 64.7009f
    path.cubic_to(
        (f32::from_bits(0x43766666), f32::from_bits(0x4281660a)),
        (f32::from_bits(0x4377199a), f32::from_bits(0x42816651)),
        (f32::from_bits(0x4377cccd), f32::from_bits(0x428163ba)),
    ); // 246.4f, 64.6993f, 247.1f, 64.6998f, 247.8f, 64.6948f
    path.cubic_to(
        (f32::from_bits(0x43788000), f32::from_bits(0x42816123)),
        (f32::from_bits(0x43793333), f32::from_bits(0x42815b5b)),
        (f32::from_bits(0x4379e666), f32::from_bits(0x4281574e)),
    ); // 248.5f, 64.6897f, 249.2f, 64.6784f, 249.9f, 64.6705f
    path.cubic_to(
        (f32::from_bits(0x437a999a), f32::from_bits(0x42815342)),
        (f32::from_bits(0x437b4ccd), f32::from_bits(0x42814fad)),
        (f32::from_bits(0x437c0000), f32::from_bits(0x42814b6f)),
    ); // 250.6f, 64.6626f, 251.3f, 64.6556f, 252, 64.6473f
    path.cubic_to(
        (f32::from_bits(0x437cb333), f32::from_bits(0x42814732)),
        (f32::from_bits(0x437d6666), f32::from_bits(0x42813eb7)),
        (f32::from_bits(0x437e199a), f32::from_bits(0x42813dde)),
    ); // 252.7f, 64.6391f, 253.4f, 64.6225f, 254.1f, 64.6208f
    path.cubic_to(
        (f32::from_bits(0x437ecccd), f32::from_bits(0x42813d05)),
        (f32::from_bits(0x437f8000), f32::from_bits(0x428137d7)),
        (f32::from_bits(0x4380199a), f32::from_bits(0x42814659)),
    ); // 254.8f, 64.6192f, 255.5f, 64.6091f, 256.2f, 64.6374f
    path.cubic_to(
        (f32::from_bits(0x43807333), f32::from_bits(0x428154da)),
        (f32::from_bits(0x4380cccd), f32::from_bits(0x42817565)),
        (f32::from_bits(0x43812666), f32::from_bits(0x428194e8)),
    ); // 256.9f, 64.6657f, 257.6f, 64.7293f, 258.3f, 64.7908f
    path.cubic_to(
        (f32::from_bits(0x43818000), f32::from_bits(0x4281b46a)),
        (f32::from_bits(0x4381d99a), f32::from_bits(0x4281e906)),
        (f32::from_bits(0x43823333), f32::from_bits(0x42820368)),
    ); // 259, 64.8524f, 259.7f, 64.9551f, 260.4f, 65.0067f
    path.cubic_to(
        (f32::from_bits(0x43828ccd), f32::from_bits(0x42821dca)),
        (f32::from_bits(0x4382e666), f32::from_bits(0x42822b3c)),
        (f32::from_bits(0x43834000), f32::from_bits(0x42823333)),
    ); // 261.1f, 65.0582f, 261.8f, 65.0844f, 262.5f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x4383999a), f32::from_bits(0x42823b2a)),
        (f32::from_bits(0x4383f333), f32::from_bits(0x42823333)),
        (f32::from_bits(0x43844ccd), f32::from_bits(0x42823333)),
    ); // 263.2f, 65.1156f, 263.9f, 65.1f, 264.6f, 65.1f
    path.line_to((f32::from_bits(0x43844ccd), f32::from_bits(0x42823333))); // 264.6f, 65.1f
    path.line_to((f32::from_bits(0x431d8000), f32::from_bits(0x42823333))); // 157.5f, 65.1f
    path.close();
    path.move_to((f32::from_bits(0x438dc000), f32::from_bits(0x42823333))); // 283.5f, 65.1f
    path.line_to((f32::from_bits(0x438dc000), f32::from_bits(0x42823333))); // 283.5f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x438e199a), f32::from_bits(0x428230fb)),
        (f32::from_bits(0x438e7333), f32::from_bits(0x4282293a)),
        (f32::from_bits(0x438ecccd), f32::from_bits(0x428225e0)),
    ); // 284.2f, 65.0957f, 284.9f, 65.0805f, 285.6f, 65.074f
    path.cubic_to(
        (f32::from_bits(0x438f2666), f32::from_bits(0x42822286)),
        (f32::from_bits(0x438f8000), f32::from_bits(0x42821cde)),
        (f32::from_bits(0x438fd99a), f32::from_bits(0x42821f17)),
    ); // 286.3f, 65.0674f, 287, 65.0564f, 287.7f, 65.0607f
    path.cubic_to(
        (f32::from_bits(0x43903333), f32::from_bits(0x42822150)),
        (f32::from_bits(0x43908ccd), f32::from_bits(0x42822fd9)),
        (f32::from_bits(0x4390e666), f32::from_bits(0x42823333)),
    ); // 288.4f, 65.0651f, 289.1f, 65.0935f, 289.8f, 65.1f
    path.line_to((f32::from_bits(0x4390e666), f32::from_bits(0x42823333))); // 289.8f, 65.1f
    path.line_to((f32::from_bits(0x438dc000), f32::from_bits(0x42823333))); // 283.5f, 65.1f
    path.close();
    path.move_to((f32::from_bits(0x43994ccd), f32::from_bits(0x42823333))); // 306.6f, 65.1f
    path.line_to((f32::from_bits(0x43994ccd), f32::from_bits(0x42823333))); // 306.6f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x4399a666), f32::from_bits(0x42823332)),
        (f32::from_bits(0x439a0000), f32::from_bits(0x42823842)),
        (f32::from_bits(0x439a599a), f32::from_bits(0x4282332a)),
    ); // 307.3f, 65.1f, 308, 65.1099f, 308.7f, 65.0999f
    path.cubic_to(
        (f32::from_bits(0x439ab333), f32::from_bits(0x42822e12)),
        (f32::from_bits(0x439b0ccd), f32::from_bits(0x42821e94)),
        (f32::from_bits(0x439b6666), f32::from_bits(0x428214a4)),
    ); // 309.4f, 65.09f, 310.1f, 65.0597f, 310.8f, 65.0403f
    path.cubic_to(
        (f32::from_bits(0x439bc000), f32::from_bits(0x42820ab4)),
        (f32::from_bits(0x439c199a), f32::from_bits(0x42820185)),
        (f32::from_bits(0x439c7333), f32::from_bits(0x4281f789)),
    ); // 311.5f, 65.0209f, 312.2f, 65.003f, 312.9f, 64.9835f
    path.cubic_to(
        (f32::from_bits(0x439ccccd), f32::from_bits(0x4281ed8d)),
        (f32::from_bits(0x439d2666), f32::from_bits(0x4281e391)),
        (f32::from_bits(0x439d8000), f32::from_bits(0x4281d8bc)),
    ); // 313.6f, 64.964f, 314.3f, 64.9445f, 315, 64.9233f
    path.cubic_to(
        (f32::from_bits(0x439dd99a), f32::from_bits(0x4281cde7)),
        (f32::from_bits(0x439e3333), f32::from_bits(0x4281c0c4)),
        (f32::from_bits(0x439e8ccd), f32::from_bits(0x4281b68b)),
    ); // 315.7f, 64.9022f, 316.4f, 64.8765f, 317.1f, 64.8565f
    path.cubic_to(
        (f32::from_bits(0x439ee666), f32::from_bits(0x4281ac53)),
        (f32::from_bits(0x439f4000), f32::from_bits(0x4281a27a)),
        (f32::from_bits(0x439f999a), f32::from_bits(0x42819b69)),
    ); // 317.8f, 64.8366f, 318.5f, 64.8173f, 319.2f, 64.8035f
    path.cubic_to(
        (f32::from_bits(0x439ff333), f32::from_bits(0x42819459)),
        (f32::from_bits(0x43a04ccd), f32::from_bits(0x42818f8b)),
        (f32::from_bits(0x43a0a666), f32::from_bits(0x42818c26)),
    ); // 319.9f, 64.7897f, 320.6f, 64.7804f, 321.3f, 64.7737f
    path.cubic_to(
        (f32::from_bits(0x43a10000), f32::from_bits(0x428188c2)),
        (f32::from_bits(0x43a1599a), f32::from_bits(0x42818795)),
        (f32::from_bits(0x43a1b333), f32::from_bits(0x42818710)),
    ); // 322, 64.7671f, 322.7f, 64.7648f, 323.4f, 64.7638f
    path.cubic_to(
        (f32::from_bits(0x43a20ccd), f32::from_bits(0x4281868b)),
        (f32::from_bits(0x43a26666), f32::from_bits(0x42818824)),
        (f32::from_bits(0x43a2c000), f32::from_bits(0x42818909)),
    ); // 324.1f, 64.7628f, 324.8f, 64.7659f, 325.5f, 64.7676f
    path.cubic_to(
        (f32::from_bits(0x43a3199a), f32::from_bits(0x428189ee)),
        (f32::from_bits(0x43a37333), f32::from_bits(0x42818de2)),
        (f32::from_bits(0x43a3cccd), f32::from_bits(0x42818c6d)),
    ); // 326.2f, 64.7694f, 326.9f, 64.7771f, 327.6f, 64.7743f
    path.cubic_to(
        (f32::from_bits(0x43a42666), f32::from_bits(0x42818af7)),
        (f32::from_bits(0x43a48000), f32::from_bits(0x428185be)),
        (f32::from_bits(0x43a4d99a), f32::from_bits(0x42818048)),
    ); // 328.3f, 64.7714f, 329, 64.7612f, 329.7f, 64.7505f
    path.cubic_to(
        (f32::from_bits(0x43a53333), f32::from_bits(0x42817ad1)),
        (f32::from_bits(0x43a58ccd), f32::from_bits(0x42816e33)),
        (f32::from_bits(0x43a5e666), f32::from_bits(0x42816ba7)),
    ); // 330.4f, 64.7399f, 331.1f, 64.7152f, 331.8f, 64.7103f
    path.cubic_to(
        (f32::from_bits(0x43a64000), f32::from_bits(0x4281691c)),
        (f32::from_bits(0x43a6999a), f32::from_bits(0x42816b46)),
        (f32::from_bits(0x43a6f333), f32::from_bits(0x42817104)),
    ); // 332.5f, 64.7053f, 333.2f, 64.7095f, 333.9f, 64.7207f
    path.cubic_to(
        (f32::from_bits(0x43a74ccd), f32::from_bits(0x428176c3)),
        (f32::from_bits(0x43a7a666), f32::from_bits(0x42817fa9)),
        (f32::from_bits(0x43a80000), f32::from_bits(0x42818e1f)),
    ); // 334.6f, 64.732f, 335.3f, 64.7493f, 336, 64.7776f
    path.cubic_to(
        (f32::from_bits(0x43a8599a), f32::from_bits(0x42819c95)),
        (f32::from_bits(0x43a8b333), f32::from_bits(0x4281b1ec)),
        (f32::from_bits(0x43a90ccd), f32::from_bits(0x4281c7c7)),
    ); // 336.7f, 64.8058f, 337.4f, 64.8475f, 338.1f, 64.8902f
    path.cubic_to(
        (f32::from_bits(0x43a96666), f32::from_bits(0x4281dda2)),
        (f32::from_bits(0x43a9c000), f32::from_bits(0x428209cf)),
        (f32::from_bits(0x43aa199a), f32::from_bits(0x42821140)),
    ); // 338.8f, 64.9329f, 339.5f, 65.0192f, 340.2f, 65.0337f
    path.cubic_to(
        (f32::from_bits(0x43aa7333), f32::from_bits(0x428218b0)),
        (f32::from_bits(0x43aacccd), f32::from_bits(0x42820dff)),
        (f32::from_bits(0x43ab2666), f32::from_bits(0x4281f46b)),
    ); // 340.9f, 65.0482f, 341.6f, 65.0273f, 342.3f, 64.9774f
    path.cubic_to(
        (f32::from_bits(0x43ab8000), f32::from_bits(0x4281dad8)),
        (f32::from_bits(0x43abd99a), f32::from_bits(0x42819956)),
        (f32::from_bits(0x43ac3333), f32::from_bits(0x428177cd)),
    ); // 343, 64.9274f, 343.7f, 64.7995f, 344.4f, 64.734f
    path.cubic_to(
        (f32::from_bits(0x43ac8ccd), f32::from_bits(0x42815644)),
        (f32::from_bits(0x43ace666), f32::from_bits(0x42813910)),
        (f32::from_bits(0x43ad4000), f32::from_bits(0x42812b37)),
    ); // 345.1f, 64.6685f, 345.8f, 64.6115f, 346.5f, 64.5844f
    path.cubic_to(
        (f32::from_bits(0x43ad999a), f32::from_bits(0x42811d5e)),
        (f32::from_bits(0x43adf333), f32::from_bits(0x42812394)),
        (f32::from_bits(0x43ae4ccd), f32::from_bits(0x428124b5)),
    ); // 347.2f, 64.5574f, 347.9f, 64.5695f, 348.6f, 64.5717f
    path.cubic_to(
        (f32::from_bits(0x43aea666), f32::from_bits(0x428125d6)),
        (f32::from_bits(0x43af0000), f32::from_bits(0x42812c1c)),
        (f32::from_bits(0x43af599a), f32::from_bits(0x428131ff)),
    ); // 349.3f, 64.5739f, 350, 64.5862f, 350.7f, 64.5976f
    path.cubic_to(
        (f32::from_bits(0x43afb333), f32::from_bits(0x428137e3)),
        (f32::from_bits(0x43b00ccd), f32::from_bits(0x4281417f)),
        (f32::from_bits(0x43b06666), f32::from_bits(0x4281480b)),
    ); // 351.4f, 64.6092f, 352.1f, 64.6279f, 352.8f, 64.6407f
    path.cubic_to(
        (f32::from_bits(0x43b0c000), f32::from_bits(0x42814e97)),
        (f32::from_bits(0x43b1199a), f32::from_bits(0x4281534c)),
        (f32::from_bits(0x43b17333), f32::from_bits(0x42815947)),
    ); // 353.5f, 64.6535f, 354.2f, 64.6627f, 354.9f, 64.6744f
    path.cubic_to(
        (f32::from_bits(0x43b1cccd), f32::from_bits(0x42815f42)),
        (f32::from_bits(0x43b22666), f32::from_bits(0x428165ff)),
        (f32::from_bits(0x43b28000), f32::from_bits(0x42816bee)),
    ); // 355.6f, 64.6861f, 356.3f, 64.6992f, 357, 64.7108f
    path.cubic_to(
        (f32::from_bits(0x43b2d99a), f32::from_bits(0x428171de)),
        (f32::from_bits(0x43b33333), f32::from_bits(0x42817af5)),
        (f32::from_bits(0x43b38ccd), f32::from_bits(0x42817ce3)),
    ); // 357.7f, 64.7224f, 358.4f, 64.7402f, 359.1f, 64.7439f
    path.cubic_to(
        (f32::from_bits(0x43b3e666), f32::from_bits(0x42817ed2)),
        (f32::from_bits(0x43b44000), f32::from_bits(0x42817bcf)),
        (f32::from_bits(0x43b4999a), f32::from_bits(0x42817786)),
    ); // 359.8f, 64.7477f, 360.5f, 64.7418f, 361.2f, 64.7334f
    path.cubic_to(
        (f32::from_bits(0x43b4f333), f32::from_bits(0x4281733d)),
        (f32::from_bits(0x43b54ccd), f32::from_bits(0x428167a7)),
        (f32::from_bits(0x43b5a666), f32::from_bits(0x4281632d)),
    ); // 361.9f, 64.7251f, 362.6f, 64.7024f, 363.3f, 64.6937f
    path.cubic_to(
        (f32::from_bits(0x43b60000), f32::from_bits(0x42815eb3)),
        (f32::from_bits(0x43b6599a), f32::from_bits(0x42815b7e)),
        (f32::from_bits(0x43b6b333), f32::from_bits(0x42815cab)),
    ); // 364, 64.685f, 364.7f, 64.6787f, 365.4f, 64.681f
    path.cubic_to(
        (f32::from_bits(0x43b70ccd), f32::from_bits(0x42815dd8)),
        (f32::from_bits(0x43b76666), f32::from_bits(0x4281644d)),
        (f32::from_bits(0x43b7c000), f32::from_bits(0x42816a3c)),
    ); // 366.1f, 64.6833f, 366.8f, 64.6959f, 367.5f, 64.7075f
    path.cubic_to(
        (f32::from_bits(0x43b8199a), f32::from_bits(0x4281702b)),
        (f32::from_bits(0x43b87333), f32::from_bits(0x428179d3)),
        (f32::from_bits(0x43b8cccd), f32::from_bits(0x42818048)),
    ); // 368.2f, 64.7191f, 368.9f, 64.7379f, 369.6f, 64.7505f
    path.cubic_to(
        (f32::from_bits(0x43b92666), f32::from_bits(0x428186bc)),
        (f32::from_bits(0x43b98000), f32::from_bits(0x42818d4a)),
        (f32::from_bits(0x43b9d99a), f32::from_bits(0x428190f6)),
    ); // 370.3f, 64.7632f, 371, 64.776f, 371.7f, 64.7831f
    path.cubic_to(
        (f32::from_bits(0x43ba3333), f32::from_bits(0x428194a3)),
        (f32::from_bits(0x43ba8ccd), f32::from_bits(0x428193b0)),
        (f32::from_bits(0x43bae666), f32::from_bits(0x42819653)),
    ); // 372.4f, 64.7903f, 373.1f, 64.7885f, 373.8f, 64.7936f
    path.cubic_to(
        (f32::from_bits(0x43bb4000), f32::from_bits(0x428198f6)),
        (f32::from_bits(0x43bb999a), f32::from_bits(0x42819840)),
        (f32::from_bits(0x43bbf333), f32::from_bits(0x4281a0c6)),
    ); // 374.5f, 64.7988f, 375.2f, 64.7974f, 375.9f, 64.814f
    path.cubic_to(
        (f32::from_bits(0x43bc4ccd), f32::from_bits(0x4281a94d)),
        (f32::from_bits(0x43bca666), f32::from_bits(0x4281bc0d)),
        (f32::from_bits(0x43bd0000), f32::from_bits(0x4281c979)),
    ); // 376.6f, 64.8307f, 377.3f, 64.8673f, 378, 64.8935f
    path.cubic_to(
        (f32::from_bits(0x43bd599a), f32::from_bits(0x4281d6e5)),
        (f32::from_bits(0x43bdb333), f32::from_bits(0x4281e6fe)),
        (f32::from_bits(0x43be0ccd), f32::from_bits(0x4281f14e)),
    ); // 378.7f, 64.9197f, 379.4f, 64.9512f, 380.1f, 64.9713f
    path.cubic_to(
        (f32::from_bits(0x43be6666), f32::from_bits(0x4281fb9e)),
        (f32::from_bits(0x43bec000), f32::from_bits(0x4281fd75)),
        (f32::from_bits(0x43bf199a), f32::from_bits(0x42820759)),
    ); // 380.8f, 64.9914f, 381.5f, 64.995f, 382.2f, 65.0144f
    path.cubic_to(
        (f32::from_bits(0x43bf7333), f32::from_bits(0x4282113e)),
        (f32::from_bits(0x43bfcccd), f32::from_bits(0x42822559)),
        (f32::from_bits(0x43c02666), f32::from_bits(0x42822ca8)),
    ); // 382.9f, 65.0337f, 383.6f, 65.0729f, 384.3f, 65.0872f
    path.line_to((f32::from_bits(0x43c02666), f32::from_bits(0x42823333))); // 384.3f, 65.1f
    path.line_to((f32::from_bits(0x43994ccd), f32::from_bits(0x42823333))); // 306.6f, 65.1f
    path.close();
    path.move_to((f32::from_bits(0x43c24000), f32::from_bits(0x42823333))); // 388.5f, 65.1f
    path.line_to((f32::from_bits(0x43c24000), f32::from_bits(0x42823333))); // 388.5f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x43c2999a), f32::from_bits(0x42823333)),
        (f32::from_bits(0x43c2f333), f32::from_bits(0x428239f1)),
        (f32::from_bits(0x43c34ccd), f32::from_bits(0x42823333)),
    ); // 389.2f, 65.1f, 389.9f, 65.1132f, 390.6f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x43c3a666), f32::from_bits(0x42822c75)),
        (f32::from_bits(0x43c40000), f32::from_bits(0x42822289)),
        (f32::from_bits(0x43c4599a), f32::from_bits(0x42820abe)),
    ); // 391.3f, 65.0868f, 392, 65.0675f, 392.7f, 65.021f
    path.cubic_to(
        (f32::from_bits(0x43c4b333), f32::from_bits(0x4281f2f3)),
        (f32::from_bits(0x43c50ccd), f32::from_bits(0x4281be4d)),
        (f32::from_bits(0x43c56666), f32::from_bits(0x4281a471)),
    ); // 393.4f, 64.9745f, 394.1f, 64.8717f, 394.8f, 64.8212f
    path.cubic_to(
        (f32::from_bits(0x43c5c000), f32::from_bits(0x42818a96)),
        (f32::from_bits(0x43c6199a), f32::from_bits(0x428177e3)),
        (f32::from_bits(0x43c67333), f32::from_bits(0x42816f99)),
    ); // 395.5f, 64.7707f, 396.2f, 64.7342f, 396.9f, 64.718f
    path.cubic_to(
        (f32::from_bits(0x43c6cccd), f32::from_bits(0x4281674f)),
        (f32::from_bits(0x43c72666), f32::from_bits(0x42817195)),
        (f32::from_bits(0x43c78000), f32::from_bits(0x428172b7)),
    ); // 397.6f, 64.7018f, 398.3f, 64.7218f, 399, 64.7241f
    path.cubic_to(
        (f32::from_bits(0x43c7d99a), f32::from_bits(0x428173d8)),
        (f32::from_bits(0x43c83333), f32::from_bits(0x42817528)),
        (f32::from_bits(0x43c88ccd), f32::from_bits(0x42817661)),
    ); // 399.7f, 64.7263f, 400.4f, 64.7288f, 401.1f, 64.7312f
    path.cubic_to(
        (f32::from_bits(0x43c8e666), f32::from_bits(0x4281779a)),
        (f32::from_bits(0x43c94000), f32::from_bits(0x4281778d)),
        (f32::from_bits(0x43c9999a), f32::from_bits(0x42817a0c)),
    ); // 401.8f, 64.7336f, 402.5f, 64.7335f, 403.2f, 64.7384f
    path.cubic_to(
        (f32::from_bits(0x43c9f333), f32::from_bits(0x42817c8c)),
        (f32::from_bits(0x43ca4ccd), f32::from_bits(0x42817f49)),
        (f32::from_bits(0x43caa666), f32::from_bits(0x4281855e)),
    ); // 403.9f, 64.7433f, 404.6f, 64.7486f, 405.3f, 64.7605f
    path.cubic_to(
        (f32::from_bits(0x43cb0000), f32::from_bits(0x42818b72)),
        (f32::from_bits(0x43cb599a), f32::from_bits(0x4281985b)),
        (f32::from_bits(0x43cbb333), f32::from_bits(0x42819e87)),
    ); // 406, 64.7724f, 406.7f, 64.7976f, 407.4f, 64.8096f
    path.cubic_to(
        (f32::from_bits(0x43cc0ccd), f32::from_bits(0x4281a4b3)),
        (f32::from_bits(0x43cc6666), f32::from_bits(0x4281a6ea)),
        (f32::from_bits(0x43ccc000), f32::from_bits(0x4281aa66)),
    ); // 408.1f, 64.8217f, 408.8f, 64.826f, 409.5f, 64.8328f
    path.cubic_to(
        (f32::from_bits(0x43cd199a), f32::from_bits(0x4281ade2)),
        (f32::from_bits(0x43cd7333), f32::from_bits(0x4281aad0)),
        (f32::from_bits(0x43cdcccd), f32::from_bits(0x4281b36e)),
    ); // 410.2f, 64.8396f, 410.9f, 64.8336f, 411.6f, 64.8504f
    path.cubic_to(
        (f32::from_bits(0x43ce2666), f32::from_bits(0x4281bc0c)),
        (f32::from_bits(0x43ce8000), f32::from_bits(0x4281d071)),
        (f32::from_bits(0x43ced99a), f32::from_bits(0x4281de19)),
    ); // 412.3f, 64.8673f, 413, 64.9071f, 413.7f, 64.9338f
    path.cubic_to(
        (f32::from_bits(0x43cf3333), f32::from_bits(0x4281ebc2)),
        (f32::from_bits(0x43cf8ccd), f32::from_bits(0x4281fb65)),
        (f32::from_bits(0x43cfe666), f32::from_bits(0x42820561)),
    ); // 414.4f, 64.9605f, 415.1f, 64.991f, 415.8f, 65.0105f
    path.cubic_to(
        (f32::from_bits(0x43d04000), f32::from_bits(0x42820f5d)),
        (f32::from_bits(0x43d0999a), f32::from_bits(0x428217a6)),
        (f32::from_bits(0x43d0f333), f32::from_bits(0x42821a01)),
    ); // 416.5f, 65.03f, 417.2f, 65.0462f, 417.9f, 65.0508f
    path.cubic_to(
        (f32::from_bits(0x43d14ccd), f32::from_bits(0x42821c5b)),
        (f32::from_bits(0x43d1a666), f32::from_bits(0x42821a47)),
        (f32::from_bits(0x43d20000), f32::from_bits(0x4282137f)),
    ); // 418.6f, 65.0554f, 419.3f, 65.0513f, 420, 65.0381f
    path.cubic_to(
        (f32::from_bits(0x43d2599a), f32::from_bits(0x42820cb6)),
        (f32::from_bits(0x43d2b333), f32::from_bits(0x4281fcb3)),
        (f32::from_bits(0x43d30ccd), f32::from_bits(0x4281f14e)),
    ); // 420.7f, 65.0248f, 421.4f, 64.9936f, 422.1f, 64.9713f
    path.cubic_to(
        (f32::from_bits(0x43d36666), f32::from_bits(0x4281e5e8)),
        (f32::from_bits(0x43d3c000), f32::from_bits(0x4281d645)),
        (f32::from_bits(0x43d4199a), f32::from_bits(0x4281cf1d)),
    ); // 422.8f, 64.949f, 423.5f, 64.9185f, 424.2f, 64.9045f
    path.cubic_to(
        (f32::from_bits(0x43d47333), f32::from_bits(0x4281c7f4)),
        (f32::from_bits(0x43d4cccd), f32::from_bits(0x4281c5d7)),
        (f32::from_bits(0x43d52666), f32::from_bits(0x4281c65c)),
    ); // 424.9f, 64.8905f, 425.6f, 64.8864f, 426.3f, 64.8874f
    path.cubic_to(
        (f32::from_bits(0x43d58000), f32::from_bits(0x4281c6e1)),
        (f32::from_bits(0x43d5d99a), f32::from_bits(0x4281d040)),
        (f32::from_bits(0x43d63333), f32::from_bits(0x4281d23a)),
    ); // 427, 64.8884f, 427.7f, 64.9067f, 428.4f, 64.9106f
    path.cubic_to(
        (f32::from_bits(0x43d68ccd), f32::from_bits(0x4281d435)),
        (f32::from_bits(0x43d6e666), f32::from_bits(0x4281d7ed)),
        (f32::from_bits(0x43d74000), f32::from_bits(0x4281d23a)),
    ); // 429.1f, 64.9145f, 429.8f, 64.9217f, 430.5f, 64.9106f
    path.cubic_to(
        (f32::from_bits(0x43d7999a), f32::from_bits(0x4281cc88)),
        (f32::from_bits(0x43d7f333), f32::from_bits(0x4281ba4e)),
        (f32::from_bits(0x43d84ccd), f32::from_bits(0x4281b009)),
    ); // 431.2f, 64.8995f, 431.9f, 64.8639f, 432.6f, 64.8438f
    path.cubic_to(
        (f32::from_bits(0x43d8a666), f32::from_bits(0x4281a5c5)),
        (f32::from_bits(0x43d90000), f32::from_bits(0x4281997b)),
        (f32::from_bits(0x43d9599a), f32::from_bits(0x428194a1)),
    ); // 433.3f, 64.8238f, 434, 64.7998f, 434.7f, 64.7903f
    path.cubic_to(
        (f32::from_bits(0x43d9b333), f32::from_bits(0x42818fc7)),
        (f32::from_bits(0x43da0ccd), f32::from_bits(0x4281929b)),
        (f32::from_bits(0x43da6666), f32::from_bits(0x428192ef)),
    ); // 435.4f, 64.7808f, 436.1f, 64.7863f, 436.8f, 64.787f
    path.cubic_to(
        (f32::from_bits(0x43dac000), f32::from_bits(0x42819343)),
        (f32::from_bits(0x43db199a), f32::from_bits(0x428194dc)),
        (f32::from_bits(0x43db7333), f32::from_bits(0x4281969a)),
    ); // 437.5f, 64.7876f, 438.2f, 64.7907f, 438.9f, 64.7941f
    path.cubic_to(
        (f32::from_bits(0x43dbcccd), f32::from_bits(0x42819858)),
        (f32::from_bits(0x43dc2666), f32::from_bits(0x42819925)),
        (f32::from_bits(0x43dc8000), f32::from_bits(0x42819d62)),
    ); // 439.6f, 64.7975f, 440.3f, 64.7991f, 441, 64.8074f
    path.cubic_to(
        (f32::from_bits(0x43dcd99a), f32::from_bits(0x4281a19f)),
        (f32::from_bits(0x43dd3333), f32::from_bits(0x4281a9d2)),
        (f32::from_bits(0x43dd8ccd), f32::from_bits(0x4281b009)),
    ); // 441.7f, 64.8157f, 442.4f, 64.8317f, 443.1f, 64.8438f
    path.cubic_to(
        (f32::from_bits(0x43dde666), f32::from_bits(0x4281b641)),
        (f32::from_bits(0x43de4000), f32::from_bits(0x4281be1f)),
        (f32::from_bits(0x43de999a), f32::from_bits(0x4281c2b1)),
    ); // 443.8f, 64.856f, 444.5f, 64.8713f, 445.2f, 64.8803f
    path.cubic_to(
        (f32::from_bits(0x43def333), f32::from_bits(0x4281c742)),
        (f32::from_bits(0x43df4ccd), f32::from_bits(0x4281ca45)),
        (f32::from_bits(0x43dfa666), f32::from_bits(0x4281cb72)),
    ); // 445.9f, 64.8892f, 446.6f, 64.8951f, 447.3f, 64.8974f
    path.cubic_to(
        (f32::from_bits(0x43e00000), f32::from_bits(0x4281cc9f)),
        (f32::from_bits(0x43e0599a), f32::from_bits(0x4281cb72)),
        (f32::from_bits(0x43e0b333), f32::from_bits(0x4281c9c0)),
    ); // 448, 64.8997f, 448.7f, 64.8974f, 449.4f, 64.894f
    path.cubic_to(
        (f32::from_bits(0x43e10ccd), f32::from_bits(0x4281c80e)),
        (f32::from_bits(0x43e16666), f32::from_bits(0x4281c34c)),
        (f32::from_bits(0x43e1c000), f32::from_bits(0x4281c145)),
    ); // 450.1f, 64.8907f, 450.8f, 64.8814f, 451.5f, 64.8775f
    path.cubic_to(
        (f32::from_bits(0x43e2199a), f32::from_bits(0x4281bf3f)),
        (f32::from_bits(0x43e27333), f32::from_bits(0x4281c026)),
        (f32::from_bits(0x43e2cccd), f32::from_bits(0x4281bd9a)),
    ); // 452.2f, 64.8735f, 452.9f, 64.8753f, 453.6f, 64.8703f
    path.cubic_to(
        (f32::from_bits(0x43e32666), f32::from_bits(0x4281bb0f)),
        (f32::from_bits(0x43e38000), f32::from_bits(0x4281b877)),
        (f32::from_bits(0x43e3d99a), f32::from_bits(0x4281b202)),
    ); // 454.3f, 64.8653f, 455, 64.8603f, 455.7f, 64.8477f
    path.cubic_to(
        (f32::from_bits(0x43e43333), f32::from_bits(0x4281ab8e)),
        (f32::from_bits(0x43e48ccd), f32::from_bits(0x4281a1fe)),
        (f32::from_bits(0x43e4e666), f32::from_bits(0x428196e0)),
    ); // 456.4f, 64.8351f, 457.1f, 64.8164f, 457.8f, 64.7947f
    path.cubic_to(
        (f32::from_bits(0x43e54000), f32::from_bits(0x42818bc3)),
        (f32::from_bits(0x43e5999a), f32::from_bits(0x42817cb2)),
        (f32::from_bits(0x43e5f333), f32::from_bits(0x42816f52)),
    ); // 458.5f, 64.773f, 459.2f, 64.7435f, 459.9f, 64.7174f
    path.cubic_to(
        (f32::from_bits(0x43e64ccd), f32::from_bits(0x428161f2)),
        (f32::from_bits(0x43e6a666), f32::from_bits(0x428151a5)),
        (f32::from_bits(0x43e70000), f32::from_bits(0x4281469f)),
    ); // 460.6f, 64.6913f, 461.3f, 64.6595f, 462, 64.6379f
    path.cubic_to(
        (f32::from_bits(0x43e7599a), f32::from_bits(0x42813b9a)),
        (f32::from_bits(0x43e7b333), f32::from_bits(0x428132d7)),
        (f32::from_bits(0x43e80ccd), f32::from_bits(0x42812d30)),
    ); // 462.7f, 64.6164f, 463.4f, 64.5993f, 464.1f, 64.5883f
    path.cubic_to(
        (f32::from_bits(0x43e86666), f32::from_bits(0x42812789)),
        (f32::from_bits(0x43e8c000), f32::from_bits(0x428125d6)),
        (f32::from_bits(0x43e9199a), f32::from_bits(0x428124b5)),
    ); // 464.8f, 64.5772f, 465.5f, 64.5739f, 466.2f, 64.5717f
    path.cubic_to(
        (f32::from_bits(0x43e97333), f32::from_bits(0x42812394)),
        (f32::from_bits(0x43e9cccd), f32::from_bits(0x4281258e)),
        (f32::from_bits(0x43ea2666), f32::from_bits(0x42812667)),
    ); // 466.9f, 64.5695f, 467.6f, 64.5733f, 468.3f, 64.575f
    path.cubic_to(
        (f32::from_bits(0x43ea8000), f32::from_bits(0x42812740)),
        (f32::from_bits(0x43ead99a), f32::from_bits(0x42812819)),
        (f32::from_bits(0x43eb3333), f32::from_bits(0x428129cb)),
    ); // 469, 64.5767f, 469.7f, 64.5783f, 470.4f, 64.5816f
    path.cubic_to(
        (f32::from_bits(0x43eb8ccd), f32::from_bits(0x42812b7e)),
        (f32::from_bits(0x43ebe666), f32::from_bits(0x42812734)),
        (f32::from_bits(0x43ec4000), f32::from_bits(0x42813094)),
    ); // 471.1f, 64.5849f, 471.8f, 64.5766f, 472.5f, 64.5949f
    path.cubic_to(
        (f32::from_bits(0x43ec999a), f32::from_bits(0x428139f3)),
        (f32::from_bits(0x43ecf333), f32::from_bits(0x42814838)),
        (f32::from_bits(0x43ed4ccd), f32::from_bits(0x42816208)),
    ); // 473.2f, 64.6132f, 473.9f, 64.6411f, 474.6f, 64.6915f
    path.cubic_to(
        (f32::from_bits(0x43eda666), f32::from_bits(0x42817bd8)),
        (f32::from_bits(0x43ee0000), f32::from_bits(0x4281a8c8)),
        (f32::from_bits(0x43ee599a), f32::from_bits(0x4281cb72)),
    ); // 475.3f, 64.7419f, 476, 64.8297f, 476.7f, 64.8974f
    path.cubic_to(
        (f32::from_bits(0x43eeb333), f32::from_bits(0x4281ee1c)),
        (f32::from_bits(0x43ef0ccd), f32::from_bits(0x428220ba)),
        (f32::from_bits(0x43ef6666), f32::from_bits(0x42823205)),
    ); // 477.4f, 64.9651f, 478.1f, 65.0639f, 478.8f, 65.0977f
    path.cubic_to(
        (f32::from_bits(0x43efc000), f32::from_bits(0x42824350)),
        (f32::from_bits(0x43f0199a), f32::from_bits(0x42823301)),
        (f32::from_bits(0x43f07333), f32::from_bits(0x42823333)),
    ); // 479.5f, 65.1315f, 480.2f, 65.0996f, 480.9f, 65.1f
    path.line_to((f32::from_bits(0x43f07333), f32::from_bits(0x42823333))); // 480.9f, 65.1f
    path.line_to((f32::from_bits(0x43c24000), f32::from_bits(0x42823333))); // 388.5f, 65.1f
    path.close();
    path.move_to((f32::from_bits(0x43fc0000), f32::from_bits(0x42823333))); // 504, 65.1f
    path.line_to((f32::from_bits(0x43fc0000), f32::from_bits(0x42823333))); // 504, 65.1f
    path.cubic_to(
        (f32::from_bits(0x43fc599a), f32::from_bits(0x42823333)),
        (f32::from_bits(0x43fcb333), f32::from_bits(0x42823772)),
        (f32::from_bits(0x43fd0ccd), f32::from_bits(0x42823333)),
    ); // 504.7f, 65.1f, 505.4f, 65.1083f, 506.1f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x43fd6666), f32::from_bits(0x42822ef4)),
        (f32::from_bits(0x43fdc000), f32::from_bits(0x428227e9)),
        (f32::from_bits(0x43fe199a), f32::from_bits(0x428219ba)),
    ); // 506.8f, 65.0917f, 507.5f, 65.0779f, 508.2f, 65.0502f
    path.cubic_to(
        (f32::from_bits(0x43fe7333), f32::from_bits(0x42820b8b)),
        (f32::from_bits(0x43fecccd), f32::from_bits(0x4281ebf3)),
        (f32::from_bits(0x43ff2666), f32::from_bits(0x4281de19)),
    ); // 508.9f, 65.0225f, 509.6f, 64.9608f, 510.3f, 64.9338f
    path.cubic_to(
        (f32::from_bits(0x43ff8000), f32::from_bits(0x4281d040)),
        (f32::from_bits(0x43ffd99a), f32::from_bits(0x4281cbde)),
        (f32::from_bits(0x4400199a), f32::from_bits(0x4281c6a2)),
    ); // 511, 64.9067f, 511.7f, 64.8982f, 512.4f, 64.888f
    path.cubic_to(
        (f32::from_bits(0x44004666), f32::from_bits(0x4281c167)),
        (f32::from_bits(0x44007333), f32::from_bits(0x4281bf82)),
        (f32::from_bits(0x4400a000), f32::from_bits(0x4281beb5)),
    ); // 513.1f, 64.8777f, 513.8f, 64.874f, 514.5f, 64.8725f
    path.cubic_to(
        (f32::from_bits(0x4400cccd), f32::from_bits(0x4281bde8)),
        (f32::from_bits(0x4400f99a), f32::from_bits(0x4281bfe4)),
        (f32::from_bits(0x44012666), f32::from_bits(0x4281c1d2)),
    ); // 515.2f, 64.8709f, 515.9f, 64.8748f, 516.6f, 64.8786f
    path.cubic_to(
        (f32::from_bits(0x44015333), f32::from_bits(0x4281c3c1)),
        (f32::from_bits(0x44018000), f32::from_bits(0x4281c822)),
        (f32::from_bits(0x4401accd), f32::from_bits(0x4281ca4d)),
    ); // 517.3f, 64.8823f, 518, 64.8909f, 518.7f, 64.8951f
    path.cubic_to(
        (f32::from_bits(0x4401d99a), f32::from_bits(0x4281cc78)),
        (f32::from_bits(0x44020666), f32::from_bits(0x4281cf4e)),
        (f32::from_bits(0x44023333), f32::from_bits(0x4281ced6)),
    ); // 519.4f, 64.8994f, 520.1f, 64.9049f, 520.8f, 64.904f
    path.cubic_to(
        (f32::from_bits(0x44026000), f32::from_bits(0x4281ce5f)),
        (f32::from_bits(0x44028ccd), f32::from_bits(0x4281cb81)),
        (f32::from_bits(0x4402b99a), f32::from_bits(0x4281c781)),
    ); // 521.5f, 64.9031f, 522.2f, 64.8975f, 522.9f, 64.8897f
    path.cubic_to(
        (f32::from_bits(0x4402e666), f32::from_bits(0x4281c380)),
        (f32::from_bits(0x44031333), f32::from_bits(0x4281babb)),
        (f32::from_bits(0x44034000), f32::from_bits(0x4281b6d2)),
    ); // 523.6f, 64.8818f, 524.3f, 64.8647f, 525, 64.8571f
    path.cubic_to(
        (f32::from_bits(0x44036ccd), f32::from_bits(0x4281b2e9)),
        (f32::from_bits(0x4403999a), f32::from_bits(0x4281b0a6)),
        (f32::from_bits(0x4403c666), f32::from_bits(0x4281b009)),
    ); // 525.7f, 64.8494f, 526.4f, 64.845f, 527.1f, 64.8438f
    path.cubic_to(
        (f32::from_bits(0x4403f333), f32::from_bits(0x4281af6d)),
        (f32::from_bits(0x44042000), f32::from_bits(0x42819d4b)),
        (f32::from_bits(0x44044ccd), f32::from_bits(0x4281b327)),
    ); // 527.8f, 64.8426f, 528.5f, 64.8072f, 529.2f, 64.8499f
    path.cubic_to(
        (f32::from_bits(0x4404799a), f32::from_bits(0x4281c903)),
        (f32::from_bits(0x4404a666), f32::from_bits(0x42821ddc)),
        (f32::from_bits(0x4404d333), f32::from_bits(0x42823333)),
    ); // 529.9f, 64.8926f, 530.6f, 65.0583f, 531.3f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x44050000), f32::from_bits(0x4282488b)),
        (f32::from_bits(0x44052ccd), f32::from_bits(0x42823333)),
        (f32::from_bits(0x4405599a), f32::from_bits(0x42823333)),
    ); // 532, 65.1417f, 532.7f, 65.1f, 533.4f, 65.1f
    path.line_to((f32::from_bits(0x4405599a), f32::from_bits(0x42823333))); // 533.4f, 65.1f
    path.line_to((f32::from_bits(0x43fc0000), f32::from_bits(0x42823333))); // 504, 65.1f
    path.close();
    path.detach()
}

// Port of: tests/PathOpsIssue3651.cpp#L199-L376 (chrome/m156)
fn path2() -> Path {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x431d8000), f32::from_bits(0x42823333))); // 157.5f, 65.1f
    path.line_to((f32::from_bits(0x431d8000), f32::from_bits(0x42823333))); // 157.5f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x431e3333), f32::from_bits(0x42823333)),
        (f32::from_bits(0x431ee666), f32::from_bits(0x42822fd9)),
        (f32::from_bits(0x431f999a), f32::from_bits(0x42823333)),
    ); // 158.2f, 65.1f, 158.9f, 65.0935f, 159.6f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x43204ccd), f32::from_bits(0x4282368d)),
        (f32::from_bits(0x43210000), f32::from_bits(0x42823e05)),
        (f32::from_bits(0x4321b333), f32::from_bits(0x4282474f)),
    ); // 160.3f, 65.1065f, 161, 65.1211f, 161.7f, 65.1393f
    path.cubic_to(
        (f32::from_bits(0x43226666), f32::from_bits(0x42825098)),
        (f32::from_bits(0x4323199a), f32::from_bits(0x42825f49)),
        (f32::from_bits(0x4323cccd), f32::from_bits(0x42826aeb)),
    ); // 162.4f, 65.1574f, 163.1f, 65.1861f, 163.8f, 65.2088f
    path.cubic_to(
        (f32::from_bits(0x43248000), f32::from_bits(0x4282768e)),
        (f32::from_bits(0x43253333), f32::from_bits(0x428281ff)),
        (f32::from_bits(0x4325e666), f32::from_bits(0x42828d1c)),
    ); // 164.5f, 65.2316f, 165.2f, 65.2539f, 165.9f, 65.2756f
    path.cubic_to(
        (f32::from_bits(0x4326999a), f32::from_bits(0x4282983a)),
        (f32::from_bits(0x43274ccd), f32::from_bits(0x4282a509)),
        (f32::from_bits(0x43280000), f32::from_bits(0x4282ad9b)),
    ); // 166.6f, 65.2973f, 167.3f, 65.3223f, 168, 65.3391f
    path.cubic_to(
        (f32::from_bits(0x4328b333), f32::from_bits(0x4282b62d)),
        (f32::from_bits(0x43296666), f32::from_bits(0x4282bff9)),
        (f32::from_bits(0x432a199a), f32::from_bits(0x4282c089)),
    ); // 168.7f, 65.3558f, 169.4f, 65.3749f, 170.1f, 65.376f
    path.cubic_to(
        (f32::from_bits(0x432acccd), f32::from_bits(0x4282c11a)),
        (f32::from_bits(0x432b8000), f32::from_bits(0x4282b797)),
        (f32::from_bits(0x432c3333), f32::from_bits(0x4282b100)),
    ); // 170.8f, 65.3772f, 171.5f, 65.3586f, 172.2f, 65.3457f
    path.cubic_to(
        (f32::from_bits(0x432ce666), f32::from_bits(0x4282aa68)),
        (f32::from_bits(0x432d999a), f32::from_bits(0x4282a054)),
        (f32::from_bits(0x432e4ccd), f32::from_bits(0x428298fb)),
    ); // 172.9f, 65.3328f, 173.6f, 65.3131f, 174.3f, 65.2988f
    path.cubic_to(
        (f32::from_bits(0x432f0000), f32::from_bits(0x428291a2)),
        (f32::from_bits(0x432fb333), f32::from_bits(0x428289f3)),
        (f32::from_bits(0x43306666), f32::from_bits(0x428284e8)),
    ); // 175, 65.2844f, 175.7f, 65.2694f, 176.4f, 65.2596f
    path.cubic_to(
        (f32::from_bits(0x4331199a), f32::from_bits(0x42827fde)),
        (f32::from_bits(0x4331cccd), f32::from_bits(0x42827cff)),
        (f32::from_bits(0x43328000), f32::from_bits(0x42827abc)),
    ); // 177.1f, 65.2497f, 177.8f, 65.2441f, 178.5f, 65.2397f
    path.cubic_to(
        (f32::from_bits(0x43333333), f32::from_bits(0x42827879)),
        (f32::from_bits(0x4333e666), f32::from_bits(0x428277a0)),
        (f32::from_bits(0x4334999a), f32::from_bits(0x42827757)),
    ); // 179.2f, 65.2353f, 179.9f, 65.2336f, 180.6f, 65.2331f
    path.cubic_to(
        (f32::from_bits(0x43354ccd), f32::from_bits(0x4282770f)),
        (f32::from_bits(0x43360000), f32::from_bits(0x428277ac)),
        (f32::from_bits(0x4336b333), f32::from_bits(0x4282790a)),
    ); // 181.3f, 65.2325f, 182, 65.2337f, 182.7f, 65.2364f
    path.cubic_to(
        (f32::from_bits(0x43376666), f32::from_bits(0x42827a68)),
        (f32::from_bits(0x4338199a), f32::from_bits(0x42827d9d)),
        (f32::from_bits(0x4338cccd), f32::from_bits(0x42827f8b)),
    ); // 183.4f, 65.2391f, 184.1f, 65.2453f, 184.8f, 65.2491f
    path.cubic_to(
        (f32::from_bits(0x43398000), f32::from_bits(0x4282817a)),
        (f32::from_bits(0x433a3333), f32::from_bits(0x428283c9)),
        (f32::from_bits(0x433ae666), f32::from_bits(0x428284a2)),
    ); // 185.5f, 65.2529f, 186.2f, 65.2574f, 186.9f, 65.259f
    path.cubic_to(
        (f32::from_bits(0x433b999a), f32::from_bits(0x4282857b)),
        (f32::from_bits(0x433c4ccd), f32::from_bits(0x428284de)),
        (f32::from_bits(0x433d0000), f32::from_bits(0x428284a2)),
    ); // 187.6f, 65.2607f, 188.3f, 65.2595f, 189, 65.259f
    path.cubic_to(
        (f32::from_bits(0x433db333), f32::from_bits(0x42828465)),
        (f32::from_bits(0x433e6666), f32::from_bits(0x42828251)),
        (f32::from_bits(0x433f199a), f32::from_bits(0x42828336)),
    ); // 189.7f, 65.2586f, 190.4f, 65.2545f, 191.1f, 65.2563f
    path.cubic_to(
        (f32::from_bits(0x433fcccd), f32::from_bits(0x4282841b)),
        (f32::from_bits(0x43408000), f32::from_bits(0x428286ef)),
        (f32::from_bits(0x43413333), f32::from_bits(0x428289ff)),
    ); // 191.8f, 65.258f, 192.5f, 65.2635f, 193.2f, 65.2695f
    path.cubic_to(
        (f32::from_bits(0x4341e666), f32::from_bits(0x42828d0f)),
        (f32::from_bits(0x4342999a), f32::from_bits(0x4282930c)),
        (f32::from_bits(0x43434ccd), f32::from_bits(0x42829597)),
    ); // 193.9f, 65.2755f, 194.6f, 65.2872f, 195.3f, 65.2922f
    path.cubic_to(
        (f32::from_bits(0x43440000), f32::from_bits(0x42829822)),
        (f32::from_bits(0x4344b333), f32::from_bits(0x428298fa)),
        (f32::from_bits(0x43456666), f32::from_bits(0x42829942)),
    ); // 196, 65.2971f, 196.7f, 65.2988f, 197.4f, 65.2993f
    path.cubic_to(
        (f32::from_bits(0x4346199a), f32::from_bits(0x4282998a)),
        (f32::from_bits(0x4346cccd), f32::from_bits(0x42829749)),
        (f32::from_bits(0x43478000), f32::from_bits(0x42829749)),
    ); // 198.1f, 65.2999f, 198.8f, 65.2955f, 199.5f, 65.2955f
    path.cubic_to(
        (f32::from_bits(0x43483333), f32::from_bits(0x42829749)),
        (f32::from_bits(0x4348e666), f32::from_bits(0x428297d8)),
        (f32::from_bits(0x4349999a), f32::from_bits(0x42829942)),
    ); // 200.2f, 65.2955f, 200.9f, 65.2966f, 201.6f, 65.2993f
    path.cubic_to(
        (f32::from_bits(0x434a4ccd), f32::from_bits(0x42829aac)),
        (f32::from_bits(0x434b0000), f32::from_bits(0x42829e12)),
        (f32::from_bits(0x434bb333), f32::from_bits(0x42829fc4)),
    ); // 202.3f, 65.3021f, 203, 65.3087f, 203.7f, 65.312f
    path.cubic_to(
        (f32::from_bits(0x434c6666), f32::from_bits(0x4282a176)),
        (f32::from_bits(0x434d199a), f32::from_bits(0x4282a1f9)),
        (f32::from_bits(0x434dcccd), f32::from_bits(0x4282a36f)),
    ); // 204.4f, 65.3154f, 205.1f, 65.3164f, 205.8f, 65.3192f
    path.cubic_to(
        (f32::from_bits(0x434e8000), f32::from_bits(0x4282a4e4)),
        (f32::from_bits(0x434f3333), f32::from_bits(0x4282a71b)),
        (f32::from_bits(0x434fe666), f32::from_bits(0x4282a885)),
    ); // 206.5f, 65.3221f, 207.2f, 65.3264f, 207.9f, 65.3291f
    path.cubic_to(
        (f32::from_bits(0x4350999a), f32::from_bits(0x4282a9ef)),
        (f32::from_bits(0x43514ccd), f32::from_bits(0x4282aad4)),
        (f32::from_bits(0x43520000), f32::from_bits(0x4282abe9)),
    ); // 208.6f, 65.3319f, 209.3f, 65.3336f, 210, 65.3358f
    path.cubic_to(
        (f32::from_bits(0x4352b333), f32::from_bits(0x4282acff)),
        (f32::from_bits(0x43536666), f32::from_bits(0x4282ad0c)),
        (f32::from_bits(0x4354199a), f32::from_bits(0x4282af07)),
    ); // 210.7f, 65.3379f, 211.4f, 65.338f, 212.1f, 65.3419f
    path.cubic_to(
        (f32::from_bits(0x4354cccd), f32::from_bits(0x4282b101)),
        (f32::from_bits(0x43558000), f32::from_bits(0x4282b5c2)),
        (f32::from_bits(0x43563333), f32::from_bits(0x4282b7c8)),
    ); // 212.8f, 65.3457f, 213.5f, 65.355f, 214.2f, 65.3589f
    path.cubic_to(
        (f32::from_bits(0x4356e666), f32::from_bits(0x4282b9ce)),
        (f32::from_bits(0x4357999a), f32::from_bits(0x4282b9c3)),
        (f32::from_bits(0x43584ccd), f32::from_bits(0x4282bb2c)),
    ); // 214.9f, 65.3629f, 215.6f, 65.3628f, 216.3f, 65.3656f
    path.cubic_to(
        (f32::from_bits(0x43590000), f32::from_bits(0x4282bc96)),
        (f32::from_bits(0x4359b333), f32::from_bits(0x4282be3c)),
        (f32::from_bits(0x435a6666), f32::from_bits(0x4282c043)),
    ); // 217, 65.3683f, 217.7f, 65.3716f, 218.4f, 65.3755f
    path.cubic_to(
        (f32::from_bits(0x435b199a), f32::from_bits(0x4282c249)),
        (f32::from_bits(0x435bcccd), f32::from_bits(0x4282c50f)),
        (f32::from_bits(0x435c8000), f32::from_bits(0x4282c752)),
    ); // 219.1f, 65.3795f, 219.8f, 65.3849f, 220.5f, 65.3893f
    path.cubic_to(
        (f32::from_bits(0x435d3333), f32::from_bits(0x4282c995)),
        (f32::from_bits(0x435de666), f32::from_bits(0x4282cc2d)),
        (f32::from_bits(0x435e999a), f32::from_bits(0x4282cdd4)),
    ); // 221.2f, 65.3937f, 221.9f, 65.3988f, 222.6f, 65.402f
    path.cubic_to(
        (f32::from_bits(0x435f4ccd), f32::from_bits(0x4282cf7a)),
        (f32::from_bits(0x43600000), f32::from_bits(0x4282d211)),
        (f32::from_bits(0x4360b333), f32::from_bits(0x4282d138)),
    ); // 223.3f, 65.4052f, 224, 65.4103f, 224.7f, 65.4086f
    path.cubic_to(
        (f32::from_bits(0x43616666), f32::from_bits(0x4282d05f)),
        (f32::from_bits(0x4362199a), f32::from_bits(0x4282cd7f)),
        (f32::from_bits(0x4362cccd), f32::from_bits(0x4282c8bd)),
    ); // 225.4f, 65.407f, 226.1f, 65.4014f, 226.8f, 65.3921f
    path.cubic_to(
        (f32::from_bits(0x43638000), f32::from_bits(0x4282c3fb)),
        (f32::from_bits(0x43643333), f32::from_bits(0x4282b8dc)),
        (f32::from_bits(0x4364e666), f32::from_bits(0x4282b4aa)),
    ); // 227.5f, 65.3828f, 228.2f, 65.3611f, 228.9f, 65.3529f
    path.cubic_to(
        (f32::from_bits(0x4365999a), f32::from_bits(0x4282b079)),
        (f32::from_bits(0x43664ccd), f32::from_bits(0x4282af57)),
        (f32::from_bits(0x43670000), f32::from_bits(0x4282af94)),
    ); // 229.6f, 65.3447f, 230.3f, 65.3425f, 231, 65.3429f
    path.cubic_to(
        (f32::from_bits(0x4367b333), f32::from_bits(0x4282afd1)),
        (f32::from_bits(0x43686666), f32::from_bits(0x4282b38b)),
        (f32::from_bits(0x4369199a), f32::from_bits(0x4282b616)),
    ); // 231.7f, 65.3434f, 232.4f, 65.3507f, 233.1f, 65.3556f
    path.cubic_to(
        (f32::from_bits(0x4369cccd), f32::from_bits(0x4282b8a1)),
        (f32::from_bits(0x436a8000), f32::from_bits(0x4282bc7d)),
        (f32::from_bits(0x436b3333), f32::from_bits(0x4282bed7)),
    ); // 233.8f, 65.3606f, 234.5f, 65.3681f, 235.2f, 65.3727f
    path.cubic_to(
        (f32::from_bits(0x436be666), f32::from_bits(0x4282c131)),
        (f32::from_bits(0x436c999a), f32::from_bits(0x4282c10c)),
        (f32::from_bits(0x436d4ccd), f32::from_bits(0x4282c434)),
    ); // 235.9f, 65.3773f, 236.6f, 65.377f, 237.3f, 65.3832f
    path.cubic_to(
        (f32::from_bits(0x436e0000), f32::from_bits(0x4282c75c)),
        (f32::from_bits(0x436eb333), f32::from_bits(0x4282cb8d)),
        (f32::from_bits(0x436f6666), f32::from_bits(0x4282d1c5)),
    ); // 238, 65.3894f, 238.7f, 65.3976f, 239.4f, 65.4097f
    path.cubic_to(
        (f32::from_bits(0x4370199a), f32::from_bits(0x4282d7fd)),
        (f32::from_bits(0x4370cccd), f32::from_bits(0x4282e22a)),
        (f32::from_bits(0x43718000), f32::from_bits(0x4282e983)),
    ); // 240.1f, 65.4219f, 240.8f, 65.4417f, 241.5f, 65.4561f
    path.cubic_to(
        (f32::from_bits(0x43723333), f32::from_bits(0x4282f0dc)),
        (f32::from_bits(0x4372e666), f32::from_bits(0x4282fa30)),
        (f32::from_bits(0x4373999a), f32::from_bits(0x4282fddc)),
    ); // 242.2f, 65.4704f, 242.9f, 65.4886f, 243.6f, 65.4958f
    path.cubic_to(
        (f32::from_bits(0x43744ccd), f32::from_bits(0x42830189)),
        (f32::from_bits(0x43750000), f32::from_bits(0x4282fec1)),
        (f32::from_bits(0x4375b333), f32::from_bits(0x4282ff8e)),
    ); // 244.3f, 65.503f, 245, 65.4976f, 245.7f, 65.4991f
    path.cubic_to(
        (f32::from_bits(0x43766666), f32::from_bits(0x4283005c)),
        (f32::from_bits(0x4377199a), f32::from_bits(0x42830015)),
        (f32::from_bits(0x4377cccd), f32::from_bits(0x428302ac)),
    ); // 246.4f, 65.5007f, 247.1f, 65.5002f, 247.8f, 65.5052f
    path.cubic_to(
        (f32::from_bits(0x43788000), f32::from_bits(0x42830543)),
        (f32::from_bits(0x43793333), f32::from_bits(0x42830b0b)),
        (f32::from_bits(0x4379e666), f32::from_bits(0x42830f18)),
    ); // 248.5f, 65.5103f, 249.2f, 65.5216f, 249.9f, 65.5295f
    path.cubic_to(
        (f32::from_bits(0x437a999a), f32::from_bits(0x42831324)),
        (f32::from_bits(0x437b4ccd), f32::from_bits(0x428316b9)),
        (f32::from_bits(0x437c0000), f32::from_bits(0x42831af7)),
    ); // 250.6f, 65.5374f, 251.3f, 65.5444f, 252, 65.5527f
    path.cubic_to(
        (f32::from_bits(0x437cb333), f32::from_bits(0x42831f34)),
        (f32::from_bits(0x437d6666), f32::from_bits(0x428327af)),
        (f32::from_bits(0x437e199a), f32::from_bits(0x42832888)),
    ); // 252.7f, 65.5609f, 253.4f, 65.5775f, 254.1f, 65.5792f
    path.cubic_to(
        (f32::from_bits(0x437ecccd), f32::from_bits(0x42832961)),
        (f32::from_bits(0x437f8000), f32::from_bits(0x42832e8f)),
        (f32::from_bits(0x4380199a), f32::from_bits(0x4283200d)),
    ); // 254.8f, 65.5808f, 255.5f, 65.5909f, 256.2f, 65.5626f
    path.cubic_to(
        (f32::from_bits(0x43807333), f32::from_bits(0x4283118c)),
        (f32::from_bits(0x4380cccd), f32::from_bits(0x4282f101)),
        (f32::from_bits(0x43812666), f32::from_bits(0x4282d17e)),
    ); // 256.9f, 65.5343f, 257.6f, 65.4707f, 258.3f, 65.4092f
    path.cubic_to(
        (f32::from_bits(0x43818000), f32::from_bits(0x4282b1fc)),
        (f32::from_bits(0x4381d99a), f32::from_bits(0x42827d60)),
        (f32::from_bits(0x43823333), f32::from_bits(0x428262fe)),
    ); // 259, 65.3476f, 259.7f, 65.2449f, 260.4f, 65.1933f
    path.cubic_to(
        (f32::from_bits(0x43828ccd), f32::from_bits(0x4282489c)),
        (f32::from_bits(0x4382e666), f32::from_bits(0x42823b2a)),
        (f32::from_bits(0x43834000), f32::from_bits(0x42823333)),
    ); // 261.1f, 65.1418f, 261.8f, 65.1156f, 262.5f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x4383999a), f32::from_bits(0x42822b3c)),
        (f32::from_bits(0x4383f333), f32::from_bits(0x42823333)),
        (f32::from_bits(0x43844ccd), f32::from_bits(0x42823333)),
    ); // 263.2f, 65.0844f, 263.9f, 65.1f, 264.6f, 65.1f
    path.line_to((f32::from_bits(0x43844ccd), f32::from_bits(0x42823333))); // 264.6f, 65.1f
    path.line_to((f32::from_bits(0x431d8000), f32::from_bits(0x42823333))); // 157.5f, 65.1f
    path.close();
    path.move_to((f32::from_bits(0x438dc000), f32::from_bits(0x42823333))); // 283.5f, 65.1f
    path.line_to((f32::from_bits(0x438dc000), f32::from_bits(0x42823333))); // 283.5f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x438e199a), f32::from_bits(0x4282356b)),
        (f32::from_bits(0x438e7333), f32::from_bits(0x42823d2c)),
        (f32::from_bits(0x438ecccd), f32::from_bits(0x42824086)),
    ); // 284.2f, 65.1043f, 284.9f, 65.1195f, 285.6f, 65.126f
    path.cubic_to(
        (f32::from_bits(0x438f2666), f32::from_bits(0x428243e0)),
        (f32::from_bits(0x438f8000), f32::from_bits(0x42824988)),
        (f32::from_bits(0x438fd99a), f32::from_bits(0x4282474f)),
    ); // 286.3f, 65.1326f, 287, 65.1436f, 287.7f, 65.1393f
    path.cubic_to(
        (f32::from_bits(0x43903333), f32::from_bits(0x42824516)),
        (f32::from_bits(0x43908ccd), f32::from_bits(0x4282368d)),
        (f32::from_bits(0x4390e666), f32::from_bits(0x42823333)),
    ); // 288.4f, 65.1349f, 289.1f, 65.1065f, 289.8f, 65.1f
    path.line_to((f32::from_bits(0x4390e666), f32::from_bits(0x42823333))); // 289.8f, 65.1f
    path.line_to((f32::from_bits(0x438dc000), f32::from_bits(0x42823333))); // 283.5f, 65.1f
    path.close();
    path.move_to((f32::from_bits(0x43994ccd), f32::from_bits(0x42823333))); // 306.6f, 65.1f
    path.line_to((f32::from_bits(0x43994ccd), f32::from_bits(0x42823333))); // 306.6f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x4399a666), f32::from_bits(0x42823334)),
        (f32::from_bits(0x439a0000), f32::from_bits(0x42822e24)),
        (f32::from_bits(0x439a599a), f32::from_bits(0x4282333c)),
    ); // 307.3f, 65.1f, 308, 65.0901f, 308.7f, 65.1001f
    path.cubic_to(
        (f32::from_bits(0x439ab333), f32::from_bits(0x42823854)),
        (f32::from_bits(0x439b0ccd), f32::from_bits(0x428247d2)),
        (f32::from_bits(0x439b6666), f32::from_bits(0x428251c2)),
    ); // 309.4f, 65.11f, 310.1f, 65.1403f, 310.8f, 65.1597f
    path.cubic_to(
        (f32::from_bits(0x439bc000), f32::from_bits(0x42825bb2)),
        (f32::from_bits(0x439c199a), f32::from_bits(0x428264e1)),
        (f32::from_bits(0x439c7333), f32::from_bits(0x42826edd)),
    ); // 311.5f, 65.1791f, 312.2f, 65.197f, 312.9f, 65.2165f
    path.cubic_to(
        (f32::from_bits(0x439ccccd), f32::from_bits(0x428278d9)),
        (f32::from_bits(0x439d2666), f32::from_bits(0x428282d5)),
        (f32::from_bits(0x439d8000), f32::from_bits(0x42828daa)),
    ); // 313.6f, 65.236f, 314.3f, 65.2555f, 315, 65.2767f
    path.cubic_to(
        (f32::from_bits(0x439dd99a), f32::from_bits(0x4282987f)),
        (f32::from_bits(0x439e3333), f32::from_bits(0x4282a5a2)),
        (f32::from_bits(0x439e8ccd), f32::from_bits(0x4282afdb)),
    ); // 315.7f, 65.2978f, 316.4f, 65.3235f, 317.1f, 65.3435f
    path.cubic_to(
        (f32::from_bits(0x439ee666), f32::from_bits(0x4282ba13)),
        (f32::from_bits(0x439f4000), f32::from_bits(0x4282c3ec)),
        (f32::from_bits(0x439f999a), f32::from_bits(0x4282cafd)),
    ); // 317.8f, 65.3634f, 318.5f, 65.3827f, 319.2f, 65.3965f
    path.cubic_to(
        (f32::from_bits(0x439ff333), f32::from_bits(0x4282d20d)),
        (f32::from_bits(0x43a04ccd), f32::from_bits(0x4282d6db)),
        (f32::from_bits(0x43a0a666), f32::from_bits(0x4282da40)),
    ); // 319.9f, 65.4103f, 320.6f, 65.4196f, 321.3f, 65.4263f
    path.cubic_to(
        (f32::from_bits(0x43a10000), f32::from_bits(0x4282dda4)),
        (f32::from_bits(0x43a1599a), f32::from_bits(0x4282ded1)),
        (f32::from_bits(0x43a1b333), f32::from_bits(0x4282df56)),
    ); // 322, 65.4329f, 322.7f, 65.4352f, 323.4f, 65.4362f
    path.cubic_to(
        (f32::from_bits(0x43a20ccd), f32::from_bits(0x4282dfdb)),
        (f32::from_bits(0x43a26666), f32::from_bits(0x4282de42)),
        (f32::from_bits(0x43a2c000), f32::from_bits(0x4282dd5d)),
    ); // 324.1f, 65.4372f, 324.8f, 65.4341f, 325.5f, 65.4324f
    path.cubic_to(
        (f32::from_bits(0x43a3199a), f32::from_bits(0x4282dc78)),
        (f32::from_bits(0x43a37333), f32::from_bits(0x4282d884)),
        (f32::from_bits(0x43a3cccd), f32::from_bits(0x4282d9f9)),
    ); // 326.2f, 65.4306f, 326.9f, 65.4229f, 327.6f, 65.4257f
    path.cubic_to(
        (f32::from_bits(0x43a42666), f32::from_bits(0x4282db6f)),
        (f32::from_bits(0x43a48000), f32::from_bits(0x4282e0a8)),
        (f32::from_bits(0x43a4d99a), f32::from_bits(0x4282e61e)),
    ); // 328.3f, 65.4286f, 329, 65.4388f, 329.7f, 65.4494f
    path.cubic_to(
        (f32::from_bits(0x43a53333), f32::from_bits(0x4282eb95)),
        (f32::from_bits(0x43a58ccd), f32::from_bits(0x4282f833)),
        (f32::from_bits(0x43a5e666), f32::from_bits(0x4282fabf)),
    ); // 330.4f, 65.4601f, 331.1f, 65.4848f, 331.8f, 65.4897f
    path.cubic_to(
        (f32::from_bits(0x43a64000), f32::from_bits(0x4282fd4a)),
        (f32::from_bits(0x43a6999a), f32::from_bits(0x4282fb20)),
        (f32::from_bits(0x43a6f333), f32::from_bits(0x4282f562)),
    ); // 332.5f, 65.4947f, 333.2f, 65.4905f, 333.9f, 65.4793f
    path.cubic_to(
        (f32::from_bits(0x43a74ccd), f32::from_bits(0x4282efa3)),
        (f32::from_bits(0x43a7a666), f32::from_bits(0x4282e6bd)),
        (f32::from_bits(0x43a80000), f32::from_bits(0x4282d847)),
    ); // 334.6f, 65.468f, 335.3f, 65.4507f, 336, 65.4224f
    path.cubic_to(
        (f32::from_bits(0x43a8599a), f32::from_bits(0x4282c9d1)),
        (f32::from_bits(0x43a8b333), f32::from_bits(0x4282b47a)),
        (f32::from_bits(0x43a90ccd), f32::from_bits(0x42829e9f)),
    ); // 336.7f, 65.3942f, 337.4f, 65.3525f, 338.1f, 65.3098f
    path.cubic_to(
        (f32::from_bits(0x43a96666), f32::from_bits(0x428288c4)),
        (f32::from_bits(0x43a9c000), f32::from_bits(0x42825c97)),
        (f32::from_bits(0x43aa199a), f32::from_bits(0x42825526)),
    ); // 338.8f, 65.2671f, 339.5f, 65.1808f, 340.2f, 65.1663f
    path.cubic_to(
        (f32::from_bits(0x43aa7333), f32::from_bits(0x42824db6)),
        (f32::from_bits(0x43aacccd), f32::from_bits(0x42825867)),
        (f32::from_bits(0x43ab2666), f32::from_bits(0x428271fb)),
    ); // 340.9f, 65.1518f, 341.6f, 65.1727f, 342.3f, 65.2226f
    path.cubic_to(
        (f32::from_bits(0x43ab8000), f32::from_bits(0x42828b8e)),
        (f32::from_bits(0x43abd99a), f32::from_bits(0x4282cd10)),
        (f32::from_bits(0x43ac3333), f32::from_bits(0x4282ee99)),
    ); // 343, 65.2726f, 343.7f, 65.4005f, 344.4f, 65.466f
    path.cubic_to(
        (f32::from_bits(0x43ac8ccd), f32::from_bits(0x42831022)),
        (f32::from_bits(0x43ace666), f32::from_bits(0x42832d56)),
        (f32::from_bits(0x43ad4000), f32::from_bits(0x42833b2f)),
    ); // 345.1f, 65.5315f, 345.8f, 65.5885f, 346.5f, 65.6156f
    path.cubic_to(
        (f32::from_bits(0x43ad999a), f32::from_bits(0x42834908)),
        (f32::from_bits(0x43adf333), f32::from_bits(0x428342d2)),
        (f32::from_bits(0x43ae4ccd), f32::from_bits(0x428341b1)),
    ); // 347.2f, 65.6426f, 347.9f, 65.6305f, 348.6f, 65.6283f
    path.cubic_to(
        (f32::from_bits(0x43aea666), f32::from_bits(0x42834090)),
        (f32::from_bits(0x43af0000), f32::from_bits(0x42833a4a)),
        (f32::from_bits(0x43af599a), f32::from_bits(0x42833467)),
    ); // 349.3f, 65.6261f, 350, 65.6138f, 350.7f, 65.6023f
    path.cubic_to(
        (f32::from_bits(0x43afb333), f32::from_bits(0x42832e83)),
        (f32::from_bits(0x43b00ccd), f32::from_bits(0x428324e7)),
        (f32::from_bits(0x43b06666), f32::from_bits(0x42831e5b)),
    ); // 351.4f, 65.5908f, 352.1f, 65.5721f, 352.8f, 65.5593f
    path.cubic_to(
        (f32::from_bits(0x43b0c000), f32::from_bits(0x428317cf)),
        (f32::from_bits(0x43b1199a), f32::from_bits(0x4283131a)),
        (f32::from_bits(0x43b17333), f32::from_bits(0x42830d1f)),
    ); // 353.5f, 65.5465f, 354.2f, 65.5373f, 354.9f, 65.5256f
    path.cubic_to(
        (f32::from_bits(0x43b1cccd), f32::from_bits(0x42830724)),
        (f32::from_bits(0x43b22666), f32::from_bits(0x42830067)),
        (f32::from_bits(0x43b28000), f32::from_bits(0x4282fa78)),
    ); // 355.6f, 65.5139f, 356.3f, 65.5008f, 357, 65.4892f
    path.cubic_to(
        (f32::from_bits(0x43b2d99a), f32::from_bits(0x4282f488)),
        (f32::from_bits(0x43b33333), f32::from_bits(0x4282eb71)),
        (f32::from_bits(0x43b38ccd), f32::from_bits(0x4282e983)),
    ); // 357.7f, 65.4776f, 358.4f, 65.4598f, 359.1f, 65.4561f
    path.cubic_to(
        (f32::from_bits(0x43b3e666), f32::from_bits(0x4282e794)),
        (f32::from_bits(0x43b44000), f32::from_bits(0x4282ea97)),
        (f32::from_bits(0x43b4999a), f32::from_bits(0x4282eee0)),
    ); // 359.8f, 65.4523f, 360.5f, 65.4582f, 361.2f, 65.4666f
    path.cubic_to(
        (f32::from_bits(0x43b4f333), f32::from_bits(0x4282f329)),
        (f32::from_bits(0x43b54ccd), f32::from_bits(0x4282febf)),
        (f32::from_bits(0x43b5a666), f32::from_bits(0x42830339)),
    ); // 361.9f, 65.4749f, 362.6f, 65.4976f, 363.3f, 65.5063f
    path.cubic_to(
        (f32::from_bits(0x43b60000), f32::from_bits(0x428307b3)),
        (f32::from_bits(0x43b6599a), f32::from_bits(0x42830ae8)),
        (f32::from_bits(0x43b6b333), f32::from_bits(0x428309bb)),
    ); // 364, 65.515f, 364.7f, 65.5213f, 365.4f, 65.519f
    path.cubic_to(
        (f32::from_bits(0x43b70ccd), f32::from_bits(0x4283088e)),
        (f32::from_bits(0x43b76666), f32::from_bits(0x42830219)),
        (f32::from_bits(0x43b7c000), f32::from_bits(0x4282fc2a)),
    ); // 366.1f, 65.5167f, 366.8f, 65.5041f, 367.5f, 65.4925f
    path.cubic_to(
        (f32::from_bits(0x43b8199a), f32::from_bits(0x4282f63b)),
        (f32::from_bits(0x43b87333), f32::from_bits(0x4282ec93)),
        (f32::from_bits(0x43b8cccd), f32::from_bits(0x4282e61e)),
    ); // 368.2f, 65.4809f, 368.9f, 65.4621f, 369.6f, 65.4494f
    path.cubic_to(
        (f32::from_bits(0x43b92666), f32::from_bits(0x4282dfaa)),
        (f32::from_bits(0x43b98000), f32::from_bits(0x4282d91c)),
        (f32::from_bits(0x43b9d99a), f32::from_bits(0x4282d570)),
    ); // 370.3f, 65.4368f, 371, 65.424f, 371.7f, 65.4169f
    path.cubic_to(
        (f32::from_bits(0x43ba3333), f32::from_bits(0x4282d1c3)),
        (f32::from_bits(0x43ba8ccd), f32::from_bits(0x4282d2b6)),
        (f32::from_bits(0x43bae666), f32::from_bits(0x4282d013)),
    ); // 372.4f, 65.4097f, 373.1f, 65.4115f, 373.8f, 65.4064f
    path.cubic_to(
        (f32::from_bits(0x43bb4000), f32::from_bits(0x4282cd70)),
        (f32::from_bits(0x43bb999a), f32::from_bits(0x4282ce26)),
        (f32::from_bits(0x43bbf333), f32::from_bits(0x4282c5a0)),
    ); // 374.5f, 65.4012f, 375.2f, 65.4026f, 375.9f, 65.386f
    path.cubic_to(
        (f32::from_bits(0x43bc4ccd), f32::from_bits(0x4282bd19)),
        (f32::from_bits(0x43bca666), f32::from_bits(0x4282aa59)),
        (f32::from_bits(0x43bd0000), f32::from_bits(0x42829ced)),
    ); // 376.6f, 65.3693f, 377.3f, 65.3327f, 378, 65.3065f
    path.cubic_to(
        (f32::from_bits(0x43bd599a), f32::from_bits(0x42828f81)),
        (f32::from_bits(0x43bdb333), f32::from_bits(0x42827f68)),
        (f32::from_bits(0x43be0ccd), f32::from_bits(0x42827518)),
    ); // 378.7f, 65.2803f, 379.4f, 65.2488f, 380.1f, 65.2287f
    path.cubic_to(
        (f32::from_bits(0x43be6666), f32::from_bits(0x42826ac8)),
        (f32::from_bits(0x43bec000), f32::from_bits(0x428268f1)),
        (f32::from_bits(0x43bf199a), f32::from_bits(0x42825f0d)),
    ); // 380.8f, 65.2086f, 381.5f, 65.205f, 382.2f, 65.1856f
    path.cubic_to(
        (f32::from_bits(0x43bf7333), f32::from_bits(0x42825528)),
        (f32::from_bits(0x43bfcccd), f32::from_bits(0x4282410d)),
        (f32::from_bits(0x43c02666), f32::from_bits(0x428239be)),
    ); // 382.9f, 65.1663f, 383.6f, 65.1271f, 384.3f, 65.1128f
    path.line_to((f32::from_bits(0x43c02666), f32::from_bits(0x42823333))); // 384.3f, 65.1f
    path.line_to((f32::from_bits(0x43994ccd), f32::from_bits(0x42823333))); // 306.6f, 65.1f
    path.close();
    path.move_to((f32::from_bits(0x43c24000), f32::from_bits(0x42823333))); // 388.5f, 65.1f
    path.line_to((f32::from_bits(0x43c24000), f32::from_bits(0x42823333))); // 388.5f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x43c2999a), f32::from_bits(0x42823333)),
        (f32::from_bits(0x43c2f333), f32::from_bits(0x42822c75)),
        (f32::from_bits(0x43c34ccd), f32::from_bits(0x42823333)),
    ); // 389.2f, 65.1f, 389.9f, 65.0868f, 390.6f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x43c3a666), f32::from_bits(0x428239f1)),
        (f32::from_bits(0x43c40000), f32::from_bits(0x428243dd)),
        (f32::from_bits(0x43c4599a), f32::from_bits(0x42825ba8)),
    ); // 391.3f, 65.1132f, 392, 65.1325f, 392.7f, 65.179f
    path.cubic_to(
        (f32::from_bits(0x43c4b333), f32::from_bits(0x42827373)),
        (f32::from_bits(0x43c50ccd), f32::from_bits(0x4282a819)),
        (f32::from_bits(0x43c56666), f32::from_bits(0x4282c1f5)),
    ); // 393.4f, 65.2255f, 394.1f, 65.3283f, 394.8f, 65.3788f
    path.cubic_to(
        (f32::from_bits(0x43c5c000), f32::from_bits(0x4282dbd0)),
        (f32::from_bits(0x43c6199a), f32::from_bits(0x4282ee83)),
        (f32::from_bits(0x43c67333), f32::from_bits(0x4282f6cd)),
    ); // 395.5f, 65.4293f, 396.2f, 65.4658f, 396.9f, 65.482f
    path.cubic_to(
        (f32::from_bits(0x43c6cccd), f32::from_bits(0x4282ff17)),
        (f32::from_bits(0x43c72666), f32::from_bits(0x4282f4d1)),
        (f32::from_bits(0x43c78000), f32::from_bits(0x4282f3af)),
    ); // 397.6f, 65.4982f, 398.3f, 65.4782f, 399, 65.4759f
    path.cubic_to(
        (f32::from_bits(0x43c7d99a), f32::from_bits(0x4282f28e)),
        (f32::from_bits(0x43c83333), f32::from_bits(0x4282f13e)),
        (f32::from_bits(0x43c88ccd), f32::from_bits(0x4282f005)),
    ); // 399.7f, 65.4737f, 400.4f, 65.4712f, 401.1f, 65.4688f
    path.cubic_to(
        (f32::from_bits(0x43c8e666), f32::from_bits(0x4282eecc)),
        (f32::from_bits(0x43c94000), f32::from_bits(0x4282eed9)),
        (f32::from_bits(0x43c9999a), f32::from_bits(0x4282ec5a)),
    ); // 401.8f, 65.4664f, 402.5f, 65.4665f, 403.2f, 65.4616f
    path.cubic_to(
        (f32::from_bits(0x43c9f333), f32::from_bits(0x4282e9da)),
        (f32::from_bits(0x43ca4ccd), f32::from_bits(0x4282e71d)),
        (f32::from_bits(0x43caa666), f32::from_bits(0x4282e108)),
    ); // 403.9f, 65.4567f, 404.6f, 65.4514f, 405.3f, 65.4395f
    path.cubic_to(
        (f32::from_bits(0x43cb0000), f32::from_bits(0x4282daf4)),
        (f32::from_bits(0x43cb599a), f32::from_bits(0x4282ce0b)),
        (f32::from_bits(0x43cbb333), f32::from_bits(0x4282c7df)),
    ); // 406, 65.4276f, 406.7f, 65.4024f, 407.4f, 65.3904f
    path.cubic_to(
        (f32::from_bits(0x43cc0ccd), f32::from_bits(0x4282c1b3)),
        (f32::from_bits(0x43cc6666), f32::from_bits(0x4282bf7c)),
        (f32::from_bits(0x43ccc000), f32::from_bits(0x4282bc00)),
    ); // 408.1f, 65.3783f, 408.8f, 65.374f, 409.5f, 65.3672f
    path.cubic_to(
        (f32::from_bits(0x43cd199a), f32::from_bits(0x4282b884)),
        (f32::from_bits(0x43cd7333), f32::from_bits(0x4282bb96)),
        (f32::from_bits(0x43cdcccd), f32::from_bits(0x4282b2f8)),
    ); // 410.2f, 65.3604f, 410.9f, 65.3664f, 411.6f, 65.3495f
    path.cubic_to(
        (f32::from_bits(0x43ce2666), f32::from_bits(0x4282aa5a)),
        (f32::from_bits(0x43ce8000), f32::from_bits(0x428295f5)),
        (f32::from_bits(0x43ced99a), f32::from_bits(0x4282884d)),
    ); // 412.3f, 65.3327f, 413, 65.2929f, 413.7f, 65.2662f
    path.cubic_to(
        (f32::from_bits(0x43cf3333), f32::from_bits(0x42827aa4)),
        (f32::from_bits(0x43cf8ccd), f32::from_bits(0x42826b01)),
        (f32::from_bits(0x43cfe666), f32::from_bits(0x42826105)),
    ); // 414.4f, 65.2395f, 415.1f, 65.209f, 415.8f, 65.1895f
    path.cubic_to(
        (f32::from_bits(0x43d04000), f32::from_bits(0x42825709)),
        (f32::from_bits(0x43d0999a), f32::from_bits(0x42824ec0)),
        (f32::from_bits(0x43d0f333), f32::from_bits(0x42824c65)),
    ); // 416.5f, 65.17f, 417.2f, 65.1538f, 417.9f, 65.1492f
    path.cubic_to(
        (f32::from_bits(0x43d14ccd), f32::from_bits(0x42824a0b)),
        (f32::from_bits(0x43d1a666), f32::from_bits(0x42824c1f)),
        (f32::from_bits(0x43d20000), f32::from_bits(0x428252e7)),
    ); // 418.6f, 65.1446f, 419.3f, 65.1487f, 420, 65.1619f
    path.cubic_to(
        (f32::from_bits(0x43d2599a), f32::from_bits(0x428259b0)),
        (f32::from_bits(0x43d2b333), f32::from_bits(0x428269b3)),
        (f32::from_bits(0x43d30ccd), f32::from_bits(0x42827518)),
    ); // 420.7f, 65.1752f, 421.4f, 65.2064f, 422.1f, 65.2287f
    path.cubic_to(
        (f32::from_bits(0x43d36666), f32::from_bits(0x4282807e)),
        (f32::from_bits(0x43d3c000), f32::from_bits(0x42829021)),
        (f32::from_bits(0x43d4199a), f32::from_bits(0x42829749)),
    ); // 422.8f, 65.251f, 423.5f, 65.2815f, 424.2f, 65.2955f
    path.cubic_to(
        (f32::from_bits(0x43d47333), f32::from_bits(0x42829e72)),
        (f32::from_bits(0x43d4cccd), f32::from_bits(0x4282a08f)),
        (f32::from_bits(0x43d52666), f32::from_bits(0x4282a00a)),
    ); // 424.9f, 65.3095f, 425.6f, 65.3136f, 426.3f, 65.3126f
    path.cubic_to(
        (f32::from_bits(0x43d58000), f32::from_bits(0x42829f85)),
        (f32::from_bits(0x43d5d99a), f32::from_bits(0x42829626)),
        (f32::from_bits(0x43d63333), f32::from_bits(0x4282942c)),
    ); // 427, 65.3116f, 427.7f, 65.2933f, 428.4f, 65.2894f
    path.cubic_to(
        (f32::from_bits(0x43d68ccd), f32::from_bits(0x42829231)),
        (f32::from_bits(0x43d6e666), f32::from_bits(0x42828e79)),
        (f32::from_bits(0x43d74000), f32::from_bits(0x4282942c)),
    ); // 429.1f, 65.2855f, 429.8f, 65.2783f, 430.5f, 65.2894f
    path.cubic_to(
        (f32::from_bits(0x43d7999a), f32::from_bits(0x428299de)),
        (f32::from_bits(0x43d7f333), f32::from_bits(0x4282ac18)),
        (f32::from_bits(0x43d84ccd), f32::from_bits(0x4282b65d)),
    ); // 431.2f, 65.3005f, 431.9f, 65.3361f, 432.6f, 65.3562f
    path.cubic_to(
        (f32::from_bits(0x43d8a666), f32::from_bits(0x4282c0a1)),
        (f32::from_bits(0x43d90000), f32::from_bits(0x4282cceb)),
        (f32::from_bits(0x43d9599a), f32::from_bits(0x4282d1c5)),
    ); // 433.3f, 65.3762f, 434, 65.4002f, 434.7f, 65.4097f
    path.cubic_to(
        (f32::from_bits(0x43d9b333), f32::from_bits(0x4282d69f)),
        (f32::from_bits(0x43da0ccd), f32::from_bits(0x4282d3cb)),
        (f32::from_bits(0x43da6666), f32::from_bits(0x4282d377)),
    ); // 435.4f, 65.4192f, 436.1f, 65.4137f, 436.8f, 65.413f
    path.cubic_to(
        (f32::from_bits(0x43dac000), f32::from_bits(0x4282d323)),
        (f32::from_bits(0x43db199a), f32::from_bits(0x4282d18a)),
        (f32::from_bits(0x43db7333), f32::from_bits(0x4282cfcc)),
    ); // 437.5f, 65.4124f, 438.2f, 65.4093f, 438.9f, 65.4059f
    path.cubic_to(
        (f32::from_bits(0x43dbcccd), f32::from_bits(0x4282ce0e)),
        (f32::from_bits(0x43dc2666), f32::from_bits(0x4282cd41)),
        (f32::from_bits(0x43dc8000), f32::from_bits(0x4282c904)),
    ); // 439.6f, 65.4025f, 440.3f, 65.4009f, 441, 65.3926f
    path.cubic_to(
        (f32::from_bits(0x43dcd99a), f32::from_bits(0x4282c4c7)),
        (f32::from_bits(0x43dd3333), f32::from_bits(0x4282bc94)),
        (f32::from_bits(0x43dd8ccd), f32::from_bits(0x4282b65d)),
    ); // 441.7f, 65.3843f, 442.4f, 65.3683f, 443.1f, 65.3562f
    path.cubic_to(
        (f32::from_bits(0x43dde666), f32::from_bits(0x4282b025)),
        (f32::from_bits(0x43de4000), f32::from_bits(0x4282a847)),
        (f32::from_bits(0x43de999a), f32::from_bits(0x4282a3b5)),
    ); // 443.8f, 65.344f, 444.5f, 65.3287f, 445.2f, 65.3197f
    path.cubic_to(
        (f32::from_bits(0x43def333), f32::from_bits(0x42829f24)),
        (f32::from_bits(0x43df4ccd), f32::from_bits(0x42829c21)),
        (f32::from_bits(0x43dfa666), f32::from_bits(0x42829af4)),
    ); // 445.9f, 65.3108f, 446.6f, 65.3049f, 447.3f, 65.3026f
    path.cubic_to(
        (f32::from_bits(0x43e00000), f32::from_bits(0x428299c7)),
        (f32::from_bits(0x43e0599a), f32::from_bits(0x42829af4)),
        (f32::from_bits(0x43e0b333), f32::from_bits(0x42829ca6)),
    ); // 448, 65.3003f, 448.7f, 65.3026f, 449.4f, 65.306f
    path.cubic_to(
        (f32::from_bits(0x43e10ccd), f32::from_bits(0x42829e58)),
        (f32::from_bits(0x43e16666), f32::from_bits(0x4282a31a)),
        (f32::from_bits(0x43e1c000), f32::from_bits(0x4282a521)),
    ); // 450.1f, 65.3093f, 450.8f, 65.3186f, 451.5f, 65.3225f
    path.cubic_to(
        (f32::from_bits(0x43e2199a), f32::from_bits(0x4282a727)),
        (f32::from_bits(0x43e27333), f32::from_bits(0x4282a640)),
        (f32::from_bits(0x43e2cccd), f32::from_bits(0x4282a8cc)),
    ); // 452.2f, 65.3265f, 452.9f, 65.3247f, 453.6f, 65.3297f
    path.cubic_to(
        (f32::from_bits(0x43e32666), f32::from_bits(0x4282ab57)),
        (f32::from_bits(0x43e38000), f32::from_bits(0x4282adef)),
        (f32::from_bits(0x43e3d99a), f32::from_bits(0x4282b464)),
    ); // 454.3f, 65.3346f, 455, 65.3397f, 455.7f, 65.3523f
    path.cubic_to(
        (f32::from_bits(0x43e43333), f32::from_bits(0x4282bad8)),
        (f32::from_bits(0x43e48ccd), f32::from_bits(0x4282c468)),
        (f32::from_bits(0x43e4e666), f32::from_bits(0x4282cf86)),
    ); // 456.4f, 65.3649f, 457.1f, 65.3836f, 457.8f, 65.4053f
    path.cubic_to(
        (f32::from_bits(0x43e54000), f32::from_bits(0x4282daa3)),
        (f32::from_bits(0x43e5999a), f32::from_bits(0x4282e9b4)),
        (f32::from_bits(0x43e5f333), f32::from_bits(0x4282f714)),
    ); // 458.5f, 65.427f, 459.2f, 65.4565f, 459.9f, 65.4826f
    path.cubic_to(
        (f32::from_bits(0x43e64ccd), f32::from_bits(0x42830474)),
        (f32::from_bits(0x43e6a666), f32::from_bits(0x428314c1)),
        (f32::from_bits(0x43e70000), f32::from_bits(0x42831fc7)),
    ); // 460.6f, 65.5087f, 461.3f, 65.5405f, 462, 65.5621f
    path.cubic_to(
        (f32::from_bits(0x43e7599a), f32::from_bits(0x42832acc)),
        (f32::from_bits(0x43e7b333), f32::from_bits(0x4283338f)),
        (f32::from_bits(0x43e80ccd), f32::from_bits(0x42833936)),
    ); // 462.7f, 65.5836f, 463.4f, 65.6007f, 464.1f, 65.6117f
    path.cubic_to(
        (f32::from_bits(0x43e86666), f32::from_bits(0x42833edd)),
        (f32::from_bits(0x43e8c000), f32::from_bits(0x42834090)),
        (f32::from_bits(0x43e9199a), f32::from_bits(0x428341b1)),
    ); // 464.8f, 65.6228f, 465.5f, 65.6261f, 466.2f, 65.6283f
    path.cubic_to(
        (f32::from_bits(0x43e97333), f32::from_bits(0x428342d2)),
        (f32::from_bits(0x43e9cccd), f32::from_bits(0x428340d8)),
        (f32::from_bits(0x43ea2666), f32::from_bits(0x42833fff)),
    ); // 466.9f, 65.6305f, 467.6f, 65.6266f, 468.3f, 65.625f
    path.cubic_to(
        (f32::from_bits(0x43ea8000), f32::from_bits(0x42833f26)),
        (f32::from_bits(0x43ead99a), f32::from_bits(0x42833e4d)),
        (f32::from_bits(0x43eb3333), f32::from_bits(0x42833c9b)),
    ); // 469, 65.6233f, 469.7f, 65.6217f, 470.4f, 65.6184f
    path.cubic_to(
        (f32::from_bits(0x43eb8ccd), f32::from_bits(0x42833ae8)),
        (f32::from_bits(0x43ebe666), f32::from_bits(0x42833f32)),
        (f32::from_bits(0x43ec4000), f32::from_bits(0x428335d2)),
    ); // 471.1f, 65.6151f, 471.8f, 65.6234f, 472.5f, 65.6051f
    path.cubic_to(
        (f32::from_bits(0x43ec999a), f32::from_bits(0x42832c73)),
        (f32::from_bits(0x43ecf333), f32::from_bits(0x42831e2e)),
        (f32::from_bits(0x43ed4ccd), f32::from_bits(0x4283045e)),
    ); // 473.2f, 65.5868f, 473.9f, 65.5589f, 474.6f, 65.5085f
    path.cubic_to(
        (f32::from_bits(0x43eda666), f32::from_bits(0x4282ea8e)),
        (f32::from_bits(0x43ee0000), f32::from_bits(0x4282bd9e)),
        (f32::from_bits(0x43ee599a), f32::from_bits(0x42829af4)),
    ); // 475.3f, 65.4581f, 476, 65.3703f, 476.7f, 65.3026f
    path.cubic_to(
        (f32::from_bits(0x43eeb333), f32::from_bits(0x4282784a)),
        (f32::from_bits(0x43ef0ccd), f32::from_bits(0x428245ac)),
        (f32::from_bits(0x43ef6666), f32::from_bits(0x42823461)),
    ); // 477.4f, 65.2349f, 478.1f, 65.1361f, 478.8f, 65.1023f
    path.cubic_to(
        (f32::from_bits(0x43efc000), f32::from_bits(0x42822316)),
        (f32::from_bits(0x43f0199a), f32::from_bits(0x42823365)),
        (f32::from_bits(0x43f07333), f32::from_bits(0x42823333)),
    ); // 479.5f, 65.0685f, 480.2f, 65.1004f, 480.9f, 65.1f
    path.line_to((f32::from_bits(0x43f07333), f32::from_bits(0x42823333))); // 480.9f, 65.1f
    path.line_to((f32::from_bits(0x43c24000), f32::from_bits(0x42823333))); // 388.5f, 65.1f
    path.close();
    path.move_to((f32::from_bits(0x43fc0000), f32::from_bits(0x42823333))); // 504, 65.1f
    path.line_to((f32::from_bits(0x43fc0000), f32::from_bits(0x42823333))); // 504, 65.1f
    path.cubic_to(
        (f32::from_bits(0x43fc599a), f32::from_bits(0x42823333)),
        (f32::from_bits(0x43fcb333), f32::from_bits(0x42822ef4)),
        (f32::from_bits(0x43fd0ccd), f32::from_bits(0x42823333)),
    ); // 504.7f, 65.1f, 505.4f, 65.0917f, 506.1f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x43fd6666), f32::from_bits(0x42823772)),
        (f32::from_bits(0x43fdc000), f32::from_bits(0x42823e7d)),
        (f32::from_bits(0x43fe199a), f32::from_bits(0x42824cac)),
    ); // 506.8f, 65.1083f, 507.5f, 65.122f, 508.2f, 65.1497f
    path.cubic_to(
        (f32::from_bits(0x43fe7333), f32::from_bits(0x42825adb)),
        (f32::from_bits(0x43fecccd), f32::from_bits(0x42827a73)),
        (f32::from_bits(0x43ff2666), f32::from_bits(0x4282884d)),
    ); // 508.9f, 65.1775f, 509.6f, 65.2392f, 510.3f, 65.2662f
    path.cubic_to(
        (f32::from_bits(0x43ff8000), f32::from_bits(0x42829626)),
        (f32::from_bits(0x43ffd99a), f32::from_bits(0x42829a88)),
        (f32::from_bits(0x4400199a), f32::from_bits(0x42829fc4)),
    ); // 511, 65.2933f, 511.7f, 65.3018f, 512.4f, 65.312f
    path.cubic_to(
        (f32::from_bits(0x44004666), f32::from_bits(0x4282a4ff)),
        (f32::from_bits(0x44007333), f32::from_bits(0x4282a6e4)),
        (f32::from_bits(0x4400a000), f32::from_bits(0x4282a7b1)),
    ); // 513.1f, 65.3223f, 513.8f, 65.326f, 514.5f, 65.3275f
    path.cubic_to(
        (f32::from_bits(0x4400cccd), f32::from_bits(0x4282a87e)),
        (f32::from_bits(0x4400f99a), f32::from_bits(0x4282a682)),
        (f32::from_bits(0x44012666), f32::from_bits(0x4282a494)),
    ); // 515.2f, 65.3291f, 515.9f, 65.3252f, 516.6f, 65.3214f
    path.cubic_to(
        (f32::from_bits(0x44015333), f32::from_bits(0x4282a2a5)),
        (f32::from_bits(0x44018000), f32::from_bits(0x42829e44)),
        (f32::from_bits(0x4401accd), f32::from_bits(0x42829c19)),
    ); // 517.3f, 65.3177f, 518, 65.3091f, 518.7f, 65.3049f
    path.cubic_to(
        (f32::from_bits(0x4401d99a), f32::from_bits(0x428299ee)),
        (f32::from_bits(0x44020666), f32::from_bits(0x42829718)),
        (f32::from_bits(0x44023333), f32::from_bits(0x42829790)),
    ); // 519.4f, 65.3006f, 520.1f, 65.2951f, 520.8f, 65.296f
    path.cubic_to(
        (f32::from_bits(0x44026000), f32::from_bits(0x42829807)),
        (f32::from_bits(0x44028ccd), f32::from_bits(0x42829ae5)),
        (f32::from_bits(0x4402b99a), f32::from_bits(0x42829ee5)),
    ); // 521.5f, 65.2969f, 522.2f, 65.3025f, 522.9f, 65.3103f
    path.cubic_to(
        (f32::from_bits(0x4402e666), f32::from_bits(0x4282a2e6)),
        (f32::from_bits(0x44031333), f32::from_bits(0x4282abab)),
        (f32::from_bits(0x44034000), f32::from_bits(0x4282af94)),
    ); // 523.6f, 65.3182f, 524.3f, 65.3353f, 525, 65.3429f
    path.cubic_to(
        (f32::from_bits(0x44036ccd), f32::from_bits(0x4282b37d)),
        (f32::from_bits(0x4403999a), f32::from_bits(0x4282b5c0)),
        (f32::from_bits(0x4403c666), f32::from_bits(0x4282b65d)),
    ); // 525.7f, 65.3506f, 526.4f, 65.355f, 527.1f, 65.3562f
    path.cubic_to(
        (f32::from_bits(0x4403f333), f32::from_bits(0x4282b6f9)),
        (f32::from_bits(0x44042000), f32::from_bits(0x4282c91b)),
        (f32::from_bits(0x44044ccd), f32::from_bits(0x4282b33f)),
    ); // 527.8f, 65.3574f, 528.5f, 65.3928f, 529.2f, 65.3501f
    path.cubic_to(
        (f32::from_bits(0x4404799a), f32::from_bits(0x42829d63)),
        (f32::from_bits(0x4404a666), f32::from_bits(0x4282488a)),
        (f32::from_bits(0x4404d333), f32::from_bits(0x42823333)),
    ); // 529.9f, 65.3074f, 530.6f, 65.1417f, 531.3f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x44050000), f32::from_bits(0x42821ddb)),
        (f32::from_bits(0x44052ccd), f32::from_bits(0x42823333)),
        (f32::from_bits(0x4405599a), f32::from_bits(0x42823333)),
    ); // 532, 65.0583f, 532.7f, 65.1f, 533.4f, 65.1f
    path.line_to((f32::from_bits(0x4405599a), f32::from_bits(0x42823333))); // 533.4f, 65.1f
    path.line_to((f32::from_bits(0x43fc0000), f32::from_bits(0x42823333))); // 504, 65.1f
    path.close();
    path.detach()
}

// Port of: tests/PathOpsIssue3651.cpp#L378-L427 (chrome/m156)
fn path1_a() -> Path {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x43994ccd), f32::from_bits(0x42823333))); // 306.6f, 65.1f
    path.line_to((f32::from_bits(0x43994ccd), f32::from_bits(0x42823333))); // 306.6f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x4399a666), f32::from_bits(0x42823332)),
        (f32::from_bits(0x439a0000), f32::from_bits(0x42823842)),
        (f32::from_bits(0x439a599a), f32::from_bits(0x4282332a)),
    ); // 307.3f, 65.1f, 308, 65.1099f, 308.7f, 65.0999f
    path.cubic_to(
        (f32::from_bits(0x439ab333), f32::from_bits(0x42822e12)),
        (f32::from_bits(0x439b0ccd), f32::from_bits(0x42821e94)),
        (f32::from_bits(0x439b6666), f32::from_bits(0x428214a4)),
    ); // 309.4f, 65.09f, 310.1f, 65.0597f, 310.8f, 65.0403f
    path.cubic_to(
        (f32::from_bits(0x439bc000), f32::from_bits(0x42820ab4)),
        (f32::from_bits(0x439c199a), f32::from_bits(0x42820185)),
        (f32::from_bits(0x439c7333), f32::from_bits(0x4281f789)),
    ); // 311.5f, 65.0209f, 312.2f, 65.003f, 312.9f, 64.9835f
    path.line_to((f32::from_bits(0x43c02666), f32::from_bits(0x42823333))); // 384.3f, 65.1f
    path.line_to((f32::from_bits(0x43994ccd), f32::from_bits(0x42823333))); // 306.6f, 65.1f
    path.close();
    path.detach()
}

// Port of: tests/PathOpsIssue3651.cpp#L429-L478 (chrome/m156)
fn path2_a() -> Path {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x43994ccd), f32::from_bits(0x42823333))); // 306.6f, 65.1f
    path.line_to((f32::from_bits(0x43994ccd), f32::from_bits(0x42823333))); // 306.6f, 65.1f
    path.cubic_to(
        (f32::from_bits(0x4399a666), f32::from_bits(0x42823334)),
        (f32::from_bits(0x439a0000), f32::from_bits(0x42822e24)),
        (f32::from_bits(0x439a599a), f32::from_bits(0x4282333c)),
    ); // 307.3f, 65.1f, 308, 65.0901f, 308.7f, 65.1001f
    path.cubic_to(
        (f32::from_bits(0x439ab333), f32::from_bits(0x42823854)),
        (f32::from_bits(0x439b0ccd), f32::from_bits(0x428247d2)),
        (f32::from_bits(0x439b6666), f32::from_bits(0x428251c2)),
    ); // 309.4f, 65.11f, 310.1f, 65.1403f, 310.8f, 65.1597f
    path.cubic_to(
        (f32::from_bits(0x439bc000), f32::from_bits(0x42825bb2)),
        (f32::from_bits(0x439c199a), f32::from_bits(0x428264e1)),
        (f32::from_bits(0x439c7333), f32::from_bits(0x42826edd)),
    ); // 311.5f, 65.1791f, 312.2f, 65.197f, 312.9f, 65.2165f
    path.line_to((f32::from_bits(0x43c02666), f32::from_bits(0x42823333))); // 384.3f, 65.1f
    path.line_to((f32::from_bits(0x43994ccd), f32::from_bits(0x42823333))); // 306.6f, 65.1f
    path.close();
    path.detach()
}

// Port of: tests/PathOpsIssue3651.cpp#L480-L486 (chrome/m156)
fn issue3651_1a(reporter: &mut Reporter, filename: &str) {
    let path = path1_a();
    let pathB = path2_a();
    // DEBUG_UNDER_DEVELOPMENT  issue3651_1a disable expectation check for now
    test_path_op_check(reporter, &path, &pathB, PathOp::Union, filename, true);
}

// Port of: tests/PathOpsIssue3651.cpp#L488-L657 (chrome/m156)
fn path3() -> Path {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x42b06666), f32::from_bits(0x42bd0000))); // 88.2f, 94.5f
    path.line_to((f32::from_bits(0x42b06666), f32::from_bits(0x42bd0000))); // 88.2f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x42b1cccd), f32::from_bits(0x42bd0000)),
        (f32::from_bits(0x42b33333), f32::from_bits(0x42bd2573)),
        (f32::from_bits(0x42b4999a), f32::from_bits(0x42bd0000)),
    ); // 88.9f, 94.5f, 89.6f, 94.5731f, 90.3f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x42b60000), f32::from_bits(0x42bcda8d)),
        (f32::from_bits(0x42b76666), f32::from_bits(0x42bc4598)),
        (f32::from_bits(0x42b8cccd), f32::from_bits(0x42bc1f4b)),
    ); // 91, 94.4269f, 91.7f, 94.1359f, 92.4f, 94.0611f
    path.cubic_to(
        (f32::from_bits(0x42ba3333), f32::from_bits(0x42bbf8ff)),
        (f32::from_bits(0x42bb999a), f32::from_bits(0x42bc1b9e)),
        (f32::from_bits(0x42bd0000), f32::from_bits(0x42bc1a35)),
    ); // 93.1f, 93.9863f, 93.8f, 94.0539f, 94.5f, 94.0512f
    path.cubic_to(
        (f32::from_bits(0x42be6666), f32::from_bits(0x42bc18cb)),
        (f32::from_bits(0x42bfcccd), f32::from_bits(0x42bc17f2)),
        (f32::from_bits(0x42c13333), f32::from_bits(0x42bc16d0)),
    ); // 95.2f, 94.0484f, 95.9f, 94.0468f, 96.6f, 94.0446f
    path.cubic_to(
        (f32::from_bits(0x42c2999a), f32::from_bits(0x42bc15af)),
        (f32::from_bits(0x42c40000), f32::from_bits(0x42bc13fd)),
        (f32::from_bits(0x42c56666), f32::from_bits(0x42bc136c)),
    ); // 97.3f, 94.0424f, 98, 94.039f, 98.7f, 94.0379f
    path.cubic_to(
        (f32::from_bits(0x42c6cccd), f32::from_bits(0x42bc12dc)),
        (f32::from_bits(0x42c83333), f32::from_bits(0x42bc12e7)),
        (f32::from_bits(0x42c9999a), f32::from_bits(0x42bc136c)),
    ); // 99.4f, 94.0368f, 100.1f, 94.0369f, 100.8f, 94.0379f
    path.cubic_to(
        (f32::from_bits(0x42cb0000), f32::from_bits(0x42bc13f1)),
        (f32::from_bits(0x42cc6666), f32::from_bits(0x42bc15c8)),
        (f32::from_bits(0x42cdcccd), f32::from_bits(0x42bc168a)),
    ); // 101.5f, 94.0389f, 102.2f, 94.0425f, 102.9f, 94.044f
    path.cubic_to(
        (f32::from_bits(0x42cf3333), f32::from_bits(0x42bc174b)),
        (f32::from_bits(0x42d0999a), f32::from_bits(0x42bc17ad)),
        (f32::from_bits(0x42d20000), f32::from_bits(0x42bc17f5)),
    ); // 103.6f, 94.0455f, 104.3f, 94.0462f, 105, 94.0468f
    path.cubic_to(
        (f32::from_bits(0x42d36666), f32::from_bits(0x42bc183e)),
        (f32::from_bits(0x42d4cccd), f32::from_bits(0x42bc162a)),
        (f32::from_bits(0x42d63333), f32::from_bits(0x42bc183c)),
    ); // 105.7f, 94.0473f, 106.4f, 94.0433f, 107.1f, 94.0473f
    path.cubic_to(
        (f32::from_bits(0x42d7999a), f32::from_bits(0x42bc1a4e)),
        (f32::from_bits(0x42d90000), f32::from_bits(0x42bc1e66)),
        (f32::from_bits(0x42da6666), f32::from_bits(0x42bc2461)),
    ); // 107.8f, 94.0514f, 108.5f, 94.0594f, 109.2f, 94.0711f
    path.cubic_to(
        (f32::from_bits(0x42dbcccd), f32::from_bits(0x42bc2a5d)),
        (f32::from_bits(0x42dd3333), f32::from_bits(0x42bc33f9)),
        (f32::from_bits(0x42de999a), f32::from_bits(0x42bc3c1f)),
    ); // 109.9f, 94.0827f, 110.6f, 94.1015f, 111.3f, 94.1174f
    path.cubic_to(
        (f32::from_bits(0x42e00000), f32::from_bits(0x42bc4446)),
        (f32::from_bits(0x42e16666), f32::from_bits(0x42bc4cce)),
        (f32::from_bits(0x42e2cccd), f32::from_bits(0x42bc5548)),
    ); // 112, 94.1333f, 112.7f, 94.15f, 113.4f, 94.1666f
    path.cubic_to(
        (f32::from_bits(0x42e43333), f32::from_bits(0x42bc5dc3)),
        (f32::from_bits(0x42e5999a), f32::from_bits(0x42bc6472)),
        (f32::from_bits(0x42e70000), f32::from_bits(0x42bc6eff)),
    ); // 114.1f, 94.1831f, 114.8f, 94.1962f, 115.5f, 94.2168f
    path.cubic_to(
        (f32::from_bits(0x42e86666), f32::from_bits(0x42bc798b)),
        (f32::from_bits(0x42e9cccd), f32::from_bits(0x42bc8607)),
        (f32::from_bits(0x42eb3333), f32::from_bits(0x42bc9494)),
    ); // 116.2f, 94.2374f, 116.9f, 94.2618f, 117.6f, 94.2902f
    path.cubic_to(
        (f32::from_bits(0x42ec999a), f32::from_bits(0x42bca321)),
        (f32::from_bits(0x42ee0000), f32::from_bits(0x42bcb9f8)),
        (f32::from_bits(0x42ef6666), f32::from_bits(0x42bcc64f)),
    ); // 118.3f, 94.3186f, 119, 94.3632f, 119.7f, 94.3873f
    path.cubic_to(
        (f32::from_bits(0x42f0cccd), f32::from_bits(0x42bcd2a5)),
        (f32::from_bits(0x42f23333), f32::from_bits(0x42bcdb35)),
        (f32::from_bits(0x42f3999a), f32::from_bits(0x42bcde9a)),
    ); // 120.4f, 94.4114f, 121.1f, 94.4281f, 121.8f, 94.4348f
    path.cubic_to(
        (f32::from_bits(0x42f50000), f32::from_bits(0x42bce1fe)),
        (f32::from_bits(0x42f66666), f32::from_bits(0x42bcdd3f)),
        (f32::from_bits(0x42f7cccd), f32::from_bits(0x42bcdaa8)),
    ); // 122.5f, 94.4414f, 123.2f, 94.4321f, 123.9f, 94.4271f
    path.cubic_to(
        (f32::from_bits(0x42f93333), f32::from_bits(0x42bcd811)),
        (f32::from_bits(0x42fa999a), f32::from_bits(0x42bcd25d)),
        (f32::from_bits(0x42fc0000), f32::from_bits(0x42bccf10)),
    ); // 124.6f, 94.422f, 125.3f, 94.4109f, 126, 94.4044f
    path.cubic_to(
        (f32::from_bits(0x42fd6666), f32::from_bits(0x42bccbc3)),
        (f32::from_bits(0x42fecccd), f32::from_bits(0x42bcc95b)),
        (f32::from_bits(0x4300199a), f32::from_bits(0x42bcc6dc)),
    ); // 126.7f, 94.398f, 127.4f, 94.3933f, 128.1f, 94.3884f
    path.cubic_to(
        (f32::from_bits(0x4300cccd), f32::from_bits(0x42bcc45c)),
        (f32::from_bits(0x43018000), f32::from_bits(0x42bcc0ec)),
        (f32::from_bits(0x43023333), f32::from_bits(0x42bcc013)),
    ); // 128.8f, 94.3835f, 129.5f, 94.3768f, 130.2f, 94.3751f
    path.cubic_to(
        (f32::from_bits(0x4302e666), f32::from_bits(0x42bcbf3a)),
        (f32::from_bits(0x4303999a), f32::from_bits(0x42bcc0b0)),
        (f32::from_bits(0x43044ccd), f32::from_bits(0x42bcc1c5)),
    ); // 130.9f, 94.3735f, 131.6f, 94.3763f, 132.3f, 94.3785f
    path.cubic_to(
        (f32::from_bits(0x43050000), f32::from_bits(0x42bcc2db)),
        (f32::from_bits(0x4305b333), f32::from_bits(0x42bcc5bc)),
        (f32::from_bits(0x43066666), f32::from_bits(0x42bcc695)),
    ); // 133, 94.3806f, 133.7f, 94.3862f, 134.4f, 94.3879f
    path.cubic_to(
        (f32::from_bits(0x4307199a), f32::from_bits(0x42bcc76e)),
        (f32::from_bits(0x4307cccd), f32::from_bits(0x42bcc688)),
        (f32::from_bits(0x43088000), f32::from_bits(0x42bcc6dc)),
    ); // 135.1f, 94.3895f, 135.8f, 94.3878f, 136.5f, 94.3884f
    path.cubic_to(
        (f32::from_bits(0x43093333), f32::from_bits(0x42bcc730)),
        (f32::from_bits(0x4309e666), f32::from_bits(0x42bcc89a)),
        (f32::from_bits(0x430a999a), f32::from_bits(0x42bcc88e)),
    ); // 137.2f, 94.389f, 137.9f, 94.3918f, 138.6f, 94.3917f
    path.cubic_to(
        (f32::from_bits(0x430b4ccd), f32::from_bits(0x42bcc882)),
        (f32::from_bits(0x430c0000), f32::from_bits(0x42bcc76e)),
        (f32::from_bits(0x430cb333), f32::from_bits(0x42bcc695)),
    ); // 139.3f, 94.3916f, 140, 94.3895f, 140.7f, 94.3879f
    path.cubic_to(
        (f32::from_bits(0x430d6666), f32::from_bits(0x42bcc5bc)),
        (f32::from_bits(0x430e199a), f32::from_bits(0x42bcc445)),
        (f32::from_bits(0x430ecccd), f32::from_bits(0x42bcc378)),
    ); // 141.4f, 94.3862f, 142.1f, 94.3833f, 142.8f, 94.3818f
    path.cubic_to(
        (f32::from_bits(0x430f8000), f32::from_bits(0x42bcc2aa)),
        (f32::from_bits(0x43103333), f32::from_bits(0x42bcc32f)),
        (f32::from_bits(0x4310e666), f32::from_bits(0x42bcc1c5)),
    ); // 143.5f, 94.3802f, 144.2f, 94.3812f, 144.9f, 94.3785f
    path.cubic_to(
        (f32::from_bits(0x4311999a), f32::from_bits(0x42bcc05c)),
        (f32::from_bits(0x43124ccd), f32::from_bits(0x42bcbd88)),
        (f32::from_bits(0x43130000), f32::from_bits(0x42bcbafd)),
    ); // 145.6f, 94.3757f, 146.3f, 94.3702f, 147, 94.3652f
    path.cubic_to(
        (f32::from_bits(0x4313b333), f32::from_bits(0x42bcb872)),
        (f32::from_bits(0x43146666), f32::from_bits(0x42bcb50e)),
        (f32::from_bits(0x4315199a), f32::from_bits(0x42bcb282)),
    ); // 147.7f, 94.3602f, 148.4f, 94.3536f, 149.1f, 94.3486f
    path.cubic_to(
        (f32::from_bits(0x4315cccd), f32::from_bits(0x42bcaff7)),
        (f32::from_bits(0x43168000), f32::from_bits(0x42bcac56)),
        (f32::from_bits(0x43173333), f32::from_bits(0x42bcabba)),
    ); // 149.8f, 94.3437f, 150.5f, 94.3366f, 151.2f, 94.3354f
    path.cubic_to(
        (f32::from_bits(0x4317e666), f32::from_bits(0x42bcab1d)),
        (f32::from_bits(0x4318999a), f32::from_bits(0x42bcadfe)),
        (f32::from_bits(0x43194ccd), f32::from_bits(0x42bcaed8)),
    ); // 151.9f, 94.3342f, 152.6f, 94.3398f, 153.3f, 94.3415f
    path.cubic_to(
        (f32::from_bits(0x431a0000), f32::from_bits(0x42bcafb1)),
        (f32::from_bits(0x431ab333), f32::from_bits(0x42bcb034)),
        (f32::from_bits(0x431b6666), f32::from_bits(0x42bcb0d0)),
    ); // 154, 94.3431f, 154.7f, 94.3441f, 155.4f, 94.3453f
    path.cubic_to(
        (f32::from_bits(0x431c199a), f32::from_bits(0x42bcb16d)),
        (f32::from_bits(0x431ccccd), f32::from_bits(0x42bcb119)),
        (f32::from_bits(0x431d8000), f32::from_bits(0x42bcb282)),
    ); // 156.1f, 94.3465f, 156.8f, 94.3459f, 157.5f, 94.3486f
    path.cubic_to(
        (f32::from_bits(0x431e3333), f32::from_bits(0x42bcb3ec)),
        (f32::from_bits(0x431ee666), f32::from_bits(0x42bcb708)),
        (f32::from_bits(0x431f999a), f32::from_bits(0x42bcb94b)),
    ); // 158.2f, 94.3514f, 158.9f, 94.3575f, 159.6f, 94.3619f
    path.cubic_to(
        (f32::from_bits(0x43204ccd), f32::from_bits(0x42bcbb8e)),
        (f32::from_bits(0x43210000), f32::from_bits(0x42bcbef2)),
        (f32::from_bits(0x4321b333), f32::from_bits(0x42bcc013)),
    ); // 160.3f, 94.3663f, 161, 94.3729f, 161.7f, 94.3751f
    path.cubic_to(
        (f32::from_bits(0x43226666), f32::from_bits(0x42bcc135)),
        (f32::from_bits(0x4323199a), f32::from_bits(0x42bcbfe3)),
        (f32::from_bits(0x4323cccd), f32::from_bits(0x42bcc013)),
    ); // 162.4f, 94.3774f, 163.1f, 94.3748f, 163.8f, 94.3751f
    path.cubic_to(
        (f32::from_bits(0x43248000), f32::from_bits(0x42bcc044)),
        (f32::from_bits(0x43253333), f32::from_bits(0x42bcbf19)),
        (f32::from_bits(0x4325e666), f32::from_bits(0x42bcc138)),
    ); // 164.5f, 94.3755f, 165.2f, 94.3732f, 165.9f, 94.3774f
    path.cubic_to(
        (f32::from_bits(0x4326999a), f32::from_bits(0x42bcc358)),
        (f32::from_bits(0x43274ccd), f32::from_bits(0x42bcc7c6)),
        (f32::from_bits(0x43280000), f32::from_bits(0x42bcccd1)),
    ); // 166.6f, 94.3815f, 167.3f, 94.3902f, 168, 94.4f
    path.cubic_to(
        (f32::from_bits(0x4328b333), f32::from_bits(0x42bcd1db)),
        (f32::from_bits(0x43296666), f32::from_bits(0x42bcd9b9)),
        (f32::from_bits(0x432a199a), f32::from_bits(0x42bcdf78)),
    ); // 168.7f, 94.4099f, 169.4f, 94.4252f, 170.1f, 94.4365f
    path.cubic_to(
        (f32::from_bits(0x432acccd), f32::from_bits(0x42bce536)),
        (f32::from_bits(0x432b8000), f32::from_bits(0x42bceb84)),
        (f32::from_bits(0x432c3333), f32::from_bits(0x42bcef48)),
    ); // 170.8f, 94.4477f, 171.5f, 94.46f, 172.2f, 94.4673f
    path.cubic_to(
        (f32::from_bits(0x432ce666), f32::from_bits(0x42bcf30c)),
        (f32::from_bits(0x432d999a), f32::from_bits(0x42bcf3ce)),
        (f32::from_bits(0x432e4ccd), f32::from_bits(0x42bcf611)),
    ); // 172.9f, 94.4747f, 173.6f, 94.4762f, 174.3f, 94.4806f
    path.cubic_to(
        (f32::from_bits(0x432f0000), f32::from_bits(0x42bcf853)),
        (f32::from_bits(0x432fb333), f32::from_bits(0x42bcfb31)),
        (f32::from_bits(0x43306666), f32::from_bits(0x42bcfcd9)),
    ); // 175, 94.485f, 175.7f, 94.4906f, 176.4f, 94.4938f
    path.line_to((f32::from_bits(0x43306666), f32::from_bits(0x42bd0000))); // 176.4f, 94.5f
    path.line_to((f32::from_bits(0x42b06666), f32::from_bits(0x42bd0000))); // 88.2f, 94.5f
    path.close();
    path.move_to((f32::from_bits(0x43413333), f32::from_bits(0x42bd0000))); // 193.2f, 94.5f
    path.line_to((f32::from_bits(0x43413333), f32::from_bits(0x42bd0000))); // 193.2f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x4341e666), f32::from_bits(0x42bd0000)),
        (f32::from_bits(0x4342999a), f32::from_bits(0x42bd0549)),
        (f32::from_bits(0x43434ccd), f32::from_bits(0x42bd0000)),
    ); // 193.9f, 94.5f, 194.6f, 94.5103f, 195.3f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x43440000), f32::from_bits(0x42bcfab7)),
        (f32::from_bits(0x4344b333), f32::from_bits(0x42bcec20)),
        (f32::from_bits(0x43456666), f32::from_bits(0x42bce04c)),
    ); // 196, 94.4897f, 196.7f, 94.4612f, 197.4f, 94.4381f
    path.cubic_to(
        (f32::from_bits(0x4346199a), f32::from_bits(0x42bcd477)),
        (f32::from_bits(0x4346cccd), f32::from_bits(0x42bcc14e)),
        (f32::from_bits(0x43478000), f32::from_bits(0x42bcb904)),
    ); // 198.1f, 94.415f, 198.8f, 94.3775f, 199.5f, 94.3614f
    path.cubic_to(
        (f32::from_bits(0x43483333), f32::from_bits(0x42bcb0ba)),
        (f32::from_bits(0x4348e666), f32::from_bits(0x42bcaed9)),
        (f32::from_bits(0x4349999a), f32::from_bits(0x42bcae91)),
    ); // 200.2f, 94.3452f, 200.9f, 94.3415f, 201.6f, 94.341f
    path.cubic_to(
        (f32::from_bits(0x434a4ccd), f32::from_bits(0x42bcae49)),
        (f32::from_bits(0x434b0000), f32::from_bits(0x42bcb23c)),
        (f32::from_bits(0x434bb333), f32::from_bits(0x42bcb752)),
    ); // 202.3f, 94.3404f, 203, 94.3481f, 203.7f, 94.358f
    path.cubic_to(
        (f32::from_bits(0x434c6666), f32::from_bits(0x42bcbc69)),
        (f32::from_bits(0x434d199a), f32::from_bits(0x42bcc612)),
        (f32::from_bits(0x434dcccd), f32::from_bits(0x42bccd17)),
    ); // 204.4f, 94.368f, 205.1f, 94.3869f, 205.8f, 94.4006f
    path.cubic_to(
        (f32::from_bits(0x434e8000), f32::from_bits(0x42bcd41c)),
        (f32::from_bits(0x434f3333), f32::from_bits(0x42bcdc5a)),
        (f32::from_bits(0x434fe666), f32::from_bits(0x42bce171)),
    ); // 206.5f, 94.4143f, 207.2f, 94.4304f, 207.9f, 94.4403f
    path.cubic_to(
        (f32::from_bits(0x4350999a), f32::from_bits(0x42bce687)),
        (f32::from_bits(0x43514ccd), f32::from_bits(0x42bceb0d)),
        (f32::from_bits(0x43520000), f32::from_bits(0x42bceb9d)),
    ); // 208.6f, 94.4502f, 209.3f, 94.4591f, 210, 94.4602f
    path.cubic_to(
        (f32::from_bits(0x4352b333), f32::from_bits(0x42bcec2e)),
        (f32::from_bits(0x43536666), f32::from_bits(0x42bcea70)),
        (f32::from_bits(0x4354199a), f32::from_bits(0x42bce4d5)),
    ); // 210.7f, 94.4613f, 211.4f, 94.4579f, 212.1f, 94.4469f
    path.cubic_to(
        (f32::from_bits(0x4354cccd), f32::from_bits(0x42bcdf39)),
        (f32::from_bits(0x43558000), f32::from_bits(0x42bcd432)),
        (f32::from_bits(0x43563333), f32::from_bits(0x42bcc9f9)),
    ); // 212.8f, 94.436f, 213.5f, 94.4144f, 214.2f, 94.3945f
    path.cubic_to(
        (f32::from_bits(0x4356e666), f32::from_bits(0x42bcbfc1)),
        (f32::from_bits(0x4357999a), f32::from_bits(0x42bcb9bc)),
        (f32::from_bits(0x43584ccd), f32::from_bits(0x42bca782)),
    ); // 214.9f, 94.3745f, 215.6f, 94.3628f, 216.3f, 94.3272f
    path.cubic_to(
        (f32::from_bits(0x43590000), f32::from_bits(0x42bc9548)),
        (f32::from_bits(0x4359b333), f32::from_bits(0x42bc76fe)),
        (f32::from_bits(0x435a6666), f32::from_bits(0x42bc5c9e)),
    ); // 217, 94.2916f, 217.7f, 94.2324f, 218.4f, 94.1809f
    path.cubic_to(
        (f32::from_bits(0x435b199a), f32::from_bits(0x42bc423e)),
        (f32::from_bits(0x435bcccd), f32::from_bits(0x42bc19b0)),
        (f32::from_bits(0x435c8000), f32::from_bits(0x42bc0940)),
    ); // 219.1f, 94.1294f, 219.8f, 94.0502f, 220.5f, 94.0181f
    path.cubic_to(
        (f32::from_bits(0x435d3333), f32::from_bits(0x42bbf8cf)),
        (f32::from_bits(0x435de666), f32::from_bits(0x42bbfbf7)),
        (f32::from_bits(0x435e999a), f32::from_bits(0x42bbf9fc)),
    ); // 221.2f, 93.986f, 221.9f, 93.9921f, 222.6f, 93.9883f
    path.cubic_to(
        (f32::from_bits(0x435f4ccd), f32::from_bits(0x42bbf802)),
        (f32::from_bits(0x43600000), f32::from_bits(0x42bbfad6)),
        (f32::from_bits(0x4360b333), f32::from_bits(0x42bbfd61)),
    ); // 223.3f, 93.9844f, 224, 93.9899f, 224.7f, 93.9949f
    path.cubic_to(
        (f32::from_bits(0x43616666), f32::from_bits(0x42bbffec)),
        (f32::from_bits(0x4362199a), f32::from_bits(0x42bc06fd)),
        (f32::from_bits(0x4362cccd), f32::from_bits(0x42bc0940)),
    ); // 225.4f, 93.9998f, 226.1f, 94.0136f, 226.8f, 94.0181f
    path.cubic_to(
        (f32::from_bits(0x43638000), f32::from_bits(0x42bc0b82)),
        (f32::from_bits(0x43643333), f32::from_bits(0x42bc0c13)),
        (f32::from_bits(0x4364e666), f32::from_bits(0x42bc0af2)),
    ); // 227.5f, 94.0225f, 228.2f, 94.0236f, 228.9f, 94.0214f
    path.cubic_to(
        (f32::from_bits(0x4365999a), f32::from_bits(0x42bc09d0)),
        (f32::from_bits(0x43664ccd), f32::from_bits(0x42bc019e)),
        (f32::from_bits(0x43670000), f32::from_bits(0x42bc0277)),
    ); // 229.6f, 94.0192f, 230.3f, 94.0032f, 231, 94.0048f
    path.cubic_to(
        (f32::from_bits(0x4367b333), f32::from_bits(0x42bc0350)),
        (f32::from_bits(0x43686666), f32::from_bits(0x42bc0b3a)),
        (f32::from_bits(0x4369199a), f32::from_bits(0x42bc1008)),
    ); // 231.7f, 94.0065f, 232.4f, 94.0219f, 233.1f, 94.0313f
    path.cubic_to(
        (f32::from_bits(0x4369cccd), f32::from_bits(0x42bc14d6)),
        (f32::from_bits(0x436a8000), f32::from_bits(0x42bc1d08)),
        (f32::from_bits(0x436b3333), f32::from_bits(0x42bc1f4b)),
    ); // 233.8f, 94.0407f, 234.5f, 94.0567f, 235.2f, 94.0611f
    path.cubic_to(
        (f32::from_bits(0x436be666), f32::from_bits(0x42bc218e)),
        (f32::from_bits(0x436c999a), f32::from_bits(0x42bc1e2a)),
        (f32::from_bits(0x436d4ccd), f32::from_bits(0x42bc1d99)),
    ); // 235.9f, 94.0655f, 236.6f, 94.0589f, 237.3f, 94.0578f
    path.cubic_to(
        (f32::from_bits(0x436e0000), f32::from_bits(0x42bc1d08)),
        (f32::from_bits(0x436eb333), f32::from_bits(0x42bc1cc0)),
        (f32::from_bits(0x436f6666), f32::from_bits(0x42bc1be7)),
    ); // 238, 94.0567f, 238.7f, 94.0562f, 239.4f, 94.0545f
    path.cubic_to(
        (f32::from_bits(0x4370199a), f32::from_bits(0x42bc1b0e)),
        (f32::from_bits(0x4370cccd), f32::from_bits(0x42bc195c)),
        (f32::from_bits(0x43718000), f32::from_bits(0x42bc1883)),
    ); // 240.1f, 94.0528f, 240.8f, 94.0495f, 241.5f, 94.0479f
    path.cubic_to(
        (f32::from_bits(0x43723333), f32::from_bits(0x42bc17aa)),
        (f32::from_bits(0x4372e666), f32::from_bits(0x42bc1719)),
        (f32::from_bits(0x4373999a), f32::from_bits(0x42bc16d0)),
    ); // 242.2f, 94.0462f, 242.9f, 94.0451f, 243.6f, 94.0446f
    path.cubic_to(
        (f32::from_bits(0x43744ccd), f32::from_bits(0x42bc1688)),
        (f32::from_bits(0x43750000), f32::from_bits(0x42bc1719)),
        (f32::from_bits(0x4375b333), f32::from_bits(0x42bc16d0)),
    ); // 244.3f, 94.044f, 245, 94.0451f, 245.7f, 94.0446f
    path.cubic_to(
        (f32::from_bits(0x43766666), f32::from_bits(0x42bc1688)),
        (f32::from_bits(0x4377199a), f32::from_bits(0x42bc1567)),
        (f32::from_bits(0x4377cccd), f32::from_bits(0x42bc151e)),
    ); // 246.4f, 94.044f, 247.1f, 94.0418f, 247.8f, 94.0412f
    path.cubic_to(
        (f32::from_bits(0x43788000), f32::from_bits(0x42bc14d6)),
        (f32::from_bits(0x43793333), f32::from_bits(0x42bc148e)),
        (f32::from_bits(0x4379e666), f32::from_bits(0x42bc151e)),
    ); // 248.5f, 94.0407f, 249.2f, 94.0401f, 249.9f, 94.0412f
    path.cubic_to(
        (f32::from_bits(0x437a999a), f32::from_bits(0x42bc15af)),
        (f32::from_bits(0x437b4ccd), f32::from_bits(0x42bc1761)),
        (f32::from_bits(0x437c0000), f32::from_bits(0x42bc1883)),
    ); // 250.6f, 94.0424f, 251.3f, 94.0457f, 252, 94.0479f
    path.cubic_to(
        (f32::from_bits(0x437cb333), f32::from_bits(0x42bc19a4)),
        (f32::from_bits(0x437d6666), f32::from_bits(0x42bc1b0e)),
        (f32::from_bits(0x437e199a), f32::from_bits(0x42bc1be7)),
    ); // 252.7f, 94.0501f, 253.4f, 94.0528f, 254.1f, 94.0545f
    path.cubic_to(
        (f32::from_bits(0x437ecccd), f32::from_bits(0x42bc1cc0)),
        (f32::from_bits(0x437f8000), f32::from_bits(0x42bc1d99)),
        (f32::from_bits(0x4380199a), f32::from_bits(0x42bc1d99)),
    ); // 254.8f, 94.0562f, 255.5f, 94.0578f, 256.2f, 94.0578f
    path.cubic_to(
        (f32::from_bits(0x43807333), f32::from_bits(0x42bc1d99)),
        (f32::from_bits(0x4380cccd), f32::from_bits(0x42bc1d08)),
        (f32::from_bits(0x43812666), f32::from_bits(0x42bc1be7)),
    ); // 256.9f, 94.0578f, 257.6f, 94.0567f, 258.3f, 94.0545f
    path.cubic_to(
        (f32::from_bits(0x43818000), f32::from_bits(0x42bc1ac5)),
        (f32::from_bits(0x4381d99a), f32::from_bits(0x42bc183a)),
        (f32::from_bits(0x43823333), f32::from_bits(0x42bc16d0)),
    ); // 259, 94.0523f, 259.7f, 94.0473f, 260.4f, 94.0446f
    path.cubic_to(
        (f32::from_bits(0x43828ccd), f32::from_bits(0x42bc1567)),
        (f32::from_bits(0x4382e666), f32::from_bits(0x42bc13fd)),
        (f32::from_bits(0x43834000), f32::from_bits(0x42bc136c)),
    ); // 261.1f, 94.0418f, 261.8f, 94.039f, 262.5f, 94.0379f
    path.cubic_to(
        (f32::from_bits(0x4383999a), f32::from_bits(0x42bc12dc)),
        (f32::from_bits(0x4383f333), f32::from_bits(0x42bc1324)),
        (f32::from_bits(0x43844ccd), f32::from_bits(0x42bc136c)),
    ); // 263.2f, 94.0368f, 263.9f, 94.0374f, 264.6f, 94.0379f
    path.cubic_to(
        (f32::from_bits(0x4384a666), f32::from_bits(0x42bc13b5)),
        (f32::from_bits(0x43850000), f32::from_bits(0x42bc148e)),
        (f32::from_bits(0x4385599a), f32::from_bits(0x42bc151e)),
    ); // 265.3f, 94.0385f, 266, 94.0401f, 266.7f, 94.0412f
    path.cubic_to(
        (f32::from_bits(0x4385b333), f32::from_bits(0x42bc15af)),
        (f32::from_bits(0x43860ccd), f32::from_bits(0x42bc1688)),
        (f32::from_bits(0x43866666), f32::from_bits(0x42bc16d0)),
    ); // 267.4f, 94.0424f, 268.1f, 94.044f, 268.8f, 94.0446f
    path.cubic_to(
        (f32::from_bits(0x4386c000), f32::from_bits(0x42bc1719)),
        (f32::from_bits(0x4387199a), f32::from_bits(0x42bc1719)),
        (f32::from_bits(0x43877333), f32::from_bits(0x42bc16d0)),
    ); // 269.5f, 94.0451f, 270.2f, 94.0451f, 270.9f, 94.0446f
    path.cubic_to(
        (f32::from_bits(0x4387cccd), f32::from_bits(0x42bc1688)),
        (f32::from_bits(0x43882666), f32::from_bits(0x42bc1567)),
        (f32::from_bits(0x43888000), f32::from_bits(0x42bc151e)),
    ); // 271.6f, 94.044f, 272.3f, 94.0418f, 273, 94.0412f
    path.cubic_to(
        (f32::from_bits(0x4388d99a), f32::from_bits(0x42bc14d6)),
        (f32::from_bits(0x43893333), f32::from_bits(0x42bc151e)),
        (f32::from_bits(0x43898ccd), f32::from_bits(0x42bc151e)),
    ); // 273.7f, 94.0407f, 274.4f, 94.0412f, 275.1f, 94.0412f
    path.cubic_to(
        (f32::from_bits(0x4389e666), f32::from_bits(0x42bc151e)),
        (f32::from_bits(0x438a4000), f32::from_bits(0x42bc1567)),
        (f32::from_bits(0x438a999a), f32::from_bits(0x42bc151e)),
    ); // 275.8f, 94.0412f, 276.5f, 94.0418f, 277.2f, 94.0412f
    path.cubic_to(
        (f32::from_bits(0x438af333), f32::from_bits(0x42bc14d6)),
        (f32::from_bits(0x438b4ccd), f32::from_bits(0x42bc136c)),
        (f32::from_bits(0x438ba666), f32::from_bits(0x42bc136c)),
    ); // 277.9f, 94.0407f, 278.6f, 94.0379f, 279.3f, 94.0379f
    path.cubic_to(
        (f32::from_bits(0x438c0000), f32::from_bits(0x42bc136c)),
        (f32::from_bits(0x438c599a), f32::from_bits(0x42bc152a)),
        (f32::from_bits(0x438cb333), f32::from_bits(0x42bc151e)),
    ); // 280, 94.0379f, 280.7f, 94.0413f, 281.4f, 94.0412f
    path.cubic_to(
        (f32::from_bits(0x438d0ccd), f32::from_bits(0x42bc1513)),
        (f32::from_bits(0x438d6666), f32::from_bits(0x42bc14ef)),
        (f32::from_bits(0x438dc000), f32::from_bits(0x42bc1326)),
    ); // 282.1f, 94.0412f, 282.8f, 94.0409f, 283.5f, 94.0374f
    path.cubic_to(
        (f32::from_bits(0x438e199a), f32::from_bits(0x42bc115c)),
        (f32::from_bits(0x438e7333), f32::from_bits(0x42bc0c17)),
        (f32::from_bits(0x438ecccd), f32::from_bits(0x42bc0a64)),
    ); // 284.2f, 94.0339f, 284.9f, 94.0236f, 285.6f, 94.0203f
    path.cubic_to(
        (f32::from_bits(0x438f2666), f32::from_bits(0x42bc08b2)),
        (f32::from_bits(0x438f8000), f32::from_bits(0x42bc0413)),
        (f32::from_bits(0x438fd99a), f32::from_bits(0x42bc08f9)),
    ); // 286.3f, 94.017f, 287, 94.008f, 287.7f, 94.0175f
    path.cubic_to(
        (f32::from_bits(0x43903333), f32::from_bits(0x42bc0dde)),
        (f32::from_bits(0x43908ccd), f32::from_bits(0x42bc1476)),
        (f32::from_bits(0x4390e666), f32::from_bits(0x42bc27c6)),
    ); // 288.4f, 94.0271f, 289.1f, 94.04f, 289.8f, 94.0777f
    path.cubic_to(
        (f32::from_bits(0x43914000), f32::from_bits(0x42bc3b15)),
        (f32::from_bits(0x4391999a), f32::from_bits(0x42bc5916)),
        (f32::from_bits(0x4391f333), f32::from_bits(0x42bc7cd6)),
    ); // 290.5f, 94.1154f, 291.2f, 94.174f, 291.9f, 94.2438f
    path.cubic_to(
        (f32::from_bits(0x43924ccd), f32::from_bits(0x42bca096)),
        (f32::from_bits(0x4392a666), f32::from_bits(0x42bce868)),
        (f32::from_bits(0x43930000), f32::from_bits(0x42bcfe45)),
    ); // 292.6f, 94.3136f, 293.3f, 94.4539f, 294, 94.4966f
    path.cubic_to(
        (f32::from_bits(0x4393599a), f32::from_bits(0x42bd1421)),
        (f32::from_bits(0x4393b333), f32::from_bits(0x42bcffb6)),
        (f32::from_bits(0x43940ccd), f32::from_bits(0x42bd0000)),
    ); // 294.7f, 94.5393f, 295.4f, 94.4994f, 296.1f, 94.5f
    path.line_to((f32::from_bits(0x43940ccd), f32::from_bits(0x42bd0000))); // 296.1f, 94.5f
    path.line_to((f32::from_bits(0x43413333), f32::from_bits(0x42bd0000))); // 193.2f, 94.5f
    path.close();
    path.move_to((f32::from_bits(0x43ac3333), f32::from_bits(0x42bd0000))); // 344.4f, 94.5f
    path.line_to((f32::from_bits(0x43ac3333), f32::from_bits(0x42bd0000))); // 344.4f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x43ac8ccd), f32::from_bits(0x42bd0000)),
        (f32::from_bits(0x43ace666), f32::from_bits(0x42bd03a2)),
        (f32::from_bits(0x43ad4000), f32::from_bits(0x42bd0000)),
    ); // 345.1f, 94.5f, 345.8f, 94.5071f, 346.5f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x43ad999a), f32::from_bits(0x42bcfc5e)),
        (f32::from_bits(0x43adf333), f32::from_bits(0x42bd069a)),
        (f32::from_bits(0x43ae4ccd), f32::from_bits(0x42bcea32)),
    ); // 347.2f, 94.4929f, 347.9f, 94.5129f, 348.6f, 94.4574f
    path.cubic_to(
        (f32::from_bits(0x43aea666), f32::from_bits(0x42bccdca)),
        (f32::from_bits(0x43af0000), f32::from_bits(0x42bc7b18)),
        (f32::from_bits(0x43af599a), f32::from_bits(0x42bc558f)),
    ); // 349.3f, 94.4019f, 350, 94.2404f, 350.7f, 94.1671f
    path.cubic_to(
        (f32::from_bits(0x43afb333), f32::from_bits(0x42bc3005)),
        (f32::from_bits(0x43b00ccd), f32::from_bits(0x42bc16d2)),
        (f32::from_bits(0x43b06666), f32::from_bits(0x42bc08f9)),
    ); // 351.4f, 94.0938f, 352.1f, 94.0446f, 352.8f, 94.0175f
    path.cubic_to(
        (f32::from_bits(0x43b0c000), f32::from_bits(0x42bbfb20)),
        (f32::from_bits(0x43b1199a), f32::from_bits(0x42bc03d5)),
        (f32::from_bits(0x43b17333), f32::from_bits(0x42bc0277)),
    ); // 353.5f, 93.9905f, 354.2f, 94.0075f, 354.9f, 94.0048f
    path.cubic_to(
        (f32::from_bits(0x43b1cccd), f32::from_bits(0x42bc0119)),
        (f32::from_bits(0x43b22666), f32::from_bits(0x42bc019e)),
        (f32::from_bits(0x43b28000), f32::from_bits(0x42bc00c5)),
    ); // 355.6f, 94.0021f, 356.3f, 94.0032f, 357, 94.0015f
    path.cubic_to(
        (f32::from_bits(0x43b2d99a), f32::from_bits(0x42bbffec)),
        (f32::from_bits(0x43b33333), f32::from_bits(0x42bbfdf1)),
        (f32::from_bits(0x43b38ccd), f32::from_bits(0x42bbfd61)),
    ); // 357.7f, 93.9998f, 358.4f, 93.996f, 359.1f, 93.9949f
    path.cubic_to(
        (f32::from_bits(0x43b3e666), f32::from_bits(0x42bbfcd0)),
        (f32::from_bits(0x43b44000), f32::from_bits(0x42bbfdf1)),
        (f32::from_bits(0x43b4999a), f32::from_bits(0x42bbfd61)),
    ); // 359.8f, 93.9938f, 360.5f, 93.996f, 361.2f, 93.9949f
    path.cubic_to(
        (f32::from_bits(0x43b4f333), f32::from_bits(0x42bbfcd0)),
        (f32::from_bits(0x43b54ccd), f32::from_bits(0x42bbfad6)),
        (f32::from_bits(0x43b5a666), f32::from_bits(0x42bbf9fc)),
    ); // 361.9f, 93.9938f, 362.6f, 93.9899f, 363.3f, 93.9883f
    path.cubic_to(
        (f32::from_bits(0x43b60000), f32::from_bits(0x42bbf923)),
        (f32::from_bits(0x43b6599a), f32::from_bits(0x42bbf8db)),
        (f32::from_bits(0x43b6b333), f32::from_bits(0x42bbf84a)),
    ); // 364, 93.9866f, 364.7f, 93.986f, 365.4f, 93.9849f
    path.cubic_to(
        (f32::from_bits(0x43b70ccd), f32::from_bits(0x42bbf7ba)),
        (f32::from_bits(0x43b76666), f32::from_bits(0x42bbf729)),
        (f32::from_bits(0x43b7c000), f32::from_bits(0x42bbf698)),
    ); // 366.1f, 93.9838f, 366.8f, 93.9827f, 367.5f, 93.9816f
    path.cubic_to(
        (f32::from_bits(0x43b8199a), f32::from_bits(0x42bbf608)),
        (f32::from_bits(0x43b87333), f32::from_bits(0x42bbf52e)),
        (f32::from_bits(0x43b8cccd), f32::from_bits(0x42bbf4e6)),
    ); // 368.2f, 93.9805f, 368.9f, 93.9789f, 369.6f, 93.9783f
    path.cubic_to(
        (f32::from_bits(0x43b92666), f32::from_bits(0x42bbf49e)),
        (f32::from_bits(0x43b98000), f32::from_bits(0x42bbf455)),
        (f32::from_bits(0x43b9d99a), f32::from_bits(0x42bbf4e6)),
    ); // 370.3f, 93.9778f, 371, 93.9772f, 371.7f, 93.9783f
    path.cubic_to(
        (f32::from_bits(0x43ba3333), f32::from_bits(0x42bbf577)),
        (f32::from_bits(0x43ba8ccd), f32::from_bits(0x42bbf771)),
        (f32::from_bits(0x43bae666), f32::from_bits(0x42bbf84a)),
    ); // 372.4f, 93.9794f, 373.1f, 93.9833f, 373.8f, 93.9849f
    path.cubic_to(
        (f32::from_bits(0x43bb4000), f32::from_bits(0x42bbf923)),
        (f32::from_bits(0x43bb999a), f32::from_bits(0x42bbf9b4)),
        (f32::from_bits(0x43bbf333), f32::from_bits(0x42bbf9fc)),
    ); // 374.5f, 93.9866f, 375.2f, 93.9877f, 375.9f, 93.9883f
    path.cubic_to(
        (f32::from_bits(0x43bc4ccd), f32::from_bits(0x42bbfa45)),
        (f32::from_bits(0x43bca666), f32::from_bits(0x42bbf9fc)),
        (f32::from_bits(0x43bd0000), f32::from_bits(0x42bbf9fc)),
    ); // 376.6f, 93.9888f, 377.3f, 93.9883f, 378, 93.9883f
    path.cubic_to(
        (f32::from_bits(0x43bd599a), f32::from_bits(0x42bbf9fc)),
        (f32::from_bits(0x43bdb333), f32::from_bits(0x42bbf9fc)),
        (f32::from_bits(0x43be0ccd), f32::from_bits(0x42bbf9fc)),
    ); // 378.7f, 93.9883f, 379.4f, 93.9883f, 380.1f, 93.9883f
    path.cubic_to(
        (f32::from_bits(0x43be6666), f32::from_bits(0x42bbf9fc)),
        (f32::from_bits(0x43bec000), f32::from_bits(0x42bbf8db)),
        (f32::from_bits(0x43bf199a), f32::from_bits(0x42bbf9fc)),
    ); // 380.8f, 93.9883f, 381.5f, 93.986f, 382.2f, 93.9883f
    path.cubic_to(
        (f32::from_bits(0x43bf7333), f32::from_bits(0x42bbfb1e)),
        (f32::from_bits(0x43bfcccd), f32::from_bits(0x42bbfeca)),
        (f32::from_bits(0x43c02666), f32::from_bits(0x42bc00c5)),
    ); // 382.9f, 93.9905f, 383.6f, 93.9976f, 384.3f, 94.0015f
    path.cubic_to(
        (f32::from_bits(0x43c08000), f32::from_bits(0x42bc02bf)),
        (f32::from_bits(0x43c0d99a), f32::from_bits(0x42bc0593)),
        (f32::from_bits(0x43c13333), f32::from_bits(0x42bc05db)),
    ); // 385, 94.0054f, 385.7f, 94.0109f, 386.4f, 94.0114f
    path.cubic_to(
        (f32::from_bits(0x43c18ccd), f32::from_bits(0x42bc0624)),
        (f32::from_bits(0x43c1e666), f32::from_bits(0x42bc0308)),
        (f32::from_bits(0x43c24000), f32::from_bits(0x42bc0277)),
    ); // 387.1f, 94.012f, 387.8f, 94.0059f, 388.5f, 94.0048f
    path.cubic_to(
        (f32::from_bits(0x43c2999a), f32::from_bits(0x42bc01e6)),
        (f32::from_bits(0x43c2f333), f32::from_bits(0x42bc022f)),
        (f32::from_bits(0x43c34ccd), f32::from_bits(0x42bc0277)),
    ); // 389.2f, 94.0037f, 389.9f, 94.0043f, 390.6f, 94.0048f
    path.cubic_to(
        (f32::from_bits(0x43c3a666), f32::from_bits(0x42bc02bf)),
        (f32::from_bits(0x43c40000), f32::from_bits(0x42bc02bf)),
        (f32::from_bits(0x43c4599a), f32::from_bits(0x42bc0429)),
    ); // 391.3f, 94.0054f, 392, 94.0054f, 392.7f, 94.0081f
    path.cubic_to(
        (f32::from_bits(0x43c4b333), f32::from_bits(0x42bc0593)),
        (f32::from_bits(0x43c50ccd), f32::from_bits(0x42bc08f7)),
        (f32::from_bits(0x43c56666), f32::from_bits(0x42bc0af2)),
    ); // 393.4f, 94.0109f, 394.1f, 94.0175f, 394.8f, 94.0214f
    path.cubic_to(
        (f32::from_bits(0x43c5c000), f32::from_bits(0x42bc0cec)),
        (f32::from_bits(0x43c6199a), f32::from_bits(0x42bc0f2f)),
        (f32::from_bits(0x43c67333), f32::from_bits(0x42bc1008)),
    ); // 395.5f, 94.0252f, 396.2f, 94.0297f, 396.9f, 94.0313f
    path.cubic_to(
        (f32::from_bits(0x43c6cccd), f32::from_bits(0x42bc10e1)),
        (f32::from_bits(0x43c72666), f32::from_bits(0x42bc0fc0)),
        (f32::from_bits(0x43c78000), f32::from_bits(0x42bc1008)),
    ); // 397.6f, 94.033f, 398.3f, 94.0308f, 399, 94.0313f
    path.cubic_to(
        (f32::from_bits(0x43c7d99a), f32::from_bits(0x42bc1050)),
        (f32::from_bits(0x43c83333), f32::from_bits(0x42bc1172)),
        (f32::from_bits(0x43c88ccd), f32::from_bits(0x42bc11ba)),
    ); // 399.7f, 94.0319f, 400.4f, 94.0341f, 401.1f, 94.0346f
    path.cubic_to(
        (f32::from_bits(0x43c8e666), f32::from_bits(0x42bc1202)),
        (f32::from_bits(0x43c94000), f32::from_bits(0x42bc11ba)),
        (f32::from_bits(0x43c9999a), f32::from_bits(0x42bc11ba)),
    ); // 401.8f, 94.0352f, 402.5f, 94.0346f, 403.2f, 94.0346f
    path.cubic_to(
        (f32::from_bits(0x43c9f333), f32::from_bits(0x42bc11ba)),
        (f32::from_bits(0x43ca4ccd), f32::from_bits(0x42bc124b)),
        (f32::from_bits(0x43caa666), f32::from_bits(0x42bc11ba)),
    ); // 403.9f, 94.0346f, 404.6f, 94.0357f, 405.3f, 94.0346f
    path.cubic_to(
        (f32::from_bits(0x43cb0000), f32::from_bits(0x42bc1129)),
        (f32::from_bits(0x43cb599a), f32::from_bits(0x42bc0f77)),
        (f32::from_bits(0x43cbb333), f32::from_bits(0x42bc0e56)),
    ); // 406, 94.0335f, 406.7f, 94.0302f, 407.4f, 94.028f
    path.cubic_to(
        (f32::from_bits(0x43cc0ccd), f32::from_bits(0x42bc0d34)),
        (f32::from_bits(0x43cc6666), f32::from_bits(0x42bc0b82)),
        (f32::from_bits(0x43ccc000), f32::from_bits(0x42bc0af2)),
    ); // 408.1f, 94.0258f, 408.8f, 94.0225f, 409.5f, 94.0214f
    path.cubic_to(
        (f32::from_bits(0x43cd199a), f32::from_bits(0x42bc0a61)),
        (f32::from_bits(0x43cd7333), f32::from_bits(0x42bc0b3a)),
        (f32::from_bits(0x43cdcccd), f32::from_bits(0x42bc0af2)),
    ); // 410.2f, 94.0203f, 410.9f, 94.0219f, 411.6f, 94.0214f
    path.cubic_to(
        (f32::from_bits(0x43ce2666), f32::from_bits(0x42bc0aa9)),
        (f32::from_bits(0x43ce8000), f32::from_bits(0x42bc0aa9)),
        (f32::from_bits(0x43ced99a), f32::from_bits(0x42bc0940)),
    ); // 412.3f, 94.0208f, 413, 94.0208f, 413.7f, 94.0181f
    path.cubic_to(
        (f32::from_bits(0x43cf3333), f32::from_bits(0x42bc07d6)),
        (f32::from_bits(0x43cf8ccd), f32::from_bits(0x42bc0502)),
        (f32::from_bits(0x43cfe666), f32::from_bits(0x42bc0277)),
    ); // 414.4f, 94.0153f, 415.1f, 94.0098f, 415.8f, 94.0048f
    path.cubic_to(
        (f32::from_bits(0x43d04000), f32::from_bits(0x42bbffec)),
        (f32::from_bits(0x43d0999a), f32::from_bits(0x42bbfc88)),
        (f32::from_bits(0x43d0f333), f32::from_bits(0x42bbf9fc)),
    ); // 416.5f, 93.9998f, 417.2f, 93.9932f, 417.9f, 93.9883f
    path.cubic_to(
        (f32::from_bits(0x43d14ccd), f32::from_bits(0x42bbf771)),
        (f32::from_bits(0x43d1a666), f32::from_bits(0x42bbf4e6)),
        (f32::from_bits(0x43d20000), f32::from_bits(0x42bbf334)),
    ); // 418.6f, 93.9833f, 419.3f, 93.9783f, 420, 93.975f
    path.cubic_to(
        (f32::from_bits(0x43d2599a), f32::from_bits(0x42bbf182)),
        (f32::from_bits(0x43d2b333), f32::from_bits(0x42bbee66)),
        (f32::from_bits(0x43d30ccd), f32::from_bits(0x42bbefd0)),
    ); // 420.7f, 93.9717f, 421.4f, 93.9656f, 422.1f, 93.9684f
    path.cubic_to(
        (f32::from_bits(0x43d36666), f32::from_bits(0x42bbf13a)),
        (f32::from_bits(0x43d3c000), f32::from_bits(0x42bbf52e)),
        (f32::from_bits(0x43d4199a), f32::from_bits(0x42bbfbaf)),
    ); // 422.8f, 93.9711f, 423.5f, 93.9789f, 424.2f, 93.9916f
    path.cubic_to(
        (f32::from_bits(0x43d47333), f32::from_bits(0x42bc022f)),
        (f32::from_bits(0x43d4cccd), f32::from_bits(0x42bc1014)),
        (f32::from_bits(0x43d52666), f32::from_bits(0x42bc16d0)),
    ); // 424.9f, 94.0043f, 425.6f, 94.0314f, 426.3f, 94.0446f
    path.cubic_to(
        (f32::from_bits(0x43d58000), f32::from_bits(0x42bc1d8d)),
        (f32::from_bits(0x43d5d99a), f32::from_bits(0x42bc1de3)),
        (f32::from_bits(0x43d63333), f32::from_bits(0x42bc241b)),
    ); // 427, 94.0577f, 427.7f, 94.0584f, 428.4f, 94.0705f
    path.cubic_to(
        (f32::from_bits(0x43d68ccd), f32::from_bits(0x42bc2a53)),
        (f32::from_bits(0x43d6e666), f32::from_bits(0x42bc30c5)),
        (f32::from_bits(0x43d74000), f32::from_bits(0x42bc3c1f)),
    ); // 429.1f, 94.0827f, 429.8f, 94.0953f, 430.5f, 94.1174f
    path.cubic_to(
        (f32::from_bits(0x43d7999a), f32::from_bits(0x42bc4779)),
        (f32::from_bits(0x43d7f333), f32::from_bits(0x42bc6283)),
        (f32::from_bits(0x43d84ccd), f32::from_bits(0x42bc6836)),
    ); // 431.2f, 94.1396f, 431.9f, 94.1924f, 432.6f, 94.2035f
    path.cubic_to(
        (f32::from_bits(0x43d8a666), f32::from_bits(0x42bc6de9)),
        (f32::from_bits(0x43d90000), f32::from_bits(0x42bc67e0)),
        (f32::from_bits(0x43d9599a), f32::from_bits(0x42bc5e50)),
    ); // 433.3f, 94.2147f, 434, 94.2029f, 434.7f, 94.1842f
    path.cubic_to(
        (f32::from_bits(0x43d9b333), f32::from_bits(0x42bc54c0)),
        (f32::from_bits(0x43da0ccd), f32::from_bits(0x42bc3a23)),
        (f32::from_bits(0x43da6666), f32::from_bits(0x42bc2ed5)),
    ); // 435.4f, 94.1655f, 436.1f, 94.1135f, 436.8f, 94.0915f
    path.cubic_to(
        (f32::from_bits(0x43dac000), f32::from_bits(0x42bc2387)),
        (f32::from_bits(0x43db199a), f32::from_bits(0x42bc1e34)),
        (f32::from_bits(0x43db7333), f32::from_bits(0x42bc1a7b)),
    ); // 437.5f, 94.0694f, 438.2f, 94.059f, 438.9f, 94.0517f
    path.cubic_to(
        (f32::from_bits(0x43dbcccd), f32::from_bits(0x42bc16c3)),
        (f32::from_bits(0x43dc2666), f32::from_bits(0x42bc18a6)),
        (f32::from_bits(0x43dc8000), f32::from_bits(0x42bc1883)),
    ); // 439.6f, 94.0445f, 440.3f, 94.0481f, 441, 94.0479f
    path.cubic_to(
        (f32::from_bits(0x43dcd99a), f32::from_bits(0x42bc185f)),
        (f32::from_bits(0x43dd3333), f32::from_bits(0x42bc19f0)),
        (f32::from_bits(0x43dd8ccd), f32::from_bits(0x42bc19a8)),
    ); // 441.7f, 94.0476f, 442.4f, 94.0507f, 443.1f, 94.0501f
    path.cubic_to(
        (f32::from_bits(0x43dde666), f32::from_bits(0x42bc195f)),
        (f32::from_bits(0x43de4000), f32::from_bits(0x42bc15d4)),
        (f32::from_bits(0x43de999a), f32::from_bits(0x42bc16d0)),
    ); // 443.8f, 94.0496f, 444.5f, 94.0426f, 445.2f, 94.0446f
    path.cubic_to(
        (f32::from_bits(0x43def333), f32::from_bits(0x42bc17cd)),
        (f32::from_bits(0x43df4ccd), f32::from_bits(0x42bc11a1)),
        (f32::from_bits(0x43dfa666), f32::from_bits(0x42bc1f92)),
    ); // 445.9f, 94.0465f, 446.6f, 94.0344f, 447.3f, 94.0617f
    path.cubic_to(
        (f32::from_bits(0x43e00000), f32::from_bits(0x42bc2d83)),
        (f32::from_bits(0x43e0599a), f32::from_bits(0x42bc478f)),
        (f32::from_bits(0x43e0b333), f32::from_bits(0x42bc6a76)),
    ); // 448, 94.0889f, 448.7f, 94.1398f, 449.4f, 94.2079f
    path.cubic_to(
        (f32::from_bits(0x43e10ccd), f32::from_bits(0x42bc8d5c)),
        (f32::from_bits(0x43e16666), f32::from_bits(0x42bcd80e)),
        (f32::from_bits(0x43e1c000), f32::from_bits(0x42bcf0fa)),
    ); // 450.1f, 94.2761f, 450.8f, 94.422f, 451.5f, 94.4707f
    path.cubic_to(
        (f32::from_bits(0x43e2199a), f32::from_bits(0x42bd09e7)),
        (f32::from_bits(0x43e27333), f32::from_bits(0x42bd0449)),
        (f32::from_bits(0x43e2cccd), f32::from_bits(0x42bd0000)),
    ); // 452.2f, 94.5193f, 452.9f, 94.5084f, 453.6f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x43e32666), f32::from_bits(0x42bcfbb7)),
        (f32::from_bits(0x43e38000), f32::from_bits(0x42bcebe7)),
        (f32::from_bits(0x43e3d99a), f32::from_bits(0x42bcd744)),
    ); // 454.3f, 94.4916f, 455, 94.4607f, 455.7f, 94.4204f
    path.cubic_to(
        (f32::from_bits(0x43e43333), f32::from_bits(0x42bcc2a0)),
        (f32::from_bits(0x43e48ccd), f32::from_bits(0x42bc8dc8)),
        (f32::from_bits(0x43e4e666), f32::from_bits(0x42bc842c)),
    ); // 456.4f, 94.3801f, 457.1f, 94.2769f, 457.8f, 94.2581f
    path.cubic_to(
        (f32::from_bits(0x43e54000), f32::from_bits(0x42bc7a90)),
        (f32::from_bits(0x43e5999a), f32::from_bits(0x42bc8f32)),
        (f32::from_bits(0x43e5f333), f32::from_bits(0x42bc9d9c)),
    ); // 458.5f, 94.2394f, 459.2f, 94.2797f, 459.9f, 94.3078f
    path.cubic_to(
        (f32::from_bits(0x43e64ccd), f32::from_bits(0x42bcac06)),
        (f32::from_bits(0x43e6a666), f32::from_bits(0x42bcca42)),
        (f32::from_bits(0x43e70000), f32::from_bits(0x42bcdaa8)),
    ); // 460.6f, 94.336f, 461.3f, 94.395f, 462, 94.4271f
    path.cubic_to(
        (f32::from_bits(0x43e7599a), f32::from_bits(0x42bceb0e)),
        (f32::from_bits(0x43e7b333), f32::from_bits(0x42bcf9c7)),
        (f32::from_bits(0x43e80ccd), f32::from_bits(0x42bd0000)),
    ); // 462.7f, 94.4591f, 463.4f, 94.4878f, 464.1f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x43e86666), f32::from_bits(0x42bd0639)),
        (f32::from_bits(0x43e8c000), f32::from_bits(0x42bd010b)),
        (f32::from_bits(0x43e9199a), f32::from_bits(0x42bd0000)),
    ); // 464.8f, 94.5122f, 465.5f, 94.502f, 466.2f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x43e97333), f32::from_bits(0x42bcfef5)),
        (f32::from_bits(0x43e9cccd), f32::from_bits(0x42bcf9bb)),
        (f32::from_bits(0x43ea2666), f32::from_bits(0x42bcf9bb)),
    ); // 466.9f, 94.498f, 467.6f, 94.4878f, 468.3f, 94.4878f
    path.cubic_to(
        (f32::from_bits(0x43ea8000), f32::from_bits(0x42bcf9bb)),
        (f32::from_bits(0x43ead99a), f32::from_bits(0x42bcfef5)),
        (f32::from_bits(0x43eb3333), f32::from_bits(0x42bd0000)),
    ); // 469, 94.4878f, 469.7f, 94.498f, 470.4f, 94.5f
    path.line_to((f32::from_bits(0x43eb3333), f32::from_bits(0x42bd0000))); // 470.4f, 94.5f
    path.line_to((f32::from_bits(0x43ac3333), f32::from_bits(0x42bd0000))); // 344.4f, 94.5f
    path.close();
    path.detach()
}

// Port of: tests/PathOpsIssue3651.cpp#L659-L828 (chrome/m156)
fn path4() -> Path {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x42b06666), f32::from_bits(0x42bd0000))); // 88.2f, 94.5f
    path.line_to((f32::from_bits(0x42b06666), f32::from_bits(0x42bd0000))); // 88.2f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x42b1cccd), f32::from_bits(0x42bd0000)),
        (f32::from_bits(0x42b33333), f32::from_bits(0x42bcda8d)),
        (f32::from_bits(0x42b4999a), f32::from_bits(0x42bd0000)),
    ); // 88.9f, 94.5f, 89.6f, 94.4269f, 90.3f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x42b60000), f32::from_bits(0x42bd2573)),
        (f32::from_bits(0x42b76666), f32::from_bits(0x42bdba68)),
        (f32::from_bits(0x42b8cccd), f32::from_bits(0x42bde0b5)),
    ); // 91, 94.5731f, 91.7f, 94.8641f, 92.4f, 94.9389f
    path.cubic_to(
        (f32::from_bits(0x42ba3333), f32::from_bits(0x42be0701)),
        (f32::from_bits(0x42bb999a), f32::from_bits(0x42bde462)),
        (f32::from_bits(0x42bd0000), f32::from_bits(0x42bde5cb)),
    ); // 93.1f, 95.0137f, 93.8f, 94.9461f, 94.5f, 94.9488f
    path.cubic_to(
        (f32::from_bits(0x42be6666), f32::from_bits(0x42bde735)),
        (f32::from_bits(0x42bfcccd), f32::from_bits(0x42bde80e)),
        (f32::from_bits(0x42c13333), f32::from_bits(0x42bde930)),
    ); // 95.2f, 94.9516f, 95.9f, 94.9532f, 96.6f, 94.9554f
    path.cubic_to(
        (f32::from_bits(0x42c2999a), f32::from_bits(0x42bdea51)),
        (f32::from_bits(0x42c40000), f32::from_bits(0x42bdec03)),
        (f32::from_bits(0x42c56666), f32::from_bits(0x42bdec94)),
    ); // 97.3f, 94.9576f, 98, 94.961f, 98.7f, 94.9621f
    path.cubic_to(
        (f32::from_bits(0x42c6cccd), f32::from_bits(0x42bded24)),
        (f32::from_bits(0x42c83333), f32::from_bits(0x42bded19)),
        (f32::from_bits(0x42c9999a), f32::from_bits(0x42bdec94)),
    ); // 99.4f, 94.9632f, 100.1f, 94.9631f, 100.8f, 94.9621f
    path.cubic_to(
        (f32::from_bits(0x42cb0000), f32::from_bits(0x42bdec0f)),
        (f32::from_bits(0x42cc6666), f32::from_bits(0x42bdea38)),
        (f32::from_bits(0x42cdcccd), f32::from_bits(0x42bde976)),
    ); // 101.5f, 94.9611f, 102.2f, 94.9575f, 102.9f, 94.956f
    path.cubic_to(
        (f32::from_bits(0x42cf3333), f32::from_bits(0x42bde8b5)),
        (f32::from_bits(0x42d0999a), f32::from_bits(0x42bde853)),
        (f32::from_bits(0x42d20000), f32::from_bits(0x42bde80b)),
    ); // 103.6f, 94.9545f, 104.3f, 94.9538f, 105, 94.9532f
    path.cubic_to(
        (f32::from_bits(0x42d36666), f32::from_bits(0x42bde7c2)),
        (f32::from_bits(0x42d4cccd), f32::from_bits(0x42bde9d6)),
        (f32::from_bits(0x42d63333), f32::from_bits(0x42bde7c4)),
    ); // 105.7f, 94.9527f, 106.4f, 94.9567f, 107.1f, 94.9527f
    path.cubic_to(
        (f32::from_bits(0x42d7999a), f32::from_bits(0x42bde5b2)),
        (f32::from_bits(0x42d90000), f32::from_bits(0x42bde19a)),
        (f32::from_bits(0x42da6666), f32::from_bits(0x42bddb9f)),
    ); // 107.8f, 94.9486f, 108.5f, 94.9406f, 109.2f, 94.9289f
    path.cubic_to(
        (f32::from_bits(0x42dbcccd), f32::from_bits(0x42bdd5a3)),
        (f32::from_bits(0x42dd3333), f32::from_bits(0x42bdcc07)),
        (f32::from_bits(0x42de999a), f32::from_bits(0x42bdc3e1)),
    ); // 109.9f, 94.9173f, 110.6f, 94.8985f, 111.3f, 94.8826f
    path.cubic_to(
        (f32::from_bits(0x42e00000), f32::from_bits(0x42bdbbba)),
        (f32::from_bits(0x42e16666), f32::from_bits(0x42bdb332)),
        (f32::from_bits(0x42e2cccd), f32::from_bits(0x42bdaab8)),
    ); // 112, 94.8667f, 112.7f, 94.85f, 113.4f, 94.8334f
    path.cubic_to(
        (f32::from_bits(0x42e43333), f32::from_bits(0x42bda23d)),
        (f32::from_bits(0x42e5999a), f32::from_bits(0x42bd9b8e)),
        (f32::from_bits(0x42e70000), f32::from_bits(0x42bd9101)),
    ); // 114.1f, 94.8169f, 114.8f, 94.8038f, 115.5f, 94.7832f
    path.cubic_to(
        (f32::from_bits(0x42e86666), f32::from_bits(0x42bd8675)),
        (f32::from_bits(0x42e9cccd), f32::from_bits(0x42bd79f9)),
        (f32::from_bits(0x42eb3333), f32::from_bits(0x42bd6b6c)),
    ); // 116.2f, 94.7626f, 116.9f, 94.7382f, 117.6f, 94.7098f
    path.cubic_to(
        (f32::from_bits(0x42ec999a), f32::from_bits(0x42bd5cdf)),
        (f32::from_bits(0x42ee0000), f32::from_bits(0x42bd4608)),
        (f32::from_bits(0x42ef6666), f32::from_bits(0x42bd39b1)),
    ); // 118.3f, 94.6814f, 119, 94.6368f, 119.7f, 94.6127f
    path.cubic_to(
        (f32::from_bits(0x42f0cccd), f32::from_bits(0x42bd2d5b)),
        (f32::from_bits(0x42f23333), f32::from_bits(0x42bd24cb)),
        (f32::from_bits(0x42f3999a), f32::from_bits(0x42bd2166)),
    ); // 120.4f, 94.5886f, 121.1f, 94.5719f, 121.8f, 94.5652f
    path.cubic_to(
        (f32::from_bits(0x42f50000), f32::from_bits(0x42bd1e02)),
        (f32::from_bits(0x42f66666), f32::from_bits(0x42bd22c1)),
        (f32::from_bits(0x42f7cccd), f32::from_bits(0x42bd2558)),
    ); // 122.5f, 94.5586f, 123.2f, 94.5679f, 123.9f, 94.5729f
    path.cubic_to(
        (f32::from_bits(0x42f93333), f32::from_bits(0x42bd27ef)),
        (f32::from_bits(0x42fa999a), f32::from_bits(0x42bd2da3)),
        (f32::from_bits(0x42fc0000), f32::from_bits(0x42bd30f0)),
    ); // 124.6f, 94.578f, 125.3f, 94.5891f, 126, 94.5956f
    path.cubic_to(
        (f32::from_bits(0x42fd6666), f32::from_bits(0x42bd343d)),
        (f32::from_bits(0x42fecccd), f32::from_bits(0x42bd36a5)),
        (f32::from_bits(0x4300199a), f32::from_bits(0x42bd3924)),
    ); // 126.7f, 94.602f, 127.4f, 94.6067f, 128.1f, 94.6116f
    path.cubic_to(
        (f32::from_bits(0x4300cccd), f32::from_bits(0x42bd3ba4)),
        (f32::from_bits(0x43018000), f32::from_bits(0x42bd3f14)),
        (f32::from_bits(0x43023333), f32::from_bits(0x42bd3fed)),
    ); // 128.8f, 94.6165f, 129.5f, 94.6232f, 130.2f, 94.6249f
    path.cubic_to(
        (f32::from_bits(0x4302e666), f32::from_bits(0x42bd40c6)),
        (f32::from_bits(0x4303999a), f32::from_bits(0x42bd3f50)),
        (f32::from_bits(0x43044ccd), f32::from_bits(0x42bd3e3b)),
    ); // 130.9f, 94.6265f, 131.6f, 94.6237f, 132.3f, 94.6215f
    path.cubic_to(
        (f32::from_bits(0x43050000), f32::from_bits(0x42bd3d25)),
        (f32::from_bits(0x4305b333), f32::from_bits(0x42bd3a44)),
        (f32::from_bits(0x43066666), f32::from_bits(0x42bd396b)),
    ); // 133, 94.6194f, 133.7f, 94.6138f, 134.4f, 94.6121f
    path.cubic_to(
        (f32::from_bits(0x4307199a), f32::from_bits(0x42bd3892)),
        (f32::from_bits(0x4307cccd), f32::from_bits(0x42bd3978)),
        (f32::from_bits(0x43088000), f32::from_bits(0x42bd3924)),
    ); // 135.1f, 94.6105f, 135.8f, 94.6122f, 136.5f, 94.6116f
    path.cubic_to(
        (f32::from_bits(0x43093333), f32::from_bits(0x42bd38d0)),
        (f32::from_bits(0x4309e666), f32::from_bits(0x42bd3766)),
        (f32::from_bits(0x430a999a), f32::from_bits(0x42bd3772)),
    ); // 137.2f, 94.611f, 137.9f, 94.6082f, 138.6f, 94.6083f
    path.cubic_to(
        (f32::from_bits(0x430b4ccd), f32::from_bits(0x42bd377e)),
        (f32::from_bits(0x430c0000), f32::from_bits(0x42bd3892)),
        (f32::from_bits(0x430cb333), f32::from_bits(0x42bd396b)),
    ); // 139.3f, 94.6084f, 140, 94.6105f, 140.7f, 94.6121f
    path.cubic_to(
        (f32::from_bits(0x430d6666), f32::from_bits(0x42bd3a44)),
        (f32::from_bits(0x430e199a), f32::from_bits(0x42bd3bbb)),
        (f32::from_bits(0x430ecccd), f32::from_bits(0x42bd3c88)),
    ); // 141.4f, 94.6138f, 142.1f, 94.6167f, 142.8f, 94.6182f
    path.cubic_to(
        (f32::from_bits(0x430f8000), f32::from_bits(0x42bd3d56)),
        (f32::from_bits(0x43103333), f32::from_bits(0x42bd3cd1)),
        (f32::from_bits(0x4310e666), f32::from_bits(0x42bd3e3b)),
    ); // 143.5f, 94.6198f, 144.2f, 94.6188f, 144.9f, 94.6215f
    path.cubic_to(
        (f32::from_bits(0x4311999a), f32::from_bits(0x42bd3fa4)),
        (f32::from_bits(0x43124ccd), f32::from_bits(0x42bd4278)),
        (f32::from_bits(0x43130000), f32::from_bits(0x42bd4503)),
    ); // 145.6f, 94.6243f, 146.3f, 94.6298f, 147, 94.6348f
    path.cubic_to(
        (f32::from_bits(0x4313b333), f32::from_bits(0x42bd478e)),
        (f32::from_bits(0x43146666), f32::from_bits(0x42bd4af2)),
        (f32::from_bits(0x4315199a), f32::from_bits(0x42bd4d7e)),
    ); // 147.7f, 94.6398f, 148.4f, 94.6464f, 149.1f, 94.6514f
    path.cubic_to(
        (f32::from_bits(0x4315cccd), f32::from_bits(0x42bd5009)),
        (f32::from_bits(0x43168000), f32::from_bits(0x42bd53aa)),
        (f32::from_bits(0x43173333), f32::from_bits(0x42bd5446)),
    ); // 149.8f, 94.6563f, 150.5f, 94.6634f, 151.2f, 94.6646f
    path.cubic_to(
        (f32::from_bits(0x4317e666), f32::from_bits(0x42bd54e3)),
        (f32::from_bits(0x4318999a), f32::from_bits(0x42bd5202)),
        (f32::from_bits(0x43194ccd), f32::from_bits(0x42bd5128)),
    ); // 151.9f, 94.6658f, 152.6f, 94.6602f, 153.3f, 94.6585f
    path.cubic_to(
        (f32::from_bits(0x431a0000), f32::from_bits(0x42bd504f)),
        (f32::from_bits(0x431ab333), f32::from_bits(0x42bd4fcc)),
        (f32::from_bits(0x431b6666), f32::from_bits(0x42bd4f30)),
    ); // 154, 94.6569f, 154.7f, 94.6559f, 155.4f, 94.6547f
    path.cubic_to(
        (f32::from_bits(0x431c199a), f32::from_bits(0x42bd4e93)),
        (f32::from_bits(0x431ccccd), f32::from_bits(0x42bd4ee7)),
        (f32::from_bits(0x431d8000), f32::from_bits(0x42bd4d7e)),
    ); // 156.1f, 94.6535f, 156.8f, 94.6541f, 157.5f, 94.6514f
    path.cubic_to(
        (f32::from_bits(0x431e3333), f32::from_bits(0x42bd4c14)),
        (f32::from_bits(0x431ee666), f32::from_bits(0x42bd48f8)),
        (f32::from_bits(0x431f999a), f32::from_bits(0x42bd46b5)),
    ); // 158.2f, 94.6486f, 158.9f, 94.6425f, 159.6f, 94.6381f
    path.cubic_to(
        (f32::from_bits(0x43204ccd), f32::from_bits(0x42bd4472)),
        (f32::from_bits(0x43210000), f32::from_bits(0x42bd410e)),
        (f32::from_bits(0x4321b333), f32::from_bits(0x42bd3fed)),
    ); // 160.3f, 94.6337f, 161, 94.6271f, 161.7f, 94.6249f
    path.cubic_to(
        (f32::from_bits(0x43226666), f32::from_bits(0x42bd3ecb)),
        (f32::from_bits(0x4323199a), f32::from_bits(0x42bd401d)),
        (f32::from_bits(0x4323cccd), f32::from_bits(0x42bd3fed)),
    ); // 162.4f, 94.6226f, 163.1f, 94.6252f, 163.8f, 94.6249f
    path.cubic_to(
        (f32::from_bits(0x43248000), f32::from_bits(0x42bd3fbc)),
        (f32::from_bits(0x43253333), f32::from_bits(0x42bd40e7)),
        (f32::from_bits(0x4325e666), f32::from_bits(0x42bd3ec8)),
    ); // 164.5f, 94.6245f, 165.2f, 94.6268f, 165.9f, 94.6226f
    path.cubic_to(
        (f32::from_bits(0x4326999a), f32::from_bits(0x42bd3ca8)),
        (f32::from_bits(0x43274ccd), f32::from_bits(0x42bd383a)),
        (f32::from_bits(0x43280000), f32::from_bits(0x42bd332f)),
    ); // 166.6f, 94.6185f, 167.3f, 94.6098f, 168, 94.6f
    path.cubic_to(
        (f32::from_bits(0x4328b333), f32::from_bits(0x42bd2e25)),
        (f32::from_bits(0x43296666), f32::from_bits(0x42bd2647)),
        (f32::from_bits(0x432a199a), f32::from_bits(0x42bd2088)),
    ); // 168.7f, 94.5901f, 169.4f, 94.5748f, 170.1f, 94.5635f
    path.cubic_to(
        (f32::from_bits(0x432acccd), f32::from_bits(0x42bd1aca)),
        (f32::from_bits(0x432b8000), f32::from_bits(0x42bd147c)),
        (f32::from_bits(0x432c3333), f32::from_bits(0x42bd10b8)),
    ); // 170.8f, 94.5523f, 171.5f, 94.54f, 172.2f, 94.5327f
    path.cubic_to(
        (f32::from_bits(0x432ce666), f32::from_bits(0x42bd0cf4)),
        (f32::from_bits(0x432d999a), f32::from_bits(0x42bd0c32)),
        (f32::from_bits(0x432e4ccd), f32::from_bits(0x42bd09ef)),
    ); // 172.9f, 94.5253f, 173.6f, 94.5238f, 174.3f, 94.5194f
    path.cubic_to(
        (f32::from_bits(0x432f0000), f32::from_bits(0x42bd07ad)),
        (f32::from_bits(0x432fb333), f32::from_bits(0x42bd04cf)),
        (f32::from_bits(0x43306666), f32::from_bits(0x42bd0327)),
    ); // 175, 94.515f, 175.7f, 94.5094f, 176.4f, 94.5062f
    path.line_to((f32::from_bits(0x43306666), f32::from_bits(0x42bd0000))); // 176.4f, 94.5f
    path.line_to((f32::from_bits(0x42b06666), f32::from_bits(0x42bd0000))); // 88.2f, 94.5f
    path.close();
    path.move_to((f32::from_bits(0x43413333), f32::from_bits(0x42bd0000))); // 193.2f, 94.5f
    path.line_to((f32::from_bits(0x43413333), f32::from_bits(0x42bd0000))); // 193.2f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x4341e666), f32::from_bits(0x42bd0000)),
        (f32::from_bits(0x4342999a), f32::from_bits(0x42bcfab7)),
        (f32::from_bits(0x43434ccd), f32::from_bits(0x42bd0000)),
    ); // 193.9f, 94.5f, 194.6f, 94.4897f, 195.3f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x43440000), f32::from_bits(0x42bd0549)),
        (f32::from_bits(0x4344b333), f32::from_bits(0x42bd13e0)),
        (f32::from_bits(0x43456666), f32::from_bits(0x42bd1fb4)),
    ); // 196, 94.5103f, 196.7f, 94.5388f, 197.4f, 94.5619f
    path.cubic_to(
        (f32::from_bits(0x4346199a), f32::from_bits(0x42bd2b89)),
        (f32::from_bits(0x4346cccd), f32::from_bits(0x42bd3eb2)),
        (f32::from_bits(0x43478000), f32::from_bits(0x42bd46fc)),
    ); // 198.1f, 94.585f, 198.8f, 94.6225f, 199.5f, 94.6386f
    path.cubic_to(
        (f32::from_bits(0x43483333), f32::from_bits(0x42bd4f46)),
        (f32::from_bits(0x4348e666), f32::from_bits(0x42bd5127)),
        (f32::from_bits(0x4349999a), f32::from_bits(0x42bd516f)),
    ); // 200.2f, 94.6548f, 200.9f, 94.6585f, 201.6f, 94.659f
    path.cubic_to(
        (f32::from_bits(0x434a4ccd), f32::from_bits(0x42bd51b7)),
        (f32::from_bits(0x434b0000), f32::from_bits(0x42bd4dc4)),
        (f32::from_bits(0x434bb333), f32::from_bits(0x42bd48ae)),
    ); // 202.3f, 94.6596f, 203, 94.6519f, 203.7f, 94.642f
    path.cubic_to(
        (f32::from_bits(0x434c6666), f32::from_bits(0x42bd4397)),
        (f32::from_bits(0x434d199a), f32::from_bits(0x42bd39ee)),
        (f32::from_bits(0x434dcccd), f32::from_bits(0x42bd32e9)),
    ); // 204.4f, 94.632f, 205.1f, 94.6131f, 205.8f, 94.5994f
    path.cubic_to(
        (f32::from_bits(0x434e8000), f32::from_bits(0x42bd2be4)),
        (f32::from_bits(0x434f3333), f32::from_bits(0x42bd23a6)),
        (f32::from_bits(0x434fe666), f32::from_bits(0x42bd1e8f)),
    ); // 206.5f, 94.5857f, 207.2f, 94.5696f, 207.9f, 94.5597f
    path.cubic_to(
        (f32::from_bits(0x4350999a), f32::from_bits(0x42bd1979)),
        (f32::from_bits(0x43514ccd), f32::from_bits(0x42bd14f3)),
        (f32::from_bits(0x43520000), f32::from_bits(0x42bd1463)),
    ); // 208.6f, 94.5498f, 209.3f, 94.5409f, 210, 94.5398f
    path.cubic_to(
        (f32::from_bits(0x4352b333), f32::from_bits(0x42bd13d2)),
        (f32::from_bits(0x43536666), f32::from_bits(0x42bd1590)),
        (f32::from_bits(0x4354199a), f32::from_bits(0x42bd1b2b)),
    ); // 210.7f, 94.5387f, 211.4f, 94.5421f, 212.1f, 94.5531f
    path.cubic_to(
        (f32::from_bits(0x4354cccd), f32::from_bits(0x42bd20c7)),
        (f32::from_bits(0x43558000), f32::from_bits(0x42bd2bce)),
        (f32::from_bits(0x43563333), f32::from_bits(0x42bd3607)),
    ); // 212.8f, 94.564f, 213.5f, 94.5856f, 214.2f, 94.6055f
    path.cubic_to(
        (f32::from_bits(0x4356e666), f32::from_bits(0x42bd403f)),
        (f32::from_bits(0x4357999a), f32::from_bits(0x42bd4644)),
        (f32::from_bits(0x43584ccd), f32::from_bits(0x42bd587e)),
    ); // 214.9f, 94.6255f, 215.6f, 94.6372f, 216.3f, 94.6728f
    path.cubic_to(
        (f32::from_bits(0x43590000), f32::from_bits(0x42bd6ab8)),
        (f32::from_bits(0x4359b333), f32::from_bits(0x42bd8902)),
        (f32::from_bits(0x435a6666), f32::from_bits(0x42bda362)),
    ); // 217, 94.7084f, 217.7f, 94.7676f, 218.4f, 94.8191f
    path.cubic_to(
        (f32::from_bits(0x435b199a), f32::from_bits(0x42bdbdc2)),
        (f32::from_bits(0x435bcccd), f32::from_bits(0x42bde650)),
        (f32::from_bits(0x435c8000), f32::from_bits(0x42bdf6c0)),
    ); // 219.1f, 94.8706f, 219.8f, 94.9498f, 220.5f, 94.9819f
    path.cubic_to(
        (f32::from_bits(0x435d3333), f32::from_bits(0x42be0731)),
        (f32::from_bits(0x435de666), f32::from_bits(0x42be0409)),
        (f32::from_bits(0x435e999a), f32::from_bits(0x42be0604)),
    ); // 221.2f, 95.014f, 221.9f, 95.0079f, 222.6f, 95.0117f
    path.cubic_to(
        (f32::from_bits(0x435f4ccd), f32::from_bits(0x42be07fe)),
        (f32::from_bits(0x43600000), f32::from_bits(0x42be052a)),
        (f32::from_bits(0x4360b333), f32::from_bits(0x42be029f)),
    ); // 223.3f, 95.0156f, 224, 95.0101f, 224.7f, 95.0051f
    path.cubic_to(
        (f32::from_bits(0x43616666), f32::from_bits(0x42be0014)),
        (f32::from_bits(0x4362199a), f32::from_bits(0x42bdf903)),
        (f32::from_bits(0x4362cccd), f32::from_bits(0x42bdf6c0)),
    ); // 225.4f, 95.0002f, 226.1f, 94.9864f, 226.8f, 94.9819f
    path.cubic_to(
        (f32::from_bits(0x43638000), f32::from_bits(0x42bdf47e)),
        (f32::from_bits(0x43643333), f32::from_bits(0x42bdf3ed)),
        (f32::from_bits(0x4364e666), f32::from_bits(0x42bdf50e)),
    ); // 227.5f, 94.9775f, 228.2f, 94.9764f, 228.9f, 94.9786f
    path.cubic_to(
        (f32::from_bits(0x4365999a), f32::from_bits(0x42bdf630)),
        (f32::from_bits(0x43664ccd), f32::from_bits(0x42bdfe62)),
        (f32::from_bits(0x43670000), f32::from_bits(0x42bdfd89)),
    ); // 229.6f, 94.9808f, 230.3f, 94.9968f, 231, 94.9952f
    path.cubic_to(
        (f32::from_bits(0x4367b333), f32::from_bits(0x42bdfcb0)),
        (f32::from_bits(0x43686666), f32::from_bits(0x42bdf4c6)),
        (f32::from_bits(0x4369199a), f32::from_bits(0x42bdeff8)),
    ); // 231.7f, 94.9935f, 232.4f, 94.9781f, 233.1f, 94.9687f
    path.cubic_to(
        (f32::from_bits(0x4369cccd), f32::from_bits(0x42bdeb2a)),
        (f32::from_bits(0x436a8000), f32::from_bits(0x42bde2f8)),
        (f32::from_bits(0x436b3333), f32::from_bits(0x42bde0b5)),
    ); // 233.8f, 94.9593f, 234.5f, 94.9433f, 235.2f, 94.9389f
    path.cubic_to(
        (f32::from_bits(0x436be666), f32::from_bits(0x42bdde72)),
        (f32::from_bits(0x436c999a), f32::from_bits(0x42bde1d6)),
        (f32::from_bits(0x436d4ccd), f32::from_bits(0x42bde267)),
    ); // 235.9f, 94.9345f, 236.6f, 94.9411f, 237.3f, 94.9422f
    path.cubic_to(
        (f32::from_bits(0x436e0000), f32::from_bits(0x42bde2f8)),
        (f32::from_bits(0x436eb333), f32::from_bits(0x42bde340)),
        (f32::from_bits(0x436f6666), f32::from_bits(0x42bde419)),
    ); // 238, 94.9433f, 238.7f, 94.9438f, 239.4f, 94.9455f
    path.cubic_to(
        (f32::from_bits(0x4370199a), f32::from_bits(0x42bde4f2)),
        (f32::from_bits(0x4370cccd), f32::from_bits(0x42bde6a4)),
        (f32::from_bits(0x43718000), f32::from_bits(0x42bde77d)),
    ); // 240.1f, 94.9472f, 240.8f, 94.9505f, 241.5f, 94.9521f
    path.cubic_to(
        (f32::from_bits(0x43723333), f32::from_bits(0x42bde856)),
        (f32::from_bits(0x4372e666), f32::from_bits(0x42bde8e7)),
        (f32::from_bits(0x4373999a), f32::from_bits(0x42bde930)),
    ); // 242.2f, 94.9538f, 242.9f, 94.9549f, 243.6f, 94.9554f
    path.cubic_to(
        (f32::from_bits(0x43744ccd), f32::from_bits(0x42bde978)),
        (f32::from_bits(0x43750000), f32::from_bits(0x42bde8e7)),
        (f32::from_bits(0x4375b333), f32::from_bits(0x42bde930)),
    ); // 244.3f, 94.956f, 245, 94.9549f, 245.7f, 94.9554f
    path.cubic_to(
        (f32::from_bits(0x43766666), f32::from_bits(0x42bde978)),
        (f32::from_bits(0x4377199a), f32::from_bits(0x42bdea99)),
        (f32::from_bits(0x4377cccd), f32::from_bits(0x42bdeae2)),
    ); // 246.4f, 94.956f, 247.1f, 94.9582f, 247.8f, 94.9588f
    path.cubic_to(
        (f32::from_bits(0x43788000), f32::from_bits(0x42bdeb2a)),
        (f32::from_bits(0x43793333), f32::from_bits(0x42bdeb72)),
        (f32::from_bits(0x4379e666), f32::from_bits(0x42bdeae2)),
    ); // 248.5f, 94.9593f, 249.2f, 94.9599f, 249.9f, 94.9588f
    path.cubic_to(
        (f32::from_bits(0x437a999a), f32::from_bits(0x42bdea51)),
        (f32::from_bits(0x437b4ccd), f32::from_bits(0x42bde89f)),
        (f32::from_bits(0x437c0000), f32::from_bits(0x42bde77d)),
    ); // 250.6f, 94.9576f, 251.3f, 94.9543f, 252, 94.9521f
    path.cubic_to(
        (f32::from_bits(0x437cb333), f32::from_bits(0x42bde65c)),
        (f32::from_bits(0x437d6666), f32::from_bits(0x42bde4f2)),
        (f32::from_bits(0x437e199a), f32::from_bits(0x42bde419)),
    ); // 252.7f, 94.9499f, 253.4f, 94.9472f, 254.1f, 94.9455f
    path.cubic_to(
        (f32::from_bits(0x437ecccd), f32::from_bits(0x42bde340)),
        (f32::from_bits(0x437f8000), f32::from_bits(0x42bde267)),
        (f32::from_bits(0x4380199a), f32::from_bits(0x42bde267)),
    ); // 254.8f, 94.9438f, 255.5f, 94.9422f, 256.2f, 94.9422f
    path.cubic_to(
        (f32::from_bits(0x43807333), f32::from_bits(0x42bde267)),
        (f32::from_bits(0x4380cccd), f32::from_bits(0x42bde2f8)),
        (f32::from_bits(0x43812666), f32::from_bits(0x42bde419)),
    ); // 256.9f, 94.9422f, 257.6f, 94.9433f, 258.3f, 94.9455f
    path.cubic_to(
        (f32::from_bits(0x43818000), f32::from_bits(0x42bde53b)),
        (f32::from_bits(0x4381d99a), f32::from_bits(0x42bde7c6)),
        (f32::from_bits(0x43823333), f32::from_bits(0x42bde930)),
    ); // 259, 94.9477f, 259.7f, 94.9527f, 260.4f, 94.9554f
    path.cubic_to(
        (f32::from_bits(0x43828ccd), f32::from_bits(0x42bdea99)),
        (f32::from_bits(0x4382e666), f32::from_bits(0x42bdec03)),
        (f32::from_bits(0x43834000), f32::from_bits(0x42bdec94)),
    ); // 261.1f, 94.9582f, 261.8f, 94.961f, 262.5f, 94.9621f
    path.cubic_to(
        (f32::from_bits(0x4383999a), f32::from_bits(0x42bded24)),
        (f32::from_bits(0x4383f333), f32::from_bits(0x42bdecdc)),
        (f32::from_bits(0x43844ccd), f32::from_bits(0x42bdec94)),
    ); // 263.2f, 94.9632f, 263.9f, 94.9626f, 264.6f, 94.9621f
    path.cubic_to(
        (f32::from_bits(0x4384a666), f32::from_bits(0x42bdec4b)),
        (f32::from_bits(0x43850000), f32::from_bits(0x42bdeb72)),
        (f32::from_bits(0x4385599a), f32::from_bits(0x42bdeae2)),
    ); // 265.3f, 94.9615f, 266, 94.9599f, 266.7f, 94.9588f
    path.cubic_to(
        (f32::from_bits(0x4385b333), f32::from_bits(0x42bdea51)),
        (f32::from_bits(0x43860ccd), f32::from_bits(0x42bde978)),
        (f32::from_bits(0x43866666), f32::from_bits(0x42bde930)),
    ); // 267.4f, 94.9576f, 268.1f, 94.956f, 268.8f, 94.9554f
    path.cubic_to(
        (f32::from_bits(0x4386c000), f32::from_bits(0x42bde8e7)),
        (f32::from_bits(0x4387199a), f32::from_bits(0x42bde8e7)),
        (f32::from_bits(0x43877333), f32::from_bits(0x42bde930)),
    ); // 269.5f, 94.9549f, 270.2f, 94.9549f, 270.9f, 94.9554f
    path.cubic_to(
        (f32::from_bits(0x4387cccd), f32::from_bits(0x42bde978)),
        (f32::from_bits(0x43882666), f32::from_bits(0x42bdea99)),
        (f32::from_bits(0x43888000), f32::from_bits(0x42bdeae2)),
    ); // 271.6f, 94.956f, 272.3f, 94.9582f, 273, 94.9588f
    path.cubic_to(
        (f32::from_bits(0x4388d99a), f32::from_bits(0x42bdeb2a)),
        (f32::from_bits(0x43893333), f32::from_bits(0x42bdeae2)),
        (f32::from_bits(0x43898ccd), f32::from_bits(0x42bdeae2)),
    ); // 273.7f, 94.9593f, 274.4f, 94.9588f, 275.1f, 94.9588f
    path.cubic_to(
        (f32::from_bits(0x4389e666), f32::from_bits(0x42bdeae2)),
        (f32::from_bits(0x438a4000), f32::from_bits(0x42bdea99)),
        (f32::from_bits(0x438a999a), f32::from_bits(0x42bdeae2)),
    ); // 275.8f, 94.9588f, 276.5f, 94.9582f, 277.2f, 94.9588f
    path.cubic_to(
        (f32::from_bits(0x438af333), f32::from_bits(0x42bdeb2a)),
        (f32::from_bits(0x438b4ccd), f32::from_bits(0x42bdec94)),
        (f32::from_bits(0x438ba666), f32::from_bits(0x42bdec94)),
    ); // 277.9f, 94.9593f, 278.6f, 94.9621f, 279.3f, 94.9621f
    path.cubic_to(
        (f32::from_bits(0x438c0000), f32::from_bits(0x42bdec94)),
        (f32::from_bits(0x438c599a), f32::from_bits(0x42bdead6)),
        (f32::from_bits(0x438cb333), f32::from_bits(0x42bdeae2)),
    ); // 280, 94.9621f, 280.7f, 94.9587f, 281.4f, 94.9588f
    path.cubic_to(
        (f32::from_bits(0x438d0ccd), f32::from_bits(0x42bdeaed)),
        (f32::from_bits(0x438d6666), f32::from_bits(0x42bdeb11)),
        (f32::from_bits(0x438dc000), f32::from_bits(0x42bdecda)),
    ); // 282.1f, 94.9588f, 282.8f, 94.9591f, 283.5f, 94.9626f
    path.cubic_to(
        (f32::from_bits(0x438e199a), f32::from_bits(0x42bdeea4)),
        (f32::from_bits(0x438e7333), f32::from_bits(0x42bdf3e9)),
        (f32::from_bits(0x438ecccd), f32::from_bits(0x42bdf59c)),
    ); // 284.2f, 94.9661f, 284.9f, 94.9764f, 285.6f, 94.9797f
    path.cubic_to(
        (f32::from_bits(0x438f2666), f32::from_bits(0x42bdf74e)),
        (f32::from_bits(0x438f8000), f32::from_bits(0x42bdfbed)),
        (f32::from_bits(0x438fd99a), f32::from_bits(0x42bdf707)),
    ); // 286.3f, 94.983f, 287, 94.992f, 287.7f, 94.9825f
    path.cubic_to(
        (f32::from_bits(0x43903333), f32::from_bits(0x42bdf222)),
        (f32::from_bits(0x43908ccd), f32::from_bits(0x42bdeb8a)),
        (f32::from_bits(0x4390e666), f32::from_bits(0x42bdd83a)),
    ); // 288.4f, 94.9729f, 289.1f, 94.96f, 289.8f, 94.9223f
    path.cubic_to(
        (f32::from_bits(0x43914000), f32::from_bits(0x42bdc4eb)),
        (f32::from_bits(0x4391999a), f32::from_bits(0x42bda6ea)),
        (f32::from_bits(0x4391f333), f32::from_bits(0x42bd832a)),
    ); // 290.5f, 94.8846f, 291.2f, 94.826f, 291.9f, 94.7562f
    path.cubic_to(
        (f32::from_bits(0x43924ccd), f32::from_bits(0x42bd5f6a)),
        (f32::from_bits(0x4392a666), f32::from_bits(0x42bd1798)),
        (f32::from_bits(0x43930000), f32::from_bits(0x42bd01bb)),
    ); // 292.6f, 94.6864f, 293.3f, 94.5461f, 294, 94.5034f
    path.cubic_to(
        (f32::from_bits(0x4393599a), f32::from_bits(0x42bcebdf)),
        (f32::from_bits(0x4393b333), f32::from_bits(0x42bd004a)),
        (f32::from_bits(0x43940ccd), f32::from_bits(0x42bd0000)),
    ); // 294.7f, 94.4607f, 295.4f, 94.5006f, 296.1f, 94.5f
    path.line_to((f32::from_bits(0x43940ccd), f32::from_bits(0x42bd0000))); // 296.1f, 94.5f
    path.line_to((f32::from_bits(0x43413333), f32::from_bits(0x42bd0000))); // 193.2f, 94.5f
    path.close();
    path.move_to((f32::from_bits(0x43ac3333), f32::from_bits(0x42bd0000))); // 344.4f, 94.5f
    path.line_to((f32::from_bits(0x43ac3333), f32::from_bits(0x42bd0000))); // 344.4f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x43ac8ccd), f32::from_bits(0x42bd0000)),
        (f32::from_bits(0x43ace666), f32::from_bits(0x42bcfc5e)),
        (f32::from_bits(0x43ad4000), f32::from_bits(0x42bd0000)),
    ); // 345.1f, 94.5f, 345.8f, 94.4929f, 346.5f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x43ad999a), f32::from_bits(0x42bd03a2)),
        (f32::from_bits(0x43adf333), f32::from_bits(0x42bcf966)),
        (f32::from_bits(0x43ae4ccd), f32::from_bits(0x42bd15ce)),
    ); // 347.2f, 94.5071f, 347.9f, 94.4871f, 348.6f, 94.5426f
    path.cubic_to(
        (f32::from_bits(0x43aea666), f32::from_bits(0x42bd3236)),
        (f32::from_bits(0x43af0000), f32::from_bits(0x42bd84e8)),
        (f32::from_bits(0x43af599a), f32::from_bits(0x42bdaa71)),
    ); // 349.3f, 94.5981f, 350, 94.7596f, 350.7f, 94.8329f
    path.cubic_to(
        (f32::from_bits(0x43afb333), f32::from_bits(0x42bdcffb)),
        (f32::from_bits(0x43b00ccd), f32::from_bits(0x42bde92e)),
        (f32::from_bits(0x43b06666), f32::from_bits(0x42bdf707)),
    ); // 351.4f, 94.9062f, 352.1f, 94.9554f, 352.8f, 94.9825f
    path.cubic_to(
        (f32::from_bits(0x43b0c000), f32::from_bits(0x42be04e0)),
        (f32::from_bits(0x43b1199a), f32::from_bits(0x42bdfc2b)),
        (f32::from_bits(0x43b17333), f32::from_bits(0x42bdfd89)),
    ); // 353.5f, 95.0095f, 354.2f, 94.9925f, 354.9f, 94.9952f
    path.cubic_to(
        (f32::from_bits(0x43b1cccd), f32::from_bits(0x42bdfee7)),
        (f32::from_bits(0x43b22666), f32::from_bits(0x42bdfe62)),
        (f32::from_bits(0x43b28000), f32::from_bits(0x42bdff3b)),
    ); // 355.6f, 94.9979f, 356.3f, 94.9968f, 357, 94.9985f
    path.cubic_to(
        (f32::from_bits(0x43b2d99a), f32::from_bits(0x42be0014)),
        (f32::from_bits(0x43b33333), f32::from_bits(0x42be020f)),
        (f32::from_bits(0x43b38ccd), f32::from_bits(0x42be029f)),
    ); // 357.7f, 95.0002f, 358.4f, 95.004f, 359.1f, 95.0051f
    path.cubic_to(
        (f32::from_bits(0x43b3e666), f32::from_bits(0x42be0330)),
        (f32::from_bits(0x43b44000), f32::from_bits(0x42be020f)),
        (f32::from_bits(0x43b4999a), f32::from_bits(0x42be029f)),
    ); // 359.8f, 95.0062f, 360.5f, 95.004f, 361.2f, 95.0051f
    path.cubic_to(
        (f32::from_bits(0x43b4f333), f32::from_bits(0x42be0330)),
        (f32::from_bits(0x43b54ccd), f32::from_bits(0x42be052a)),
        (f32::from_bits(0x43b5a666), f32::from_bits(0x42be0604)),
    ); // 361.9f, 95.0062f, 362.6f, 95.0101f, 363.3f, 95.0117f
    path.cubic_to(
        (f32::from_bits(0x43b60000), f32::from_bits(0x42be06dd)),
        (f32::from_bits(0x43b6599a), f32::from_bits(0x42be0725)),
        (f32::from_bits(0x43b6b333), f32::from_bits(0x42be07b6)),
    ); // 364, 95.0134f, 364.7f, 95.014f, 365.4f, 95.0151f
    path.cubic_to(
        (f32::from_bits(0x43b70ccd), f32::from_bits(0x42be0846)),
        (f32::from_bits(0x43b76666), f32::from_bits(0x42be08d7)),
        (f32::from_bits(0x43b7c000), f32::from_bits(0x42be0968)),
    ); // 366.1f, 95.0162f, 366.8f, 95.0173f, 367.5f, 95.0184f
    path.cubic_to(
        (f32::from_bits(0x43b8199a), f32::from_bits(0x42be09f8)),
        (f32::from_bits(0x43b87333), f32::from_bits(0x42be0ad2)),
        (f32::from_bits(0x43b8cccd), f32::from_bits(0x42be0b1a)),
    ); // 368.2f, 95.0195f, 368.9f, 95.0211f, 369.6f, 95.0217f
    path.cubic_to(
        (f32::from_bits(0x43b92666), f32::from_bits(0x42be0b62)),
        (f32::from_bits(0x43b98000), f32::from_bits(0x42be0bab)),
        (f32::from_bits(0x43b9d99a), f32::from_bits(0x42be0b1a)),
    ); // 370.3f, 95.0222f, 371, 95.0228f, 371.7f, 95.0217f
    path.cubic_to(
        (f32::from_bits(0x43ba3333), f32::from_bits(0x42be0a89)),
        (f32::from_bits(0x43ba8ccd), f32::from_bits(0x42be088f)),
        (f32::from_bits(0x43bae666), f32::from_bits(0x42be07b6)),
    ); // 372.4f, 95.0206f, 373.1f, 95.0167f, 373.8f, 95.0151f
    path.cubic_to(
        (f32::from_bits(0x43bb4000), f32::from_bits(0x42be06dd)),
        (f32::from_bits(0x43bb999a), f32::from_bits(0x42be064c)),
        (f32::from_bits(0x43bbf333), f32::from_bits(0x42be0604)),
    ); // 374.5f, 95.0134f, 375.2f, 95.0123f, 375.9f, 95.0117f
    path.cubic_to(
        (f32::from_bits(0x43bc4ccd), f32::from_bits(0x42be05bb)),
        (f32::from_bits(0x43bca666), f32::from_bits(0x42be0604)),
        (f32::from_bits(0x43bd0000), f32::from_bits(0x42be0604)),
    ); // 376.6f, 95.0112f, 377.3f, 95.0117f, 378, 95.0117f
    path.cubic_to(
        (f32::from_bits(0x43bd599a), f32::from_bits(0x42be0604)),
        (f32::from_bits(0x43bdb333), f32::from_bits(0x42be0604)),
        (f32::from_bits(0x43be0ccd), f32::from_bits(0x42be0604)),
    ); // 378.7f, 95.0117f, 379.4f, 95.0117f, 380.1f, 95.0117f
    path.cubic_to(
        (f32::from_bits(0x43be6666), f32::from_bits(0x42be0604)),
        (f32::from_bits(0x43bec000), f32::from_bits(0x42be0725)),
        (f32::from_bits(0x43bf199a), f32::from_bits(0x42be0604)),
    ); // 380.8f, 95.0117f, 381.5f, 95.014f, 382.2f, 95.0117f
    path.cubic_to(
        (f32::from_bits(0x43bf7333), f32::from_bits(0x42be04e2)),
        (f32::from_bits(0x43bfcccd), f32::from_bits(0x42be0136)),
        (f32::from_bits(0x43c02666), f32::from_bits(0x42bdff3b)),
    ); // 382.9f, 95.0095f, 383.6f, 95.0024f, 384.3f, 94.9985f
    path.cubic_to(
        (f32::from_bits(0x43c08000), f32::from_bits(0x42bdfd41)),
        (f32::from_bits(0x43c0d99a), f32::from_bits(0x42bdfa6d)),
        (f32::from_bits(0x43c13333), f32::from_bits(0x42bdfa25)),
    ); // 385, 94.9946f, 385.7f, 94.9891f, 386.4f, 94.9886f
    path.cubic_to(
        (f32::from_bits(0x43c18ccd), f32::from_bits(0x42bdf9dc)),
        (f32::from_bits(0x43c1e666), f32::from_bits(0x42bdfcf8)),
        (f32::from_bits(0x43c24000), f32::from_bits(0x42bdfd89)),
    ); // 387.1f, 94.988f, 387.8f, 94.9941f, 388.5f, 94.9952f
    path.cubic_to(
        (f32::from_bits(0x43c2999a), f32::from_bits(0x42bdfe1a)),
        (f32::from_bits(0x43c2f333), f32::from_bits(0x42bdfdd1)),
        (f32::from_bits(0x43c34ccd), f32::from_bits(0x42bdfd89)),
    ); // 389.2f, 94.9963f, 389.9f, 94.9957f, 390.6f, 94.9952f
    path.cubic_to(
        (f32::from_bits(0x43c3a666), f32::from_bits(0x42bdfd41)),
        (f32::from_bits(0x43c40000), f32::from_bits(0x42bdfd41)),
        (f32::from_bits(0x43c4599a), f32::from_bits(0x42bdfbd7)),
    ); // 391.3f, 94.9946f, 392, 94.9946f, 392.7f, 94.9919f
    path.cubic_to(
        (f32::from_bits(0x43c4b333), f32::from_bits(0x42bdfa6d)),
        (f32::from_bits(0x43c50ccd), f32::from_bits(0x42bdf709)),
        (f32::from_bits(0x43c56666), f32::from_bits(0x42bdf50e)),
    ); // 393.4f, 94.9891f, 394.1f, 94.9825f, 394.8f, 94.9786f
    path.cubic_to(
        (f32::from_bits(0x43c5c000), f32::from_bits(0x42bdf314)),
        (f32::from_bits(0x43c6199a), f32::from_bits(0x42bdf0d1)),
        (f32::from_bits(0x43c67333), f32::from_bits(0x42bdeff8)),
    ); // 395.5f, 94.9748f, 396.2f, 94.9703f, 396.9f, 94.9687f
    path.cubic_to(
        (f32::from_bits(0x43c6cccd), f32::from_bits(0x42bdef1f)),
        (f32::from_bits(0x43c72666), f32::from_bits(0x42bdf040)),
        (f32::from_bits(0x43c78000), f32::from_bits(0x42bdeff8)),
    ); // 397.6f, 94.967f, 398.3f, 94.9692f, 399, 94.9687f
    path.cubic_to(
        (f32::from_bits(0x43c7d99a), f32::from_bits(0x42bdefb0)),
        (f32::from_bits(0x43c83333), f32::from_bits(0x42bdee8e)),
        (f32::from_bits(0x43c88ccd), f32::from_bits(0x42bdee46)),
    ); // 399.7f, 94.9681f, 400.4f, 94.9659f, 401.1f, 94.9654f
    path.cubic_to(
        (f32::from_bits(0x43c8e666), f32::from_bits(0x42bdedfe)),
        (f32::from_bits(0x43c94000), f32::from_bits(0x42bdee46)),
        (f32::from_bits(0x43c9999a), f32::from_bits(0x42bdee46)),
    ); // 401.8f, 94.9648f, 402.5f, 94.9654f, 403.2f, 94.9654f
    path.cubic_to(
        (f32::from_bits(0x43c9f333), f32::from_bits(0x42bdee46)),
        (f32::from_bits(0x43ca4ccd), f32::from_bits(0x42bdedb5)),
        (f32::from_bits(0x43caa666), f32::from_bits(0x42bdee46)),
    ); // 403.9f, 94.9654f, 404.6f, 94.9643f, 405.3f, 94.9654f
    path.cubic_to(
        (f32::from_bits(0x43cb0000), f32::from_bits(0x42bdeed7)),
        (f32::from_bits(0x43cb599a), f32::from_bits(0x42bdf089)),
        (f32::from_bits(0x43cbb333), f32::from_bits(0x42bdf1aa)),
    ); // 406, 94.9665f, 406.7f, 94.9698f, 407.4f, 94.972f
    path.cubic_to(
        (f32::from_bits(0x43cc0ccd), f32::from_bits(0x42bdf2cc)),
        (f32::from_bits(0x43cc6666), f32::from_bits(0x42bdf47e)),
        (f32::from_bits(0x43ccc000), f32::from_bits(0x42bdf50e)),
    ); // 408.1f, 94.9742f, 408.8f, 94.9775f, 409.5f, 94.9786f
    path.cubic_to(
        (f32::from_bits(0x43cd199a), f32::from_bits(0x42bdf59f)),
        (f32::from_bits(0x43cd7333), f32::from_bits(0x42bdf4c6)),
        (f32::from_bits(0x43cdcccd), f32::from_bits(0x42bdf50e)),
    ); // 410.2f, 94.9797f, 410.9f, 94.9781f, 411.6f, 94.9786f
    path.cubic_to(
        (f32::from_bits(0x43ce2666), f32::from_bits(0x42bdf557)),
        (f32::from_bits(0x43ce8000), f32::from_bits(0x42bdf557)),
        (f32::from_bits(0x43ced99a), f32::from_bits(0x42bdf6c0)),
    ); // 412.3f, 94.9792f, 413, 94.9792f, 413.7f, 94.9819f
    path.cubic_to(
        (f32::from_bits(0x43cf3333), f32::from_bits(0x42bdf82a)),
        (f32::from_bits(0x43cf8ccd), f32::from_bits(0x42bdfafe)),
        (f32::from_bits(0x43cfe666), f32::from_bits(0x42bdfd89)),
    ); // 414.4f, 94.9847f, 415.1f, 94.9902f, 415.8f, 94.9952f
    path.cubic_to(
        (f32::from_bits(0x43d04000), f32::from_bits(0x42be0014)),
        (f32::from_bits(0x43d0999a), f32::from_bits(0x42be0378)),
        (f32::from_bits(0x43d0f333), f32::from_bits(0x42be0604)),
    ); // 416.5f, 95.0002f, 417.2f, 95.0068f, 417.9f, 95.0117f
    path.cubic_to(
        (f32::from_bits(0x43d14ccd), f32::from_bits(0x42be088f)),
        (f32::from_bits(0x43d1a666), f32::from_bits(0x42be0b1a)),
        (f32::from_bits(0x43d20000), f32::from_bits(0x42be0ccc)),
    ); // 418.6f, 95.0167f, 419.3f, 95.0217f, 420, 95.025f
    path.cubic_to(
        (f32::from_bits(0x43d2599a), f32::from_bits(0x42be0e7e)),
        (f32::from_bits(0x43d2b333), f32::from_bits(0x42be119a)),
        (f32::from_bits(0x43d30ccd), f32::from_bits(0x42be1030)),
    ); // 420.7f, 95.0283f, 421.4f, 95.0344f, 422.1f, 95.0316f
    path.cubic_to(
        (f32::from_bits(0x43d36666), f32::from_bits(0x42be0ec6)),
        (f32::from_bits(0x43d3c000), f32::from_bits(0x42be0ad2)),
        (f32::from_bits(0x43d4199a), f32::from_bits(0x42be0451)),
    ); // 422.8f, 95.0289f, 423.5f, 95.0211f, 424.2f, 95.0084f
    path.cubic_to(
        (f32::from_bits(0x43d47333), f32::from_bits(0x42bdfdd1)),
        (f32::from_bits(0x43d4cccd), f32::from_bits(0x42bdefec)),
        (f32::from_bits(0x43d52666), f32::from_bits(0x42bde930)),
    ); // 424.9f, 94.9957f, 425.6f, 94.9686f, 426.3f, 94.9554f
    path.cubic_to(
        (f32::from_bits(0x43d58000), f32::from_bits(0x42bde273)),
        (f32::from_bits(0x43d5d99a), f32::from_bits(0x42bde21d)),
        (f32::from_bits(0x43d63333), f32::from_bits(0x42bddbe5)),
    ); // 427, 94.9423f, 427.7f, 94.9416f, 428.4f, 94.9295f
    path.cubic_to(
        (f32::from_bits(0x43d68ccd), f32::from_bits(0x42bdd5ad)),
        (f32::from_bits(0x43d6e666), f32::from_bits(0x42bdcf3b)),
        (f32::from_bits(0x43d74000), f32::from_bits(0x42bdc3e1)),
    ); // 429.1f, 94.9173f, 429.8f, 94.9047f, 430.5f, 94.8826f
    path.cubic_to(
        (f32::from_bits(0x43d7999a), f32::from_bits(0x42bdb887)),
        (f32::from_bits(0x43d7f333), f32::from_bits(0x42bd9d7d)),
        (f32::from_bits(0x43d84ccd), f32::from_bits(0x42bd97ca)),
    ); // 431.2f, 94.8604f, 431.9f, 94.8076f, 432.6f, 94.7965f
    path.cubic_to(
        (f32::from_bits(0x43d8a666), f32::from_bits(0x42bd9217)),
        (f32::from_bits(0x43d90000), f32::from_bits(0x42bd9820)),
        (f32::from_bits(0x43d9599a), f32::from_bits(0x42bda1b0)),
    ); // 433.3f, 94.7853f, 434, 94.7971f, 434.7f, 94.8158f
    path.cubic_to(
        (f32::from_bits(0x43d9b333), f32::from_bits(0x42bdab40)),
        (f32::from_bits(0x43da0ccd), f32::from_bits(0x42bdc5dd)),
        (f32::from_bits(0x43da6666), f32::from_bits(0x42bdd12b)),
    ); // 435.4f, 94.8345f, 436.1f, 94.8865f, 436.8f, 94.9085f
    path.cubic_to(
        (f32::from_bits(0x43dac000), f32::from_bits(0x42bddc79)),
        (f32::from_bits(0x43db199a), f32::from_bits(0x42bde1cc)),
        (f32::from_bits(0x43db7333), f32::from_bits(0x42bde585)),
    ); // 437.5f, 94.9306f, 438.2f, 94.941f, 438.9f, 94.9483f
    path.cubic_to(
        (f32::from_bits(0x43dbcccd), f32::from_bits(0x42bde93d)),
        (f32::from_bits(0x43dc2666), f32::from_bits(0x42bde75a)),
        (f32::from_bits(0x43dc8000), f32::from_bits(0x42bde77d)),
    ); // 439.6f, 94.9555f, 440.3f, 94.9519f, 441, 94.9521f
    path.cubic_to(
        (f32::from_bits(0x43dcd99a), f32::from_bits(0x42bde7a1)),
        (f32::from_bits(0x43dd3333), f32::from_bits(0x42bde610)),
        (f32::from_bits(0x43dd8ccd), f32::from_bits(0x42bde658)),
    ); // 441.7f, 94.9524f, 442.4f, 94.9493f, 443.1f, 94.9499f
    path.cubic_to(
        (f32::from_bits(0x43dde666), f32::from_bits(0x42bde6a1)),
        (f32::from_bits(0x43de4000), f32::from_bits(0x42bdea2c)),
        (f32::from_bits(0x43de999a), f32::from_bits(0x42bde930)),
    ); // 443.8f, 94.9504f, 444.5f, 94.9574f, 445.2f, 94.9554f
    path.cubic_to(
        (f32::from_bits(0x43def333), f32::from_bits(0x42bde833)),
        (f32::from_bits(0x43df4ccd), f32::from_bits(0x42bdee5f)),
        (f32::from_bits(0x43dfa666), f32::from_bits(0x42bde06e)),
    ); // 445.9f, 94.9535f, 446.6f, 94.9656f, 447.3f, 94.9383f
    path.cubic_to(
        (f32::from_bits(0x43e00000), f32::from_bits(0x42bdd27d)),
        (f32::from_bits(0x43e0599a), f32::from_bits(0x42bdb871)),
        (f32::from_bits(0x43e0b333), f32::from_bits(0x42bd958a)),
    ); // 448, 94.9111f, 448.7f, 94.8602f, 449.4f, 94.7921f
    path.cubic_to(
        (f32::from_bits(0x43e10ccd), f32::from_bits(0x42bd72a4)),
        (f32::from_bits(0x43e16666), f32::from_bits(0x42bd27f2)),
        (f32::from_bits(0x43e1c000), f32::from_bits(0x42bd0f06)),
    ); // 450.1f, 94.7239f, 450.8f, 94.578f, 451.5f, 94.5293f
    path.cubic_to(
        (f32::from_bits(0x43e2199a), f32::from_bits(0x42bcf619)),
        (f32::from_bits(0x43e27333), f32::from_bits(0x42bcfbb7)),
        (f32::from_bits(0x43e2cccd), f32::from_bits(0x42bd0000)),
    ); // 452.2f, 94.4807f, 452.9f, 94.4916f, 453.6f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x43e32666), f32::from_bits(0x42bd0449)),
        (f32::from_bits(0x43e38000), f32::from_bits(0x42bd1419)),
        (f32::from_bits(0x43e3d99a), f32::from_bits(0x42bd28bc)),
    ); // 454.3f, 94.5084f, 455, 94.5393f, 455.7f, 94.5796f
    path.cubic_to(
        (f32::from_bits(0x43e43333), f32::from_bits(0x42bd3d60)),
        (f32::from_bits(0x43e48ccd), f32::from_bits(0x42bd7238)),
        (f32::from_bits(0x43e4e666), f32::from_bits(0x42bd7bd4)),
    ); // 456.4f, 94.6199f, 457.1f, 94.7231f, 457.8f, 94.7419f
    path.cubic_to(
        (f32::from_bits(0x43e54000), f32::from_bits(0x42bd8570)),
        (f32::from_bits(0x43e5999a), f32::from_bits(0x42bd70ce)),
        (f32::from_bits(0x43e5f333), f32::from_bits(0x42bd6264)),
    ); // 458.5f, 94.7606f, 459.2f, 94.7203f, 459.9f, 94.6922f
    path.cubic_to(
        (f32::from_bits(0x43e64ccd), f32::from_bits(0x42bd53fa)),
        (f32::from_bits(0x43e6a666), f32::from_bits(0x42bd35be)),
        (f32::from_bits(0x43e70000), f32::from_bits(0x42bd2558)),
    ); // 460.6f, 94.664f, 461.3f, 94.605f, 462, 94.5729f
    path.cubic_to(
        (f32::from_bits(0x43e7599a), f32::from_bits(0x42bd14f2)),
        (f32::from_bits(0x43e7b333), f32::from_bits(0x42bd0639)),
        (f32::from_bits(0x43e80ccd), f32::from_bits(0x42bd0000)),
    ); // 462.7f, 94.5409f, 463.4f, 94.5122f, 464.1f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x43e86666), f32::from_bits(0x42bcf9c7)),
        (f32::from_bits(0x43e8c000), f32::from_bits(0x42bcfef5)),
        (f32::from_bits(0x43e9199a), f32::from_bits(0x42bd0000)),
    ); // 464.8f, 94.4878f, 465.5f, 94.498f, 466.2f, 94.5f
    path.cubic_to(
        (f32::from_bits(0x43e97333), f32::from_bits(0x42bd010b)),
        (f32::from_bits(0x43e9cccd), f32::from_bits(0x42bd0645)),
        (f32::from_bits(0x43ea2666), f32::from_bits(0x42bd0645)),
    ); // 466.9f, 94.502f, 467.6f, 94.5122f, 468.3f, 94.5122f
    path.cubic_to(
        (f32::from_bits(0x43ea8000), f32::from_bits(0x42bd0645)),
        (f32::from_bits(0x43ead99a), f32::from_bits(0x42bd010b)),
        (f32::from_bits(0x43eb3333), f32::from_bits(0x42bd0000)),
    ); // 469, 94.5122f, 469.7f, 94.502f, 470.4f, 94.5f
    path.line_to((f32::from_bits(0x43eb3333), f32::from_bits(0x42bd0000))); // 470.4f, 94.5f
    path.line_to((f32::from_bits(0x43ac3333), f32::from_bits(0x42bd0000))); // 344.4f, 94.5f
    path.close();
    path.detach()
}

// Port of: tests/PathOpsIssue3651.cpp#L830-L1019 (chrome/m156)
fn path5() -> Path {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x42b06666), f32::from_bits(0x42c9999a))); // 88.2f, 100.8f
    path.line_to((f32::from_bits(0x42b06666), f32::from_bits(0x42c9999a))); // 88.2f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x42b1cccd), f32::from_bits(0x42c9999a)),
        (f32::from_bits(0x42b33333), f32::from_bits(0x42c9b407)),
        (f32::from_bits(0x42b4999a), f32::from_bits(0x42c9999a)),
    ); // 88.9f, 100.8f, 89.6f, 100.852f, 90.3f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x42b60000), f32::from_bits(0x42c97f2c)),
        (f32::from_bits(0x42b76666), f32::from_bits(0x42c91521)),
        (f32::from_bits(0x42b8cccd), f32::from_bits(0x42c8fb07)),
    ); // 91, 100.748f, 91.7f, 100.541f, 92.4f, 100.49f
    path.cubic_to(
        (f32::from_bits(0x42ba3333), f32::from_bits(0x42c8e0ee)),
        (f32::from_bits(0x42bb999a), f32::from_bits(0x42c8fd00)),
        (f32::from_bits(0x42bd0000), f32::from_bits(0x42c8fd00)),
    ); // 93.1f, 100.439f, 93.8f, 100.494f, 94.5f, 100.494f
    path.cubic_to(
        (f32::from_bits(0x42be6666), f32::from_bits(0x42c8fd00)),
        (f32::from_bits(0x42bfcccd), f32::from_bits(0x42c8facb)),
        (f32::from_bits(0x42c13333), f32::from_bits(0x42c8fb07)),
    ); // 95.2f, 100.494f, 95.9f, 100.49f, 96.6f, 100.49f
    path.cubic_to(
        (f32::from_bits(0x42c2999a), f32::from_bits(0x42c8fb44)),
        (f32::from_bits(0x42c40000), f32::from_bits(0x42c8fd4a)),
        (f32::from_bits(0x42c56666), f32::from_bits(0x42c8fe6c)),
    ); // 97.3f, 100.491f, 98, 100.495f, 98.7f, 100.497f
    path.cubic_to(
        (f32::from_bits(0x42c6cccd), f32::from_bits(0x42c8ff8d)),
        (f32::from_bits(0x42c83333), f32::from_bits(0x42c90218)),
        (f32::from_bits(0x42c9999a), f32::from_bits(0x42c901d0)),
    ); // 99.4f, 100.499f, 100.1f, 100.504f, 100.8f, 100.504f
    path.cubic_to(
        (f32::from_bits(0x42cb0000), f32::from_bits(0x42c90187)),
        (f32::from_bits(0x42cc6666), f32::from_bits(0x42c8ff39)),
        (f32::from_bits(0x42cdcccd), f32::from_bits(0x42c8fcb9)),
    ); // 101.5f, 100.503f, 102.2f, 100.498f, 102.9f, 100.494f
    path.cubic_to(
        (f32::from_bits(0x42cf3333), f32::from_bits(0x42c8fa3a)),
        (f32::from_bits(0x42d0999a), f32::from_bits(0x42c8f364)),
        (f32::from_bits(0x42d20000), f32::from_bits(0x42c8f2d3)),
    ); // 103.6f, 100.489f, 104.3f, 100.475f, 105, 100.474f
    path.cubic_to(
        (f32::from_bits(0x42d36666), f32::from_bits(0x42c8f243)),
        (f32::from_bits(0x42d4cccd), f32::from_bits(0x42c8f402)),
        (f32::from_bits(0x42d63333), f32::from_bits(0x42c8f955)),
    ); // 105.7f, 100.473f, 106.4f, 100.477f, 107.1f, 100.487f
    path.cubic_to(
        (f32::from_bits(0x42d7999a), f32::from_bits(0x42c8fea8)),
        (f32::from_bits(0x42d90000), f32::from_bits(0x42c90daf)),
        (f32::from_bits(0x42da6666), f32::from_bits(0x42c912c5)),
    ); // 107.8f, 100.497f, 108.5f, 100.527f, 109.2f, 100.537f
    path.cubic_to(
        (f32::from_bits(0x42dbcccd), f32::from_bits(0x42c917db)),
        (f32::from_bits(0x42dd3333), f32::from_bits(0x42c918f1)),
        (f32::from_bits(0x42de999a), f32::from_bits(0x42c917db)),
    ); // 109.9f, 100.547f, 110.6f, 100.549f, 111.3f, 100.547f
    path.cubic_to(
        (f32::from_bits(0x42e00000), f32::from_bits(0x42c916c6)),
        (f32::from_bits(0x42e16666), f32::from_bits(0x42c90ac2)),
        (f32::from_bits(0x42e2cccd), f32::from_bits(0x42c90c43)),
    ); // 112, 100.544f, 112.7f, 100.521f, 113.4f, 100.524f
    path.cubic_to(
        (f32::from_bits(0x42e43333), f32::from_bits(0x42c90dc4)),
        (f32::from_bits(0x42e5999a), f32::from_bits(0x42c91420)),
        (f32::from_bits(0x42e70000), f32::from_bits(0x42c920e3)),
    ); // 114.1f, 100.527f, 114.8f, 100.539f, 115.5f, 100.564f
    path.cubic_to(
        (f32::from_bits(0x42e86666), f32::from_bits(0x42c92da7)),
        (f32::from_bits(0x42e9cccd), f32::from_bits(0x42c946ab)),
        (f32::from_bits(0x42eb3333), f32::from_bits(0x42c958d9)),
    ); // 116.2f, 100.589f, 116.9f, 100.638f, 117.6f, 100.674f
    path.cubic_to(
        (f32::from_bits(0x42ec999a), f32::from_bits(0x42c96b07)),
        (f32::from_bits(0x42ee0000), f32::from_bits(0x42c9832d)),
        (f32::from_bits(0x42ef6666), f32::from_bits(0x42c98df8)),
    ); // 118.3f, 100.709f, 119, 100.756f, 119.7f, 100.777f
    path.cubic_to(
        (f32::from_bits(0x42f0cccd), f32::from_bits(0x42c998c3)),
        (f32::from_bits(0x42f23333), f32::from_bits(0x42c997a9)),
        (f32::from_bits(0x42f3999a), f32::from_bits(0x42c9999a)),
    ); // 120.4f, 100.798f, 121.1f, 100.796f, 121.8f, 100.8f
    path.line_to((f32::from_bits(0x42f3999a), f32::from_bits(0x42c9999a))); // 121.8f, 100.8f
    path.line_to((f32::from_bits(0x42b06666), f32::from_bits(0x42c9999a))); // 88.2f, 100.8f
    path.close();
    path.move_to((f32::from_bits(0x4300199a), f32::from_bits(0x42c9999a))); // 128.1f, 100.8f
    path.line_to((f32::from_bits(0x4300199a), f32::from_bits(0x42c99825))); // 128.1f, 100.797f
    path.cubic_to(
        (f32::from_bits(0x4300cccd), f32::from_bits(0x42c99756)),
        (f32::from_bits(0x43018000), f32::from_bits(0x42c99482)),
        (f32::from_bits(0x43023333), f32::from_bits(0x42c994c1)),
    ); // 128.8f, 100.796f, 129.5f, 100.79f, 130.2f, 100.791f
    path.cubic_to(
        (f32::from_bits(0x4302e666), f32::from_bits(0x42c994ff)),
        (f32::from_bits(0x4303999a), f32::from_bits(0x42c998cb)),
        (f32::from_bits(0x43044ccd), f32::from_bits(0x42c9999a)),
    ); // 130.9f, 100.791f, 131.6f, 100.798f, 132.3f, 100.8f
    path.line_to((f32::from_bits(0x43044ccd), f32::from_bits(0x42c9999a))); // 132.3f, 100.8f
    path.line_to((f32::from_bits(0x4300199a), f32::from_bits(0x42c9999a))); // 128.1f, 100.8f
    path.close();
    path.move_to((f32::from_bits(0x431b6666), f32::from_bits(0x42c9999a))); // 155.4f, 100.8f
    path.line_to((f32::from_bits(0x431b6666), f32::from_bits(0x42c9999a))); // 155.4f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x431c199a), f32::from_bits(0x42c9999a)),
        (f32::from_bits(0x431ccccd), f32::from_bits(0x42c99dcd)),
        (f32::from_bits(0x431d8000), f32::from_bits(0x42c9999a)),
    ); // 156.1f, 100.8f, 156.8f, 100.808f, 157.5f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x431e3333), f32::from_bits(0x42c99567)),
        (f32::from_bits(0x431ee666), f32::from_bits(0x42c98d21)),
        (f32::from_bits(0x431f999a), f32::from_bits(0x42c98067)),
    ); // 158.2f, 100.792f, 158.9f, 100.776f, 159.6f, 100.751f
    path.cubic_to(
        (f32::from_bits(0x43204ccd), f32::from_bits(0x42c973ae)),
        (f32::from_bits(0x43210000), f32::from_bits(0x42c95aa1)),
        (f32::from_bits(0x4321b333), f32::from_bits(0x42c94d41)),
    ); // 160.3f, 100.726f, 161, 100.677f, 161.7f, 100.651f
    path.cubic_to(
        (f32::from_bits(0x43226666), f32::from_bits(0x42c93fe1)),
        (f32::from_bits(0x4323199a), f32::from_bits(0x42c935e5)),
        (f32::from_bits(0x4323cccd), f32::from_bits(0x42c93026)),
    ); // 162.4f, 100.625f, 163.1f, 100.605f, 163.8f, 100.594f
    path.cubic_to(
        (f32::from_bits(0x43248000), f32::from_bits(0x42c92a68)),
        (f32::from_bits(0x43253333), f32::from_bits(0x42c92b66)),
        (f32::from_bits(0x4325e666), f32::from_bits(0x42c92ac9)),
    ); // 164.5f, 100.583f, 165.2f, 100.585f, 165.9f, 100.584f
    path.cubic_to(
        (f32::from_bits(0x4326999a), f32::from_bits(0x42c92a2d)),
        (f32::from_bits(0x43274ccd), f32::from_bits(0x42c92759)),
        (f32::from_bits(0x43280000), f32::from_bits(0x42c92c7b)),
    ); // 166.6f, 100.582f, 167.3f, 100.577f, 168, 100.587f
    path.cubic_to(
        (f32::from_bits(0x4328b333), f32::from_bits(0x42c9319e)),
        (f32::from_bits(0x43296666), f32::from_bits(0x42c93d57)),
        (f32::from_bits(0x432a199a), f32::from_bits(0x42c94996)),
    ); // 168.7f, 100.597f, 169.4f, 100.62f, 170.1f, 100.644f
    path.cubic_to(
        (f32::from_bits(0x432acccd), f32::from_bits(0x42c955d5)),
        (f32::from_bits(0x432b8000), f32::from_bits(0x42c968e8)),
        (f32::from_bits(0x432c3333), f32::from_bits(0x42c975f4)),
    ); // 170.8f, 100.668f, 171.5f, 100.705f, 172.2f, 100.73f
    path.cubic_to(
        (f32::from_bits(0x432ce666), f32::from_bits(0x42c98300)),
        (f32::from_bits(0x432d999a), f32::from_bits(0x42c991ed)),
        (f32::from_bits(0x432e4ccd), f32::from_bits(0x42c997de)),
    ); // 172.9f, 100.756f, 173.6f, 100.785f, 174.3f, 100.797f
    path.cubic_to(
        (f32::from_bits(0x432f0000), f32::from_bits(0x42c99dcf)),
        (f32::from_bits(0x432fb333), f32::from_bits(0x42c99950)),
        (f32::from_bits(0x43306666), f32::from_bits(0x42c9999a)),
    ); // 175, 100.808f, 175.7f, 100.799f, 176.4f, 100.8f
    path.line_to((f32::from_bits(0x43306666), f32::from_bits(0x42c9999a))); // 176.4f, 100.8f
    path.line_to((f32::from_bits(0x431b6666), f32::from_bits(0x42c9999a))); // 155.4f, 100.8f
    path.close();
    path.move_to((f32::from_bits(0x43478000), f32::from_bits(0x42c9999a))); // 199.5f, 100.8f
    path.line_to((f32::from_bits(0x43478000), f32::from_bits(0x42c9999a))); // 199.5f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43483333), f32::from_bits(0x42c9999a)),
        (f32::from_bits(0x4348e666), f32::from_bits(0x42c99cf4)),
        (f32::from_bits(0x4349999a), f32::from_bits(0x42c9999a)),
    ); // 200.2f, 100.8f, 200.9f, 100.807f, 201.6f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x434a4ccd), f32::from_bits(0x42c99640)),
        (f32::from_bits(0x434b0000), f32::from_bits(0x42c98bf3)),
        (f32::from_bits(0x434bb333), f32::from_bits(0x42c9857d)),
    ); // 202.3f, 100.793f, 203, 100.773f, 203.7f, 100.761f
    path.cubic_to(
        (f32::from_bits(0x434c6666), f32::from_bits(0x42c97f08)),
        (f32::from_bits(0x434d199a), f32::from_bits(0x42c9771f)),
        (f32::from_bits(0x434dcccd), f32::from_bits(0x42c972d6)),
    ); // 204.4f, 100.748f, 205.1f, 100.733f, 205.8f, 100.724f
    path.cubic_to(
        (f32::from_bits(0x434e8000), f32::from_bits(0x42c96e8d)),
        (f32::from_bits(0x434f3333), f32::from_bits(0x42c96dc2)),
        (f32::from_bits(0x434fe666), f32::from_bits(0x42c96bc7)),
    ); // 206.5f, 100.716f, 207.2f, 100.714f, 207.9f, 100.711f
    path.cubic_to(
        (f32::from_bits(0x4350999a), f32::from_bits(0x42c969cd)),
        (f32::from_bits(0x43514ccd), f32::from_bits(0x42c967c5)),
        (f32::from_bits(0x43520000), f32::from_bits(0x42c966f7)),
    ); // 208.6f, 100.707f, 209.3f, 100.703f, 210, 100.701f
    path.cubic_to(
        (f32::from_bits(0x4352b333), f32::from_bits(0x42c9662a)),
        (f32::from_bits(0x43536666), f32::from_bits(0x42c966af)),
        (f32::from_bits(0x4354199a), f32::from_bits(0x42c966f7)),
    ); // 210.7f, 100.7f, 211.4f, 100.701f, 212.1f, 100.701f
    path.cubic_to(
        (f32::from_bits(0x4354cccd), f32::from_bits(0x42c96740)),
        (f32::from_bits(0x43558000), f32::from_bits(0x42c96b40)),
        (f32::from_bits(0x43563333), f32::from_bits(0x42c968a9)),
    ); // 212.8f, 100.702f, 213.5f, 100.709f, 214.2f, 100.704f
    path.cubic_to(
        (f32::from_bits(0x4356e666), f32::from_bits(0x42c96612)),
        (f32::from_bits(0x4357999a), f32::from_bits(0x42c9624e)),
        (f32::from_bits(0x43584ccd), f32::from_bits(0x42c9576e)),
    ); // 214.9f, 100.699f, 215.6f, 100.692f, 216.3f, 100.671f
    path.cubic_to(
        (f32::from_bits(0x43590000), f32::from_bits(0x42c94c8d)),
        (f32::from_bits(0x4359b333), f32::from_bits(0x42c935e7)),
        (f32::from_bits(0x435a6666), f32::from_bits(0x42c92765)),
    ); // 217, 100.65f, 217.7f, 100.605f, 218.4f, 100.577f
    path.cubic_to(
        (f32::from_bits(0x435b199a), f32::from_bits(0x42c918e4)),
        (f32::from_bits(0x435bcccd), f32::from_bits(0x42c9097b)),
        (f32::from_bits(0x435c8000), f32::from_bits(0x42c90064)),
    ); // 219.1f, 100.549f, 219.8f, 100.519f, 220.5f, 100.501f
    path.cubic_to(
        (f32::from_bits(0x435d3333), f32::from_bits(0x42c8f74d)),
        (f32::from_bits(0x435de666), f32::from_bits(0x42c8f840)),
        (f32::from_bits(0x435e999a), f32::from_bits(0x42c8f0db)),
    ); // 221.2f, 100.483f, 221.9f, 100.485f, 222.6f, 100.47f
    path.cubic_to(
        (f32::from_bits(0x435f4ccd), f32::from_bits(0x42c8e976)),
        (f32::from_bits(0x43600000), f32::from_bits(0x42c8df18)),
        (f32::from_bits(0x4360b333), f32::from_bits(0x42c8d407)),
    ); // 223.3f, 100.456f, 224, 100.436f, 224.7f, 100.414f
    path.cubic_to(
        (f32::from_bits(0x43616666), f32::from_bits(0x42c8c8f5)),
        (f32::from_bits(0x4362199a), f32::from_bits(0x42c8b92f)),
        (f32::from_bits(0x4362cccd), f32::from_bits(0x42c8ae71)),
    ); // 225.4f, 100.392f, 226.1f, 100.362f, 226.8f, 100.341f
    path.cubic_to(
        (f32::from_bits(0x43638000), f32::from_bits(0x42c8a3b4)),
        (f32::from_bits(0x43643333), f32::from_bits(0x42c89926)),
        (f32::from_bits(0x4364e666), f32::from_bits(0x42c89396)),
    ); // 227.5f, 100.32f, 228.2f, 100.299f, 228.9f, 100.288f
    path.cubic_to(
        (f32::from_bits(0x4365999a), f32::from_bits(0x42c88e07)),
        (f32::from_bits(0x43664ccd), f32::from_bits(0x42c88baa)),
        (f32::from_bits(0x43670000), f32::from_bits(0x42c88d14)),
    ); // 229.6f, 100.277f, 230.3f, 100.273f, 231, 100.276f
    path.cubic_to(
        (f32::from_bits(0x4367b333), f32::from_bits(0x42c88e7e)),
        (f32::from_bits(0x43686666), f32::from_bits(0x42c89675)),
        (f32::from_bits(0x4369199a), f32::from_bits(0x42c89c11)),
    ); // 231.7f, 100.278f, 232.4f, 100.294f, 233.1f, 100.305f
    path.cubic_to(
        (f32::from_bits(0x4369cccd), f32::from_bits(0x42c8a1ac)),
        (f32::from_bits(0x436a8000), f32::from_bits(0x42c8a9ea)),
        (f32::from_bits(0x436b3333), f32::from_bits(0x42c8aeb8)),
    ); // 233.8f, 100.316f, 234.5f, 100.332f, 235.2f, 100.341f
    path.cubic_to(
        (f32::from_bits(0x436be666), f32::from_bits(0x42c8b386)),
        (f32::from_bits(0x436c999a), f32::from_bits(0x42c8b733)),
        (f32::from_bits(0x436d4ccd), f32::from_bits(0x42c8b8e5)),
    ); // 235.9f, 100.351f, 236.6f, 100.358f, 237.3f, 100.361f
    path.cubic_to(
        (f32::from_bits(0x436e0000), f32::from_bits(0x42c8ba97)),
        (f32::from_bits(0x436eb333), f32::from_bits(0x42c8b9be)),
        (f32::from_bits(0x436f6666), f32::from_bits(0x42c8b8e5)),
    ); // 238, 100.364f, 238.7f, 100.363f, 239.4f, 100.361f
    path.cubic_to(
        (f32::from_bits(0x4370199a), f32::from_bits(0x42c8b80c)),
        (f32::from_bits(0x4370cccd), f32::from_bits(0x42c8b45f)),
        (f32::from_bits(0x43718000), f32::from_bits(0x42c8b3ce)),
    ); // 240.1f, 100.359f, 240.8f, 100.352f, 241.5f, 100.351f
    path.cubic_to(
        (f32::from_bits(0x43723333), f32::from_bits(0x42c8b33e)),
        (f32::from_bits(0x4372e666), f32::from_bits(0x42c8b4f0)),
        (f32::from_bits(0x4373999a), f32::from_bits(0x42c8b580)),
    ); // 242.2f, 100.35f, 242.9f, 100.353f, 243.6f, 100.354f
    path.cubic_to(
        (f32::from_bits(0x43744ccd), f32::from_bits(0x42c8b611)),
        (f32::from_bits(0x43750000), f32::from_bits(0x42c8b6ea)),
        (f32::from_bits(0x4375b333), f32::from_bits(0x42c8b733)),
    ); // 244.3f, 100.356f, 245, 100.357f, 245.7f, 100.358f
    path.cubic_to(
        (f32::from_bits(0x43766666), f32::from_bits(0x42c8b77b)),
        (f32::from_bits(0x4377199a), f32::from_bits(0x42c8b77b)),
        (f32::from_bits(0x4377cccd), f32::from_bits(0x42c8b733)),
    ); // 246.4f, 100.358f, 247.1f, 100.358f, 247.8f, 100.358f
    path.cubic_to(
        (f32::from_bits(0x43788000), f32::from_bits(0x42c8b6ea)),
        (f32::from_bits(0x43793333), f32::from_bits(0x42c8b5c9)),
        (f32::from_bits(0x4379e666), f32::from_bits(0x42c8b580)),
    ); // 248.5f, 100.357f, 249.2f, 100.355f, 249.9f, 100.354f
    path.cubic_to(
        (f32::from_bits(0x437a999a), f32::from_bits(0x42c8b538)),
        (f32::from_bits(0x437b4ccd), f32::from_bits(0x42c8b538)),
        (f32::from_bits(0x437c0000), f32::from_bits(0x42c8b580)),
    ); // 250.6f, 100.354f, 251.3f, 100.354f, 252, 100.354f
    path.cubic_to(
        (f32::from_bits(0x437cb333), f32::from_bits(0x42c8b5c9)),
        (f32::from_bits(0x437d6666), f32::from_bits(0x42c8b6ea)),
        (f32::from_bits(0x437e199a), f32::from_bits(0x42c8b733)),
    ); // 252.7f, 100.355f, 253.4f, 100.357f, 254.1f, 100.358f
    path.cubic_to(
        (f32::from_bits(0x437ecccd), f32::from_bits(0x42c8b77b)),
        (f32::from_bits(0x437f8000), f32::from_bits(0x42c8b77b)),
        (f32::from_bits(0x4380199a), f32::from_bits(0x42c8b733)),
    ); // 254.8f, 100.358f, 255.5f, 100.358f, 256.2f, 100.358f
    path.cubic_to(
        (f32::from_bits(0x43807333), f32::from_bits(0x42c8b6ea)),
        (f32::from_bits(0x4380cccd), f32::from_bits(0x42c8b5c9)),
        (f32::from_bits(0x43812666), f32::from_bits(0x42c8b580)),
    ); // 256.9f, 100.357f, 257.6f, 100.355f, 258.3f, 100.354f
    path.cubic_to(
        (f32::from_bits(0x43818000), f32::from_bits(0x42c8b538)),
        (f32::from_bits(0x4381d99a), f32::from_bits(0x42c8b580)),
        (f32::from_bits(0x43823333), f32::from_bits(0x42c8b580)),
    ); // 259, 100.354f, 259.7f, 100.354f, 260.4f, 100.354f
    path.cubic_to(
        (f32::from_bits(0x43828ccd), f32::from_bits(0x42c8b580)),
        (f32::from_bits(0x4382e666), f32::from_bits(0x42c8b580)),
        (f32::from_bits(0x43834000), f32::from_bits(0x42c8b580)),
    ); // 261.1f, 100.354f, 261.8f, 100.354f, 262.5f, 100.354f
    path.cubic_to(
        (f32::from_bits(0x4383999a), f32::from_bits(0x42c8b580)),
        (f32::from_bits(0x4383f333), f32::from_bits(0x42c8b5c9)),
        (f32::from_bits(0x43844ccd), f32::from_bits(0x42c8b580)),
    ); // 263.2f, 100.354f, 263.9f, 100.355f, 264.6f, 100.354f
    path.cubic_to(
        (f32::from_bits(0x4384a666), f32::from_bits(0x42c8b538)),
        (f32::from_bits(0x43850000), f32::from_bits(0x42c8b417)),
        (f32::from_bits(0x4385599a), f32::from_bits(0x42c8b3ce)),
    ); // 265.3f, 100.354f, 266, 100.352f, 266.7f, 100.351f
    path.cubic_to(
        (f32::from_bits(0x4385b333), f32::from_bits(0x42c8b386)),
        (f32::from_bits(0x43860ccd), f32::from_bits(0x42c8b386)),
        (f32::from_bits(0x43866666), f32::from_bits(0x42c8b3ce)),
    ); // 267.4f, 100.351f, 268.1f, 100.351f, 268.8f, 100.351f
    path.cubic_to(
        (f32::from_bits(0x4386c000), f32::from_bits(0x42c8b417)),
        (f32::from_bits(0x4387199a), f32::from_bits(0x42c8b4f0)),
        (f32::from_bits(0x43877333), f32::from_bits(0x42c8b580)),
    ); // 269.5f, 100.352f, 270.2f, 100.353f, 270.9f, 100.354f
    path.cubic_to(
        (f32::from_bits(0x4387cccd), f32::from_bits(0x42c8b611)),
        (f32::from_bits(0x43882666), f32::from_bits(0x42c8b6ea)),
        (f32::from_bits(0x43888000), f32::from_bits(0x42c8b733)),
    ); // 271.6f, 100.356f, 272.3f, 100.357f, 273, 100.358f
    path.cubic_to(
        (f32::from_bits(0x4388d99a), f32::from_bits(0x42c8b77b)),
        (f32::from_bits(0x43893333), f32::from_bits(0x42c8b6a2)),
        (f32::from_bits(0x43898ccd), f32::from_bits(0x42c8b733)),
    ); // 273.7f, 100.358f, 274.4f, 100.357f, 275.1f, 100.358f
    path.cubic_to(
        (f32::from_bits(0x4389e666), f32::from_bits(0x42c8b7c3)),
        (f32::from_bits(0x438a4000), f32::from_bits(0x42c8ba97)),
        (f32::from_bits(0x438a999a), f32::from_bits(0x42c8ba97)),
    ); // 275.8f, 100.359f, 276.5f, 100.364f, 277.2f, 100.364f
    path.cubic_to(
        (f32::from_bits(0x438af333), f32::from_bits(0x42c8ba97)),
        (f32::from_bits(0x438b4ccd), f32::from_bits(0x42c8ba5a)),
        (f32::from_bits(0x438ba666), f32::from_bits(0x42c8b733)),
    ); // 277.9f, 100.364f, 278.6f, 100.364f, 279.3f, 100.358f
    path.cubic_to(
        (f32::from_bits(0x438c0000), f32::from_bits(0x42c8b40b)),
        (f32::from_bits(0x438c599a), f32::from_bits(0x42c8aad1)),
        (f32::from_bits(0x438cb333), f32::from_bits(0x42c8a7a9)),
    ); // 280, 100.352f, 280.7f, 100.334f, 281.4f, 100.327f
    path.cubic_to(
        (f32::from_bits(0x438d0ccd), f32::from_bits(0x42c8a481)),
        (f32::from_bits(0x438d6666), f32::from_bits(0x42c89f23)),
        (f32::from_bits(0x438dc000), f32::from_bits(0x42c8a445)),
    ); // 282.1f, 100.321f, 282.8f, 100.311f, 283.5f, 100.321f
    path.cubic_to(
        (f32::from_bits(0x438e199a), f32::from_bits(0x42c8a967)),
        (f32::from_bits(0x438e7333), f32::from_bits(0x42c8b67f)),
        (f32::from_bits(0x438ecccd), f32::from_bits(0x42c8c676)),
    ); // 284.2f, 100.331f, 284.9f, 100.356f, 285.6f, 100.388f
    path.cubic_to(
        (f32::from_bits(0x438f2666), f32::from_bits(0x42c8d66d)),
        (f32::from_bits(0x438f8000), f32::from_bits(0x42c8ecb3)),
        (f32::from_bits(0x438fd99a), f32::from_bits(0x42c9040f)),
    ); // 286.3f, 100.419f, 287, 100.462f, 287.7f, 100.508f
    path.cubic_to(
        (f32::from_bits(0x43903333), f32::from_bits(0x42c91b6b)),
        (f32::from_bits(0x43908ccd), f32::from_bits(0x42c939b1)),
        (f32::from_bits(0x4390e666), f32::from_bits(0x42c9529e)),
    ); // 288.4f, 100.554f, 289.1f, 100.613f, 289.8f, 100.661f
    path.cubic_to(
        (f32::from_bits(0x43914000), f32::from_bits(0x42c96b8a)),
        (f32::from_bits(0x4391999a), f32::from_bits(0x42c98dc5)),
        (f32::from_bits(0x4391f333), f32::from_bits(0x42c9999a)),
    ); // 290.5f, 100.71f, 291.2f, 100.777f, 291.9f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43924ccd), f32::from_bits(0x42c9a56e)),
        (f32::from_bits(0x4392a666), f32::from_bits(0x42c9999a)),
        (f32::from_bits(0x43930000), f32::from_bits(0x42c9999a)),
    ); // 292.6f, 100.823f, 293.3f, 100.8f, 294, 100.8f
    path.line_to((f32::from_bits(0x43930000), f32::from_bits(0x42c9999a))); // 294, 100.8f
    path.line_to((f32::from_bits(0x43478000), f32::from_bits(0x42c9999a))); // 199.5f, 100.8f
    path.close();
    path.move_to((f32::from_bits(0x43ab2666), f32::from_bits(0x42c9999a))); // 342.3f, 100.8f
    path.line_to((f32::from_bits(0x43ab2666), f32::from_bits(0x42c9999a))); // 342.3f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43ab8000), f32::from_bits(0x42c9999a)),
        (f32::from_bits(0x43abd99a), f32::from_bits(0x42c9a526)),
        (f32::from_bits(0x43ac3333), f32::from_bits(0x42c9999a)),
    ); // 343, 100.8f, 343.7f, 100.823f, 344.4f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43ac8ccd), f32::from_bits(0x42c98e0d)),
        (f32::from_bits(0x43ace666), f32::from_bits(0x42c9760b)),
        (f32::from_bits(0x43ad4000), f32::from_bits(0x42c95450)),
    ); // 345.1f, 100.777f, 345.8f, 100.731f, 346.5f, 100.665f
    path.cubic_to(
        (f32::from_bits(0x43ad999a), f32::from_bits(0x42c93295)),
        (f32::from_bits(0x43adf333), f32::from_bits(0x42c8ebfd)),
        (f32::from_bits(0x43ae4ccd), f32::from_bits(0x42c8cf37)),
    ); // 347.2f, 100.599f, 347.9f, 100.461f, 348.6f, 100.405f
    path.cubic_to(
        (f32::from_bits(0x43aea666), f32::from_bits(0x42c8b270)),
        (f32::from_bits(0x43af0000), f32::from_bits(0x42c8afe7)),
        (f32::from_bits(0x43af599a), f32::from_bits(0x42c8a7a9)),
    ); // 349.3f, 100.349f, 350, 100.344f, 350.7f, 100.327f
    path.cubic_to(
        (f32::from_bits(0x43afb333), f32::from_bits(0x42c89f6b)),
        (f32::from_bits(0x43b00ccd), f32::from_bits(0x42c8a08b)),
        (f32::from_bits(0x43b06666), f32::from_bits(0x42c89dc3)),
    ); // 351.4f, 100.311f, 352.1f, 100.314f, 352.8f, 100.308f
    path.cubic_to(
        (f32::from_bits(0x43b0c000), f32::from_bits(0x42c89afb)),
        (f32::from_bits(0x43b1199a), f32::from_bits(0x42c89864)),
        (f32::from_bits(0x43b17333), f32::from_bits(0x42c896fa)),
    ); // 353.5f, 100.303f, 354.2f, 100.298f, 354.9f, 100.295f
    path.cubic_to(
        (f32::from_bits(0x43b1cccd), f32::from_bits(0x42c89591)),
        (f32::from_bits(0x43b22666), f32::from_bits(0x42c89591)),
        (f32::from_bits(0x43b28000), f32::from_bits(0x42c89548)),
    ); // 355.6f, 100.292f, 356.3f, 100.292f, 357, 100.292f
    path.cubic_to(
        (f32::from_bits(0x43b2d99a), f32::from_bits(0x42c89500)),
        (f32::from_bits(0x43b33333), f32::from_bits(0x42c89500)),
        (f32::from_bits(0x43b38ccd), f32::from_bits(0x42c89548)),
    ); // 357.7f, 100.291f, 358.4f, 100.291f, 359.1f, 100.292f
    path.cubic_to(
        (f32::from_bits(0x43b3e666), f32::from_bits(0x42c89591)),
        (f32::from_bits(0x43b44000), f32::from_bits(0x42c896b2)),
        (f32::from_bits(0x43b4999a), f32::from_bits(0x42c896fa)),
    ); // 359.8f, 100.292f, 360.5f, 100.294f, 361.2f, 100.295f
    path.cubic_to(
        (f32::from_bits(0x43b4f333), f32::from_bits(0x42c89743)),
        (f32::from_bits(0x43b54ccd), f32::from_bits(0x42c896fa)),
        (f32::from_bits(0x43b5a666), f32::from_bits(0x42c896fa)),
    ); // 361.9f, 100.295f, 362.6f, 100.295f, 363.3f, 100.295f
    path.cubic_to(
        (f32::from_bits(0x43b60000), f32::from_bits(0x42c896fa)),
        (f32::from_bits(0x43b6599a), f32::from_bits(0x42c89743)),
        (f32::from_bits(0x43b6b333), f32::from_bits(0x42c896fa)),
    ); // 364, 100.295f, 364.7f, 100.295f, 365.4f, 100.295f
    path.cubic_to(
        (f32::from_bits(0x43b70ccd), f32::from_bits(0x42c896b2)),
        (f32::from_bits(0x43b76666), f32::from_bits(0x42c89591)),
        (f32::from_bits(0x43b7c000), f32::from_bits(0x42c89548)),
    ); // 366.1f, 100.294f, 366.8f, 100.292f, 367.5f, 100.292f
    path.cubic_to(
        (f32::from_bits(0x43b8199a), f32::from_bits(0x42c89500)),
        (f32::from_bits(0x43b87333), f32::from_bits(0x42c89548)),
        (f32::from_bits(0x43b8cccd), f32::from_bits(0x42c89548)),
    ); // 368.2f, 100.291f, 368.9f, 100.292f, 369.6f, 100.292f
    path.cubic_to(
        (f32::from_bits(0x43b92666), f32::from_bits(0x42c89548)),
        (f32::from_bits(0x43b98000), f32::from_bits(0x42c89548)),
        (f32::from_bits(0x43b9d99a), f32::from_bits(0x42c89548)),
    ); // 370.3f, 100.292f, 371, 100.292f, 371.7f, 100.292f
    path.cubic_to(
        (f32::from_bits(0x43ba3333), f32::from_bits(0x42c89548)),
        (f32::from_bits(0x43ba8ccd), f32::from_bits(0x42c894b7)),
        (f32::from_bits(0x43bae666), f32::from_bits(0x42c89548)),
    ); // 372.4f, 100.292f, 373.1f, 100.29f, 373.8f, 100.292f
    path.cubic_to(
        (f32::from_bits(0x43bb4000), f32::from_bits(0x42c895d9)),
        (f32::from_bits(0x43bb999a), f32::from_bits(0x42c897d3)),
        (f32::from_bits(0x43bbf333), f32::from_bits(0x42c898ac)),
    ); // 374.5f, 100.293f, 375.2f, 100.297f, 375.9f, 100.298f
    path.cubic_to(
        (f32::from_bits(0x43bc4ccd), f32::from_bits(0x42c89985)),
        (f32::from_bits(0x43bca666), f32::from_bits(0x42c89a16)),
        (f32::from_bits(0x43bd0000), f32::from_bits(0x42c89a5f)),
    ); // 376.6f, 100.3f, 377.3f, 100.301f, 378, 100.302f
    path.cubic_to(
        (f32::from_bits(0x43bd599a), f32::from_bits(0x42c89aa7)),
        (f32::from_bits(0x43bdb333), f32::from_bits(0x42c89aa7)),
        (f32::from_bits(0x43be0ccd), f32::from_bits(0x42c89a5f)),
    ); // 378.7f, 100.302f, 379.4f, 100.302f, 380.1f, 100.302f
    path.cubic_to(
        (f32::from_bits(0x43be6666), f32::from_bits(0x42c89a16)),
        (f32::from_bits(0x43bec000), f32::from_bits(0x42c8993d)),
        (f32::from_bits(0x43bf199a), f32::from_bits(0x42c898ac)),
    ); // 380.8f, 100.301f, 381.5f, 100.299f, 382.2f, 100.298f
    path.cubic_to(
        (f32::from_bits(0x43bf7333), f32::from_bits(0x42c8981c)),
        (f32::from_bits(0x43bfcccd), f32::from_bits(0x42c8978b)),
        (f32::from_bits(0x43c02666), f32::from_bits(0x42c896fa)),
    ); // 382.9f, 100.297f, 383.6f, 100.296f, 384.3f, 100.295f
    path.cubic_to(
        (f32::from_bits(0x43c08000), f32::from_bits(0x42c8966a)),
        (f32::from_bits(0x43c0d99a), f32::from_bits(0x42c8946f)),
        (f32::from_bits(0x43c13333), f32::from_bits(0x42c89548)),
    ); // 385, 100.294f, 385.7f, 100.29f, 386.4f, 100.292f
    path.cubic_to(
        (f32::from_bits(0x43c18ccd), f32::from_bits(0x42c89621)),
        (f32::from_bits(0x43c1e666), f32::from_bits(0x42c898f5)),
        (f32::from_bits(0x43c24000), f32::from_bits(0x42c89c11)),
    ); // 387.1f, 100.293f, 387.8f, 100.299f, 388.5f, 100.305f
    path.cubic_to(
        (f32::from_bits(0x43c2999a), f32::from_bits(0x42c89f2d)),
        (f32::from_bits(0x43c2f333), f32::from_bits(0x42c8a5ad)),
        (f32::from_bits(0x43c34ccd), f32::from_bits(0x42c8a7ef)),
    ); // 389.2f, 100.311f, 389.9f, 100.324f, 390.6f, 100.328f
    path.cubic_to(
        (f32::from_bits(0x43c3a666), f32::from_bits(0x42c8aa32)),
        (f32::from_bits(0x43c40000), f32::from_bits(0x42c8a9a2)),
        (f32::from_bits(0x43c4599a), f32::from_bits(0x42c8a9a2)),
    ); // 391.3f, 100.332f, 392, 100.331f, 392.7f, 100.331f
    path.cubic_to(
        (f32::from_bits(0x43c4b333), f32::from_bits(0x42c8a9a2)),
        (f32::from_bits(0x43c50ccd), f32::from_bits(0x42c8a8c9)),
        (f32::from_bits(0x43c56666), f32::from_bits(0x42c8a7ef)),
    ); // 393.4f, 100.331f, 394.1f, 100.33f, 394.8f, 100.328f
    path.cubic_to(
        (f32::from_bits(0x43c5c000), f32::from_bits(0x42c8a716)),
        (f32::from_bits(0x43c6199a), f32::from_bits(0x42c8a5ad)),
        (f32::from_bits(0x43c67333), f32::from_bits(0x42c8a48b)),
    ); // 395.5f, 100.326f, 396.2f, 100.324f, 396.9f, 100.321f
    path.cubic_to(
        (f32::from_bits(0x43c6cccd), f32::from_bits(0x42c8a36a)),
        (f32::from_bits(0x43c72666), f32::from_bits(0x42c8a291)),
        (f32::from_bits(0x43c78000), f32::from_bits(0x42c8a127)),
    ); // 397.6f, 100.319f, 398.3f, 100.318f, 399, 100.315f
    path.cubic_to(
        (f32::from_bits(0x43c7d99a), f32::from_bits(0x42c89fbd)),
        (f32::from_bits(0x43c83333), f32::from_bits(0x42c89dc3)),
        (f32::from_bits(0x43c88ccd), f32::from_bits(0x42c89c11)),
    ); // 399.7f, 100.312f, 400.4f, 100.308f, 401.1f, 100.305f
    path.cubic_to(
        (f32::from_bits(0x43c8e666), f32::from_bits(0x42c89a5f)),
        (f32::from_bits(0x43c94000), f32::from_bits(0x42c898ac)),
        (f32::from_bits(0x43c9999a), f32::from_bits(0x42c896fa)),
    ); // 401.8f, 100.302f, 402.5f, 100.298f, 403.2f, 100.295f
    path.cubic_to(
        (f32::from_bits(0x43c9f333), f32::from_bits(0x42c89548)),
        (f32::from_bits(0x43ca4ccd), f32::from_bits(0x42c89305)),
        (f32::from_bits(0x43caa666), f32::from_bits(0x42c891e4)),
    ); // 403.9f, 100.292f, 404.6f, 100.287f, 405.3f, 100.285f
    path.cubic_to(
        (f32::from_bits(0x43cb0000), f32::from_bits(0x42c890c3)),
        (f32::from_bits(0x43cb599a), f32::from_bits(0x42c88ec8)),
        (f32::from_bits(0x43cbb333), f32::from_bits(0x42c89032)),
    ); // 406, 100.283f, 406.7f, 100.279f, 407.4f, 100.282f
    path.cubic_to(
        (f32::from_bits(0x43cc0ccd), f32::from_bits(0x42c8919c)),
        (f32::from_bits(0x43cc6666), f32::from_bits(0x42c89864)),
        (f32::from_bits(0x43ccc000), f32::from_bits(0x42c89a5f)),
    ); // 408.1f, 100.284f, 408.8f, 100.298f, 409.5f, 100.302f
    path.cubic_to(
        (f32::from_bits(0x43cd199a), f32::from_bits(0x42c89c59)),
        (f32::from_bits(0x43cd7333), f32::from_bits(0x42c89dff)),
        (f32::from_bits(0x43cdcccd), f32::from_bits(0x42c89c11)),
    ); // 410.2f, 100.305f, 410.9f, 100.309f, 411.6f, 100.305f
    path.cubic_to(
        (f32::from_bits(0x43ce2666), f32::from_bits(0x42c89a22)),
        (f32::from_bits(0x43ce8000), f32::from_bits(0x42c8919a)),
        (f32::from_bits(0x43ced99a), f32::from_bits(0x42c88ec6)),
    ); // 412.3f, 100.301f, 413, 100.284f, 413.7f, 100.279f
    path.cubic_to(
        (f32::from_bits(0x43cf3333), f32::from_bits(0x42c88bf3)),
        (f32::from_bits(0x43cf8ccd), f32::from_bits(0x42c88b70)),
        (f32::from_bits(0x43cfe666), f32::from_bits(0x42c88b1b)),
    ); // 414.4f, 100.273f, 415.1f, 100.272f, 415.8f, 100.272f
    path.cubic_to(
        (f32::from_bits(0x43d04000), f32::from_bits(0x42c88ac7)),
        (f32::from_bits(0x43d0999a), f32::from_bits(0x42c88bac)),
        (f32::from_bits(0x43d0f333), f32::from_bits(0x42c88cce)),
    ); // 416.5f, 100.271f, 417.2f, 100.273f, 417.9f, 100.275f
    path.cubic_to(
        (f32::from_bits(0x43d14ccd), f32::from_bits(0x42c88def)),
        (f32::from_bits(0x43d1a666), f32::from_bits(0x42c89032)),
        (f32::from_bits(0x43d20000), f32::from_bits(0x42c891e4)),
    ); // 418.6f, 100.277f, 419.3f, 100.282f, 420, 100.285f
    path.cubic_to(
        (f32::from_bits(0x43d2599a), f32::from_bits(0x42c89396)),
        (f32::from_bits(0x43d2b333), f32::from_bits(0x42c89621)),
        (f32::from_bits(0x43d30ccd), f32::from_bits(0x42c896fa)),
    ); // 420.7f, 100.288f, 421.4f, 100.293f, 422.1f, 100.295f
    path.cubic_to(
        (f32::from_bits(0x43d36666), f32::from_bits(0x42c897d3)),
        (f32::from_bits(0x43d3c000), f32::from_bits(0x42c89810)),
        (f32::from_bits(0x43d4199a), f32::from_bits(0x42c896fa)),
    ); // 422.8f, 100.297f, 423.5f, 100.297f, 424.2f, 100.295f
    path.cubic_to(
        (f32::from_bits(0x43d47333), f32::from_bits(0x42c895e5)),
        (f32::from_bits(0x43d4cccd), f32::from_bits(0x42c88d99)),
        (f32::from_bits(0x43d52666), f32::from_bits(0x42c89078)),
    ); // 424.9f, 100.293f, 425.6f, 100.277f, 426.3f, 100.282f
    path.cubic_to(
        (f32::from_bits(0x43d58000), f32::from_bits(0x42c89358)),
        (f32::from_bits(0x43d5d99a), f32::from_bits(0x42c88f0f)),
        (f32::from_bits(0x43d63333), f32::from_bits(0x42c8a836)),
    ); // 427, 100.288f, 427.7f, 100.279f, 428.4f, 100.329f
    path.cubic_to(
        (f32::from_bits(0x43d68ccd), f32::from_bits(0x42c8c15e)),
        (f32::from_bits(0x43d6e666), f32::from_bits(0x42c8ff2a)),
        (f32::from_bits(0x43d74000), f32::from_bits(0x42c92765)),
    ); // 429.1f, 100.378f, 429.8f, 100.498f, 430.5f, 100.577f
    path.cubic_to(
        (f32::from_bits(0x43d7999a), f32::from_bits(0x42c94fa0)),
        (f32::from_bits(0x43d7f333), f32::from_bits(0x42c98691)),
        (f32::from_bits(0x43d84ccd), f32::from_bits(0x42c9999a)),
    ); // 431.2f, 100.656f, 431.9f, 100.763f, 432.6f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43d8a666), f32::from_bits(0x42c9aca2)),
        (f32::from_bits(0x43d90000), f32::from_bits(0x42c9999a)),
        (f32::from_bits(0x43d9599a), f32::from_bits(0x42c9999a)),
    ); // 433.3f, 100.837f, 434, 100.8f, 434.7f, 100.8f
    path.line_to((f32::from_bits(0x43d9599a), f32::from_bits(0x42c9999a))); // 434.7f, 100.8f
    path.line_to((f32::from_bits(0x43ab2666), f32::from_bits(0x42c9999a))); // 342.3f, 100.8f
    path.close();
    path.move_to((f32::from_bits(0x43dfa666), f32::from_bits(0x42c9999a))); // 447.3f, 100.8f
    path.line_to((f32::from_bits(0x43dfa666), f32::from_bits(0x42c9999a))); // 447.3f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43e00000), f32::from_bits(0x42c99877)),
        (f32::from_bits(0x43e0599a), f32::from_bits(0x42c99312)),
        (f32::from_bits(0x43e0b333), f32::from_bits(0x42c992c8)),
    ); // 448, 100.798f, 448.7f, 100.787f, 449.4f, 100.787f
    path.cubic_to(
        (f32::from_bits(0x43e10ccd), f32::from_bits(0x42c9927e)),
        (f32::from_bits(0x43e16666), f32::from_bits(0x42c996bb)),
        (f32::from_bits(0x43e1c000), f32::from_bits(0x42c997de)),
    ); // 450.1f, 100.786f, 450.8f, 100.794f, 451.5f, 100.797f
    path.cubic_to(
        (f32::from_bits(0x43e2199a), f32::from_bits(0x42c99901)),
        (f32::from_bits(0x43e27333), f32::from_bits(0x42c9a0aa)),
        (f32::from_bits(0x43e2cccd), f32::from_bits(0x42c9999a)),
    ); // 452.2f, 100.799f, 452.9f, 100.814f, 453.6f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43e32666), f32::from_bits(0x42c99289)),
        (f32::from_bits(0x43e38000), f32::from_bits(0x42c97cb2)),
        (f32::from_bits(0x43e3d99a), f32::from_bits(0x42c96d79)),
    ); // 454.3f, 100.786f, 455, 100.744f, 455.7f, 100.714f
    path.cubic_to(
        (f32::from_bits(0x43e43333), f32::from_bits(0x42c95e40)),
        (f32::from_bits(0x43e48ccd), f32::from_bits(0x42c943a3)),
        (f32::from_bits(0x43e4e666), f32::from_bits(0x42c93e44)),
    ); // 456.4f, 100.684f, 457.1f, 100.632f, 457.8f, 100.622f
    path.cubic_to(
        (f32::from_bits(0x43e54000), f32::from_bits(0x42c938e6)),
        (f32::from_bits(0x43e5999a), f32::from_bits(0x42c949f4)),
        (f32::from_bits(0x43e5f333), f32::from_bits(0x42c94d41)),
    ); // 458.5f, 100.611f, 459.2f, 100.644f, 459.9f, 100.651f
    path.cubic_to(
        (f32::from_bits(0x43e64ccd), f32::from_bits(0x42c9508e)),
        (f32::from_bits(0x43e6a666), f32::from_bits(0x42c94fce)),
        (f32::from_bits(0x43e70000), f32::from_bits(0x42c95211)),
    ); // 460.6f, 100.657f, 461.3f, 100.656f, 462, 100.66f
    path.cubic_to(
        (f32::from_bits(0x43e7599a), f32::from_bits(0x42c95454)),
        (f32::from_bits(0x43e7b333), f32::from_bits(0x42c9595c)),
        (f32::from_bits(0x43e80ccd), f32::from_bits(0x42c95ad2)),
    ); // 462.7f, 100.665f, 463.4f, 100.675f, 464.1f, 100.677f
    path.cubic_to(
        (f32::from_bits(0x43e86666), f32::from_bits(0x42c95c47)),
        (f32::from_bits(0x43e8c000), f32::from_bits(0x42c959f9)),
        (f32::from_bits(0x43e9199a), f32::from_bits(0x42c95ad2)),
    ); // 464.8f, 100.68f, 465.5f, 100.676f, 466.2f, 100.677f
    path.cubic_to(
        (f32::from_bits(0x43e97333), f32::from_bits(0x42c95bab)),
        (f32::from_bits(0x43e9cccd), f32::from_bits(0x42c95d20)),
        (f32::from_bits(0x43ea2666), f32::from_bits(0x42c95fe8)),
    ); // 466.9f, 100.679f, 467.6f, 100.682f, 468.3f, 100.687f
    path.cubic_to(
        (f32::from_bits(0x43ea8000), f32::from_bits(0x42c962b0)),
        (f32::from_bits(0x43ead99a), f32::from_bits(0x42c96743)),
        (f32::from_bits(0x43eb3333), f32::from_bits(0x42c96b80)),
    ); // 469, 100.693f, 469.7f, 100.702f, 470.4f, 100.71f
    path.cubic_to(
        (f32::from_bits(0x43eb8ccd), f32::from_bits(0x42c96fbe)),
        (f32::from_bits(0x43ebe666), f32::from_bits(0x42c9754c)),
        (f32::from_bits(0x43ec4000), f32::from_bits(0x42c97958)),
    ); // 471.1f, 100.718f, 471.8f, 100.729f, 472.5f, 100.737f
    path.cubic_to(
        (f32::from_bits(0x43ec999a), f32::from_bits(0x42c97d65)),
        (f32::from_bits(0x43ecf333), f32::from_bits(0x42c97ef2)),
        (f32::from_bits(0x43ed4ccd), f32::from_bits(0x42c983cb)),
    ); // 473.2f, 100.745f, 473.9f, 100.748f, 474.6f, 100.757f
    path.cubic_to(
        (f32::from_bits(0x43eda666), f32::from_bits(0x42c988a5)),
        (f32::from_bits(0x43ee0000), f32::from_bits(0x42c992d0)),
        (f32::from_bits(0x43ee599a), f32::from_bits(0x42c99673)),
    ); // 475.3f, 100.767f, 476, 100.787f, 476.7f, 100.794f
    path.cubic_to(
        (f32::from_bits(0x43eeb333), f32::from_bits(0x42c99a15)),
        (f32::from_bits(0x43ef0ccd), f32::from_bits(0x42c99bdc)),
        (f32::from_bits(0x43ef6666), f32::from_bits(0x42c9999a)),
    ); // 477.4f, 100.801f, 478.1f, 100.804f, 478.8f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43efc000), f32::from_bits(0x42c99757)),
        (f32::from_bits(0x43f0199a), f32::from_bits(0x42c99426)),
        (f32::from_bits(0x43f07333), f32::from_bits(0x42c988e2)),
    ); // 479.5f, 100.796f, 480.2f, 100.789f, 480.9f, 100.767f
    path.cubic_to(
        (f32::from_bits(0x43f0cccd), f32::from_bits(0x42c97d9e)),
        (f32::from_bits(0x43f12666), f32::from_bits(0x42c96356)),
        (f32::from_bits(0x43f18000), f32::from_bits(0x42c95602)),
    ); // 481.6f, 100.745f, 482.3f, 100.694f, 483, 100.668f
    path.cubic_to(
        (f32::from_bits(0x43f1d99a), f32::from_bits(0x42c948ae)),
        (f32::from_bits(0x43f23333), f32::from_bits(0x42c93ee3)),
        (f32::from_bits(0x43f28ccd), f32::from_bits(0x42c938e7)),
    ); // 483.7f, 100.642f, 484.4f, 100.623f, 485.1f, 100.611f
    path.cubic_to(
        (f32::from_bits(0x43f2e666), f32::from_bits(0x42c932ec)),
        (f32::from_bits(0x43f34000), f32::from_bits(0x42c93741)),
        (f32::from_bits(0x43f3999a), f32::from_bits(0x42c9321f)),
    ); // 485.8f, 100.599f, 486.5f, 100.608f, 487.2f, 100.598f
    path.cubic_to(
        (f32::from_bits(0x43f3f333), f32::from_bits(0x42c92cfd)),
        (f32::from_bits(0x43f44ccd), f32::from_bits(0x42c922f5)),
        (f32::from_bits(0x43f4a666), f32::from_bits(0x42c91a1b)),
    ); // 487.9f, 100.588f, 488.6f, 100.568f, 489.3f, 100.551f
    path.cubic_to(
        (f32::from_bits(0x43f50000), f32::from_bits(0x42c91140)),
        (f32::from_bits(0x43f5599a), f32::from_bits(0x42c904ad)),
        (f32::from_bits(0x43f5b333), f32::from_bits(0x42c8fd00)),
    ); // 490, 100.534f, 490.7f, 100.509f, 491.4f, 100.494f
    path.cubic_to(
        (f32::from_bits(0x43f60ccd), f32::from_bits(0x42c8f553)),
        (f32::from_bits(0x43f66666), f32::from_bits(0x42c8ef7b)),
        (f32::from_bits(0x43f6c000), f32::from_bits(0x42c8ec0b)),
    ); // 492.1f, 100.479f, 492.8f, 100.468f, 493.5f, 100.461f
    path.cubic_to(
        (f32::from_bits(0x43f7199a), f32::from_bits(0x42c8e89b)),
        (f32::from_bits(0x43f77333), f32::from_bits(0x42c8e981)),
        (f32::from_bits(0x43f7cccd), f32::from_bits(0x42c8e860)),
    ); // 494.2f, 100.454f, 494.9f, 100.456f, 495.6f, 100.454f
    path.cubic_to(
        (f32::from_bits(0x43f82666), f32::from_bits(0x42c8e73f)),
        (f32::from_bits(0x43f88000), f32::from_bits(0x42c8e658)),
        (f32::from_bits(0x43f8d99a), f32::from_bits(0x42c8e542)),
    ); // 496.3f, 100.452f, 497, 100.45f, 497.7f, 100.448f
    path.cubic_to(
        (f32::from_bits(0x43f93333), f32::from_bits(0x42c8e42d)),
        (f32::from_bits(0x43f98ccd), f32::from_bits(0x42c8e348)),
        (f32::from_bits(0x43f9e666), f32::from_bits(0x42c8e1de)),
    ); // 498.4f, 100.446f, 499.1f, 100.444f, 499.8f, 100.441f
    path.cubic_to(
        (f32::from_bits(0x43fa4000), f32::from_bits(0x42c8e074)),
        (f32::from_bits(0x43fa999a), f32::from_bits(0x42c8df53)),
        (f32::from_bits(0x43faf333), f32::from_bits(0x42c8dcc8)),
    ); // 500.5f, 100.438f, 501.2f, 100.436f, 501.9f, 100.431f
    path.cubic_to(
        (f32::from_bits(0x43fb4ccd), f32::from_bits(0x42c8da3d)),
        (f32::from_bits(0x43fba666), f32::from_bits(0x42c8d5c3)),
        (f32::from_bits(0x43fc0000), f32::from_bits(0x42c8d29b)),
    ); // 502.6f, 100.426f, 503.3f, 100.418f, 504, 100.411f
    path.cubic_to(
        (f32::from_bits(0x43fc599a), f32::from_bits(0x42c8cf73)),
        (f32::from_bits(0x43fcb333), f32::from_bits(0x42c8ccf6)),
        (f32::from_bits(0x43fd0ccd), f32::from_bits(0x42c8c9da)),
    ); // 504.7f, 100.405f, 505.4f, 100.4f, 506.1f, 100.394f
    path.cubic_to(
        (f32::from_bits(0x43fd6666), f32::from_bits(0x42c8c6be)),
        (f32::from_bits(0x43fdc000), f32::from_bits(0x42c8c310)),
        (f32::from_bits(0x43fe199a), f32::from_bits(0x42c8bff4)),
    ); // 506.8f, 100.388f, 507.5f, 100.381f, 508.2f, 100.375f
    path.cubic_to(
        (f32::from_bits(0x43fe7333), f32::from_bits(0x42c8bcd8)),
        (f32::from_bits(0x43fecccd), f32::from_bits(0x42c8b8f0)),
        (f32::from_bits(0x43ff2666), f32::from_bits(0x42c8b733)),
    ); // 508.9f, 100.369f, 509.6f, 100.361f, 510.3f, 100.358f
    path.cubic_to(
        (f32::from_bits(0x43ff8000), f32::from_bits(0x42c8b575)),
        (f32::from_bits(0x43ffd99a), f32::from_bits(0x42c8b45f)),
        (f32::from_bits(0x4400199a), f32::from_bits(0x42c8b580)),
    ); // 511, 100.354f, 511.7f, 100.352f, 512.4f, 100.354f
    path.cubic_to(
        (f32::from_bits(0x44004666), f32::from_bits(0x42c8b6a2)),
        (f32::from_bits(0x44007333), f32::from_bits(0x42c8bb28)),
        (f32::from_bits(0x4400a000), f32::from_bits(0x42c8bdfb)),
    ); // 513.1f, 100.357f, 513.8f, 100.366f, 514.5f, 100.371f
    path.cubic_to(
        (f32::from_bits(0x4400cccd), f32::from_bits(0x42c8c0cf)),
        (f32::from_bits(0x4400f99a), f32::from_bits(0x42c8c31d)),
        (f32::from_bits(0x44012666), f32::from_bits(0x42c8c676)),
    ); // 515.2f, 100.377f, 515.9f, 100.381f, 516.6f, 100.388f
    path.cubic_to(
        (f32::from_bits(0x44015333), f32::from_bits(0x42c8c9ce)),
        (f32::from_bits(0x44018000), f32::from_bits(0x42c8caa9)),
        (f32::from_bits(0x4401accd), f32::from_bits(0x42c8d20e)),
    ); // 517.3f, 100.394f, 518, 100.396f, 518.7f, 100.41f
    path.cubic_to(
        (f32::from_bits(0x4401d99a), f32::from_bits(0x42c8d973)),
        (f32::from_bits(0x44020666), f32::from_bits(0x42c8e446)),
        (f32::from_bits(0x44023333), f32::from_bits(0x42c8f2d3)),
    ); // 519.4f, 100.425f, 520.1f, 100.446f, 520.8f, 100.474f
    path.cubic_to(
        (f32::from_bits(0x44026000), f32::from_bits(0x42c90161)),
        (f32::from_bits(0x44028ccd), f32::from_bits(0x42c917c0)),
        (f32::from_bits(0x4402b99a), f32::from_bits(0x42c9295e)),
    ); // 521.5f, 100.503f, 522.2f, 100.546f, 522.9f, 100.581f
    path.cubic_to(
        (f32::from_bits(0x4402e666), f32::from_bits(0x42c93afb)),
        (f32::from_bits(0x44031333), f32::from_bits(0x42c94c14)),
        (f32::from_bits(0x44034000), f32::from_bits(0x42c95c84)),
    ); // 523.6f, 100.615f, 524.3f, 100.649f, 525, 100.681f
    path.cubic_to(
        (f32::from_bits(0x44036ccd), f32::from_bits(0x42c96cf4)),
        (f32::from_bits(0x4403999a), f32::from_bits(0x42c981d1)),
        (f32::from_bits(0x4403c666), f32::from_bits(0x42c98bff)),
    ); // 525.7f, 100.713f, 526.4f, 100.754f, 527.1f, 100.773f
    path.cubic_to(
        (f32::from_bits(0x4403f333), f32::from_bits(0x42c9962e)),
        (f32::from_bits(0x44042000), f32::from_bits(0x42c99755)),
        (f32::from_bits(0x44044ccd), f32::from_bits(0x42c9999a)),
    ); // 527.8f, 100.793f, 528.5f, 100.796f, 529.2f, 100.8f
    path.line_to((f32::from_bits(0x44044ccd), f32::from_bits(0x42c9999a))); // 529.2f, 100.8f
    path.line_to((f32::from_bits(0x43dfa666), f32::from_bits(0x42c9999a))); // 447.3f, 100.8f
    path.close();
    path.detach()
}

// Port of: tests/PathOpsIssue3651.cpp#L1021-L1210 (chrome/m156)
fn path6() -> Path {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x42b06666), f32::from_bits(0x42c9999a))); // 88.2f, 100.8f
    path.line_to((f32::from_bits(0x42b06666), f32::from_bits(0x42c9999a))); // 88.2f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x42b1cccd), f32::from_bits(0x42c9999a)),
        (f32::from_bits(0x42b33333), f32::from_bits(0x42c97f2d)),
        (f32::from_bits(0x42b4999a), f32::from_bits(0x42c9999a)),
    ); // 88.9f, 100.8f, 89.6f, 100.748f, 90.3f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x42b60000), f32::from_bits(0x42c9b408)),
        (f32::from_bits(0x42b76666), f32::from_bits(0x42ca1e13)),
        (f32::from_bits(0x42b8cccd), f32::from_bits(0x42ca382d)),
    ); // 91, 100.852f, 91.7f, 101.059f, 92.4f, 101.11f
    path.cubic_to(
        (f32::from_bits(0x42ba3333), f32::from_bits(0x42ca5246)),
        (f32::from_bits(0x42bb999a), f32::from_bits(0x42ca3634)),
        (f32::from_bits(0x42bd0000), f32::from_bits(0x42ca3634)),
    ); // 93.1f, 101.161f, 93.8f, 101.106f, 94.5f, 101.106f
    path.cubic_to(
        (f32::from_bits(0x42be6666), f32::from_bits(0x42ca3634)),
        (f32::from_bits(0x42bfcccd), f32::from_bits(0x42ca3869)),
        (f32::from_bits(0x42c13333), f32::from_bits(0x42ca382d)),
    ); // 95.2f, 101.106f, 95.9f, 101.11f, 96.6f, 101.11f
    path.cubic_to(
        (f32::from_bits(0x42c2999a), f32::from_bits(0x42ca37f0)),
        (f32::from_bits(0x42c40000), f32::from_bits(0x42ca35ea)),
        (f32::from_bits(0x42c56666), f32::from_bits(0x42ca34c8)),
    ); // 97.3f, 101.109f, 98, 101.105f, 98.7f, 101.103f
    path.cubic_to(
        (f32::from_bits(0x42c6cccd), f32::from_bits(0x42ca33a7)),
        (f32::from_bits(0x42c83333), f32::from_bits(0x42ca311c)),
        (f32::from_bits(0x42c9999a), f32::from_bits(0x42ca3164)),
    ); // 99.4f, 101.101f, 100.1f, 101.096f, 100.8f, 101.096f
    path.cubic_to(
        (f32::from_bits(0x42cb0000), f32::from_bits(0x42ca31ad)),
        (f32::from_bits(0x42cc6666), f32::from_bits(0x42ca33fb)),
        (f32::from_bits(0x42cdcccd), f32::from_bits(0x42ca367b)),
    ); // 101.5f, 101.097f, 102.2f, 101.102f, 102.9f, 101.106f
    path.cubic_to(
        (f32::from_bits(0x42cf3333), f32::from_bits(0x42ca38fa)),
        (f32::from_bits(0x42d0999a), f32::from_bits(0x42ca3fd0)),
        (f32::from_bits(0x42d20000), f32::from_bits(0x42ca4061)),
    ); // 103.6f, 101.111f, 104.3f, 101.125f, 105, 101.126f
    path.cubic_to(
        (f32::from_bits(0x42d36666), f32::from_bits(0x42ca40f1)),
        (f32::from_bits(0x42d4cccd), f32::from_bits(0x42ca3f32)),
        (f32::from_bits(0x42d63333), f32::from_bits(0x42ca39df)),
    ); // 105.7f, 101.127f, 106.4f, 101.123f, 107.1f, 101.113f
    path.cubic_to(
        (f32::from_bits(0x42d7999a), f32::from_bits(0x42ca348c)),
        (f32::from_bits(0x42d90000), f32::from_bits(0x42ca2585)),
        (f32::from_bits(0x42da6666), f32::from_bits(0x42ca206f)),
    ); // 107.8f, 101.103f, 108.5f, 101.073f, 109.2f, 101.063f
    path.cubic_to(
        (f32::from_bits(0x42dbcccd), f32::from_bits(0x42ca1b59)),
        (f32::from_bits(0x42dd3333), f32::from_bits(0x42ca1a43)),
        (f32::from_bits(0x42de999a), f32::from_bits(0x42ca1b59)),
    ); // 109.9f, 101.053f, 110.6f, 101.051f, 111.3f, 101.053f
    path.cubic_to(
        (f32::from_bits(0x42e00000), f32::from_bits(0x42ca1c6e)),
        (f32::from_bits(0x42e16666), f32::from_bits(0x42ca2872)),
        (f32::from_bits(0x42e2cccd), f32::from_bits(0x42ca26f1)),
    ); // 112, 101.056f, 112.7f, 101.079f, 113.4f, 101.076f
    path.cubic_to(
        (f32::from_bits(0x42e43333), f32::from_bits(0x42ca2570)),
        (f32::from_bits(0x42e5999a), f32::from_bits(0x42ca1f14)),
        (f32::from_bits(0x42e70000), f32::from_bits(0x42ca1251)),
    ); // 114.1f, 101.073f, 114.8f, 101.061f, 115.5f, 101.036f
    path.cubic_to(
        (f32::from_bits(0x42e86666), f32::from_bits(0x42ca058d)),
        (f32::from_bits(0x42e9cccd), f32::from_bits(0x42c9ec89)),
        (f32::from_bits(0x42eb3333), f32::from_bits(0x42c9da5b)),
    ); // 116.2f, 101.011f, 116.9f, 100.962f, 117.6f, 100.926f
    path.cubic_to(
        (f32::from_bits(0x42ec999a), f32::from_bits(0x42c9c82d)),
        (f32::from_bits(0x42ee0000), f32::from_bits(0x42c9b007)),
        (f32::from_bits(0x42ef6666), f32::from_bits(0x42c9a53c)),
    ); // 118.3f, 100.891f, 119, 100.844f, 119.7f, 100.823f
    path.cubic_to(
        (f32::from_bits(0x42f0cccd), f32::from_bits(0x42c99a71)),
        (f32::from_bits(0x42f23333), f32::from_bits(0x42c99b8b)),
        (f32::from_bits(0x42f3999a), f32::from_bits(0x42c9999a)),
    ); // 120.4f, 100.802f, 121.1f, 100.804f, 121.8f, 100.8f
    path.line_to((f32::from_bits(0x42f3999a), f32::from_bits(0x42c9999a))); // 121.8f, 100.8f
    path.line_to((f32::from_bits(0x42b06666), f32::from_bits(0x42c9999a))); // 88.2f, 100.8f
    path.close();
    path.move_to((f32::from_bits(0x4300199a), f32::from_bits(0x42c9999a))); // 128.1f, 100.8f
    path.line_to((f32::from_bits(0x4300199a), f32::from_bits(0x42c99b0f))); // 128.1f, 100.803f
    path.cubic_to(
        (f32::from_bits(0x4300cccd), f32::from_bits(0x42c99bde)),
        (f32::from_bits(0x43018000), f32::from_bits(0x42c99eb2)),
        (f32::from_bits(0x43023333), f32::from_bits(0x42c99e73)),
    ); // 128.8f, 100.804f, 129.5f, 100.81f, 130.2f, 100.809f
    path.cubic_to(
        (f32::from_bits(0x4302e666), f32::from_bits(0x42c99e35)),
        (f32::from_bits(0x4303999a), f32::from_bits(0x42c99a69)),
        (f32::from_bits(0x43044ccd), f32::from_bits(0x42c9999a)),
    ); // 130.9f, 100.809f, 131.6f, 100.802f, 132.3f, 100.8f
    path.line_to((f32::from_bits(0x43044ccd), f32::from_bits(0x42c9999a))); // 132.3f, 100.8f
    path.line_to((f32::from_bits(0x4300199a), f32::from_bits(0x42c9999a))); // 128.1f, 100.8f
    path.close();
    path.move_to((f32::from_bits(0x431b6666), f32::from_bits(0x42c9999a))); // 155.4f, 100.8f
    path.line_to((f32::from_bits(0x431b6666), f32::from_bits(0x42c9999a))); // 155.4f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x431c199a), f32::from_bits(0x42c9999a)),
        (f32::from_bits(0x431ccccd), f32::from_bits(0x42c99567)),
        (f32::from_bits(0x431d8000), f32::from_bits(0x42c9999a)),
    ); // 156.1f, 100.8f, 156.8f, 100.792f, 157.5f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x431e3333), f32::from_bits(0x42c99dcd)),
        (f32::from_bits(0x431ee666), f32::from_bits(0x42c9a613)),
        (f32::from_bits(0x431f999a), f32::from_bits(0x42c9b2cd)),
    ); // 158.2f, 100.808f, 158.9f, 100.824f, 159.6f, 100.849f
    path.cubic_to(
        (f32::from_bits(0x43204ccd), f32::from_bits(0x42c9bf86)),
        (f32::from_bits(0x43210000), f32::from_bits(0x42c9d893)),
        (f32::from_bits(0x4321b333), f32::from_bits(0x42c9e5f3)),
    ); // 160.3f, 100.874f, 161, 100.923f, 161.7f, 100.949f
    path.cubic_to(
        (f32::from_bits(0x43226666), f32::from_bits(0x42c9f353)),
        (f32::from_bits(0x4323199a), f32::from_bits(0x42c9fd4f)),
        (f32::from_bits(0x4323cccd), f32::from_bits(0x42ca030e)),
    ); // 162.4f, 100.975f, 163.1f, 100.995f, 163.8f, 101.006f
    path.cubic_to(
        (f32::from_bits(0x43248000), f32::from_bits(0x42ca08cc)),
        (f32::from_bits(0x43253333), f32::from_bits(0x42ca07ce)),
        (f32::from_bits(0x4325e666), f32::from_bits(0x42ca086b)),
    ); // 164.5f, 101.017f, 165.2f, 101.015f, 165.9f, 101.016f
    path.cubic_to(
        (f32::from_bits(0x4326999a), f32::from_bits(0x42ca0907)),
        (f32::from_bits(0x43274ccd), f32::from_bits(0x42ca0bdb)),
        (f32::from_bits(0x43280000), f32::from_bits(0x42ca06b9)),
    ); // 166.6f, 101.018f, 167.3f, 101.023f, 168, 101.013f
    path.cubic_to(
        (f32::from_bits(0x4328b333), f32::from_bits(0x42ca0196)),
        (f32::from_bits(0x43296666), f32::from_bits(0x42c9f5dd)),
        (f32::from_bits(0x432a199a), f32::from_bits(0x42c9e99e)),
    ); // 168.7f, 101.003f, 169.4f, 100.98f, 170.1f, 100.956f
    path.cubic_to(
        (f32::from_bits(0x432acccd), f32::from_bits(0x42c9dd5f)),
        (f32::from_bits(0x432b8000), f32::from_bits(0x42c9ca4c)),
        (f32::from_bits(0x432c3333), f32::from_bits(0x42c9bd40)),
    ); // 170.8f, 100.932f, 171.5f, 100.895f, 172.2f, 100.87f
    path.cubic_to(
        (f32::from_bits(0x432ce666), f32::from_bits(0x42c9b034)),
        (f32::from_bits(0x432d999a), f32::from_bits(0x42c9a147)),
        (f32::from_bits(0x432e4ccd), f32::from_bits(0x42c99b56)),
    ); // 172.9f, 100.844f, 173.6f, 100.815f, 174.3f, 100.803f
    path.cubic_to(
        (f32::from_bits(0x432f0000), f32::from_bits(0x42c99565)),
        (f32::from_bits(0x432fb333), f32::from_bits(0x42c999e4)),
        (f32::from_bits(0x43306666), f32::from_bits(0x42c9999a)),
    ); // 175, 100.792f, 175.7f, 100.801f, 176.4f, 100.8f
    path.line_to((f32::from_bits(0x43306666), f32::from_bits(0x42c9999a))); // 176.4f, 100.8f
    path.line_to((f32::from_bits(0x431b6666), f32::from_bits(0x42c9999a))); // 155.4f, 100.8f
    path.close();
    path.move_to((f32::from_bits(0x43478000), f32::from_bits(0x42c9999a))); // 199.5f, 100.8f
    path.line_to((f32::from_bits(0x43478000), f32::from_bits(0x42c9999a))); // 199.5f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43483333), f32::from_bits(0x42c9999a)),
        (f32::from_bits(0x4348e666), f32::from_bits(0x42c99640)),
        (f32::from_bits(0x4349999a), f32::from_bits(0x42c9999a)),
    ); // 200.2f, 100.8f, 200.9f, 100.793f, 201.6f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x434a4ccd), f32::from_bits(0x42c99cf4)),
        (f32::from_bits(0x434b0000), f32::from_bits(0x42c9a741)),
        (f32::from_bits(0x434bb333), f32::from_bits(0x42c9adb7)),
    ); // 202.3f, 100.807f, 203, 100.827f, 203.7f, 100.839f
    path.cubic_to(
        (f32::from_bits(0x434c6666), f32::from_bits(0x42c9b42c)),
        (f32::from_bits(0x434d199a), f32::from_bits(0x42c9bc15)),
        (f32::from_bits(0x434dcccd), f32::from_bits(0x42c9c05e)),
    ); // 204.4f, 100.852f, 205.1f, 100.867f, 205.8f, 100.876f
    path.cubic_to(
        (f32::from_bits(0x434e8000), f32::from_bits(0x42c9c4a7)),
        (f32::from_bits(0x434f3333), f32::from_bits(0x42c9c572)),
        (f32::from_bits(0x434fe666), f32::from_bits(0x42c9c76d)),
    ); // 206.5f, 100.884f, 207.2f, 100.886f, 207.9f, 100.89f
    path.cubic_to(
        (f32::from_bits(0x4350999a), f32::from_bits(0x42c9c967)),
        (f32::from_bits(0x43514ccd), f32::from_bits(0x42c9cb6f)),
        (f32::from_bits(0x43520000), f32::from_bits(0x42c9cc3d)),
    ); // 208.6f, 100.893f, 209.3f, 100.897f, 210, 100.899f
    path.cubic_to(
        (f32::from_bits(0x4352b333), f32::from_bits(0x42c9cd0a)),
        (f32::from_bits(0x43536666), f32::from_bits(0x42c9cc85)),
        (f32::from_bits(0x4354199a), f32::from_bits(0x42c9cc3d)),
    ); // 210.7f, 100.9f, 211.4f, 100.899f, 212.1f, 100.899f
    path.cubic_to(
        (f32::from_bits(0x4354cccd), f32::from_bits(0x42c9cbf4)),
        (f32::from_bits(0x43558000), f32::from_bits(0x42c9c7f4)),
        (f32::from_bits(0x43563333), f32::from_bits(0x42c9ca8b)),
    ); // 212.8f, 100.898f, 213.5f, 100.891f, 214.2f, 100.896f
    path.cubic_to(
        (f32::from_bits(0x4356e666), f32::from_bits(0x42c9cd22)),
        (f32::from_bits(0x4357999a), f32::from_bits(0x42c9d0e6)),
        (f32::from_bits(0x43584ccd), f32::from_bits(0x42c9dbc6)),
    ); // 214.9f, 100.901f, 215.6f, 100.908f, 216.3f, 100.929f
    path.cubic_to(
        (f32::from_bits(0x43590000), f32::from_bits(0x42c9e6a7)),
        (f32::from_bits(0x4359b333), f32::from_bits(0x42c9fd4d)),
        (f32::from_bits(0x435a6666), f32::from_bits(0x42ca0bcf)),
    ); // 217, 100.95f, 217.7f, 100.995f, 218.4f, 101.023f
    path.cubic_to(
        (f32::from_bits(0x435b199a), f32::from_bits(0x42ca1a50)),
        (f32::from_bits(0x435bcccd), f32::from_bits(0x42ca29b9)),
        (f32::from_bits(0x435c8000), f32::from_bits(0x42ca32d0)),
    ); // 219.1f, 101.051f, 219.8f, 101.081f, 220.5f, 101.099f
    path.cubic_to(
        (f32::from_bits(0x435d3333), f32::from_bits(0x42ca3be7)),
        (f32::from_bits(0x435de666), f32::from_bits(0x42ca3af4)),
        (f32::from_bits(0x435e999a), f32::from_bits(0x42ca4259)),
    ); // 221.2f, 101.117f, 221.9f, 101.115f, 222.6f, 101.13f
    path.cubic_to(
        (f32::from_bits(0x435f4ccd), f32::from_bits(0x42ca49be)),
        (f32::from_bits(0x43600000), f32::from_bits(0x42ca541c)),
        (f32::from_bits(0x4360b333), f32::from_bits(0x42ca5f2d)),
    ); // 223.3f, 101.144f, 224, 101.164f, 224.7f, 101.186f
    path.cubic_to(
        (f32::from_bits(0x43616666), f32::from_bits(0x42ca6a3f)),
        (f32::from_bits(0x4362199a), f32::from_bits(0x42ca7a05)),
        (f32::from_bits(0x4362cccd), f32::from_bits(0x42ca84c3)),
    ); // 225.4f, 101.208f, 226.1f, 101.238f, 226.8f, 101.259f
    path.cubic_to(
        (f32::from_bits(0x43638000), f32::from_bits(0x42ca8f80)),
        (f32::from_bits(0x43643333), f32::from_bits(0x42ca9a0e)),
        (f32::from_bits(0x4364e666), f32::from_bits(0x42ca9f9e)),
    ); // 227.5f, 101.28f, 228.2f, 101.301f, 228.9f, 101.312f
    path.cubic_to(
        (f32::from_bits(0x4365999a), f32::from_bits(0x42caa52d)),
        (f32::from_bits(0x43664ccd), f32::from_bits(0x42caa78a)),
        (f32::from_bits(0x43670000), f32::from_bits(0x42caa620)),
    ); // 229.6f, 101.323f, 230.3f, 101.327f, 231, 101.324f
    path.cubic_to(
        (f32::from_bits(0x4367b333), f32::from_bits(0x42caa4b6)),
        (f32::from_bits(0x43686666), f32::from_bits(0x42ca9cbf)),
        (f32::from_bits(0x4369199a), f32::from_bits(0x42ca9723)),
    ); // 231.7f, 101.322f, 232.4f, 101.306f, 233.1f, 101.295f
    path.cubic_to(
        (f32::from_bits(0x4369cccd), f32::from_bits(0x42ca9188)),
        (f32::from_bits(0x436a8000), f32::from_bits(0x42ca894a)),
        (f32::from_bits(0x436b3333), f32::from_bits(0x42ca847c)),
    ); // 233.8f, 101.284f, 234.5f, 101.268f, 235.2f, 101.259f
    path.cubic_to(
        (f32::from_bits(0x436be666), f32::from_bits(0x42ca7fae)),
        (f32::from_bits(0x436c999a), f32::from_bits(0x42ca7c01)),
        (f32::from_bits(0x436d4ccd), f32::from_bits(0x42ca7a4f)),
    ); // 235.9f, 101.249f, 236.6f, 101.242f, 237.3f, 101.239f
    path.cubic_to(
        (f32::from_bits(0x436e0000), f32::from_bits(0x42ca789d)),
        (f32::from_bits(0x436eb333), f32::from_bits(0x42ca7976)),
        (f32::from_bits(0x436f6666), f32::from_bits(0x42ca7a4f)),
    ); // 238, 101.236f, 238.7f, 101.237f, 239.4f, 101.239f
    path.cubic_to(
        (f32::from_bits(0x4370199a), f32::from_bits(0x42ca7b28)),
        (f32::from_bits(0x4370cccd), f32::from_bits(0x42ca7ed5)),
        (f32::from_bits(0x43718000), f32::from_bits(0x42ca7f66)),
    ); // 240.1f, 101.241f, 240.8f, 101.248f, 241.5f, 101.249f
    path.cubic_to(
        (f32::from_bits(0x43723333), f32::from_bits(0x42ca7ff6)),
        (f32::from_bits(0x4372e666), f32::from_bits(0x42ca7e44)),
        (f32::from_bits(0x4373999a), f32::from_bits(0x42ca7db4)),
    ); // 242.2f, 101.25f, 242.9f, 101.247f, 243.6f, 101.246f
    path.cubic_to(
        (f32::from_bits(0x43744ccd), f32::from_bits(0x42ca7d23)),
        (f32::from_bits(0x43750000), f32::from_bits(0x42ca7c4a)),
        (f32::from_bits(0x4375b333), f32::from_bits(0x42ca7c01)),
    ); // 244.3f, 101.244f, 245, 101.243f, 245.7f, 101.242f
    path.cubic_to(
        (f32::from_bits(0x43766666), f32::from_bits(0x42ca7bb9)),
        (f32::from_bits(0x4377199a), f32::from_bits(0x42ca7bb9)),
        (f32::from_bits(0x4377cccd), f32::from_bits(0x42ca7c01)),
    ); // 246.4f, 101.242f, 247.1f, 101.242f, 247.8f, 101.242f
    path.cubic_to(
        (f32::from_bits(0x43788000), f32::from_bits(0x42ca7c4a)),
        (f32::from_bits(0x43793333), f32::from_bits(0x42ca7d6b)),
        (f32::from_bits(0x4379e666), f32::from_bits(0x42ca7db4)),
    ); // 248.5f, 101.243f, 249.2f, 101.245f, 249.9f, 101.246f
    path.cubic_to(
        (f32::from_bits(0x437a999a), f32::from_bits(0x42ca7dfc)),
        (f32::from_bits(0x437b4ccd), f32::from_bits(0x42ca7dfc)),
        (f32::from_bits(0x437c0000), f32::from_bits(0x42ca7db4)),
    ); // 250.6f, 101.246f, 251.3f, 101.246f, 252, 101.246f
    path.cubic_to(
        (f32::from_bits(0x437cb333), f32::from_bits(0x42ca7d6b)),
        (f32::from_bits(0x437d6666), f32::from_bits(0x42ca7c4a)),
        (f32::from_bits(0x437e199a), f32::from_bits(0x42ca7c01)),
    ); // 252.7f, 101.245f, 253.4f, 101.243f, 254.1f, 101.242f
    path.cubic_to(
        (f32::from_bits(0x437ecccd), f32::from_bits(0x42ca7bb9)),
        (f32::from_bits(0x437f8000), f32::from_bits(0x42ca7bb9)),
        (f32::from_bits(0x4380199a), f32::from_bits(0x42ca7c01)),
    ); // 254.8f, 101.242f, 255.5f, 101.242f, 256.2f, 101.242f
    path.cubic_to(
        (f32::from_bits(0x43807333), f32::from_bits(0x42ca7c4a)),
        (f32::from_bits(0x4380cccd), f32::from_bits(0x42ca7d6b)),
        (f32::from_bits(0x43812666), f32::from_bits(0x42ca7db4)),
    ); // 256.9f, 101.243f, 257.6f, 101.245f, 258.3f, 101.246f
    path.cubic_to(
        (f32::from_bits(0x43818000), f32::from_bits(0x42ca7dfc)),
        (f32::from_bits(0x4381d99a), f32::from_bits(0x42ca7db4)),
        (f32::from_bits(0x43823333), f32::from_bits(0x42ca7db4)),
    ); // 259, 101.246f, 259.7f, 101.246f, 260.4f, 101.246f
    path.cubic_to(
        (f32::from_bits(0x43828ccd), f32::from_bits(0x42ca7db4)),
        (f32::from_bits(0x4382e666), f32::from_bits(0x42ca7db4)),
        (f32::from_bits(0x43834000), f32::from_bits(0x42ca7db4)),
    ); // 261.1f, 101.246f, 261.8f, 101.246f, 262.5f, 101.246f
    path.cubic_to(
        (f32::from_bits(0x4383999a), f32::from_bits(0x42ca7db4)),
        (f32::from_bits(0x4383f333), f32::from_bits(0x42ca7d6b)),
        (f32::from_bits(0x43844ccd), f32::from_bits(0x42ca7db4)),
    ); // 263.2f, 101.246f, 263.9f, 101.245f, 264.6f, 101.246f
    path.cubic_to(
        (f32::from_bits(0x4384a666), f32::from_bits(0x42ca7dfc)),
        (f32::from_bits(0x43850000), f32::from_bits(0x42ca7f1d)),
        (f32::from_bits(0x4385599a), f32::from_bits(0x42ca7f66)),
    ); // 265.3f, 101.246f, 266, 101.248f, 266.7f, 101.249f
    path.cubic_to(
        (f32::from_bits(0x4385b333), f32::from_bits(0x42ca7fae)),
        (f32::from_bits(0x43860ccd), f32::from_bits(0x42ca7fae)),
        (f32::from_bits(0x43866666), f32::from_bits(0x42ca7f66)),
    ); // 267.4f, 101.249f, 268.1f, 101.249f, 268.8f, 101.249f
    path.cubic_to(
        (f32::from_bits(0x4386c000), f32::from_bits(0x42ca7f1d)),
        (f32::from_bits(0x4387199a), f32::from_bits(0x42ca7e44)),
        (f32::from_bits(0x43877333), f32::from_bits(0x42ca7db4)),
    ); // 269.5f, 101.248f, 270.2f, 101.247f, 270.9f, 101.246f
    path.cubic_to(
        (f32::from_bits(0x4387cccd), f32::from_bits(0x42ca7d23)),
        (f32::from_bits(0x43882666), f32::from_bits(0x42ca7c4a)),
        (f32::from_bits(0x43888000), f32::from_bits(0x42ca7c01)),
    ); // 271.6f, 101.244f, 272.3f, 101.243f, 273, 101.242f
    path.cubic_to(
        (f32::from_bits(0x4388d99a), f32::from_bits(0x42ca7bb9)),
        (f32::from_bits(0x43893333), f32::from_bits(0x42ca7c92)),
        (f32::from_bits(0x43898ccd), f32::from_bits(0x42ca7c01)),
    ); // 273.7f, 101.242f, 274.4f, 101.243f, 275.1f, 101.242f
    path.cubic_to(
        (f32::from_bits(0x4389e666), f32::from_bits(0x42ca7b71)),
        (f32::from_bits(0x438a4000), f32::from_bits(0x42ca789d)),
        (f32::from_bits(0x438a999a), f32::from_bits(0x42ca789d)),
    ); // 275.8f, 101.241f, 276.5f, 101.236f, 277.2f, 101.236f
    path.cubic_to(
        (f32::from_bits(0x438af333), f32::from_bits(0x42ca789d)),
        (f32::from_bits(0x438b4ccd), f32::from_bits(0x42ca78da)),
        (f32::from_bits(0x438ba666), f32::from_bits(0x42ca7c01)),
    ); // 277.9f, 101.236f, 278.6f, 101.236f, 279.3f, 101.242f
    path.cubic_to(
        (f32::from_bits(0x438c0000), f32::from_bits(0x42ca7f29)),
        (f32::from_bits(0x438c599a), f32::from_bits(0x42ca8863)),
        (f32::from_bits(0x438cb333), f32::from_bits(0x42ca8b8b)),
    ); // 280, 101.248f, 280.7f, 101.266f, 281.4f, 101.273f
    path.cubic_to(
        (f32::from_bits(0x438d0ccd), f32::from_bits(0x42ca8eb3)),
        (f32::from_bits(0x438d6666), f32::from_bits(0x42ca9411)),
        (f32::from_bits(0x438dc000), f32::from_bits(0x42ca8eef)),
    ); // 282.1f, 101.279f, 282.8f, 101.289f, 283.5f, 101.279f
    path.cubic_to(
        (f32::from_bits(0x438e199a), f32::from_bits(0x42ca89cd)),
        (f32::from_bits(0x438e7333), f32::from_bits(0x42ca7cb5)),
        (f32::from_bits(0x438ecccd), f32::from_bits(0x42ca6cbe)),
    ); // 284.2f, 101.269f, 284.9f, 101.244f, 285.6f, 101.212f
    path.cubic_to(
        (f32::from_bits(0x438f2666), f32::from_bits(0x42ca5cc7)),
        (f32::from_bits(0x438f8000), f32::from_bits(0x42ca4681)),
        (f32::from_bits(0x438fd99a), f32::from_bits(0x42ca2f25)),
    ); // 286.3f, 101.181f, 287, 101.138f, 287.7f, 101.092f
    path.cubic_to(
        (f32::from_bits(0x43903333), f32::from_bits(0x42ca17c9)),
        (f32::from_bits(0x43908ccd), f32::from_bits(0x42c9f983)),
        (f32::from_bits(0x4390e666), f32::from_bits(0x42c9e096)),
    ); // 288.4f, 101.046f, 289.1f, 100.987f, 289.8f, 100.939f
    path.cubic_to(
        (f32::from_bits(0x43914000), f32::from_bits(0x42c9c7aa)),
        (f32::from_bits(0x4391999a), f32::from_bits(0x42c9a56f)),
        (f32::from_bits(0x4391f333), f32::from_bits(0x42c9999a)),
    ); // 290.5f, 100.89f, 291.2f, 100.823f, 291.9f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43924ccd), f32::from_bits(0x42c98dc6)),
        (f32::from_bits(0x4392a666), f32::from_bits(0x42c9999a)),
        (f32::from_bits(0x43930000), f32::from_bits(0x42c9999a)),
    ); // 292.6f, 100.777f, 293.3f, 100.8f, 294, 100.8f
    path.line_to((f32::from_bits(0x43930000), f32::from_bits(0x42c9999a))); // 294, 100.8f
    path.line_to((f32::from_bits(0x43478000), f32::from_bits(0x42c9999a))); // 199.5f, 100.8f
    path.close();
    path.move_to((f32::from_bits(0x43ab2666), f32::from_bits(0x42c9999a))); // 342.3f, 100.8f
    path.line_to((f32::from_bits(0x43ab2666), f32::from_bits(0x42c9999a))); // 342.3f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43ab8000), f32::from_bits(0x42c9999a)),
        (f32::from_bits(0x43abd99a), f32::from_bits(0x42c98e0e)),
        (f32::from_bits(0x43ac3333), f32::from_bits(0x42c9999a)),
    ); // 343, 100.8f, 343.7f, 100.777f, 344.4f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43ac8ccd), f32::from_bits(0x42c9a527)),
        (f32::from_bits(0x43ace666), f32::from_bits(0x42c9bd29)),
        (f32::from_bits(0x43ad4000), f32::from_bits(0x42c9dee4)),
    ); // 345.1f, 100.823f, 345.8f, 100.869f, 346.5f, 100.935f
    path.cubic_to(
        (f32::from_bits(0x43ad999a), f32::from_bits(0x42ca009f)),
        (f32::from_bits(0x43adf333), f32::from_bits(0x42ca4737)),
        (f32::from_bits(0x43ae4ccd), f32::from_bits(0x42ca63fd)),
    ); // 347.2f, 101.001f, 347.9f, 101.139f, 348.6f, 101.195f
    path.cubic_to(
        (f32::from_bits(0x43aea666), f32::from_bits(0x42ca80c4)),
        (f32::from_bits(0x43af0000), f32::from_bits(0x42ca834d)),
        (f32::from_bits(0x43af599a), f32::from_bits(0x42ca8b8b)),
    ); // 349.3f, 101.251f, 350, 101.256f, 350.7f, 101.273f
    path.cubic_to(
        (f32::from_bits(0x43afb333), f32::from_bits(0x42ca93c9)),
        (f32::from_bits(0x43b00ccd), f32::from_bits(0x42ca92a9)),
        (f32::from_bits(0x43b06666), f32::from_bits(0x42ca9571)),
    ); // 351.4f, 101.289f, 352.1f, 101.286f, 352.8f, 101.292f
    path.cubic_to(
        (f32::from_bits(0x43b0c000), f32::from_bits(0x42ca9839)),
        (f32::from_bits(0x43b1199a), f32::from_bits(0x42ca9ad0)),
        (f32::from_bits(0x43b17333), f32::from_bits(0x42ca9c3a)),
    ); // 353.5f, 101.297f, 354.2f, 101.302f, 354.9f, 101.305f
    path.cubic_to(
        (f32::from_bits(0x43b1cccd), f32::from_bits(0x42ca9da3)),
        (f32::from_bits(0x43b22666), f32::from_bits(0x42ca9da3)),
        (f32::from_bits(0x43b28000), f32::from_bits(0x42ca9dec)),
    ); // 355.6f, 101.308f, 356.3f, 101.308f, 357, 101.308f
    path.cubic_to(
        (f32::from_bits(0x43b2d99a), f32::from_bits(0x42ca9e34)),
        (f32::from_bits(0x43b33333), f32::from_bits(0x42ca9e34)),
        (f32::from_bits(0x43b38ccd), f32::from_bits(0x42ca9dec)),
    ); // 357.7f, 101.309f, 358.4f, 101.309f, 359.1f, 101.308f
    path.cubic_to(
        (f32::from_bits(0x43b3e666), f32::from_bits(0x42ca9da3)),
        (f32::from_bits(0x43b44000), f32::from_bits(0x42ca9c82)),
        (f32::from_bits(0x43b4999a), f32::from_bits(0x42ca9c3a)),
    ); // 359.8f, 101.308f, 360.5f, 101.306f, 361.2f, 101.305f
    path.cubic_to(
        (f32::from_bits(0x43b4f333), f32::from_bits(0x42ca9bf1)),
        (f32::from_bits(0x43b54ccd), f32::from_bits(0x42ca9c3a)),
        (f32::from_bits(0x43b5a666), f32::from_bits(0x42ca9c3a)),
    ); // 361.9f, 101.305f, 362.6f, 101.305f, 363.3f, 101.305f
    path.cubic_to(
        (f32::from_bits(0x43b60000), f32::from_bits(0x42ca9c3a)),
        (f32::from_bits(0x43b6599a), f32::from_bits(0x42ca9bf1)),
        (f32::from_bits(0x43b6b333), f32::from_bits(0x42ca9c3a)),
    ); // 364, 101.305f, 364.7f, 101.305f, 365.4f, 101.305f
    path.cubic_to(
        (f32::from_bits(0x43b70ccd), f32::from_bits(0x42ca9c82)),
        (f32::from_bits(0x43b76666), f32::from_bits(0x42ca9da3)),
        (f32::from_bits(0x43b7c000), f32::from_bits(0x42ca9dec)),
    ); // 366.1f, 101.306f, 366.8f, 101.308f, 367.5f, 101.308f
    path.cubic_to(
        (f32::from_bits(0x43b8199a), f32::from_bits(0x42ca9e34)),
        (f32::from_bits(0x43b87333), f32::from_bits(0x42ca9dec)),
        (f32::from_bits(0x43b8cccd), f32::from_bits(0x42ca9dec)),
    ); // 368.2f, 101.309f, 368.9f, 101.308f, 369.6f, 101.308f
    path.cubic_to(
        (f32::from_bits(0x43b92666), f32::from_bits(0x42ca9dec)),
        (f32::from_bits(0x43b98000), f32::from_bits(0x42ca9dec)),
        (f32::from_bits(0x43b9d99a), f32::from_bits(0x42ca9dec)),
    ); // 370.3f, 101.308f, 371, 101.308f, 371.7f, 101.308f
    path.cubic_to(
        (f32::from_bits(0x43ba3333), f32::from_bits(0x42ca9dec)),
        (f32::from_bits(0x43ba8ccd), f32::from_bits(0x42ca9e7d)),
        (f32::from_bits(0x43bae666), f32::from_bits(0x42ca9dec)),
    ); // 372.4f, 101.308f, 373.1f, 101.31f, 373.8f, 101.308f
    path.cubic_to(
        (f32::from_bits(0x43bb4000), f32::from_bits(0x42ca9d5b)),
        (f32::from_bits(0x43bb999a), f32::from_bits(0x42ca9b61)),
        (f32::from_bits(0x43bbf333), f32::from_bits(0x42ca9a88)),
    ); // 374.5f, 101.307f, 375.2f, 101.303f, 375.9f, 101.302f
    path.cubic_to(
        (f32::from_bits(0x43bc4ccd), f32::from_bits(0x42ca99af)),
        (f32::from_bits(0x43bca666), f32::from_bits(0x42ca991e)),
        (f32::from_bits(0x43bd0000), f32::from_bits(0x42ca98d5)),
    ); // 376.6f, 101.3f, 377.3f, 101.299f, 378, 101.299f
    path.cubic_to(
        (f32::from_bits(0x43bd599a), f32::from_bits(0x42ca988d)),
        (f32::from_bits(0x43bdb333), f32::from_bits(0x42ca988d)),
        (f32::from_bits(0x43be0ccd), f32::from_bits(0x42ca98d5)),
    ); // 378.7f, 101.298f, 379.4f, 101.298f, 380.1f, 101.299f
    path.cubic_to(
        (f32::from_bits(0x43be6666), f32::from_bits(0x42ca991e)),
        (f32::from_bits(0x43bec000), f32::from_bits(0x42ca99f7)),
        (f32::from_bits(0x43bf199a), f32::from_bits(0x42ca9a88)),
    ); // 380.8f, 101.299f, 381.5f, 101.301f, 382.2f, 101.302f
    path.cubic_to(
        (f32::from_bits(0x43bf7333), f32::from_bits(0x42ca9b18)),
        (f32::from_bits(0x43bfcccd), f32::from_bits(0x42ca9ba9)),
        (f32::from_bits(0x43c02666), f32::from_bits(0x42ca9c3a)),
    ); // 382.9f, 101.303f, 383.6f, 101.304f, 384.3f, 101.305f
    path.cubic_to(
        (f32::from_bits(0x43c08000), f32::from_bits(0x42ca9cca)),
        (f32::from_bits(0x43c0d99a), f32::from_bits(0x42ca9ec5)),
        (f32::from_bits(0x43c13333), f32::from_bits(0x42ca9dec)),
    ); // 385, 101.306f, 385.7f, 101.31f, 386.4f, 101.308f
    path.cubic_to(
        (f32::from_bits(0x43c18ccd), f32::from_bits(0x42ca9d13)),
        (f32::from_bits(0x43c1e666), f32::from_bits(0x42ca9a3f)),
        (f32::from_bits(0x43c24000), f32::from_bits(0x42ca9723)),
    ); // 387.1f, 101.307f, 387.8f, 101.301f, 388.5f, 101.295f
    path.cubic_to(
        (f32::from_bits(0x43c2999a), f32::from_bits(0x42ca9407)),
        (f32::from_bits(0x43c2f333), f32::from_bits(0x42ca8d87)),
        (f32::from_bits(0x43c34ccd), f32::from_bits(0x42ca8b45)),
    ); // 389.2f, 101.289f, 389.9f, 101.276f, 390.6f, 101.272f
    path.cubic_to(
        (f32::from_bits(0x43c3a666), f32::from_bits(0x42ca8902)),
        (f32::from_bits(0x43c40000), f32::from_bits(0x42ca8992)),
        (f32::from_bits(0x43c4599a), f32::from_bits(0x42ca8992)),
    ); // 391.3f, 101.268f, 392, 101.269f, 392.7f, 101.269f
    path.cubic_to(
        (f32::from_bits(0x43c4b333), f32::from_bits(0x42ca8992)),
        (f32::from_bits(0x43c50ccd), f32::from_bits(0x42ca8a6b)),
        (f32::from_bits(0x43c56666), f32::from_bits(0x42ca8b45)),
    ); // 393.4f, 101.269f, 394.1f, 101.27f, 394.8f, 101.272f
    path.cubic_to(
        (f32::from_bits(0x43c5c000), f32::from_bits(0x42ca8c1e)),
        (f32::from_bits(0x43c6199a), f32::from_bits(0x42ca8d87)),
        (f32::from_bits(0x43c67333), f32::from_bits(0x42ca8ea9)),
    ); // 395.5f, 101.274f, 396.2f, 101.276f, 396.9f, 101.279f
    path.cubic_to(
        (f32::from_bits(0x43c6cccd), f32::from_bits(0x42ca8fca)),
        (f32::from_bits(0x43c72666), f32::from_bits(0x42ca90a3)),
        (f32::from_bits(0x43c78000), f32::from_bits(0x42ca920d)),
    ); // 397.6f, 101.281f, 398.3f, 101.282f, 399, 101.285f
    path.cubic_to(
        (f32::from_bits(0x43c7d99a), f32::from_bits(0x42ca9377)),
        (f32::from_bits(0x43c83333), f32::from_bits(0x42ca9571)),
        (f32::from_bits(0x43c88ccd), f32::from_bits(0x42ca9723)),
    ); // 399.7f, 101.288f, 400.4f, 101.292f, 401.1f, 101.295f
    path.cubic_to(
        (f32::from_bits(0x43c8e666), f32::from_bits(0x42ca98d5)),
        (f32::from_bits(0x43c94000), f32::from_bits(0x42ca9a88)),
        (f32::from_bits(0x43c9999a), f32::from_bits(0x42ca9c3a)),
    ); // 401.8f, 101.299f, 402.5f, 101.302f, 403.2f, 101.305f
    path.cubic_to(
        (f32::from_bits(0x43c9f333), f32::from_bits(0x42ca9dec)),
        (f32::from_bits(0x43ca4ccd), f32::from_bits(0x42caa02f)),
        (f32::from_bits(0x43caa666), f32::from_bits(0x42caa150)),
    ); // 403.9f, 101.308f, 404.6f, 101.313f, 405.3f, 101.315f
    path.cubic_to(
        (f32::from_bits(0x43cb0000), f32::from_bits(0x42caa271)),
        (f32::from_bits(0x43cb599a), f32::from_bits(0x42caa46c)),
        (f32::from_bits(0x43cbb333), f32::from_bits(0x42caa302)),
    ); // 406, 101.317f, 406.7f, 101.321f, 407.4f, 101.318f
    path.cubic_to(
        (f32::from_bits(0x43cc0ccd), f32::from_bits(0x42caa198)),
        (f32::from_bits(0x43cc6666), f32::from_bits(0x42ca9ad0)),
        (f32::from_bits(0x43ccc000), f32::from_bits(0x42ca98d5)),
    ); // 408.1f, 101.316f, 408.8f, 101.302f, 409.5f, 101.299f
    path.cubic_to(
        (f32::from_bits(0x43cd199a), f32::from_bits(0x42ca96db)),
        (f32::from_bits(0x43cd7333), f32::from_bits(0x42ca9535)),
        (f32::from_bits(0x43cdcccd), f32::from_bits(0x42ca9723)),
    ); // 410.2f, 101.295f, 410.9f, 101.291f, 411.6f, 101.295f
    path.cubic_to(
        (f32::from_bits(0x43ce2666), f32::from_bits(0x42ca9912)),
        (f32::from_bits(0x43ce8000), f32::from_bits(0x42caa19a)),
        (f32::from_bits(0x43ced99a), f32::from_bits(0x42caa46e)),
    ); // 412.3f, 101.299f, 413, 101.316f, 413.7f, 101.321f
    path.cubic_to(
        (f32::from_bits(0x43cf3333), f32::from_bits(0x42caa741)),
        (f32::from_bits(0x43cf8ccd), f32::from_bits(0x42caa7c4)),
        (f32::from_bits(0x43cfe666), f32::from_bits(0x42caa819)),
    ); // 414.4f, 101.327f, 415.1f, 101.328f, 415.8f, 101.328f
    path.cubic_to(
        (f32::from_bits(0x43d04000), f32::from_bits(0x42caa86d)),
        (f32::from_bits(0x43d0999a), f32::from_bits(0x42caa788)),
        (f32::from_bits(0x43d0f333), f32::from_bits(0x42caa666)),
    ); // 416.5f, 101.329f, 417.2f, 101.327f, 417.9f, 101.325f
    path.cubic_to(
        (f32::from_bits(0x43d14ccd), f32::from_bits(0x42caa545)),
        (f32::from_bits(0x43d1a666), f32::from_bits(0x42caa302)),
        (f32::from_bits(0x43d20000), f32::from_bits(0x42caa150)),
    ); // 418.6f, 101.323f, 419.3f, 101.318f, 420, 101.315f
    path.cubic_to(
        (f32::from_bits(0x43d2599a), f32::from_bits(0x42ca9f9e)),
        (f32::from_bits(0x43d2b333), f32::from_bits(0x42ca9d13)),
        (f32::from_bits(0x43d30ccd), f32::from_bits(0x42ca9c3a)),
    ); // 420.7f, 101.312f, 421.4f, 101.307f, 422.1f, 101.305f
    path.cubic_to(
        (f32::from_bits(0x43d36666), f32::from_bits(0x42ca9b61)),
        (f32::from_bits(0x43d3c000), f32::from_bits(0x42ca9b24)),
        (f32::from_bits(0x43d4199a), f32::from_bits(0x42ca9c3a)),
    ); // 422.8f, 101.303f, 423.5f, 101.303f, 424.2f, 101.305f
    path.cubic_to(
        (f32::from_bits(0x43d47333), f32::from_bits(0x42ca9d4f)),
        (f32::from_bits(0x43d4cccd), f32::from_bits(0x42caa59b)),
        (f32::from_bits(0x43d52666), f32::from_bits(0x42caa2bc)),
    ); // 424.9f, 101.307f, 425.6f, 101.323f, 426.3f, 101.318f
    path.cubic_to(
        (f32::from_bits(0x43d58000), f32::from_bits(0x42ca9fdc)),
        (f32::from_bits(0x43d5d99a), f32::from_bits(0x42caa425)),
        (f32::from_bits(0x43d63333), f32::from_bits(0x42ca8afe)),
    ); // 427, 101.312f, 427.7f, 101.321f, 428.4f, 101.271f
    path.cubic_to(
        (f32::from_bits(0x43d68ccd), f32::from_bits(0x42ca71d6)),
        (f32::from_bits(0x43d6e666), f32::from_bits(0x42ca340a)),
        (f32::from_bits(0x43d74000), f32::from_bits(0x42ca0bcf)),
    ); // 429.1f, 101.222f, 429.8f, 101.102f, 430.5f, 101.023f
    path.cubic_to(
        (f32::from_bits(0x43d7999a), f32::from_bits(0x42c9e394)),
        (f32::from_bits(0x43d7f333), f32::from_bits(0x42c9aca3)),
        (f32::from_bits(0x43d84ccd), f32::from_bits(0x42c9999a)),
    ); // 431.2f, 100.944f, 431.9f, 100.837f, 432.6f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43d8a666), f32::from_bits(0x42c98692)),
        (f32::from_bits(0x43d90000), f32::from_bits(0x42c9999a)),
        (f32::from_bits(0x43d9599a), f32::from_bits(0x42c9999a)),
    ); // 433.3f, 100.763f, 434, 100.8f, 434.7f, 100.8f
    path.line_to((f32::from_bits(0x43d9599a), f32::from_bits(0x42c9999a))); // 434.7f, 100.8f
    path.line_to((f32::from_bits(0x43ab2666), f32::from_bits(0x42c9999a))); // 342.3f, 100.8f
    path.close();
    path.move_to((f32::from_bits(0x43dfa666), f32::from_bits(0x42c9999a))); // 447.3f, 100.8f
    path.line_to((f32::from_bits(0x43dfa666), f32::from_bits(0x42c9999a))); // 447.3f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43e00000), f32::from_bits(0x42c99abd)),
        (f32::from_bits(0x43e0599a), f32::from_bits(0x42c9a022)),
        (f32::from_bits(0x43e0b333), f32::from_bits(0x42c9a06c)),
    ); // 448, 100.802f, 448.7f, 100.813f, 449.4f, 100.813f
    path.cubic_to(
        (f32::from_bits(0x43e10ccd), f32::from_bits(0x42c9a0b6)),
        (f32::from_bits(0x43e16666), f32::from_bits(0x42c99c79)),
        (f32::from_bits(0x43e1c000), f32::from_bits(0x42c99b56)),
    ); // 450.1f, 100.814f, 450.8f, 100.806f, 451.5f, 100.803f
    path.cubic_to(
        (f32::from_bits(0x43e2199a), f32::from_bits(0x42c99a33)),
        (f32::from_bits(0x43e27333), f32::from_bits(0x42c9928a)),
        (f32::from_bits(0x43e2cccd), f32::from_bits(0x42c9999a)),
    ); // 452.2f, 100.801f, 452.9f, 100.786f, 453.6f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43e32666), f32::from_bits(0x42c9a0ab)),
        (f32::from_bits(0x43e38000), f32::from_bits(0x42c9b682)),
        (f32::from_bits(0x43e3d99a), f32::from_bits(0x42c9c5bb)),
    ); // 454.3f, 100.814f, 455, 100.856f, 455.7f, 100.886f
    path.cubic_to(
        (f32::from_bits(0x43e43333), f32::from_bits(0x42c9d4f4)),
        (f32::from_bits(0x43e48ccd), f32::from_bits(0x42c9ef91)),
        (f32::from_bits(0x43e4e666), f32::from_bits(0x42c9f4f0)),
    ); // 456.4f, 100.916f, 457.1f, 100.968f, 457.8f, 100.978f
    path.cubic_to(
        (f32::from_bits(0x43e54000), f32::from_bits(0x42c9fa4e)),
        (f32::from_bits(0x43e5999a), f32::from_bits(0x42c9e940)),
        (f32::from_bits(0x43e5f333), f32::from_bits(0x42c9e5f3)),
    ); // 458.5f, 100.989f, 459.2f, 100.956f, 459.9f, 100.949f
    path.cubic_to(
        (f32::from_bits(0x43e64ccd), f32::from_bits(0x42c9e2a6)),
        (f32::from_bits(0x43e6a666), f32::from_bits(0x42c9e366)),
        (f32::from_bits(0x43e70000), f32::from_bits(0x42c9e123)),
    ); // 460.6f, 100.943f, 461.3f, 100.944f, 462, 100.94f
    path.cubic_to(
        (f32::from_bits(0x43e7599a), f32::from_bits(0x42c9dee0)),
        (f32::from_bits(0x43e7b333), f32::from_bits(0x42c9d9d8)),
        (f32::from_bits(0x43e80ccd), f32::from_bits(0x42c9d862)),
    ); // 462.7f, 100.935f, 463.4f, 100.925f, 464.1f, 100.923f
    path.cubic_to(
        (f32::from_bits(0x43e86666), f32::from_bits(0x42c9d6ed)),
        (f32::from_bits(0x43e8c000), f32::from_bits(0x42c9d93b)),
        (f32::from_bits(0x43e9199a), f32::from_bits(0x42c9d862)),
    ); // 464.8f, 100.92f, 465.5f, 100.924f, 466.2f, 100.923f
    path.cubic_to(
        (f32::from_bits(0x43e97333), f32::from_bits(0x42c9d789)),
        (f32::from_bits(0x43e9cccd), f32::from_bits(0x42c9d614)),
        (f32::from_bits(0x43ea2666), f32::from_bits(0x42c9d34c)),
    ); // 466.9f, 100.921f, 467.6f, 100.918f, 468.3f, 100.913f
    path.cubic_to(
        (f32::from_bits(0x43ea8000), f32::from_bits(0x42c9d084)),
        (f32::from_bits(0x43ead99a), f32::from_bits(0x42c9cbf1)),
        (f32::from_bits(0x43eb3333), f32::from_bits(0x42c9c7b4)),
    ); // 469, 100.907f, 469.7f, 100.898f, 470.4f, 100.89f
    path.cubic_to(
        (f32::from_bits(0x43eb8ccd), f32::from_bits(0x42c9c376)),
        (f32::from_bits(0x43ebe666), f32::from_bits(0x42c9bde8)),
        (f32::from_bits(0x43ec4000), f32::from_bits(0x42c9b9dc)),
    ); // 471.1f, 100.882f, 471.8f, 100.871f, 472.5f, 100.863f
    path.cubic_to(
        (f32::from_bits(0x43ec999a), f32::from_bits(0x42c9b5cf)),
        (f32::from_bits(0x43ecf333), f32::from_bits(0x42c9b442)),
        (f32::from_bits(0x43ed4ccd), f32::from_bits(0x42c9af69)),
    ); // 473.2f, 100.855f, 473.9f, 100.852f, 474.6f, 100.843f
    path.cubic_to(
        (f32::from_bits(0x43eda666), f32::from_bits(0x42c9aa8f)),
        (f32::from_bits(0x43ee0000), f32::from_bits(0x42c9a064)),
        (f32::from_bits(0x43ee599a), f32::from_bits(0x42c99cc1)),
    ); // 475.3f, 100.833f, 476, 100.813f, 476.7f, 100.806f
    path.cubic_to(
        (f32::from_bits(0x43eeb333), f32::from_bits(0x42c9991f)),
        (f32::from_bits(0x43ef0ccd), f32::from_bits(0x42c99758)),
        (f32::from_bits(0x43ef6666), f32::from_bits(0x42c9999a)),
    ); // 477.4f, 100.799f, 478.1f, 100.796f, 478.8f, 100.8f
    path.cubic_to(
        (f32::from_bits(0x43efc000), f32::from_bits(0x42c99bdd)),
        (f32::from_bits(0x43f0199a), f32::from_bits(0x42c99f0e)),
        (f32::from_bits(0x43f07333), f32::from_bits(0x42c9aa52)),
    ); // 479.5f, 100.804f, 480.2f, 100.811f, 480.9f, 100.833f
    path.cubic_to(
        (f32::from_bits(0x43f0cccd), f32::from_bits(0x42c9b596)),
        (f32::from_bits(0x43f12666), f32::from_bits(0x42c9cfde)),
        (f32::from_bits(0x43f18000), f32::from_bits(0x42c9dd32)),
    ); // 481.6f, 100.855f, 482.3f, 100.906f, 483, 100.932f
    path.cubic_to(
        (f32::from_bits(0x43f1d99a), f32::from_bits(0x42c9ea86)),
        (f32::from_bits(0x43f23333), f32::from_bits(0x42c9f451)),
        (f32::from_bits(0x43f28ccd), f32::from_bits(0x42c9fa4d)),
    ); // 483.7f, 100.958f, 484.4f, 100.977f, 485.1f, 100.989f
    path.cubic_to(
        (f32::from_bits(0x43f2e666), f32::from_bits(0x42ca0048)),
        (f32::from_bits(0x43f34000), f32::from_bits(0x42c9fbf3)),
        (f32::from_bits(0x43f3999a), f32::from_bits(0x42ca0115)),
    ); // 485.8f, 101.001f, 486.5f, 100.992f, 487.2f, 101.002f
    path.cubic_to(
        (f32::from_bits(0x43f3f333), f32::from_bits(0x42ca0637)),
        (f32::from_bits(0x43f44ccd), f32::from_bits(0x42ca103f)),
        (f32::from_bits(0x43f4a666), f32::from_bits(0x42ca1919)),
    ); // 487.9f, 101.012f, 488.6f, 101.032f, 489.3f, 101.049f
    path.cubic_to(
        (f32::from_bits(0x43f50000), f32::from_bits(0x42ca21f4)),
        (f32::from_bits(0x43f5599a), f32::from_bits(0x42ca2e87)),
        (f32::from_bits(0x43f5b333), f32::from_bits(0x42ca3634)),
    ); // 490, 101.066f, 490.7f, 101.091f, 491.4f, 101.106f
    path.cubic_to(
        (f32::from_bits(0x43f60ccd), f32::from_bits(0x42ca3de1)),
        (f32::from_bits(0x43f66666), f32::from_bits(0x42ca43b9)),
        (f32::from_bits(0x43f6c000), f32::from_bits(0x42ca4729)),
    ); // 492.1f, 101.121f, 492.8f, 101.132f, 493.5f, 101.139f
    path.cubic_to(
        (f32::from_bits(0x43f7199a), f32::from_bits(0x42ca4a99)),
        (f32::from_bits(0x43f77333), f32::from_bits(0x42ca49b3)),
        (f32::from_bits(0x43f7cccd), f32::from_bits(0x42ca4ad4)),
    ); // 494.2f, 101.146f, 494.9f, 101.144f, 495.6f, 101.146f
    path.cubic_to(
        (f32::from_bits(0x43f82666), f32::from_bits(0x42ca4bf5)),
        (f32::from_bits(0x43f88000), f32::from_bits(0x42ca4cdc)),
        (f32::from_bits(0x43f8d99a), f32::from_bits(0x42ca4df2)),
    ); // 496.3f, 101.148f, 497, 101.15f, 497.7f, 101.152f
    path.cubic_to(
        (f32::from_bits(0x43f93333), f32::from_bits(0x42ca4f07)),
        (f32::from_bits(0x43f98ccd), f32::from_bits(0x42ca4fec)),
        (f32::from_bits(0x43f9e666), f32::from_bits(0x42ca5156)),
    ); // 498.4f, 101.154f, 499.1f, 101.156f, 499.8f, 101.159f
    path.cubic_to(
        (f32::from_bits(0x43fa4000), f32::from_bits(0x42ca52c0)),
        (f32::from_bits(0x43fa999a), f32::from_bits(0x42ca53e1)),
        (f32::from_bits(0x43faf333), f32::from_bits(0x42ca566c)),
    ); // 500.5f, 101.162f, 501.2f, 101.164f, 501.9f, 101.169f
    path.cubic_to(
        (f32::from_bits(0x43fb4ccd), f32::from_bits(0x42ca58f7)),
        (f32::from_bits(0x43fba666), f32::from_bits(0x42ca5d71)),
        (f32::from_bits(0x43fc0000), f32::from_bits(0x42ca6099)),
    ); // 502.6f, 101.174f, 503.3f, 101.183f, 504, 101.189f
    path.cubic_to(
        (f32::from_bits(0x43fc599a), f32::from_bits(0x42ca63c1)),
        (f32::from_bits(0x43fcb333), f32::from_bits(0x42ca663e)),
        (f32::from_bits(0x43fd0ccd), f32::from_bits(0x42ca695a)),
    ); // 504.7f, 101.195f, 505.4f, 101.2f, 506.1f, 101.206f
    path.cubic_to(
        (f32::from_bits(0x43fd6666), f32::from_bits(0x42ca6c76)),
        (f32::from_bits(0x43fdc000), f32::from_bits(0x42ca7024)),
        (f32::from_bits(0x43fe199a), f32::from_bits(0x42ca7340)),
    ); // 506.8f, 101.212f, 507.5f, 101.219f, 508.2f, 101.225f
    path.cubic_to(
        (f32::from_bits(0x43fe7333), f32::from_bits(0x42ca765c)),
        (f32::from_bits(0x43fecccd), f32::from_bits(0x42ca7a44)),
        (f32::from_bits(0x43ff2666), f32::from_bits(0x42ca7c01)),
    ); // 508.9f, 101.231f, 509.6f, 101.239f, 510.3f, 101.242f
    path.cubic_to(
        (f32::from_bits(0x43ff8000), f32::from_bits(0x42ca7dbf)),
        (f32::from_bits(0x43ffd99a), f32::from_bits(0x42ca7ed5)),
        (f32::from_bits(0x4400199a), f32::from_bits(0x42ca7db4)),
    ); // 511, 101.246f, 511.7f, 101.248f, 512.4f, 101.246f
    path.cubic_to(
        (f32::from_bits(0x44004666), f32::from_bits(0x42ca7c92)),
        (f32::from_bits(0x44007333), f32::from_bits(0x42ca780c)),
        (f32::from_bits(0x4400a000), f32::from_bits(0x42ca7539)),
    ); // 513.1f, 101.243f, 513.8f, 101.234f, 514.5f, 101.229f
    path.cubic_to(
        (f32::from_bits(0x4400cccd), f32::from_bits(0x42ca7265)),
        (f32::from_bits(0x4400f99a), f32::from_bits(0x42ca7017)),
        (f32::from_bits(0x44012666), f32::from_bits(0x42ca6cbe)),
    ); // 515.2f, 101.223f, 515.9f, 101.219f, 516.6f, 101.212f
    path.cubic_to(
        (f32::from_bits(0x44015333), f32::from_bits(0x42ca6966)),
        (f32::from_bits(0x44018000), f32::from_bits(0x42ca688b)),
        (f32::from_bits(0x4401accd), f32::from_bits(0x42ca6126)),
    ); // 517.3f, 101.206f, 518, 101.204f, 518.7f, 101.19f
    path.cubic_to(
        (f32::from_bits(0x4401d99a), f32::from_bits(0x42ca59c1)),
        (f32::from_bits(0x44020666), f32::from_bits(0x42ca4eee)),
        (f32::from_bits(0x44023333), f32::from_bits(0x42ca4061)),
    ); // 519.4f, 101.175f, 520.1f, 101.154f, 520.8f, 101.126f
    path.cubic_to(
        (f32::from_bits(0x44026000), f32::from_bits(0x42ca31d3)),
        (f32::from_bits(0x44028ccd), f32::from_bits(0x42ca1b74)),
        (f32::from_bits(0x4402b99a), f32::from_bits(0x42ca09d6)),
    ); // 521.5f, 101.097f, 522.2f, 101.054f, 522.9f, 101.019f
    path.cubic_to(
        (f32::from_bits(0x4402e666), f32::from_bits(0x42c9f839)),
        (f32::from_bits(0x44031333), f32::from_bits(0x42c9e720)),
        (f32::from_bits(0x44034000), f32::from_bits(0x42c9d6b0)),
    ); // 523.6f, 100.985f, 524.3f, 100.951f, 525, 100.919f
    path.cubic_to(
        (f32::from_bits(0x44036ccd), f32::from_bits(0x42c9c640)),
        (f32::from_bits(0x4403999a), f32::from_bits(0x42c9b163)),
        (f32::from_bits(0x4403c666), f32::from_bits(0x42c9a735)),
    ); // 525.7f, 100.887f, 526.4f, 100.846f, 527.1f, 100.827f
    path.cubic_to(
        (f32::from_bits(0x4403f333), f32::from_bits(0x42c99d06)),
        (f32::from_bits(0x44042000), f32::from_bits(0x42c99bdf)),
        (f32::from_bits(0x44044ccd), f32::from_bits(0x42c9999a)),
    ); // 527.8f, 100.807f, 528.5f, 100.804f, 529.2f, 100.8f
    path.line_to((f32::from_bits(0x44044ccd), f32::from_bits(0x42c9999a))); // 529.2f, 100.8f
    path.line_to((f32::from_bits(0x43dfa666), f32::from_bits(0x42c9999a))); // 447.3f, 100.8f
    path.close();
    path.detach()
}

// Port of: tests/PathOpsIssue3651.cpp#L1212-L1218 (chrome/m156)
fn issue3651_1(reporter: &mut Reporter, filename: &str) {
    let path = path1();
    let pathB = path2();
    // DEBUG_UNDER_DEVELOPMENT  issue3651_1 disable expectation check for now
    test_path_op_check(reporter, &path, &pathB, PathOp::Union, filename, true);
}

// Port of: tests/PathOpsIssue3651.cpp#L1220-L1224 (chrome/m156)
fn issue3651_2(reporter: &mut Reporter, filename: &str) {
    let path = path3();
    let pathB = path4();
    test_path_op(reporter, &path, &pathB, PathOp::Union, filename);
}

// Port of: tests/PathOpsIssue3651.cpp#L1226-L1230 (chrome/m156)
fn issue3651_3(reporter: &mut Reporter, filename: &str) {
    let path = path5();
    let pathB = path6();
    test_path_op(reporter, &path, &pathB, PathOp::Union, filename);
}

// Port of: tests/PathOpsIssue3651.cpp#L1232-L1405 (chrome/m156)
fn issue3651_4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x42033333), f32::from_bits(0x43346666))); // 32.8f, 180.4f
    path.line_to((f32::from_bits(0x42033333), f32::from_bits(0x43346666))); // 32.8f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x4205eeef), f32::from_bits(0x43346666)),
        (f32::from_bits(0x4208aaab), f32::from_bits(0x4334780f)),
        (f32::from_bits(0x420b6666), f32::from_bits(0x43346666)),
    ); // 33.4833f, 180.4f, 34.1667f, 180.469f, 34.85f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x420e2222), f32::from_bits(0x433454be)),
        (f32::from_bits(0x4210ddde), f32::from_bits(0x43340d56)),
        (f32::from_bits(0x4213999a), f32::from_bits(0x4333fc72)),
    ); // 35.5333f, 180.331f, 36.2167f, 180.052f, 36.9f, 179.986f
    path.cubic_to(
        (f32::from_bits(0x42165555), f32::from_bits(0x4333eb8e)),
        (f32::from_bits(0x42191111), f32::from_bits(0x4333fbf4)),
        (f32::from_bits(0x421bcccd), f32::from_bits(0x4334010f)),
    ); // 37.5833f, 179.92f, 38.2667f, 179.984f, 38.95f, 180.004f
    path.cubic_to(
        (f32::from_bits(0x421e8889), f32::from_bits(0x4334062b)),
        (f32::from_bits(0x42214444), f32::from_bits(0x43341213)),
        (f32::from_bits(0x42240000), f32::from_bits(0x43341b17)),
    ); // 39.6333f, 180.024f, 40.3167f, 180.071f, 41, 180.106f
    path.cubic_to(
        (f32::from_bits(0x4226bbbc), f32::from_bits(0x4334241b)),
        (f32::from_bits(0x42297777), f32::from_bits(0x4334339e)),
        (f32::from_bits(0x422c3333), f32::from_bits(0x43343728)),
    ); // 41.6833f, 180.141f, 42.3667f, 180.202f, 43.05f, 180.215f
    path.cubic_to(
        (f32::from_bits(0x422eeeef), f32::from_bits(0x43343ab2)),
        (f32::from_bits(0x4231aaab), f32::from_bits(0x4334337f)),
        (f32::from_bits(0x42346666), f32::from_bits(0x43343054)),
    ); // 43.7333f, 180.229f, 44.4167f, 180.201f, 45.1f, 180.189f
    path.cubic_to(
        (f32::from_bits(0x42372222), f32::from_bits(0x43342d28)),
        (f32::from_bits(0x4239ddde), f32::from_bits(0x43342281)),
        (f32::from_bits(0x423c999a), f32::from_bits(0x43342423)),
    ); // 45.7833f, 180.176f, 46.4667f, 180.135f, 47.15f, 180.141f
    path.cubic_to(
        (f32::from_bits(0x423f5555), f32::from_bits(0x433425c5)),
        (f32::from_bits(0x42421111), f32::from_bits(0x43343381)),
        (f32::from_bits(0x4244cccd), f32::from_bits(0x43343a1f)),
    ); // 47.8333f, 180.148f, 48.5167f, 180.201f, 49.2f, 180.227f
    path.cubic_to(
        (f32::from_bits(0x42478889), f32::from_bits(0x433440be)),
        (f32::from_bits(0x424a4444), f32::from_bits(0x43344d5d)),
        (f32::from_bits(0x424d0000), f32::from_bits(0x43344bdb)),
    ); // 49.8833f, 180.253f, 50.5667f, 180.302f, 51.25f, 180.296f
    path.cubic_to(
        (f32::from_bits(0x424fbbbc), f32::from_bits(0x43344a59)),
        (f32::from_bits(0x42527777), f32::from_bits(0x43342ca7)),
        (f32::from_bits(0x42553333), f32::from_bits(0x43343113)),
    ); // 51.9333f, 180.29f, 52.6167f, 180.174f, 53.3f, 180.192f
    path.cubic_to(
        (f32::from_bits(0x4257eeef), f32::from_bits(0x43343580)),
        (f32::from_bits(0x425aaaab), f32::from_bits(0x4334654b)),
        (f32::from_bits(0x425d6666), f32::from_bits(0x43346666)),
    ); // 53.9833f, 180.209f, 54.6667f, 180.396f, 55.35f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x42602222), f32::from_bits(0x43346782)),
        (f32::from_bits(0x4262ddde), f32::from_bits(0x43343ee4)),
        (f32::from_bits(0x4265999a), f32::from_bits(0x433437ba)),
    ); // 56.0333f, 180.404f, 56.7167f, 180.246f, 57.4f, 180.218f
    path.cubic_to(
        (f32::from_bits(0x42685555), f32::from_bits(0x4334308f)),
        (f32::from_bits(0x426b1111), f32::from_bits(0x43343ce3)),
        (f32::from_bits(0x426dcccd), f32::from_bits(0x43343b69)),
    ); // 58.0833f, 180.19f, 58.7667f, 180.238f, 59.45f, 180.232f
    path.cubic_to(
        (f32::from_bits(0x42708889), f32::from_bits(0x433439ef)),
        (f32::from_bits(0x42734444), f32::from_bits(0x433437d8)),
        (f32::from_bits(0x42760000), f32::from_bits(0x43342edc)),
    ); // 60.1333f, 180.226f, 60.8167f, 180.218f, 61.5f, 180.183f
    path.cubic_to(
        (f32::from_bits(0x4278bbbc), f32::from_bits(0x433425df)),
        (f32::from_bits(0x427b7777), f32::from_bits(0x43340d4f)),
        (f32::from_bits(0x427e3333), f32::from_bits(0x4334057e)),
    ); // 62.1833f, 180.148f, 62.8667f, 180.052f, 63.55f, 180.021f
    path.cubic_to(
        (f32::from_bits(0x42807777), f32::from_bits(0x4333fdad)),
        (f32::from_bits(0x4281d555), f32::from_bits(0x4333fb55)),
        (f32::from_bits(0x42833333), f32::from_bits(0x4333fff3)),
    ); // 64.2333f, 179.991f, 64.9167f, 179.982f, 65.6f, 180
    path.cubic_to(
        (f32::from_bits(0x42849111), f32::from_bits(0x43340492)),
        (f32::from_bits(0x4285eeef), f32::from_bits(0x43341020)),
        (f32::from_bits(0x42874ccd), f32::from_bits(0x43342133)),
    ); // 66.2833f, 180.018f, 66.9667f, 180.063f, 67.65f, 180.13f
    path.cubic_to(
        (f32::from_bits(0x4288aaab), f32::from_bits(0x43343246)),
        (f32::from_bits(0x428a0889), f32::from_bits(0x43345ade)),
        (f32::from_bits(0x428b6666), f32::from_bits(0x43346666)),
    ); // 68.3333f, 180.196f, 69.0167f, 180.355f, 69.7f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x428cc444), f32::from_bits(0x433471ef)),
        (f32::from_bits(0x428e2222), f32::from_bits(0x433467af)),
        (f32::from_bits(0x428f8000), f32::from_bits(0x43346666)),
    ); // 70.3833f, 180.445f, 71.0667f, 180.405f, 71.75f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x4290ddde), f32::from_bits(0x4334651e)),
        (f32::from_bits(0x42923bbc), f32::from_bits(0x43346442)),
        (f32::from_bits(0x4293999a), f32::from_bits(0x43345eb2)),
    ); // 72.4333f, 180.395f, 73.1167f, 180.392f, 73.8f, 180.37f
    path.cubic_to(
        (f32::from_bits(0x4294f777), f32::from_bits(0x43345922)),
        (f32::from_bits(0x42965555), f32::from_bits(0x43344bb4)),
        (f32::from_bits(0x4297b333), f32::from_bits(0x43344506)),
    ); // 74.4833f, 180.348f, 75.1667f, 180.296f, 75.85f, 180.27f
    path.cubic_to(
        (f32::from_bits(0x42991111), f32::from_bits(0x43343e58)),
        (f32::from_bits(0x429a6eef), f32::from_bits(0x433438a6)),
        (f32::from_bits(0x429bcccd), f32::from_bits(0x4334369e)),
    ); // 76.5333f, 180.244f, 77.2167f, 180.221f, 77.9f, 180.213f
    path.cubic_to(
        (f32::from_bits(0x429d2aab), f32::from_bits(0x43343496)),
        (f32::from_bits(0x429e8889), f32::from_bits(0x43343fb3)),
        (f32::from_bits(0x429fe666), f32::from_bits(0x433438d5)),
    ); // 78.5833f, 180.205f, 79.2667f, 180.249f, 79.95f, 180.222f
    path.cubic_to(
        (f32::from_bits(0x42a14444), f32::from_bits(0x433431f8)),
        (f32::from_bits(0x42a2a222), f32::from_bits(0x433411e5)),
        (f32::from_bits(0x42a40000), f32::from_bits(0x43340d6e)),
    ); // 80.6333f, 180.195f, 81.3167f, 180.07f, 82, 180.052f
    path.cubic_to(
        (f32::from_bits(0x42a55dde), f32::from_bits(0x433408f8)),
        (f32::from_bits(0x42a6bbbc), f32::from_bits(0x43341ae2)),
        (f32::from_bits(0x42a8199a), f32::from_bits(0x43341e0e)),
    ); // 82.6833f, 180.035f, 83.3667f, 180.105f, 84.05f, 180.117f
    path.cubic_to(
        (f32::from_bits(0x42a97777), f32::from_bits(0x43342139)),
        (f32::from_bits(0x42aad555), f32::from_bits(0x433427a8)),
        (f32::from_bits(0x42ac3333), f32::from_bits(0x43342073)),
    ); // 84.7333f, 180.13f, 85.4167f, 180.155f, 86.1f, 180.127f
    path.cubic_to(
        (f32::from_bits(0x42ad9111), f32::from_bits(0x4334193f)),
        (f32::from_bits(0x42aeeeef), f32::from_bits(0x4333fa48)),
        (f32::from_bits(0x42b04ccd), f32::from_bits(0x4333f2d5)),
    ); // 86.7833f, 180.099f, 87.4667f, 179.978f, 88.15f, 179.949f
    path.cubic_to(
        (f32::from_bits(0x42b1aaab), f32::from_bits(0x4333eb62)),
        (f32::from_bits(0x42b30889), f32::from_bits(0x4333f0fd)),
        (f32::from_bits(0x42b46666), f32::from_bits(0x4333f3c3)),
    ); // 88.8333f, 179.919f, 89.5167f, 179.941f, 90.2f, 179.952f
    path.cubic_to(
        (f32::from_bits(0x42b5c444), f32::from_bits(0x4333f688)),
        (f32::from_bits(0x42b72222), f32::from_bits(0x4333f7ca)),
        (f32::from_bits(0x42b88000), f32::from_bits(0x43340375)),
    ); // 90.8833f, 179.963f, 91.5667f, 179.968f, 92.25f, 180.014f
    path.cubic_to(
        (f32::from_bits(0x42b9ddde), f32::from_bits(0x43340f1f)),
        (f32::from_bits(0x42bb3bbc), f32::from_bits(0x43342b53)),
        (f32::from_bits(0x42bc999a), f32::from_bits(0x433439c3)),
    ); // 92.9333f, 180.059f, 93.6167f, 180.169f, 94.3f, 180.226f
    path.cubic_to(
        (f32::from_bits(0x42bdf777), f32::from_bits(0x43344833)),
        (f32::from_bits(0x42bf5555), f32::from_bits(0x43345473)),
        (f32::from_bits(0x42c0b333), f32::from_bits(0x43345a15)),
    ); // 94.9833f, 180.282f, 95.6667f, 180.33f, 96.35f, 180.352f
    path.cubic_to(
        (f32::from_bits(0x42c21111), f32::from_bits(0x43345fb6)),
        (f32::from_bits(0x42c36eef), f32::from_bits(0x433467f5)),
        (f32::from_bits(0x42c4cccd), f32::from_bits(0x43345b8d)),
    ); // 97.0333f, 180.374f, 97.7167f, 180.406f, 98.4f, 180.358f
    path.cubic_to(
        (f32::from_bits(0x42c62aab), f32::from_bits(0x43344f25)),
        (f32::from_bits(0x42c78889), f32::from_bits(0x43342bb0)),
        (f32::from_bits(0x42c8e666), f32::from_bits(0x43340fa6)),
    ); // 99.0833f, 180.309f, 99.7667f, 180.171f, 100.45f, 180.061f
    path.cubic_to(
        (f32::from_bits(0x42ca4444), f32::from_bits(0x4333f39b)),
        (f32::from_bits(0x42cba222), f32::from_bits(0x4333c2e0)),
        (f32::from_bits(0x42cd0000), f32::from_bits(0x4333b34d)),
    ); // 101.133f, 179.952f, 101.817f, 179.761f, 102.5f, 179.7f
    path.cubic_to(
        (f32::from_bits(0x42ce5dde), f32::from_bits(0x4333a3b9)),
        (f32::from_bits(0x42cfbbbc), f32::from_bits(0x4333b115)),
        (f32::from_bits(0x42d1199a), f32::from_bits(0x4333b231)),
    ); // 103.183f, 179.64f, 103.867f, 179.692f, 104.55f, 179.696f
    path.cubic_to(
        (f32::from_bits(0x42d27777), f32::from_bits(0x4333b34d)),
        (f32::from_bits(0x42d3d555), f32::from_bits(0x4333b6a0)),
        (f32::from_bits(0x42d53333), f32::from_bits(0x4333b9f3)),
    ); // 105.233f, 179.7f, 105.917f, 179.713f, 106.6f, 179.726f
    path.cubic_to(
        (f32::from_bits(0x42d69111), f32::from_bits(0x4333bd46)),
        (f32::from_bits(0x42d7eeef), f32::from_bits(0x4333c308)),
        (f32::from_bits(0x42d94ccd), f32::from_bits(0x4333c624)),
    ); // 107.283f, 179.739f, 107.967f, 179.762f, 108.65f, 179.774f
    path.cubic_to(
        (f32::from_bits(0x42daaaab), f32::from_bits(0x4333c940)),
        (f32::from_bits(0x42dc0889), f32::from_bits(0x4333b41c)),
        (f32::from_bits(0x42dd6666), f32::from_bits(0x4333cc9c)),
    ); // 109.333f, 179.786f, 110.017f, 179.704f, 110.7f, 179.799f
    path.cubic_to(
        (f32::from_bits(0x42dec444), f32::from_bits(0x4333e51d)),
        (f32::from_bits(0x42e02222), f32::from_bits(0x43343f85)),
        (f32::from_bits(0x42e18000), f32::from_bits(0x43345927)),
    ); // 111.383f, 179.895f, 112.067f, 180.248f, 112.75f, 180.348f
    path.cubic_to(
        (f32::from_bits(0x42e2ddde), f32::from_bits(0x433472c9)),
        (f32::from_bits(0x42e43bbc), f32::from_bits(0x43346431)),
        (f32::from_bits(0x42e5999a), f32::from_bits(0x43346666)),
    ); // 113.433f, 180.448f, 114.117f, 180.391f, 114.8f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x42e6f777), f32::from_bits(0x4334689c)),
        (f32::from_bits(0x42e85555), f32::from_bits(0x43346666)),
        (f32::from_bits(0x42e9b333), f32::from_bits(0x43346666)),
    ); // 115.483f, 180.409f, 116.167f, 180.4f, 116.85f, 180.4f
    path.line_to((f32::from_bits(0x42e9b333), f32::from_bits(0x43346666))); // 116.85f, 180.4f
    path.line_to((f32::from_bits(0x42033333), f32::from_bits(0x43346666))); // 32.8f, 180.4f
    path.close();
    path.move_to((f32::from_bits(0x43054000), f32::from_bits(0x43346666))); // 133.25f, 180.4f
    path.line_to((f32::from_bits(0x43054000), f32::from_bits(0x43346666))); // 133.25f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x4305eeef), f32::from_bits(0x43346666)),
        (f32::from_bits(0x43069dde), f32::from_bits(0x43347a6e)),
        (f32::from_bits(0x43074ccd), f32::from_bits(0x43346666)),
    ); // 133.933f, 180.4f, 134.617f, 180.478f, 135.3f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x4307fbbc), f32::from_bits(0x4334525f)),
        (f32::from_bits(0x4308aaab), f32::from_bits(0x43340a40)),
        (f32::from_bits(0x4309599a), f32::from_bits(0x4333ee38)),
    ); // 135.983f, 180.322f, 136.667f, 180.04f, 137.35f, 179.931f
    path.cubic_to(
        (f32::from_bits(0x430a0889), f32::from_bits(0x4333d230)),
        (f32::from_bits(0x430ab777), f32::from_bits(0x4333c68b)),
        (f32::from_bits(0x430b6666), f32::from_bits(0x4333be34)),
    ); // 138.033f, 179.821f, 138.717f, 179.776f, 139.4f, 179.743f
    path.cubic_to(
        (f32::from_bits(0x430c1555), f32::from_bits(0x4333b5dc)),
        (f32::from_bits(0x430cc444), f32::from_bits(0x4333bc82)),
        (f32::from_bits(0x430d7333), f32::from_bits(0x4333bc2b)),
    ); // 140.083f, 179.71f, 140.767f, 179.736f, 141.45f, 179.735f
    path.cubic_to(
        (f32::from_bits(0x430e2222), f32::from_bits(0x4333bbd4)),
        (f32::from_bits(0x430ed111), f32::from_bits(0x4333bd76)),
        (f32::from_bits(0x430f8000), f32::from_bits(0x4333bc2b)),
    ); // 142.133f, 179.734f, 142.817f, 179.74f, 143.5f, 179.735f
    path.cubic_to(
        (f32::from_bits(0x43102eef), f32::from_bits(0x4333bae0)),
        (f32::from_bits(0x4310ddde), f32::from_bits(0x4333b72e)),
        (f32::from_bits(0x43118ccd), f32::from_bits(0x4333b469)),
    ); // 144.183f, 179.73f, 144.867f, 179.716f, 145.55f, 179.705f
    path.cubic_to(
        (f32::from_bits(0x43123bbc), f32::from_bits(0x4333b1a3)),
        (f32::from_bits(0x4312eaab), f32::from_bits(0x4333ad34)),
        (f32::from_bits(0x4313999a), f32::from_bits(0x4333ab8b)),
    ); // 146.233f, 179.694f, 146.917f, 179.677f, 147.6f, 179.67f
    path.cubic_to(
        (f32::from_bits(0x43144889), f32::from_bits(0x4333a9e1)),
        (f32::from_bits(0x4314f777), f32::from_bits(0x4333aa97)),
        (f32::from_bits(0x4315a666), f32::from_bits(0x4333aa6f)),
    ); // 148.283f, 179.664f, 148.967f, 179.666f, 149.65f, 179.666f
    path.cubic_to(
        (f32::from_bits(0x43165555), f32::from_bits(0x4333aa48)),
        (f32::from_bits(0x43170444), f32::from_bits(0x4333aac5)),
        (f32::from_bits(0x4317b333), f32::from_bits(0x4333aa9d)),
    ); // 150.333f, 179.665f, 151.017f, 179.667f, 151.7f, 179.666f
    path.cubic_to(
        (f32::from_bits(0x43186222), f32::from_bits(0x4333aa76)),
        (f32::from_bits(0x43191111), f32::from_bits(0x4333a962)),
        (f32::from_bits(0x4319c000), f32::from_bits(0x4333a982)),
    ); // 152.383f, 179.666f, 153.067f, 179.662f, 153.75f, 179.662f
    path.cubic_to(
        (f32::from_bits(0x431a6eef), f32::from_bits(0x4333a9a2)),
        (f32::from_bits(0x431b1dde), f32::from_bits(0x4333ab0e)),
        (f32::from_bits(0x431bcccd), f32::from_bits(0x4333ab5d)),
    ); // 154.433f, 179.663f, 155.117f, 179.668f, 155.8f, 179.669f
    path.cubic_to(
        (f32::from_bits(0x431c7bbc), f32::from_bits(0x4333abac)),
        (f32::from_bits(0x431d2aab), f32::from_bits(0x4333ab84)),
        (f32::from_bits(0x431dd99a), f32::from_bits(0x4333ab5d)),
    ); // 156.483f, 179.671f, 157.167f, 179.67f, 157.85f, 179.669f
    path.cubic_to(
        (f32::from_bits(0x431e8889), f32::from_bits(0x4333ab35)),
        (f32::from_bits(0x431f3777), f32::from_bits(0x4333aa8f)),
        (f32::from_bits(0x431fe666), f32::from_bits(0x4333aa6f)),
    ); // 158.533f, 179.669f, 159.217f, 179.666f, 159.9f, 179.666f
    path.cubic_to(
        (f32::from_bits(0x43209555), f32::from_bits(0x4333aa4f)),
        (f32::from_bits(0x43214444), f32::from_bits(0x4333a9b1)),
        (f32::from_bits(0x4321f333), f32::from_bits(0x4333aa9d)),
    ); // 160.583f, 179.665f, 161.267f, 179.663f, 161.95f, 179.666f
    path.cubic_to(
        (f32::from_bits(0x4322a222), f32::from_bits(0x4333ab8a)),
        (f32::from_bits(0x43235111), f32::from_bits(0x4333aeb6)),
        (f32::from_bits(0x43240000), f32::from_bits(0x4333affa)),
    ); // 162.633f, 179.67f, 163.317f, 179.682f, 164, 179.687f
    path.cubic_to(
        (f32::from_bits(0x4324aeef), f32::from_bits(0x4333b13d)),
        (f32::from_bits(0x43255dde), f32::from_bits(0x4333b1a3)),
        (f32::from_bits(0x43260ccd), f32::from_bits(0x4333b231)),
    ); // 164.683f, 179.692f, 165.367f, 179.694f, 166.05f, 179.696f
    path.cubic_to(
        (f32::from_bits(0x4326bbbc), f32::from_bits(0x4333b2bf)),
        (f32::from_bits(0x43276aab), f32::from_bits(0x4333b439)),
        (f32::from_bits(0x4328199a), f32::from_bits(0x4333b34d)),
    ); // 166.733f, 179.698f, 167.417f, 179.704f, 168.1f, 179.7f
    path.cubic_to(
        (f32::from_bits(0x4328c889), f32::from_bits(0x4333b260)),
        (f32::from_bits(0x43297777), f32::from_bits(0x4333ae48)),
        (f32::from_bits(0x432a2666), f32::from_bits(0x4333aca7)),
    ); // 168.783f, 179.697f, 169.467f, 179.681f, 170.15f, 179.674f
    path.cubic_to(
        (f32::from_bits(0x432ad555), f32::from_bits(0x4333ab05)),
        (f32::from_bits(0x432b8444), f32::from_bits(0x4333a9d8)),
        (f32::from_bits(0x432c3333), f32::from_bits(0x4333a982)),
    ); // 170.833f, 179.668f, 171.517f, 179.663f, 172.2f, 179.662f
    path.cubic_to(
        (f32::from_bits(0x432ce222), f32::from_bits(0x4333a92b)),
        (f32::from_bits(0x432d9111), f32::from_bits(0x4333a63e)),
        (f32::from_bits(0x432e4000), f32::from_bits(0x4333aa9d)),
    ); // 172.883f, 179.661f, 173.567f, 179.649f, 174.25f, 179.666f
    path.cubic_to(
        (f32::from_bits(0x432eeeef), f32::from_bits(0x4333aefd)),
        (f32::from_bits(0x432f9dde), f32::from_bits(0x4333aacf)),
        (f32::from_bits(0x43304ccd), f32::from_bits(0x4333c3bf)),
    ); // 174.933f, 179.684f, 175.617f, 179.667f, 176.3f, 179.765f
    path.cubic_to(
        (f32::from_bits(0x4330fbbc), f32::from_bits(0x4333dcae)),
        (f32::from_bits(0x4331aaab), f32::from_bits(0x433427ba)),
        (f32::from_bits(0x4332599a), f32::from_bits(0x4334403b)),
    ); // 176.983f, 179.862f, 177.667f, 180.155f, 178.35f, 180.251f
    path.cubic_to(
        (f32::from_bits(0x43330889), f32::from_bits(0x433458bc)),
        (f32::from_bits(0x4333b777), f32::from_bits(0x43345065)),
        (f32::from_bits(0x43346666), f32::from_bits(0x433456c2)),
    ); // 179.033f, 180.347f, 179.717f, 180.314f, 180.4f, 180.339f
    path.cubic_to(
        (f32::from_bits(0x43351555), f32::from_bits(0x43345d1e)),
        (f32::from_bits(0x4335c444), f32::from_bits(0x433463cb)),
        (f32::from_bits(0x43367333), f32::from_bits(0x43346666)),
    ); // 181.083f, 180.364f, 181.767f, 180.39f, 182.45f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x43372222), f32::from_bits(0x43346902)),
        (f32::from_bits(0x4337d111), f32::from_bits(0x43346666)),
        (f32::from_bits(0x43388000), f32::from_bits(0x43346666)),
    ); // 183.133f, 180.41f, 183.817f, 180.4f, 184.5f, 180.4f
    path.line_to((f32::from_bits(0x43388000), f32::from_bits(0x43346666))); // 184.5f, 180.4f
    path.line_to((f32::from_bits(0x43054000), f32::from_bits(0x43346666))); // 133.25f, 180.4f
    path.close();
    path.move_to((f32::from_bits(0x433a8ccd), f32::from_bits(0x43346666))); // 186.55f, 180.4f
    path.line_to((f32::from_bits(0x433a8ccd), f32::from_bits(0x43346666))); // 186.55f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x433b3bbc), f32::from_bits(0x433465db)),
        (f32::from_bits(0x433beaab), f32::from_bits(0x433463ac)),
        (f32::from_bits(0x433c999a), f32::from_bits(0x43346321)),
    ); // 187.233f, 180.398f, 187.917f, 180.389f, 188.6f, 180.387f
    path.cubic_to(
        (f32::from_bits(0x433d4889), f32::from_bits(0x43346295)),
        (f32::from_bits(0x433df777), f32::from_bits(0x43346295)),
        (f32::from_bits(0x433ea666), f32::from_bits(0x43346321)),
    ); // 189.283f, 180.385f, 189.967f, 180.385f, 190.65f, 180.387f
    path.cubic_to(
        (f32::from_bits(0x433f5555), f32::from_bits(0x433463ac)),
        (f32::from_bits(0x43400444), f32::from_bits(0x433465db)),
        (f32::from_bits(0x4340b333), f32::from_bits(0x43346666)),
    ); // 191.333f, 180.389f, 192.017f, 180.398f, 192.7f, 180.4f
    path.line_to((f32::from_bits(0x4340b333), f32::from_bits(0x43346666))); // 192.7f, 180.4f
    path.line_to((f32::from_bits(0x433a8ccd), f32::from_bits(0x43346666))); // 186.55f, 180.4f
    path.close();
    let pathA = path.detach();
    path.move_to((f32::from_bits(0x42033333), f32::from_bits(0x43346666))); // 32.8f, 180.4f
    path.line_to((f32::from_bits(0x42033333), f32::from_bits(0x43346666))); // 32.8f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x4205eeef), f32::from_bits(0x43346666)),
        (f32::from_bits(0x4208aaab), f32::from_bits(0x433454bd)),
        (f32::from_bits(0x420b6666), f32::from_bits(0x43346666)),
    ); // 33.4833f, 180.4f, 34.1667f, 180.331f, 34.85f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x420e2222), f32::from_bits(0x4334780e)),
        (f32::from_bits(0x4210ddde), f32::from_bits(0x4334bf76)),
        (f32::from_bits(0x4213999a), f32::from_bits(0x4334d05a)),
    ); // 35.5333f, 180.469f, 36.2167f, 180.748f, 36.9f, 180.814f
    path.cubic_to(
        (f32::from_bits(0x42165555), f32::from_bits(0x4334e13e)),
        (f32::from_bits(0x42191111), f32::from_bits(0x4334d0d8)),
        (f32::from_bits(0x421bcccd), f32::from_bits(0x4334cbbd)),
    ); // 37.5833f, 180.88f, 38.2667f, 180.816f, 38.95f, 180.796f
    path.cubic_to(
        (f32::from_bits(0x421e8889), f32::from_bits(0x4334c6a1)),
        (f32::from_bits(0x42214444), f32::from_bits(0x4334bab9)),
        (f32::from_bits(0x42240000), f32::from_bits(0x4334b1b5)),
    ); // 39.6333f, 180.776f, 40.3167f, 180.729f, 41, 180.694f
    path.cubic_to(
        (f32::from_bits(0x4226bbbc), f32::from_bits(0x4334a8b1)),
        (f32::from_bits(0x42297777), f32::from_bits(0x4334992e)),
        (f32::from_bits(0x422c3333), f32::from_bits(0x433495a4)),
    ); // 41.6833f, 180.659f, 42.3667f, 180.598f, 43.05f, 180.585f
    path.cubic_to(
        (f32::from_bits(0x422eeeef), f32::from_bits(0x4334921a)),
        (f32::from_bits(0x4231aaab), f32::from_bits(0x4334994d)),
        (f32::from_bits(0x42346666), f32::from_bits(0x43349c78)),
    ); // 43.7333f, 180.571f, 44.4167f, 180.599f, 45.1f, 180.611f
    path.cubic_to(
        (f32::from_bits(0x42372222), f32::from_bits(0x43349fa4)),
        (f32::from_bits(0x4239ddde), f32::from_bits(0x4334aa4b)),
        (f32::from_bits(0x423c999a), f32::from_bits(0x4334a8a9)),
    ); // 45.7833f, 180.624f, 46.4667f, 180.665f, 47.15f, 180.659f
    path.cubic_to(
        (f32::from_bits(0x423f5555), f32::from_bits(0x4334a707)),
        (f32::from_bits(0x42421111), f32::from_bits(0x4334994b)),
        (f32::from_bits(0x4244cccd), f32::from_bits(0x433492ad)),
    ); // 47.8333f, 180.652f, 48.5167f, 180.599f, 49.2f, 180.573f
    path.cubic_to(
        (f32::from_bits(0x42478889), f32::from_bits(0x43348c0e)),
        (f32::from_bits(0x424a4444), f32::from_bits(0x43347f6f)),
        (f32::from_bits(0x424d0000), f32::from_bits(0x433480f1)),
    ); // 49.8833f, 180.547f, 50.5667f, 180.498f, 51.25f, 180.504f
    path.cubic_to(
        (f32::from_bits(0x424fbbbc), f32::from_bits(0x43348273)),
        (f32::from_bits(0x42527777), f32::from_bits(0x4334a025)),
        (f32::from_bits(0x42553333), f32::from_bits(0x43349bb9)),
    ); // 51.9333f, 180.51f, 52.6167f, 180.626f, 53.3f, 180.608f
    path.cubic_to(
        (f32::from_bits(0x4257eeef), f32::from_bits(0x4334974c)),
        (f32::from_bits(0x425aaaab), f32::from_bits(0x43346781)),
        (f32::from_bits(0x425d6666), f32::from_bits(0x43346666)),
    ); // 53.9833f, 180.591f, 54.6667f, 180.404f, 55.35f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x42602222), f32::from_bits(0x4334654a)),
        (f32::from_bits(0x4262ddde), f32::from_bits(0x43348de8)),
        (f32::from_bits(0x4265999a), f32::from_bits(0x43349512)),
    ); // 56.0333f, 180.396f, 56.7167f, 180.554f, 57.4f, 180.582f
    path.cubic_to(
        (f32::from_bits(0x42685555), f32::from_bits(0x43349c3d)),
        (f32::from_bits(0x426b1111), f32::from_bits(0x43348fe9)),
        (f32::from_bits(0x426dcccd), f32::from_bits(0x43349163)),
    ); // 58.0833f, 180.61f, 58.7667f, 180.562f, 59.45f, 180.568f
    path.cubic_to(
        (f32::from_bits(0x42708889), f32::from_bits(0x433492dd)),
        (f32::from_bits(0x42734444), f32::from_bits(0x433494f4)),
        (f32::from_bits(0x42760000), f32::from_bits(0x43349df0)),
    ); // 60.1333f, 180.574f, 60.8167f, 180.582f, 61.5f, 180.617f
    path.cubic_to(
        (f32::from_bits(0x4278bbbc), f32::from_bits(0x4334a6ed)),
        (f32::from_bits(0x427b7777), f32::from_bits(0x4334bf7d)),
        (f32::from_bits(0x427e3333), f32::from_bits(0x4334c74e)),
    ); // 62.1833f, 180.652f, 62.8667f, 180.748f, 63.55f, 180.779f
    path.cubic_to(
        (f32::from_bits(0x42807777), f32::from_bits(0x4334cf1f)),
        (f32::from_bits(0x4281d555), f32::from_bits(0x4334d177)),
        (f32::from_bits(0x42833333), f32::from_bits(0x4334ccd9)),
    ); // 64.2333f, 180.809f, 64.9167f, 180.818f, 65.6f, 180.8f
    path.cubic_to(
        (f32::from_bits(0x42849111), f32::from_bits(0x4334c83a)),
        (f32::from_bits(0x4285eeef), f32::from_bits(0x4334bcac)),
        (f32::from_bits(0x42874ccd), f32::from_bits(0x4334ab99)),
    ); // 66.2833f, 180.782f, 66.9667f, 180.737f, 67.65f, 180.67f
    path.cubic_to(
        (f32::from_bits(0x4288aaab), f32::from_bits(0x43349a86)),
        (f32::from_bits(0x428a0889), f32::from_bits(0x433471ee)),
        (f32::from_bits(0x428b6666), f32::from_bits(0x43346666)),
    ); // 68.3333f, 180.604f, 69.0167f, 180.445f, 69.7f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x428cc444), f32::from_bits(0x43345add)),
        (f32::from_bits(0x428e2222), f32::from_bits(0x4334651d)),
        (f32::from_bits(0x428f8000), f32::from_bits(0x43346666)),
    ); // 70.3833f, 180.355f, 71.0667f, 180.395f, 71.75f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x4290ddde), f32::from_bits(0x433467ae)),
        (f32::from_bits(0x42923bbc), f32::from_bits(0x4334688a)),
        (f32::from_bits(0x4293999a), f32::from_bits(0x43346e1a)),
    ); // 72.4333f, 180.405f, 73.1167f, 180.408f, 73.8f, 180.43f
    path.cubic_to(
        (f32::from_bits(0x4294f777), f32::from_bits(0x433473aa)),
        (f32::from_bits(0x42965555), f32::from_bits(0x43348118)),
        (f32::from_bits(0x4297b333), f32::from_bits(0x433487c6)),
    ); // 74.4833f, 180.452f, 75.1667f, 180.504f, 75.85f, 180.53f
    path.cubic_to(
        (f32::from_bits(0x42991111), f32::from_bits(0x43348e74)),
        (f32::from_bits(0x429a6eef), f32::from_bits(0x43349426)),
        (f32::from_bits(0x429bcccd), f32::from_bits(0x4334962e)),
    ); // 76.5333f, 180.556f, 77.2167f, 180.579f, 77.9f, 180.587f
    path.cubic_to(
        (f32::from_bits(0x429d2aab), f32::from_bits(0x43349836)),
        (f32::from_bits(0x429e8889), f32::from_bits(0x43348d19)),
        (f32::from_bits(0x429fe666), f32::from_bits(0x433493f7)),
    ); // 78.5833f, 180.595f, 79.2667f, 180.551f, 79.95f, 180.578f
    path.cubic_to(
        (f32::from_bits(0x42a14444), f32::from_bits(0x43349ad4)),
        (f32::from_bits(0x42a2a222), f32::from_bits(0x4334bae7)),
        (f32::from_bits(0x42a40000), f32::from_bits(0x4334bf5e)),
    ); // 80.6333f, 180.605f, 81.3167f, 180.73f, 82, 180.748f
    path.cubic_to(
        (f32::from_bits(0x42a55dde), f32::from_bits(0x4334c3d4)),
        (f32::from_bits(0x42a6bbbc), f32::from_bits(0x4334b1ea)),
        (f32::from_bits(0x42a8199a), f32::from_bits(0x4334aebe)),
    ); // 82.6833f, 180.765f, 83.3667f, 180.695f, 84.05f, 180.683f
    path.cubic_to(
        (f32::from_bits(0x42a97777), f32::from_bits(0x4334ab93)),
        (f32::from_bits(0x42aad555), f32::from_bits(0x4334a524)),
        (f32::from_bits(0x42ac3333), f32::from_bits(0x4334ac59)),
    ); // 84.7333f, 180.67f, 85.4167f, 180.645f, 86.1f, 180.673f
    path.cubic_to(
        (f32::from_bits(0x42ad9111), f32::from_bits(0x4334b38d)),
        (f32::from_bits(0x42aeeeef), f32::from_bits(0x4334d284)),
        (f32::from_bits(0x42b04ccd), f32::from_bits(0x4334d9f7)),
    ); // 86.7833f, 180.701f, 87.4667f, 180.822f, 88.15f, 180.851f
    path.cubic_to(
        (f32::from_bits(0x42b1aaab), f32::from_bits(0x4334e16a)),
        (f32::from_bits(0x42b30889), f32::from_bits(0x4334dbcf)),
        (f32::from_bits(0x42b46666), f32::from_bits(0x4334d909)),
    ); // 88.8333f, 180.881f, 89.5167f, 180.859f, 90.2f, 180.848f
    path.cubic_to(
        (f32::from_bits(0x42b5c444), f32::from_bits(0x4334d644)),
        (f32::from_bits(0x42b72222), f32::from_bits(0x4334d502)),
        (f32::from_bits(0x42b88000), f32::from_bits(0x4334c957)),
    ); // 90.8833f, 180.837f, 91.5667f, 180.832f, 92.25f, 180.786f
    path.cubic_to(
        (f32::from_bits(0x42b9ddde), f32::from_bits(0x4334bdad)),
        (f32::from_bits(0x42bb3bbc), f32::from_bits(0x4334a179)),
        (f32::from_bits(0x42bc999a), f32::from_bits(0x43349309)),
    ); // 92.9333f, 180.741f, 93.6167f, 180.631f, 94.3f, 180.574f
    path.cubic_to(
        (f32::from_bits(0x42bdf777), f32::from_bits(0x43348499)),
        (f32::from_bits(0x42bf5555), f32::from_bits(0x43347859)),
        (f32::from_bits(0x42c0b333), f32::from_bits(0x433472b7)),
    ); // 94.9833f, 180.518f, 95.6667f, 180.47f, 96.35f, 180.448f
    path.cubic_to(
        (f32::from_bits(0x42c21111), f32::from_bits(0x43346d16)),
        (f32::from_bits(0x42c36eef), f32::from_bits(0x433464d7)),
        (f32::from_bits(0x42c4cccd), f32::from_bits(0x4334713f)),
    ); // 97.0333f, 180.426f, 97.7167f, 180.394f, 98.4f, 180.442f
    path.cubic_to(
        (f32::from_bits(0x42c62aab), f32::from_bits(0x43347da7)),
        (f32::from_bits(0x42c78889), f32::from_bits(0x4334a11c)),
        (f32::from_bits(0x42c8e666), f32::from_bits(0x4334bd26)),
    ); // 99.0833f, 180.491f, 99.7667f, 180.629f, 100.45f, 180.739f
    path.cubic_to(
        (f32::from_bits(0x42ca4444), f32::from_bits(0x4334d931)),
        (f32::from_bits(0x42cba222), f32::from_bits(0x433509ec)),
        (f32::from_bits(0x42cd0000), f32::from_bits(0x4335197f)),
    ); // 101.133f, 180.848f, 101.817f, 181.039f, 102.5f, 181.1f
    path.cubic_to(
        (f32::from_bits(0x42ce5dde), f32::from_bits(0x43352913)),
        (f32::from_bits(0x42cfbbbc), f32::from_bits(0x43351bb7)),
        (f32::from_bits(0x42d1199a), f32::from_bits(0x43351a9b)),
    ); // 103.183f, 181.16f, 103.867f, 181.108f, 104.55f, 181.104f
    path.cubic_to(
        (f32::from_bits(0x42d27777), f32::from_bits(0x4335197f)),
        (f32::from_bits(0x42d3d555), f32::from_bits(0x4335162c)),
        (f32::from_bits(0x42d53333), f32::from_bits(0x433512d9)),
    ); // 105.233f, 181.1f, 105.917f, 181.087f, 106.6f, 181.074f
    path.cubic_to(
        (f32::from_bits(0x42d69111), f32::from_bits(0x43350f86)),
        (f32::from_bits(0x42d7eeef), f32::from_bits(0x433509c4)),
        (f32::from_bits(0x42d94ccd), f32::from_bits(0x433506a8)),
    ); // 107.283f, 181.061f, 107.967f, 181.038f, 108.65f, 181.026f
    path.cubic_to(
        (f32::from_bits(0x42daaaab), f32::from_bits(0x4335038c)),
        (f32::from_bits(0x42dc0889), f32::from_bits(0x433518b0)),
        (f32::from_bits(0x42dd6666), f32::from_bits(0x43350030)),
    ); // 109.333f, 181.014f, 110.017f, 181.096f, 110.7f, 181.001f
    path.cubic_to(
        (f32::from_bits(0x42dec444), f32::from_bits(0x4334e7af)),
        (f32::from_bits(0x42e02222), f32::from_bits(0x43348d47)),
        (f32::from_bits(0x42e18000), f32::from_bits(0x433473a5)),
    ); // 111.383f, 180.905f, 112.067f, 180.552f, 112.75f, 180.452f
    path.cubic_to(
        (f32::from_bits(0x42e2ddde), f32::from_bits(0x43345a03)),
        (f32::from_bits(0x42e43bbc), f32::from_bits(0x4334689b)),
        (f32::from_bits(0x42e5999a), f32::from_bits(0x43346666)),
    ); // 113.433f, 180.352f, 114.117f, 180.409f, 114.8f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x42e6f777), f32::from_bits(0x43346430)),
        (f32::from_bits(0x42e85555), f32::from_bits(0x43346666)),
        (f32::from_bits(0x42e9b333), f32::from_bits(0x43346666)),
    ); // 115.483f, 180.391f, 116.167f, 180.4f, 116.85f, 180.4f
    path.line_to((f32::from_bits(0x42e9b333), f32::from_bits(0x43346666))); // 116.85f, 180.4f
    path.line_to((f32::from_bits(0x42033333), f32::from_bits(0x43346666))); // 32.8f, 180.4f
    path.close();
    path.move_to((f32::from_bits(0x43054000), f32::from_bits(0x43346666))); // 133.25f, 180.4f
    path.line_to((f32::from_bits(0x43054000), f32::from_bits(0x43346666))); // 133.25f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x4305eeef), f32::from_bits(0x43346666)),
        (f32::from_bits(0x43069dde), f32::from_bits(0x4334525e)),
        (f32::from_bits(0x43074ccd), f32::from_bits(0x43346666)),
    ); // 133.933f, 180.4f, 134.617f, 180.322f, 135.3f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x4307fbbc), f32::from_bits(0x43347a6d)),
        (f32::from_bits(0x4308aaab), f32::from_bits(0x4334c28c)),
        (f32::from_bits(0x4309599a), f32::from_bits(0x4334de94)),
    ); // 135.983f, 180.478f, 136.667f, 180.76f, 137.35f, 180.869f
    path.cubic_to(
        (f32::from_bits(0x430a0889), f32::from_bits(0x4334fa9c)),
        (f32::from_bits(0x430ab777), f32::from_bits(0x43350641)),
        (f32::from_bits(0x430b6666), f32::from_bits(0x43350e98)),
    ); // 138.033f, 180.979f, 138.717f, 181.024f, 139.4f, 181.057f
    path.cubic_to(
        (f32::from_bits(0x430c1555), f32::from_bits(0x433516f0)),
        (f32::from_bits(0x430cc444), f32::from_bits(0x4335104a)),
        (f32::from_bits(0x430d7333), f32::from_bits(0x433510a1)),
    ); // 140.083f, 181.09f, 140.767f, 181.064f, 141.45f, 181.065f
    path.cubic_to(
        (f32::from_bits(0x430e2222), f32::from_bits(0x433510f8)),
        (f32::from_bits(0x430ed111), f32::from_bits(0x43350f56)),
        (f32::from_bits(0x430f8000), f32::from_bits(0x433510a1)),
    ); // 142.133f, 181.066f, 142.817f, 181.06f, 143.5f, 181.065f
    path.cubic_to(
        (f32::from_bits(0x43102eef), f32::from_bits(0x433511ec)),
        (f32::from_bits(0x4310ddde), f32::from_bits(0x4335159e)),
        (f32::from_bits(0x43118ccd), f32::from_bits(0x43351863)),
    ); // 144.183f, 181.07f, 144.867f, 181.084f, 145.55f, 181.095f
    path.cubic_to(
        (f32::from_bits(0x43123bbc), f32::from_bits(0x43351b29)),
        (f32::from_bits(0x4312eaab), f32::from_bits(0x43351f98)),
        (f32::from_bits(0x4313999a), f32::from_bits(0x43352141)),
    ); // 146.233f, 181.106f, 146.917f, 181.123f, 147.6f, 181.13f
    path.cubic_to(
        (f32::from_bits(0x43144889), f32::from_bits(0x433522eb)),
        (f32::from_bits(0x4314f777), f32::from_bits(0x43352235)),
        (f32::from_bits(0x4315a666), f32::from_bits(0x4335225d)),
    ); // 148.283f, 181.136f, 148.967f, 181.134f, 149.65f, 181.134f
    path.cubic_to(
        (f32::from_bits(0x43165555), f32::from_bits(0x43352284)),
        (f32::from_bits(0x43170444), f32::from_bits(0x43352207)),
        (f32::from_bits(0x4317b333), f32::from_bits(0x4335222f)),
    ); // 150.333f, 181.135f, 151.017f, 181.133f, 151.7f, 181.134f
    path.cubic_to(
        (f32::from_bits(0x43186222), f32::from_bits(0x43352256)),
        (f32::from_bits(0x43191111), f32::from_bits(0x4335236a)),
        (f32::from_bits(0x4319c000), f32::from_bits(0x4335234a)),
    ); // 152.383f, 181.134f, 153.067f, 181.138f, 153.75f, 181.138f
    path.cubic_to(
        (f32::from_bits(0x431a6eef), f32::from_bits(0x4335232a)),
        (f32::from_bits(0x431b1dde), f32::from_bits(0x433521be)),
        (f32::from_bits(0x431bcccd), f32::from_bits(0x4335216f)),
    ); // 154.433f, 181.137f, 155.117f, 181.132f, 155.8f, 181.131f
    path.cubic_to(
        (f32::from_bits(0x431c7bbc), f32::from_bits(0x43352120)),
        (f32::from_bits(0x431d2aab), f32::from_bits(0x43352148)),
        (f32::from_bits(0x431dd99a), f32::from_bits(0x4335216f)),
    ); // 156.483f, 181.129f, 157.167f, 181.13f, 157.85f, 181.131f
    path.cubic_to(
        (f32::from_bits(0x431e8889), f32::from_bits(0x43352197)),
        (f32::from_bits(0x431f3777), f32::from_bits(0x4335223d)),
        (f32::from_bits(0x431fe666), f32::from_bits(0x4335225d)),
    ); // 158.533f, 181.131f, 159.217f, 181.134f, 159.9f, 181.134f
    path.cubic_to(
        (f32::from_bits(0x43209555), f32::from_bits(0x4335227d)),
        (f32::from_bits(0x43214444), f32::from_bits(0x4335231b)),
        (f32::from_bits(0x4321f333), f32::from_bits(0x4335222f)),
    ); // 160.583f, 181.135f, 161.267f, 181.137f, 161.95f, 181.134f
    path.cubic_to(
        (f32::from_bits(0x4322a222), f32::from_bits(0x43352142)),
        (f32::from_bits(0x43235111), f32::from_bits(0x43351e16)),
        (f32::from_bits(0x43240000), f32::from_bits(0x43351cd2)),
    ); // 162.633f, 181.13f, 163.317f, 181.118f, 164, 181.113f
    path.cubic_to(
        (f32::from_bits(0x4324aeef), f32::from_bits(0x43351b8f)),
        (f32::from_bits(0x43255dde), f32::from_bits(0x43351b29)),
        (f32::from_bits(0x43260ccd), f32::from_bits(0x43351a9b)),
    ); // 164.683f, 181.108f, 165.367f, 181.106f, 166.05f, 181.104f
    path.cubic_to(
        (f32::from_bits(0x4326bbbc), f32::from_bits(0x43351a0d)),
        (f32::from_bits(0x43276aab), f32::from_bits(0x43351893)),
        (f32::from_bits(0x4328199a), f32::from_bits(0x4335197f)),
    ); // 166.733f, 181.102f, 167.417f, 181.096f, 168.1f, 181.1f
    path.cubic_to(
        (f32::from_bits(0x4328c889), f32::from_bits(0x43351a6c)),
        (f32::from_bits(0x43297777), f32::from_bits(0x43351e84)),
        (f32::from_bits(0x432a2666), f32::from_bits(0x43352025)),
    ); // 168.783f, 181.103f, 169.467f, 181.119f, 170.15f, 181.126f
    path.cubic_to(
        (f32::from_bits(0x432ad555), f32::from_bits(0x433521c7)),
        (f32::from_bits(0x432b8444), f32::from_bits(0x433522f4)),
        (f32::from_bits(0x432c3333), f32::from_bits(0x4335234a)),
    ); // 170.833f, 181.132f, 171.517f, 181.137f, 172.2f, 181.138f
    path.cubic_to(
        (f32::from_bits(0x432ce222), f32::from_bits(0x433523a1)),
        (f32::from_bits(0x432d9111), f32::from_bits(0x4335268e)),
        (f32::from_bits(0x432e4000), f32::from_bits(0x4335222f)),
    ); // 172.883f, 181.139f, 173.567f, 181.151f, 174.25f, 181.134f
    path.cubic_to(
        (f32::from_bits(0x432eeeef), f32::from_bits(0x43351dcf)),
        (f32::from_bits(0x432f9dde), f32::from_bits(0x433521fd)),
        (f32::from_bits(0x43304ccd), f32::from_bits(0x4335090d)),
    ); // 174.933f, 181.116f, 175.617f, 181.133f, 176.3f, 181.035f
    path.cubic_to(
        (f32::from_bits(0x4330fbbc), f32::from_bits(0x4334f01e)),
        (f32::from_bits(0x4331aaab), f32::from_bits(0x4334a512)),
        (f32::from_bits(0x4332599a), f32::from_bits(0x43348c91)),
    ); // 176.983f, 180.938f, 177.667f, 180.645f, 178.35f, 180.549f
    path.cubic_to(
        (f32::from_bits(0x43330889), f32::from_bits(0x43347410)),
        (f32::from_bits(0x4333b777), f32::from_bits(0x43347c67)),
        (f32::from_bits(0x43346666), f32::from_bits(0x4334760a)),
    ); // 179.033f, 180.453f, 179.717f, 180.486f, 180.4f, 180.461f
    path.cubic_to(
        (f32::from_bits(0x43351555), f32::from_bits(0x43346fae)),
        (f32::from_bits(0x4335c444), f32::from_bits(0x43346901)),
        (f32::from_bits(0x43367333), f32::from_bits(0x43346666)),
    ); // 181.083f, 180.436f, 181.767f, 180.41f, 182.45f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x43372222), f32::from_bits(0x433463ca)),
        (f32::from_bits(0x4337d111), f32::from_bits(0x43346666)),
        (f32::from_bits(0x43388000), f32::from_bits(0x43346666)),
    ); // 183.133f, 180.39f, 183.817f, 180.4f, 184.5f, 180.4f
    path.line_to((f32::from_bits(0x43388000), f32::from_bits(0x43346666))); // 184.5f, 180.4f
    path.line_to((f32::from_bits(0x43054000), f32::from_bits(0x43346666))); // 133.25f, 180.4f
    path.close();
    path.move_to((f32::from_bits(0x433a8ccd), f32::from_bits(0x43346666))); // 186.55f, 180.4f
    path.line_to((f32::from_bits(0x433a8ccd), f32::from_bits(0x43346666))); // 186.55f, 180.4f
    path.cubic_to(
        (f32::from_bits(0x433b3bbc), f32::from_bits(0x433466f1)),
        (f32::from_bits(0x433beaab), f32::from_bits(0x43346920)),
        (f32::from_bits(0x433c999a), f32::from_bits(0x433469ab)),
    ); // 187.233f, 180.402f, 187.917f, 180.411f, 188.6f, 180.413f
    path.cubic_to(
        (f32::from_bits(0x433d4889), f32::from_bits(0x43346a37)),
        (f32::from_bits(0x433df777), f32::from_bits(0x43346a37)),
        (f32::from_bits(0x433ea666), f32::from_bits(0x433469ab)),
    ); // 189.283f, 180.415f, 189.967f, 180.415f, 190.65f, 180.413f
    path.cubic_to(
        (f32::from_bits(0x433f5555), f32::from_bits(0x43346920)),
        (f32::from_bits(0x43400444), f32::from_bits(0x433466f1)),
        (f32::from_bits(0x4340b333), f32::from_bits(0x43346666)),
    ); // 191.333f, 180.411f, 192.017f, 180.402f, 192.7f, 180.4f
    path.line_to((f32::from_bits(0x4340b333), f32::from_bits(0x43346666))); // 192.7f, 180.4f
    path.line_to((f32::from_bits(0x433a8ccd), f32::from_bits(0x43346666))); // 186.55f, 180.4f
    path.close();
    test_path_op(reporter, &pathA, &path.detach(), PathOp::Union, filename);
}

// Port of: tests/PathOpsIssue3651.cpp#L1407-L1468 (chrome/m156)
fn issue3651_5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x411e6666), f32::from_bits(0x4380b333))); // 9.9f, 257.4f
    path.line_to((f32::from_bits(0x411e6666), f32::from_bits(0x4380b333))); // 9.9f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x41300000), f32::from_bits(0x4380b333)),
        (f32::from_bits(0x4141999a), f32::from_bits(0x4380ba9e)),
        (f32::from_bits(0x41533333), f32::from_bits(0x4380b333)),
    ); // 11, 257.4f, 12.1f, 257.458f, 13.2f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x4164cccd), f32::from_bits(0x4380abc8)),
        (f32::from_bits(0x41766666), f32::from_bits(0x43809a93)),
        (f32::from_bits(0x41840000), f32::from_bits(0x438086b0)),
    ); // 14.3f, 257.342f, 15.4f, 257.208f, 16.5f, 257.052f
    path.cubic_to(
        (f32::from_bits(0x418ccccd), f32::from_bits(0x438072cc)),
        (f32::from_bits(0x4195999a), f32::from_bits(0x4380498b)),
        (f32::from_bits(0x419e6666), f32::from_bits(0x43803bdc)),
    ); // 17.6f, 256.897f, 18.7f, 256.575f, 19.8f, 256.468f
    path.cubic_to(
        (f32::from_bits(0x41a73333), f32::from_bits(0x43802e2d)),
        (f32::from_bits(0x41b00000), f32::from_bits(0x438026f3)),
        (f32::from_bits(0x41b8cccd), f32::from_bits(0x43803498)),
    ); // 20.9f, 256.361f, 22, 256.304f, 23.1f, 256.411f
    path.cubic_to(
        (f32::from_bits(0x41c1999a), f32::from_bits(0x4380423d)),
        (f32::from_bits(0x41ca6666), f32::from_bits(0x4380789f)),
        (f32::from_bits(0x41d33333), f32::from_bits(0x43808db9)),
    ); // 24.2f, 256.517f, 25.3f, 256.942f, 26.4f, 257.107f
    path.cubic_to(
        (f32::from_bits(0x41dc0000), f32::from_bits(0x4380a2d3)),
        (f32::from_bits(0x41e4cccd), f32::from_bits(0x4380b36f)),
        (f32::from_bits(0x41ed999a), f32::from_bits(0x4380b333)),
    ); // 27.5f, 257.272f, 28.6f, 257.402f, 29.7f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x41f66666), f32::from_bits(0x4380b2f7)),
        (f32::from_bits(0x41ff3333), f32::from_bits(0x4380a1a6)),
        (f32::from_bits(0x42040000), f32::from_bits(0x43808c51)),
    ); // 30.8f, 257.398f, 31.9f, 257.263f, 33, 257.096f
    path.cubic_to(
        (f32::from_bits(0x42086666), f32::from_bits(0x438076fc)),
        (f32::from_bits(0x420ccccd), f32::from_bits(0x43803f7a)),
        (f32::from_bits(0x42113333), f32::from_bits(0x43803333)),
    ); // 34.1f, 256.93f, 35.2f, 256.496f, 36.3f, 256.4f
    path.cubic_to(
        (f32::from_bits(0x4215999a), f32::from_bits(0x438026ed)),
        (f32::from_bits(0x421a0000), f32::from_bits(0x43802d56)),
        (f32::from_bits(0x421e6666), f32::from_bits(0x438042ab)),
    ); // 37.4f, 256.304f, 38.5f, 256.354f, 39.6f, 256.521f
    path.cubic_to(
        (f32::from_bits(0x4222cccd), f32::from_bits(0x43805800)),
        (f32::from_bits(0x42273333), f32::from_bits(0x4380a072)),
        (f32::from_bits(0x422b999a), f32::from_bits(0x4380b333)),
    ); // 40.7f, 256.688f, 41.8f, 257.253f, 42.9f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x42300000), f32::from_bits(0x4380c5f5)),
        (f32::from_bits(0x42346666), f32::from_bits(0x4380b333)),
        (f32::from_bits(0x4238cccd), f32::from_bits(0x4380b333)),
    ); // 44, 257.547f, 45.1f, 257.4f, 46.2f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x423d3333), f32::from_bits(0x4380b333)),
        (f32::from_bits(0x4241999a), f32::from_bits(0x4380c1e4)),
        (f32::from_bits(0x42460000), f32::from_bits(0x4380b333)),
    ); // 47.3f, 257.4f, 48.4f, 257.515f, 49.5f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x424a6666), f32::from_bits(0x4380a482)),
        (f32::from_bits(0x424ecccd), f32::from_bits(0x43807063)),
        (f32::from_bits(0x42533333), f32::from_bits(0x43805b0e)),
    ); // 50.6f, 257.285f, 51.7f, 256.878f, 52.8f, 256.711f
    path.cubic_to(
        (f32::from_bits(0x4257999a), f32::from_bits(0x438045b8)),
        (f32::from_bits(0x425c0000), f32::from_bits(0x438039d8)),
        (f32::from_bits(0x42606666), f32::from_bits(0x43803333)),
    ); // 53.9f, 256.545f, 55, 256.452f, 56.1f, 256.4f
    path.cubic_to(
        (f32::from_bits(0x4264cccd), f32::from_bits(0x43802c8f)),
        (f32::from_bits(0x42693333), f32::from_bits(0x43803333)),
        (f32::from_bits(0x426d999a), f32::from_bits(0x43803333)),
    ); // 57.2f, 256.348f, 58.3f, 256.4f, 59.4f, 256.4f
    path.cubic_to(
        (f32::from_bits(0x42720000), f32::from_bits(0x43803333)),
        (f32::from_bits(0x42766666), f32::from_bits(0x43802bdb)),
        (f32::from_bits(0x427acccd), f32::from_bits(0x43803333)),
    ); // 60.5f, 256.4f, 61.6f, 256.343f, 62.7f, 256.4f
    path.cubic_to(
        (f32::from_bits(0x427f3333), f32::from_bits(0x43803a8c)),
        (f32::from_bits(0x4281cccd), f32::from_bits(0x438049f1)),
        (f32::from_bits(0x42840000), f32::from_bits(0x43805f47)),
    ); // 63.8f, 256.457f, 64.9f, 256.578f, 66, 256.744f
    path.cubic_to(
        (f32::from_bits(0x42863333), f32::from_bits(0x4380749c)),
        (f32::from_bits(0x42886666), f32::from_bits(0x4380a536)),
        (f32::from_bits(0x428a999a), f32::from_bits(0x4380b333)),
    ); // 67.1f, 256.911f, 68.2f, 257.291f, 69.3f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x428ccccd), f32::from_bits(0x4380c130)),
        (f32::from_bits(0x428f0000), f32::from_bits(0x4380b333)),
        (f32::from_bits(0x42913333), f32::from_bits(0x4380b333)),
    ); // 70.4f, 257.509f, 71.5f, 257.4f, 72.6f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x42936666), f32::from_bits(0x4380b333)),
        (f32::from_bits(0x4295999a), f32::from_bits(0x4380bb17)),
        (f32::from_bits(0x4297cccd), f32::from_bits(0x4380b333)),
    ); // 73.7f, 257.4f, 74.8f, 257.462f, 75.9f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x429a0000), f32::from_bits(0x4380ab50)),
        (f32::from_bits(0x429c3333), f32::from_bits(0x438083df)),
        (f32::from_bits(0x429e6666), f32::from_bits(0x438083df)),
    ); // 77, 257.338f, 78.1f, 257.03f, 79.2f, 257.03f
    path.cubic_to(
        (f32::from_bits(0x42a0999a), f32::from_bits(0x438083df)),
        (f32::from_bits(0x42a2cccd), f32::from_bits(0x4380ab50)),
        (f32::from_bits(0x42a50000), f32::from_bits(0x4380b333)),
    ); // 80.3f, 257.03f, 81.4f, 257.338f, 82.5f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x42a73333), f32::from_bits(0x4380bb17)),
        (f32::from_bits(0x42a96666), f32::from_bits(0x4380b333)),
        (f32::from_bits(0x42ab999a), f32::from_bits(0x4380b333)),
    ); // 83.6f, 257.462f, 84.7f, 257.4f, 85.8f, 257.4f
    path.line_to((f32::from_bits(0x42ab999a), f32::from_bits(0x4380b333))); // 85.8f, 257.4f
    path.line_to((f32::from_bits(0x411e6666), f32::from_bits(0x4380b333))); // 9.9f, 257.4f
    path.close();
    let pathA = path.detach();
    path.move_to((f32::from_bits(0x411e6666), f32::from_bits(0x4380b333))); // 9.9f, 257.4f
    path.line_to((f32::from_bits(0x411e6666), f32::from_bits(0x4380b333))); // 9.9f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x41300000), f32::from_bits(0x4380b333)),
        (f32::from_bits(0x4141999a), f32::from_bits(0x4380abc8)),
        (f32::from_bits(0x41533333), f32::from_bits(0x4380b333)),
    ); // 11, 257.4f, 12.1f, 257.342f, 13.2f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x4164cccd), f32::from_bits(0x4380ba9e)),
        (f32::from_bits(0x41766666), f32::from_bits(0x4380cbd3)),
        (f32::from_bits(0x41840000), f32::from_bits(0x4380dfb6)),
    ); // 14.3f, 257.458f, 15.4f, 257.592f, 16.5f, 257.748f
    path.cubic_to(
        (f32::from_bits(0x418ccccd), f32::from_bits(0x4380f39a)),
        (f32::from_bits(0x4195999a), f32::from_bits(0x43811cdb)),
        (f32::from_bits(0x419e6666), f32::from_bits(0x43812a8a)),
    ); // 17.6f, 257.903f, 18.7f, 258.225f, 19.8f, 258.332f
    path.cubic_to(
        (f32::from_bits(0x41a73333), f32::from_bits(0x43813839)),
        (f32::from_bits(0x41b00000), f32::from_bits(0x43813f73)),
        (f32::from_bits(0x41b8cccd), f32::from_bits(0x438131ce)),
    ); // 20.9f, 258.439f, 22, 258.496f, 23.1f, 258.389f
    path.cubic_to(
        (f32::from_bits(0x41c1999a), f32::from_bits(0x43812429)),
        (f32::from_bits(0x41ca6666), f32::from_bits(0x4380edc7)),
        (f32::from_bits(0x41d33333), f32::from_bits(0x4380d8ad)),
    ); // 24.2f, 258.283f, 25.3f, 257.858f, 26.4f, 257.693f
    path.cubic_to(
        (f32::from_bits(0x41dc0000), f32::from_bits(0x4380c393)),
        (f32::from_bits(0x41e4cccd), f32::from_bits(0x4380b2f7)),
        (f32::from_bits(0x41ed999a), f32::from_bits(0x4380b333)),
    ); // 27.5f, 257.528f, 28.6f, 257.398f, 29.7f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x41f66666), f32::from_bits(0x4380b36f)),
        (f32::from_bits(0x41ff3333), f32::from_bits(0x4380c4c0)),
        (f32::from_bits(0x42040000), f32::from_bits(0x4380da15)),
    ); // 30.8f, 257.402f, 31.9f, 257.537f, 33, 257.704f
    path.cubic_to(
        (f32::from_bits(0x42086666), f32::from_bits(0x4380ef6a)),
        (f32::from_bits(0x420ccccd), f32::from_bits(0x438126ec)),
        (f32::from_bits(0x42113333), f32::from_bits(0x43813333)),
    ); // 34.1f, 257.87f, 35.2f, 258.304f, 36.3f, 258.4f
    path.cubic_to(
        (f32::from_bits(0x4215999a), f32::from_bits(0x43813f79)),
        (f32::from_bits(0x421a0000), f32::from_bits(0x43813910)),
        (f32::from_bits(0x421e6666), f32::from_bits(0x438123bb)),
    ); // 37.4f, 258.496f, 38.5f, 258.446f, 39.6f, 258.279f
    path.cubic_to(
        (f32::from_bits(0x4222cccd), f32::from_bits(0x43810e66)),
        (f32::from_bits(0x42273333), f32::from_bits(0x4380c5f4)),
        (f32::from_bits(0x422b999a), f32::from_bits(0x4380b333)),
    ); // 40.7f, 258.112f, 41.8f, 257.547f, 42.9f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x42300000), f32::from_bits(0x4380a071)),
        (f32::from_bits(0x42346666), f32::from_bits(0x4380b333)),
        (f32::from_bits(0x4238cccd), f32::from_bits(0x4380b333)),
    ); // 44, 257.253f, 45.1f, 257.4f, 46.2f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x423d3333), f32::from_bits(0x4380b333)),
        (f32::from_bits(0x4241999a), f32::from_bits(0x4380a482)),
        (f32::from_bits(0x42460000), f32::from_bits(0x4380b333)),
    ); // 47.3f, 257.4f, 48.4f, 257.285f, 49.5f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x424a6666), f32::from_bits(0x4380c1e4)),
        (f32::from_bits(0x424ecccd), f32::from_bits(0x4380f603)),
        (f32::from_bits(0x42533333), f32::from_bits(0x43810b58)),
    ); // 50.6f, 257.515f, 51.7f, 257.922f, 52.8f, 258.089f
    path.cubic_to(
        (f32::from_bits(0x4257999a), f32::from_bits(0x438120ae)),
        (f32::from_bits(0x425c0000), f32::from_bits(0x43812c8e)),
        (f32::from_bits(0x42606666), f32::from_bits(0x43813333)),
    ); // 53.9f, 258.255f, 55, 258.348f, 56.1f, 258.4f
    path.cubic_to(
        (f32::from_bits(0x4264cccd), f32::from_bits(0x438139d7)),
        (f32::from_bits(0x42693333), f32::from_bits(0x43813333)),
        (f32::from_bits(0x426d999a), f32::from_bits(0x43813333)),
    ); // 57.2f, 258.452f, 58.3f, 258.4f, 59.4f, 258.4f
    path.cubic_to(
        (f32::from_bits(0x42720000), f32::from_bits(0x43813333)),
        (f32::from_bits(0x42766666), f32::from_bits(0x43813a8b)),
        (f32::from_bits(0x427acccd), f32::from_bits(0x43813333)),
    ); // 60.5f, 258.4f, 61.6f, 258.457f, 62.7f, 258.4f
    path.cubic_to(
        (f32::from_bits(0x427f3333), f32::from_bits(0x43812bda)),
        (f32::from_bits(0x4281cccd), f32::from_bits(0x43811c75)),
        (f32::from_bits(0x42840000), f32::from_bits(0x4381071f)),
    ); // 63.8f, 258.343f, 64.9f, 258.222f, 66, 258.056f
    path.cubic_to(
        (f32::from_bits(0x42863333), f32::from_bits(0x4380f1ca)),
        (f32::from_bits(0x42886666), f32::from_bits(0x4380c130)),
        (f32::from_bits(0x428a999a), f32::from_bits(0x4380b333)),
    ); // 67.1f, 257.889f, 68.2f, 257.509f, 69.3f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x428ccccd), f32::from_bits(0x4380a536)),
        (f32::from_bits(0x428f0000), f32::from_bits(0x4380b333)),
        (f32::from_bits(0x42913333), f32::from_bits(0x4380b333)),
    ); // 70.4f, 257.291f, 71.5f, 257.4f, 72.6f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x42936666), f32::from_bits(0x4380b333)),
        (f32::from_bits(0x4295999a), f32::from_bits(0x4380ab4f)),
        (f32::from_bits(0x4297cccd), f32::from_bits(0x4380b333)),
    ); // 73.7f, 257.4f, 74.8f, 257.338f, 75.9f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x429a0000), f32::from_bits(0x4380bb16)),
        (f32::from_bits(0x429c3333), f32::from_bits(0x4380e287)),
        (f32::from_bits(0x429e6666), f32::from_bits(0x4380e287)),
    ); // 77, 257.462f, 78.1f, 257.77f, 79.2f, 257.77f
    path.cubic_to(
        (f32::from_bits(0x42a0999a), f32::from_bits(0x4380e287)),
        (f32::from_bits(0x42a2cccd), f32::from_bits(0x4380bb16)),
        (f32::from_bits(0x42a50000), f32::from_bits(0x4380b333)),
    ); // 80.3f, 257.77f, 81.4f, 257.462f, 82.5f, 257.4f
    path.cubic_to(
        (f32::from_bits(0x42a73333), f32::from_bits(0x4380ab4f)),
        (f32::from_bits(0x42a96666), f32::from_bits(0x4380b333)),
        (f32::from_bits(0x42ab999a), f32::from_bits(0x4380b333)),
    ); // 83.6f, 257.338f, 84.7f, 257.4f, 85.8f, 257.4f
    path.line_to((f32::from_bits(0x42ab999a), f32::from_bits(0x4380b333))); // 85.8f, 257.4f
    path.line_to((f32::from_bits(0x411e6666), f32::from_bits(0x4380b333))); // 9.9f, 257.4f
    path.close();
    test_path_op(reporter, &pathA, &path.detach(), PathOp::Union, filename);
}

// Port of: tests/PathOpsIssue3651.cpp#L1470-L1481 (chrome/m156)
fn issue3651_6(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.cubic_to(
        (f32::from_bits(0x41c1999a), f32::from_bits(0x4380423d)),
        (f32::from_bits(0x41ca6666), f32::from_bits(0x4380789f)),
        (f32::from_bits(0x41d33333), f32::from_bits(0x43808db9)),
    ); // 24.2f, 256.517f, 25.3f, 256.942f, 26.4f, 257.107f
    path.cubic_to(
        (f32::from_bits(0x41dc0000), f32::from_bits(0x4380a2d3)),
        (f32::from_bits(0x41e4cccd), f32::from_bits(0x4380b36f)),
        (f32::from_bits(0x41ed999a), f32::from_bits(0x4380b333)),
    ); // 27.5f, 257.272f, 28.6f, 257.402f, 29.7f, 257.4f
    path.line_to((f32::from_bits(0x411e6666), f32::from_bits(0x4380b333))); // 9.9f, 257.4f
    path.close();
    let pathA = path.detach();
    path.cubic_to(
        (f32::from_bits(0x41c1999a), f32::from_bits(0x43812429)),
        (f32::from_bits(0x41ca6666), f32::from_bits(0x4380edc7)),
        (f32::from_bits(0x41d33333), f32::from_bits(0x4380d8ad)),
    ); // 24.2f, 258.283f, 25.3f, 257.858f, 26.4f, 257.693f
    path.cubic_to(
        (f32::from_bits(0x41dc0000), f32::from_bits(0x4380c393)),
        (f32::from_bits(0x41e4cccd), f32::from_bits(0x4380b2f7)),
        (f32::from_bits(0x41ed999a), f32::from_bits(0x4380b333)),
    ); // 27.5f, 257.528f, 28.6f, 257.398f, 29.7f, 257.4f
    test_path_op(reporter, &pathA, &path.detach(), PathOp::Union, filename);
}

// Port of: tests/PathOpsIssue3651.cpp#L1483-L1666 (chrome/m156)
fn issue3651_7(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x42886666), f32::from_bits(0x4295999a))); // 68.2f, 74.8f
    path.line_to((f32::from_bits(0x42886666), f32::from_bits(0x4295999a))); // 68.2f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x4289ddde), f32::from_bits(0x4295999a)),
        (f32::from_bits(0x428b5555), f32::from_bits(0x4295ab91)),
        (f32::from_bits(0x428ccccd), f32::from_bits(0x4295999a)),
    ); // 68.9333f, 74.8f, 69.6667f, 74.8351f, 70.4f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x428e4444), f32::from_bits(0x429587a2)),
        (f32::from_bits(0x428fbbbc), f32::from_bits(0x42954339)),
        (f32::from_bits(0x42913333), f32::from_bits(0x42952dcf)),
    ); // 71.1333f, 74.7649f, 71.8667f, 74.6313f, 72.6f, 74.5895f
    path.cubic_to(
        (f32::from_bits(0x4292aaab), f32::from_bits(0x42951865)),
        (f32::from_bits(0x42942222), f32::from_bits(0x42951dec)),
        (f32::from_bits(0x4295999a), f32::from_bits(0x4295191f)),
    ); // 73.3333f, 74.5476f, 74.0667f, 74.5584f, 74.8f, 74.5491f
    path.cubic_to(
        (f32::from_bits(0x42971111), f32::from_bits(0x42951452)),
        (f32::from_bits(0x42988889), f32::from_bits(0x4294fb9a)),
        (f32::from_bits(0x429a0000), f32::from_bits(0x42951101)),
    ); // 75.5333f, 74.5397f, 76.2667f, 74.4914f, 77, 74.5332f
    path.cubic_to(
        (f32::from_bits(0x429b7777), f32::from_bits(0x42952667)),
        (f32::from_bits(0x429ceeef), f32::from_bits(0x429582c2)),
        (f32::from_bits(0x429e6666), f32::from_bits(0x42959986)),
    ); // 77.7333f, 74.575f, 78.4667f, 74.7554f, 79.2f, 74.7999f
    path.cubic_to(
        (f32::from_bits(0x429fddde), f32::from_bits(0x4295b04b)),
        (f32::from_bits(0x42a15555), f32::from_bits(0x42959996)),
        (f32::from_bits(0x42a2cccd), f32::from_bits(0x4295999a)),
    ); // 79.9333f, 74.8443f, 80.6667f, 74.8f, 81.4f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x42a44444), f32::from_bits(0x4295999d)),
        (f32::from_bits(0x42a5bbbc), f32::from_bits(0x42959e78)),
        (f32::from_bits(0x42a73333), f32::from_bits(0x4295999a)),
    ); // 82.1333f, 74.8f, 82.8667f, 74.8095f, 83.6f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x42a8aaab), f32::from_bits(0x429594bb)),
        (f32::from_bits(0x42aa2222), f32::from_bits(0x42957c62)),
        (f32::from_bits(0x42ab999a), f32::from_bits(0x42957c62)),
    ); // 84.3333f, 74.7905f, 85.0667f, 74.7429f, 85.8f, 74.7429f
    path.cubic_to(
        (f32::from_bits(0x42ad1111), f32::from_bits(0x42957c62)),
        (f32::from_bits(0x42ae8889), f32::from_bits(0x429594bb)),
        (f32::from_bits(0x42b00000), f32::from_bits(0x4295999a)),
    ); // 86.5333f, 74.7429f, 87.2667f, 74.7905f, 88, 74.8f
    path.cubic_to(
        (f32::from_bits(0x42b17777), f32::from_bits(0x42959e78)),
        (f32::from_bits(0x42b2eeef), f32::from_bits(0x4295ac2d)),
        (f32::from_bits(0x42b46666), f32::from_bits(0x4295999a)),
    ); // 88.7333f, 74.8095f, 89.4667f, 74.8363f, 90.2f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x42b5ddde), f32::from_bits(0x42958706)),
        (f32::from_bits(0x42b75555), f32::from_bits(0x42953b44)),
        (f32::from_bits(0x42b8cccd), f32::from_bits(0x42952a25)),
    ); // 90.9333f, 74.7637f, 91.6667f, 74.6158f, 92.4f, 74.5823f
    path.cubic_to(
        (f32::from_bits(0x42ba4444), f32::from_bits(0x42951906)),
        (f32::from_bits(0x42bbbbbc), f32::from_bits(0x42952e4b)),
        (f32::from_bits(0x42bd3333), f32::from_bits(0x429532e0)),
    ); // 93.1333f, 74.5489f, 93.8667f, 74.5904f, 94.6f, 74.5994f
    path.cubic_to(
        (f32::from_bits(0x42beaaab), f32::from_bits(0x42953775)),
        (f32::from_bits(0x42c02222), f32::from_bits(0x42954fda)),
        (f32::from_bits(0x42c1999a), f32::from_bits(0x429545a4)),
    ); // 95.3333f, 74.6083f, 96.0667f, 74.656f, 96.8f, 74.636f
    path.cubic_to(
        (f32::from_bits(0x42c31111), f32::from_bits(0x42953b6d)),
        (f32::from_bits(0x42c48889), f32::from_bits(0x42950c1f)),
        (f32::from_bits(0x42c60000), f32::from_bits(0x4294f599)),
    ); // 97.5333f, 74.6161f, 98.2667f, 74.5237f, 99, 74.4797f
    path.cubic_to(
        (f32::from_bits(0x42c77777), f32::from_bits(0x4294df14)),
        (f32::from_bits(0x42c8eeef), f32::from_bits(0x4294c8f4)),
        (f32::from_bits(0x42ca6666), f32::from_bits(0x4294be85)),
    ); // 99.7333f, 74.4357f, 100.467f, 74.3925f, 101.2f, 74.3721f
    path.cubic_to(
        (f32::from_bits(0x42cbddde), f32::from_bits(0x4294b417)),
        (f32::from_bits(0x42cd5555), f32::from_bits(0x4294ba72)),
        (f32::from_bits(0x42cecccd), f32::from_bits(0x4294b703)),
    ); // 101.933f, 74.3517f, 102.667f, 74.3642f, 103.4f, 74.3574f
    path.cubic_to(
        (f32::from_bits(0x42d04444), f32::from_bits(0x4294b395)),
        (f32::from_bits(0x42d1bbbc), f32::from_bits(0x4294b0ca)),
        (f32::from_bits(0x42d33333), f32::from_bits(0x4294a9ec)),
    ); // 104.133f, 74.3507f, 104.867f, 74.3453f, 105.6f, 74.3319f
    path.cubic_to(
        (f32::from_bits(0x42d4aaab), f32::from_bits(0x4294a30e)),
        (f32::from_bits(0x42d62222), f32::from_bits(0x429493f5)),
        (f32::from_bits(0x42d7999a), f32::from_bits(0x42948dd1)),
    ); // 106.333f, 74.3185f, 107.067f, 74.289f, 107.8f, 74.277f
    path.cubic_to(
        (f32::from_bits(0x42d91111), f32::from_bits(0x429487ae)),
        (f32::from_bits(0x42da8889), f32::from_bits(0x42947f24)),
        (f32::from_bits(0x42dc0000), f32::from_bits(0x42948517)),
    ); // 108.533f, 74.265f, 109.267f, 74.2483f, 110, 74.2599f
    path.cubic_to(
        (f32::from_bits(0x42dd7777), f32::from_bits(0x42948b0a)),
        (f32::from_bits(0x42deeeef), f32::from_bits(0x42949db5)),
        (f32::from_bits(0x42e06666), f32::from_bits(0x4294b185)),
    ); // 110.733f, 74.2716f, 111.467f, 74.308f, 112.2f, 74.3467f
    path.cubic_to(
        (f32::from_bits(0x42e1ddde), f32::from_bits(0x4294c556)),
        (f32::from_bits(0x42e35555), f32::from_bits(0x4294ed7d)),
        (f32::from_bits(0x42e4cccd), f32::from_bits(0x4294fbfa)),
    ); // 112.933f, 74.3854f, 113.667f, 74.4638f, 114.4f, 74.4921f
    path.cubic_to(
        (f32::from_bits(0x42e64444), f32::from_bits(0x42950a77)),
        (f32::from_bits(0x42e7bbbc), f32::from_bits(0x42950d94)),
        (f32::from_bits(0x42e93333), f32::from_bits(0x42950875)),
    ); // 115.133f, 74.5204f, 115.867f, 74.5265f, 116.6f, 74.5165f
    path.cubic_to(
        (f32::from_bits(0x42eaaaab), f32::from_bits(0x42950356)),
        (f32::from_bits(0x42ec2222), f32::from_bits(0x4294e77a)),
        (f32::from_bits(0x42ed999a), f32::from_bits(0x4294dd40)),
    ); // 117.333f, 74.5065f, 118.067f, 74.4521f, 118.8f, 74.4321f
    path.cubic_to(
        (f32::from_bits(0x42ef1111), f32::from_bits(0x4294d305)),
        (f32::from_bits(0x42f08889), f32::from_bits(0x4294c50a)),
        (f32::from_bits(0x42f20000), f32::from_bits(0x4294cb18)),
    ); // 119.533f, 74.4121f, 120.267f, 74.3848f, 121, 74.3967f
    path.cubic_to(
        (f32::from_bits(0x42f37777), f32::from_bits(0x4294d125)),
        (f32::from_bits(0x42f4eeef), f32::from_bits(0x4294ece7)),
        (f32::from_bits(0x42f66666), f32::from_bits(0x42950190)),
    ); // 121.733f, 74.4085f, 122.467f, 74.4627f, 123.2f, 74.5031f
    path.cubic_to(
        (f32::from_bits(0x42f7ddde), f32::from_bits(0x42951638)),
        (f32::from_bits(0x42f95555), f32::from_bits(0x42953e63)),
        (f32::from_bits(0x42facccd), f32::from_bits(0x4295470b)),
    ); // 123.933f, 74.5434f, 124.667f, 74.6218f, 125.4f, 74.6388f
    path.cubic_to(
        (f32::from_bits(0x42fc4444), f32::from_bits(0x42954fb3)),
        (f32::from_bits(0x42fdbbbc), f32::from_bits(0x4295478d)),
        (f32::from_bits(0x42ff3333), f32::from_bits(0x4295357f)),
    ); // 126.133f, 74.6557f, 126.867f, 74.6397f, 127.6f, 74.6045f
    path.cubic_to(
        (f32::from_bits(0x43005555), f32::from_bits(0x42952371)),
        (f32::from_bits(0x43011111), f32::from_bits(0x4294fc35)),
        (f32::from_bits(0x4301cccd), f32::from_bits(0x4294dab7)),
    ); // 128.333f, 74.5692f, 129.067f, 74.4926f, 129.8f, 74.4272f
    path.cubic_to(
        (f32::from_bits(0x43028889), f32::from_bits(0x4294b93a)),
        (f32::from_bits(0x43034444), f32::from_bits(0x429480ff)),
        (f32::from_bits(0x43040000), f32::from_bits(0x42946c8f)),
    ); // 130.533f, 74.3618f, 131.267f, 74.2519f, 132, 74.212f
    path.cubic_to(
        (f32::from_bits(0x4304bbbc), f32::from_bits(0x4294581e)),
        (f32::from_bits(0x43057777), f32::from_bits(0x42945d0d)),
        (f32::from_bits(0x43063333), f32::from_bits(0x42946014)),
    ); // 132.733f, 74.1721f, 133.467f, 74.1817f, 134.2f, 74.1877f
    path.cubic_to(
        (f32::from_bits(0x4306eeef), f32::from_bits(0x4294631a)),
        (f32::from_bits(0x4307aaab), f32::from_bits(0x42947340)),
        (f32::from_bits(0x43086666), f32::from_bits(0x42947eb7)),
    ); // 134.933f, 74.1936f, 135.667f, 74.2251f, 136.4f, 74.2475f
    path.cubic_to(
        (f32::from_bits(0x43092222), f32::from_bits(0x42948a2d)),
        (f32::from_bits(0x4309ddde), f32::from_bits(0x42949abf)),
        (f32::from_bits(0x430a999a), f32::from_bits(0x4294a4db)),
    ); // 137.133f, 74.2699f, 137.867f, 74.3022f, 138.6f, 74.322f
    path.cubic_to(
        (f32::from_bits(0x430b5555), f32::from_bits(0x4294aef8)),
        (f32::from_bits(0x430c1111), f32::from_bits(0x4294bfa8)),
        (f32::from_bits(0x430ccccd), f32::from_bits(0x4294bb61)),
    ); // 139.333f, 74.3417f, 140.067f, 74.3743f, 140.8f, 74.366f
    path.cubic_to(
        (f32::from_bits(0x430d8889), f32::from_bits(0x4294b71a)),
        (f32::from_bits(0x430e4444), f32::from_bits(0x429490b9)),
        (f32::from_bits(0x430f0000), f32::from_bits(0x42948b32)),
    ); // 141.533f, 74.3576f, 142.267f, 74.2827f, 143, 74.2719f
    path.cubic_to(
        (f32::from_bits(0x430fbbbc), f32::from_bits(0x429485ab)),
        (f32::from_bits(0x43107777), f32::from_bits(0x429483e4)),
        (f32::from_bits(0x43113333), f32::from_bits(0x42949a35)),
    ); // 143.733f, 74.2611f, 144.467f, 74.2576f, 145.2f, 74.3012f
    path.cubic_to(
        (f32::from_bits(0x4311eeef), f32::from_bits(0x4294b086)),
        (f32::from_bits(0x4312aaab), f32::from_bits(0x4294f894)),
        (f32::from_bits(0x43136666), f32::from_bits(0x42951118)),
    ); // 145.933f, 74.3448f, 146.667f, 74.4855f, 147.4f, 74.5334f
    path.cubic_to(
        (f32::from_bits(0x43142222), f32::from_bits(0x4295299b)),
        (f32::from_bits(0x4314ddde), f32::from_bits(0x4295342c)),
        (f32::from_bits(0x4315999a), f32::from_bits(0x42952d4a)),
    ); // 148.133f, 74.5813f, 148.867f, 74.6019f, 149.6f, 74.5885f
    path.cubic_to(
        (f32::from_bits(0x43165555), f32::from_bits(0x42952668)),
        (f32::from_bits(0x43171111), f32::from_bits(0x4294f257)),
        (f32::from_bits(0x4317cccd), f32::from_bits(0x4294e7cf)),
    ); // 150.333f, 74.575f, 151.067f, 74.4733f, 151.8f, 74.4528f
    path.cubic_to(
        (f32::from_bits(0x43188889), f32::from_bits(0x4294dd46)),
        (f32::from_bits(0x43194444), f32::from_bits(0x4294e9b3)),
        (f32::from_bits(0x431a0000), f32::from_bits(0x4294ee18)),
    ); // 152.533f, 74.4322f, 153.267f, 74.4564f, 154, 74.465f
    path.cubic_to(
        (f32::from_bits(0x431abbbc), f32::from_bits(0x4294f27d)),
        (f32::from_bits(0x431b7777), f32::from_bits(0x42950422)),
        (f32::from_bits(0x431c3333), f32::from_bits(0x4295022c)),
    ); // 154.733f, 74.4736f, 155.467f, 74.5081f, 156.2f, 74.5042f
    path.cubic_to(
        (f32::from_bits(0x431ceeef), f32::from_bits(0x42950035)),
        (f32::from_bits(0x431daaab), f32::from_bits(0x4294e324)),
        (f32::from_bits(0x431e6666), f32::from_bits(0x4294e250)),
    ); // 156.933f, 74.5004f, 157.667f, 74.4436f, 158.4f, 74.442f
    path.cubic_to(
        (f32::from_bits(0x431f2222), f32::from_bits(0x4294e17c)),
        (f32::from_bits(0x431fddde), f32::from_bits(0x4294e5a1)),
        (f32::from_bits(0x4320999a), f32::from_bits(0x4294fd32)),
    ); // 159.133f, 74.4404f, 159.867f, 74.4485f, 160.6f, 74.4945f
    path.cubic_to(
        (f32::from_bits(0x43215555), f32::from_bits(0x429514c4)),
        (f32::from_bits(0x43221111), f32::from_bits(0x42955c02)),
        (f32::from_bits(0x4322cccd), f32::from_bits(0x42956fb8)),
    ); // 161.333f, 74.5406f, 162.067f, 74.6797f, 162.8f, 74.7182f
    path.cubic_to(
        (f32::from_bits(0x43238889), f32::from_bits(0x4295836e)),
        (f32::from_bits(0x43244444), f32::from_bits(0x429586fb)),
        (f32::from_bits(0x43250000), f32::from_bits(0x42957379)),
    ); // 163.533f, 74.7567f, 164.267f, 74.7636f, 165, 74.7255f
    path.cubic_to(
        (f32::from_bits(0x4325bbbc), f32::from_bits(0x42955ff7)),
        (f32::from_bits(0x43267777), f32::from_bits(0x42950a52)),
        (f32::from_bits(0x43273333), f32::from_bits(0x4294faaa)),
    ); // 165.733f, 74.6874f, 166.467f, 74.5202f, 167.2f, 74.4896f
    path.cubic_to(
        (f32::from_bits(0x4327eeef), f32::from_bits(0x4294eb03)),
        (f32::from_bits(0x4328aaab), f32::from_bits(0x4294fb0f)),
        (f32::from_bits(0x43296666), f32::from_bits(0x4295158c)),
    ); // 167.933f, 74.459f, 168.667f, 74.4903f, 169.4f, 74.5421f
    path.cubic_to(
        (f32::from_bits(0x432a2222), f32::from_bits(0x4295300a)),
        (f32::from_bits(0x432addde), f32::from_bits(0x42958397)),
        (f32::from_bits(0x432b999a), f32::from_bits(0x4295999a)),
    ); // 170.133f, 74.5938f, 170.867f, 74.757f, 171.6f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x432c5555), f32::from_bits(0x4295af9c)),
        (f32::from_bits(0x432d1111), f32::from_bits(0x4295ace7)),
        (f32::from_bits(0x432dcccd), f32::from_bits(0x4295999a)),
    ); // 172.333f, 74.843f, 173.067f, 74.8377f, 173.8f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x432e8889), f32::from_bits(0x4295864c)),
        (f32::from_bits(0x432f4444), f32::from_bits(0x4295555a)),
        (f32::from_bits(0x43300000), f32::from_bits(0x429525c8)),
    ); // 174.533f, 74.7623f, 175.267f, 74.6667f, 176, 74.5738f
    path.cubic_to(
        (f32::from_bits(0x4330bbbc), f32::from_bits(0x4294f636)),
        (f32::from_bits(0x43317777), f32::from_bits(0x42949a53)),
        (f32::from_bits(0x43323333), f32::from_bits(0x42947c2e)),
    ); // 176.733f, 74.4809f, 177.467f, 74.3014f, 178.2f, 74.2425f
    path.cubic_to(
        (f32::from_bits(0x4332eeef), f32::from_bits(0x42945e0a)),
        (f32::from_bits(0x4333aaab), f32::from_bits(0x42946377)),
        (f32::from_bits(0x43346666), f32::from_bits(0x429470ec)),
    ); // 178.933f, 74.1837f, 179.667f, 74.1943f, 180.4f, 74.2206f
    path.cubic_to(
        (f32::from_bits(0x43352222), f32::from_bits(0x42947e61)),
        (f32::from_bits(0x4335ddde), f32::from_bits(0x4294b4b7)),
        (f32::from_bits(0x4336999a), f32::from_bits(0x4294ccec)),
    ); // 181.133f, 74.2468f, 181.867f, 74.353f, 182.6f, 74.4002f
    path.cubic_to(
        (f32::from_bits(0x43375555), f32::from_bits(0x4294e522)),
        (f32::from_bits(0x43381111), f32::from_bits(0x4294f1c6)),
        (f32::from_bits(0x4338cccd), f32::from_bits(0x4295022c)),
    ); // 183.333f, 74.4475f, 184.067f, 74.4722f, 184.8f, 74.5042f
    path.cubic_to(
        (f32::from_bits(0x43398889), f32::from_bits(0x42951291)),
        (f32::from_bits(0x433a4444), f32::from_bits(0x42952241)),
        (f32::from_bits(0x433b0000), f32::from_bits(0x42952f4d)),
    ); // 185.533f, 74.5363f, 186.267f, 74.5669f, 187, 74.5924f
    path.cubic_to(
        (f32::from_bits(0x433bbbbc), f32::from_bits(0x42953c5a)),
        (f32::from_bits(0x433c7777), f32::from_bits(0x42955a4b)),
        (f32::from_bits(0x433d3333), f32::from_bits(0x42955079)),
    ); // 187.733f, 74.6179f, 188.467f, 74.6764f, 189.2f, 74.6572f
    path.cubic_to(
        (f32::from_bits(0x433deeef), f32::from_bits(0x429546a7)),
        (f32::from_bits(0x433eaaab), f32::from_bits(0x4294fc01)),
        (f32::from_bits(0x433f6666), f32::from_bits(0x4294f461)),
    ); // 189.933f, 74.638f, 190.667f, 74.4922f, 191.4f, 74.4773f
    path.cubic_to(
        (f32::from_bits(0x43402222), f32::from_bits(0x4294ecc1)),
        (f32::from_bits(0x4340ddde), f32::from_bits(0x42950f1f)),
        (f32::from_bits(0x4341999a), f32::from_bits(0x429522bb)),
    ); // 192.133f, 74.4624f, 192.867f, 74.5295f, 193.6f, 74.5678f
    path.cubic_to(
        (f32::from_bits(0x43425555), f32::from_bits(0x42953657)),
        (f32::from_bits(0x43431111), f32::from_bits(0x4295616b)),
        (f32::from_bits(0x4343cccd), f32::from_bits(0x42956a0b)),
    ); // 194.333f, 74.6061f, 195.067f, 74.6903f, 195.8f, 74.7071f
    path.cubic_to(
        (f32::from_bits(0x43448889), f32::from_bits(0x429572ab)),
        (f32::from_bits(0x43454444), f32::from_bits(0x4295608a)),
        (f32::from_bits(0x43460000), f32::from_bits(0x4295567c)),
    ); // 196.533f, 74.724f, 197.267f, 74.6886f, 198, 74.6689f
    path.cubic_to(
        (f32::from_bits(0x4346bbbc), f32::from_bits(0x42954c6e)),
        (f32::from_bits(0x43477777), f32::from_bits(0x42953beb)),
        (f32::from_bits(0x43483333), f32::from_bits(0x42952db8)),
    ); // 198.733f, 74.6493f, 199.467f, 74.617f, 200.2f, 74.5893f
    path.cubic_to(
        (f32::from_bits(0x4348eeef), f32::from_bits(0x42951f85)),
        (f32::from_bits(0x4349aaab), f32::from_bits(0x42950a92)),
        (f32::from_bits(0x434a6666), f32::from_bits(0x4295014a)),
    ); // 200.933f, 74.5616f, 201.667f, 74.5206f, 202.4f, 74.5025f
    path.cubic_to(
        (f32::from_bits(0x434b2222), f32::from_bits(0x4294f802)),
        (f32::from_bits(0x434bddde), f32::from_bits(0x4294f9c8)),
        (f32::from_bits(0x434c999a), f32::from_bits(0x4294f607)),
    ); // 203.133f, 74.4844f, 203.867f, 74.4879f, 204.6f, 74.4805f
    path.cubic_to(
        (f32::from_bits(0x434d5555), f32::from_bits(0x4294f246)),
        (f32::from_bits(0x434e1111), f32::from_bits(0x4294f080)),
        (f32::from_bits(0x434ecccd), f32::from_bits(0x4294eac5)),
    ); // 205.333f, 74.4732f, 206.067f, 74.4697f, 206.8f, 74.4585f
    path.cubic_to(
        (f32::from_bits(0x434f8889), f32::from_bits(0x4294e509)),
        (f32::from_bits(0x43504444), f32::from_bits(0x4294dd6d)),
        (f32::from_bits(0x43510000), f32::from_bits(0x4294d3a3)),
    ); // 207.533f, 74.4473f, 208.267f, 74.4325f, 209, 74.4134f
    path.cubic_to(
        (f32::from_bits(0x4351bbbc), f32::from_bits(0x4294c9d9)),
        (f32::from_bits(0x43527777), f32::from_bits(0x4294b931)),
        (f32::from_bits(0x43533333), f32::from_bits(0x4294b007)),
    ); // 209.733f, 74.3942f, 210.467f, 74.3617f, 211.2f, 74.3438f
    path.cubic_to(
        (f32::from_bits(0x4353eeef), f32::from_bits(0x4294a6dd)),
        (f32::from_bits(0x4354aaab), f32::from_bits(0x4294a02f)),
        (f32::from_bits(0x43556666), f32::from_bits(0x42949ca6)),
    ); // 211.933f, 74.3259f, 212.667f, 74.3129f, 213.4f, 74.306f
    path.cubic_to(
        (f32::from_bits(0x43562222), f32::from_bits(0x4294991d)),
        (f32::from_bits(0x4356ddde), f32::from_bits(0x42949775)),
        (f32::from_bits(0x4357999a), f32::from_bits(0x42949ad1)),
    ); // 214.133f, 74.299f, 214.867f, 74.2958f, 215.6f, 74.3024f
    path.cubic_to(
        (f32::from_bits(0x43585555), f32::from_bits(0x42949e2e)),
        (f32::from_bits(0x43591111), f32::from_bits(0x4294a9b4)),
        (f32::from_bits(0x4359cccd), f32::from_bits(0x4294b0d2)),
    ); // 216.333f, 74.3089f, 217.067f, 74.3315f, 217.8f, 74.3454f
    path.cubic_to(
        (f32::from_bits(0x435a8889), f32::from_bits(0x4294b7ef)),
        (f32::from_bits(0x435b4444), f32::from_bits(0x4294d2c3)),
        (f32::from_bits(0x435c0000), f32::from_bits(0x4294c582)),
    ); // 218.533f, 74.3592f, 219.267f, 74.4116f, 220, 74.3858f
    path.cubic_to(
        (f32::from_bits(0x435cbbbc), f32::from_bits(0x4294b841)),
        (f32::from_bits(0x435d7777), f32::from_bits(0x42947919)),
        (f32::from_bits(0x435e3333), f32::from_bits(0x4294614c)),
    ); // 220.733f, 74.3599f, 221.467f, 74.2365f, 222.2f, 74.19f
    path.cubic_to(
        (f32::from_bits(0x435eeeef), f32::from_bits(0x4294497f)),
        (f32::from_bits(0x435faaab), f32::from_bits(0x42943521)),
        (f32::from_bits(0x43606666), f32::from_bits(0x429436b3)),
    ); // 222.933f, 74.1435f, 223.667f, 74.1038f, 224.4f, 74.1068f
    path.cubic_to(
        (f32::from_bits(0x43612222), f32::from_bits(0x42943845)),
        (f32::from_bits(0x4361ddde), f32::from_bits(0x42945996)),
        (f32::from_bits(0x4362999a), f32::from_bits(0x42946aba)),
    ); // 225.133f, 74.1099f, 225.867f, 74.175f, 226.6f, 74.2085f
    path.cubic_to(
        (f32::from_bits(0x43635555), f32::from_bits(0x42947bdd)),
        (f32::from_bits(0x43641111), f32::from_bits(0x4294973f)),
        (f32::from_bits(0x4364cccd), f32::from_bits(0x42949d88)),
    ); // 227.333f, 74.2419f, 228.067f, 74.2954f, 228.8f, 74.3077f
    path.cubic_to(
        (f32::from_bits(0x43658889), f32::from_bits(0x4294a3d2)),
        (f32::from_bits(0x43664444), f32::from_bits(0x429495a6)),
        (f32::from_bits(0x43670000), f32::from_bits(0x42949071)),
    ); // 229.533f, 74.32f, 230.267f, 74.2923f, 231, 74.2821f
    path.cubic_to(
        (f32::from_bits(0x4367bbbc), f32::from_bits(0x42948b3c)),
        (f32::from_bits(0x43687777), f32::from_bits(0x429480b0)),
        (f32::from_bits(0x43693333), f32::from_bits(0x42947e49)),
    ); // 231.733f, 74.2719f, 232.467f, 74.2513f, 233.2f, 74.2467f
    path.cubic_to(
        (f32::from_bits(0x4369eeef), f32::from_bits(0x42947be3)),
        (f32::from_bits(0x436aaaab), f32::from_bits(0x42947a88)),
        (f32::from_bits(0x436b6666), f32::from_bits(0x4294820a)),
    ); // 233.933f, 74.242f, 234.667f, 74.2393f, 235.4f, 74.254f
    path.cubic_to(
        (f32::from_bits(0x436c2222), f32::from_bits(0x4294898c)),
        (f32::from_bits(0x436cddde), f32::from_bits(0x42949eee)),
        (f32::from_bits(0x436d999a), f32::from_bits(0x4294ab53)),
    ); // 236.133f, 74.2686f, 236.867f, 74.3104f, 237.6f, 74.3346f
    path.cubic_to(
        (f32::from_bits(0x436e5555), f32::from_bits(0x4294b7b8)),
        (f32::from_bits(0x436f1111), f32::from_bits(0x4294c2bb)),
        (f32::from_bits(0x436fcccd), f32::from_bits(0x4294cc67)),
    ); // 238.333f, 74.3588f, 239.067f, 74.3803f, 239.8f, 74.3992f
    path.cubic_to(
        (f32::from_bits(0x43708889), f32::from_bits(0x4294d614)),
        (f32::from_bits(0x43714444), f32::from_bits(0x4294d5a0)),
        (f32::from_bits(0x43720000), f32::from_bits(0x4294e55e)),
    ); // 240.533f, 74.4181f, 241.267f, 74.4172f, 242, 74.448f
    path.cubic_to(
        (f32::from_bits(0x4372bbbc), f32::from_bits(0x4294f51b)),
        (f32::from_bits(0x43737777), f32::from_bits(0x429516d0)),
        (f32::from_bits(0x43743333), f32::from_bits(0x42952ad9)),
    ); // 242.733f, 74.4787f, 243.467f, 74.5446f, 244.2f, 74.5837f
    path.cubic_to(
        (f32::from_bits(0x4374eeef), f32::from_bits(0x42953ee1)),
        (f32::from_bits(0x4375aaab), f32::from_bits(0x42954f2d)),
        (f32::from_bits(0x43766666), f32::from_bits(0x42955d90)),
    ); // 244.933f, 74.6228f, 245.667f, 74.6546f, 246.4f, 74.6827f
    path.cubic_to(
        (f32::from_bits(0x43772222), f32::from_bits(0x42956bf3)),
        (f32::from_bits(0x4377ddde), f32::from_bits(0x4295772b)),
        (f32::from_bits(0x4378999a), f32::from_bits(0x4295812d)),
    ); // 247.133f, 74.7108f, 247.867f, 74.7327f, 248.6f, 74.7523f
    path.cubic_to(
        (f32::from_bits(0x43795555), f32::from_bits(0x42958b2e)),
        (f32::from_bits(0x437a1111), f32::from_bits(0x42959587)),
        (f32::from_bits(0x437acccd), f32::from_bits(0x4295999a)),
    ); // 249.333f, 74.7718f, 250.067f, 74.792f, 250.8f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x437b8889), f32::from_bits(0x42959dac)),
        (f32::from_bits(0x437c4444), f32::from_bits(0x4295999a)),
        (f32::from_bits(0x437d0000), f32::from_bits(0x4295999a)),
    ); // 251.533f, 74.808f, 252.267f, 74.8f, 253, 74.8f
    path.line_to((f32::from_bits(0x437d0000), f32::from_bits(0x4295999a))); // 253, 74.8f
    path.line_to((f32::from_bits(0x42886666), f32::from_bits(0x4295999a))); // 68.2f, 74.8f
    path.close();
    let pathA = path.detach();
    path.move_to((f32::from_bits(0x42886666), f32::from_bits(0x4295999a))); // 68.2f, 74.8f
    path.line_to((f32::from_bits(0x42886666), f32::from_bits(0x4295999a))); // 68.2f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x4289ddde), f32::from_bits(0x4295999a)),
        (f32::from_bits(0x428b5555), f32::from_bits(0x429587a3)),
        (f32::from_bits(0x428ccccd), f32::from_bits(0x4295999a)),
    ); // 68.9333f, 74.8f, 69.6667f, 74.7649f, 70.4f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x428e4444), f32::from_bits(0x4295ab92)),
        (f32::from_bits(0x428fbbbc), f32::from_bits(0x4295effb)),
        (f32::from_bits(0x42913333), f32::from_bits(0x42960565)),
    ); // 71.1333f, 74.8351f, 71.8667f, 74.9687f, 72.6f, 75.0105f
    path.cubic_to(
        (f32::from_bits(0x4292aaab), f32::from_bits(0x42961acf)),
        (f32::from_bits(0x42942222), f32::from_bits(0x42961548)),
        (f32::from_bits(0x4295999a), f32::from_bits(0x42961a15)),
    ); // 73.3333f, 75.0524f, 74.0667f, 75.0416f, 74.8f, 75.0509f
    path.cubic_to(
        (f32::from_bits(0x42971111), f32::from_bits(0x42961ee2)),
        (f32::from_bits(0x42988889), f32::from_bits(0x4296379a)),
        (f32::from_bits(0x429a0000), f32::from_bits(0x42962233)),
    ); // 75.5333f, 75.0603f, 76.2667f, 75.1086f, 77, 75.0668f
    path.cubic_to(
        (f32::from_bits(0x429b7777), f32::from_bits(0x42960ccd)),
        (f32::from_bits(0x429ceeef), f32::from_bits(0x4295b072)),
        (f32::from_bits(0x429e6666), f32::from_bits(0x429599ae)),
    ); // 77.7333f, 75.025f, 78.4667f, 74.8446f, 79.2f, 74.8002f
    path.cubic_to(
        (f32::from_bits(0x429fddde), f32::from_bits(0x429582e9)),
        (f32::from_bits(0x42a15555), f32::from_bits(0x4295999e)),
        (f32::from_bits(0x42a2cccd), f32::from_bits(0x4295999a)),
    ); // 79.9333f, 74.7557f, 80.6667f, 74.8f, 81.4f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x42a44444), f32::from_bits(0x42959997)),
        (f32::from_bits(0x42a5bbbc), f32::from_bits(0x429594bc)),
        (f32::from_bits(0x42a73333), f32::from_bits(0x4295999a)),
    ); // 82.1333f, 74.8f, 82.8667f, 74.7905f, 83.6f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x42a8aaab), f32::from_bits(0x42959e79)),
        (f32::from_bits(0x42aa2222), f32::from_bits(0x4295b6d2)),
        (f32::from_bits(0x42ab999a), f32::from_bits(0x4295b6d2)),
    ); // 84.3333f, 74.8095f, 85.0667f, 74.8571f, 85.8f, 74.8571f
    path.cubic_to(
        (f32::from_bits(0x42ad1111), f32::from_bits(0x4295b6d2)),
        (f32::from_bits(0x42ae8889), f32::from_bits(0x42959e79)),
        (f32::from_bits(0x42b00000), f32::from_bits(0x4295999a)),
    ); // 86.5333f, 74.8571f, 87.2667f, 74.8095f, 88, 74.8f
    path.cubic_to(
        (f32::from_bits(0x42b17777), f32::from_bits(0x429594bc)),
        (f32::from_bits(0x42b2eeef), f32::from_bits(0x42958707)),
        (f32::from_bits(0x42b46666), f32::from_bits(0x4295999a)),
    ); // 88.7333f, 74.7905f, 89.4667f, 74.7637f, 90.2f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x42b5ddde), f32::from_bits(0x4295ac2e)),
        (f32::from_bits(0x42b75555), f32::from_bits(0x4295f7f0)),
        (f32::from_bits(0x42b8cccd), f32::from_bits(0x4296090f)),
    ); // 90.9333f, 74.8363f, 91.6667f, 74.9843f, 92.4f, 75.0177f
    path.cubic_to(
        (f32::from_bits(0x42ba4444), f32::from_bits(0x42961a2e)),
        (f32::from_bits(0x42bbbbbc), f32::from_bits(0x429604e9)),
        (f32::from_bits(0x42bd3333), f32::from_bits(0x42960054)),
    ); // 93.1333f, 75.0511f, 93.8667f, 75.0096f, 94.6f, 75.0006f
    path.cubic_to(
        (f32::from_bits(0x42beaaab), f32::from_bits(0x4295fbbf)),
        (f32::from_bits(0x42c02222), f32::from_bits(0x4295e35a)),
        (f32::from_bits(0x42c1999a), f32::from_bits(0x4295ed90)),
    ); // 95.3333f, 74.9917f, 96.0667f, 74.944f, 96.8f, 74.964f
    path.cubic_to(
        (f32::from_bits(0x42c31111), f32::from_bits(0x4295f7c7)),
        (f32::from_bits(0x42c48889), f32::from_bits(0x42962715)),
        (f32::from_bits(0x42c60000), f32::from_bits(0x42963d9b)),
    ); // 97.5333f, 74.9839f, 98.2667f, 75.0763f, 99, 75.1203f
    path.cubic_to(
        (f32::from_bits(0x42c77777), f32::from_bits(0x42965420)),
        (f32::from_bits(0x42c8eeef), f32::from_bits(0x42966a40)),
        (f32::from_bits(0x42ca6666), f32::from_bits(0x429674af)),
    ); // 99.7333f, 75.1643f, 100.467f, 75.2075f, 101.2f, 75.2279f
    path.cubic_to(
        (f32::from_bits(0x42cbddde), f32::from_bits(0x42967f1d)),
        (f32::from_bits(0x42cd5555), f32::from_bits(0x429678c2)),
        (f32::from_bits(0x42cecccd), f32::from_bits(0x42967c31)),
    ); // 101.933f, 75.2483f, 102.667f, 75.2359f, 103.4f, 75.2426f
    path.cubic_to(
        (f32::from_bits(0x42d04444), f32::from_bits(0x42967f9f)),
        (f32::from_bits(0x42d1bbbc), f32::from_bits(0x4296826a)),
        (f32::from_bits(0x42d33333), f32::from_bits(0x42968948)),
    ); // 104.133f, 75.2493f, 104.867f, 75.2547f, 105.6f, 75.2681f
    path.cubic_to(
        (f32::from_bits(0x42d4aaab), f32::from_bits(0x42969026)),
        (f32::from_bits(0x42d62222), f32::from_bits(0x42969f3f)),
        (f32::from_bits(0x42d7999a), f32::from_bits(0x4296a563)),
    ); // 106.333f, 75.2815f, 107.067f, 75.311f, 107.8f, 75.323f
    path.cubic_to(
        (f32::from_bits(0x42d91111), f32::from_bits(0x4296ab86)),
        (f32::from_bits(0x42da8889), f32::from_bits(0x4296b410)),
        (f32::from_bits(0x42dc0000), f32::from_bits(0x4296ae1d)),
    ); // 108.533f, 75.335f, 109.267f, 75.3517f, 110, 75.3401f
    path.cubic_to(
        (f32::from_bits(0x42dd7777), f32::from_bits(0x4296a82a)),
        (f32::from_bits(0x42deeeef), f32::from_bits(0x4296957f)),
        (f32::from_bits(0x42e06666), f32::from_bits(0x429681af)),
    ); // 110.733f, 75.3284f, 111.467f, 75.292f, 112.2f, 75.2533f
    path.cubic_to(
        (f32::from_bits(0x42e1ddde), f32::from_bits(0x42966dde)),
        (f32::from_bits(0x42e35555), f32::from_bits(0x429645b7)),
        (f32::from_bits(0x42e4cccd), f32::from_bits(0x4296373a)),
    ); // 112.933f, 75.2146f, 113.667f, 75.1362f, 114.4f, 75.1079f
    path.cubic_to(
        (f32::from_bits(0x42e64444), f32::from_bits(0x429628bd)),
        (f32::from_bits(0x42e7bbbc), f32::from_bits(0x429625a0)),
        (f32::from_bits(0x42e93333), f32::from_bits(0x42962abf)),
    ); // 115.133f, 75.0796f, 115.867f, 75.0735f, 116.6f, 75.0835f
    path.cubic_to(
        (f32::from_bits(0x42eaaaab), f32::from_bits(0x42962fde)),
        (f32::from_bits(0x42ec2222), f32::from_bits(0x42964bba)),
        (f32::from_bits(0x42ed999a), f32::from_bits(0x429655f4)),
    ); // 117.333f, 75.0935f, 118.067f, 75.1479f, 118.8f, 75.1679f
    path.cubic_to(
        (f32::from_bits(0x42ef1111), f32::from_bits(0x4296602f)),
        (f32::from_bits(0x42f08889), f32::from_bits(0x42966e2a)),
        (f32::from_bits(0x42f20000), f32::from_bits(0x4296681c)),
    ); // 119.533f, 75.1879f, 120.267f, 75.2152f, 121, 75.2033f
    path.cubic_to(
        (f32::from_bits(0x42f37777), f32::from_bits(0x4296620f)),
        (f32::from_bits(0x42f4eeef), f32::from_bits(0x4296464d)),
        (f32::from_bits(0x42f66666), f32::from_bits(0x429631a4)),
    ); // 121.733f, 75.1915f, 122.467f, 75.1373f, 123.2f, 75.097f
    path.cubic_to(
        (f32::from_bits(0x42f7ddde), f32::from_bits(0x42961cfc)),
        (f32::from_bits(0x42f95555), f32::from_bits(0x4295f4d1)),
        (f32::from_bits(0x42facccd), f32::from_bits(0x4295ec29)),
    ); // 123.933f, 75.0566f, 124.667f, 74.9782f, 125.4f, 74.9613f
    path.cubic_to(
        (f32::from_bits(0x42fc4444), f32::from_bits(0x4295e381)),
        (f32::from_bits(0x42fdbbbc), f32::from_bits(0x4295eba7)),
        (f32::from_bits(0x42ff3333), f32::from_bits(0x4295fdb5)),
    ); // 126.133f, 74.9443f, 126.867f, 74.9603f, 127.6f, 74.9955f
    path.cubic_to(
        (f32::from_bits(0x43005555), f32::from_bits(0x42960fc3)),
        (f32::from_bits(0x43011111), f32::from_bits(0x429636ff)),
        (f32::from_bits(0x4301cccd), f32::from_bits(0x4296587d)),
    ); // 128.333f, 75.0308f, 129.067f, 75.1074f, 129.8f, 75.1728f
    path.cubic_to(
        (f32::from_bits(0x43028889), f32::from_bits(0x429679fa)),
        (f32::from_bits(0x43034444), f32::from_bits(0x4296b235)),
        (f32::from_bits(0x43040000), f32::from_bits(0x4296c6a5)),
    ); // 130.533f, 75.2382f, 131.267f, 75.3481f, 132, 75.388f
    path.cubic_to(
        (f32::from_bits(0x4304bbbc), f32::from_bits(0x4296db16)),
        (f32::from_bits(0x43057777), f32::from_bits(0x4296d627)),
        (f32::from_bits(0x43063333), f32::from_bits(0x4296d320)),
    ); // 132.733f, 75.4279f, 133.467f, 75.4183f, 134.2f, 75.4124f
    path.cubic_to(
        (f32::from_bits(0x4306eeef), f32::from_bits(0x4296d01a)),
        (f32::from_bits(0x4307aaab), f32::from_bits(0x4296bff4)),
        (f32::from_bits(0x43086666), f32::from_bits(0x4296b47d)),
    ); // 134.933f, 75.4064f, 135.667f, 75.3749f, 136.4f, 75.3525f
    path.cubic_to(
        (f32::from_bits(0x43092222), f32::from_bits(0x4296a907)),
        (f32::from_bits(0x4309ddde), f32::from_bits(0x42969875)),
        (f32::from_bits(0x430a999a), f32::from_bits(0x42968e59)),
    ); // 137.133f, 75.3301f, 137.867f, 75.2978f, 138.6f, 75.278f
    path.cubic_to(
        (f32::from_bits(0x430b5555), f32::from_bits(0x4296843c)),
        (f32::from_bits(0x430c1111), f32::from_bits(0x4296738c)),
        (f32::from_bits(0x430ccccd), f32::from_bits(0x429677d3)),
    ); // 139.333f, 75.2583f, 140.067f, 75.2257f, 140.8f, 75.234f
    path.cubic_to(
        (f32::from_bits(0x430d8889), f32::from_bits(0x42967c1a)),
        (f32::from_bits(0x430e4444), f32::from_bits(0x4296a27b)),
        (f32::from_bits(0x430f0000), f32::from_bits(0x4296a802)),
    ); // 141.533f, 75.2424f, 142.267f, 75.3173f, 143, 75.3281f
    path.cubic_to(
        (f32::from_bits(0x430fbbbc), f32::from_bits(0x4296ad89)),
        (f32::from_bits(0x43107777), f32::from_bits(0x4296af50)),
        (f32::from_bits(0x43113333), f32::from_bits(0x429698ff)),
    ); // 143.733f, 75.3389f, 144.467f, 75.3424f, 145.2f, 75.2988f
    path.cubic_to(
        (f32::from_bits(0x4311eeef), f32::from_bits(0x429682ae)),
        (f32::from_bits(0x4312aaab), f32::from_bits(0x42963aa0)),
        (f32::from_bits(0x43136666), f32::from_bits(0x4296221c)),
    ); // 145.933f, 75.2552f, 146.667f, 75.1145f, 147.4f, 75.0666f
    path.cubic_to(
        (f32::from_bits(0x43142222), f32::from_bits(0x42960999)),
        (f32::from_bits(0x4314ddde), f32::from_bits(0x4295ff08)),
        (f32::from_bits(0x4315999a), f32::from_bits(0x429605ea)),
    ); // 148.133f, 75.0187f, 148.867f, 74.9981f, 149.6f, 75.0116f
    path.cubic_to(
        (f32::from_bits(0x43165555), f32::from_bits(0x42960ccc)),
        (f32::from_bits(0x43171111), f32::from_bits(0x429640dd)),
        (f32::from_bits(0x4317cccd), f32::from_bits(0x42964b65)),
    ); // 150.333f, 75.025f, 151.067f, 75.1267f, 151.8f, 75.1473f
    path.cubic_to(
        (f32::from_bits(0x43188889), f32::from_bits(0x429655ee)),
        (f32::from_bits(0x43194444), f32::from_bits(0x42964981)),
        (f32::from_bits(0x431a0000), f32::from_bits(0x4296451c)),
    ); // 152.533f, 75.1678f, 153.267f, 75.1436f, 154, 75.135f
    path.cubic_to(
        (f32::from_bits(0x431abbbc), f32::from_bits(0x429640b7)),
        (f32::from_bits(0x431b7777), f32::from_bits(0x42962f12)),
        (f32::from_bits(0x431c3333), f32::from_bits(0x42963108)),
    ); // 154.733f, 75.1264f, 155.467f, 75.0919f, 156.2f, 75.0958f
    path.cubic_to(
        (f32::from_bits(0x431ceeef), f32::from_bits(0x429632ff)),
        (f32::from_bits(0x431daaab), f32::from_bits(0x42965010)),
        (f32::from_bits(0x431e6666), f32::from_bits(0x429650e4)),
    ); // 156.933f, 75.0996f, 157.667f, 75.1564f, 158.4f, 75.158f
    path.cubic_to(
        (f32::from_bits(0x431f2222), f32::from_bits(0x429651b8)),
        (f32::from_bits(0x431fddde), f32::from_bits(0x42964d93)),
        (f32::from_bits(0x4320999a), f32::from_bits(0x42963602)),
    ); // 159.133f, 75.1596f, 159.867f, 75.1515f, 160.6f, 75.1055f
    path.cubic_to(
        (f32::from_bits(0x43215555), f32::from_bits(0x42961e70)),
        (f32::from_bits(0x43221111), f32::from_bits(0x4295d732)),
        (f32::from_bits(0x4322cccd), f32::from_bits(0x4295c37c)),
    ); // 161.333f, 75.0594f, 162.067f, 74.9203f, 162.8f, 74.8818f
    path.cubic_to(
        (f32::from_bits(0x43238889), f32::from_bits(0x4295afc6)),
        (f32::from_bits(0x43244444), f32::from_bits(0x4295ac39)),
        (f32::from_bits(0x43250000), f32::from_bits(0x4295bfbb)),
    ); // 163.533f, 74.8433f, 164.267f, 74.8364f, 165, 74.8745f
    path.cubic_to(
        (f32::from_bits(0x4325bbbc), f32::from_bits(0x4295d33d)),
        (f32::from_bits(0x43267777), f32::from_bits(0x429628e2)),
        (f32::from_bits(0x43273333), f32::from_bits(0x4296388a)),
    ); // 165.733f, 74.9126f, 166.467f, 75.0798f, 167.2f, 75.1104f
    path.cubic_to(
        (f32::from_bits(0x4327eeef), f32::from_bits(0x42964831)),
        (f32::from_bits(0x4328aaab), f32::from_bits(0x42963825)),
        (f32::from_bits(0x43296666), f32::from_bits(0x42961da8)),
    ); // 167.933f, 75.141f, 168.667f, 75.1097f, 169.4f, 75.0579f
    path.cubic_to(
        (f32::from_bits(0x432a2222), f32::from_bits(0x4296032a)),
        (f32::from_bits(0x432addde), f32::from_bits(0x4295af9d)),
        (f32::from_bits(0x432b999a), f32::from_bits(0x4295999a)),
    ); // 170.133f, 75.0062f, 170.867f, 74.843f, 171.6f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x432c5555), f32::from_bits(0x42958398)),
        (f32::from_bits(0x432d1111), f32::from_bits(0x4295864d)),
        (f32::from_bits(0x432dcccd), f32::from_bits(0x4295999a)),
    ); // 172.333f, 74.757f, 173.067f, 74.7623f, 173.8f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x432e8889), f32::from_bits(0x4295ace8)),
        (f32::from_bits(0x432f4444), f32::from_bits(0x4295ddda)),
        (f32::from_bits(0x43300000), f32::from_bits(0x42960d6c)),
    ); // 174.533f, 74.8377f, 175.267f, 74.9333f, 176, 75.0262f
    path.cubic_to(
        (f32::from_bits(0x4330bbbc), f32::from_bits(0x42963cfe)),
        (f32::from_bits(0x43317777), f32::from_bits(0x429698e1)),
        (f32::from_bits(0x43323333), f32::from_bits(0x4296b706)),
    ); // 176.733f, 75.1191f, 177.467f, 75.2986f, 178.2f, 75.3575f
    path.cubic_to(
        (f32::from_bits(0x4332eeef), f32::from_bits(0x4296d52a)),
        (f32::from_bits(0x4333aaab), f32::from_bits(0x4296cfbd)),
        (f32::from_bits(0x43346666), f32::from_bits(0x4296c248)),
    ); // 178.933f, 75.4163f, 179.667f, 75.4057f, 180.4f, 75.3795f
    path.cubic_to(
        (f32::from_bits(0x43352222), f32::from_bits(0x4296b4d3)),
        (f32::from_bits(0x4335ddde), f32::from_bits(0x42967e7d)),
        (f32::from_bits(0x4336999a), f32::from_bits(0x42966648)),
    ); // 181.133f, 75.3532f, 181.867f, 75.247f, 182.6f, 75.1998f
    path.cubic_to(
        (f32::from_bits(0x43375555), f32::from_bits(0x42964e12)),
        (f32::from_bits(0x43381111), f32::from_bits(0x4296416e)),
        (f32::from_bits(0x4338cccd), f32::from_bits(0x42963108)),
    ); // 183.333f, 75.1525f, 184.067f, 75.1278f, 184.8f, 75.0958f
    path.cubic_to(
        (f32::from_bits(0x43398889), f32::from_bits(0x429620a3)),
        (f32::from_bits(0x433a4444), f32::from_bits(0x429610f3)),
        (f32::from_bits(0x433b0000), f32::from_bits(0x429603e7)),
    ); // 185.533f, 75.0637f, 186.267f, 75.0331f, 187, 75.0076f
    path.cubic_to(
        (f32::from_bits(0x433bbbbc), f32::from_bits(0x4295f6da)),
        (f32::from_bits(0x433c7777), f32::from_bits(0x4295d8e9)),
        (f32::from_bits(0x433d3333), f32::from_bits(0x4295e2bb)),
    ); // 187.733f, 74.9821f, 188.467f, 74.9237f, 189.2f, 74.9428f
    path.cubic_to(
        (f32::from_bits(0x433deeef), f32::from_bits(0x4295ec8d)),
        (f32::from_bits(0x433eaaab), f32::from_bits(0x42963733)),
        (f32::from_bits(0x433f6666), f32::from_bits(0x42963ed3)),
    ); // 189.933f, 74.962f, 190.667f, 75.1078f, 191.4f, 75.1227f
    path.cubic_to(
        (f32::from_bits(0x43402222), f32::from_bits(0x42964673)),
        (f32::from_bits(0x4340ddde), f32::from_bits(0x42962415)),
        (f32::from_bits(0x4341999a), f32::from_bits(0x42961079)),
    ); // 192.133f, 75.1376f, 192.867f, 75.0705f, 193.6f, 75.0322f
    path.cubic_to(
        (f32::from_bits(0x43425555), f32::from_bits(0x4295fcdd)),
        (f32::from_bits(0x43431111), f32::from_bits(0x4295d1c9)),
        (f32::from_bits(0x4343cccd), f32::from_bits(0x4295c929)),
    ); // 194.333f, 74.9939f, 195.067f, 74.9097f, 195.8f, 74.8929f
    path.cubic_to(
        (f32::from_bits(0x43448889), f32::from_bits(0x4295c089)),
        (f32::from_bits(0x43454444), f32::from_bits(0x4295d2aa)),
        (f32::from_bits(0x43460000), f32::from_bits(0x4295dcb8)),
    ); // 196.533f, 74.876f, 197.267f, 74.9115f, 198, 74.9311f
    path.cubic_to(
        (f32::from_bits(0x4346bbbc), f32::from_bits(0x4295e6c6)),
        (f32::from_bits(0x43477777), f32::from_bits(0x4295f749)),
        (f32::from_bits(0x43483333), f32::from_bits(0x4296057c)),
    ); // 198.733f, 74.9507f, 199.467f, 74.983f, 200.2f, 75.0107f
    path.cubic_to(
        (f32::from_bits(0x4348eeef), f32::from_bits(0x429613af)),
        (f32::from_bits(0x4349aaab), f32::from_bits(0x429628a2)),
        (f32::from_bits(0x434a6666), f32::from_bits(0x429631ea)),
    ); // 200.933f, 75.0384f, 201.667f, 75.0794f, 202.4f, 75.0975f
    path.cubic_to(
        (f32::from_bits(0x434b2222), f32::from_bits(0x42963b32)),
        (f32::from_bits(0x434bddde), f32::from_bits(0x4296396c)),
        (f32::from_bits(0x434c999a), f32::from_bits(0x42963d2d)),
    ); // 203.133f, 75.1156f, 203.867f, 75.1122f, 204.6f, 75.1195f
    path.cubic_to(
        (f32::from_bits(0x434d5555), f32::from_bits(0x429640ee)),
        (f32::from_bits(0x434e1111), f32::from_bits(0x429642b4)),
        (f32::from_bits(0x434ecccd), f32::from_bits(0x4296486f)),
    ); // 205.333f, 75.1268f, 206.067f, 75.1303f, 206.8f, 75.1415f
    path.cubic_to(
        (f32::from_bits(0x434f8889), f32::from_bits(0x42964e2b)),
        (f32::from_bits(0x43504444), f32::from_bits(0x429655c7)),
        (f32::from_bits(0x43510000), f32::from_bits(0x42965f91)),
    ); // 207.533f, 75.1527f, 208.267f, 75.1675f, 209, 75.1867f
    path.cubic_to(
        (f32::from_bits(0x4351bbbc), f32::from_bits(0x4296695b)),
        (f32::from_bits(0x43527777), f32::from_bits(0x42967a03)),
        (f32::from_bits(0x43533333), f32::from_bits(0x4296832d)),
    ); // 209.733f, 75.2058f, 210.467f, 75.2383f, 211.2f, 75.2562f
    path.cubic_to(
        (f32::from_bits(0x4353eeef), f32::from_bits(0x42968c57)),
        (f32::from_bits(0x4354aaab), f32::from_bits(0x42969305)),
        (f32::from_bits(0x43556666), f32::from_bits(0x4296968e)),
    ); // 211.933f, 75.2741f, 212.667f, 75.2871f, 213.4f, 75.2941f
    path.cubic_to(
        (f32::from_bits(0x43562222), f32::from_bits(0x42969a17)),
        (f32::from_bits(0x4356ddde), f32::from_bits(0x42969bbf)),
        (f32::from_bits(0x4357999a), f32::from_bits(0x42969863)),
    ); // 214.133f, 75.301f, 214.867f, 75.3042f, 215.6f, 75.2976f
    path.cubic_to(
        (f32::from_bits(0x43585555), f32::from_bits(0x42969506)),
        (f32::from_bits(0x43591111), f32::from_bits(0x42968980)),
        (f32::from_bits(0x4359cccd), f32::from_bits(0x42968262)),
    ); // 216.333f, 75.2911f, 217.067f, 75.2686f, 217.8f, 75.2547f
    path.cubic_to(
        (f32::from_bits(0x435a8889), f32::from_bits(0x42967b45)),
        (f32::from_bits(0x435b4444), f32::from_bits(0x42966071)),
        (f32::from_bits(0x435c0000), f32::from_bits(0x42966db2)),
    ); // 218.533f, 75.2408f, 219.267f, 75.1884f, 220, 75.2142f
    path.cubic_to(
        (f32::from_bits(0x435cbbbc), f32::from_bits(0x42967af3)),
        (f32::from_bits(0x435d7777), f32::from_bits(0x4296ba1b)),
        (f32::from_bits(0x435e3333), f32::from_bits(0x4296d1e8)),
    ); // 220.733f, 75.2401f, 221.467f, 75.3635f, 222.2f, 75.41f
    path.cubic_to(
        (f32::from_bits(0x435eeeef), f32::from_bits(0x4296e9b5)),
        (f32::from_bits(0x435faaab), f32::from_bits(0x4296fe13)),
        (f32::from_bits(0x43606666), f32::from_bits(0x4296fc81)),
    ); // 222.933f, 75.4565f, 223.667f, 75.4962f, 224.4f, 75.4932f
    path.cubic_to(
        (f32::from_bits(0x43612222), f32::from_bits(0x4296faef)),
        (f32::from_bits(0x4361ddde), f32::from_bits(0x4296d99e)),
        (f32::from_bits(0x4362999a), f32::from_bits(0x4296c87a)),
    ); // 225.133f, 75.4901f, 225.867f, 75.425f, 226.6f, 75.3916f
    path.cubic_to(
        (f32::from_bits(0x43635555), f32::from_bits(0x4296b757)),
        (f32::from_bits(0x43641111), f32::from_bits(0x42969bf5)),
        (f32::from_bits(0x4364cccd), f32::from_bits(0x429695ac)),
    ); // 227.333f, 75.3581f, 228.067f, 75.3046f, 228.8f, 75.2923f
    path.cubic_to(
        (f32::from_bits(0x43658889), f32::from_bits(0x42968f62)),
        (f32::from_bits(0x43664444), f32::from_bits(0x42969d8e)),
        (f32::from_bits(0x43670000), f32::from_bits(0x4296a2c3)),
    ); // 229.533f, 75.28f, 230.267f, 75.3077f, 231, 75.3179f
    path.cubic_to(
        (f32::from_bits(0x4367bbbc), f32::from_bits(0x4296a7f8)),
        (f32::from_bits(0x43687777), f32::from_bits(0x4296b284)),
        (f32::from_bits(0x43693333), f32::from_bits(0x4296b4eb)),
    ); // 231.733f, 75.3281f, 232.467f, 75.3487f, 233.2f, 75.3534f
    path.cubic_to(
        (f32::from_bits(0x4369eeef), f32::from_bits(0x4296b751)),
        (f32::from_bits(0x436aaaab), f32::from_bits(0x4296b8ac)),
        (f32::from_bits(0x436b6666), f32::from_bits(0x4296b12a)),
    ); // 233.933f, 75.358f, 234.667f, 75.3607f, 235.4f, 75.346f
    path.cubic_to(
        (f32::from_bits(0x436c2222), f32::from_bits(0x4296a9a8)),
        (f32::from_bits(0x436cddde), f32::from_bits(0x42969446)),
        (f32::from_bits(0x436d999a), f32::from_bits(0x429687e1)),
    ); // 236.133f, 75.3314f, 236.867f, 75.2896f, 237.6f, 75.2654f
    path.cubic_to(
        (f32::from_bits(0x436e5555), f32::from_bits(0x42967b7c)),
        (f32::from_bits(0x436f1111), f32::from_bits(0x42967079)),
        (f32::from_bits(0x436fcccd), f32::from_bits(0x429666cd)),
    ); // 238.333f, 75.2412f, 239.067f, 75.2197f, 239.8f, 75.2008f
    path.cubic_to(
        (f32::from_bits(0x43708889), f32::from_bits(0x42965d20)),
        (f32::from_bits(0x43714444), f32::from_bits(0x42965d94)),
        (f32::from_bits(0x43720000), f32::from_bits(0x42964dd6)),
    ); // 240.533f, 75.1819f, 241.267f, 75.1828f, 242, 75.152f
    path.cubic_to(
        (f32::from_bits(0x4372bbbc), f32::from_bits(0x42963e19)),
        (f32::from_bits(0x43737777), f32::from_bits(0x42961c64)),
        (f32::from_bits(0x43743333), f32::from_bits(0x4296085b)),
    ); // 242.733f, 75.1213f, 243.467f, 75.0555f, 244.2f, 75.0163f
    path.cubic_to(
        (f32::from_bits(0x4374eeef), f32::from_bits(0x4295f453)),
        (f32::from_bits(0x4375aaab), f32::from_bits(0x4295e407)),
        (f32::from_bits(0x43766666), f32::from_bits(0x4295d5a4)),
    ); // 244.933f, 74.9772f, 245.667f, 74.9454f, 246.4f, 74.9173f
    path.cubic_to(
        (f32::from_bits(0x43772222), f32::from_bits(0x4295c741)),
        (f32::from_bits(0x4377ddde), f32::from_bits(0x4295bc09)),
        (f32::from_bits(0x4378999a), f32::from_bits(0x4295b207)),
    ); // 247.133f, 74.8892f, 247.867f, 74.8673f, 248.6f, 74.8477f
    path.cubic_to(
        (f32::from_bits(0x43795555), f32::from_bits(0x4295a806)),
        (f32::from_bits(0x437a1111), f32::from_bits(0x42959dad)),
        (f32::from_bits(0x437acccd), f32::from_bits(0x4295999a)),
    ); // 249.333f, 74.8282f, 250.067f, 74.808f, 250.8f, 74.8f
    path.cubic_to(
        (f32::from_bits(0x437b8889), f32::from_bits(0x42959588)),
        (f32::from_bits(0x437c4444), f32::from_bits(0x4295999a)),
        (f32::from_bits(0x437d0000), f32::from_bits(0x4295999a)),
    ); // 251.533f, 74.7921f, 252.267f, 74.8f, 253, 74.8f
    path.line_to((f32::from_bits(0x437d0000), f32::from_bits(0x4295999a))); // 253, 74.8f
    path.line_to((f32::from_bits(0x42886666), f32::from_bits(0x4295999a))); // 68.2f, 74.8f
    path.close();
    test_path_op(reporter, &pathA, &path.detach(), PathOp::Union, filename);
}

// Port of: tests/PathOpsIssue3651.cpp#L1688-L1690 (chrome/m156)
def_test!(PathOpsIssue3651, |reporter| {
    // RunTestSet(reporter, tests, testCount, firstTest = issue3651_1, nullptr, nullptr, false):
    // issue3651_1 runs first, then every other case in table order.
    issue3651_1(reporter, "issue3651_1");
    issue3651_6(reporter, "issue3651_6");
    issue3651_1a(reporter, "issue3651_1a");
    issue3651_7(reporter, "issue3651_7");
    issue3651_5(reporter, "issue3651_5");
    issue3651_4(reporter, "issue3651_4");
    issue3651_2(reporter, "issue3651_2");
    issue3651_3(reporter, "issue3651_3");
});
