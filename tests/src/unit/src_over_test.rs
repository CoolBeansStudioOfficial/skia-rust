// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SrcOverTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::color_priv::{alpha_255_to_256, alpha_mul};
use skia_rust_core::math::mul_div_255_round;

use crate::{def_test, reporter_assert};

// our std SkAlpha255To256
// Port of: tests/SrcOverTest.cpp#L16-L19 (chrome/m156)
#[allow(clippy::cast_possible_wrap)] // the values are at most 256
fn test_srcover0(dst: u32, alpha: u32) -> i32 {
    alpha as i32 + alpha_mul(dst as i32, alpha_255_to_256(255 - alpha) as i32)
}

// faster hack +1
// Port of: tests/SrcOverTest.cpp#L21-L24 (chrome/m156)
#[allow(clippy::cast_possible_wrap)] // the values are at most 256
fn test_srcover1(dst: u32, alpha: u32) -> i32 {
    alpha as i32 + alpha_mul(dst as i32, 256 - alpha as i32)
}

// slower "correct"
// Port of: tests/SrcOverTest.cpp#L26-L29 (chrome/m156)
#[allow(clippy::cast_possible_wrap)] // the values are at most 255
fn test_srcover2(dst: u32, alpha: u32) -> i32 {
    alpha as i32 + mul_div_255_round(dst, 255 - alpha) as i32
}

// Port of: tests/SrcOverTest.cpp#L29-L78 (chrome/m156)
def_test!(
    #[allow(clippy::cast_sign_loss)] // the results are in 0..=255 + 255 and compared as unsigned
    SrcOver,
    |reporter| {
        /*  Here's the idea. Can we ensure that when we blend on top of an opaque
           dst, that the result always stay's opaque (i.e. exactly 255)?
        */

        let mut opaque_counter0 = 0;
        let mut opaque_counter1 = 0;
        let mut opaque_counter2 = 0;
        for i in 0..=255u32 {
            let result0 = test_srcover0(0xFF, i) as u32;
            let result1 = test_srcover1(0xFF, i) as u32;
            let result2 = test_srcover2(0xFF, i) as u32;
            opaque_counter0 += i32::from(result0 == 0xFF);
            opaque_counter1 += i32::from(result1 == 0xFF);
            opaque_counter2 += i32::from(result2 == 0xFF);
        }
        // we acknowledge that technique0 does not always return opaque
        reporter_assert!(reporter, opaque_counter0 == 256);
        reporter_assert!(reporter, opaque_counter1 == 256);
        reporter_assert!(reporter, opaque_counter2 == 256);

        // Now ensure that we never over/underflow a byte
        for i in 0..=255u32 {
            for dst in 0..=255u32 {
                let r0 = test_srcover0(dst, i) as u32;
                let r1 = test_srcover1(dst, i) as u32;
                let r2 = test_srcover2(dst, i) as u32;
                let max = dst.max(i);
                // ignore the known failure
                if dst != 255 {
                    reporter_assert!(reporter, r0 <= 255 && r0 >= max);
                }
                reporter_assert!(reporter, r1 <= 255 && r1 >= max);
                reporter_assert!(reporter, r2 <= 255 && r2 >= max);
            }
        }
    }
);
