// Copyright 2020 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ExtendedSkColorTypeTests.cpp (chrome/m156)
//
// Only the raster test is ported; `ExtendedSkColorTypeTests_gpu` (`ganesh_tests`) is Ganesh-only.

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::{Color4f, ColorChannelFlag, colors};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_info_priv::color_type_channel_flags;
use skia_rust_core::images;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_raster::surfaces;

use crate::{Reporter, def_tier_test, reporter_assert};

const SIZE: i32 = 32;

// Port of: tests/ExtendedSkColorTypeTests.cpp#L55-L62 (chrome/m156)
fn get_trans_black_expected_color(channels: ColorChannelFlag) -> Color4f {
    let mut a = 0.0;
    if (channels & ColorChannelFlag::ALPHA).bits() == 0 {
        a = 1.0;
    }

    Color4f::new(0.0, 0.0, 0.0, a)
}

// Port of: tests/ExtendedSkColorTypeTests.cpp#L64-L81 (chrome/m156)
fn get_opaque_white_expected_color(channels: ColorChannelFlag) -> Color4f {
    if (channels & ColorChannelFlag::GRAY).bits() != 0 {
        return Color4f::new(1.0, 1.0, 1.0, 1.0);
    }

    let mut r = 1.0;
    let mut g = 1.0;
    let mut b = 1.0;
    if (channels & ColorChannelFlag::RED).bits() == 0 {
        r = 0.0;
    }
    if (channels & ColorChannelFlag::GREEN).bits() == 0 {
        g = 0.0;
    }
    if (channels & ColorChannelFlag::BLUE).bits() == 0 {
        b = 0.0;
    }

    Color4f::new(r, g, b, 1.0)
}

// Port of: tests/ExtendedSkColorTypeTests.cpp#L83-L88 (chrome/m156)
struct TestCase {
    color_type: ColorType,
    alpha_type: AlphaType,
    channels: ColorChannelFlag,
    #[allow(dead_code)] // only the Ganesh test reads it
    gpu_can_make_surfaces: bool,
}

// Port of: tests/ExtendedSkColorTypeTests.cpp#L90-L113 (chrome/m156)
fn g_tests() -> Vec<TestCase> {
    use AlphaType::{Opaque, Premul};
    use ColorChannelFlag as F;
    let tc = |color_type, alpha_type, channels, gpu_can_make_surfaces| TestCase {
        color_type,
        alpha_type,
        channels,
        gpu_can_make_surfaces,
    };
    vec![
        tc(ColorType::Alpha8, Premul, F::ALPHA, true),
        tc(ColorType::A16UNorm, Premul, F::ALPHA, false),
        tc(ColorType::A16Float, Premul, F::ALPHA, false),
        tc(ColorType::RGB565, Opaque, F::RGB, true),
        tc(ColorType::ARGB4444, Premul, F::RGBA, true),
        tc(ColorType::RGBA8888, Premul, F::RGBA, true),
        tc(ColorType::RGB888x, Opaque, F::RGB, true),
        tc(ColorType::BGRA8888, Premul, F::RGBA, true),
        tc(ColorType::RGBA1010102, Premul, F::RGBA, true),
        tc(ColorType::RGB101010x, Opaque, F::RGB, true),
        tc(ColorType::Gray8, Opaque, F::GRAY, true),
        tc(ColorType::RGBAF16Norm, Premul, F::RGBA, true),
        tc(ColorType::RGBAF16, Premul, F::RGBA, true),
        tc(ColorType::RGBF16F16F16x, Opaque, F::RGB, true),
        tc(ColorType::RGBAF32, Premul, F::RGBA, true),
        tc(ColorType::R8G8UNorm, Opaque, F::RG, true),
        tc(ColorType::R16UNorm, Opaque, F::RED, false),
        tc(ColorType::R16G16UNorm, Opaque, F::RG, false),
        tc(ColorType::R16Float, Opaque, F::RED, false),
        tc(ColorType::R16G16Float, Opaque, F::RG, false),
        tc(ColorType::R16G16B16A16UNorm, Premul, F::RGBA, false),
    ]
}

// Port of: tests/ExtendedSkColorTypeTests.cpp#L115-L188 (chrome/m156)
fn raster_tests(reporter: &mut Reporter, test: &TestCase) {
    let native_ii = ImageInfo::new((SIZE, SIZE), test.color_type, test.alpha_type, None);
    let f32_unpremul = ImageInfo::new((SIZE, SIZE), ColorType::RGBAF32, AlphaType::Unpremul, None);

    let actual_channels = color_type_channel_flags(test.color_type);
    reporter_assert!(reporter, test.channels == actual_channels);

    // all colorTypes can be drawn to
    {
        let s = surfaces::raster(&native_ii, None, None);
        reporter_assert!(reporter, s.is_some());
    }

    // `SkAutoPixmapStorage`: pixels allocated and zeroed for `info`.
    let storage = |info: &ImageInfo| vec![0_u8; info.compute_min_byte_size()];

    // opaque formats should make transparent black become opaque
    {
        let mut bytes = storage(&native_ii);
        let mut pm =
            Pixmap::new(&native_ii, &mut bytes, native_ii.min_row_bytes()).expect("a pixmap");
        let _ = pm.erase_4f(colors::TRANSPARENT, None);
        let actual = pm.get_color((0, 0));
        let expected = get_trans_black_expected_color(test.channels);
        reporter_assert!(reporter, expected.to_color() == actual);
    }

    // unused channels should drop out
    {
        let mut bytes = storage(&native_ii);
        let mut pm =
            Pixmap::new(&native_ii, &mut bytes, native_ii.min_row_bytes()).expect("a pixmap");
        let _ = pm.erase_4f(colors::WHITE, None);
        let actual = pm.get_color((0, 0));
        let expected = get_opaque_white_expected_color(test.channels);
        reporter_assert!(reporter, expected.to_color() == actual);
    }

    // Reading back from an image to the same colorType should always work
    {
        let mut src_bytes = storage(&native_ii);
        let mut src_pm =
            Pixmap::new(&native_ii, &mut src_bytes, native_ii.min_row_bytes()).expect("a pixmap");
        let _ = src_pm.erase_4f(colors::WHITE, None);
        let i = images::raster_from_pixmap(&src_pm, || {});
        reporter_assert!(reporter, i.is_some());
        let Some(i) = i else {
            return;
        };

        let mut readback_bytes = storage(&native_ii);
        let mut readback_pm =
            Pixmap::new(&native_ii, &mut readback_bytes, native_ii.min_row_bytes())
                .expect("a pixmap");
        let _ = readback_pm.erase_4f(colors::TRANSPARENT, None);

        reporter_assert!(reporter, i.read_pixels_to_pixmap(&mut readback_pm, (0, 0)));

        let expected = src_pm.get_color((0, 0));
        let actual = readback_pm.get_color((0, 0));
        reporter_assert!(reporter, expected == actual);
    }

    // Rendering to an F32 surface should always work
    {
        let mut src_bytes = storage(&native_ii);
        let mut src_pm =
            Pixmap::new(&native_ii, &mut src_bytes, native_ii.min_row_bytes()).expect("a pixmap");
        let _ = src_pm.erase_4f(colors::WHITE, None);
        let i = images::raster_from_pixmap(&src_pm, || {});
        reporter_assert!(reporter, i.is_some());
        let Some(i) = i else {
            return;
        };

        let s = surfaces::raster(&f32_unpremul, None, None);
        reporter_assert!(reporter, s.is_some());
        let Some(mut s) = s else {
            return;
        };

        {
            let c = s.canvas();
            c.draw_image(&i, (0.0, 0.0), None);
        }

        let mut readback_bytes = storage(&f32_unpremul);
        let mut readback_pm = Pixmap::new(
            &f32_unpremul,
            &mut readback_bytes,
            f32_unpremul.min_row_bytes(),
        )
        .expect("a pixmap");
        let _ = readback_pm.erase_4f(colors::TRANSPARENT, None);

        reporter_assert!(reporter, i.read_pixels_to_pixmap(&mut readback_pm, (0, 0)));

        let expected = src_pm.get_color((0, 0));
        let actual = readback_pm.get_color((0, 0));
        reporter_assert!(reporter, expected == actual);
    }
}

// Port of: tests/ExtendedSkColorTypeTests.cpp#L355-L358 (chrome/m156)
def_tier_test!(ExtendedSkColorTypeTests_raster, |reporter| {
    for test in &g_tests() {
        raster_tests(reporter, test);
    }
});
