// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RuntimeBlendTest.cpp (chrome/m156)

//! `RuntimeBlendTest`: runtime blenders must blend exactly as the built-in blend modes do.
//!
//! The helper `GetRuntimeBlendForBlendMode` is `tools/RuntimeBlendUtils.cpp`. The Ganesh variant
//! needs that backend and is not ported.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{RuntimeEffect, RuntimeShaderBuilder};
use skia_rust_core::shaders;
use skia_rust_core::canvas::Canvas;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::surface_graphite::Surface as GraphiteSurface;
use skia_rust_gpu::graphite::wgpu::WgpuContext;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

use crate::{Reporter, def_graphite_adapter_test, def_test, reporter_assert};

// Port of: tests/RuntimeBlendTest.cpp#L39-L45 (chrome/m156)
fn nearly_equal(x: Color, y: Color) -> bool {
    // (Skia's own tolerance: the runtime blend must match the built-in one to within one unit.)
    let k_tolerance = 1;
    (i32::from(x.a()) - i32::from(y.a())).abs() <= k_tolerance
        && (i32::from(x.r()) - i32::from(y.r())).abs() <= k_tolerance
        && (i32::from(x.g()) - i32::from(y.g())).abs() <= k_tolerance
        && (i32::from(x.b()) - i32::from(y.b())).abs() <= k_tolerance
}

// Port of: tools/RuntimeBlendUtils.cpp#L8-L20 (chrome/m156)
fn get_runtime_blend_for_blend_mode(mode: BlendMode) -> Blender {
    let result = RuntimeEffect::make_for_blender(
        "uniform blender b;\
         half4 main(half4 src, half4 dst) {\
             return b.eval(src, dst);\
         }",
        None,
    );
    let effect = result.expect("the blend effect compiles");
    let mut builder = RuntimeShaderBuilder::new(effect);
    builder.child("b").assign(Blender::mode(mode));
    builder
        .make_blender()
        .expect("the blend effect makes a blender")
}

/// The surface `SkSurface*` that `test_blend` draws on and reads back, so the raster and Graphite
/// variants share its body.
trait BlendTarget {
    fn image_info(&self) -> ImageInfo;
    fn canvas(&mut self) -> &Canvas;
    /// `surface->readPixels(bitmap.info(), bitmap.getPixels(), bitmap.rowBytes(), 0, 0)`.
    fn read_pixels(&mut self, bitmap: &mut Bitmap) -> bool;
}

impl BlendTarget for Surface<'_> {
    fn image_info(&self) -> ImageInfo {
        Surface::image_info(self)
    }
    fn canvas(&mut self) -> &Canvas {
        Surface::canvas(self)
    }
    fn read_pixels(&mut self, bitmap: &mut Bitmap) -> bool {
        self.read_pixels_to_bitmap(bitmap, (0, 0))
    }
}

/// A Graphite surface with the context that reads it back.
struct GraphiteBlendTarget<'a> {
    context: &'a mut WgpuContext,
    surface: &'a GraphiteSurface,
}

impl BlendTarget for GraphiteBlendTarget<'_> {
    fn image_info(&self) -> ImageInfo {
        self.surface.image_info().clone()
    }
    fn canvas(&mut self) -> &Canvas {
        self.surface.canvas()
    }
    fn read_pixels(&mut self, bitmap: &mut Bitmap) -> bool {
        let Some(mut pm) = bitmap.peek_pixels_mut() else {
            return false;
        };
        self.context.read_surface_pixels(self.surface, &mut pm, 0, 0)
    }
}

// Port of: tests/RuntimeBlendTest.cpp#L47-L106 (chrome/m156)
fn test_blend(r: &mut Reporter, surface: &mut dyn BlendTarget) {
    let mut bitmap = Bitmap::new();
    reporter_assert!(r, bitmap.try_alloc_pixels_info(&surface.image_info(), None));

    for m in 0..BlendMode::COUNT {
        let mode = BlendMode::VALUES[m];
        for alpha in [0x80_u8, 0xFF] {
            for use_shader in [false, true] {
                let mut colors_out: Vec<Color> = Vec::new();
                for use_runtime_blend in [false, true] {
                    // Draw a solid red pixel.
                    let mut paint = Paint::default();
                    paint.set_color(Color::RED);
                    paint.set_blend_mode(BlendMode::Src);
                    surface.canvas().draw_rect(Rect::from_wh(1.0, 1.0), &paint);

                    // Draw a blue pixel on top of it, using the passed-in blend mode.
                    if use_shader {
                        // Install a different color in the paint, to ensure we're using the shader
                        paint.set_color(Color::GREEN);
                        paint.set_shader(shaders::color(Color::from_argb(alpha, 0x00, 0x00, 0xFF)));
                    } else {
                        paint.set_color(Color::from_argb(alpha, 0x00, 0x00, 0xFF));
                    }
                    if use_runtime_blend {
                        paint.set_blender(get_runtime_blend_for_blend_mode(mode));
                    } else {
                        paint.set_blend_mode(mode);
                    }
                    surface.canvas().draw_rect(Rect::from_wh(1.0, 1.0), &paint);

                    // Read back the red/blue blended pixel.
                    reporter_assert!(r, surface.read_pixels(&mut bitmap));
                    colors_out.push(bitmap.get_color((0, 0)));
                }

                reporter_assert!(
                    r,
                    nearly_equal(colors_out[0], colors_out[1]),
                    "Expected: {:?} {} {} blend matches. Actual: Built-in A={:02X} R={:02X} \
                     G={:02X} B={:02X}, Runtime A={:02X} R={:02X} G={:02X} B={:02X}",
                    mode,
                    if alpha == 0xFF {
                        "solid"
                    } else {
                        "transparent"
                    },
                    if use_shader { "shader" } else { "paint" },
                    colors_out[0].a(),
                    colors_out[0].r(),
                    colors_out[0].g(),
                    colors_out[0].b(),
                    colors_out[1].a(),
                    colors_out[1].r(),
                    colors_out[1].g(),
                    colors_out[1].b()
                );
            }
        }
    }
}

// Port of: tests/RuntimeBlendTest.cpp#L108-L113 (chrome/m156)
def_test!(SkRuntimeBlender_CPU, |r| {
    let info = ImageInfo::new((1, 1), ColorType::N32, AlphaType::Premul, None);
    let mut surface = surfaces::raster(&info, None, None).expect("a raster surface");

    test_blend(r, &mut surface);
});

// Port of: tests/RuntimeBlendTest.cpp#L127-L140 (chrome/m156)
def_graphite_adapter_test!(SkRuntimeBlender_Graphite, |reporter, context| {
    let recorder = context.make_recorder(None);

    let info = ImageInfo::new_n32_premul((1, 1), None);
    let surface = GraphiteSurface::render_target(&recorder, &info, Mipmapped::No, None, "");
    reporter_assert!(reporter, surface.is_some());
    let Some(surface) = surface else {
        return;
    };

    test_blend(
        reporter,
        &mut GraphiteBlendTarget {
            context,
            surface: &surface,
        },
    );
});
