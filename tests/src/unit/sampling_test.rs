// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SamplingTest.cpp (chrome/m156)

#![cfg(test)]

use crate::resources::get_resource_as_image;
use crate::tools::tool_utils::equal_pixels_image;
use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::random::Random;
use skia_rust_core::sampling_options::{CubicResampler, SamplingOptions};
use skia_rust_core::sampling_priv::no_change_with_identity_matrix;
use skia_rust_raster::surfaces;

// Port of: tests/SamplingTest.cpp#L13-L44 (chrome/m156), sampling_with_identity_matrix
def_test!(sampling_with_identity_matrix, |r| {
    const N: usize = 30; // try a bunch of random values
    let names = ["images/mandrill_128.png", "images/color_wheel.jpg"];

    let mut rand = Random::default();
    for name in names {
        let src = get_resource_as_image(name).expect("resource image");
        let mut surf = surfaces::raster_n32_premul((src.width(), src.height())).expect("surface");

        let dotest = |surf: &mut skia_rust_raster::surface::Surface<'_>,
                      r: &mut Reporter,
                      sampling: SamplingOptions,
                      expect_same: bool| {
            surf.canvas().clear(0u32);
            surf.canvas()
                .draw_image_with_sampling_options(&src, (0, 0), sampling, None);
            let dst = surf.image_snapshot().expect("snapshot");

            reporter_assert!(r, no_change_with_identity_matrix(&sampling) == expect_same);
            reporter_assert!(r, equal_pixels_image(&src, &dst) == expect_same);
        };

        // Exercise all non-cubics -- expecting no changes
        for m in [
            skia_rust_core::sampling_options::MipmapMode::None,
            skia_rust_core::sampling_options::MipmapMode::Nearest,
            skia_rust_core::sampling_options::MipmapMode::Linear,
        ] {
            for f in [
                skia_rust_core::sampling_options::FilterMode::Nearest,
                skia_rust_core::sampling_options::FilterMode::Linear,
            ] {
                dotest(&mut surf, r, SamplingOptions::new(f, m), true);
            }
        }

        // Exercise cubic variants with B zero and non-zero
        for _ in 0..N {
            let c = rand.next_f();
            dotest(
                &mut surf,
                r,
                SamplingOptions::from(CubicResampler { b: 0.0, c }),
                true,
            );

            let b = rand.next_f() * 0.9 + 0.05; // non-zero but still within (0,,,1]
            debug_assert!(b != 0.0);
            dotest(
                &mut surf,
                r,
                SamplingOptions::from(CubicResampler { b, c }),
                false,
            );
        }
    }
});
