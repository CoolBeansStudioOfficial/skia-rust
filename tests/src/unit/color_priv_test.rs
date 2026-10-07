// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ColorPrivTest.cpp (chrome/m156)

use skia_rust_core::color::{Color, PMColor, pre_multiply_color};
use skia_rust_core::color_data::{
    blend_argb32, fast_four_byte_interp, fast_four_byte_interp256, four_byte_interp,
    four_byte_interp256, splay, splay64, unsplay, unsplay64,
};
use skia_rust_core::color_priv::{get_packed_a32, get_packed_b32, get_packed_g32, get_packed_r32};

use crate::{def_test, reporter_assert};

// `#define ASSERT(expr) REPORTER_ASSERT(r, expr)` is expanded inline below.

// Port of: tests/ColorPrivTest.cpp#L23-L36 (chrome/m156)
def_test!(Splay, |r| {
    let color: PMColor = 0xA1B2_C3D4;

    let (ag, rb) = splay(color);
    reporter_assert!(r, ag == 0x00A1_00C3);
    reporter_assert!(r, rb == 0x00B2_00D4);
    reporter_assert!(r, unsplay(ag << 8, rb << 8) == color);

    let agrb = splay64(color);
    reporter_assert!(r, agrb == 0x00A1_00C3_00B2_00D4u64);
    reporter_assert!(r, unsplay64(agrb << 8) == color);
});

// Port of: tests/ColorPrivTest.cpp#L38-L61 (chrome/m156)
def_test!(FourByteInterp, |r| {
    let (src, dst): (PMColor, PMColor) = (0xAB99_8877, 0x6633_4455);
    for scale in 0..=256u32 {
        reporter_assert!(
            r,
            four_byte_interp256(src, dst, i32::try_from(scale).unwrap())
                == fast_four_byte_interp256(src, dst, scale)
        );
    }

    for scale in 0..256u32 {
        // SkFourByteInterp and SkFastFourByteInterp convert from [0, 255] to [0, 256] differently.
        // In particular, slow may end up a little too high (weirdly, fast is more accurate).
        let slow = four_byte_interp(src, dst, scale);
        let fast = fast_four_byte_interp(src, dst, scale);

        let delta_a = i64::from(get_packed_a32(slow)) - i64::from(get_packed_a32(fast));
        let delta_r = i64::from(get_packed_r32(slow)) - i64::from(get_packed_r32(fast));
        let delta_g = i64::from(get_packed_g32(slow)) - i64::from(get_packed_g32(fast));
        let delta_b = i64::from(get_packed_b32(slow)) - i64::from(get_packed_b32(fast));

        reporter_assert!(r, delta_a == 0 || delta_a == 1);
        reporter_assert!(r, delta_r == 0 || delta_r == 1);
        reporter_assert!(r, delta_g == 0 || delta_g == 1);
        reporter_assert!(r, delta_b == 0 || delta_b == 1);
    }
});

// Port of: tests/ColorPrivTest.cpp#L63-L81 (chrome/m156)
def_test!(
    SkBlendARGB32_SameOpaqueSrcAndDstWithAnyAlpha_ProducesSameColor,
    |r| {
        let colors = [
            Color::WHITE,
            Color::BLACK,
            Color::GRAY,
            Color::RED,
            Color::GREEN,
            Color::BLUE,
            Color::YELLOW,
            Color::CYAN,
            Color::MAGENTA,
            Color::from_argb(255, 255 / 4, 255 / 3, 255 / 2),
            // https://bugzilla.mozilla.org/show_bug.cgi?id=1200684 and https://codereview.chromium.org/2097883002
            Color::new(0xFF2E_3338),
        ];

        for c in colors {
            debug_assert!(c.a() == 0xFF, "this is only true for opaque colors");

            let src_and_dst = pre_multiply_color(c);

            for scale in 0..256u32 {
                let alpha = scale;

                let output = blend_argb32(src_and_dst, src_and_dst, alpha);
                reporter_assert!(
                    r,
                    src_and_dst == output,
                    "SkBlendARGB32(0x{src_and_dst:08x}, 0x{src_and_dst:08x}, {alpha:02x}) = {output:08x} instead of the original"
                );
            }
        }
    }
);
