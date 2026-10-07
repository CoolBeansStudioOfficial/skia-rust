// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/BlendTest.cpp (chrome/m156)
//
// Only `Blend_byte_multiply` (pure integer arithmetic) is ported here; the other test in the
// file is a Ganesh/GL test.

#![cfg(test)]

use crate::{def_test, reporter_assert};

// Port of: tests/BlendTest.cpp#L33-L33 (chrome/m156)
struct Results {
    diffs: i32,
    diffs_0x00: i32,
    diffs_0xff: i32,
    diffs_by_1: i32,
}

// Port of: tests/BlendTest.cpp#L35-L43 (chrome/m156)
fn acceptable(r: &Results) -> bool {
    r.diffs_by_1 == r.diffs   // never off by more than 1
        && r.diffs_0x00 == 0  // transparent must stay transparent
        && r.diffs_0xff == 0 // opaque must stay opaque
}

// Port of: tests/BlendTest.cpp#L45-L62 (chrome/m156)
fn test(multiply: fn(i32, i32) -> i32) -> Results {
    let mut r = Results {
        diffs: 0,
        diffs_0x00: 0,
        diffs_0xff: 0,
        diffs_by_1: 0,
    };
    for x in 0..256 {
        for y in 0..256 {
            let p = multiply(x, y);
            let ideal = (x * y + 127) / 255;
            if p != ideal {
                r.diffs += 1;
                if x == 0x00 || y == 0x00 {
                    r.diffs_0x00 += 1;
                }
                if x == 0xff || y == 0xff {
                    r.diffs_0xff += 1;
                }
                if (ideal - p).abs() == 1 {
                    r.diffs_by_1 += 1;
                }
            }
        }
    }
    r
}

// Port of: tests/BlendTest.cpp#L64-L89 (chrome/m156)
def_test!(Blend_byte_multiply, |r| {
    // These are all temptingly close but fundamentally broken.
    let broken: [fn(i32, i32) -> i32; 3] = [
        |x, y| (x * y) >> 8,
        |x, y| (x * y + 128) >> 8,
        |x, mut y| {
            y += y >> 7;
            (x * y) >> 8
        },
    ];
    for multiply in broken {
        reporter_assert!(r, !acceptable(&test(multiply)));
    }

    // These are fine to use, but not perfect.
    let fine: [fn(i32, i32) -> i32; 4] = [
        |x, y| (x * y + x) >> 8,
        |x, y| (x * y + y) >> 8,
        |x, y| (x * y + 255) >> 8,
        |x, mut y| {
            y += y >> 7;
            (x * y + 128) >> 8
        },
    ];
    for multiply in fine {
        reporter_assert!(r, acceptable(&test(multiply)));
    }

    // These are pefect.
    let perfect: [fn(i32, i32) -> i32; 3] = [
        |x, y| (x * y + 127) / 255, // Duh.
        |x, y| {
            let p = x * y + 128;
            (p + (p >> 8)) >> 8
        },
        |x, y| ((x * y + 128) * 257) >> 16,
    ];
    for multiply in perfect {
        reporter_assert!(r, test(multiply).diffs == 0);
    }
});
