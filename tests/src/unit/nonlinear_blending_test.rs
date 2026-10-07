// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/NonlinearBlendingTest.cpp (chrome/m156)

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::{ColorSpace, named_gamut, named_transfer_fn};
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_skcms::{self as skcms, AlphaFormat, PixelFormat};

use crate::{def_test, reporter_assert};

// Reads four native-endian floats (an RGBA_ffff pixel).
fn floats_from_bytes(bytes: &[u8; 16]) -> [f32; 4] {
    let mut out = [0.0f32; 4];
    for (i, v) in out.iter_mut().enumerate() {
        *v = f32::from_ne_bytes([
            bytes[4 * i],
            bytes[4 * i + 1],
            bytes[4 * i + 2],
            bytes[4 * i + 3],
        ]);
    }
    out
}

// Writes four native-endian floats (an RGBA_ffff pixel).
fn bytes_from_floats(floats: &[f32; 4]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (i, v) in floats.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&v.to_ne_bytes());
    }
    out
}

// Port of: tests/NonlinearBlendingTest.cpp#L18-L75 (chrome/m156)
def_test!(
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // mirrors REPORTER_ASSERT(r, a <= b) on floats
    SkColorSpaceXformSteps_vs_skcms,
    |r| {
        let srgb = ColorSpace::new_srgb();
        let dp3 = ColorSpace::new_rgb(&named_transfer_fn::SRGB, &named_gamut::DISPLAY_P3)
            .expect("a valid color space");

        let srgb_profile = srgb.to_profile();
        let dp3_profile = dp3.to_profile();

        // These colors provide a good spread of interesting test cases.
        let colors = [
            Color::new(0xffff_0000),
            Color::new(0x7fff_0000),
            Color::new(0x7f7f_0000),
            Color::new(0xff00_ff00),
            Color::new(0x7f00_ff00),
            Color::new(0x7f00_7f00),
        ];

        for color in colors {
            let bgra = PixelFormat::Bgra8888;
            let f32_ = PixelFormat::RgbaFfff;
            let unpremul = AlphaFormat::Unpremul;
            let premul = AlphaFormat::PremulAsEncoded;

            let color_bytes = u32::from(color).to_ne_bytes();
            let mut via_skcms_bytes = [0u8; 16];
            let _ = skcms::transform(
                &color_bytes,
                bgra,
                unpremul,
                Some(&srgb_profile),
                &mut via_skcms_bytes,
                f32_,
                premul,
                Some(&dp3_profile),
                1,
            );
            let via_skcms = floats_from_bytes(&via_skcms_bytes);

            let steps = ColorSpaceXformSteps::new(
                Some(&srgb),
                AlphaType::Unpremul,
                Some(&dp3),
                AlphaType::Premul,
            );
            let mut via_steps = [
                f32::from(color.r()) * (1.0 / 255.0f32),
                f32::from(color.g()) * (1.0 / 255.0f32),
                f32::from(color.b()) * (1.0 / 255.0f32),
                f32::from(color.a()) * (1.0 / 255.0f32),
            ];
            steps.apply(&mut via_steps);

            reporter_assert!(r, (via_skcms[0] - via_steps[0]).abs() <= 0.005);
            reporter_assert!(r, (via_skcms[1] - via_steps[1]).abs() <= 0.005);
            reporter_assert!(r, (via_skcms[2] - via_steps[2]).abs() <= 0.005);
            reporter_assert!(r, (via_skcms[3] - via_steps[3]).abs() <= 0.0);

            // Now go back using the other method's inverse transform
            let via_steps_bytes = bytes_from_floats(&via_steps);
            let mut steps_to_skcms_bytes = [0u8; 16];
            let _ = skcms::transform(
                &via_steps_bytes,
                f32_,
                premul,
                Some(&dp3_profile),
                &mut steps_to_skcms_bytes,
                f32_,
                premul,
                Some(&srgb_profile),
                1,
            );
            let steps_to_skcms = floats_from_bytes(&steps_to_skcms_bytes);

            let mut skcms_to_steps = [via_skcms[0], via_skcms[1], via_skcms[2], via_skcms[3]];
            let inv_steps = ColorSpaceXformSteps::new(
                Some(&dp3),
                AlphaType::Premul,
                Some(&srgb),
                AlphaType::Premul,
            );
            inv_steps.apply(&mut skcms_to_steps);

            reporter_assert!(r, (skcms_to_steps[0] - steps_to_skcms[0]).abs() <= 0.005);
            reporter_assert!(r, (skcms_to_steps[1] - steps_to_skcms[1]).abs() <= 0.005);
            reporter_assert!(r, (skcms_to_steps[2] - steps_to_skcms[2]).abs() <= 0.005);
            reporter_assert!(r, (skcms_to_steps[3] - steps_to_skcms[3]).abs() <= 0.0);
        }
    }
);
