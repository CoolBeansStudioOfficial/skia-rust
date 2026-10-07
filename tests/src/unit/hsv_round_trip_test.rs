// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/HSVRoundTripTest.cpp (chrome/m156)

use skia_rust_core::color::Color;

use crate::{def_test, errorf};

// Port of: tests/HSVRoundTripTest.cpp#L15-L31 (chrome/m156)
def_test!(ColorToHSVRoundTrip, |reporter| {
    for r in 0..=255u8 {
        for g in 0..=255u8 {
            for b in 0..=255u8 {
                let color = Color::from_rgb(r, g, b);
                let hsv = color.to_hsv();
                let result = hsv.to_color(0xFF);
                if result != color {
                    errorf!(
                        reporter,
                        "HSV roundtrip mismatch!\n\toriginal: {:X}\n\tHSV: {:.6}, {:.6}, {:.6}\n\tresult: {:X}\n",
                        u32::from(color),
                        hsv.h,
                        hsv.s,
                        hsv.v,
                        u32::from(result)
                    );
                }
            }
        }
    }
});
