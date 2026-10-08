// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ColorFilterTest.cpp (chrome/m156)
//
// Only `ColorFilter_OpaqueShaderPaintAlpha` is ported here. `ColorFilter` needs the flattening
// round trip (`reincarnate_colorfilter`), and `WorkingFormatFilterFlags` needs
// `SkWorkingFormatColorFilter`; both are tracked as `todo` in the manifest.

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::color_filters;
use skia_rust_core::paint::Paint;
use skia_rust_core::shaders;
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
