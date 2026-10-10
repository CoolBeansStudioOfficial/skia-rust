// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/F16DrawTest.cpp (chrome/m156)
//
// The Ganesh variant (`F16DrawTest_Ganesh`) needs that backend and is excluded.

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::colors;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::surface_graphite::Surface as GraphiteSurface;

use crate::tools::test_surface::{GraphiteTestSurface, TestSurface};
use crate::{Reporter, def_graphite_adapter_test, errorf, reporter_assert};

// Port of: tests/F16DrawTest.cpp#L26-L93 (chrome/m156)
// Tests that draws to an F16 surface blend as expected.
// The length mirrors the C++ function, whose expectation table is part of it.
#[allow(clippy::too_many_lines)]
fn test_f16(reporter: &mut Reporter, surface: &mut dyn TestSurface) {
    // Some blend modes and their corresponding expected red channel output when blending premul src
    // (2, 0, 0, 0) with dst (0, 0, 0, 0) on an F16 surface.
    const K_HALF_FLOAT0: u16 = 0x0000;
    const K_HALF_FLOAT1: u16 = 0x3c00;
    const K_HALF_FLOAT2: u16 = 0x4000;
    struct Expectation {
        blend_mode: BlendMode,
        red: u16,
    }
    // C++ `kExpectations[19]` has 11 initializers: the last 8 are value-initialized, which is
    // `{ SkBlendMode::kClear, 0 }`.
    let k_expectations: [Expectation; 19] = [
        Expectation {
            blend_mode: BlendMode::Clear,
            red: K_HALF_FLOAT0,
        },
        Expectation {
            blend_mode: BlendMode::Src,
            red: K_HALF_FLOAT2,
        },
        Expectation {
            blend_mode: BlendMode::Dst,
            red: K_HALF_FLOAT0,
        },
        Expectation {
            blend_mode: BlendMode::SrcOver,
            red: K_HALF_FLOAT2,
        },
        Expectation {
            blend_mode: BlendMode::DstOver,
            red: K_HALF_FLOAT2,
        },
        Expectation {
            blend_mode: BlendMode::SrcIn,
            red: K_HALF_FLOAT0,
        },
        Expectation {
            blend_mode: BlendMode::DstIn,
            red: K_HALF_FLOAT0,
        },
        Expectation {
            blend_mode: BlendMode::SrcOut,
            red: K_HALF_FLOAT2,
        },
        Expectation {
            blend_mode: BlendMode::DstOut,
            red: K_HALF_FLOAT0,
        },
        Expectation {
            blend_mode: BlendMode::Plus,
            red: K_HALF_FLOAT1,
        },
        Expectation {
            blend_mode: BlendMode::Screen,
            red: K_HALF_FLOAT2,
        },
        Expectation {
            blend_mode: BlendMode::Clear,
            red: 0,
        },
        Expectation {
            blend_mode: BlendMode::Clear,
            red: 0,
        },
        Expectation {
            blend_mode: BlendMode::Clear,
            red: 0,
        },
        Expectation {
            blend_mode: BlendMode::Clear,
            red: 0,
        },
        Expectation {
            blend_mode: BlendMode::Clear,
            red: 0,
        },
        Expectation {
            blend_mode: BlendMode::Clear,
            red: 0,
        },
        Expectation {
            blend_mode: BlendMode::Clear,
            red: 0,
        },
        Expectation {
            blend_mode: BlendMode::Clear,
            red: 0,
        },
    ];

    // The surface is created by the caller with the F16 image info (`imageInfo`).
    let image_info = ImageInfo::new((1, 1), ColorType::RGBAF16, AlphaType::Premul, None);

    for expectation in &k_expectations {
        // Draw to the F16 surface.
        let mut paint = Paint::default();
        let effect = RuntimeEffect::make_for_shader(
            "float4 main(vec2 xy) {
                return float4(2.0, 0.0, 0.0, 0.0);
            }",
            None,
        )
        .expect("the F16 effect compiles");
        paint.set_shader(effect.make_shader(Data::new_empty(), &[], None));
        paint.set_blend_mode(expectation.blend_mode);
        surface.canvas().clear(colors::TRANSPARENT);
        surface.canvas().draw_paint(&paint);

        // Read pixels.
        let mut bitmap = Bitmap::new();
        bitmap.alloc_pixels_info(&image_info, None);
        if !surface.read_pixels(&mut bitmap) {
            errorf!(reporter, "readPixels failed");
            return;
        }

        // Check that the correct color was drawn: `channels[0]`, the first half-float of the
        // pixels, in the host's byte order as the C++ reads it.
        let Some(pixmap) = bitmap.peek_pixels() else {
            return;
        };
        let Some(addr) = pixmap.addr() else {
            return;
        };
        let actual = u16::from_ne_bytes([addr[0], addr[1]]);
        let expected = expectation.red;
        reporter_assert!(
            reporter,
            actual == expected,
            "Wrong color with blend mode {:?}, expected {:04x}, found {:04x}",
            expectation.blend_mode,
            expected,
            actual
        );
    }
}

// Port of: tests/F16DrawTest.cpp#L110-L119 (chrome/m156)
def_graphite_adapter_test!(F16DrawTest_Graphite, |reporter, context| {
    let recorder = context.make_recorder(None);

    // `createSurface(imageInfo)` for the F16 image info; a surface that cannot be made is
    // skipped without a failure, as in the C++.
    let image_info = ImageInfo::new((1, 1), ColorType::RGBAF16, AlphaType::Premul, None);
    let surface = GraphiteSurface::render_target(&recorder, &image_info, Mipmapped::No, None, "");
    let Some(surface) = surface else {
        return;
    };
    test_f16(
        reporter,
        &mut GraphiteTestSurface {
            context,
            surface: &surface,
        },
    );
});
