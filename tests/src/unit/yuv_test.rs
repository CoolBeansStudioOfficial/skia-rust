// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/YUVTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::image_info::YUVColorSpace;
use skia_rust_core::scalar::{Scalar, scalar};

use crate::{def_test, reporter_assert};

// Port of: tests/YUVTest.cpp#L212-L240 (chrome/m156)
def_test!(YUVMath, |reporter| {
    let spaces = [
        YUVColorSpace::JPEGFull,
        YUVColorSpace::Rec601Limited,
        YUVColorSpace::Rec709Full,
        YUVColorSpace::BT2020_8BitFull,
        YUVColorSpace::Identity,
    ];

    // Not sure what the theoretical precision we can hope for is, so pick a big value that
    // passes (when I think we're correct).
    let tolerance: f32 = 1.0 / (1 << 18) as f32;

    for cs in spaces {
        let mut r2ym = ColorMatrix::rgb_to_yuv(cs);
        let y2rm = ColorMatrix::yuv_to_rgb(cs);
        r2ym.post_concat(&y2rm);

        let mut tmp = [0.0f32; 20];
        r2ym.get_row_major(&mut tmp);
        for (i, &value) in tmp.iter().enumerate() {
            // diagonal
            let expected = if i % 6 == 0 { 1.0 } else { 0.0 };
            reporter_assert!(reporter, scalar::nearly_equal(value, expected, tolerance));
        }
    }
});
