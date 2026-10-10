// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/MatrixProcsTest.cpp (chrome/m156)

#![cfg(test)]
// The literals are copied verbatim from the C++ test, digits and all.
#![allow(clippy::excessive_precision, clippy::unreadable_literal)]

use skia_rust_core::bitmap_proc_state_priv::{
    decode_packed_coordinates_and_weight, pack_clamp, pack_mirror, pack_repeat,
};
use skia_rust_core::fixed::{Fixed, float_to_fixed};

use crate::{def_test, reporter_assert};

// Port of: tests/MatrixProcsTest.cpp#L19-L21 (chrome/m156)
fn high_bits(rv: u32) -> u32 {
    (rv >> 18) & ((1 << 14) - 1)
}

// Port of: tests/MatrixProcsTest.cpp#L23-L25 (chrome/m156)
fn middle_bits(rv: u32) -> u32 {
    (rv >> 14) & ((1 << 4) - 1)
}

// Port of: tests/MatrixProcsTest.cpp#L27-L29 (chrome/m156)
fn low_bits(rv: u32) -> u32 {
    rv & ((1 << 14) - 1)
}

// Port of: tests/MatrixProcsTest.cpp#L31-L81 (chrome/m156)
def_test!(MatrixProcs_pack_clamp, |r| {
    struct TestCase {
        name: &'static str,
        input: Fixed,
        max: u32,
        expected_output: u32,
    }
    // The input values are somewhat arbitrary, inspired by real-world values
    // with some edge cases added as well.
    let tests = [
        // Negative values keep the fractional part out of convenience, but it is effectively
        // ignored later.
        TestCase {
            name: "-2.100 => {0x00, 0xe, 0x00}",
            input: float_to_fixed(-2.100),
            max: 63,
            expected_output: 0x38000,
        },
        TestCase {
            name: "-1.900 => {0x00, 0x1, 0x00}",
            input: float_to_fixed(-1.900),
            max: 63,
            expected_output: 0x04000,
        },
        TestCase {
            name: "-0.500 => {0x00, 0x8, 0x00}",
            input: float_to_fixed(-0.500),
            max: 63,
            expected_output: 0x20000,
        },
        TestCase {
            name: "0.0000 => {0x00, 0x0, 0x01}",
            input: float_to_fixed(0.0000),
            max: 63,
            expected_output: 0x000001,
        },
        TestCase {
            name: "0.0416 => {0x00, 0x0, 0x01}",
            input: float_to_fixed(0.0416),
            max: 63,
            expected_output: 0x000001,
        },
        TestCase {
            name: "1.8583 => {0x01, 0xd, 0x02}",
            input: float_to_fixed(1.8583),
            max: 63,
            expected_output: 0x074002,
        },
        TestCase {
            name: "3.6749 => {0x03, 0xa, 0x04}",
            input: float_to_fixed(3.6749),
            max: 63,
            expected_output: 0x0e8004,
        },
        TestCase {
            name: "5.4916 => {0x05, 0x7, 0x06}",
            input: float_to_fixed(5.4916),
            max: 63,
            expected_output: 0x15c006,
        },
        TestCase {
            name: "7.3083 => {0x07, 0x4, 0x08}",
            input: float_to_fixed(7.3083),
            max: 63,
            expected_output: 0x1d0008,
        },
        TestCase {
            name: "9.0000 => {0x09, 0x0, 0x0a}",
            input: float_to_fixed(9.0000),
            max: 63,
            expected_output: 0x24000a,
        },
        TestCase {
            name: "50.000 => {0x32, 0x0, 0x33}",
            input: float_to_fixed(50.000),
            max: 63,
            expected_output: 0xc80033,
        },
        TestCase {
            name: "50.875 => {0x32, 0xe, 0x33}",
            input: float_to_fixed(50.875),
            max: 63,
            expected_output: 0xcb8033,
        },
        TestCase {
            name: "62.123 => {0x3e, 0x1, 0x3f}",
            input: float_to_fixed(62.123),
            max: 63,
            expected_output: 0xf8403f,
        },
        TestCase {
            name: "62.999 => {0x3e, 0xf, 0x3f}",
            input: float_to_fixed(62.999),
            max: 63,
            expected_output: 0xfbc03f,
        },
        TestCase {
            name: "63.000 => {0x3f, 0x0, 0x3f}",
            input: float_to_fixed(63.000),
            max: 63,
            expected_output: 0xfc003f,
        },
        // Similarly, overflow keeps the fractional part.
        TestCase {
            name: "64.500 => {0x3f, 0x8, 0x3f}",
            input: float_to_fixed(64.500),
            max: 63,
            expected_output: 0xfe003f,
        },
        TestCase {
            name: "127.20 => {0x3f, 0x3, 0x3f}",
            input: float_to_fixed(127.20),
            max: 63,
            expected_output: 0xfcc03f,
        },
        // Maximum has changed from 63 to 256
        TestCase {
            name: "64.510 => {0x40,  0x8, 0x41}",
            input: float_to_fixed(64.510),
            max: 256,
            expected_output: 0x1020041,
        },
        TestCase {
            name: "127.21 => {0x7f,  0x3, 0x80}",
            input: float_to_fixed(127.21),
            max: 256,
            expected_output: 0x1fcc080,
        },
        TestCase {
            name: "256.77 => {0x100, 0xc, 0x100}",
            input: float_to_fixed(256.77),
            max: 256,
            expected_output: 0x4030100,
        },
        TestCase {
            name: "10000. => {0x100, 0x0, 0x100}",
            input: float_to_fixed(10000.),
            max: 256,
            expected_output: 0x4000100,
        },
    ];
    for tc in &tests {
        let rv = pack_clamp(tc.input, tc.max);
        let exp = tc.expected_output;
        reporter_assert!(
            r,
            rv == tc.expected_output,
            "{} | {:x} != {:x} | {{{:x}, {:x}, {:x}}} != {{{:x}, {:x}, {:x}}}\n",
            tc.name,
            rv,
            exp,
            high_bits(rv),
            middle_bits(rv),
            low_bits(rv),
            high_bits(exp),
            middle_bits(exp),
            low_bits(exp)
        );
    }
});

// Port of: tests/MatrixProcsTest.cpp#L83-L123 (chrome/m156)
def_test!(MatrixProcs_pack_clamp_out_of_range, |r| {
    struct TestCase {
        name: &'static str,
        input: Fixed,
        max: u32,
        expected_output: u32,
    }
    // See https://crbug.com/1357122
    // None of these should crash or cause UBSAN errors, although if they were used in drawing,
    // they might be incorrect due to packing 16 bits of SkFixed into 14 bits of space.
    const MAX_PACKED_VALUE: u32 = (1 << 14) - 1;
    let tests = [
        TestCase {
            name: "16000.42=>{0xff, 6, 0xff}",
            input: float_to_fixed(16000.42),
            max: 255,
            expected_output: 0x3fd80ff,
        },
        TestCase {
            name: "17000.42=>{0xff, 6, 0xff}",
            input: float_to_fixed(17000.42),
            max: 255,
            expected_output: 0x3fd80ff,
        },
        TestCase {
            name: "18000.42=>{0xff, 6, 0xff}",
            input: float_to_fixed(18000.42),
            max: 255,
            expected_output: 0x3fd80ff,
        },
        TestCase {
            name: "16000.42=>{0x3e80, 6, 0x3e81}",
            input: float_to_fixed(16000.42),
            max: MAX_PACKED_VALUE,
            expected_output: 0xfa01be81,
        },
        TestCase {
            name: "16382.00=>{0x3ffe, 0, 0x3fff}",
            input: float_to_fixed(16382.00),
            max: MAX_PACKED_VALUE,
            expected_output: 0xfff83fff,
        },
        TestCase {
            name: "16382.51=>{0x3ffe, 8, 0x3fff}}",
            input: float_to_fixed(16382.51),
            max: MAX_PACKED_VALUE,
            expected_output: 0xfffa3fff,
        },
        TestCase {
            name: "17000.42=>{0x3fff, 6, 0x3fff}",
            input: float_to_fixed(17000.42),
            max: MAX_PACKED_VALUE,
            expected_output: 0xfffdbfff,
        },
        TestCase {
            name: "18000.42=>{0x3fff, 6, 0x3fff}",
            input: float_to_fixed(18000.42),
            max: MAX_PACKED_VALUE,
            expected_output: 0xfffdbfff,
        },
        // Adding 1 to this would overflow and cause an UBSAN issue, if it were not suppressed.
        // We suppress the warning and it wraps around.
        TestCase {
            name: "32767.90=>{0x3fff, e, 0x0}",
            input: float_to_fixed(32767.90),
            max: MAX_PACKED_VALUE,
            expected_output: 0xffff8000,
        },
    ];
    for tc in &tests {
        let rv = pack_clamp(tc.input, tc.max);
        let exp = tc.expected_output;
        reporter_assert!(
            r,
            rv == tc.expected_output,
            "{} | {:x} != {:x} | {{{:x}, {:x}, {:x}}} != {{{:x}, {:x}, {:x}}}\n",
            tc.name,
            rv,
            exp,
            high_bits(rv),
            middle_bits(rv),
            low_bits(rv),
            high_bits(exp),
            middle_bits(exp),
            low_bits(exp)
        );
    }
});

// Port of: tests/MatrixProcsTest.cpp#L125-L183 (chrome/m156)
def_test!(MatrixProcs_pack_repeat, |r| {
    struct TestCase {
        name: &'static str,
        input: Fixed,
        max: u32,
        expected_output: u32,
        width: usize,
    }
    let tests = [
        // negative values wrap back around
        TestCase {
            name: "-0.300 => {0x2c, 0xc, 0x2d}",
            input: float_to_fixed(-0.300),
            max: 63,
            expected_output: 0xb3002d,
            width: 63,
        },
        TestCase {
            name: "-0.200 => {0x33, 0x3, 0x34}",
            input: float_to_fixed(-0.200),
            max: 63,
            expected_output: 0xccc034,
            width: 63,
        },
        TestCase {
            name: "-0.100 => {0x39, 0x9, 0x3a}",
            input: float_to_fixed(-0.100),
            max: 63,
            expected_output: 0xe6403a,
            width: 63,
        },
        // The domain of the function is primarily [0.0, 1.0)
        TestCase {
            name: "0.0000 => {0x00, 0x0, 0x01}",
            input: float_to_fixed(0.0000),
            max: 63,
            expected_output: 0x000001,
            width: 63,
        },
        TestCase {
            name: "0.1000 => {0x06, 0x6, 0x07}",
            input: float_to_fixed(0.1000),
            max: 63,
            expected_output: 0x198007,
            width: 63,
        },
        TestCase {
            name: "0.1234 => {0x07, 0xe, 0x08}",
            input: float_to_fixed(0.1234),
            max: 63,
            expected_output: 0x1f8008,
            width: 63,
        },
        TestCase {
            name: "0.2000 => {0x0c, 0xc, 0x0d}",
            input: float_to_fixed(0.2000),
            max: 63,
            expected_output: 0x33000d,
            width: 63,
        },
        TestCase {
            name: "0.3000 => {0x13, 0x3, 0x14}",
            input: float_to_fixed(0.3000),
            max: 63,
            expected_output: 0x4cc014,
            width: 63,
        },
        TestCase {
            name: "0.4000 => {0x19, 0x9, 0x1a}",
            input: float_to_fixed(0.4000),
            max: 63,
            expected_output: 0x66401a,
            width: 63,
        },
        TestCase {
            name: "0.5000 => {0x20, 0x0, 0x21}",
            input: float_to_fixed(0.5000),
            max: 63,
            expected_output: 0x800021,
            width: 63,
        },
        TestCase {
            name: "0.5678 => {0x24, 0x5, 0x25}",
            input: float_to_fixed(0.5678),
            max: 63,
            expected_output: 0x914025,
            width: 63,
        },
        TestCase {
            name: "0.6000 => {0x26, 0x6, 0x27}",
            input: float_to_fixed(0.6000),
            max: 63,
            expected_output: 0x998027,
            width: 63,
        },
        TestCase {
            name: "0.7000 => {0x2c, 0xc, 0x2d}",
            input: float_to_fixed(0.7000),
            max: 63,
            expected_output: 0xb3002d,
            width: 63,
        },
        TestCase {
            name: "0.8000 => {0x33, 0x3, 0x34}",
            input: float_to_fixed(0.8000),
            max: 63,
            expected_output: 0xccc034,
            width: 63,
        },
        TestCase {
            name: "0.9000 => {0x39, 0x9, 0x3a}",
            input: float_to_fixed(0.9000),
            max: 63,
            expected_output: 0xe6403a,
            width: 63,
        },
        TestCase {
            name: "0.9500 => {0x3c, 0xc, 0x3d}",
            input: float_to_fixed(0.9500),
            max: 63,
            expected_output: 0xf3003d,
            width: 63,
        },
        TestCase {
            name: "0.9990 => {0x3f, 0xe, 0x00}",
            input: float_to_fixed(0.9990),
            max: 63,
            expected_output: 0xff8000,
            width: 63,
        },
        // As we go past 1.0, we wrap around, conceptually similar to modular arithmetic.
        TestCase {
            name: "1.0000 => {0x00, 0x0, 0x01}",
            input: float_to_fixed(1.0000),
            max: 63,
            expected_output: 0x000001,
            width: 63,
        },
        TestCase {
            name: "1.1000 => {0x06, 0x6, 0x07}",
            input: float_to_fixed(1.1000),
            max: 63,
            expected_output: 0x198007,
            width: 63,
        },
        TestCase {
            name: "1.1234 => {0x07, 0xe, 0x08}",
            input: float_to_fixed(1.1234),
            max: 63,
            expected_output: 0x1f8008,
            width: 63,
        },
        TestCase {
            name: "1.9500 => {0x3c, 0xc, 0x3d}",
            input: float_to_fixed(1.9500),
            max: 63,
            expected_output: 0xf3003d,
            width: 63,
        },
        // Maximum has changed from 63 to 256
        TestCase {
            name: "0.4567 => {0x75, 0x5, 0x76}",
            input: float_to_fixed(0.4567),
            max: 256,
            expected_output: 0x1d54076,
            width: 256,
        },
        TestCase {
            name: "1.0000 => {0x00, 0x0, 0x01}",
            input: float_to_fixed(1.0000),
            max: 256,
            expected_output: 0x0000001,
            width: 256,
        },
        TestCase {
            name: "1.2345 => {0x3c, 0x4, 0x3d}",
            input: float_to_fixed(1.2345),
            max: 256,
            expected_output: 0x0f1003d,
            width: 256,
        },
        // width does not have to match the maximum value (e.g. rescaling)
        TestCase {
            name: "0.1111 [64,128] => {0x07, 0x3, 0x07}",
            input: float_to_fixed(0.1111),
            max: 64,
            expected_output: 0x1cc007,
            width: 128,
        },
        TestCase {
            name: "0.1111 [64,256] => {0x07, 0x3, 0x07}",
            input: float_to_fixed(0.1111),
            max: 64,
            expected_output: 0x1cc007,
            width: 256,
        },
        TestCase {
            name: "0.1111 [64,512] => {0x07, 0x3, 0x07}",
            input: float_to_fixed(0.1111),
            max: 64,
            expected_output: 0x1cc007,
            width: 512,
        },
        TestCase {
            name: "0.1111 [64, 32] => {0x07, 0x3, 0x09}",
            input: float_to_fixed(0.1111),
            max: 64,
            expected_output: 0x1cc009,
            width: 32,
        },
        TestCase {
            name: "0.1111 [64,  8] => {0x07, 0x3, 0x0f}",
            input: float_to_fixed(0.1111),
            max: 64,
            expected_output: 0x1cc00f,
            width: 8,
        },
    ];
    for tc in &tests {
        let rv = pack_repeat(tc.input, tc.max, tc.width);
        let exp = tc.expected_output;
        reporter_assert!(
            r,
            rv == tc.expected_output,
            "{} | {:x} != {:x} | {{{:x}, {:x}, {:x}}} != {{{:x}, {:x}, {:x}}}\n",
            tc.name,
            rv,
            exp,
            high_bits(rv),
            middle_bits(rv),
            low_bits(rv),
            high_bits(exp),
            middle_bits(exp),
            low_bits(exp)
        );
    }
});

// Port of: tests/MatrixProcsTest.cpp#L185-L244 (chrome/m156)
def_test!(MatrixProcs_pack_mirror, |r| {
    struct TestCase {
        name: &'static str,
        input: Fixed,
        max: u32,
        expected_output: u32,
        width: usize,
    }
    let tests = [
        // negative values are treated similarly to absolute values, except
        // the first integer is bigger than the last integer.
        TestCase {
            name: "-0.300 => {0x13, 0xc, 0x12}",
            input: float_to_fixed(-0.300),
            max: 63,
            expected_output: 0x4f0012,
            width: 63,
        },
        TestCase {
            name: "-0.200 => {0x0c, 0x3, 0x0b}",
            input: float_to_fixed(-0.200),
            max: 63,
            expected_output: 0x30c00b,
            width: 63,
        },
        TestCase {
            name: "-0.100 => {0x06, 0x9, 0x05}",
            input: float_to_fixed(-0.100),
            max: 63,
            expected_output: 0x1a4005,
            width: 63,
        },
        // The domain of the function is primarily [0.0, 1.0)
        TestCase {
            name: "0.0000 => {0x00, 0x0, 0x01}",
            input: float_to_fixed(0.0000),
            max: 63,
            expected_output: 0x000001,
            width: 63,
        },
        TestCase {
            name: "0.1000 => {0x06, 0x6, 0x07}",
            input: float_to_fixed(0.1000),
            max: 63,
            expected_output: 0x198007,
            width: 63,
        },
        TestCase {
            name: "0.1234 => {0x07, 0xe, 0x08}",
            input: float_to_fixed(0.1234),
            max: 63,
            expected_output: 0x1f8008,
            width: 63,
        },
        TestCase {
            name: "0.2000 => {0x0c, 0xc, 0x0d}",
            input: float_to_fixed(0.2000),
            max: 63,
            expected_output: 0x33000d,
            width: 63,
        },
        TestCase {
            name: "0.3000 => {0x13, 0x3, 0x14}",
            input: float_to_fixed(0.3000),
            max: 63,
            expected_output: 0x4cc014,
            width: 63,
        },
        TestCase {
            name: "0.4000 => {0x19, 0x9, 0x1a}",
            input: float_to_fixed(0.4000),
            max: 63,
            expected_output: 0x66401a,
            width: 63,
        },
        TestCase {
            name: "0.5000 => {0x20, 0x0, 0x21}",
            input: float_to_fixed(0.5000),
            max: 63,
            expected_output: 0x800021,
            width: 63,
        },
        TestCase {
            name: "0.5678 => {0x24, 0x5, 0x25}",
            input: float_to_fixed(0.5678),
            max: 63,
            expected_output: 0x914025,
            width: 63,
        },
        TestCase {
            name: "0.6000 => {0x26, 0x6, 0x27}",
            input: float_to_fixed(0.6000),
            max: 63,
            expected_output: 0x998027,
            width: 63,
        },
        TestCase {
            name: "0.7000 => {0x2c, 0xc, 0x2d}",
            input: float_to_fixed(0.7000),
            max: 63,
            expected_output: 0xb3002d,
            width: 63,
        },
        TestCase {
            name: "0.8000 => {0x33, 0x3, 0x34}",
            input: float_to_fixed(0.8000),
            max: 63,
            expected_output: 0xccc034,
            width: 63,
        },
        TestCase {
            name: "0.9000 => {0x39, 0x9, 0x3a}",
            input: float_to_fixed(0.9000),
            max: 63,
            expected_output: 0xe6403a,
            width: 63,
        },
        TestCase {
            name: "0.9500 => {0x3c, 0xc, 0x3d}",
            input: float_to_fixed(0.9500),
            max: 63,
            expected_output: 0xf3003d,
            width: 63,
        },
        TestCase {
            name: "0.9990 => {0x3f, 0xe, 0x3f}",
            input: float_to_fixed(0.9990),
            max: 63,
            expected_output: 0xff803f,
            width: 63,
        },
        // As we go past 1.0, we bounce back, as off a wall or reflecting off a mirror.
        TestCase {
            name: "1.0000 => {0x3f, 0x0, 0x3e}",
            input: float_to_fixed(1.0000),
            max: 63,
            expected_output: 0xfc003e,
            width: 63,
        },
        TestCase {
            name: "1.1000 => {0x39, 0x6, 0x38}",
            input: float_to_fixed(1.1000),
            max: 63,
            expected_output: 0xe58038,
            width: 63,
        },
        TestCase {
            name: "1.1234 => {0x38, 0xe, 0x37}",
            input: float_to_fixed(1.1234),
            max: 63,
            expected_output: 0xe38037,
            width: 63,
        },
        TestCase {
            name: "1.9500 => {0x03, 0xc, 0x02}",
            input: float_to_fixed(1.9500),
            max: 63,
            expected_output: 0xf0002,
            width: 63,
        },
        // Maximum has changed from 63 to 256
        TestCase {
            name: "0.4567 => {0x75, 0x5, 0x76}",
            input: float_to_fixed(0.4567),
            max: 256,
            expected_output: 0x1d54076,
            width: 256,
        },
        TestCase {
            name: "1.0000 => {0x100,0x0, 0xff}",
            input: float_to_fixed(1.0000),
            max: 256,
            expected_output: 0x40000ff,
            width: 256,
        },
        TestCase {
            name: "1.2345 => {0xc4, 0x4, 0xc3}",
            input: float_to_fixed(1.2345),
            max: 256,
            expected_output: 0x31100c3,
            width: 256,
        },
        // width does not have to match the maximum value (e.g. rescaling)
        TestCase {
            name: "0.1111 [64,128] => {0x07, 0x3, 0x07}",
            input: float_to_fixed(0.1111),
            max: 64,
            expected_output: 0x1cc007,
            width: 128,
        },
        TestCase {
            name: "0.1111 [64,256] => {0x07, 0x3, 0x07}",
            input: float_to_fixed(0.1111),
            max: 64,
            expected_output: 0x1cc007,
            width: 256,
        },
        TestCase {
            name: "0.1111 [64,512] => {0x07, 0x3, 0x07}",
            input: float_to_fixed(0.1111),
            max: 64,
            expected_output: 0x1cc007,
            width: 512,
        },
        TestCase {
            name: "0.1111 [64, 32] => {0x07, 0x3, 0x09}",
            input: float_to_fixed(0.1111),
            max: 64,
            expected_output: 0x1cc009,
            width: 32,
        },
        TestCase {
            name: "0.1111 [64,  8] => {0x07, 0x3, 0x0f}",
            input: float_to_fixed(0.1111),
            max: 64,
            expected_output: 0x1cc00f,
            width: 8,
        },
    ];
    for tc in &tests {
        let rv = pack_mirror(tc.input, tc.max, tc.width);
        let exp = tc.expected_output;
        reporter_assert!(
            r,
            rv == tc.expected_output,
            "{} | {:x} != {:x} | {{{:x}, {:x}, {:x}}} != {{{:x}, {:x}, {:x}}}\n",
            tc.name,
            rv,
            exp,
            high_bits(rv),
            middle_bits(rv),
            low_bits(rv),
            high_bits(exp),
            middle_bits(exp),
            low_bits(exp)
        );
    }
});

// Port of: tests/MatrixProcsTest.cpp#L246-L283 (chrome/m156)
def_test!(MatrixProcs_unpack_int, |r| {
    struct TestCase {
        name: &'static str,
        input: u32,
        expected_lower_bound: u32,
        expected_lerp: u32,
        expected_upper_bound: u32,
    }
    // These are selected from earlier tests to make sure the packed values unpack correctly.
    let tests = [
        TestCase {
            name: "0x000000  => {0x00, 0x0, 0x00}",
            input: 0x000000,
            expected_lower_bound: 0x00,
            expected_lerp: 0x0,
            expected_upper_bound: 0x00,
        },
        TestCase {
            name: "0x074002  => {0x01, 0xd, 0x02}",
            input: 0x074002,
            expected_lower_bound: 0x01,
            expected_lerp: 0xd,
            expected_upper_bound: 0x02,
        },
        TestCase {
            name: "0x15c006  => {0x05, 0x7, 0x06}",
            input: 0x15c006,
            expected_lower_bound: 0x05,
            expected_lerp: 0x7,
            expected_upper_bound: 0x06,
        },
        TestCase {
            name: "0x1d0008  => {0x07, 0x4, 0x08}",
            input: 0x1d0008,
            expected_lower_bound: 0x07,
            expected_lerp: 0x4,
            expected_upper_bound: 0x08,
        },
        TestCase {
            name: "0x24000a  => {0x09, 0x0, 0x0a}",
            input: 0x24000a,
            expected_lower_bound: 0x09,
            expected_lerp: 0x0,
            expected_upper_bound: 0x0a,
        },
        TestCase {
            name: "0xfc003f  => {0x3f, 0x0, 0x3f}",
            input: 0xfc003f,
            expected_lower_bound: 0x3f,
            expected_lerp: 0x0,
            expected_upper_bound: 0x3f,
        },
        TestCase {
            name: "0x4000100 => {0x100, 0x0, 0x100}",
            input: 0x4000100,
            expected_lower_bound: 0x100,
            expected_lerp: 0x0,
            expected_upper_bound: 0x100,
        },
    ];

    for tc in &tests {
        let mut lower = 0;
        let mut upper = 0;
        let mut lerp = 0;
        decode_packed_coordinates_and_weight(tc.input, &mut lower, &mut upper, &mut lerp);

        reporter_assert!(
            r,
            lower == tc.expected_lower_bound,
            "{} lower {:x} != {:x}",
            tc.name,
            lower,
            tc.expected_lower_bound
        );
        reporter_assert!(
            r,
            lerp == tc.expected_lerp,
            "{} lerp {:x} != {:x}",
            tc.name,
            lerp,
            tc.expected_lerp
        );
        reporter_assert!(
            r,
            upper == tc.expected_upper_bound,
            "{} upper {:x} != {:x}",
            tc.name,
            upper,
            tc.expected_upper_bound
        );
        // Make sure our helpers work as expected.
        debug_assert_eq!(tc.expected_lower_bound, high_bits(tc.input));
        debug_assert_eq!(tc.expected_lerp, middle_bits(tc.input));
        debug_assert_eq!(tc.expected_upper_bound, low_bits(tc.input));
    }
});
