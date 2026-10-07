// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/MemsetTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_simd::memset::{memset16, memset32};

use crate::{Reporter, def_test, errorf};

// Port of: tests/MemsetTest.cpp#L18-L23 (chrome/m156)
fn set_zero<T: Default + Copy>(dst: &mut [T]) {
    for x in dst {
        *x = T::default();
    }
}

// Port of: tests/MemsetTest.cpp#L25-L30 (chrome/m156)
const MAX_ALIGNMENT: usize = 64;
const MAX_COUNT: usize = MAX_ALIGNMENT * 32;
const PAD: usize = 32;
const TOTAL: usize = PAD + MAX_ALIGNMENT + MAX_COUNT + PAD;

// Port of: tests/MemsetTest.cpp#L32-L33 (chrome/m156)
const VALUE16: u16 = 0x1234;
const VALUE32: u32 = 0x1234_5678;

// Port of: tests/MemsetTest.cpp#L35-L43 (chrome/m156)
fn compare16(reporter: &mut Reporter, base: &[u16], value: u16, count: usize) {
    for (i, &b) in base[..count].iter().enumerate() {
        if b != value {
            errorf!(reporter, "[{}] expected {:x} found {:x}\n", i, value, b);
            return;
        }
    }
}

// Port of: tests/MemsetTest.cpp#L45-L53 (chrome/m156)
fn compare32(reporter: &mut Reporter, base: &[u32], value: u32, count: usize) {
    for (i, &b) in base[..count].iter().enumerate() {
        if b != value {
            errorf!(reporter, "[{}] expected {:x} found {:x}\n", i, value, b);
            return;
        }
    }
}

// Port of: tests/MemsetTest.cpp#L55-L70 (chrome/m156)
fn test_16(reporter: &mut Reporter) {
    let mut buffer = [0u16; TOTAL];

    for count in 0..MAX_COUNT {
        for alignment in 0..MAX_ALIGNMENT {
            set_zero(&mut buffer);

            let base = PAD + alignment;
            memset16(&mut buffer[base..], VALUE16, count);

            compare16(reporter, &buffer, 0, PAD + alignment);
            compare16(reporter, &buffer[base..], VALUE16, count);
            compare16(
                reporter,
                &buffer[base + count..],
                0,
                TOTAL - count - PAD - alignment,
            );
        }
    }
}

// Port of: tests/MemsetTest.cpp#L72-L87 (chrome/m156)
fn test_32(reporter: &mut Reporter) {
    let mut buffer = [0u32; TOTAL];

    for count in 0..MAX_COUNT {
        for alignment in 0..MAX_ALIGNMENT {
            set_zero(&mut buffer);

            let base = PAD + alignment;
            memset32(&mut buffer[base..], VALUE32, count);

            compare32(reporter, &buffer, 0, PAD + alignment);
            compare32(reporter, &buffer[base..], VALUE32, count);
            compare32(
                reporter,
                &buffer[base + count..],
                0,
                TOTAL - count - PAD - alignment,
            );
        }
    }
}

// Test SkOpts::memset16 and SkOpts::memset32.
// For performance considerations, implementations may take different paths
// depending on the alignment of the dst, and/or the size of the count.
// Port of: tests/MemsetTest.cpp#L88-L91 (chrome/m156)
def_test!(Memset, |reporter| {
    test_16(reporter);
    test_32(reporter);
});
