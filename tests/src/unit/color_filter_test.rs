// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ColorFilterTest.cpp (chrome/m156)
//
// `WorkingFormatFilterFlags` needs `SkWorkingFormatColorFilter`, which is not ported; it is tracked
// as `todo` in the manifest.

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters;
use skia_rust_core::paint::Paint;
use skia_rust_core::random::Random;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::shaders;
use skia_rust_core::write_buffer::BinaryWriteBuffer;
use skia_rust_effects::flattenable::REGISTRY;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::{def_test, reporter_assert};

// Port of: tests/ColorFilterTest.cpp#L193-L207 (chrome/m156)
def_test!(ColorFilter_OpaqueShaderPaintAlpha, |r| {
    // skbug.com/40045529: Prior to the fix, CPU backend would produce gray, not white. (It told the
    // color filter that the shader output was opaque, ignoring the effect of paint alpha).
    let mut paint = Paint::default();
    paint.set_shader(shaders::color(Color::WHITE));
    paint.set_alpha_f(0.5);
    paint.set_color_filter(color_filters::srgb_to_linear_gamma());

    let mut bmp = Bitmap::new();
    bmp.alloc_n32_pixels((1, 1), None);
    {
        let canvas = Canvas::from_bitmap(&mut bmp, None).expect("canvas");
        canvas.draw_color(Color::WHITE, BlendMode::SrcOver);
        canvas.draw_paint(&paint);
    }
    reporter_assert!(r, bmp.get_color((0, 0)) == Color::WHITE);
});

/// `reincarnate_colorfilter`: writes `filter` as a flattenable and reads it back.
// Port of: tests/ColorFilterTest.cpp#L47-L58 (chrome/m156)
fn reincarnate_colorfilter(filter: &ColorFilter) -> Option<ColorFilter> {
    let mut wb = BinaryWriteBuffer::new();
    wb.write_color_filter(Some(filter));

    let mut storage = vec![0; wb.bytes_written()];
    wb.write_to_memory(&mut storage);

    let mut rb = ReadBuffer::new(&storage);
    rb.read_color_filter(&REGISTRY)
}

// Port of: tests/ColorFilterTest.cpp#L64-L118 (chrome/m156), ColorFilter
def_test!(ColorFilter, |reporter| {
    let mut rand = Random::default();

    for mode in BlendMode::VALUES {
        let color = Color::new(rand.next_u());

        // ensure we always get a filter, by avoiding the possibility of a
        // special case that would return nullptr (if color's alpha is 0 or 0xFF)
        // SkColorSetA(color, 0x7F)
        let color = color.with_a(0x7F);

        let cf = color_filters::blend_color(color, mode);

        // allow for no filter if we're in Dst mode (its a no op)
        if mode == BlendMode::Dst && cf.is_none() {
            continue;
        }

        reporter_assert!(reporter, cf.is_some());
        let Some(cf) = cf else {
            continue;
        };

        // SkColor c = ~color;
        let c_not = Color::from_argb(!color.a(), !color.r(), !color.g(), !color.b());
        // ILLEGAL_MODE: `asAColorMode` failing leaves `c` and `m` unset, which the checks catch.
        let (mut c, mut m) = (c_not, None);

        let mut expected_color = color;
        let mut expected_mode = mode;

        let got = cf.to_a_color_mode();
        reporter_assert!(reporter, got.is_some());
        if let Some((got_color, got_mode)) = got {
            c = got_color;
            m = Some(got_mode);
        }

        // handle special-case folding by the factory
        if mode == BlendMode::Clear {
            if c != expected_color {
                expected_color = Color::new(0);
            }
            if m != Some(expected_mode) {
                expected_mode = BlendMode::Src;
            }
        }

        reporter_assert!(reporter, c == expected_color);
        reporter_assert!(reporter, m == Some(expected_mode));

        {
            let cf2 = reincarnate_colorfilter(&cf);
            reporter_assert!(reporter, cf2.is_some());

            let mut c2 = c_not;
            let mut m2 = None;
            if let Some((got_color, got_mode)) = cf2.as_ref().and_then(ColorFilter::to_a_color_mode)
            {
                c2 = got_color;
                m2 = Some(got_mode);
            }
            reporter_assert!(reporter, c2 == expected_color);
            reporter_assert!(reporter, m2 == Some(expected_mode));
        }
    }
});
